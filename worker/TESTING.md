# Worker Testing Guide

Complete guide for testing the NEAR OutLayer Worker component.

## Prerequisites

- Docker installed and running
- PostgreSQL and Redis (via docker-compose in coordinator/)
- Coordinator API running on http://localhost:8080
- Rust 1.85+ with wasm32-wasip1 target

## Quick Test Suite

Run all tests:
```bash
# Full integration test
../scripts/test_worker_flow.sh

# Unit tests
cargo test
```

## Test 1: Coordinator + Worker Flow

### What It Tests
✅ Coordinator health check
✅ WASM upload/download
✅ Task creation
✅ Task polling
✅ Distributed locks

### Run Test
```bash
cd ../scripts
./test_worker_flow.sh
```

### Expected Output
```
🧪 Testing Worker + Coordinator Flow
====================================

📋 Test 1: Coordinator Health Check
✓ Coordinator is healthy

📋 Test 2: Upload WASM File
✓ WASM uploaded successfully

📋 Test 3: Verify WASM Exists
✓ WASM exists in cache

📋 Test 4: Download WASM
✓ WASM downloaded successfully (113915 bytes)

📋 Test 5: Create Execution Task
✓ Task created successfully

📋 Test 6: Poll for Task
✓ Task received (request_id: 999)

📋 Test 7: Distributed Lock
✓ Lock acquired
✓ Lock released

====================================
✅ All tests passed!
```

### Troubleshooting
- **422 on task creation**: Missing `input_data` field in JSON
- **Connection refused**: Coordinator not running (start with `cd coordinator && cargo run`)
- **Redis connection failed**: Run `docker-compose up -d` in coordinator/

## Test 2: GitHub Compilation

### What It Tests
✅ Docker container creation
✅ GitHub repository cloning
✅ Rust toolchain installation
✅ WASM compilation in sandboxed environment
✅ WASM file extraction via tar streaming
✅ Magic number validation
✅ Checksum verification

### Run Test
```bash
# Quick script
./scripts/test_github_compilation.sh

# Or manually with cargo
cargo test test_real_github_compilation -- --ignored --nocapture
```

### Supported Build Targets
- ✅ `wasm32-wasip1` - WASI Preview 1 (recommended)
- ✅ `wasm32-wasi` - Legacy name (normalized to wasip1)
- ✅ `wasm32-wasip2` - WASI Preview 2 (for HTTP components)

### Expected Output
```
Compiling https://github.com/zavodil/random-ark @ 6491b31... for wasm32-wasi
Compiled WASM size: 113915 bytes
Compiled WASM checksum: ba2c7a75c93b7cd7bc3e2f7e12943ba2dacac6ea444f6a2e853023b892ca8acc
Expected WASM checksum: ba2c7a75c93b7cd7bc3e2f7e12943ba2dacac6ea444f6a2e853023b892ca8acc
✅ Compilation successful! Size difference: 0 bytes
```

### Troubleshooting
- **Docker not found**: Install Docker and start daemon
- **Permission denied**: Add user to docker group or use sudo
- **Network timeout**: Check GitHub access and Docker network settings
- **Compilation failed**: Check Docker logs with `docker logs <container_id>`

## Test 3: WASM Execution

### What It Tests
✅ WASI P1 module execution (wasm32-wasip1)
✅ WASI P2 component execution (wasm32-wasip2)
✅ Fuel metering (instruction counting)
✅ Input/output via stdin/stdout
✅ Resource limits enforcement
✅ Environment variables injection

### Preparation
```bash
# Build get-random example (WASI P1)
cd ../wasi-examples/get-random
cargo build --release --target wasm32-wasip1

# Build ai-ark example (WASI P2)
cd ../wasi-examples/ai-ark
cargo build --release --target wasm32-wasip2

# Return to worker
cd ../../worker
```

### Run Test
```bash
# Test WASI P1 execution
cargo test test_wasm_execution -- --nocapture

# Test with debug logging
RUST_LOG=offchainvm_worker=debug cargo test test_wasm_execution -- --nocapture
```

### Expected Output
```
✅ Loaded WASM: 111234 bytes
⚙️  Executing WASM...
✅ Execution result:
   Success: true
   Instructions consumed: 20351
   Time: 5ms
   Output: {"random_number":42}
```

### Execution Architecture

The worker supports three execution paths:

1. **WASI P2 Component** (priority 1)
   - Target: `wasm32-wasip2`
   - Runtime: wasmtime 28
   - Features: HTTP, filesystem, advanced I/O
   - Entry: Component model with `wasi:cli/run`

2. **WASI P1 Module** (priority 2)
   - Target: `wasm32-wasip1` or `wasm32-wasi`
   - Runtime: wasmtime 28
   - Features: Basic I/O, random, environment
   - Entry: `_start` function (from `main()`)

3. **Error** (fallback)
   - Returns error if binary is neither P2 component nor P1 module

### Troubleshooting
- **Empty output**: Missing `io::stdout().flush()` in WASM code
- **Failed to instantiate**: Wrong build target or missing `main()` function
- **No fuel consumed**: Fuel metering not enabled
- **HTTP request failed**: WASI P2 component needs network access (not available in test)

## Test 4: WASM Validation

Use the universal test runner to validate any WASM module:

```bash
cd ../wasi-examples/wasi-test-runner
cargo build --release

# Test WASI P1 module
./target/release/wasi-test \
  --wasm ../get-random/target/wasm32-wasip1/release/get-random-example.wasm \
  --input '{"min":1,"max":100}'

# Test WASI P2 component
./target/release/wasi-test \
  --wasm ../ai-ark/target/wasm32-wasip2/release/ai-ark.wasm \
  --input '{"prompt":"test"}'
```

See [wasi-examples/wasi-test-runner/README.md](../wasi-examples/wasi-test-runner/README.md) for details.

## Test 5: End-to-End Contract Flow

### Prerequisites
- Contract deployed to testnet: `outlayer.testnet`
- Operator account: `worker.testnet`
- Worker configured with operator keys
- Coordinator and Worker running

### Setup
```bash
# 1. Start coordinator
cd coordinator
cargo run

# 2. Start worker in another terminal
cd worker
RUST_LOG=info cargo run
```

### Run Test
```bash
# Request execution via contract
near call outlayer.testnet request_execution '{
  "code_source": {
    "type": "GitHub",
    "repo": "https://github.com/zavodil/random-ark",
    "commit": "main",
    "build_target": "wasm32-wasip1"
  },
  "resource_limits": {
    "max_instructions": 10000000,
    "max_memory_mb": 128,
    "max_execution_seconds": 60
  },
  "input_data": "{\"min\":1,\"max\":100}"
}' --accountId your.testnet --deposit 0.1
```

### Expected Worker Logs
```
INFO  Received task: Compile { request_id: 16, data_id: "...", ... }
INFO  🔨 Starting compilation for request_id=16
INFO  Checking if WASM exists in cache: checksum=ba2c7a75...
INFO  WASM already exists in cache
INFO  ✅ Compilation successful: checksum=ba2c7a75...
INFO  📥 Downloading compiled WASM: checksum=ba2c7a75...
INFO  ✅ Downloaded WASM: 113915 bytes
INFO  ⚙️  Executing WASM for request_id=16 (size=113915 bytes)
INFO  Loaded as WASI Preview 1 module
INFO  WASM execution consumed 20351 instructions
INFO  ✅ Execution completed: success=true, output={"random_number":42}
INFO  📤 Submitting result to NEAR contract
INFO  📡 Submitting execution result: data_id=..., success=true
INFO  Transaction: https://testnet.nearblocks.io/txns/...
INFO  ✅ Result submitted successfully
```

### Troubleshooting
- **Transaction rejected**: Check operator permissions on contract
- **Execution failed**: Check WASM module compatibility with `wasi-test-runner`
- **Timeout**: Increase resource limits or optimize WASM code
- **Wrong output format**: Ensure WASM writes JSON to stdout

## Test 6: Encrypted Secrets

### Prerequisites
- Keystore worker running on http://localhost:8081
- Secrets encrypted with `encrypt_secrets.py`

### Encrypt Secrets
```bash
cd ../keystore-worker
./scripts/encrypt_secrets.py '{"OPENAI_API_KEY":"sk-...","API_TOKEN":"secret123"}'
# Output: [123, 45, 67, ...]
```

### Test with Contract
```bash
near call outlayer.testnet request_execution '{
  "code_source": {
    "type": "GitHub",
    "repo": "https://github.com/user/ai-ark",
    "commit": "main",
    "build_target": "wasm32-wasip2"
  },
  "resource_limits": {
    "max_instructions": 100000000,
    "max_memory_mb": 128,
    "max_execution_seconds": 60
  },
  "input_data": "{\"prompt\":\"test\"}",
  "encrypted_secrets": [123, 45, 67, ...]
}' --accountId your.testnet --deposit 0.1
```

### Expected Logs
```
INFO  🔐 Decrypting secrets via keystore
INFO  ✅ Secrets decrypted and parsed: 2 keys
INFO  Added env var: OPENAI_API_KEY
INFO  Added env var: API_TOKEN
INFO  ⚙️  Executing WASM with environment variables
```

## Debugging

### Enable Debug Logging
```bash
# Debug level for worker
RUST_LOG=offchainvm_worker=debug cargo run

# Trace level for executor
RUST_LOG=offchainvm_worker::executor=trace cargo run

# All debug output
RUST_LOG=debug cargo run
```

### Inspect WASM Files
```bash
# Check binary type
file your-app.wasm

# Check size
ls -lh your-app.wasm

# Inspect with wasm-tools
wasm-tools print your-app.wasm | head -50

# For P2 components
wasm-tools component wit your-app.wasm
```

### Common Log Patterns

**Successful execution**:
```
✅ Compilation successful
✅ Downloaded WASM
✅ Execution completed: success=true
✅ Result submitted successfully
```

**Compilation failure**:
```
❌ Compilation failed: exit code 101
ERROR Cargo build failed
```

**Execution failure**:
```
❌ WASM execution failed: Failed to instantiate WASM module
ERROR Check that module is valid WASI P1 or P2
```

**Contract submission failure**:
```
❌ Failed to submit result to NEAR: Invalid signature
ERROR Check operator account permissions
```

## Performance Benchmarks

Expected metrics for test modules:

| Module | Size | Build Time | Execution Time | Instructions |
|--------|------|------------|----------------|--------------|
| get-random | ~111 KB | ~30s | ~5ms | ~20,000 |
| ai-ark (no HTTP) | ~500 KB | ~45s | ~10ms | ~8,000 |
| ai-ark (with HTTP) | ~500 KB | ~45s | ~2000ms | ~80,000 |

## CI/CD Integration

### GitHub Actions Example
```yaml
name: Test Worker

on: [push, pull_request]

jobs:
  test:
    runs-on: ubuntu-latest
    services:
      postgres:
        image: postgres:14
      redis:
        image: redis:7
    steps:
      - uses: actions/checkout@v3
      - uses: actions-rs/toolchain@v1
        with:
          toolchain: stable
      - name: Run tests
        run: cargo test
      - name: Run integration test
        run: ./scripts/test_worker_flow.sh
```

## Additional Resources

- [WASI Development Tutorial](../wasi-examples/WASI_TUTORIAL.md)
- [WASI Test Runner](../wasi-examples/wasi-test-runner/)
- [Worker Configuration](./README.md)
- [Coordinator API Docs](../coordinator/README.md)

---

**Last updated**: 2025-10-15
**Worker version**: MVP Phase 1
**Supported WASI**: Preview 1 & Preview 2
