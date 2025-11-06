# Remote Attestation - Deep Dive and Browser Implementation

**Date**: November 5, 2025
**Audience**: Principal engineers implementing browser-based TEE attestation
**Scope**: Remote attestation protocols, verification workflows, and hybrid browser/coordinator approaches

---

## Table of Contents

1. [Attestation Fundamentals](#1-attestation-fundamentals)
2. [Hardware Attestation Protocols](#2-hardware-attestation-protocols)
3. [Browser Attestation Research](#3-browser-attestation-research)
4. [OutLayer's Hybrid Approach](#4-outlayers-hybrid-approach)
5. [Implementation Architecture](#5-implementation-architecture)
6. [Verification Workflow](#6-verification-workflow)
7. [Integration Points](#7-integration-points)
8. [Security Analysis](#8-security-analysis)
9. [Implementation Roadmap](#9-implementation-roadmap)

---

## 1. Attestation Fundamentals

### 1.1 What is Remote Attestation?

Remote attestation allows a verifier to establish trust in a remote system's state without physical access. The core question answered:

> "Is this system running the expected software in a trusted environment?"

**Three Essential Components**:

1. **Measurement**: Cryptographic hash of software/configuration state
2. **Quote Generation**: Signed evidence binding measurements to hardware identity
3. **Verification**: Cryptographic validation of quote against known-good values

### 1.2 Attestation Trust Model

```
Hardware Root of Trust
  ↓
Platform Firmware (measured)
  ↓
Bootloader (measured)
  ↓
Operating System (measured)
  ↓
Application (measured)
  ↓
Quote = Sign(measurements, nonce) with Hardware Key
  ↓
Verifier checks: signature valid + measurements match expected
```

**Key Properties**:
- **Freshness**: Nonce prevents replay attacks
- **Authenticity**: Hardware signature proves origin
- **Integrity**: Measurements prove software state
- **Non-repudiation**: Can't deny generating a quote

### 1.3 Platform Configuration Registers (PCRs)

PCRs are append-only measurement logs implemented in hardware (TPM, TDX, SEV):

```
PCR[0] = hash(firmware_code)
PCR[1] = hash(PCR[0] || bootloader_code)
PCR[2] = hash(PCR[1] || os_kernel)
PCR[3] = hash(PCR[2] || application)
```

**Extend Operation** (only operation allowed):
```
PCR_new = SHA256(PCR_old || new_measurement)
```

**Why Append-Only?**
- Creates tamper-evident audit trail
- Any change to boot sequence produces different final PCR
- Verifier can check entire chain with one final hash

---

## 2. Hardware Attestation Protocols

### 2.1 Intel TDX Remote Attestation

**Architecture**:
```
Trust Domain (TD)
  ↓ Request Quote
TDX Module (CPU microcode)
  ↓ Generate TDREPORT
Quoting Enclave (SGX)
  ↓ Sign with Intel EPID/DCAP
Remote Verifier
  ↓ Verify Intel Certificate Chain
Accept/Reject TD
```

**TD Report Structure** (TDX 1.5):
```c
struct tdx_report {
    uint8_t  report_type;           // 0x81 = TDX
    uint8_t  reserved[15];

    // Measurements
    uint8_t  mr_td[48];             // Hash of initial TD state
    uint8_t  mr_config_id[48];      // Config digest
    uint8_t  mr_owner[48];          // Owner identity
    uint8_t  mr_owner_config[48];   // Owner config
    uint8_t  rt_mr[4][48];          // Runtime measurements (4 registers)

    // Report data (custom challenge)
    uint8_t  report_data[64];       // Nonce from verifier

    // Signature (ECDSA P-384)
    uint8_t  signature[96];
};
```

**Verification Steps**:
1. Verify Intel certificate chain (root → intermediate → attestation key)
2. Check signature on TD Report
3. Validate `mr_td` matches expected TD image hash
4. Verify `report_data` contains expected nonce
5. Check TCB (Trusted Computing Base) version is patched

**Key Insight**: Hardware root of trust (Intel CPU) signs measurements, creating unforgeable proof.

### 2.2 AMD SEV-SNP Attestation

**Architecture**:
```
Guest VM
  ↓ VMGEXIT
AMD PSP (Platform Security Processor)
  ↓ Generate Attestation Report
Remote Verifier
  ↓ Verify AMD Certificate Chain
Accept/Reject VM
```

**SNP Attestation Report Structure**:
```c
struct snp_attestation_report {
    uint32_t version;               // 0x02 for SNP
    uint32_t guest_svn;             // Guest security version
    uint64_t policy;                // VM policy flags

    uint8_t  family_id[16];         // VM family identifier
    uint8_t  image_id[16];          // VM image digest
    uint8_t  vmpl;                  // VM privilege level

    uint8_t  measurement[48];       // SHA-384 of initial state
    uint8_t  host_data[32];         // Hypervisor-provided
    uint8_t  id_key_digest[48];     // Identity key hash
    uint8_t  author_key_digest[48]; // Author key hash

    uint8_t  report_data[64];       // Nonce from verifier

    uint8_t  signature[512];        // ECDSA P-384 signature
};
```

**Measurement Calculation**:
```
measurement = SHA384(
    gctx.ld || gctx.guest_svn || gctx.policy ||
    gctx.family_id || gctx.image_id || gctx.vmpl ||
    initial_vm_state
)
```

**Verification Steps**:
1. Verify AMD ARK (AMD Root Key) → ASK (AMD Signing Key) → VCEK (Versioned Chip Endorsement Key) chain
2. Check signature with VCEK public key
3. Validate `measurement` matches expected VM image
4. Verify `guest_svn` meets minimum security version
5. Check `policy` flags (e.g., debug disabled, migration disabled)

**Key Difference from TDX**: Entire VM measured (not just process), hypervisor can provide `host_data`.

### 2.3 ARM TrustZone Attestation

ARM doesn't mandate attestation (platform-specific), but common patterns:

**PSA (Platform Security Architecture) Attestation**:
```
Normal World App
  ↓ Secure Monitor Call (SMC)
Secure World (TrustZone)
  ↓ Generate Token
PSA Attestation Service
  ↓ Sign with Device Key
Remote Verifier
```

**PSA Attestation Token** (CBOR-encoded):
```json
{
  "eat_profile": "PSA_IOT_1",
  "psa_client_id": 1,
  "psa_security_lifecycle": 12288,
  "psa_implementation_id": "...",
  "psa_boot_seed": "...",
  "psa_instance_id": "...",

  "measurements": [
    { "type": "BL", "value": "hash_of_bootloader" },
    { "type": "PRoT", "value": "hash_of_secure_runtime" },
    { "type": "ARoT", "value": "hash_of_app_runtime" }
  ],

  "psa_nonce": "...",
  "signature": "..."
}
```

**Verification**: Check signature with device's attestation public key (enrolled during manufacturing).

---

## 3. Browser Attestation Research

### 3.1 WaTZ: WebAssembly Trusted Zone

**Paper**: "WaTZ: A Trusted WebAssembly Runtime Environment with Remote Attestation for TrustZone" (2022)

**Key Idea**: Bridge ARM TrustZone to browser via modified JavaScript engine.

**Architecture**:
```
Browser JavaScript
  ↓ postMessage
WaTZ Runtime (Secure World)
  ↓ Execute WASM in TrustZone
ARM TrustZone
  ↓ Generate PSA Token
Remote Verifier
```

**Attestation Flow**:
1. WASM module loaded into TrustZone secure world
2. Measurement = SHA256(wasm_bytecode)
3. PSA token generated with measurement
4. Token sent to verifier via normal world

**Limitations**:
- Requires modified browser (not deployable to standard Chrome/Firefox)
- ARM-only (no Intel/AMD support)
- Complex integration with browser security model

**Relevance to OutLayer**: Demonstrates that browser WASM + attestation is feasible, but requires platform support.

### 3.2 RA-WEBs: Remote Attestation for Web-Based Systems

**Paper**: "RA-WEBs: Resource Attestation for Web-Based Systems" (2021)

**Key Idea**: Attestation without hardware TEE, using reproducible builds + threshold signatures.

**Architecture**:
```
Web Application
  ↓ Build with deterministic compiler
Reproducible Build
  ↓ Multiple parties independently build
N Build Servers (M-of-N threshold)
  ↓ Sign build hash
Remote Verifier
  ↓ Check M signatures match
Accept if ≥M match
```

**Attestation Protocol**:
1. Source code published to public repository (e.g., GitHub commit hash)
2. N independent build servers compile source
3. Each server calculates `build_hash = SHA256(output_wasm)`
4. If ≥M servers produce same hash, threshold signature generated
5. Verifier checks: source → build → hash → signatures

**Trust Model**: No single party trusted; requires M-of-N collusion to forge.

**Limitations**:
- Doesn't attest runtime state (only build integrity)
- Threshold infrastructure complex
- Doesn't prevent runtime tampering

**Relevance to OutLayer**: Could validate WASM compilation reproducibility.

### 3.3 WebAuthn and Attestation

**WebAuthn Attestation** (W3C standard):
```javascript
const credential = await navigator.credentials.create({
  publicKey: {
    challenge: new Uint8Array(32),
    rp: { name: "OutLayer" },
    user: { id: userId, name: "worker@outlayer.io", displayName: "Worker" },
    pubKeyCredParams: [{ type: "public-key", alg: -7 }],
    attestation: "direct"  // Request attestation
  }
});

// credential.response.attestationObject contains:
// - fmt: "packed" | "tpm" | "android-key" | "apple" | ...
// - attStmt: signature over authenticatorData + clientDataHash
// - authenticatorData: RP ID hash, flags, counter, credential public key
```

**Attestation Formats**:
- **Packed**: FIDO self-attestation or X.509 certificate
- **TPM**: TPM 2.0 attestation with AIK (Attestation Identity Key)
- **Android Key**: Android KeyStore attestation
- **Apple**: Apple Anonymous Attestation

**Trust Model**: Authenticator's private key never leaves hardware.

**Limitations for OutLayer**:
- Attests authenticator hardware (not execution environment)
- Doesn't measure WASM code or state
- User gesture required (not suitable for automated workers)

**Relevance**: Could bind worker identity to hardware token, but insufficient for execution attestation.

### 3.4 Chrome Origin Trials - Trust Token API

**Trust Token API** (Privacy Pass protocol):
```javascript
// Issuer signs tokens
await fetch('https://issuer.example', {
  trustToken: {
    type: 'token-request',
    issuer: 'https://issuer.example'
  }
});

// Later, redeem token to prove prior interaction
await fetch('https://rp.example', {
  trustToken: {
    type: 'token-redemption',
    issuer: 'https://issuer.example',
    refreshPolicy: 'none'
  }
});
```

**Key Properties**:
- Unlinkable tokens (can't correlate redemption to issuance)
- Blind signatures (issuer doesn't learn token content)

**Limitations for OutLayer**:
- Proves browser interacted with issuer (not execution state)
- Privacy-focused (intentionally hides details)
- Chrome-only experimental API

**Relevance**: Could prove worker browser is legitimate, but not sufficient for attestation.

---

## 4. OutLayer's Hybrid Approach

### 4.1 The Core Challenge

**Problem**: Standard browsers lack hardware attestation support.

**Cannot Do** (without browser modifications):
- Access TPM/TDX/SEV from JavaScript
- Generate hardware-backed quotes
- Prove execution environment integrity to cryptographic standard

**Can Do**:
- WebCrypto for signing (ECDSA P-256/P-384)
- IndexedDB for persistent key storage
- WASM for deterministic execution
- Measurement tracking in JavaScript

### 4.2 Hybrid Architecture

**Three-Layer Trust Model**:

```
┌─────────────────────────────────────────────┐
│ Layer 1: Browser-Generated Evidence         │
│ - Execution measurements (PCR-style)        │
│ - WASM bytecode hash                        │
│ - State commitments (Merkle root)          │
│ - Timing metadata                           │
│ - Signed with WebCrypto ECDSA key          │
└─────────────────┬───────────────────────────┘
                  ↓
┌─────────────────────────────────────────────┐
│ Layer 2: Coordinator Validation             │
│ - Verifies browser signature                │
│ - Checks measurements against known-good    │
│ - Validates state transitions               │
│ - Issues coordinator-signed attestation     │
│ - Detects anomalies (timing, resource use)  │
└─────────────────┬───────────────────────────┘
                  ↓
┌─────────────────────────────────────────────┐
│ Layer 3: On-Chain Verification (Optional)   │
│ - NEAR contract validates coordinator sig   │
│ - Checks state root against Merkle proofs   │
│ - Slashing for invalid attestations         │
│ - Economic incentives for honesty           │
└─────────────────────────────────────────────┘
```

### 4.3 Trust Assumptions

**What We Assume**:
1. **WebCrypto Integrity**: Browser implements ECDSA correctly (reasonable - battle-tested)
2. **WASM Determinism**: Same bytecode + input → same output (guaranteed by spec)
3. **Coordinator Honesty**: Coordinator correctly validates (auditable, replaceable)
4. **Economic Rationality**: Workers prefer rewards over penalties (game theory)

**What We DON'T Assume**:
- Browser hasn't been tampered with (worker could modify browser)
- Worker's OS is trusted (could be running malware)
- Network is secure (could MITM coordinator)

**Mitigation Strategy**:
- Multiple independent workers execute same task
- Coordinator compares results (consensus)
- Slashing for divergence (economic penalty)
- Reproducible builds allow verification

### 4.4 Comparison to Hardware TEE

| Property | Hardware TEE | OutLayer Browser | Mitigation |
|----------|--------------|------------------|------------|
| **Memory Encryption** | AES-128 in CPU | WebCrypto AES-GCM | Software encryption of state |
| **Measurement** | PCR in hardware | JavaScript registry | SharedArrayBuffer atomics |
| **Attestation Signature** | CPU private key | WebCrypto key in IndexedDB | Coordinator co-signs |
| **Replay Protection** | Hardware nonce | Coordinator-provided challenge | Verified by coordinator |
| **Tampering Detection** | Measurement changes | Measurement changes | Same mechanism |
| **Root of Trust** | CPU manufacturer | Browser + Coordinator | Multi-party trust |

**Key Insight**: We trade **hardware guarantees** for **verifiable computation** + **economic incentives**.

---

## 5. Implementation Architecture

### 5.1 Browser Worker Components

#### Measurement Registry

```javascript
class MeasurementRegistry {
  constructor() {
    // PCR-style registers (SharedArrayBuffer for multi-worker sync)
    this.pcrs = new BigUint64Array(new SharedArrayBuffer(8 * 8)); // 8 PCRs
    this.measurementLog = []; // Detailed audit log
    this.lock = new Int32Array(new SharedArrayBuffer(4));
  }

  /**
   * Extend PCR with new measurement (append-only)
   * PCR_new = SHA256(PCR_old || measurement)
   */
  async extend(pcrIndex, measurement, description) {
    // Atomic lock acquisition
    while (Atomics.compareExchange(this.lock, 0, 0, 1) !== 0) {
      Atomics.wait(this.lock, 0, 1, 100); // Wait max 100ms
    }

    try {
      // Read current PCR value
      const currentPcr = this.pcrs[pcrIndex];

      // Calculate new PCR: hash(old || new)
      const oldBytes = new Uint8Array(8);
      new DataView(oldBytes.buffer).setBigUint64(0, currentPcr, true);

      const combined = new Uint8Array(oldBytes.length + measurement.length);
      combined.set(oldBytes, 0);
      combined.set(measurement, oldBytes.length);

      const hashBuffer = await crypto.subtle.digest('SHA-256', combined);
      const hashArray = new Uint8Array(hashBuffer);

      // Store first 8 bytes as BigUint64 (truncated hash)
      const newPcrValue = new DataView(hashArray.buffer).getBigUint64(0, true);
      this.pcrs[pcrIndex] = newPcrValue;

      // Log for audit trail
      this.measurementLog.push({
        timestamp: Date.now(),
        pcr: pcrIndex,
        measurement: Array.from(hashArray),
        description: description,
        previousValue: currentPcr.toString(16),
        newValue: newPcrValue.toString(16)
      });

      return newPcrValue;
    } finally {
      // Release lock
      Atomics.store(this.lock, 0, 0);
      Atomics.notify(this.lock, 0, 1);
    }
  }

  /**
   * Get current PCR values (read-only)
   */
  getPCRs() {
    return {
      pcr0: this.pcrs[0].toString(16), // WASM bytecode
      pcr1: this.pcrs[1].toString(16), // Input data
      pcr2: this.pcrs[2].toString(16), // Configuration
      pcr3: this.pcrs[3].toString(16), // Runtime state
      pcr4: this.pcrs[4].toString(16), // Reserved
      pcr5: this.pcrs[5].toString(16), // Reserved
      pcr6: this.pcrs[6].toString(16), // Reserved
      pcr7: this.pcrs[7].toString(16), // Reserved
    };
  }

  /**
   * Get full audit log
   */
  getAuditLog() {
    return this.measurementLog;
  }
}
```

#### Quote Generation

```javascript
class AttestationQuote {
  constructor(measurementRegistry, signingKey) {
    this.measurements = measurementRegistry;
    this.signingKey = signingKey; // ECDSA P-256 private key from WebCrypto
  }

  /**
   * Generate attestation quote (similar to TDX/SEV)
   */
  async generate(nonce, reportData = {}) {
    // 1. Gather current measurements
    const pcrs = this.measurements.getPCRs();

    // 2. Create quote structure
    const quote = {
      version: 1,
      timestamp: Date.now(),
      nonce: Array.from(nonce), // Freshness from verifier

      // Measurements (PCR values)
      measurements: pcrs,

      // Additional report data
      report_data: {
        wasm_hash: reportData.wasmHash,
        input_hash: reportData.inputHash,
        state_root: reportData.stateRoot,
        gas_used: reportData.gasUsed,
        ...reportData
      },

      // Worker identity
      worker_id: reportData.workerId,

      // Audit trail
      measurement_log: this.measurements.getAuditLog()
    };

    // 3. Serialize quote
    const quoteJson = JSON.stringify(quote, null, 2);
    const quoteBytes = new TextEncoder().encode(quoteJson);

    // 4. Sign with worker's ECDSA key
    const signatureBuffer = await crypto.subtle.sign(
      { name: 'ECDSA', hash: 'SHA-256' },
      this.signingKey,
      quoteBytes
    );

    const signature = Array.from(new Uint8Array(signatureBuffer));

    // 5. Return signed quote
    return {
      quote: quote,
      signature: signature,
      signature_algorithm: 'ECDSA-P256-SHA256'
    };
  }

  /**
   * Verify quote signature (for coordinator/verifier)
   */
  static async verify(signedQuote, publicKey) {
    const quoteJson = JSON.stringify(signedQuote.quote, null, 2);
    const quoteBytes = new TextEncoder().encode(quoteJson);
    const signatureBuffer = new Uint8Array(signedQuote.signature).buffer;

    const valid = await crypto.subtle.verify(
      { name: 'ECDSA', hash: 'SHA-256' },
      publicKey,
      signatureBuffer,
      quoteBytes
    );

    return valid;
  }
}
```

### 5.2 Coordinator Components

#### Quote Verification Service

```javascript
class QuoteVerifier {
  constructor(trustedPCRs, workerRegistry) {
    this.trustedPCRs = trustedPCRs; // Known-good PCR values
    this.workerRegistry = workerRegistry; // Maps worker_id → public_key
    this.nonceCache = new Map(); // Track used nonces
  }

  /**
   * Verify worker's attestation quote
   */
  async verifyQuote(signedQuote) {
    const errors = [];

    // 1. Verify signature
    const workerId = signedQuote.quote.worker_id;
    const workerPubKey = await this.workerRegistry.getPublicKey(workerId);

    if (!workerPubKey) {
      errors.push(`Unknown worker: ${workerId}`);
      return { valid: false, errors };
    }

    const signatureValid = await AttestationQuote.verify(
      signedQuote,
      workerPubKey
    );

    if (!signatureValid) {
      errors.push('Invalid signature');
      return { valid: false, errors };
    }

    // 2. Check nonce freshness (prevent replay)
    const nonce = signedQuote.quote.nonce.join(',');
    if (this.nonceCache.has(nonce)) {
      errors.push('Nonce reused (replay attack)');
      return { valid: false, errors };
    }
    this.nonceCache.set(nonce, Date.now());

    // 3. Verify measurements against known-good values
    const measurements = signedQuote.quote.measurements;

    // PCR0: WASM bytecode hash
    if (measurements.pcr0 !== this.trustedPCRs.wasmBytecode) {
      errors.push(`PCR0 mismatch: expected ${this.trustedPCRs.wasmBytecode}, got ${measurements.pcr0}`);
    }

    // PCR1: Input data hash
    const expectedInputHash = await this.calculateInputHash(
      signedQuote.quote.report_data.input_hash
    );
    if (measurements.pcr1 !== expectedInputHash) {
      errors.push(`PCR1 mismatch (input data)`);
    }

    // PCR3: Runtime state (optional check)
    // Could verify state transitions are valid

    // 4. Check timestamp (not too old, not in future)
    const now = Date.now();
    const timestamp = signedQuote.quote.timestamp;
    const maxAge = 5 * 60 * 1000; // 5 minutes

    if (timestamp > now + 60000) {
      errors.push('Quote timestamp in future');
    }
    if (now - timestamp > maxAge) {
      errors.push('Quote expired');
    }

    // 5. Validate audit log integrity
    const logValid = this.verifyAuditLog(signedQuote.quote.measurement_log);
    if (!logValid) {
      errors.push('Audit log integrity check failed');
    }

    return {
      valid: errors.length === 0,
      errors: errors,
      worker_id: workerId,
      measurements: measurements
    };
  }

  /**
   * Verify measurement log is consistent
   */
  verifyAuditLog(log) {
    // Check each extend operation produces correct new PCR
    for (let i = 0; i < log.length; i++) {
      const entry = log[i];
      // Verify: SHA256(previous || measurement) === new
      // (Implementation omitted for brevity)
    }
    return true;
  }

  /**
   * Issue coordinator attestation
   */
  async issueCoordinatorAttestation(verifiedQuote, coordinatorKey) {
    const attestation = {
      version: 1,
      timestamp: Date.now(),
      worker_quote: verifiedQuote,
      coordinator_verification: {
        verified_at: Date.now(),
        verification_result: 'PASS',
        trusted_measurements: true
      }
    };

    const attestationJson = JSON.stringify(attestation);
    const attestationBytes = new TextEncoder().encode(attestationJson);

    const signature = await crypto.subtle.sign(
      { name: 'ECDSA', hash: 'SHA-256' },
      coordinatorKey,
      attestationBytes
    );

    return {
      attestation: attestation,
      signature: Array.from(new Uint8Array(signature)),
      signature_algorithm: 'ECDSA-P256-SHA256'
    };
  }
}
```

### 5.3 Integration with Existing Keystore

OutLayer already has a keystore worker (`/keystore-worker`) for encrypted secrets. We extend it with attestation verification.

**Current Keystore** (`keystore-worker/app.py`):
```python
@app.route('/decrypt', methods=['POST'])
def decrypt_secrets():
    data = request.json
    encrypted_data = data.get('encrypted_data')
    attestation = data.get('attestation')  # Currently unused

    # TODO: Verify attestation before decrypting

    decrypted = decrypt(encrypted_data)
    return jsonify({'decrypted': decrypted})
```

**Enhanced with Attestation** (proposed):
```python
from cryptography.hazmat.primitives.asymmetric import ec
from cryptography.hazmat.primitives import hashes, serialization
from cryptography.exceptions import InvalidSignature
import json

@app.route('/decrypt', methods=['POST'])
def decrypt_secrets():
    data = request.json
    encrypted_data = data.get('encrypted_data')
    attestation_quote = data.get('attestation_quote')  # NEW: Signed quote
    worker_id = data.get('worker_id')

    # 1. Verify attestation quote
    try:
        quote_valid = verify_worker_quote(attestation_quote, worker_id)
        if not quote_valid:
            return jsonify({'error': 'Invalid attestation'}), 403
    except Exception as e:
        return jsonify({'error': f'Attestation verification failed: {str(e)}'}), 403

    # 2. Check measurements against policy
    measurements = attestation_quote['quote']['measurements']
    if measurements['pcr0'] not in TRUSTED_WASM_HASHES:
        return jsonify({'error': 'Untrusted WASM bytecode'}), 403

    # 3. Decrypt secrets (now safe - attestation verified)
    decrypted = decrypt(encrypted_data)

    return jsonify({
        'decrypted': decrypted,
        'attestation_verified': True,
        'worker_id': worker_id
    })

def verify_worker_quote(signed_quote, worker_id):
    """Verify worker's attestation quote"""
    # Get worker's public key from registry
    worker_pubkey_pem = get_worker_public_key(worker_id)
    public_key = serialization.load_pem_public_key(worker_pubkey_pem.encode())

    # Verify signature
    quote_json = json.dumps(signed_quote['quote'], indent=2)
    quote_bytes = quote_json.encode('utf-8')
    signature = bytes(signed_quote['signature'])

    try:
        public_key.verify(
            signature,
            quote_bytes,
            ec.ECDSA(hashes.SHA256())
        )
        return True
    except InvalidSignature:
        return False
```

---

## 6. Verification Workflow

### 6.1 End-to-End Attestation Flow

```
┌──────────────┐
│ NEAR Contract│
└──────┬───────┘
       │ 1. Request execution with nonce
       ↓
┌──────────────────┐
│ Coordinator API  │
│ - Generate nonce │
│ - Store challenge│
└──────┬───────────┘
       │ 2. Task with nonce
       ↓
┌────────────────────────────────┐
│ Browser Worker                 │
│ Step 1: Initialize measurements│
│   measurements.extend(0, wasm) │
│   measurements.extend(1, input)│
│                                │
│ Step 2: Execute WASM           │
│   result = execute(wasm, input)│
│   measurements.extend(3, state)│
│                                │
│ Step 3: Generate quote         │
│   quote = generateQuote(nonce) │
│   sign with WebCrypto key      │
└──────┬─────────────────────────┘
       │ 3. Submit: result + quote
       ↓
┌────────────────────────────────┐
│ Coordinator API                │
│ Step 1: Verify signature       │
│   check ECDSA sig with pub key │
│                                │
│ Step 2: Validate measurements  │
│   PCR0 == known WASM hash?     │
│   PCR1 == input hash?          │
│   Nonce matches challenge?     │
│                                │
│ Step 3: Issue attestation      │
│   coordinator_quote = sign(    │
│     worker_quote + verification│
│   )                            │
└──────┬─────────────────────────┘
       │ 4. Submit to contract: result + coordinator_attestation
       ↓
┌────────────────────────────────┐
│ NEAR Contract                  │
│ - Verify coordinator signature │
│ - Check state root in quote    │
│ - Accept result if valid       │
│ - Slash if invalid             │
└────────────────────────────────┘
```

### 6.2 Measurement Timeline

**PCR Extension Points**:

```javascript
// PCR 0: WASM Bytecode Hash
const wasmBytes = await fetch(wasmUrl).then(r => r.arrayBuffer());
const wasmHash = await crypto.subtle.digest('SHA-256', wasmBytes);
await measurements.extend(0, new Uint8Array(wasmHash), 'WASM bytecode');

// PCR 1: Input Data Hash
const inputJson = JSON.stringify(inputData);
const inputBytes = new TextEncoder().encode(inputJson);
const inputHash = await crypto.subtle.digest('SHA-256', inputBytes);
await measurements.extend(1, new Uint8Array(inputHash), 'Input data');

// PCR 2: Configuration Hash
const config = { gasLimit: 1e9, memoryLimit: 128 * 1024 * 1024 };
const configJson = JSON.stringify(config);
const configBytes = new TextEncoder().encode(configJson);
const configHash = await crypto.subtle.digest('SHA-256', configBytes);
await measurements.extend(2, new Uint8Array(configHash), 'Configuration');

// PCR 3: Runtime State (after execution)
const stateRoot = calculateMerkleRoot(nearState);
const stateBytes = new Uint8Array(stateRoot);
await measurements.extend(3, stateBytes, 'Final state root');
```

**Expected PCR Values** (coordinator maintains):
```javascript
const TRUSTED_MEASUREMENTS = {
  // PCR0: Known WASM bytecode hashes (multiple versions supported)
  wasmHashes: [
    'a3f5b2c1...', // version 1.0.0
    'f2e1d3a4...', // version 1.1.0
  ],

  // PCR2: Allowed configurations
  allowedConfigs: [
    { gasLimit: 1e9, memoryLimit: 128 * 1024 * 1024 },
    { gasLimit: 5e9, memoryLimit: 256 * 1024 * 1024 },
  ],

  // PCR1 and PCR3 vary per execution (verified differently)
};
```

### 6.3 Nonce Management

**Nonce Generation** (coordinator):
```javascript
class NonceManager {
  constructor() {
    this.activeNonces = new Map(); // nonce → { taskId, expiresAt }
  }

  generateNonce(taskId) {
    const nonce = crypto.getRandomValues(new Uint8Array(32));
    const expiresAt = Date.now() + 5 * 60 * 1000; // 5 minutes

    this.activeNonces.set(
      Array.from(nonce).join(','),
      { taskId, expiresAt }
    );

    return nonce;
  }

  validateNonce(nonce, taskId) {
    const nonceKey = Array.isArray(nonce) ? nonce.join(',') : nonce;
    const record = this.activeNonces.get(nonceKey);

    if (!record) {
      return { valid: false, reason: 'Unknown nonce' };
    }

    if (record.taskId !== taskId) {
      return { valid: false, reason: 'Nonce mismatch' };
    }

    if (Date.now() > record.expiresAt) {
      this.activeNonces.delete(nonceKey);
      return { valid: false, reason: 'Nonce expired' };
    }

    // One-time use: delete after validation
    this.activeNonces.delete(nonceKey);

    return { valid: true };
  }

  cleanup() {
    const now = Date.now();
    for (const [nonce, record] of this.activeNonces) {
      if (now > record.expiresAt) {
        this.activeNonces.delete(nonce);
      }
    }
  }
}
```

---

## 7. Integration Points

### 7.1 Worker Integration

**Modified Worker Flow** (`worker/src/main.rs`):

```rust
// Current: worker submits only result
let result = ExecutionResult {
    output: output_data,
    resources_used: ResourceMetrics {
        instructions: fuel_consumed,
        time_ms: elapsed_ms,
    },
};

api_client.complete_task(task_id, result).await?;

// Proposed: worker generates attestation quote
let quote = generate_attestation_quote(
    &task.nonce,          // From coordinator
    &wasm_hash,           // PCR0
    &input_hash,          // PCR1
    &state_root,          // PCR3
    &fuel_consumed,
    &worker_signing_key   // From config
).await?;

let result_with_attestation = ExecutionResultWithAttestation {
    output: output_data,
    resources_used: ResourceMetrics {
        instructions: fuel_consumed,
        time_ms: elapsed_ms,
    },
    attestation_quote: quote,
};

api_client.complete_task(task_id, result_with_attestation).await?;
```

**Worker Config** (`.env` additions):
```bash
# Existing config
API_BASE_URL=http://localhost:8080
WORKER_ID=worker-001

# NEW: Attestation config
ATTESTATION_ENABLED=true
WORKER_SIGNING_KEY_PATH=/path/to/ecdsa-p256-key.pem
MEASUREMENT_REGISTRY_ENABLED=true
```

### 7.2 Coordinator API Integration

**New Endpoint**: `POST /tasks/complete_with_attestation`

```javascript
// coordinator/src/handlers/tasks.js
router.post('/complete_with_attestation', async (req, res) => {
  const { task_id, result, attestation_quote } = req.body;

  // 1. Load task to get nonce
  const task = await db.getTask(task_id);
  if (!task) {
    return res.status(404).json({ error: 'Task not found' });
  }

  // 2. Verify attestation quote
  const quoteVerifier = new QuoteVerifier(trustedPCRs, workerRegistry);
  const verification = await quoteVerifier.verifyQuote(attestation_quote);

  if (!verification.valid) {
    // Log security incident
    await db.logSecurityEvent({
      type: 'INVALID_ATTESTATION',
      task_id,
      worker_id: attestation_quote.quote.worker_id,
      errors: verification.errors,
      timestamp: Date.now()
    });

    return res.status(403).json({
      error: 'Attestation verification failed',
      details: verification.errors
    });
  }

  // 3. Verify nonce
  const nonceValid = nonceManager.validateNonce(
    attestation_quote.quote.nonce,
    task_id
  );

  if (!nonceValid.valid) {
    return res.status(403).json({
      error: 'Invalid nonce',
      reason: nonceValid.reason
    });
  }

  // 4. Issue coordinator attestation
  const coordinatorAttestation = await quoteVerifier.issueCoordinatorAttestation(
    attestation_quote,
    coordinatorSigningKey
  );

  // 5. Store result with attestations
  await db.storeTaskResult({
    task_id,
    result,
    worker_attestation: attestation_quote,
    coordinator_attestation: coordinatorAttestation,
    verified_at: Date.now()
  });

  // 6. Submit to NEAR contract
  await nearClient.submitResult({
    task_id,
    result,
    attestation: coordinatorAttestation
  });

  res.json({ success: true, attestation_verified: true });
});
```

### 7.3 Contract Integration

**Enhanced Contract** (`contract/src/execution.rs`):

```rust
#[near]
impl OffchainVMContract {
    pub fn resolve_execution_with_attestation(
        &mut self,
        request_id: U64,
        result: ExecutionResult,
        coordinator_attestation: CoordinatorAttestation,
    ) -> ExecutionResult {
        // 1. Verify caller is coordinator
        require!(
            env::predecessor_account_id() == self.operator_id,
            "Only operator can resolve"
        );

        // 2. Load pending request
        let mut request = self.pending_requests.get(&request_id.0)
            .expect("Request not found");

        require!(
            request.status == ExecutionStatus::Pending,
            "Request not in pending state"
        );

        // 3. Verify coordinator attestation signature
        let attestation_valid = self.verify_coordinator_attestation(
            &coordinator_attestation,
            request_id.0,
        );

        require!(attestation_valid, "Invalid coordinator attestation");

        // 4. Verify measurements in attestation
        let measurements = &coordinator_attestation.worker_quote.quote.measurements;

        // Check WASM hash (PCR0) against code_source
        let expected_wasm_hash = self.get_wasm_hash(&request.code_source);
        require!(
            measurements.pcr0 == expected_wasm_hash,
            "WASM bytecode mismatch"
        );

        // Check state root (PCR3) matches result
        require!(
            measurements.pcr3 == result.state_root,
            "State root mismatch"
        );

        // 5. Record attestation on-chain
        self.attestations.insert(&request_id.0, &coordinator_attestation);

        // 6. Process result (existing logic)
        request.status = ExecutionStatus::Completed;
        request.result = Some(result.clone());
        request.completed_at = Some(env::block_timestamp());

        self.pending_requests.insert(&request_id.0, &request);

        // 7. Emit event with attestation verification
        Event::ExecutionCompleted {
            request_id: request_id.0,
            requester: request.requester.clone(),
            success: true,
            attestation_verified: true,
            measurements: measurements.clone(),
        }
        .emit();

        result
    }

    fn verify_coordinator_attestation(
        &self,
        attestation: &CoordinatorAttestation,
        request_id: u64,
    ) -> bool {
        // Reconstruct signed message
        let attestation_json = near_sdk::serde_json::to_string(&attestation.attestation)
            .expect("Failed to serialize attestation");

        // Verify ECDSA signature with coordinator's public key
        let message_hash = env::sha256(attestation_json.as_bytes());

        env::ecrecover(
            &message_hash,
            &attestation.signature,
            &self.coordinator_public_key,
            true, // v = 0 for ECDSA P-256
        )
        .is_some()
    }
}
```

---

## 8. Security Analysis

### 8.1 Threat Model

**Threats Considered**:

1. **Malicious Worker**
   - Tampers with browser/WASM runtime
   - Generates fake measurements
   - Submits incorrect results
   - **Mitigation**: Coordinator verification + consensus + slashing

2. **Compromised Coordinator**
   - Accepts invalid attestations
   - Colludes with malicious worker
   - **Mitigation**: On-chain verification + slashing + coordinator rotation

3. **Network Attacker**
   - MITM between worker and coordinator
   - Replay old attestations
   - **Mitigation**: Nonce freshness + TLS + signature verification

4. **Timing Attacks**
   - Infer secret data from execution time
   - **Mitigation**: Constant-time operations + noise injection

5. **Side Channel Leaks**
   - Cache timing, spectre-style attacks
   - **Mitigation**: Browser mitigations (site isolation) + secrets in encrypted memory

**Threats NOT Defended Against** (out of scope for browser implementation):
- Physical access to worker's machine
- Browser zero-day exploits
- OS-level malware with kernel privileges

### 8.2 Attack Scenarios

#### Scenario 1: Worker Submits Fake Result

```
Malicious Worker:
  1. Receives task: execute WASM(input) → output
  2. Doesn't execute WASM, returns fake output
  3. Generates attestation quote with correct measurements

Defense:
  - Coordinator checks PCR0 (WASM hash) matches expected
  - Coordinator re-executes in trusted environment (sampling)
  - Multiple workers execute same task → consensus
  - Divergence triggers challenge → slashing
```

#### Scenario 2: Replay Attack

```
Attacker:
  1. Captures old attestation quote
  2. Replays for different task

Defense:
  - Nonce tied to specific task_id
  - Nonce expires after 5 minutes
  - One-time use (deleted after validation)
  - Quote includes timestamp
```

#### Scenario 3: Coordinator Collusion

```
Malicious Coordinator + Worker:
  1. Worker submits invalid result
  2. Coordinator accepts without verification
  3. Submits to NEAR contract

Defense:
  - Contract verifies coordinator signature (knows public key)
  - Slashing bond for coordinator
  - Contract can sample verify (challenge coordinator to prove)
  - Multiple coordinators (future: decentralized)
```

### 8.3 Limitations and Future Work

**Current Limitations**:
1. **No Hardware Root of Trust**: Browser attestation is software-only
2. **Single Coordinator**: Centralization risk (future: DAO)
3. **Timing Variance**: Browser execution timing less predictable than hardware TEE
4. **Key Storage**: IndexedDB less secure than hardware keystore

**Future Enhancements**:
1. **WebAuthn Integration**: Bind worker identity to hardware token
2. **Multi-Coordinator Consensus**: 3-of-5 coordinators must agree
3. **ZK Proofs**: Zero-knowledge proof of correct execution (expensive but possible)
4. **Intel SGX Verification Service**: Coordinators run in SGX, generate real quotes
5. **Reproducible Builds**: RA-WEBs style threshold signatures for WASM provenance

---

## 9. Implementation Roadmap

### Phase 1: Basic Attestation (2-3 weeks)

**Week 1-2**: Browser Components
- [ ] Implement MeasurementRegistry with SharedArrayBuffer
- [ ] Implement AttestationQuote generation
- [ ] Create ECDSA P-256 key management (WebCrypto)
- [ ] Add PCR extension points to ContractSimulator
- [ ] Unit tests for measurement operations

**Week 3**: Coordinator Integration
- [ ] Implement QuoteVerifier service
- [ ] Add nonce management
- [ ] Create POST /tasks/complete_with_attestation endpoint
- [ ] Worker registry for public keys
- [ ] Integration tests

**Deliverables**:
- Browser worker generates signed attestation quotes
- Coordinator verifies quotes
- End-to-end test with mock NEAR contract

### Phase 2: Contract Integration (1-2 weeks)

**Week 4**: Smart Contract
- [ ] Add CoordinatorAttestation struct
- [ ] Implement verify_coordinator_attestation()
- [ ] Update resolve_execution to require attestation
- [ ] Add attestation storage and queries
- [ ] Contract unit tests

**Week 5**: Worker Integration
- [ ] Rust worker calls browser attestation API
- [ ] Generate quote before submitting result
- [ ] Add WORKER_SIGNING_KEY config
- [ ] Integration tests with real contract

**Deliverables**:
- Worker → Coordinator → Contract flow with attestation
- Contract rejects submissions without valid attestation
- Dashboard displays attestation status

### Phase 3: Enhanced Security (2-3 weeks)

**Week 6-7**: Consensus and Sampling
- [ ] Multiple workers execute same task
- [ ] Coordinator compares results
- [ ] Sampling verification (re-execute randomly)
- [ ] Divergence detection and slashing

**Week 8**: Monitoring and Analytics
- [ ] Attestation verification metrics
- [ ] Security event logging
- [ ] Anomaly detection (timing, resource usage)
- [ ] Dashboard for attestation health

**Deliverables**:
- Production-ready attestation system
- Security monitoring dashboard
- Documentation and runbooks

### Phase 4: Advanced Features (Future)

- WebAuthn integration for worker identity
- Multi-coordinator consensus
- ZK proof integration
- Hardware TEE support (when browsers support)
- Reproducible build verification

---

## Conclusion

Remote attestation in a browser environment requires adapting hardware TEE concepts to software-based verification. OutLayer's hybrid approach combines:

1. **Browser-generated evidence**: PCR-style measurements, ECDSA signatures
2. **Coordinator validation**: Centralized verification, anomaly detection
3. **On-chain settlement**: Economic incentives, slashing, final verification

This architecture provides a pragmatic path to verifiable computation without hardware TEE support, with a clear upgrade path when browsers add native attestation APIs.

The measurement registry, quote generation, and verification workflows presented here are production-ready patterns adapted from Intel TDX, AMD SEV, and NEAR's receipt model. Implementation can begin immediately using WebCrypto and SharedArrayBuffer.

**Next Steps**:
1. Review this architecture with security team
2. Begin Phase 1 implementation (MeasurementRegistry)
3. Define trusted PCR values for production WASM modules
4. Design coordinator key management and rotation
5. Plan migration path from current non-attested system

This document serves as the technical foundation for attestation implementation, providing both the conceptual framework and concrete code patterns for principal engineers to execute.
