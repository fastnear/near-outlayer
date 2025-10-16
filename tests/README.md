# NEAR Offshore Test Suite

Complete test suite for NEAR Offshore platform.

## Test Structure

```
tests/
├── unit.sh          - Unit tests (WASM execution, cargo test)
├── compilation.sh   - Compilation tests (GitHub → WASM via Docker)
├── integration.sh   - Integration tests (Coordinator + Worker API)
├── e2e.sh          - End-to-end tests (NEAR contract flow)
├── transactions.sh  - Transaction tests (real testnet execution)
└── run_all.sh      - Run all tests in sequence
```

## Quick Start

### Run All Tests

```bash
cd tests
./run_all.sh
```

### Run Individual Tests

```bash
# Unit tests only
./unit.sh

# Compilation tests (requires Docker)
./compilation.sh

# Integration tests (requires coordinator running)
./integration.sh

# End-to-end tests (requires testnet contract)
./e2e.sh
```

## Test 1: Unit Tests

**File**: `unit.sh`

**What it tests**:
- ✅ Building WASM test modules (get-random, ai-ark)
- ✅ Worker unit tests with cargo
- ✅ WASM execution functionality
- ✅ Fuel metering

**Prerequisites**:
- Rust 1.85+
- wasm32-wasip1 and wasm32-wasip2 targets installed

**Run**:
```bash
./unit.sh
```

**Expected output**:
```
🧪 Unit Tests - WASM Execution
===============================

🔨 Building test WASM modules...

📦 Building get-random (WASI P1)...
✓ get-random built successfully
📦 Building ai-ark (WASI P2)...
✓ ai-ark built successfully

🧪 Running worker unit tests...

✅ All unit tests passed!
```

## Test 2: Compilation Tests

**File**: `compilation.sh`

**What it tests**:
- ✅ Docker container creation
- ✅ GitHub repository cloning
- ✅ Rust toolchain installation
- ✅ WASM compilation from source
- ✅ WASM extraction via tar streaming
- ✅ Magic number validation
- ✅ Checksum verification

**Prerequisites**:
- Docker installed and running
- Network access to GitHub
- ~2GB free disk space for Docker images

**Run**:
```bash
./compilation.sh
```

**Expected output**:
```
🧪 Compilation Test - GitHub to WASM
=====================================

🔍 Checking prerequisites...
✓ Docker is running

📦 Testing compilation:
  Repository: https://github.com/zavodil/random-ark
  Commit: 6491b317afa33534b56cebe9957844e16ac720e8
  Target: wasm32-wasi

Compiling https://github.com/zavodil/random-ark @ 6491b31...
Compiled WASM size: 113915 bytes
Compiled WASM checksum: ba2c7a75...
Expected WASM checksum: ba2c7a75...
✅ Compilation successful! Size difference: 0 bytes

✅ Compilation test passed!
```

**Duration**: ~30-60 seconds (first run slower due to Docker image pull)

## Test 3: Integration Tests

**File**: `integration.sh`

**What it tests**:
- ✅ Coordinator health check
- ✅ WASM upload/download
- ✅ WASM cache verification
- ✅ Task creation via API
- ✅ Task polling
- ✅ Distributed locks

**Prerequisites**:
- Coordinator running on http://localhost:8080
- PostgreSQL and Redis (via docker-compose)
- Test WASM modules built (run `unit.sh` first)

**Setup**:
```bash
# Terminal 1: Start coordinator
cd coordinator
cargo run
```

**Run**:
```bash
# Terminal 2: Run tests
cd tests
./integration.sh
```

**Expected output**:
```
🧪 Integration Tests - Coordinator + Worker Flow
=================================================

📋 Test 1: Coordinator Health Check
✓ Coordinator is healthy

📋 Test 2: Upload WASM File
✓ WASM uploaded successfully

📋 Test 3: Verify WASM Exists
✓ WASM exists in cache

📋 Test 4: Download WASM
✓ WASM downloaded successfully (111234 bytes)

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

## Test 3: End-to-End Tests

**File**: `e2e.sh`

**What it tests**:
- ✅ Full flow from contract to result
- ✅ Event monitoring
- ✅ Compilation in Docker
- ✅ WASM execution
- ✅ Result submission to contract

**Prerequisites**:
- Contract deployed on testnet (`offchainvm.testnet`)
- NEAR CLI installed
- Coordinator running
- Worker running with valid operator keys
- Testnet account with NEAR tokens

**Setup**:
```bash
# Terminal 1: Start coordinator
cd coordinator
cargo run

# Terminal 2: Start worker
cd worker
RUST_LOG=info cargo run

# Terminal 3: Run test
cd tests
./e2e.sh
```

**Configuration** (optional environment variables):
```bash
# Use custom contract
CONTRACT_ID=mycontract.testnet ./e2e.sh

# Use custom caller account
CALLER_ACCOUNT=myaccount.testnet ./e2e.sh

# Custom payment amount
PAYMENT=0.5 ./e2e.sh
```

**Expected output**:
```
🧪 End-to-End Test - NEAR Contract Flow
========================================

📝 Configuration:
  Contract: c3.offchainvm.testnet
  Caller: offchainvm.testnet
  Payment: 0.1 NEAR
  Repo: https://github.com/zavodil/random-ark
  Commit: main

🔍 Checking prerequisites...
✓ NEAR CLI available

🚀 Sending execution request to contract...

Transaction sent successfully!

✅ Transaction sent!

📊 Next steps:
  1. Check worker logs for execution progress
  2. View result in NEAR Explorer
  3. Query contract state
```

**Worker logs to expect**:
```
INFO  Received task: Compile { request_id: 16, ... }
INFO  🔨 Starting compilation
INFO  ✅ Compilation successful
INFO  ⚙️  Executing WASM
INFO  ✅ Execution completed: success=true
INFO  📤 Submitting result to NEAR
INFO  ✅ Result submitted successfully
```

## Run All Tests

**File**: `run_all.sh`

Runs all tests in sequence:
1. Unit tests (always runs)
2. Compilation tests (skips if Docker not running)
3. Integration tests (skips if coordinator not running)
4. E2E tests (manual - requires testnet setup)

```bash
./run_all.sh
```

## Troubleshooting

### Unit Tests

**"Target not found: wasm32-wasip1"**
```bash
rustup target add wasm32-wasip1 wasm32-wasip2
```

**"Cargo build failed"**
```bash
# Clean and rebuild
cd wasi-examples/get-random
cargo clean
cargo build --release --target wasm32-wasip1
```

### Integration Tests

**"Connection refused"**
- Start coordinator: `cd coordinator && cargo run`

**"Redis connection failed"**
- Start services: `cd coordinator && docker-compose up -d`

**"Test WASM not found"**
- Build modules: `./unit.sh`

**"Task creation failed (HTTP 422)"**
- Check JSON format (must include `input_data` field)

### End-to-End Tests

**"NEAR CLI not found"**
```bash
npm install -g near-cli
```

**"Transaction rejected"**
- Check operator permissions on contract
- Verify worker has correct private keys

**"Worker not processing"**
- Check worker logs: `RUST_LOG=info cargo run`
- Verify worker .env configuration
- Check event monitor is enabled

**"Execution timeout"**
- Increase resource limits in request
- Check Docker compilation logs

## CI/CD Integration

### GitHub Actions Example

```yaml
name: Test Suite

on: [push, pull_request]

jobs:
  test:
    runs-on: ubuntu-latest
    services:
      postgres:
        image: postgres:14
        env:
          POSTGRES_PASSWORD: postgres
      redis:
        image: redis:7

    steps:
      - uses: actions/checkout@v3

      - uses: actions-rs/toolchain@v1
        with:
          toolchain: stable
          target: wasm32-wasip1

      - name: Run unit tests
        run: ./tests/unit.sh

      - name: Start coordinator
        run: |
          cd coordinator
          cargo run &
          sleep 5

      - name: Run integration tests
        run: ./tests/integration.sh
```

## Additional Resources

- [Worker Testing Guide](../worker/TESTING.md) - Detailed testing documentation
- [WASI Tutorial](../wasi-examples/WASI_TUTORIAL.md) - WASM module development
- [WASI Test Runner](../wasi-examples/wasi-test-runner/) - Module validation tool

## Test Development

### Adding New Tests

1. Create test script in `/tests/`
2. Make it executable: `chmod +x your_test.sh`
3. Follow naming convention: `test_*.sh`
4. Add to `run_all.sh` if appropriate
5. Document in this README

### Test Script Template

```bash
#!/bin/bash
# Description of what this test does

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

echo "🧪 Test Name"
echo "============"
echo ""

# Your test logic here

echo ""
echo "✅ Test passed!"
```

---

**Last updated**: 2025-10-15
**Test coverage**: Unit + Integration + E2E
**Platform**: NEAR Offshore MVP
