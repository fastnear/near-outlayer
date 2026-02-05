# Worker Attestation

How NEAR OutLayer cryptographically verifies that workers run inside Intel TDX and restricts sensitive operations to attested code.

## Overview

Every worker runs inside an **Intel TDX** (Trust Domain Extension) confidential VM on Phala Cloud. Before a worker can submit execution results or decrypt user secrets, it must prove two things:

1. **Its code is genuine** — the TDX hardware measurement (RTMR3) matches an admin-approved value.
2. **It holds a TEE-generated private key** — the ed25519 keypair was created inside the TEE and the public key is registered on-chain.

These proofs happen at two distinct stages: **key registration** (on-chain, at startup) and **session establishment** (off-chain, challenge-response with coordinator and keystore).

## Architecture

```
┌─────────────────────────────────────────────────────┐
│  Worker (Intel TDX)                                 │
│                                                     │
│  1. Generate ed25519 keypair                        │
│  2. Generate TDX quote (public key in report_data)  │
│  3. Call register-contract → on-chain key            │
│  4. Challenge-response → coordinator session         │
│  5. Challenge-response → keystore session            │
│  6. Work: auth_key + X-TEE-Session on every request │
└────────┬──────────────────┬─────────────────────────┘
         │                  │
         ▼                  ▼
┌────────────────┐  ┌───────────────┐
│  Coordinator   │  │  Keystore     │
│  (PostgreSQL)  │  │  (in-memory)  │
│                │  │               │
│  verify sig    │  │  verify sig   │
│  check NEAR    │  │  check NEAR   │ ← independent RPC
│  issue session │  │  issue session│
└────────────────┘  └───────────────┘
         │
         ▼
┌─────────────────────────────┐
│  register-contract (NEAR)   │
│  Source of truth for keys   │
│  Verifies TDX quotes        │
│  Stores keys as access keys │
└─────────────────────────────┘
```

## Stage 1: On-Chain Key Registration

Happens once per worker startup. Code: `worker/src/registration.rs`.

### Steps

1. **Keypair generation.** The worker generates a fresh ed25519 keypair inside the TEE (`worker/src/registration.rs:80-99`). The private key never leaves the confidential VM. The keypair is persisted to `~/.near-credentials/worker-keypair.json` so that a soft restart reuses the same key.

2. **TDX quote generation.** The worker calls the Phala dstack SDK to produce a TDX quote (`worker/src/tdx_attestation.rs:33-54`). The worker's public key is embedded in the first 32 bytes of `report_data`, cryptographically binding the key to the TEE instance. The quote also contains **RTMR3** — a hardware measurement of the entire TEE image.

3. **On-chain verification.** The worker sends the quote to `register_worker_key()` on the register-contract (`register-contract/src/lib.rs:120-187`). The contract:
   - Verifies the Intel TDX signature using `dcap-qvl` (same library as NEAR MPC Node).
   - Extracts RTMR3 and checks it against the admin-approved list.
   - Extracts the public key from `report_data` and confirms it matches the `public_key` argument.
   - Adds the public key as an access key on its own account, scoped to `resolve_execution` and related methods on the main outlayer contract.

4. **Result.** The worker now has a NEAR access key that can only call specific methods on the outlayer contract. This key is the basis for all subsequent attestation.

### What the contract proves

| Property | How |
|----------|-----|
| Key was generated in a TEE | Public key extracted from TDX `report_data`, signed by Intel |
| TEE runs approved code | RTMR3 checked against admin-maintained allowlist |
| Key is scoped | Access key limited to specific contract methods with 10 NEAR gas allowance |

## Stage 2: TEE Session Establishment (Challenge-Response)

After on-chain registration, the worker establishes **sessions** with the coordinator and keystore using a challenge-response protocol. This proves to each service that the caller holds the private key registered on-chain — without re-verifying the full TDX quote.

Shared cryptographic logic lives in the `tee-auth` crate (`tee-auth/src/lib.rs`), used by both coordinator and keystore.

### Protocol

```
Worker                          Server (coordinator or keystore)
  │                                │
  │  POST /tee-challenge           │
  │  (bearer auth_key)             │
  │ ─────────────────────────────► │
  │                                │  Generate 32 random bytes
  │  { challenge: "a1b2c3..." }    │  Store with timestamp
  │ ◄───────────────────────────── │
  │                                │
  │  Sign challenge with TEE key   │
  │                                │
  │  POST /register-tee            │
  │  { public_key, challenge,      │
  │    signature }                 │
  │ ─────────────────────────────► │
  │                                │  1. Validate challenge (one-time, <60s)
  │                                │  2. Verify ed25519 signature
  │                                │  3. NEAR RPC view_access_key
  │                                │     → key exists on register-contract?
  │                                │  4. Create session
  │  { session_id: UUID }          │
  │ ◄───────────────────────────── │
  │                                │
  │  All subsequent requests:      │
  │  Authorization: Bearer <token> │
  │  X-TEE-Session: <session_id>   │
  │ ─────────────────────────────► │
```

### Verification steps (server-side)

1. **Challenge lookup.** Find the challenge in storage, verify it belongs to this worker's token and is less than 60 seconds old. Delete it (one-time use).

2. **Signature verification.** `tee_auth::verify_signature()` — parse the ed25519 public key, decode the hex challenge to raw bytes, verify the signature. Uses `ed25519-dalek`.

3. **On-chain key check.** `tee_auth::check_access_key_on_contract()` — NEAR RPC `view_access_key` query against the register-contract account. If the key exists, it was registered via TDX attestation. If it was removed (admin revocation), the check fails.

4. **Session creation.**
   - Coordinator: stored in PostgreSQL (`worker_tee_sessions` table), survives restarts.
   - Keystore: stored in-memory (`HashMap<Uuid, TeeSession>`), lost on restart.

### Coordinator specifics

- Endpoints: `POST /workers/tee-challenge`, `POST /workers/register-tee`
- Auth middleware (`coordinator/src/auth.rs`) extracts `X-TEE-Session` header and validates against DB.
- DB query only runs when `REQUIRE_TEE_SESSION=true` (no overhead when feature is off).
- HTTPS call handler (`coordinator/src/handlers/call.rs`) rejects results without a valid session when the feature flag is on.

### Keystore specifics

- Endpoints: `POST /tee-challenge`, `POST /register-tee` (proxied via coordinator at `/keystore/tee-challenge`, `/keystore/register-tee`)
- Keystore verifies the key on register-contract **independently** — a compromised coordinator cannot forge sessions.
- Session middleware checks `X-TEE-Session` on all worker endpoints (`/decrypt`, `/encrypt`, `/storage/*`).
- In-memory sessions are lost on keystore restart; workers detect 403 and re-register automatically (two HTTP calls, no blockchain transaction).

### Worker specifics

- Code: `worker/src/api_client.rs` — `register_tee_session()` and `register_keystore_tee_session()` both use the shared `do_tee_challenge_response()` method.
- On startup, the worker registers sessions with both coordinator and keystore (`worker/src/main.rs:311-340`).
- `ApiClient` and `KeystoreClient` both attach `X-TEE-Session` to every subsequent request via `add_auth_headers()`.
- If session registration fails, the worker logs a warning and continues (graceful degradation when `REQUIRE_TEE_SESSION=false`).

## Layers of Defense

| Layer | What it does | When |
|-------|-------------|------|
| `auth_key` (bearer token) | Anti-spam, identifies worker | Every request |
| TDX attestation (on-chain) | Proves code identity + key origin | Worker startup |
| Challenge-response (coordinator) | Proves private key possession | Session setup |
| Challenge-response (keystore) | Independent proof, not trusting coordinator | Session setup |
| `X-TEE-Session` header | Binds requests to verified identity | Every request |
| `view_access_key` (NEAR RPC) | Checks key still exists on contract | Session setup |

## Threat Model

| Threat | Mitigation |
|--------|-----------|
| Attacker knows auth_key | Cannot sign challenge — no TEE private key |
| Attacker reads public keys from chain | Cannot sign challenge |
| Attacker replays a signed challenge | Challenge is one-time use and expires in 60 seconds |
| Coordinator is compromised | Keystore verifies independently via its own NEAR RPC call |
| Worker restarts | New keypair generated → new on-chain registration → new sessions |
| Leaked worker private key | Admin calls `remove_worker_keys()` on contract → `view_access_key` returns false → no new sessions |
| Stale keys accumulate on contract | `remove_worker_keys()` deletes keys individually (independent promises) and frees ~0.042 NEAR storage per key |

## Configuration

### Coordinator

| Env var | Default | Description |
|---------|---------|-------------|
| `REGISTER_CONTRACT_ID` | (none) | Account ID of register-contract. Required for TEE session endpoints. |
| `REQUIRE_TEE_SESSION` | `false` | When `true`, HTTPS call completions require a valid `X-TEE-Session`. |

### Keystore

| Env var | Default | Description |
|---------|---------|-------------|
| `REGISTER_CONTRACT_ID` | (none) | Account ID for independent NEAR RPC verification. |
| `REQUIRE_TEE_SESSION` | `false` | When `true`, `/decrypt` and `/encrypt` require a valid `X-TEE-Session`. |

### Worker

| Env var | Default | Description |
|---------|---------|-------------|
| `USE_TEE_REGISTRATION` | `true` | Enable TDX-based key registration flow. |
| `REGISTER_CONTRACT_ID` | (none) | Account to register keys on. |
| `TEE_MODE` | `tdx` | `tdx` for production, `simulated` or `none` for dev. |

## Zero-Downtime Rollout

Deploy in this order:

1. Coordinator + keystore with `REQUIRE_TEE_SESSION=false` — new endpoints exist but are not enforced.
2. Workers — they register TEE sessions on startup. Existing workers without sessions continue to work.
3. Set `REQUIRE_TEE_SESSION=true` on coordinator and keystore — only attested workers can submit results and decrypt secrets.

## Key Cleanup

Each worker restart generates a new keypair, leaving dead access keys on the register-contract (~0.042 NEAR storage each). Clean up periodically:

```bash
near call register.outlayer.near remove_worker_keys \
  '{"public_keys": ["ed25519:...", "ed25519:..."]}' \
  --accountId outlayer.near \
  --gas 300000000000000
```

Removing a key also instantly invalidates any TEE sessions that depend on it — the next `view_access_key` check will fail.
