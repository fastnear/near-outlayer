# NEAR OutLayer (OffchainVM)

**OutLayer execution for on-chain contracts**

Verifiable off-chain computation platform for NEAR smart contracts using yield/resume mechanism.

## Project Structure

```
near-outlayer/
├── contract/          # NEAR smart contract (outlayer.near)
├── register-contract/ # Worker registration contract (TEE attestation)
├── coordinator/       # Coordinator API server (Rust + Axum)
├── worker/           # Worker nodes (Rust + Tokio)
├── keystore-worker/  # Encrypted secrets management (Python + FastAPI)
├── dashboard/        # Web UI (Next.js + React)
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

### 1. Deploy Contracts

```bash
# Main contract
cd contract
cargo near build --release
near contract deploy outlayer.testnet use-file res/local/outlayer_contract.wasm with-init-call new json-args '{"owner_id":"outlayer.testnet","operator_id":"worker.outlayer.testnet"}' prepaid-gas '100.0 Tgas' attached-deposit '0 NEAR' network-config testnet sign-with-keychain send

# Worker registration contract (for TEE workers)
cd ../register-contract
cargo near build --release
near contract deploy register.outlayer.testnet use-file res/local/register_contract.wasm with-init-call new json-args '{}' prepaid-gas '100.0 Tgas' attached-deposit '0 NEAR' network-config testnet sign-with-keychain send
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

#### Option A: Local Development Worker (Docker compilation)

```bash
cd worker
cp .env.example .env
# Edit .env with your configuration

# Worker 1 (with event monitor)
ENABLE_EVENT_MONITOR=true cargo run --release

# Worker 2 (execution only)
cargo run --release
```

#### Option B: TEE Worker (Phala Cloud - native compilation)

**For TEE environments** (Phala, Intel TDX) where Docker-in-Docker doesn't work:

```bash
# Build Docker image with native Rust compiler
./scripts/build_and_push_phala.sh <dockerhub-username> latest worker-compiler

# Deploy to Phala Cloud
cd docker
cp .env.testnet-worker-phala.example .env.testnet-worker-phala
# Edit .env.testnet-worker-phala with your configuration

phala deploy \
  --name outlayer-testnet-worker \
  --compose docker-compose.worker-compiler.phala.yml \
  --env-file .env.testnet-worker-phala \
  --vcpu 2 --memory 4G --disk-size 10G \
  --kms-id phala-prod10
```

**TEE Worker Features**:
- ✅ **Native WASI compilation** (no Docker-in-Docker)
- ✅ **Environment isolation** via `env -i` (clears all worker secrets)
- ✅ **Resource limits** via `ulimit` (2GB RAM, 5min CPU, 1024 processes)
- ✅ **Intel TDX attestation** (hardware-level isolation)
- ✅ **Build.rs validation** (rejects malicious build scripts)
- ✅ **Auto-registration** via register-contract (generates keypair in TEE)
- ✅ **Secrets decryption** via Keystore with access control

**Security Model**:
1. **Environment isolation**: `env -i` clears OPERATOR_PRIVATE_KEY and other worker secrets
2. **Process isolation**: Linux kernel prevents memory access between processes
3. **Hardware isolation**: Intel TDX encrypts memory, protects from host
4. **Resource limits**: Prevents DoS attacks (memory bombs, infinite loops)
5. **Build.rs blocked**: No arbitrary code execution during compilation
6. **Network allowed**: Needed for crates.io (cargo downloads dependencies)

**Image size**: ~700-800MB (includes Rust toolchain + WASI SDK)
**Use case**: Production deployments in TEE environments

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
   - Repo-based encrypted secrets storage

2. **Worker Registration Contract** (`register.outlayer.testnet`)
   - TEE worker registration and verification
   - Stores worker public keys and attestation data
   - Validates Intel TDX quotes (RTMR measurements)
   - Manages worker stake and slashing conditions

3. **Coordinator API Server**
   - Central HTTP API server (port 8080)
   - PostgreSQL + Redis + Local WASM cache
   - Worker authentication via bearer tokens
   - LRU cache eviction
   - Proxies requests to Keystore (isolated)
   - GitHub API integration with Redis caching

4. **Keystore Worker** (Optional)
   - Encrypted secrets management (port 8081)
   - **Isolated**: Only accessible via Coordinator proxy
   - Access control validation (Whitelist, NEAR/FT/NFT balance, Logic)
   - Public key generation for client-side encryption
   - TEE attestation verification

5. **Workers**
   - Poll tasks from Coordinator API
   - **Two compilation modes**:
     - **Docker mode**: Docker-in-Docker compilation (local dev)
     - **Native mode**: env isolation + ulimit (TEE environments)
   - Execute WASM with resource limits (wasmi)
   - Report results to contract with attestation
   - Decrypt secrets via Keystore (if provided)
   - **TEE Support**: Intel TDX attestation via dstack.sock

6. **Dashboard** (Optional)
   - Next.js web UI (port 3000)
   - Secrets management interface
   - Execution history and monitoring
   - Worker status tracking
   - TEE attestation verification

7. **Test WASM Projects**
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

## Production Deployment

### TEE Worker Deployment (Phala Cloud)

**Prerequisites**:
- Phala Cloud account
- Docker Hub account
- Deployed contracts (outlayer.testnet + register.outlayer.testnet)
- Init account with NEAR tokens (pays gas for worker registration)

**Step 1: Build and push Docker image**

```bash
# Build worker-compiler image (includes Rust toolchain)
./scripts/build_and_push_phala.sh <dockerhub-username> latest worker-compiler

# Image will be: <dockerhub-username>/near-outlayer-worker-compiler:latest
```

**Step 2: Configure environment**

```bash
cd docker
cp .env.testnet-worker-phala.example .env.testnet-worker-phala

# Edit .env.testnet-worker-phala:
# - DOCKER_IMAGE_WORKER=<dockerhub-username>/near-outlayer-worker-compiler:latest
# - API_BASE_URL=https://coordinator.outlayer.testnet
# - API_AUTH_TOKEN=<your-worker-token>
# - NEAR_RPC_URL=https://rpc.testnet.near.org
# - OFFCHAINVM_CONTRACT_ID=outlayer.testnet
# - REGISTER_CONTRACT_ID=register.outlayer.testnet
# - USE_TEE_REGISTRATION=true
# - TEE_MODE=tdx
# - INIT_ACCOUNT_ID=<init-account>.testnet
# - INIT_ACCOUNT_PRIVATE_KEY=ed25519:...
# - COMPILATION_MODE=native
# - COMPILATION_ENABLED=true
# - EXECUTION_ENABLED=true
```

**Step 3: Deploy to Phala Cloud**

```bash
phala deploy \
  --name outlayer-testnet-worker \
  --compose docker-compose.worker-compiler.phala.yml \
  --env-file .env.testnet-worker-phala \
  --vcpu 2 --memory 4G --disk-size 10G \
  --kms-id phala-prod10
```

**Step 4: Monitor deployment**

```bash
# Check status
phala cvms status outlayer-testnet-worker

# View logs
phala cvms logs outlayer-testnet-worker --follow

# Check registration on contract
near view register.outlayer.testnet get_worker '{"account_id":"<worker-account>.testnet"}'
```

### Coordinator Deployment

```bash
# Use your preferred hosting (AWS, DigitalOcean, etc.)
# Requirements:
# - PostgreSQL 14+
# - Redis 7+
# - 2+ CPU cores
# - 4GB+ RAM
# - Ports: 8080 (API), 8081 (Keystore - internal only)

# Setup PostgreSQL and Redis
docker-compose -f docker/docker-compose.yml up -d postgres redis

# Run migrations
cd coordinator
sqlx migrate run

# Start coordinator
cargo run --release
```

### Security Considerations

**TEE Workers**:
- ✅ Keys generated inside TEE (never exposed)
- ✅ Intel TDX attestation proves code integrity
- ✅ Environment isolation prevents secret leakage
- ✅ Build.rs blocked to prevent code execution attacks
- ✅ Resource limits prevent DoS attacks

**Keystore**:
- ⚠️ Must be isolated (not exposed to public internet)
- ✅ Access control validation before decryption
- ✅ TEE attestation verification
- ✅ Only coordinator can access keystore

**Coordinator**:
- ✅ Bearer token authentication for workers
- ✅ Rate limiting via Redis
- ✅ SQL injection protection (sqlx with prepared statements)
- ✅ WASM cache with LRU eviction

## License

MIT
