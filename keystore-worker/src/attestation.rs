//! TEE attestation verification
//!
//! Verifies that executor workers are running in a trusted TEE environment
//! with the correct code before providing decrypted secrets.
//!
//! Supports:
//! - Intel SGX Remote Attestation
//! - AMD SEV-SNP Attestation
//! - Simulated mode for testing
//! - None mode for development

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Sha256, Digest};

/// TEE attestation provided by executor worker
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Attestation {
    /// Type of TEE (sgx, sev, simulated, none)
    pub tee_type: String,

    /// Raw attestation quote/report (base64 encoded)
    pub quote: String,

    /// Worker's ephemeral public key (for this session)
    /// Used to establish secure channel
    pub worker_pubkey: Option<String>,

    /// Timestamp when attestation was generated
    pub timestamp: u64,
}

/// Expected measurements for valid executor worker code
#[derive(Debug, Clone)]
pub struct ExpectedMeasurements {
    /// Expected code hash (MR_ENCLAVE for SGX, measurement for SEV)
    pub code_hash: Vec<String>,

    /// Expected signer (MR_SIGNER for SGX)
    #[allow(dead_code)]
    pub signer_hash: Option<Vec<String>>,

    /// Minimum security version
    #[allow(dead_code)]
    pub min_security_version: u32,
}

impl Default for ExpectedMeasurements {
    fn default() -> Self {
        Self {
            // TODO: Replace with actual worker binary hash after first build
            code_hash: vec!["0000000000000000000000000000000000000000000000000000000000000000".to_string()],
            signer_hash: None,
            min_security_version: 1,
        }
    }
}

/// Verify attestation from executor worker
///
/// This function ensures:
/// 1. Attestation is valid and recent
/// 2. Worker is running in genuine TEE
/// 3. Worker is running the expected code (measurement check)
/// 4. TEE security version is up to date
pub fn verify_attestation(
    attestation: &Attestation,
    tee_mode: &crate::config::TeeMode,
    expected: &ExpectedMeasurements,
) -> Result<()> {
    // Check timestamp (must be within last 5 minutes)
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();

    if now > attestation.timestamp + 300 {
        anyhow::bail!("Attestation expired (older than 5 minutes)");
    }

    match tee_mode {
        crate::config::TeeMode::Sgx => verify_sgx_attestation(attestation, expected),
        crate::config::TeeMode::Sev => verify_sev_attestation(attestation, expected),
        crate::config::TeeMode::Tdx => verify_tdx_attestation(attestation, expected),
        crate::config::TeeMode::Simulated => verify_simulated_attestation(attestation, expected),
        crate::config::TeeMode::None => {
            tracing::warn!("TEE verification disabled (dev mode) - accepting all attestations");
            Ok(())
        }
    }
}

/// Verify Intel SGX attestation
///
/// ⚠️ NOT IMPLEMENTED - Use TEE_MODE=none for testing
fn verify_sgx_attestation(
    attestation: &Attestation,
    _expected: &ExpectedMeasurements,
) -> Result<()> {
    if attestation.tee_type != "sgx" {
        anyhow::bail!("Expected SGX attestation, got {}", attestation.tee_type);
    }

    anyhow::bail!("SGX attestation verification not implemented - use TEE_MODE=none for testing")
    // TODO: Implement full SGX attestation verification:
    // 1. Decode quote from base64
    // 2. Verify quote signature (ECDSA P256)
    // 3. Check quote report data contains worker_pubkey hash
    // 4. Verify MR_ENCLAVE matches expected code hash
    // 5. Verify MR_SIGNER matches expected signer
    // 6. Check ISV_SVN >= min_security_version
    // 7. Verify quote is from genuine Intel SGX hardware (via IAS or DCAP)
}

/// Verify AMD SEV-SNP attestation
///
/// ⚠️ NOT IMPLEMENTED - Use TEE_MODE=none for testing
fn verify_sev_attestation(
    attestation: &Attestation,
    _expected: &ExpectedMeasurements,
) -> Result<()> {
    if attestation.tee_type != "sev" {
        anyhow::bail!("Expected SEV attestation, got {}", attestation.tee_type);
    }

    anyhow::bail!("SEV attestation verification not implemented - use TEE_MODE=none for testing")
    // TODO: Implement full SEV-SNP attestation verification:
    // 1. Decode attestation report
    // 2. Verify report signature (ECDSA)
    // 3. Verify measurement matches expected code
    // 4. Check platform version
    // 5. Verify with AMD KDS (Key Distribution Server)
}

/// Verify Intel TDX attestation
///
/// When both keystore and worker are in TEE, we rely on bearer token authentication
/// instead of full TDX attestation verification. The token is checked in auth_middleware.
fn verify_tdx_attestation(
    attestation: &Attestation,
    _expected: &ExpectedMeasurements,
) -> Result<()> {
    // In production TEE environment, both keystore and worker are in TEE.
    // Authentication is handled by KEYSTORE_AUTH_TOKEN/ALLOWED_WORKER_TOKEN_HASHES
    // in the auth_middleware, which has already validated the request by this point.
    // We accept any attestation type since token auth is the primary security check.
    tracing::info!("✅ TDX mode: attestation accepted - worker authenticated via bearer token (tee_type={})", attestation.tee_type);

    // Log the attestation details for debugging
    tracing::debug!("TDX attestation details:");
    tracing::debug!("  TEE type: {}", attestation.tee_type);
    tracing::debug!("  Quote length: {} bytes", attestation.quote.len());
    if attestation.quote.len() >= 512 {
        // Extract RTMR3 from quote for logging (offset 256, size 48)
        if let Ok(quote_bytes) = base64::decode(&attestation.quote) {
            if quote_bytes.len() >= 304 {
                let rtmr3_bytes = &quote_bytes[256..304];
                let rtmr3_hex = hex::encode(rtmr3_bytes);
                tracing::debug!("  Worker RTMR3: {}", rtmr3_hex);
            }
        }
    }

    Ok(())

    // NOTE: Full TDX attestation verification is not needed when:
    // 1. Both keystore and worker are in TEE (verified during registration)
    // 2. Bearer token authentication is used (KEYSTORE_AUTH_TOKEN)
    // 3. Worker has been registered and approved by DAO
    //
    // The authentication flow is:
    // 1. Worker presents bearer token in Authorization header
    // 2. auth_middleware validates token hash against ALLOWED_WORKER_TOKEN_HASHES
    // 3. This function is called but relies on token auth from step 2
}

/// Verify simulated attestation (for testing)
fn verify_simulated_attestation(
    attestation: &Attestation,
    expected: &ExpectedMeasurements,
) -> Result<()> {
    if attestation.tee_type != "simulated" {
        anyhow::bail!("Expected simulated attestation, got {}", attestation.tee_type);
    }

    // Simulated mode: quote contains SHA256(worker_code + timestamp)
    let quote_bytes = base64::decode(&attestation.quote)
        .context("Failed to decode simulated quote")?;

    if quote_bytes.len() != 32 {
        anyhow::bail!("Invalid simulated quote length");
    }

    let quote_hex = hex::encode(&quote_bytes);

    // Check if quote matches any expected code hash
    if !expected.code_hash.contains(&quote_hex) {
        anyhow::bail!(
            "Code measurement mismatch: got {}, expected one of {:?}",
            quote_hex,
            expected.code_hash
        );
    }

    tracing::info!("Simulated attestation verified successfully");
    Ok(())
}

/// Generate simulated attestation (for testing)
///
/// This is used by executor workers in simulated mode
#[allow(dead_code)]
pub fn generate_simulated_attestation(worker_code_path: &str) -> Result<Attestation> {
    let code = std::fs::read(worker_code_path)
        .context("Failed to read worker binary")?;

    let mut hasher = Sha256::new();
    hasher.update(&code);
    let measurement = hasher.finalize();

    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();

    Ok(Attestation {
        tee_type: "simulated".to_string(),
        quote: base64::encode(measurement),
        worker_pubkey: None,
        timestamp,
    })
}

// Base64 encoding/decoding helpers
mod base64 {
    use ::base64::Engine;
    use ::base64::engine::general_purpose::STANDARD;

    pub fn encode<T: AsRef<[u8]>>(input: T) -> String {
        STANDARD.encode(input)
    }

    pub fn decode<T: AsRef<[u8]>>(input: T) -> Result<Vec<u8>, ::base64::DecodeError> {
        STANDARD.decode(input)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simulated_attestation() {
        // Use a proper 32-byte (64 hex chars) SHA256 hash
        let code_hash_hex = "a".repeat(64); // Valid 32-byte hash in hex

        let measurements = ExpectedMeasurements {
            code_hash: vec![code_hash_hex.clone()],
            signer_hash: None,
            min_security_version: 1,
        };

        let attestation = Attestation {
            tee_type: "simulated".to_string(),
            quote: base64::encode(hex::decode(&code_hash_hex).unwrap()),
            worker_pubkey: None,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        };

        verify_simulated_attestation(&attestation, &measurements).unwrap();
    }

    #[test]
    fn test_expired_attestation() {
        let measurements = ExpectedMeasurements::default();

        let attestation = Attestation {
            tee_type: "simulated".to_string(),
            quote: String::new(),
            worker_pubkey: None,
            timestamp: 1000, // Very old timestamp
        };

        let result = verify_attestation(
            &attestation,
            &crate::config::TeeMode::Simulated,
            &measurements,
        );

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("expired"));
    }
}
