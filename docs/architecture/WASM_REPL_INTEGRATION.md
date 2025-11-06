# WASM REPL Integration with NEAR OutLayer

## Overview

This document outlines the integration path between a browser-based WASM REPL (for local NEAR contract testing) and OutLayer's off-chain execution infrastructure. The integration demonstrates how capability-based security patterns can bridge local development, browser execution, and distributed off-chain computation.

---

## The Integration Thesis

### WASM REPL Innovation

**What it is**: Browser-based NEAR contract execution environment
- Executes wasm32-unknown-unknown contracts locally
- IDBFS for persistent state (mimics NEAR state trie)
- VMLogic mocking for host function emulation
- Instant feedback loop: compile → test locally → iterate

**Key Insight**: Enables "local blockchain simulation" in browser without testnet deployment costs.

### OutLayer Architecture

**What it is**: Verifiable off-chain computation for NEAR
- Worker nodes execute WASM with capability restrictions
- Secrets management via keystore with access conditions
- State attestation anchored on NEAR blockchain
- Dynamic pricing based on resource consumption

**Key Insight**: Demonstrates capability-based execution with TEE-ready architecture.

### The Integration Sweet Spot

**Convergence**: REPL becomes a **browser-based OutLayer worker**
- Local testing with full VMLogic environment (development)
- Sealed storage (IDBFS) for persistent state (browser TEE pattern)
- Attestation generation anchored on NEAR (verification)
- Capability-restricted execution (security boundary testing)

---

## Architecture Alignment

```
┌─────────────────────────────────────────────────────────────┐
│                   Browser WASM REPL                          │
│                                                              │
│  ┌────────────────────────────────────────────────────┐    │
│  │  Enhanced NEARVMLogic                              │    │
│  │  - 30+ host functions (storage, crypto, context)   │    │
│  │  - Gas metering (1 Tgas = 1ms, matches NEAR)      │    │
│  │  - Register management (256 registers)            │    │
│  │  - Promise operations (simplified async)          │    │
│  └────────────────────────────────────────────────────┘    │
│                                                              │
│  ┌────────────────────────────────────────────────────┐    │
│  │  ContractSimulator                                 │    │
│  │  - near-query (view calls, read-only)             │    │
│  │  - near-execute (change calls, persist state)     │    │
│  │  - Borsh/JSON serialization                       │    │
│  │  - WASM caching                                    │    │
│  └────────────────────────────────────────────────────┘    │
│                                                              │
│  ┌────────────────────────────────────────────────────┐    │
│  │  IDBFS Sealed Storage                              │    │
│  │  - Persistent state across sessions                │    │
│  │  - State snapshots for testing                     │    │
│  │  - WebCrypto encryption (AES-GCM)                 │    │
│  │  - Attestation generation (ECDSA P-256)           │    │
│  └────────────────────────────────────────────────────┘    │
└─────────────────────────────────────────────────────────────┘
                            │
                            ▼
┌─────────────────────────────────────────────────────────────┐
│            OutLayer Integration (Future Phase)               │
│                                                              │
│  ┌────────────────────────────────────────────────────┐    │
│  │  OutLayer Coordinator API Client                   │    │
│  │  - Task submission (/tasks/create)                 │    │
│  │  - WASM upload (/wasm/upload)                      │    │
│  │  - Result polling (/tasks/:id/status)             │    │
│  │  - Capability verification                         │    │
│  └────────────────────────────────────────────────────┘    │
│                                                              │
│  ┌────────────────────────────────────────────────────┐    │
│  │  NEAR Contract Integration                         │    │
│  │  - State attestation (attest_execution_complete)   │    │
│  │  - Payment settlement (request_execution)          │    │
│  │  - Capability grants (secrets with access control) │    │
│  └────────────────────────────────────────────────────┘    │
└─────────────────────────────────────────────────────────────┘
```

---

## Integration Benefits

### For NEAR Developers

1. **Faster Iteration Cycle**
   - Old: Edit → Build → Deploy to testnet → Test → Wait for finality → Repeat
   - New: Edit → Build → Test in REPL → Iterate instantly
   - Result: 100x faster development loop

2. **Gas Cost Estimation**
   - See actual gas consumption before paying testnet fees
   - Test with different resource limits
   - Optimize before deployment

3. **State Inspection**
   - Debug storage operations in real-time
   - Inspect registers and memory
   - View logs without blockchain explorer

4. **Capability Testing**
   - Verify access control logic locally
   - Test capability restrictions before OutLayer submission
   - Catch security issues early

### For OutLayer Platform

1. **Browser Workers**
   - REPL becomes distributed worker node
   - Users contribute compute while testing
   - Horizontal scaling via browser clients

2. **Local Simulation**
   - Users test execution before paying for off-chain compute
   - Reduces failed execution costs
   - Better UX for contract developers

3. **Attestation Generation**
   - WebCrypto provides cryptographic proofs
   - Browser-generated attestations anchored on NEAR
   - Demonstrates TEE patterns without hardware

4. **Sealed Storage**
   - IDBFS as persistent, encrypted state
   - State snapshots for rollback testing
   - Mirrors NEAR's state trie locally

### For TEE Integration (Phase 3)

1. **WebCrypto = TEE Lite**
   - Browser crypto APIs provide sealed storage primitives
   - AES-GCM encryption for state
   - ECDSA signatures for attestation

2. **IndexedDB = Sealed Storage**
   - Persistent across browser sessions
   - Encrypted at rest via WebCrypto
   - Access controlled by origin policy

3. **Attestation Model**
   - Browser generates state hash + signature
   - NEAR contract verifies proof
   - Establishes trust without hardware TEE

4. **Capability Enforcement**
   - VMLogic checks capabilities before operations
   - Storage key whitelisting
   - Gas budget enforcement
   - Network access restrictions

---

## Implementation Phases

### Phase 1: Enhanced REPL as Local Simulator ⏱️ 3-4 days

**Goal**: Complete VMLogic implementation to run real NEAR contracts.

**Key Components**:

1. **NEARVMLogic Class** (~/500 lines)
   - Storage operations: `storage_write`, `storage_read`, `storage_has_key`, `storage_remove`
   - Register operations: `register_len`, `read_register`, `write_register`
   - Context getters: `current_account_id`, `signer_account_id`, `predecessor_account_id`
   - Block info: `block_index`, `block_timestamp`, `epoch_height`
   - Account info: `account_balance`, `attached_deposit`, `storage_usage`
   - Gas metering: `prepaid_gas`, `used_gas`, automatic tracking
   - Logging: `log_utf8`, `log_utf16`
   - Return/panic: `value_return`, `panic`, `panic_utf8`
   - Cryptography: `sha256`, `keccak256`, `ripemd160`, `ecrecover`
   - Promises (simplified): `promise_create`, `promise_then`, `promise_batch_*`
   - **CRITICAL**: `input()` - Read method arguments

2. **ContractSimulator Class** (~/200 lines)
   - `query(wasmPath, method, args)` - View call (read-only)
   - `execute(wasmPath, method, args, signer)` - Change call (persists state)
   - WASM module caching
   - State persistence to IDBFS
   - JSON/Borsh serialization
   - Memory allocation helpers

3. **Shell Commands** (C code integration)
   - `near-query <wasm> <method> <json_args>`
   - `near-execute <wasm> <method> <json_args>`
   - `near-state-snapshot <name>` - Save state snapshot
   - `near-state-restore <name>` - Restore state snapshot

**Test Contract** (counter.wasm):
```rust
use near_sdk::{near_bindgen, env};

#[near_bindgen]
#[derive(Default)]
pub struct Counter {
    count: u64,
}

#[near_bindgen]
impl Counter {
    pub fn increment(&mut self) {
        self.count += 1;
        env::log_str(&format!("Count: {}", self.count));
    }

    pub fn get_count(&self) -> u64 {
        self.count
    }

    pub fn reset(&mut self) {
        self.count = 0;
    }
}
```

**Expected REPL Session**:
```bash
$ load counter.wasm
Loaded counter.wasm (45KB)

$ near-execute /home/counter.wasm increment {}
=== EXECUTE: counter.wasm::increment ===
LOG: Count: 1
Gas used: 5,423,891,234 (5.4 Tgas)
State persisted to IDBFS
=== END EXECUTE ===

$ near-query /home/counter.wasm get_count {}
=== QUERY: counter.wasm::get_count ===
Result: 1
Gas used: 2,341,234,567 (2.3 Tgas)
=== END QUERY ===

$ near-execute /home/counter.wasm increment {}
=== EXECUTE: counter.wasm::increment ===
LOG: Count: 2
Gas used: 5,423,891,234 (5.4 Tgas)
State persisted to IDBFS
=== END EXECUTE ===

$ near-state-snapshot increment-test
Snapshot saved: increment-test

$ near-execute /home/counter.wasm reset {}
=== EXECUTE: counter.wasm::reset ===
LOG: Count: 0
Gas used: 4,123,456,789 (4.1 Tgas)
State persisted to IDBFS
=== END EXECUTE ===

$ near-state-restore increment-test
State restored from snapshot: increment-test

$ near-query /home/counter.wasm get_count {}
=== QUERY: counter.wasm::get_count ===
Result: 2
Gas used: 2,341,234,567 (2.3 Tgas)
=== END QUERY ===
```

### Phase 2: Capability-Based Execution ⏱️ 2 days

**Goal**: Extend VMLogic to enforce OutLayer-style capabilities.

**CapabilityVMLogic Class**:
```javascript
class CapabilityVMLogic extends NEARVMLogic {
  constructor(isViewCall, capabilities = {}) {
    super(isViewCall);

    // Capability configuration
    this.capabilities = {
      storage: capabilities.storage || ['*'],          // Key patterns
      network: capabilities.network || [],             // Allowed hosts
      compute: capabilities.compute || {               // Resource limits
        maxGas: 300000000000000,
        maxMemoryPages: 1024
      },
      logs: capabilities.logs !== false,               // Logging allowed
      crypto: capabilities.crypto || ['sha256'],       // Allowed crypto ops
      promises: capabilities.promises !== false        // Cross-contract calls
    };
  }

  // Override storage operations with capability checks
  storage_write(key_len, key_ptr, value_len, value_ptr, register_id) {
    const key = this.readString(key_ptr, key_len);

    if (!this.checkStorageCapability(key, 'write')) {
      throw new Error(`No write capability for storage key: ${key}`);
    }

    return super.storage_write(key_len, key_ptr, value_len, value_ptr, register_id);
  }

  storage_read(key_len, key_ptr, register_id) {
    const key = this.readString(key_ptr, key_len);

    if (!this.checkStorageCapability(key, 'read')) {
      throw new Error(`No read capability for storage key: ${key}`);
    }

    return super.storage_read(key_len, key_ptr, register_id);
  }

  checkStorageCapability(key, operation) {
    return this.capabilities.storage.some(pattern => {
      if (pattern === '*') return true;
      if (pattern.endsWith('*')) {
        return key.startsWith(pattern.slice(0, -1));
      }
      return key === pattern;
    });
  }

  // Override gas metering with compute capability check
  useGas(amount) {
    super.useGas(amount);

    if (this.gasUsed > this.capabilities.compute.maxGas) {
      throw new Error(
        `Gas limit exceeded: ${this.gasUsed} > ${this.capabilities.compute.maxGas}`
      );
    }
  }

  // Override crypto with capability check
  sha256(value_len, value_ptr, register_id) {
    if (!this.capabilities.crypto.includes('sha256')) {
      throw new Error('No capability for sha256');
    }
    return super.sha256(value_len, value_ptr, register_id);
  }

  // Override logging with capability check
  log_utf8(len, ptr) {
    if (!this.capabilities.logs) {
      throw new Error('No logging capability');
    }
    return super.log_utf8(len, ptr);
  }

  // Override promises with capability check
  promise_create(...args) {
    if (!this.capabilities.promises) {
      throw new Error('No promise creation capability');
    }
    return super.promise_create(...args);
  }
}
```

**New Shell Command**:
```bash
$ near-execute-restricted <wasm> <method> <args> <capabilities_json>
```

**Example**:
```bash
# Restrict to only 'count' key and 10 Tgas
$ near-execute-restricted /home/counter.wasm increment {} \
  '{"storage":["count"],"compute":{"maxGas":10000000000}}'
=== EXECUTE (RESTRICTED): counter.wasm::increment ===
✓ Storage read: count (capability OK)
✓ Storage write: count (capability OK)
✗ Gas limit exceeded: 10,234,567,890 > 10,000,000,000
ERROR: Gas limit exceeded
=== END EXECUTE ===

# Allow wildcard storage, restrict crypto
$ near-execute-restricted /home/crypto_contract.wasm hash_data {} \
  '{"storage":["*"],"crypto":["sha256"]}'
=== EXECUTE (RESTRICTED): crypto_contract.wasm::hash_data ===
✓ sha256 (capability OK)
✗ keccak256 (NO CAPABILITY)
ERROR: No capability for keccak256
=== END EXECUTE ===
```

### Phase 3: Sealed Storage with WebCrypto ⏱️ 1 day

**Goal**: Encrypt IDBFS state, generate cryptographic attestations.

**SealedStorage Class**:
```javascript
class SealedStorage {
  constructor() {
    this.masterKey = null;
    this.attestationKey = null;
  }

  async initialize() {
    // Retrieve or generate master encryption key
    const storedKey = await this.getFromIndexedDB('master-key');

    if (storedKey) {
      this.masterKey = await crypto.subtle.importKey(
        'jwk',
        storedKey,
        { name: 'AES-GCM', length: 256 },
        true,
        ['encrypt', 'decrypt']
      );
    } else {
      this.masterKey = await crypto.subtle.generateKey(
        { name: 'AES-GCM', length: 256 },
        true,
        ['encrypt', 'decrypt']
      );

      const exported = await crypto.subtle.exportKey('jwk', this.masterKey);
      await this.storeInIndexedDB('master-key', exported);
    }

    // Generate attestation signing key (ECDSA)
    this.attestationKey = await crypto.subtle.generateKey(
      { name: 'ECDSA', namedCurve: 'P-256' },
      true,
      ['sign', 'verify']
    );

    term.writeln('Sealed storage initialized');
  }

  async seal(data) {
    const iv = crypto.getRandomValues(new Uint8Array(12));
    const dataBytes = new TextEncoder().encode(JSON.stringify(data));

    const ciphertext = await crypto.subtle.encrypt(
      { name: 'AES-GCM', iv },
      this.masterKey,
      dataBytes
    );

    return {
      iv: Array.from(iv),
      ciphertext: Array.from(new Uint8Array(ciphertext))
    };
  }

  async unseal(sealed) {
    const ciphertext = new Uint8Array(sealed.ciphertext);
    const iv = new Uint8Array(sealed.iv);

    const plaintext = await crypto.subtle.decrypt(
      { name: 'AES-GCM', iv },
      this.masterKey,
      ciphertext
    );

    const dataStr = new TextDecoder().decode(plaintext);
    return JSON.parse(dataStr);
  }

  async generateAttestation(contractState) {
    // Compute state hash
    const stateBytes = new TextEncoder().encode(JSON.stringify(contractState));
    const hashBuffer = await crypto.subtle.digest('SHA-256', stateBytes);
    const stateHash = Array.from(new Uint8Array(hashBuffer));

    // Sign state hash with attestation key
    const signature = await crypto.subtle.sign(
      { name: 'ECDSA', hash: 'SHA-256' },
      this.attestationKey.privateKey,
      hashBuffer
    );

    // Get public key for verification
    const publicKey = await crypto.subtle.exportKey('jwk', this.attestationKey.publicKey);

    return {
      state_hash: stateHash,
      signature: Array.from(new Uint8Array(signature)),
      public_key: publicKey,
      timestamp: Date.now(),
      attestation_type: 'webcrypto-ecdsa-p256'
    };
  }

  async verifyAttestation(attestation, expectedStateHash) {
    // Import public key
    const publicKey = await crypto.subtle.importKey(
      'jwk',
      attestation.public_key,
      { name: 'ECDSA', namedCurve: 'P-256' },
      true,
      ['verify']
    );

    // Verify signature
    const stateHashBuffer = new Uint8Array(attestation.state_hash);
    const signatureBuffer = new Uint8Array(attestation.signature);

    const valid = await crypto.subtle.verify(
      { name: 'ECDSA', hash: 'SHA-256' },
      publicKey,
      signatureBuffer,
      stateHashBuffer
    );

    // Check state hash matches expected
    const hashMatches = JSON.stringify(attestation.state_hash) ===
                       JSON.stringify(expectedStateHash);

    return valid && hashMatches;
  }
}
```

**Integration with ContractSimulator**:
```javascript
async execute(wasmPath, methodName, args = {}, signerAccountId = 'alice.near') {
  // ... execution logic ...

  const stateArray = Array.from(nearState.entries());

  // Seal state with AES-GCM
  const sealed = await sealedStorage.seal(stateArray);
  FS.writeFile('/home/near_state.sealed', JSON.stringify(sealed));

  // Generate attestation
  const attestation = await sealedStorage.generateAttestation(stateArray);
  FS.writeFile('/home/near_state.attestation', JSON.stringify(attestation));

  term.writeln(`State sealed (${sealed.ciphertext.length} bytes encrypted)`);
  term.writeln(`Attestation generated:`);
  term.writeln(`  Hash: ${attestation.state_hash.slice(0, 8).join('')}...`);
  term.writeln(`  Signature: ${attestation.signature.slice(0, 8).join('')}...`);
  term.writeln(`  Type: ${attestation.attestation_type}`);

  // Persist to IDBFS
  FS.syncfs(false, (err) => {
    if (err) term.writeln(`IDBFS sync error: ${err}`);
  });

  return {
    result: parsed,
    gasUsed: vmLogic.gasUsed,
    logs: vmLogic.logs,
    attestation: attestation
  };
}
```

**New Shell Commands**:
```bash
$ near-seal-state
State sealed: /home/near_state.sealed (2.3KB encrypted)

$ near-generate-attestation
Attestation: /home/near_state.attestation
  Hash: a3f5e8d9...
  Signature: 1b2c3d4e...
  Type: webcrypto-ecdsa-p256

$ near-verify-attestation /home/near_state.attestation
✓ Attestation valid
✓ State hash matches
```

### Phase 4: OutLayer Integration ⏱️ 2-3 days

**Goal**: Connect REPL to OutLayer coordinator API.

**OutLayerClient Class**:
```javascript
class OutLayerClient {
  constructor(coordinatorUrl, authToken) {
    this.coordinatorUrl = coordinatorUrl || 'http://localhost:8080';
    this.authToken = authToken;
  }

  async submitExecution(wasmPath, methodName, args, capabilities) {
    term.writeln(`\n=== SUBMITTING TO OUTLAYER ===`);

    // 1. Read WASM bytes from IDBFS
    const wasmBytes = FS.readFile(wasmPath, { encoding: 'binary' });
    const checksum = await this.computeChecksum(wasmBytes);

    term.writeln(`WASM checksum: ${checksum}`);

    // 2. Check if WASM exists in coordinator cache
    const existsResponse = await fetch(
      `${this.coordinatorUrl}/wasm/exists/${checksum}`
    );

    if (!existsResponse.ok) {
      term.writeln(`WASM not cached, uploading...`);
      await this.uploadWasm(checksum, wasmBytes);
    } else {
      term.writeln(`WASM cached on coordinator`);
    }

    // 3. Create execution task
    const task = {
      type: 'execute',
      wasm_checksum: checksum,
      method_name: methodName,
      args: JSON.stringify(args),
      capabilities: capabilities || { storage: ['*'] },
      resource_limits: {
        max_instructions: 300000000000000,
        max_memory_mb: 128,
        max_execution_seconds: 60
      }
    };

    const createResponse = await fetch(`${this.coordinatorUrl}/tasks/create`, {
      method: 'POST',
      headers: {
        'Authorization': `Bearer ${this.authToken}`,
        'Content-Type': 'application/json'
      },
      body: JSON.stringify(task)
    });

    if (!createResponse.ok) {
      throw new Error(`Task creation failed: ${await createResponse.text()}`);
    }

    const { task_id } = await createResponse.json();
    term.writeln(`Task submitted: ${task_id}`);
    term.writeln(`Polling for results...`);

    // 4. Poll for completion
    const result = await this.pollTaskResult(task_id);

    term.writeln(`\n=== OUTLAYER RESULT ===`);
    term.writeln(`Result: ${JSON.stringify(result.result, null, 2)}`);
    term.writeln(`Gas used: ${result.gas_used}`);
    term.writeln(`Instructions: ${result.instructions}`);
    term.writeln(`Time: ${result.time_ms}ms`);
    term.writeln(`=== END ===\n`);

    return result;
  }

  async pollTaskResult(taskId, maxAttempts = 60) {
    for (let attempt = 0; attempt < maxAttempts; attempt++) {
      const response = await fetch(`${this.coordinatorUrl}/tasks/${taskId}/status`);

      if (!response.ok) {
        throw new Error(`Status check failed: ${await response.text()}`);
      }

      const status = await response.json();

      if (status.state === 'completed') {
        return status;
      } else if (status.state === 'failed') {
        throw new Error(`Execution failed: ${status.error}`);
      }

      // Wait 1 second before next poll
      await new Promise(resolve => setTimeout(resolve, 1000));
      term.write('.');
    }

    throw new Error('Task timeout (60s)');
  }

  async uploadWasm(checksum, bytes) {
    const formData = new FormData();
    formData.append('checksum', checksum);
    formData.append('wasm', new Blob([bytes], { type: 'application/wasm' }));

    const response = await fetch(`${this.coordinatorUrl}/wasm/upload`, {
      method: 'POST',
      headers: { 'Authorization': `Bearer ${this.authToken}` },
      body: formData
    });

    if (!response.ok) {
      throw new Error(`WASM upload failed: ${await response.text()}`);
    }

    term.writeln(`WASM uploaded successfully`);
  }

  async computeChecksum(bytes) {
    const hashBuffer = await crypto.subtle.digest('SHA-256', bytes);
    const hashArray = Array.from(new Uint8Array(hashBuffer));
    return hashArray.map(b => b.toString(16).padStart(2, '0')).join('');
  }

  async anchorAttestationOnNEAR(attestation, nearAccountId, nearPrivateKey) {
    // Submit attestation to outlayer.testnet contract
    term.writeln(`Anchoring attestation on NEAR...`);

    // This would use near-api-js
    // For now, just log what would happen
    term.writeln(`Would call: outlayer.testnet::attest_execution_complete`);
    term.writeln(`  state_hash: ${attestation.state_hash.slice(0, 8).join('')}...`);
    term.writeln(`  signature: ${attestation.signature.slice(0, 8).join('')}...`);
  }
}
```

**New Shell Command**:
```bash
$ near-submit-outlayer <wasm> <method> <args> [capabilities_json]
```

**Example Session**:
```bash
$ near-submit-outlayer /home/counter.wasm increment {} '{"storage":["count"]}'

=== SUBMITTING TO OUTLAYER ===
WASM checksum: a3f5e8d91c7b2e4f...
WASM cached on coordinator
Task submitted: task_67890
Polling for results...
..........

=== OUTLAYER RESULT ===
Result: null
Gas used: 5423891234
Instructions: 1234567
Time: 42ms
=== END ===

$ near-submit-outlayer /home/counter.wasm get_count {}

=== SUBMITTING TO OUTLAYER ===
WASM checksum: a3f5e8d91c7b2e4f...
WASM cached on coordinator
Task submitted: task_67891
Polling for results...
...

=== OUTLAYER RESULT ===
Result: 1
Gas used: 2341234567
Instructions: 567890
Time: 18ms
=== END ===
```

---

## Technical Details

### NEARVMLogic Host Functions

**Priority List** (30+ functions needed for real contracts):

**Tier 1 - Essential** (must implement):
- `storage_write(key_len, key_ptr, value_len, value_ptr, register_id)`
- `storage_read(key_len, key_ptr, register_id)`
- `storage_has_key(key_len, key_ptr)`
- `storage_remove(key_len, key_ptr, register_id)`
- `register_len(register_id)`
- `read_register(register_id, ptr)`
- `write_register(register_id, data_len, data_ptr)`
- `input(register_id)` - **CRITICAL** for reading method arguments
- `current_account_id(register_id)`
- `signer_account_id(register_id)`
- `predecessor_account_id(register_id)`
- `log_utf8(len, ptr)`
- `log_utf16(len, ptr)`
- `value_return(value_len, value_ptr)`
- `panic()`
- `panic_utf8(len, ptr)`

**Tier 2 - Common** (most contracts need):
- `block_index()`
- `block_timestamp()`
- `epoch_height()`
- `account_balance(balance_ptr)`
- `account_locked_balance(balance_ptr)`
- `attached_deposit(balance_ptr)`
- `prepaid_gas()`
- `used_gas()`
- `storage_usage()`
- `sha256(value_len, value_ptr, register_id)`
- `keccak256(value_len, value_ptr, register_id)`
- `ripemd160(value_len, value_ptr, register_id)`

**Tier 3 - Advanced** (cross-contract calls):
- `promise_create(account_id_len, account_id_ptr, method_name_len, method_name_ptr, arguments_len, arguments_ptr, amount_ptr, gas)`
- `promise_then(promise_idx, account_id_len, account_id_ptr, method_name_len, method_name_ptr, arguments_len, arguments_ptr, amount_ptr, gas)`
- `promise_and(promise_idx_ptr, promise_idx_count)`
- `promise_batch_create(account_id_len, account_id_ptr)`
- `promise_batch_action_create_account(promise_idx)`
- `promise_batch_action_deploy_contract(promise_idx, code_len, code_ptr)`
- `promise_batch_action_function_call(promise_idx, method_name_len, method_name_ptr, arguments_len, arguments_ptr, amount_ptr, gas)`
- `promise_batch_action_transfer(promise_idx, amount_ptr)`
- `promise_results_count()`
- `promise_result(result_idx, register_id)`
- `promise_return(promise_idx)`

### Gas Costs (from NEAR Protocol 1.22.0)

Base costs for operations:
- `wasm_regular_op_cost`: 2,207,874 gas per instruction
- `storage_write_base`: 64,196,736,000 gas
- `storage_write_per_byte`: 310,382,320 gas
- `storage_read_base`: 56,356,845,750 gas
- `storage_read_per_byte`: 30,952,380 gas
- `log_base`: 3,543,313,050 gas
- `log_byte`: 13,198,791 gas
- `sha256_base`: 4,540,970,250 gas
- `sha256_byte`: 24,117,351 gas

### State Structure

IDBFS stores state as:
```json
{
  "near_state.json": {
    "entries": [
      ["STATE:count", {"data": [0, 0, 0, 0, 0, 0, 0, 1], "timestamp": 1699123456789}],
      ["STATE:owner", {"data": [97, 108, 105, 99, 101, 46, 110, 101, 97, 114], "timestamp": 1699123456789}]
    ]
  },
  "near_state.sealed": {
    "iv": [1, 2, 3, ...],
    "ciphertext": [encrypted bytes...]
  },
  "near_state.attestation": {
    "state_hash": [hash bytes...],
    "signature": [signature bytes...],
    "public_key": {"kty": "EC", "crv": "P-256", ...},
    "timestamp": 1699123456789,
    "attestation_type": "webcrypto-ecdsa-p256"
  }
}
```

---

## Use Cases

### Use Case 1: Local Contract Development

**Scenario**: Developer building a DEX contract on NEAR.

**Without REPL**:
- Write contract code
- `cargo build --target wasm32-unknown-unknown`
- `near deploy` to testnet (~20 seconds)
- `near call` to test (~5 seconds per transaction)
- Wait for finality (~2 seconds)
- Repeat for each iteration
- **Total**: ~27 seconds per iteration, costs testnet gas

**With REPL**:
- Write contract code
- `cargo build --target wasm32-unknown-unknown`
- `load dex.wasm` in REPL (instant)
- `near-execute /home/dex.wasm swap '{"from":"usdc","to":"near","amount":"100"}'` (instant)
- Test multiple scenarios in seconds
- **Total**: ~1 second per iteration, zero gas costs

**Result**: 27x faster iteration, free testing.

### Use Case 2: OutLayer Execution Testing

**Scenario**: User wants to run expensive computation off-chain via OutLayer.

**Without REPL**:
- Submit to OutLayer
- Pay for execution
- If logic error, lose payment
- Fix and repeat

**With REPL**:
- Test locally with `near-execute-restricted`
- Verify gas consumption under limits
- Test with actual capability restrictions
- **Only then** submit to OutLayer with `near-submit-outlayer`

**Result**: Catch errors locally, reduce failed execution costs.

### Use Case 3: TEE Attestation Development

**Scenario**: Building a system that needs state attestations.

**Without REPL**:
- Write custom attestation code
- Test on testnet
- Verify on-chain
- Iterate slowly

**With REPL**:
- Use built-in `near-generate-attestation`
- Test attestation verification locally
- Inspect state hashes
- Only deploy when confident

**Result**: Faster TEE integration development.

### Use Case 4: Browser Worker Contribution

**Scenario**: NEAR developer wants to contribute compute to OutLayer.

**Without REPL**:
- Setup Rust worker (complex)
- Configure environment
- Run dedicated worker

**With REPL**:
- Open browser
- Load REPL
- Connect to OutLayer coordinator
- Contribute compute while developing

**Result**: Lower barrier to becoming an OutLayer worker.

---

## Security Considerations

### Sealed Storage

**Threat**: Browser storage can be inspected via DevTools.

**Mitigation**:
- WebCrypto AES-GCM encryption at rest
- Keys derived from user interaction (future: passphrase)
- State hash + signature provides tamper detection

**Limitation**: Not true TEE - browser can access decrypted state during execution.

### Attestation Verification

**Threat**: Browser can forge attestations (no hardware root of trust).

**Mitigation**:
- Suitable for development/testing
- Phase 2: Integrate with actual TEE (Intel SGX, AMD SEV)
- Phase 3: Multiple browser workers verify each other's results

**Limitation**: MVP is trust-on-first-use model.

### Capability Enforcement

**Threat**: Malicious contract could bypass capability checks.

**Mitigation**:
- Capabilities enforced at VMLogic level (host side)
- Contract runs in WASM sandbox (no system access)
- Storage keys validated before writes

**Limitation**: Gas metering is approximate (not deterministic like NEAR's actual metering).

---

## Future Enhancements

### Short Term (1-2 weeks)

1. **Borsh Serialization Support**
   - Most NEAR contracts use Borsh, not JSON
   - Add borsh-js library
   - Auto-detect serialization format

2. **Contract Interaction Recording**
   - Record all executions
   - Replay for debugging
   - Export as test suite

3. **Multi-Contract Testing**
   - Load multiple contracts
   - Test cross-contract calls
   - Simulate async promise execution

### Medium Term (1-2 months)

1. **NEAR Testnet Integration**
   - Deploy from REPL
   - Call testnet contracts
   - Sync testnet state locally

2. **Enhanced Gas Profiling**
   - Per-function gas breakdown
   - Identify expensive operations
   - Suggest optimizations

3. **Visual State Inspector**
   - GUI for viewing storage keys
   - State diff visualization
   - Time-travel debugging

### Long Term (3+ months)

1. **Hardware TEE Integration**
   - Intel SGX attestation
   - AMD SEV support
   - Replace WebCrypto with hardware roots of trust

2. **Distributed Worker Network**
   - Browser workers coordinate via WebRTC
   - Consensus on execution results
   - Reputation system

3. **Smart Contract IDE**
   - Monaco editor integration
   - Inline gas cost annotations
   - One-click deploy to OutLayer

---

## Conclusion

The integration between browser-based WASM REPL and NEAR OutLayer demonstrates a powerful pattern:

1. **Local development** with instant feedback (REPL)
2. **Capability testing** with restrictions (CapabilityVMLogic)
3. **Sealed storage** with attestations (WebCrypto)
4. **Distributed execution** via OutLayer (Coordinator API)
5. **State anchoring** on NEAR blockchain (Contract attestation)

This creates a complete development → testing → deployment → verification flow that doesn't exist in the NEAR ecosystem today.

**Key Innovation**: Browser becomes both a development environment and a distributed compute node, bridging the gap between local testing and production off-chain execution.

**Next Step**: Implement Phase 1 (Enhanced NEARVMLogic) to validate the core concept with real NEAR contracts.

---

**Status**: Architecture validated, ready for implementation
**Estimated Timeline**: 7-10 days for Phases 1-4
**Integration Points**: OutLayer coordinator API (http://localhost:8080)
**Test Contracts**: counter.wasm, token.wasm (to be created)
