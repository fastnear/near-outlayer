/**
 * Host-Side Signer with WebCrypto Custody
 *
 * Security model from docs/browser-sec-architecture.md section 4.2:
 * - Private keys NEVER enter QuickJS/WASM
 * - QuickJS computes { bytesToSign, newState }
 * - Host performs signing via WebCrypto (preferred) or vetted WASM crypto lib
 * - Non-extractable keys when possible
 * - Encrypted raw bytes as fallback
 *
 * Pattern:
 * 1. QuickJS: prepareTransfer(args, state) → { bytesToSign: Uint8Array, ... }
 * 2. Host: signer.sign(bytesToSign) → signature
 * 3. Keys stay in WebCrypto or encrypted IndexedDB, never in linear memory
 */

/**
 * Create a signer with WebCrypto custody (preferred) or software fallback.
 *
 * @returns {Promise<{ publicKey: Uint8Array, sign: (bytes: Uint8Array) => Promise<Uint8Array> }>}
 */
export async function createSigner() {
  const algo = { name: "Ed25519" };

  try {
    // Preferred: WebCrypto-native Ed25519 (when available)
    // extractable: false means key cannot be exported/leaked
    const kp = await crypto.subtle.generateKey(algo, false, ["sign", "verify"]);
    const pub = new Uint8Array(await crypto.subtle.exportKey("raw", kp.publicKey));

    return {
      publicKey: pub,
      async sign(bytes) {
        return new Uint8Array(await crypto.subtle.sign(algo, kp.privateKey, bytes));
      }
    };
  } catch (err) {
    // Fallback: Software WASM (@noble/ed25519 or libsodium)
    // Store encrypted raw seed in IndexedDB, decrypt just-in-time
    console.warn('[host-signer] WebCrypto Ed25519 not available, using software fallback');

    const noble = await import("@noble/ed25519");
    const seed = await loadEncryptedSeed();   // Your AES-GCM decrypt from IndexedDB
    const pub = await noble.getPublicKey(seed);

    return {
      publicKey: pub,
      async sign(bytes) {
        try {
          return await noble.sign(bytes, seed);
        } finally {
          // Best-effort scrub (not guaranteed in JS, but reduces exposure window)
          seed.fill(0);
        }
      }
    };
  }
}

/**
 * Load encrypted seed from IndexedDB and decrypt.
 * This is a placeholder - implement with your actual storage/encryption.
 *
 * Pattern:
 * 1. Derive wrapping key from user passphrase (PBKDF2)
 * 2. Store AES-GCM encrypted seed in IndexedDB
 * 3. Decrypt just-in-time for signing
 *
 * @returns {Promise<Uint8Array>} 32-byte Ed25519 seed
 */
async function loadEncryptedSeed() {
  // TODO(implementer): Replace with your actual IndexedDB + AES-GCM decryption
  // Example pattern:
  // const db = await openDB('wallet-keys', 1);
  // const encrypted = await db.get('keys', 'ed25519-seed');
  // const wrappingKey = await deriveKeyFromPassphrase(userPassphrase);
  // const seed = await crypto.subtle.decrypt(
  //   { name: 'AES-GCM', iv: encrypted.iv },
  //   wrappingKey,
  //   encrypted.ciphertext
  // );
  // return new Uint8Array(seed);

  // Placeholder: generate ephemeral seed (FOR DEMO ONLY, NOT PERSISTENT)
  console.warn('[loadEncryptedSeed] Using ephemeral seed - NOT FOR PRODUCTION');
  return crypto.getRandomValues(new Uint8Array(32));
}

/**
 * Example: Integrate with QuickJS to sign a transfer.
 *
 * @param {QuickJSEnclave} enclave - The QuickJS sandbox
 * @param {string} contractSource - JavaScript contract code
 * @param {object} transferArgs - { from, to, amount }
 * @param {object} state - Contract state
 * @returns {Promise<{ signature: Uint8Array, newState: object }>}
 */
export async function signTransfer(enclave, contractSource, transferArgs, state) {
  // Step 1: QuickJS computes WHAT to sign (message bytes)
  const result = await enclave.invoke({
    source: contractSource,
    func: 'prepareTransfer',
    args: [transferArgs],
    priorState: state,
    seed: 'transfer-seed',
    policy: { timeMs: 200, memoryBytes: 32 << 20 }
  });

  if (!result.result || !result.result.bytesToSign) {
    throw new Error('Contract did not return bytesToSign');
  }

  // Convert from JSON-serializable array to Uint8Array
  const bytesToSign = new Uint8Array(result.result.bytesToSign);

  // Step 2: Host performs signing (keys never entered QuickJS)
  const signer = await createSigner();
  const signature = await signer.sign(bytesToSign);

  return {
    signature,
    newState: result.state,
    publicKey: signer.publicKey
  };
}

/**
 * Example contract that computes bytesToSign (runs inside QuickJS).
 *
 * Usage:
 * const enclave = await QuickJSEnclave.create();
 * const { signature, newState } = await signTransfer(enclave, TRANSFER_CONTRACT, args, state);
 */
export const TRANSFER_CONTRACT_EXAMPLE = `
globalThis.prepareTransfer = function(args, state) {
  // Pure computation: build canonical message bytes
  const { from, to, amount } = args;

  if (!from || !to || typeof amount !== 'number') {
    throw new Error('Invalid transfer args');
  }

  // Increment nonce for replay protection
  const nonce = (state.nonce || 0) + 1;

  // Canonical JSON (deterministic key ordering)
  const message = JSON.stringify({
    from,
    to,
    amount,
    nonce,
    timestamp: Date.now()  // Will be 0 in deterministic mode
  });

  // Convert to bytes
  const encoder = new TextEncoder();
  const bytesToSign = encoder.encode(message);

  // Log for debugging (captured in diagnostics.logs)
  near.log('Prepared transfer:', from, '→', to, amount);

  return {
    bytesToSign: Array.from(bytesToSign), // JSON-serializable
    nextState: { nonce },
    messagePreview: message.substring(0, 100)
  };
};
`;
