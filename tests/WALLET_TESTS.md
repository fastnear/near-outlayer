# Wallet Tests

## Unit Tests (no infrastructure needed)

```bash
# All wallet unit tests at once
cd coordinator && SQLX_OFFLINE=true cargo test wallet
cd keystore-worker && cargo test test_policy
cd worker && cargo test wallet --lib

# Individual modules
cd coordinator && SQLX_OFFLINE=true cargo test wallet::auth
cd coordinator && SQLX_OFFLINE=true cargo test wallet::types
cd coordinator && SQLX_OFFLINE=true cargo test wallet::policy
cd coordinator && SQLX_OFFLINE=true cargo test wallet::nonce
cd coordinator && SQLX_OFFLINE=true cargo test wallet::webhooks
cd coordinator && SQLX_OFFLINE=true cargo test wallet::handlers
```

## Integration Tests (require running services)

### Prerequisites

Coordinator, keystore, PostgreSQL, and Redis must be running.

### Mode 1 — Simple Agent (no policy)

Agent registers via POST /register and tests all wallet methods with API key auth.

```bash
./tests/wallet_mode1_agent.sh
```

### Mode 2 — User with Policy

Registers wallet + approver, encrypts a policy (limits, whitelist, approval threshold), then tests enforcement and multisig approval flow.

```bash
./tests/wallet_mode2_policy.sh
```

**Note:** Some Mode 2 tests require the encrypted policy to be stored on-chain. Without that, the policy engine returns "no policy" and allows everything. Tests that depend on active policy will show as SKIP.

### Run everything

```bash
./tests/run_all.sh
```

## Test Counts

| Layer | Location | Tests |
|-------|----------|-------|
| Unit | `keystore-worker/src/api.rs` (evaluate_policy) | 21 |
| Unit | `coordinator/src/wallet/auth.rs` | 18 |
| Unit | `coordinator/src/wallet/types.rs` | 5 |
| Unit | `coordinator/src/wallet/policy.rs` | 4 |
| Unit | `coordinator/src/wallet/nonce.rs` | 3 |
| Unit | `coordinator/src/wallet/webhooks.rs` | 4 |
| Unit | `coordinator/src/wallet/handlers.rs` | 3 |
| Unit | `worker/src/outlayer_wallet/host_functions.rs` | 3 |
| Integration | `tests/wallet_mode1_agent.sh` | 21 |
| Integration | `tests/wallet_mode2_policy.sh` | 17 |
| **Total** | | **99** |
