//! Cryptographic operations for keystore
//!
//! Uses master secret + HMAC-SHA256 to derive repo-specific keypairs.
//! Each repository (with owner) gets a unique keypair derived from the same master secret.
//! All operations are designed to be TEE-safe (no key material leaves secure enclave).

use anyhow::{Context, Result};
use ed25519_dalek::{SigningKey, VerifyingKey, Signature, Signer, Verifier};
use hmac::{Hmac, Mac};
use rand::{rngs::OsRng, RngCore};
use sha2::{Sha256, Digest};
use std::collections::HashMap;
use std::sync::RwLock;

type HmacSha256 = Hmac<Sha256>;

/// Maximum size for encrypted data (10 MB)
const MAX_ENCRYPTED_SIZE: usize = 10 * 1024 * 1024;

/// Keystore holds the master secret and caches derived keypairs
#[derive(Debug, Clone)]
pub struct Keystore {
    /// Master secret (32 bytes, NEVER leaves TEE memory)
    master_secret: [u8; 32],

    /// Cache of derived keypairs (seed -> keypair)
    /// Cached to avoid recomputing HMAC for every request
    /// Wrapped in Arc for cheap cloning
    keypair_cache: std::sync::Arc<RwLock<HashMap<String, (SigningKey, VerifyingKey)>>>,
}

impl Keystore {
    /// Generate a new keystore with random master secret
    ///
    /// In production TEE:
    /// - Master secret is generated using TEE hardware RNG
    /// - Sealed to TEE persistent storage
    /// - Only accessible within the same TEE enclave
    pub fn generate() -> Self {
        let mut master_secret = [0u8; 32];
        OsRng.fill_bytes(&mut master_secret);

        tracing::info!("Generated new keystore with random master secret");

        Self {
            master_secret,
            keypair_cache: std::sync::Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Load keystore from existing master secret (hex encoded)
    pub fn from_master_secret_hex(master_secret_hex: &str) -> Result<Self> {
        let bytes = hex::decode(master_secret_hex)
            .context("Invalid hex encoding for master secret")?;

        if bytes.len() != 32 {
            anyhow::bail!("Master secret must be 32 bytes, got {}", bytes.len());
        }

        let mut master_secret = [0u8; 32];
        master_secret.copy_from_slice(&bytes);

        tracing::info!("Loaded keystore from existing master secret");

        Ok(Self {
            master_secret,
            keypair_cache: std::sync::Arc::new(RwLock::new(HashMap::new())),
        })
    }

    /// Export master secret as hex (for backup/persistence)
    /// WARNING: This should ONLY be used for initial setup or backup
    pub fn master_secret_hex(&self) -> String {
        hex::encode(&self.master_secret)
    }

    /// Derive a keypair for a specific seed (repo + owner + optional branch)
    ///
    /// Seed format examples:
    /// - "github.com/alice/project:alice.near" (all branches)
    /// - "github.com/alice/project:alice.near:main" (specific branch)
    ///
    /// Uses HMAC-SHA256(master_secret, seed) to derive deterministic keypair.
    /// Different seeds produce different keypairs, but same seed always produces same keypair.
    pub fn derive_keypair(&self, seed: &str) -> Result<(SigningKey, VerifyingKey)> {
        // Check cache first
        {
            let cache = self.keypair_cache.read().unwrap();
            if let Some(keypair) = cache.get(seed) {
                return Ok(keypair.clone());
            }
        }

        // Derive keypair using HMAC-SHA256
        let mut mac = HmacSha256::new_from_slice(&self.master_secret)
            .expect("HMAC can take key of any size");
        mac.update(seed.as_bytes());
        let derived_bytes = mac.finalize().into_bytes();

        // Use first 32 bytes as Ed25519 secret key
        let mut secret_bytes = [0u8; 32];
        secret_bytes.copy_from_slice(&derived_bytes[..32]);

        let signing_key = SigningKey::from_bytes(&secret_bytes);
        let verifying_key = signing_key.verifying_key();

        tracing::debug!(
            "Derived keypair for seed='{}', pubkey={}",
            seed,
            hex::encode(verifying_key.as_bytes())
        );

        // Cache the result
        {
            let mut cache = self.keypair_cache.write().unwrap();
            cache.insert(seed.to_string(), (signing_key.clone(), verifying_key));
        }

        Ok((signing_key, verifying_key))
    }

    /// Get public key for a specific seed
    pub fn get_public_key_for_seed(&self, seed: &str) -> Result<VerifyingKey> {
        let (_signing_key, verifying_key) = self.derive_keypair(seed)?;
        Ok(verifying_key)
    }

    /// Get public key as hex string for a specific seed
    pub fn public_key_hex(&self, seed: &str) -> Result<String> {
        let verifying_key = self.get_public_key_for_seed(seed)?;
        Ok(hex::encode(verifying_key.as_bytes()))
    }

    /// Get public key as base58 string (NEAR format) for a specific seed
    pub fn public_key_base58(&self, seed: &str) -> Result<String> {
        let verifying_key = self.get_public_key_for_seed(seed)?;
        Ok(bs58::encode(verifying_key.as_bytes()).into_string())
    }

    /// Decrypt data that was encrypted for a specific seed
    ///
    /// This uses a simplified encryption scheme for MVP:
    /// - XOR with key derived from public key
    ///
    /// For production, use X25519-ECDH + ChaCha20-Poly1305.
    pub fn decrypt(&self, seed: &str, encrypted_data: &[u8]) -> Result<Vec<u8>> {
        if encrypted_data.len() > MAX_ENCRYPTED_SIZE {
            anyhow::bail!("Encrypted data too large: {} bytes", encrypted_data.len());
        }

        let (_signing_key, verifying_key) = self.derive_keypair(seed)?;

        // Derive symmetric key from PUBLIC key (same as encryption)
        let key_material = verifying_key.to_bytes();
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

    /// Sign a message with the private key for a specific seed
    pub fn sign(&self, seed: &str, message: &[u8]) -> Result<Signature> {
        let (signing_key, _) = self.derive_keypair(seed)?;
        Ok(signing_key.sign(message))
    }

    /// Verify a signature for a specific seed
    pub fn verify(&self, seed: &str, message: &[u8], signature: &Signature) -> Result<()> {
        let (_, verifying_key) = self.derive_keypair(seed)?;
        verifying_key
            .verify(message, signature)
            .context("Signature verification failed")
    }

    /// Clear the keypair cache (for testing or memory management)
    pub fn clear_cache(&self) {
        let mut cache = self.keypair_cache.write().unwrap();
        cache.clear();
        tracing::debug!("Cleared keypair cache");
    }
}

/// Encrypt data for the keystore with a specific public key (used by clients)
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
        let pubkey = keystore.public_key_hex("test-seed").unwrap();
        assert_eq!(pubkey.len(), 64); // 32 bytes = 64 hex chars
    }

    #[test]
    fn test_deterministic_derivation() {
        let keystore = Keystore::generate();

        let pubkey1 = keystore.public_key_hex("github.com/alice/project:alice.near").unwrap();
        let pubkey2 = keystore.public_key_hex("github.com/alice/project:alice.near").unwrap();

        assert_eq!(pubkey1, pubkey2, "Same seed should produce same key");
    }

    #[test]
    fn test_different_seeds_different_keys() {
        let keystore = Keystore::generate();

        let pubkey_alice = keystore.public_key_hex("github.com/alice/project:alice.near").unwrap();
        let pubkey_bob = keystore.public_key_hex("github.com/alice/project:bob.near").unwrap();

        assert_ne!(pubkey_alice, pubkey_bob, "Different seeds should produce different keys");
    }

    #[test]
    fn test_encrypt_decrypt() {
        let keystore = Keystore::generate();
        let seed = "github.com/alice/project:alice.near";
        let plaintext = b"my secret API key: sk-1234567890";

        let pubkey_hex = keystore.public_key_hex(seed).unwrap();
        let encrypted = encrypt_for_keystore(&pubkey_hex, plaintext).unwrap();
        let decrypted = keystore.decrypt(seed, &encrypted).unwrap();

        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn test_sign_verify() {
        let keystore = Keystore::generate();
        let seed = "github.com/alice/project:alice.near";
        let message = b"hello world";

        let signature = keystore.sign(seed, message).unwrap();
        keystore.verify(seed, message, &signature).unwrap();
    }

    #[test]
    fn test_master_secret_persistence() {
        let keystore1 = Keystore::generate();
        let seed = "github.com/test/repo:test.near";
        let pubkey1 = keystore1.public_key_hex(seed).unwrap();

        // Serialize master secret (in production, this would be sealed storage)
        let master_secret_hex = hex::encode(&keystore1.master_secret);

        // Load from same master secret
        let keystore2 = Keystore::from_master_secret_hex(&master_secret_hex).unwrap();
        let pubkey2 = keystore2.public_key_hex(seed).unwrap();

        assert_eq!(pubkey1, pubkey2, "Same master secret should produce same derived keys");
    }
}
