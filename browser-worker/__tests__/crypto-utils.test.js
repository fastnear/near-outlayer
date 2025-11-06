/**
 * Crypto Utils Unit Tests
 *
 * Tests WebCrypto wrapper functionality
 */

import { describe, test, expect, beforeAll } from '@jest/globals';

let CryptoUtils;

beforeAll(async () => {
  const module = await import('../src/crypto-utils.js');
  CryptoUtils = module.CryptoUtils;
});

describe('CryptoUtils: WebCrypto Wrapper', () => {

  test('should create instance with default options', () => {
    const crypto = new CryptoUtils();
    expect(crypto).toBeInstanceOf(CryptoUtils);
    expect(crypto.options.algorithm).toBe('AES-GCM');
    expect(crypto.options.keySize).toBe(256);
  });

  test('should generate AES-GCM key', async () => {
    const crypto = new CryptoUtils({ verbose: false });

    const key = await crypto.generateKey();

    expect(key).toBeInstanceOf(CryptoKey);
    expect(key.type).toBe('secret');
    expect(key.algorithm.name).toBe('AES-GCM');
  });

  test('should import and export keys', async () => {
    const crypto = new CryptoUtils({ verbose: false });

    const originalKeyData = new Uint8Array(32); // 256-bit key
    crypto.generateIV().forEach((byte, i) => { originalKeyData[i % 32] = byte; });

    const key = await crypto.importKey(originalKeyData);
    expect(key).toBeInstanceOf(CryptoKey);

    const exportedKeyData = await crypto.exportKey(key);
    expect(exportedKeyData).toEqual(originalKeyData);
  });

  test('should generate random IV', () => {
    const crypto = new CryptoUtils({ verbose: false });

    const iv1 = crypto.generateIV();
    const iv2 = crypto.generateIV();

    expect(iv1).toBeInstanceOf(Uint8Array);
    expect(iv1.length).toBe(12); // Default IV size
    expect(iv1).not.toEqual(iv2); // Should be random
  });

  test('should encrypt and decrypt data', async () => {
    const crypto = new CryptoUtils({ verbose: false });

    const plaintext = 'Hello, Hermes Enclave!';
    const key = await crypto.generateKey();

    // Encrypt
    const { iv, ciphertext } = await crypto.encrypt(plaintext, key);

    expect(iv).toBeInstanceOf(Uint8Array);
    expect(ciphertext).toBeInstanceOf(Uint8Array);
    expect(ciphertext.length).toBeGreaterThan(plaintext.length); // Due to auth tag

    // Decrypt
    const decrypted = await crypto.decrypt(ciphertext, key, iv, true);

    expect(decrypted).toBe(plaintext);
  });

  test('should encrypt with raw key bytes', async () => {
    const crypto = new CryptoUtils({ verbose: false });

    const plaintext = 'Test message';
    const keyBytes = crypto.hexToBytes('0123456789abcdef'.repeat(4)); // 32 bytes

    const { iv, ciphertext } = await crypto.encrypt(plaintext, keyBytes);

    expect(ciphertext).toBeInstanceOf(Uint8Array);

    // Decrypt with same key
    const decrypted = await crypto.decrypt(ciphertext, keyBytes, iv, true);
    expect(decrypted).toBe(plaintext);
  });

  test('should use encryptSimple / decryptSimple', async () => {
    const crypto = new CryptoUtils({ verbose: false });

    const plaintext = 'Simple interface test';
    const keyHex = '0123456789abcdef'.repeat(4);

    // Encrypt (returns base64)
    const encrypted = await crypto.encryptSimple(plaintext, keyHex);

    expect(typeof encrypted).toBe('string');
    expect(encrypted).toMatch(/^[A-Za-z0-9+/=]+$/); // Base64
    expect(encrypted).not.toContain(plaintext); // Not plaintext!

    // Decrypt
    const decrypted = await crypto.decryptSimple(encrypted, keyHex);

    expect(decrypted).toBe(plaintext);
  });

  test('should hash data with SHA-256', async () => {
    const crypto = new CryptoUtils({ verbose: false });

    const data = 'Hash this message';
    const hash = await crypto.hash(data);

    expect(hash).toBeInstanceOf(Uint8Array);
    expect(hash.length).toBe(32); // SHA-256 = 32 bytes

    // Same input -> same hash (deterministic)
    const hash2 = await crypto.hash(data);
    expect(hash).toEqual(hash2);

    // Different input -> different hash
    const hash3 = await crypto.hash(data + '!');
    expect(hash).not.toEqual(hash3);
  });

  test('should compute HMAC', async () => {
    const crypto = new CryptoUtils({ verbose: false });

    const data = 'Message to authenticate';
    const keyBytes = crypto.hexToBytes('fedcba9876543210'.repeat(4));

    const mac = await crypto.hmac(data, keyBytes);

    expect(mac).toBeInstanceOf(Uint8Array);
    expect(mac.length).toBe(32); // HMAC-SHA256 = 32 bytes
  });

  test('should derive key from password (PBKDF2)', async () => {
    const crypto = new CryptoUtils({ verbose: false });

    const password = 'my-secure-password';
    const salt = crypto.generateIV(); // Random salt

    const derivedKey = await crypto.deriveKey(password, salt, 1000); // Low iterations for test speed

    expect(derivedKey).toBeInstanceOf(CryptoKey);
    expect(derivedKey.algorithm.name).toBe('AES-GCM');

    // Can use derived key for encryption
    const { iv, ciphertext } = await crypto.encrypt('test', derivedKey);
    const decrypted = await crypto.decrypt(ciphertext, derivedKey, iv);
    expect(decrypted).toBe('test');
  });

  test('should convert between hex and bytes', () => {
    const crypto = new CryptoUtils({ verbose: false });

    const original = new Uint8Array([0x01, 0x23, 0x45, 0x67, 0x89, 0xab, 0xcd, 0xef]);
    const hex = crypto.bytesToHex(original);

    expect(hex).toBe('0123456789abcdef');

    const bytes = crypto.hexToBytes(hex);
    expect(bytes).toEqual(original);
  });

  test('should convert between base64 and bytes', () => {
    const crypto = new CryptoUtils({ verbose: false });

    const original = new Uint8Array([72, 101, 108, 108, 111]); // "Hello"
    const base64 = crypto.bytesToBase64(original);

    expect(typeof base64).toBe('string');

    const bytes = crypto.base64ToBytes(base64);
    expect(bytes).toEqual(original);
  });

  test('should track statistics', async () => {
    const crypto = new CryptoUtils({ verbose: false });

    const key = await crypto.generateKey();
    await crypto.encrypt('test1', key);
    await crypto.encrypt('test2', key);
    await crypto.hash('data');

    const stats = crypto.getStats();

    expect(stats.encryptions).toBe(2);
    expect(stats.keyGenerations).toBe(1);
    expect(stats.hashes).toBe(1);
  });

  test('should reset statistics', async () => {
    const crypto = new CryptoUtils({ verbose: false });

    await crypto.hash('data');
    crypto.resetStats();

    const stats = crypto.getStats();
    expect(stats.hashes).toBe(0);
  });

  test('should fail decryption with wrong key', async () => {
    const crypto = new CryptoUtils({ verbose: false });

    const plaintext = 'Secret message';
    const correctKey = await crypto.generateKey();
    const wrongKey = await crypto.generateKey();

    const { iv, ciphertext } = await crypto.encrypt(plaintext, correctKey);

    // Try to decrypt with wrong key (should fail)
    await expect(async () => {
      await crypto.decrypt(ciphertext, wrongKey, iv);
    }).rejects.toThrow();
  });

  test('should fail decryption with tampered ciphertext', async () => {
    const crypto = new CryptoUtils({ verbose: false });

    const plaintext = 'Secret message';
    const key = await crypto.generateKey();

    const { iv, ciphertext } = await crypto.encrypt(plaintext, key);

    // Tamper with ciphertext
    ciphertext[0] ^= 0xFF;

    // Decryption should fail (authenticated encryption)
    await expect(async () => {
      await crypto.decrypt(ciphertext, key, iv);
    }).rejects.toThrow();
  });
});
