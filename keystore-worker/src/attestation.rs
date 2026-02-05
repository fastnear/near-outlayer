//! TEE attestation verification
//!
//! In OutLayer TEE mode, workers are authenticated via challenge-response TEE sessions
//! (see tee-auth crate). Per-request attestation is a no-op since the session middleware
//! already guarantees the worker holds a TEE-registered private key.
//!
//! In None mode (dev), all attestations are accepted.

use anyhow::Result;
use serde::{Deserialize, Serialize};

/// TEE attestation provided by executor worker
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Attestation {
    /// Type of TEE (outlayer_tee, none)
    pub tee_type: String,

    /// Raw attestation quote/report (base64 encoded)
    pub quote: String,

    /// Worker's ephemeral public key (for this session)
    pub worker_pubkey: Option<String>,

    /// Timestamp when attestation was generated
    pub timestamp: u64,
}

/// Expected measurements for valid executor worker code
#[derive(Debug, Clone)]
pub struct ExpectedMeasurements {
    /// Expected code hash
    #[allow(dead_code)]
    pub code_hash: Vec<String>,

    /// Expected signer
    #[allow(dead_code)]
    pub signer_hash: Option<Vec<String>>,

    /// Minimum security version
    #[allow(dead_code)]
    pub min_security_version: u32,
}

impl Default for ExpectedMeasurements {
    fn default() -> Self {
        Self {
            code_hash: vec!["0000000000000000000000000000000000000000000000000000000000000000".to_string()],
            signer_hash: None,
            min_security_version: 1,
        }
    }
}

/// Verify attestation from executor worker
///
/// In OutlayerTee mode: no-op — worker is authenticated via TEE session (X-TEE-Session header).
/// In None mode: no-op — dev mode, all attestations accepted.
pub fn verify_attestation(
    _attestation: &Attestation,
    tee_mode: &crate::config::TeeMode,
    _expected: &ExpectedMeasurements,
) -> Result<()> {
    match tee_mode {
        crate::config::TeeMode::OutlayerTee => {
            // Worker authenticated via challenge-response TEE session.
            // Session middleware (validate_tee_session) already verified the worker
            // holds a private key registered on the register-contract.
            Ok(())
        }
        crate::config::TeeMode::None => {
            tracing::warn!("TEE verification disabled (dev mode)");
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_verify_attestation_none_mode() {
        let attestation = Attestation {
            tee_type: "none".to_string(),
            quote: String::new(),
            worker_pubkey: None,
            timestamp: 0,
        };

        let result = verify_attestation(
            &attestation,
            &crate::config::TeeMode::None,
            &ExpectedMeasurements::default(),
        );
        assert!(result.is_ok());
    }

    #[test]
    fn test_verify_attestation_outlayer_tee_mode() {
        let attestation = Attestation {
            tee_type: "outlayer_tee".to_string(),
            quote: String::new(),
            worker_pubkey: None,
            timestamp: 0,
        };

        let result = verify_attestation(
            &attestation,
            &crate::config::TeeMode::OutlayerTee,
            &ExpectedMeasurements::default(),
        );
        assert!(result.is_ok());
    }
}
