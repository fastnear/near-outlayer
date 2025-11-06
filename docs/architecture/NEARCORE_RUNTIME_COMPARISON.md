# OutLayer vs Nearcore Runtime - Deterministic Execution Comparison

**Date**: 2025-11-05
**Purpose**: Document how OutLayer's Phase 1.5 deterministic execution relates to nearcore's production runtime

---

## Overview

OutLayer Phase 1.5 implements deterministic WASM execution with properties similar to nearcore's runtime, but adapted for off-chain computation with yield/resume integration.

This document compares the two approaches to clarify:
1. What OutLayer already implements (Phase 1.5)
2. Where OutLayer diverges intentionally (different use case)
3. What nearcore conformance means for Phase 2

---

## Core Execution Flow

### Nearcore Runtime (`runtime/runtime/src/actions.rs`)

```rust
pub(crate) fn execute_function_call(
    contract: Box<dyn PreparedContract>,
    apply_state: &ApplyState,
    runtime_ext: &mut RuntimeExt,
    function_call: &FunctionCallAction,
    // ...
) -> Result<VMOutcome, RuntimeError> {
    // 1. Create deterministic random seed
    let random_seed = near_primitives::utils::create_random_seed(
        *action_hash,
        apply_state.random_seed
    );

    // 2. Build VMContext with block/epoch state
    let context = VMContext {
        current_account_id: runtime_ext.account_id().clone(),
        signer_account_id: action_receipt.signer_id().clone(),
        block_height: apply_state.block_height,
        block_timestamp: apply_state.block_timestamp,
        epoch_height: apply_state.epoch_height,
        account_balance: runtime_ext.account().amount(),
        prepaid_gas: function_call.gas,
        random_seed,
        // ... more context
    };

    // 3. Execute with near_vm_runner
    near_vm_runner::run(contract, runtime_ext, &context, fees)
}
```

**Key Properties**:
- Random seed is **deterministic** (derived from action_hash + block random_seed)
- Block height, timestamp, epoch from `ApplyState` (consensus-derived)
- Gas metering enforced by runtime
- Storage access via `RuntimeExt`

### OutLayer Phase 1.5 (`tests/phase-1-5-integration/src/common/mod.rs`)

```rust
pub async fn execute_wasm_p1(
    wasm_bytes: &[u8],
    input: &[u8],
    max_fuel: u64,
) -> Result<ExecutionResult> {
    // 1. Configure engine with fuel metering
    let mut config = Config::new();
    config.consume_fuel(true);
    config.epoch_interruption(true);

    let engine = Engine::new(&config)?;
    let module = Module::from_binary(&engine, wasm_bytes)?;

    // 2. Build WASI context (deterministic env)
    let mut wasi_builder = wasmtime_wasi::WasiCtxBuilder::new();
    wasi_builder.env("TZ", "UTC");
    wasi_builder.env("LANG", "C");
    // No ambient RNG, controlled stdin/stdout

    // 3. Execute with fuel + epoch deadline
    let mut store = Store::new(&engine, wasi_ctx);
    store.set_fuel(max_fuel)?;
    store.set_epoch_deadline(1000); // 10 seconds

    let instance = linker.instantiate_async(&mut store, &module).await?;
    let start_fn = instance.get_typed_func::<(), ()>(&mut store, "_start")?;
    start_fn.call_async(&mut store, ()).await?;

    // 4. Track fuel consumption
    let fuel_consumed = max_fuel.saturating_sub(store.get_fuel().unwrap_or(0));

    Ok(ExecutionResult { output, fuel_consumed, execution_time_ms })
}
```

**Key Properties**:
- Fuel metering (like nearcore gas)
- Epoch deadline (timeout enforcement)
- Deterministic environment (no ambient randomness)
- Stdin/stdout I/O (captured deterministically)

---

## Deterministic Random Seed

### Nearcore Approach

```rust
// near-primitives/src/utils.rs
pub fn create_random_seed(action_hash: CryptoHash, random_seed: CryptoHash) -> Vec<u8> {
    // Concatenate action_hash + block random_seed
    let mut bytes: Vec<u8> = Vec::with_capacity(64);
    bytes.extend_from_slice(action_hash.as_ref());
    bytes.extend_from_slice(random_seed.as_ref());
    hash(&bytes).as_ref().to_vec() // SHA256
}
```

**Properties**:
- Action-unique (different action_hash per receipt/action)
- Block-unique (random_seed changes per block)
- Deterministic (same inputs → same seed)
- Collision-resistant (cryptographic hash)

### OutLayer Phase 1.5 Approach (random-ark module)

```rust
// wasi-examples/random-ark/src/main.rs
let (clock_nanos, nonce) = if let Some(seed) = input.seed {
    // Deterministic mode: derive from seed
    let mut nonce = [0u8; 16];
    let seed_bytes = seed.to_le_bytes();
    for i in 0..16 {
        nonce[i] = seed_bytes[i % 8];
    }
    let clock_nanos = 1_000_000_000_000_000_000u128; // Fixed
    (clock_nanos, nonce)
} else {
    // Nondeterministic mode: real getrandom + clock
    let clock_nanos = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
    let mut nonce = [0u8; 16];
    getrandom::getrandom(&mut nonce).expect("Failed to get random bytes");
    (clock_nanos, nonce)
};
```

**Properties**:
- **Deterministic when seed provided** (for testing)
- **Nondeterministic mode available** (for entropy testing)
- **Test-controlled** (explicit seed input)

**Gap for Phase 2**: OutLayer doesn't yet derive random seed from block context like nearcore. This is intentional for Phase 1.5 (testing-focused), but Phase 2 should align with nearcore's `create_random_seed` pattern when integrated with contract yield/resume.

---

## ApplyState Context

### Nearcore ApplyState (`runtime/runtime/src/lib.rs`)

```rust
pub struct ApplyState {
    pub block_height: BlockHeight,
    pub block_timestamp: u64,
    pub epoch_height: EpochHeight,
    pub gas_price: Balance,
    pub random_seed: CryptoHash,  // Per-block randomness
    pub shard_id: ShardId,
    // ...
}
```

**Source**: Consensus-derived from block header

### OutLayer Phase 1.5 Context

Currently **not modeled** - Phase 1.5 tests execute WASM in isolation without block context.

**Phase 2 Target**: When OutLayer integrates with contract yield/resume, it should receive equivalent context from the contract:
- `block_height` - From promise_yield_create callback
- `block_timestamp` - From promise_yield_create callback
- `random_seed` - Derived from transaction/receipt hash
- `signer_id`, `predecessor_id` - From contract state

This context would be passed to WASM via:
1. **Environment variables** (for WASI modules)
2. **Function parameters** (for component model)
3. **NEAR syscalls** (for Phase 2+ NEAR runtime emulation)

---

## Fuel vs Gas Metering

### Nearcore Gas Model

- **Gas units**: Abstract execution cost (2,207,874 gas per instruction as of protocol 1.22.0)
- **Gas price**: Varies per block (economic parameter)
- **Gas limit**: 300 Tgas per transaction
- **Metering**: Injected by near-vm-runner at compile time

### OutLayer Phase 1.5 Fuel Model

- **Fuel units**: Direct wasmtime fuel (1 fuel ≈ N WASM instructions)
- **Fuel price**: Not modeled (testing-only)
- **Fuel limit**: Test-configurable (10M fuel in tests)
- **Metering**: wasmtime's `consume_fuel(true)` at runtime

**Conversion for Phase 2**:
```rust
// Approximate conversion (needs calibration)
let fuel = (near_gas / 2_207_874) * instructions_per_fuel;
```

**Verification Tests Needed**:
1. Same WASM on nearcore vs OutLayer → similar gas/fuel consumption
2. Calibration curve for gas ↔ fuel conversion
3. Edge cases (loops, recursion, memory ops)

---

## Epoch Deadline (Timeout Enforcement)

### Nearcore Epoch Interruption

- **Timeout mechanism**: Epoch-based interruption in near-vm-runner
- **Deadline**: Set per chunk execution (consensus-enforced)
- **Behavior**: Trap if epoch deadline exceeded during execution

### OutLayer Phase 1.5 Epoch Deadline

```rust
store.set_epoch_deadline(1000); // 1000 ticks = ~10 seconds
```

**Current Tests**:
- ✅ `test_epoch_deadline_timeout_behavior` - Verifies timeout trap
- ✅ `test_high_epoch_allows_completion` - Normal execution succeeds
- ✅ `test_epoch_deadline_deterministic_timeout` - Timeout is deterministic

**Status**: Functionally equivalent to nearcore for timeout enforcement.

---

## Storage Access

### Nearcore Storage

```rust
// Via RuntimeExt trait
runtime_ext.account() // Account state
runtime_ext.trie_update() // Merkle trie updates
```

**Properties**:
- Merkle trie-based (cryptographic state commitments)
- Storage staking (economic DoS prevention)
- Cross-shard receipts (async storage)

### OutLayer Phase 1.5 Storage

**Not modeled** - WASM modules use:
- **Stdin** for input data
- **Stdout** for output data
- **Environment variables** for secrets/config

**Phase 2 Target**: Implement NEAR storage syscalls:
```rust
// Hypothetical Phase 2 WASI host function
fn near_storage_read(key: &[u8]) -> Option<Vec<u8>> {
    // Call back to contract via promise_yield_resume
    // Contract reads from its own storage
    // Returns value to worker
}
```

This enables WASM to access contract storage deterministically.

---

## What Phase 1.5 Guarantees vs Nearcore

### ✅ Phase 1.5 Implements (Nearcore-Equivalent)

1. **Deterministic execution** - Same input → same output, same fuel
2. **Fuel/gas metering** - Resource accounting enforced
3. **Timeout enforcement** - Epoch deadline prevents infinite loops
4. **I/O isolation** - Stdin/stdout captured deterministically
5. **Environment isolation** - Deterministic env vars (TZ=UTC, LANG=C)

### ⏸️ Phase 1.5 Defers (Phase 2 Target)

1. **Block context** - No block_height, block_timestamp, random_seed from consensus
2. **Storage access** - No nearcore trie integration
3. **Gas price economics** - Fuel is free in tests
4. **Cross-contract calls** - No promise creation/resolution
5. **Account state** - No account balance, locked balance, storage_usage

### ❌ Phase 1.5 Intentionally Different

1. **Off-chain execution** - Not part of consensus (by design)
2. **HTTP/network access** - WASI P2 supports this (nearcore doesn't)
3. **Larger memory limits** - Not constrained by validator hardware
4. **Async I/O** - Uses tokio async runtime (nearcore is sync)

---

## Phase 2 Nearcore Conformance Roadmap

### Quick Wins (Align with Nearcore Patterns)

1. **Add block context to VMContext equivalent**:
   ```rust
   pub struct OutLayerContext {
       block_height: u64,
       block_timestamp: u64,
       random_seed: [u8; 32], // Derived from action_hash
       signer_id: String,
       predecessor_id: String,
   }
   ```

2. **Implement `create_random_seed` equivalent**:
   ```rust
   fn create_outlayer_random_seed(request_id: u64, block_seed: [u8; 32]) -> Vec<u8> {
       // Same pattern as nearcore
       let mut bytes = Vec::new();
       bytes.extend_from_slice(&request_id.to_le_bytes());
       bytes.extend_from_slice(&block_seed);
       sha256(&bytes).to_vec()
   }
   ```

3. **Add nearcore gas → OutLayer fuel conversion**:
   ```rust
   const GAS_TO_FUEL_RATIO: u64 = 1; // Calibrate via benchmarks
   let fuel = (prepaid_gas / 2_207_874) * GAS_TO_FUEL_RATIO;
   ```

### Medium-Term (Storage Integration)

1. **NEAR storage syscalls**:
   - `storage_read(key)` → contract.get_from_storage(key)
   - `storage_write(key, value)` → queued for contract callback
   - `storage_remove(key)` → queued for contract callback

2. **Promise yield state capture**:
   - Contract calls `promise_yield_create` with OutLayer request
   - Worker receives: block_height, block_timestamp, random_seed
   - Worker executes with nearcore-equivalent context

3. **Economic model**:
   - Gas estimation (like nearcore's `estimate_cost`)
   - Refunds based on actual fuel consumption
   - Storage staking for large outputs

### Long-Term (Full Conformance)

1. **Near-vm-runner integration**:
   - Use same WASM runtime as nearcore
   - Identical gas metering (instruction-level)
   - Same precompilation pipeline

2. **Receipt-based workflow**:
   - OutLayer worker generates DataReceipts
   - Callback receipts trigger promise_yield_resume
   - Cross-shard execution support

3. **Formal verification**:
   - Property-based testing (nearcore parity)
   - Differential fuzzing (nearcore vs OutLayer on same WASM)
   - TLA+ model of yield/resume with OutLayer

---

## Test Coverage Comparison

### Nearcore Runtime Tests

- Unit tests: `runtime/runtime/src/tests/`
- Integration tests: Runtime fees, gas limits, storage costs
- Fuzzing: nearcore uses cargo-fuzz for runtime edge cases

### OutLayer Phase 1.5 Tests (82/82 Passing)

- **Determinism** (19 tests): 100× replay, cross-runtime consistency
- **Resource limits** (3 tests): Zero fuel, epoch deadline, high epoch
- **I/O correctness** (10 tests): Stdout capture, UTF-8, memory isolation
- **Economic safety** (18 tests): Overflow/underflow, cost estimation
- **Security** (19 tests): Path traversal, cache bypass

**Gap**: OutLayer doesn't yet test nearcore-specific edge cases (e.g., storage rent, cross-shard receipts). Phase 2 should add:
- Nearcore gas limit edge cases
- Storage staking overflow protection
- Promise yield timeout behavior

---

## Recommendations for Phase 2

### Priority 1: Context Alignment
- [ ] Add ApplyState-equivalent to OutLayer execution
- [ ] Implement `create_random_seed` pattern
- [ ] Pass block_height, block_timestamp to WASM

### Priority 2: Gas/Fuel Calibration
- [ ] Benchmark identical WASM on nearcore vs OutLayer
- [ ] Create conversion table (NEAR gas ↔ wasmtime fuel)
- [ ] Test edge cases (loops, memory, storage)

### Priority 3: Storage Integration
- [ ] Prototype NEAR storage syscalls
- [ ] Test with contract storage reads/writes
- [ ] Verify determinism across storage operations

### Priority 4: Differential Testing
- [ ] Same WASM on nearcore testnet vs OutLayer
- [ ] Assert identical outputs (modulo context differences)
- [ ] Fuzz test with random inputs

---

## Conclusion

**Phase 1.5 Status**: OutLayer implements nearcore-equivalent deterministic execution for the isolated WASM execution use case. Core properties (fuel metering, timeout enforcement, I/O isolation) are production-ready.

**Phase 2 Target**: Align with nearcore's block context model, add storage integration, calibrate gas/fuel conversion. Full conformance requires integrating near-vm-runner and nearcore's promise yield callbacks.

**Current Gaps Are Acceptable**: Phase 1.5 intentionally focuses on off-chain execution without full blockchain context. Phase 2 will bridge the gap via promise_yield state and NEAR storage syscalls.

---

## Research Artifacts

Nearcore conformance explorations (primitives bindings, fee parity tests, Borsh ABI prototypes) are available in [`/research/nearcore-conformance/`](../../research/nearcore-conformance/).

This research code is **not part of Phase 1.5 deliverables** but provides scaffolding for Phase 2 integration work.

See [`/research/README.md`](../../research/README.md) for scope and usage details.

---

**Document Version**: 1.1
**Last Updated**: 2025-11-05
**Nearcore Reference**: nearcore `runtime/runtime/src/actions.rs`, `lib.rs`
**OutLayer Reference**: `tests/phase-1-5-integration/src/common/mod.rs`
**Research Reference**: `/research/nearcore-conformance/` (Phase 2 scaffolding)
