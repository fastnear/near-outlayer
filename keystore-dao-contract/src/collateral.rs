// Code taken 1:1 from MPC Node (audited code):
// https://github.com/near/mpc/blob/main/crates/attestation/src/collateral.rs

use near_sdk::borsh::{BorshDeserialize, BorshSerialize};
use near_sdk::serde::{Deserialize, Serialize};
use serde_json::Value;

pub use dcap_qvl::QuoteCollateralV3;

/// Supplemental data for the TEE quote, including Intel certificates to verify it came from genuine
/// Intel hardware, along with details about the Trusted Computing Base (TCB) versioning, status,
/// and other relevant info.
#[derive(Clone, Debug, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
#[serde(crate = "near_sdk::serde")]
#[serde(try_from = "Value")]
pub struct Collateral(QuoteCollateralV3);

impl Collateral {
    /// Attempts to create a [`Collateral`] from a JSON value containing quote collateral data.
    ///
    /// # Errors
    ///
    /// Returns a [`CollateralError`] if:
    /// - Any required field is missing or has an invalid type
    /// - Hex fields cannot be decoded
    pub fn try_from_json(v: Value) -> Result<Self, CollateralError> {
        fn get_str(v: &Value, key: &str) -> Result<String, CollateralError> {
            v.get(key)
                .and_then(Value::as_str)
                .map(String::from)
                .ok_or_else(|| CollateralError::MissingField(String::from(key)))
        }

        fn get_hex(v: &Value, key: &str) -> Result<Vec<u8>, CollateralError> {
            let hex_str = get_str(v, key)?;
            hex::decode(hex_str).map_err(|source| CollateralError::HexDecode {
                field: String::from(key),
                source,
            })
        }

        fn get_str_optional(v: &Value, key: &str) -> Option<String> {
            v.get(key).and_then(Value::as_str).map(String::from)
        }

        let quote_collateral = QuoteCollateralV3 {
            tcb_info_issuer_chain: get_str(&v, "tcb_info_issuer_chain")?,
            tcb_info: get_str(&v, "tcb_info")?,
            tcb_info_signature: get_hex(&v, "tcb_info_signature")?,
            qe_identity_issuer_chain: get_str(&v, "qe_identity_issuer_chain")?,
            qe_identity: get_str(&v, "qe_identity")?,
            qe_identity_signature: get_hex(&v, "qe_identity_signature")?,
            pck_crl_issuer_chain: get_str(&v, "pck_crl_issuer_chain")?,
            root_ca_crl: get_hex(&v, "root_ca_crl")?,
            pck_crl: get_hex(&v, "pck_crl")?,
            pck_certificate_chain: get_str_optional(&v, "pck_certificate_chain"),
        };
        Ok(Self(quote_collateral))
    }

    /// Get reference to inner QuoteCollateralV3
    pub fn inner(&self) -> &QuoteCollateralV3 {
        &self.0
    }
}

impl TryFrom<Value> for Collateral {
    type Error = CollateralError;

    fn try_from(value: Value) -> Result<Self, Self::Error> {
        Self::try_from_json(value)
    }
}

#[derive(Debug)]
pub enum CollateralError {
    MissingField(String),
    HexDecode {
        field: String,
        source: hex::FromHexError,
    },
}

impl core::fmt::Display for CollateralError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            CollateralError::MissingField(field) => {
                write!(f, "Missing or invalid field: {}", field)
            }
            CollateralError::HexDecode { field, source } => {
                write!(f, "Failed to decode hex field '{}': {}", field, source)
            }
        }
    }
}
