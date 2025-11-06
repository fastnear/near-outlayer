# NEAR Runtime Architecture Deep Dive for Browser-Based TEE Implementation

**Date: November 5, 2025**
**Focus: Production Runtime Analysis for Browser TEE Compatibility**

---

## EXECUTIVE SUMMARY

This document provides a thorough architectural analysis of NEAR's production runtime, focusing on components and patterns critical for browser-based TEE (Trusted Execution Environment) implementation. The analysis reveals that NEAR's runtime is split into three main components:

1. **VMLogic** - Host functions and execution state management
2. **Runtime** - Receipt/transaction processing and state transitions  
3. **Storage Layer** - Trie-based state persistence with Merkle proofs

For browser TEE implementation, we need to:
- Implement a minimal VMLogic with core host functions
- Support async receipt processing (promise yield/resume)
- Calculate state roots deterministically using Merkle trees
- Support gas metering via instruction counting
- Track and prove storage access patterns

---

## PART 1: RUNTIME ARCHITECTURE OVERVIEW

### 1.1 Component Hierarchy

```
┌─────────────────────────────────────────────┐
│   NEAR Runtime (runtime/runtime/src/)       │
│                                             │
│  ┌─────────────────────────────────────┐   │
│  │ Runtime::apply()                    │   │
│  │ - Transaction processing            │   │
│  │ - Receipt processing                │   │
│  │ - State root calculation            │   │
│  └────┬────────────────────────────────┘   │
│       │                                    │
│       ├─► ApplyProcessingState             │
│       │   - state_update: TrieUpdate       │
│       │   - trie: Trie                     │
│       │                                    │
│       ├─► ApplyResult                      │
│       │   - state_root: StateRoot          │
│       │   - trie_changes: TrieChanges      │
│       │   - outcomes: ExecutionOutcome[]   │
│       │   - state_changes: StateChange[]   │
│       │                                    │
│       └─► Execute via RuntimeExt           │
│                                             │
│  ┌─────────────────────────────────────┐   │
│  │ RuntimeExt (runtime/src/ext.rs)     │   │
│  │ Implements: External trait          │   │
│  │ - Storage read/write/remove         │   │
│  │ - Promise creation                  │   │
│  │ - Receipt management                │   │
│  └─────────────────────────────────────┘   │
│                                             │
│  ┌─────────────────────────────────────┐   │
│  │ ActionResult                        │   │
│  │ - gas_burnt, gas_used               │   │
│  │ - result: ReturnData                │   │
│  │ - new_receipts: Receipt[]           │   │
│  │ - logs: LogEntry[]                  │   │
│  └─────────────────────────────────────┘   │
└─────────────────────────────────────────────┘
            │
            ▼
┌─────────────────────────────────────────────┐
│ VMLogic (near-vm-runner/src/logic/)         │
│                                             │
│ ┌─────────────────────────────────────┐   │
│ │ VMLogic<'a>                         │   │
│ │ - ext: &mut dyn External            │   │
│ │ - context: &VMContext               │   │
│ │ - memory: Memory<'a>                │   │
│ │ - registers: Registers              │   │
│ │ - gas_counter: GasCounter           │   │
│ │ - promises: Vec<Promise>            │   │
│ │ - result_state: ExecutionResultState│   │
│ └─────────────────────────────────────┘   │
│                                             │
│ Host Functions (~60):                       │
│ • storage_* (read, write, remove, etc)     │
│ • promise_* (create, then, and, yield)     │
│ • context_* (signer, account, etc)         │
│ • crypto_* (ed25519, ECDSA, hash)          │
│                                             │
│ ┌─────────────────────────────────────┐   │
│ │ ExecutionResultState                │   │
│ │ - gas_counter: GasCounter           │   │
│ │ - logs: Vec<String>                 │   │
│ │ - return_data: ReturnData           │   │
│ │ - current_account_balance: Balance  │   │
│ │ - current_storage_usage: Storage    │   │
│ └─────────────────────────────────────┘   │
│                                             │
│ ┌─────────────────────────────────────┐   │
│ │ VMOutcome                           │   │
│ │ - burnt_gas, used_gas               │   │
│ │ - balance, storage_usage            │   │
│ │ - return_data, logs                 │   │
│ │ - compute_usage (profile)           │   │
│ └─────────────────────────────────────┘   │
└─────────────────────────────────────────────┘
            │
            ▼
┌─────────────────────────────────────────────┐
│ Storage Layer (core/store/src/trie/)        │
│                                             │
│ ┌─────────────────────────────────────┐   │
│ │ TrieUpdate                          │   │
│ │ - trie: &'a Trie                    │   │
│ │ - changes: HashMap<TrieKey, Value>  │   │
│ └─────────────────────────────────────┘   │
│                                             │
│ ┌─────────────────────────────────────┐   │
│ │ TrieChanges                         │   │
│ │ - new_root: MerkleHash (StateRoot)  │   │
│ │ - old_root: MerkleHash              │   │
│ │ - insertions/deletions              │   │
│ └─────────────────────────────────────┘   │
│                                             │
│ ┌─────────────────────────────────────┐   │
│ │ Merkle Tree (merkle.rs)             │   │
│ │ - combine_hash(h1, h2) -> h3        │   │
│ │ - merklize(items) -> (root, paths)  │   │
│ │ - verify_path(root, path, item)     │   │
│ │ - PartialMerkleTree (incremental)   │   │
│ └─────────────────────────────────────┘   │
└─────────────────────────────────────────────┘
```

### 1.2 Execution Flow

```
Transaction/Receipt Processing Flow:

1. Runtime::apply()
   ├─ Validate transactions
   ├─ Create receipts from transactions
   └─ Process all receipts

2. For each Receipt:
   ├─ Apply to TrieUpdate (state changes)
   ├─ Execute contract if action receipt
   │  └─ Call RuntimeExt::execute_action()
   │     └─ VMLogic::new() + contract execution
   │        └─ Host functions called via External trait
   │           └─ RuntimeExt implements External
   ├─ Generate new receipts (promises)
   └─ Calculate gas used

3. After all receipts:
   ├─ Finalize TrieUpdate
   ├─ Calculate new_root from trie_changes
   ├─ Return ApplyResult with:
   │  - state_root (new Merkle hash)
   │  - trie_changes (proof of changes)
   │  - outcomes (execution results)
   │  - state_changes (for indexing)
   └─ Block can be committed
```

---

## PART 2: VMLogic - HOST FUNCTION INTERFACE

### 2.1 VMLogic Structure

**File**: `/Users/mikepurvis/near/nearcore/runtime/near-vm-runner/src/logic/logic.rs`

```rust
pub struct VMLogic<'a> {
    // External interface to storage and receipts
    ext: &'a mut dyn External,
    
    // Execution context (block/account/signer info)
    context: &'a VMContext,
    
    // Guest WebAssembly memory
    memory: Memory<'a>,
    
    // Execution state
    config: Arc<Config>,
    fees_config: Arc<RuntimeFeesConfig>,
    current_account_locked_balance: Balance,
    
    // Guest-accessible data
    registers: Registers,           // 16 named storage slots
    promises: Vec<Promise>,         // DAG of promises
    remaining_stack: u64,
    
    // Result accumulation
    result_state: ExecutionResultState,
}
```

### 2.2 VMContext - Execution Environment

**File**: `/Users/mikepurvis/near/nearcore/runtime/near-vm-runner/src/logic/context.rs`

```rust
pub struct VMContext {
    // Account information
    current_account_id: AccountId,
    signer_account_id: AccountId,
    signer_account_pk: PublicKey,
    predecessor_account_id: AccountId,
    refund_to_account_id: AccountId,
    
    // Contract input
    input: Rc<[u8]>,
    
    // Promise results (for callbacks)
    promise_results: Arc<[PromiseResult]>,
    
    // Block context
    block_height: BlockHeight,
    block_timestamp: u64,
    epoch_height: EpochHeight,
    
    // Account state
    account_balance: Balance,
    account_locked_balance: Balance,
    storage_usage: StorageUsage,
    account_contract: AccountContract,
    
    // Transaction context
    attached_deposit: Balance,
    prepaid_gas: Gas,
    random_seed: Vec<u8>,
    
    // View-only mode
    view_config: Option<ViewConfig>,
    
    // Output receivers (for data receipts)
    output_data_receivers: Vec<AccountId>,
}
```

### 2.3 External Trait - Storage & Receipt Interface

**File**: `/Users/mikepurvis/near/nearcore/runtime/near-vm-runner/src/logic/dependencies.rs`

This trait defines the boundary between VMLogic and the host (storage/blockchain).

#### Storage Operations
```rust
pub trait External {
    // Core storage operations
    fn storage_set(
        &mut self,
        access_tracker: &mut dyn StorageAccessTracker,
        key: &[u8],
        value: &[u8],
    ) -> Result<Option<Vec<u8>>>;
    
    fn storage_get(
        &self,
        access_tracker: &mut dyn StorageAccessTracker,
        key: &[u8],
    ) -> Result<Option<Box<dyn ValuePtr>>>;
    
    fn storage_remove(
        &mut self,
        access_tracker: &mut dyn StorageAccessTracker,
        key: &[u8],
    ) -> Result<Option<Vec<u8>>>;
    
    fn storage_has_key(
        &mut self,
        access_tracker: &mut dyn StorageAccessTracker,
        key: &[u8],
    ) -> Result<bool>;
}
```

#### Promise/Receipt Operations
```rust
pub trait External {
    // Promise/Receipt creation
    fn create_action_receipt(
        &mut self,
        receipt_indices: Vec<ReceiptIndex>,
        receiver_id: AccountId,
    ) -> Result<ReceiptIndex>;
    
    // Promise Yield (async execution - CRITICAL FOR OUTLAYER)
    fn create_promise_yield_receipt(
        &mut self,
        receiver_id: AccountId,
    ) -> Result<(ReceiptIndex, CryptoHash)>;
    
    fn submit_promise_resume_data(
        &mut self,
        data_id: CryptoHash,
        data: Vec<u8>,
    ) -> Result<bool>;
    
    // Action attachment
    fn append_action_function_call_weight(
        &mut self,
        receipt_index: ReceiptIndex,
        method_name: Vec<u8>,
        arguments: Vec<u8>,
        attached_deposit: Balance,
        prepaid_gas: Gas,
        gas_weight: GasWeight,
    ) -> Result<()>;
    
    // ... 30+ more action types ...
}
```

### 2.4 Host Function Categories

#### Core Host Functions (~60 total)

1. **Storage Operations**
   - `storage_write(key, value) -> Option<old_value>`
   - `storage_read(key) -> Option<value>`
   - `storage_remove(key) -> Option<old_value>`
   - `storage_has_key(key) -> bool`
   - `storage_usage() -> bytes`

2. **Promise Creation (Core DAG)**
   - `promise_create(account, method, args, deposit, gas) -> promise_idx`
   - `promise_then(promise, account, method, args, deposit, gas) -> promise_idx`
   - `promise_and(promises[]) -> promise_idx` (combine multiple)
   - `promise_batch_create(account) -> promise_idx`
   - `promise_batch_then(promise, account) -> promise_idx`

3. **Promise Yield (CRITICAL - Async Execution)**
   ```
   // Pause execution, create callback
   promise_yield_create(
       method_name: &str,
       arguments: &[u8],
       gas: u64,
       gas_weight: u64
   ) -> (promise_idx, data_id)
   
   // Resume from outside (must provide data)
   promise_yield_resume(
       data_id: &[u8],
       payload: &[u8]
   ) -> bool  // success
   ```

4. **Promise Actions (Batch)**
   - `promise_batch_action_create_account(promise)`
   - `promise_batch_action_deploy_contract(promise, code)`
   - `promise_batch_action_function_call(promise, method, args, deposit, gas)`
   - `promise_batch_action_transfer(promise, amount)`
   - `promise_batch_action_stake(promise, amount, public_key)`
   - `promise_batch_action_add_key(promise, pk, access_key)`
   - `promise_batch_action_delete_key(promise, pk)`
   - `promise_batch_action_delete_account(promise, account)`

5. **Promise Results & Control**
   - `promise_results_count() -> count`
   - `promise_result(index) -> (status, data)`
   - `promise_return(promise_idx)` (return from callback)

6. **Context Information**
   - `current_account_id() -> account`
   - `signer_account_id() -> account`
   - `signer_account_pk() -> public_key`
   - `predecessor_account_id() -> account`
   - `input() -> data`
   - `block_height() -> height`
   - `block_timestamp() -> nanos`
   - `epoch_height() -> epoch`
   - `account_balance() -> balance`
   - `account_locked_balance() -> balance`
   - `storage_usage() -> bytes`

7. **Cryptographic Operations**
   - `sha256(data) -> hash`
   - `keccak256(data) -> hash`
   - `ripemd160(data) -> hash`
   - `ed25519_verify(sig, data, pk) -> bool`
   - `ed25519_recover_pk(sig, data, recovery_id) -> pk`
   - `secp256k1_verify(sig, data, pk) -> bool`
   - `secp256k1_recover_pk(sig, data, recovery_id) -> pk`
   - `alt_bn128_*`, `bls12381_*` (curve operations)

8. **Logging & Utilities**
   - `log(message)` - emit logs
   - `log_utf8(message)`
   - `abort(message)` - fail execution

---

## PART 3: GAS METERING & RESOURCE ACCOUNTING

### 3.1 Gas Counter Structure

**File**: `/Users/mikepurvis/near/nearcore/runtime/near-vm-runner/src/logic/gas_counter.rs`

```rust
pub struct GasCounter {
    fast_counter: FastGasCounter,
    promises_gas: Gas,
    max_gas_burnt: Gas,
    prepaid_gas: Gas,
    is_view: bool,
    ext_costs_config: ExtCostsConfig,
    profile: ProfileDataV3,
}

#[repr(C)]
pub struct FastGasCounter {
    pub burnt_gas: u64,        // Actually consumed
    pub gas_limit: u64,        // Maximum allowed
    pub opcode_cost: u64,      // Per-instruction cost
}
```

### 3.2 Gas Accounting Model

```
Gas Flow:
1. prepaid_gas (attached to transaction)
2. Execution starts:
   - Pay base cost for each host function
   - Pay per-byte/item costs
   - Instruction counter tracks fuel
3. On promise creation:
   - gas_burnt stays paid by this execution
   - gas_weight distributes remaining
4. On completion:
   - burnt_gas: irreversibly consumed
   - used_gas: includes promises gas
   - Refund = prepaid_gas - used_gas
```

### 3.3 Cost Structure

```rust
pub enum ExtCosts {
    // Base costs (per call)
    base,
    contract_loading_base,
    contract_loading_bytes,
    
    // Storage costs
    storage_write_base,
    storage_write_evicted_byte,
    storage_read_base,
    storage_read_value_byte,
    storage_remove_base,
    storage_remove_value_byte,
    storage_has_key_base,
    touching_trie_node,
    read_cached_trie_node,
    
    // Promise costs
    promise_and_base,
    promise_and_per_promise,
    promise_return_base,
    
    // Yield costs
    yield_create_base,
    yield_create_byte,
    yield_resume_base,
    yield_resume_byte,
    
    // Crypto costs
    ed25519_verify_base,
    ed25519_verify_byte,
    secp256k1_verify_base,
    secp256k1_verify_byte,
    // ... many more ...
}
```

---

## PART 4: STATE MANAGEMENT & MERKLE TREES

### 4.1 TrieKey Structure

**File**: `/Users/mikepurvis/near/nearcore/core/primitives/src/trie_key.rs`

State is keyed by TrieKey enum:
```rust
pub enum TrieKey {
    // Account state
    Account { account_id: AccountId },
    AccessKey { account_id: AccountId, public_key: PublicKey },
    
    // Contract state
    ContractData { account_id: AccountId, key: Vec<u8> },
    
    // Contract code
    ContractCode { account_id: AccountId },
    
    // Receipt storage
    PostponedReceipt { receiver_id: AccountId, receipt_id: CryptoHash },
    ReceivedData { receiver_id: AccountId, data_id: CryptoHash },
    PromiseYieldReceipt { account_id: AccountId, data_id: CryptoHash },
    PromiseYieldTimeout { account_id: AccountId, data_id: CryptoHash },
}
```

### 4.2 Merkle Tree Implementation

**File**: `/Users/mikepurvis/near/nearcore/core/primitives/src/merkle.rs`

```rust
// Core merkle operations
pub fn combine_hash(hash1: &MerkleHash, hash2: &MerkleHash) -> MerkleHash {
    CryptoHash::hash_borsh((hash1, hash2))
}

pub fn merklize<T: BorshSerialize>(
    arr: &[T]
) -> (MerkleHash, Vec<MerklePath>) {
    // Complete binary tree with padding
    // Returns root hash and proof path for each item
}

pub fn verify_path<T: BorshSerialize>(
    root: MerkleHash,
    path: &MerklePath,
    item: T
) -> bool {
    // Verify item is in tree at given root
}

// Incremental merkle tree (for block production)
pub struct PartialMerkleTree {
    path: Vec<MerkleHash>,   // Binary tree path
    size: u64,               // Number of items
}

impl PartialMerkleTree {
    pub fn insert(&mut self, elem: MerkleHash) { }
    pub fn root(&self) -> MerkleHash { }
}
```

### 4.3 State Root Calculation

```
TrieUpdate Process:
1. Start with old_root from parent state
2. Apply changes:
   - storage_set/remove modify trie nodes
   - Changes tracked in TrieUpdate
3. Finalize:
   - Trie encodes nodes to bytes
   - Hash each node's bytes
   - Build merkle proof upward
   - new_root = hash(all_changes)
4. Return TrieChanges:
   - old_root, new_root
   - insertions, deletions
   - For stateless validation
```

### 4.4 State Root Hash

```rust
pub type StateRoot = MerkleHash;

pub struct ApplyResult {
    pub state_root: StateRoot,        // Hash of entire state
    pub trie_changes: TrieChanges,    // Proof of changes
    pub state_changes: Vec<RawStateChangesWithTrieKey>,  // Changes for indexing
}
```

---

## PART 5: RECEIPT PROCESSING & ASYNC EXECUTION

### 5.1 Receipt Types

**File**: `/Users/mikepurvis/near/nearcore/core/primitives/src/receipt.rs`

```rust
pub struct ReceiptV0 {
    pub predecessor_id: AccountId,
    pub receiver_id: AccountId,
    pub receipt_id: CryptoHash,
    pub receipt: ReceiptEnum,
}

pub enum ReceiptEnum {
    Action(ActionReceipt),
    Data(DataReceipt),
    PromiseYield(ActionReceipt),      // Yield point
    PromiseYieldV2(ActionReceipt),
}

pub struct ActionReceipt {
    pub signer_id: AccountId,
    pub signer_public_key: PublicKey,
    pub gas_price: Balance,
    pub output_data_receivers: Vec<AccountId>,
    pub input_data_ids: Vec<CryptoHash>,    // Data dependencies
    pub actions: Vec<Action>,
}

pub struct DataReceipt {
    pub data_id: CryptoHash,
    pub data: Vec<u8>,
}
```

### 5.2 Promise Yield (Critical for Outlayer)

Promise yield enables **off-chain computation**:

```
Contract Execution:
1. Call promise_yield_create(method, args, gas)
   ├─ Creates yield receipt (pause point)
   ├─ Returns data_id (resumption token)
   └─ Callback scheduled for later

2. Off-chain system (Outlayer):
   ├─ Detects yield receipt
   ├─ Executes computation externally
   ├─ Gets result

3. Call promise_yield_resume(data_id, result)
   ├─ Submits result to blockchain
   └─ Resumes callback execution

4. Callback receives result:
   ├─ Calls promise_result() to get data
   └─ Executes rest of computation
```

**Runtime Processing** (in `/Users/mikepurvis/near/nearcore/runtime/runtime/src/lib.rs`):
```rust
// Yield receipt waiting for data
VersionedReceiptEnum::PromiseYield(_) => {
    set_promise_yield_receipt(state_update, receipt);
    // Block until resume
}

// Resume receipt with data
ReceiptEnum::Data(data_receipt) => {
    if let Some(yield_receipt) = get_promise_yield_receipt(...) {
        remove_promise_yield_receipt(...);
        // Execute callback with data
        execute_receipt(state_update, yield_receipt)?;
    }
}
```

### 5.3 Receipt Processing Flow

```
Runtime::apply():
1. Load delayed_receipts from state
2. Process incoming_receipts:
   ├─ Action Receipt:
   │  ├─ Check input_data_ids satisfied
   │  ├─ If not: buffer as delayed
   │  └─ If yes: execute via VMLogic
   │
   ├─ Data Receipt:
   │  └─ Match to yield receipt
   │     └─ Resume execution
   │
   └─ PromiseYield Receipt:
      └─ Store waiting for resume
3. Finalize state_root
4. Return ApplyResult with outcomes
```

---

## PART 6: EXECUTION RESULT STRUCTURES

### 6.1 ActionResult

**File**: `/Users/mikepurvis/near/nearcore/runtime/runtime/src/lib.rs:258`

```rust
pub struct ActionResult {
    pub gas_burnt: Gas,                    // Irreversibly used
    pub gas_burnt_for_function_call: Gas,  // Function call specific
    pub gas_used: Gas,                     // Including promises
    pub compute_usage: Compute,            // Profile data
    pub result: Result<ReturnData, ActionError>,
    pub logs: Vec<LogEntry>,
    pub new_receipts: Vec<Receipt>,        // Outgoing promises
    pub validator_proposals: Vec<ValidatorStake>,
    pub profile: Box<ProfileDataV3>,       // Detailed gas breakdown
}
```

### 6.2 VMOutcome

```rust
pub struct VMOutcome {
    pub balance: Balance,
    pub storage_usage: StorageUsage,
    pub return_data: ReturnData,
    pub burnt_gas: Gas,
    pub used_gas: Gas,
    pub compute_usage: Compute,
    pub logs: Vec<String>,
    pub profile: ProfileDataV3,
    pub aborted: Option<FunctionCallError>,
}

pub enum ReturnData {
    None,
    ReceiptIndex(u64),
    Value(Vec<u8>),
}
```

### 6.3 ApplyResult

**File**: `/Users/mikepurvis/near/nearcore/runtime/runtime/src/lib.rs:236`

```rust
pub struct ApplyResult {
    // State commitment
    pub state_root: StateRoot,             // New merkle hash
    pub trie_changes: TrieChanges,         // Proof for validators
    
    // Execution results
    pub outcomes: Vec<ExecutionOutcomeWithId>,
    pub state_changes: Vec<RawStateChangesWithTrieKey>,
    
    // Outgoing data
    pub outgoing_receipts: Vec<Receipt>,
    pub processed_delayed_receipts: Vec<Receipt>,
    pub processed_yield_timeouts: Vec<PromiseYieldTimeout>,
    
    // Validator updates
    pub validator_proposals: Vec<ValidatorStake>,
    
    // Metadata
    pub stats: ChunkApplyStatsV0,
    pub delayed_receipts_count: u64,
    pub proof: Option<PartialStorage>,
    pub metrics: Option<ApplyMetrics>,
    pub congestion_info: Option<CongestionInfo>,
    pub bandwidth_requests: BandwidthRequests,
    pub bandwidth_scheduler_state_hash: CryptoHash,
    pub contract_updates: ContractUpdates,
}
```

---

## PART 7: CRITICAL PATTERNS FOR BROWSER TEE

### 7.1 Determinism Requirements

For browser-based TEE to work:

1. **Deterministic Execution**
   - Same input = same output, always
   - No floating point
   - Ordered iteration over maps
   - Seeded randomness (from block_hash)

2. **Provable State Transitions**
   - State root (old + new) in every proof
   - Gas consumed tracked exactly
   - Merkle proofs must verify

3. **Async Pattern (Critical)**
   - promise_yield_create pauses execution
   - Data submitted via promise_yield_resume
   - Runtime handles coordination
   - Contract code remains synchronous

### 7.2 Storage Access Tracking

```rust
pub trait StorageAccessTracker {
    fn trie_node_touched(&mut self, count: u64) -> Result<()>;
    fn cached_trie_node_access(&mut self, count: u64) -> Result<()>;
    fn deref_write_evicted_value_bytes(&mut self, bytes: u64) -> Result<()>;
    fn deref_removed_value_bytes(&mut self, bytes: u64) -> Result<()>;
}
```

For browser TEE:
- Track every storage read/write
- Calculate TTN (trie touched nodes)
- Proof of storage access pattern
- Charge gas based on actual access

### 7.3 Validation Pattern

```
Block Validation Flow:
1. Get ApplyResult from node
   ├─ old_state_root
   └─ new_state_root

2. Verify state transition:
   ├─ Re-execute transactions/receipts
   ├─ Calculate new_state_root
   └─ Compare with provided

3. Accept block if:
   └─ Calculated == Provided
```

---

## PART 8: KEY FILES & IMPLEMENTATIONS

### Core Files for Browser Implementation

| Component | File | Lines | Purpose |
|-----------|------|-------|---------|
| **VMLogic** | `near-vm-runner/src/logic/logic.rs` | 3,500+ | Host functions impl |
| **External Trait** | `near-vm-runner/src/logic/dependencies.rs` | 600+ | Storage/receipt interface |
| **Context** | `near-vm-runner/src/logic/context.rs` | 88 | Execution environment |
| **Gas Counter** | `near-vm-runner/src/logic/gas_counter.rs` | 300+ | Gas metering |
| **RuntimeExt** | `runtime/src/ext.rs` | 600+ | External implementation |
| **Merkle Tree** | `core/primitives/src/merkle.rs` | 362 | State root calculation |
| **Receipt** | `core/primitives/src/receipt.rs` | 600+ | Receipt structures |
| **Runtime Apply** | `runtime/src/lib.rs` | 4,000+ | Receipt processing |
| **TrieKey** | `core/primitives/src/trie_key.rs` | 400+ | Storage keys |

---

## PART 9: BROWSER TEE IMPLEMENTATION CHECKLIST

### Phase 1: Core VMLogic
- [ ] Implement memory interface (bounded allocation)
- [ ] Implement registers (16 slots)
- [ ] Implement basic host functions:
  - [ ] storage_read, storage_write, storage_remove
  - [ ] context_* (read-only account/block info)
  - [ ] promise_create, promise_then
  - [ ] promise_yield_create, promise_yield_resume
  - [ ] Basic crypto (ed25519_verify, sha256)

### Phase 2: State & Storage
- [ ] Implement Merkle tree operations
- [ ] Implement TrieUpdate (in-memory state changes)
- [ ] Implement state root calculation
- [ ] Add storage access tracking

### Phase 3: Async Execution
- [ ] Implement yield receipt handling
- [ ] Implement resume data submission
- [ ] Queue promise execution
- [ ] Handle callback promises

### Phase 4: Gas Metering
- [ ] Implement gas counter with limits
- [ ] Add instruction counting (from wasmtime/wasmi)
- [ ] Calculate actual costs per operation
- [ ] Track compute usage profile

### Phase 5: Validation
- [ ] Create merkle proofs for state transitions
- [ ] Verify promise yields from off-chain system
- [ ] Validate execution outcomes
- [ ] Track TTN (storage access costs)

---

## PART 10: KEY DESIGN INSIGHTS

### 10.1 Why Promise Yield Matters

Traditional approach:
```
Contract → RPC Call → Off-chain → (WAIT) → Back to Contract
- Doesn't work on chain
- No consensus on off-chain results
```

Promise Yield approach:
```
Contract → promise_yield_create() [PAUSE]
         ↓
Off-chain System [Sees yield receipt]
         ↓
Solves problem externally
         ↓
promise_yield_resume(data_id, result) [RESUME]
         ↓
Contract receives result in callback
```

This is **NEAR's native async model** - built into the protocol!

### 10.2 Merkle Tree for State Proofs

NEAR uses binary merkle trees:
- Leaf: hash(serialized_item)
- Branch: hash(left_hash || right_hash)
- Root: hash of all state

For browser TEE:
- Prove "state X at block Y"
- Update state with new values
- Calculate new root
- Consensus on root = consensus on state

### 10.3 Gas as Proof

Gas spent = resources used:
- storage_read(1MB) = 1000x gas vs storage_read(1KB)
- Instruction count (via fuel metering) = computation
- TTN (trie touched nodes) = storage proof complexity

Browser TEE must track actual resource usage, not estimates.

### 10.4 Determinism as Foundation

NEAR is fully deterministic:
- Same input block → same state root
- Any node can validate independently
- No consensus needed on computation, only on state

Browser TEE must maintain this:
- No floating point
- Ordered data structures
- Seeded randomness
- Exact gas calculation

---

## PART 11: ATTESTATION & VERIFICATION POINTS

For browser TEE with Phala (Phase 2):

1. **Worker Attestation**
   - Prove code running in TEE
   - Sign attestation with key

2. **State Root Commitment**
   - Before: old_state_root
   - After: new_state_root
   - Signed by attested worker

3. **Proof Verification**
   - Verify state transition mathematically
   - No re-execution needed
   - Only cryptographic verification

4. **Incentive Alignment**
   - Worker bonds capital
   - Slashed if false attestation
   - Rewarded for valid proofs

---

## PART 12: COMPARISON WITH OutLayer MVP

Current MVP:
- Worker executes in plain Rust
- Uses wasmi for instruction counting
- No cryptographic proofs
- Trust via worker reputation

Browser TEE Version:
- Execute in browser (or iframe)
- Produce merkle proofs of state
- Cryptographically attest results
- Phala integration for production

Key files to adapt from MVP:
- `worker/src/executor.rs` - wasmi execution loop
- `worker/src/near_client.rs` - receipt submission
- `contract/src/lib.rs` - resource tracking

---

## SUMMARY: Architecture for Browser Implementation

```
┌─────────────────────────────────────────────────────┐
│ Browser TEE Runtime (Goal: Implement)               │
├─────────────────────────────────────────────────────┤
│                                                      │
│  Minimal VMLogic                                    │
│  ├─ 10-15 core host functions                       │
│  ├─ Memory management (bounded)                     │
│  ├─ Register storage (16 slots)                     │
│  └─ Promise queue (yield/resume)                    │
│                                                      │
│  State Management                                   │
│  ├─ TrieUpdate (in-memory changes)                 │
│  ├─ Merkle tree (state proofs)                     │
│  ├─ Storage access tracking                        │
│  └─ Gas metering with instruction counting         │
│                                                      │
│  Async Execution                                    │
│  ├─ promise_yield_create (pause)                   │
│  ├─ promise_yield_resume (resume)                  │
│  ├─ Callback queue                                 │
│  └─ Promise DAG handling                           │
│                                                      │
│  Verification                                       │
│  ├─ State root calculation                         │
│  ├─ Merkle proof generation                        │
│  ├─ Gas accounting validation                      │
│  └─ Attestation signing                            │
│                                                      │
└─────────────────────────────────────────────────────┘
```

This gives us a **production-compatible, browser-executable runtime** that:
1. Maintains NEAR protocol determinism
2. Produces cryptographic proofs
3. Supports async/off-chain execution
4. Integrates with Phala for production

---

**Generated**: November 5, 2025  
**Sources**: nearcore production runtime, v1.35+  
**Focus**: Browser TEE + Phala integration compatibility
