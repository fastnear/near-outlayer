//! Cryptographic operations for keystore
//!
//! Uses master secret + HMAC-SHA256 to derive repo-specific keypairs.
//! Each repository (with owner) gets a unique keypair derived from the same master secret.
//! All operations are designed to be TEE-safe (no key material leaves secure enclave).
//!
//! Encryption: ECIES with X25519 ECDH + HKDF-SHA256 + ChaCha20-Poly1305
//! - Asymmetric: encrypt with public key, only TEE can decrypt with private key
//! - Format v1: [0x01 | ephemeral_x25519_pubkey (32) | nonce (12) | ciphertext | auth_tag (16)]
//! - Legacy format: [nonce (12) | ciphertext | auth_tag (16)] (symmetric, being phased out)

use anyhow::{Context, Result};
use k256::elliptic_curve::sec1::ToEncodedPoint;
use chacha20poly1305::{
    aead::{Aead, AeadCore, KeyInit, OsRng as AeadOsRng},
    ChaCha20Poly1305, Nonce,
};
use ed25519_dalek::{SigningKey, VerifyingKey, Signature, Signer, Verifier};
use hmac::{Hmac, Mac};
use hkdf::Hkdf;
use rand::{rngs::OsRng, RngCore};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::sync::RwLock;
use x25519_dalek::{EphemeralSecret, PublicKey as X25519PublicKey, StaticSecret};

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

    /// Get Ed25519 public key for a specific seed (used for signing/verification/VRF)
    pub fn get_public_key_for_seed(&self, seed: &str) -> Result<VerifyingKey> {
        let (_signing_key, verifying_key) = self.derive_keypair(seed)?;
        Ok(verifying_key)
    }

    /// Derive X25519 keypair from seed (for ECIES encryption/decryption)
    ///
    /// Uses domain-separated HMAC: HMAC-SHA256(master_secret, "ecies:" || seed)
    /// so that encryption keys are independent from Ed25519 signing keys.
    /// The X25519 public key is safe to expose — it cannot decrypt without the private key.
    fn derive_x25519_keypair(&self, seed: &str) -> Result<(StaticSecret, X25519PublicKey)> {
        let mut mac = <HmacSha256 as Mac>::new_from_slice(&self.master_secret)
            .expect("HMAC can take key of any size");
        mac.update(b"ecies:");
        mac.update(seed.as_bytes());
        let derived_bytes = mac.finalize().into_bytes();

        let mut secret_bytes = [0u8; 32];
        secret_bytes.copy_from_slice(&derived_bytes[..32]);

        let static_secret = StaticSecret::from(secret_bytes);
        let public_key = X25519PublicKey::from(&static_secret);
        Ok((static_secret, public_key))
    }

    /// Get X25519 public key as hex string for a specific seed (for encryption)
    ///
    /// This is the key returned by `/pubkey` endpoint. Safe to expose publicly —
    /// it can only be used to encrypt, not decrypt. Only the TEE holds the private key.
    pub fn public_key_hex(&self, seed: &str) -> Result<String> {
        let (_, x25519_pub) = self.derive_x25519_keypair(seed)?;
        Ok(hex::encode(x25519_pub.as_bytes()))
    }

    /// Get Ed25519 public key as base58 string (NEAR format) for a specific seed
    #[allow(dead_code)]
    pub fn public_key_base58(&self, seed: &str) -> Result<String> {
        let verifying_key = self.get_public_key_for_seed(seed)?;
        Ok(bs58::encode(verifying_key.as_bytes()).into_string())
    }

    /// HKDF info string — must be identical across all implementations (Rust, TypeScript)
    const HKDF_INFO: &'static [u8] = b"outlayer-keystore-v1";

    /// Version byte for ECIES format
    const ECIES_VERSION: u8 = 0x01;

    /// Decrypt data that was encrypted for a specific seed
    ///
    /// Supports both formats:
    /// - ECIES v1: [0x01 | ephemeral_x25519_pubkey (32) | nonce (12) | ciphertext | tag (16)]
    /// - Legacy:   [nonce (12) | ciphertext | tag (16)] (symmetric, pubkey as ChaCha20 key)
    pub fn decrypt(&self, seed: &str, encrypted_data: &[u8]) -> Result<Vec<u8>> {
        if encrypted_data.len() > MAX_ENCRYPTED_SIZE {
            anyhow::bail!("Encrypted data too large: {} bytes", encrypted_data.len());
        }

        // Minimum legacy size: 12 (nonce) + 16 (tag) = 28 bytes
        if encrypted_data.len() < 28 {
            anyhow::bail!(
                "Encrypted data too short: {} bytes (minimum 28)",
                encrypted_data.len()
            );
        }

        // Try ECIES v1 format: [0x01 | ephemeral_pub(32) | nonce(12) | ciphertext | tag(16)]
        // Minimum ECIES size: 1 + 32 + 12 + 16 = 61 bytes
        if encrypted_data[0] == Self::ECIES_VERSION && encrypted_data.len() >= 61 {
            match self.decrypt_ecies(seed, encrypted_data) {
                Ok(plaintext) => return Ok(plaintext),
                Err(e) => {
                    // AEAD failure could mean this is legacy data that happens to start with 0x01
                    tracing::debug!("ECIES decrypt failed, trying legacy format: {}", e);
                }
            }
        }

        // Legacy format: [nonce(12) | ciphertext | tag(16)]
        self.decrypt_legacy(seed, encrypted_data)
    }

    /// Decrypt ECIES v1 format
    fn decrypt_ecies(&self, seed: &str, encrypted_data: &[u8]) -> Result<Vec<u8>> {
        let ephemeral_pub_bytes: [u8; 32] = encrypted_data[1..33]
            .try_into()
            .context("Invalid ephemeral public key")?;
        let ephemeral_pub = X25519PublicKey::from(ephemeral_pub_bytes);

        let (x25519_secret, _) = self.derive_x25519_keypair(seed)?;
        let shared_secret = x25519_secret.diffie_hellman(&ephemeral_pub);

        let sym_key = Self::hkdf_derive_key(shared_secret.as_bytes())?;
        let cipher = ChaCha20Poly1305::new((&sym_key).into());

        let nonce = Nonce::from_slice(&encrypted_data[33..45]);
        let ciphertext_with_tag = &encrypted_data[45..];

        let plaintext = cipher
            .decrypt(nonce, ciphertext_with_tag)
            .map_err(|e| anyhow::anyhow!("ECIES decryption failed: {}", e))?;

        Ok(plaintext)
    }

    /// Decrypt legacy format (symmetric, Ed25519 pubkey as ChaCha20 key)
    /// TODO: Remove after migration of all secrets to ECIES format
    fn decrypt_legacy(&self, seed: &str, encrypted_data: &[u8]) -> Result<Vec<u8>> {
        let (_signing_key, verifying_key) = self.derive_keypair(seed)?;

        let key_bytes = verifying_key.to_bytes();
        let cipher = ChaCha20Poly1305::new((&key_bytes).into());

        let nonce = Nonce::from_slice(&encrypted_data[0..12]);
        let ciphertext_with_tag = &encrypted_data[12..];

        let plaintext = cipher
            .decrypt(nonce, ciphertext_with_tag)
            .map_err(|e| anyhow::anyhow!("Legacy decryption failed (data tampered or wrong key): {}", e))?;

        Ok(plaintext)
    }

    /// Encrypt plaintext data for a specific seed using ECIES
    ///
    /// Uses ephemeral X25519 keypair + ECDH + HKDF-SHA256 + ChaCha20-Poly1305.
    /// Returns: [0x01 | ephemeral_x25519_pubkey (32) | nonce (12) | ciphertext | auth_tag (16)]
    pub fn encrypt(&self, seed: &str, plaintext: &[u8]) -> Result<Vec<u8>> {
        if plaintext.len() > MAX_ENCRYPTED_SIZE {
            anyhow::bail!("Plaintext too large: {} bytes", plaintext.len());
        }

        let (_, recipient_pub) = self.derive_x25519_keypair(seed)?;

        // Generate ephemeral X25519 keypair
        let ephemeral_secret = EphemeralSecret::random_from_rng(OsRng);
        let ephemeral_pub = X25519PublicKey::from(&ephemeral_secret);

        // ECDH shared secret
        let shared_secret = ephemeral_secret.diffie_hellman(&recipient_pub);

        // Derive symmetric key via HKDF-SHA256
        let sym_key = Self::hkdf_derive_key(shared_secret.as_bytes())?;
        let cipher = ChaCha20Poly1305::new((&sym_key).into());

        // Generate random 12-byte nonce
        let nonce = ChaCha20Poly1305::generate_nonce(&mut AeadOsRng);

        // Encrypt
        let ciphertext_with_tag = cipher
            .encrypt(&nonce, plaintext)
            .map_err(|e| anyhow::anyhow!("Encryption failed: {}", e))?;

        // Format: [0x01 | ephemeral_pub(32) | nonce(12) | ciphertext | tag(16)]
        let mut result = Vec::with_capacity(1 + 32 + 12 + ciphertext_with_tag.len());
        result.push(Self::ECIES_VERSION);
        result.extend_from_slice(ephemeral_pub.as_bytes());
        result.extend_from_slice(&nonce);
        result.extend_from_slice(&ciphertext_with_tag);

        Ok(result)
    }

    /// Derive 32-byte symmetric key from ECDH shared secret using HKDF-SHA256
    fn hkdf_derive_key(shared_secret: &[u8]) -> Result<[u8; 32]> {
        let hkdf = Hkdf::<Sha256>::new(None, shared_secret);
        let mut key = [0u8; 32];
        hkdf.expand(Self::HKDF_INFO, &mut key)
            .map_err(|e| anyhow::anyhow!("HKDF expand failed: {}", e))?;
        Ok(key)
    }

    /// Sign a message with the Ed25519 private key for a specific seed
    #[allow(dead_code)]
    pub fn sign(&self, seed: &str, message: &[u8]) -> Result<Signature> {
        let (signing_key, _) = self.derive_keypair(seed)?;
        Ok(signing_key.sign(message))
    }

    /// Verify an Ed25519 signature for a specific seed
    #[allow(dead_code)]
    pub fn verify(&self, seed: &str, message: &[u8], signature: &Signature) -> Result<()> {
        let (_, verifying_key) = self.derive_keypair(seed)?;
        verifying_key
            .verify(message, signature)
            .context("Signature verification failed")
    }

    // =========================================================================
    // secp256k1 (Ethereum/Base/EVM chains)
    // See: docs/MULTI_CHAIN.md for integration guide
    // =========================================================================

    /// Derive a secp256k1 keypair from seed (for EVM chains).
    ///
    /// Uses the same HMAC-SHA256(master_secret, seed) derivation as Ed25519,
    /// but interprets the 32 bytes as a secp256k1 scalar.
    pub fn derive_secp256k1_keypair(
        &self,
        seed: &str,
    ) -> Result<(k256::ecdsa::SigningKey, k256::elliptic_curve::PublicKey<k256::Secp256k1>)> {
        let mut mac = <HmacSha256 as Mac>::new_from_slice(&self.master_secret)
            .expect("HMAC can take key of any size");
        mac.update(seed.as_bytes());
        let derived_bytes = mac.finalize().into_bytes();

        let mut secret_bytes = [0u8; 32];
        secret_bytes.copy_from_slice(&derived_bytes[..32]);
        let signing_key = k256::ecdsa::SigningKey::from_slice(&secret_bytes)
            .context("Derived bytes are not a valid secp256k1 scalar (astronomically unlikely)")?;
        let public_key = signing_key.verifying_key().into();

        Ok((signing_key, public_key))
    }

    /// Derive Ethereum address from seed: keccak256(uncompressed_pubkey[1..65])[12..32]
    pub fn derive_eth_address(&self, seed: &str) -> Result<(String, String)> {
        let (_, public_key) = self.derive_secp256k1_keypair(seed)?;

        // Uncompressed public key = 0x04 || x (32 bytes) || y (32 bytes) = 65 bytes
        let uncompressed = public_key.to_encoded_point(false);
        let pubkey_bytes = &uncompressed.as_bytes()[1..]; // skip 0x04 prefix

        use sha3::{Digest as Sha3Digest, Keccak256};
        let hash = Keccak256::digest(pubkey_bytes);
        let address = format!("0x{}", hex::encode(&hash[12..]));

        // Return compressed public key (33 bytes) for on-chain storage
        let compressed = public_key.to_encoded_point(true);
        let pubkey_hex = hex::encode(compressed.as_bytes());

        Ok((address, pubkey_hex))
    }

    /// Sign a message with secp256k1 ECDSA (for EVM chains)
    pub fn sign_secp256k1(&self, seed: &str, message: &[u8]) -> Result<Vec<u8>> {
        use k256::ecdsa::{signature::Signer as _, Signature as EcdsaSignature};
        let (signing_key, _) = self.derive_secp256k1_keypair(seed)?;
        let signature: EcdsaSignature = signing_key.sign(message);
        Ok(signature.to_bytes().to_vec())
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
    fn test_encrypt_decrypt_ecies() {
        let keystore = Keystore::generate();
        let seed = "github.com/alice/project:alice.near";
        let plaintext = b"my secret API key: sk-1234567890";

        let encrypted = keystore.encrypt(seed, plaintext).unwrap();

        // Verify ECIES format: starts with version byte
        assert_eq!(encrypted[0], Keystore::ECIES_VERSION);
        // Minimum size: 1 + 32 + 12 + 16 = 61 (+ plaintext)
        assert!(encrypted.len() >= 61);

        let decrypted = keystore.decrypt(seed, &encrypted).unwrap();
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn test_decrypt_legacy_format() {
        let keystore = Keystore::generate();
        let seed = "github.com/alice/project:alice.near";
        let plaintext = b"legacy secret data";

        // Manually create legacy format: [nonce(12) | ciphertext | tag(16)]
        let (_, verifying_key) = keystore.derive_keypair(seed).unwrap();
        let key_bytes = verifying_key.to_bytes();
        let cipher = ChaCha20Poly1305::new((&key_bytes).into());
        let nonce = ChaCha20Poly1305::generate_nonce(&mut AeadOsRng);
        let ciphertext_with_tag = cipher.encrypt(&nonce, &plaintext[..]).unwrap();
        let mut legacy_encrypted = Vec::new();
        legacy_encrypted.extend_from_slice(&nonce);
        legacy_encrypted.extend_from_slice(&ciphertext_with_tag);

        // decrypt() should handle legacy format via fallback
        let decrypted = keystore.decrypt(seed, &legacy_encrypted).unwrap();
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn test_ecies_pubkey_cannot_decrypt() {
        let keystore = Keystore::generate();
        let seed = "github.com/alice/project:alice.near";
        let plaintext = b"secret";

        let encrypted = keystore.encrypt(seed, plaintext).unwrap();
        let pubkey_hex = keystore.public_key_hex(seed).unwrap();
        let pubkey_bytes = hex::decode(&pubkey_hex).unwrap();

        // Try using the public key as a ChaCha20 symmetric key (the old vulnerable way)
        let cipher = ChaCha20Poly1305::new_from_slice(&pubkey_bytes).unwrap();
        // With ECIES format, nonce starts at byte 33
        let nonce = Nonce::from_slice(&encrypted[33..45]);
        let result = cipher.decrypt(nonce, &encrypted[45..]);

        // Must fail — public key is not the encryption key anymore
        assert!(result.is_err(), "Public key must NOT be able to decrypt ECIES data");
    }

    #[test]
    fn test_ecies_different_ciphertext_each_time() {
        let keystore = Keystore::generate();
        let seed = "test-seed";
        let plaintext = b"same plaintext";

        let enc1 = keystore.encrypt(seed, plaintext).unwrap();
        let enc2 = keystore.encrypt(seed, plaintext).unwrap();

        // Ephemeral keypair is random, so ciphertexts must differ
        assert_ne!(enc1, enc2);

        // But both decrypt to same plaintext
        assert_eq!(keystore.decrypt(seed, &enc1).unwrap(), plaintext);
        assert_eq!(keystore.decrypt(seed, &enc2).unwrap(), plaintext);
    }

    #[test]
    fn test_wrong_seed_cannot_decrypt() {
        let keystore = Keystore::generate();
        let encrypted = keystore.encrypt("seed-a", b"secret").unwrap();
        let result = keystore.decrypt("seed-b", &encrypted);
        assert!(result.is_err());
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

    // ======================= Wallet subkey derivation tests ====================
    // Convention: "wallet:{id}:{chain}:{sub_path}" for sub-keys under a wallet

    #[test]
    fn test_wallet_subkey_deterministic() {
        let ks = Keystore::generate();
        let seed = "wallet:abc:near:check:0";
        let (sk1, vk1) = ks.derive_keypair(seed).unwrap();
        let (sk2, vk2) = ks.derive_keypair(seed).unwrap();
        assert_eq!(sk1.to_bytes(), sk2.to_bytes());
        assert_eq!(vk1.as_bytes(), vk2.as_bytes());
    }

    #[test]
    fn test_wallet_subkey_differs_by_sub_path() {
        let ks = Keystore::generate();
        let (_, vk0) = ks.derive_keypair("wallet:abc:near:check:0").unwrap();
        let (_, vk1) = ks.derive_keypair("wallet:abc:near:check:1").unwrap();
        assert_ne!(vk0.as_bytes(), vk1.as_bytes());
    }

    #[test]
    fn test_wallet_subkey_differs_from_main_key() {
        let ks = Keystore::generate();
        let (_, wallet_vk) = ks.derive_keypair("wallet:test-id:near").unwrap();
        let (_, sub_vk) = ks.derive_keypair("wallet:test-id:near:check:0").unwrap();
        assert_ne!(wallet_vk.as_bytes(), sub_vk.as_bytes());
    }

    #[test]
    fn test_wallet_subkey_implicit_account_is_64_hex() {
        let ks = Keystore::generate();
        let (_, vk) = ks.derive_keypair("wallet:abc:near:check:42").unwrap();
        assert_eq!(hex::encode(vk.as_bytes()).len(), 64);
    }
}
