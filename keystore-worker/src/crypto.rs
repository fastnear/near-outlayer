//! Cryptographic operations for keystore
//!
//! Uses master secret + HMAC-SHA256 to derive repo-specific keypairs.
//! Each repository (with owner) gets a unique keypair derived from the same master secret.
//! All operations are designed to be TEE-safe (no key material leaves secure enclave).
//!
//! Encryption: ChaCha20-Poly1305 AEAD (RFC 7539)
//! - Symmetric encryption with authentication
//! - 12-byte random nonce per encryption
//! - Format: [nonce (12 bytes) | ciphertext | auth_tag (16 bytes)]

use anyhow::{Context, Result};
use chacha20poly1305::{
    aead::{Aead, AeadCore, KeyInit, OsRng as AeadOsRng},
    ChaCha20Poly1305, Nonce,
};
use ed25519_dalek::{SigningKey, VerifyingKey, Signature, Signer, Verifier};
use hmac::{Hmac, Mac};
use rand::{rngs::OsRng, RngCore};
use sha2::{Digest, Sha256};
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

        Self::from_master_secret(&master_secret)
    }

    /// Create keystore from master secret bytes
    pub fn from_master_secret(master_secret: &[u8; 32]) -> Result<Self> {
        // Log hash of master secret for debugging (if enabled)
        if std::env::var("LOG_MASTER_KEY_HASH").unwrap_or_else(|_| "false".to_string()) == "true" {
            let mut hasher = Sha256::new();
            hasher.update(master_secret);
            let hash = hasher.finalize();
            tracing::warn!("🔑 MASTER KEY HASH (SHA256): {}", hex::encode(hash));
            tracing::warn!("   This is for debugging only! Remove LOG_MASTER_KEY_HASH in production!");
        }

        tracing::info!("Loaded keystore from master secret");

        Ok(Self {
            master_secret: *master_secret,
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
        let mut mac = <HmacSha256 as Mac>::new_from_slice(&self.master_secret)
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
    #[allow(dead_code)]
    pub fn public_key_base58(&self, seed: &str) -> Result<String> {
        let verifying_key = self.get_public_key_for_seed(seed)?;
        Ok(bs58::encode(verifying_key.as_bytes()).into_string())
    }

    /// Decrypt data that was encrypted for a specific seed
    ///
    /// Uses ChaCha20-Poly1305 AEAD encryption.
    /// Format: [nonce (12 bytes) | ciphertext | auth_tag (16 bytes)]
    pub fn decrypt(&self, seed: &str, encrypted_data: &[u8]) -> Result<Vec<u8>> {
        if encrypted_data.len() > MAX_ENCRYPTED_SIZE {
            anyhow::bail!("Encrypted data too large: {} bytes", encrypted_data.len());
        }

        // Minimum size: 12 (nonce) + 16 (tag) = 28 bytes
        if encrypted_data.len() < 28 {
            anyhow::bail!(
                "Encrypted data too short: {} bytes (minimum 28)",
                encrypted_data.len()
            );
        }

        let (_signing_key, verifying_key) = self.derive_keypair(seed)?;

        // Use Ed25519 public key as ChaCha20 key (32 bytes)
        let key_bytes = verifying_key.to_bytes();
        let cipher = ChaCha20Poly1305::new((&key_bytes).into());

        // Extract nonce (first 12 bytes)
        let nonce = Nonce::from_slice(&encrypted_data[0..12]);

        // Extract ciphertext + tag (remaining bytes)
        let ciphertext_with_tag = &encrypted_data[12..];

        // Decrypt and verify auth tag
        let plaintext = cipher
            .decrypt(nonce, ciphertext_with_tag)
            .map_err(|e| anyhow::anyhow!("Decryption failed (data tampered or wrong key): {}", e))?;

        Ok(plaintext)
    }

    /// Encrypt plaintext data for a specific seed (server-side encryption)
    ///
    /// Uses ChaCha20-Poly1305 AEAD with random nonce.
    /// Returns: [nonce (12 bytes) | ciphertext | auth_tag (16 bytes)]
    pub fn encrypt(&self, seed: &str, plaintext: &[u8]) -> Result<Vec<u8>> {
        if plaintext.len() > MAX_ENCRYPTED_SIZE {
            anyhow::bail!("Plaintext too large: {} bytes", plaintext.len());
        }

        let (_signing_key, verifying_key) = self.derive_keypair(seed)?;

        // Use Ed25519 public key as ChaCha20 key
        let key_bytes = verifying_key.to_bytes();
        let cipher = ChaCha20Poly1305::new((&key_bytes).into());

        // Generate random 12-byte nonce
        let nonce = ChaCha20Poly1305::generate_nonce(&mut AeadOsRng);

        // Encrypt and append auth tag
        let ciphertext_with_tag = cipher
            .encrypt(&nonce, plaintext)
            .map_err(|e| anyhow::anyhow!("Encryption failed: {}", e))?;

        // Combine: [nonce | ciphertext | tag]
        let mut result = Vec::with_capacity(12 + ciphertext_with_tag.len());
        result.extend_from_slice(&nonce);
        result.extend_from_slice(&ciphertext_with_tag);

        Ok(result)
    }

    /// Sign a message with the private key for a specific seed
    #[allow(dead_code)]
    pub fn sign(&self, seed: &str, message: &[u8]) -> Result<Signature> {
        let (signing_key, _) = self.derive_keypair(seed)?;
        Ok(signing_key.sign(message))
    }

    /// Verify a signature for a specific seed
    #[allow(dead_code)]
    pub fn verify(&self, seed: &str, message: &[u8], signature: &Signature) -> Result<()> {
        let (_, verifying_key) = self.derive_keypair(seed)?;
        verifying_key
            .verify(message, signature)
            .context("Signature verification failed")
    }

    /// Generate VRF output and proof for the given alpha bytes.
    ///
    /// Uses Ed25519 deterministic signature (RFC 8032) as VRF:
    /// - Proof = Ed25519 signature of alpha (deterministic: same key + same alpha = same signature)
    /// - Output = SHA256(signature) (random bytes derived from proof)
    ///
    /// Verification: `ed25519_verify(vrf_pubkey, alpha, signature)` — works on-chain in NEAR contracts.
    /// The VRF key is derived from master_secret with fixed seed "vrf-key".
    ///
    /// Returns (output_hex, signature_hex).
    pub fn vrf_generate(&self, alpha: &[u8]) -> Result<(String, String)> {
        let (signing_key, _) = self.derive_keypair("vrf-key")?;
        let signature = signing_key.sign(alpha);

        let mut hasher = Sha256::new();
        hasher.update(signature.to_bytes());
        let output = hasher.finalize();

        Ok((hex::encode(output), hex::encode(signature.to_bytes())))
    }

    /// Get the VRF public key as hex string.
    ///
    /// This key should be registered on-chain for verification.
    /// All keystores with the same master_secret produce the same VRF pubkey.
    pub fn vrf_public_key_hex(&self) -> Result<String> {
        let (_, verifying_key) = self.derive_keypair("vrf-key")?;
        Ok(hex::encode(verifying_key.as_bytes()))
    }

    /// Clear the keypair cache (for testing or memory management)
    #[allow(dead_code)]
    pub fn clear_cache(&self) {
        let mut cache = self.keypair_cache.write().unwrap();
        cache.clear();
        tracing::debug!("Cleared keypair cache");
    }
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

        // Use keystore.encrypt (ChaCha20) instead of encrypt_for_keystore (XOR)
        let encrypted = keystore.encrypt(seed, plaintext).unwrap();
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
    fn test_vrf_deterministic() {
        let keystore = Keystore::generate();
        let alpha = b"vrf:42:my-seed";

        let (output1, sig1) = keystore.vrf_generate(alpha).unwrap();
        let (output2, sig2) = keystore.vrf_generate(alpha).unwrap();

        assert_eq!(output1, output2, "Same alpha must produce same output");
        assert_eq!(sig1, sig2, "Same alpha must produce same signature");
    }

    #[test]
    fn test_vrf_different_alpha_different_output() {
        let keystore = Keystore::generate();

        let (out_a, _) = keystore.vrf_generate(b"vrf:1:seed-a").unwrap();
        let (out_b, _) = keystore.vrf_generate(b"vrf:1:seed-b").unwrap();

        assert_ne!(out_a, out_b, "Different alphas must produce different outputs");
    }

    #[test]
    fn test_vrf_self_verify() {
        let keystore = Keystore::generate();
        let alpha = b"vrf:100:test";

        let (_, sig_hex) = keystore.vrf_generate(alpha).unwrap();

        // Reconstruct signature and verify with public key
        let sig_bytes: [u8; 64] = hex::decode(&sig_hex).unwrap().try_into().unwrap();
        let signature = Signature::from_bytes(&sig_bytes);
        let vrf_pubkey = keystore.get_public_key_for_seed("vrf-key").unwrap();

        vrf_pubkey.verify(alpha, &signature).expect("VRF signature must verify");
    }

    #[test]
    fn test_vrf_pubkey_stable() {
        let keystore = Keystore::generate();

        let pk1 = keystore.vrf_public_key_hex().unwrap();
        let pk2 = keystore.vrf_public_key_hex().unwrap();

        assert_eq!(pk1, pk2);
        assert_eq!(pk1.len(), 64); // 32-byte Ed25519 pubkey = 64 hex chars
    }

    #[test]
    fn test_vrf_same_master_secret_same_output() {
        let ks1 = Keystore::generate();
        let master_hex = ks1.master_secret_hex();
        let ks2 = Keystore::from_master_secret_hex(&master_hex).unwrap();

        let alpha = b"vrf:1:test";
        let (out1, _) = ks1.vrf_generate(alpha).unwrap();
        let (out2, _) = ks2.vrf_generate(alpha).unwrap();

        assert_eq!(out1, out2, "Same master secret must produce same VRF output");
        assert_eq!(ks1.vrf_public_key_hex().unwrap(), ks2.vrf_public_key_hex().unwrap());
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
