# NEAR OutLayer

**"OutLayer execution for on-chain contracts"**

## Executive Summary

**NEAR OutLayer** is a verifiable off-chain computation platform that enables any NEAR smart contract to execute arbitrary untrusted code off-chain using NEAR Protocol's yield/resume mechanism.

Just as offshore zones provide efficient environments for operations while maintaining regulatory compliance, **NEAR OutLayer** provides an efficient execution environment for heavy computation while keeping security guarantees and final settlement on NEAR L1.

This creates a secure, scalable infrastructure for smart contracts to break free from gas limitations while maintaining cryptographic security guarantees through TEE attestation.

### Positioning

**The Offshore Jurisdiction for Smart Contract Computation**

**What NEAR OutLayer is:**
- **Computational OutLayer Zone**: Move expensive operations off-chain, just like moving assets offshore
- **Smart Contract Co-Processor**: Handles compute-heavy operations that are impractical on-chain
- **Verifiable Execution Service**: TEE-attested execution provides cryptographic proof of correctness
- **Developer Infrastructure**: Like AWS Lambda, but for smart contracts with blockchain guarantees

**What NEAR OutLayer is NOT:**
- **Not an L2**: No separate consensus, no new chain, no bridging complexity
- **Not an Oracle**: Doesn't fetch external data, executes arbitrary user code
- **Not a Sidechain**: Lives entirely off-chain, results return to L1
- **Not Traditional Cloud**: Execution is trustless, verifiable, and crypto-economically secured

**The OutLayer Metaphor:**
```
Financial Offshore → Move assets for efficiency, keep ownership
Computational OutLayer → Move computation for efficiency, keep security

Traditional Offshore:          NEAR OutLayer:
✅ Lower costs                  ✅ Lower gas costs (100x cheaper)
✅ Efficiency                   ✅ Unlimited computation power
✅ Privacy                      ✅ Repo-based secrets with access control
✅ Optimization                 ✅ Optimize without compromise
✅ Still yours                  ✅ Results return to your contract
```

**Mental Model**: Think of NEAR OutLayer as **"Offshore jurisdiction for computation"** - move heavy lifting off-chain for efficiency, but funds and final settlement stay on NEAR L1.

---

## Architecture Overview

### Core Flow

```
User Contract → NEAR OutLayer Contract (yield) → Worker Network → Resume with Results → User Contract
```

### Components

1. **NEAR OutLayer Smart Contract** (`offchainvm.near`)
   - Entry point for all computation requests
   - Payment validation and escrow
   - Yield/resume orchestration
   - Timeout and failure handling

2. **Worker Network** (Distributed Off-Chain Executors)
   - Event monitoring and task discovery
   - WASM compilation/caching
   - Sandboxed code execution (WASI runtime)
   - Multi-worker coordination
   - Result signing and resume

3. **Client Smart Contracts** (`client.near`)
   - Initiate execution requests
   - Provide payment and parameters
   - Receive computation results

---

## Technical Design

### 1. Execution Request Structure

```rust
pub struct ExecutionRequest {
    // Unique identifier
    request_id: u64,
    data_id: CryptoHash,

    // Client info
    sender_id: AccountId,
    callback_method: String,

    // Code source (one of):
    code_source: CodeSource {
        GitRepo { url: String, commit_hash: String },
        WasmUrl { url: String, checksum: String },
    },

    // Execution parameters
    secrets_ref: Option<SecretsReference>,  // Reference to repo-based secrets
    input_data: Option<String>,
    resource_limits: ResourceLimits {
        max_instructions: u64,
        max_memory_mb: u64,
        max_execution_seconds: u64,
    },

    // Economics
    payment: Balance,
    timestamp: u64,
}
```

### 2. Worker Execution Environment

**Three-Layer Security Model:**

#### Layer 1: Process Isolation
- Each execution runs in a separate OS process
- Process killed on timeout (hard kill after max_execution_seconds)
- No shared memory between executions
- Process memory cleared after completion

#### Layer 2: WASI Sandboxing with wasmi
- **Memory isolation**: Pre-allocated memory limits (max_memory_mb enforced)
- **Instruction metering**: Count every WASM instruction, abort after max_cpu_cycles
- **OOM detection**: Catch allocation failures, return error to contract
- **No network access**: WASI capabilities exclude networking
- **No filesystem access**: Except read-only access to WASM binary itself
- **Limited syscalls**: Only WASI ABI functions (environ_get, fd_write for stdout/stderr, clock_time_get)

#### Layer 3: TEE Attestation (Intel SGX / AWS Nitro Enclaves)
- **Cryptographic proof**: Every execution produces attestation report
- **Code verification**: TEE measures exact WASM bytecode executed
- **Secret isolation**: Decrypted secrets never leave enclave memory
- **Tamper evidence**: Any modification to worker breaks attestation
- **Remote verification**: Clients can verify attestation before trusting results

**Resource Enforcement:**
```rust
// Instruction metering
let mut executor = wasmi::Executor::new()
    .with_max_instructions(max_cpu_cycles);

// Memory limits
let memory_limit = max_memory_mb * 1024 * 1024;
let memory = Memory::new(memory_limit)?;

// Timeout enforcement
tokio::select! {
    result = executor.execute() => result,
    _ = tokio::time::sleep(Duration::from_secs(max_execution_seconds)) => {
        // Kill process group
        kill_process_tree(child_pid);
        return ExecutionResult::Timeout;
    }
}
```

**Security Guarantees:**
- ✅ Cannot escape sandbox (memory-safe Rust + minimal WASI)
- ✅ Cannot run forever (instruction counting + timeout)
- ✅ Cannot exhaust memory (pre-allocated limits + OOM detection)
- ✅ Cannot access network (WASI capabilities restricted)
- ✅ Cannot persist state (ephemeral process)
- ✅ Execution is verifiable (TEE attestation)
- ✅ Secrets are protected (TEE memory encryption)

### 3. Secret Management with TEE

**TEE-Based Secret Handling:**

#### Key Generation (One-time Setup)
```
1. TEE enclave boots up
2. Enclave generates Ed25519 keypair INSIDE TEE
3. Private key NEVER leaves TEE memory (encrypted at rest by CPU)
4. Public key published on-chain via Offshore contract
5. TEE produces attestation report proving:
   - Exact worker code hash
   - Public key ownership
   - Hardware security guarantees
```

#### Repo-Based Secrets (New System)
```
Secret Storage (One-time setup):
1. User stores secrets in contract: store_secrets(repo, branch, profile, encrypted_data, access_rules)
2. Secrets encrypted client-side with keystore's repo-specific public key
3. Stored on-chain with access conditions (AllowAll, Whitelist, NEAR balance, FT/NFT ownership, Logic)
4. Indexed by user for O(1) lookups

Execution (Automated):
1. Contract stores secrets_ref: {profile: "default", account_id: "alice.near"}
2. Worker fetches secrets from contract via get_secrets()
3. Keystore validates access conditions (NEAR/FT/NFT balance checks via RPC)
4. If authorized, keystore decrypts secrets using repo-specific keypair
5. Secrets injected as WASI environment variables (std::env::var())
```

#### Secret Decryption (Inside TEE)
```
TEE Enclave:
1. Receives secrets_ref from execution request
2. Fetches encrypted secrets from contract
3. Validates access conditions (balance checks, whitelist, regex patterns)
4. Decrypts with repo-specific keypair (derived via HMAC-SHA256)
5. Decrypted secrets exposed as env vars to WASM runtime
6. WASM execution happens inside TEE
7. After execution: enclave memory cleared (CPU-level encryption)
```

**Security Properties:**
- ✅ Secrets stored once, used everywhere (no inline passing)
- ✅ Access control enforced by keystore (NEAR/FT/NFT balance, whitelist, regex)
- ✅ Master secret never leaves TEE (repo-specific keys derived via HMAC)
- ✅ Decrypted secrets never touch host OS memory
- ✅ Worker operator cannot extract secrets (hardware-enforced)
- ✅ Remote attestation proves correct enclave code
- ✅ Storage costs refunded on deletion
- ✅ Per-branch secrets support (main, dev, staging profiles)

**Attestation Verification Flow:**
```
Client → "Who will execute my code?"
Offshore Contract → Returns: {
  worker_public_key: "ed25519:...",
  attestation_report: "base64_encoded_report",
  enclave_measurements: {
    code_hash: "sha256 of worker binary",
    cpu_svn: "security version",
    ...
  }
}
Client → Verifies attestation with Intel/AWS APIs
Client → "OK, this is legit TEE running correct code"
Client → Proceeds with execute() call
```

**Trust Model:**
- **Before TEE**: Trust the operator (like trusting AWS)
- **With TEE**: Trust Intel/AWS hardware + open-source worker code (verifiable)
- **No need to trust**: Operator, infrastructure, network, OS

### 4. Multi-Worker Coordination

**Task Distribution:**
```
Shared Queue (Redis/PostgreSQL)
    ↑
Multiple Worker Processes
    ↑
Event Monitor (single instance per network)
```

**Concurrency Model:**
- One event monitor per blockchain network
- Configurable worker pool size per physical server
- Task locking to prevent double-execution
- Heartbeat mechanism for stale task detection
- Priority queue based on payment amount

### 5. Payment and Economics

**Fee Structure:**
```
Base Fee + (CPU × CPU_Rate) + (Memory × Memory_Rate) + (Time × Time_Rate)
```

**Payment Flow:**
1. Client attaches NEAR tokens with `execute()` call
2. Contract validates minimum payment
3. Funds escrowed during execution
4. On success: Worker paid, excess refunded to client
5. On timeout/failure: Worker paid for attempted execution (anti-DoS)

**No Refunds Policy:**
- Prevents resource exhaustion attacks
- Workers compensated for CPU/memory consumption
- Failed executions still cost resources
- Clients incentivized to test code before production

### 6. Failure Handling

**Timeout Mechanism:**
```rust
if block_timestamp > request.timestamp + request.max_execution_seconds {
    // Worker sends timeout response
    return ExecutionResult::Timeout {
        partial_logs: String,
        resources_consumed: ResourceMetrics,
    }
}
```

**Error Categories:**
1. **Compilation Error**: Invalid WASM, missing dependencies
2. **Runtime Error**: Panic, out-of-bounds, segfault
3. **Timeout**: Exceeded max_execution_seconds
4. **Resource Limit**: OOM, CPU limit exceeded
5. **Worker Failure**: Process crash, network issue

**Resolution:**
- All errors result in `resolve_execution()` call with error details
- Client contract receives structured error information
- Failed tasks can be retried with adjusted parameters
- Worker reputation system (future enhancement)

---

## Security Analysis

### Attack Vectors and Mitigations

| Attack | Mitigation |
|--------|-----------|
| **Malicious WASM code** | WASI sandboxing, resource limits, no network access |
| **Infinite loops** | Instruction metering, hard timeouts |
| **Memory exhaustion** | Pre-allocated limits, OOM detection |
| **VM escape** | wasmi is memory-safe (Rust), WASI API is minimal |
| **Secret theft** | Encrypted transport, worker memory clearing, TEE (future) |
| **DoS attacks** | No refunds, rate limiting, payment requirements |
| **Result manipulation** | Deterministic execution, multi-worker verification (future) |
| **Replay attacks** | Unique data_id per request, nonce tracking |

### Trust Assumptions

**With TEE Integration (Production Model):**
- ✅ **Trust Intel/AWS/ARM hardware**: Industry-standard TEE providers
- ✅ **Trust open-source worker code**: Auditable, reproducible builds
- ✅ **Trust cryptography**: Ed25519, AES-GCM for secrets
- ❌ **No need to trust operator**: TEE attestation proves correct execution
- ❌ **No need to trust infrastructure**: OS compromise doesn't leak secrets
- ❌ **No need to trust network**: Encrypted secrets, signed results

**Trusted Computing Base (TCB):**
1. TEE hardware (Intel SGX / AWS Nitro Enclave)
2. Worker binary (open-source, verified by attestation)
3. WASM runtime (wasmi - memory-safe Rust)
4. User's own WASM code (they control this)

**What operator CANNOT do even if malicious:**
- Extract decrypted secrets from TEE
- Modify execution results without detection
- Run different code than attested
- Access user WASM execution memory
- Forge attestation reports

**What operator CAN do:**
- Refuse to execute certain code (censorship)
- Shut down infrastructure (availability attack)
- See encrypted secrets and metadata (but not decrypt)

**Mitigation for operator risks:**
- Multiple independent operators (client chooses)
- Slashing for availability failures (future)
- Reputation system based on uptime (future)

---

## Comparison with Existing Project

### Current: NEAR Intents Integration
- **Specific use case**: Token swaps via NEAR Intents API
- **Centralized logic**: Hardcoded swap execution
- **Limited scope**: Only supports pre-defined operations
- **Trust model**: Single operator with private keys

### Proposed: NEAR OutLayer Platform
- **General purpose**: Any computation expressible in WASM
- **Arbitrary code**: Users provide their own logic
- **Extensible**: Supports unlimited use cases
- **Sandboxed**: Untrusted code execution with TEE security guarantees

### Code Reuse from Current Project

✅ **Keep:**
- Yield/resume pattern implementation
- Event monitoring architecture
- Worker coordination logic
- Payment validation
- Timeout handling

❌ **Remove:**
- NEAR Intents API integration
- Token swap specific logic
- Hardcoded business logic

🆕 **Add:**
- WASM compilation pipeline (sandboxed)
- WASI runtime integration with wasmi
- TEE integration (Intel SGX / AWS Nitro)
- Secret encryption/decryption in TEE
- Resource metering (instructions, memory, time)
- Instruction counting for loop protection
- OOM detection and handling
- Process timeout enforcement
- Arbitrary code execution engine
- Compilation result caching
- Asynchronous compilation handling

---

## WASM Compilation Pipeline

### Design Philosophy
**Transparency over convenience**: Users should be able to verify exactly what code is running, even if it means slightly more complex workflow.

### Compilation Process

#### Step 1: Source Code Compilation (Sandboxed)
```
GitHub Repo + Commit Hash
    ↓
Worker downloads specific commit
    ↓
Compilation in isolated container (no network, limited resources)
    ↓
Rust → WASM (or other languages via wasm-pack, emscripten, etc.)
    ↓
WASM binary + metadata
```

**Sandboxing compilation:**
- Separate Docker container per compilation
- No network access during build
- CPU and memory limits
- Timeout (e.g., 5 minutes max)
- Read-only filesystem except for build output

**Compilation security:**
```rust
// Pseudo-code for sandboxed compilation
fn compile_from_source(repo_url: String, commit_hash: String) -> Result<WasmBinary> {
    // Clone repo in isolated container
    let container = DockerContainer::new()
        .no_network()
        .memory_limit(2_GB)
        .cpu_limit(2_cores)
        .timeout(300_seconds);

    container.exec(format!(
        "git clone {repo_url} /src && cd /src && git checkout {commit_hash}"
    ))?;

    // Build WASM
    container.exec("cargo build --target wasm32-wasi --release")?;

    // Extract WASM binary
    let wasm_bytes = container.read_file("/src/target/wasm32-wasi/release/*.wasm")?;

    // Compute checksum
    let checksum = sha256(&wasm_bytes);

    Ok(WasmBinary {
        bytes: wasm_bytes,
        checksum,
        metadata: CompilationMetadata {
            repo_url,
            commit_hash,
            compiled_at: now(),
            compiler_version: rust_version(),
        }
    })
}
```

#### Step 2: WASM Caching
```
WASM Binary
    ↓
Content-addressed storage (checksum as key)
    ↓
Local filesystem: /var/offshore/wasm_cache/{sha256_checksum}.wasm
    ↓
Metadata: /var/offshore/wasm_cache/{sha256_checksum}.json
```

**Cache structure:**
```json
{
  "checksum": "sha256:abc123...",
  "source": {
    "type": "github",
    "repo": "https://github.com/user/project",
    "commit": "abc123",
    "compiled_at": "2025-01-15T10:30:00Z"
  },
  "size_bytes": 1048576,
  "first_seen": "2025-01-15T10:30:00Z",
  "last_used": "2025-01-15T12:45:00Z",
  "execution_count": 42
}
```

#### Step 3: Asynchronous Compilation Handling

**Problem**: Compilation takes 1-5 minutes, but blockchain expects fast responses.

**Solution**: Two-phase execution model

##### First Request (WASM not cached):
```
Client calls execute() → Event emitted
    ↓
Worker sees event, checks cache → MISS
    ↓
Worker responds immediately: resolve_execution(
  result = CompilationInProgress {
    estimated_time: 180_seconds,
    retry_after: block_height + 20
  }
)
    ↓
Client contract receives CompilationInProgress
    ↓
Contract stores state, refunds user (or keeps in escrow)
    ↓
User sees: "Compiling your code, please retry in 3 minutes"
```

##### Background Compilation:
```
Worker continues compilation in background
    ↓
WASM binary ready, stored in cache
    ↓
(No automatic retry - client must call execute() again)
```

##### Second Request (WASM cached):
```
Client calls execute() again → Event emitted
    ↓
Worker sees event, checks cache → HIT
    ↓
Worker executes WASM immediately
    ↓
Results returned within seconds
```

**Contract interface for compilation status:**
```rust
pub enum ExecutionResult {
    Success {
        return_value: Vec<u8>,
        resources_used: ResourceMetrics,
        logs: String,
        attestation: AttestationReport, // TEE proof
    },

    CompilationInProgress {
        estimated_seconds: u64,
        retry_after_blocks: u64,
        cache_key: String, // So client knows what to reference
    },

    CompilationFailed {
        error_message: String,
        build_logs: String,
    },

    // ... other error types
}
```

**User experience:**
```
First call:  "Compiling your code... Estimated time: 3 minutes"
             [Progress bar or loading indicator]
             [Automatically retry every 30 seconds, or manual retry button]

Cache hit:   "Executing..." → Results in 2-5 seconds
```

### GitHub-Only Policy (Security)

**Why only GitHub:**
- ✅ Reproducible builds (commit hashes are immutable)
- ✅ Audit trail (anyone can inspect source code)
- ✅ Community review (malicious code can be spotted)
- ✅ Trust model (if you trust GitHub, you can verify code)
- ❌ No arbitrary URLs (could serve different code each time)
- ❌ No private repos (code must be auditable)

**Allowed:**
- `https://github.com/user/repo` + commit hash
- `https://github.com/user/repo` + tag (resolved to commit hash)

**Not allowed:**
- `https://example.com/my-wasm.wasm` (not verifiable)
- `https://github.com/user/private-repo` (not auditable)
- `git@github.com:user/repo.git` (must use HTTPS)

**Verification flow:**
```rust
fn validate_code_source(source: &CodeSource) -> Result<()> {
    match source {
        CodeSource::GitHub { repo, commit } => {
            // Verify repo is public
            assert!(is_public_repo(repo)?, "Repository must be public");

            // Verify commit exists
            assert!(commit_exists(repo, commit)?, "Commit not found");

            // Verify it's a valid git commit hash (40 hex chars)
            assert!(is_valid_commit_hash(commit), "Invalid commit hash");

            Ok(())
        }
        _ => Err("Only GitHub public repositories are supported")
    }
}
```

### Cache Eviction Policy

**When to evict:**
- LRU (Least Recently Used) when disk space > 80% full
- Never used in last 30 days
- Manual eviction by operator for malicious code

**Cache retention:**
- Popular WASM (>100 executions): Keep indefinitely
- Moderate use (10-100 executions): Keep 90 days
- Rare use (<10 executions): Keep 30 days

---

## Use Cases Enabled by NEAR OutLayer

### 1. DeFi Applications
- **Complex trading strategies**: Multi-DEX arbitrage, portfolio rebalancing
- **AI-powered trading**: ML models for price prediction, risk analysis
- **Credit scoring**: Off-chain computation for DeFi lending protocols
- **Options pricing**: Black-Scholes and exotic derivatives calculations

### 2. Gaming
- **Procedural generation**: Map generation, loot tables
- **Physics simulations**: Complex game mechanics
- **AI opponents**: Pathfinding, behavior trees
- **Game state validation**: Anti-cheat verification

### 3. NFTs and Creative
- **Generative art**: On-demand NFT generation
- **Image processing**: Filters, transformations, AI enhancement
- **Music synthesis**: Algorithmic composition
- **3D rendering**: Preview generation for metaverse assets

### 4. Data and Analytics
- **On-chain analytics**: Complex queries over blockchain data
- **Data aggregation**: Multi-source data processing
- **Machine learning inference**: Model serving for smart contracts
- **Cryptographic operations**: ZK proof generation, heavy encryption

### 5. Enterprise
- **Document processing**: PDF generation, OCR, data extraction
- **Identity verification**: KYC/AML checks (with privacy)
- **Supply chain**: Route optimization, inventory management
- **IoT integration**: Sensor data processing, edge computation

### 6. Cross-Chain
- **Bridge verification**: Merkle proof validation
- **Multi-chain state queries**: Aggregate data from multiple chains
- **Interoperability protocols**: Message passing between ecosystems

---

## Ecosystem Impact

### For Developers
- **No more gas limits**: Complex logic moves off-chain
- **Familiar languages**: Any language that compiles to WASM (Rust, C++, AssemblyScript, Go)
- **Rapid iteration**: Update worker code without redeploying contracts
- **Cost efficiency**: Pay only for computation used

### For NEAR Protocol
- **Competitive advantage**: Features impossible on other L1s
- **Developer attraction**: Unlock new application categories
- **Gas efficiency**: Reduces on-chain computational load
- **Innovation catalyst**: Enables experimental features

### For Users
- **Better UX**: Complex operations feel instant (no multi-step flows)
- **Lower costs**: Off-chain computation is cheaper than gas
- **More features**: Applications can do things previously impossible
- **Transparency**: Execution is verifiable and auditable

---

## Roadmap

### Phase 1: TEE-Based MVP (4-5 months)
**Goal**: Production-ready system with TEE security from day one

**Smart Contract:**
- [ ] NEAR OutLayer smart contract with yield/resume pattern
- [ ] Payment validation and escrow
- [ ] Timeout and cancellation handling
- [ ] Event emission for workers
- [ ] Public key storage and attestation verification

**Worker (TEE-based):**
- [ ] AWS Nitro Enclaves OR Intel SGX integration
- [ ] Keypair generation inside TEE
- [ ] Remote attestation report generation
- [ ] Event monitoring (outside TEE)
- [ ] Task queue and coordination

**WASM Execution:**
- [ ] wasmi integration with instruction metering
- [ ] WASI runtime with minimal capabilities
- [ ] Memory limits and OOM detection
- [ ] Process timeout enforcement (kill after max_execution_seconds)
- [ ] Resource usage tracking

**Compilation Pipeline:**
- [ ] Sandboxed Docker compilation environment
- [ ] GitHub repo cloning and validation (public repos only)
- [ ] WASM binary caching (content-addressed storage)
- [ ] Asynchronous compilation with CompilationInProgress response
- [ ] Cache eviction policy (LRU)

**Secret Management:**
- [ ] Client-side secret encryption (Ed25519 public key)
- [ ] TEE-based secret decryption
- [ ] Environment variable injection into WASM
- [ ] Memory clearing after execution

**Testing & Validation:**
- [ ] End-to-end test suite
- [ ] Security audit (smart contract + worker)
- [ ] TEE attestation verification
- [ ] Load testing (multiple concurrent executions)

### Phase 2: Production Scaling (2-3 months)
**Goal**: Handle high throughput and multiple workers

- [ ] Multi-worker coordination (Redis/PostgreSQL task queue)
- [ ] Worker pool scaling (configurable pool size)
- [ ] Horizontal scaling support (multiple physical servers)
- [ ] Advanced monitoring (Prometheus, Grafana)
- [ ] SLA guarantees (99.9% uptime)
- [ ] Dynamic pricing based on resource usage
- [ ] Compilation result sharing (S3/IPFS for WASM binaries)

### Phase 3: Operator Decentralization (6+ months)
**Goal**: Permissionless worker marketplace

- [ ] Multi-operator support (clients choose worker by pubkey)
- [ ] Slashing for availability failures
- [ ] Payment splitting (protocol fee + worker fee)
- [ ] Reputation system based on uptime and correctness
- [ ] Governance for protocol parameters
- [ ] Dispute resolution mechanism

### Phase 4: Advanced Features (Ongoing)
- [ ] ZK proofs for succinctness (optional, for expensive verifications)
- [ ] GPU support via WebGPU (for ML inference)
- [ ] Precompiled WASM templates (popular libraries)
- [ ] CDN for WASM distribution (faster cold starts)
- [ ] Cross-chain support (other yield/resume compatible chains)

---

## Critical Design Questions & Decisions

### 1. ✅ Compilation from Source (Not Pre-compiled WASM)
**Decision**: Focus on GitHub source compilation for transparency

**Rationale**:
- 🔍 **Auditability**: Users can inspect source code before using
- 🔒 **Trust**: Reproducible builds from immutable commit hashes
- 🛡️ **Security**: Sandboxed compilation protects workers
- 📦 **Flexibility**: Support any Rust/C++/AssemblyScript project

**Mitigations for compilation risks**:
- Sandboxed Docker environment (no network, resource limits)
- Only public GitHub repos (no private or arbitrary URLs)
- Async compilation model (don't block on compile)
- Cache popular WASM binaries (subsequent calls are instant)

**Trade-off accepted**: First execution is slower (3-5 min compile time)

### 2. ✅ TEE from Phase 1 (Not "Future Work")
**Decision**: Launch with TEE integration from MVP

**Rationale**:
- 🔐 **Day-1 trustlessness**: No need to trust operator for secrets
- 🎯 **Market positioning**: Compete with AWS Lambda on security
- 📊 **Attestation proof**: Cryptographic guarantee of correct execution
- 🚀 **No migration pain**: Don't need to migrate users later

**Implementation**:
- Start with AWS Nitro Enclaves (easier than Intel SGX)
- Fall back to Intel SGX for bare-metal deployments
- Open-source worker code + reproducible builds

**Trade-off accepted**: Higher development complexity in Phase 1

### 3. ✅ No Reputation System (Trust-Required Model)
**Decision**: Require users to trust the operator, mitigate with TEE

**Rationale**:
- ⚡ **Simplicity**: No staking, slashing, or governance complexity
- 🎯 **Focus**: Solve execution problem first, decentralization later
- 🔒 **TEE is enough**: Attestation provides trustlessness without reputation
- 📈 **Iterate fast**: Launch MVP faster, add reputation in Phase 3

**Operator responsibilities**:
- Maintain uptime (users can switch operators if unreliable)
- Pay for infrastructure
- Provide honest execution (enforced by TEE)

**What operator CANNOT abuse** (even if malicious):
- Cannot steal secrets (TEE protection)
- Cannot forge results (attestation would fail)
- Cannot run different code (attestation measures exact binary)

**What operator CAN do**:
- Refuse to execute certain code (censorship)
- Shut down service (availability attack)
- Set high prices (market forces will create competitors)

**Phase 3 mitigation**: Multiple competing operators, users choose via public key

### 4. ✅ Gas Economics: Pay-Per-Use, No Refunds
**Decision**: Fixed base price + resource-based pricing, no refunds even on failure

**Pricing model**:
```
Total Cost = Base Fee + (Instructions × $0.000001) + (Memory_MB × $0.0001) + (Time_Sec × $0.01)

Example costs:
- Simple calculation (1M instructions, 10MB, 1sec): $0.01 + $0.000001 + $0.001 + $0.01 = ~$0.02
- ML inference (1B instructions, 500MB, 30sec): $0.01 + $1 + $0.05 + $0.30 = ~$1.36
- Long computation (100M inst, 100MB, 300sec): $0.01 + $0.10 + $0.01 + $3.00 = ~$3.11
```

**No refunds policy**:
- ✅ Protects workers from DoS (can't spam free compilations)
- ✅ Fair pricing (you pay for resources consumed, not success)
- ✅ Predictable costs (estimate before execution)

**Client protection**:
- View estimated cost before execution
- Set max_payment limit in execute() call
- Cancel stale requests after timeout

### 5. ✅ Async Compilation Model (Not "Instant Everything")
**Decision**: Return CompilationInProgress for cache misses, user retries

**Why not auto-retry**:
- 🚫 No recurring payments (worker can't charge user multiple times)
- 🚫 No callback hell (smart contracts can't easily wait 3 minutes)
- ✅ User control (user decides when to retry)
- ✅ Transparency (user knows compilation is happening)

**UX flow**:
```
User → execute(github.com/user/repo, commit_abc123)
      ↓
Contract → Emits event, charges compilation fee ($0.10)
      ↓
Worker → "Compiling... ETA 3 minutes"
      ↓
Contract → Returns CompilationInProgress { retry_after: 180 }
      ↓
User → (waits or retries periodically)
      ↓
User → execute(same repo/commit) [3 minutes later]
      ↓
Worker → Cache hit! Executes immediately
      ↓
Contract → Returns Success { result, attestation }
```

**Compilation fee**:
- Separate from execution fee
- Covers Docker container + CPU time
- Only charged once (cached results are free)

### 6. ✅ GitHub-Only Policy (No Arbitrary URLs)
**Decision**: Only accept public GitHub repos with commit hashes

**Why this restriction**:
- 🔍 **Verifiability**: Anyone can audit code at specific commit
- 🔒 **Immutability**: Commit hashes don't change
- 📊 **Transparency**: Community can review code
- 🚫 **No rug pulls**: Owner can't swap code after approval

**Rejected alternatives**:
- ❌ Arbitrary WASM URLs: Could serve different code to different users
- ❌ Private repos: Not auditable by third parties
- ❌ IPFS: No source code, only compiled binary (less transparent)

**Future consideration**: GitLab / Gitea support (same model, different host)

---

## Technical Specification

### Contract Interface

```rust
#[near_bindgen]
impl Contract {
    /// Main entry point for execution requests
    #[payable]
    pub fn request_execution(
        &mut self,
        code_source: CodeSource,
        resource_limits: Option<ResourceLimits>,
        input_data: Option<String>,
        secrets_ref: Option<SecretsReference>,  // Reference to repo-based secrets
        response_format: Option<ResponseFormat>,
    );

    /// Worker calls this to submit execution result
    pub fn resolve_execution(
        &mut self,
        request_id: u64,
        success: bool,
        output: Option<ExecutionOutput>,
        error: Option<String>,
        resources_used: ResourceMetrics,
        compilation_note: Option<String>,
    );

    /// Client can cancel stale requests (10 min timeout)
    pub fn cancel_stale_execution(&mut self, request_id: u64);

    // Secrets management
    #[payable]
    pub fn store_secrets(
        &mut self,
        repo: String,
        branch: Option<String>,
        profile: String,
        encrypted_secrets_base64: String,
        access: AccessCondition,
    );

    pub fn delete_secrets(&mut self, repo: String, branch: Option<String>, profile: String);
    pub fn get_secrets(&self, repo: String, branch: Option<String>, profile: String, owner: AccountId) -> Option<SecretProfileView>;
    pub fn list_user_secrets(&self, account_id: AccountId) -> Vec<UserSecretInfo>;

    /// View functions
    pub fn get_request(&self, request_id: u64) -> Option<ExecutionRequest>;
    pub fn get_pricing(&self) -> PricingConfig;
    pub fn get_stats(&self) -> ContractStats;
    pub fn estimate_execution_cost(&self, resource_limits: ResourceLimits) -> U128;
}

pub enum ExecutionResult {
    Success {
        return_value: Vec<u8>,
        resources_used: ResourceMetrics,
        logs: String,
        attestation: AttestationReport, // TEE proof of execution
    },
    CompilationInProgress {
        estimated_seconds: u64,
        retry_after_blocks: u64,
        cache_key: String,
    },
    CompilationFailed {
        error_message: String,
        build_logs: String,
    },
    Error {
        error_type: ErrorType,
        error_message: String,
        partial_logs: String,
    },
    Timeout {
        partial_logs: String,
        resources_used: ResourceMetrics,
    },
}

pub struct AttestationReport {
    pub report_data: Vec<u8>,        // Raw attestation from TEE
    pub enclave_hash: String,         // Hash of worker binary
    pub public_key: PublicKey,        // Key that signed results
    pub timestamp: u64,
    pub platform: TeePlatform,        // AWS_NITRO or INTEL_SGX
}
```

### Worker Interface

```rust
// WASM module must export this function
#[no_mangle]
pub extern "C" fn execute() -> *const u8 {
    // User code here
    // Returns JSON-encoded result
}

// Available WASI capabilities:
// - Environment variables (secrets)
// - stdout/stderr (logs)
// - Limited clock access
// - No filesystem, network, or raw syscalls
```

### Event Schema

```json
{
  "standard": "near_offshore",
  "version": "1.0.0",
  "event": "execution_request",
  "data": {
    "request_id": 12345,
    "data_id": "0x...",
    "sender_id": "client.near",
    "code_source": {
      "type": "GitHub",
      "repo": "https://github.com/user/project",
      "commit": "abc123def456",
      "build_target": "wasm32-wasip1"
    },
    "secrets_ref": {
      "profile": "production",
      "account_id": "alice.near"
    },
    "input_data": "{}",
    "resource_limits": {
      "max_instructions": 1000000000,
      "max_memory_mb": 128,
      "max_execution_seconds": 60
    },
    "response_format": "Text",
    "payment": "1000000000000000000000000",
    "timestamp": 1234567890
  }
}
```

---

## Business Model

### Revenue Streams
1. **Execution fees**: Per-request pricing based on resources
2. **Premium features**: Guaranteed SLA, priority execution
3. **Enterprise licenses**: Private worker deployments
4. **Marketplace fees**: Commission on worker marketplace (Phase 3)

### Cost Structure
- Worker infrastructure (compute, memory, storage)
- NEAR transaction fees (resume calls)
- Development and maintenance
- Security audits and insurance

### Competitive Pricing
- **AWS Lambda equivalent**: ~$0.0000166667 per GB-second
- **NEAR OutLayer target**: 50-70% of AWS pricing (cheaper due to no cloud markup)
- **NEAR gas equivalent**: 100x cheaper than pure on-chain computation

---

## Risk Assessment

### Technical Risks
| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|------------|
| WASM VM escape | Low | Critical | Use battle-tested wasmi, regular audits, WASI restrictions |
| Worker infrastructure failure | Medium | High | Multi-region deployment, failover |
| Secret leakage | Low | Critical | TEE integration from Phase 1, hardware-enforced isolation |
| DoS attacks | Medium | Medium | No refunds policy, payment requirements, rate limiting |
| Compilation attacks | Medium | High | Sandboxed Docker, no network during build, resource limits |
| Infinite loops in user code | High | Medium | Instruction metering, process timeout (hard kill) |
| Memory exhaustion | High | Medium | Pre-allocated limits, OOM detection |

### Business Risks
| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|------------|
| Low adoption | Medium | Critical | Developer education, partnerships |
| Regulatory scrutiny | Low | High | Legal compliance, KYC for workers (if needed) |
| Competitor launch | Medium | Medium | Fast iteration, ecosystem lock-in |
| NEAR protocol changes | Low | High | Close relationship with NEAR Foundation |

### Operational Risks
| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|------------|
| Key compromise | Low | Critical | TEE-based key generation, never leaves enclave |
| Worker misbehavior | Low | Low | TEE attestation prevents result tampering |
| Cost overruns | Medium | Medium | Dynamic pricing, monitoring, resource limits |

---

## Success Metrics

### Phase 1 (MVP)
- 10+ developers testing the platform
- 1,000+ successful executions
- <5% failure rate
- <30s average execution time

### Phase 2 (Production)
- 100+ active developer accounts
- 10,000+ daily executions
- 10+ production dApps integrated
- 99.9% uptime SLA

### Phase 3 (Decentralization)
- 50+ independent worker operators
- $1M+ monthly execution volume
- 5+ major protocols using NEAR OutLayer as core infrastructure
- Profitable unit economics

---

## Conclusion

**NEAR OutLayer** represents a paradigm shift in smart contract capabilities, enabling developers to build applications that were previously impossible on blockchain. By leveraging NEAR's unique yield/resume mechanism and combining it with TEE-attested off-chain computation, we create a platform that:

1. **Extends L1 capabilities** without protocol changes or new chains
2. **Maintains security guarantees** through TEE attestation and sandboxing
3. **Enables transparency** via GitHub-based compilation and open audits
4. **Enables new use cases** across DeFi, gaming, NFTs, AI/ML, and enterprise
5. **Creates a sustainable business** with clear revenue models and growth paths

### Key Differentiators

**vs. Traditional Cloud (AWS Lambda):**
- ✅ Blockchain-native (smart contracts can call directly)
- ✅ Cryptographic proof of execution (TEE attestation)
- ✅ Transparent code (GitHub-based, auditable)
- ✅ Crypto payments (NEAR tokens, not credit cards)

**vs. Oracles (Chainlink):**
- ✅ Executes arbitrary user code (not just data fetching)
- ✅ Supports complex computation (ML models, simulations)
- ✅ User-controlled logic (not operator-controlled)

**vs. L2s/Sidechains:**
- ✅ No new chain to secure
- ✅ No bridging complexity
- ✅ Results return directly to NEAR L1
- ✅ No separate consensus mechanism

### Why This Matters

Current blockchain limitations force developers to choose:
- **Option A**: Keep everything on-chain → Expensive, slow, limited functionality
- **Option B**: Move to L2/sidechain → Bridging complexity, fragmented liquidity
- **Option C**: Build off-chain infrastructure → Trust assumptions, centralization

**NEAR OutLayer provides Option D**: Keep funds and logic on NEAR L1, but execute heavy computation offshore with TEE-guaranteed correctness. Best of both worlds—just like financial offshore structures.

### The Path Forward

The roadmap prioritizes **security and transparency from day one**:
- ✅ TEE integration in Phase 1 (not "future work")
- ✅ GitHub-only policy for auditability
- ✅ No refunds to prevent DoS
- ✅ Instruction metering to prevent infinite loops
- ✅ Process isolation to prevent escapes

This is not just a service—**it's foundational infrastructure that unlocks a new category of blockchain applications**. Applications that were theoretically possible but practically infeasible due to gas limits can now be built on NEAR.

**NEAR OutLayer is to smart contracts what financial offshore zones are to businesses**: Optimize expensive operations in an efficient jurisdiction, but maintain control and final settlement on your home base (NEAR L1).

---

## Next Steps

### For Implementation
1. Choose TEE platform (recommend AWS Nitro for ease, Intel SGX for bare-metal)
2. Design smart contract API (reuse yield/resume patterns from current project)
3. Build WASM compilation pipeline with Docker sandboxing
4. Implement instruction metering in wasmi
5. Integrate TEE attestation verification
6. Launch testnet pilot with 5-10 developers

### For Business
1. Define pricing model (base fee + resource-based)
2. Identify launch partners (DeFi protocols, gaming projects)
3. Security audit (both contract and worker)
4. Legal review (compliance, terms of service)
5. Marketing positioning ("AWS Lambda for Smart Contracts")

### For Ecosystem
1. Create example WASM projects (GitHub repos)
2. Write developer documentation
3. Build SDK/libraries for common languages
4. Launch developer community (Discord, forum)
5. Organize hackathon to bootstrap adoption

**Timeline**: 4-5 months to production-ready MVP with TEE security from day one.
