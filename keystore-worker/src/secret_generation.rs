//! Secret generation module
//!
//! Generates cryptographically secure secrets based on user-specified types.
//! Used when user requests auto-generation instead of providing secrets manually.

use anyhow::Result;
use ed25519_dalek::SigningKey;
use rand::{rngs::OsRng, RngCore, Rng};

/// Prefix for generation directives in secrets JSON
pub const GENERATION_PREFIX: &str = "generate_outlayer_secret:";

/// Check if a value is a generation directive
pub fn is_generation_directive(value: &str) -> bool {
    value.starts_with(GENERATION_PREFIX)
}

/// Generate a secret based on specification string
///
/// # Format
/// `generate_outlayer_secret:TYPE[:PARAM]`
///
/// # Types
/// - `hex32` - 32 bytes in hex (64 chars) - for HKDF seeds
/// - `hex16` - 16 bytes in hex (32 chars)
/// - `hex64` - 64 bytes in hex (128 chars)
/// - `ed25519` - ED25519 private key in NEAR format (ed25519:base58)
/// - `ed25519_seed` - ED25519 seed (64 hex chars)
/// - `password` - Alphanumeric password, 32 chars default
/// - `password:N` - Alphanumeric password, N chars
///
/// # Examples
/// ```
/// let key = generate_secret("generate_outlayer_secret:hex32")?;
/// // Returns: "a1b2c3d4..." (64 hex chars)
///
/// let pass = generate_secret("generate_outlayer_secret:password:64")?;
/// // Returns: "Abc123XyZ..." (64 alphanumeric chars)
/// ```
pub fn generate_secret(spec: &str) -> Result<String> {
    // Remove prefix
    let spec_without_prefix = spec
        .strip_prefix(GENERATION_PREFIX)
        .ok_or_else(|| anyhow::anyhow!("Invalid generation spec: missing prefix"))?;

    // Parse type and optional parameter
    let parts: Vec<&str> = spec_without_prefix.split(':').collect();
    let secret_type = parts[0];
    let param = parts.get(1).copied();

    match secret_type {
        "hex32" => generate_hex_bytes(32),
        "hex16" => generate_hex_bytes(16),
        "hex64" => generate_hex_bytes(64),
        "ed25519" => generate_ed25519_key(),
        "ed25519_seed" => generate_hex_bytes(32), // ED25519 seed is 32 bytes
        "password" => {
            let length = if let Some(p) = param {
                p.parse::<usize>()
                    .map_err(|_| anyhow::anyhow!("Invalid password length: {}", p))?
            } else {
                32 // default
            };
            generate_password(length)
        }
        _ => Err(anyhow::anyhow!("Unknown secret type: {}", secret_type)),
    }
}

/// Generate N random bytes and return as hex string
fn generate_hex_bytes(n: usize) -> Result<String> {
    let mut bytes = vec![0u8; n];
    OsRng.fill_bytes(&mut bytes);
    Ok(hex::encode(bytes))
}

/// Generate ED25519 key in NEAR format (ed25519:base58...)
fn generate_ed25519_key() -> Result<String> {
    let signing_key = SigningKey::generate(&mut OsRng);
    let secret_bytes = signing_key.to_bytes();

    // NEAR format: "ed25519:" + base58(secret_key)
    let base58_key = bs58::encode(&secret_bytes).into_string();
    Ok(format!("ed25519:{}", base58_key))
}

/// Generate alphanumeric password
fn generate_password(length: usize) -> Result<String> {
    const CHARSET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";

    let mut rng = OsRng;
    let password: String = (0..length)
        .map(|_| {
            let idx = rng.gen_range(0..CHARSET.len());
            CHARSET[idx] as char
        })
        .collect();

    Ok(password)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_generation_directive() {
        assert!(is_generation_directive("generate_outlayer_secret:hex32"));
        assert!(is_generation_directive("generate_outlayer_secret:password"));
        assert!(!is_generation_directive("my-api-key"));
        assert!(!is_generation_directive("generate:hex32"));
    }

    #[test]
    fn test_generate_hex32() {
        let secret = generate_secret("generate_outlayer_secret:hex32").unwrap();
        assert_eq!(secret.len(), 64); // 32 bytes = 64 hex chars
        assert!(secret.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn test_generate_hex16() {
        let secret = generate_secret("generate_outlayer_secret:hex16").unwrap();
        assert_eq!(secret.len(), 32); // 16 bytes = 32 hex chars
    }

    #[test]
    fn test_generate_hex64() {
        let secret = generate_secret("generate_outlayer_secret:hex64").unwrap();
        assert_eq!(secret.len(), 128); // 64 bytes = 128 hex chars
    }

    #[test]
    fn test_generate_ed25519() {
        let secret = generate_secret("generate_outlayer_secret:ed25519").unwrap();
        assert!(secret.starts_with("ed25519:"));

        // Base58 part should be ~43-44 chars for 32-byte key
        let base58_part = secret.strip_prefix("ed25519:").unwrap();
        assert!(base58_part.len() >= 43 && base58_part.len() <= 45);
    }

    #[test]
    fn test_generate_password_default() {
        let secret = generate_secret("generate_outlayer_secret:password").unwrap();
        assert_eq!(secret.len(), 32);
        assert!(secret.chars().all(|c| c.is_alphanumeric()));
    }

    #[test]
    fn test_generate_password_custom_length() {
        let secret = generate_secret("generate_outlayer_secret:password:64").unwrap();
        assert_eq!(secret.len(), 64);
        assert!(secret.chars().all(|c| c.is_alphanumeric()));
    }

    #[test]
    fn test_generate_password_short() {
        let secret = generate_secret("generate_outlayer_secret:password:16").unwrap();
        assert_eq!(secret.len(), 16);
    }

    #[test]
    fn test_invalid_type() {
        let result = generate_secret("generate_outlayer_secret:invalid");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Unknown secret type"));
    }

    #[test]
    fn test_invalid_prefix() {
        let result = generate_secret("wrong_prefix:hex32");
        assert!(result.is_err());
    }

    #[test]
    fn test_randomness() {
        // Generate two secrets and ensure they're different (extremely unlikely to be same)
        let secret1 = generate_secret("generate_outlayer_secret:hex32").unwrap();
        let secret2 = generate_secret("generate_outlayer_secret:hex32").unwrap();
        assert_ne!(secret1, secret2);
    }
}
