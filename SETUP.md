# NEAR Offshore MVP - Setup Guide

Complete setup guide for running the NEAR Offshore platform.

## System Requirements

- **Rust** 1.75 or later
- **PostgreSQL** 14 or later
- **Redis** 7 or later
- **Docker** (for compilation sandbox)
- **NEAR CLI** (for account management)

## Architecture Overview

```
Contract (NEAR) → Coordinator API → Worker
                ↓                    ↓
           PostgreSQL/Redis ← WASM Cache
```

## Quick Start

### 1. Start Infrastructure

```bash
# Start PostgreSQL and Redis
cd coordinator
docker-compose up -d

# Verify services are running
docker-compose ps
```

### 2. Configure Coordinator

```bash
cd coordinator

# Copy example config
cp .env.example .env

# Edit .env with your settings
# Minimum required:
# - DATABASE_URL (default: postgres://postgres:postgres@localhost/offchainvm)
# - REDIS_URL (default: redis://localhost:6379)
# - REQUIRE_AUTH=false (for dev) or true (for prod)
```

### 3. Run Database Migrations

```bash
cd coordinator
cargo sqlx database create
cargo sqlx migrate run
```

### 4. Start Coordinator

```bash
cd coordinator
cargo run --release

# Should see:
# Coordinator API server listening on 0.0.0.0:8080
```

### 5. Configure Worker

```bash
cd worker

# Copy example config
cp .env.example .env

# Edit .env - REQUIRED fields:
nano .env
```

Required configuration:
```bash
API_BASE_URL=http://localhost:8080
API_AUTH_TOKEN=dev-token  # Any string for dev mode
NEAR_RPC_URL=https://rpc.testnet.near.org
OFFCHAINVM_CONTRACT_ID=your-contract.testnet
OPERATOR_ACCOUNT_ID=your-worker.testnet
OPERATOR_PRIVATE_KEY=ed25519:...
```

### 6. Start Worker

```bash
cd worker
cargo run --release

# Should see:
# Worker ID: worker-xyz
# Starting worker loop...
```

## Detailed Configuration

### Coordinator API Configuration

The coordinator manages task distribution and WASM caching.

**Environment Variables** (`.env` in `/coordinator`):

```bash
# Server
HOST=0.0.0.0
PORT=8080

# Database
DATABASE_URL=postgres://postgres:postgres@localhost/offchainvm
DB_POOL_SIZE=10

# Redis
REDIS_URL=redis://localhost:6379

# WASM Cache
WASM_CACHE_DIR=/tmp/offchainvm/wasm
WASM_CACHE_MAX_SIZE_GB=10
LRU_EVICTION_CHECK_INTERVAL_SECONDS=300

# Authentication
REQUIRE_AUTH=false  # Set to true for production
```

**Creating Worker Auth Tokens** (if `REQUIRE_AUTH=true`):

```bash
# Connect to PostgreSQL
psql postgres://postgres:postgres@localhost/offchainvm

# Create token (SHA256 hash of "my-secret-token")
INSERT INTO worker_auth_tokens (token_hash, worker_name, is_active)
VALUES (
    encode(sha256('my-secret-token'), 'hex'),
    'worker-1',
    true
);
```

### Worker Configuration

**Environment Variables** (`.env` in `/worker`):

See [worker/.env.example](worker/.env.example) for full list.

**Minimal Configuration:**
```bash
API_BASE_URL=http://localhost:8080
API_AUTH_TOKEN=my-secret-token
NEAR_RPC_URL=https://rpc.testnet.near.org
OFFCHAINVM_CONTRACT_ID=offchainvm.testnet
OPERATOR_ACCOUNT_ID=worker.testnet
OPERATOR_PRIVATE_KEY=ed25519:...
```

**Getting NEAR Private Key:**

```bash
# Using NEAR CLI
near login

# Keys are stored in ~/.near-credentials/
cat ~/.near-credentials/testnet/your-account.testnet.json
```

## Contract Deployment

### 1. Build Contract

```bash
cd contract
cargo build --target wasm32-unknown-unknown --release
```

### 2. Deploy to Testnet

```bash
# Create sub-account for contract
near create-account offchainvm.your-account.testnet \
  --masterAccount your-account.testnet \
  --initialBalance 10

# Deploy contract
near deploy offchainvm.your-account.testnet \
  target/wasm32-unknown-unknown/release/offchainvm.wasm \
  --initFunction new \
  --initArgs '{"owner_id": "your-account.testnet", "operator_id": "worker.testnet"}'
```

### 3. Set Worker as Operator

```bash
near call offchainvm.your-account.testnet \
  set_operator \
  '{"operator_id": "worker.testnet"}' \
  --accountId your-account.testnet
```

## Testing the System

### 1. Create Test WASM

```bash
cd test-wasm
cargo build --release --target wasm32-unknown-unknown

# WASM output:
# target/wasm32-unknown-unknown/release/test_wasm.wasm
```

### 2. Upload Test WASM to GitHub

```bash
git init
git add .
git commit -m "Initial commit"
gh repo create near-offshore-test-wasm --public
git push origin main
```

### 3. Request Execution

```bash
# Call from client contract or directly:
near call offchainvm.your-account.testnet \
  request_execution \
  '{
    "code_source": {
      "repo": "https://github.com/your-username/near-offshore-test-wasm",
      "commit": "main",
      "build_target": "wasm32-unknown-unknown"
    },
    "resource_limits": {
      "max_instructions": 1000000000,
      "max_memory_mb": 128,
      "max_execution_seconds": 60
    },
    "callback_gas": "50000000000000"
  }' \
  --accountId your-account.testnet \
  --amount 1 \
  --gas 300000000000000
```

### 4. Monitor Execution

**Check Coordinator Logs:**
```bash
cd coordinator
# Watch for incoming tasks
tail -f logs/coordinator.log
```

**Check Worker Logs:**
```bash
cd worker
RUST_LOG=debug cargo run

# Should see:
# Received task: Compile { ... }
# Compiling code source...
# Compilation successful
# Received task: Execute { ... }
# WASM execution succeeded
```

**Check Contract State:**
```bash
# View pending requests
near view offchainvm.your-account.testnet \
  get_request '{"request_id": 0}'
```

## Troubleshooting

### Coordinator Issues

**Port already in use:**
```bash
# Change port in coordinator/.env
PORT=8081
```

**Database connection error:**
```bash
# Check PostgreSQL is running
docker-compose ps

# Test connection
psql postgres://postgres:postgres@localhost/offchainvm
```

**Redis connection error:**
```bash
# Check Redis
redis-cli ping
# Should return: PONG
```

### Worker Issues

**Cannot connect to coordinator:**
```bash
# Test API connectivity
curl http://localhost:8080/health
# Should return: "OK"
```

**NEAR transaction errors:**
```bash
# Check account has balance
near state worker.testnet

# Check operator role
near view offchainvm.your-account.testnet get_operator
```

**Docker compilation fails:**
```bash
# Check Docker is running
docker ps

# Pull Rust image manually
docker pull rust:1.75
```

## Production Deployment

### Security Checklist

- [ ] Enable authentication (`REQUIRE_AUTH=true`)
- [ ] Use strong worker tokens (64+ random characters)
- [ ] Use HTTPS for coordinator API
- [ ] Restrict coordinator API to worker IPs only
- [ ] Enable Docker resource limits
- [ ] Set up log rotation
- [ ] Monitor disk usage for WASM cache
- [ ] Use dedicated NEAR accounts for operators
- [ ] Keep operator private keys secure (use env vars, not files)

### Recommended Setup

**Coordinator:**
- Run behind nginx/traefik with HTTPS
- Use managed PostgreSQL (AWS RDS, etc.)
- Use managed Redis (AWS ElastiCache, etc.)
- Set up monitoring (Prometheus/Grafana)

**Worker:**
- Run multiple workers for redundancy
- One worker with `ENABLE_EVENT_MONITOR=true`
- Others with `ENABLE_EVENT_MONITOR=false`
- Use systemd for auto-restart
- Set up log aggregation

### Systemd Service Example

```ini
# /etc/systemd/system/offchainvm-worker.service
[Unit]
Description=OffchainVM Worker
After=network.target

[Service]
Type=simple
User=offchainvm
WorkingDirectory=/opt/offchainvm/worker
EnvironmentFile=/opt/offchainvm/worker/.env
ExecStart=/opt/offchainvm/worker/target/release/offchainvm-worker
Restart=always
RestartSec=10

[Install]
WantedBy=multi-user.target
```

## Monitoring

### Health Checks

**Coordinator:**
```bash
curl http://localhost:8080/health
```

**Worker:**
Check logs for "Starting worker loop" message

### Metrics to Monitor

- **Coordinator:**
  - HTTP request rate/latency
  - PostgreSQL connection pool usage
  - Redis connection status
  - WASM cache disk usage
  - Task queue length

- **Worker:**
  - Task processing rate
  - Compilation success/failure rate
  - WASM execution time
  - NEAR transaction success rate
  - Docker container resource usage

## Next Steps

- Set up monitoring and alerting
- Implement backup strategy for PostgreSQL
- Configure log rotation
- Set up CI/CD for automatic deployments
- Implement rate limiting
- Add metrics collection (Prometheus)
- Create operational runbooks

## Support

For issues and questions:
- GitHub Issues: https://github.com/your-org/near-offshore/issues
- Documentation: See [PROJECT.md](PROJECT.md)
- Examples: See [test-wasm/](test-wasm/)
