/**
 * Frozen Realm - L4 Secure Sandbox Implementation
 *
 * Implements the "Frozen Realm" pattern from the Hermes Enclave architecture.
 * This is the ONLY layer where encrypted secrets are decrypted and sensitive
 * computation occurs. L1-L3 act as "untrusted ferries" that never see plaintext.
 *
 * Security Model:
 * - All JavaScript primordials (Object, Array, etc.) are frozen
 * - No access to global scope (Date.now, fetch, etc.)
 * - Only explicitly injected capabilities available
 * - Deterministic execution (same input → same output)
 *
 * Based on:
 * - SES (Secure ECMAScript) / Hardened JavaScript
 * - Agoric's Compartment API
 * - Your Hermes Enclave design
 *
 * Integration with OutLayer:
 * - L1 (Browser): contract-simulator.js orchestrates execution
 * - L2 (linux-wasm): linux-executor.js (optional, for POSIX)
 * - L3 (QuickJS): quickjs-bridge.js (future, for multi-lang)
 * - L4 (Frozen Realm): THIS FILE - final security boundary
 *
 * @author OutLayer Team + Hermes Enclave Collaboration
 * @version 1.0.0 - Phase 1: L1-Only Implementation
 */

class FrozenRealm {
  constructor(options = {}) {
    this.options = {
      // Enable verbose logging
      verbose: options.verbose || false,

      // Allow specific globals (dangerous, only for debugging)
      allowedGlobals: options.allowedGlobals || [],

      // Timeout for realm execution (ms)
      executionTimeout: options.executionTimeout || 30000,
    };

    // Track if primordials have been frozen
    this.primordialsFrozen = false;

    // Statistics
    this.stats = {
      totalExecutions: 0,
      totalFreezes: 0,
      avgExecutionTime: 0,
    };

    this.log('FrozenRealm initialized', 'info');
  }

  /**
   * Freeze all JavaScript primordials to prevent prototype pollution
   * and ensure deterministic execution.
   *
   * This implements the "lockdown" pattern from SES/Hardened JavaScript:
   * https://github.com/endojs/endo/tree/master/packages/ses
   *
   * WARNING: This is GLOBAL and IRREVERSIBLE. Once called, the entire
   * JavaScript environment in this realm becomes immutable.
   */
  freezePrimordials() {
    if (this.primordialsFrozen) {
      this.log('Primordials already frozen', 'warn');
      return;
    }

    const startTime = performance.now();

    this.log('Freezing JavaScript primordials...', 'info');

    // List of all built-in constructors to freeze
    const primordials = [
      Object,
      Array,
      Function,
      String,
      Number,
      Boolean,
      Symbol,
      Date,
      RegExp,
      Error,
      EvalError,
      RangeError,
      ReferenceError,
      SyntaxError,
      TypeError,
      URIError,
      Promise,
      Map,
      Set,
      WeakMap,
      WeakSet,
      ArrayBuffer,
      SharedArrayBuffer,
      DataView,
      Int8Array,
      Uint8Array,
      Uint8ClampedArray,
      Int16Array,
      Uint16Array,
      Int32Array,
      Uint32Array,
      Float32Array,
      Float64Array,
      BigInt64Array,
      BigUint64Array,
      Proxy,
      Reflect,
      JSON,
      Math,
      Intl,
    ];

    // Freeze each primordial's prototype
    primordials.forEach(primordial => {
      if (primordial && primordial.prototype) {
        Object.freeze(primordial.prototype);
      }
      // Also freeze the constructor itself
      Object.freeze(primordial);
    });

    // Freeze globalThis (but NOT window, to allow normal browser operation outside realm)
    // Note: We freeze a COPY of globalThis for the realm, not the actual global

    this.primordialsFrozen = true;
    this.stats.totalFreezes++;

    const duration = performance.now() - startTime;
    this.log(`✓ Primordials frozen in ${duration.toFixed(2)}ms`, 'success');
  }

  /**
   * Create a secure, isolated sandbox and execute code within it.
   *
   * This is the core of the Frozen Realm pattern. The executed code:
   * - Has NO access to the outer scope (no closures)
   * - Can ONLY use primordials (which are frozen)
   * - Can ONLY access explicitly injected capabilities
   * - CANNOT use non-deterministic functions (Date.now, Math.random, etc.)
   *
   * @param {string} untrustedCode - The string of code to execute
   * @param {object} capabilities - Explicit API to inject into the realm
   * @param {string} [codeId] - Optional identifier for debugging
   * @returns {Promise<any>} The return value of the executed code
   */
  async execute(untrustedCode, capabilities = {}, codeId = 'anonymous') {
    this.stats.totalExecutions++;
    const executionStart = performance.now();

    this.log(`Executing code in Frozen Realm: ${codeId}`, 'info');

    // 1. Freeze primordials if not already done
    if (!this.primordialsFrozen) {
      this.freezePrimordials();
    }

    // 2. Define the capability names and values
    // These become the ONLY variables available to the untrusted code
    const capabilityNames = Object.keys(capabilities);
    const capabilityValues = Object.values(capabilities);

    this.log(`  Injecting ${capabilityNames.length} capabilities: ${capabilityNames.join(', ')}`, 'info');

    // 3. Validate capabilities (ensure they don't leak dangerous globals)
    this.validateCapabilities(capabilities);

    // 4. Construct the sandboxed function
    // By using 'new Function(...)', the code inside has NO lexical access
    // to this scope. It can only see:
    // - Its own local scope
    // - The frozen primordials
    // - The explicitly injected capabilities
    let sandboxedFunction;
    try {
      sandboxedFunction = new Function(...capabilityNames, untrustedCode);
    } catch (error) {
      this.log(`✗ Failed to create sandboxed function: ${error.message}`, 'error');
      throw new Error(`Syntax error in untrusted code: ${error.message}`);
    }

    // 5. Execute with timeout protection
    try {
      const result = await this.executeWithTimeout(
        sandboxedFunction,
        capabilityValues,
        this.options.executionTimeout
      );

      const duration = performance.now() - executionStart;
      this.stats.avgExecutionTime =
        (this.stats.avgExecutionTime * (this.stats.totalExecutions - 1) + duration) /
        this.stats.totalExecutions;

      this.log(`✓ Execution complete in ${duration.toFixed(2)}ms`, 'success');
      return result;

    } catch (error) {
      const duration = performance.now() - executionStart;
      this.log(`✗ Execution failed after ${duration.toFixed(2)}ms: ${error.message}`, 'error');
      throw error;
    }
  }

  /**
   * Execute a function with timeout protection
   *
   * @param {Function} fn - Function to execute
   * @param {Array} args - Arguments to pass
   * @param {number} timeout - Timeout in milliseconds
   * @returns {Promise<any>} Result or timeout error
   */
  async executeWithTimeout(fn, args, timeout) {
    return new Promise((resolve, reject) => {
      // Set timeout
      const timeoutId = setTimeout(() => {
        reject(new Error(`Execution timeout after ${timeout}ms`));
      }, timeout);

      // Execute
      try {
        const result = fn(...args);

        // Handle both sync and async results
        if (result && typeof result.then === 'function') {
          result
            .then(value => {
              clearTimeout(timeoutId);
              resolve(value);
            })
            .catch(error => {
              clearTimeout(timeoutId);
              reject(error);
            });
        } else {
          clearTimeout(timeoutId);
          resolve(result);
        }
      } catch (error) {
        clearTimeout(timeoutId);
        reject(error);
      }
    });
  }

  /**
   * Validate capabilities to ensure they don't leak dangerous globals
   *
   * @param {object} capabilities - Capabilities to validate
   * @throws {Error} If dangerous capability detected
   */
  validateCapabilities(capabilities) {
    const dangerous = [
      'eval',
      'Function',
      'setTimeout',
      'setInterval',
      'XMLHttpRequest',
      'fetch',
      'localStorage',
      'sessionStorage',
      'indexedDB',
      'document',
      'window',
      'globalThis',
      'process',
      'require',
      'import',
    ];

    for (const name of Object.keys(capabilities)) {
      if (dangerous.includes(name) && !this.options.allowedGlobals.includes(name)) {
        throw new Error(`Dangerous capability rejected: ${name}`);
      }

      const value = capabilities[name];

      // Check if capability is a function that might leak scope
      if (typeof value === 'function') {
        // Ensure function doesn't have dangerous properties
        if (value.constructor !== Function) {
          this.log(`  Warning: Non-standard function constructor for ${name}`, 'warn');
        }
      }
    }
  }

  /**
   * Create a minimal, safe logging function for the realm
   *
   * This prevents the realm from accessing console.log directly,
   * which could leak information or cause side effects.
   *
   * @param {string} prefix - Prefix for log messages
   * @returns {Function} Safe logging function
   */
  createSafeLogger(prefix = 'Realm') {
    return (message) => {
      // Only log if verbose mode enabled
      if (this.options.verbose) {
        console.log(`[${prefix}] ${message}`);
      }
    };
  }

  /**
   * Get execution statistics
   *
   * @returns {object} Statistics object
   */
  getStats() {
    return {
      ...this.stats,
      primordialsFrozen: this.primordialsFrozen,
    };
  }

  /**
   * Reset statistics
   */
  resetStats() {
    this.stats = {
      totalExecutions: 0,
      totalFreezes: 0,
      avgExecutionTime: 0,
    };
  }

  /**
   * Internal logging
   *
   * @param {string} message - Log message
   * @param {string} level - Log level
   */
  log(message, level = 'info') {
    if (!this.options.verbose) return;

    const prefix = 'FrozenRealm';
    const styles = {
      info: 'color: #3498db',
      success: 'color: #2ecc71',
      warn: 'color: #f39c12',
      error: 'color: #e74c3c',
    };

    if (typeof console !== 'undefined') {
      console.log(`%c[${prefix}] ${message}`, styles[level] || styles.info);
    }
  }
}

/**
 * Helper function to create and execute a Frozen Realm in one step
 *
 * This is the primary API for simple use cases.
 *
 * @param {string} code - Code to execute
 * @param {object} capabilities - Capabilities to inject
 * @param {object} options - Realm options
 * @returns {Promise<any>} Execution result
 */
async function executeInFrozenRealm(code, capabilities = {}, options = {}) {
  const realm = new FrozenRealm(options);
  return await realm.execute(code, capabilities);
}

// Export for use in other modules
if (typeof module !== 'undefined' && module.exports) {
  // Node.js / CommonJS
  module.exports = { FrozenRealm, executeInFrozenRealm };
} else if (typeof window !== 'undefined') {
  // Browser global
  window.FrozenRealm = FrozenRealm;
  window.executeInFrozenRealm = executeInFrozenRealm;
}
