# Testing — Per-customer Vaults

Comprehensive guide to verifying the per-customer vault rollout
(Phases 1-10 of `partitioned-dreaming-patterson.md`). Read end-to-end
before launch; each section has a "what it covers" + "how to run" +
"what passing means" structure.

## Prerequisites

| Tool | Version | Why |
|---|---|---|
| **rustc** | `1.88.0` | Several transitive deps (`time-macros`, `darling`, `alloy-*`, `nybbles`) bumped MSRV to 1.88. Earlier toolchains fail at compile with "requires rustc 1.88.0". `rustup install 1.88.0` then prefix cargo commands with `+1.88.0` OR set as default. The `vault-contract` crate has its own `rust-toolchain.toml` pinning 1.85.0 for the contract itself (intentional — production WASM should be reproducible against a known toolchain), but its **dev-dependencies** need 1.88. |
| **cargo near** | `0.16.0+` | `cargo install cargo-near` — used to build vault-contract WASM with the right metadata. |
| **NEAR CLI** | `near-cli-rs` | `npm i -g near-cli-rs`. Used by `tests/vault_recovery_e2e.sh`. |
| **Node + npm** | `20+` | Dashboard build / type-check. |
| **Local services** | coordinator + keystore-worker | E2E scripts assume `localhost:8080` (coordinator). Keystore-worker URL is internal — operator config. |

---

## Quick: full sweep (CI smoke check)

This is what should run on every PR. ~70 seconds when caches are
warm. **All must pass before merge.**

```bash
# From repo root
RUSTC=1.88.0

echo "=== keystore-worker ===" && \
  cargo +$RUSTC test --quiet --manifest-path keystore-worker/Cargo.toml --bins

echo "=== keystore-dao-contract ===" && \
  cargo +$RUSTC test --quiet --manifest-path keystore-dao-contract/Cargo.toml --lib

echo "=== vault-contract lib ===" && \
  cargo +$RUSTC test --quiet --manifest-path vault-contract/Cargo.toml --lib

echo "=== vault-contract integration (sandbox) ===" && \
  cargo test --quiet --manifest-path vault-contract/Cargo.toml \
        --features test-timing --test integration

echo "=== outlayer-monitor (default + lake-source) ===" && \
  cargo +$RUSTC test --quiet --manifest-path outlayer-monitor/Cargo.toml --bins && \
  cargo +$RUSTC test --quiet --manifest-path outlayer-monitor/Cargo.toml --bins --features lake-source

echo "=== vault-checker WASI ===" && \
  cargo +$RUSTC test --quiet --manifest-path wasi-examples/vault-checker/Cargo.toml --bins && \
  cargo +$RUSTC build --quiet --target wasm32-wasip2 --release \
        --manifest-path wasi-examples/vault-checker/Cargo.toml

echo "=== outlayer-cli (separate repo) ===" && \
  cargo +$RUSTC test --quiet --manifest-path ../outlayer-cli/Cargo.toml --lib

echo "=== outlayer-coordinator (separate repo) ===" && \
  SQLX_OFFLINE=true cargo +$RUSTC test --quiet \
        --manifest-path ../outlayer-coordinator/Cargo.toml --bins

echo "=== dashboard ===" && \
  (cd dashboard && npx tsc --noEmit && npm run build)
```

**Expected counts:**

| Crate / target | Test count | Notes |
|---|---|---|
| keystore-worker | 148 | |
| keystore-dao-contract | 26 | Includes 4 quorum tests for vault-version multisig |
| vault-contract lib | 28 | |
| vault-contract integration | 22 (1 ignored) | Real near-sandbox; ~60s |
| outlayer-monitor default | 25 | Includes 9 `parse_vault_log` tests |
| outlayer-monitor lake-source | 25 | Same suite, feature-gated path |
| outlayer-cli | 2 lib + 15 integration | |
| outlayer-coordinator | 122 (1 ignored) | |
| vault-checker | 19 | |
| dashboard tsc + build | clean (0 errors) | `/vault`, `/docs/vaults`, `/wallet` routes present |

**Total: 432 tests, 0 failed, 2 ignored (diagnostic only).**

---

## Per-crate detail

### `keystore-worker/`

**What it covers**: Multi-customer master cache, lazy MPC CKD load,
per-customer derivation isolation, all 13+ wallet+secret API
handlers with vault scope, `/sign-vault-verification`,
`/admin/evict-customer`, `/admin/ban-vault`, `/derive-vault-tee-key`,
worker-token vs coordinator-token auth lanes.

**Run**: `cargo +1.88.0 test --manifest-path keystore-worker/Cargo.toml --bins`

**What passing means**: Multi-customer isolation enforced; backward
compat with default-master-only requests; lazy load gate respects
`is_vault_verified`; `/admin/ban-vault` is idempotent; vault routes
accept both coord and worker tokens.

### `keystore-dao-contract/`

**What it covers**: DAO governance for keystore TEE registration;
vault registry (verified set, banned set, code-hash whitelist with
**multisig** approve/revoke after Phase 7 audit F4 fix); cessation
flag (declare/revoke); `mark_vault_verified` access-key gate.

**Run**: `cargo +1.88.0 test --manifest-path keystore-dao-contract/Cargo.toml --lib`

**Highlights**: Tests for `approve_vault_version_does_not_execute_on_single_vote`,
`approve_vault_version_distinct_args_are_independent_proposals`,
`approve_vault_version_3_of_5_quorum`. Verifies the master cannot
be revoked: `approved_keystores: UnorderedSet<PublicKey>` has no
removal method, only `submit_keystore_registration` adds.

### `vault-contract/`

**Library tests** (28): pure logic — `assert_exit_window_in_range`,
`new()` defaults, recovery state transitions, MAX_REGISTERED_TEE_KEYS
cap, `unlocked_add_key` parent-only gate.

**Integration tests** (22, near-sandbox): full happy paths against
real sandbox neard.

```bash
cargo test --manifest-path vault-contract/Cargo.toml --features test-timing --test integration
```

**Highlights** — these are the reason Phase 10 S3/S4 are covered in
automation:

- `cessation_full_happy_path_unlocks_after_7d` — declare cessation
  → initiate → fast-forward 30s (test-timing) → finalize → unlocked.
- `cessation_recovery_cancelled_if_dao_revokes` — start, then DAO
  revokes mid-window → finalize sees `is_ceased == false` → state
  cleared, vault stays locked.
- `cessation_finalize_after_14d_clears_state` — past
  `finalize_before` → recovery auto-cancels.
- `unilateral_full_happy_path_24h_window` — parent calls
  unilateral_initiate → wait window → finalize → unlocked.
- `set_exit_window_then_initiate_uses_new_window` — exit window
  config respected.
- `propose_tee_key_*` — DAO callback gates, max cap, duplicate
  rejection.
- `unlocked_add_key_*` — full-access vs FCAK paths, default
  allowance, parent-only gate after unlock.

**Why test-timing feature**: collapses 7-day cessation delay to 30s
and 24h unilateral minimum to 10s so the suite runs in ~60s instead
of needing fast-forward of 1.3M sandbox blocks.

### `outlayer-monitor/`

**What it covers**: Race-attack detection (duplicate MPC calls),
vault contract event forwarding, RPC cross-check, persistent
`last_processed_block` checkpoint.

**Default features** (`Detector` + `Sinks` only, no production source):
```bash
cargo +1.88.0 test --manifest-path outlayer-monitor/Cargo.toml --bins
```

**`lake-source` feature** (also exercises `parse_vault_log` parser
unit tests for vault contract logs):
```bash
cargo +1.88.0 test --manifest-path outlayer-monitor/Cargo.toml --bins --features lake-source
```

**Highlights**: `pipeline_dispatches_mcp_and_vault_events_separately`
(end-to-end: MPC receipts → ban; vault logs → forward — separate
sinks, no cross-talk); `parse_vault_log_*` (9 tests covering
vault-emitted, DAO-emitted, malformed, unknown-account paths).

### `outlayer-cli/` (separate repo at `../outlayer-cli`)

**What it covers**: All `outlayer vault` subcommands, atomic-deploy
action builder, exit-window parser, name validation.

```bash
cargo +1.88.0 test --manifest-path ../outlayer-cli/Cargo.toml --lib
```

**Highlights**: `parse_exit_window_basic` (24h/7d/30d → seconds),
`parse_exit_window_rejects_bad`. Integration tests for vault-init
flow are deferred to `tests/vault_e2e.sh` and
`tests/vault_recovery_e2e.sh` since they need a live coordinator.

### `outlayer-coordinator/` (separate repo at `../outlayer-coordinator`)

**What it covers**: `/customer/register`, `/customer/derive-tee-key`,
`/customer/sign-verification`, `/customer/list-vaults`,
`/internal/vault-event` proxies; auth handlers; webhook delivery
(generic infra now usable for vault events).

```bash
SQLX_OFFLINE=true cargo +1.88.0 test \
    --manifest-path ../outlayer-coordinator/Cargo.toml --bins
```

`SQLX_OFFLINE=true` — coordinator uses sqlx compile-time SQL checks
against a live DB. Without `SQLX_OFFLINE` the test environment must
have PostgreSQL running with the schema migrated.

### `wasi-examples/vault-checker/`

**Native** (build verification + non-WASM tests):
```bash
cargo +1.88.0 test --manifest-path wasi-examples/vault-checker/Cargo.toml --bins
```

**WASM build** (target the actual deploy artefact):
```bash
cargo +1.88.0 build --target wasm32-wasip2 --release \
    --manifest-path wasi-examples/vault-checker/Cargo.toml
```

**What passing means**: All 5 verification checks (`is_vault_code_approved`,
`view_account.code_hash`, `view_access_key_list`, `vault.get_state()`,
`registered_tee_keys ⊆ access_keys`) compile and run on the
non-WASM `cargo test` path against mock RPC. WASM build produces
a deployable agent ready for `outlayer.near/vault-checker`.

### `dashboard/`

**Type-check + build**:
```bash
cd dashboard
npx tsc --noEmit
npm run build
```

**Routes that must appear in build output**:
- `/vault` — vault management page
- `/docs/vaults` — customer-facing docs
- `/wallet` — vault binding display

**Manual UI smoke** (after `npm run dev`):
1. Open `/vault`, verify create form + inspect form render.
2. Connect a wallet (testnet account with NEAR balance).
3. Try "Create vault" with the test-timing build (or testnet —
   note the 24h delay).
4. Open `/secrets`, confirm the "Encryption master" toggle is
   visible (only in non-update mode).
5. Open `/wallet?key=<api_key>` for a vault-bound key, verify
   "Master key: Vault X" shows.

### Vault WASM resolution

The CLI and dashboard no longer bundle a copy of `vault_contract.wasm`.
Both resolve the approved code hash at deploy time by view-calling
`keystore-DAO::list_approved_vault_versions()` and use a NEP-591
`UseGlobalContract(code_hash)` action — the WASM bytes themselves live
in NEAR's global-contract registry, not in our repo binaries. So the
canonical artefact (`vault-contract/res/vault_contract.wasm`) only
needs to exist so the operator can `cargo near deploy` it as a global
contract and submit its hash to `approve_vault_version` on the DAO.
There is nothing to "sync" across binaries.

---

## E2E scenarios

### `tests/vault_e2e.sh` — happy / isolation / backward-compat

```bash
# Dry-run (prints commands without executing)
./tests/vault_e2e.sh all

# Real execution against testnet
NETWORK=testnet \
CUSTOMER_A=alice.testnet \
CUSTOMER_B=bob.testnet \
COORDINATOR_URL=http://localhost:8080 \
  ./tests/vault_e2e.sh all --apply
```

**Scenarios automated**:
1. **Happy path** — `outlayer vault init` → `vault status` →
   `vault verify` → wallet API derive-address → NEP-413 sign.
2. **Multi-customer isolation** — A and B each deploy a vault →
   API keys are bound to vault, X-Customer-Vault header from one
   wallet ignored when API key is from another.
3. **Backward compat** — `POST /register {}` without vault scope
   still produces a default-master wallet that works.

**Manual sub-scenarios** (printed by `manual` arg, see plan §10):
- S2 keystore-worker code update + DAO vote + propose_tee_key (live DAO)
- S3 cessation full happy path (7d wait, or test-timing build)
- S4 cessation cancelled (DAO revoke mid-window)
- S7 alpha tester walkthrough

### `tests/vault_recovery_e2e.sh` — unilateral recovery on testnet

```bash
PARENT=alice.testnet \
VAULT_NAME=recovery-$(date +%s) \
  ./tests/vault_recovery_e2e.sh unilateral --apply
```

**What it does**: builds vault-contract with `--features test-timing`
(10s exit window), deploys to fresh testnet sub-account, drives full
unilateral recovery cycle in <60 seconds, asserts `unlocked == true`,
installs parent's full-access key. Cleanup hint at the end.

**Note**: this WASM has a different sha256 than the production build
and **will fail vault-checker** — that's deliberate. It's for
recovery flow testing only, not for production registration.

---

## What is NOT covered by automated tests

These are tracked in `memory/project_vault_operator_debts.md`
and require operator-team action before launch:

1. **`outlayer-monitor` with `--features lake-source` deployed to
   real testnet** — automated tests cover the source filter logic
   and the dispatch pipeline. Operator must wire neardata feed
   credentials (none required for free tier) and observe a real
   race-attack scenario (or simulate one) to flip
   `--auto-ban-enabled`.

2. **Phase 10 S2** — keystore-worker v2 build, DAO members vote on
   `submit_keystore_registration`, customer calls `propose_tee_key`
   on existing vault. Needs live DAO membership + a v2 keystore
   binary.

3. **Phase 10 S4 with real DAO** — sandbox covers the contract
   logic (cessation_recovery_cancelled_if_dao_revokes); a live
   testnet run requires DAO members willing to declare/revoke
   cessation as a drill.

4. **Phase 10 S7** — alpha-tester onboarding. One trusted external
   user walks through `outlayer login → vault init → derive →
   verify → initiate-unilateral-recovery`, captures UX feedback.

5. **Race-attack simulation** — `outlayer-monitor` ban path tested
   with mocks; deliberate race attack on testnet (sneak backup
   key, observe MPC tx, replay) is operator drill, not CI.

6. **Dashboard screenshots in `/docs/vaults`** — to be added after
   first testnet UI run.

7. **Webhook deliveries end-to-end** — coordinator's
   `enqueue_webhook` is unit-tested; actual HTTPS delivery to a
   customer endpoint with retries needs an integration env.

---

## Common breakage modes

| Symptom | Likely cause | Fix |
|---|---|---|
| `time-macros@0.2.27 requires rustc 1.88.0` (any crate) | Default rustc is older than 1.88 | `rustup install 1.88.0` then `cargo +1.88.0 test ...` |
| `CompilationError(PrepareError(Deserialization))` in vault-contract integration | The cargo-near / near-sdk / sandbox neard triple has drifted out of sync. Fixed as of 2026-05-11 with cargo-near `0.20.1` + near-workspaces `0.22.1` (sandbox 2.11.0) + an explicit `--override-toolchain 1.85.0` in `vault-contract/tests/integration.rs::build_with_features`. If the error returns after a future bump, the override pin in integration.rs is the first thing to revisit. | Verify cargo-near version (`cargo near --version` ≥ 0.20). Confirm the override-toolchain flag is still present. If a newer cargo-near re-tightens the rustc gate, sync `1.85.0` with whatever cargo-near's `--help` documents as the max supported version. |
| `wasm, compiled with 1.87.0 or newer rust toolchain is currently not compatible with nearcore VM` | cargo-near 0.20 refuses to build WASM with rustc ≥ 1.87, but `cargo +1.88.0 test` propagates `RUSTUP_TOOLCHAIN=1.88.0` to child `cargo near build` invocations. | The fix is in-tree: `tests/integration.rs::build_with_features` already passes `--override-toolchain 1.85.0`. If this re-appears, check that the args vector still includes that pair. |
| Dashboard build fails on `/vault` route | `lib/vault.ts` imports break | Re-run `npx tsc --noEmit` for actual error; usually a missing field on `VaultListEntry` or `RecoveryState` |
| Coordinator integration tests fail with sqlx errors | `SQLX_OFFLINE=true` not set, or sqlx-data.json stale | Set the env var, OR `cargo sqlx prepare` against a live DB |

---

## Pre-launch checklist

Before flipping `auto_ban_enabled = true` in production:

- [ ] All quick-sweep tests green (run on the actual deploy commit, not main)
- [ ] `tests/vault_e2e.sh all --apply` against testnet — happy/isolation/compat all pass
- [ ] `tests/vault_recovery_e2e.sh unilateral --apply` — full cycle ≤ 60s
- [ ] `outlayer-monitor` deployed in alert-only mode for ≥7 days, no false positives in logs
- [ ] Manual UI walkthrough of `/vault` page on testnet by ≥1 non-developer
- [ ] DAO vote done to add the production vault-contract WASM hash to `keystore-dao.approved_vault_code_hashes` (multisig → ≥ approval_threshold members)
- [ ] Operator-debts file (`memory/project_vault_operator_debts.md`) reviewed; each item either resolved or "we accept this risk because…" documented
