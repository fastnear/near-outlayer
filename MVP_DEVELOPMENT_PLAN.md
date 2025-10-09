# NEAR Offshore (OffchainVM) - MVP Development Plan

**Goal**: Build production-ready MVP without TEE, with architecture that allows easy TEE integration via Phala Network in Phase 2.

---

## Quick Summary

### What We're Building
**OffchainVM** - Off-chain execution layer for NEAR smart contracts. Smart contracts can execute arbitrary WASM code off-chain using NEAR's yield/resume mechanism.

### Key Decisions (MVP)
- **Contract**: `offchainvm.near` (Rust, near-sdk)
- **Worker**: Rust + Tokio + wasmi + Docker
- **Coordinator API**: Centralized HTTP API server (Rust + Axum)
  - Workers authenticate via bearer tokens
  - Combines PostgreSQL + Redis + Local WASM storage
  - Single point of control, no direct DB access from workers
- **WASM Storage**: Local filesystem on API server
  - Path: `/var/offchainvm/wasm/{checksum}.wasm`
  - LRU eviction (configurable size, delete old unused files)
  - Workers download via API: `GET /wasm/{checksum}`
- **Task Queue**: Redis (behind API, workers poll via HTTP)
- **Compilation**: Docker sandboxed (no network, resource limits)
- **Execution**: wasmi with instruction metering
- **Phase 2**: Phala TEE integration (not in MVP)

### Test Project
Random number generator WASM to test end-to-end flow:
- GitHub: `offchainvm-test-random`
- Returns random number in given range (or 0-9 default)
- Used for E2E testing

### Timeline (Estimated)
- **Smart Contract**: 1 week
- **Coordinator API Server**: 1-2 weeks
- **Worker (all iterations)**: 2-3 weeks
- **Testing & Validation**: 1 week
- **Total MVP**: 5-7 weeks (1 experienced Rust dev)
- **Phase 2 (Phala)**: +2-3 weeks

---

## Architecture Overview

### MVP Architecture (Phase 1 - No TEE)
```
┌─────────────────┐
│  Client Contract│
│   (client.near) │
└────────┬────────┘
         │ execute()
         ↓
┌───────────────────┐         ┌─────────────────┐
│OffchainVM Contract│←────────│  Event Indexer  │
│(offchainvm.near)  │  events │ (neardata.xyz)  │
└────────┬──────────┘         └─────────────────┘
         │ yield                      │
         ↓                            │
    [Paused]                          │
         ↑                            │
         │                            ↓
         │                   ┌────────────────────┐
         │                   │ Coordinator API    │
         │                   │ (Rust + Axum)      │
         │                   │                    │
         │                   │ ┌────────────────┐ │
         │                   │ │  PostgreSQL    │ │
         │                   │ │  (metadata)    │ │
         │                   │ └────────────────┘ │
         │                   │ ┌────────────────┐ │
         │                   │ │  Redis         │ │
         │                   │ │  (queue/locks) │ │
         │                   │ └────────────────┘ │
         │                   │ ┌────────────────┐ │
         │                   │ │  Local FS      │ │
         │                   │ │  (WASM cache)  │ │
         │                   │ └────────────────┘ │
         │                   └──────────┬─────────┘
         │                              │
         │                              │ HTTP API
         │                    ┌─────────┴─────────┐
         │                    │ (auth tokens)     │
         │            ┌───────▼──────┬────────────▼───────┐
         │            │              │                    │
         │      ┌─────▼────┐   ┌─────▼────┐   ┌──────────▼─┐
         │      │ Worker 1 │   │ Worker 2 │   │  Worker 3  │
         │      │ (Rust)   │   │ (Rust)   │   │  (Rust)    │
         │      └─────┬────┘   └─────┬────┘   └──────┬─────┘
         │            │              │               │
         │            │              │               │
         └────────────┴──────────────┴───────────────┘
                  resume_execution()

API Endpoints:
- POST /tasks/poll         - Get next task (blocking)
- POST /tasks/complete     - Mark task done
- GET  /wasm/{checksum}    - Download WASM
- POST /wasm/upload        - Upload compiled WASM
- POST /events/new         - Event monitor pushes events
- GET  /health             - Worker heartbeat

Key Benefits:
- Single API server controls everything
- Workers authenticate via tokens (anti-DDoS)
- Local WASM storage with LRU eviction
- No S3 dependency
- Easy to monitor/debug (single point)
```

### Future Architecture (Phase 2 - With Phala TEE)
```
Same as above, but Worker Node runs inside Phala TEE enclave
Worker registers public key on-chain
Secrets encrypted with Phala's public key
Execution produces attestation report
```

---

## Technology Stack

### Smart Contract (NEAR L1)
- **Language**: Rust
- **Framework**: `near-sdk` v5.x (latest stable)
- **Build Target**: `wasm32-unknown-unknown`
- **Tools**:
  - `cargo-near` for building/deployment
  - `near-workspaces` for integration tests
  - `near-cli-rs` for CLI interactions

### Worker (Off-Chain)
- **Language**: Rust (performance, safety, WASM ecosystem)
- **Runtime**: Tokio async runtime
- **Key Libraries**:
  - `wasmi` v0.31+ - WASM interpreter with instruction metering
  - `tokio` - async runtime
  - `near-jsonrpc-client` - NEAR RPC interactions
  - `near-crypto` - key management, signing
  - `near-primitives` - NEAR types
  - `serde_json` - JSON handling
  - `reqwest` - HTTP client (for GitHub API, event indexer)
  - `sqlx` - PostgreSQL async driver with connection pooling
  - `redis` - Redis client (async, for task queue and pub/sub)
  - `aws-sdk-s3` / `object_store` - S3-compatible storage
  - `tracing` / `tracing-subscriber` - logging
  - `anyhow` / `thiserror` - error handling
  - `bollard` - Docker client

### Event Monitoring
- **Indexer**: neardata.xyz API (HTTP polling)
- **Alternative**: NEAR Lake Framework (for production scaling)
- **Fallback**: Direct RPC with `EXPERIMENTAL_tx_status`

### WASM Compilation
- **Runtime**: Docker
- **Base Image**: `rust:1.75-slim` (or latest stable)
- **Compiler**: `rustc` with `wasm32-wasi` target
- **Security**: Docker with `--network=none`, resource limits

### Storage & Coordination (Multi-Worker from Day 1)

#### Coordinator API Server
- **Technology**: Rust + Axum HTTP server
- **Purpose**: Single point of control, workers have no direct database access
- **Components**:
  - PostgreSQL (metadata & analytics)
  - Redis (task queue & distributed locks)
  - Local filesystem (WASM cache with LRU eviction)
- **Authentication**: Bearer tokens for workers (anti-DDoS)
- **Deployment**: Single instance initially, can scale horizontally later

#### WASM Cache (Local Filesystem with LRU)
- **Storage**: Local filesystem on API server
  - Path: `/var/offchainvm/wasm/{sha256}.wasm`
  - Configurable size limit (e.g., 10GB, 50GB)
  - LRU eviction: track last access time, delete oldest when full
- **Metadata Table**: `wasm_cache` in PostgreSQL
  ```sql
  CREATE TABLE wasm_cache (
    checksum TEXT PRIMARY KEY,
    repo_url TEXT NOT NULL,
    commit_hash TEXT NOT NULL,
    file_size BIGINT NOT NULL,
    created_at TIMESTAMP NOT NULL,
    last_accessed_at TIMESTAMP NOT NULL,
    access_count BIGINT DEFAULT 0
  );
  ```
- **Flow**:
  - Worker requests: `GET /wasm/{checksum}`
  - If file exists: update `last_accessed_at`, return file
  - If not: worker compiles, uploads via `POST /wasm/upload`
  - API server saves to disk, inserts metadata
  - LRU eviction runs periodically (cron job or on-demand)

#### PostgreSQL (Metadata & Analytics)
- **Tables**:
  - `execution_requests` - Request tracking and history
  - `wasm_cache` - Repo → checksum mapping with LRU metadata
  - `execution_history` - Analytics, debugging, performance metrics
  - `worker_auth_tokens` - Bearer tokens for worker authentication
- **Access**: Only via Coordinator API, no direct worker connections
- **Connection pooling**: API server uses `sqlx` with 20-50 connections

#### Redis (Task Queue & Coordination)
- **Task Queue**:
  - `LIST`: `LPUSH` for new tasks, workers poll via `POST /tasks/poll`
  - API server performs `BRPOP` internally, returns task to worker via HTTP
  - Workers use long-polling (30-60s timeout) to avoid busy-waiting
- **Distributed Locks**:
  - Lock during compilation: `SET compilation:{repo}:{commit} NX EX 300`
  - API endpoint: `POST /locks/acquire` and `DELETE /locks/release`
  - Prevents duplicate compilations across workers
  - Expires after 5 min (handles worker crashes)
- **Access**: Only via Coordinator API

#### Coordinator API Endpoints
```
Authentication: All endpoints require `Authorization: Bearer <token>` header

GET  /tasks/poll?timeout=60          - Long-poll for next task (blocks up to 60s)
POST /tasks/complete                 - Mark task as completed with result
POST /tasks/fail                     - Mark task as failed with error

GET  /wasm/{checksum}                - Download compiled WASM file
POST /wasm/upload                    - Upload newly compiled WASM (multipart)
GET  /wasm/exists/{checksum}         - Check if WASM exists (avoid re-compile)

POST /locks/acquire                  - Acquire distributed lock (compilation)
DELETE /locks/release/{lock_key}     - Release distributed lock

POST /requests/create                - Create new execution request (internal)
GET  /requests/{request_id}          - Get request status and metadata
```

#### Why This Architecture?
- ✅ **Centralized control**: API server manages all state, workers are dumb clients
- ✅ **Anti-DDoS**: Bearer token authentication prevents unauthorized workers
- ✅ **No S3 dependency**: Local filesystem with LRU eviction is simple and fast
- ✅ **LRU eviction**: Automatically removes old WASM files to save disk space
- ✅ **HTTP API**: Workers use simple HTTP client, no database drivers needed
- ✅ **Stateless workers**: Workers can be killed/restarted anytime
- ✅ **Horizontal scaling**: Add more workers by issuing new auth tokens
- ✅ **Observability**: API server can log all worker activity centrally

### Development Tools
- **Testing**:
  - `cargo test` - unit tests
  - `near-workspaces` - integration tests
  - `cargo-nextest` - fast test runner
- **Linting**:
  - `clippy` - Rust linter
  - `rustfmt` - code formatting
- **CI/CD**:
  - GitHub Actions
  - Automated testing on PR
  - Contract deployment scripts

---

## Phase 1: MVP (No TEE)

### 1. Smart Contract Development

#### 1.1 Core Contract Structure
```rust
// File: contract/src/lib.rs

#[near_bindgen]
pub struct OffshoreContract {
    owner_id: AccountId,
    operator_id: AccountId,  // Who can call resolve_execution
    paused: bool,

    // Pricing
    base_fee: Balance,
    per_instruction_fee: Balance,
    per_mb_fee: Balance,
    per_second_fee: Balance,

    // Request tracking
    next_request_id: u64,
    pending_requests: LookupMap<u64, ExecutionRequest>,

    // Statistics
    total_executions: u64,
    total_fees_collected: Balance,
}

pub struct ExecutionRequest {
    request_id: u64,
    data_id: CryptoHash,
    sender_id: AccountId,
    code_source: CodeSource,
    resource_limits: ResourceLimits,
    payment: Balance,
    timestamp: u64,
    status: RequestStatus,
}

pub enum CodeSource {
    GitHub {
        repo: String,      // "https://github.com/user/repo"
        commit: String,    // full commit hash
        build_target: String, // "wasm32-wasi"
    },
}

pub struct ResourceLimits {
    max_execution_seconds: u64,
    max_memory_mb: u64,
    max_instructions: u64,
}

pub enum RequestStatus {
    Pending,
    Compiling,
    Executing,
    Completed,
    Failed,
    Timeout,
}
```

#### 1.2 Contract Methods

**Admin Methods**:
```rust
#[init]
pub fn new(owner_id: AccountId, operator_id: AccountId) -> Self

pub fn set_operator(&mut self, operator_id: AccountId)
pub fn set_pricing(&mut self, base_fee: U128, per_instruction_fee: U128, ...)
pub fn pause(&mut self)
pub fn unpause(&mut self)
pub fn withdraw_fees(&mut self, amount: U128)
```

**Main Methods**:
```rust
#[payable]
pub fn execute(
    &mut self,
    code_source: CodeSource,
    resource_limits: ResourceLimits,
) -> Promise

pub fn resolve_execution(
    &mut self,
    data_id: CryptoHash,
    result: ExecutionResult,
)

pub fn cancel_stale_request(&mut self, request_id: u64)
```

**View Methods**:
```rust
pub fn get_request(&self, request_id: u64) -> Option<ExecutionRequest>
pub fn get_pending_requests(&self, from: u64, limit: u64) -> Vec<ExecutionRequest>
pub fn get_pricing(&self) -> PricingInfo
pub fn estimate_cost(&self, limits: ResourceLimits) -> U128
```

#### 1.3 Events
```rust
#[derive(Serialize)]
#[serde(tag = "event", content = "data")]
pub enum OffshoreEvent {
    ExecutionRequest {
        request_id: u64,
        data_id: String,
        sender_id: AccountId,
        code_source: CodeSource,
        resource_limits: ResourceLimits,
    },
    ExecutionCompleted {
        request_id: u64,
        success: bool,
        instructions_used: u64,
        memory_used_mb: u64,
        time_seconds: u64,
    },
    CompilationProgress {
        request_id: u64,
        status: String,
        estimated_seconds: u64,
    },
}
```

#### 1.4 Testing Requirements
- Unit tests for all methods
- Integration tests with `near-workspaces`:
  - Execute → yield → resolve flow
  - Payment validation
  - Timeout handling
  - Stale request cancellation
- Gas consumption benchmarks
- Edge cases:
  - Zero payment
  - Invalid GitHub URL
  - Malformed data_id

---

### 2. Worker Development

#### 2.1 Project Structure
```
worker/
├── Cargo.toml
├── src/
│   ├── main.rs              # Entry point, CLI
│   ├── config.rs            # Configuration from env/file
│   ├── event_monitor.rs     # Watch blockchain for events
│   ├── task_queue.rs        # Persistent task queue
│   ├── compiler.rs          # GitHub → Docker → WASM
│   ├── executor.rs          # WASM execution with wasmi
│   ├── near_client.rs       # NEAR RPC interactions
│   ├── cache.rs             # WASM caching logic
│   ├── metrics.rs           # Resource usage tracking
│   └── error.rs             # Error types
├── tests/
│   ├── integration_test.rs
│   └── fixtures/
└── docker/
    └── compiler.Dockerfile  # Sandboxed compilation
```

#### 2.2 Configuration
```rust
// config.rs
pub struct Config {
    // NEAR connection
    pub near_rpc_url: String,
    pub near_network: String,

    // Contract
    pub offchainvm_contract_id: AccountId,
    pub operator_account_id: AccountId,
    pub operator_private_key: SecretKey,

    // Coordinator API Server
    pub api_base_url: String,  // http://localhost:8080
    pub api_auth_token: String,  // Bearer token for authentication
    pub api_timeout_seconds: u64,  // HTTP request timeout (default: 60)
    pub task_poll_timeout_seconds: u64,  // Long-polling timeout (default: 60)

    // Event monitoring (optional - only for event monitor worker)
    pub enable_event_monitor: bool,  // Only ONE worker should enable this
    pub start_block_height: BlockHeight,
    pub poll_interval_ms: u64,
    pub indexer_url: String,  // neardata.xyz or Lake

    // Compilation
    pub docker_image: String,
    pub compile_timeout_seconds: u64,
    pub max_repo_size_mb: u64,

    // Execution
    pub max_concurrent_executions: usize,

    // Worker identity
    pub worker_id: String,  // unique per worker instance

    // Monitoring
    pub log_level: String,
    pub metrics_port: u16,
}
```

#### 2.3 Event Monitor (Single Instance)
```rust
// event_monitor.rs

// Only ONE event monitor should run across all workers
// Use distributed lock via Coordinator API to ensure single instance

pub struct EventMonitor {
    config: Arc<Config>,
    near_client: NearClient,
    api_client: ApiClient,  // HTTP client for Coordinator API
    last_processed_block: BlockHeight,
}

impl EventMonitor {
    pub async fn start(&mut self) -> Result<()> {
        // Acquire distributed lock (only one monitor across all workers)
        let lock_key = "event_monitor";

        loop {
            // Try to acquire lock via API
            let request = AcquireLockRequest {
                lock_key: lock_key.to_string(),
                worker_id: self.config.worker_id.clone(),
                ttl_seconds: 60,
            };

            let acquired = self.api_client
                .post("/locks/acquire")
                .json(&request)
                .send()
                .await?
                .json::<AcquireLockResponse>()
                .await?
                .acquired;

            if !acquired {
                // Another worker is monitoring events
                info!("Event monitor lock held by another worker, sleeping...");
                tokio::time::sleep(Duration::from_secs(30)).await;
                continue;
            }

            // We have the lock - monitor events
            info!("Event monitor lock acquired, starting monitoring loop");
            match self.monitor_loop(lock_key).await {
                Ok(_) => break,
                Err(e) => {
                    error!("Event monitor error: {}", e);
                    // Release lock on error
                    let _ = self.api_client
                        .delete(&format!("/locks/release/{}", lock_key))
                        .send()
                        .await;
                    tokio::time::sleep(Duration::from_secs(5)).await;
                }
            }
        }

        Ok(())
    }

    async fn monitor_loop(&mut self, lock_key: &str) -> Result<()> {
        loop {
            // Renew lock every 30s via API
            tokio::select! {
                _ = tokio::time::sleep(Duration::from_secs(30)) => {
                    let request = AcquireLockRequest {
                        lock_key: lock_key.to_string(),
                        worker_id: self.config.worker_id.clone(),
                        ttl_seconds: 60,
                    };
                    // Renew by re-acquiring with same worker_id
                    self.api_client
                        .post("/locks/acquire")
                        .json(&request)
                        .send()
                        .await?;
                }

                result = self.process_blocks() => {
                    result?;
                }
            }
        }
    }

    async fn process_blocks(&mut self) -> Result<()> {
        // Fetch new blocks from indexer
        let blocks = self.fetch_blocks_since(self.last_processed_block).await?;

        for block in blocks {
            // Parse events from contract
            let events = self.extract_events(&block)?;

            for event in events {
                match event {
                    ExecutionRequest { request_id, code_source, .. } => {
                        // Create task and push via Coordinator API
                        let task = CreateTaskRequest {
                            request_id,
                            code_source: code_source.clone(),
                            resource_limits: event.resource_limits.clone(),
                            data_id: event.data_id,
                        };

                        self.api_client
                            .post("/tasks/create")
                            .json(&task)
                            .send()
                            .await?;

                        info!("New task {} pushed to queue via API", request_id);
                    }
                }
            }

            self.last_processed_block = block.height;
        }

        tokio::time::sleep(Duration::from_millis(self.config.poll_interval_ms)).await;
        Ok(())
    }
}
```

#### 2.4 Task Queue (Via Coordinator API)
```rust
// task_queue.rs

pub enum Task {
    Compile {
        request_id: u64,
        code_source: CodeSource,
    },
    Execute {
        request_id: u64,
        data_id: CryptoHash,
        wasm_checksum: String,
        resource_limits: ResourceLimits,
    },
}

pub struct TaskQueue {
    api_client: ApiClient,  // HTTP client for Coordinator API
    config: Arc<Config>,
}

impl TaskQueue {
    pub async fn poll(&self) -> Result<Option<Task>> {
        // Long-poll for next task via Coordinator API
        // API server performs BRPOP internally and returns task
        let timeout = self.config.task_poll_timeout_seconds;

        let response = self.api_client
            .get(&format!("/tasks/poll?timeout={}", timeout))
            .timeout(Duration::from_secs(timeout + 5)) // Add 5s buffer
            .send()
            .await?;

        if response.status() == 204 {
            // No tasks available (timeout reached)
            return Ok(None);
        }

        let task: Task = response.json().await?;
        Ok(Some(task))
    }

    pub async fn mark_completed(&self, request_id: u64, result: ExecutionResult) -> Result<()> {
        let payload = CompleteTaskRequest {
            request_id,
            success: result.success,
            output: result.output,
            error: result.error,
            execution_time_ms: result.execution_time_ms,
        };

        self.api_client
            .post("/tasks/complete")
            .json(&payload)
            .send()
            .await?;

        Ok(())
    }

    pub async fn mark_failed(&self, request_id: u64, error: String) -> Result<()> {
        let payload = FailTaskRequest {
            request_id,
            error,
        };

        self.api_client
            .post("/tasks/fail")
            .json(&payload)
            .send()
            .await?;

        Ok(())
    }
}
```

#### 2.5 Compiler (with Distributed Lock & API Cache)
```rust
// compiler.rs

pub struct Compiler {
    config: Arc<Config>,
    docker_client: Docker,
    api_client: ApiClient,  // HTTP client for Coordinator API
}

impl Compiler {
    pub async fn compile(
        &self,
        code_source: &CodeSource,
    ) -> Result<CompilationResult> {
        match code_source {
            CodeSource::GitHub { repo, commit, build_target } => {
                let checksum = self.compute_expected_checksum(repo, commit)?;

                // 1. Check if WASM exists in cache via API
                let exists_response = self.api_client
                    .get(&format!("/wasm/exists/{}", checksum))
                    .send()
                    .await?
                    .json::<WasmExistsResponse>()
                    .await?;

                if exists_response.exists {
                    info!("WASM found in cache: {}", checksum);
                    return Ok(CompilationResult::Cached(checksum));
                }

                // 2. Acquire distributed lock via API to prevent duplicate compilations
                let lock_key = format!("compilation:{}:{}", repo, commit);
                let acquire_request = AcquireLockRequest {
                    lock_key: lock_key.clone(),
                    worker_id: self.config.worker_id.clone(),
                    ttl_seconds: 300, // 5 min
                };

                let acquired = self.api_client
                    .post("/locks/acquire")
                    .json(&acquire_request)
                    .send()
                    .await?
                    .json::<AcquireLockResponse>()
                    .await?
                    .acquired;

                if !acquired {
                    // Another worker is compiling this repo
                    info!("Another worker is compiling {}, waiting...", repo);

                    // Wait for compilation to complete (poll API for WASM existence)
                    for _ in 0..60 {  // Wait up to 5 minutes
                        tokio::time::sleep(Duration::from_secs(5)).await;

                        let check = self.api_client
                            .get(&format!("/wasm/exists/{}", checksum))
                            .send()
                            .await?
                            .json::<WasmExistsResponse>()
                            .await?;

                        if check.exists {
                            return Ok(CompilationResult::Cached(checksum));
                        }
                    }

                    return Err(anyhow!("Compilation timeout waiting for other worker"));
                }

                // 3. We have the lock - compile
                info!("Starting compilation: {}/{}", repo, commit);

                // Verify repo is public
                self.verify_repo_public(repo).await?;

                // Create isolated Docker container
                let container_name = format!("compile-{}-{}", checksum, Uuid::new_v4());
                let container = self.docker_client.create_container(
                    Some(CreateContainerOptions { name: container_name.clone() }),
                    ContainerConfig {
                        image: Some(self.config.docker_image.clone()),
                        network_disabled: Some(true),  // No network!
                        memory: Some(2 * 1024 * 1024 * 1024), // 2GB
                        cpu_quota: Some(200_000), // 2 cores
                        cmd: Some(vec![
                            "sh", "-c",
                            &format!(
                                "git clone {} /src && \
                                 cd /src && \
                                 git checkout {} && \
                                 cargo build --release --target {}",
                                repo, commit, build_target
                            )
                        ]),
                        ..Default::default()
                    },
                ).await?;

                // Start with timeout
                self.docker_client.start_container(&container.id, None).await?;

                let compile_result = tokio::select! {
                    _ = tokio::time::sleep(Duration::from_secs(self.config.compile_timeout_seconds)) => {
                        self.docker_client.stop_container(&container.id, None).await?;
                        Err(anyhow!("Compilation timeout"))
                    }
                    result = self.docker_client.wait_container(&container.id, None) => {
                        result
                    }
                };

                // Extract WASM binary
                let wasm_bytes = self.extract_wasm_from_container(&container.id).await?;

                // Cleanup Docker
                self.docker_client.remove_container(&container.id, None).await?;

                compile_result?;

                // 4. Upload to Coordinator API
                let form = multipart::Form::new()
                    .text("checksum", checksum.clone())
                    .text("repo_url", repo.clone())
                    .text("commit_hash", commit.clone())
                    .part("wasm_file", multipart::Part::bytes(wasm_bytes.clone()));

                self.api_client
                    .post("/wasm/upload")
                    .multipart(form)
                    .send()
                    .await?;

                // 5. Release lock via API
                self.api_client
                    .delete(&format!("/locks/release/{}", lock_key))
                    .send()
                    .await?;

                info!("Compilation successful: {}, uploaded to API", checksum);

                Ok(CompilationResult::Success { checksum })
            }
        }
    }
}
```

#### 2.6 WASM Executor
```rust
// executor.rs

use wasmi::*;

pub struct WasmExecutor {
    config: Arc<Config>,
}

pub struct ExecutionResult {
    pub success: bool,
    pub return_value: Option<Vec<u8>>,
    pub logs: String,
    pub resources_used: ResourceMetrics,
    pub error: Option<String>,
}

pub struct ResourceMetrics {
    pub instructions: u64,
    pub memory_bytes: u64,
    pub time_seconds: u64,
}

impl WasmExecutor {
    pub async fn execute(
        &self,
        wasm_bytes: &[u8],
        limits: ResourceLimits,
    ) -> Result<ExecutionResult> {
        let start_time = Instant::now();

        // Parse and validate WASM
        let module = Module::new(&Engine::default(), wasm_bytes)?;

        // Create store with resource limits
        let mut store = Store::new(
            &Engine::default(),
            (),
        );

        // Set instruction metering
        store.limiter(|_| ResourceLimiter {
            max_instructions: limits.max_instructions,
            max_memory_bytes: limits.max_memory_mb * 1024 * 1024,
            instructions_used: 0,
            memory_used: 0,
        });

        // Create linker with minimal WASI
        let mut linker = Linker::new(&Engine::default());
        let wasi_ctx = WasiCtxBuilder::new()
            .inherit_stdout()
            .inherit_stderr()
            .build();
        wasmtime_wasi::add_to_linker(&mut linker, |s| s)?;

        // Instantiate
        let instance = linker.instantiate(&mut store, &module)?;

        // Find entry point
        let execute_fn = instance
            .get_typed_func::<(), i32>(&mut store, "execute")?;

        // Execute with timeout
        let result = tokio::select! {
            _ = tokio::time::sleep(Duration::from_secs(limits.max_execution_seconds)) => {
                Err(anyhow!("Execution timeout"))
            }
            result = tokio::task::spawn_blocking(move || {
                execute_fn.call(&mut store, ())
            }) => {
                result?
            }
        };

        let elapsed = start_time.elapsed();

        // Collect metrics from store
        let metrics = ResourceMetrics {
            instructions: store.limiter().instructions_used,
            memory_bytes: store.limiter().memory_used,
            time_seconds: elapsed.as_secs(),
        };

        match result {
            Ok(ret_val) => {
                // Parse return value (WASM linear memory pointer)
                let return_data = self.read_return_data(&instance, &mut store, ret_val)?;

                Ok(ExecutionResult {
                    success: true,
                    return_value: Some(return_data),
                    logs: String::new(), // Captured from stdout
                    resources_used: metrics,
                    error: None,
                })
            }
            Err(e) => {
                Ok(ExecutionResult {
                    success: false,
                    return_value: None,
                    logs: String::new(),
                    resources_used: metrics,
                    error: Some(e.to_string()),
                })
            }
        }
    }
}

// Resource limiter implementation
struct ResourceLimiter {
    max_instructions: u64,
    max_memory_bytes: u64,
    instructions_used: u64,
    memory_used: u64,
}

impl wasmi::ResourceLimiter for ResourceLimiter {
    fn memory_growing(&mut self, current: usize, desired: usize, _maximum: Option<usize>) -> bool {
        let new_size = desired * 65536; // WASM page size
        if new_size > self.max_memory_bytes as usize {
            return false;
        }
        self.memory_used = new_size as u64;
        true
    }

    fn instruction_executed(&mut self) -> bool {
        self.instructions_used += 1;
        self.instructions_used < self.max_instructions
    }
}
```

#### 2.7 Worker Main Loop (API-Based)
```rust
// main.rs

#[tokio::main]
async fn main() -> Result<()> {
    // Load config
    let config = Config::from_env()?;

    // Initialize API client with authentication
    let api_client = ApiClient::new(&config.api_base_url, &config.api_auth_token);

    // Initialize components
    let near_client = NearClient::new(&config).await?;
    let task_queue = TaskQueue::new(api_client.clone(), config.clone());
    let compiler = Compiler::new(&config, api_client.clone()).await?;
    let executor = WasmExecutor::new(&config);

    // Start event monitor in background (only if enabled)
    if config.enable_event_monitor {
        let event_monitor = EventMonitor::new(
            config.clone(),
            near_client.clone(),
            api_client.clone()
        );
        tokio::spawn(async move {
            event_monitor.start().await
        });
    }

    // Worker pool
    let mut workers = Vec::new();
    for worker_id in 0..config.max_concurrent_executions {
        let task_queue = task_queue.clone();
        let compiler = compiler.clone();
        let executor = executor.clone();
        let near_client = near_client.clone();
        let api_client = api_client.clone();
        let worker_name = format!("worker-{}", worker_id);

        workers.push(tokio::spawn(async move {
            loop {
                // Long-poll for task from API
                let task = match task_queue.poll().await {
                    Ok(Some(task)) => task,
                    Ok(None) => {
                        // Timeout reached, no tasks available
                        continue;
                    }
                    Err(e) => {
                        error!("{}: Failed to poll tasks: {}", worker_name, e);
                        tokio::time::sleep(Duration::from_secs(5)).await;
                        continue;
                    }
                };

                match task {
                    Task::Compile { request_id, code_source } => {
                        info!("{}: Compiling request {}", worker_name, request_id);

                        match compiler.compile(&code_source).await {
                            Ok(CompilationResult::Success { checksum }) |
                            Ok(CompilationResult::Cached(checksum)) => {
                                info!("{}: Compilation successful, checksum: {}", worker_name, checksum);
                                // Note: Task is now converted to Execute automatically by API
                                // or we queue it manually here
                            }
                            Err(e) => {
                                error!("{}: Compilation failed: {}", worker_name, e);
                                task_queue.mark_failed(request_id, e.to_string()).await?;
                            }
                        }
                    }

                    Task::Execute { request_id, data_id, wasm_checksum, resource_limits } => {
                        info!("{}: Executing request {}", worker_name, request_id);

                        // Download WASM from API
                        let wasm_bytes = match api_client
                            .get(&format!("/wasm/{}", wasm_checksum))
                            .send()
                            .await
                        {
                            Ok(response) => response.bytes().await?,
                            Err(e) => {
                                error!("{}: Failed to download WASM: {}", worker_name, e);
                                task_queue.mark_failed(request_id, e.to_string()).await?;
                                continue;
                            }
                        };

                        // Execute WASM
                        let result = executor.execute(&wasm_bytes, resource_limits).await?;

                        // Report result to contract via NEAR
                        match near_client.resolve_execution(data_id, result.clone()).await {
                            Ok(_) => {
                                info!("{}: Execution completed successfully", worker_name);
                                task_queue.mark_completed(request_id, result).await?;
                            }
                            Err(e) => {
                                error!("{}: Failed to resolve execution on-chain: {}", worker_name, e);
                                task_queue.mark_failed(request_id, e.to_string()).await?;
                            }
                        }
                    }
                }
            }
        }));
    }

    // Wait for all workers
    futures::future::join_all(workers).await;

    Ok(())
}
```

---

### 3. Database Schema

#### PostgreSQL Schema (Multi-Worker from Day 1)

**File**: `worker/schema.sql`

```sql
-- Request tracking
CREATE TABLE execution_requests (
    request_id BIGINT PRIMARY KEY,
    data_id BYTEA NOT NULL,
    sender_id TEXT NOT NULL,
    code_source JSONB NOT NULL,
    resource_limits JSONB NOT NULL,
    payment TEXT NOT NULL,
    status TEXT NOT NULL,
    created_at TIMESTAMP DEFAULT NOW(),
    updated_at TIMESTAMP DEFAULT NOW()
);

CREATE INDEX idx_requests_status ON execution_requests(status);
CREATE INDEX idx_requests_created ON execution_requests(created_at);
CREATE INDEX idx_requests_sender ON execution_requests(sender_id);

-- WASM cache metadata (local filesystem with LRU)
CREATE TABLE wasm_cache (
    checksum TEXT PRIMARY KEY,
    repo_url TEXT NOT NULL,
    commit_hash TEXT NOT NULL,
    file_size BIGINT NOT NULL,
    created_at TIMESTAMP DEFAULT NOW(),
    last_accessed_at TIMESTAMP DEFAULT NOW(),
    access_count BIGINT DEFAULT 0,
    compilation_time_seconds INT
);

CREATE INDEX idx_cache_repo_commit ON wasm_cache(repo_url, commit_hash);
CREATE INDEX idx_cache_last_accessed ON wasm_cache(last_accessed_at);  -- For LRU eviction

-- Worker authentication tokens
CREATE TABLE worker_auth_tokens (
    token_hash TEXT PRIMARY KEY,  -- SHA256 of bearer token
    worker_name TEXT NOT NULL,
    created_at TIMESTAMP DEFAULT NOW(),
    last_used_at TIMESTAMP DEFAULT NOW(),
    is_active BOOLEAN DEFAULT true
);

CREATE INDEX idx_tokens_active ON worker_auth_tokens(is_active);

-- Execution history (analytics)
CREATE TABLE execution_history (
    id BIGSERIAL PRIMARY KEY,
    request_id BIGINT REFERENCES execution_requests(request_id),
    worker_id TEXT NOT NULL,
    success BOOLEAN NOT NULL,
    instructions_used BIGINT,
    memory_used_bytes BIGINT,
    time_seconds INT,
    error_message TEXT,
    wasm_checksum TEXT REFERENCES wasm_cache_metadata(checksum),
    completed_at TIMESTAMP DEFAULT NOW()
);

CREATE INDEX idx_history_request ON execution_history(request_id);
CREATE INDEX idx_history_completed ON execution_history(completed_at);
CREATE INDEX idx_history_worker ON execution_history(worker_id);
CREATE INDEX idx_history_checksum ON execution_history(wasm_checksum);

-- Worker heartbeats (monitoring)
CREATE TABLE worker_heartbeats (
    worker_id TEXT PRIMARY KEY,
    last_seen TIMESTAMP DEFAULT NOW(),
    version TEXT,
    active_tasks INT DEFAULT 0
);

-- Notes:
-- NO task_queue table - Redis handles this
-- PostgreSQL for metadata, analytics, and monitoring
-- Use connection pooling (sqlx with 10-20 connections)
```

#### Redis Data Structures

**Task Queue**:
```
LIST: "offchainvm:tasks"
Format: JSON-serialized Task enum
Operations:
  - LPUSH to add new tasks
  - BRPOP to claim tasks (blocking, no polling)
```

**Distributed Locks**:
```
KEY: "compilation:{repo}:{commit}"
VALUE: worker_id
TTL: 300 seconds (5 min)
Operations:
  - SET NX EX 300 to acquire lock
  - DEL to release lock
```

**Event Monitor Lock**:
```
KEY: "offchainvm:event_monitor:lock"
VALUE: worker_id
TTL: 60 seconds
Operations:
  - Only one worker holds this lock
  - Renewed every 30 seconds
```

---

### 4. Coordinator API Server

The Coordinator API Server is the central component that manages all state and coordinates workers.

#### 4.1 Project Structure
```
coordinator/
├── Cargo.toml
├── src/
│   ├── main.rs              # HTTP server entry point
│   ├── config.rs            # Configuration
│   ├── auth.rs              # Bearer token authentication middleware
│   ├── handlers/
│   │   ├── mod.rs
│   │   ├── tasks.rs         # Task queue endpoints
│   │   ├── wasm.rs          # WASM cache endpoints
│   │   └── locks.rs         # Distributed lock endpoints
│   ├── storage/
│   │   ├── mod.rs
│   │   ├── database.rs      # PostgreSQL operations
│   │   ├── redis.rs         # Redis operations
│   │   ├── wasm_cache.rs    # Local filesystem WASM cache with LRU
│   │   └── lru_eviction.rs  # LRU eviction logic
│   └── models.rs            # Request/response types
└── tests/
    └── integration_test.rs
```

#### 4.2 Technology Stack
```toml
[dependencies]
axum = "0.7"                    # HTTP framework
tokio = { version = "1", features = ["full"] }
tower = "0.4"                   # Middleware
tower-http = { version = "0.5", features = ["trace", "cors"] }
sqlx = { version = "0.7", features = ["postgres", "runtime-tokio-native-tls"] }
redis = { version = "0.24", features = ["tokio-comp"] }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
tracing = "0.1"
tracing-subscriber = "0.3"
sha2 = "0.10"                   # Token hashing
```

#### 4.3 API Server Configuration
```rust
// config.rs
pub struct Config {
    // HTTP server
    pub host: String,               // "0.0.0.0"
    pub port: u16,                  // 8080

    // PostgreSQL
    pub database_url: String,
    pub db_pool_size: u32,          // 50 connections

    // Redis
    pub redis_url: String,
    pub redis_task_queue: String,   // "offchainvm:tasks"

    // WASM cache
    pub wasm_cache_dir: PathBuf,    // "/var/offchainvm/wasm"
    pub wasm_cache_max_size_gb: u64, // 50 GB
    pub lru_eviction_check_interval_seconds: u64, // 3600 (1 hour)

    // Auth
    pub require_auth: bool,         // true in production

    // Timeouts
    pub task_poll_timeout_seconds: u64, // 60
    pub lock_default_ttl_seconds: u64,  // 300
}
```

#### 4.4 Authentication Middleware
```rust
// auth.rs

use axum::{
    extract::Request,
    http::{StatusCode, HeaderValue},
    middleware::Next,
    response::Response,
};
use sha2::{Sha256, Digest};

pub async fn auth_middleware(
    req: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let auth_header = req.headers()
        .get("Authorization")
        .and_then(|h| h.to_str().ok())
        .ok_or(StatusCode::UNAUTHORIZED)?;

    if !auth_header.starts_with("Bearer ") {
        return Err(StatusCode::UNAUTHORIZED);
    }

    let token = &auth_header[7..];

    // Hash token and check against database
    let mut hasher = Sha256::new();
    hasher.update(token.as_bytes());
    let token_hash = format!("{:x}", hasher.finalize());

    // Check in database (via extension or state)
    let db = req.extensions().get::<PgPool>()
        .ok_or(StatusCode::INTERNAL_SERVER_ERROR)?;

    let is_valid = sqlx::query!(
        "SELECT is_active FROM worker_auth_tokens WHERE token_hash = $1",
        token_hash
    )
    .fetch_optional(db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    .map(|r| r.is_active)
    .unwrap_or(false);

    if !is_valid {
        return Err(StatusCode::UNAUTHORIZED);
    }

    // Update last_used_at
    let _ = sqlx::query!(
        "UPDATE worker_auth_tokens SET last_used_at = NOW() WHERE token_hash = $1",
        token_hash
    )
    .execute(db)
    .await;

    Ok(next.run(req).await)
}
```

#### 4.5 Task Queue Handlers
```rust
// handlers/tasks.rs

use axum::{extract::State, Json, http::StatusCode};
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
pub struct PollTaskRequest {
    #[serde(default = "default_timeout")]
    timeout: u64,
}

fn default_timeout() -> u64 { 60 }

pub async fn poll_task(
    State(state): State<AppState>,
    Query(params): Query<PollTaskRequest>,
) -> Result<Json<Task>, StatusCode> {
    // Perform BRPOP on Redis (blocking pop with timeout)
    let timeout = params.timeout.min(120); // Max 2 minutes

    let result: Option<String> = state.redis
        .brpop(&state.config.redis_task_queue, timeout as usize)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    match result {
        Some(json) => {
            let task: Task = serde_json::from_str(&json)
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
            Ok(Json(task))
        }
        None => {
            // Timeout - no tasks available
            Err(StatusCode::NO_CONTENT)
        }
    }
}

#[derive(Deserialize)]
pub struct CompleteTaskRequest {
    request_id: u64,
    success: bool,
    output: Option<Vec<u8>>,
    error: Option<String>,
    execution_time_ms: u64,
}

pub async fn complete_task(
    State(state): State<AppState>,
    Json(payload): Json<CompleteTaskRequest>,
) -> StatusCode {
    // Store result in database
    let result = sqlx::query!(
        "UPDATE execution_requests SET status = 'completed', updated_at = NOW() WHERE request_id = $1",
        payload.request_id as i64
    )
    .execute(&state.db)
    .await;

    match result {
        Ok(_) => StatusCode::OK,
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR,
    }
}
```

#### 4.6 WASM Cache Handlers
```rust
// handlers/wasm.rs

use axum::{
    extract::{Path, State, Multipart},
    http::StatusCode,
    response::IntoResponse,
};
use tokio::fs;

pub async fn get_wasm(
    State(state): State<AppState>,
    Path(checksum): Path<String>,
) -> Result<impl IntoResponse, StatusCode> {
    let wasm_path = state.config.wasm_cache_dir.join(format!("{}.wasm", checksum));

    // Update last_accessed_at in database
    let _ = sqlx::query!(
        "UPDATE wasm_cache SET last_accessed_at = NOW(), access_count = access_count + 1 WHERE checksum = $1",
        checksum
    )
    .execute(&state.db)
    .await;

    // Read and return file
    let bytes = fs::read(wasm_path)
        .await
        .map_err(|_| StatusCode::NOT_FOUND)?;

    Ok((StatusCode::OK, bytes))
}

pub async fn upload_wasm(
    State(state): State<AppState>,
    mut multipart: Multipart,
) -> StatusCode {
    let mut checksum = String::new();
    let mut repo_url = String::new();
    let mut commit_hash = String::new();
    let mut wasm_bytes: Option<Vec<u8>> = None;

    while let Some(field) = multipart.next_field().await.unwrap() {
        let name = field.name().unwrap().to_string();
        match name.as_str() {
            "checksum" => checksum = field.text().await.unwrap(),
            "repo_url" => repo_url = field.text().await.unwrap(),
            "commit_hash" => commit_hash = field.text().await.unwrap(),
            "wasm_file" => wasm_bytes = Some(field.bytes().await.unwrap().to_vec()),
            _ => {}
        }
    }

    let wasm_bytes = match wasm_bytes {
        Some(b) => b,
        None => return StatusCode::BAD_REQUEST,
    };

    // Save to filesystem
    let wasm_path = state.config.wasm_cache_dir.join(format!("{}.wasm", checksum));
    if let Err(_) = fs::write(&wasm_path, &wasm_bytes).await {
        return StatusCode::INTERNAL_SERVER_ERROR;
    }

    // Insert metadata into database
    let file_size = wasm_bytes.len() as i64;
    let result = sqlx::query!(
        "INSERT INTO wasm_cache (checksum, repo_url, commit_hash, file_size) VALUES ($1, $2, $3, $4)
         ON CONFLICT (checksum) DO UPDATE SET last_accessed_at = NOW()",
        checksum, repo_url, commit_hash, file_size
    )
    .execute(&state.db)
    .await;

    // Check if LRU eviction is needed
    state.wasm_cache.check_and_evict().await;

    match result {
        Ok(_) => StatusCode::CREATED,
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR,
    }
}
```

#### 4.7 LRU Eviction Logic
```rust
// storage/lru_eviction.rs

pub struct LruEviction {
    db: PgPool,
    wasm_cache_dir: PathBuf,
    max_size_bytes: u64,
}

impl LruEviction {
    pub async fn check_and_evict(&self) -> Result<()> {
        // Calculate current cache size
        let current_size: i64 = sqlx::query_scalar!(
            "SELECT COALESCE(SUM(file_size), 0) FROM wasm_cache"
        )
        .fetch_one(&self.db)
        .await?;

        if current_size as u64 <= self.max_size_bytes {
            return Ok(());
        }

        // Need to evict - get LRU items
        let bytes_to_free = current_size as u64 - self.max_size_bytes;
        let mut bytes_freed = 0u64;

        let lru_items = sqlx::query!(
            "SELECT checksum, file_size FROM wasm_cache ORDER BY last_accessed_at ASC"
        )
        .fetch_all(&self.db)
        .await?;

        for item in lru_items {
            if bytes_freed >= bytes_to_free {
                break;
            }

            // Delete from filesystem
            let wasm_path = self.wasm_cache_dir.join(format!("{}.wasm", item.checksum));
            let _ = tokio::fs::remove_file(wasm_path).await;

            // Delete from database
            sqlx::query!("DELETE FROM wasm_cache WHERE checksum = $1", item.checksum)
                .execute(&self.db)
                .await?;

            bytes_freed += item.file_size as u64;
            info!("Evicted WASM cache: {}", item.checksum);
        }

        info!("LRU eviction freed {} bytes", bytes_freed);
        Ok(())
    }

    pub async fn run_periodic_check(&self, interval: Duration) {
        loop {
            tokio::time::sleep(interval).await;
            if let Err(e) = self.check_and_evict().await {
                error!("LRU eviction error: {}", e);
            }
        }
    }
}
```

#### 4.8 Main Server
```rust
// main.rs

#[tokio::main]
async fn main() -> Result<()> {
    let config = Config::from_env()?;

    // Initialize database pool
    let db = PgPoolOptions::new()
        .max_connections(config.db_pool_size)
        .connect(&config.database_url)
        .await?;

    // Initialize Redis client
    let redis = redis::Client::open(config.redis_url.clone())?;

    // Initialize LRU eviction
    let lru_eviction = LruEviction::new(
        db.clone(),
        config.wasm_cache_dir.clone(),
        config.wasm_cache_max_size_gb * 1024 * 1024 * 1024,
    );

    // Start LRU eviction background task
    let eviction_interval = Duration::from_secs(config.lru_eviction_check_interval_seconds);
    tokio::spawn(lru_eviction.clone().run_periodic_check(eviction_interval));

    // Build API router
    let app = Router::new()
        .route("/tasks/poll", get(handlers::tasks::poll_task))
        .route("/tasks/complete", post(handlers::tasks::complete_task))
        .route("/tasks/fail", post(handlers::tasks::fail_task))
        .route("/wasm/:checksum", get(handlers::wasm::get_wasm))
        .route("/wasm/upload", post(handlers::wasm::upload_wasm))
        .route("/wasm/exists/:checksum", get(handlers::wasm::wasm_exists))
        .route("/locks/acquire", post(handlers::locks::acquire_lock))
        .route("/locks/release/:lock_key", delete(handlers::locks::release_lock))
        .layer(axum::middleware::from_fn(auth::auth_middleware))
        .with_state(AppState { db, redis, config, lru_eviction });

    // Start HTTP server
    let addr = format!("{}:{}", config.host, config.port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;

    info!("Coordinator API server listening on {}", addr);
    axum::serve(listener, app).await?;

    Ok(())
}
```

---

### 5. Testing Strategy

#### 4.1 Unit Tests
- Each module has comprehensive unit tests
- Mock external dependencies (NEAR RPC, Docker, DB)
- Test error handling and edge cases

#### 4.2 Integration Tests
- End-to-end flow with real NEAR testnet
- Real Docker compilation
- Real WASM execution

#### 4.3 Load Testing
- Simulate 100+ concurrent requests
- Measure throughput (requests/sec)
- Identify bottlenecks

#### 4.4 Security Testing
- Malicious WASM (infinite loops, OOM)
- Malicious git repos (large files, fork bombs)
- Docker escape attempts

---

### 5. Test WASM Project

#### 5.1 Random Number Generator (Test Project)

This project will be used to test the entire flow end-to-end.

**GitHub Repository**: `https://github.com/YOUR_USERNAME/offchainvm-test-random`

**Project Structure**:
```
offchainvm-test-random/
├── Cargo.toml
├── src/
│   └── lib.rs
└── README.md
```

**Cargo.toml**:
```toml
[package]
name = "offchainvm-test-random"
version = "0.1.0"
edition = "2021"

[lib]
crate-type = ["cdylib"]

[dependencies]
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
getrandom = { version = "0.2", features = ["wasi"] }
```

**src/lib.rs**:
```rust
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
struct Input {
    #[serde(default = "default_min")]
    min: u32,
    #[serde(default = "default_max")]
    max: u32,
}

fn default_min() -> u32 { 0 }
fn default_max() -> u32 { 9 }

#[derive(Serialize)]
struct Output {
    random_number: u32,
}

#[no_mangle]
pub extern "C" fn execute() -> *const u8 {
    // Read input from environment variable (passed by worker)
    let input_json = std::env::var("INPUT").unwrap_or_else(|_| "{}".to_string());

    let input: Input = serde_json::from_str(&input_json)
        .unwrap_or(Input { min: 0, max: 9 });

    // Validate range
    let (min, max) = if input.min <= input.max {
        (input.min, input.max)
    } else {
        (input.max, input.min)
    };

    // Generate random number
    let mut buf = [0u8; 4];
    getrandom::getrandom(&mut buf).expect("Failed to generate random number");
    let random_value = u32::from_le_bytes(buf);

    let random_number = if max > min {
        min + (random_value % (max - min + 1))
    } else {
        min
    };

    // Return result as JSON
    let output = Output { random_number };
    let json = serde_json::to_string(&output).unwrap();

    // Allocate string in WASM memory, return pointer
    let bytes = json.into_bytes();
    let ptr = bytes.as_ptr();
    std::mem::forget(bytes);
    ptr
}
```

**Build Instructions**:
```bash
# Install wasm32-wasi target
rustup target add wasm32-wasi

# Build
cargo build --release --target wasm32-wasi

# Output: target/wasm32-wasi/release/offchainvm_test_random.wasm
```

**Usage Examples**:
- No arguments: Returns random number 0-9
- `{"min": 1, "max": 100}`: Returns random number 1-100
- `{"max": 50}`: Returns random number 0-50
- `{"min": 10, "max": 5}`: Auto-corrects to 5-10

**Testing Locally**:
```bash
# Run with wasmi (same as worker uses)
wasmi target/wasm32-wasi/release/offchainvm_test_random.wasm
```

---

### 6. Deployment

#### 6.1 Contract Deployment Script

**File**: `scripts/deploy_testnet.sh`

```bash
#!/bin/bash
set -e

# Configuration
CONTRACT_NAME="offchainvm.testnet"
OWNER_ID="alice.testnet"
OPERATOR_ID="worker.testnet"
NETWORK="testnet"

echo "Building contract..."
cd contract
cargo near build --release

echo "Deploying contract to $CONTRACT_NAME..."

# Deploy contract
near contract deploy $CONTRACT_NAME \
    use-file target/near/offchainvm.wasm \
    without-init-call \
    network-config $NETWORK \
    sign-with $OWNER_ID

echo "Initializing contract..."
near contract call-function as-transaction $CONTRACT_NAME new json-args \
    "{\"owner_id\": \"$OWNER_ID\", \"operator_id\": \"$OPERATOR_ID\"}" \
    prepaid-gas 100.0 Tgas \
    attached-deposit 0 NEAR \
    sign-as $OWNER_ID \
    network-config $NETWORK

echo "Setting pricing..."
near contract call-function as-transaction $CONTRACT_NAME set_pricing json-args \
    '{
        "base_fee": "100000000000000000000000",
        "per_instruction_fee": "1000000000000000",
        "per_mb_fee": "10000000000000000000",
        "per_second_fee": "10000000000000000000000"
    }' \
    prepaid-gas 10.0 Tgas \
    attached-deposit 0 NEAR \
    sign-as $OWNER_ID \
    network-config $NETWORK

echo "Contract deployed and configured!"
echo "Contract ID: $CONTRACT_NAME"
echo ""
echo "To view contract state:"
echo "  near contract call-function as-read-only $CONTRACT_NAME get_pricing json-args {} network-config $NETWORK"
```

**Make executable**:
```bash
chmod +x scripts/deploy_testnet.sh
```

**Run deployment**:
```bash
./scripts/deploy_testnet.sh
```

#### 6.2 Infrastructure Setup (Required)

**PostgreSQL**:
```bash
# Install PostgreSQL
apt-get install postgresql-14

# Create database
sudo -u postgres createdb offchainvm

# Create user
sudo -u postgres psql -c "CREATE USER offchainvm WITH PASSWORD 'secure_password';"
sudo -u postgres psql -c "GRANT ALL PRIVILEGES ON DATABASE offchainvm TO offchainvm;"

# Load schema
psql -U offchainvm -d offchainvm < worker/schema.sql
```

**Redis**:
```bash
# Install Redis
apt-get install redis-server

# Configure Redis for persistence
echo "appendonly yes" >> /etc/redis/redis.conf
systemctl restart redis
```

**MinIO (S3-compatible storage)**:
```bash
# Install MinIO (or use AWS S3/Cloudflare R2)
wget https://dl.min.io/server/minio/release/linux-amd64/minio
chmod +x minio
./minio server /mnt/data --console-address ":9001"

# Create bucket
mc alias set myminio http://localhost:9000 minioadmin minioadmin
mc mb myminio/offchainvm-wasm
```

#### 6.3 Worker Deployment

**Build**:
```bash
cd worker
cargo build --release
```

**Configuration** (`config.toml` or environment variables):
```bash
# NEAR
export NEAR_RPC_URL=https://rpc.testnet.near.org
export OFFCHAINVM_CONTRACT_ID=offchainvm.testnet
export OPERATOR_ACCOUNT_ID=worker.testnet
export OPERATOR_PRIVATE_KEY=ed25519:...

# PostgreSQL
export DATABASE_URL=postgres://offchainvm:secure_password@localhost/offchainvm
export DB_POOL_SIZE=20

# Redis
export REDIS_URL=redis://localhost:6379
export REDIS_TASK_QUEUE=offchainvm:tasks

# S3 (MinIO or AWS)
export S3_ENDPOINT=http://localhost:9000
export S3_BUCKET=offchainvm-wasm
export S3_REGION=us-east-1
export S3_ACCESS_KEY=minioadmin
export S3_SECRET_KEY=minioadmin

# Worker identity (unique per instance)
export WORKER_ID=$(hostname)-$(date +%s)

# Docker
export DOCKER_IMAGE=rust:1.75-slim

# Execution
export MAX_CONCURRENT_EXECUTIONS=4
export COMPILE_TIMEOUT_SECONDS=300
```

**Run worker**:
```bash
./target/release/offchainvm-worker
```

**Run multiple workers** (on different machines):
```bash
# Worker 1
export WORKER_ID=worker-1
./target/release/offchainvm-worker &

# Worker 2
export WORKER_ID=worker-2
./target/release/offchainvm-worker &

# Worker 3
export WORKER_ID=worker-3
./target/release/offchainvm-worker &

# All workers share: PostgreSQL, Redis, S3
# Only ONE worker holds event_monitor lock at a time
# All workers process tasks from shared Redis queue
```

#### 6.4 Docker Setup
```dockerfile
# docker/compiler.Dockerfile
FROM rust:1.75-slim

RUN apt-get update && apt-get install -y git && rm -rf /var/lib/apt/lists/*
RUN rustup target add wasm32-wasi

WORKDIR /workspace
CMD ["bash"]
```

---

## Phase 2: TEE Integration (Phala Network)

### 6.1 Phala Integration Architecture

**Key Changes from MVP**:
1. Worker runs inside Phala's Pink Runtime (confidential smart contract)
2. Keypair generated inside TEE, public key registered on-chain
3. Secrets encrypted with TEE public key
4. Execution produces attestation report
5. Contract verifies attestation before accepting results

### 6.2 Phala-Specific Development

#### Worker as Phala Fat Contract
```rust
// worker/src/pink_contract.rs

#[pink::contract]
mod offshore_worker {
    use pink_extension as pink;

    #[ink(storage)]
    pub struct OffshoreWorker {
        keypair: [u8; 64],  // Stored in encrypted TEE memory
        near_rpc_url: String,
        offchainvm_contract_id: String,
    }

    impl OffshoreWorker {
        #[ink(constructor)]
        pub fn new() -> Self {
            // Generate keypair inside TEE
            let keypair = pink::ext().getrandom(64);

            Self {
                keypair,
                near_rpc_url: String::from("https://rpc.mainnet.near.org"),
                offchainvm_contract_id: String::new(),
            }
        }

        #[ink(message)]
        pub fn get_public_key(&self) -> Vec<u8> {
            // Return public key (first 32 bytes)
            self.keypair[..32].to_vec()
        }

        #[ink(message)]
        pub fn execute_wasm(
            &self,
            request_id: u64,
            encrypted_secrets: Vec<(String, Vec<u8>)>,
            wasm_bytes: Vec<u8>,
            resource_limits: ResourceLimits,
        ) -> ExecutionResult {
            // Decrypt secrets inside TEE
            let secrets = self.decrypt_secrets(&encrypted_secrets);

            // Set environment variables
            for (key, value) in secrets {
                pink::ext().set_env(&key, &value);
            }

            // Execute WASM (same logic as Phase 1)
            let result = self.execute_wasm_internal(&wasm_bytes, resource_limits);

            // Generate attestation
            let attestation = pink::ext().sign_attestation(&result);

            ExecutionResult {
                ...result,
                attestation: Some(attestation),
            }
        }
    }
}
```

#### Contract Updates for Attestation Verification
```rust
// contract/src/lib.rs

pub struct OffshoreContract {
    // ... existing fields

    // TEE public keys (operator can register multiple workers)
    tee_public_keys: UnorderedSet<PublicKey>,
}

#[near_bindgen]
impl OffshoreContract {
    pub fn register_tee_worker(&mut self, public_key: PublicKey, attestation: AttestationReport) {
        self.assert_owner();

        // Verify attestation (basic check - could integrate with Phala's verification)
        assert!(self.verify_attestation(&attestation), "Invalid attestation");

        self.tee_public_keys.insert(&public_key);

        log!("Registered TEE worker with public key: {}", public_key);
    }

    pub fn resolve_execution(
        &mut self,
        data_id: CryptoHash,
        result: ExecutionResult,
    ) {
        // In Phase 2, verify attestation
        if let Some(attestation) = &result.attestation {
            assert!(
                self.verify_result_attestation(&result, attestation),
                "Invalid TEE attestation"
            );
        }

        // ... rest of resolve logic
    }
}
```

---

## Development Phases

### Phase 1: MVP (No TEE, Multi-Worker)
**Core Features**:
- ✅ Smart contract with yield/resume
- ✅ Multi-worker architecture from day 1
- ✅ PostgreSQL for metadata & analytics
- ✅ Redis for task queue & coordination
- ✅ S3-compatible cache (MinIO/AWS/R2)
- ✅ Worker with compilation + execution
- ✅ Event monitoring (distributed lock)
- ✅ WASM caching (shared S3)
- ✅ Resource limits (instruction metering, timeout, memory)
- ✅ Payment handling
- ✅ Basic security (Docker sandboxing)

**Trust Model**: Users trust operator to execute correctly
**Scalability**: Ready for 100+ workers immediately

### Phase 2: TEE Integration
**Additional Features**:
- ✅ Worker runs in Phala TEE
- ✅ Keypair generated inside TEE
- ✅ Secret encryption/decryption in TEE
- ✅ Attestation report generation
- ✅ On-chain attestation verification

**Trust Model**: Users trust Phala hardware + open-source code

---

## Key Decisions

### Why Rust for Worker?
- ✅ Performance (critical for WASM execution)
- ✅ Safety (memory safety, no GC pauses)
- ✅ Ecosystem (wasmi, near-sdk, tokio)
- ✅ WASM compatibility (can compile worker itself to WASM for Phala)

### Why wasmi over wasmtime?
- ✅ Simpler (pure Rust, no C dependencies)
- ✅ Better control (instruction metering built-in)
- ✅ Smaller binary (easier to deploy in TEE)
- ❌ Slower (but acceptable for MVP)

### Why PostgreSQL + Redis + S3 from Day 1?

**PostgreSQL (Metadata & Analytics)**:
- ✅ ACID guarantees (reliable metadata)
- ✅ Complex queries (analytics, debugging)
- ✅ JSONB for flexible schema
- ✅ Connection pooling (10-20 per worker)
- ✅ Perfect for multi-worker coordination
- ❌ NOT for task queue (too slow for high-frequency operations)

**Redis (Task Queue & Locks)**:
- ✅ Extremely fast (in-memory)
- ✅ `BRPOP` blocks workers (no polling!)
- ✅ Atomic operations (no race conditions)
- ✅ Distributed locks with TTL
- ✅ Handles worker crashes gracefully (lock expiration)
- ✅ Pub/sub for real-time coordination

**S3 (Shared WASM Cache)**:
- ✅ Content-addressed: `s3://bucket/wasm/{checksum}.wasm`
- ✅ Workers share cache (no duplication)
- ✅ First worker compiles, others download
- ✅ Idempotent uploads (same content = same file)
- ✅ No local disk needed
- ✅ Infinite scalability

**Why not SQLite?**
- ❌ File locking doesn't scale beyond 1-2 workers
- ❌ SKIP LOCKED not available
- ❌ Not designed for distributed systems
- ❌ Would need major rewrite for multi-worker

**Architecture decision**: Build for scale from day 1, avoid rewrite later

### Why Docker for Compilation?
- ✅ Strong isolation (no network, limited resources)
- ✅ Standard (any language can be compiled)
- ✅ Reproducible (Dockerfile defines environment)
- ❌ Overhead (but acceptable for async compilation)

---

## Monitoring & Observability

### Metrics to Track
- Requests per minute
- Compilation success rate
- Execution success rate
- Average execution time
- Resource usage (CPU, memory, disk)
- Cache hit rate
- Task queue depth

### Logging
- Structured logging with `tracing`
- Logs to stdout (captured by systemd/docker)
- Error tracking (Sentry integration optional)

### Dashboards
- Grafana for metrics visualization
- Prometheus for metrics collection
- PostgreSQL queries for historical data

---

## Security Considerations (MVP)

### Compilation Sandboxing
- ✅ Docker with `--network=none`
- ✅ CPU and memory limits
- ✅ Timeout enforcement
- ✅ No privileged mode

### Execution Sandboxing
- ✅ WASI with minimal capabilities
- ✅ Instruction metering
- ✅ Memory limits
- ✅ Timeout enforcement
- ✅ No filesystem access
- ✅ No network access

### Operator Key Security
- 🔐 Store private key in environment variable or file
- 🔐 Use hardware wallet for mainnet (Ledger support)
- 🔐 Rotate keys periodically
- ⚠️ Phase 1: Operator can see execution (no confidentiality)
- ✅ Phase 2: TEE provides confidentiality

---

## Estimated Complexity

### Smart Contract: **Medium**
- ~500-800 lines of Rust
- Standard yield/resume pattern (reuse existing code)
- Payment logic straightforward

### Worker: **High**
- ~2000-3000 lines of Rust
- Complex async coordination
- Docker integration non-trivial
- wasmi integration requires learning

### Testing: **Medium**
- Unit tests straightforward
- Integration tests require testnet setup
- Security testing requires manual effort

### Total Effort (MVP): **4-6 weeks for 1 experienced Rust developer**

### Phase 2 (Phala): **+2-3 weeks**
- Learning Phala SDK
- Porting worker to Pink Runtime
- Attestation integration

---

## Success Criteria

### MVP Ready When:
- ✅ 10+ successful end-to-end executions on testnet
- ✅ Compilation works for public GitHub repos
- ✅ Execution handles timeouts gracefully
- ✅ Resource limits enforced (no infinite loops)
- ✅ Payment logic works correctly
- ✅ Cache improves performance (2nd execution is instant)
- ✅ Documentation complete (README, API docs)

### Production Ready When:
- ✅ Security audit passed
- ✅ 99%+ uptime over 1 week
- ✅ Load tested (100+ concurrent requests)
- ✅ Monitoring dashboards set up
- ✅ On-call procedures documented
- ✅ Disaster recovery plan

---

## Next Steps

### Preparation
1. **Set up development environment**
   - Install Rust, Docker, SQLite
   - Create testnet accounts (owner, operator)
   - Set up project structure

2. **Create test WASM project**
   - Create `offchainvm-test-random` GitHub repo
   - Implement random number generator
   - Build and test locally with wasmi
   - Push to GitHub (will be used for E2E testing)

### Development Iterations

3. **Iteration 1: Smart Contract**
   - Write core structs (`OffshoreContract`, `ExecutionRequest`, etc.)
   - Implement `execute()` with yield
   - Implement `resolve_execution()` with resume
   - Write unit tests
   - Deploy to testnet using `scripts/deploy_testnet.sh`

4. **Iteration 2: Worker - Event Monitoring**
   - Set up project structure
   - Implement config loading
   - Implement event monitor (neardata.xyz polling)
   - Parse `ExecutionRequest` events
   - Store in SQLite task queue
   - Log events, don't process yet

5. **Iteration 3: Worker - Compilation**
   - Implement Docker-based compiler
   - Clone GitHub repo (validate public)
   - Build WASM in sandboxed Docker
   - Store in local cache or S3
   - Update task status to "compiled"

6. **Iteration 4: Worker - Execution**
   - Implement wasmi executor with instruction metering
   - Load WASM from cache
   - Execute with resource limits
   - Capture return value and metrics
   - Handle timeouts and OOM

7. **Iteration 5: Worker - Resume**
   - Implement NEAR client
   - Call `resolve_execution()` on contract
   - Pass `ExecutionResult` with metrics
   - Handle success and error cases

### Testing & Validation

8. **End-to-end testing**
   - Deploy test WASM project to GitHub
   - Call `execute()` from testnet account
   - Verify compilation happens
   - Verify execution succeeds
   - Verify result returns to contract
   - Test cache hit (2nd execution is instant)

9. **Security testing**
   - Test infinite loop (instruction metering stops it)
   - Test OOM (memory limit enforced)
   - Test timeout (process killed)
   - Test malicious Docker (network disabled)

10. **Performance testing**
    - Measure compilation time (3-5 min expected)
    - Measure execution time (<10s expected)
    - Test concurrent requests
    - Measure cache hit rate

### Production

11. **Production deployment**
    - Deploy contract to mainnet
    - Configure worker for mainnet
    - Set up monitoring (logs, metrics)
    - Document operational procedures

12. **Phase 2: Phala integration**
    - Learn Phala Pink Runtime
    - Port worker to Phala
    - Add attestation generation
    - Add attestation verification in contract
    - Test and deploy
