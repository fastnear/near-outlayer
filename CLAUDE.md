# CLAUDE.md

<CRITICAL>
You are an assistant, you must write correct and clean code. You speak with a programmer human, you can always ask human's point of view. Do not introduce tasks that were not mentioned by user, he is technical and he knows the potencial scope. Thus said, be sure human's knowledge is limited so you can alsways suggest a better way to solve the problem, but didn't write code if you have something to dicsuss first.

If you are not sure, just reply "I don't know how to do it". It's totally ok, hyman will provide more details

**IMPORTANT**: Human will manage coordinator restarts, docker-compose, and contract deployment himself. DO NOT try to restart coordinator, deploy contract, or manage docker containers. Just write code and let human handle the deployment.

**DO NOT CREATE SUMMARY FILES**: Human does NOT read "summary", "changelog", or "update" files. DO NOT create files like:
- DOCUMENTATION_UPDATE_*.md
- BUILD_COMMANDS_SUMMARY.md
- FRONTEND_ECIES_DONE.md
- *_SUMMARY.md
- CHANGES.md
These files waste time. Human only reads code and core documentation (README, TUTORIAL, etc).

**NEVER CREATE DOCUMENTATION ABOUT CHANGES**: When making fixes or changes to the codebase, NEVER create documentation files describing what was changed or fixed. Simply make the code changes directly without creating any explanatory documentation. Human doesn't want or need these documentation files.

**CRITICAL - WASI Development**: When human asks to write a new WASI container/example, you MUST:
1. FIRST read existing examples in `wasi-examples/` directory to understand the patterns
2. ALWAYS read and follow `wasi-examples/WASI_TUTORIAL.md` tutorial
3. Study how other examples are structured (Cargo.toml, build scripts, WASI imports)
4. DO NOT just write code from scratch - follow the established patterns
5. Copy the structure and conventions from existing working examples
6. Ask human which example to use as a template if multiple exist
This ensures consistency and reduces bugs by reusing proven patterns.

**CRITICAL - NEAR Contract Development**: When writing NEAR smart contracts:
1. ALWAYS use `cargo near build` - NEVER use raw `cargo build --target wasm32-unknown-unknown`
2. ALWAYS create `rust-toolchain.toml` with `channel = "1.85.0"`
3. ALWAYS add `schemars = "0.8"` to Cargo.toml dependencies
4. ALWAYS add `use schemars::JsonSchema;` to types.rs
5. ALWAYS add `JsonSchema` derive to ALL enums and structs used in public API
6. For AccountId fields in structs with JsonSchema, use `#[schemars(with = "String")]`
7. Update near-sdk to version 5.9.0 (not 5.5.0)
See `wasi-examples/WASI_TUTORIAL.md` section "WASI vs NEAR Smart Contracts" for details.

**CRITICAL - LATENCY**: NEVER introduce arbitrary delays (sleep, wait) without strong justification:
- Avoid `tokio::time::sleep()` for retry logic when there are better alternatives
- If you must add a delay, it should be the last resort
- Always consider: what will change during this delay? If nothing - don't delay
- Better alternatives: exponential backoff, lock TTL, event-driven waiting
- If you add latency, document WHY in comments and discuss with human first

**CRITICAL - NO MVP CODE**: This is a PRODUCTION project, NOT an MVP. NEVER:
- Write "MVP" or "for MVP" comments in code
- Leave TODO comments saying "fix for production"
- Implement simplified/temporary solutions marked as MVP
- Use placeholder encryption or security (like XOR "for MVP")
- Skip proper error handling "for MVP simplicity"
All code must be production-ready. If something requires proper implementation, implement it properly or discuss with human first.

**CRITICAL - NO LIMITED FUNCTIONALITY**: NEVER implement features with reduced functionality for one platform/target without explicit user approval. Examples:
- DO NOT say "P1 doesn't support X, so we skip it" without asking
- DO NOT implement feature for P2 but leave P1 without it
- DO NOT assume one target is "less important" or can have fewer features
- If technical limitations exist (e.g., P1 vs P2 differences), EXPLAIN the situation and ASK how to proceed
- Always aim for feature parity across all supported targets unless user explicitly says otherwise

**CRITICAL - NO STUB IMPLEMENTATIONS**: NEVER write stub/placeholder implementations that return empty data or log warnings like "not implemented". This is STRICTLY FORBIDDEN:
- DO NOT write functions that return `vec![]` with a "TODO" or "requires implementation" comment
- DO NOT write functions that just `log!("WARNING: X requires Y implementation")`
- DO NOT add public API methods that don't actually work
- If a feature requires different data structures - implement them properly or ASK human first
- If you're not sure how to implement something - ASK, don't stub
- Every public function MUST be fully functional when written
- Example of FORBIDDEN code:
  ```rust
  pub fn list_items(&self) -> Vec<Item> {
      // TODO: need to add index
      log!("WARNING: requires indexing implementation");
      vec![]  // <-- THIS IS FORBIDDEN
  }
  ```
- If the data structure doesn't support iteration - either change the structure or don't expose the method

**CRITICAL - ERROR PROPAGATION TO USERS**: When writing code that needs to inform users about errors:
- `tracing::debug!()` / `tracing::warn!()` / `tracing::error!()` go to SERVER LOGS ONLY - users never see them
- To propagate errors to users, use `anyhow::bail!("descriptive message")` or `return Err(...)`
- In keystore-worker: errors from `validate()` propagate via `?` to API handlers → `ApiError::InternalError(format!("...: {}", e))`
- In worker: errors propagate to coordinator → stored in job result → visible in dashboard
- Example - WRONG (user sees nothing):
  ```rust
  tracing::debug!("Feature X not supported");
  return Ok(false);  // User just sees "Access denied" with no explanation
  ```
- Example - CORRECT (user sees the reason):
  ```rust
  anyhow::bail!("Feature X is not supported. Please use Y instead.");
  // User sees: "Access validation failed: Feature X is not supported. Please use Y instead."
  ```
- Rule: If user needs to know WHY something failed to fix their configuration, use `Err()` not `Ok(false)` + log
</CRITICAL>

# NEAR OutLayer Production Development - Context

## 📍 Project Overview

**NEAR OutLayer (OffchainVM)** is a verifiable off-chain computation platform for NEAR smart contracts. Smart contracts can execute arbitrary WASM code off-chain using NEAR Protocol's yield/resume mechanism.

**Metaphor**: "OutLayer for computation" - move heavy computation off-chain for efficiency while maintaining security and final settlement on NEAR L1.

## 🎯 Current Phase

Production version with full TEE (Trusted Execution Environment) support via Phala Network / Intel TDX.

## 📊 Current Progress (Updated: 2025-10-22)

### ✅ Already Implemented:

#### 1. **Smart Contract** (`/contract`) - 100% ✅ COMPLETE + ENHANCED
- ✅ Contract `outlayer.near` **FULLY READY** with proper techniques from working contract
- ✅ **promise_yield_create / promise_yield_resume** - correct yield/resume implementation
- ✅ **DATA_ID_REGISTER** (register 37) for data_id
- ✅ **Modular structure**: lib.rs, execution.rs, events.rs, views.rs, admin.rs, tests/
- ✅ **Execution Functions**:
  - `request_execution` - request off-chain execution with payment (with resource limit validation)
  - `resolve_execution` - resolve by operator (with resources_used logging)
  - `on_execution_response` - callback with result (cost calculation, refund, resources_used logging)
  - `cancel_stale_execution` - cancel stale requests (10 min timeout)
- ✅ **Secrets Management Functions** ✨ **NEW (2025-10-22)**:
  - `store_secrets` - Store encrypted secrets with repo/branch/profile/access_condition
  - `delete_secrets` - Delete secrets and refund storage deposit
  - `get_secrets` - Retrieve encrypted secrets (access control validated by keystore)
  - `secrets_exist` - Check if secrets exist for given key
  - `list_user_secrets` - List all secrets owned by a user (with indexing)
  - **User index**: `LookupMap<AccountId, UnorderedSet<SecretKey>>` for efficient lookups
  - **Storage cost**: Proportional to data size + ~64 bytes for index entry
- ✅ **Admin Functions**: set_owner, set_operator, set_paused, set_pricing, emergency_cancel_execution
- ✅ **View Functions**: get_request, get_stats, get_pricing, get_config, is_paused, **estimate_execution_cost**, **get_max_limits**, **list_user_secrets**
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
- ✅ **Events**: execution_requested, execution_completed (standard: "near-outlayer")
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

#### 5. **Keystore Worker** (`/keystore-worker`) - 100% ✅ COMPLETE + ENHANCED
- ✅ **Rust + Axum API server** for secret management running on port 8081
- ✅ **Secrets Endpoints**:
  - `GET /pubkey?repo=X&owner=Y&branch=Z` - Get encryption public key for specific repo
  - `POST /decrypt` - Decrypt secrets with TEE attestation + access control validation ✨ **NEW: Reserved keywords validation**
  - `GET /health` - Health check
- ✅ **Access Control Validation**:
  - Validates access conditions before decrypting (AllowAll, Whitelist, AccountPattern, NEAR balance, FT balance, NFT ownership)
  - Makes RPC calls to NEAR for balance checks
  - Supports complex Logic conditions (AND/OR/NOT)
- ✅ **Reserved Keywords Protection** ✨ **NEW (2025-11-06)**:
  - Rejects secrets containing reserved NEAR environment variables (NEAR_SENDER_ID, NEAR_TRANSACTION_HASH, etc.)
  - Prevents accidental override of system variables
  - Returns clear error message to user
- ✅ **Simple XOR encryption** (MVP) - will be replaced with ChaCha20-Poly1305 in production
- ✅ **Attestation verification** - validates worker's TEE measurements
- ✅ **Helper Scripts**:
  - `encrypt_secrets.py` - CLI tool to encrypt secrets:
    - **JSON format** - accepts `{"KEY":"value"}`
    - ✨ **NEW: Validates reserved keywords** before encryption
    - Outputs encrypted array for contract calls
- ✅ **Docker support** with docker-compose.yml

#### 6. **Dashboard** (`/dashboard`) - 100% ✅ COMPLETE + REFACTORED
- ✅ **Next.js 15 + TypeScript** web application running on port 3000
- ✅ **NEAR Wallet Integration** via @near-wallet-selector
- ✅ **Pages**:
  - `/` - Home page with project overview
  - `/executions` - List execution requests and results
  - `/secrets` - **Secrets management** ✨ **FULLY REFACTORED (2025-10-22)**
  - `/stats` - Platform statistics
  - `/workers` - Worker monitoring
  - `/settings` - User settings and earnings
  - `/playground` - Test WASM execution
- ✅ **Secrets Page Architecture** ✨ **NEW (2025-10-22)**:
  - **Modular component structure** (6 files, ~480 lines):
    - `page.tsx` (168 lines) - Main page with state management
    - `types.ts` - TypeScript type definitions
    - `utils.ts` - Helper functions with proper type guards
    - `AccessConditionBuilder.tsx` - Form for access conditions
    - `SecretCard.tsx` - Individual secret display card
    - `SecretsList.tsx` - List container with loading/empty states
    - `SecretsForm.tsx` - Create/edit form with client-side encryption
  - **Features**:
    - View all user's secrets via `list_user_secrets` contract method
    - Create new secrets with repo/branch/profile/access control
    - Edit existing secrets (loads data into form)
    - Delete secrets with confirmation + storage refund
    - Client-side encryption using **coordinator proxy** (`/secrets/pubkey` endpoint)
    - Access condition builder with all types (AllowAll, Whitelist, NEAR balance, FT, NFT, Logic)
    - Real-time secrets list refresh after operations
  - **Security**: Dashboard never directly accesses keystore (port 8081) - all requests go through coordinator proxy
    - Responsive design with Tailwind CSS
- ✅ **Build Status**: TypeScript compilation successful, no errors
- ✅ **Documentation**: Full refactoring summary in `REFACTORING_SUMMARY.md`

### 🔧 Recent Enhancements (2025-10-22):

1. **Branch Resolution via Coordinator API with Redis Caching** ✨ **NEW (2025-10-22)**:
   - **Architecture**: Coordinator handles GitHub API + Redis caching, workers call coordinator
   - **Coordinator** (`coordinator/src/handlers/github.rs`):
     - Endpoint: `GET /github/resolve-branch?repo=...&commit=...` (public, no auth)
     - Detects if commit is SHA (40/7-8 hex chars) or branch name
     - **For SHA commits**: Queries GitHub API `/commits/{sha}/branches-where-head`
     - **For branch names**: Returns as-is without API call (fast path)
     - **Caching**: All results cached in Redis for 7 days
   - **Worker** (`worker/src/api_client.rs`):
     - New method: `api_client.resolve_branch(repo, commit)`
     - Calls coordinator before secrets decryption in Compile/Execute tasks
     - Fallback to `branch=None` if coordinator API fails
   - **Benefits**: Centralized rate limit management, Redis caching, enables per-branch secrets
   - **No API key required** for public repositories

2. **Repo-Based Secrets Management** ✨ **NEW**:
   - Contract: `store_secrets`, `delete_secrets`, `get_secrets`, `secrets_exist`, `list_user_secrets`
   - User index: `LookupMap<AccountId, UnorderedSet<SecretKey>>` for O(1) user lookups
   - Storage cost: Base data + ~64 bytes for index entry, refunded on delete
   - Access control: AllowAll, Whitelist, AccountPattern, NEAR/FT/NFT balance checks, Logic (AND/OR/NOT)
   - Keystore integration: Validates access conditions before decryption
   - Worker integration: Fetches secrets from contract, decrypts via keystore, injects into WASI env

3. **Dashboard Secrets Page Refactoring** ✨ **NEW**:
   - Reduced main page from 667 → 168 lines (75% reduction)
   - Created 5 reusable components (types, utils, form, list, card, builder)
   - Improved type safety: replaced `any` with `unknown` + type guards
   - React best practices: useCallback, proper dependencies
   - Features: view/create/edit/delete secrets with real-time updates
   - Client-side encryption with XOR (MVP)

### 🔧 Previous Enhancements (2025-10-10):

1. **Encrypted Secrets with WASI Environment Variables**:
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
│   OutLayer Contract         │
│   (outlayer.near)           │
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
near contract deploy outlayer.testnet \
  use-file res/local/offchainvm_contract.wasm \
  with-init-call new \
  json-args '{"owner_id":"outlayer.testnet","operator_id":"worker.testnet"}' \
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
# - OFFCHAINVM_CONTRACT_ID=outlayer.testnet
# - OPERATOR_ACCOUNT_ID=worker.testnet
# - OPERATOR_PRIVATE_KEY=ed25519:...
# - KEYSTORE_BASE_URL=http://localhost:8081 (optional, for encrypted secrets)
# - KEYSTORE_AUTH_TOKEN=your-keystore-token (optional)

# Run worker
cargo run
```

### Using Repo-Based Secrets (NEW - Recommended!)
```bash
# 1. Start dashboard
cd dashboard
npm run dev
# Open http://localhost:3000/secrets

# 2. Connect wallet and create secrets via UI:
#    - Enter repo: github.com/alice/myproject
#    - Enter branch (optional): main
#    - Enter profile: production
#    - Enter JSON secrets: {"OPENAI_KEY":"sk-...", "API_TOKEN":"secret123"}
#    - Select access condition (e.g., AllowAll, Whitelist, NEAR balance)
#    - Click "Encrypt & Store Secrets"
#    - Secrets are encrypted client-side and stored on contract

# 3. Request execution with secrets_ref
near call outlayer.testnet request_execution \
  '{
    "code_source": {
      "repo": "https://github.com/alice/myproject",
      "commit": "main",
      "build_target": "wasm32-wasip1"
    },
    "secrets_ref": {
      "profile": "production",
      "account_id": "alice.testnet"
    },
    "resource_limits": {
      "max_instructions": 1000000000,
      "max_memory_mb": 128,
      "max_execution_seconds": 60
    },
    "input_data": "{}"
  }' \
  --accountId user.testnet \
  --deposit 0.1

# 4. Worker will automatically:
#    - Fetch encrypted secrets from contract (repo + branch + profile + owner)
#    - Validate access conditions via keystore
#    - Decrypt secrets via keystore
#    - Parse JSON: {"OPENAI_KEY":"sk-...", "API_TOKEN":"secret123"}
#    - Inject into WASI environment
#    - Your WASM code can use: std::env::var("OPENAI_KEY")

# View all your secrets:
near view outlayer.testnet list_user_secrets '{"account_id":"alice.testnet"}'

# Delete secrets (with storage refund):
near call outlayer.testnet delete_secrets \
  '{
    "repo": "github.com/alice/myproject",
    "branch": "main",
    "profile": "production"
  }' \
  --accountId alice.testnet \
  --depositYocto 1
```

## 📚 Documentation

- [PROJECT.md](PROJECT.md) - Full technical specification
- [MVP_DEVELOPMENT_PLAN.md](MVP_DEVELOPMENT_PLAN.md) - Development plan with code examples
- [NEAROffshoreOnepager.md](NEAROffshoreOnepager.md) - Marketing one-pager
- [README.md](README.md) - Quick start guide
- [contract/README.md](contract/README.md) - Contract API documentation
- [worker/README.md](worker/README.md) - Worker setup and configuration
- [dashboard/REFACTORING_SUMMARY.md](dashboard/REFACTORING_SUMMARY.md) - Dashboard refactoring details ✨ **NEW**

## 🎯 Timeline

- ✅ **Contract**: 1 week - COMPLETE + ENHANCED
- ✅ **Coordinator API**: 1-2 weeks - COMPLETE
- ✅ **Worker**: 2-3 weeks - COMPLETE (100%)
- ⏳ **Test WASM**: 1 day - 50%
- ⏳ **Testing**: 1 week - 0%
- **Total MVP**: ~5-7 weeks for 1 experienced Rust developer

## 💡 Important Notes

1. **Coordinator API running on port 8080** - can test endpoints right now
2. **Keystore worker running on port 8081** - **ISOLATED, accessed only via coordinator proxy** (not directly from outside)
   - **Docker networking**: Coordinator uses `host.docker.internal:8081` on Mac/Windows
   - Set `KEYSTORE_BASE_URL=http://host.docker.internal:8081` in `coordinator/.env`
3. **Dashboard running on port 3000** - full UI for secrets management ✨ **NEW**
   - Dashboard uses coordinator endpoints (`/secrets/pubkey`, `/github/resolve-branch`)
   - **Never directly accesses keystore (port 8081)** for security
4. **Auth disabled in dev mode** (`REQUIRE_AUTH=false` in `.env`) - can make requests without token
5. **WASM cache** created in `/tmp/offchainvm/wasm` - remember to change to production path
6. **PostgreSQL and Redis** running via docker-compose
7. **All SQL migrations applied** - database is ready
8. **Resource metrics are now real** - instructions and time_ms from actual execution
9. **Dynamic pricing** - users see estimated cost before execution
10. **API keys** - can be embedded in NEAR_RPC_URL and NEARDATA_API_URL for paid services
11. **Repo-based secrets** - Store once, use everywhere. Secrets indexed by user for O(1) lookups ✨ **NEW**
12. **Access control** - Whitelist, NEAR/FT/NFT balance checks, regex patterns, complex Logic conditions ✨ **NEW**

## 🔐 TEE (Trusted Execution Environment) Integration

OutLayer supports **optional TEE mode** using Intel TDX (Trust Domain Extensions) via Phala Cloud. Workers can run in two modes:
- **Standard mode**: Regular execution without attestation (faster, cheaper)
- **TEE mode**: Execution inside Intel TDX with cryptographic attestation (verifiable, secure)

### Worker Registration Contract (`register-contract`)

**Deployed at**: `worker.outlayer.testnet` (testnet) / `worker.outlayer.near` (mainnet)

Before a TEE worker can execute code, it must **register on-chain** with proof of its TEE environment:

1. **Worker generates TDX quote** - Calls Intel TDX hardware to get attestation quote
2. **Submit to register-contract** - Calls `register_worker(tdx_quote, collateral, public_key)`
3. **On-chain verification** - Contract uses Intel DCAP QVL library to verify quote signature
4. **Extract RTMR3** - Contract extracts worker measurement (SHA384 hash) from verified quote
5. **Whitelist worker** - RTMR3 added to whitelist + worker's public key added for `resolve_execution` auth

**Key Point**: The registered public key can ONLY call `resolve_execution` on the main contract to finalize executions. This prevents unauthorized result submission.

**See**: [PHALA_TEE_REGISTRATION_FLOW.md](PHALA_TEE_REGISTRATION_FLOW.md) for detailed registration steps.

### Keystore Worker (TEE-Required)

**Port**: 8081 (isolated, accessed only via coordinator proxy)

The keystore worker MUST run in TEE because it handles secret decryption. It provides:

- `GET /pubkey?repo=X&branch=Y` - Get encryption public key for secrets
- `POST /decrypt` - Decrypt secrets with TEE attestation + access control validation
- `GET /health` - Health check

**Architecture**:
```
┌─────────────────────┐
│ Client (Dashboard)  │
│  or WASM Executor   │
└──────────┬──────────┘
           │ Never direct access!
           ↓
┌─────────────────────┐
│  Coordinator Proxy  │ ← All requests go through here
│  /secrets/pubkey    │
│  /secrets/decrypt   │
└──────────┬──────────┘
           │ host.docker.internal:8081
           ↓
┌─────────────────────┐
│  Keystore Worker    │ ← MUST run in TEE
│  (Intel TDX)        │
│  - Decrypt secrets  │
│  - Validate access  │
│  - Generate quotes  │
└─────────────────────┘
```

**Security**: Keystore worker validates access conditions (AllowAll, Whitelist, NEAR balance, etc.) before decrypting secrets. This ensures users control who can access their secrets.

### TEE Execution Attestation

When a TEE worker executes WASM code, it generates an **attestation** stored in the coordinator database:

**Attested Data**:
- `worker_measurement` (RTMR3) - SHA384 hash proving worker identity
- `repo_url` + `commit_hash` - Exact source code compiled
- `wasm_hash` - SHA256 of compiled WASM binary
- `input_hash` - SHA256 of input data
- `output_hash` - SHA256 of output result
- `tdx_quote` - Full Intel TDX quote (base64) signed by Intel
- `transaction_hash` - NEAR transaction for blockchain verification

**Verification**:
1. Extract RTMR3 from TDX quote (offset 256, 48 bytes) → compare to `worker_measurement`
2. Fetch NEAR transaction → extract input/output → calculate SHA256 → compare to hashes
3. View source code on GitHub → audit what was executed
4. Download WASM binary → calculate SHA256 → compare to `wasm_hash`

**Dashboard**: Navigate to `/executions` → click "View Attestation" → interactive verification with live hash calculation.

### Phala Cloud Deployment

OutLayer uses **Phala Cloud** (dstack) to run TEE workers with Intel TDX. We have dedicated Docker configurations for Phala deployment.

#### Docker Compose Files

**Main Worker** - [docker/docker-compose.phala.yml](docker/docker-compose.phala.yml)
- Builds worker with Phala/TDX dependencies
- Mounts `/var/run/dstack.sock` for TDX quote generation
- Uses `.env.phala` for configuration
- Includes health checks and restart policies

**Keystore Worker** - [keystore-worker/docker-compose.yml](keystore-worker/docker-compose.yml)
- Runs keystore service in TEE (port 8081)
- MUST run in Phala TEE environment
- Handles secret decryption with access control
- Uses same dstack socket for attestation

**Environment Files**:
- [docker/.env.phala.example](docker/.env.phala.example) - Template for Phala worker config
- [keystore-worker/.env.example](keystore-worker/.env.example) - Template for keystore config

#### Full Deployment Process (from scratch)

**Step 1: Prepare Configuration**
```bash
# Copy example env files
cp docker/.env.phala.example docker/.env.phala
cp keystore-worker/.env.example keystore-worker/.env

# Edit .env.phala with your values:
# - DSTACK_SIMULATOR_ENDPOINT=/var/run/dstack.sock
# - NEAR_RPC_URL=https://rpc.testnet.near.org (or with API key)
# - OFFCHAINVM_CONTRACT_ID=outlayer.testnet
# - REGISTER_CONTRACT_ID=worker.outlayer.testnet
# - OPERATOR_ACCOUNT_ID=yourworker.testnet
# - OPERATOR_PRIVATE_KEY=ed25519:your_private_key
# - KEYSTORE_BASE_URL=http://keystore:8081 (internal docker network)
# - TEE_MODE=tdx
```

**Step 2: Build Docker Images**
```bash
cd docker

# Build worker image
docker-compose -f docker-compose.phala.yml build

# Build and push to Phala registry (if deploying to Phala Cloud)
./scripts/build_and_push_phala.sh
```

**Step 3: Deploy to Phala Cloud**
```bash
# Using Phala CLI
phala cvms create --name outlayer-worker --image your-registry/outlayer-worker:latest

# Or deploy via Phala dashboard
# Upload docker-compose.phala.yml to Phala Cloud UI
```

**Step 4: Worker Registration (ONE-TIME)**

Once the worker starts in Phala TEE, it must register before executing any tasks:

```bash
# Worker automatically runs registration on startup if not registered
# Check worker logs to see registration status:
docker logs outlayer-worker-1

# Expected log output:
# [INFO] Connecting to Phala dstack socket: /var/run/dstack.sock
# [INFO] Generating TDX quote...
# [INFO] Fetching Intel collateral...
# [INFO] Submitting registration to worker.outlayer.testnet...
# [INFO] Registration successful! RTMR3: 2641ff132f21a1e9...
# [INFO] Worker public key added to main contract
# [INFO] Worker is now authorized to execute tasks
```

**Registration happens automatically** via `worker/src/registration.rs`:
1. Worker generates fresh NEAR key pair (or loads existing from env)
2. Connects to `/var/run/dstack.sock` (Phala dstack daemon)
3. Calls `tdx_quote()` API to get Intel TDX quote
4. Fetches Intel collateral (certificates, TCB info, CRLs) from Intel PCS API
5. Calls `register_worker(tdx_quote, collateral, public_key)` on `worker.outlayer.testnet`
6. Contract verifies Intel signature with DCAP QVL library
7. Contract extracts RTMR3 from quote → adds to whitelist
8. Contract stores worker's public key for `resolve_execution` authorization
9. Worker saves registration status locally (skips re-registration on restart)

**Step 5: Start Keystore Worker**
```bash
cd keystore-worker

# Deploy keystore to Phala TEE (separate instance)
docker-compose up -d

# Verify keystore is accessible from worker
curl http://keystore:8081/health
# Expected: {"status":"ok"}
```

**Step 6: Verify Deployment**
```bash
# Check worker is polling for tasks
docker logs outlayer-worker-1 --tail 50

# Expected log output:
# [INFO] Polling coordinator for tasks...
# [INFO] No tasks available, waiting...
# [INFO] Polling coordinator for tasks...

# Check registration on blockchain
near view worker.outlayer.testnet get_worker '{"rtmr3":"2641ff132f21a1e9..."}'
# Expected: worker info with public_key and registration timestamp
```

**Step 7: Monitor Attestations**
```bash
# Check coordinator database for attestations
psql postgres://postgres:postgres@coordinator-db/offchainvm \
  -c "SELECT job_id, worker_measurement, created_at FROM attestations ORDER BY created_at DESC LIMIT 5;"

# View attestations in dashboard
# Navigate to: https://dashboard.outlayer.io/executions
# Click "View Attestation" on any completed execution
```

#### Configuration Details

**Key Environment Variables** (`.env.phala`):

| Variable | Description | Example |
|----------|-------------|---------|
| `DSTACK_SIMULATOR_ENDPOINT` | Phala dstack socket path | `/var/run/dstack.sock` |
| `TEE_MODE` | TEE attestation mode | `tdx` (Phala), `sgx`, `sev`, `none` |
| `REGISTER_CONTRACT_ID` | Worker registration contract | `worker.outlayer.testnet` |
| `OFFCHAINVM_CONTRACT_ID` | Main execution contract | `outlayer.testnet` |
| `OPERATOR_ACCOUNT_ID` | Worker's NEAR account | `worker.testnet` |
| `OPERATOR_PRIVATE_KEY` | Worker's NEAR private key | `ed25519:5J...` |
| `KEYSTORE_BASE_URL` | Keystore service URL | `http://keystore:8081` |
| `KEYSTORE_AUTH_TOKEN` | Auth token for keystore | `your-token` |
| `API_BASE_URL` | Coordinator API URL | `https://coordinator.outlayer.io` |
| `API_AUTH_TOKEN` | Coordinator auth token | `your-token` |
| `NEAR_RPC_URL` | NEAR RPC endpoint | `https://rpc.testnet.near.org` |

**Important Notes**:
- Worker and Keystore MUST both run in Phala TEE for attestation to work
- Registration only happens once per worker (RTMR3 + public key pair)
- If you rebuild worker with code changes, RTMR3 changes → must re-register
- Keystore should be on separate Phala instance for security isolation
- Use internal Docker networking (`keystore:8081`) for worker ↔ keystore communication

**Troubleshooting**:
```bash
# Check dstack socket is accessible
ls -la /var/run/dstack.sock

# Test TDX quote generation manually
curl --unix-socket /var/run/dstack.sock http://localhost/tdx_quote

# Check Intel collateral fetching
curl "https://api.trustedservices.intel.com/sgx/certification/v4/tcb?fmspc=00906ED50000"

# View registration transaction on blockchain
near tx-status <TRANSACTION_HASH> --accountId worker.testnet
```

**See Also**:
- [docker/TUTORIAL.md](docker/TUTORIAL.md) - Detailed Phala deployment guide
- [PHALA_TEE_REGISTRATION_FLOW.md](PHALA_TEE_REGISTRATION_FLOW.md) - Registration flow documentation
- [worker/src/registration.rs](worker/src/registration.rs) - Registration implementation code
- [register-contract/src/lib.rs](register-contract/src/lib.rs) - Registration contract code

### Two-Mode System

OutLayer supports **mixed worker fleet**:

| Feature | Standard Mode | TEE Mode |
|---------|--------------|----------|
| Execution Speed | Faster | Slight overhead |
| Cost | Lower | Higher (TEE infrastructure) |
| Attestation | No | Yes (Intel TDX) |
| Secrets Access | Limited | Full (via keystore) |
| Verification | Trust operator | Cryptographically verifiable |
| Use Case | Public computations | Private/sensitive data |

**How to Choose**:
- Use **standard mode** for public data, speed-critical tasks
- Use **TEE mode** for secrets, verifiable computation, high-security requirements

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

**Current Date**: 2025-11-15
**Version**: Production with TEE Support
**Status**: Contract ✅ | Coordinator ✅ | Worker ✅ | Keystore ✅ | Dashboard ✅ | TEE Attestation ✅ | Phala Integration ✅ | Register Contract ✅
