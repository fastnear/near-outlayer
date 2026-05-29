# Agent Custody — Developer Reference

Institutional-grade custody wallets for AI agents. An agent gets an API key to operate a NEAR-native wallet whose cross-chain value is custodied on `intents.near`. Private keys live exclusively inside a TEE (Intel TDX). The wallet owner sets policy (spending limits, whitelists, multisig, freeze) — all enforced inside the TEE. Cross-chain deposits/withdrawals via NEAR Intents + the 1Click solver (gasless), like a CEX: deposit, operate, withdraw to an external address. The wallet does not sign native Ethereum/Solana transactions itself (planned, not shipped).

> **⚠️ Only send whitelisted Intents assets — anything else is lost permanently.**
> Deposits/withdrawals only work for assets in the NEAR Intents / 1Click token
> catalog (`GET /wallet/v1/tokens`), on the exact chain a deposit address was
> issued for. Sending an unsupported token, the wrong token, a token on the
> wrong chain, an NFT, or an unlisted native gas coin to a deposit address is
> **unrecoverable**. Deposit addresses from `/wallet/v1/deposit-intent` are
> per-request and expire (30 min) — never reuse one or send after expiry.

---

## Integrating

For most use cases, use the **TypeScript SDK** instead of calling the HTTP API directly:

```bash
npm install @outlayer/sdk
```

```ts
import { OutlayerClient } from '@outlayer/sdk';

// 1. Register a wallet (anonymous, returns API key once)
const { apiKey, walletId, handoffUrl } = await OutlayerClient.register();

// 2. Use it
const client = new OutlayerClient({ apiKey });
const result = await client.withdraw({
  chain: 'ethereum',
  to: '0x742d35Cc6634C0532925a3b844Bc9e7595f8b4f5',
  amount: '1000000',
  token: 'nep141:usdt.tether-token.near',
});
```

- **SDK source**: [out-layer/sdk-js](https://github.com/out-layer/sdk-js) (MIT)
- **SDK on npm**: [`@outlayer/sdk`](https://www.npmjs.com/package/@outlayer/sdk)
- **OpenAPI spec**: [out-layer/api-spec](https://github.com/out-layer/api-spec)
- **Interactive API docs**: https://api.outlayer.fastnear.com/docs (Scalar UI)

The SDK auto-generates types from the OpenAPI spec, adds typed error classes (`PolicyDeniedError`, `WalletFrozenError`, etc.), automatic idempotency keys, and retry with backoff on 5xx + network errors. SDK feature parity with the raw HTTP API; the rest of this document is the reference for both.

For other languages, generate a client from the OpenAPI spec:

```bash
# Python
openapi-python-client generate --url https://api.outlayer.fastnear.com/openapi.json
# Go
oapi-codegen -generate types https://api.outlayer.fastnear.com/openapi.json > types.go
```

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
| `intents_deposit()` | — | Deposit FT into intents.near via `ft_transfer_call` (intents.near auto-registers callers via its own `ft_on_transfer` hook — no NEP-145 `storage_deposit` issued) |
| `swap()` | — | Swap via 1Click: quote → ft_transfer_call to intents.near → mt_transfer → poll |
| `deposit()` | 857 | Cross-chain deposit via Intents quote |
| `get_address()` | 345 | Derive wallet address. **`chain=near` only** — `validate_chain()` rejects other chains (no native spend path yet; cross-chain value uses Intents). |
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

> The keystore *can* derive and sign for all of the above (secp256k1 for EVM,
> Ed25519 for NEAR/Solana). But the public `GET /wallet/v1/address` endpoint
> currently returns the **NEAR** address only — the coordinator has no native
> tx builder/broadcast for EVM/Solana yet, so it does not hand out fund-able
> native addresses (that would risk stuck funds). Cross-chain value movement
> does not need them: it goes through NEAR Intents + the 1Click solver. See
> [coordinator `docs/MULTI_CHAIN.md`](https://github.com/out-layer/coordinator/blob/main/docs/MULTI_CHAIN.md).

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
get-address(chain) → (string, string)                         # currently: near only
withdraw(chain, to, amount, token) → (string, string)         # cross-chain via Intents (whitelisted assets only)
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

**Token options for `chain=near`** — the `token` field selects what the recipient receives:

| `token` | Recipient receives | Notes |
|---------|--------------------|-------|
| omitted / `"near"` / `"native"` | **native NEAR** (default) | intents.near unwraps the wallet's wNEAR and sends native NEAR via the `native_withdraw` intent. Gasless; recipient needs **no** `wrap.near` storage. The recipient account must already exist (or be a 64-char implicit account) — a `native_withdraw` to a non-existent named account burns the wNEAR and is rejected up front. |
| `"nep141:wrap.near"` (or `"wrap.near"`) | **wNEAR** (NEP-141) | Explicit opt-in. Recipient must be storage-registered on `wrap.near` (`POST /wallet/v1/storage-deposit`). |
| other `nep141:<token>` | that NEP-141 | Recipient must be storage-registered on that token. |

This solves the "wallet holds only wNEAR, 0 native NEAR" case: it can withdraw native NEAR for gas/staking without first unwrapping. For cross-chain (`chain=ethereum`, etc.) the `token` is the source Intents asset and 1Click delivers the destination chain's native asset.

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
| POST | `/wallet/v1/deposit-intent` | Cross-chain deposit (1Click bridge address; `source_asset` or `chain`+`token` shape) |
| POST | `/wallet/v1/confidential/deposit` | SHIELD: public intents → confidential shard (503 if not enabled) |
| POST | `/wallet/v1/confidential/unshield` | Confidential → public intents |
| POST | `/wallet/v1/confidential/withdraw` | Confidential → external chain (or `chain="near"` for **native NEAR** delivery via `intents.near native_withdraw`) |
| POST | `/wallet/v1/confidential/withdraw/dry-run` | Quote a confidential withdraw |
| POST | `/wallet/v1/confidential/transfer` | Private confidential → confidential transfer |
| POST | `/wallet/v1/confidential/swap` | Confidential swap (distinct assets) |
| POST | `/wallet/v1/confidential/swap/quote` | Quote a confidential swap |
| POST | `/wallet/v1/confidential/deposit-intent` | Cross-chain deposit into confidential (bridge address) |
| GET | `/wallet/v1/confidential/balance` | Read confidential balances (private shard `intents.far`, no public RPC) |
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

## Confidential Intents

> **Building an agent?** See the integration guide
> [`CONFIDENTIAL_INTENTS.md`](https://github.com/out-layer/coordinator/blob/main/docs/CONFIDENTIAL_INTENTS.md)
> in the coordinator repo — it covers the mental model (private on-chain shard,
> same-wallet identity, what privacy you actually get) + all methods, written
> for agent developers. This section is the operator/architecture summary.

The `/wallet/v1/confidential/*` routes mirror `/wallet/v1/intents/*` but operate
on the Defuse **confidential** shard — a separate PRIVATE shard (the
`intents.far` contract), distinct from public `intents.near`. Disabled by default —
gated by `ENABLE_CONFIDENTIAL_INTENTS` plus a **separate** Defuse partner
agreement (`ONECLICK_CONFIDENTIAL_BASE_URL` + `ONECLICK_CONFIDENTIAL_JWT`, which
**must differ** from the public `ONECLICK_JWT`). When unconfigured, every
confidential route returns **HTTP 503** `confidential_unavailable`.

Pipeline per op: NEP-413 challenge → per-account JWT (cached in Redis
`wallet:{id}:cfjwt`, 14 min) → 1Click quote → generate-intent → sign via
keystore → submit-intent. Ops are async; status is refreshed on read of
`GET /wallet/v1/requests/{id}` until terminal.

**Privacy** (must be disclosed to users):

- Confidential balances are **real on-chain state** on the private `intents.far`
  shard — not off-chain, not a solver database. The privacy is that this shard
  has **no public RPC**: you cannot read it (verified — `intents.far` resolves to
  `UNKNOWN_ACCOUNT` on public mainnet RPC). It is an auditable smart contract:
  the operator/Defuse, auditors, or law enforcement with a warrant CAN read it.
- Internal moves (confidential transfer/swap) leave **no public-chain trace** —
  they settle on the private shard. Only the edges touch the public chain.
- **SHIELD/UNSHIELD link the wallet on-chain** (entry/exit reveal); cross-chain
  DEPOSIT/WITHDRAW only expose the external-chain sender/receiver (public on that
  chain), not the confidential shard's internal moves.
- **Not hidden, ever**: the Defuse/1Click solver layer (sees plaintext intents),
  the `partner_id` mapping, and the source-chain identity.
- **Cross-chain DEPOSIT/WITHDRAW are still correlatable by timing and amount**:
  the source-chain deposit (at T) and destination-chain delivery (at T+N, e.g.
  0.5 in / 0.44 out after bridge fee) are both visible on their public chains
  and join trivially. True unlinkability needs jitter delays + amount splitting.

Each wallet has a single confidential identity (the custody wallet itself);
there is no separate or unlinkable confidential identity.

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

## Per-customer Vaults (sovereignty option)

By default, every wallet's keys are derived from the **OutLayer
default master** (HMAC-SHA256 chain rooted in the keystore-worker's
TEE secret). Convenient and recovery-free, but if OutLayer ceases,
the derived keys are gone with it.

A **per-customer vault** replaces that shared master with a
per-customer master derived via NEAR's MPC network from a
sub-account the customer controls. The wallet's API key is bound
to the vault at registration time; subsequent wallet operations
forward `X-Customer-Vault: <vault_id>` to the keystore, which
routes derivations through the per-vault master.

### Wallet creation flow with vault scope

```
Customer → outlayer vault init  (or dashboard /vault page)
    Atomic NEAR tx (5 actions, all-or-nothing):
        CreateAccount(vault.<customer.near>)
        Transfer(0.1 NEAR)               // storage stake + MPC-call gas reserve
        UseGlobalContract(approved_code_hash)
        FunctionCall("new", {parent, keystore_dao, mpc_contract, exit_window})
        AddKey(tee_pubkey, FCAK on vault.request_master)
    POST /customer/sign-verification → keystore re-verifies + signs
                                       mark_vault_verified on chain
    POST /customer/register {vault_id, webhook_url?}
    Coordinator:
        1. View-call keystore_dao.is_vault_verified(vault_id) — must be true
        2. INSERT wallet_accounts (wallet_id, vault_id, vault_webhook_url)
        3. INSERT wallet_api_keys (key_hash, customer_account_id=vault_id)
        4. POST /wallet/derive-address  (with X-Customer-Vault header)
    Keystore TEE:
        5. Lazy-load: ensure_customer_loaded(vault_id) drives MPC CKD
           with derivation_path = HMAC(default_master, "vault-master:{vault_id}")
        6. Cache per-vault master in masters: HashMap<AccountId, [u8;32]>
        7. HMAC(per_vault_master, "wallet:{wallet_id}:near") → keypair
        8. Return { address, public_key }
    Coordinator:
        9. Save derived public_key on the wallet row
       10. Commit transaction; return API key + fire vault_registered webhook
```

The customer's API key is now permanently bound to the vault. Every
wallet operation uses the per-vault master; on cessation or
unilateral exit, the customer recovers control of the vault account
and the per-vault master remains derivable by any post-recovery
DAO-approved TEE worker (deterministic — same `(default_master,
vault_id)` → same `secret_path` → same MPC-derived master).

### Recovery flow (cessation path)

```
DAO members → keystore_dao.declare_cessation()      [is_ceased() = true]

Anyone      → vault.initiate_recovery()
                  → cross-contract is_ceased() check
                  → recovery = {trigger: Cessation,
                                finalize_after: now+7d,
                                finalize_before: now+14d}

(7-day delay)

Anyone      → vault.finalize_recovery()
                  → cross-contract is_ceased() check (still true?)
                  → unlocked = true
                  → recovery = None

Parent      → vault.unlocked_add_key(parent_pubkey, full_access: true)
              [parent now controls the sub-account; can withdraw funds
               and migrate to a new custody provider]
```

### Recovery flow (unilateral path)

```
Parent → vault.set_exit_window(86400)            [optional, 24h-30d range]
Parent → vault.unilateral_initiate_recovery()
            → recovery = {trigger: Unilateral,
                          finalize_after: now + window_secs}

(configured delay — default 24h)

Anyone → vault.finalize_recovery()                [no DAO check]
            → unlocked = true

Parent → vault.unlocked_add_key(...)
```

For the architectural reference (two-layer key derivation, race-attack
mitigation, governance fixes), see [VAULTS.md](VAULTS.md). For the
customer-facing how-to, see `dashboard/app/docs/vaults/page.tsx`.

---

## Security Model

1. **MPC master secret** — obtained from NEAR Protocol MPC network via DAO-governed process. Lives only inside TEE. Individual wallet keys derived deterministically via HMAC-SHA256.

2. **TEE isolation** — Intel TDX enclaves. Key derivation, signing, policy evaluation all inside TEE. Even infrastructure operator cannot extract keys or bypass policy.

3. **Policy on-chain** — encrypted, stored in NEAR contract `LookupMap`. Only TEE can decrypt. Controller can freeze wallet directly on-chain without going through API.

4. **API key security** — only SHA-256 hash stored in DB. Plaintext shown once at registration. Key prefix `wk_` for identification.

5. **Velocity limits** — tracked in coordinator DB (`wallet_usage` table). Usage recorded BEFORE execution (prevents bypass via intentional failures). Per-tx limits checked in TEE (not bypassable even if DB is compromised).

6. **Agent compromise recovery** — freeze wallet (instant, on-chain) → revoke API key → create new key. Private key never exposed — nothing to rotate.
