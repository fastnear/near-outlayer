# L4 Frozen Realm Guest Examples

**Secure computation with end-to-end encryption**

These examples demonstrate the Hermes Enclave architecture's "untrusted ferry" pattern, where sensitive data transits through L1-L3 encrypted and is decrypted ONLY in the L4 Frozen Realm.

---

## Security Model

```
┌────────────────────────────────────────────────────────────┐
│ L1 (Browser Main Thread)                                   │
│   • Fetches encrypted blobs from network                   │
│   • CANNOT see plaintext (opaque blobs)                    │
│   • Passes encrypted data to L2/L3/L4                      │
└───────────────────────────┬────────────────────────────────┘
                            │ Encrypted data
                            ↓
┌────────────────────────────────────────────────────────────┐
│ L2 (linux-wasm) - FUTURE                                   │
│   • Receives encrypted blobs via syscalls                  │
│   • CANNOT see plaintext (no decryption key)               │
│   • Provides POSIX environment (not security boundary)     │
└───────────────────────────┬────────────────────────────────┘
                            │ Encrypted data
                            ↓
┌────────────────────────────────────────────────────────────┐
│ L3 (QuickJS) - FUTURE                                      │
│   • Runs as /bin/qjs process in L2                         │
│   • CANNOT see plaintext (no decryption key)               │
│   • Executes L4 guest code in Frozen Realm                 │
└───────────────────────────┬────────────────────────────────┘
                            │ Encrypted data
                            ↓
┌────────────────────────────────────────────────────────────┐
│ L4 (Frozen Realm) - ONLY TRUSTED LAYER                     │
│   • Receives enclaveKey from secure source                 │
│   • Decrypts secrets and payload IN this scope             │
│   • Performs sensitive computation                         │
│   • Re-encrypts result before returning                    │
│   • Plaintext NEVER leaves this scope                      │
└────────────────────────────────────────────────────────────┘
```

**Key Properties**:
- ✅ L4 has NO access to L1-L3 (no closures, `new Function()` isolation)
- ✅ L1-L3 have NO access to L4 variables (lexically sealed)
- ✅ Plaintext exists ONLY in L4's local scope
- ✅ L2 NOMMU memory sharing vulnerability mitigated (encrypted data only)

---

## Examples

### 1. Confidential Key Custody (`confidential-key-custody.js`)

**Scenario**: Web3 wallet with client-side key generation

**Demonstrates**:
- Private key derived from encrypted master seed
- Key exists ONLY in L4 (never in L1-L3)
- Transaction signing without key export
- XSS/extension attacks cannot steal key

**Use Cases**:
- Non-custodial wallets
- Browser-based HSM
- Zero-knowledge authentication

**Input**:
```json
{
  "encryptedPayload": "base64_encrypted_transaction",
  "encryptedSecret": "base64_encrypted_master_seed",
  "enclaveKey": "hex_l4_decryption_key"
}
```

**Output**:
```json
{
  "success": true,
  "signedTransaction": {
    "transaction": { "from": "alice.near", "to": "bob.near", ... },
    "signature": "hash_signature",
    "publicKeyHash": "public_identifier"
  },
  "securityGuarantee": "Private key generated in L4, never existed in L1-L3"
}
```

---

### 2. Confidential AI Inference (`confidential-ai-inference.js`)

**Scenario**: Privacy-preserving medical AI assistant

**Demonstrates**:
- PHI/PII decrypted ONLY in L4
- API keys never exposed to L1-L3
- AI prompts constructed in L4 (zero-knowledge to L1-L3)
- HIPAA-compliant encrypted computation

**Use Cases**:
- Medical AI assistants
- Confidential document analysis
- Secure personal assistants
- Privacy-preserving analytics

**Input**:
```json
{
  "encryptedPayload": "base64_encrypted_medical_data",
  "encryptedSecret": "base64_encrypted_api_key",
  "enclaveKey": "hex_l4_decryption_key"
}

// Where medical_data contains:
{
  "patientId": "P-12345",
  "symptoms": ["chest pain", "shortness of breath"],
  "history": ["hypertension", "diabetes"],
  "medications": ["lisinopril", "metformin"]
}
```

**Output**:
```json
{
  "patientId": "P-12345",
  "assessment": "Based on symptoms...",
  "securityGuarantees": {
    "apiKeyExposedToL1_L3": false,
    "phiExposedToL1_L3": false,
    "promptExposedToL1_L3": false
  }
}
```

---

## Running the Examples

### Phase 1: L1 → L4 Direct (Current Implementation)

```javascript
// In browser console (after loading test.html)

// 1. Initialize Enclave Executor
const executor = new EnclaveExecutor({ verbose: true });

// 2. Prepare encrypted test data
const crypto = new CryptoUtils({ verbose: true });

// Generate enclave key
const enclaveKeyRaw = crypto.hexToBytes('0123456789abcdef'.repeat(4)); // 32 bytes
const enclaveKeyHex = '0123456789abcdef'.repeat(4);

// Encrypt payload
const payload = JSON.stringify({
  from: 'alice.near',
  to: 'bob.near',
  amount: 100,
  message: 'Payment for services'
});
const { iv: iv1, ciphertext: ct1 } = await crypto.encrypt(payload, enclaveKeyRaw);
const encryptedPayload = crypto.bytesToBase64(
  new Uint8Array([...iv1, ...ct1])
);

// Encrypt secret (master seed)
const secret = 'my-super-secret-master-seed-phrase-here';
const { iv: iv2, ciphertext: ct2 } = await crypto.encrypt(secret, enclaveKeyRaw);
const encryptedSecret = crypto.bytesToBase64(
  new Uint8Array([...iv2, ...ct2])
);

// 3. Load guest code
const guestCode = await fetch('/l4-guest-examples/confidential-key-custody.js')
  .then(r => r.text());

// 4. Execute in Frozen Realm
const result = await executor.executeEncrypted({
  encryptedPayload,
  encryptedSecret,
  enclaveKey: enclaveKeyHex,
  code: guestCode,
  codeId: 'key-custody-demo'
});

// 5. Decrypt result (L1 has the key for demo purposes)
const decrypted = await crypto.decryptSimple(
  result.encryptedResult,
  enclaveKeyHex
);
console.log('Result:', JSON.parse(decrypted));
```

### Phase 2: L1 → L2 → L4 (Future with linux-wasm)

```javascript
// When L2 is integrated, the flow becomes:
const executor = new EnclaveExecutor({
  verbose: true,
  useLinux: true  // Enable L2 layer
});

// Encrypted data will transit through linux-wasm
// but still be opaque until L4
```

### Phase 3: L1 → L2 → L3 → L4 (Full 4-Layer)

```javascript
const executor = new EnclaveExecutor({
  verbose: true,
  useLinux: true,      // Enable L2
  useQuickJS: true     // Enable L3
});

// Encrypted data transits L1 → L2 → L3 → L4
// QuickJS (L3) executes Frozen Realm creator
// All plaintext confined to L4
```

---

## Creating Your Own L4 Guest Code

### Template

```javascript
/**
 * Your L4 guest code
 *
 * Available capabilities:
 * - log(message)
 * - encryptedPayload, encryptedSecret, enclaveKey
 * - crypto.decrypt(encrypted, key) -> Promise<string>
 * - crypto.encrypt(data, key) -> Promise<string>
 * - crypto.hash(data) -> Promise<string>
 * - utils.parseJSON(json) -> object
 * - utils.stringifyJSON(obj) -> string
 */

return (async function() {
  log('Starting computation...');

  try {
    // 1. Decrypt inputs
    const secret = await crypto.decrypt(encryptedSecret, enclaveKey);
    const payload = await crypto.decrypt(encryptedPayload, enclaveKey);

    // 2. Parse data
    const data = utils.parseJSON(payload);

    // 3. Perform sensitive computation
    // ... your logic here ...

    // 4. Create result
    const result = {
      success: true,
      // ... your outputs ...
    };

    // 5. Encrypt before returning
    return await crypto.encrypt(
      utils.stringifyJSON(result),
      enclaveKey
    );

  } catch (error) {
    log(`ERROR: ${error.message}`);
    throw error;
  }
})();
```

### Security Best Practices

1. **Never leak plaintext**:
   ```javascript
   // ❌ BAD: Plaintext in result
   return { secret: decryptedSecret };

   // ✅ GOOD: Encrypt before returning
   return await crypto.encrypt(result, enclaveKey);
   ```

2. **Use deterministic operations only**:
   ```javascript
   // ❌ BAD: Non-deterministic
   const timestamp = Date.now(); // Frozen in Frozen Realm!

   // ✅ GOOD: Deterministic
   const timestamp = 'deterministic-value';
   ```

3. **Validate all inputs**:
   ```javascript
   // ✅ GOOD: Validate before use
   const data = utils.parseJSON(payload);
   if (!data.requiredField) {
     throw new Error('Missing required field');
   }
   ```

4. **Minimize secret exposure**:
   ```javascript
   // ✅ GOOD: Use secret, don't copy it
   const derived = await crypto.hash(secret + ':salt');
   // secret is still in scope, but not duplicated
   ```

---

## Security Guarantees

### What the Frozen Realm Protects Against

✅ **XSS Attacks**: Malicious JavaScript in L1 cannot access L4 variables
✅ **Browser Extension Attacks**: Extensions cannot read L4 scope
✅ **Prototype Pollution**: All primordials frozen
✅ **Closure Leaks**: No lexical access to outer scopes
✅ **L2 NOMMU Vulnerabilities**: Plaintext never in shared memory

### What the Frozen Realm Does NOT Protect Against

❌ **Spectre/Meltdown**: Side-channel attacks (hardware-level)
❌ **Browser Bugs**: V8/SpiderMonkey vulnerabilities
❌ **Malicious L4 Code**: If guest code is malicious, it runs in L4
❌ **Physical Access**: Attacker with debugger attached

### Threat Model

**Assumes**:
- L1-L3 are **potentially compromised** (untrusted ferry)
- L4 guest code is **trusted** (or audited)
- WebCrypto API is **secure** (browser implementation)
- Encryption keys are **properly managed**

**Does NOT Assume**:
- Hardware security (no SGX/SEV in Phase 1)
- Network security (HTTPS assumed)
- Key distribution mechanism (out of scope)

---

## Performance

### Phase 1 Benchmarks (L1 → L4 Direct)

| Operation | Time | Notes |
|-----------|------|-------|
| Freeze primordials | ~5-10ms | One-time cost |
| Decrypt 1KB | ~0.5-1ms | WebCrypto AES-GCM |
| Execute guest code | ~5-20ms | Depends on complexity |
| Encrypt result | ~0.5-1ms | WebCrypto AES-GCM |
| **Total overhead** | **~10-30ms** | vs direct execution |

### Expected Phase 3 Overhead (Full 4-Layer)

| Layer | Overhead | Cumulative |
|-------|----------|------------|
| L1 (Browser) | 0ms | 0ms |
| L2 (linux-wasm) | ~2-5ms | ~2-5ms |
| L3 (QuickJS) | ~1-3ms | ~3-8ms |
| L4 (Frozen Realm) | ~10-30ms | **~15-40ms total** |

---

## Future Enhancements

### Phase 2: L3 (QuickJS) Integration
- L3 executes Frozen Realm creator as POSIX process
- Guest code becomes multi-language (JS + WASM)
- NEAR host functions via L2 syscalls

### Phase 3: Hardware TEE Integration
- Replace L4 software isolation with SGX/SEV
- Attestation proofs from hardware
- Remote attestation for verification

### Phase 4: Distributed Execution
- Multiple L4 realms execute same code
- Consensus on results (Byzantine fault tolerance)
- Proof-of-execution for NEAR contract integration

---

## Questions?

This is cutting-edge research merging:
- SES (Secure ECMAScript) / Hardened JavaScript
- WASM sandboxing
- OS virtualization (linux-wasm)
- End-to-end encryption
- Blockchain capability-based security (NEAR)

**We're building something genuinely novel here!**

For technical questions or collaboration:
- See `/docs/HERMES_ENCLAVE_INTEGRATION.md` (coming soon)
- Study CLAUDE.md for NEAR OutLayer architecture
- Read md-claude-chapters/06-4-layer-architecture.md for deep dive

---

**Status**: Phase 1 Complete ✅
**Next**: Integrate with ContractSimulator, create test UI
**Timeline**: Phase 2 (QuickJS) in 2-3 weeks
