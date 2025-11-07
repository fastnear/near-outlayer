# Browser-Based Secure Execution for NEAR
**An honest security architecture with deterministic compute, WebCrypto key custody, and a TEE upgrade path**

## Executive Summary

Browser WASM is **excellent** at sandboxing untrusted code, but it **cannot** prevent the host page from reading WASM memory or intercepting imports. We therefore do **not** claim a "browser enclave." We do claim a **material security improvement** over localStorage by combining:

1. **Deterministic, capability-minimized execution** (QuickJS with a locked environment)
2. **WebCrypto custody** (non-extractable keys; signing outside the WASM heap)
3. **Ephemeral exposure** (keys only live during signing; memory scrub where possible)
4. **Hygiene & policy** (CSP/Trusted Types/isolated workers; strict APIs)
5. **Upgrade path** (TEE attestation for higher stakes; on-chain anchoring via compact endorsements)

## 1. Security Boundaries (What is true, what isn't)

### What browser WASM *does* guarantee

- Guest code (WASM/JS inside QuickJS) cannot escape its sandbox without host-provided imports.
- Memory safety inside the WASM sandbox (bounds-checked linear memory).
- Determinism is attainable when the host **freezes** the environment (no `Date.now`, `Math.random`, timers, I/O).

### What browser WASM *does not* guarantee

- Confidentiality *from the host*: the page can inspect linear memory and control imports.
- Resistance against malicious extensions or compromised page context.
- Constant-time cryptography in plain JS across engines/JITs.

**Conclusion:** Treat the browser as an **execution sandbox**, not a secure keystore. Keep keys in **WebCrypto** and let QuickJS compute *what to sign*, not *perform the signing* with raw key bytes.

## 2. Security Properties Matrix (pragmatic)

| Property                         | Tier 1 (In-Page) | Tier 2 (Cross-Origin) | Tier 3 (TEE) | Pure TEE |
|----------------------------------|------------------|----------------------|--------------|----------|
| XSS resilience (vs localStorage) | ✓✓                | ✓✓✓                 | ✓✓✓          | ✓✓✓      |
| Malicious extensions             | ~                 | ~                   | ✓            | ✓✓       |
| OS compromise                    | ✗                 | ✗                   | ✓            | ✓✓✓      |
| Memory confidentiality           | ~ (keys in WebCrypto) | ✓ (SOP barrier) | ✓✓          | ✓✓✓      |
| Determinism (guarded)            | ✓✓✓               | ✓✓✓                 | ✓✓✓          | ✓✓✓      |
| Remote attestation               | ✗                 | ✗                   | ✓✓✓          | ✓✓✓      |
| Ubiquity/portability             | ✓✓✓               | ✓✓✓                 | ✓            | ✗        |
| Setup complexity                 | Low               | Medium              | High         | High     |

Legend: ✓✓✓ strong, ✓✓ moderate, ✓ limited, ~ marginal, ✗ none.

**Implementations:**
- **Tier 1**: `quickjs-enclave.ts` (in-page sandbox, WebCrypto custody)
- **Tier 2**: `soft-enclave/` (cross-origin iframe, encrypted RPC, egress guard)
- **Tier 3**: TEE backend (SGX/SEV/Nitro with attestation)

## 3. Threat-tiering for NEAR

- **Tier 1 (< $100)**: `quickjs-enclave.ts` - In-page sandbox; WebCrypto custody; deterministic compute; strict CSP. Convenience first.
- **Tier 2 ($100–$10k)**: `soft-enclave/` - Cross-origin iframe; encrypted RPC; SOP barrier; egress guard. Defense-in-depth.
- **Tier 3 (>$10k)**: TEE backend - Hardware attestation; remote verification; hardware wallets where possible.

## 4. Architecture

### 4.1 Deterministic compute in QuickJS (no secrets inside)

QuickJS computes *message bytes* to sign and validates preconditions. The **host** performs signing via WebCrypto, keeping private keys out of WASM memory.

```ts
// quickjs_bridge.ts
type ComputeRequest = {
  code: string;           // contract code (pure; no Date/Math.random/timers)
  entry: string;          // function name
  args: unknown[];        // JSON args
  state: unknown;         // JSON state
};
type ComputeResponse = { ok: true; bytesToSign: Uint8Array; newState: unknown; logs: string[] }
                     | { ok: false; error: string };

export async function quickjsCompute(req: ComputeRequest): Promise<ComputeResponse> {
  const qjs = await QuickJSEnclave.create();
  // Enclave disables Date.now/Math.random/timers; freezes selected prototypes.
  const out = await qjs.invoke({
    source: req.code,
    func: req.entry,
    args: [req.args, req.state],
    priorState: req.state,
    seed: "policy-seed",                     // optional determinism seed
    policy: { timeMs: 200, memoryBytes: 32<<20 }
  });
  // Contract returns canonical bytes to sign + next state
  if (!out.result || typeof out.result !== "object") {
    return { ok: false, error: "bad result" };
  }
  const r = out.result as any;
  if (!r.bytesToSign || !(r.bytesToSign instanceof Uint8Array)) {
    return { ok: false, error: "missing bytesToSign" };
  }
  return { ok: true, bytesToSign: r.bytesToSign, newState: out.state, logs: out.diagnostics.logs };
}
```

**Key point:** only bytes to sign cross the boundary. Private keys never enter QuickJS.

### 4.2 WebCrypto custody (portable path)

Prefer non-extractable keys and `subtle.sign`. When unsupported (e.g., Ed25519 on some browsers), store encrypted raw key bytes (not wrapped CryptoKey) and use a vetted WASM crypto library.

```ts
// keys.ts
export async function createSigner(): Promise<{ publicKey: Uint8Array; sign: (m: Uint8Array)=>Promise<Uint8Array> }> {
  // Preferred: WebCrypto-native Ed25519 (when available)
  const algo = { name: "Ed25519" } as any;
  try {
    const kp = await crypto.subtle.generateKey(algo, false, ["sign", "verify"]);
    const pub = new Uint8Array(await crypto.subtle.exportKey("raw", kp.publicKey));
    return {
      publicKey: pub,
      sign: (m) => crypto.subtle.sign(algo, kp.privateKey, m).then(b => new Uint8Array(b)),
    };
  } catch {
    // Fallback: software WASM (libsodium/@noble). Store **encrypted** raw secret in IndexedDB.
    const { sign, getPublicKey } = await import("@noble/ed25519");
    const seed = await loadEncryptedSeed();              // AES-GCM encrypted bytes; decrypt into Uint8Array
    const pub = await getPublicKey(seed);
    return {
      publicKey: pub,
      sign: async (m) => {
        try { return await sign(m, seed); }
        finally { seed.fill(0); }                        // best-effort scrub
      },
    };
  }
}
```

**Why not wrapKey for non-extractable keys?** Because a non-extractable CryptoKey cannot be wrapped/exported. If you need persistence, encrypt raw bytes under your own AES key and store those bytes.

### 4.3 Ephemeral exposure & hygiene

- Decrypt/derive keys just-in-time, sign, then best-effort zero buffers.
- Run QuickJS in a Worker with COOP/COEP to reduce incidental sharing; deny dangerous APIs.
- Enforce CSP/Trusted Types to reduce XSS.

### 4.4 Hybrid TEE path (Tier 2/3)

A backend TEE produces a signature and a compact attestation endorsement issued by a verifier. The dApp (or contract) checks that endorsement against a pinned key or on-chain registry.

```ts
// tee_client.ts
type TeeEndorsement = { provider: "SGX"|"SEV"|"TDX"|"Nitro"; codeHash: string; notBefore: number; notAfter: number; sig: Uint8Array };

export async function teeSign(bytes: Uint8Array, pubkey: Uint8Array) {
  const res = await fetch("/tee/sign", { method: "POST", body: bytes });
  const { signature, endorsement } = await res.json() as { signature: string; endorsement: TeeEndorsement };
  await verifyEndorsement(endorsement);   // verify against a compact verifier key you pin in the app/contract
  return { signature: base64ToBytes(signature), endorsement };
}
```

On-chain, verify a small endorsement (e.g., ed25519/secp256k1 signature from your verifier service), not raw vendor attestation.

## 5. Coding patterns that hold up

### Deterministic prelude (QuickJS)

```js
// inside the enclave before user code
(function harden(seedStr){
  let s = 0n; for (const ch of seedStr) s = (s*131n + BigInt(ch.charCodeAt(0))) & 0xffffffffffffffffn;
  Math.random = () => { s = (6364136223846793005n*s + 1442695040888963407n) & 0xffffffffffffffffn;
                        return Number(s & 0x3fffffffffffn) / Number(0x400000000000n); };
  Date.now = () => 0;
  globalThis.eval = () => { throw new Error("eval disabled"); };
  globalThis.setTimeout = undefined; globalThis.setInterval = undefined;
  try { Object.freeze(Object.prototype); } catch {}
  try { Object.freeze(Array.prototype); } catch {}
  try { Object.freeze(Function.prototype); } catch {}
})();
```

### Message-to-sign interface (contract side)

```js
// user contract code (executed inside QuickJS)
// Pure function: (args, state) -> { bytesToSign, nextState, metadata }
globalThis.buildTransfer = function(args, state){
  // construct canonical Borsh/JSON bytes deterministically
  const bytes = encodeTransferDeterministically(args);   // pure, no time/RNG
  const next = { ...state, last: "transfer" };
  return { bytesToSign: bytes, nextState: next };
};
```

### Constant-time guardrails

- Prefer WebCrypto (implementation-defined CT) or vetted WASM libraries (HACL*, fiat-crypto, libsodium).
- Avoid claiming JS constant time; at best, use constant-time selection helpers for sanity, but don't rely on it for secrets in JITed code.

## 6. Testing & verification that actually matter

- **Property-based determinism**: same inputs → identical outputs (and byte-identical bytesToSign) across 100× runs.
- **Budget tests**: enforce time/memory budgets; prove interrupts don't leak partial state.
- **Host-signer split**: prove by test that private keys never appear in QuickJS heap (e.g., by scanning heap for known test key).
- **Fuzz hostile inputs**: deep JSON args/state; large payloads; malformed encodings.

## 7. Documentation promises we can keep

- We reduce risk vs localStorage by (a) keeping keys in WebCrypto or encrypted bytes, (b) reducing exposure windows, (c) deterministic compute.
- We do not claim secrecy from the host page or extensions.
- For high-value operations we require TEE endorsement (or a hardware wallet).

## 8. Roadmap (pragmatic)

- **Now (Tier 1)**: ✅ `quickjs-enclave.ts` - QuickJS deterministic compute + WebCrypto custody + CSP/Trusted Types.
- **Now (Tier 2)**: ✅ `soft-enclave/` - Cross-origin iframe + encrypted RPC + egress guard + replay protection.
- **Next (Tier 3)**: TEE signer behind a verifier service; compact endorsements; on-chain registry of trusted verifier keys.
- **Later**: Hardware wallet integration and/or mandatory TEE for specific contract flows.

## 9. See Also

- [soft-enclave.md](./soft-enclave.md) - Tier 2 implementation details
- [QUICKJS_INTEGRATION.md](../browser-worker/QUICKJS_INTEGRATION.md) - Tier 1 implementation
- [soft-enclave/README.md](../browser-worker/soft-enclave/README.md) - Quick start guide for Tier 2

## Why this version avoids foot-guns

- Keeps secrets **out of WASM** and **out of JS** whenever WebCrypto can sign directly.
- Avoids non-portable WebCrypto recipes (PBKDF2→Ed25519 deriveKey, wrapping non-extractable keys).
- Frames extensions/OS compromise honestly.
- Describes a **compact** on-chain attestation pattern that contracts can actually verify.
- Demonstrates deterministic prelude and message-to-sign split with code.

## CSP & Trusted Types Example

```html
<!-- index.html -->
<meta http-equiv="Content-Security-Policy" content="
  default-src 'self';
  script-src 'self' 'wasm-unsafe-eval';
  worker-src 'self';
  connect-src 'self' https://rpc.mainnet.near.org;
  style-src 'self' 'unsafe-inline';
  img-src 'self' data: https:;
  font-src 'self';
  object-src 'none';
  base-uri 'self';
  form-action 'none';
  frame-ancestors 'none';
  require-trusted-types-for 'script';
">
```

```ts
// trusted-types-policy.ts
if (window.trustedTypes && window.trustedTypes.createPolicy) {
  window.trustedTypes.createPolicy('default', {
    createHTML: (input) => {
      // sanitize or throw
      return input;
    },
    createScriptURL: (input) => {
      // only allow self-hosted scripts
      if (input.startsWith('/') || input.startsWith(window.location.origin)) {
        return input;
      }
      throw new TypeError('Untrusted script URL');
    },
  });
}
```
