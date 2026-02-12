//! Verifiable Random Function (VRF) for OutLayer WASM components
//!
//! Provides cryptographically verifiable randomness using Ed25519 deterministic signatures.
//! The host auto-prepends `request_id` to the seed, preventing manipulation by the WASM guest.
//!
//! ## Usage
//!
//! ```rust,ignore
//! use outlayer::vrf;
//!
//! // Get verifiable random output
//! let result = vrf::random("my-seed").unwrap();
//! println!("Random hex: {}", result.output_hex);       // SHA256(signature)
//! println!("Proof: {}", result.signature_hex);          // Ed25519 signature
//! println!("Alpha: {}", result.alpha);                  // "vrf:{request_id}:my-seed"
//!
//! // Or get raw 32 bytes directly
//! let (bytes, signature_hex, alpha) = vrf::random_bytes("my-seed").unwrap();
//! ```
//!
//! ## On-Chain Verification
//!
//! Anyone can verify VRF output in a NEAR smart contract:
//!
//! ```rust,ignore
//! // In NEAR contract:
//! let valid = env::ed25519_verify(
//!     &vrf_pubkey_bytes,   // GET /vrf/pubkey
//!     alpha.as_bytes(),    // "vrf:{request_id}:{user_seed}"
//!     &signature_bytes,    // proof from VRF output
//! );
//! ```

use crate::raw::vrf;

/// VRF output with proof
pub struct VrfOutput {
    /// SHA256(signature) as hex string — 32 random bytes
    pub output_hex: String,
    /// Ed25519 signature as hex string — the proof
    pub signature_hex: String,
    /// Full alpha string: "vrf:{request_id}:{user_seed}" — for verification
    pub alpha: String,
}

/// Generate verifiable random output from a user seed.
///
/// The host constructs `alpha = "vrf:{request_id}:{user_seed}"` where `request_id`
/// comes from the execution context (not controllable by the WASM guest).
///
/// Returns `VrfOutput` with random bytes, proof, and the alpha used.
pub fn random(user_seed: &str) -> Result<VrfOutput, String> {
    let (output_hex, signature_hex, alpha, error) = vrf::generate(user_seed);
    if !error.is_empty() {
        return Err(error);
    }
    Ok(VrfOutput {
        output_hex,
        signature_hex,
        alpha,
    })
}

/// Generate verifiable random bytes from a user seed.
///
/// Convenience function that decodes the hex output into a `[u8; 32]` array.
///
/// Returns `(random_bytes, signature_hex, alpha)`.
pub fn random_bytes(user_seed: &str) -> Result<([u8; 32], String, String), String> {
    let result = random(user_seed)?;

    let mut bytes = [0u8; 32];
    let decoded = hex_decode(&result.output_hex)
        .map_err(|e| format!("Failed to decode VRF output hex: {}", e))?;
    if decoded.len() != 32 {
        return Err(format!(
            "VRF output has unexpected length: {} (expected 32)",
            decoded.len()
        ));
    }
    bytes.copy_from_slice(&decoded);
    Ok((bytes, result.signature_hex, result.alpha))
}

/// Get the VRF public key (hex-encoded Ed25519 public key, 32 bytes).
///
/// This key can be used for on-chain verification via `ed25519_verify`.
pub fn public_key() -> Result<String, String> {
    let (pubkey_hex, error) = vrf::pubkey();
    if !error.is_empty() {
        return Err(error);
    }
    Ok(pubkey_hex)
}

/// Simple hex decoder (avoids adding hex crate dependency)
fn hex_decode(hex: &str) -> Result<Vec<u8>, String> {
    if hex.len() % 2 != 0 {
        return Err("Odd-length hex string".to_string());
    }
    (0..hex.len())
        .step_by(2)
        .map(|i| {
            u8::from_str_radix(&hex[i..i + 2], 16)
                .map_err(|e| format!("Invalid hex at position {}: {}", i, e))
        })
        .collect()
}
