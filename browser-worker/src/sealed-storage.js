/**
 * sealed-storage.js - WebCrypto-based sealed storage for NEAR state
 *
 * Provides AES-GCM encryption and ECDSA P-256 attestation for contract state.
 * State is encrypted with a master key stored in IndexedDB.
 * Attestations prove state integrity without revealing contents.
 *
 * Architecture:
 * - Master Key: AES-GCM 256-bit key (generated once, stored in IndexedDB)
 * - Attestation Key: ECDSA P-256 keypair (ephemeral per session)
 * - Sealed State: { iv, ciphertext, timestamp }
 * - Attestation: { state_hash, signature, public_key, timestamp }
 *
 * Phase 3: Sealed Storage with WebCrypto
 */

class SealedStorage {
  constructor() {
    this.masterKey = null;
    this.attestationKeyPair = null;
    this.dbName = 'near-outlayer-storage';
    this.dbVersion = 1;
    this.db = null;
  }

  /**
   * Initialize sealed storage system
   * - Opens IndexedDB connection
   * - Retrieves or generates master encryption key
   * - Generates ephemeral attestation keypair
   *
   * @returns {Promise<void>}
   */
  async initialize() {
    // Open IndexedDB
    await this.openDatabase();

    // Retrieve or generate master key
    const storedKeyJwk = await this.getFromIndexedDB('master-key');

    if (storedKeyJwk) {
      // Import existing master key
      this.masterKey = await crypto.subtle.importKey(
        'jwk',
        storedKeyJwk,
        { name: 'AES-GCM', length: 256 },
        true,
        ['encrypt', 'decrypt']
      );
      console.log('✓ Master key loaded from IndexedDB');
    } else {
      // Generate new master key
      this.masterKey = await crypto.subtle.generateKey(
        { name: 'AES-GCM', length: 256 },
        true,
        ['encrypt', 'decrypt']
      );

      // Export and store for persistence
      const exportedKey = await crypto.subtle.exportKey('jwk', this.masterKey);
      await this.storeInIndexedDB('master-key', exportedKey);
      console.log('✓ Master key generated and stored');
    }

    // Generate ephemeral attestation keypair (not persisted - fresh per session)
    this.attestationKeyPair = await crypto.subtle.generateKey(
      { name: 'ECDSA', namedCurve: 'P-256' },
      true,
      ['sign', 'verify']
    );
    console.log('✓ Attestation keypair generated (ephemeral)');
  }

  /**
   * Open IndexedDB database
   * Creates object store if needed
   *
   * @returns {Promise<IDBDatabase>}
   */
  openDatabase() {
    return new Promise((resolve, reject) => {
      const request = indexedDB.open(this.dbName, this.dbVersion);

      request.onerror = () => reject(request.error);
      request.onsuccess = () => {
        this.db = request.result;
        resolve(this.db);
      };

      request.onupgradeneeded = (event) => {
        const db = event.target.result;

        // Create object store for key-value pairs
        if (!db.objectStoreNames.contains('kv-store')) {
          db.createObjectStore('kv-store', { keyPath: 'key' });
        }
      };
    });
  }

  /**
   * Store data in IndexedDB
   *
   * @param {string} key - Storage key
   * @param {any} value - Value to store (must be structured-cloneable)
   * @returns {Promise<void>}
   */
  async storeInIndexedDB(key, value) {
    return new Promise((resolve, reject) => {
      const transaction = this.db.transaction(['kv-store'], 'readwrite');
      const store = transaction.objectStore('kv-store');
      const request = store.put({ key, value, timestamp: Date.now() });

      request.onsuccess = () => resolve();
      request.onerror = () => reject(request.error);
    });
  }

  /**
   * Retrieve data from IndexedDB
   *
   * @param {string} key - Storage key
   * @returns {Promise<any>} - Stored value or null if not found
   */
  async getFromIndexedDB(key) {
    return new Promise((resolve, reject) => {
      const transaction = this.db.transaction(['kv-store'], 'readonly');
      const store = transaction.objectStore('kv-store');
      const request = store.get(key);

      request.onsuccess = () => {
        const result = request.result;
        resolve(result ? result.value : null);
      };
      request.onerror = () => reject(request.error);
    });
  }

  /**
   * Delete data from IndexedDB
   *
   * @param {string} key - Storage key
   * @returns {Promise<void>}
   */
  async deleteFromIndexedDB(key) {
    return new Promise((resolve, reject) => {
      const transaction = this.db.transaction(['kv-store'], 'readwrite');
      const store = transaction.objectStore('kv-store');
      const request = store.delete(key);

      request.onsuccess = () => resolve();
      request.onerror = () => reject(request.error);
    });
  }

  /**
   * Seal (encrypt) contract state
   * Uses AES-GCM with random IV
   *
   * @param {Map|Array} state - Contract state (Map or array of [key, value] pairs)
   * @returns {Promise<Object>} - { iv, ciphertext, timestamp }
   */
  async seal(state) {
    if (!this.masterKey) {
      throw new Error('SealedStorage not initialized - call initialize() first');
    }

    // Convert state to serializable format
    let stateArray;
    if (state instanceof Map) {
      stateArray = Array.from(state.entries());
    } else if (Array.isArray(state)) {
      stateArray = state;
    } else {
      throw new Error('State must be a Map or Array');
    }

    // Serialize state to JSON
    const stateJson = JSON.stringify(stateArray);
    const stateBytes = new TextEncoder().encode(stateJson);

    // Generate random IV (12 bytes for GCM)
    const iv = crypto.getRandomValues(new Uint8Array(12));

    // Encrypt with AES-GCM
    const ciphertextBuffer = await crypto.subtle.encrypt(
      { name: 'AES-GCM', iv },
      this.masterKey,
      stateBytes
    );

    const ciphertext = new Uint8Array(ciphertextBuffer);

    return {
      iv: Array.from(iv),
      ciphertext: Array.from(ciphertext),
      timestamp: Date.now()
    };
  }

  /**
   * Unseal (decrypt) contract state
   *
   * @param {Object} sealed - { iv, ciphertext }
   * @returns {Promise<Map>} - Decrypted state as Map
   */
  async unseal(sealed) {
    if (!this.masterKey) {
      throw new Error('SealedStorage not initialized - call initialize() first');
    }

    const iv = new Uint8Array(sealed.iv);
    const ciphertext = new Uint8Array(sealed.ciphertext);

    // Decrypt with AES-GCM
    const plaintextBuffer = await crypto.subtle.decrypt(
      { name: 'AES-GCM', iv },
      this.masterKey,
      ciphertext
    );

    // Deserialize JSON
    const stateJson = new TextDecoder().decode(plaintextBuffer);
    const stateArray = JSON.parse(stateJson);

    // Convert back to Map
    return new Map(stateArray);
  }

  /**
   * Generate attestation for contract state
   * Computes SHA-256 hash and signs with ECDSA P-256
   *
   * @param {Map|Array} state - Contract state
   * @returns {Promise<Object>} - { state_hash, signature, public_key, timestamp, attestation_type }
   */
  async generateAttestation(state) {
    if (!this.attestationKeyPair) {
      throw new Error('SealedStorage not initialized - call initialize() first');
    }

    // Convert state to serializable format
    let stateArray;
    if (state instanceof Map) {
      stateArray = Array.from(state.entries());
    } else if (Array.isArray(state)) {
      stateArray = state;
    } else {
      throw new Error('State must be a Map or Array');
    }

    // Compute state hash (SHA-256)
    const stateJson = JSON.stringify(stateArray);
    const stateBytes = new TextEncoder().encode(stateJson);
    const hashBuffer = await crypto.subtle.digest('SHA-256', stateBytes);
    const stateHash = new Uint8Array(hashBuffer);

    // Sign state hash with ECDSA P-256
    const signatureBuffer = await crypto.subtle.sign(
      { name: 'ECDSA', hash: 'SHA-256' },
      this.attestationKeyPair.privateKey,
      hashBuffer
    );
    const signature = new Uint8Array(signatureBuffer);

    // Export public key for verification
    const publicKeyJwk = await crypto.subtle.exportKey(
      'jwk',
      this.attestationKeyPair.publicKey
    );

    return {
      state_hash: Array.from(stateHash),
      signature: Array.from(signature),
      public_key: publicKeyJwk,
      timestamp: Date.now(),
      attestation_type: 'webcrypto-ecdsa-p256'
    };
  }

  /**
   * Verify attestation signature
   *
   * @param {Object} attestation - Attestation object
   * @param {Array} expectedStateHash - Expected state hash (optional)
   * @returns {Promise<boolean>} - True if signature is valid
   */
  async verifyAttestation(attestation, expectedStateHash = null) {
    // Import public key from attestation
    const publicKey = await crypto.subtle.importKey(
      'jwk',
      attestation.public_key,
      { name: 'ECDSA', namedCurve: 'P-256' },
      true,
      ['verify']
    );

    // Convert arrays back to typed arrays
    const stateHashBuffer = new Uint8Array(attestation.state_hash);
    const signatureBuffer = new Uint8Array(attestation.signature);

    // Verify signature
    const signatureValid = await crypto.subtle.verify(
      { name: 'ECDSA', hash: 'SHA-256' },
      publicKey,
      signatureBuffer,
      stateHashBuffer
    );

    if (!signatureValid) {
      return false;
    }

    // Optionally check state hash matches expected value
    if (expectedStateHash) {
      const hashMatches = JSON.stringify(attestation.state_hash) ===
                         JSON.stringify(expectedStateHash);
      return hashMatches;
    }

    return true;
  }

  /**
   * Persist sealed state to IndexedDB
   *
   * @param {string} contractId - Contract identifier
   * @param {Object} sealed - Sealed state object
   * @returns {Promise<void>}
   */
  async persistSealedState(contractId, sealed) {
    const key = `sealed-state:${contractId}`;
    await this.storeInIndexedDB(key, sealed);
  }

  /**
   * Load sealed state from IndexedDB
   *
   * @param {string} contractId - Contract identifier
   * @returns {Promise<Object|null>} - Sealed state or null if not found
   */
  async loadSealedState(contractId) {
    const key = `sealed-state:${contractId}`;
    return await this.getFromIndexedDB(key);
  }

  /**
   * Persist attestation to IndexedDB
   *
   * @param {string} contractId - Contract identifier
   * @param {Object} attestation - Attestation object
   * @returns {Promise<void>}
   */
  async persistAttestation(contractId, attestation) {
    const key = `attestation:${contractId}`;
    await this.storeInIndexedDB(key, attestation);
  }

  /**
   * Load attestation from IndexedDB
   *
   * @param {string} contractId - Contract identifier
   * @returns {Promise<Object|null>} - Attestation or null if not found
   */
  async loadAttestation(contractId) {
    const key = `attestation:${contractId}`;
    return await this.getFromIndexedDB(key);
  }

  /**
   * Clear all sealed storage data
   *
   * @returns {Promise<void>}
   */
  async clearAll() {
    return new Promise((resolve, reject) => {
      const transaction = this.db.transaction(['kv-store'], 'readwrite');
      const store = transaction.objectStore('kv-store');
      const request = store.clear();

      request.onsuccess = () => {
        console.log('✓ All sealed storage data cleared');
        resolve();
      };
      request.onerror = () => reject(request.error);
    });
  }

  /**
   * Export master key (for backup purposes)
   * WARNING: This exposes the encryption key!
   *
   * @returns {Promise<Object>} - JWK representation of master key
   */
  async exportMasterKey() {
    if (!this.masterKey) {
      throw new Error('SealedStorage not initialized');
    }
    return await crypto.subtle.exportKey('jwk', this.masterKey);
  }

  /**
   * Import master key (for restore purposes)
   *
   * @param {Object} keyJwk - JWK representation of master key
   * @returns {Promise<void>}
   */
  async importMasterKey(keyJwk) {
    this.masterKey = await crypto.subtle.importKey(
      'jwk',
      keyJwk,
      { name: 'AES-GCM', length: 256 },
      true,
      ['encrypt', 'decrypt']
    );
    await this.storeInIndexedDB('master-key', keyJwk);
    console.log('✓ Master key imported and stored');
  }
}

// Export for use in other modules
if (typeof module !== 'undefined' && module.exports) {
  module.exports = SealedStorage;
}
