# Browser TEE Implementation Roadmap

Based on NEAR Runtime Architecture Analysis  
**Generated**: November 5, 2025

---

## GOAL

Build a browser-executable NEAR runtime that:
1. Executes contracts deterministically
2. Produces cryptographic proofs of execution
3. Supports async via promise_yield
4. Validates without re-execution (stateless validation)
5. Integrates with Phala for production TEE

---

## ARCHITECTURE LAYERS

```
┌─────────────────────────────────────────────────────────┐
│                    Smart Contract Layer                 │
│                   (user's WASM code)                    │
│                                                         │
│  Uses host functions:                                  │
│  - storage_read/write/remove                           │
│  - promise_yield_create/resume                         │
│  - promise_create/then/and                             │
│  - context_* (read-only)                               │
│  - crypto_* (verification)                             │
└────────────────────┬────────────────────────────────────┘
                     │
                     │ Host functions
                     ▼
┌─────────────────────────────────────────────────────────┐
│            Browser VMLogic Layer (To Implement)          │
│                                                         │
│  ┌─────────────────────────────────────────────────┐   │
│  │ BrowserVMLogic (Rust → JavaScript/WASM)        │   │
│  │ - Memory management (bounded)                   │   │
│  │ - Registers (16 slots)                          │   │
│  │ - Host function dispatch                        │   │
│  │ - Gas metering (instruction count)              │   │
│  │ - Promise DAG tracking                          │   │
│  └─────────────────────────────────────────────────┘   │
│                     │                                   │
│                     │ External trait calls             │
│                     ▼                                   │
│  ┌─────────────────────────────────────────────────┐   │
│  │ BrowserRuntimeExt (Implements External)         │   │
│  │ - In-memory state storage                       │   │
│  │ - Storage tracking (for proofs)                 │   │
│  │ - Promise/receipt management                    │   │
│  │ - Yield detection & handling                    │   │
│  └─────────────────────────────────────────────────┘   │
└────────────────────┬────────────────────────────────────┘
                     │
                     │ State changes + promises
                     ▼
┌─────────────────────────────────────────────────────────┐
│            State & Proof Layer (To Implement)            │
│                                                         │
│  ┌─────────────────────────────────────────────────┐   │
│  │ In-Memory State Tracker                         │   │
│  │ - BTreeMap<TrieKey, Vec<u8>>                    │   │
│  │ - Tracked changes                               │   │
│  │ - Old/new root hashes                           │   │
│  └─────────────────────────────────────────────────┘   │
│                     │                                   │
│                     ▼                                   │
│  ┌─────────────────────────────────────────────────┐   │
│  │ Merkle Tree Builder                             │   │
│  │ - Calculate state root (new hash)               │   │
│  │ - Generate merkle proofs                        │   │
│  │ - Build inclusion/exclusion paths               │   │
│  └─────────────────────────────────────────────────┘   │
│                     │                                   │
│                     ▼                                   │
│  ┌─────────────────────────────────────────────────┐   │
│  │ Proof Structure                                 │   │
│  │ - old_state_root: Hash                          │   │
│  │ - new_state_root: Hash                          │   │
│  │ - changes: Vec<(TrieKey, Value)>                │   │
│  │ - gas_used: u64                                 │   │
│  │ - merkle_paths: Vec<MerklePath>                 │   │
│  │ - signature: Option<Attestation>                │   │
│  └─────────────────────────────────────────────────┘   │
└────────────────────┬────────────────────────────────────┘
                     │
                     │ Proof submission
                     ▼
┌─────────────────────────────────────────────────────────┐
│         NEAR Contract Layer (Already Exists)             │
│                                                         │
│  ┌─────────────────────────────────────────────────┐   │
│  │ offchainvm.near contract                        │   │
│  │ - Receives proof from browser                   │   │
│  │ - Validates merkle paths                        │   │
│  │ - Verifies gas consumption                      │   │
│  │ - Accepts/rejects execution                     │   │
│  │ - Manages slashing (Phase 2)                    │   │
│  └─────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────┘
```

---

## PHASE 1: MINIMAL MVP (2-4 weeks)

### Goal: Execute simple contract, prove it

### Step 1: Core Structures (3 days)

Create these TypeScript/Rust types:

```typescript
// In-memory state
type TrieKey = 
  | { Account: { account_id: string } }
  | { ContractData: { account_id: string, key: Uint8Array } }
  | ...;

type State = Map<string, Uint8Array>;

// Execution result
interface ExecutionResult {
  state_root: Hash;
  old_state_root: Hash;
  changes: Map<TrieKey, Uint8Array>;
  gas_used: u64;
  logs: string[];
  new_receipts: Receipt[];
  yield_indices: Map<data_id, callback>;
}
```

### Step 2: Minimal External Trait (3 days)

Implement 10 core methods:

```rust
// Storage (4 methods)
- storage_set(key, value) -> old_value
- storage_get(key) -> value
- storage_remove(key) -> old_value
- storage_has_key(key) -> bool

// Promise Yield (2 methods - CRITICAL!)
- create_promise_yield_receipt() -> (receipt_idx, data_id)
- submit_promise_resume_data(data_id, payload) -> success

// Promises (2 methods)
- create_action_receipt(dependencies, receiver) -> receipt_idx
- append_action_function_call(...) -> result

// Utilities (2 methods)
- generate_data_id() -> CryptoHash
- get_recorded_storage_size() -> usize
```

### Step 3: Gas Metering (3 days)

Add instruction counter:

```rust
// Via wasmi/wasmtime fuel metering
loop {
  result = execute_instruction();
  gas_counter.burn(INSTRUCTION_COST)?;
  if gas_counter.burnt_gas > limit {
    return Err(OutOfGas);
  }
}
```

### Step 4: Merkle Tree (2 days)

Implement from NEAR source:

```rust
pub fn combine_hash(h1: Hash, h2: Hash) -> Hash {
    hash(h1 || h2)
}

pub fn merkle_root(items: Vec<Hash>) -> Hash {
    // Binary tree with padding
}

pub fn verify_path(root: Hash, path: Vec<(Hash, Direction)>, item: Hash) -> bool {
    // Re-calculate root from path
}
```

### Step 5: Test Harness (2 days)

Create simple tests:

```bash
1. Load contract from WASM file
2. Execute simple method (no promises)
3. Verify state root matches
4. Check gas calculation
5. Generate and verify merkle proof
```

### Deliverables:
- [ ] BrowserRuntimeExt with 10 methods
- [ ] Gas metering working
- [ ] Merkle proofs generated
- [ ] Tests passing with simple contract
- [ ] Proof verified by contract

---

## PHASE 2: PROMISE YIELD SUPPORT (2 weeks)

### Goal: Support async execution via yield

### Step 1: Yield Receipt Detection (3 days)

```rust
// When contract calls promise_yield_create:
1. Create receipt with PromiseYield marker
2. Return data_id to caller
3. Pause execution (set state)
4. Return to coordinator

// On next message with data_id:
1. Look up yield receipt
2. Get provided data
3. Resume from pause point
4. Continue execution
```

### Step 2: Promise Queue (3 days)

```rust
struct PromiseQueue {
    // Index in execution
    promises: Vec<Promise>,
    
    // For callbacks
    callback_indices: Vec<ReceiptIndex>,
    
    // Results when promised complete
    results: HashMap<ReceiptIndex, Vec<u8>>,
}

// For yield:
yield_receipts: HashMap<data_id, ExecutionState> {
    pause_offset,
    register_state,
    stack,
}
```

### Step 3: Test with Outlayer MVP (3 days)

```bash
1. Contract calls promise_yield_create()
2. Browser returns yield receipt
3. Coordinator detects, solves off-chain
4. Submits promise_yield_resume()
5. Browser resumes, calculates new_root
6. Contract validates result
```

### Step 4: Expand Host Functions (3 days)

Add:
- `promise_create`, `promise_then`, `promise_and`
- Action attachment methods
- Promise result reading
- Register management

### Deliverables:
- [ ] Yield receipt handling
- [ ] Promise queue management
- [ ] Resume execution working
- [ ] Integration test with Outlayer
- [ ] All 10-15 core functions working

---

## PHASE 3: COMPLETE HOST FUNCTIONS (2 weeks)

### Goal: Support all core contract operations

Add remaining 45+ host functions:

```
1. Storage iteration (5 methods)
2. All action types (20+ methods)
3. Context reading (10 methods)
4. Basic crypto (5 methods)
5. Utilities (5+ methods)
```

### Key Functions to Add:

```rust
// Storage iteration
- storage_iter_prefix() -> iterator
- storage_iter_range() -> iterator
- storage_iter_next() -> (key, value)

// Context (read-only)
- current_account_id()
- signer_account_id()
- predecessor_account_id()
- block_height()
- block_timestamp()
- epoch_height()

// Crypto
- sha256(data)
- keccak256(data)
- ed25519_verify(sig, msg, pk)

// Logging
- log(msg)
- log_utf8(msg)
```

### Testing:
- [ ] Each function tested individually
- [ ] Integration tests with real contracts
- [ ] Gas costs validated
- [ ] Proofs verified

---

## PHASE 4: COMPREHENSIVE PROOFS (2 weeks)

### Goal: Full stateless validation support

### Add:

1. **Storage Access Tracking**
   ```rust
   struct StorageProof {
       trie_nodes_touched: u64,
       bytes_read: u64,
       bytes_written: u64,
       paths: Vec<MerklePath>,
   }
   ```

2. **Gas Breakdown Profile**
   ```rust
   struct GasProfile {
       instruction_cost: u64,
       storage_cost: u64,
       action_cost: u64,
       crypto_cost: u64,
       total: u64,
   }
   ```

3. **Complete Proof Package**
   ```rust
   struct ExecutionProof {
       old_state_root: Hash,
       new_state_root: Hash,
       changes: Vec<(TrieKey, Value)>,
       merkle_paths: Vec<MerklePath>,
       gas_profile: GasProfile,
       storage_proof: StorageProof,
       logs: Vec<String>,
       outcomes: Vec<ExecutionOutcome>,
   }
   ```

### Contract Integration:

```rust
// In offchainvm.near contract
pub fn validate_execution_proof(proof: ExecutionProof) -> bool {
    // 1. Verify merkle paths
    for path in &proof.merkle_paths {
        verify_path(proof.old_state_root, path, &change)?;
    }
    
    // 2. Recalculate new_state_root
    let calculated = merkle_root_from_changes(&proof.changes);
    assert_eq!(calculated, proof.new_state_root);
    
    // 3. Validate gas consumption
    assert_eq!(proof.gas_profile.total, proof.measured_gas);
    
    // 4. Accept or reject
    accept()
}
```

### Deliverables:
- [ ] Full proof generation working
- [ ] On-chain proof validation
- [ ] Contract accepts valid proofs
- [ ] Invalid proofs rejected
- [ ] Gas accounting correct

---

## PHASE 5: PRODUCTION HARDENING (2 weeks)

### Performance:
- [ ] Benchmark merkle tree generation
- [ ] Optimize state storage
- [ ] Cache frequent lookups
- [ ] Parallel proof verification

### Security:
- [ ] Validate all inputs
- [ ] Bound memory usage
- [ ] Prevent integer overflow
- [ ] Test with malicious inputs

### Testing:
- [ ] Property-based testing (merkle trees)
- [ ] Fuzzing (host functions)
- [ ] Stress testing (large state)
- [ ] Integration with full network

### Documentation:
- [ ] API reference
- [ ] Implementation guide
- [ ] Troubleshooting guide
- [ ] Example contracts

### Deliverables:
- [ ] Production-ready code
- [ ] Comprehensive test suite
- [ ] Documentation complete
- [ ] Performance benchmarks

---

## PHASE 6: PHALA INTEGRATION (4 weeks)

### Goal: Move to production TEE environment

### Step 1: Attestation (1 week)
```rust
// Sign proofs with TEE key
struct AttestationProof {
    execution_proof: ExecutionProof,
    tee_signature: Signature,
    tee_attestation: PhalaAttestation,
    timestamp: BlockHeight,
}
```

### Step 2: Challenge-Response (1 week)
```rust
// If proof disputed:
1. Challenger provides merkle proof of mismatch
2. TEE worker responds with original execution trace
3. Contract validates response
4. Determine who was wrong
5. Slash invalid party
```

### Step 3: Slashing Mechanism (1 week)
```rust
// Worker bonds collateral
// Invalid proof → slash bond
// Multiple violations → permanent ban
// Correct proofs → rewards
```

### Step 4: Production Deployment (1 week)
```rust
// Replace browser with Phala
// Use real TEE attestations
// Enable economic incentives
// Go to mainnet
```

### Deliverables:
- [ ] Phala attestation integration
- [ ] Challenge-response system
- [ ] Slashing mechanism
- [ ] Mainnet deployment

---

## IMPLEMENTATION CHECKLIST

### Phase 1 (MVP)
- [ ] External trait with 10 methods
- [ ] Gas metering (instruction count)
- [ ] Merkle tree implementation
- [ ] Test harness
- [ ] Simple contract execution

### Phase 2 (Yield)
- [ ] Yield receipt detection
- [ ] Promise queue
- [ ] Resume execution
- [ ] Outlayer integration test
- [ ] 15 core host functions

### Phase 3 (Complete)
- [ ] All 60 host functions
- [ ] Storage iteration
- [ ] Context reading
- [ ] Crypto verification
- [ ] Comprehensive tests

### Phase 4 (Proofs)
- [ ] Storage access tracking
- [ ] Gas profile generation
- [ ] Full proof package
- [ ] On-chain validation
- [ ] Contract integration

### Phase 5 (Production)
- [ ] Performance optimization
- [ ] Security hardening
- [ ] Comprehensive testing
- [ ] Documentation
- [ ] Benchmarks

### Phase 6 (Phala)
- [ ] Attestation integration
- [ ] Challenge-response
- [ ] Slashing mechanism
- [ ] Mainnet deployment

---

## RESOURCES & REFERENCES

### From NEAR Codebase:
- `/runtime/near-vm-runner/src/logic/` - VMLogic implementation
- `/runtime/runtime/src/ext.rs` - RuntimeExt (External impl)
- `/core/primitives/src/merkle.rs` - Merkle tree implementation
- `/core/primitives/src/receipt.rs` - Receipt structures

### External Links:
- NEAR Nomicon: https://nomicon.io
- Promise yield: https://nomicon.io/Proposals/0019-promise-yield
- Phala Network: https://phala.network

### Technologies:
- Wasmi/Wasmtime: https://github.com/wasmerio/wasmer
- SHA3/Blake2: Rust crypto libraries
- Merkle trees: Open source implementations

---

## ESTIMATED TIMELINE

| Phase | Duration | Team | Status |
|-------|----------|------|--------|
| **1: MVP** | 2-4 weeks | 1-2 engineers | Ready to start |
| **2: Yield** | 2 weeks | 1 engineer | After Phase 1 |
| **3: Complete** | 2 weeks | 1 engineer | After Phase 2 |
| **4: Proofs** | 2 weeks | 1-2 engineers | After Phase 3 |
| **5: Production** | 2 weeks | 2 engineers | Parallel with Phase 4 |
| **6: Phala** | 4 weeks | 2 engineers | After Phase 5 |
| **Total** | ~14 weeks | 2-3 engineers | Q1 2026 |

---

## SUCCESS CRITERIA

### Phase 1:
- Simple contract executes correctly
- Merkle proofs verify on-chain
- Gas calculation matches expectation

### Phase 2:
- Yield receipts created and resumed
- Outlayer MVP fully integrated
- Callback execution correct

### Phase 3:
- All contract operations supported
- Complex contracts execute
- Backward compatible with NEAR

### Phase 4:
- Stateless validation working
- On-chain proof verification
- No re-execution needed

### Phase 5:
- Production performance achieved
- Security audited
- Comprehensive docs available

### Phase 6:
- Mainnet deployment successful
- Economic incentives working
- High availability proven

---

## RISKS & MITIGATIONS

| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|-----------|
| Merkle tree bugs | Medium | High | Extensive testing, use proven impl |
| Gas metering inaccuracy | Low | High | Match NEAR exactly, test thoroughly |
| Promise yield edge cases | Medium | Medium | Real contract testing, fuzzing |
| Performance issues | Medium | High | Benchmark early, optimize incrementally |
| Phala integration delay | Low | Medium | Plan integration early, keep modular |

---

## NEXT STEPS

1. **This Week**: Start Phase 1
   - Create project structure
   - Implement External trait skeleton
   - Set up test framework

2. **Week 2-3**: Core functionality
   - Implement 10 host methods
   - Add gas metering
   - Build merkle tree

3. **Week 4**: Testing & integration
   - Create test contracts
   - Integrate with Outlayer MVP
   - Validate proofs on-chain

4. **Then**: Expand to full implementation
   - Follow roadmap phases
   - Iterate based on learnings
   - Build toward production

---

**Key Insight**: This is not building a runtime from scratch. We're implementing NEAR's proven architecture in the browser, using production patterns and proofs. The architecture is already validated by thousands of nodes. We just need to port it accurately.

**Timeline**: Realistic 14-week path to production with Phala, starting from today.

**Confidence**: High - the architecture is clean, the patterns are proven, and the integration points are clear.
