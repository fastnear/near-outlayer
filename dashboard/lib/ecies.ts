import { ChaCha20Poly1305 } from '@stablelib/chacha20poly1305';
import { randomBytes } from '@stablelib/random';
import { generateKeyPair, sharedKey } from '@stablelib/x25519';
import { HKDF } from '@stablelib/hkdf';
import { SHA256 } from '@stablelib/sha256';

const HKDF_INFO = new TextEncoder().encode('outlayer-keystore-v1');
const ECIES_VERSION = 0x01;

function hexToBytes(hex: string): Uint8Array {
  const bytes = new Uint8Array(hex.length / 2);
  for (let i = 0; i < hex.length; i += 2) {
    bytes[i / 2] = parseInt(hex.substring(i, i + 2), 16);
  }
  return bytes;
}

/**
 * ECIES encrypt: X25519 ECDH + HKDF-SHA256 + ChaCha20-Poly1305
 *
 * Format: [0x01 | ephemeral_x25519_pubkey(32) | nonce(12) | ciphertext | tag(16)]
 */
export function eciesEncrypt(pubkeyHex: string, plaintext: Uint8Array): Uint8Array {
  const recipientPub = hexToBytes(pubkeyHex);
  if (recipientPub.length !== 32) {
    throw new Error(`Invalid public key length: ${recipientPub.length}, expected 32`);
  }
  const ephemeral = generateKeyPair();
  const shared = sharedKey(ephemeral.secretKey, recipientPub);
  const derived = new HKDF(SHA256, shared, undefined, HKDF_INFO).expand(32);
  const cipher = new ChaCha20Poly1305(derived);
  const nonce = randomBytes(12);
  const ciphertextWithTag = cipher.seal(nonce, plaintext);
  const result = new Uint8Array(1 + 32 + 12 + ciphertextWithTag.length);
  result[0] = ECIES_VERSION;
  result.set(ephemeral.publicKey, 1);
  result.set(nonce, 33);
  result.set(ciphertextWithTag, 45);
  return result;
}
