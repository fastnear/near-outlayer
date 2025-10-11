//! Client for communicating with keystore worker
//!
//! Handles TEE attestation generation and secret decryption requests.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// TEE attestation for keystore verification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Attestation {
    /// Type of TEE (sgx, sev, simulated, none)
    pub tee_type: String,
    /// Raw attestation quote/report (base64 encoded)
    pub quote: String,
    /// Worker's ephemeral public key
    pub worker_pubkey: Option<String>,
    /// Timestamp when attestation was generated
    pub timestamp: u64,
}

/// Request to decrypt secrets
#[derive(Debug, Serialize)]
struct DecryptRequest {
    encrypted_secrets: String,
    attestation: Attestation,
    task_id: Option<String>,
}

/// Response with decrypted secrets
#[derive(Debug, Deserialize)]
struct DecryptResponse {
    plaintext_secrets: String,
}

/// Client for keystore worker API
pub struct KeystoreClient {
    base_url: String,
    auth_token: String,
    http_client: reqwest::Client,
    tee_mode: String,
}

impl KeystoreClient {
    /// Create new keystore client
    pub fn new(base_url: String, auth_token: String, tee_mode: String) -> Self {
        Self {
            base_url,
            auth_token,
            http_client: reqwest::Client::new(),
            tee_mode,
        }
    }

    /// Generate TEE attestation for this worker
    ///
    /// In production TEE:
    /// - SGX: Use sgx_create_report() + sgx_get_quote()
    /// - SEV: Use SNP guest tools to generate attestation
    ///
    /// For MVP (simulated/none):
    /// - Generate fake attestation for testing
    pub fn generate_attestation(&self) -> Result<Attestation> {
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        match self.tee_mode.as_str() {
            "sgx" => {
                // TODO: Implement real SGX attestation
                // Steps:
                // 1. Get target info from keystore (or IAS)
                // 2. Create report with sgx_create_report()
                // 3. Get quote with sgx_get_quote()
                // 4. Return quote as base64
                tracing::warn!("SGX attestation not implemented, using placeholder");
                Ok(Attestation {
                    tee_type: "sgx".to_string(),
                    quote: base64::encode(b"placeholder-sgx-quote"),
                    worker_pubkey: None,
                    timestamp,
                })
            }
            "sev" => {
                // TODO: Implement real SEV-SNP attestation
                tracing::warn!("SEV attestation not implemented, using placeholder");
                Ok(Attestation {
                    tee_type: "sev".to_string(),
                    quote: base64::encode(b"placeholder-sev-report"),
                    worker_pubkey: None,
                    timestamp,
                })
            }
            "simulated" => {
                // Simulated mode: Use hash of worker binary as measurement
                let binary_path = std::env::current_exe()
                    .context("Failed to get current executable path")?;

                let binary = std::fs::read(&binary_path)
                    .context("Failed to read worker binary")?;

                let mut hasher = Sha256::new();
                hasher.update(&binary);
                let measurement = hasher.finalize();

                tracing::debug!(
                    measurement = %hex::encode(&measurement),
                    "Generated simulated attestation"
                );

                Ok(Attestation {
                    tee_type: "simulated".to_string(),
                    quote: base64::encode(measurement),
                    worker_pubkey: None,
                    timestamp,
                })
            }
            "none" => {
                // Dev mode: No attestation
                Ok(Attestation {
                    tee_type: "none".to_string(),
                    quote: base64::encode(b"no-attestation"),
                    worker_pubkey: None,
                    timestamp,
                })
            }
            other => {
                anyhow::bail!("Unsupported TEE mode: {}", other);
            }
        }
    }

    /// Decrypt secrets using keystore worker
    ///
    /// Sends encrypted secrets + attestation to keystore.
    /// Keystore verifies attestation and returns plaintext JSON.
    ///
    /// Returns: HashMap of environment variables (key-value pairs)
    pub async fn decrypt_secrets(
        &self,
        encrypted_secrets: &[u8],
        task_id: Option<&str>,
    ) -> Result<std::collections::HashMap<String, String>> {
        // Generate attestation
        let attestation = self.generate_attestation()
            .context("Failed to generate attestation")?;

        // Prepare request
        let request = DecryptRequest {
            encrypted_secrets: base64::encode(encrypted_secrets),
            attestation,
            task_id: task_id.map(|s| s.to_string()),
        };

        // Send request to keystore
        let url = format!("{}/decrypt", self.base_url);

        tracing::debug!(
            url = %url,
            task_id = ?task_id,
            "Requesting secret decryption from keystore"
        );

        let response = self
            .http_client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.auth_token))
            .json(&request)
            .send()
            .await
            .context("Failed to send decrypt request")?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            anyhow::bail!(
                "Keystore decrypt request failed: {} - {}",
                status,
                error_text
            );
        }

        let decrypt_response: DecryptResponse = response
            .json()
            .await
            .context("Failed to parse decrypt response")?;

        // Decode plaintext from base64
        let plaintext = base64::decode(&decrypt_response.plaintext_secrets)
            .context("Failed to decode plaintext secrets")?;

        tracing::info!(
            task_id = ?task_id,
            plaintext_size = plaintext.len(),
            "Successfully decrypted secrets"
        );

        // Parse JSON to HashMap
        let plaintext_str = String::from_utf8(plaintext)
            .context("Decrypted secrets are not valid UTF-8")?;

        let env_vars: std::collections::HashMap<String, String> = serde_json::from_str(&plaintext_str)
            .context("Failed to parse decrypted secrets as JSON object")?;

        tracing::debug!(
            task_id = ?task_id,
            env_count = env_vars.len(),
            "Parsed environment variables from decrypted secrets"
        );

        Ok(env_vars)
    }

    /// Get keystore public key (for testing/verification)
    pub async fn get_public_key(&self) -> Result<String> {
        let url = format!("{}/pubkey", self.base_url);

        let response = self
            .http_client
            .get(&url)
            .send()
            .await
            .context("Failed to get public key")?;

        let data: serde_json::Value = response
            .json()
            .await
            .context("Failed to parse pubkey response")?;

        let pubkey_hex = data["public_key_hex"]
            .as_str()
            .context("Missing public_key_hex in response")?
            .to_string();

        Ok(pubkey_hex)
    }
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
    fn test_generate_attestation_simulated() {
        let client = KeystoreClient::new(
            "http://localhost:8081".to_string(),
            "test-token".to_string(),
            "simulated".to_string(),
        );

        let attestation = client.generate_attestation().unwrap();
        assert_eq!(attestation.tee_type, "simulated");
        assert!(!attestation.quote.is_empty());
    }

    #[test]
    fn test_generate_attestation_none() {
        let client = KeystoreClient::new(
            "http://localhost:8081".to_string(),
            "test-token".to_string(),
            "none".to_string(),
        );

        let attestation = client.generate_attestation().unwrap();
        assert_eq!(attestation.tee_type, "none");
    }
}
