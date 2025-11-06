# NEAR OutLayer (OffchainVM)

**OutLayer execution for on-chain contracts**

Verifiable off-chain computation platform for NEAR smart contracts using yield/resume mechanism.

## Project Structure

```
near-outlayer/
├── contract/                    # NEAR smart contract (offchainvm.near)
├── coordinator/                 # Coordinator API server (Rust + Axum)
├── worker/                     # Worker nodes (Rust + Tokio)
├── wasi-examples/              # WASI example projects (random-ark, ai-ark, etc.)
├── outlayer-verification-suite/ # Property-based testing (512+ adversarial cases)
├── tests/verification-tests/    # Integration tests - 82/82 passing
├── clients/ts/outlayer-client/  # TypeScript SDK with idempotency support
├── research/                   # Experimental features (nearcore conformance)
├── scripts/                    # Deployment & verification scripts
├── docker/                     # Docker configurations
└── docs/                       # Documentation
```

### Production Features vs Research

**Production-Ready Core Features**:
- ✅ Deterministic WASM execution (100× replay verified, fuel = 27,111)
- ✅ NEP-297 event compliance
- ✅ Overflow/underflow protection (checked arithmetic)
- ✅ Path traversal prevention (GitHub URL canonicalization)
- ✅ WASM I/O correctness (stdout capture)
- ✅ Idempotency middleware
- ✅ Property-based testing (512+ adversarial cases)
- ✅ 82/82 integration tests passing
- ✅ Machine verification (12/12 checks)

**Research & Experimental Features**: Located in `research/` directory
- 🔬 Nearcore conformance oracle (primitives bindings, fee parity)
- 🔬 Hardware TEE attestation (Intel SGX, AMD SEV)
- 🔬 Borsh ABI prototypes
- 🔬 Differential fuzzing against nearcore runtime

See [docs/DoD-Verification-Tests.md](docs/DoD-Verification-Tests.md) for detailed acceptance criteria and [research/README.md](research/README.md) for research scope.

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
near contract deploy offchainvm.testnet use-file res/local/offchainvm_contract.wasm with-init-call new json-args '{"owner_id":"offchainvm.testnet","operator_id":"worker.offchainvm.testnet"}' prepaid-gas '100.0 Tgas' attached-deposit '0 NEAR' network-config testnet sign-with-keychain send
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

- **[QUICK_START.md](QUICK_START.md)** - Fast getting started guide
- **[SETUP.md](SETUP.md)** - Detailed setup instructions
- **[CLAUDE.md](CLAUDE.md)** - AI assistant development guide
- **[docs/](docs/)** - Complete documentation library
  - [Project Vision](docs/PROJECT_VISION.md) - Strategic positioning and vision
  - [Architecture](docs/architecture/) - Deep technical documentation
  - [Phases](docs/phases/) - Completed phase reports
  - [Guides](docs/guides/) - Operational guides (auth, deployment)
  - [Proposals](docs/proposals/) - Strategic proposals and planning

See **[docs/README.md](docs/README.md)** for complete documentation index.

## Architecture

### Core Components

1. **Smart Contract** (`offchainvm.near`)
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
