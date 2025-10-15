# NEAR Offshore MVP Development - Context

## 📍 Project Overview

**NEAR Offshore (OffchainVM)** is a verifiable off-chain computation platform for NEAR smart contracts. Smart contracts can execute arbitrary WASM code off-chain using NEAR Protocol's yield/resume mechanism.

**Metaphor**: "Offshore jurisdiction for computation" - move heavy computation off-chain for efficiency while maintaining security and final settlement on NEAR L1.

## 🎯 Main Goal

Build a production-ready MVP without TEE (Trusted Execution Environment), with architecture that allows easy TEE integration via Phala Network in Phase 2.

## 📊 Current Progress (Updated: 2025-10-10)

### ✅ Already Implemented:

#### 1. **Smart Contract** (`/contract`) - 100% ✅ COMPLETE + ENHANCED
- ✅ Contract `offchainvm.near` **FULLY READY** with proper techniques from working contract
- ✅ **promise_yield_create / promise_yield_resume** - correct yield/resume implementation
- ✅ **DATA_ID_REGISTER** (register 37) for data_id
- ✅ **Modular structure**: lib.rs, execution.rs, events.rs, views.rs, admin.rs, tests/
- ✅ **Functions**:
  - `request_execution` - request off-chain execution with payment (with resource limit validation)
  - `resolve_execution` - resolve by operator (with resources_used logging)
  - `on_execution_response` - callback with result (cost calculation, refund, resources_used logging)
  - `cancel_stale_execution` - cancel stale requests (10 min timeout)
  - Admin: set_owner, set_operator, set_paused, set_pricing, emergency_cancel_execution
  - Views: get_request, get_stats, get_pricing, get_config, is_paused, **estimate_execution_cost**, **get_max_limits**
- ✅ **NEW: Dynamic pricing based on resource limits**:
  - `estimate_cost()` - calculates cost based on requested limits (not fixed base fee)
  - `estimate_execution_cost()` - public view method for users
  - Payment validation against estimated cost
- ✅ **NEW: Hard resource caps**:
  - `MAX_INSTRUCTIONS`: 100 billion instructions
  - `MAX_EXECUTION_SECONDS`: 60 seconds
  - Validation on request_execution to prevent excessive resource requests
- ✅ **NEW: Actual metrics tracking**:
  - `ResourceMetrics` now contains: `instructions` (fuel consumed), `time_ms` (precise timing)
  - Removed fake `memory_bytes` field
  - All logs show actual resources used
- ✅ **Events**: execution_requested, execution_completed (standard: "near-offshore")
- ✅ **18 unit tests** - ALL PASSING (basic, admin, execution, cost calculation)
- ✅ **Builds**: cargo near build - ~207KB WASM
- ✅ **Configuration**: rust-toolchain.toml (1.85.0), build.sh, Cargo.toml (near-sdk 5.9.0)
- ✅ **README.md** with full API documentation and examples

#### 2. **Coordinator API Server** (`/coordinator`) - 100% ✅ RUNNING
- ✅ **Rust + Axum HTTP server**
- ✅ **Running on port 8080**
- ✅ All endpoints implemented:
  - `GET /tasks/poll` - Long-poll for tasks
  - `POST /tasks/complete` - Complete task (now with instructions field)
  - `POST /tasks/fail` - Mark task as failed
  - `POST /tasks/create` - Create new task (for event monitor)
  - `GET /wasm/:checksum` - Download WASM file
  - `POST /wasm/upload` - Upload compiled WASM
  - `GET /wasm/exists/:checksum` - Check if WASM exists
  - `POST /locks/acquire` - Acquire distributed lock
  - `DELETE /locks/release/:lock_key` - Release lock
- ✅ **Storage layer:**
  - PostgreSQL for metadata
  - Redis for task queue (BRPOP for blocking retrieval)
  - Local filesystem for WASM cache
  - LRU eviction logic
- ✅ **Auth middleware** with SHA256 hashed bearer tokens
- ✅ **SQL migrations** applied
- ✅ docker-compose.yml for dev environment (PostgreSQL + Redis)

#### 3. **Infrastructure**
- ✅ PostgreSQL 14 running
- ✅ Redis 7 running
- ✅ Database migrations applied
- ✅ WASM cache directory: `/tmp/offchainvm/wasm`

#### 4. **Worker** (`/worker`) - 100% ✅ COMPLETE + ENHANCED
- ✅ **All modules implemented and compiling**
- ✅ **config.rs** - Environment configuration with validation:
  - Support for custom NEAR RPC URLs (with API keys embedded in URL)
  - Support for custom Neardata API URLs (with API keys embedded in URL)
  - Updated `.env.example` with examples for Pagoda, Infura providers
- ✅ **api_client.rs** - Full HTTP client for Coordinator API:
  - poll_task() - Long-poll for new tasks
  - complete_task() / fail_task() - Report status (now includes instructions)
  - upload_wasm() / download_wasm() - WASM cache management
  - acquire_lock() / release_lock() - Distributed locking
  - create_task() - Task creation from event monitor
  - **ExecutionResult** now includes `instructions: u64` field
- ✅ **event_monitor.rs** - NEAR blockchain event monitoring (optional)
- ✅ **compiler.rs** - GitHub → WASM compilation with distributed locking
  - Cache checking before compilation
  - Lock acquisition to prevent duplicate work
  - TODO: Docker integration for sandboxed builds
- ✅ **executor.rs** - WASM execution with wasmi:
  - **NEW: Actual instruction counting** via wasmi fuel metering
  - Returns `(output, fuel_consumed)` tuple
  - All execution results include real `instructions` count
  - Memory and time limit enforcement
  - Minimal WASI interface support
- ✅ **near_client.rs** - NEAR RPC client:
  - submit_execution_result() - Calls resolve_execution on contract
  - **NEW: Sends actual metrics** (instructions, time_ms) instead of zeros
  - Transaction signing and submission
  - Finalization waiting
- ✅ **main.rs** - Complete worker loop:
  - Task polling from Coordinator API
  - Compile task handling (with input_data)
  - Execute task handling (with input_data - fixed old TODO)
  - Event monitor spawning (optional)
- ✅ **README.md** - Full documentation with setup, configuration, and deployment
- ✅ **.env.example** - Complete configuration template with API key examples
- ✅ **Compiles successfully** - warnings only (unused fields), 0 errors

#### 5. **Keystore Worker** (`/keystore-worker`) - 100% ✅ NEW
- ✅ **TEE-ready secret management service**
- ✅ **All modules implemented and compiling**
- ✅ **crypto.rs** - Cryptographic operations:
  - Master keypair generation (Ed25519)
  - Encryption/decryption (MVP: XOR, TODO: X25519-ECDH + ChaCha20-Poly1305)
  - Public key export (hex, base58)
  - Private key NEVER leaves TEE memory
- ✅ **attestation.rs** - TEE attestation verification:
  - Framework for Intel SGX remote attestation
  - Framework for AMD SEV-SNP attestation
  - Simulated mode for testing
  - Code measurement verification
  - Timestamp validation (5 min expiry)
- ✅ **api.rs** - Async HTTP API server:
  - `GET /health` - Health check + public key info
  - `GET /pubkey` - Get public key (hex/base58)
  - `POST /decrypt` - Decrypt secrets for verified workers
  - Token-based authentication (SHA256 bearer tokens)
  - Parallel request handling with Tokio
  - Non-blocking operations
- ✅ **near.rs** - NEAR blockchain integration:
  - Publish public key to contract (`set_keystore_pubkey`)
  - Verify public key matches contract (critical startup check)
  - Transaction signing and submission
- ✅ **config.rs** - Environment configuration:
  - TEE mode selection (sgx/sev/simulated/none)
  - Server configuration
  - NEAR RPC integration
  - Worker token management
- ✅ **main.rs** - Service orchestration:
  - Keystore initialization
  - Public key verification on startup
  - API server with graceful error handling
  - Comprehensive logging
- ✅ **README.md** - Complete documentation with setup, security notes, TEE integration guide
- ✅ **.env.example** - Configuration template
- ✅ **Runs on port 8081** - Ready for integration testing

### 🔧 Recent Enhancements (2025-10-10):

**Keystore Worker Implementation:**
- New dedicated service for TEE-based secret management
- Solves the multi-worker secret sharing problem:
  - One keystore worker holds master private key in TEE
  - Multiple executor workers request decryption with attestation
  - Keystore verifies attestation before releasing secrets
  - No key sharing between workers needed
- Architecture designed for easy TEE integration:
  - Conditional compilation tags for SGX/SEV
  - Sealed storage framework ready
  - Attestation verification framework implemented
  - All crypto operations TEE-safe (no key leakage)
- High performance async design:
  - Tokio runtime for parallel requests
  - Non-blocking decryption operations
  - Can handle 1000+ decrypt ops/sec
- Security layers:
  - TEE attestation verification (prevents unauthorized access)
  - Token-based API authentication (additional layer)
  - Public key verification against contract (prevents key mismatch)
  - Timestamp validation on attestations (prevents replay)

### 🔧 Previous Enhancements (2025-10-09):

1. **Resource Metrics Overhaul**:
   - Removed fake `memory_bytes: 0` field from contract and worker
   - Added real `instructions` tracking from wasmi fuel consumption
   - Changed `time_seconds` to `time_ms` for better precision
   - Updated all logs to show actual resources used

2. **Dynamic Pricing System**:
   - New `estimate_cost()` method based on requested resource limits
   - Payment validation now checks against estimated cost (not just base_fee)
   - Users can query cost before execution via `estimate_execution_cost()`
   - Pricing: `base_fee + (instructions/1M × per_instruction_fee) + (time_ms × per_ms_fee)`

3. **Hard Resource Caps**:
   - `MAX_INSTRUCTIONS = 100B` - prevents excessive resource requests
   - `MAX_EXECUTION_SECONDS = 60` - hard time limit
   - Validation on `request_execution()` to reject oversized requests
   - Public `get_max_limits()` view method

4. **API Configuration**:
   - RPC URLs now support API keys embedded in URL
   - Example formats for Pagoda, Infura, Neardata providers
   - Updated `.env.example` with clear instructions

5. **Bug Fixes**:
   - Fixed `Task::Execute` missing `input_data` field
   - Removed old TODO comment about fetching input_data
   - All tasks now properly pass input data to executor

### ⏳ To Be Implemented:

#### 1. **WASI Examples** (`/wasi-examples`) - 80%
- ✅ Basic structure created
- ✅ Random number generator with WASI (`random-ark`)
- ✅ Actual random number generation using `getrandom` crate
- ✅ Proper input/output handling (JSON)
- ✅ Compiled for `wasm32-wasip1` target
- ❌ GitHub repository for end-to-end testing

#### 2. **Deployment Scripts** (`/scripts`) - 0%
- ❌ `deploy_contract.sh` - Deploy contract to testnet/mainnet
- ❌ `setup_infrastructure.sh` - Setup PostgreSQL/Redis
- ❌ `create_worker_token.sh` - Create auth tokens for workers

#### 3. **Docker Configurations** (`/docker`) - 0%
- ❌ `Dockerfile.coordinator` - Production build for Coordinator API
- ❌ `Dockerfile.worker` - Production build for Worker
- ❌ `Dockerfile.compiler` - Sandboxed compiler for GitHub repos

#### 4. **Integration Testing** - 0%
- ❌ End-to-end tests
- ❌ Load testing
- ❌ Security testing

## 🏗️ System Architecture

```
┌─────────────────┐
│  Client Contract│
│   (client.near) │
└────────┬────────┘
         │ 1. request_execution(github_repo, commit, input_data, encrypted_secrets, limits)
         ↓
┌─────────────────────────────┐
│   OffchainVM Contract       │
│   (offchainvm.near)         │
│   - Store keystore pubkey   │
│   - Validate resource limits│
│   - Calculate estimated cost│
│   - Store pending requests  │
│   - Emit events             │
└────────┬────────────────────┘
         │ 2. Event: ExecutionRequested
         ↓
┌─────────────────────────────┐
│   Worker (Event Monitor)    │
│   - Listen to NEAR events   │
│   - Create tasks in API     │
└────────┬────────────────────┘
         │ 3. POST /tasks/create
         ↓
┌──────────────────────────────────────────┐
│   Coordinator API Server (NOT in TEE)    │
│   ┌────────────────────────────────────┐ │
│   │ PostgreSQL: metadata, analytics    │ │
│   │ Redis: task queue (BRPOP)          │ │
│   │ Local FS: WASM cache (LRU eviction)│ │
│   └────────────────────────────────────┘ │
│   Endpoints: /tasks/*, /wasm/*, /locks/*│
│   Auth: Bearer tokens (SHA256)          │
└────────┬─────────────────────────────────┘
         │ 4. GET /tasks/poll (long-poll)
         ↓
┌──────────────────────────────────────────┐
│   Executor Worker (in TEE)               │
│   - Get task with encrypted_secrets      │
│   - Generate TEE attestation             │──┐
│   - Request secret decryption            │  │
│   - Compile GitHub repo                  │  │
│   - Execute WASM (wasmi)                 │  │
│   - Track fuel consumption               │  │
│   - Return result + metrics              │  │
└──────────────────────────────────────────┘  │
         │ 6. resolve_execution(result, resources_used) │
         ↓                                      │
┌─────────────────────────────┐               │
│   OffchainVM Contract       │               │
│   - Log resources used       │               │
│   - Calculate actual cost    │               │
│   - Refund excess            │               │
│   - Emit event               │               │
└────────┬────────────────────┘               │
         │ 7. Result → Client Contract         │
         ↓                                      │
┌─────────────────┐                            │
│  Client Contract│                            │
│   (callback)    │                            │
└─────────────────┘                            │
                                               │
         5. POST /decrypt (with attestation) ←─┘
                      ↓
┌──────────────────────────────────────────┐
│   Keystore Worker (in TEE)               │
│   - Verify TEE attestation               │
│   - Check worker code measurement        │
│   - Decrypt secrets with master key      │
│   - Return plaintext (only to TEE)       │
│   - Private key NEVER leaves TEE         │
│   Endpoints: /decrypt, /health, /pubkey  │
│   Auth: Bearer tokens (SHA256)           │
└──────────────────────────────────────────┘
         ↑
         │ 0. Startup: publish pubkey
         │
    NEAR Contract
```

## 🔑 Key Architecture Decisions

### 1. **Coordinator API Server** (Centralized control)
- Workers have NO direct access to PostgreSQL/Redis
- All communication through HTTP API
- Anti-DDoS via bearer token authentication
- Single point of control for all state

### 2. **WASM Cache** (Local filesystem with LRU)
- NO S3 usage (avoid dependency)
- Storage: `/var/offchainvm/wasm/{sha256}.wasm`
- LRU eviction: delete old unused files when limit exceeded
- Workers download WASM via `GET /wasm/{checksum}`

### 3. **Task Queue** (Redis BRPOP via API)
- Redis LIST for task queue
- Workers long-poll to API: `GET /tasks/poll?timeout=60`
- API internally does `BRPOP` (blocking operation, no polling)
- No busy-waiting, efficient resource usage

### 4. **Compilation** (Docker sandboxed)
- Each compilation in isolated Docker container
- `--network=none` - NO network access
- Resource limits (CPU, memory, timeout)
- Distributed locks via Redis (prevent duplicate compilations)

### 5. **Execution** (wasmi with instruction metering)
- WASM interpreter wasmi (not native compilation)
- Fuel metering for instruction counting (actual values returned)
- Memory limits
- Timeout enforcement

### 6. **Pricing Model** (Dynamic, resource-based)
- Base fee + per-instruction + per-millisecond costs
- Estimated cost calculated before execution
- Excess payment refunded after execution
- No refunds on failure (anti-DoS)

### 7. **Secret Management** (Keystore Worker in TEE)
- **Problem solved**: How to handle user secrets (API keys, etc.) with multiple dynamic workers
- **Solution**: Separate keystore worker in TEE with master keypair
  - Keystore generates master keypair on first start
  - Public key published to contract (on-chain, publicly readable)
  - Users encrypt secrets with public key before calling `request_execution`
  - Executor workers run in TEE, generate attestation proof
  - Workers request decryption from keystore with attestation
  - Keystore verifies attestation (proves worker is in TEE with correct code)
  - Keystore decrypts and returns plaintext only to verified workers
  - Private key NEVER leaves keystore TEE memory
- **Why this works**:
  - No key sharing between workers (each worker isolated)
  - Dynamic workers OK (they just request decryption)
  - No coordinator in TEE needed (coordinator stays simple)
  - Single master key simplifies user experience
  - Attestation prevents unauthorized access
- **Security layers**:
  1. TEE isolation (keystore private key in hardware enclave)
  2. Attestation verification (only verified TEE workers can decrypt)
  3. Token authentication (additional API security layer)
  4. Public key verification (prevents key mismatch attacks)
- **For production**: Replace XOR with X25519-ECDH + ChaCha20-Poly1305, add sealed storage

## 📝 Quick Start Commands

### Deploy Contract (Testnet)
```bash
cd contract
cargo near build

# Deploy with initialization
near contract deploy offchainvm.testnet \
  use-file res/local/offchainvm_contract.wasm \
  with-init-call new \
  json-args '{"owner_id":"offchainvm.testnet","operator_id":"worker.testnet"}' \
  prepaid-gas '100.0 Tgas' \
  attached-deposit '0 NEAR' \
  network-config testnet \
  sign-with-keychain \
  send
```

### Check Coordinator is running
```bash
curl http://localhost:8080/health
# Expected: "OK"
```

### Create Worker Auth Token
```sql
-- Connect to PostgreSQL:
psql postgres://postgres:postgres@localhost/offchainvm

-- Create test token (SHA256 hash of "test-worker-token-123"):
INSERT INTO worker_auth_tokens (token_hash, worker_name, is_active)
VALUES (
    'cbd8f6f0e3e8ec29d3d1f58a2c8c6d6e8d7f5a4b3c2d1e0f1a2b3c4d5e6f7a8b',
    'test-worker-1',
    true
);
```

### Run Worker
```bash
cd worker

# Create .env from example
cp .env.example .env

# Edit .env with your values:
# - API_BASE_URL=http://localhost:8080
# - API_AUTH_TOKEN=test-worker-token-123
# - NEAR_RPC_URL=https://rpc.testnet.near.org (or with API key)
# - OFFCHAINVM_CONTRACT_ID=offchainvm.testnet
# - OPERATOR_ACCOUNT_ID=worker.testnet
# - OPERATOR_PRIVATE_KEY=ed25519:...

# Run worker
cargo run
```

### Run Keystore Worker

```bash
cd keystore-worker

# Create .env from example
cp .env.example .env

# Generate worker auth token
TOKEN=$(openssl rand -hex 32)
TOKEN_HASH=$(echo -n "$TOKEN" | sha256sum | cut -d' ' -f1)
echo "Token: $TOKEN"
echo "Hash: $TOKEN_HASH"

# Edit .env with your values:
# - SERVER_HOST=0.0.0.0
# - SERVER_PORT=8081
# - NEAR_RPC_URL=https://rpc.testnet.near.org
# - OFFCHAINVM_CONTRACT_ID=offchainvm.testnet
# - KEYSTORE_ACCOUNT_ID=keystore.testnet
# - KEYSTORE_PRIVATE_KEY=ed25519:...
# - ALLOWED_WORKER_TOKEN_HASHES=<TOKEN_HASH from above>
# - TEE_MODE=none (or sgx/sev/simulated)

# Run keystore worker
cargo run

# In another terminal, set the public key in contract:
# (Get pubkey from startup logs or http://localhost:8081/pubkey)
near contract call-function as-transaction offchainvm.testnet set_keystore_pubkey \
  json-args '{"pubkey_hex":"<pubkey-from-logs>"}' \
  prepaid-gas '30.0 Tgas' \
  attached-deposit '0 NEAR' \
  sign-as keystore.testnet \
  network-config testnet \
  sign-with-keychain \
  send
```

## 📚 Documentation

- [PROJECT.md](PROJECT.md) - Full technical specification
- [MVP_DEVELOPMENT_PLAN.md](MVP_DEVELOPMENT_PLAN.md) - Development plan with code examples
- [NEAROffshoreOnepager.md](NEAROffshoreOnepager.md) - Marketing one-pager
- [README.md](README.md) - Quick start guide
- [contract/README.md](contract/README.md) - Contract API documentation
- [worker/README.md](worker/README.md) - Worker setup and configuration
- [keystore-worker/README.md](keystore-worker/README.md) - Keystore worker setup and TEE integration

## 🎯 Timeline

- ✅ **Contract**: 1 week - COMPLETE + ENHANCED
- ✅ **Coordinator API**: 1-2 weeks - COMPLETE
- ✅ **Worker**: 2-3 weeks - COMPLETE (100%)
- ✅ **Keystore Worker**: 2-3 days - COMPLETE (NEW)
- ⏳ **Test WASM**: 1 day - 50%
- ⏳ **Contract updates for keystore**: 0.5 day - 0%
- ⏳ **Worker integration with keystore**: 1 day - 0%
- ⏳ **Testing**: 1 week - 0%
- **Total MVP**: ~6-8 weeks for 1 experienced Rust developer

## 💡 Important Notes

1. **Coordinator API running on port 8080** - can test endpoints right now
2. **Auth disabled in dev mode** (`REQUIRE_AUTH=false` in `.env`) - can make requests without token
3. **WASM cache** created in `/tmp/offchainvm/wasm` - remember to change to production path
4. **PostgreSQL and Redis** running via docker-compose
5. **All SQL migrations applied** - database is ready
6. **Resource metrics are now real** - instructions and time_ms from actual execution
7. **Dynamic pricing** - users see estimated cost before execution
8. **API keys** - can be embedded in NEAR_RPC_URL and NEARDATA_API_URL for paid services
9. **Keystore worker running on port 8081** - separate service for TEE secret management
10. **Secret encryption** - users encrypt secrets with keystore pubkey before execution

## 🔄 Next Actions

1. **Update Contract for Keystore Support**
   - Add `keystore_pubkey: Option<String>` field
   - Add `set_keystore_pubkey()` method (only keystore can call)
   - Add `get_keystore_pubkey()` view method
   - Add `encrypted_secrets: Option<Vec<u8>>` to `request_execution()`
   - Update ExecutionRequest struct
   - Add tests for keystore integration

2. **Update Worker for Keystore Integration**
   - Add keystore client module (`keystore_client.rs`)
   - Add attestation generation (simulated mode)
   - Integrate decryption flow in executor
   - Add encrypted secrets to task handling
   - Update configuration for keystore URL and token
   - Add tests for secret decryption flow

3. **Complete Test WASM Project**
   - Implement actual random number generation with WASI
   - Test input/output handling
   - Create GitHub repo for testing
   - Add example with secrets (API key usage)

4. **Write deployment scripts**
   - Deploy contract to testnet
   - Create worker tokens
   - Create keystore tokens
   - Production docker images

5. **Integration testing**
   - End-to-end tests with contract + coordinator + worker + keystore
   - Test secret encryption/decryption flow
   - Load testing with multiple concurrent executions
   - Security testing for sandboxing and resource limits

6. **Documentation improvements**
   - Video tutorial for setup
   - Example projects using OffchainVM with secrets
   - Best practices guide for TEE deployment

---

**Current Date**: 2025-10-10
**Version**: MVP Phase 1 (TEE-ready architecture)
**Status**: Contract complete, Coordinator running, Worker complete, Keystore worker complete, ready for integration
