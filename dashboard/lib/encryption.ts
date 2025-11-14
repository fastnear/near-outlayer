/**
 * Client-side encryption utilities for encrypted secrets
 *
 * NOTE: This uses simple XOR encryption (MVP).
 * For production, should be replaced with proper hybrid encryption:
 * - X25519 ECDH for key exchange
 * - ChaCha20-Poly1305 for authenticated encryption
 */

/**
 * Encrypt plaintext secrets using keystore's public key
 *
 * @param pubkeyHex - Hex-encoded public key from keystore
 * @param plaintext - JSON string with secrets (e.g. '{"KEY":"value"}')
 * @returns Array of encrypted bytes
 */
export async function encryptSecrets(pubkeyHex: string, plaintext: string): Promise<number[]> {
  // Validate JSON format
  try {
    const parsed = JSON.parse(plaintext);
    if (typeof parsed !== 'object' || Array.isArray(parsed)) {
      throw new Error('Secrets must be a JSON object');
    }
  } catch {
    throw new Error('Invalid JSON format for secrets');
  }

  // Convert public key from hex to bytes
  const keyMaterial = hexToBytes(pubkeyHex);

  // Derive symmetric key (same as keystore does)
  const keyData = new Uint8Array([...keyMaterial, ...stringToBytes('keystore-encryption-v1')]);
  const derivedKeyBuffer = await crypto.subtle.digest('SHA-256', keyData);
  const derivedKey = new Uint8Array(derivedKeyBuffer);

  // XOR encryption
  const plaintextBytes = stringToBytes(plaintext);
  const ciphertext = new Uint8Array(plaintextBytes.length);

  for (let i = 0; i < plaintextBytes.length; i++) {
    ciphertext[i] = plaintextBytes[i] ^ derivedKey[i % derivedKey.length];
  }

  return Array.from(ciphertext);
}

/**
 * Convert hex string to byte array
 */
function hexToBytes(hex: string): Uint8Array {
  const bytes = new Uint8Array(hex.length / 2);
  for (let i = 0; i < hex.length; i += 2) {
    bytes[i / 2] = parseInt(hex.substr(i, 2), 16);
  }
  return bytes;
}

/**
 * Convert string to UTF-8 byte array
 */
function stringToBytes(str: string): Uint8Array {
  return new TextEncoder().encode(str);
}
