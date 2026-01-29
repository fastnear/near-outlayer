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

1. **NEAR OutLayer Smart Contract** (`outlayer.near`)
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

### 5.1 Developer Payments (Stablecoin)

Project developers can receive payments from users who call their projects.

**Flow for Blockchain Calls:**
1. User deposits stablecoins via `ft_transfer_call` with `msg: {"action": "deposit_balance"}`
2. User calls `request_execution` with `attached_usd: U128(amount)` parameter
3. Amount deducted from user's stablecoin balance in contract
4. On successful execution, amount credited to project owner's `developer_earnings`
5. WASM can call `refund_usd(amount)` to return partial payment to caller
6. Project owner withdraws via `withdraw_developer_earnings()`

**Flow for HTTPS API Calls:**
1. User creates payment key with deposited balance
2. User calls HTTPS API with `X-Attached-Deposit` header
3. On successful execution, amount credited to project owner in coordinator DB
4. Earnings tracked in `project_owner_earnings` and `earnings_history` tables

**Earnings History:**
- Unified `earnings_history` table tracks both blockchain and HTTPS earnings
- Fields: `project_owner`, `project_id`, `attached_usd`, `refund_usd`, `amount`, `source`
- Blockchain-specific: `tx_hash`, `caller`, `request_id`
- HTTPS-specific: `call_id`, `payment_key_owner`, `payment_key_nonce`
- Dashboard shows earnings history at `/earnings` page

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

## Production Applications

### near.email - Secure Blockchain Email

**Live at**: [near.email](https://near.email) | **Docs**: [near.email/docs](https://near.email/docs)

The first production application built on NEAR OutLayer. Provides secure, private email for NEAR ecosystem users.

#### Features
- **Wallet-based identity**: Your NEAR account = your email (alice.near → alice@near.email)
- **End-to-end encryption**: ECIES with secp256k1, emails encrypted before storage
- **TEE-protected server**: Intel TDX ensures operators cannot read emails
- **NEAR MPC key derivation**: Encryption keys derived via Chain Signatures network
- **Internal email**: NEAR-to-NEAR emails never touch external SMTP servers
- **External email**: Full compatibility with Gmail, Outlook, etc.

#### Security Model
- Server receives email → **immediately encrypts** → stores encrypted blob → **deletes original**
- Only wallet owner can decrypt their emails
- TEE attestation proves correct code execution
- To compromise: must break Intel TDX OR compromise 27+ MPC validators

#### Access Modes
| Feature | Blockchain Mode | HTTPS Mode (Payment Key) |
|---------|----------------|-------------------------|
| Authentication | Wallet signature | Payment Key |
| Cost | ~0.001 NEAR gas | ~$0.001 per operation |
| Attachment download | 1.1 MB max | 18 MB max |
| Security | TEE + MPC | TEE + MPC (identical) |

#### Technical Stack
- **Frontend**: Next.js with TypeScript
- **WASM Module**: Rust with WASI
- **Encryption**: ECIES (secp256k1) + AES-GCM
- **Key derivation**: NEAR Chain Signatures MPC
- **TEE**: Intel TDX via Phala Cloud

#### Documentation Features
- Comparison with Gmail/ProtonMail
- Detailed security explanation for technical and non-technical users
- FAQ covering pricing, privacy, and trust model
- "What would it take to compromise" section

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

## Implementation Status (Updated: 2026-01-13)

### Completed Components

| Component | Status | Notes |
|-----------|--------|-------|
| **Smart Contract** | ✅ 100% | yield/resume, secrets management, dynamic pricing |
| **Coordinator API** | ✅ 100% | PostgreSQL + Redis, task queue, WASM cache |
| **Worker** | ✅ 100% | wasmi execution, fuel metering, WASI env vars |
| **Keystore Worker** | ✅ 100% | TEE attestation, access control validation |
| **Dashboard** | ✅ 100% | Next.js, secrets management, executions view, earnings page |
| **Register Contract** | ✅ 100% | Intel TDX verification, worker whitelist |

### Smart Contract Features
- ✅ `request_execution` with resource limit validation
- ✅ `resolve_execution` with actual metrics logging
- ✅ `store_secrets` / `delete_secrets` / `get_secrets` / `list_user_secrets`
- ✅ Dynamic pricing: `base_fee + (instructions × rate) + (time × rate)`
- ✅ Hard caps: 100B instructions, 60s execution time
- ✅ User secrets index for O(1) lookups
- ✅ Developer payments: `attached_usd` parameter for stablecoin payments to project owners
- ✅ User stablecoin balances: deposit via `ft_transfer_call` with `action=deposit_balance`
- ✅ Developer earnings withdrawal: `withdraw_developer_earnings()`
- ✅ Refund support: WASM can call `refund_usd()` to return partial payment to caller

### Worker Features
- ✅ wasmi with fuel metering (real instruction counting)
- ✅ WASI environment variables from decrypted secrets
- ✅ Docker sandboxed compilation (no network, resource limits)
- ✅ GitHub branch resolution via coordinator API with Redis caching
- ✅ TEE attestation (Intel TDX via Phala dstack)
- ✅ Payment host functions: `refund_usd()` for WASM to return partial payment
- ✅ `ATTACHED_USD` environment variable available to WASM

### Keystore Features
- ✅ Access control: AllowAll, Whitelist, AccountPattern, NEAR/FT/NFT balance
- ✅ Logic conditions (AND/OR/NOT)
- ✅ Reserved keywords protection (NEAR_SENDER_ID, etc.)
- ✅ Per-repo encryption keys (HMAC-SHA256 derived)

### Coordinator Features
- ✅ PostgreSQL + Redis task queue
- ✅ WASM cache with LRU eviction
- ✅ HTTPS API calls with payment keys
- ✅ Earnings history tracking (`earnings_history` table)
- ✅ Project owner earnings for HTTPS calls (`project_owner_earnings` table)
- ✅ Public API: `/public/project-earnings/:owner` and `/public/project-earnings/:owner/history`

### Infrastructure
- ✅ PostgreSQL + Redis via docker-compose
- ✅ WASM cache with LRU eviction
- ✅ Bearer token auth (SHA256 hashed)
- ✅ Phala Cloud deployment configs

### Pending Work

| Item | Priority | Notes |
|------|----------|-------|
| End-to-end tests | High | Integration testing |
| Load testing | Medium | Multiple concurrent executions |
| Security audit | High | Contract + worker |
| Deployment scripts | Low | Human handles deployment |

---

## Roadmap (Reference)

### Phase 1: TEE-Based MVP ✅ COMPLETE
- [x] Smart contract with yield/resume pattern
- [x] Payment validation and escrow
- [x] Timeout and cancellation handling
- [x] Event emission for workers
- [x] Intel TDX integration (via Phala Cloud)
- [x] Keypair generation inside TEE
- [x] Remote attestation report generation
- [x] wasmi with instruction metering
- [x] WASI runtime with minimal capabilities
- [x] Memory limits and timeout enforcement
- [x] Sandboxed Docker compilation
- [x] WASM binary caching (LRU)
- [x] TEE-based secret decryption
- [x] Environment variable injection

### Phase 2: Production Scaling ✅ MOSTLY COMPLETE
- [x] Multi-worker coordination (Redis task queue)
- [x] Dynamic pricing based on resource usage
- [ ] Advanced monitoring (Prometheus, Grafana)
- [ ] SLA guarantees documentation

### Phase 3: Operator Decentralization (Future)
- [ ] Multi-operator support
- [ ] Slashing for availability failures
- [ ] Reputation system

### Phase 4: Advanced Features (Future)
- [ ] ZK proofs for verification
- [ ] GPU support via WebGPU
- [ ] Cross-chain support

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

---

## System Hidden Logs - Admin Debugging Guide

### ⚠️ CRITICAL SECURITY WARNING

The `system_hidden_logs` table contains **RAW stderr/stdout** from compilation and execution containers. This data **MUST NEVER** be exposed via public API endpoints.

#### Security Risk

Malicious users can craft code that outputs system file contents:

```rust
// In build.rs or main.rs
fn main() {
    // This will be captured in stderr/stdout
    std::process::Command::new("cat")
        .arg("/etc/passwd")
        .output()
        .unwrap();
}
```

If these logs are exposed publicly, attackers can:
- Read server configuration files
- Leak environment variables
- Discover internal paths and secrets
- Enumerate installed packages

### Access Control

#### ✅ Safe Access Methods

1. **SSH/Localhost Only**
   ```bash
   # Connect to server via SSH
   ssh admin@coordinator-server

   # Query logs (localhost only)
   curl http://localhost:8080/admin/system-logs/186
   ```

2. **Direct Database Access**
   ```sql
   -- Connect to PostgreSQL
   psql postgres://postgres:password@localhost/offchainvm

   -- Query logs by request_id
   SELECT * FROM system_hidden_logs WHERE request_id = 186;
   ```

#### ❌ NEVER Do This

- ❌ Expose `/admin/system-logs/:request_id` via public URL
- ❌ Add this data to `/public/*` endpoints
- ❌ Return raw logs in API responses to users
- ❌ Include in dashboard frontend (even for logged-in users)

### Configuration

#### Disable Log Storage (Production)

Set environment variable in worker:

```bash
# worker/.env
SAVE_SYSTEM_HIDDEN_LOGS_TO_DEBUG=false
```

This prevents workers from storing raw logs. Users will still see **safe, classified error messages** like:
- "Repository not found. Please check that the repository URL is correct..."
- "Rust compilation failed. Your code contains syntax errors..."

#### Enable Log Storage (Development)

```bash
# worker/.env
SAVE_SYSTEM_HIDDEN_LOGS_TO_DEBUG=true  # Default
```

Useful for debugging new error classifications.

### API Endpoints

#### POST /internal/system-logs
**Internal only** - Used by workers to store logs. No authentication required (workers are trusted).

#### GET /admin/system-logs/:request_id
**Admin only** - Returns raw logs for debugging.

**Example:**
```bash
curl http://localhost:8080/admin/system-logs/186
```

**Response:**
```json
[
  {
    "id": 1,
    "request_id": 186,
    "job_id": 16,
    "log_type": "compilation",
    "stderr": "fatal: could not read Username for 'https://github.com': No such device or address\n...",
    "stdout": "",
    "exit_code": 128,
    "execution_error": null,
    "created_at": "2025-10-27T12:00:00Z"
  }
]
```

### Use Cases

#### 1. Debug Error Classifications

When users report confusing error messages:

1. Get request_id from dashboard
2. SSH to coordinator server
3. Query logs: `curl http://localhost:8080/admin/system-logs/{request_id}`
4. Analyze raw stderr to understand error
5. Add new error classification to `worker/src/compiler/docker.rs`

#### 2. Investigate Compilation Failures

```bash
# Get logs
curl http://localhost:8080/admin/system-logs/186 | jq '.[0].stderr'

# Look for patterns
# - "fatal: repository ... not found" → repository_not_found
# - "error[E0425]" → rust_compilation_error
# - "connection timed out" → network_error
```

#### 3. Monitor for Exploit Attempts

```sql
-- Check for suspicious patterns in logs
SELECT request_id, stderr
FROM system_hidden_logs
WHERE stderr LIKE '%/etc/%'
   OR stderr LIKE '%password%'
   OR stderr LIKE '%secret%'
LIMIT 20;
```

### Best Practices

1. **Production**: Set `SAVE_SYSTEM_HIDDEN_LOGS_TO_DEBUG=false` unless actively debugging
2. **Access**: Only allow `/admin/*` routes from localhost/private network
3. **Firewall**: Block external access to port 8080 `/admin/*` endpoints
4. **Monitoring**: Alert on unusual log patterns (system file paths, etc.)
5. **Retention**: Consider periodic cleanup of old logs (>30 days)

### Database Schema

```sql
CREATE TABLE system_hidden_logs (
    id BIGSERIAL PRIMARY KEY,
    request_id BIGINT NOT NULL,
    job_id BIGINT,
    log_type VARCHAR(50) NOT NULL, -- 'compilation' or 'execution'
    stderr TEXT,                     -- ⚠️ May contain leaked data
    stdout TEXT,                     -- ⚠️ May contain leaked data
    exit_code INTEGER,
    execution_error TEXT,
    created_at TIMESTAMP NOT NULL DEFAULT NOW()
);
```

### Related Files

- `coordinator/migrations/20251027000001_add_compilation_logs.sql` - Table definition with security warnings
- `coordinator/src/handlers/internal.rs` - Admin endpoints implementation
- `coordinator/src/models.rs` - SystemHiddenLog struct
- `worker/src/compiler/docker.rs` - Error extraction and classification logic
- `worker/src/config.rs` - SAVE_SYSTEM_HIDDEN_LOGS_TO_DEBUG flag
- `worker/.env.example` - Configuration template with security notes

### Error Classification System

The worker classifies errors into safe, user-facing descriptions:

- `repository_not_found` - "Repository not found. Please check that the repository URL is correct..."
- `repository_access_denied` - "Cannot access repository. The repository may be private..."
- `invalid_repository_url` - "Invalid repository URL format. The URL should not contain spaces..."
- `git_error` - "Git operation failed. Please verify the repository URL..."
- `network_error` - "Network connection error. The repository server may be unreachable..."
- `rust_compilation_error` - "Rust compilation failed. Your code contains syntax errors..."
- `dependency_not_found` - "Dependency resolution failed. One or more dependencies could not be found..."
- `build_script_error` - "Build script execution failed..."
- `git_fatal_error` - "Git fatal error occurred..."
- `git_usage_error` - "Git command error. The repository URL or parameters are invalid..."
- `compilation_error` - Generic fallback

Users see only these safe descriptions. Admins can access raw stderr/stdout via `system_hidden_logs` table for debugging.
