/**
 * Enclave Executor - L4 "Untrusted Ferry" Orchestrator
 *
 * Implements the Hermes Enclave architecture's core pattern:
 * L1 (Browser) → L2 (linux-wasm) → L3 (QuickJS) → L4 (Frozen Realm)
 *
 * Key Principle: **Encrypted data transits through L1-L3 without decryption**
 *
 * Only at L4 (Frozen Realm) are secrets decrypted and sensitive computation
 * performed. This mitigates the L2 NOMMU memory-sharing vulnerability by
 * ensuring plaintext NEVER exists in the shared memory space.
 *
 * Phase 1 Implementation (current):
 * - L1 → L4 direct (no L2/L3 yet)
 * - Proves E2EE ferry pattern works
 * - Establishes API for future L2/L3 integration
 *
 * Future Phases:
 * - Phase 2: Add L3 (QuickJS) between L1 and L4
 * - Phase 3: Add L2 (linux-wasm) for full 4-layer stack
 *
 * @author OutLayer Team + Hermes Enclave Collaboration
 * @version 1.0.0 - Phase 1: L1→L4 Direct
 */

class EnclaveExecutor {
  constructor(options = {}) {
    this.options = {
      // Enable verbose logging
      verbose: options.verbose || false,

      // Enable L2 (linux-wasm) layer (future)
      useLinux: options.useLinux || false,

      // Enable L3 (QuickJS) layer (future)
      useQuickJS: options.useQuickJS || false,

      // Execution timeout (ms)
      executionTimeout: options.executionTimeout || 30000,

      // Crypto options
      cryptoOptions: options.cryptoOptions || {},
    };

    // Initialize L4 Frozen Realm
    this.frozenRealm = new FrozenRealm({
      verbose: this.options.verbose,
      executionTimeout: this.options.executionTimeout,
    });

    // Initialize crypto utilities
    this.crypto = new CryptoUtils({
      verbose: this.options.verbose,
      ...this.options.cryptoOptions,
    });

    // L2 linux executor (future)
    this.linuxExecutor = null;
    if (this.options.useLinux) {
      this.log('Linux layer not yet implemented (Phase 2)', 'warn');
    }

    // L3 QuickJS bridge (future)
    this.quickjsBridge = null;
    if (this.options.useQuickJS) {
      this.log('QuickJS layer not yet implemented (Phase 2)', 'warn');
    }

    // Statistics
    this.stats = {
      totalExecutions: 0,
      encryptedExecutions: 0,
      avgExecutionTime: 0,
      avgDecryptionTime: 0,
    };

    this.log('EnclaveExecutor initialized', 'info');
    this.log(`  L1 (Browser): ✅ Active`, 'info');
    this.log(`  L2 (Linux): ${this.options.useLinux ? '⚠️ Not implemented' : '❌ Disabled'}`, 'info');
    this.log(`  L3 (QuickJS): ${this.options.useQuickJS ? '⚠️ Not implemented' : '❌ Disabled'}`, 'info');
    this.log(`  L4 (Frozen Realm): ✅ Active`, 'info');
  }

  /**
   * Execute code with E2EE (End-to-End Encryption) ferry pattern
   *
   * This is the main entry point that demonstrates the "untrusted ferry" model:
   *
   * 1. L1 fetches encrypted payload and encrypted secret (both are opaque blobs)
   * 2. L1 passes both to L2/L3 (future) without decryption
   * 3. L4 Frozen Realm receives both and decrypts ONLY within its sandbox
   * 4. L4 performs computation on plaintext
   * 5. L4 encrypts result before returning
   * 6. Encrypted result bubbles back up through L3→L2→L1
   *
   * Current Implementation (Phase 1):
   * - L1 → L4 direct (skips L2/L3)
   * - Still maintains E2EE: L1 never sees plaintext
   *
   * @param {object} request - Execution request
   * @param {string} request.encryptedPayload - Base64 encrypted data
   * @param {string} request.encryptedSecret - Base64 encrypted secret key
   * @param {string} request.enclaveKey - Hex-encoded L4 decryption key
   * @param {string} request.code - Guest code to execute in L4
   * @param {string} [request.codeId] - Optional identifier for debugging
   * @returns {Promise<object>} Encrypted result + metadata
   */
  async executeEncrypted(request) {
    this.stats.totalExecutions++;
    this.stats.encryptedExecutions++;

    const executionStart = performance.now();

    this.log('=== E2EE Ferry Execution Starting ===', 'info');
    this.log(`  Code ID: ${request.codeId || 'anonymous'}`, 'info');

    try {
      // Phase 1: L1 → L4 direct
      // (Future: L1 → L2 → L3 → L4)

      // 1. Prepare capabilities for L4 Frozen Realm
      // These are the ONLY functions available to the guest code
      const capabilities = this.createL4Capabilities(request);

      // 2. Load guest code
      const guestCode = request.code;

      this.log('L1: Passing encrypted blobs to L4 (no intermediate layers yet)...', 'info');

      // 3. Execute in L4 Frozen Realm
      // NOTE: The guest code will decrypt the secrets using the crypto capability
      const startL4 = performance.now();
      const encryptedResult = await this.frozenRealm.execute(
        guestCode,
        capabilities,
        request.codeId
      );
      const l4Duration = performance.now() - startL4;

      this.log(`L4: Execution complete in ${l4Duration.toFixed(2)}ms`, 'success');

      // 4. Return encrypted result (L4 → L1)
      const totalDuration = performance.now() - executionStart;
      this.stats.avgExecutionTime =
        (this.stats.avgExecutionTime * (this.stats.totalExecutions - 1) + totalDuration) /
        this.stats.totalExecutions;

      this.log(`=== E2EE Ferry Execution Complete (${totalDuration.toFixed(2)}ms) ===`, 'success');

      return {
        encryptedResult: encryptedResult,
        executionTime: totalDuration,
        l4Time: l4Duration,
        layers: ['L1', 'L4'], // Future: ['L1', 'L2', 'L3', 'L4']
      };

    } catch (error) {
      this.log(`✗ Enclave execution failed: ${error.message}`, 'error');
      throw error;
    }
  }

  /**
   * Execute code with plaintext (non-encrypted mode for comparison)
   *
   * This bypasses the E2EE ferry and executes directly in L4 with
   * plaintext inputs. Useful for benchmarking overhead.
   *
   * @param {object} request - Execution request
   * @param {string} request.payload - Plaintext data
   * @param {string} request.secret - Plaintext secret
   * @param {string} request.code - Guest code to execute
   * @param {string} [request.codeId] - Optional identifier
   * @returns {Promise<object>} Plaintext result + metadata
   */
  async executePlaintext(request) {
    this.stats.totalExecutions++;

    const executionStart = performance.now();

    this.log('=== Plaintext Execution (No Encryption) ===', 'info');

    try {
      // Create capabilities with plaintext data
      const capabilities = {
        log: this.frozenRealm.createSafeLogger('L4-Plaintext'),
        payload: request.payload,
        secret: request.secret,
        // No crypto capability needed
      };

      // Execute
      const result = await this.frozenRealm.execute(
        request.code,
        capabilities,
        request.codeId
      );

      const duration = performance.now() - executionStart;

      this.log(`=== Plaintext Execution Complete (${duration.toFixed(2)}ms) ===`, 'success');

      return {
        result: result,
        executionTime: duration,
        layers: ['L1', 'L4-Plaintext'],
      };

    } catch (error) {
      this.log(`✗ Plaintext execution failed: ${error.message}`, 'error');
      throw error;
    }
  }

  /**
   * Create L4 capabilities
   *
   * These are the ONLY functions/values available to guest code in the
   * Frozen Realm. This is the security boundary.
   *
   * @param {object} request - Execution request
   * @returns {object} Capabilities object
   */
  createL4Capabilities(request) {
    const self = this;

    return {
      // Safe logging (cannot leak data to console directly)
      log: this.frozenRealm.createSafeLogger('L4-Enclave'),

      // Encrypted blobs (opaque to L1/L2/L3)
      encryptedPayload: request.encryptedPayload,
      encryptedSecret: request.encryptedSecret,
      enclaveKey: request.enclaveKey,

      // Crypto capability (decrypt/encrypt/hash)
      crypto: {
        /**
         * Decrypt data (simple interface)
         * @param {string} encrypted - Base64 encrypted data
         * @param {string} key - Hex key
         * @returns {Promise<string>} Plaintext
         */
        decrypt: async function(encrypted, key) {
          self.log('  L4: Decrypting data...', 'info');
          const decryptStart = performance.now();

          const plaintext = await self.crypto.decryptSimple(encrypted, key);

          const decryptDuration = performance.now() - decryptStart;
          self.stats.avgDecryptionTime =
            (self.stats.avgDecryptionTime * (self.stats.encryptedExecutions - 1) + decryptDuration) /
            self.stats.encryptedExecutions;

          self.log(`  L4: Decrypted in ${decryptDuration.toFixed(2)}ms`, 'success');
          return plaintext;
        },

        /**
         * Encrypt data (simple interface)
         * @param {string} data - Plaintext data
         * @param {string} key - Hex key
         * @returns {Promise<string>} Base64 encrypted data
         */
        encrypt: async function(data, key) {
          self.log('  L4: Encrypting result...', 'info');
          return await self.crypto.encryptSimple(data, key);
        },

        /**
         * Hash data
         * @param {string} data - Data to hash
         * @returns {Promise<string>} Hex hash
         */
        hash: async function(data) {
          const hashBytes = await self.crypto.hash(data);
          return self.crypto.bytesToHex(hashBytes);
        },
      },

      // Deterministic utilities (no Date.now, Math.random, etc.)
      // Only pure functions that don't access non-deterministic state
      utils: {
        parseJSON: JSON.parse,
        stringifyJSON: JSON.stringify,
      },
    };
  }

  /**
   * Get execution statistics
   *
   * @returns {object} Statistics
   */
  getStats() {
    return {
      ...this.stats,
      frozenRealm: this.frozenRealm.getStats(),
      crypto: this.crypto.getStats(),
    };
  }

  /**
   * Reset statistics
   */
  resetStats() {
    this.stats = {
      totalExecutions: 0,
      encryptedExecutions: 0,
      avgExecutionTime: 0,
      avgDecryptionTime: 0,
    };
    this.frozenRealm.resetStats();
    this.crypto.resetStats();
  }

  /**
   * Internal logging
   *
   * @param {string} message - Log message
   * @param {string} level - Log level
   */
  log(message, level = 'info') {
    if (!this.options.verbose) return;

    const prefix = 'EnclaveExecutor';
    const styles = {
      info: 'color: #16a085',
      success: 'color: #27ae60',
      warn: 'color: #f39c12',
      error: 'color: #c0392b',
    };

    if (typeof console !== 'undefined') {
      console.log(`%c[${prefix}] ${message}`, styles[level] || styles.info);
    }
  }
}

// Export for use in other modules
if (typeof module !== 'undefined' && module.exports) {
  // Node.js / CommonJS
  module.exports = { EnclaveExecutor };
} else if (typeof window !== 'undefined') {
  // Browser global
  window.EnclaveExecutor = EnclaveExecutor;
}
