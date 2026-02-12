//! TEE Auth - shared challenge-response authentication for TEE workers
//!
//! Used by coordinator and keystore-worker to verify that a worker
//! possesses a private key registered on the register-contract via TDX attestation.
//!
//! Flow:
//! 1. Server generates random challenge via `generate_challenge()`
//! 2. Worker signs challenge with TEE private key
//! 3. Server verifies signature via `verify_signature()`
//! 4. Server checks key exists on register-contract via `check_access_key_on_contract()`

use ed25519_dalek::{Signature, VerifyingKey};
use rand::RngCore;

/// Generate a random 32-byte challenge as hex string (64 chars).
pub fn generate_challenge() -> String {
    let mut bytes = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut bytes);
    hex::encode(bytes)
}

/// Verify an ed25519 signature over a challenge.
///
/// # Arguments
/// * `public_key` - "ed25519:..." format (base58-encoded) or raw hex (64 chars)
/// * `challenge` - hex-encoded challenge string (as returned by `generate_challenge`)
/// * `signature` - hex-encoded ed25519 signature (128 hex chars = 64 bytes)
pub fn verify_signature(
    public_key: &str,
    challenge: &str,
    signature: &str,
) -> Result<(), TeeAuthError> {
    // Parse public key
    let pk_bytes = parse_public_key_bytes(public_key)?;
    let verifying_key = VerifyingKey::from_bytes(&pk_bytes)
        .map_err(|e| TeeAuthError::InvalidPublicKey(format!("ed25519 parse error: {}", e)))?;

    // Parse signature
    let sig_bytes = hex::decode(signature)
        .map_err(|e| TeeAuthError::InvalidSignature(format!("hex decode error: {}", e)))?;
    if sig_bytes.len() != 64 {
        return Err(TeeAuthError::InvalidSignature(format!(
            "expected 64 bytes, got {}",
            sig_bytes.len()
        )));
    }
    let sig = Signature::from_bytes(
        sig_bytes
            .as_slice()
            .try_into()
            .map_err(|_| TeeAuthError::InvalidSignature("slice conversion failed".into()))?,
    );

    // Parse challenge (verify the raw challenge bytes, not the hex string)
    let challenge_bytes = hex::decode(challenge)
        .map_err(|e| TeeAuthError::InvalidChallenge(format!("hex decode error: {}", e)))?;

    // Verify
    use ed25519_dalek::Verifier;
    verifying_key
        .verify(&challenge_bytes, &sig)
        .map_err(|_| TeeAuthError::SignatureVerificationFailed)?;

    Ok(())
}

/// Check if a public key exists as an access key on a NEAR account via RPC.
///
/// Uses `view_access_key` RPC query to check if the key is registered
/// on the register-contract account (proving it was TEE-attested).
///
/// # Arguments
/// * `client` - reqwest HTTP client
/// * `rpc_url` - NEAR RPC URL (e.g., "https://rpc.mainnet.near.org")
/// * `account_id` - register-contract account (e.g., "register.outlayer.near")
/// * `public_key` - "ed25519:..." format
pub async fn check_access_key_on_contract(
    client: &reqwest::Client,
    rpc_url: &str,
    account_id: &str,
    public_key: &str,
) -> Result<bool, TeeAuthError> {
    // Ensure key is in "ed25519:..." format for NEAR RPC
    let near_key = if public_key.starts_with("ed25519:") {
        public_key.to_string()
    } else {
        // Assume hex-encoded raw bytes, convert to base58 with prefix
        let bytes = hex::decode(public_key)
            .map_err(|e| TeeAuthError::InvalidPublicKey(format!("hex decode: {}", e)))?;
        format!("ed25519:{}", bs58::encode(&bytes).into_string())
    };

    let body = serde_json::json!({
        "jsonrpc": "2.0",
        "id": "tee-auth",
        "method": "query",
        "params": {
            "request_type": "view_access_key",
            "finality": "optimistic",
            "account_id": account_id,
            "public_key": near_key
        }
    });

    let response = client
        .post(rpc_url)
        .json(&body)
        .send()
        .await
        .map_err(|e| TeeAuthError::NearRpcError(format!("request failed: {}", e)))?;

    let json: serde_json::Value = response
        .json()
        .await
        .map_err(|e| TeeAuthError::NearRpcError(format!("response parse failed: {}", e)))?;

    // If there's an error field, the key doesn't exist
    if let Some(error) = json.get("error") {
        // Check structured error first: error.cause.name == "UNKNOWN_ACCESS_KEY"
        let is_unknown_key = error
            .get("cause")
            .and_then(|c| c.get("name"))
            .and_then(|n| n.as_str())
            .map(|name| name == "UNKNOWN_ACCESS_KEY")
            .unwrap_or(false);

        if is_unknown_key {
            return Ok(false);
        }

        // Fallback: check error.data string for older RPC versions
        let error_data = error
            .get("data")
            .and_then(|d| d.as_str())
            .unwrap_or("");
        if error_data.contains("does not exist") || error_data.contains("doesn't exist") {
            return Ok(false);
        }

        return Err(TeeAuthError::NearRpcError(format!("RPC error: {}", error)));
    }

    // Check that result exists and has permission field (valid access key)
    if json.get("result").and_then(|r| r.get("permission")).is_some() {
        Ok(true)
    } else {
        Ok(false)
    }
}

/// Check if a public key exists on a NEAR account, with retry for finality lag.
///
/// Creates its own HTTP client to avoid reqwest version conflicts between crates.
/// Retries up to 3 times with 3s delay when key is not found (may not be visible yet).
///
/// # Arguments
/// * `rpc_url` - NEAR RPC URL (e.g., "https://rpc.mainnet.near.org")
/// * `account_id` - operator account (e.g., "worker.outlayer.near")
/// * `public_key` - "ed25519:..." format
pub async fn check_access_key_with_retry(
    rpc_url: &str,
    account_id: &str,
    public_key: &str,
) -> Result<bool, TeeAuthError> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .map_err(|e| TeeAuthError::NearRpcError(format!("HTTP client error: {}", e)))?;

    const MAX_RETRIES: u32 = 3;
    const RETRY_DELAY: std::time::Duration = std::time::Duration::from_secs(3);

    for attempt in 1..=MAX_RETRIES {
        match check_access_key_on_contract(&client, rpc_url, account_id, public_key).await {
            Ok(true) => return Ok(true),
            Ok(false) if attempt < MAX_RETRIES => {
                tracing::info!(
                    attempt = attempt,
                    public_key = %public_key,
                    account_id = %account_id,
                    "Key not yet visible on-chain, retrying in {}s...",
                    RETRY_DELAY.as_secs()
                );
                tokio::time::sleep(RETRY_DELAY).await;
            }
            Ok(false) => return Ok(false),
            Err(e) if attempt < MAX_RETRIES => {
                tracing::warn!(
                    attempt = attempt,
                    error = %e,
                    "NEAR RPC check failed, retrying..."
                );
                tokio::time::sleep(RETRY_DELAY).await;
            }
            Err(e) => return Err(e),
        }
    }

    Ok(false)
}

/// Parse public key bytes from "ed25519:..." (base58) or raw hex format.
fn parse_public_key_bytes(public_key: &str) -> Result<[u8; 32], TeeAuthError> {
    let raw_bytes = if let Some(b58) = public_key.strip_prefix("ed25519:") {
        bs58::decode(b58)
            .into_vec()
            .map_err(|e| TeeAuthError::InvalidPublicKey(format!("base58 decode: {}", e)))?
    } else if public_key.len() == 64 {
        // Assume hex
        hex::decode(public_key)
            .map_err(|e| TeeAuthError::InvalidPublicKey(format!("hex decode: {}", e)))?
    } else {
        return Err(TeeAuthError::InvalidPublicKey(format!(
            "unrecognized format (expected 'ed25519:...' or 64 hex chars): {}",
            public_key
        )));
    };

    if raw_bytes.len() != 32 {
        return Err(TeeAuthError::InvalidPublicKey(format!(
            "expected 32 bytes, got {}",
            raw_bytes.len()
        )));
    }

    let mut arr = [0u8; 32];
    arr.copy_from_slice(&raw_bytes);
    Ok(arr)
}

#[derive(Debug, thiserror::Error)]
pub enum TeeAuthError {
    #[error("invalid public key: {0}")]
    InvalidPublicKey(String),
    #[error("invalid signature: {0}")]
    InvalidSignature(String),
    #[error("invalid challenge: {0}")]
    InvalidChallenge(String),
    #[error("signature verification failed")]
    SignatureVerificationFailed,
    #[error("NEAR RPC error: {0}")]
    NearRpcError(String),
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::SigningKey;

    #[test]
    fn test_challenge_generation() {
        let c1 = generate_challenge();
        let c2 = generate_challenge();
        assert_eq!(c1.len(), 64); // 32 bytes = 64 hex chars
        assert_ne!(c1, c2); // Should be random
    }

    #[test]
    fn test_sign_and_verify() {
        let signing_key = SigningKey::generate(&mut rand::thread_rng());
        let verifying_key = signing_key.verifying_key();
        let pub_key_hex = hex::encode(verifying_key.as_bytes());

        let challenge = generate_challenge();
        let challenge_bytes = hex::decode(&challenge).unwrap();

        use ed25519_dalek::Signer;
        let signature = signing_key.sign(&challenge_bytes);
        let sig_hex = hex::encode(signature.to_bytes());

        // Should succeed
        verify_signature(&pub_key_hex, &challenge, &sig_hex).unwrap();
    }

    #[test]
    fn test_verify_wrong_signature() {
        let signing_key = SigningKey::generate(&mut rand::thread_rng());
        let verifying_key = signing_key.verifying_key();
        let pub_key_hex = hex::encode(verifying_key.as_bytes());

        let challenge = generate_challenge();

        // Wrong signature (all zeros)
        let bad_sig = hex::encode([0u8; 64]);

        let result = verify_signature(&pub_key_hex, &challenge, &bad_sig);
        assert!(result.is_err());
    }

    #[test]
    fn test_verify_wrong_key() {
        let signing_key = SigningKey::generate(&mut rand::thread_rng());
        let other_key = SigningKey::generate(&mut rand::thread_rng());
        let other_pub_hex = hex::encode(other_key.verifying_key().as_bytes());

        let challenge = generate_challenge();
        let challenge_bytes = hex::decode(&challenge).unwrap();

        use ed25519_dalek::Signer;
        let signature = signing_key.sign(&challenge_bytes);
        let sig_hex = hex::encode(signature.to_bytes());

        // Verify with wrong public key
        let result = verify_signature(&other_pub_hex, &challenge, &sig_hex);
        assert!(result.is_err());
    }

    #[test]
    fn test_ed25519_prefix_format() {
        let signing_key = SigningKey::generate(&mut rand::thread_rng());
        let verifying_key = signing_key.verifying_key();
        let pub_key_b58 = format!("ed25519:{}", bs58::encode(verifying_key.as_bytes()).into_string());

        let challenge = generate_challenge();
        let challenge_bytes = hex::decode(&challenge).unwrap();

        use ed25519_dalek::Signer;
        let signature = signing_key.sign(&challenge_bytes);
        let sig_hex = hex::encode(signature.to_bytes());

        verify_signature(&pub_key_b58, &challenge, &sig_hex).unwrap();
    }

    #[test]
    fn test_parse_public_key_hex_and_bs58() {
        let signing_key = SigningKey::generate(&mut rand::thread_rng());
        let pk_bytes = signing_key.verifying_key().to_bytes();
        let pk_hex = hex::encode(pk_bytes);
        let pk_b58 = format!("ed25519:{}", bs58::encode(&pk_bytes).into_string());

        // Both formats should parse to the same bytes
        let from_hex = parse_public_key_bytes(&pk_hex).unwrap();
        let from_b58 = parse_public_key_bytes(&pk_b58).unwrap();
        assert_eq!(from_hex, pk_bytes);
        assert_eq!(from_b58, pk_bytes);
    }
}
