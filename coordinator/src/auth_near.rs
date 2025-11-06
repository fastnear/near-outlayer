//! NEAR-Signed Authentication
//!
//! Phase 1 Hardening: Authenticate requests using ed25519 signatures.
//!
//! ## Protocol
//!
//! Client signs: `method|path|body_sha256|timestamp`
//! Server verifies signature against registered public key.
//!
//! ## Headers
//! - `X-Near-Account`: AccountId (e.g., "worker.near")
//! - `X-Near-Signature`: base58-encoded ed25519 signature
//! - `X-Near-Timestamp`: Unix timestamp in seconds
//!
//! ## Security Properties
//! - Replay protection via timestamp validation (±5 minute window)
//! - Integrity protection via body hash
//! - Unforgeable (ed25519 signatures)
//!
//! ## Configuration
//! - `REQUIRE_NEAR_SIGNED=true` - Enable NEAR-signed auth (production)
//! - `REQUIRE_NEAR_SIGNED=false` - Disable (development only)

use axum::{
    extract::{Request, State},
    http::StatusCode,
    middleware::Next,
    response::Response,
};
use ed25519_dalek::{PublicKey, Signature, Verifier};
use sha2::{Digest, Sha256};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::{debug, warn};

/// NEAR account ID → ed25519 public key registry
pub struct NearAuthRegistry {
    /// Map of account_id → base58-encoded public key
    allowed_accounts: std::collections::HashMap<String, String>,
}

impl NearAuthRegistry {
    /// Create new registry with allowed accounts
    pub fn new(accounts: Vec<(String, String)>) -> Self {
        Self {
            allowed_accounts: accounts.into_iter().collect(),
        }
    }

    /// Get public key for account (if registered)
    pub fn get_pubkey(&self, account_id: &str) -> Option<&str> {
        self.allowed_accounts.get(account_id).map(|s| s.as_str())
    }
}

/// Verify NEAR-signed request
///
/// Returns Ok(account_id) if valid, Err(error_message) otherwise
pub fn verify_near_signature(
    method: &str,
    path: &str,
    body: &[u8],
    account_id: &str,
    signature_b58: &str,
    timestamp_str: &str,
    registry: &NearAuthRegistry,
) -> Result<String, String> {
    // 1. Validate timestamp (prevent replay attacks)
    let timestamp: u64 = timestamp_str
        .parse()
        .map_err(|_| "Invalid timestamp format".to_string())?;

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();

    // Allow ±5 minute window
    let time_diff = if now > timestamp {
        now - timestamp
    } else {
        timestamp - now
    };

    if time_diff > 300 {
        return Err(format!(
            "Timestamp too old/new (diff: {}s, max: 300s)",
            time_diff
        ));
    }

    // 2. Get registered public key for this account
    let pubkey_b58 = registry
        .get_pubkey(account_id)
        .ok_or_else(|| format!("Account '{}' not registered", account_id))?;

    // 3. Decode public key from base58
    let pubkey_bytes = bs58::decode(pubkey_b58)
        .into_vec()
        .map_err(|e| format!("Invalid base58 public key: {}", e))?;

    let pubkey = PublicKey::from_bytes(&pubkey_bytes)
        .map_err(|e| format!("Invalid ed25519 public key: {}", e))?;

    // 4. Compute body hash
    let mut hasher = Sha256::new();
    hasher.update(body);
    let body_hash = hasher.finalize();
    let body_hash_hex = hex::encode(body_hash);

    // 5. Reconstruct signed message: method|path|body_hash|timestamp
    let message = format!("{}|{}|{}|{}", method, path, body_hash_hex, timestamp_str);

    // 6. Decode signature from base58
    let sig_bytes = bs58::decode(signature_b58)
        .into_vec()
        .map_err(|e| format!("Invalid base58 signature: {}", e))?;

    let signature = Signature::from_bytes(&sig_bytes)
        .map_err(|e| format!("Invalid ed25519 signature: {}", e))?;

    // 7. Verify signature
    pubkey
        .verify(message.as_bytes(), &signature)
        .map_err(|_| "Signature verification failed".to_string())?;

    debug!(
        "✅ NEAR-signed request verified: account={}, method={}, path={}",
        account_id, method, path
    );

    Ok(account_id.to_string())
}

/// Axum middleware for NEAR-signed authentication
pub async fn near_auth_middleware(
    State(registry): State<Arc<NearAuthRegistry>>,
    mut req: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    // Extract headers
    let account_id = req
        .headers()
        .get("X-Near-Account")
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| {
            warn!("Missing X-Near-Account header");
            StatusCode::UNAUTHORIZED
        })?;

    let signature = req
        .headers()
        .get("X-Near-Signature")
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| {
            warn!("Missing X-Near-Signature header");
            StatusCode::UNAUTHORIZED
        })?;

    let timestamp = req
        .headers()
        .get("X-Near-Timestamp")
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| {
            warn!("Missing X-Near-Timestamp header");
            StatusCode::UNAUTHORIZED
        })?;

    // Extract method and path
    let method = req.method().as_str();
    let path = req.uri().path();

    // Extract body (need to consume and re-insert)
    let (parts, body) = req.into_parts();
    let body_bytes = axum::body::to_bytes(body, usize::MAX)
        .await
        .map_err(|e| {
            warn!("Failed to read request body: {}", e);
            StatusCode::BAD_REQUEST
        })?;

    // Verify signature
    match verify_near_signature(
        method,
        path,
        &body_bytes,
        account_id,
        signature,
        timestamp,
        &registry,
    ) {
        Ok(verified_account) => {
            debug!("Request authenticated as: {}", verified_account);

            // Store verified account in request extensions for handlers
            let mut req = Request::from_parts(parts, axum::body::Body::from(body_bytes));
            req.extensions_mut().insert(verified_account);

            Ok(next.run(req).await)
        }
        Err(error) => {
            warn!("NEAR signature verification failed: {}", error);
            Err(StatusCode::UNAUTHORIZED)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_registry_lookup() {
        let registry = NearAuthRegistry::new(vec![(
            "worker.near".to_string(),
            "ed25519:ABC123".to_string(),
        )]);

        assert_eq!(registry.get_pubkey("worker.near"), Some("ed25519:ABC123"));
        assert_eq!(registry.get_pubkey("unknown.near"), None);
    }

    #[test]
    fn test_timestamp_validation() {
        let registry = NearAuthRegistry::new(vec![]);
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        // Valid timestamp (current time)
        let result = verify_near_signature(
            "GET",
            "/test",
            b"",
            "test.near",
            "invalid_sig",
            &now.to_string(),
            &registry,
        );

        // Should fail on account lookup (not timestamp)
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not registered"));

        // Invalid timestamp (too old)
        let old_timestamp = now - 400; // 400 seconds ago
        let result = verify_near_signature(
            "GET",
            "/test",
            b"",
            "test.near",
            "invalid_sig",
            &old_timestamp.to_string(),
            &registry,
        );

        assert!(result.is_err());
        assert!(result.unwrap_err().contains("too old/new"));
    }
}
