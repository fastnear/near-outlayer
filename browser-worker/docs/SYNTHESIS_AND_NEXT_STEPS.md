# Architecture Synthesis and Implementation Strategy

**Integration of TEE Concepts, Linux/WASM Patterns, and NEAR Runtime**

---

## Executive Summary

This document synthesizes insights from three comprehensive architectural analyses:

1. **TEE Architecture** (hardware → browser mapping)
2. **Linux/WASM** (production WASM runtime patterns)
3. **NEAR Runtime** (production blockchain execution layer)

The synthesis reveals a clear implementation path that leverages proven patterns from each domain to build a browser-based confidential computing platform for NEAR smart contracts.

---

## 1. Architecture Convergence

### 1.1 Three-Domain Mapping

| Concept | Hardware TEE | Linux/WASM | NEAR Runtime | Our Implementation |
|---------|-------------|------------|--------------|-------------------|
| **Isolation** | TDX/SEV memory | WASM linear memory | External trait | WebAssembly sandbox |
| **Measurement** | MRTD (PCR) | Code hash | State root | Merkle tree hash |
| **Attestation** | ECDSA quote | N/A | State proof | ECDSA + Merkle path |
| **State** | Encrypted DRAM | SharedArrayBuffer | Trie storage | IndexedDB + encryption |
| **Async** | N/A | Cooperative | Promise yield | Promise yield (native!) |
| **Coordination** | Hypercalls | postMessage | Receipt DAG | Same as NEAR |

**Key Insight**: Each domain solves similar problems with different primitives. We can compose them into a coherent browser implementation.

### 1.2 Architectural Layers

```
┌─────────────────────────────────────────────────────────────┐
│ NEAR Smart Contract (WASM)                                  │
│ - Uses promise_yield for async                              │
│ - Deterministic execution                                   │
└────────────────────┬────────────────────────────────────────┘
                     │ Host functions (60+)
┌────────────────────▼────────────────────────────────────────┐
│ Browser VMLogic (Our NEARVMLogic.js)                        │
│ ├─ Storage operations → MemoryEncryptionLayer               │
│ ├─ Promise operations → PromiseQueue                        │
│ ├─ Yield detection → Pause/resume mechanism                 │
│ ├─ Gas metering → Instruction counter                       │
│ └─ Measurements → MeasurementRegistry (SharedArrayBuffer)   │
└────────────────────┬────────────────────────────────────────┘
                     │ External trait boundary
┌────────────────────▼────────────────────────────────────────┐
│ BrowserRuntimeExt (NEAR's RuntimeExt pattern)               │
│ ├─ State: Map<TrieKey, Value> (in-memory)                   │
│ ├─ Changes tracking (for merkle proof)                      │
│ ├─ Receipt queue (promise DAG)                              │
│ └─ Yield registry (data_id → callback)                      │
└────────────────────┬────────────────────────────────────────┘
                     │ State + proofs
┌────────────────────▼────────────────────────────────────────┐
│ State & Proof Layer                                         │
│ ├─ MerkleStateTree (NEAR's trie structure)                  │
│ ├─ ExecutionReceipt (Ethereum-style)                        │
│ ├─ RemoteAttestation (TEE-style ECDSA)                      │
│ └─ SealedStorage (hardware TEE pattern)                     │
└────────────────────┬────────────────────────────────────────┘
                     │ Cryptographic proofs
┌────────────────────▼────────────────────────────────────────┐
│ Verification Layer                                          │
│ ├─ NEAR L1 Contract (on-chain)                              │
│ ├─ OutLayer Coordinator (off-chain)                         │
│ └─ Phala TEE (production, Phase 4)                          │
└─────────────────────────────────────────────────────────────┘
```

---

## 2. Critical Discoveries

### 2.1 From NEAR Runtime Analysis

**Discovery**: Promise yield is native to NEAR protocol.

**Implication**: OutLayer doesn't need to hack async execution. The mechanism already exists:

```rust
// Contract code
promise_yield_create(
    "offchainvm.near",
    json!({ "task": "complex_computation" }).to_string()
)
// Returns: (receipt_index, data_id)

// Later, coordinator calls:
promise_yield_resume(data_id, result_payload)
// Contract callback executes with result
```

**Implementation Strategy**:
1. Detect `promise_yield_create` in VMLogic
2. Pause execution, serialize state
3. Return `data_id` to coordinator
4. Coordinator submits work to browser worker
5. Browser executes, generates proof
6. Coordinator calls `promise_yield_resume` with proof
7. Original contract callback receives result

**Status**: Already designed in OutLayer contract. Browser worker needs to implement the execution side.

### 2.2 From Linux/WASM Analysis

**Discovery**: SharedArrayBuffer + Atomics enable lock-free coordination.

**Application**: Measurement registry shared across contract instances:

```javascript
class SharedMeasurementRegistry {
  constructor() {
    this.lockBuffer = new SharedArrayBuffer(64);
    this.locks = new Int32Array(this.lockBuffer);
    this.PCR = new BigUint64Array(this.lockBuffer, 16, 4);
  }

  async extendPCR(index, data) {
    // Acquire lock atomically
    while (Atomics.compareExchange(this.locks, index, 0, 1) !== 0) {
      Atomics.wait(this.locks, index, 1);
    }

    // Extend measurement (append-only)
    const oldPCR = this.PCR[index];
    const newPCR = await this.hash(oldPCR, data);
    this.PCR[index] = newPCR;

    // Release and notify
    Atomics.store(this.locks, index, 0);
    Atomics.notify(this.locks, index, 1);

    return newPCR;
  }
}
```

**Benefit**: True concurrent contract execution with shared measurement state.

**Challenge**: Browser COOP/COEP headers required. Fallback to single-threaded if unavailable.

### 2.3 From TEE Architecture Analysis

**Discovery**: Measurement registers (PCRs) provide append-only audit log.

**Application**: Track all state operations for attestation:

```javascript
// In NEARVMLogic storage operations
storage_write(key, value) {
  const oldValue = this.state.get(key);
  this.state.set(key, value);

  // Extend PCR[3] (operations register)
  measurementRegistry.extend(3, {
    op: 'write',
    key: sha256(key),
    oldValueHash: sha256(oldValue),
    newValueHash: sha256(value),
    timestamp: Date.now(),
    contractId: this.context.current_account_id
  });
}
```

**Attestation Generation**:
```javascript
const attestation = {
  measurements: {
    pcr0: codeHash,        // WASM module hash
    pcr1: initialStateHash, // Genesis state
    pcr2: configHash,       // Gas limits, etc.
    pcr3: opsHash           // All operations (append-only)
  },
  nonce: verifierNonce,
  timestamp: Date.now(),
  signature: await sign(measurements, attestationKey)
};
```

**Property**: Anyone can verify execution by checking PCR[3] matches expected operations.

---

## 3. Implementation Priorities

### 3.1 High-Impact, Low-Effort (Do First)

**1. Merkle State Tree (2-3 days)**

Already have structure from NEAR primitives. Implement:

```javascript
// From nearcore/core/primitives/src/merkle.rs
function combine_hash(h1, h2) {
  return sha256(concat(h1, h2));
}

function merkle_root(items) {
  if (items.length === 0) return ZERO_HASH;
  if (items.length === 1) return items[0];

  // Pair-wise hashing
  const level = [];
  for (let i = 0; i < items.length; i += 2) {
    if (i + 1 < items.length) {
      level.push(combine_hash(items[i], items[i+1]));
    } else {
      level.push(items[i]);  // Odd element
    }
  }

  return merkle_root(level);  // Recursive
}

function generate_path(items, index) {
  // Generate proof path for item at index
  const path = [];
  // ... implementation details
  return path;
}

function verify_path(root, path, leaf) {
  let current = leaf;
  for (const [sibling, direction] of path) {
    current = direction === 'left'
      ? combine_hash(sibling, current)
      : combine_hash(current, sibling);
  }
  return current === root;
}
```

**Integration**:
```javascript
class MerkleStateTree {
  constructor(state) {
    this.items = Array.from(state.entries()).map(([k, v]) =>
      sha256(concat(k, v))
    );
  }

  root() {
    return merkle_root(this.items);
  }

  proof(key) {
    const index = this.findIndex(key);
    return generate_path(this.items, index);
  }
}
```

**Value**: State commitments for attestation.

**2. Measurement Registry with SharedArrayBuffer (2-3 days)**

Use Linux/WASM pattern:

```javascript
// measurement-registry.js
class MeasurementRegistry {
  constructor() {
    if (typeof SharedArrayBuffer !== 'undefined') {
      this.useShared = true;
      this.buffer = new SharedArrayBuffer(256);
      this.locks = new Int32Array(this.buffer, 0, 4);
      this.pcrs = new BigUint64Array(this.buffer, 32, 4);
    } else {
      this.useShared = false;
      this.pcrs = [0n, 0n, 0n, 0n];
    }
  }

  async extend(pcrIndex, data) {
    const dataHash = await sha256(JSON.stringify(data));

    if (this.useShared) {
      return this.extendAtomic(pcrIndex, dataHash);
    } else {
      return this.extendLocal(pcrIndex, dataHash);
    }
  }

  extendAtomic(pcrIndex, dataHash) {
    while (Atomics.compareExchange(this.locks, pcrIndex, 0, 1) !== 0) {
      Atomics.wait(this.locks, pcrIndex, 1, 100);
    }

    const oldPCR = this.pcrs[pcrIndex];
    const newPCR = sha256(concat(oldPCR, dataHash));
    this.pcrs[pcrIndex] = newPCR;

    Atomics.store(this.locks, pcrIndex, 0);
    Atomics.notify(this.locks, pcrIndex, 1);

    return newPCR;
  }

  extendLocal(pcrIndex, dataHash) {
    const oldPCR = this.pcrs[pcrIndex];
    const newPCR = sha256(concat(oldPCR, dataHash));
    this.pcrs[pcrIndex] = newPCR;
    return newPCR;
  }
}
```

**Value**: Atomic measurement updates, concurrent execution support.

**3. Execution Receipt (1-2 days)**

Ethereum-style execution receipt for off-chain verification:

```javascript
class ExecutionReceipt {
  constructor(execution) {
    this.contractId = execution.contractId;
    this.method = execution.method;
    this.args = execution.args;

    // Execution results
    this.result = execution.result;
    this.gasUsed = execution.gasUsed;
    this.logs = execution.logs;

    // State changes
    this.oldStateRoot = execution.oldStateRoot;
    this.newStateRoot = execution.newStateRoot;
    this.stateChanges = execution.stateChanges;

    // Proof
    this.merklePaths = execution.merklePaths;

    // Measurements
    this.measurements = {
      pcr0: execution.codeHash,
      pcr1: execution.initialStateHash,
      pcr2: execution.configHash,
      pcr3: execution.opsHash
    };

    // Metadata
    this.timestamp = Date.now();
    this.executionTime = execution.executionTime;
    this.vmVersion = "browser-tee-v1";
  }

  async sign(attestationKey) {
    const message = JSON.stringify({
      contractId: this.contractId,
      method: this.method,
      oldStateRoot: this.oldStateRoot,
      newStateRoot: this.newStateRoot,
      gasUsed: this.gasUsed,
      measurements: this.measurements,
      timestamp: this.timestamp
    });

    this.signature = await ecdsa_sign(message, attestationKey);
  }

  verify(expectedOldRoot, expectedCodeHash) {
    return (
      this.oldStateRoot === expectedOldRoot &&
      this.measurements.pcr0 === expectedCodeHash &&
      verify_ecdsa(this.signature, this.message, this.publicKey)
    );
  }

  toJSON() {
    return {
      receipt: {
        contractId: this.contractId,
        method: this.method,
        result: this.result,
        gasUsed: this.gasUsed,
        oldStateRoot: this.oldStateRoot,
        newStateRoot: this.newStateRoot,
        stateChanges: Array.from(this.stateChanges.entries()),
        merklePaths: this.merklePaths,
        measurements: this.measurements,
        timestamp: this.timestamp,
        signature: this.signature
      }
    };
  }
}
```

**Value**: Complete execution proof for verification without re-execution.

### 3.2 Medium-Impact, Medium-Effort (Do Second)

**4. Remote Attestation (3-4 days)**

Hybrid browser + OutLayer coordinator verification:

```javascript
// remote-attestation.js
class RemoteAttestation {
  constructor(mode = 'browser') {
    this.mode = mode;  // 'browser', 'outlayer-hybrid', 'phala'
    this.attestationKeyPair = null;
  }

  async initialize() {
    // Generate ephemeral or load persistent attestation key
    this.attestationKeyPair = await crypto.subtle.generateKey(
      { name: 'ECDSA', namedCurve: 'P-256' },
      true,
      ['sign', 'verify']
    );
  }

  async generateQuote(execution, nonce) {
    const quote = {
      version: 1,
      attestation_type: this.getAttestationType(),

      // Measurements
      measurements: {
        code_hash: execution.codeHash,
        state_hash: execution.newStateRoot,
        config_hash: execution.configHash,
        operations_hash: execution.opsHash
      },

      // Execution proof
      gas_used: execution.gasUsed,
      state_changes: execution.stateChanges.length,
      merkle_paths: execution.merklePaths,

      // Freshness
      nonce: nonce,
      timestamp: Date.now(),

      // Environment
      user_agent: navigator.userAgent,
      capabilities: this.getBrowserCapabilities()
    };

    // Sign quote
    const message = JSON.stringify(quote);
    const signature = await crypto.subtle.sign(
      { name: 'ECDSA', hash: 'SHA-256' },
      this.attestationKeyPair.privateKey,
      new TextEncoder().encode(message)
    );

    quote.signature = Array.from(new Uint8Array(signature));
    quote.public_key = await crypto.subtle.exportKey(
      'jwk',
      this.attestationKeyPair.publicKey
    );

    return quote;
  }

  async verifyQuote(quote, expectedMeasurements) {
    // 1. Verify signature
    const publicKey = await crypto.subtle.importKey(
      'jwk',
      quote.public_key,
      { name: 'ECDSA', namedCurve: 'P-256' },
      true,
      ['verify']
    );

    const message = JSON.stringify({
      version: quote.version,
      attestation_type: quote.attestation_type,
      measurements: quote.measurements,
      gas_used: quote.gas_used,
      state_changes: quote.state_changes,
      nonce: quote.nonce,
      timestamp: quote.timestamp,
      user_agent: quote.user_agent,
      capabilities: quote.capabilities
    });

    const signatureValid = await crypto.subtle.verify(
      { name: 'ECDSA', hash: 'SHA-256' },
      publicKey,
      new Uint8Array(quote.signature),
      new TextEncoder().encode(message)
    );

    if (!signatureValid) return false;

    // 2. Check measurements
    if (expectedMeasurements) {
      if (expectedMeasurements.code_hash &&
          quote.measurements.code_hash !== expectedMeasurements.code_hash) {
        return false;
      }
      // ... check other measurements
    }

    // 3. Check freshness (nonce + timestamp)
    const age = Date.now() - quote.timestamp;
    if (age > 60000) {  // 1 minute max
      return false;
    }

    return true;
  }

  getAttestationType() {
    switch (this.mode) {
      case 'browser': return 'browser-webcrypto-ecdsa-p256';
      case 'outlayer-hybrid': return 'outlayer-browser-hybrid';
      case 'phala': return 'phala-sgx-attestation';
      default: return 'unknown';
    }
  }

  getBrowserCapabilities() {
    return {
      sharedArrayBuffer: typeof SharedArrayBuffer !== 'undefined',
      atomics: typeof Atomics !== 'undefined',
      webCrypto: typeof crypto?.subtle !== 'undefined',
      webAssembly: typeof WebAssembly !== 'undefined',
      workers: typeof Worker !== 'undefined'
    };
  }

  async submitToOutLayer(quote) {
    if (this.mode !== 'outlayer-hybrid') {
      throw new Error('Not in hybrid mode');
    }

    // Submit to OutLayer coordinator for server-side verification
    const response = await fetch('http://coordinator/api/verify-attestation', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(quote)
    });

    return await response.json();
  }
}
```

**Value**: Cryptographic proof of correct execution.

### 3.3 High-Impact, High-Effort (Do Third)

**5. Memory Encryption Layer (4-5 days)**

Per-key encryption using WebCrypto:

```javascript
// memory-encryption-layer.js
class MemoryEncryptionLayer {
  constructor(masterKey) {
    this.masterKey = masterKey;
    this.derivedKeys = new Map();  // Cache
  }

  async deriveKey(storageKey) {
    if (this.derivedKeys.has(storageKey)) {
      return this.derivedKeys.get(storageKey);
    }

    const info = new TextEncoder().encode(`outlayer:${storageKey}`);
    const salt = new TextEncoder().encode('outlayer-kdf-v1');

    const derivedKey = await crypto.subtle.deriveKey(
      {
        name: 'HKDF',
        hash: 'SHA-256',
        salt: salt,
        info: info
      },
      this.masterKey,
      { name: 'AES-GCM', length: 256 },
      false,
      ['encrypt', 'decrypt']
    );

    this.derivedKeys.set(storageKey, derivedKey);
    return derivedKey;
  }

  async encrypt(storageKey, plaintext) {
    const key = await this.deriveKey(storageKey);
    const iv = crypto.getRandomValues(new Uint8Array(12));

    const ciphertext = await crypto.subtle.encrypt(
      { name: 'AES-GCM', iv },
      key,
      plaintext
    );

    return {
      iv: Array.from(iv),
      ciphertext: Array.from(new Uint8Array(ciphertext)),
      timestamp: Date.now()
    };
  }

  async decrypt(storageKey, encrypted) {
    const key = await this.deriveKey(storageKey);

    const plaintext = await crypto.subtle.decrypt(
      { name: 'AES-GCM', iv: new Uint8Array(encrypted.iv) },
      key,
      new Uint8Array(encrypted.ciphertext)
    );

    return new Uint8Array(plaintext);
  }
}
```

**Integration with NEARVMLogic**:
```javascript
// Modify storage operations
async storage_write(key_len, key_ptr, value_len, value_ptr, register_id) {
  const key = this.readString(key_ptr, key_len);
  const value = this.readBytes(value_ptr, value_len);

  // Encrypt before storing
  const encrypted = await memoryEncryptionLayer.encrypt(key, value);
  this.state.set(key, encrypted);

  // Extend measurement
  measurementRegistry.extend(3, { op: 'write', key, valueHash: sha256(value) });

  this.useGas(this.gasCosts.storage_write_base +
              (key.length + value.length) * this.gasCosts.storage_write_per_byte);

  return 1;  // Success
}

async storage_read(key_len, key_ptr, register_id) {
  const key = this.readString(key_ptr, key_len);

  const encrypted = this.state.get(key);
  if (!encrypted) {
    return 0;  // Not found
  }

  // Decrypt
  const plaintext = await memoryEncryptionLayer.decrypt(key, encrypted);

  // Write to register
  this.registers[register_id] = plaintext;

  this.useGas(this.gasCosts.storage_read_base +
              plaintext.length * this.gasCosts.storage_read_per_byte);

  return 1;  // Success
}
```

**Value**: Hardware TEE-style memory encryption pattern.

**Challenge**: Async WebCrypto in sync host functions. Solution: Make all host functions async or use JSPI (JavaScript Promise Integration).

---

## 4. Integration Strategy

### 4.1 Incremental Rollout

**Week 1-2**: Documentation + Merkle + Measurements
- Complete remaining documentation
- Implement MerkleStateTree
- Implement MeasurementRegistry
- Tests with counter.wasm

**Week 3-4**: Receipts + Attestation
- ExecutionReceipt implementation
- RemoteAttestation (browser mode)
- Integration tests

**Week 5-6**: Memory Encryption + Promise Yield
- MemoryEncryptionLayer
- Promise yield detection
- End-to-end async test

**Week 7-8**: OutLayer Integration
- Hybrid attestation mode
- Coordinator API integration
- Production deployment

**Week 9-12**: Phala TEE Integration (Optional)
- Replace browser attestation with Phala SGX
- Full production security
- Mainnet deployment

### 4.2 Validation Strategy

**For each component**:

1. **Unit tests**: Isolated component testing
2. **Integration tests**: Component interaction
3. **Compatibility tests**: Against real NEAR runtime
4. **Performance tests**: Gas accuracy, execution time
5. **Security tests**: Attestation verification, proof validation

**Validation tools**:
- nearcore test vectors (state root calculations)
- Gas cost comparison (browser vs node)
- Merkle proof verification against production
- Attestation signature validation

### 4.3 Fallback Strategy

If SharedArrayBuffer unavailable:
- Fall back to single-threaded execution
- Use postMessage for coordination
- Performance degradation acceptable for MVP

If WebCrypto limited:
- Use WASM crypto libraries (slower)
- Still secure, just not hardware-backed
- Consider libsodium.js or similar

---

## 5. Success Criteria

**MVP Success** (Week 4):
- [ ] Execute simple NEAR contract in browser
- [ ] Generate merkle proof of state transition
- [ ] Verify proof matches NEAR runtime
- [ ] Gas calculation within 5% of production
- [ ] Execution receipt with signature

**Production Ready** (Week 8):
- [ ] All 60+ host functions implemented
- [ ] Promise yield working
- [ ] Hybrid attestation with OutLayer
- [ ] Performance: <2x slower than native
- [ ] Security: Attestations verifiable on-chain

**Phala Integration** (Week 12):
- [ ] SGX attestation replacing browser attestation
- [ ] Hardware-backed keys
- [ ] Mainnet deployment
- [ ] Slashing conditions enforced

---

## 6. Risk Mitigation

**Risk**: Browser compatibility (SharedArrayBuffer, WebCrypto)
**Mitigation**: Feature detection + fallbacks

**Risk**: Performance (encryption overhead)
**Mitigation**: Batch operations, lazy sealing

**Risk**: Gas calculation accuracy
**Mitigation**: Use nearcore test vectors, continuous validation

**Risk**: Attestation security (browser keys not hardware)
**Mitigation**: Clear documentation, Phala upgrade path

**Risk**: State size limits (browser storage)
**Mitigation**: Chunking, WASM memory growth patterns from Linux/WASM

---

## 7. Documentation Priorities

**Remaining documents** (in order):

1. **ATTESTATION_DEEP_DIVE.md** (3-4 days)
   - Remote attestation protocols (WaTZ, RA-WEBs, Intel/AMD)
   - Browser attestation vs hardware attestation
   - Verification workflow
   - Phala integration design

2. **BLOCKCHAIN_INTEGRATION.md** (2-3 days)
   - State commitments (Merkle trees)
   - Execution receipts
   - On-chain verification
   - NEAR-specific patterns (receipts, yield/resume)

3. **CONFIDENTIAL_COMPUTING_PATTERNS.md** (2-3 days)
   - Memory encryption best practices
   - Measurement registry patterns
   - Sealed storage techniques
   - Real-world examples

**Total**: ~7-10 days for remaining documentation.

**Recommendation**: Complete documentation in parallel with implementation. Documenting forces architectural clarity.

---

## 8. Conclusion

The convergence of three architectural analyses reveals a clear path forward:

**From TEE Architecture**: Measurement registers, sealed storage, attestation patterns
**From Linux/WASM**: SharedArrayBuffer coordination, direct function pointers, memory growth
**From NEAR Runtime**: External trait boundary, promise yield, merkle proofs, deterministic gas

**Next immediate actions**:
1. Implement MerkleStateTree (2-3 days)
2. Implement MeasurementRegistry with SharedArrayBuffer (2-3 days)
3. Implement ExecutionReceipt (1-2 days)
4. Integration tests with counter.wasm (1-2 days)

**Timeline to MVP**: 2-4 weeks of focused implementation.

**Timeline to Production**: 14 weeks with Phala integration.

The foundation is solid. The patterns are proven. The path is clear. Time to build.

---

**Document Version**: 1.0
**Last Updated**: 2025-01-05
**Authors**: NEAR OutLayer Team
**Status**: Strategic Planning Document

**Referenced Documents**:
- `TEE_ARCHITECTURE.md` (800 lines)
- `LINUX_WASM_COMPAT.md` (800 lines)
- `/Users/mikepurvis/near/fn/near-outlayer/NEAR_RUNTIME_ARCHITECTURE.md` (1,027 lines)
- `/Users/mikepurvis/near/fn/near-outlayer/BROWSER_TEE_IMPLEMENTATION_ROADMAP.md` (400 lines)
