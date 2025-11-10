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
    pub signer_hash: Option<Vec<String>>,

    /// Minimum security version
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
        crate::config::TeeMode::Simulated => verify_simulated_attestation(attestation, expected),
        crate::config::TeeMode::None => {
            tracing::warn!("TEE verification disabled (dev mode) - accepting all attestations");
            Ok(())
        }
    }
}

/// Verify Intel SGX attestation
fn verify_sgx_attestation(
    attestation: &Attestation,
    _expected: &ExpectedMeasurements,
) -> Result<()> {
    if attestation.tee_type != "sgx" {
        anyhow::bail!("Expected SGX attestation, got {}", attestation.tee_type);
    }

    // TODO: Implement full SGX attestation verification
    // Steps:
    // 1. Decode quote from base64
    // 2. Verify quote signature (ECDSA P256)
    // 3. Check quote report data contains worker_pubkey hash
    // 4. Verify MR_ENCLAVE matches expected code hash
    // 5. Verify MR_SIGNER matches expected signer
    // 6. Check ISV_SVN >= min_security_version
    // 7. Verify quote is from genuine Intel SGX hardware (via IAS or DCAP)

    tracing::warn!("SGX attestation verification not fully implemented - using placeholder");

    // Placeholder: just check quote is not empty
    let quote_bytes = base64::decode(&attestation.quote)
        .context("Failed to decode quote")?;

    if quote_bytes.is_empty() {
        anyhow::bail!("Empty SGX quote");
    }

    Ok(())
}

/// Verify AMD SEV-SNP attestation
fn verify_sev_attestation(
    attestation: &Attestation,
    _expected: &ExpectedMeasurements,
) -> Result<()> {
    if attestation.tee_type != "sev" {
        anyhow::bail!("Expected SEV attestation, got {}", attestation.tee_type);
    }

    // TODO: Implement full SEV-SNP attestation verification
    // Steps:
    // 1. Decode attestation report
    // 2. Verify report signature (ECDSA)
    // 3. Verify measurement matches expected code
    // 4. Check platform version
    // 5. Verify with AMD KDS (Key Distribution Server)

    tracing::warn!("SEV attestation verification not fully implemented - using placeholder");

    let report_bytes = base64::decode(&attestation.quote)
        .context("Failed to decode report")?;

    if report_bytes.is_empty() {
        anyhow::bail!("Empty SEV report");
    }

    Ok(())
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
