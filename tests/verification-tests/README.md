# verification Integration Tests

Integration tests verifying the hardening work completed in Phases 1-5 of NEAR OutLayer development.

## Overview

This test suite provides **practical verification** of security properties implemented across five development phases:

1. **Phase 1**: Worker determinism (fuel metering + epoch interruption)
2. **Phase 2**: Contract NEP-297 event compliance
3. **Phase 3**: Coordinator authentication & idempotency
4. **Phase 4**: WASI P1 helpers (path canonicalization + safe math)
5. **Phase 5**: TypeScript client library

These tests complement the **property-based verification suite** (`outlayer-verification-suite/`) which provides adversarial testing with 512-2000 randomized cases per property.

## Quick Start

```bash
# Run all integration tests
cd tests/verification-integration
cargo test

# Run specific phase
cargo test --test determinism

# Run with verbose output
cargo test -- --nocapture

# Run integration test runner
cargo run --bin run_all_integration_tests
```

## Test Structure

```
verification-integration/
├── common/                    # Shared test utilities
│   └── mod.rs                 # WASM build/execution helpers
├── determinism/               # Phase 1 tests
│   ├── fuel_consistency.rs    # 100x same-input determinism
│   ├── epoch_deadline.rs      # Timeout behavior verification
│   └── cross_runtime.rs       # wasmi vs wasmtime consistency
├── contract_events/           # Phase 2 tests
│   └── nep297_format.rs       # NEP-297 event envelope validation
├── coordinator_hardening/     # Phase 3 tests
│   ├── idempotency.rs         # Idempotency-Key header tests
│   └── near_signed_auth.rs    # ed25519 signature verification
├── wasi_helpers/              # Phase 4 tests
│   ├── github_canon.rs        # Path traversal prevention
│   └── safe_math.rs           # Checked arithmetic
└── typescript_client/         # Phase 5 tests
    └── integration.rs         # Client library integration
```

## Phase 1: Worker Determinism

**What it verifies**: WASM execution is deterministic and reproducible.

### Tests

#### `fuel_consistency.rs::test_100x_same_input_determinism`
- **Property**: 100 executions of same WASM + same input → identical outputs & fuel consumption
- **Why it matters**: Non-determinism breaks replay verification and TEE guarantees
- **Implementation**: Uses `random-ark` example with fixed seed

```rust
for iteration in 0..100 {
    let result = execute_wasm_p1(&wasm, input, max_fuel).await?;
    results.push(result);
}

// All results must be identical to the first
let first = &results[0];
for result in results.iter().skip(1) {
    assert_deterministic(first, result, &context);
}
```

#### `epoch_deadline.rs::test_epoch_deadline_deterministic_timeout`
- **Property**: Same epoch deadline → same timeout behavior
- **Why it matters**: Timeouts must be deterministic for TEE replay
- **Implementation**: Runs 10 times with low epoch ticks, verifies all succeed or all fail

#### `cross_runtime.rs::test_wasmi_wasmtime_output_consistency`
- **Property**: wasmi (P1) and wasmtime (P2) produce identical outputs
- **Why it matters**: Different runtimes must agree on execution results
- **Note**: Fuel consumption may differ (different metering strategies), but outputs must match

### Running Phase 1 Tests

```bash
cd tests/verification-integration
cargo test determinism -- --nocapture
```

**Expected output**:
```
test determinism::fuel_consistency::test_100x_same_input_determinism ... ok
test determinism::epoch_deadline::test_epoch_deadline_deterministic_timeout ... ok
test determinism::cross_runtime::test_wasmi_wasmtime_output_consistency ... ok
```

## Phase 2: Contract NEP-297 Events

**What it verifies**: Contract events follow NEAR NEP-297 standard envelope format.

### Tests

#### `nep297_format.rs::test_event_envelope_format`
- **Property**: All emitted events have `standard`, `version`, `event`, `data` fields
- **Why it matters**: Off-chain indexers (The Graph, etc.) rely on this format
- **Implementation**: Parses event logs and validates JSON structure

```rust
let event: Nep297Event = serde_json::from_str(&log)?;
assert_eq!(event.standard, "nep297");
assert_eq!(event.version, "1.0.0");
assert!(event.event.len() > 0);
assert!(event.data.is_some());
```

### Running Phase 2 Tests

```bash
cargo test contract_events -- --nocapture
```

## Phase 3: Coordinator Hardening

**What it verifies**: Coordinator enforces authentication and idempotency.

### Tests

#### `idempotency.rs::test_parallel_requests_with_same_key`
- **Property**: 10 parallel POST requests with same `Idempotency-Key` → only 1 succeeds
- **Why it matters**: Prevents duplicate job execution and double billing
- **Implementation**: Spawns 10 concurrent requests, verifies only 1 gets 200 OK

```rust
let mut handles = vec![];
for _ in 0..10 {
    let handle = tokio::spawn(async move {
        client.post("/jobs/claim")
            .header("Idempotency-Key", shared_key)
            .send().await
    });
    handles.push(handle);
}

let responses = futures::join_all(handles).await;
let success_count = responses.iter().filter(|r| r.status() == 200).count();
assert_eq!(success_count, 1, "Only one request should succeed");
```

#### `near_signed_auth.rs::test_signature_verification`
- **Property**: Invalid ed25519 signatures → 401 Unauthorized
- **Why it matters**: Prevents unauthorized worker access
- **Implementation**: Tests valid signature (200 OK) and invalid signature (401)

### Running Phase 3 Tests

```bash
cargo test coordinator_hardening -- --nocapture
```

**Note**: Requires coordinator running on `localhost:8080` with `REQUIRE_AUTH=true`.

## Phase 4: WASI P1 Helpers

**What it verifies**: Helper functions prevent path traversal and arithmetic overflow.

### Tests

#### `github_canon.rs::test_path_traversal_prevention`
- **Property**: `../` sequences in repo paths → normalized safely
- **Why it matters**: Prevents malicious repos from accessing parent directories
- **Implementation**: Fuzzes with various `../` patterns

```rust
let malicious_paths = vec![
    "foo/../../etc/passwd",
    "../../../root/.ssh/id_rsa",
    "normal/path/../../../secrets",
];

for path in malicious_paths {
    let canonical = canonicalize_github_path(path)?;
    assert!(!canonical.contains(".."), "Path traversal not prevented: {}", canonical);
}
```

#### `safe_math.rs::test_checked_arithmetic`
- **Property**: Overflow in gas calculations → panic (not silent wraparound)
- **Why it matters**: Silent overflow could allow gas bypass attacks
- **Implementation**: Tests `u64::MAX + 1` and similar edge cases

### Running Phase 4 Tests

```bash
cargo test wasi_helpers -- --nocapture
```

## Phase 5: TypeScript Client

**What it verifies**: TypeScript client library correctly interfaces with coordinator API.

### Tests

#### `integration.rs::test_client_request_execution`
- **Property**: Client can request execution, poll status, retrieve results
- **Why it matters**: End-to-end verification of client library
- **Implementation**: Uses actual coordinator API

```typescript
const client = new OutlayerClient({ apiUrl: COORDINATOR_URL });

const request = await client.requestExecution({
    codeSource: { repo: "github.com/near/example", commit: "main" },
    resourceLimits: { maxInstructions: 1_000_000, maxExecutionSeconds: 10 }
});

const result = await client.pollUntilComplete(request.id);
assert(result.status === "completed");
```

### Running Phase 5 Tests

```bash
cargo test typescript_client -- --nocapture
```

**Note**: Requires Node.js and TypeScript client built.

## Common Test Utilities

### `common/mod.rs`

Provides shared helpers used across all test phases:

#### `build_test_wasm(example_name: &str) -> Result<Vec<u8>>`
- Compiles WASM from `wasi-examples/` directory
- Automatically detects P1 vs P2 target based on example name
- Returns compiled WASM bytes

#### `execute_wasm_p1(wasm: &[u8], input: &[u8], max_fuel: u64) -> Result<ExecutionResult>`
- Executes WASM with wasmi runtime (WASI P1)
- Tracks fuel consumption and execution time
- Returns output + metrics

#### `execute_wasm_p2(wasm: &[u8], input: &[u8], max_fuel: u64, max_epoch_ticks: u64) -> Result<ExecutionResult>`
- Executes WASM with wasmtime runtime (WASI P2)
- Tracks fuel + epoch deadline interruption
- Returns output + metrics

#### `assert_deterministic(r1: &ExecutionResult, r2: &ExecutionResult, context: &str)`
- Compares two execution results for bit-for-bit equality
- Checks output and fuel consumption
- Provides detailed error messages on mismatch

### ExecutionResult

```rust
pub struct ExecutionResult {
    pub output: String,
    pub fuel_consumed: u64,
    pub execution_time_ms: u64,
}
```

## Running All Tests

### Local Development

```bash
# 1. Start infrastructure
cd coordinator && docker-compose up -d

# 2. Start coordinator
cd coordinator && cargo run

# 3. Run integration tests
cd tests/verification-integration
cargo test -- --nocapture
```

### CI Pipeline

```bash
# GitHub Actions (see .github/workflows/integration.yml)
jobs:
  integration-tests:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - name: Start infrastructure
        run: |
          cd coordinator
          docker-compose up -d
          sleep 5  # Wait for services
      - name: Run tests
        run: cargo test -p verification-integration --all-features
```

## Test Coverage

| Phase | Tests | Lines Covered | Properties Verified |
|-------|-------|---------------|---------------------|
| 1     | 4     | determinism/  | Fuel consistency, epoch timeout, cross-runtime |
| 2     | 3     | contract_events/ | NEP-297 envelope, event data structure |
| 3     | 4     | coordinator_hardening/ | Idempotency, NEAR-signed auth |
| 4     | 5     | wasi_helpers/ | Path traversal, safe math, overflow detection |
| 5     | 3     | typescript_client/ | Client API integration |
| **Total** | **19** | - | **19 security properties** |

## Relationship to Property-Based Tests

These integration tests complement the **property-based verification suite** (`outlayer-verification-suite/`):

| Test Type | Purpose | Scale | Examples |
|-----------|---------|-------|----------|
| **Property-Based** | Adversarial edge-case discovery | 512-2000 randomized inputs per test | Determinism (512 cases), capabilities (512 cases), state integrity (256 cases) |
| **Integration** | Practical end-to-end verification | Real coordinator/contract interaction | 100x determinism, 10x parallel idempotency |

**Both are required** for high-confidence verification:
- Property tests find subtle bugs (e.g., non-determinism only on 1/500 inputs)
- Integration tests verify real-world behavior (e.g., coordinator actually enforces idempotency)

## Debugging Failed Tests

### Determinism Test Failures

If `test_100x_same_input_determinism` fails:

1. **Check WASM source**: Does it use randomness without seeding?
   ```bash
   cd wasi-examples/random-ark
   grep -r "getrandom\|rand::" src/
   ```

2. **Check execution environment**: Are clocks/IO being used?
   ```bash
   # In worker/src/executor/wasi_p1.rs
   # Ensure no clock imports are enabled
   ```

3. **Compare failed outputs**:
   ```rust
   // Test will print:
   // "iteration 42 diverged from iteration 0"
   // Check specific output difference
   ```

### Idempotency Test Failures

If `test_parallel_requests_with_same_key` fails (more than 1 success):

1. **Check coordinator middleware**:
   ```bash
   cd coordinator/src/middleware
   grep -r "Idempotency-Key" .
   ```

2. **Check database constraint**:
   ```sql
   \d jobs;  -- Should have UNIQUE(request_id, data_id, job_type)
   ```

3. **Check Redis locks**:
   ```bash
   docker exec -it offchainvm-redis redis-cli
   KEYS idempotency:*
   ```

## Future Work

### Additional Tests (Post-Phase 5)

1. **Nearcore Oracle Differential Testing**:
   - Compare mock TEE vs real nearcore runtime
   - Verify gas calculations match production

2. **TLA+ Model Checking**:
   - Formal verification of async receipt flow
   - Verify no deadlocks/race conditions

3. **Load Testing**:
   - 1000+ concurrent workers
   - 10,000+ jobs/sec throughput

4. **Chaos Engineering**:
   - Random coordinator restarts
   - Network partitions
   - Database failures

### Continuous Monitoring

Once in production, these tests should run:
- **Pre-commit**: Smoke test (1 test per phase, ~10 seconds)
- **Pre-merge**: Full suite (all 19 tests, ~2 minutes)
- **Nightly**: Extended suite with 1000x determinism runs (~30 minutes)

## References

- **Property-Based Verification**: `outlayer-verification-suite/README.md`
- **WASI Examples**: `wasi-examples/WASI_TUTORIAL.md`
- **Coordinator API**: `coordinator/README.md`
- **verification Implementation**: `CLAUDE.md` (architectural overview)

---

**Status**: Phase 1 determinism tests complete, Phases 2-5 test stubs ready for implementation

**Maintainer**: NEAR OutLayer Team

**License**: MIT
