# Soft Enclave - Tier 2 Security Architecture

**Cross-origin iframe isolation with encrypted RPC for browser-based confidential compute**

## Quick Start

### 1. Run Two Servers (Required)

Soft Enclave requires **two separate origins** to enforce Same-Origin Policy (SOP) barrier:

```bash
# Terminal 1: Host page
cd browser-worker/soft-enclave/host
python3 -m http.server 8080

# Terminal 2: Enclave iframe
cd browser-worker/soft-enclave/enclave
python3 -m http.server 9090
```

### 2. Run Automated Tests

1. Open `http://localhost:8080` in your browser
2. Click **Run tests**
3. Expect **4/4 green**:
   - ✅ SOP barrier (host cannot access enclave memory)
   - ✅ Ciphertext-only egress (plaintext blocked by guard)
   - ✅ Nonce/replay protection (duplicate messages rejected)
   - ✅ Context binding (session keys bound to origins + codeHash)

## What Soft Enclave Provides

### Security Properties
- **SOP (Same-Origin Policy) barrier**: Host and enclave run on different origins; host cannot introspect enclave frame or memory
- **Ciphertext-only egress**: Runtime guard enforces that only encrypted messages leave the enclave
- **Replay protection**: IV (nonce) tracking prevents message replay attacks
- **Context binding**: Session keys are bound to (hostOrigin, enclaveOrigin, codeHash)

### Cryptographic Protocol
```
Handshake:  ECDH → HKDF → AES-GCM-256 + base IV
Encryption: IV(seq) = baseIV ⊕ seq (counter mode)
AAD:        Operation-specific string (e.g., "op=evalQuickJS")
```

## Architecture

```
┌──────────────────────────────────┐
│ Host (localhost:8080)            │
│ ┌─────────────────────────────┐  │
│ │ EnclaveClient               │  │
│ │ - ECDH keygen               │  │
│ │ - Session derivation        │  │
│ │ - Seal/open messages        │  │
│ └─────────────────────────────┘  │
└────────┬─────────────────────────┘
         │ MessageChannel (encrypted)
         ↓
┌──────────────────────────────────┐
│ Enclave (localhost:9090)         │
│ ┌─────────────────────────────┐  │
│ │ Egress Guard                │  │
│ │ - Blocks plaintext          │  │
│ └─────────────────────────────┘  │
│ ┌─────────────────────────────┐  │
│ │ Enclave Main                │  │
│ │ - Replay cache (IV check)   │  │
│ │ - QuickJS runtime           │  │
│ │ - Encrypted operations      │  │
│ └─────────────────────────────┘  │
└──────────────────────────────────┘
```

## When to Use Soft Enclave

**Use Tier 2 (Soft Enclave) when:**
- Transaction value: **$100 - $10,000**
- Threat model: XSS, malicious scripts on main origin
- Requirement: Defense-in-depth beyond in-page sandbox
- User accepts: Two-origin setup (slight UX friction)

**Use Tier 1 (quickjs-enclave.ts) when:**
- Transaction value: **< $100**
- User expects: Single-page UX (no iframe)
- Goal: Deterministic compute + WebCrypto custody

**Use Tier 3 (TEE) when:**
- Transaction value: **> $10,000**
- Requirement: Hardware-backed attestation
- Deployment: Backend TEE (SGX/SEV/Nitro)

## Optional: Add QuickJS

For full QuickJS execution (vs fallback to `new Function`):

1. Download [quickjs-emscripten](https://www.npmjs.com/package/quickjs-emscripten)
2. Copy `quickjs-emscripten.mjs` and `quickjs-emscripten.wasm` to `enclave/vendor/`
3. Reload enclave - auto-detects and uses QuickJS

## Files

```
soft-enclave/
├── README.md                    # This file
├── host/
│   ├── index.html               # Test UI
│   ├── test-runner.js           # 4 automated tests
│   ├── enclave-client.js        # Handshake + encrypted RPC
│   └── crypto-protocol.js       # ECDH→HKDF→AES-GCM
├── enclave/
│   ├── boot.html                # Tight CSP; loads guard first
│   ├── egress-assert.js         # Ciphertext-only enforcement
│   ├── enclave-main.js          # Replay cache + ops
│   ├── quickjs-runtime.js       # QuickJS loader with fallback
│   ├── crypto-protocol.js       # Same as host version
│   └── vendor/                  # (optional) QuickJS WASM
```

## Documentation

- [docs/soft-enclave.md](../../docs/soft-enclave.md) - Full documentation with protocol details
- [docs/browser-sec-architecture.md](../../docs/browser-sec-architecture.md) - Complete security model
- [QUICKJS_INTEGRATION.md](../QUICKJS_INTEGRATION.md) - Tier 1 (in-page) implementation

## Threat Model

### Protects Against
- ✅ XSS on host origin
- ✅ Malicious scripts injected into main page
- ✅ Accidental plaintext leakage
- ✅ Replay attacks
- ✅ MITM between host and enclave

### Does NOT Protect Against
- ❌ Malicious browser extensions
- ❌ OS/kernel compromise
- ❌ Physical access attacks
- ❌ Side-channel attacks

## Status

**Current**: Manual testing with local servers
**Next**: Integration with contract simulator
**Future**: Production deployment with proper origins (e.g., app.near.org + enclave.near.org)
