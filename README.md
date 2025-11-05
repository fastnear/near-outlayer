# NEAR OutLayer (OffchainVM)

**OutLayer execution for on-chain contracts**

Verifiable off-chain computation platform for NEAR smart contracts using yield/resume mechanism.

## Project Structure

```
near-outlayer/
├── contract/          # NEAR smart contract (outlayer.near)
├── coordinator/       # Coordinator API server (Rust + Axum)
├── worker/           # Worker nodes (Rust + Tokio)
├── wasi-examples/    # WASI example projects (random-ark, ai-ark, etc.)
├── scripts/          # Deployment scripts
├── docker/           # Docker configurations
└── docs/             # Documentation
```

## Quick Start

### Prerequisites

- Rust 1.75+
- Docker
- PostgreSQL 14+
- Redis 7+
- NEAR CLI

### 1. Deploy Contract

```bash
cd contract
cargo near build --release
near contract deploy outlayer.testnet use-file res/local/outlayer_contract.wasm with-init-call new json-args '{"owner_id":"outlayer.testnet","operator_id":"worker.outlayer.testnet"}' prepaid-gas '100.0 Tgas' attached-deposit '0 NEAR' network-config testnet sign-with-keychain send
```

### 2. Setup Infrastructure

```bash
# Start PostgreSQL and Redis
docker-compose up -d postgres redis

# Run database migrations
cd coordinator
cargo install sqlx-cli --no-default-features --features rustls,postgres
sqlx migrate run
```

### 3. Start Keystore (Optional - for encrypted secrets)

```bash
cd keystore-worker
docker-compose up -d

# Or run without Docker:
pip install -r requirements.txt
python src/api.py
```

**Important**: If running coordinator in Docker, set `KEYSTORE_BASE_URL=http://host.docker.internal:8081` in `coordinator/.env`

### 4. Start Coordinator API

```bash
cd coordinator
cargo run --release

# Or with Docker:
docker-compose up -d coordinator
```

### 5. Start Workers

```bash
cd worker
# Worker 1 (with event monitor)
ENABLE_EVENT_MONITOR=true cargo run --release

# Worker 2 (execution only)
cargo run --release
```

## Documentation

- [PROJECT.md](PROJECT.md) - Technical specification
- [MVP_DEVELOPMENT_PLAN.md](MVP_DEVELOPMENT_PLAN.md) - Development roadmap
- [NEAROffshoreOnepager.md](NEAROffshoreOnepager.md) - Marketing one-pager

## Architecture

### Core Components

1. **Smart Contract** (`outlayer.near`)
   - Manages execution requests
   - Handles payments and refunds
   - Yield/resume mechanism integration

2. **Coordinator API Server**
   - Central HTTP API server (port 8080)
   - PostgreSQL + Redis + Local WASM cache
   - Worker authentication via bearer tokens
   - LRU cache eviction
   - Proxies requests to Keystore (isolated)
   - GitHub API integration with Redis caching

3. **Keystore Worker** (Optional)
   - Encrypted secrets management (port 8081)
   - **Isolated**: Only accessible via Coordinator proxy
   - Access control validation (Whitelist, NEAR/FT/NFT balance, Logic)
   - Public key generation for client-side encryption

4. **Workers**
   - Poll tasks from Coordinator API
   - Compile GitHub repos to WASM (Docker sandboxed)
   - Execute WASM with resource limits (wasmi)
   - Report results to contract
   - Decrypt secrets via Keystore (if provided)

5. **Dashboard** (Optional)
   - Next.js web UI (port 3000)
   - Secrets management interface
   - Execution history and monitoring
   - Worker status tracking

6. **Test WASM Projects**
   - Random number generator (random-ark)
   - AI integration examples (ai-ark)
   - Used for end-to-end testing

## Development

### Build All Components

```bash
# Contract
cd contract && cargo near build

# Coordinator
cd coordinator && cargo build --release

# Worker
cd worker && cargo build --release

# Test WASM
clone https://github.com/zavodil/random-ark
cd random-ark && cargo build --target wasm32-wasi --release
```

### Run Tests

```bash
# Contract tests
cd contract && cargo test

# Coordinator tests
cd coordinator && cargo test

# Worker tests
cd worker && cargo test
```

## License

MIT
