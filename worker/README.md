# OffchainVM Worker

Worker implementation for executing off-chain computations triggered by NEAR smart contracts.

## Features

- **Task Polling**: Long-polling from Coordinator API for new tasks
- **GitHub Compilation**: Clones and compiles GitHub repositories to WASM (Docker-sandboxed)
- **WASM Execution**: Executes WASM with resource limits using wasmi
- **NEAR Integration**: Submits execution results back to blockchain via `resolve_execution`
- **Event Monitoring** (optional): Monitors NEAR blockchain for `execution_requested` events
- **Distributed Locking**: Prevents duplicate compilation across multiple workers

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                        Worker Process                         │
├─────────────────────────────────────────────────────────────┤
│                                                               │
│  ┌──────────────┐    ┌────────────────┐    ┌─────────────┐ │
│  │Event Monitor │───▶│ Coordinator API│◀───│ Main Loop   │ │
│  │(optional)    │    │   Client       │    │(long-poll)  │ │
│  └──────────────┘    └────────────────┘    └─────────────┘ │
│         │                    │                      │        │
│         │                    ▼                      ▼        │
│         │            ┌──────────────┐      ┌──────────────┐ │
│         │            │  Compiler    │      │  Executor    │ │
│         │            │  (Docker)    │      │  (wasmi)     │ │
│         │            └──────────────┘      └──────────────┘ │
│         │                                          │         │
│         └──────────────────────────────────────────┼────────┐│
│                                                     ▼        ││
│                                            ┌──────────────┐ ││
│                                            │ NEAR Client  │ ││
│                                            │(resolve tx)  │ ││
│                                            └──────────────┘ ││
└─────────────────────────────────────────────────────────────┘│
                                                      │         │
                                                      ▼         │
                                            ┌──────────────────┐│
                                            │  NEAR Blockchain ││
                                            │  (testnet/main)  ││
                                            └──────────────────┘│
                                                                 │
                                                                 ▼
```

## Components

### 1. Config (`src/config.rs`)
Loads configuration from environment variables:
- Coordinator API credentials
- NEAR RPC endpoint and operator keys
- Docker compilation settings
- Default resource limits

### 2. API Client (`src/api_client.rs`)
HTTP client for Coordinator API:
- `poll_task()` - Long-poll for new tasks (compile or execute)
- `complete_task()` / `fail_task()` - Report task status
- `upload_wasm()` / `download_wasm()` - WASM cache management
- `acquire_lock()` / `release_lock()` - Distributed locking for compilation

### 3. Compiler (`src/compiler.rs`)
Compiles GitHub repositories to WASM:
- Checks WASM cache before compiling
- Acquires distributed lock to prevent duplicate work
- Runs compilation in Docker sandbox (TODO: implement Docker integration)
- Uploads compiled WASM to coordinator

### 4. Executor (`src/executor.rs`)
Executes WASM with resource metering:
- Uses wasmi engine with fuel metering (instruction counting)
- Enforces memory and time limits
- Provides minimal WASI interface
- Returns execution results with timing metrics

### 5. NEAR Client (`src/near_client.rs`)
Interacts with NEAR blockchain:
- Calls `resolve_execution(data_id, response)` on OffchainVM contract
- Handles transaction signing and submission
- Waits for finalization

### 6. Event Monitor (`src/event_monitor.rs`) (Optional)
Monitors blockchain for events:
- Polls NEAR RPC for new blocks
- Parses `EVENT_JSON` logs for `execution_requested` events
- Creates tasks in Coordinator API
- Alternative to relying solely on contract-side task creation

### 7. Main Loop (`src/main.rs`)
Orchestrates all components:
1. Poll for task from Coordinator API
2. If compile task: compile and upload WASM
3. If execute task: download WASM, execute, submit result to NEAR
4. Repeat

## Setup

### Prerequisites

- Rust 1.75+ with `wasm32-unknown-unknown` target
- Docker (for compilation sandbox)
- NEAR account with operator role on OffchainVM contract
- Access to Coordinator API

### Installation

```bash
# Clone repository
cd worker

# Build worker
cargo build --release

# Create .env file (see Configuration below)
cp .env.example .env
nano .env

# Run worker
cargo run --release
```

## Configuration

Create a `.env` file with the following variables:

### Required

```bash
# Coordinator API
API_BASE_URL=http://localhost:8080
API_AUTH_TOKEN=your_secret_token_here

# NEAR Configuration
NEAR_RPC_URL=https://rpc.testnet.near.org
OFFCHAINVM_CONTRACT_ID=offchainvm.testnet
OPERATOR_ACCOUNT_ID=worker1.testnet
OPERATOR_PRIVATE_KEY=ed25519:YOUR_PRIVATE_KEY_HERE
```

### Optional (with defaults)

```bash
# Worker Settings
WORKER_ID=worker-1                    # default: random UUID
ENABLE_EVENT_MONITOR=false            # default: false
POLL_TIMEOUT_SECONDS=60               # default: 60

# Docker Compilation
DOCKER_IMAGE=rust:1.75                # default: rust:1.75
COMPILE_TIMEOUT_SECONDS=300           # default: 300 (5 minutes)
COMPILE_MEMORY_LIMIT_MB=2048          # default: 2048
COMPILE_CPU_LIMIT=2.0                 # default: 2.0

# WASM Execution Defaults
DEFAULT_MAX_INSTRUCTIONS=10000000000  # default: 10 billion
DEFAULT_MAX_MEMORY_MB=128             # default: 128 MB
DEFAULT_MAX_EXECUTION_SECONDS=60      # default: 60 seconds
```

## Usage

### Running the Worker

```bash
# Development mode (with logs)
RUST_LOG=offchainvm_worker=debug cargo run

# Production mode
cargo run --release
```

### Environment Variables for Logging

```bash
# Info level (default)
RUST_LOG=offchainvm_worker=info cargo run

# Debug level (verbose)
RUST_LOG=offchainvm_worker=debug cargo run

# Trace level (very verbose)
RUST_LOG=offchainvm_worker=trace cargo run
```

## Task Flow

### Compile Task

1. Receive `Task::Compile` from coordinator
2. Compute checksum for (repo, commit, build_target)
3. Check if WASM exists in cache → return if yes
4. Acquire distributed lock for this compilation
5. Clone GitHub repository (TODO: implement)
6. Run `cargo build --target wasm32-wasi` in Docker (TODO: implement)
7. Extract WASM binary
8. Upload to coordinator with checksum
9. Release lock
10. Complete task

### Execute Task

1. Receive `Task::Execute` from coordinator
2. Download WASM binary from cache using checksum
3. Fetch input data using data_id (TODO: implement proper data fetching)
4. Execute WASM with resource limits:
   - Fuel metering for instruction counting
   - Memory limit enforcement
   - Timeout enforcement
5. Collect execution metrics
6. Submit result to NEAR via `resolve_execution(data_id, response)`
7. Complete task in coordinator

## WASM Interface

Compiled WASM modules must export:

```rust
#[no_mangle]
pub extern "C" fn execute(input_ptr: i32, input_len: i32) -> i32 {
    // Read input data from memory
    // Perform computation
    // Write output to memory as: [4-byte length][output data]
    // Return pointer to output (or negative for error)
}
```

Memory layout:
- Input: written at offset 0 by executor
- Output: 4-byte little-endian length followed by data

## Security

### Sandboxing

- **Docker Isolation**: Compilation runs in isolated Docker containers
- **Resource Limits**: CPU, memory, and timeout constraints
- **WASM Sandbox**: Execution in wasmi with no access to host system
- **Fuel Metering**: Instruction-level resource tracking

### Authentication

- **API Token**: Bearer token authentication for Coordinator API
- **NEAR Keys**: Ed25519 private key for operator transactions
- **Distributed Locks**: Prevent race conditions in multi-worker setup

## Testing

```bash
# Run unit tests
cargo test

# Run with test configuration
API_BASE_URL=http://localhost:8080 \
API_AUTH_TOKEN=test-token \
NEAR_RPC_URL=https://rpc.testnet.near.org \
OFFCHAINVM_CONTRACT_ID=offchainvm.testnet \
OPERATOR_ACCOUNT_ID=test.testnet \
OPERATOR_PRIVATE_KEY=ed25519:... \
cargo run
```

## Development

### Adding New Features

1. **Config** - Add environment variable to `config.rs`
2. **API Client** - Add new endpoint methods to `api_client.rs`
3. **Compilation** - Extend `compiler.rs` with Docker integration
4. **Execution** - Extend `executor.rs` with new WASI functions
5. **NEAR** - Add contract methods to `near_client.rs`

### TODO

- [ ] Implement Docker-based GitHub compilation
- [ ] Implement proper input data fetching via data_id
- [ ] Add metrics collection and reporting
- [ ] Implement health checks and worker registration
- [ ] Add graceful shutdown handling
- [ ] Implement WASM caching on worker disk
- [ ] Add support for multiple WASM runtimes (wasmtime, wasmer)
- [ ] Implement full WASI support for more complex programs

## Troubleshooting

### Worker can't connect to Coordinator API

- Check `API_BASE_URL` is correct
- Verify coordinator is running: `curl http://localhost:8080/health`
- Check `API_AUTH_TOKEN` matches coordinator configuration

### NEAR transaction failures

- Verify `OPERATOR_ACCOUNT_ID` has operator role on contract
- Check `OPERATOR_PRIVATE_KEY` is valid and has sufficient balance
- Ensure contract ID is correct for your network (testnet/mainnet)
- Check RPC endpoint is responsive

### Compilation errors

- Verify Docker is running: `docker ps`
- Check Docker image is available: `docker pull rust:1.75`
- Ensure sufficient disk space for compilation
- Check GitHub repository is accessible

### Execution timeouts

- Increase `DEFAULT_MAX_EXECUTION_SECONDS` for longer computations
- Check WASM binary is optimized (release build)
- Verify resource limits are not too restrictive

## Production Deployment

### Systemd Service

Create `/etc/systemd/system/offchainvm-worker.service`:

```ini
[Unit]
Description=OffchainVM Worker
After=network.target docker.service
Requires=docker.service

[Service]
Type=simple
User=offchainvm
WorkingDirectory=/opt/offchainvm-worker
EnvironmentFile=/opt/offchainvm-worker/.env
ExecStart=/opt/offchainvm-worker/target/release/offchainvm-worker
Restart=always
RestartSec=10

[Install]
WantedBy=multi-user.target
```

Enable and start:

```bash
sudo systemctl enable offchainvm-worker
sudo systemctl start offchainvm-worker
sudo systemctl status offchainvm-worker
```

### Docker Deployment

```dockerfile
FROM rust:1.75 as builder
WORKDIR /app
COPY . .
RUN cargo build --release

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates libssl3 && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/offchainvm-worker /usr/local/bin/
CMD ["offchainvm-worker"]
```

Run with:

```bash
docker build -t offchainvm-worker .
docker run -d \
  --name worker1 \
  --env-file .env \
  -v /var/run/docker.sock:/var/run/docker.sock \
  offchainvm-worker
```

### Multi-Worker Setup

Run multiple workers for redundancy and load balancing:

```bash
# Worker 1
WORKER_ID=worker-1 cargo run --release &

# Worker 2
WORKER_ID=worker-2 cargo run --release &

# Worker 3
WORKER_ID=worker-3 cargo run --release &
```

Workers coordinate via distributed locks in Redis to avoid duplicate work.

## License

MIT
