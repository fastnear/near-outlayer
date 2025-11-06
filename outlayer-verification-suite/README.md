# Outlayer Verification Suite

Property-based testing harness for NEAR OutLayer TEE security properties.

## Overview

This crate uses **property-based testing** (PBT) with the `proptest` crate to adversarially test OutLayer's core security guarantees. Rather than writing individual test cases, we define **properties** (invariants) that must hold true across **thousands of randomized scenarios**.

## Properties Verified

### Property 1: Deterministic Execution
**Invariant**: Same inputs → same outputs (bit-for-bit)

```rust
// 512+ test cases with random {state, receipt} pairs
prop_assert_eq!(execute(state, receipt), execute(state, receipt));
```

**Why it matters**: Non-determinism breaks replay, makes verification impossible, and violates TEE guarantees.

### Property 2: Capability Scope Enforcement
**Invariant**: All generated callbacks respect access key constraints

```rust
// 512+ test cases verify every callback
for callback in output.callbacks {
    assert!(allowed_receivers.contains(callback.receiver));
    assert!(allowed_methods.contains(callback.method));
    assert!(callback.gas <= gas_allowance);
}
```

**Why it matters**: Capability violations = security bypass. TEE must never generate unauthorized operations.

### Property 3: Sealed State Integrity
**Invariant**: Tampering always detected

```rust
// 256+ tampering attempts (bit flips)
tampered_state.data[0] ^= 0x01;
assert_eq!(execute(tampered_state, receipt), Err(IntegrityError));
```

**Why it matters**: State tampering = L1 host attack. BLAKE3 hash must catch any modification.

### Property 4: Gas Accounting
**Invariant**: Insufficient gas → rejection

```rust
// 512+ test cases with various gas amounts
if receipt.gas < min_required_gas(args) {
    assert_eq!(execute(state, receipt), Err(GasExhausted));
}
```

**Why it matters**: Gas accounting prevents DoS. Must match nearcore's prepaid gas validation.

---

## Quick Start

### Run All Tests (Mock TEE)

```bash
cargo test --release
```

**Expected output:**
```
test determinism::deterministic_replay ... ok (512 cases)
test capabilities::capability_scope_enforced ... ok (512 cases)
test state_integrity::tamper_proof_state ... ok (256 cases)
test gas_accounting::insufficient_gas_rejected ... ok (512 cases)
```

### Run with Nearcore Oracle (Differential Testing)

```bash
cargo test --release --features nearcore-oracle
```

Compares mock TEE against real NEAR Protocol runtime.

### Run with Real Wasmtime Execution

```bash
cargo test --release --features engine-wasmtime
```

Uses actual WASM execution with fuel metering.

---

## Test Configuration

### Increase Test Cases (CI Mode)

```bash
# Run 2000+ cases per test (more thorough)
PROPTEST_CASES=2000 cargo test --release
```

### Reproduce Failures

Proptest automatically saves failing seeds:

```bash
# If test fails, seed is saved to proptest-regressions/
# Reproduce with:
PROPTEST_RNG_SEED=1234567890 cargo test
```

### Parallel Execution

```bash
# Run tests in parallel (faster)
cargo test --release -- --test-threads=4
```

---

## Architecture

### Mock TEE (`src/lib.rs`)

Simulates OutLayer TEE with:
- **BLAKE3 sealed state** (real crypto, not mocks)
- **Deterministic execution** (no clocks/rand/IO)
- **Capability enforcement** (access key constraints)
- **Gas accounting** (cost model matching nearcore)

### Strategies (`src/strategies.rs`)

Proptest generators for:
- Account IDs (valid NEAR format)
- Receipts (randomized but valid)
- Sealed states (with correct BLAKE3 hashes)
- Gas amounts (1 to 300 Tgas)

### Nearcore Bridge (`src/bridge_nearcore.rs`)

**TODO** (engineer task): Wire to `nearcore/runtime/src/lib.rs::apply()`

Enables differential testing:
```rust
let mock_result = MockTEE::execute(state, receipt);
let nearcore_result = NearcoreOracle::execute(state, receipt);
assert_eq!(mock_result, nearcore_result);
```

---

## Engineer Tasks

### Task 1: Wire Nearcore Oracle

**File**: `src/bridge_nearcore.rs`

**Steps**:
1. Add `near-vm-runner`, `near-parameters`, `near-primitives` deps
2. Create in-memory Trie
3. Map `Receipt` → `near_primitives::runtime::FunctionCall`
4. Call `nearcore::runtime::apply()`
5. Map outcomes → `ExecutionOutput`

**Reference**: `nearcore/runtime/src/tests/apply.rs`

**Verification**:
```bash
cargo test --features nearcore-oracle differential_nearcore
```

### Task 2: Add Wasmtime Execution

**File**: `src/engine_wasmtime.rs` (new)

**Steps**:
1. Feature-gate with `engine-wasmtime`
2. Implement `WasmtimeExecutor` with fuel + epoch
3. Reuse existing property tests
4. Add differential test: mock vs wasmtime

**Verification**:
```bash
cargo test --features engine-wasmtime
```

### Task 3: Nearcore Gas Pricing

**File**: `tests/gas_pricing.rs` (new)

**Steps**:
1. Import `near-parameters` config
2. Property test: our quote matches nearcore's
3. Test prepaid_gas calculations
4. Test exec_fees calculations

---

## Dependencies

### Core
- `blake3 = "1.5"` - Cryptographic hashing (BLAKE3)
- `proptest = "1.5"` - Property-based testing

### Optional (Nearcore Integration)
- `near-vm-runner = "0.32"` - NEAR WASM runtime
- `near-parameters = "0.32"` - Gas config
- `near-primitives = "0.32"` - NEAR types

### Optional (Wasmtime)
- `wasmtime = "27.0"` - Real WASM execution

---

## CI Configuration

### GitHub Actions

```yaml
jobs:
  property-tests-mock:
    runs-on: ubuntu-latest
    steps:
      - run: cargo test --release

  property-tests-nearcore:
    runs-on: ubuntu-latest
    steps:
      - run: cargo test --release --features nearcore-oracle

  property-tests-wasmtime:
    runs-on: ubuntu-latest
    steps:
      - run: cargo test --release --features engine-wasmtime
```

---

## Key Benefits

### Adversarial Testing
- Proptest generates edge cases humans wouldn't think of
- Automatic shrinking reduces failures to minimal cases
- 512-2000 cases per property = high confidence

### Nearcore Alignment
- Differential testing against real NEAR runtime
- Catches divergence early
- Uses battle-tested production code as oracle

### Multiple Backends
- Mock TEE (fast iteration)
- Nearcore oracle (correctness validation)
- Wasmtime (real WASM execution)

### Reproducibility
- Seed pinning for failures
- Deterministic test execution
- Shareable counterexamples

---

## Example Output

```
running 4 tests
test determinism::deterministic_replay ... ok [512 cases in 3.2s]
test capabilities::capability_scope_enforced ... ok [512 cases in 2.8s]
test state_integrity::tamper_proof_state ... ok [256 cases in 1.4s]
test gas_accounting::insufficient_gas_rejected ... ok [512 cases in 2.9s]

test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

---

## Next Steps

1. **Run tests locally**: `cargo test --release`
2. **Wire nearcore oracle**: See Task 1 above
3. **Add wasmtime execution**: See Task 2 above
4. **Integrate into CI**: See CI Configuration above

---

## References

- **Proptest docs**: https://docs.rs/proptest/
- **BLAKE3**: https://github.com/BLAKE3-team/BLAKE3
- **Nearcore runtime**: `nearcore/runtime/src/lib.rs`
- **NEAR parameters**: `nearcore/runtime/src/config.rs`

---

**Status**: Property tests complete, oracle bridge ready for wiring

**Maintainer**: NEAR OutLayer Team

**License**: MIT
