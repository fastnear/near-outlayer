//! Cryptographic operations for keystore
//!
//! Uses master secret + HMAC-SHA256 to derive repo-specific keypairs.
//! Each repository (with owner) gets a unique keypair derived from the same master secret.
//! All operations are designed to be TEE-safe (no key material leaves secure enclave).
//!
//! Encryption: ECIES with X25519 ECDH + HKDF-SHA256 + ChaCha20-Poly1305
//! - Asymmetric: encrypt with public key, only TEE can decrypt with private key
//! - Format v1: [0x01 | ephemeral_x25519_pubkey (32) | nonce (12) | ciphertext | auth_tag (16)]
//! - Legacy format: [nonce (12) | ciphertext | auth_tag (16)] (symmetric, deprecated)

use anyhow::{Context, Result};
use k256::elliptic_curve::sec1::ToEncodedPoint;
use chacha20poly1305::{
    aead::{Aead, AeadCore, KeyInit, OsRng as AeadOsRng},
    ChaCha20Poly1305, Nonce,
};
use ed25519_dalek::{SigningKey, VerifyingKey, Signature, Signer, Verifier};
use hmac::{Hmac, Mac};
use hkdf::Hkdf;
use near_primitives::types::AccountId;
use rand::{rngs::OsRng, RngCore};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use x25519_dalek::{EphemeralSecret, PublicKey as X25519PublicKey, StaticSecret};

type HmacSha256 = Hmac<Sha256>;

/// Maximum size for encrypted data (10 MB)
const MAX_ENCRYPTED_SIZE: usize = 10 * 1024 * 1024;

/// Keystore holds master secrets and caches derived keypairs.
///
/// The single `master_secret` is split into:
///
/// * `default_master` — the shared OutLayer master, used for Category B
///   (operational) data and any caller that does not name a customer.
///   This is what every legacy secret was encrypted against and what
///   the worker still falls back to when `customer = None`.
/// * `masters` — per-customer (per-vault) masters, populated lazily on
///   first request. `add_customer` inserts; `evict_customer` removes.
///   The map is wrapped in `Arc<RwLock<…>>` so the keystore can be
///   cloned cheaply across handler tasks (read-mostly, write-rare).
///
/// Lookup convention:
///   * `customer = None` ⇒ use `default_master` (legacy / Category B).
///   * `customer = Some(c)` ⇒ require an entry in `masters` for `c`;
///     panic-bail otherwise so callers must `add_customer` first
///     (the lazy-load code path lives one layer up in `mpc_ckd.rs`).
///
/// **Clone semantics — IMPORTANT for the lazy-load gate.** `Clone`
/// duplicates `default_master` byte-for-byte (Copy) but *shares* the
/// `Arc<RwLock<…>>` fields with the original. Snapshotting the
/// keystore via `.clone()` (e.g. inside `AppState::ensure_customer_loaded`)
/// is therefore safe: inserts into the snapshot's `masters` propagate
/// back to all other clones, and reads see whatever the latest writer
/// produced. **Any future field added to this struct that is not
/// `Arc`-shared will break that invariant** — e.g. a per-keystore
/// generation counter would need to be `Arc<AtomicU64>` rather than
/// a plain `u64`. Treat this as a hard constraint when extending.
#[derive(Debug, Clone)]
pub struct Keystore {
    /// Shared OutLayer master (32 bytes, NEVER leaves TEE memory).
    default_master: [u8; 32],

    /// Per-customer masters keyed by vault account id. Populated by
    /// [`Keystore::add_customer`] (typically called from the
    /// `mpc_ckd.rs` lazy-load path after a fresh `vault.add_customer`
    /// MPC CKD round-trip).
    masters: Arc<RwLock<HashMap<AccountId, [u8; 32]>>>,

    /// Cache of derived keypairs. Keyed by `(customer, seed)` so
    /// customer A's keypair for `seed=alice/repo` cannot collide with
    /// customer B's keypair for the same seed.
    keypair_cache: Arc<RwLock<HashMap<(Option<AccountId>, String), (SigningKey, VerifyingKey)>>>,
}

impl Keystore {
    /// Generate a new keystore with random master secret
    ///
    /// In production TEE:
    /// - Master secret is generated using TEE hardware RNG
    /// - Sealed to TEE persistent storage
    /// - Only accessible within the same TEE enclave
    pub fn generate() -> Self {
        let mut default_master = [0u8; 32];
        OsRng.fill_bytes(&mut default_master);

        tracing::info!("Generated new keystore with random default master secret");

        Self {
            default_master,
            masters: Arc::new(RwLock::new(HashMap::new())),
            keypair_cache: Arc::new(RwLock::new(HashMap::new())),
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

        tracing::info!("Loaded keystore from default master secret");

        Ok(Self {
            default_master: *master_secret,
            masters: Arc::new(RwLock::new(HashMap::new())),
            keypair_cache: Arc::new(RwLock::new(HashMap::new())),
        })
    }

    /// Export the default master as hex (for backup / persistence).
    ///
    /// **WARNING:** only for initial setup or backup. Per-customer
    /// (per-vault) masters are NOT included — they are re-derivable
    /// lazily through MPC CKD on demand and never persisted to disk.
    /// The name says `default_master_hex` deliberately so a future
    /// reader can't confuse this with "back up everything"; backing
    /// this up only restores the OutLayer master, not customer state.
    pub fn default_master_hex(&self) -> String {
        hex::encode(self.default_master)
    }

    // =========================================================================
    // Per-customer master management
    // =========================================================================

    /// Insert a per-customer (per-vault) master. Called by
    /// `mpc_ckd.rs::add_customer` after a successful Layer-2 CKD
    /// round-trip materialises the per-vault master inside the TEE.
    ///
    /// Idempotent: re-inserting the same master overwrites the entry.
    /// The keypair cache is invalidated for this customer so any
    /// previously-cached keypairs (which would have been derived from
    /// a stale master) are dropped.
    pub fn add_customer(&self, customer: AccountId, master: [u8; 32]) {
        {
            let mut masters = self.masters.write().unwrap();
            masters.insert(customer.clone(), master);
        }
        self.evict_customer_cache(&customer);
        tracing::info!(
            customer = %customer,
            "Per-customer master loaded into keystore"
        );
    }

    /// Remove a per-customer master and drop any cached keypairs for
    /// it. Called from the `/admin/evict-customer` endpoint when the
    /// monitoring service detects a vault should no longer operate
    /// (race-attack ban, etc.). Subsequent derive_* calls for this
    /// customer will fail until `add_customer` is called again.
    pub fn evict_customer(&self, customer: &AccountId) {
        {
            let mut masters = self.masters.write().unwrap();
            masters.remove(customer);
        }
        self.evict_customer_cache(customer);
        tracing::info!(customer = %customer, "Per-customer master evicted");
    }

    /// Returns `true` if a per-customer master is currently loaded.
    /// Used by the lazy-load gate to decide whether to skip MPC CKD.
    pub fn has_customer(&self, customer: &AccountId) -> bool {
        self.masters.read().unwrap().contains_key(customer)
    }

    /// Drop every cache entry whose key references the given customer.
    fn evict_customer_cache(&self, customer: &AccountId) {
        let mut cache = self.keypair_cache.write().unwrap();
        cache.retain(|(c, _), _| c.as_ref() != Some(customer));
    }

    /// Resolve a `customer` parameter to the master bytes that should
    /// be used as the HMAC key for derivation.
    ///
    /// * `None` ⇒ `default_master`.
    /// * `Some(c)` ⇒ master from the `masters` map; bail with a
    ///   diagnostic error if missing — the lazy-load layer must run
    ///   `add_customer` before invoking any derive_* method.
    fn master_for(&self, customer: Option<&AccountId>) -> Result<[u8; 32]> {
        match customer {
            None => Ok(self.default_master),
            Some(c) => {
                let masters = self.masters.read().unwrap();
                masters.get(c).copied().ok_or_else(|| {
                    anyhow::anyhow!(
                        "per-customer master not loaded for {c}; \
                         run mpc_ckd::add_customer first"
                    )
                })
            }
        }
    }

    /// Derive an Ed25519 keypair from `(customer, seed)`.
    ///
    /// `customer = None` uses the OutLayer default master; `Some(c)`
    /// uses customer `c`'s per-vault master (must be loaded via
    /// [`Keystore::add_customer`] first — otherwise this returns an
    /// error). Different `customer` values produce disjoint keyspaces
    /// for the SAME seed — that is the customer-isolation invariant.
    ///
    /// Seed format examples:
    /// - `"github.com/alice/project:alice.near"` (all branches)
    /// - `"github.com/alice/project:alice.near:main"` (specific branch)
    ///
    /// Uses `HMAC-SHA256(master, seed)`; deterministic for any
    /// fixed `(master, seed)` pair.
    pub fn derive_keypair(
        &self,
        customer: Option<&AccountId>,
        seed: &str,
    ) -> Result<(SigningKey, VerifyingKey)> {
        let cache_key = (customer.cloned(), seed.to_string());
        // Check cache first
        {
            let cache = self.keypair_cache.read().unwrap();
            if let Some(keypair) = cache.get(&cache_key) {
                return Ok(keypair.clone());
            }
        }

        // Derive keypair using HMAC-SHA256 over the appropriate master.
        let master = self.master_for(customer)?;
        let mut mac = <HmacSha256 as Mac>::new_from_slice(&master)
            .expect("HMAC can take key of any size");
        mac.update(seed.as_bytes());
        let derived_bytes = mac.finalize().into_bytes();

        // Use first 32 bytes as Ed25519 secret key
        let mut secret_bytes = [0u8; 32];
        secret_bytes.copy_from_slice(&derived_bytes[..32]);

        let signing_key = SigningKey::from_bytes(&secret_bytes);
        let verifying_key = signing_key.verifying_key();

        tracing::debug!(
            customer = ?customer,
            "Derived keypair for seed='{}', pubkey={}",
            seed,
            hex::encode(verifying_key.as_bytes())
        );

        // Cache the result
        {
            let mut cache = self.keypair_cache.write().unwrap();
            cache.insert(cache_key, (signing_key.clone(), verifying_key));
        }

        Ok((signing_key, verifying_key))
    }

    /// Get Ed25519 public key for `(customer, seed)`.
    pub fn get_public_key_for_seed(
        &self,
        customer: Option<&AccountId>,
        seed: &str,
    ) -> Result<VerifyingKey> {
        let (_signing_key, verifying_key) = self.derive_keypair(customer, seed)?;
        Ok(verifying_key)
    }

    /// Derive X25519 keypair from `(customer, seed)` (for ECIES).
    ///
    /// Domain-separated `HMAC-SHA256(master, "ecies:" || seed)` so the
    /// encryption keys are independent from Ed25519 signing keys.
    fn derive_x25519_keypair(
        &self,
        customer: Option<&AccountId>,
        seed: &str,
    ) -> Result<(StaticSecret, X25519PublicKey)> {
        let master = self.master_for(customer)?;
        let mut mac = <HmacSha256 as Mac>::new_from_slice(&master)
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

    /// Derive a deterministic string from `(customer, seed)` suitable for
    /// use as the MPC CKD `derivation_path` argument.
    ///
    /// Produces a 64-char lowercase hex digest of `HMAC-SHA256(master, "secret-path:" || seed)`.
    ///
    /// **Why this matters (Layer 2 of per-vault master derivation):** the
    /// per-customer master is requested from MPC by signing FROM the
    /// customer's vault account. The MPC contract derives a per-app key
    /// from `SHA3(prefix || predecessor || derivation_path)`. If the
    /// derivation_path were customer-controllable (e.g. literally `vault.id`
    /// or empty string), a malicious customer with a backup vault key could
    /// pre-empt the worker and call MPC themselves before vault-checker
    /// runs, getting the same master.
    ///
    /// **Race-window protection, not forever-secret.** Before the worker's
    /// first MPC call, the path is unguessable without OutLayer master
    /// access — that's the property that buys us the race-window. After
    /// the worker submits the tx, the path goes on-chain in plaintext as
    /// part of the tx args; from that moment it is publicly visible. The
    /// race-attack mitigation is therefore *first-write-wins*: an
    /// off-chain indexer watches for duplicate `(predecessor, path)`
    /// pairs and bans vaults that race the worker. Pair this with the
    /// guarantee that a customer can't compute the path before the
    /// worker uses it, and the attack surface collapses to a few-block
    /// window that the indexer covers.
    ///
    /// Recovery flow still works: any approved TEE has the default
    /// master, can re-derive the same path, and gets the same master
    /// back from MPC.
    ///
    /// Domain separator `"secret-path:"` keeps this output disjoint from
    /// `derive_keypair`/`derive_x25519_keypair`/etc. for the same seed.
    pub fn derive_secret_string(
        &self,
        customer: Option<&AccountId>,
        seed: &str,
    ) -> Result<String> {
        let master = self.master_for(customer)?;
        let mut mac = <HmacSha256 as Mac>::new_from_slice(&master)
            .expect("HMAC can take key of any size");
        mac.update(b"secret-path:");
        mac.update(seed.as_bytes());
        let digest = mac.finalize().into_bytes();
        Ok(hex::encode(digest))
    }

    /// Get X25519 public key as hex string for `(customer, seed)`
    /// (the key returned by `/pubkey`). Safe to expose publicly — it
    /// can only encrypt, not decrypt. Only the TEE holds the private key.
    pub fn public_key_hex(
        &self,
        customer: Option<&AccountId>,
        seed: &str,
    ) -> Result<String> {
        let (_, x25519_pub) = self.derive_x25519_keypair(customer, seed)?;
        Ok(hex::encode(x25519_pub.as_bytes()))
    }

    /// Get Ed25519 public key as base58 string (NEAR format) for
    /// `(customer, seed)`.
    #[allow(dead_code)]
    pub fn public_key_base58(
        &self,
        customer: Option<&AccountId>,
        seed: &str,
    ) -> Result<String> {
        let verifying_key = self.get_public_key_for_seed(customer, seed)?;
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
    pub fn decrypt(
        &self,
        customer: Option<&AccountId>,
        seed: &str,
        encrypted_data: &[u8],
    ) -> Result<Vec<u8>> {
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
            match self.decrypt_ecies(customer, seed, encrypted_data) {
                Ok(plaintext) => return Ok(plaintext),
                Err(e) => {
                    // AEAD failure could mean this is legacy data that happens to start with 0x01
                    tracing::debug!("ECIES decrypt failed, trying legacy format: {}", e);
                }
            }
        }

        // Legacy format: [nonce(12) | ciphertext | tag(16)]
        self.decrypt_legacy(customer, seed, encrypted_data)
    }

    /// Decrypt ECIES v1 format
    fn decrypt_ecies(
        &self,
        customer: Option<&AccountId>,
        seed: &str,
        encrypted_data: &[u8],
    ) -> Result<Vec<u8>> {
        let ephemeral_pub_bytes: [u8; 32] = encrypted_data[1..33]
            .try_into()
            .context("Invalid ephemeral public key")?;
        let ephemeral_pub = X25519PublicKey::from(ephemeral_pub_bytes);

        let (x25519_secret, _) = self.derive_x25519_keypair(customer, seed)?;
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
    fn decrypt_legacy(
        &self,
        customer: Option<&AccountId>,
        seed: &str,
        encrypted_data: &[u8],
    ) -> Result<Vec<u8>> {
        let (_signing_key, verifying_key) = self.derive_keypair(customer, seed)?;

        let key_bytes = verifying_key.to_bytes();
        let cipher = ChaCha20Poly1305::new((&key_bytes).into());

        let nonce = Nonce::from_slice(&encrypted_data[0..12]);
        let ciphertext_with_tag = &encrypted_data[12..];

        let plaintext = cipher
            .decrypt(nonce, ciphertext_with_tag)
            .map_err(|e| anyhow::anyhow!("Legacy decryption failed (data tampered or wrong key): {}", e))?;

        Ok(plaintext)
    }

    /// Encrypt plaintext for `(customer, seed)` using ECIES.
    ///
    /// Uses ephemeral X25519 keypair + ECDH + HKDF-SHA256 + ChaCha20-Poly1305.
    /// Returns: `[0x01 | ephemeral_x25519_pubkey (32) | nonce (12) | ciphertext | auth_tag (16)]`.
    pub fn encrypt(
        &self,
        customer: Option<&AccountId>,
        seed: &str,
        plaintext: &[u8],
    ) -> Result<Vec<u8>> {
        if plaintext.len() > MAX_ENCRYPTED_SIZE {
            anyhow::bail!("Plaintext too large: {} bytes", plaintext.len());
        }

        let (_, recipient_pub) = self.derive_x25519_keypair(customer, seed)?;

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

    /// Sign a message with the Ed25519 private key for `(customer, seed)`.
    #[allow(dead_code)]
    pub fn sign(
        &self,
        customer: Option<&AccountId>,
        seed: &str,
        message: &[u8],
    ) -> Result<Signature> {
        let (signing_key, _) = self.derive_keypair(customer, seed)?;
        Ok(signing_key.sign(message))
    }

    /// Verify an Ed25519 signature for `(customer, seed)`.
    #[allow(dead_code)]
    pub fn verify(
        &self,
        customer: Option<&AccountId>,
        seed: &str,
        message: &[u8],
        signature: &Signature,
    ) -> Result<()> {
        let (_, verifying_key) = self.derive_keypair(customer, seed)?;
        verifying_key
            .verify(message, signature)
            .context("Signature verification failed")
    }

    // =========================================================================
    // secp256k1 (Ethereum/Base/EVM chains)
    // See: docs/MULTI_CHAIN.md for integration guide
    // =========================================================================

    /// Derive a secp256k1 keypair from `(customer, seed)` (for EVM chains).
    ///
    /// Same HMAC-SHA256 derivation as Ed25519, with the 32-byte output
    /// interpreted as a secp256k1 scalar.
    pub fn derive_secp256k1_keypair(
        &self,
        customer: Option<&AccountId>,
        seed: &str,
    ) -> Result<(k256::ecdsa::SigningKey, k256::elliptic_curve::PublicKey<k256::Secp256k1>)> {
        let master = self.master_for(customer)?;
        let mut mac = <HmacSha256 as Mac>::new_from_slice(&master)
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

    /// Derive Ethereum address from `(customer, seed)`:
    /// `keccak256(uncompressed_pubkey[1..65])[12..32]`.
    pub fn derive_eth_address(
        &self,
        customer: Option<&AccountId>,
        seed: &str,
    ) -> Result<(String, String)> {
        let (_, public_key) = self.derive_secp256k1_keypair(customer, seed)?;

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

    /// Sign a message with secp256k1 ECDSA for `(customer, seed)`.
    pub fn sign_secp256k1(
        &self,
        customer: Option<&AccountId>,
        seed: &str,
        message: &[u8],
    ) -> Result<Vec<u8>> {
        use k256::ecdsa::{signature::Signer as _, Signature as EcdsaSignature};
        let (signing_key, _) = self.derive_secp256k1_keypair(customer, seed)?;
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
    ///
    /// **Always uses the OutLayer default master**, regardless of which
    /// customer is calling. VRF is Category B (operational randomness)
    /// per the per-vault master plan — its lifetime is tied to the
    /// coordinator, not to any individual customer/vault.
    pub fn vrf_generate(&self, alpha: &[u8]) -> Result<(String, String)> {
        let (signing_key, _) = self.derive_keypair(None, "vrf-key")?;
        let signature = signing_key.sign(alpha);

        let mut hasher = Sha256::new();
        hasher.update(signature.to_bytes());
        let output = hasher.finalize();

        Ok((hex::encode(output), hex::encode(signature.to_bytes())))
    }

    /// Get the VRF public key as hex string. Always derived from the
    /// default master (Category B).
    pub fn vrf_public_key_hex(&self) -> Result<String> {
        let (_, verifying_key) = self.derive_keypair(None, "vrf-key")?;
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
    use base64::Engine;

    #[test]
    fn test_keystore_generation() {
        let keystore = Keystore::generate();
        let pubkey = keystore.public_key_hex(None, "test-seed").unwrap();
        assert_eq!(pubkey.len(), 64); // 32 bytes = 64 hex chars
    }

    #[test]
    fn test_deterministic_derivation() {
        let keystore = Keystore::generate();

        let pubkey1 = keystore.public_key_hex(None, "github.com/alice/project:alice.near").unwrap();
        let pubkey2 = keystore.public_key_hex(None, "github.com/alice/project:alice.near").unwrap();

        assert_eq!(pubkey1, pubkey2, "Same seed should produce same key");
    }

    #[test]
    fn test_different_seeds_different_keys() {
        let keystore = Keystore::generate();

        let pubkey_alice = keystore.public_key_hex(None, "github.com/alice/project:alice.near").unwrap();
        let pubkey_bob = keystore.public_key_hex(None, "github.com/alice/project:bob.near").unwrap();

        assert_ne!(pubkey_alice, pubkey_bob, "Different seeds should produce different keys");
    }

    #[test]
    fn test_encrypt_decrypt_ecies() {
        let keystore = Keystore::generate();
        let seed = "github.com/alice/project:alice.near";
        let plaintext = b"my secret API key: sk-1234567890";

        let encrypted = keystore.encrypt(None, seed, plaintext).unwrap();

        // Verify ECIES format: starts with version byte
        assert_eq!(encrypted[0], Keystore::ECIES_VERSION);
        // Minimum size: 1 + 32 + 12 + 16 = 61 (+ plaintext)
        assert!(encrypted.len() >= 61);

        let decrypted = keystore.decrypt(None, seed, &encrypted).unwrap();
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn test_decrypt_legacy_format() {
        let keystore = Keystore::generate();
        let seed = "github.com/alice/project:alice.near";
        let plaintext = b"legacy secret data";

        // Manually create legacy format: [nonce(12) | ciphertext | tag(16)]
        let (_, verifying_key) = keystore.derive_keypair(None, seed).unwrap();
        let key_bytes = verifying_key.to_bytes();
        let cipher = ChaCha20Poly1305::new((&key_bytes).into());
        let nonce = ChaCha20Poly1305::generate_nonce(&mut AeadOsRng);
        let ciphertext_with_tag = cipher.encrypt(&nonce, &plaintext[..]).unwrap();
        let mut legacy_encrypted = Vec::new();
        legacy_encrypted.extend_from_slice(&nonce);
        legacy_encrypted.extend_from_slice(&ciphertext_with_tag);

        // decrypt() should handle legacy format via fallback
        let decrypted = keystore.decrypt(None, seed, &legacy_encrypted).unwrap();
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn test_ecies_pubkey_cannot_decrypt() {
        let keystore = Keystore::generate();
        let seed = "github.com/alice/project:alice.near";
        let plaintext = b"secret";

        let encrypted = keystore.encrypt(None, seed, plaintext).unwrap();
        let pubkey_hex = keystore.public_key_hex(None, seed).unwrap();
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

        let enc1 = keystore.encrypt(None, seed, plaintext).unwrap();
        let enc2 = keystore.encrypt(None, seed, plaintext).unwrap();

        // Ephemeral keypair is random, so ciphertexts must differ
        assert_ne!(enc1, enc2);

        // But both decrypt to same plaintext
        assert_eq!(keystore.decrypt(None, seed, &enc1).unwrap(), plaintext);
        assert_eq!(keystore.decrypt(None, seed, &enc2).unwrap(), plaintext);
    }

    #[test]
    fn test_wrong_seed_cannot_decrypt() {
        let keystore = Keystore::generate();
        let encrypted = keystore.encrypt(None, "seed-a", b"secret").unwrap();
        let result = keystore.decrypt(None, "seed-b", &encrypted);
        assert!(result.is_err());
    }

    #[test]
    fn test_sign_verify() {
        let keystore = Keystore::generate();
        let seed = "github.com/alice/project:alice.near";
        let message = b"hello world";

        let signature = keystore.sign(None, seed, message).unwrap();
        keystore.verify(None, seed, message, &signature).unwrap();
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
        let vrf_pubkey = keystore.get_public_key_for_seed(None, "vrf-key").unwrap();

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
        let master_hex = ks1.default_master_hex();
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
        let pubkey1 = keystore1.public_key_hex(None, seed).unwrap();

        // Serialize master secret (in production, this would be sealed storage)
        let master_secret_hex = hex::encode(&keystore1.default_master);

        // Load from same master secret
        let keystore2 = Keystore::from_master_secret_hex(&master_secret_hex).unwrap();
        let pubkey2 = keystore2.public_key_hex(None, seed).unwrap();

        assert_eq!(pubkey1, pubkey2, "Same master secret should produce same derived keys");
    }

    // ======================= Wallet subkey derivation tests ====================
    // Convention: "wallet:{id}:{chain}:{sub_path}" for sub-keys under a wallet

    #[test]
    fn test_wallet_subkey_deterministic() {
        let ks = Keystore::generate();
        let seed = "wallet:abc:near:check:0";
        let (sk1, vk1) = ks.derive_keypair(None, seed).unwrap();
        let (sk2, vk2) = ks.derive_keypair(None, seed).unwrap();
        assert_eq!(sk1.to_bytes(), sk2.to_bytes());
        assert_eq!(vk1.as_bytes(), vk2.as_bytes());
    }

    #[test]
    fn test_wallet_subkey_differs_by_sub_path() {
        let ks = Keystore::generate();
        let (_, vk0) = ks.derive_keypair(None, "wallet:abc:near:check:0").unwrap();
        let (_, vk1) = ks.derive_keypair(None, "wallet:abc:near:check:1").unwrap();
        assert_ne!(vk0.as_bytes(), vk1.as_bytes());
    }

    #[test]
    fn test_wallet_subkey_differs_from_main_key() {
        let ks = Keystore::generate();
        let (_, wallet_vk) = ks.derive_keypair(None, "wallet:test-id:near").unwrap();
        let (_, sub_vk) = ks.derive_keypair(None, "wallet:test-id:near:check:0").unwrap();
        assert_ne!(wallet_vk.as_bytes(), sub_vk.as_bytes());
    }

    #[test]
    fn test_wallet_subkey_implicit_account_is_64_hex() {
        let ks = Keystore::generate();
        let (_, vk) = ks.derive_keypair(None, "wallet:abc:near:check:42").unwrap();
        assert_eq!(hex::encode(vk.as_bytes()).len(), 64);
    }

    // ======================= Policy signing key tests ==========================
    // After ECIES migration, public_key_hex() returns X25519 (encryption) key,
    // while get_public_key_for_seed() returns Ed25519 (signing) key.
    // The contract's store_wallet_policy needs Ed25519 for ed25519_verify.

    #[test]
    fn test_public_key_hex_and_ed25519_are_different_keys() {
        let ks = Keystore::generate();
        let seed = "wallet:test-id:near";

        // public_key_hex returns X25519 (encryption key)
        let x25519_hex = ks.public_key_hex(None, seed).unwrap();

        // get_public_key_for_seed returns Ed25519 (signing key)
        let ed25519_vk = ks.get_public_key_for_seed(None, seed).unwrap();
        let ed25519_hex = hex::encode(ed25519_vk.as_bytes());

        // Both are 32 bytes (64 hex chars) but different keys
        assert_eq!(x25519_hex.len(), 64);
        assert_eq!(ed25519_hex.len(), 64);
        assert_ne!(
            x25519_hex, ed25519_hex,
            "X25519 and Ed25519 keys must differ for the same seed"
        );
    }

    #[test]
    fn test_sign_verify_with_ed25519_pubkey() {
        let ks = Keystore::generate();
        let seed = "wallet:test-id:near";
        let message = b"test message for policy";

        let signature = ks.sign(None, seed, message).unwrap();
        let ed25519_vk = ks.get_public_key_for_seed(None, seed).unwrap();

        // Verification with correct Ed25519 key must succeed
        ed25519_vk
            .verify(message, &signature)
            .expect("Ed25519 verify must succeed with matching key");
    }

    #[test]
    fn test_ed25519_verify_fails_with_x25519_pubkey() {
        let ks = Keystore::generate();
        let seed = "wallet:test-id:near";
        let message = b"test message for policy";

        // Sign with Ed25519 key
        let signature = ks.sign(None, seed, message).unwrap();

        // Get X25519 key (what public_key_hex returns after ECIES migration)
        let x25519_hex = ks.public_key_hex(None, seed).unwrap();
        let x25519_bytes: [u8; 32] = hex::decode(&x25519_hex)
            .unwrap()
            .try_into()
            .unwrap();

        // Try to use X25519 bytes as Ed25519 verifying key — this is what the
        // contract does when sign-policy returns the wrong public_key_hex.
        // It must fail: either VerifyingKey::from_bytes rejects the point,
        // or verify() returns an error.
        let result = VerifyingKey::from_bytes(&x25519_bytes)
            .and_then(|vk| vk.verify(message, &signature));
        assert!(
            result.is_err(),
            "Verification with X25519 key as Ed25519 must fail"
        );
    }

    #[test]
    fn test_sign_policy_flow_end_to_end() {
        let ks = Keystore::generate();
        let wallet_seed = "wallet:test-id:near";
        let policy_seed = "wallet-policy:test-id";

        // Step 1: Encrypt policy (uses separate policy seed)
        let policy_json = br#"{"version":1,"frozen":false,"rules":{}}"#;
        let encrypted = ks.encrypt(None, policy_seed, policy_json).unwrap();
        let encrypted_base64 = base64::engine::general_purpose::STANDARD.encode(&encrypted);

        // Step 2: SHA256 of encrypted_data string (what contract does)
        let mut hasher = Sha256::new();
        hasher.update(encrypted_base64.as_bytes());
        let hash: [u8; 32] = hasher.finalize().into();

        // Step 3: Sign hash with wallet key
        let signature = ks.sign(None, wallet_seed, &hash).unwrap();

        // Step 4: Verify with Ed25519 key — must succeed
        let ed25519_vk = ks.get_public_key_for_seed(None, wallet_seed).unwrap();
        ed25519_vk
            .verify(&hash, &signature)
            .expect("Verify with Ed25519 key must succeed");

        // Step 5: Verify with X25519 key — must fail (reproduces the bug)
        let x25519_hex = ks.public_key_hex(None, wallet_seed).unwrap();
        let x25519_bytes: [u8; 32] = hex::decode(&x25519_hex)
            .unwrap()
            .try_into()
            .unwrap();
        let result = VerifyingKey::from_bytes(&x25519_bytes)
            .and_then(|vk| vk.verify(&hash, &signature));
        assert!(
            result.is_err(),
            "Verify with X25519 key must fail — this is the bug"
        );
    }

    // ============== derive_secret_string (MPC CKD path) ==============
    // Used as the MPC CKD `derivation_path` argument when adding a
    // per-customer master. Must be (a) deterministic for replay across
    // worker restarts, (b) dependent on the master so a customer cannot
    // forge it without OutLayer master access, (c) domain-separated from
    // keypair derivation, (d) hex-encoded and exactly 64 chars.

    #[test]
    fn test_derive_secret_string_deterministic() {
        let ks = Keystore::generate();
        let s1 = ks.derive_secret_string(None, "vault-master:vault.alice.testnet").unwrap();
        let s2 = ks.derive_secret_string(None, "vault-master:vault.alice.testnet").unwrap();
        assert_eq!(s1, s2);
        assert_eq!(s1.len(), 64, "HMAC-SHA256 hex must be 64 chars");
        assert!(s1.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn test_derive_secret_string_seed_separates() {
        let ks = Keystore::generate();
        let a = ks.derive_secret_string(None, "vault-master:vault.alice.testnet").unwrap();
        let b = ks.derive_secret_string(None, "vault-master:vault.bob.testnet").unwrap();
        assert_ne!(a, b);
    }

    #[test]
    fn test_derive_secret_string_master_dependent() {
        // Different masters → different paths for the same seed.
        // Critical for the unforgeability story — a customer cannot
        // compute the path without the OutLayer master.
        let ks1 = Keystore::generate();
        let ks2 = Keystore::generate();
        let seed = "vault-master:vault.alice.testnet";
        let s1 = ks1.derive_secret_string(None, seed).unwrap();
        let s2 = ks2.derive_secret_string(None, seed).unwrap();
        assert_ne!(s1, s2);
    }

    #[test]
    fn test_derive_secret_string_domain_separated_from_keypair() {
        // The output must NOT be derivable as a side-effect of any
        // existing derive_* method, otherwise a leak of the keypair
        // bytes would reveal the path. Different domain prefix means
        // different HMAC, which means different output even for the
        // same seed string.
        let ks = Keystore::generate();
        let seed = "vault.alice.testnet";
        let path = ks.derive_secret_string(None, seed).unwrap();
        let (_, vk) = ks.derive_keypair(None, seed).unwrap();
        assert_ne!(path, hex::encode(vk.as_bytes()));
    }

    // ============== Multi-customer isolation ==============
    // These tests pin the customer-isolation invariant at the CRYPTO
    // layer — anything above (api handlers, MPC CKD) is built on top
    // of these guarantees, so if the crypto layer leaks across
    // customers, every higher layer leaks too.
    //
    // Each test loads two distinct per-customer masters with KNOWN
    // bytes (so test failures aren't blamed on RNG); HMAC is
    // deterministic, so the test is fully reproducible.
    use std::str::FromStr;

    fn ks_with_two_customers() -> (Keystore, AccountId, AccountId) {
        let ks = Keystore::generate();
        let alice = AccountId::from_str("vault.alice.testnet").unwrap();
        let bob = AccountId::from_str("vault.bob.testnet").unwrap();
        // Use distinct, well-known masters so any leak surfaces.
        ks.add_customer(alice.clone(), [0xAA; 32]);
        ks.add_customer(bob.clone(), [0xBB; 32]);
        (ks, alice, bob)
    }

    #[test]
    fn isolation_ed25519_pubkeys_disjoint_per_customer() {
        let (ks, alice, bob) = ks_with_two_customers();
        let seed = "wallet:abc:near";
        let (_, vk_a) = ks.derive_keypair(Some(&alice), seed).unwrap();
        let (_, vk_b) = ks.derive_keypair(Some(&bob), seed).unwrap();
        let (_, vk_default) = ks.derive_keypair(None, seed).unwrap();
        assert_ne!(vk_a.as_bytes(), vk_b.as_bytes());
        assert_ne!(vk_a.as_bytes(), vk_default.as_bytes());
        assert_ne!(vk_b.as_bytes(), vk_default.as_bytes());
    }

    #[test]
    fn isolation_x25519_pubkeys_disjoint_per_customer() {
        let (ks, alice, bob) = ks_with_two_customers();
        let seed = "wallet-policy:abc";
        let pub_a = ks.public_key_hex(Some(&alice), seed).unwrap();
        let pub_b = ks.public_key_hex(Some(&bob), seed).unwrap();
        let pub_default = ks.public_key_hex(None, seed).unwrap();
        assert_ne!(pub_a, pub_b);
        assert_ne!(pub_a, pub_default);
        assert_ne!(pub_b, pub_default);
    }

    #[test]
    fn isolation_eth_addresses_disjoint_per_customer() {
        let (ks, alice, bob) = ks_with_two_customers();
        let seed = "wallet:abc:ethereum";
        let (addr_a, _) = ks.derive_eth_address(Some(&alice), seed).unwrap();
        let (addr_b, _) = ks.derive_eth_address(Some(&bob), seed).unwrap();
        let (addr_default, _) = ks.derive_eth_address(None, seed).unwrap();
        assert_ne!(addr_a, addr_b);
        assert_ne!(addr_a, addr_default);
        assert_ne!(addr_b, addr_default);
    }

    #[test]
    fn isolation_encrypt_a_cannot_decrypt_b() {
        // The custody-grade guarantee: a ciphertext encrypted under
        // customer A's master is unreadable under customer B's master.
        // Even if a malicious caller submits B's vault_id alongside
        // ciphertext encrypted to A, the decrypt fails — no plaintext
        // recovered.
        let (ks, alice, bob) = ks_with_two_customers();
        let seed = "user:secret-payload";
        let plaintext = b"alice's hardcoded api key";
        let ciphertext = ks.encrypt(Some(&alice), seed, plaintext).unwrap();

        // B can't decrypt A's ciphertext.
        let result_b = ks.decrypt(Some(&bob), seed, &ciphertext);
        assert!(result_b.is_err(), "customer B must NOT be able to decrypt customer A's secret");

        // Default master can't decrypt A's ciphertext.
        let result_default = ks.decrypt(None, seed, &ciphertext);
        assert!(result_default.is_err(), "default master must NOT be able to decrypt customer A's secret");

        // Sanity: A still can.
        let recovered = ks.decrypt(Some(&alice), seed, &ciphertext).unwrap();
        assert_eq!(recovered, plaintext);
    }

    #[test]
    fn isolation_signature_verifies_only_under_correct_customer() {
        let (ks, alice, bob) = ks_with_two_customers();
        let seed = "wallet:abc:near";
        let msg = b"transfer 1 NEAR";
        let sig_a = ks.sign(Some(&alice), seed, msg).unwrap();

        // A's pubkey verifies A's signature.
        ks.verify(Some(&alice), seed, msg, &sig_a).unwrap();

        // B's pubkey does NOT verify A's signature.
        let result_b = ks.verify(Some(&bob), seed, msg, &sig_a);
        assert!(result_b.is_err(), "B's key must NOT verify A's signature");
    }

    #[test]
    fn isolation_secp256k1_keys_disjoint_per_customer() {
        let (ks, alice, bob) = ks_with_two_customers();
        let seed = "wallet:abc:base";
        let (sk_a, _) = ks.derive_secp256k1_keypair(Some(&alice), seed).unwrap();
        let (sk_b, _) = ks.derive_secp256k1_keypair(Some(&bob), seed).unwrap();
        // Different scalars (the secret key is the salient bit).
        assert_ne!(sk_a.to_bytes(), sk_b.to_bytes());
    }

    #[test]
    fn isolation_secret_path_disjoint_per_customer() {
        let (ks, alice, bob) = ks_with_two_customers();
        // Even given the same input string, customer's master gates
        // the path. If a customer somehow guesses another customer's
        // seed, they still can't compute the path.
        let s = "vault-master:vault.eve.testnet";
        let path_a = ks.derive_secret_string(Some(&alice), s).unwrap();
        let path_b = ks.derive_secret_string(Some(&bob), s).unwrap();
        let path_default = ks.derive_secret_string(None, s).unwrap();
        assert_ne!(path_a, path_b);
        assert_ne!(path_a, path_default);
    }

    #[test]
    fn isolation_evict_then_re_derive_fails_without_master() {
        // Plan: "/admin/evict-customer: banned vault triggers eviction
        // → next call fails verify check". At the crypto layer the
        // evict-then-derive shape is: after evict, derive_* fails
        // with the master-not-loaded error. The lazy-load gate
        // (mpc_ckd) is what tries to refresh on top — but if the
        // gate is bypassed (e.g. cached customer assumption broken),
        // the crypto layer must still refuse.
        let ks = Keystore::generate();
        let alice = AccountId::from_str("vault.alice.testnet").unwrap();
        ks.add_customer(alice.clone(), [0xAA; 32]);
        assert!(ks.has_customer(&alice));

        // Eviction.
        ks.evict_customer(&alice);
        assert!(!ks.has_customer(&alice));

        // Now any derive_* for alice must fail. We use derive_keypair
        // as the canonical path; all other derive_* paths funnel
        // through `master_for(customer)` which produces the same
        // error.
        let result = ks.derive_keypair(Some(&alice), "wallet:abc:near");
        assert!(result.is_err());
        let msg = format!("{:#}", result.unwrap_err());
        assert!(
            msg.contains("per-customer master not loaded"),
            "evict must surface a 'master not loaded' error, got: {msg}"
        );
    }

    // ============== Restart determinism ==============
    // The "auto re-derive" path goes through MPC CKD at runtime, but
    // the CRYPTO-LAYER guarantee — that loading the same default
    // master in a fresh Keystore reproduces every derivation —
    // doesn't need MPC. These tests verify the keystore restart
    // produces bit-identical output.

    #[test]
    fn restart_default_master_path_reproduces_all_derivations() {
        // First instance: derive a battery of representative outputs
        // under the default master.
        let ks1 = Keystore::generate();
        let master_hex = ks1.default_master_hex();
        let seed = "wallet:test-restart:near";

        let (_, vk1) = ks1.derive_keypair(None, seed).unwrap();
        let pub_x25519_1 = ks1.public_key_hex(None, seed).unwrap();
        let (eth_addr_1, _) = ks1.derive_eth_address(None, seed).unwrap();
        let path_1 = ks1.derive_secret_string(None, "vault-master:vault.alice.testnet").unwrap();

        // Restart: load identical default master, re-derive everything.
        let ks2 = Keystore::from_master_secret_hex(&master_hex).unwrap();
        let (_, vk2) = ks2.derive_keypair(None, seed).unwrap();
        let pub_x25519_2 = ks2.public_key_hex(None, seed).unwrap();
        let (eth_addr_2, _) = ks2.derive_eth_address(None, seed).unwrap();
        let path_2 = ks2.derive_secret_string(None, "vault-master:vault.alice.testnet").unwrap();

        assert_eq!(vk1.as_bytes(), vk2.as_bytes());
        assert_eq!(pub_x25519_1, pub_x25519_2);
        assert_eq!(eth_addr_1, eth_addr_2);
        assert_eq!(path_1, path_2);
    }

    #[test]
    fn restart_per_customer_master_reproduces_under_known_master() {
        // The lazy-load path provides the per-customer master via
        // MPC CKD; once loaded, it stays in memory. After restart
        // the master must be re-loadable (Layer 2 secret_path is
        // deterministic, so MPC CKD returns the same bytes). At the
        // crypto layer we can verify: given the SAME per-customer
        // master bytes loaded into a fresh Keystore, every derived
        // key matches.
        let ks1 = Keystore::generate();
        let alice = AccountId::from_str("vault.alice.testnet").unwrap();
        let alice_master = [0xCC; 32];
        ks1.add_customer(alice.clone(), alice_master);

        let seed = "wallet:abc:near";
        let (sk1, vk1) = ks1.derive_keypair(Some(&alice), seed).unwrap();
        let path_1 = ks1.derive_secret_string(Some(&alice), "anything").unwrap();

        // Simulate restart: keep default_master; re-load alice's
        // master (in production this comes from MPC CKD).
        let master_hex = ks1.default_master_hex();
        let ks2 = Keystore::from_master_secret_hex(&master_hex).unwrap();
        ks2.add_customer(alice.clone(), alice_master);

        let (sk2, vk2) = ks2.derive_keypair(Some(&alice), seed).unwrap();
        let path_2 = ks2.derive_secret_string(Some(&alice), "anything").unwrap();

        assert_eq!(sk1.to_bytes(), sk2.to_bytes());
        assert_eq!(vk1.as_bytes(), vk2.as_bytes());
        assert_eq!(path_1, path_2);
    }
}
