/**
 * TypeScript definitions for crypto-utils.js
 * Plain vanilla types - here to help, not hinder!
 */

export interface CryptoUtilsOptions {
  /** Algorithm for symmetric encryption (default: AES-GCM) */
  algorithm?: 'AES-GCM';
  /** Key size in bits (default: 256) */
  keySize?: 256;
  /** IV size in bytes (default: 12) */
  ivSize?: 12;
  /** Tag length in bits for AES-GCM (default: 128) */
  tagLength?: 128;
  /** Enable verbose logging */
  verbose?: boolean;
}

export interface EncryptResult {
  iv: Uint8Array;
  ciphertext: Uint8Array;
}

export interface CryptoStats {
  encryptions: number;
  decryptions: number;
  keyGenerations: number;
  hashes: number;
}

export class CryptoUtils {
  constructor(options?: CryptoUtilsOptions);

  /** Generate new AES-GCM key */
  generateKey(extractable?: boolean): Promise<CryptoKey>;

  /** Import key from raw bytes */
  importKey(keyData: Uint8Array | ArrayBuffer, extractable?: boolean): Promise<CryptoKey>;

  /** Export key to raw bytes */
  exportKey(key: CryptoKey): Promise<Uint8Array>;

  /** Generate cryptographically secure random IV */
  generateIV(): Uint8Array;

  /** Encrypt data with AES-GCM */
  encrypt(
    data: string | Uint8Array,
    key: CryptoKey | Uint8Array,
    iv?: Uint8Array
  ): Promise<EncryptResult>;

  /** Decrypt data with AES-GCM */
  decrypt(
    ciphertext: Uint8Array,
    key: CryptoKey | Uint8Array,
    iv: Uint8Array,
    asString?: boolean
  ): Promise<string | Uint8Array>;

  /** Hash data with SHA-256 */
  hash(data: string | Uint8Array): Promise<Uint8Array>;

  /** Compute HMAC */
  hmac(data: string | Uint8Array, key: CryptoKey | Uint8Array): Promise<Uint8Array>;

  /** Derive key from password using PBKDF2 */
  deriveKey(password: string, salt: Uint8Array, iterations?: number): Promise<CryptoKey>;

  /** Simple encrypt (returns base64) - for Frozen Realm */
  encryptSimple(data: string, keyHex: string): Promise<string>;

  /** Simple decrypt (from base64) - for Frozen Realm */
  decryptSimple(encryptedBase64: string, keyHex: string): Promise<string>;

  /** Convert bytes to hex string */
  bytesToHex(bytes: Uint8Array): string;

  /** Convert hex string to bytes */
  hexToBytes(hex: string): Uint8Array;

  /** Convert bytes to base64 string */
  bytesToBase64(bytes: Uint8Array): string;

  /** Convert base64 string to bytes */
  base64ToBytes(base64: string): Uint8Array;

  /** Get statistics */
  getStats(): CryptoStats;

  /** Reset statistics */
  resetStats(): void;
}
