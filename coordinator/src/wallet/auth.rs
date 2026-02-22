//! Wallet authentication via API keys and internal worker tokens.
//!
//! API keys are bearer tokens — only SHA-256 hashes are stored in the database.
//! The plaintext key is returned once at registration and never persisted.
//!
//! Auth paths:
//! 1. Authorization: Bearer <key> — external agents (primary, DB lookup)
//! 2. X-Internal-Wallet-Auth + X-Wallet-Id — WASI arks via worker (trusted token hash)
//!
//! Signature utilities (verify_wallet_signature, verify_ed25519, verify_secp256k1)
//! are kept for future use: policy handoff (MetaMask/NEAR wallet signing),
//! multisig approvals from external signers, dashboard wallet management.

use super::types::WalletError;
use axum::http::HeaderMap;
use ed25519_dalek::{Signature as Ed25519Signature, VerifyingKey};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::RwLock;

/// Maximum allowed clock skew in seconds (for signature-based auth utilities)
#[allow(dead_code)]
const MAX_TIMESTAMP_SKEW: u64 = 30;

/// TTL for API key cache entries (seconds)
const API_KEY_CACHE_TTL_SECS: u64 = 60;

/// In-memory cache for API key → wallet_id lookups.
/// Avoids a DB JOIN on every authenticated request.
#[derive(Clone)]
pub struct ApiKeyCache {
    /// key_hash → (wallet_id, cached_at)
    entries: Arc<RwLock<HashMap<String, (String, Instant)>>>,
}

impl ApiKeyCache {
    pub fn new() -> Self {
        Self {
            entries: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Get cached wallet_id for a key_hash, or None if expired/missing.
    async fn get(&self, key_hash: &str) -> Option<String> {
        let entries = self.entries.read().await;
        if let Some((wallet_id, cached_at)) = entries.get(key_hash) {
            if cached_at.elapsed().as_secs() < API_KEY_CACHE_TTL_SECS {
                return Some(wallet_id.clone());
            }
        }
        None
    }

    /// Cache a key_hash → wallet_id mapping.
    async fn set(&self, key_hash: String, wallet_id: String) {
        let mut entries = self.entries.write().await;
        // Lazy cleanup: cap at 10K entries to prevent unbounded growth
        if entries.len() > 10_000 {
            let now = Instant::now();
            entries.retain(|_, (_, cached_at)| {
                now.duration_since(*cached_at).as_secs() < API_KEY_CACHE_TTL_SECS
            });
        }
        entries.insert(key_hash, (wallet_id, Instant::now()));
    }

    /// Remove a key_hash from cache (called on revocation).
    #[allow(dead_code)]
    pub async fn invalidate(&self, key_hash: &str) {
        let mut entries = self.entries.write().await;
        entries.remove(key_hash);
    }

    /// Remove all entries for a wallet_id (e.g. on key rotation).
    pub async fn invalidate_wallet(&self, wallet_id: &str) {
        let mut entries = self.entries.write().await;
        entries.retain(|_, (wid, _)| wid != wallet_id);
    }
}

/// Parsed wallet auth — result of authentication
#[derive(Debug, Clone)]
pub struct WalletAuth {
    pub wallet_id: String,
    pub idempotency_key: Option<String>,
    /// True if auth was via internal worker token (skip signature verification)
    pub is_internal: bool,
}

/// Parse wallet ID from header value. Validates UUID format.
pub fn parse_wallet_id(wallet_id: &str) -> Result<(), WalletError> {
    if uuid::Uuid::parse_str(wallet_id).is_err() {
        return Err(WalletError::InvalidWalletIdFormat(
            format!("Invalid wallet ID: expected UUID, got '{}'", wallet_id),
        ));
    }
    Ok(())
}

/// Authenticate via API key (`Authorization: Bearer <key>` header).
///
/// Returns `Ok(Some(auth))` if key is present and valid,
/// `Ok(None)` if header is absent, `Err` if key is invalid/revoked.
pub async fn extract_api_key_auth(
    headers: &HeaderMap,
    db: &sqlx::PgPool,
    cache: &ApiKeyCache,
) -> Result<Option<WalletAuth>, WalletError> {
    let api_key = match headers.get("authorization").and_then(|v| v.to_str().ok()) {
        Some(header) => match header.strip_prefix("Bearer ") {
            Some(token) => token.to_string(),
            None => return Ok(None),
        },
        None => return Ok(None),
    };

    // Hash the API key
    let key_hash = {
        let mut hasher = Sha256::new();
        hasher.update(api_key.as_bytes());
        hex::encode(hasher.finalize())
    };

    // Try cache first
    let wallet_id = if let Some(cached) = cache.get(&key_hash).await {
        cached
    } else {
        // Lookup in DB: direct query on wallet_api_keys
        let row = sqlx::query_as::<_, (String,)>(
            "SELECT wallet_id FROM wallet_api_keys WHERE key_hash = $1 AND revoked_at IS NULL",
        )
        .bind(&key_hash)
        .fetch_optional(db)
        .await
        .map_err(|e| WalletError::InternalError(format!("DB error: {}", e)))?;

        let wid = match row {
            Some((wid,)) => wid,
            None => return Err(WalletError::InvalidApiKey),
        };

        // Cache the result
        cache.set(key_hash, wid.clone()).await;
        wid
    };

    let idempotency_key = headers
        .get("x-idempotency-key")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());

    Ok(Some(WalletAuth {
        wallet_id,
        idempotency_key,
        is_internal: false,
    }))
}

/// Authenticate via internal worker token (X-Internal-Wallet-Auth + X-Wallet-Id).
///
/// Used by WASI arks calling wallet operations through the worker host functions.
pub fn extract_internal_auth(
    headers: &HeaderMap,
    allowed_worker_token_hashes: &[String],
) -> Result<Option<WalletAuth>, WalletError> {
    let internal_token = match headers
        .get("x-internal-wallet-auth")
        .and_then(|v| v.to_str().ok())
    {
        Some(t) => t,
        None => return Ok(None),
    };

    let wallet_id = headers
        .get("x-wallet-id")
        .and_then(|v| v.to_str().ok())
        .ok_or(WalletError::MissingWalletId)?
        .to_string();

    // Validate wallet_id format
    parse_wallet_id(&wallet_id)?;

    let token_hash = {
        let mut hasher = Sha256::new();
        hasher.update(internal_token.as_bytes());
        hex::encode(hasher.finalize())
    };

    if allowed_worker_token_hashes
        .iter()
        .any(|h| h == &token_hash)
    {
        return Ok(Some(WalletAuth {
            wallet_id,
            idempotency_key: None,
            is_internal: true,
        }));
    }

    Err(WalletError::InvalidSignature(
        "Invalid internal wallet auth token".to_string(),
    ))
}

/// Unified authentication: tries internal worker token first (trusted, no DB query),
/// then API key (cached DB lookup), else error.
pub async fn authenticate(
    headers: &HeaderMap,
    allowed_worker_token_hashes: &[String],
    db: &sqlx::PgPool,
    api_key_cache: &ApiKeyCache,
) -> Result<WalletAuth, WalletError> {
    // 1. Try X-Internal-Wallet-Auth + X-Wallet-Id (trusted, no DB query)
    if let Some(auth) = extract_internal_auth(headers, allowed_worker_token_hashes)? {
        return Ok(auth);
    }

    // 2. Try Authorization: Bearer (cached DB lookup)
    if let Some(auth) = extract_api_key_auth(headers, db, api_key_cache).await? {
        return Ok(auth);
    }

    // 3. Nothing provided
    Err(WalletError::MissingAuth)
}

// ============================================================================
// Signature verification utilities (kept for future handoff/approvals)
// ============================================================================

/// Verify a wallet signature (Ed25519 or secp256k1).
///
/// Used for: policy handoff (MetaMask/NEAR wallet), multisig approvals,
/// dashboard wallet management.
///
/// For POST: message = timestamp + ":" + body_json
/// For GET: message = timestamp + ":" + path_with_query
#[allow(dead_code)]
pub fn verify_wallet_signature(
    wallet_id: &str,
    signature_hex: &str,
    timestamp: u64,
    message_payload: &str,
) -> Result<(), WalletError> {
    // Check timestamp freshness
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();

    let diff = if now > timestamp {
        now - timestamp
    } else {
        timestamp - now
    };

    if diff > MAX_TIMESTAMP_SKEW {
        return Err(WalletError::TimestampExpired);
    }

    // Build signed message: timestamp + ":" + payload
    let message = format!("{}:{}", timestamp, message_payload);

    let parts: Vec<&str> = wallet_id.splitn(2, ':').collect();
    if parts.len() != 2 {
        return Err(WalletError::InvalidWalletIdFormat(
            "Expected format: 'ed25519:<hex>' or 'secp256k1:<hex>'".to_string(),
        ));
    }
    let key_type = parts[0];
    let pubkey_bytes = hex::decode(parts[1]).map_err(|_| {
        WalletError::InvalidWalletIdFormat("Invalid hex encoding in wallet ID".to_string())
    })?;

    let sig_bytes = hex::decode(signature_hex).map_err(|_| {
        WalletError::InvalidSignature("Invalid hex encoding in signature".to_string())
    })?;

    match key_type {
        "ed25519" => verify_ed25519(&pubkey_bytes, &sig_bytes, message.as_bytes()),
        "secp256k1" => verify_secp256k1(&pubkey_bytes, &sig_bytes, message.as_bytes()),
        _ => Err(WalletError::InvalidSignature(format!(
            "Unsupported key type: {}",
            key_type
        ))),
    }
}

/// Verify an Ed25519 signature.
#[allow(dead_code)]
fn verify_ed25519(pubkey: &[u8], signature: &[u8], message: &[u8]) -> Result<(), WalletError> {
    if signature.len() != 64 {
        return Err(WalletError::InvalidSignature(format!(
            "Ed25519 signature must be 64 bytes, got {}",
            signature.len()
        )));
    }

    let verifying_key = VerifyingKey::from_bytes(pubkey.try_into().unwrap()).map_err(|e| {
        WalletError::InvalidSignature(format!("Invalid Ed25519 public key: {}", e))
    })?;

    let sig = Ed25519Signature::from_bytes(signature.try_into().unwrap());

    verifying_key
        .verify_strict(message, &sig)
        .map_err(|_| WalletError::InvalidSignature("Ed25519 signature verification failed".to_string()))
}

/// Verify a secp256k1 signature with recovery.
#[allow(dead_code)]
fn verify_secp256k1(pubkey: &[u8], signature: &[u8], message: &[u8]) -> Result<(), WalletError> {
    // Signature: 64 bytes (r || s) + 1 byte recovery id
    if signature.len() != 65 {
        return Err(WalletError::InvalidSignature(format!(
            "Secp256k1 signature must be 65 bytes, got {}",
            signature.len()
        )));
    }

    // Hash the message with SHA256 (secp256k1 signs hashes, not raw messages)
    let mut hasher = Sha256::new();
    hasher.update(message);
    let message_hash: [u8; 32] = hasher.finalize().into();

    let sig: [u8; 64] = signature[..64].try_into().unwrap();
    let v = signature[64];

    if v > 1 {
        return Err(WalletError::InvalidSignature(format!(
            "Recovery id must be 0 or 1, got {}",
            v
        )));
    }

    // Use k256 crate for secp256k1 recovery
    use k256::ecdsa::{RecoveryId, Signature as K256Signature, VerifyingKey as K256VerifyingKey};
    use k256::ecdsa::signature::hazmat::PrehashVerifier;

    let recovery_id = RecoveryId::new(v != 0, false);
    let k256_sig = K256Signature::from_bytes((&sig).into()).map_err(|e| {
        WalletError::InvalidSignature(format!("Invalid secp256k1 signature: {}", e))
    })?;

    let recovered_key =
        K256VerifyingKey::recover_from_prehash(&message_hash, &k256_sig, recovery_id).map_err(
            |e| WalletError::InvalidSignature(format!("Secp256k1 recovery failed: {}", e)),
        )?;

    // Compare compressed keys
    let recovered_compressed = recovered_key.to_encoded_point(true);
    if recovered_compressed.as_bytes() != pubkey {
        return Err(WalletError::InvalidSignature(
            "Recovered secp256k1 key does not match wallet ID".to_string(),
        ));
    }

    // Additional: verify signature with the public key
    let expected_key = K256VerifyingKey::from_sec1_bytes(pubkey).map_err(|e| {
        WalletError::InvalidSignature(format!("Invalid secp256k1 public key: {}", e))
    })?;

    expected_key
        .verify_prehash(&message_hash, &k256_sig)
        .map_err(|_| {
            WalletError::InvalidSignature("Secp256k1 signature verification failed".to_string())
        })
}

// ============================================================================
// NEP-413 signature verification (for multisig approvals via NEAR wallet)
// ============================================================================

/// NEP-413 payload (Borsh-serialized before signing)
#[derive(borsh::BorshSerialize)]
struct Nep413Payload {
    message: String,
    nonce: [u8; 32],
    recipient: String,
    callback_url: Option<String>,
}

/// NEP-413 tag: 2^31 + 413
const NEP413_TAG: u32 = 2147484061;

/// Verify a NEP-413 signed message (NEAR wallet signature).
///
/// The signed payload is: SHA256(NEP413_TAG_LE_BYTES || Borsh(Nep413Payload))
///
/// Parameters:
/// - `message` — the plaintext message that was signed
/// - `signature_base64` — base64-encoded 64-byte ed25519 signature
/// - `public_key` — NEAR format: `"ed25519:<base58>"`
/// - `nonce_base64` — base64-encoded 32-byte nonce
/// - `recipient` — the intended recipient (e.g. contract ID)
pub fn verify_nep413_signature(
    message: &str,
    signature_base64: &str,
    public_key: &str,
    nonce_base64: &str,
    recipient: &str,
) -> Result<(), WalletError> {
    use base64::Engine;
    use ed25519_dalek::Verifier;

    // Parse public key: "ed25519:base58..." → 32 bytes
    let pubkey_parts: Vec<&str> = public_key.split(':').collect();
    if pubkey_parts.len() != 2 || pubkey_parts[0] != "ed25519" {
        return Err(WalletError::InvalidSignature(
            "Invalid public key format, expected 'ed25519:<base58>'".to_string(),
        ));
    }
    let pubkey_bytes = bs58::decode(pubkey_parts[1])
        .into_vec()
        .map_err(|e| WalletError::InvalidSignature(format!("Failed to decode public key: {}", e)))?;
    if pubkey_bytes.len() != 32 {
        return Err(WalletError::InvalidSignature(format!(
            "Invalid public key length: {} (expected 32)",
            pubkey_bytes.len()
        )));
    }

    // Decode signature: base64 → 64 bytes
    let signature_bytes = base64::engine::general_purpose::STANDARD
        .decode(signature_base64)
        .map_err(|e| WalletError::InvalidSignature(format!("Failed to decode signature: {}", e)))?;
    if signature_bytes.len() != 64 {
        return Err(WalletError::InvalidSignature(format!(
            "Invalid signature length: {} (expected 64)",
            signature_bytes.len()
        )));
    }

    // Decode nonce: base64 → 32 bytes
    let nonce_bytes = base64::engine::general_purpose::STANDARD
        .decode(nonce_base64)
        .map_err(|e| WalletError::InvalidSignature(format!("Failed to decode nonce: {}", e)))?;
    if nonce_bytes.len() != 32 {
        return Err(WalletError::InvalidSignature(format!(
            "Invalid nonce length: {} (expected 32)",
            nonce_bytes.len()
        )));
    }
    let nonce_array: [u8; 32] = nonce_bytes
        .try_into()
        .map_err(|_| WalletError::InvalidSignature("Failed to convert nonce".to_string()))?;

    // Build NEP-413 payload and Borsh-serialize
    let payload = Nep413Payload {
        message: message.to_string(),
        nonce: nonce_array,
        recipient: recipient.to_string(),
        callback_url: None,
    };
    let payload_bytes = borsh::to_vec(&payload)
        .map_err(|e| WalletError::InvalidSignature(format!("Failed to serialize NEP-413 payload: {}", e)))?;

    // Hash: SHA256(tag_le || borsh_payload)
    let mut to_hash = Vec::with_capacity(4 + payload_bytes.len());
    to_hash.extend_from_slice(&NEP413_TAG.to_le_bytes());
    to_hash.extend_from_slice(&payload_bytes);
    let hash = Sha256::digest(&to_hash);

    // Verify ed25519 signature
    let verifying_key = VerifyingKey::from_bytes(
        <&[u8; 32]>::try_from(pubkey_bytes.as_slice())
            .map_err(|_| WalletError::InvalidSignature("Invalid public key bytes".to_string()))?,
    )
    .map_err(|e| WalletError::InvalidSignature(format!("Invalid public key: {}", e)))?;

    let signature = Ed25519Signature::from_bytes(
        <&[u8; 64]>::try_from(signature_bytes.as_slice())
            .map_err(|_| WalletError::InvalidSignature("Invalid signature bytes".to_string()))?,
    );

    verifying_key
        .verify(&hash, &signature)
        .map_err(|_| WalletError::InvalidSignature("NEP-413 signature verification failed".to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    // =========================================================================
    // parse_wallet_id tests (UUID validation)
    // =========================================================================

    #[test]
    fn test_parse_uuid_v4_valid() {
        assert!(parse_wallet_id("550e8400-e29b-41d4-a716-446655440000").is_ok());
    }

    #[test]
    fn test_parse_uuid_generated() {
        let id = uuid::Uuid::new_v4().to_string();
        assert!(parse_wallet_id(&id).is_ok());
    }

    #[test]
    fn test_parse_uuid_no_hyphens_valid() {
        // uuid crate accepts both formats
        assert!(parse_wallet_id("550e8400e29b41d4a716446655440000").is_ok());
    }

    #[test]
    fn test_parse_not_uuid() {
        assert!(matches!(
            parse_wallet_id("not-a-uuid"),
            Err(WalletError::InvalidWalletIdFormat(_))
        ));
    }

    #[test]
    fn test_parse_empty() {
        assert!(matches!(
            parse_wallet_id(""),
            Err(WalletError::InvalidWalletIdFormat(_))
        ));
    }

    #[test]
    fn test_parse_old_account_prefix_rejected() {
        assert!(matches!(
            parse_wallet_id("account:550e8400-e29b-41d4-a716-446655440000"),
            Err(WalletError::InvalidWalletIdFormat(_))
        ));
    }

    #[test]
    fn test_parse_ed25519_rejected() {
        let hex_key = "a".repeat(64);
        assert!(matches!(
            parse_wallet_id(&format!("ed25519:{}", hex_key)),
            Err(WalletError::InvalidWalletIdFormat(_))
        ));
    }

    // =========================================================================
    // extract_internal_auth tests
    // =========================================================================

    fn make_valid_wallet_id() -> String {
        "550e8400-e29b-41d4-a716-446655440000".to_string()
    }

    #[test]
    fn test_extract_internal_no_header() {
        let headers = HeaderMap::new();
        let result = extract_internal_auth(&headers, &[]);
        assert!(result.unwrap().is_none());
    }

    #[test]
    fn test_extract_internal_valid_token() {
        let token = "my-secret-worker-token";
        let token_hash = {
            use sha2::Digest;
            let mut hasher = Sha256::new();
            hasher.update(token.as_bytes());
            hex::encode(hasher.finalize())
        };

        let mut headers = HeaderMap::new();
        headers.insert(
            "x-wallet-id",
            make_valid_wallet_id().parse().unwrap(),
        );
        headers.insert("x-internal-wallet-auth", token.parse().unwrap());
        let auth = extract_internal_auth(&headers, &[token_hash])
            .unwrap()
            .unwrap();
        assert!(auth.is_internal);
    }

    #[test]
    fn test_extract_internal_invalid_token() {
        let mut headers = HeaderMap::new();
        headers.insert(
            "x-wallet-id",
            make_valid_wallet_id().parse().unwrap(),
        );
        headers.insert("x-internal-wallet-auth", "wrong-token".parse().unwrap());
        let result = extract_internal_auth(&headers, &["correct-hash".to_string()]);
        assert!(matches!(result, Err(WalletError::InvalidSignature(_))));
    }

    #[test]
    fn test_extract_internal_missing_wallet_id() {
        let mut headers = HeaderMap::new();
        headers.insert("x-internal-wallet-auth", "some-token".parse().unwrap());
        let result = extract_internal_auth(&headers, &[]);
        assert!(matches!(result, Err(WalletError::MissingWalletId)));
    }

    // =========================================================================
    // verify_wallet_signature tests (real crypto)
    // =========================================================================

    #[test]
    fn test_verify_ed25519_valid() {
        use ed25519_dalek::SigningKey;
        use rand::rngs::OsRng;

        let signing_key = SigningKey::generate(&mut OsRng);
        let verifying_key = signing_key.verifying_key();
        let pubkey_hex = hex::encode(verifying_key.as_bytes());
        let wallet_id = format!("ed25519:{}", pubkey_hex);

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let payload = "test-payload";
        let message = format!("{}:{}", now, payload);

        use ed25519_dalek::Signer;
        let sig = signing_key.sign(message.as_bytes());
        let sig_hex = hex::encode(sig.to_bytes());

        let result = verify_wallet_signature(&wallet_id, &sig_hex, now, payload);
        assert!(result.is_ok());
    }

    #[test]
    fn test_verify_ed25519_wrong_message() {
        use ed25519_dalek::SigningKey;
        use rand::rngs::OsRng;

        let signing_key = SigningKey::generate(&mut OsRng);
        let verifying_key = signing_key.verifying_key();
        let pubkey_hex = hex::encode(verifying_key.as_bytes());
        let wallet_id = format!("ed25519:{}", pubkey_hex);

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let message = format!("{}:{}", now, "original-payload");
        use ed25519_dalek::Signer;
        let sig = signing_key.sign(message.as_bytes());
        let sig_hex = hex::encode(sig.to_bytes());

        let result = verify_wallet_signature(&wallet_id, &sig_hex, now, "different-payload");
        assert!(matches!(result, Err(WalletError::InvalidSignature(_))));
    }

    #[test]
    fn test_verify_secp256k1_valid() {
        use k256::ecdsa::{SigningKey as K256SigningKey, signature::hazmat::PrehashSigner};
        use sha2::Digest;

        let signing_key = K256SigningKey::random(&mut rand::thread_rng());
        let verifying_key = signing_key.verifying_key();
        let compressed = verifying_key.to_encoded_point(true);
        let pubkey_hex = hex::encode(compressed.as_bytes());
        let wallet_id = format!("secp256k1:{}", pubkey_hex);

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let payload = "test-payload";
        let message = format!("{}:{}", now, payload);

        let mut hasher = Sha256::new();
        hasher.update(message.as_bytes());
        let message_hash: [u8; 32] = hasher.finalize().into();

        let (sig, recovery_id) = signing_key.sign_prehash(&message_hash).unwrap();
        let mut sig_bytes = sig.to_bytes().to_vec();
        sig_bytes.push(recovery_id.to_byte());
        let sig_hex = hex::encode(&sig_bytes);

        let result = verify_wallet_signature(&wallet_id, &sig_hex, now, payload);
        assert!(result.is_ok());
    }

    #[test]
    fn test_verify_secp256k1_wrong_key() {
        use k256::ecdsa::{SigningKey as K256SigningKey, signature::hazmat::PrehashSigner};
        use sha2::Digest;

        let signing_key = K256SigningKey::random(&mut rand::thread_rng());
        let other_key = K256SigningKey::random(&mut rand::thread_rng());
        let other_verifying = other_key.verifying_key();
        let compressed = other_verifying.to_encoded_point(true);
        let pubkey_hex = hex::encode(compressed.as_bytes());
        let wallet_id = format!("secp256k1:{}", pubkey_hex);

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let payload = "test-payload";
        let message = format!("{}:{}", now, payload);

        let mut hasher = Sha256::new();
        hasher.update(message.as_bytes());
        let message_hash: [u8; 32] = hasher.finalize().into();

        let (sig, recovery_id) = signing_key.sign_prehash(&message_hash).unwrap();
        let mut sig_bytes = sig.to_bytes().to_vec();
        sig_bytes.push(recovery_id.to_byte());
        let sig_hex = hex::encode(&sig_bytes);

        let result = verify_wallet_signature(&wallet_id, &sig_hex, now, payload);
        assert!(matches!(result, Err(WalletError::InvalidSignature(_))));
    }

    #[test]
    fn test_verify_timestamp_expired() {
        use ed25519_dalek::SigningKey;
        use rand::rngs::OsRng;

        let signing_key = SigningKey::generate(&mut OsRng);
        let verifying_key = signing_key.verifying_key();
        let pubkey_hex = hex::encode(verifying_key.as_bytes());
        let wallet_id = format!("ed25519:{}", pubkey_hex);

        let old_timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
            - 60;
        let payload = "test-payload";
        let message = format!("{}:{}", old_timestamp, payload);

        use ed25519_dalek::Signer;
        let sig = signing_key.sign(message.as_bytes());
        let sig_hex = hex::encode(sig.to_bytes());

        let result = verify_wallet_signature(&wallet_id, &sig_hex, old_timestamp, payload);
        assert!(matches!(result, Err(WalletError::TimestampExpired)));
    }

    // =========================================================================
    // NEP-413 verify tests
    // =========================================================================

    #[test]
    fn test_nep413_valid_signature() {
        use ed25519_dalek::{SigningKey, Signer};
        use rand::rngs::OsRng;
        use base64::Engine;

        let signing_key = SigningKey::generate(&mut OsRng);
        let verifying_key = signing_key.verifying_key();
        let public_key = format!("ed25519:{}", bs58::encode(verifying_key.as_bytes()).into_string());

        let message = "approve:some-approval-id:some-request-hash";
        let nonce = [42u8; 32];
        let nonce_base64 = base64::engine::general_purpose::STANDARD.encode(&nonce);
        let recipient = "outlayer.near";

        // Build the NEP-413 payload exactly as verify_nep413_signature expects
        let payload = Nep413Payload {
            message: message.to_string(),
            nonce,
            recipient: recipient.to_string(),
            callback_url: None,
        };
        let payload_bytes = borsh::to_vec(&payload).unwrap();

        let mut to_hash = Vec::with_capacity(4 + payload_bytes.len());
        to_hash.extend_from_slice(&NEP413_TAG.to_le_bytes());
        to_hash.extend_from_slice(&payload_bytes);
        let hash = Sha256::digest(&to_hash);

        let signature = signing_key.sign(&hash);
        let sig_base64 = base64::engine::general_purpose::STANDARD.encode(signature.to_bytes());

        let result = verify_nep413_signature(message, &sig_base64, &public_key, &nonce_base64, recipient);
        assert!(result.is_ok(), "Expected Ok, got {:?}", result);
    }

    #[test]
    fn test_nep413_wrong_message() {
        use ed25519_dalek::{SigningKey, Signer};
        use rand::rngs::OsRng;
        use base64::Engine;

        let signing_key = SigningKey::generate(&mut OsRng);
        let verifying_key = signing_key.verifying_key();
        let public_key = format!("ed25519:{}", bs58::encode(verifying_key.as_bytes()).into_string());

        let nonce = [42u8; 32];
        let nonce_base64 = base64::engine::general_purpose::STANDARD.encode(&nonce);
        let recipient = "outlayer.near";

        // Sign "approve:id1:hash1"
        let payload = Nep413Payload {
            message: "approve:id1:hash1".to_string(),
            nonce,
            recipient: recipient.to_string(),
            callback_url: None,
        };
        let payload_bytes = borsh::to_vec(&payload).unwrap();
        let mut to_hash = Vec::with_capacity(4 + payload_bytes.len());
        to_hash.extend_from_slice(&NEP413_TAG.to_le_bytes());
        to_hash.extend_from_slice(&payload_bytes);
        let hash = Sha256::digest(&to_hash);
        let signature = signing_key.sign(&hash);
        let sig_base64 = base64::engine::general_purpose::STANDARD.encode(signature.to_bytes());

        // Verify with different message
        let result = verify_nep413_signature("approve:id2:hash2", &sig_base64, &public_key, &nonce_base64, recipient);
        assert!(matches!(result, Err(WalletError::InvalidSignature(_))));
    }

    #[test]
    fn test_nep413_bad_public_key_format() {
        let result = verify_nep413_signature("msg", "AAAA", "not-a-key", "AAAA", "r");
        assert!(matches!(result, Err(WalletError::InvalidSignature(_))));
    }
}
