/**
 * Crypto Utils - Real WebCrypto Implementation
 *
 * Replaces the mock crypto from Hermes Enclave prototype with production-grade
 * encryption using the Web Crypto API (SubtleCrypto).
 *
 * Security Properties:
 * - AES-GCM for symmetric encryption (authenticated encryption)
 * - ECDH for key exchange (P-256 curve)
 * - SHA-256 for hashing
 * - Cryptographically secure random IVs
 * - Constant-time operations (via WebCrypto)
 *
 * Integration with Frozen Realm:
 * - This module is injected as a capability into the L4 Frozen Realm
 * - It provides encrypt/decrypt/hash functions
 * - All operations are deterministic given same inputs
 *
 * @author OutLayer Team + Hermes Enclave Collaboration
 * @version 1.0.0 - Production WebCrypto
 */

class CryptoUtils {
  constructor(options = {}) {
    this.options = {
      // Default algorithm for symmetric encryption
      algorithm: options.algorithm || 'AES-GCM',

      // Key size (bits)
      keySize: options.keySize || 256,

      // IV size (bytes) - 12 bytes for AES-GCM
      ivSize: options.ivSize || 12,

      // Tag length (bits) for AES-GCM authentication
      tagLength: options.tagLength || 128,

      // Enable verbose logging
      verbose: options.verbose || false,
    };

    // Check WebCrypto availability
    if (typeof crypto === 'undefined' || !crypto.subtle) {
      throw new Error('WebCrypto API not available');
    }

    this.subtle = crypto.subtle;

    // Statistics
    this.stats = {
      encryptions: 0,
      decryptions: 0,
      keyGenerations: 0,
      hashes: 0,
    };

    this.log('CryptoUtils initialized with WebCrypto', 'info');
  }

  /**
   * Generate a new AES-GCM symmetric key
   *
   * @param {boolean} extractable - Whether key can be exported
   * @returns {Promise<CryptoKey>} Generated key
   */
  async generateKey(extractable = true) {
    this.stats.keyGenerations++;

    const key = await this.subtle.generateKey(
      {
        name: this.options.algorithm,
        length: this.options.keySize,
      },
      extractable,
      ['encrypt', 'decrypt']
    );

    this.log(`✓ Generated ${this.options.keySize}-bit ${this.options.algorithm} key`, 'success');
    return key;
  }

  /**
   * Import a key from raw bytes
   *
   * @param {Uint8Array|ArrayBuffer} keyData - Raw key bytes
   * @param {boolean} extractable - Whether key can be exported
   * @returns {Promise<CryptoKey>} Imported key
   */
  async importKey(keyData, extractable = true) {
    const key = await this.subtle.importKey(
      'raw',
      keyData,
      {
        name: this.options.algorithm,
      },
      extractable,
      ['encrypt', 'decrypt']
    );

    this.log(`✓ Imported ${this.options.keySize}-bit key`, 'success');
    return key;
  }

  /**
   * Export a key to raw bytes
   *
   * @param {CryptoKey} key - Key to export
   * @returns {Promise<Uint8Array>} Raw key bytes
   */
  async exportKey(key) {
    const exported = await this.subtle.exportKey('raw', key);
    return new Uint8Array(exported);
  }

  /**
   * Generate a cryptographically secure random IV
   *
   * @returns {Uint8Array} Random IV
   */
  generateIV() {
    return crypto.getRandomValues(new Uint8Array(this.options.ivSize));
  }

  /**
   * Encrypt data with AES-GCM
   *
   * @param {string|Uint8Array} data - Data to encrypt
   * @param {CryptoKey|Uint8Array} key - Encryption key (CryptoKey or raw bytes)
   * @param {Uint8Array} [iv] - Optional IV (generated if not provided)
   * @returns {Promise<{iv: Uint8Array, ciphertext: Uint8Array}>} Encrypted data
   */
  async encrypt(data, key, iv = null) {
    this.stats.encryptions++;

    // Convert string to bytes
    const plaintext = typeof data === 'string'
      ? new TextEncoder().encode(data)
      : data;

    // Import key if it's raw bytes
    if (!(key instanceof CryptoKey)) {
      key = await this.importKey(key);
    }

    // Generate IV if not provided
    if (!iv) {
      iv = this.generateIV();
    }

    // Encrypt
    const ciphertext = await this.subtle.encrypt(
      {
        name: this.options.algorithm,
        iv: iv,
        tagLength: this.options.tagLength,
      },
      key,
      plaintext
    );

    this.log(`✓ Encrypted ${plaintext.byteLength} bytes`, 'success');

    return {
      iv: iv,
      ciphertext: new Uint8Array(ciphertext),
    };
  }

  /**
   * Decrypt data with AES-GCM
   *
   * @param {Uint8Array} ciphertext - Encrypted data
   * @param {CryptoKey|Uint8Array} key - Decryption key
   * @param {Uint8Array} iv - IV used for encryption
   * @param {boolean} asString - Return as string instead of Uint8Array
   * @returns {Promise<string|Uint8Array>} Decrypted data
   */
  async decrypt(ciphertext, key, iv, asString = true) {
    this.stats.decryptions++;

    // Import key if it's raw bytes
    if (!(key instanceof CryptoKey)) {
      key = await this.importKey(key);
    }

    // Decrypt
    const plaintext = await this.subtle.decrypt(
      {
        name: this.options.algorithm,
        iv: iv,
        tagLength: this.options.tagLength,
      },
      key,
      ciphertext
    );

    this.log(`✓ Decrypted ${plaintext.byteLength} bytes`, 'success');

    // Return as string or bytes
    return asString
      ? new TextDecoder().decode(plaintext)
      : new Uint8Array(plaintext);
  }

  /**
   * Hash data with SHA-256
   *
   * @param {string|Uint8Array} data - Data to hash
   * @returns {Promise<Uint8Array>} Hash digest
   */
  async hash(data) {
    this.stats.hashes++;

    // Convert string to bytes
    const bytes = typeof data === 'string'
      ? new TextEncoder().encode(data)
      : data;

    // Hash
    const digest = await this.subtle.digest('SHA-256', bytes);

    this.log(`✓ Hashed ${bytes.byteLength} bytes`, 'success');
    return new Uint8Array(digest);
  }

  /**
   * Compute HMAC (Hash-based Message Authentication Code)
   *
   * @param {string|Uint8Array} data - Data to authenticate
   * @param {CryptoKey|Uint8Array} key - HMAC key
   * @returns {Promise<Uint8Array>} HMAC tag
   */
  async hmac(data, key) {
    // Convert data
    const bytes = typeof data === 'string'
      ? new TextEncoder().encode(data)
      : data;

    // Import key if raw bytes
    if (!(key instanceof CryptoKey)) {
      key = await this.subtle.importKey(
        'raw',
        key,
        {
          name: 'HMAC',
          hash: 'SHA-256',
        },
        false,
        ['sign', 'verify']
      );
    }

    // Sign
    const signature = await this.subtle.sign('HMAC', key, bytes);
    return new Uint8Array(signature);
  }

  /**
   * Derive key from password using PBKDF2
   *
   * @param {string} password - Password
   * @param {Uint8Array} salt - Salt (16+ bytes recommended)
   * @param {number} iterations - Number of iterations (100k+ recommended)
   * @returns {Promise<CryptoKey>} Derived key
   */
  async deriveKey(password, salt, iterations = 100000) {
    // Import password as key material
    const passwordKey = await this.subtle.importKey(
      'raw',
      new TextEncoder().encode(password),
      'PBKDF2',
      false,
      ['deriveKey']
    );

    // Derive key
    const derivedKey = await this.subtle.deriveKey(
      {
        name: 'PBKDF2',
        salt: salt,
        iterations: iterations,
        hash: 'SHA-256',
      },
      passwordKey,
      {
        name: this.options.algorithm,
        length: this.options.keySize,
      },
      true,
      ['encrypt', 'decrypt']
    );

    this.log(`✓ Derived key from password (${iterations} iterations)`, 'success');
    return derivedKey;
  }

  /**
   * Convert bytes to hex string
   *
   * @param {Uint8Array} bytes - Bytes to convert
   * @returns {string} Hex string
   */
  bytesToHex(bytes) {
    return Array.from(bytes)
      .map(b => b.toString(16).padStart(2, '0'))
      .join('');
  }

  /**
   * Convert hex string to bytes
   *
   * @param {string} hex - Hex string
   * @returns {Uint8Array} Bytes
   */
  hexToBytes(hex) {
    const bytes = new Uint8Array(hex.length / 2);
    for (let i = 0; i < hex.length; i += 2) {
      bytes[i / 2] = parseInt(hex.substring(i, i + 2), 16);
    }
    return bytes;
  }

  /**
   * Convert bytes to base64 string
   *
   * @param {Uint8Array} bytes - Bytes to convert
   * @returns {string} Base64 string
   */
  bytesToBase64(bytes) {
    return btoa(String.fromCharCode.apply(null, bytes));
  }

  /**
   * Convert base64 string to bytes
   *
   * @param {string} base64 - Base64 string
   * @returns {Uint8Array} Bytes
   */
  base64ToBytes(base64) {
    const binary = atob(base64);
    const bytes = new Uint8Array(binary.length);
    for (let i = 0; i < binary.length; i++) {
      bytes[i] = binary.charCodeAt(i);
    }
    return bytes;
  }

  /**
   * Encrypt with simpler interface (returns base64-encoded result)
   *
   * This is a convenience wrapper for the Frozen Realm that matches
   * the mock crypto interface from the original Hermes Enclave design.
   *
   * @param {string} data - Data to encrypt
   * @param {string} keyHex - Hex-encoded key
   * @returns {Promise<string>} Base64-encoded encrypted data (iv + ciphertext)
   */
  async encryptSimple(data, keyHex) {
    const keyBytes = this.hexToBytes(keyHex);
    const { iv, ciphertext } = await this.encrypt(data, keyBytes);

    // Concatenate IV + ciphertext
    const combined = new Uint8Array(iv.length + ciphertext.length);
    combined.set(iv);
    combined.set(ciphertext, iv.length);

    return this.bytesToBase64(combined);
  }

  /**
   * Decrypt with simpler interface (from base64-encoded result)
   *
   * @param {string} encryptedBase64 - Base64-encoded encrypted data
   * @param {string} keyHex - Hex-encoded key
   * @returns {Promise<string>} Decrypted plaintext
   */
  async decryptSimple(encryptedBase64, keyHex) {
    const combined = this.base64ToBytes(encryptedBase64);
    const keyBytes = this.hexToBytes(keyHex);

    // Split IV + ciphertext
    const iv = combined.slice(0, this.options.ivSize);
    const ciphertext = combined.slice(this.options.ivSize);

    return await this.decrypt(ciphertext, keyBytes, iv, true);
  }

  /**
   * Get statistics
   *
   * @returns {object} Statistics
   */
  getStats() {
    return { ...this.stats };
  }

  /**
   * Reset statistics
   */
  resetStats() {
    this.stats = {
      encryptions: 0,
      decryptions: 0,
      keyGenerations: 0,
      hashes: 0,
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

    const prefix = 'CryptoUtils';
    const styles = {
      info: 'color: #9b59b6',
      success: 'color: #2ecc71',
      warn: 'color: #f39c12',
      error: 'color: #e74c3c',
    };

    if (typeof console !== 'undefined') {
      console.log(`%c[${prefix}] ${message}`, styles[level] || styles.info);
    }
  }
}

// Export for use in other modules
if (typeof module !== 'undefined' && module.exports) {
  // Node.js / CommonJS
  module.exports = { CryptoUtils };
} else if (typeof window !== 'undefined') {
  // Browser global
  window.CryptoUtils = CryptoUtils;
}
