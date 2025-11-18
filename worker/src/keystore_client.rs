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
            "tdx" => {
                // TDX mode: Use simulated attestation for keystore (TDX quote only used for registration)
                // Keystore doesn't need real TDX quote verification - registration contract handles that
                tracing::info!("Using simulated attestation for keystore (TDX mode)");

                let binary_path = std::env::current_exe()
                    .context("Failed to get current executable path")?;

                let binary = std::fs::read(&binary_path)
                    .context("Failed to read worker binary")?;

                let mut hasher = Sha256::new();
                hasher.update(&binary);
                let measurement = hasher.finalize();

                Ok(Attestation {
                    tee_type: "tdx".to_string(),
                    quote: base64::encode(measurement),
                    worker_pubkey: None,
                    timestamp,
                })
            }
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

    /// Get keystore public key (for testing/verification)
    #[allow(dead_code)]
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

    /// Decrypt secrets from contract (new repo-based system)
    ///
    /// This method:
    /// 1. Calls keystore with repo, branch, profile, owner, user_account_id
    /// 2. Keystore reads secrets from NEAR contract
    /// 3. Keystore validates access conditions (using user_account_id as caller)
    /// 4. Keystore decrypts using derived key for seed (repo:owner[:branch])
    /// 5. Returns HashMap of environment variables
    ///
    /// Note: This requires keystore to have NEAR RPC access configured
    pub async fn decrypt_secrets_from_contract(
        &self,
        repo: &str,
        branch: Option<&str>,
        profile: &str,
        owner: &str,
        user_account_id: &str,
        task_id: Option<&str>,
    ) -> Result<std::collections::HashMap<String, String>> {
        tracing::info!(
            "🔑 decrypt_secrets_from_contract called: repo={}, branch={:?}, profile={}, owner={}, task_id={:?}",
            repo, branch, profile, owner, task_id
        );

        // Generate attestation
        let attestation = self.generate_attestation()
            .context("Failed to generate attestation")?;

        // Prepare request with new fields
        #[derive(Debug, Serialize)]
        struct DecryptFromContractRequest {
            repo: String,
            branch: Option<String>,
            profile: String,
            owner: String,
            user_account_id: String,
            attestation: Attestation,
            task_id: Option<String>,
        }

        let request = DecryptFromContractRequest {
            repo: repo.to_string(),
            branch: branch.map(|s| s.to_string()),
            profile: profile.to_string(),
            owner: owner.to_string(),
            user_account_id: user_account_id.to_string(),
            attestation,
            task_id: task_id.map(|s| s.to_string()),
        };

        // Send request to keystore
        let url = format!("{}/decrypt", self.base_url);

        tracing::debug!(
            url = %url,
            repo = %repo,
            profile = %profile,
            owner = %owner,
            task_id = ?task_id,
            "Requesting secret decryption from contract via keystore"
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

            // Parse user-friendly error message
            let user_message = if status == 400 {
                // Bad request - usually "Secrets not found"
                if error_text.contains("not found") {
                    "Secrets not found. Please check that secrets exist for this repository, branch, and profile.".to_string()
                } else {
                    "Invalid secrets request. Please check your secrets configuration.".to_string()
                }
            } else if status == 401 {
                // Access denied - parse error details
                if let Ok(json) = serde_json::from_str::<serde_json::Value>(&error_text) {
                    if let Some(error) = json.get("error").and_then(|e| e.as_str()) {
                        if error.contains("Access denied") {
                            "Access to secrets denied. You do not have permission to use these secrets. Check the access conditions configured by the secret owner.".to_string()
                        } else {
                            format!("Secret access error: {}", error)
                        }
                    } else {
                        "Access to secrets denied. Check access conditions.".to_string()
                    }
                } else {
                    "Access to secrets denied. Check access conditions.".to_string()
                }
            } else if status == 404 {
                "Secrets not found. The specified secrets do not exist or have been deleted.".to_string()
            } else {
                // Generic error - don't expose technical details
                "Failed to decrypt secrets. Please check your secrets configuration.".to_string()
            };

            anyhow::bail!("{}", user_message);
        }

        let decrypt_response: DecryptResponse = response
            .json()
            .await
            .context("Failed to parse decrypt response")?;

        // Decode plaintext from base64
        let plaintext = base64::decode(&decrypt_response.plaintext_secrets)
            .context("Failed to decode plaintext secrets")?;

        tracing::info!(
            repo = %repo,
            profile = %profile,
            plaintext_size = plaintext.len(),
            "Successfully decrypted secrets from contract"
        );

        // Parse JSON to HashMap
        let plaintext_str = String::from_utf8(plaintext)
            .context("Invalid secrets format: not valid UTF-8 text")?;

        let env_vars: std::collections::HashMap<String, String> = serde_json::from_str(&plaintext_str)
            .context("Invalid secrets format: must be a JSON object with string key-value pairs")?;

        tracing::debug!(
            repo = %repo,
            env_count = env_vars.len(),
            "Parsed environment variables from decrypted secrets"
        );

        Ok(env_vars)
    }

    /// Decrypt secrets from contract by WASM hash (for WasmUrl sources)
    ///
    /// This method:
    /// 1. Calls keystore with wasm_hash, profile, owner, user_account_id
    /// 2. Keystore reads secrets from NEAR contract (get_secrets_by_wasm_hash)
    /// 3. Keystore validates access conditions (using user_account_id as caller)
    /// 4. Keystore decrypts using derived key for seed (wasm_hash:owner)
    /// 5. Returns HashMap of environment variables
    pub async fn decrypt_secrets_by_wasm_hash(
        &self,
        wasm_hash: &str,
        profile: &str,
        owner: &str,
        user_account_id: &str,
        task_id: Option<&str>,
    ) -> Result<std::collections::HashMap<String, String>> {
        tracing::info!(
            "🔑 decrypt_secrets_by_wasm_hash called: wasm_hash={}, profile={}, owner={}, task_id={:?}",
            wasm_hash, profile, owner, task_id
        );

        // Generate attestation
        let attestation = self.generate_attestation()
            .context("Failed to generate attestation")?;

        // Prepare request with wasm_hash field
        #[derive(Debug, Serialize)]
        struct DecryptByWasmHashRequest {
            wasm_hash: String,
            profile: String,
            owner: String,
            user_account_id: String,
            attestation: Attestation,
            task_id: Option<String>,
        }

        let request = DecryptByWasmHashRequest {
            wasm_hash: wasm_hash.to_string(),
            profile: profile.to_string(),
            owner: owner.to_string(),
            user_account_id: user_account_id.to_string(),
            attestation,
            task_id: task_id.map(|s| s.to_string()),
        };

        // Send request to keystore (using /decrypt-by-hash endpoint)
        let url = format!("{}/decrypt-by-hash", self.base_url);

        tracing::debug!(
            url = %url,
            wasm_hash = %wasm_hash,
            profile = %profile,
            owner = %owner,
            task_id = ?task_id,
            "Requesting secret decryption by wasm_hash via keystore"
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

            // Parse user-friendly error message
            let user_message = if status == 400 {
                if error_text.contains("not found") {
                    "Secrets not found for this WASM hash. Please check that secrets exist for this WASM binary and profile.".to_string()
                } else {
                    "Invalid secrets request. Please check your secrets configuration.".to_string()
                }
            } else if status == 401 {
                if let Ok(json) = serde_json::from_str::<serde_json::Value>(&error_text) {
                    if let Some(error) = json.get("error").and_then(|e| e.as_str()) {
                        if error.contains("Access denied") {
                            "Access to secrets denied. You do not have permission to use these secrets.".to_string()
                        } else {
                            format!("Secret access error: {}", error)
                        }
                    } else {
                        "Access to secrets denied. Check access conditions.".to_string()
                    }
                } else {
                    "Access to secrets denied. Check access conditions.".to_string()
                }
            } else if status == 404 {
                "Secrets not found for this WASM hash.".to_string()
            } else {
                "Failed to decrypt secrets. Please check your secrets configuration.".to_string()
            };

            anyhow::bail!("{}", user_message);
        }

        let decrypt_response: DecryptResponse = response
            .json()
            .await
            .context("Failed to parse decrypt response")?;

        // Decode plaintext from base64
        let plaintext = base64::decode(&decrypt_response.plaintext_secrets)
            .context("Failed to decode plaintext secrets")?;

        tracing::info!(
            wasm_hash = %wasm_hash,
            profile = %profile,
            plaintext_size = plaintext.len(),
            "Successfully decrypted secrets by wasm_hash"
        );

        // Parse JSON to HashMap
        let plaintext_str = String::from_utf8(plaintext)
            .context("Invalid secrets format: not valid UTF-8 text")?;

        let env_vars: std::collections::HashMap<String, String> = serde_json::from_str(&plaintext_str)
            .context("Invalid secrets format: must be a JSON object with string key-value pairs")?;

        tracing::debug!(
            wasm_hash = %wasm_hash,
            env_count = env_vars.len(),
            "Parsed environment variables from decrypted secrets"
        );

        Ok(env_vars)
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
