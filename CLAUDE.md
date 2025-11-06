# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

<CRITICAL>
You are an assistant working on a **capability-based execution system**. Every component implements security boundaries. When modifying code:

1. **Understand the security model first** - Read the architecture section before making changes
2. **Ask before breaking boundaries** - If unsure about trust boundaries, ask the human
3. **Follow established patterns** - Especially for WASI development (see WASI section)
4. **Do NOT manage deployment** - Human handles coordinator restarts, docker-compose, contract deployment

If you are not sure how to implement something while preserving security properties, reply "I don't know how to do it" and ask for clarification.
</CRITICAL>

---

# NEAR OutLayer: Capability-Based Off-Chain Execution

## 🎯 Architectural Vision

**NEAR OutLayer** implements NEAR Protocol's capability-based security model for verifiable off-chain computation. It is not merely an "off-chain compute layer" - it is a **distributed TEE-ready system** that demonstrates how browser-based WASM execution can integrate with blockchain primitives for trustless computation.

### Core Thesis

NEAR Protocol provides unique primitives for WASM TEE integration:
- **Function Call Access Keys** → Unforgeable capabilities with fine-grained permissions
- **Asynchronous Receipt Model** → Explicit data dependencies and verifiable execution flow
- **Gas Metering** → Deterministic resource accounting (1 Tgas = 1ms)
- **Storage Staking** → Economic capability management with reclamation
- **State Attestation** → Merkle-based verification anchors

OutLayer implements these primitives in production code **today**, creating the architecture for Phase 2 TEE integration (Phala Network, Intel SGX, AMD SEV) and Phase 3 browser WASM TEE nodes.

---

## 🏗️ Architectural Primitives → Implementation Mapping

### 1. Access Keys as Capabilities

**NEAR Protocol Pattern**: Function Call Access Keys provide protocol-level capability delegation. A key can call specific methods on specific contracts with a gas allowance, but cannot transfer tokens or modify account state.

**OutLayer Implementation**: Secrets management system (`contract/src/secrets.rs`)

```rust
pub struct SecretsEntry {
    pub encrypted_secrets: Vec<u8>,
    pub access_condition: AccessCondition,  // Unforgeable capability check
    pub stored_at: u64,
}

pub enum AccessCondition {
    AllowAll,                                // Open capability
    Whitelist(Vec<AccountId>),              // Explicit capability grant
    AccountPattern(String),                  // Pattern-based capability
    RequireNearBalance { min_balance: U128 }, // Economic capability
    RequireFtBalance { contract_id: AccountId, min_balance: U128 },
    RequireNftOwnership { contract_id: AccountId, token_id: Option<String> },
    Logic(LogicCondition),                   // Composable capabilities (AND/OR/NOT)
}
```

**Key Property**: Access conditions are **validated by keystore before decryption** (keystore-worker/src/keystore_service.py:validate_access_condition). This creates an unforgeable, verifiable capability system where the contract stores encrypted data and keystore enforces access control.

**Code Flow**:
1. User stores secrets: `contract.store_secrets()` → encrypted bytes + access condition on-chain
2. Worker requests secrets: `contract.get_secrets()` → retrieves encrypted data
3. Worker decrypts: `keystore.decrypt()` → validates access condition via NEAR RPC, then decrypts
4. Worker injects: `worker/src/executor/wasi_env.rs` → secrets become WASI environment variables

**Security Boundary**: Contract provides attestation (who stored what), keystore enforces access control, worker provides isolated execution.

### 2. Asynchronous Receipt Model → Job-Based Workflow

**NEAR Protocol Pattern**: Transactions create receipts (1-2 blocks later execution). Cross-contract calls generate DataReceipts with explicit `output_data_receivers` and `input_data_ids`, creating verifiable data flow.

**OutLayer Implementation**: Job atomicity with WASM cache optimization (`coordinator/src/handlers/jobs.rs`)

```rust
// Database constraint prevents duplicate work
CREATE TABLE jobs (
    job_id BIGSERIAL PRIMARY KEY,
    request_id BIGINT NOT NULL,
    data_id TEXT NOT NULL,
    job_type TEXT CHECK (job_type IN ('compile', 'execute')),
    UNIQUE (request_id, data_id, job_type)  // Atomic capability claim
);

pub async fn claim_jobs_for_request(
    request_id: i64,
    data_id: &str,
    wasm_checksum: Option<&str>,
) -> Result<Vec<JobType>> {
    // If WASM cached: return [Execute] only
    // If WASM not cached: return [Compile, Execute]
    // If another worker claimed: return 409 CONFLICT
}
```

**Key Property**: Job claims are **atomic and idempotent**. Multiple workers polling simultaneously get deterministic job assignment - one succeeds, others get empty result or conflict. This mirrors NEAR's receipt model where each receipt executes exactly once.

**Code Flow**:
1. Contract emits event: `ExecutionRequested` → async trigger (like receipt creation)
2. Worker polls: `GET /tasks/poll` → Redis BRPOP (blocking, no busy-wait)
3. Worker claims: `POST /jobs/claim` → atomic DB transaction
4. Worker executes: Compile job → uploads WASM → Execute job → submits result
5. Contract finalizes: `resolve_execution()` → callback to requester (like DataReceipt delivery)

**Race Condition Protection**: Distributed lock (`coordinator/src/handlers/locks.rs`) prevents duplicate compilations. Tests verify this in `tests/job_workflow.sh`.

### 3. Gas Metering → Resource Accounting

**NEAR Protocol Pattern**: Every WASM instruction costs gas (2,207,874 gas per instruction as of protocol 1.22.0). Runtime injects gas metering code at every basic block. 1 Tgas = 1ms execution time on minimum validator hardware. Maximum 300 Tgas per transaction.

**OutLayer Implementation**: Dual runtime fuel tracking (`worker/src/executor/`)

**WASI P1 (wasmi)** - Instruction-level metering:
```rust
// worker/src/executor/wasi_p1.rs
let mut store = Store::new(&engine, ());
store.limiter(|_| &mut ResourceLimiter { ... });

// Fuel metering (1 fuel = N instructions)
store.set_fuel(max_instructions)?;
instance.exports.main().call(&mut store)?;
let fuel_consumed = max_instructions - store.fuel_consumed()?;

// Return actual metrics
ResourceMetrics {
    instructions: fuel_consumed,
    time_ms: start.elapsed().as_millis() as u64,
}
```

**WASI P2 (wasmtime)** - Component model metering:
```rust
// worker/src/executor/wasi_p2.rs
let mut store = Store::new(&engine, HostState::new(env_vars));
store.set_fuel(max_instructions as u64)?;
store.set_epoch_deadline(ticks_for_duration(max_execution_seconds));

let (result, _) = func.call_and_post_return(&mut store, (input_bytes,))?;
let fuel_consumed = max_instructions as u64 - store.get_fuel()?;
```

**Key Property**: Metrics are **real and verifiable**. Contract uses these for pricing (`contract/src/execution.rs:calculate_actual_cost`). No fake zeros - actual fuel consumption from WASM runtime.

**Dynamic Pricing**:
```rust
// contract/src/execution.rs
pub fn estimate_cost(&self, limits: &ResourceLimits) -> Balance {
    let base_fee = self.pricing.base_fee;
    let instruction_cost = (limits.max_instructions / 1_000_000) as u128 * self.pricing.per_instruction_fee;
    let time_cost = limits.max_execution_seconds as u128 * 1000 * self.pricing.per_ms_fee;
    base_fee + instruction_cost + time_cost
}
```

Users pay estimated cost upfront (anti-DoS), get refund after execution based on actual usage (fairness).

### 4. Storage Staking → Capability Resource Management

**NEAR Protocol Pattern**: Accounts lock tokens proportional to storage used (1e19 yoctoNEAR per byte). Deleting data returns staked tokens. This is economic capability management - data storage requires holding resources.

**OutLayer Implementation**: Secrets storage with refunds (`contract/src/secrets.rs`)

```rust
#[payable]
pub fn store_secrets(
    &mut self,
    repo: String,
    branch: Option<String>,
    profile: String,
    encrypted_secrets: Vec<u8>,
    access_condition: AccessCondition,
) {
    let required_storage = self.calculate_storage_cost(&encrypted_secrets);
    let attached_deposit = env::attached_deposit();
    require!(attached_deposit >= required_storage, "Insufficient storage deposit");

    // Store with user index for O(1) lookups
    let key = SecretKey { repo, branch, profile, owner };
    self.secrets.insert(&key, &entry);
    self.user_secrets_index.entry(owner).or_insert(UnorderedSet::new()).insert(&key);
}

pub fn delete_secrets(&mut self, repo: String, branch: Option<String>, profile: String) {
    let key = SecretKey { repo, branch, profile, owner: env::predecessor_account_id() };
    let entry = self.secrets.remove(&key).expect("Secrets not found");

    // Refund storage deposit
    let refund = self.calculate_storage_cost(&entry.encrypted_secrets);
    Promise::new(env::predecessor_account_id()).transfer(refund);
}
```

**Key Property**: Storage is **capability-gated and economically bounded**. Users must pay to store, preventing spam. Refunds incentivize cleanup. Index structure (`LookupMap<AccountId, UnorderedSet<SecretKey>>`) mirrors NEAR's trie-based state management.

**Trade-off**: ~64 bytes index overhead per secret for O(1) user lookups. Acceptable for UX (`list_user_secrets` in dashboard).

### 5. State Attestation → Keystore Verification

**NEAR Protocol Pattern**: State root hashes in block headers. Merkle proofs verify state transitions without full state replication. Validators attest to chunk validity by signing block headers.

**OutLayer Implementation**: TEE attestation verification (`keystore-worker/src/keystore_service.py`)

```python
def verify_attestation(attestation: Dict, expected_measurements: Dict) -> bool:
    """
    Verifies worker's TEE attestation.
    MVP: Simulated (attestation_type == 'none')
    Production: SGX/SEV with measurement verification
    """
    attestation_type = attestation.get('attestation_type')

    if attestation_type == 'none':
        # MVP: Accept all (Phase 1)
        return True
    elif attestation_type == 'sgx':
        # Phase 2: Verify Intel SGX quote
        return verify_sgx_quote(attestation['quote'], expected_measurements['mrenclave'])
    elif attestation_type == 'sev':
        # Phase 2: Verify AMD SEV-SNP attestation report
        return verify_sev_report(attestation['report'], expected_measurements['measurement'])
    else:
        return False
```

**Key Property**: Keystore acts as **verifier**, not just decryptor. Before returning plaintext secrets, it:
1. Verifies worker's TEE attestation (worker/src/keystore_client.rs:generate_attestation)
2. Validates access conditions via NEAR RPC (keystore-worker/src/near_client.py)
3. Only then decrypts and returns secrets

**Current State**: Attestation is simulated (`attestation_type: 'none'`). Code structure is ready for Phase 2 SGX/SEV integration - just swap verification logic.

---

## 📦 Component Architecture

### Smart Contract - Capability Provider & Attestation Anchor

**Location**: `contract/`
**Role**: Protocol-level access control, not just storage

**Core Insight**: Contract doesn't execute computation - it **manages capabilities** and **anchors attestations**. Worker execution is off-chain, but contract provides:
- **Economic security**: Payment upfront, refund after verified execution
- **Capability delegation**: Secrets with access conditions, indexed by user
- **State attestation**: Execution requests/results recorded on-chain
- **Async coordination**: Events trigger workers (yield/resume pattern via `promise_yield_create`)

**Key Files**:
- `src/lib.rs` - Main entry point, initialization
- `src/execution.rs` - Request/resolve execution with yield/resume
- `src/secrets.rs` - Secrets storage with access conditions (capability system)
- `src/events.rs` - Event emission for async coordination
- `src/admin.rs` - Owner/operator management (privilege separation)

**Build**: `cargo near build` (requires Rust 1.85.0 via rust-toolchain.toml)
**Deploy**: `near contract deploy outlayer.testnet use-file target/near/outlayer_contract.wasm ...`
**Test**: `cargo test` (18 unit tests)

**Critical Pattern**: `promise_yield_create` in `request_execution` → `promise_yield_resume` in `resolve_execution`. This is NEAR's async execution primitive - contract yields control, worker executes, worker calls back with result.

### Coordinator - Trusted Computing Base Coordinator

**Location**: `coordinator/`
**Role**: Centralized state management with security boundaries

**Core Insight**: Coordinator is the **runtime** (NEAR's nearcore equivalent). Workers have zero direct access to PostgreSQL/Redis - all mediated through HTTP API with authentication. This enforces:
- **Deterministic state transitions**: Jobs claimed atomically, WASM cache managed centrally
- **Resource coordination**: Distributed locks, LRU eviction
- **Security boundary enforcement**: Worker authentication, rate limiting (production)

**Key Files**:
- `src/main.rs` - Axum server setup, middleware
- `src/handlers/jobs.rs` - **CRITICAL**: Atomic job claims prevent race conditions
- `src/handlers/wasm_cache.rs` - WASM storage with LRU eviction
- `src/handlers/locks.rs` - Distributed locking (Redis-based)
- `src/handlers/github.rs` - Branch resolution with Redis caching
- `src/storage/` - PostgreSQL + Redis abstractions

**Build**:
```bash
# Requires PostgreSQL connection for sqlx
cargo sqlx migrate run
cargo sqlx prepare  # Generates .sqlx/sqlx-data.json
SQLX_OFFLINE=true cargo build --release  # For Docker
```

**Critical Dependency**: Database must be running before build (sqlx compile-time verification). In Docker: generate sqlx-data.json first, then build with SQLX_OFFLINE=true.

**Ports**: 8080 (HTTP), PostgreSQL 5432, Redis 6379

**Security Note**: Dev mode has `REQUIRE_AUTH=false` in `.env`. Production MUST enable auth with SHA256 hashed tokens in database.

### Worker - Isolated Execution Environment

**Location**: `worker/`
**Role**: WASM execution with capability-restricted I/O

**Core Insight**: Worker is a **sandboxed runtime** (NEAR's near-vm-runner equivalent). Each execution:
1. Fetches secrets from contract (encrypted)
2. Decrypts via keystore (access control validated)
3. Injects secrets as WASI environment variables
4. Executes WASM in isolated runtime (wasmi or wasmtime)
5. Tracks fuel consumption (real metrics)
6. Submits result to contract via NEAR RPC

**Dual Runtime Strategy**:
- **wasmi** (WASI P1): Pure interpreter, simpler execution model, stdin/stdout I/O
- **wasmtime** (WASI P2): Component model, HTTP support, more complex capabilities

**Key Files**:
- `src/main.rs` - Main loop: poll tasks → claim jobs → execute → submit results
- `src/executor/wasi_p1.rs` - wasmi execution with fuel metering
- `src/executor/wasi_p2.rs` - wasmtime execution with component model
- `src/executor/wasi_env.rs` - **CRITICAL**: Environment variable injection (secrets → WASI)
- `src/compiler.rs` - GitHub repo → WASM compilation (Docker sandboxed)
- `src/keystore_client.rs` - Keystore communication with attestation
- `src/near_client.rs` - NEAR RPC transaction submission

**Build**: `cargo build` (requires Docker daemon for compilation features)

**Configuration** (.env):
```bash
API_BASE_URL=http://localhost:8080
API_AUTH_TOKEN=your-token
NEAR_RPC_URL=https://rpc.testnet.near.org
OFFCHAINVM_CONTRACT_ID=outlayer.testnet
OPERATOR_ACCOUNT_ID=worker.testnet
OPERATOR_PRIVATE_KEY=ed25519:...
KEYSTORE_BASE_URL=http://localhost:8081
ENABLE_EVENT_MONITOR=false  # Only one worker should monitor events
```

**Security Properties**:
- WASM execution is isolated (wasmi/wasmtime sandbox)
- Secrets never touch disk unencrypted
- Operator key only used for `resolve_execution` transactions
- Docker compilation with `--network=none` (no internet access during build)

### Keystore - Secret Capability Verifier

**Location**: `keystore-worker/`
**Role**: Access control enforcement before decryption

**Core Insight**: Keystore is **not just a decryptor** - it is a **capability verifier**. It acts as the security boundary between encrypted storage (contract) and plaintext use (worker). Before decryption:
1. Verify worker's TEE attestation (currently simulated)
2. Validate access conditions via NEAR RPC
3. Only then decrypt with master secret

**Key Files**:
- `src/keystore_service.py` - Main Flask app, encryption/decryption
- `src/near_client.py` - NEAR RPC calls for access condition validation
- `encrypt_secrets.py` - Helper script for client-side encryption (JSON format)

**Encryption**: XOR with master secret (MVP). Production will use ChaCha20-Poly1305 with ECDH key exchange.

**Ports**: 8081 (HTTP, **NEVER exposed directly** - coordinator proxies)

**Docker Networking**: Coordinator accesses keystore via `host.docker.internal:8081` on Mac/Windows. Set `KEYSTORE_BASE_URL=http://host.docker.internal:8081` in coordinator/.env.

**Security Boundary**: Dashboard calls coordinator (`/secrets/pubkey`), coordinator proxies to keystore. This enforces authentication and rate limiting at coordinator layer.

### Dashboard - Capability Grant Interface

**Location**: `dashboard/`
**Role**: User-facing secrets management with client-side encryption

**Core Insight**: Dashboard demonstrates **principle of least privilege** in browser context. It:
- Connects to NEAR wallet (wallet-selector)
- Creates secrets locally (JSON format)
- Encrypts client-side (fetches pubkey from coordinator, never plaintext to server)
- Stores encrypted bytes on contract
- Never directly accesses keystore (port 8081)

**Key Files**:
- `app/secrets/page.tsx` - Main secrets management page (168 lines after refactor)
- `app/secrets/components/SecretsForm.tsx` - Create/edit form with encryption
- `app/secrets/components/AccessConditionBuilder.tsx` - Access condition UI
- `lib/near.ts` - NEAR wallet integration

**Build**:
```bash
npm install
npm run dev     # Development (Turbopack, port 3000)
npm run build   # Production build
```

**Security Property**: Plaintext secrets never leave browser until encrypted. Contract stores only encrypted bytes. Keystore decrypts only when access conditions met.

---

## 🔐 Security Boundaries & Trust Model

### Trust Boundaries

```
┌─────────────────────────────────────────────────────────────────┐
│ TRUSTED COMPUTING BASE                                          │
│                                                                 │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐        │
│  │   Contract   │  │ Coordinator  │  │   Keystore   │        │
│  │   (NEAR L1)  │  │ (PostgreSQL  │  │   (Secrets   │        │
│  │              │  │  + Redis)    │  │   Verifier)  │        │
│  └──────────────┘  └──────────────┘  └──────────────┘        │
│         │                  │                  │                │
│         │ Attestation      │ API Auth         │ Attestation   │
│         │ Anchor           │ Enforcement      │ Verification  │
└─────────┼──────────────────┼──────────────────┼────────────────┘
          │                  │                  │
          ▼                  ▼                  ▼
┌─────────────────────────────────────────────────────────────────┐
│ UNTRUSTED EXECUTION ENVIRONMENT                                 │
│                                                                 │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐        │
│  │   Worker     │  │    WASM      │  │  Dashboard   │        │
│  │  (Executor)  │  │   Sandbox    │  │  (Browser)   │        │
│  └──────────────┘  └──────────────┘  └──────────────┘        │
│                                                                 │
│  • Must authenticate to coordinator                             │
│  • Must provide attestation to keystore                         │
│  • WASM execution isolated (no system access)                   │
│  • Secrets only available as WASI env vars (ephemeral)          │
└─────────────────────────────────────────────────────────────────┘
```

### Attack Surface Analysis

**What CANNOT be compromised by malicious worker**:
- Contract state (worker only reads, contract validates)
- Other users' secrets (access control validated by keystore)
- Coordinator database (worker has no DB access, only HTTP API)
- NEAR operator keys (worker has key but can only call `resolve_execution`)

**What CAN be compromised by malicious worker**:
- Execution results for requests it processes (worker submits arbitrary results)
- Secrets for jobs with `AllowAll` access condition (no restrictions)
- Gas costs (worker can submit inflated metrics, but contract has MAX_INSTRUCTIONS cap)

**Mitigations**:
- **Economic**: Users pay for execution, malicious results hurt worker's reputation
- **Phase 2**: TEE attestation prevents worker code tampering
- **Phase 3**: Multiple workers execute same job, results verified via consensus

**CRITICAL**: Current MVP (Phase 1) trusts workers to execute honestly. This is acceptable for:
- Development/testing
- Closed worker sets (run your own worker)
- Non-critical computations

Production (Phase 2+) requires TEE attestation.

---

## 🛠️ Critical Development Patterns

### WASI Development Pattern

**Context**: When human asks to create a new WASI example for OutLayer.

**MANDATORY STEPS** (never skip):

1. **Read existing examples first**:
   ```bash
   ls wasi-examples/
   cat wasi-examples/WASI_TUTORIAL.md  # MUST READ
   ```

2. **Understand the structure**:
   - `Cargo.toml` must use `[[bin]]` format (not `[lib]`)
   - Must have `main()` function (not `lib.rs`)
   - Must use `wasm32-wasip1` or `wasm32-wasip2` target
   - WASI P1: stdin/stdout I/O
   - WASI P2: HTTP via `wasi:http` component model

3. **Copy proven pattern**:
   ```bash
   # Choose template
   cp -r wasi-examples/random-ark wasi-examples/my-example  # For P1
   cp -r wasi-examples/ai-ark wasi-examples/my-example      # For P2
   ```

4. **Modify carefully**:
   - Keep Cargo.toml structure
   - Keep WASI imports exactly as in template
   - Only modify business logic

5. **Test with runner**:
   ```bash
   cargo build --release --target wasm32-wasip1
   cd wasi-examples/wasi-test-runner
   cargo run -- --wasm ../my-example/target/wasm32-wasip1/release/my_example.wasm --input '{}'
   ```

**Why this matters**: WASI has subtle pitfalls (component model, memory management, I/O patterns). Existing examples are battle-tested. DO NOT write from scratch.

**Ask human**: "Which example should I use as template?" if multiple viable options exist.

### Secrets Injection Flow

**Context**: How secrets get from contract to WASM code.

**Full Flow**:

1. **User stores** (dashboard or CLI):
   ```bash
   # Dashboard: Secrets page → Create secrets form
   # Encrypted client-side before contract call
   near call outlayer.testnet store_secrets '{
     "repo": "github.com/alice/myproject",
     "branch": "main",
     "profile": "production",
     "encrypted_secrets": [base64_encrypted_bytes],
     "access_condition": {"AllowAll": {}}
   }' --accountId alice.testnet --deposit 0.01
   ```

2. **User requests execution**:
   ```bash
   near call outlayer.testnet request_execution '{
     "code_source": {
       "repo": "https://github.com/alice/myproject",
       "commit": "main",
       "build_target": "wasm32-wasip1"
     },
     "secrets_ref": {
       "profile": "production",
       "account_id": "alice.testnet"
     },
     "resource_limits": { ... }
   }' --accountId user.testnet --deposit 0.1
   ```

3. **Worker fetches secrets** (`worker/src/main.rs`):
   ```rust
   let secrets_ref = task.secrets_ref;
   let encrypted_secrets = contract.get_secrets(
       secrets_ref.repo,
       secrets_ref.branch,
       secrets_ref.profile,
       secrets_ref.account_id
   ).await?;
   ```

4. **Worker decrypts** (`worker/src/keystore_client.rs`):
   ```rust
   let attestation = generate_attestation(config.attestation_mode);
   let secrets_json = keystore_client.decrypt_secrets(
       encrypted_secrets,
       attestation
   ).await?;
   // Returns HashMap<String, String> parsed from JSON
   ```

5. **Worker injects** (`worker/src/executor/wasi_env.rs`):
   ```rust
   let env_vars = secrets_map.unwrap_or_default();
   let wasi_env = WasiEnv::new(env_vars);  // Converts to "KEY=VALUE\0" format

   // In wasmi/wasmtime imports:
   "environ_get" => wasi_env.environ_get(),
   "environ_sizes_get" => wasi_env.environ_sizes_get(),
   ```

6. **WASM code uses** (user's Rust code):
   ```rust
   fn main() {
       let api_key = std::env::var("OPENAI_KEY").expect("OPENAI_KEY not set");
       let result = call_openai_api(api_key);
       println!("{}", result);
   }
   ```

**Key Code Locations**:
- Contract: `contract/src/secrets.rs` (storage)
- Worker keystore call: `worker/src/keystore_client.rs:decrypt_secrets`
- WASI injection: `worker/src/executor/wasi_env.rs`
- WASI P1 executor: `worker/src/executor/wasi_p1.rs` (imports)
- WASI P2 executor: `worker/src/executor/wasi_p2.rs` (component model)

**Security**: Secrets exist in memory only during execution, never touch disk unencrypted, WASM sandbox prevents exfiltration outside stdout.

### Keystore Isolation Pattern

**Context**: Keystore (port 8081) must NEVER be directly accessible.

**Architecture**:
```
Dashboard (browser)
    │
    └─> Coordinator (port 8080)
            │
            └─> Keystore (port 8081, internal only)
```

**Why**:
- Coordinator enforces authentication (bearer tokens)
- Coordinator enforces rate limiting
- Coordinator logs all access
- Keystore does not implement these - it only verifies attestations and access conditions

**Docker Networking**:
- Mac/Windows: `KEYSTORE_BASE_URL=http://host.docker.internal:8081` in coordinator/.env
- Linux: Use host network mode or docker network with service name

**Coordinator Proxy** (`coordinator/src/handlers/secrets.rs`):
```rust
pub async fn get_secrets_pubkey(
    Extension(keystore_client): Extension<KeystoreClient>,
    Query(params): Query<SecretsKeyQuery>,
) -> Result<Json<PubkeyResponse>, StatusCode> {
    // Coordinator proxies to keystore, adds auth/logging
    keystore_client.get_pubkey(&params.repo, &params.owner, params.branch.as_deref()).await
}
```

**Dashboard Usage** (`dashboard/lib/secrets.ts`):
```typescript
// Dashboard calls coordinator, NOT keystore directly
const response = await fetch(`${COORDINATOR_API_URL}/secrets/pubkey?repo=${repo}&owner=${owner}`);
const { pubkey } = await response.json();
const encrypted = encryptWithXOR(secretsJson, pubkey);
```

**NEVER**:
- Expose keystore port (8081) in docker-compose ports section
- Call keystore directly from dashboard
- Share keystore URL with untrusted clients

### Database Migrations Pattern

**Context**: Adding/modifying PostgreSQL schema.

**Steps**:
```bash
cd coordinator

# 1. Create migration
sqlx migrate add your_feature_name
# Creates: migrations/YYYYMMDDHHMMSS_your_feature_name.sql

# 2. Write SQL
echo "CREATE TABLE new_table (...);" > migrations/YYYYMMDDHHMMSS_your_feature_name.sql

# 3. Apply migration (requires running PostgreSQL)
sqlx migrate run

# 4. Regenerate sqlx-data.json (for Docker builds)
cargo sqlx prepare
# Updates: .sqlx/sqlx-data.json

# 5. Commit both files
git add migrations/ .sqlx/
git commit -m "feat: add new_table for your_feature"
```

**CRITICAL**: `cargo sqlx prepare` MUST be run after schema changes. Docker builds use `SQLX_OFFLINE=true` which requires up-to-date sqlx-data.json.

**Testing Migrations**:
```bash
# Reset database to clean state
docker-compose down -v
docker-compose up -d postgres
sqlx migrate run

# Verify
psql postgres://postgres:postgres@localhost/offchainvm
\dt  -- List tables
```

---

## ⚙️ Build & Test Commands

### Building Components

```bash
# Contract (requires cargo-near)
cd contract && cargo near build
# Output: target/near/outlayer_contract.wasm (~200KB)

# Coordinator (requires PostgreSQL connection)
cd coordinator
sqlx migrate run
cargo build --release
# Or for Docker: SQLX_OFFLINE=true cargo build --release

# Worker (requires Docker daemon for compilation features)
cd worker && cargo build --release

# Keystore
cd keystore-worker && cargo build --release

# Dashboard
cd dashboard
npm install
npm run build
```

### Running Tests

```bash
# Contract unit tests (fast, no dependencies)
cd contract && cargo test

# Integration tests (requires coordinator + PostgreSQL + Redis running)
cd tests
./unit.sh              # WASM builds + cargo tests
./compilation.sh       # Real GitHub repo compilation
./integration.sh       # API endpoint tests
./job_workflow.sh      # Race condition verification (CRITICAL)
./e2e.sh              # Full contract flow (requires testnet contract)
./run_all.sh          # All tests sequentially

# Individual test
cd worker && cargo test keystore_client::tests::test_decrypt_secrets
```

### Development Environment

```bash
# Start infrastructure
cd coordinator && docker-compose up -d

# Start coordinator
cd coordinator && cargo run

# Start worker
cd worker && cargo run

# Start keystore
cd keystore-worker && cargo run

# Start dashboard
cd dashboard && npm run dev

# Check health
curl http://localhost:8080/health  # Coordinator
curl http://localhost:8081/health  # Keystore
curl http://localhost:3000          # Dashboard
```

### Debugging

```bash
# Check PostgreSQL
docker exec -it offchainvm-postgres psql -U postgres -d offchainvm
\dt                    # List tables
SELECT * FROM jobs ORDER BY created_at DESC LIMIT 10;
SELECT * FROM wasm_cache ORDER BY last_accessed DESC LIMIT 10;

# Check Redis
docker exec -it offchainvm-redis redis-cli
KEYS *                 # List all keys
LLEN task_queue        # Queue length
HGETALL lock:compile:* # Check locks

# Check WASM cache
ls -lh /tmp/offchainvm/wasm/

# Monitor logs
docker-compose logs -f coordinator
tail -f worker.log
```

---

## 🧪 Integration Testing as Verification

Tests verify **security properties**, not just functionality.

### Race Condition Tests (`tests/job_workflow.sh`)

**What it verifies**: Job atomicity prevents duplicate work

```bash
# Simulates multiple workers claiming same job
# Expected: Only one worker succeeds, others get empty or conflict
# Critical property: UNIQUE constraint on (request_id, data_id, job_type)
```

**Why it matters**: Without atomicity, two workers could compile same WASM simultaneously (waste) or execute same job twice (double billing).

**Code under test**: `coordinator/src/handlers/jobs.rs:claim_jobs_for_request`

### Compilation Tests (`tests/compilation.sh`)

**What it verifies**: Docker sandboxing prevents malicious builds

```bash
# Compiles real GitHub repo in isolated container
# Expected: WASM magic number present, valid module
# Container has: --network=none (no internet access)
```

**Why it matters**: Compilation code is untrusted (from GitHub). Isolation prevents supply chain attacks.

**Code under test**: `worker/src/compiler.rs`

### E2E Tests (`tests/e2e.sh`)

**What it verifies**: Full capability flow

```bash
# 1. Contract: request_execution (payment, validation)
# 2. Worker: poll, claim, compile, execute
# 3. Contract: resolve_execution (metrics, refund)
# 4. Contract: callback to requester
# Expected: Result matches, refund correct, events emitted
```

**Why it matters**: Verifies contract ↔ worker ↔ keystore integration works end-to-end with real NEAR testnet.

**Code under test**: Entire system

---

## 🚀 Phase 2/3 Vision: Production TEE Integration

### Current State (Phase 1 - MVP)

**What works today**:
- Capability-based architecture (access conditions, secrets, resource limits)
- Atomic job coordination (race-free execution)
- Real resource metering (fuel tracking, dynamic pricing)
- Client-side encryption (dashboard → contract)
- Access control validation (keystore checks NEAR state)

**What is simulated**:
- TEE attestation (`attestation_type: 'none'`)
- XOR encryption (insecure, placeholder for ChaCha20-Poly1305)
- Trust in worker honesty (no hardware enforcement)

**Acceptable for**:
- Development and testing
- Closed worker sets (you run your own worker)
- Non-critical computations
- Proof-of-concept demonstrations

### Phase 2: Hardware TEE (Phala Network / Intel SGX / AMD SEV)

**Goal**: Replace simulated attestation with real TEE hardware.

**Changes Required**:

1. **Keystore** (`keystore-worker/src/keystore_service.py:verify_attestation`):
   ```python
   # Current: if attestation_type == 'none': return True
   # Phase 2: Verify SGX quote or SEV report
   elif attestation_type == 'sgx':
       quote = attestation['quote']
       mrenclave = expected_measurements['mrenclave']
       return verify_sgx_quote(quote, mrenclave)  # Intel SGX SDK
   ```

2. **Worker** (`worker/src/keystore_client.rs:generate_attestation`):
   ```rust
   // Current: generates dummy attestation
   // Phase 2: Request hardware attestation
   pub fn generate_attestation(mode: AttestationMode) -> Attestation {
       match mode {
           AttestationMode::Sgx => {
               let quote = sgx_quote_create();  // Intel SGX DCAP
               Attestation { attestation_type: "sgx", quote, .. }
           }
       }
   }
   ```

3. **Encryption** (keystore-worker/encrypt_secrets.py):
   ```python
   # Current: XOR with master secret (insecure!)
   # Phase 2: ChaCha20-Poly1305 with ECDH key exchange
   def encrypt_secrets(secrets_json: str, recipient_pubkey: bytes) -> bytes:
       ephemeral_keypair = X25519.generate()
       shared_secret = ephemeral_keypair.exchange(recipient_pubkey)
       chacha = ChaCha20Poly1305(derive_key(shared_secret))
       ciphertext = chacha.encrypt(nonce, secrets_json.encode(), aad)
       return ephemeral_keypair.public + ciphertext
   ```

**Files Ready for TEE**:
- ✅ `worker/src/keystore_client.rs` - Attestation generation interface
- ✅ `keystore-worker/src/keystore_service.py` - Attestation verification dispatch
- ✅ `contract/src/secrets.rs` - Encrypted storage (encryption-agnostic)
- ❌ `keystore-worker/encrypt_secrets.py` - Needs ChaCha20-Poly1305 rewrite
- ❌ `worker/src/executor/` - Needs sealed storage for keys

**Timeline**: 2-4 weeks with SGX hardware/SDK access.

### Phase 3: Browser WASM TEE Nodes

**Goal**: Browser becomes distributed OutLayer worker.

**Architecture**:
```
Browser Tab
    │
    ├─> IndexedDB (sealed storage for worker keys)
    ├─> WebCrypto (attestation key generation)
    ├─> WebAssembly (WASM execution sandbox)
    │
    └─> OutLayer Coordinator (HTTP API)
            │
            └─> NEAR Contract (capability verification)
```

**Key Patterns**:

1. **Function Call Access Keys as Worker Capabilities**:
   ```javascript
   // Browser generates key pair
   const workerKey = KeyPair.fromRandom('ed25519');

   // Request capability from user
   await wallet.addFunctionCallAccessKey({
       publicKey: workerKey.getPublicKey(),
       contractId: 'outlayer.near',
       methodNames: ['submit_execution_result'],
       allowance: NEAR.toUnits('0.25')  // Gas budget
   });

   // Store in IndexedDB (sealed storage)
   await indexedDB.put('worker-key', workerKey.toString());
   ```

2. **Long-Polling for Tasks**:
   ```javascript
   // Browser polls coordinator (like current worker)
   async function pollTasks() {
       const response = await fetch(`${COORDINATOR_API}/tasks/poll?timeout=60`, {
           headers: { 'Authorization': `Bearer ${workerToken}` }
       });
       const task = await response.json();
       if (task) {
           await executeTask(task);
       }
   }
   ```

3. **WASM Execution with Gas Metering**:
   ```javascript
   // Instantiate WASM with fuel tracking
   const wasmBytes = await fetch(task.wasm_url).then(r => r.arrayBuffer());
   const gasTracker = new GasTracker({ maxGas: task.max_gas });

   const instance = await WebAssembly.instantiate(wasmBytes, {
       env: {
           // Meter every operation
           memory_read: gasTracker.meter((...) => { ... }, 1000),
           // Capability-based network access
           http_request: gasTracker.meter(async (url) => {
               if (!task.capabilities.network.includes(url)) {
                   throw new Error('No capability for ' + url);
               }
               return await fetch(url);
           }, 10000)
       }
   });

   const result = await instance.exports.execute(task.args);
   const gasUsed = gasTracker.used();
   ```

4. **State Attestation via Contract**:
   ```javascript
   // Pre-execution attestation
   await contract.attest_execution_start({
       execution_id: task.id,
       wasm_hash: sha256(wasmBytes),
       worker_state_root: await getIndexedDBRoot(),
       timestamp: Date.now()
   });

   // Execute...

   // Post-execution attestation
   await contract.attest_execution_complete({
       execution_id: task.id,
       result_hash: sha256(result),
       gas_used: gasUsed,
       proof: generateProof(execution)
   });
   ```

**Files Ready for Browser**:
- ✅ Architecture: Coordinator API is HTTP (browser-compatible)
- ✅ Authentication: Bearer tokens (can be stored in IndexedDB)
- ✅ Task model: Job-based workflow already async
- ❌ Compilation: Browser cannot compile (must download pre-compiled WASM)
- ❌ Attestation: Need WebCrypto-based proof generation

**Timeline**: 4-8 weeks after Phase 2 completes.

---

## 📍 Quick Reference

### Port Mapping

| Service       | Port  | Access                  |
|---------------|-------|-------------------------|
| Coordinator   | 8080  | Public (HTTP API)       |
| Keystore      | 8081  | Internal only (proxied) |
| Dashboard     | 3000  | Public (browser)        |
| PostgreSQL    | 5432  | Internal only           |
| Redis         | 6379  | Internal only           |

### Key File Locations

| Pattern                | Location                                    |
|------------------------|---------------------------------------------|
| Access conditions      | `contract/src/secrets.rs`                   |
| Job atomicity          | `coordinator/src/handlers/jobs.rs`          |
| Fuel metering          | `worker/src/executor/wasi_p1.rs`, `wasi_p2.rs` |
| WASI env injection     | `worker/src/executor/wasi_env.rs`           |
| Keystore verification  | `keystore-worker/src/keystore_service.py`   |
| TEE attestation        | `worker/src/keystore_client.rs`             |
| Client encryption      | `dashboard/lib/secrets.ts`                  |

### Common Pitfalls

| Problem                          | Solution                                      |
|----------------------------------|-----------------------------------------------|
| sqlx compile error               | Run `cargo sqlx prepare` after schema changes |
| Keystore unreachable from Docker | Use `host.docker.internal:8081` on Mac/Windows |
| Worker can't access Docker       | Add user to `docker` group on Linux           |
| Contract deploy fails            | Check `cargo-near` installed, Rust 1.85.0     |
| WASM execution fails             | Check WASI imports match template exactly     |

### Environment Setup Checklist

```bash
# 1. Prerequisites
rustup target add wasm32-wasip1 wasm32-wasip2
cargo install cargo-near
npm install -g npm@latest

# 2. Infrastructure
cd coordinator && docker-compose up -d
sqlx migrate run

# 3. Contract
cd contract
cargo near build
near contract deploy outlayer.testnet use-file target/near/outlayer_contract.wasm ...

# 4. Coordinator
cd coordinator
cp .env.example .env
# Edit: DATABASE_URL, REDIS_URL, REQUIRE_AUTH=false
cargo run

# 5. Worker
cd worker
cp .env.example .env
# Edit: API_BASE_URL, NEAR_RPC_URL, OPERATOR_PRIVATE_KEY
cargo run

# 6. Keystore (optional)
cd keystore-worker
cargo run

# 7. Dashboard
cd dashboard
cp .env.example .env.local
npm run dev
```

---

## Documentation Chapters

Technical documentation covering completed work (Phases 1-2) and strategic vision (Phases 3-6) is available in `md-claude-chapters/`.

### Completed Phases

**[Chapter 1: RPC Throttling - Infrastructure Protection](md-claude-chapters/01-rpc-throttling.md)** (Complete)
- Phase 1: Token bucket algorithm, rate limit profiles (5 rps anonymous / 20 rps keyed)
- Production ready coordinator middleware with automatic retry client
- ~500 lines distilling Phase 1 completion work

**[Chapter 2: Linux/WASM Integration](md-claude-chapters/02-linux-wasm-integration.md)** (Complete)
- Phase 2: Three-layer execution model (ContractSimulator → LinuxExecutor → Workers)
- Demo mode functional with NEAR syscall mapping (400-499)
- ~800 lines explaining native WASM kernel (not x86 emulation) and NOMMU architecture

### Strategic Vision (Phases 3-6)

**[Chapter 3: Multi-Layer Roadmap](md-claude-chapters/03-multi-layer-roadmap.md)** (Strategic Plan)
- Phases 3-6: QuickJS (2-3 weeks) → Frozen Realms (2-3 weeks) → Production Linux (3-4 weeks) → Applications (4-6 weeks)
- Task breakdowns with implementation steps, code examples, go/no-go decision points
- ~1200 lines of strategic roadmap with technical depth

**[Chapter 6: 4-Layer Architecture Deep Dive](md-claude-chapters/06-4-layer-architecture.md)** (Technical Analysis)
- L1→L2→L3→L4 stack: Host WASM Runtime → Guest OS → Guest Runtime → Guest Code
- Explains security model, I/O Trombone problem (fundamental performance trade-off), competitive positioning
- ~1500 lines - most complex chapter, explains why multi-layer architecture enables capabilities impossible elsewhere
- Warning: Budget 30-45 minutes for careful reading

**[Chapter 7: Daring Applications](md-claude-chapters/07-daring-applications.md)** (Market Vision)
- Three applications: AI Trading Agents, Deterministic Plugin Systems, Stateful Multi-Process Edge Computing
- Each includes competitive analysis explaining why alternatives cannot provide these capabilities
- ~1000+ lines with complete implementation examples

### Reference Documentation

**[Chapter 4: IIFE Bundling](md-claude-chapters/04-iife-bundling.md)** (Reference - Not Yet Implemented)
- Zero-config browser distribution (drop-in `<script>` tag usage)
- 5-phase implementation roadmap, tsup configuration patterns
- ~600 lines - note: Linux kernel (~24 MB) requires lazy loading strategy

**[Chapter 5: Performance Benchmarking](md-claude-chapters/05-performance-benchmarking.md)** (Methodology)
- Benchmark framework, test scenarios, comparison matrix
- Performance targets: Direct mode (<10ms), Linux mode (<50ms cold start)
- ~700 lines - note: Targets are projections awaiting production Linux kernel implementation

### Chapter Index

**Start here**: [md-claude-chapters/README.md](md-claude-chapters/README.md) - Includes reading paths for developers, architects, and decision makers, plus detailed explanations of key concepts (NOMMU, I/O Trombone, Frozen Realms) where concepts are subtle.

---

## 🎓 Learning Path for New Contributors

**If you are new to OutLayer**, read in this order:

1. **This file (CLAUDE.md)** - Architectural vision (you are here)
2. **contract/README.md** - Smart contract API reference
3. **wasi-examples/WASI_TUTORIAL.md** - How to write WASM for OutLayer
4. **tests/job_workflow.sh** - Race condition verification (shows atomicity in action)
5. **coordinator/src/handlers/jobs.rs** - Job claim logic (core coordination pattern)
6. **worker/src/main.rs** - Main worker loop (shows full execution flow)

**If you want to implement a feature**, ask yourself:
1. Does this change security boundaries? (If yes, discuss with human first)
2. Which component owns this responsibility? (Contract, Coordinator, Worker, Keystore?)
3. How does this interact with capabilities? (Access keys, secrets, resource limits)
4. What tests verify this works? (Unit, integration, E2E)

**If you encounter resistance** (e.g., "this seems over-engineered"):
- Remember: This is a **capability-based system**, not a simple API
- Every abstraction serves a security boundary
- Shortcuts may break TEE integration path
- Ask human for clarification if unsure

---

**Current Date**: 2025-11-05
**Version**: Phase 1 (MVP without hardware TEE)
**Status**: All components operational, ready for Phase 2 TEE integration

**Next Milestone**: Replace simulated attestation with Intel SGX or AMD SEV verification in keystore.
