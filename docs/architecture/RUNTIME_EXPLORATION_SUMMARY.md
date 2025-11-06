# NEAR Runtime Architecture Exploration - Key Findings

**Date**: November 5, 2025  
**Duration**: Comprehensive codebase analysis  
**Scope**: Production NEAR runtime focusing on browser TEE compatibility

---

## EXPLORATION RESULTS

A complete architectural analysis has been generated and saved to:
**`NEAR_RUNTIME_ARCHITECTURE.md`** (1,027 lines)

### What Was Explored

1. **Runtime Directory Structure** (`/runtime/runtime/src/`)
   - 24 Rust modules totaling ~4,000 lines
   - Entry point: `Runtime::apply()` - core transaction/receipt processor
   - Key: `RuntimeExt` implementing the `External` trait

2. **VMLogic System** (`/near-vm-runner/src/logic/`)
   - 60+ host functions exposed to WebAssembly contracts
   - Critical: promise_yield_create/resume for async execution
   - Gas metering via instruction counter
   - Memory and register management

3. **Storage Layer** (`/core/store/src/trie/`)
   - Merkle tree implementation for state proofs
   - TrieKey enum for hierarchical state organization
   - State root calculation from trie changes
   - Incremental merkle trees for block production

4. **Primitives** (`/core/primitives/src/`)
   - Transaction structure (SignedTransaction)
   - Receipt types including PromiseYield
   - Block and epoch structures
   - Merkle path and proof verification

5. **Key Integration Points**
   - External trait (dependency boundary)
   - StorageAccessTracker (for TTN fees)
   - VMContext (execution environment)
   - ApplyResult (proof structure)

---

## CRITICAL FINDINGS FOR BROWSER TEE

### 1. Promise Yield is NATIVE to NEAR

**Most Important Discovery**:
NEAR has built-in async/off-chain support via `promise_yield`:
- `promise_yield_create()` pauses contract execution
- Returns `data_id` (resumption token)
- Off-chain system solves problem and calls `promise_yield_resume()`
- Contract callback receives result

This means **Outlayer doesn't need to hack async** - it's part of the protocol!

### 2. Runtime Architecture is Modular

```
Transaction
  ↓
Runtime::apply()
  ├─ Process transactions
  ├─ Process receipts (via RuntimeExt)
  │  └─ Executes via VMLogic (host functions)
  │     └─ Changes tracked in TrieUpdate
  ├─ Finalize state
  └─ Calculate state_root (Merkle hash)
```

The `External` trait is the clean boundary - perfect for browser implementation.

### 3. Host Functions are Comprehensive

60+ host functions covering:
- Storage operations (read/write/remove)
- Promise DAG creation
- Promise yield/resume
- Cryptographic operations
- Context/account information
- Logging and utilities

**For browser TEE MVP**: Can start with ~15 core functions.

### 4. Gas is Deterministic & Provable

```
Gas Flow:
1. Each operation has fixed cost
2. Instruction counter via fuel metering (wasmi/wasmtime)
3. Storage access tracked via TTN (trie touched nodes)
4. Total gas = instruction_gas + storage_gas + action_gas
5. Provable by hash(all costs)
```

**No estimates needed** - actual consumption is calculated.

### 5. State Roots are Cryptographic Proofs

```
State Root = hash(entire merkle tree)
Before: old_root
After: new_root (from trie_changes)

Merkle path proves: "Item X is in state under root Y"
```

**Key insight**: Anyone can verify a state transition without re-execution
- Just verify merkle proofs
- Check gas consumption
- Validate signatures

---

## FILES ANALYZED (11 Core Components)

| File | Lines | Purpose |
|------|-------|---------|
| `runtime/src/lib.rs` | 4,000+ | Transaction/receipt processing |
| `runtime/src/ext.rs` | 600+ | RuntimeExt (External impl) |
| `near-vm-runner/src/logic/logic.rs` | 3,500+ | Host functions |
| `near-vm-runner/src/logic/dependencies.rs` | 600+ | External trait |
| `near-vm-runner/src/logic/context.rs` | 88 | VMContext |
| `near-vm-runner/src/logic/gas_counter.rs` | 300+ | Gas metering |
| `core/primitives/src/merkle.rs` | 362 | Merkle trees |
| `core/primitives/src/receipt.rs` | 600+ | Receipt types |
| `core/primitives/src/trie_key.rs` | 400+ | Storage keys |
| `core/store/src/trie/mod.rs` | 150+ | Trie interface |
| `core/primitives/src/transaction.rs` | 300+ | Transaction structure |

---

## ARCHITECTURE PATTERNS TO IMPLEMENT

### Pattern 1: The External Trait Boundary

```rust
pub trait External {
    // Storage operations
    fn storage_set(...) -> Result<Option<Vec<u8>>>;
    fn storage_get(...) -> Result<Option<Box<dyn ValuePtr>>>;
    fn storage_remove(...) -> Result<Option<Vec<u8>>>;
    
    // Promise/receipt operations
    fn create_action_receipt(...) -> Result<ReceiptIndex>;
    fn create_promise_yield_receipt(...) -> Result<(ReceiptIndex, CryptoHash)>;
    fn submit_promise_resume_data(...) -> Result<bool>;
    
    // Action attachment (30+ methods)
    fn append_action_function_call(...) -> Result<()>;
    // ... etc
}
```

**For browser**: Implement this trait to control contract execution.

### Pattern 2: Gas Metering Structure

```rust
pub struct GasCounter {
    burnt_gas: u64,        // Irreversibly consumed
    gas_limit: u64,        // Maximum allowed
    opcode_cost: u64,      // Per-instruction
    
    profile: ProfileDataV3,  // Detailed breakdown
}
```

**For browser**: 
- Use wasmi/wasmtime instruction counter
- Each host function has fixed cost
- Accumulate in profile

### Pattern 3: State Root Calculation

```rust
TrieUpdate {
    changes: HashMap<TrieKey, Value>,
    new_root: MerkleHash,  // State commitment
}

ApplyResult {
    state_root: StateRoot,
    trie_changes: TrieChanges,  // Proof
}
```

**For browser**:
- Track all state changes in memory
- Calculate merkle hash from changes
- Return proof structure

### Pattern 4: Promise Yield (Critical!)

```rust
// Pause execution, create callback
promise_yield_create(
    method_name, args, gas, gas_weight
) -> (promise_idx, data_id)

// Resume from outside
promise_yield_resume(
    data_id, payload
) -> bool
```

**For browser**:
- Recognize yield receipts
- Return to coordinator
- Process resume when data available

---

## DETERMINISM REQUIREMENTS

### Must Guarantee (for validation):

1. **Same input blocks → Same state root**
   - No floating point math
   - Ordered iteration (BTreeMap, sorted vecs)
   - Seeded randomness (from block.hash)

2. **Exact gas consumption**
   - No approximations
   - Track every host function call
   - Count instructions exactly

3. **Proof verifiability**
   - Merkle paths must verify mathematically
   - Storage access patterns must match
   - Gas calculation must be reproducible

### Advantages:

- Validators can check proofs without re-execution
- Challenges can point to specific mismatch
- Determinism enables stateless validation

---

## COMPARISON: PRODUCTION NEAR vs OUTLAYER MVP

| Aspect | Production NEAR | Outlayer MVP | Browser TEE Goal |
|--------|-----------------|--------------|------------------|
| **Execution** | Wasmtime JIT | Wasmi interp | Wasmi/wasmtime |
| **Host Funcs** | 60+ full impl | Minimal mock | Core 15+ |
| **State** | Trie + RocksDB | In-memory mock | Merkle tree |
| **Gas** | Instruction + storage | Estimated | Actual measured |
| **Async** | promise_yield native | Coordinator hack | promise_yield |
| **Proofs** | Full merkle paths | None | Merkle + gas proofs |
| **Validation** | Stateless (via proofs) | Sequential | Stateless (goal) |

---

## NEXT STEPS FOR BROWSER TEE

### Short Term (MVP - 2-4 weeks):
1. Implement minimal VMLogic with 10 core functions
2. Add basic Merkle tree operations
3. Create in-memory state tracking
4. Wire up promise yield detection
5. Return results to coordinator

### Medium Term (Phase 1 - 4-6 weeks):
1. Add remaining host functions (40+)
2. Implement full gas metering
3. Create comprehensive proofs
4. Add cryptographic operations
5. Support all receipt types

### Long Term (Phase 2 - Phala Integration):
1. Move execution to TEE (Phala)
2. Add TEE attestation
3. Implement challenge-response validation
4. Add slashing mechanism
5. Production deployment

---

## KEY INSIGHTS

### 1. NEAR Designed for This

The protocol already has:
- Promise yield (async off-chain)
- Merkle proofs (state validation)
- Gas metering (resource proof)
- Deterministic execution (reproducible)
- Receipt-based async (perfect for browser)

We're not hacking NEAR - we're using features designed for off-chain.

### 2. Browser Execution is Feasible

Advantages:
- Deterministic (no FP, ordered data)
- Bounded (memory limits enforced)
- Verifiable (merkle proofs)
- Lightweight (wasmi is fast)
- Portable (wasmtime in browser)

### 3. Validation Without Re-execution

Stateless validation:
- Old state root + new state root + changes = proof
- Verify merkle paths without re-executing
- Check gas consumption mathematically
- Challenge fraud with merkle proofs

This is what makes stateless validation possible.

### 4. Promise Yield is the MVP Feature

For Outlayer:
- Contract calls `promise_yield_create()`
- Pauses transparently (protocol knows this)
- Off-chain solves problem
- Calls `promise_yield_resume()` with result
- Contract continues (no code change needed)

**This is why Outlayer can be deployed immediately** - NEAR already supports it!

---

## RECOMMENDATIONS

### For Browser Implementation:

1. **Start with External trait**
   - Create BrowserRuntimeExt implementing External
   - Start with 10 core methods (storage + promise_yield)
   - Expand to 60+ progressively

2. **Use Merkle trees for state**
   - Don't try to implement full trie
   - Use simple merkle tree for proof generation
   - Store state in JavaScript object/Map

3. **Leverage wasmi/wasmtime**
   - Both support instruction counting (fuel metering)
   - Deterministic execution out of box
   - No custom changes needed

4. **Implement proof verification**
   - Merkle path verification (simple)
   - Gas consumption validation (deterministic)
   - Combine for stateless validation

5. **Test with Outlayer MVP**
   - Use existing contract
   - Existing worker can submit yield receipts
   - Validate proofs on-chain (via contract)
   - Progressively add features

---

## CONCLUSION

The NEAR runtime is **beautifully architected** for what we're trying to do:

1. **Modular**: Clean External trait boundary
2. **Deterministic**: Same input = same output always
3. **Provable**: Merkle trees built-in, gas metering explicit
4. **Async-native**: promise_yield designed for off-chain
5. **Verifiable**: Stateless validation possible

**Browser TEE is not fighting NEAR - it's implementing it.**

The full architectural analysis in `NEAR_RUNTIME_ARCHITECTURE.md` provides:
- Detailed component descriptions
- Host function reference
- Gas metering model
- State root calculation
- Receipt processing flow
- Implementation checklist
- Pattern examples
- Validation strategies

This exploration revealed that the biggest challenge isn't the architecture
(it's elegant and proven) - it's **producing correct proofs at scale**.

But that's exactly what we're building Outlayer for.

---

**Next Phase**: Design browser implementation with these patterns
