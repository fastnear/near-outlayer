# Soft Enclave - Cross-Origin Iframe Isolation

**Tier 2 Security Architecture** for browser-based confidential compute

## Concept

A **Soft Enclave** provides enhanced security over in-page sandboxing (Tier 1) by using cross-origin iframe isolation. The host page and enclave run on different origins, communicating only via encrypted RPC.

### Security Model

```
Tier 1 (In-Page):     quickjs-enclave.ts
                     ↓
Tier 2 (Cross-Origin): Soft Enclave (this)
                     ↓
Tier 3 (TEE):         SGX/SEV/Nitro backend
```

**What Soft Enclave Provides:**
- **SOP (Same-Origin Policy) barrier**: Host cannot introspect enclave frame/memory
- **Ciphertext-only egress**: Egress guard enforces encrypted messages only
- **Replay protection**: Nonce tracking prevents message replay attacks
- **Context binding**: Session keys bound to (hostOrigin, enclaveOrigin, codeHash)

**What Soft Enclave Does NOT Provide:**
- **Protection from malicious extensions**: Extensions with page access can observe both origins
- **Protection from OS compromise**: Kernel/hypervisor can access all memory
- **TEE attestation**: No hardware-backed proof of execution

### Architecture

```
┌────────────────────────────────┐
│ Host Page (localhost:8080)     │
│ ┌───────────────────────────┐  │
│ │ EnclaveClient             │  │
│ │ - ECDH key generation     │  │
│ │ - Session derivation       │  │
│ │ - Seal/open (AES-GCM)     │  │
│ └───────────────────────────┘  │
└────────┬───────────────────────┘
         │ MessageChannel (encrypted RPC)
         ↓
┌────────────────────────────────┐
│ Enclave Iframe (localhost:9090)│
│ ┌───────────────────────────┐  │
│ │ Egress Guard              │  │
│ │ - Blocks plaintext        │  │
│ │ - Schema validation       │  │
│ └───────────────────────────┘  │
│ ┌───────────────────────────┐  │
│ │ Enclave Main              │  │
│ │ - Replay cache (IV check) │  │
│ │ - QuickJS runtime         │  │
│ │ - Encrypted ops           │  │
│ └───────────────────────────┘  │
└────────────────────────────────┘
```

## Cryptographic Protocol

### Handshake

1. **Enclave boots**: Generates ECDH keypair, sends public key to host
2. **Host responds**: Generates ECDH keypair, sends public key to enclave
3. **Both derive**: Session keys via ECDH → HKDF → AES-GCM + base IV

### Session Derivation

```
IKM = ECDH.deriveBits(myPriv, peerPub)
salt = SHA-256(hostOrigin || enclaveOrigin || codeHash)
aeadKey = HKDF(IKM, salt, info="soft-enclave/aead") → AES-GCM-256
baseIV = HKDF(IKM, salt, info="soft-enclave/iv") → 96 bits
```

### Message Encryption

```
IV(seq) = baseIV ⊕ (seq as 32-bit counter in last 4 bytes)
ciphertext = AES-GCM.encrypt(aeadKey, IV, plaintext, AAD)
```

**AAD (Additional Authenticated Data)**: Operation-specific string (e.g., `"op=evalQuickJS"`)

### Replay Protection

Enclave maintains an in-memory cache of seen IVs (FIFO, max 4096). If the same IV appears twice, the message is rejected.

## Security Properties (Tested)

| Property | Implementation | Test |
|----------|---------------|------|
| **SOP barrier** | Cross-origin iframe | `testSOPBarrier` - Host cannot access `iframe.contentWindow.document` |
| **Ciphertext-only egress** | `egress-assert.js` patches `MessagePort.postMessage` | `testCiphertextOnlyEgress` - Plaintext message triggers error |
| **Replay protection** | IV cache in `enclave-main.js` | `testNonceReplay` - Resending same ciphertext fails |
| **Context binding** | Session keys bound to origins + codeHash | `testContextBinding` - Tampering codeHash breaks decryption |

## Running the Tests

### Setup

Soft Enclave requires **two separate origins** (enforced by SOP). You'll run two local servers:

```bash
# Terminal 1: Host page
cd browser-worker/soft-enclave/host
python3 -m http.server 8080

# Terminal 2: Enclave iframe
cd browser-worker/soft-enclave/enclave
python3 -m http.server 9090
```

### Execute Tests

1. Open `http://localhost:8080` in your browser
2. Verify enclave origin is set to `http://localhost:9090`
3. Click **Run tests**
4. Expect **4/4 green**:
   - ✅ SOP barrier
   - ✅ Ciphertext-only egress
   - ✅ Nonce/replay
   - ✅ Context binding

### Optional: Add QuickJS

For full QuickJS execution (vs fallback to `new Function`):

1. Download `quickjs-emscripten` from NPM or CDN
2. Copy `quickjs-emscripten.mjs` and `quickjs-emscripten.wasm` to `enclave/vendor/`
3. Reload enclave - it will auto-detect and use QuickJS

## When to Use Soft Enclave

**Use Tier 2 (Soft Enclave) when:**
- Transaction value: **$100 - $10,000**
- User accepts: Two-origin setup (slight UX friction)
- Threat model: XSS, malicious scripts on main origin
- Goal: Defense-in-depth beyond in-page sandbox

**Use Tier 1 (quickjs-enclave.ts) when:**
- Transaction value: **< $100**
- User expects: Single-page UX (no iframe/cross-origin)
- Goal: Deterministic compute + WebCrypto custody

**Use Tier 3 (TEE) when:**
- Transaction value: **> $10,000**
- Requirement: Attestation, hardware-backed security
- Deployment: Backend TEE (not browser)

## Comparison with quickjs-enclave.ts

| Feature | Tier 1 (quickjs-enclave.ts) | Tier 2 (Soft Enclave) |
|---------|---------------------------|---------------------|
| **Isolation** | In-page sandbox | Cross-origin iframe |
| **SOP barrier** | ❌ | ✅ |
| **Encrypted RPC** | ❌ | ✅ (ECDH+HKDF+AES-GCM) |
| **Egress guard** | ❌ | ✅ (runtime-enforced) |
| **Replay protection** | ❌ | ✅ (IV cache) |
| **Setup complexity** | Low (single origin) | Medium (two origins) |
| **UX friction** | None | Slight (iframe load) |
| **Use case** | Convenience tier | Enhanced security tier |

## Integration with Contract Simulator

To wire Soft Enclave into your contract simulator:

```typescript
// Example (not yet implemented - see quickjs-browser adapter for pattern)
import { EnclaveClient } from './soft-enclave/host/enclave-client.js';

const client = new EnclaveClient('http://localhost:9090');
await client.boot();

const result = await client.send('evalQuickJS', {
  code: '40 + 2'
}, 'op=evalQuickJS');

console.log(result); // { ok: true, value: 42 }
```

## Threat Model

### Protects Against
- ✅ XSS on host origin
- ✅ Malicious scripts injected into main page
- ✅ Accidental plaintext leakage (egress guard)
- ✅ Replay attacks (nonce tracking)
- ✅ MITM between host and enclave (session binding)

### Does NOT Protect Against
- ❌ Malicious browser extensions (can observe both origins)
- ❌ OS/kernel compromise (can read all memory)
- ❌ Network-level MITM (no TLS termination inside browser)
- ❌ Physical access attacks
- ❌ Side-channel attacks (timing, spectre, etc.)

## Roadmap

- **Now**: Manual testing with two local servers
- **Next**: Automated browser testing (Playwright/Puppeteer)
- **Future**: Production deployment with proper origins (e.g., `app.near.org` + `enclave.near.org`)
- **Phase 3**: Upgrade to TEE backend with attestation

## References

- [browser-sec-architecture.md](./browser-sec-architecture.md) - Full security model
- [QUICKJS_INTEGRATION.md](../browser-worker/QUICKJS_INTEGRATION.md) - Tier 1 (in-page) implementation

## Definition of Done

- [x] SOP barrier enforced (cross-origin iframe)
- [x] Ciphertext-only egress (runtime guard)
- [x] Replay protection (IV cache)
- [x] Context binding (session derivation)
- [x] 4 automated tests (all passing)
- [x] Documentation (this file)
- [ ] Automated browser testing
- [ ] Production origins configured
- [ ] Integration with contract simulator
