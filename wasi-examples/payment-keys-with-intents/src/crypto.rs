use borsh::{BorshDeserialize, BorshSerialize};
use ed25519_dalek::{Signer, SigningKey, VerifyingKey};
use sha2::{Digest, Sha256};

/// NEP-413 Payload structure
#[derive(BorshSerialize, BorshDeserialize, Debug)]
pub struct Payload {
    pub message: String,
    pub nonce: [u8; 32],
    pub recipient: String,
    pub callback_url: Option<String>,
}

/// Sign a NEAR intent message using NEP-413 format
///
/// # Arguments
/// * `message` - JSON string of the intent message
/// * `nonce` - Base64-encoded 32-byte nonce
/// * `recipient` - Contract account ID (e.g. "intents.near")
/// * `private_key_base58` - Base58-encoded ed25519 private key (64 bytes)
///
/// # Returns
/// Tuple of (signature_base58, public_key_base58)
pub fn sign_nep413_intent(
    message: &str,
    nonce: &str,
    recipient: &str,
    private_key_base58: &str,
) -> Result<(String, String), Box<dyn std::error::Error>> {
    // Decode private key from base58 (should be 64 bytes)
    let private_key_bytes = bs58::decode(private_key_base58)
        .into_vec()
        .map_err(|e| format!("Failed to decode private key: {}", e))?;

    if private_key_bytes.len() != 64 {
        return Err(format!(
            "Invalid private key length: {} (expected 64)",
            private_key_bytes.len()
        )
        .into());
    }

    // Extract seed (first 32 bytes)
    let seed: [u8; 32] = private_key_bytes[..32]
        .try_into()
        .map_err(|_| "Failed to extract seed")?;

    // Create signing key
    let signing_key = SigningKey::from_bytes(&seed);
    let verifying_key = signing_key.verifying_key();

    // Decode and prepare nonce
    let nonce_bytes = base64::decode(nonce)
        .map_err(|e| format!("Failed to decode nonce: {}", e))?;

    let mut nonce_array = [0u8; 32];
    if nonce_bytes.len() > 32 {
        return Err("Nonce too long".into());
    }
    // Right-pad with zeros (as in Python code)
    nonce_array[..nonce_bytes.len()].copy_from_slice(&nonce_bytes);

    // Create payload
    let payload = Payload {
        message: message.to_string(),
        nonce: nonce_array,
        recipient: recipient.to_string(),
        callback_url: None,
    };

    // Serialize payload using Borsh
    let borsh_payload =
        borsh::to_vec(&payload).map_err(|e| format!("Failed to serialize payload: {}", e))?;

    // Create discriminant: 2^31 + 413
    let discriminant: u32 = 2_147_483_648 + 413; // 2^31 + 413
    let discriminant_bytes = discriminant.to_le_bytes();

    // Hash: sha256(discriminant || borsh_payload)
    let mut hasher = Sha256::new();
    hasher.update(&discriminant_bytes);
    hasher.update(&borsh_payload);
    let hash_to_sign = hasher.finalize();

    // Sign the hash
    let signature = signing_key.sign(&hash_to_sign);

    // Encode to base58
    let signature_base58 = bs58::encode(signature.to_bytes()).into_string();
    let public_key_base58 = bs58::encode(verifying_key.to_bytes()).into_string();

    Ok((signature_base58, public_key_base58))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_nep413_signing() {
        // This is a placeholder test
        // In production, you'd test with known test vectors
        let message = r#"{"signer_id":"test.near","deadline":"2025-01-01T00:00:00.000Z","intents":[]}"#;
        let nonce = base64::encode(&[0u8; 32]);
        let recipient = "intents.near";

        // Generate a test key
        let signing_key = SigningKey::from_bytes(&[1u8; 32]);
        let private_key_bytes = signing_key.to_bytes();
        let private_key_base58 = bs58::encode(&private_key_bytes).into_string();

        let result = sign_nep413_intent(message, &nonce, recipient, &private_key_base58);
        assert!(result.is_ok());
    }
}
