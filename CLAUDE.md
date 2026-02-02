# CLAUDE.md

## Critical Rules

You speak with a technical programmer. Always ask human's point of view when unsure. Say "I don't know how to do it" if unsure - human will provide details.

### NEVER Do
- **Deployment**: Don't restart coordinator, deploy contract, or manage docker - human handles this
- **Summary files**: Don't create DOCUMENTATION_UPDATE_*.md, *_SUMMARY.md, CHANGES.md - human doesn't read them
- **MVP/TODO code**: This is PRODUCTION. Don't leave TODO comments - implement features completely or ask human first. No "for MVP" placeholders
- **Stub implementations**: Never return `vec![]` with "requires implementation" - every public function must work
- **Limited functionality**: Don't implement features for one platform but skip another without asking
- **Arbitrary delays**: No `tokio::time::sleep()` without strong justification - discuss with human first
- **Logs for user errors**: `tracing::debug/warn/error` go to server logs only. Use `anyhow::bail!()` to propagate errors to users

### WASI Development
When writing WASI containers:
1. FIRST read existing examples in `wasi-examples/`
2. ALWAYS follow `wasi-examples/WASI_TUTORIAL.md`
3. Copy structure from working examples
4. Ask human which example to use as template

### NEAR Contract Development
1. Use `cargo near build` (never raw `cargo build --target wasm32-unknown-unknown`)
2. Create `rust-toolchain.toml` with `channel = "1.85.0"`
3. Add `schemars = "0.8"` and derive `JsonSchema` on all public API types
4. Use `#[schemars(with = "String")]` for AccountId fields
5. Use near-sdk 5.9.0

### OutLayer URLs
- **API Base**: `https://api.outlayer.fastnear.com` (for HTTPS API calls)
- **Dashboard**: `https://outlayer.fastnear.com/dashboard` (for user-facing links)
- NEVER use `https://outlayer.fastnear.com` for API calls - always use `api.` subdomain

### Error Propagation
```rust
// WRONG - user sees nothing:
tracing::debug!("Feature X not supported");
return Ok(false);
// CORRECT - user sees the reason:
anyhow::bail!("Feature X is not supported. Please use Y instead.");
```

## Project Overview

**NEAR OutLayer** - verifiable off-chain computation for NEAR smart contracts using TEE (Intel TDX via Phala Cloud).

## Components
| Component | Port | Description |
|-----------|------|-------------|
| `contract/` | - | Main NEAR contract (outlayer.near) |
| `coordinator/` | 8080 | Task queue, WASM cache (PostgreSQL + Redis) |
| `worker/` | - | Polls tasks, compiles GitHub repos, executes WASM |
| `keystore-worker/` | 8081 | Secrets decryption with TEE (via coordinator proxy) |
| `dashboard/` | 3000 | Next.js UI + docs |
| `register-contract/` | - | TEE worker registration contract |
| `keystore-dao-contract/` | - | DAO contract for keystore governance |
| `sdk/` | - | Client SDK for integration |
| `wasi-examples/` | - | WASI container examples |
| `docker/` | - | Docker configs (Phala deployment) |
| `tests/` | - | Integration tests |
| `scripts/` | - | Deployment & utility scripts |

## Commands
```bash
cd contract && ./build.sh                # Build contract
cd coordinator && cargo run              # Run coordinator
cd worker && cargo run                   # Run worker
cd dashboard && npm run dev              # Run dashboard
cd keystore-worker && cargo run          # Run keystore

# Coordinator uses sqlx compile-time query validation.
# Without DB connection, use offline mode:
cd coordinator && SQLX_OFFLINE=true cargo check
```

## Docs
- [PROJECT.md](PROJECT.md) - Tech spec + implementation status
- [dashboard/DOCS_INDEX.md](dashboard/DOCS_INDEX.md) - Integration guides, API reference
- [wasi-examples/WASI_TUTORIAL.md](wasi-examples/WASI_TUTORIAL.md) - WASI guide
- [contract/README.md](contract/README.md) - Contract API
- [worker/README.md](worker/README.md) - Worker config

## Key Database Tables (Coordinator)
| Table | Description |
|-------|-------------|
| `execution_requests` | Stores blockchain execution requests with `attached_usd`, `project_id` |
| `jobs` | Task queue for workers |
| `earnings_history` | Unified earnings log for blockchain + HTTPS calls |
| `project_owner_earnings` | HTTPS earnings balance per project owner |
| `wasm_cache` | Compiled WASM binaries metadata |

## Developer Payments Flow
1. **Blockchain**: User deposits stablecoins → calls `request_execution(attached_usd)` → project owner earns
2. **HTTPS**: User creates payment key → calls API with `X-Attached-Deposit` → project owner earns
3. **Dashboard**: `/earnings` page shows balances and history from both sources
