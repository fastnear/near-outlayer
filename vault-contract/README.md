# vault-contract

NEAR smart contract that represents a **per-customer sovereign vault**
for OutLayer. One vault per customer; the contract is the on-chain
root that lets OutLayer derive per-customer secrets while preserving
the customer's ability to fully exit OutLayer's infrastructure at any
time without losing access to their wallets or secrets.

The contract is the single source of truth for:

- Who the customer is (`parent` account) and which TEE function-call
  keys OutLayer is authorised to use against this vault.
- The DAO governance contract (`keystore_dao`) and MPC contract
  (`mpc_contract`) the vault talks to.
- Recovery state (cessation- and unilateral-triggered, with
  distinct delay / finalize-window semantics).
- The atomic key-swap that physically removes OutLayer's TEE keys
  and installs the customer's new key when `finalize_recovery` lands.

For the end-to-end **sovereign exit runbook** (initiate → wait →
finalize → master recovery → wallet re-derivation → secret decrypt),
see [`docs/LEAVING_OUTLAYER.md`](../docs/LEAVING_OUTLAYER.md).

For the **architectural overview** of why the vault exists and how
it interacts with the keystore / coordinator / MPC, see the rendered
dashboard docs at https://outlayer.fastnear.com/docs/vaults (source:
[`dashboard/app/docs/vaults/page.tsx`](../dashboard/app/docs/vaults/page.tsx)).

For **deploying the WASM as a NEP-591 global contract** so customers
can run `outlayer vault init` against it, see
[`GLOBAL-CONTACT.md`](./GLOBAL-CONTACT.md).

---

## Vault state

```rust
pub struct Vault {
    parent:                       AccountId,        // customer (immutable post-deploy)
    keystore_dao:                 AccountId,        // DAO governance (immutable)
    mpc_contract:                 AccountId,        // MPC contract for CKD (immutable)
    initial_tee_key:              Option<PublicKey>,// TEE FCAK installed at deploy
    registered_tee_keys:          Vec<PublicKey>,   // DAO-rotated TEE keys (≤32)
    recovery:                     Option<RecoveryState>, // in-flight recovery (if any)
    unlocked:                     bool,             // true after finalize_recovery
    unilateral_exit_window_secs:  u64,              // configurable delay
}
```

Key invariants enforced at runtime:

1. **Sub-account naming**: `current_account_id().is_sub_account_of(&parent)`
   is checked in `new()` so a vault account can only claim a parent
   whose namespace it lives under. Prevents `attacker.near` from
   deploying a vault that names `victim.near` as parent.
2. **Parent-only mutations**: `set_exit_window`,
   `unilateral_initiate_recovery`, `finalize_recovery`,
   `unlocked_add_key`, `clear_unused_tee_keys` all require
   `predecessor_account_id() == self.parent`.
3. **No in-place upgrade**: `Vault` is `PanicOnDefault` and there is
   no `migrate()`. The borsh layout was changed when
   `initial_tee_key` was added at position 4; old WASM hashes are
   permanently incompatible with new ones. Each customer's vault
   is bound to whatever WASM hash was used in the original
   `UseGlobalContract` action — pinned forever.
4. **Recovery is one-way**: `unlocked` only flips false → true,
   inside `callback_after_swap`, and only if the atomic
   DeleteKey + AddFullAccessKey batch succeeded. There is no method
   that re-locks a recovered vault.

---

## Public API

### Constructor

| Method | Caller | Purpose |
|---|---|---|
| `new(parent, keystore_dao, mpc_contract, initial_tee_pubkey, initial_exit_window)` | `#[init]` — called once by the atomic-deploy tx | Initialise immutable identity fields; pin the TEE function-call pubkey that the same tx installs via `AddKey`. `initial_exit_window: Option<u64>` defaults to `DEFAULT_UNILATERAL_EXIT_WINDOW_SECS` and must be within `[MIN, MAX]_UNILATERAL_EXIT_WINDOW_SECS`. |

### TEE keystore-worker key management

| Method | Caller | Purpose |
|---|---|---|
| `propose_tee_key(public_key) -> Promise` | **permissionless** (DAO gate inside the callback) | Add a DAO-approved keystore-worker pubkey as a function-call AccessKey scoped to `(receiver=vault, methods=["request_master"])`. Hard cap of `MAX_REGISTERED_TEE_KEYS = 32`. Hostile race-add attacker pays gas, can only enqueue *already* DAO-approved keys — no funds exfiltration, only future-rotation blocking; mitigated by `clear_unused_tee_keys`. |
| `clear_unused_tee_keys(public_keys) -> Promise` | parent only | Delete a list of registered TEE keys (typo-guarded — panics on unknown key). Useful for unwinding noisy `propose_tee_key` races. |
| `callback_add_tee_key(public_key, dao_approved)` | `#[private]` self-callback | Commits the AddKey promise iff the DAO view-call returned `is_keystore_approved == true`. Otherwise panics and rolls back. |

### MPC proxy

| Method | Caller | Purpose |
|---|---|---|
| `request_master(request) -> Promise` | **vault self-call only** (signed by a TEE function-call key with `signer_account_id == current_account_id`) | Cross-contract proxy to `mpc_contract.request_app_private_key(request)`. Attaches `1 yoctoNEAR` and 150 TGas; returns the MPC response payload directly to the caller. The `predecessor == current` gate prevents anyone from forging a CKD request encrypted to their own pubkey using the public on-chain derivation path. |

### Recovery — initiation

| Method | Caller | Purpose |
|---|---|---|
| `initiate_recovery() -> Promise` | **permissionless** (DAO gate inside `callback_initiate`) | Cessation flow. Cross-contract checks `keystore_dao.is_ceased() == true`; if false, the callback panics and no state mutates. On success, stores `RecoveryState { trigger: Cessation, finalize_after = now + 7d, finalize_before = finalize_after + 7d }`. |
| `unilateral_initiate_recovery()` | parent only | Voluntary exit. No DAO involvement. Stores `RecoveryState { trigger: Unilateral, finalize_after = now + unilateral_exit_window_secs, finalize_before = finalize_after + 7d }`. The window is captured at initiate time — calling `set_exit_window` mid-recovery does NOT shorten the in-flight one. |
| `set_exit_window(new_window_secs)` | parent only | Update `unilateral_exit_window_secs` for **future** recoveries. Must be in `[MIN, MAX]_UNILATERAL_EXIT_WINDOW_SECS`. |

### Recovery — finalize (atomic key-swap)

| Method | Caller | Purpose |
|---|---|---|
| `finalize_recovery(new_parent_pubkey) -> PromiseOrValue<bool>` | **parent only** (closes the front-running window) | Dispatch the atomic key-swap. For both `Cessation` and `Unilateral` triggers: deletes `initial_tee_key` + all `registered_tee_keys`, then adds `new_parent_pubkey` as FullAccess. The state mutation (`unlocked = true`, `recovery = None`) is deferred to `callback_after_swap` and gated on the Promise batch succeeding — a failed swap leaves the vault locked and the recovery state intact so the parent can retry within `finalize_before`. |
| `callback_finalize(new_parent_pubkey, dao_result)` | `#[private]` self-callback (Cessation only) | Re-checks `is_ceased()` to catch DAO revocation that landed during the delay. If revoked → cancel recovery, clear state, return `false`. If still ceased → dispatch the swap. |
| `callback_after_swap(cessation, swap_result)` | `#[private]` self-callback | Commits `unlocked = true` and clears `recovery` iff the swap receipt succeeded. Emits `recovery_finalized_unilateral` / `recovery_finalized_cessation` log on success; `recovery_finalize_swap_failed` on rollback. |

### Post-recovery key management

| Method | Caller | Purpose |
|---|---|---|
| `unlocked_add_key(public_key, full_access, allowance)` | parent only, **requires `unlocked == true`** | Add an arbitrary access key to the (now-customer-controlled) vault. `full_access = true` adds a FullAccess key; otherwise adds a function-call key scoped to `(vault, ["*"])` with the given allowance (defaults to 1 NEAR). Zero allowance is rejected. |

### Views

| Method | Returns |
|---|---|
| `get_state()` | `VaultState` — full snapshot for off-chain verifiers |
| `get_registered_keys()` | `Vec<PublicKey>` — registered TEE keys (excluding `initial_tee_key`) |
| `get_recovery_state()` | `Option<RecoveryState>` — in-flight recovery; **may be stale**, callers must compare `finalize_before` to current block timestamp |
| `get_exit_window()` | `u64` — current `unilateral_exit_window_secs` |
| `is_unlocked()` | `bool` |

---

## Recovery flow (state machine)

```
                 ┌────────────────────────────┐
                 │ locked, recovery = None    │  initial state after deploy
                 └─────────────┬──────────────┘
                               │
            DAO is_ceased=true │           parent calls
              (anyone)         │           unilateral_initiate_recovery
                               ▼
                 ┌────────────────────────────┐
                 │ locked,                    │
                 │ recovery = Some {          │
                 │   trigger,                 │
                 │   finalize_after,          │
                 │   finalize_before          │
                 │ }                          │
                 └─────────────┬──────────────┘
                               │
              now > finalize_before:           now ∈ [finalize_after, finalize_before]:
              auto-cancel on next finalize     parent calls finalize_recovery(new_parent_pubkey)
                 │                                 │
                 ▼                                 ▼
        ┌──────────────────────┐        ┌─────────────────────────────────────────┐
        │ locked,              │        │ atomic Promise batch dispatches:        │
        │ recovery = None      │        │  delete_key(initial_tee_key)            │
        │ (state cleared)      │        │  delete_key(k) for k in registered_tee_keys
        └──────────────────────┘        │  add_full_access_key(new_parent_pubkey) │
                                        │ → callback_after_swap                   │
                                        └──────────────────┬──────────────────────┘
                                                           │
                              swap failed                  │  swap succeeded
                              (e.g. pubkey collision)      │
                                  │                        ▼
                                  │              ┌─────────────────────────┐
                                  │              │ unlocked = true         │
                                  ▼              │ recovery = None         │
                            (state unchanged,    │ parent FullAccess on    │
                             retry possible)     │ vault.{customer.near}   │
                                                 └─────────────────────────┘
```

Key timing constants (mainnet — see `lib.rs` for `--features test-timing` overrides):

| Constant | Mainnet | What it gates |
|---|---|---|
| `CESSATION_DELAY_NS` | 7 days | `finalize_after` for `RecoveryTrigger::Cessation` |
| `FINALIZE_WINDOW_NS` | 7 days | `finalize_before - finalize_after` for both triggers |
| `DEFAULT_UNILATERAL_EXIT_WINDOW_SECS` | 1 day | default value for `unilateral_exit_window_secs` if `initial_exit_window == None` at deploy |
| `MIN_UNILATERAL_EXIT_WINDOW_SECS` | 1 day | lower bound for `set_exit_window` / `new()` |
| `MAX_UNILATERAL_EXIT_WINDOW_SECS` | 30 days | upper bound |

---

## Events emitted (matched by `outlayer-monitor`)

The contract emits unstructured log lines that the indexer
([`outlayer-monitor/src/source.rs`](../outlayer-monitor/src/source.rs))
parses to drive keystore eviction and operator alerts:

| Log line | Site | Meaning |
|---|---|---|
| `recovery_initiated_cessation` | `initiate_recovery` callback success | DAO cessation recovery clock started |
| `recovery_initiated_unilateral` | `unilateral_initiate_recovery` | Parent-driven exit clock started |
| `recovery_finalized_cessation` | `callback_after_swap` (cessation) | Atomic swap committed → triggers `/admin/evict-customer` on keystore |
| `recovery_finalized_unilateral` | `callback_after_swap` (unilateral) | Same as above for unilateral path |
| `recovery_finalize_swap_failed` | `callback_after_swap` rollback | Swap Promise panicked; vault still locked, retry possible |
| `recovery_finalize_failed_dao_call` | `callback_finalize` error | Cessation DAO view-call failed |
| `recovery_window_expired` | `finalize_recovery` past `finalize_before` | Auto-cancellation |
| `recovery_cancelled_dao_revoked` | `callback_finalize` (DAO revoked cessation) | Cessation cancelled during delay |
| `vault_tee_key_added` | `callback_add_tee_key` success | Indexer tracks rotations |
| `vault_tee_keys_cleared count=<n>` | `clear_unused_tee_keys` Promise | Suffix is dynamic |
| `exit_window_set_to_<n>_secs` | `set_exit_window` | Suffix is dynamic |

---

## Building

### Mainnet build

```bash
cd vault-contract
cargo near build non-reproducible-wasm --no-abi
# or, for the canonical hash that lands in DAO approve_vault_version:
./build-docker.sh
```

Produces `target/near/vault_contract.wasm` (also copied to `res/`).
The hash printed at the end is what gets passed to
`dao.outlayer.near.approve_vault_version`. Default build settings
produce mainnet semantics (7-day delays, 1-day MIN exit window, 30-day
MAX).

### Sandbox / testnet QA build

Integration tests and any vault deployed under a temporary testnet
hash need second-granularity delays:

```bash
cd vault-contract
cargo test --features test-timing --test integration
# or for a testnet WASM:
cargo near build non-reproducible-wasm --no-abi --features test-timing
```

The `test-timing` feature collapses `CESSATION_DELAY_NS` to 60s,
`FINALIZE_WINDOW_NS` to 600s, `DEFAULT/MIN` exit-window to 180/60s,
and `MAX` to 7 days. **Never** approve a `--features test-timing`
hash in the mainnet DAO — the safety floor on minimum exit window
collapses from 1 day to 60 seconds, which would let a stolen parent
key drain a vault before the customer can react.

### Deploying as a global contract

The WASM is deployed once per network as a NEP-591 global contract
addressed by SHA-256. Customers' `outlayer vault init` then references
it via `UseGlobalContract { CodeHash }` so the per-vault deploy tx
fits in a browser-wallet URL. Full procedure in
[`GLOBAL-CONTACT.md`](./GLOBAL-CONTACT.md).

---

## Tests

| Suite | Run | Coverage |
|---|---|---|
| `cargo test --lib` | unit tests, ~50 ms | 47 tests covering range checks, panic conditions, predecessor-only enforcement, deferred-state-commit invariants. Uses mocked block timestamps. |
| `cargo test --features test-timing --test integration` | near-workspaces sandbox, ~90 s | 23 tests covering full multi-tx flows: atomic deploy, cessation + unilateral happy paths, DAO revocation mid-recovery, window expiry, race conditions on `propose_tee_key`, key-swap atomicity, post-unlock `unlocked_add_key`. |
| `tests/sovereignty_e2e.sh` (repo root) | testnet, ~7 min | 14-step end-to-end sovereign exit on real testnet: deploy → wallet → keystore-signed transfer → finalize_recovery → keystore refuses → CKD recovery → local wallet derivation → local-key send-near → on-chain ciphertext fetch → local secret decrypt. |
| `tests/vault_detach_test.sh` | testnet, ~2 min | Run only the detach half against an existing vault + secret (skip deploy/setup). |
| `tests/vault_multi_customer_isolation.sh` | testnet, ~3 min | Phase 10 scenario 5: two vaults under the same parent, distinct keys, header-override ignored. |
| `tests/vault_backward_compat.sh` | testnet, ~1 min | Phase 10 scenario 6: legacy default-master path (no `vault_id`) still works. |

---

## Customer-facing HTTPS auth model (coordinator)

The contract is the on-chain root. Customers actually reach it
through OutLayer's coordinator HTTP API. Two distinct steps:

### Step 1: `outlayer vault init` (or dashboard "Create vault")

- Atomic deploy via `UseGlobalContract` (5 actions: CreateAccount,
  Transfer, UseGlobalContract, `new(...)`, AddKey).
- Calls `POST /customer/register` on the coordinator. This
  triggers the keystore-worker's `mark_vault_verified` transaction
  on the DAO so the vault lands in
  `keystore-dao.verified_vaults`.
- **No API key is issued.** The vault exists on chain and is
  DAO-verified, but no `wk_` has been minted yet.

### Step 2: `POST /register {"vault_id": "<vault>"}` (separate call)

Mint one wallet API key bound to the vault. Returns:

```json
{
  "api_key": "wk_<64 hex>",
  "wallet_id": "<UUID v4>",
  "near_account_id": "<hex of HMAC-SHA256(per_vault_master, 'wallet:<wallet_id>:near')[..32].verifying_key>",
  "handoff_url": "https://outlayer.fastnear.com/wallet?key=wk_...",
  "trial": { ... }
}
```

Call this N times for N wallets under the same vault. Migration
`20260511000001` dropped the UNIQUE constraint on
`wallet_accounts.vault_id` so the coordinator stores N rows linking
the same vault to N distinct `wallet_id`s. Each `wallet_id` gets a
cryptographically isolated NEAR address because the seed differs.

### `/wallet/v1/*` auth

```
Authorization: Bearer wk_<api_key>
```

Coordinator pipeline:
1. Hash the API key, look up in `wallet_api_keys` table → get
   `wallet_id` and `customer_account_id` (= the vault_id if
   vault-bound, else NULL for legacy default-master wallets).
2. Forward `X-Customer-Vault: <vault_id>` to the keystore on every
   signing call. Keystore uses it to select the per-vault master
   via MPC CKD (or default master if absent).

**The vault binding is auth-driven, never request-driven.** A
client-supplied `X-Customer-Vault` header on a `/wallet/v1` call
is ignored — the binding lives on the API key's DB row. Test
coverage: [`tests/vault_multi_customer_isolation.sh`](../tests/vault_multi_customer_isolation.sh).

### Per-user patterns (one parent, many sub-wallets)

For an application serving many users / tasks from one vault, two
paths exist depending on what secret the application holds:

**Path A — Stateful (random `POST /register` per user)**

Application calls `POST /register {vault_id}` once per user, stores
the returned `(user_id → wallet_id, api_key)` mapping in its own
DB. Each user gets a cryptographically isolated NEAR address.
Stateful but conservative: only `wk_`'s on the server, no NEAR
private key, no on-chain auth per request.

**Path B — Stateless Bearer (`PUT /wallet/v1/api-key` with Bearer
parent `wk_`)**

Application mints one parent `wk_` via `POST /register {vault_id}`
at setup time and stores it in env. For each sub-wallet, it
derives a `sub_key = "wk_" + sha256(seed:index:parent_wk)` locally
and registers the hash via `PUT /api-key` with `Bearer parent_wk_`.
The coordinator inherits the parent's vault binding automatically.
Stateless after one-time setup: re-deriving sub-keys needs only
the parent `wk_` + the seed. Parent's NEAR private key is never on
the application server. Documented in
[`docs/DETERMINISTIC_WALLETS.md` Flow 4a](../docs/DETERMINISTIC_WALLETS.md).

| Path | Bot-server secret | Reach if leaked | Use when |
|---|---|---|---|
| A | `wk_`'s per user | Drain just that user's wallet, revoke single `wk_` | High-value vaults; explicit per-user audit trail |
| B | parent `wk_` | Drain all sub-wallets; revoke parent `wk_` via `DELETE /api-key/:hash`, individually revoke pre-minted sub-keys | Default for stateless apps with vault sovereignty |

The legacy **deterministic `POST /register`** path (5-tuple) does
**NOT support `vault_id`** — the coordinator returns HTTP 400.
Use Path A or B above for vault-scoped sub-wallets.

### Default master vs vault — legacy compatibility

Pre-vault customers register with empty body:

```bash
curl -X POST https://api.outlayer.fastnear.com/register -d '{}'
```

This produces a wk_ tied to the **OutLayer default master** (no
vault_id). Existing pre-vault wallets keep working indefinitely —
the keystore's `derive_keypair(customer = None, seed)` path is
unchanged. Tests: [`tests/vault_backward_compat.sh`](../tests/vault_backward_compat.sh).

---

## File layout

```
vault-contract/
├── src/
│   └── lib.rs                # entire contract: ~1700 lines
├── tests/
│   ├── integration.rs        # near-workspaces sandbox suite
│   └── mock-keystore-dao/    # tiny mock keystore-DAO contract used by integration tests
├── res/
│   └── vault_contract.wasm   # build output, also referenced by tests
├── build.sh                  # non-reproducible build (fast iteration)
├── build-docker.sh           # reproducible build (canonical hash for DAO approval)
├── Cargo.toml                # `test-timing` feature flag declared here
├── GLOBAL-CONTACT.md         # one-time per-network global-contract deploy procedure
└── README.md                 # this file
```

---

## Related repos / paths

| | |
|---|---|
| keystore-dao governance contract | [`../keystore-dao-contract/`](../keystore-dao-contract/) |
| keystore-worker (in-TEE custodian) | [`../keystore-worker/`](../keystore-worker/) |
| coordinator (HTTP API) | [`outlayer-coordinator`](https://github.com/out-layer/outlayer-coordinator) (separate repo) |
| customer-recovery tool | [`../scripts/customer-recovery/`](../scripts/customer-recovery/) |
| dashboard vault UI | [`../dashboard/app/vault/page.tsx`](../dashboard/app/vault/page.tsx) |
| Sovereign exit runbook | [`../docs/LEAVING_OUTLAYER.md`](../docs/LEAVING_OUTLAYER.md) |
| Rendered architecture docs | https://outlayer.fastnear.com/docs/vaults |
