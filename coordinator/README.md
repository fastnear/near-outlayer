# Coordinator

Central orchestration service for NEAR OutLayer. Manages task queues, WASM compilation cache, worker coordination, HTTPS API, and payment processing.

## Stack

- **Framework**: Axum 0.7
- **Database**: PostgreSQL (sqlx with compile-time query validation)
- **Queue**: Redis
- **Auth**: TEE challenge-response + Bearer token

## Run

```bash
cargo run
# Or with offline sqlx validation (no DB needed for compilation):
SQLX_OFFLINE=true cargo check
```

Default: `http://localhost:8080`

## Core Functions

| Function | Description |
|----------|-------------|
| Task queue | Redis-backed job queue, workers claim compile/execute jobs |
| WASM cache | LRU cache for compiled WASM binaries (default 50GB) |
| Worker management | Heartbeats, TEE session auth, worker pool tracking |
| HTTPS API | `POST /call/{owner}/{project}` — payment key authenticated execution |
| Secrets proxy | Routes secret operations to TEE keystore service |
| Storage | Encrypted per-project storage + public cross-project storage |
| Billing | Dual earnings model: blockchain transactions + HTTPS API calls |

## Main Routes

### Worker API (TEE session auth)

```
POST   /jobs/claim                         # Claim job from queue
POST   /jobs/complete                      # Submit job results
GET    /wasm/:checksum                     # Download compiled WASM
POST   /wasm/upload                        # Upload compiled WASM
POST   /workers/heartbeat                  # Worker keepalive
POST   /workers/tee-challenge              # Initiate TEE session
POST   /workers/register-tee              # Register TEE worker
```

### HTTPS API (Payment Key auth)

```
POST   /call/:owner/:project              # Execute WASM
GET    /calls/:call_id                     # Poll call result (async mode)
GET    /payment-keys/balance               # Check payment key balance
```

### Public (no auth)

```
GET    /health                             # Liveness
GET    /health/detailed                    # Full system health (DB, Redis, workers, keystore)
GET    /public/stats                       # System statistics
GET    /public/workers                     # Active workers
GET    /public/jobs                        # Recent jobs
GET    /public/pricing                     # Current pricing config
GET    /public/storage/get                 # Read public storage
GET    /attestations/:job_id               # TDX attestation data
```

### Admin (Bearer token auth)

```
POST   /admin/grant-payment-key            # Fund payment key for testing
DELETE /admin/workers/:worker_id           # Deregister worker
```

## Environment Variables

### Required

| Variable | Default | Description |
|----------|---------|-------------|
| `DATABASE_URL` | `postgres://postgres:postgres@localhost/offchainvm` | PostgreSQL connection |
| `REDIS_URL` | `redis://localhost:6379` | Redis connection |

### Server

| Variable | Default | Description |
|----------|---------|-------------|
| `HOST` | `0.0.0.0` | Bind address |
| `PORT` | `8080` | HTTP port |
| `CORS_ALLOWED_ORIGINS` | `http://localhost:3000` | Comma-separated origins |

### NEAR

| Variable | Default | Description |
|----------|---------|-------------|
| `NEAR_RPC_URL` | `https://rpc.testnet.near.org` | NEAR RPC endpoint |
| `OFFCHAINVM_CONTRACT_ID` | `outlayer.testnet` | OutLayer contract |
| `OPERATOR_ACCOUNT_ID` | — | Account for TEE key verification |

### WASM Cache

| Variable | Default | Description |
|----------|---------|-------------|
| `WASM_CACHE_DIR` | `/var/offchainvm/wasm` | Cache directory |
| `WASM_CACHE_MAX_SIZE_GB` | `50` | Max cache size |

### Keystore

| Variable | Default | Description |
|----------|---------|-------------|
| `KEYSTORE_BASE_URL` | — | Keystore service URL |
| `KEYSTORE_AUTH_TOKEN` | — | Auth token for keystore |

### Auth & Security

| Variable | Default | Description |
|----------|---------|-------------|
| `REQUIRE_AUTH` | `true` | Enforce worker auth |
| `ADMIN_BEARER_TOKEN` | `change-this-in-production` | Admin API token |
| `REQUIRE_TEE_SESSION` | `false` | Require TEE for execution workers |

### Billing

| Variable | Default | Description |
|----------|---------|-------------|
| `STABLECOIN_CONTRACT` | USDC contract | Stablecoin for billing |
| `STABLECOIN_DECIMALS` | `6` | Token decimals |
| `DEFAULT_COMPUTE_LIMIT` | `10000` | Default budget ($0.01) |
| `HTTPS_CALL_TIMEOUT_SECONDS` | `300` | Max execution time |

## Database Tables

| Table | Description |
|-------|-------------|
| `execution_requests` | Blockchain execution requests with `attached_usd`, `project_id` |
| `jobs` | Task queue (compile/execute jobs) |
| `wasm_cache` | Compiled WASM metadata |
| `worker_status` | Active workers and heartbeats |
| `execution_results` | Job outputs and errors |
| `payment_keys` | HTTPS API payment key balances |
| `earnings_history` | Unified earnings log (blockchain + HTTPS) |
| `project_owner_earnings` | HTTPS earnings balance per project owner |
| `storage_keys` | Project encrypted/public storage |

## Rate Limiting

- `/call/*`, `/calls/*`: 100 req/min per IP
- `/public/storage/*`: 100 req/min per IP
- `/secrets/*`: 10 req/min per IP

## Background Jobs

- **LRU eviction**: Hourly, removes oldest unused WASM when cache exceeds max size
- **Payment key cleanup**: Every 5 min, deletes stale uninitialized keys
- **TEE challenge cleanup**: Every 60s, expires old auth tokens
