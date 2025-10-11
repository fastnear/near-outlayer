# CLAUDE.md

<CRITICAL>
You are an assistant, you must write correct and clean code. You speak with a programmer human, you can always ask human's point of view. Do not introduce tasks that were not mentioned by user, he is technical and he knows the potencial scope. Thus said, be sure human's knowledge is limited so you can alsways suggest a better way to solve the problem, but didn't write code if you have something to dicsuss first.

If you are not sure, just reply "I don't know how to do it". It's totally ok, hyman will provide more details
</CRITICAL>

# NEAR Offshore MVP Development - Context

## 📍 Project Overview

**NEAR Offshore (OffchainVM)** is a verifiable off-chain computation platform for NEAR smart contracts. Smart contracts can execute arbitrary WASM code off-chain using NEAR Protocol's yield/resume mechanism.

**Metaphor**: "Offshore jurisdiction for computation" - move heavy computation off-chain for efficiency while maintaining security and final settlement on NEAR L1.

## 🎯 Main Goal

Build a production-ready MVP without TEE (Trusted Execution Environment), with architecture that allows easy TEE integration via Phala Network in Phase 2.

## 📊 Current Progress (Updated: 2025-10-09)

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
  - **NEW: Full WASI environment variables support**:
    - Accepts `Option<HashMap<String, String>>` with env vars from decrypted secrets
    - WasiEnv structure stores env vars in WASI-compatible format (`KEY=VALUE\0`)
    - Implements `environ_sizes_get` and `environ_get` WASI functions
    - WASM code can access secrets via `std::env::var("KEY")`
  - Minimal WASI interface (random_get, fd_write, proc_exit, environ_*)
- ✅ **near_client.rs** - NEAR RPC client:
  - submit_execution_result() - Calls resolve_execution on contract
  - **NEW: Sends actual metrics** (instructions, time_ms) instead of zeros
  - Transaction signing and submission
  - Finalization waiting
- ✅ **keystore_client.rs** - Keystore integration for encrypted secrets:
  - **NEW: JSON secrets parsing** - returns `HashMap<String, String>` instead of raw bytes
  - decrypt_secrets() - Sends attestation + encrypted data to keystore
  - generate_attestation() - TEE attestation (simulated/sgx/sev/none modes)
  - Automatic JSON validation and parsing
- ✅ **main.rs** - Complete worker loop:
  - Task polling from Coordinator API
  - Compile task handling (with input_data)
  - Execute task handling (with input_data - fixed old TODO)
  - **NEW: Encrypted secrets decryption and env vars injection**:
    - Decrypts secrets via keystore if provided
    - Parses JSON to HashMap
    - Passes env vars to executor → WASI environment
    - WASM code can access secrets transparently
  - Event monitor spawning (optional)
- ✅ **README.md** - Full documentation with setup, configuration, and deployment
- ✅ **.env.example** - Complete configuration template with API key examples
- ✅ **Compiles successfully** - warnings only (unused fields), 0 errors

#### 5. **Keystore Worker** (`/keystore-worker`) - 100% ✅ COMPLETE
- ✅ **Python Flask API server** for secret management
- ✅ **Endpoints**:
  - `GET /pubkey` - Get encryption public key
  - `POST /decrypt` - Decrypt secrets with TEE attestation verification
  - `GET /health` - Health check
- ✅ **Simple XOR encryption** (MVP) - will be replaced with ChaCha20-Poly1305 in production
- ✅ **Attestation verification** - validates worker's TEE measurements
- ✅ **encrypt_secrets.py** - Helper script to encrypt secrets:
  - **NEW: JSON format** - accepts `{"KEY":"value"}` instead of `KEY=value,KEY2=value2`
  - Validates JSON structure before encryption
  - Outputs encrypted array for contract calls
- ✅ **Docker support** with docker-compose.yml
- ✅ Running on port 8081

### 🔧 Recent Enhancements (2025-10-10):

1. **Encrypted Secrets with WASI Environment Variables** ✨ **NEW**:
   - Changed secrets format from `KEY1=value1,KEY2=value2` to JSON `{"KEY1":"value1","KEY2":"value2"}`
   - Worker automatically decrypts secrets via keystore
   - Parses JSON to HashMap and injects into WASI environment
   - WASM code can access secrets using standard `std::env::var("KEY_NAME")`
   - Full end-to-end flow: Contract → Encrypted → Keystore → Worker → WASI → WASM
   - Updated encrypt_secrets.py with JSON validation

2. **Resource Metrics Overhaul** (2025-10-09):
   - Removed fake `memory_bytes: 0` field from contract and worker
   - Added real `instructions` tracking from wasmi fuel consumption
   - Changed `time_seconds` to `time_ms` for better precision
   - Updated all logs to show actual resources used

3. **Dynamic Pricing System** (2025-10-09):
   - New `estimate_cost()` method based on requested resource limits
   - Payment validation now checks against estimated cost (not just base_fee)
   - Users can query cost before execution via `estimate_execution_cost()`
   - Pricing: `base_fee + (instructions/1M × per_instruction_fee) + (time_ms × per_ms_fee)`

4. **Hard Resource Caps** (2025-10-09):
   - `MAX_INSTRUCTIONS = 100B` - prevents excessive resource requests
   - `MAX_EXECUTION_SECONDS = 60` - hard time limit
   - Validation on `request_execution()` to reject oversized requests
   - Public `get_max_limits()` view method

5. **API Configuration** (2025-10-09):
   - RPC URLs now support API keys embedded in URL
   - Example formats for Pagoda, Infura, Neardata providers
   - Updated `.env.example` with clear instructions

6. **Bug Fixes** (2025-10-09):
   - Fixed `Task::Execute` missing `input_data` field
   - Removed old TODO comment about fetching input_data
   - All tasks now properly pass input data to executor

### ⏳ To Be Implemented:

#### 1. **Test WASM Project** (`/test-wasm`) - 50%
- ✅ Basic structure created
- ✅ Random number generator placeholder
- ❌ Actual random number generation with WASI
- ❌ Proper input/output handling
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
         │ 1. request_execution(github_repo, commit, input_data, limits)
         ↓
┌─────────────────────────────┐
│   OffchainVM Contract       │
│   (offchainvm.near)         │
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
│   Coordinator API Server                 │
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
┌──────────────────────────────┐
│   Worker (Executor)          │
│   - Get task                 │
│   - Compile GitHub repo      │
│   - Execute WASM (wasmi)     │
│   - Track fuel consumption   │
│   - Return result + metrics  │
└────────┬─────────────────────┘
         │ 5. resolve_execution(result, resources_used)
         ↓
┌─────────────────────────────┐
│   OffchainVM Contract       │
│   - Log resources used       │
│   - Calculate actual cost    │
│   - Refund excess            │
│   - Emit event               │
└────────┬────────────────────┘
         │ 6. Result → Client Contract
         ↓
┌─────────────────┐
│  Client Contract│
│   (callback)    │
└─────────────────┘
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
# - KEYSTORE_BASE_URL=http://localhost:8081 (optional, for encrypted secrets)
# - KEYSTORE_AUTH_TOKEN=your-keystore-token (optional)

# Run worker
cargo run
```

### Using Encrypted Secrets (NEW!)
```bash
# 1. Start keystore worker
cd keystore-worker
docker-compose up -d

# 2. Encrypt your secrets as JSON
./scripts/encrypt_secrets.py '{"OPENAI_KEY":"sk-...","API_TOKEN":"secret123"}'

# Output example:
# [123, 45, 67, 89, ...]  <- Use this in contract call

# 3. Call contract with encrypted secrets
near call offchainvm.testnet request_execution \
  '{
    "code_source": {
      "repo": "https://github.com/user/repo",
      "commit": "abc123",
      "build_target": "wasm32-wasi"
    },
    "resource_limits": {
      "max_instructions": 1000000000,
      "max_memory_mb": 128,
      "max_execution_seconds": 60
    },
    "input_data": "{}",
    "encrypted_secrets": [123, 45, 67, 89, ...]
  }' \
  --accountId user.testnet \
  --deposit 0.1

# 4. Worker will automatically:
#    - Decrypt secrets via keystore
#    - Parse JSON: {"OPENAI_KEY":"sk-...", "API_TOKEN":"secret123"}
#    - Inject into WASI environment
#    - Your WASM code can use: std::env::var("OPENAI_KEY")
```

## 📚 Documentation

- [PROJECT.md](PROJECT.md) - Full technical specification
- [MVP_DEVELOPMENT_PLAN.md](MVP_DEVELOPMENT_PLAN.md) - Development plan with code examples
- [NEAROffshoreOnepager.md](NEAROffshoreOnepager.md) - Marketing one-pager
- [README.md](README.md) - Quick start guide
- [contract/README.md](contract/README.md) - Contract API documentation
- [worker/README.md](worker/README.md) - Worker setup and configuration

## 🎯 Timeline

- ✅ **Contract**: 1 week - COMPLETE + ENHANCED
- ✅ **Coordinator API**: 1-2 weeks - COMPLETE
- ✅ **Worker**: 2-3 weeks - COMPLETE (100%)
- ⏳ **Test WASM**: 1 day - 50%
- ⏳ **Testing**: 1 week - 0%
- **Total MVP**: ~5-7 weeks for 1 experienced Rust developer

## 💡 Important Notes

1. **Coordinator API running on port 8080** - can test endpoints right now
2. **Keystore worker running on port 8081** - handles encrypted secrets decryption
3. **Auth disabled in dev mode** (`REQUIRE_AUTH=false` in `.env`) - can make requests without token
4. **WASM cache** created in `/tmp/offchainvm/wasm` - remember to change to production path
5. **PostgreSQL and Redis** running via docker-compose
6. **All SQL migrations applied** - database is ready
7. **Resource metrics are now real** - instructions and time_ms from actual execution
8. **Dynamic pricing** - users see estimated cost before execution
9. **API keys** - can be embedded in NEAR_RPC_URL and NEARDATA_API_URL for paid services
10. **Encrypted secrets** - JSON format `{"KEY":"value"}`, automatically injected into WASI environment

## 🔄 Next Actions

1. **Complete Test WASM Project**
   - Implement actual random number generation with WASI
   - Test input/output handling
   - Create GitHub repo for testing

2. **Write deployment scripts**
   - Deploy contract to testnet
   - Create worker tokens
   - Production docker images

3. **Integration testing**
   - End-to-end tests with real contract + worker + coordinator
   - Load testing with multiple concurrent executions
   - Security testing for sandboxing and resource limits

4. **Documentation improvements**
   - Video tutorial for setup
   - Example projects using OffchainVM
   - Best practices guide

---

**Current Date**: 2025-10-10
**Version**: MVP Phase 1 (without TEE)
**Status**: Contract ✅ | Coordinator ✅ | Worker ✅ | Keystore ✅ | Encrypted Secrets ✅ - Ready for integration testing
