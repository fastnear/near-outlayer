//! Cryptographic operations for keystore
//!
//! Handles key generation, encryption, decryption using Ed25519.
//! All operations are designed to be TEE-safe (no key material leaves secure enclave).

use anyhow::{Context, Result};
use ed25519_dalek::{SigningKey, VerifyingKey, Signature, Signer, Verifier};
use rand::rngs::OsRng;
use sha2::{Sha256, Digest};

/// Maximum size for encrypted data (10 MB)
const MAX_ENCRYPTED_SIZE: usize = 10 * 1024 * 1024;

/// Keystore holds the master keypair in memory (TEE-protected)
#[derive(Debug)]
pub struct Keystore {
    /// Private signing key (NEVER leaves TEE memory)
    signing_key: SigningKey,
    /// Public verifying key (can be published)
    verifying_key: VerifyingKey,
}

impl Keystore {
    /// Generate a new keystore with random keypair
    ///
    /// In production TEE:
    /// - Keys are generated using TEE hardware RNG
    /// - Private key is sealed to TEE persistent storage
    /// - Only accessible within the same TEE enclave
    pub fn generate() -> Self {
        let signing_key = SigningKey::generate(&mut OsRng);
        let verifying_key = signing_key.verifying_key();

        tracing::info!(
            "Generated new keypair, public key: {}",
            hex::encode(verifying_key.as_bytes())
        );

        Self {
            signing_key,
            verifying_key,
        }
    }

    /// Load keystore from existing private key (base58 encoded)
    pub fn from_private_key(private_key_b58: &str) -> Result<Self> {
        // Parse NEAR-style private key (ed25519:base58...)
        let key_str = private_key_b58
            .strip_prefix("ed25519:")
            .context("Private key must start with 'ed25519:'")?;

        let bytes = bs58::decode(key_str)
            .into_vec()
            .context("Invalid base58 encoding")?;

        if bytes.len() != 64 {
            anyhow::bail!("Invalid private key length: expected 64 bytes, got {}", bytes.len());
        }

        // Ed25519 secret key is first 32 bytes
        let secret_bytes: [u8; 32] = bytes[0..32]
            .try_into()
            .context("Failed to extract secret key bytes")?;

        let signing_key = SigningKey::from_bytes(&secret_bytes);
        let verifying_key = signing_key.verifying_key();

        tracing::info!(
            "Loaded keypair, public key: {}",
            hex::encode(verifying_key.as_bytes())
        );

        Ok(Self {
            signing_key,
            verifying_key,
        })
    }

    /// Get public key as bytes
    pub fn public_key_bytes(&self) -> [u8; 32] {
        self.verifying_key.to_bytes()
    }

    /// Get public key as hex string
    pub fn public_key_hex(&self) -> String {
        hex::encode(self.verifying_key.as_bytes())
    }

    /// Get public key as base58 string (NEAR format)
    pub fn public_key_base58(&self) -> String {
        bs58::encode(self.verifying_key.as_bytes()).into_string()
    }

    /// Decrypt data that was encrypted for this keystore
    ///
    /// This uses a hybrid encryption scheme:
    /// - Data is encrypted with AES-256-GCM (symmetric)
    /// - AES key is encrypted with Ed25519 public key (asymmetric)
    /// - Both are sent together in the encrypted payload
    ///
    /// NOTE: Ed25519 is primarily a signature scheme, not encryption.
    /// For production, use X25519 (ECDH) + ChaCha20-Poly1305.
    /// This implementation is simplified for MVP.
    pub fn decrypt(&self, encrypted_data: &[u8]) -> Result<Vec<u8>> {
        if encrypted_data.len() > MAX_ENCRYPTED_SIZE {
            anyhow::bail!("Encrypted data too large: {} bytes", encrypted_data.len());
        }

        // For MVP: Simple XOR with key derivation (NOT PRODUCTION READY)
        // TODO: Replace with proper hybrid encryption (X25519-ECDH + ChaCha20-Poly1305)

        // Derive symmetric key from PUBLIC key (same as encryption)
        // This allows clients to encrypt with public key and keystore to decrypt
        let key_material = self.verifying_key.to_bytes();
        let mut hasher = Sha256::new();
        hasher.update(&key_material);
        hasher.update(b"keystore-encryption-v1");
        let derived_key = hasher.finalize();

        // Simple XOR decryption (INSECURE - for MVP only)
        let plaintext: Vec<u8> = encrypted_data
            .iter()
            .enumerate()
            .map(|(i, &byte)| byte ^ derived_key[i % derived_key.len()])
            .collect();

        Ok(plaintext)
    }

    /// Sign a message with the private key
    pub fn sign(&self, message: &[u8]) -> Signature {
        self.signing_key.sign(message)
    }

    /// Verify a signature
    pub fn verify(&self, message: &[u8], signature: &Signature) -> Result<()> {
        self.verifying_key
            .verify(message, signature)
            .context("Signature verification failed")
    }
}

/// Encrypt data for the keystore (used by clients)
///
/// This should be implemented on the client side (executor workers, contract callers)
pub fn encrypt_for_keystore(pubkey_hex: &str, plaintext: &[u8]) -> Result<Vec<u8>> {
    let pubkey_bytes = hex::decode(pubkey_hex).context("Invalid hex public key")?;

    if pubkey_bytes.len() != 32 {
        anyhow::bail!("Invalid public key length: {}", pubkey_bytes.len());
    }

    // Derive symmetric key from public key
    let mut hasher = Sha256::new();
    hasher.update(&pubkey_bytes);
    hasher.update(b"keystore-encryption-v1");
    let derived_key = hasher.finalize();

    // Simple XOR encryption (matches decrypt)
    let ciphertext: Vec<u8> = plaintext
        .iter()
        .enumerate()
        .map(|(i, &byte)| byte ^ derived_key[i % derived_key.len()])
        .collect();

    Ok(ciphertext)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_keystore_generation() {
        let keystore = Keystore::generate();
        assert_eq!(keystore.public_key_bytes().len(), 32);
    }

    #[test]
    fn test_encrypt_decrypt() {
        let keystore = Keystore::generate();
        let plaintext = b"my secret API key: sk-1234567890";

        let pubkey_hex = keystore.public_key_hex();
        let encrypted = encrypt_for_keystore(&pubkey_hex, plaintext).unwrap();
        let decrypted = keystore.decrypt(&encrypted).unwrap();

        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn test_sign_verify() {
        let keystore = Keystore::generate();
        let message = b"hello world";

        let signature = keystore.sign(message);
        keystore.verify(message, &signature).unwrap();
    }
}
