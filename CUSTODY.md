# Agent Custody — Developer Reference

Institutional-grade custody wallets for AI agents. An agent gets an API key to operate a multi-chain wallet. Private keys live exclusively inside a TEE (Intel TDX). The wallet owner sets policy (spending limits, whitelists, multisig, freeze) — all enforced inside the TEE. Cross-chain transfers via NEAR Intents (gasless).

---

## Architecture

```
                ┌──────────────┐               ┌───────────────┐
                │   AI Agent   │               │ Wallet Owner  │
                │ (API key)    │               │ (NEAR wallet) │
                └──────┬───────┘               └───────┬───────┘
                       │ withdraw, call, address        │ set policy, freeze
                       ▼                                ▼
              ┌─────────────────────────────────────────────────┐
              │            Coordinator (stateless proxy)         │
              │  auth API key → forward to keystore → track DB  │
              └──────────────────────┬──────────────────────────┘
                                     │
                                     ▼
              ┌─────────────────────────────────────────────────┐
              │               TEE (Intel TDX)                   │
              │                                                 │
              │  ┌────────────┐ ┌───────────┐ ┌─────────────┐  │
              │  │ Key Derivat│ │ Tx Signing│ │ Policy Eval │  │
              │  │ HMAC-SHA256│ │ Ed25519 / │ │ Decrypt from│  │
              │  │ from MPC   │ │ secp256k1 │ │ chain, check│  │
              │  │ master key │ │           │ │ all rules   │  │
              │  └────────────┘ └───────────┘ └─────────────┘  │
              └──────────────────────┬──────────────────────────┘
                                     │ submit signed tx / read policy
                                     ▼
              ┌──────────────────┐   ┌─────────────────────────┐
              │  NEAR Blockchain │   │     NEAR Intents         │
              │  policy storage  │   │  gasless cross-chain     │
              │  freeze/unfreeze │   │  NEAR, ETH, BTC, SOL    │
              └──────────────────┘   └─────────────────────────┘
```

**Coordinator is a stateless proxy.** It authenticates API keys, forwards requests to the keystore TEE, and tracks operational data in PostgreSQL. All security-critical work (key derivation, signing, policy evaluation) happens inside the TEE.

---

## Components & Key Files

### Coordinator — `coordinator/src/wallet/`

HTTP API server. Handles auth, routing, usage tracking, webhooks.

| File | Lines | Description |
|------|-------|-------------|
| [mod.rs](coordinator/src/wallet/mod.rs) | 124 | Router setup, `WalletState` struct, negative policy cache |
| [handlers.rs](coordinator/src/wallet/handlers.rs) | 3,425 | All HTTP endpoint handlers |
| [auth.rs](coordinator/src/wallet/auth.rs) | 819 | API key authentication (SHA-256 hash lookup) |
| [types.rs](coordinator/src/wallet/types.rs) | 659 | Request/response structs, error types |
| [policy.rs](coordinator/src/wallet/policy.rs) | 460 | Policy caching, NEAR RPC `has_wallet_policy()` calls |
| [backend/mod.rs](coordinator/src/wallet/backend/mod.rs) | — | `WalletBackend` trait + 1Click API types |
| [backend/intents.rs](coordinator/src/wallet/backend/intents.rs) | — | 1Click REST API (swap quotes, status polling), token list |
| [audit.rs](coordinator/src/wallet/audit.rs) | 76 | Audit log recording |
| [webhooks.rs](coordinator/src/wallet/webhooks.rs) | 278 | Webhook delivery with retry + HMAC-SHA256 |
| [idempotency.rs](coordinator/src/wallet/idempotency.rs) | 38 | Idempotency key check/store |
| [nonce.rs](coordinator/src/wallet/nonce.rs) | 74 | Per-wallet nonce mutex for concurrent withdrawals |

#### Key handler functions (handlers.rs)

| Function | Line | Description |
|----------|------|-------------|
| `register()` | 36 | Generate UUID wallet_id + API key → call keystore TEE to derive NEAR address |
| `withdraw()` | 404 | Policy check → record usage → direct ft_withdraw on intents.near |
| `withdraw_dry_run()` | 755 | Simulate withdraw: policy + balance check without execution |
| `call()` | 2186 | Native NEAR function call: policy check → keystore sign → broadcast |
| `transfer()` | 2621 | Chain-agnostic transfer (chain param, currently near only): policy → keystore sign → broadcast |
| `get_balance()` | 2983 | Chain-agnostic balance query (chain param, currently near only) via RPC |
| `intents_deposit()` | — | Deposit FT into intents.near via ft_transfer_call (auto storage deposit) |
| `swap()` | — | Swap via 1Click: quote → ft_transfer_call to intents.near → mt_transfer → poll |
| `deposit()` | 857 | Cross-chain deposit via Intents quote |
| `get_address()` | 345 | Derive address for any chain (keystore call) |
| `encrypt_policy()` | 1155 | Send policy JSON to keystore for encryption |
| `sign_policy()` | 1199 | Keystore signs encrypted policy SHA256 for on-chain verification |
| `approve()` | 1447 | Submit multisig approval (NEP-413 signature verification) |
| `reject()` | 1714 | Reject pending approval |
| `get_policy()` | 1284 | Fetch decrypted policy from keystore |
| `record_usage()` | 263 | Write spending to `wallet_usage` (daily/hourly/monthly periods) |
| `get_current_usage()` | 298 | Read current usage for velocity limit checks |
| `internal_wallet_check()` | 2623 | Worker-only: check policy for WASI execution |
| `internal_activate_policy()` | 2873 | Worker-only: activate policy after on-chain signing |
| `internal_wallet_frozen_change()` | 3106 | Sync freeze status from contract events |

### Keystore TEE — `keystore-worker/src/`

Runs inside Intel TDX. Holds master secret from NEAR MPC. All crypto happens here.

| File | Key area | Description |
|------|----------|-------------|
| [api.rs:606-612](keystore-worker/src/api.rs#L606) | Wallet routes | Router for `/wallet/*` endpoints |
| [api.rs:2439](keystore-worker/src/api.rs#L2439) | `wallet_derive_address_handler` | Derive pubkey from seed `"wallet:{wallet_id}:{chain}"` |
| [api.rs:2507](keystore-worker/src/api.rs#L2507) | `wallet_sign_transaction_handler` | Sign tx bytes with derived wallet key |
| [api.rs:2539](keystore-worker/src/api.rs#L2539) | `wallet_sign_policy_handler` | Sign encrypted policy SHA256 |
| [api.rs:2663](keystore-worker/src/api.rs#L2663) | `wallet_sign_nep413_handler` | Sign NEP-413 message for wallet auth |
| [api.rs:2740](keystore-worker/src/api.rs#L2740) | `wallet_sign_near_call_handler` | Sign NEAR function call (nonce, gas, args) |
| [api.rs](keystore-worker/src/api.rs) | `wallet_sign_near_transfer_handler` | Sign native NEAR transfer (amount only) |
| [api.rs:2843](keystore-worker/src/api.rs#L2843) | `wallet_check_policy_handler` | Decrypt policy from chain → evaluate action → return decision |
| [crypto.rs:108](keystore-worker/src/crypto.rs#L108) | `derive_keypair()` | `HMAC-SHA256(master_secret, seed)` → Ed25519 keypair |

#### Key derivation

```
master_secret (from NEAR MPC network, never leaves TEE)
    │
    ├── seed: "wallet:{wallet_id}:near"      → Ed25519 → NEAR implicit account
    ├── seed: "wallet:{wallet_id}:ethereum"   → secp256k1 → ETH address
    ├── seed: "wallet:{wallet_id}:solana"     → Ed25519 → Solana address
    └── seed: "wallet:{wallet_id}:bitcoin"    → secp256k1 → BTC address
```

Same wallet_id always produces same addresses across chains. Deterministic, stateless.

#### Policy evaluation flow (inside TEE)

1. Coordinator sends `POST /wallet/check-policy { wallet_id, action, current_usage }`
2. Keystore calls `get_wallet_policy(wallet_pubkey)` view method on NEAR (O(1) lookup)
3. Keystore decrypts `encrypted_data` with derived key
4. Checks: frozen → per-tx limit → velocity limits (hourly/daily/monthly vs `current_usage + amount`) → whitelist/blacklist → time restrictions → rate limit → approval threshold
5. Returns `PolicyDecision`: `Allowed`, `Denied(reason)`, `RequiresApproval(threshold)`, `Frozen`

### Contract — `contract/src/wallet.rs`

On-chain storage for encrypted policies and freeze flags.

| Function | Line | Description |
|----------|------|-------------|
| `store_wallet_policy()` | 164 | Store encrypted policy + verify wallet signature on-chain |
| `freeze_wallet()` | 280 | Controller-only emergency freeze (no wallet sig needed) |
| `unfreeze_wallet()` | 313 | Controller-only unfreeze |
| `delete_wallet_policy()` | 344 | Delete policy, refund storage deposit |
| `has_wallet_policy()` | 387 | View: check existence (for negative cache) |
| `get_wallet_policy()` | 394 | View: return `{ owner, encrypted_data, frozen, updated_at }` |

```rust
pub struct WalletPolicyEntry {
    pub owner: AccountId,           // Controller NEAR account
    pub encrypted_data: String,     // Encrypted by keystore TEE
    pub frozen: bool,               // Emergency freeze (separate from encrypted_data)
    pub updated_at: u64,            // Block timestamp
    pub storage_deposit: Balance,   // Refundable
}
```

**Ownership**: First `store_wallet_policy()` call sets `owner = caller`. Subsequent updates only from same owner. Wallet signature required (anti-spam + proof of key ownership).

**On-chain signature verification**: Ed25519 → `env::ed25519_verify()` (~26 Tgas), secp256k1 → `env::ecrecover()` (~35 Tgas).

### Worker WASI host functions — `worker/src/outlayer_wallet/`

WASI containers can call wallet functions via WIT interface.

| File | Description |
|------|-------------|
| [host_functions.rs](worker/src/outlayer_wallet/host_functions.rs) | WIT interface implementation (9,316 lines) |
| [mod.rs](worker/src/outlayer_wallet/mod.rs) | Module setup & linker bindings |
| [wallet.wit](worker/wit/deps/wallet.wit) | WIT interface definition |

**WIT interface** (`outlayer:wallet/api@0.1.0`):

```wit
get-id() → (string, string)
get-address(chain) → (string, string)
withdraw(chain, to, amount, token) → (string, string)         # cross-chain via Intents
withdraw-dry-run(chain, to, amount, token) → (string, string)
get-request-status(request-id) → (string, string)
list-tokens() → (string, string)
transfer(chain, to, amount) → (string, string)                # chain-specific (currently: near)
get-balance(chain, token) → (string, string)                  # chain-specific (currently: near)
intents-deposit(token, amount) → (string, string)             # deposit FT to intents.near
swap(token-in, token-out, amount-in, min-amount-out) → (string, string)  # swap via Intents
```

Available only when `WALLET_ID` env var is set (coordinator passes it when `X-Wallet-Id` header is valid). Rate limited to 50 calls per execution.

### Dashboard — `dashboard/app/wallet/`

| Page | File | Description |
|------|------|-------------|
| Handoff/setup | [page.tsx](dashboard/app/wallet/page.tsx) | Receive API key, connect NEAR wallet, set initial policy |
| Policy management | [manage/page.tsx](dashboard/app/wallet/manage/page.tsx) | Edit policy, manage approvers, freeze/unfreeze |
| Approvals list | [approvals/page.tsx](dashboard/app/wallet/approvals/page.tsx) | List pending multisig approvals |
| Approval detail | [approvals/[id]/page.tsx](dashboard/app/wallet/approvals/[id]/page.tsx) | View & sign specific approval |
| Audit log | [audit/page.tsx](dashboard/app/wallet/audit/page.tsx) | Full transaction and event history |
| Fund request | [fund/page.tsx](dashboard/app/wallet/fund/page.tsx) | User funds agent via link (?to, ?amount, ?token) |

### Documentation page

| File | Description |
|------|-------------|
| [docs/agent-custody/page.tsx](dashboard/app/docs/agent-custody/page.tsx) | User-facing docs page |

---

## Database Schema

Migrations: `coordinator/migrations/20260220000001_wallet.sql`, `20260220000002_wallet_policy_columns.sql`

| Table | Purpose |
|-------|---------|
| `wallet_accounts` | wallet_id, near_pubkey, policy_json (synced), frozen flag |
| `wallet_api_keys` | SHA-256 hash of API key → wallet_id mapping |
| `wallet_requests` | Async operation tracking (withdraw, deposit, call) |
| `wallet_pending_approvals` | Multisig approval state machine |
| `wallet_approval_signatures` | Individual approver signatures |
| `wallet_usage` | Per-token per-period spending (hourly/daily/monthly) |
| `wallet_audit_log` | Complete event history |
| `wallet_webhook_deliveries` | Webhook retry queue |

**Note**: `wallet_usage` is in the coordinator DB, not on-chain. If DB is compromised, velocity limits could be reset. Mitigation: per-tx limits and whitelists are checked in keystore TEE (not bypassable), and audit log records all operations.

---

## Flows

### Registration

```
Agent → POST /register → Coordinator
    Coordinator:
        1. Generate UUID wallet_id
        2. Generate random API key (wk_...)
        3. Store SHA-256(api_key) → wallet_id in DB
        4. Call keystore POST /wallet/derive-address { wallet_id, chain: "near" }
    Keystore TEE:
        5. HMAC-SHA256(master_secret, "wallet:{wallet_id}:near") → Ed25519 keypair
        6. Return { address, public_key }
    Coordinator:
        7. Return { api_key, near_account_id, handoff_url }
```

No blockchain transaction. Instant. API key shown once.

### Withdraw (with policy)

Withdraws tokens from the wallet's intents.near balance to a receiver via direct `ft_withdraw` contract call. Single synchronous NEAR transaction — no solver-relay needed.

```
Agent → POST /wallet/v1/intents/withdraw { to, amount, chain, token }
    with Authorization: Bearer wk_...

    Coordinator:
        1. Lookup wallet_id from SHA-256(api_key)
        2. Check idempotency key
        3. Get current_usage from wallet_usage table
        4. Call keystore POST /wallet/check-policy { wallet_id, action, current_usage }
    Keystore TEE:
        5. get_wallet_policy(wallet_pubkey) via NEAR RPC
        6. Decrypt policy → evaluate rules
        7. Return decision: Allowed / Denied / RequiresApproval / Frozen
    Coordinator (if Allowed):
        8. record_usage() → wallet_usage table (BEFORE execution)
        9. Call keystore POST /wallet/sign-near-call { intents.near, ft_withdraw, {token, receiver_id, amount} }
    Keystore TEE:
        10. Derive key → sign NEAR function call transaction
    Coordinator:
        11. Broadcast signed tx to NEAR RPC
        12. Create wallet_requests entry → return { request_id, status, tx_hash }
        13. Record audit log
        14. Enqueue webhook if configured
```

**Usage is recorded before execution** — if the backend call fails, velocity limits still accumulate. This prevents bypassing limits by causing intentional failures.

### Policy Setup

```
Dashboard → POST /wallet/v1/encrypt-policy { rules, approval, ... }
    Coordinator → Keystore: encrypt policy JSON
    Keystore → Return encrypted_base64

Dashboard → POST /wallet/v1/sign-policy { encrypted_data }
    Coordinator → Keystore: sign SHA256(encrypted_data) with wallet key
    Keystore → Return { signature, wallet_pubkey }

Dashboard → NEAR tx: store_wallet_policy(wallet_pubkey, encrypted_base64, signature)
    Contract: verify signature on-chain → store WalletPolicyEntry

Dashboard → POST /wallet/v1/invalidate-cache { wallet_id }
    Coordinator: clear negative policy cache
```

### Freeze (Emergency)

```
Wallet Owner → NEAR tx: freeze_wallet(wallet_pubkey)
    Contract: check caller == entry.owner → set frozen = true

Any subsequent wallet operation:
    Keystore reads fresh policy → sees frozen == true → rejects
```

No API gateway involvement needed. Owner can freeze directly on-chain. Latency: 2-5 seconds (blockchain confirmation).

### Multisig Approval

```
Agent → POST /wallet/v1/intents/withdraw { amount > threshold }
    Policy check → RequiresApproval(2 of 3)
    Create wallet_pending_approvals entry
    Return { status: "pending_approval", approval_id, required: 2 }

Approver 1 → POST /wallet/v1/approve/{approval_id}
    with NEP-413 wallet signature
    Store in wallet_approval_signatures → approved: 1/2

Approver 2 → POST /wallet/v1/approve/{approval_id}
    Store signature → threshold met
    Auto-execute: sign tx → submit via intents → update request status
    Enqueue webhook: request_completed
```

---

## Policy Format

Stored encrypted on NEAR blockchain. Only keystore TEE can decrypt.

```json
{
  "version": 1,
  "frozen": false,
  "rules": {
    "limits": {
      "per_transaction": { "native": "10000000000000000000000000", "nep141:usdt.tether-token.near": "1000000000" },
      "daily": { "*": "100000000000000000000000000" },
      "hourly": { "*": "50000000000000000000000000" },
      "monthly": { "*": "500000000000000000000000000" }
    },
    "addresses": { "mode": "whitelist", "list": ["bob.near", "dex.near"] },
    "transaction_types": ["withdraw", "contract_call"],
    "time_restrictions": { "timezone": "UTC", "allowed_hours": [9, 17], "allowed_days": [1, 2, 3, 4, 5] },
    "rate_limit": { "max_per_hour": 60 }
  },
  "approval": {
    "threshold": { "required": 2, "of": 3 },
    "above_usd": 1000,
    "approvers": [
      { "id": "ed25519:pubkey1", "role": "admin" },
      { "id": "ed25519:pubkey2", "role": "signer" },
      { "id": "ed25519:pubkey3", "role": "signer" }
    ]
  },
  "admin_quorum": { "required": 2, "admins": ["ed25519:pubkey1", "ed25519:pubkey2"] },
  "webhook_url": "https://myapp.com/webhook/wallet"
}
```

### Roles

| Role | Approve transactions | Modify policy | Freeze wallet |
|------|---------------------|---------------|---------------|
| admin | Yes | Yes (quorum) | Yes |
| signer | Yes | No | No |

### Limits — `"*"` = wildcard for all tokens

- `per_transaction` — max amount per single tx
- `hourly` / `daily` / `monthly` — velocity limits (checked against `current_usage` from coordinator DB)
- `rate_limit.max_per_hour` — max number of transactions

---

## API Endpoints

Base: `https://api.outlayer.fastnear.com`

### Public

| Method | Path | Description |
|--------|------|-------------|
| POST | `/register` | Create wallet, returns API key (one-time) |

### Authenticated (Bearer API key)

| Method | Path | Description |
|--------|------|-------------|
| GET | `/wallet/v1/address?chain={chain}` | Derive address (near, ethereum, solana, bitcoin) |
| POST | `/wallet/v1/intents/withdraw` | Withdraw / cross-chain transfer |
| POST | `/wallet/v1/intents/withdraw/dry-run` | Simulate withdrawal (policy + balance check) |
| POST | `/wallet/v1/call` | Native NEAR contract call |
| POST | `/wallet/v1/transfer` | Chain-agnostic transfer (`chain` param, currently near) |
| GET | `/wallet/v1/balance?chain={chain}&token={token}` | Chain-agnostic balance (defaults to near) |
| POST | `/wallet/v1/intents/deposit` | Deposit FT into intents.near (for manual intents operations) |
| POST | `/wallet/v1/intents/swap` | Swap via 1Click: quote → deposit to intents.near → mt_transfer → poll |
| POST | `/wallet/v1/deposit` | Cross-chain deposit (Intents quote) |
| GET | `/wallet/v1/requests/{id}` | Poll async operation status |
| GET | `/wallet/v1/requests` | List operations (filter: type, status, limit) |
| GET | `/wallet/v1/tokens` | List available tokens (Intents proxy) |
| GET | `/wallet/v1/policy` | View current policy (decrypted via keystore) |
| POST | `/wallet/v1/encrypt-policy` | Encrypt policy for on-chain storage |
| POST | `/wallet/v1/sign-policy` | Keystore signs encrypted policy SHA256 |
| POST | `/wallet/v1/invalidate-cache` | Clear negative policy cache |
| GET | `/wallet/v1/pending_approvals` | List pending multisig approvals |
| POST | `/wallet/v1/approve/{id}` | Submit multisig approval signature |
| POST | `/wallet/v1/reject/{id}` | Reject pending approval |
| GET | `/wallet/v1/audit` | Full event history |

### Internal (worker network only)

| Method | Path | Description |
|--------|------|-------------|
| POST | `/internal/wallet-check` | Policy check for WASI execution |
| POST | `/internal/wallet-audit` | Record audit event from WASI |

---

## Negative Policy Cache

Coordinator caches `wallet_id → NoPolicy` in-memory (HashMap, TTL 5 min). If no policy exists, subsequent requests skip the keystore call entirely (no limits to check). Cache cleared on:
- `POST /wallet/v1/invalidate-cache` (dashboard calls after on-chain tx)
- TTL expiry (5 min)

If policy exists → keystore always reads fresh from chain (never cached).

---

## Error Codes

| Error | Meaning |
|-------|---------|
| `missing_auth` | No Authorization header |
| `invalid_api_key` | Key not found or revoked |
| `policy_denied` | Operation blocked by policy rules |
| `wallet_frozen` | Wallet frozen by controller |
| `insufficient_balance` | Not enough funds |
| `pending_approval` | Needs multisig (not an error — returns approval_id) |
| `rate_limited` | Too many requests |
| `invalid_address` | Bad destination address |
| `unsupported_token` | Token not supported |

---

## Security Model

1. **MPC master secret** — obtained from NEAR Protocol MPC network via DAO-governed process. Lives only inside TEE. Individual wallet keys derived deterministically via HMAC-SHA256.

2. **TEE isolation** — Intel TDX enclaves. Key derivation, signing, policy evaluation all inside TEE. Even infrastructure operator cannot extract keys or bypass policy.

3. **Policy on-chain** — encrypted, stored in NEAR contract `LookupMap`. Only TEE can decrypt. Controller can freeze wallet directly on-chain without going through API.

4. **API key security** — only SHA-256 hash stored in DB. Plaintext shown once at registration. Key prefix `wk_` for identification.

5. **Velocity limits** — tracked in coordinator DB (`wallet_usage` table). Usage recorded BEFORE execution (prevents bypass via intentional failures). Per-tx limits checked in TEE (not bypassable even if DB is compromised).

6. **Agent compromise recovery** — freeze wallet (instant, on-chain) → revoke API key → create new key. Private key never exposed — nothing to rotate.
