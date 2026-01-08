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

/// Secret accessor type - matches keystore's SecretAccessor enum
///
/// IMPORTANT: When adding new accessor types:
/// 1. Add variant here in worker
/// 2. Add variant in keystore-worker/src/api.rs (SecretAccessor enum)
/// 3. Add variant in coordinator/src/handlers/github.rs (SecretAccessor enum)
/// 4. Add variant in contract/src/lib.rs (SecretAccessor enum)
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type")]
pub enum SecretAccessor {
    /// Secrets bound to a GitHub repository
    Repo {
        repo: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        branch: Option<String>,
    },
    /// Secrets bound to a specific WASM hash
    WasmHash {
        hash: String,
    },
    /// Secrets bound to a project (available to all versions)
    Project {
        project_id: String,
    },
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
    /// TEE modes:
    /// - TDX: Uses simulated attestation (TDX quote only used for worker registration)
    /// - SGX: Use sgx_create_report() + sgx_get_quote()
    /// - SEV: Use SNP guest tools to generate attestation
    /// - simulated/none: Generate test attestation for development
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

    /// Decrypt secrets from contract using unified accessor format
    ///
    /// This method:
    /// 1. Calls keystore /decrypt with accessor (Repo or WasmHash)
    /// 2. Keystore reads secrets from NEAR contract
    /// 3. Keystore validates access conditions (using user_account_id as caller)
    /// 4. Keystore decrypts using derived key for seed
    /// 5. Returns HashMap of environment variables
    ///
    /// Note: This requires keystore to have NEAR RPC access configured
    pub async fn decrypt_secrets(
        &self,
        accessor: SecretAccessor,
        profile: &str,
        owner: &str,
        user_account_id: &str,
        task_id: Option<&str>,
    ) -> Result<std::collections::HashMap<String, String>> {
        let accessor_desc = match &accessor {
            SecretAccessor::Repo { repo, branch } => {
                format!("Repo(repo={}, branch={:?})", repo, branch)
            }
            SecretAccessor::WasmHash { hash } => {
                format!("WasmHash({})", hash)
            }
            SecretAccessor::Project { project_id } => {
                format!("Project({})", project_id)
            }
        };

        tracing::info!(
            "🔑 decrypt_secrets called: accessor={}, profile={}, owner={}, task_id={:?}",
            accessor_desc, profile, owner, task_id
        );

        // Generate attestation
        let attestation = self.generate_attestation()
            .context("Failed to generate attestation")?;

        // Prepare request with accessor
        #[derive(Debug, Serialize)]
        struct DecryptRequest {
            accessor: SecretAccessor,
            profile: String,
            owner: String,
            user_account_id: String,
            attestation: Attestation,
            task_id: Option<String>,
        }

        let request = DecryptRequest {
            accessor: accessor.clone(),
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
            accessor = %accessor_desc,
            profile = %profile,
            owner = %owner,
            task_id = ?task_id,
            "Requesting secret decryption via keystore"
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

            // Parse user-friendly error message based on accessor type
            let context = match &accessor {
                SecretAccessor::Repo { .. } => "repository, branch, and profile",
                SecretAccessor::WasmHash { .. } => "WASM hash and profile",
                SecretAccessor::Project { .. } => "project ID and profile",
            };

            let user_message = if status == 400 {
                if error_text.contains("not found") {
                    format!("Secrets not found. Please check that secrets exist for this {}.", context)
                } else {
                    "Invalid secrets request. Please check your secrets configuration.".to_string()
                }
            } else if status == 401 {
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
                format!("Secrets not found for this {}.", context)
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
            accessor = %accessor_desc,
            profile = %profile,
            plaintext_size = plaintext.len(),
            "Successfully decrypted secrets"
        );

        // Parse JSON to HashMap
        let plaintext_str = String::from_utf8(plaintext)
            .context("Invalid secrets format: not valid UTF-8 text")?;

        let env_vars: std::collections::HashMap<String, String> = serde_json::from_str(&plaintext_str)
            .context("Invalid secrets format: must be a JSON object with string key-value pairs")?;

        tracing::debug!(
            accessor = %accessor_desc,
            env_count = env_vars.len(),
            "Parsed environment variables from decrypted secrets"
        );

        Ok(env_vars)
    }

    /// Decrypt secrets from contract (convenience wrapper for Repo accessor)
    ///
    /// This is a convenience method that wraps decrypt_secrets with Repo accessor.
    pub async fn decrypt_secrets_from_contract(
        &self,
        repo: &str,
        branch: Option<&str>,
        profile: &str,
        owner: &str,
        user_account_id: &str,
        task_id: Option<&str>,
    ) -> Result<std::collections::HashMap<String, String>> {
        let accessor = SecretAccessor::Repo {
            repo: repo.to_string(),
            branch: branch.map(|s| s.to_string()),
        };
        self.decrypt_secrets(accessor, profile, owner, user_account_id, task_id).await
    }

    /// Decrypt secrets from contract by WASM hash (convenience wrapper for WasmHash accessor)
    ///
    /// This is a convenience method that wraps decrypt_secrets with WasmHash accessor.
    pub async fn decrypt_secrets_by_wasm_hash(
        &self,
        wasm_hash: &str,
        profile: &str,
        owner: &str,
        user_account_id: &str,
        task_id: Option<&str>,
    ) -> Result<std::collections::HashMap<String, String>> {
        let accessor = SecretAccessor::WasmHash {
            hash: wasm_hash.to_string(),
        };
        self.decrypt_secrets(accessor, profile, owner, user_account_id, task_id).await
    }

    /// Decrypt secrets from contract by project ID (convenience wrapper for Project accessor)
    ///
    /// This is a convenience method that wraps decrypt_secrets with Project accessor.
    /// All versions of the project can use the same secrets.
    pub async fn decrypt_secrets_by_project(
        &self,
        project_id: &str,
        profile: &str,
        owner: &str,
        user_account_id: &str,
        task_id: Option<&str>,
    ) -> Result<std::collections::HashMap<String, String>> {
        let accessor = SecretAccessor::Project {
            project_id: project_id.to_string(),
        };
        self.decrypt_secrets(accessor, profile, owner, user_account_id, task_id).await
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

    /// Test SecretAccessor::Repo serialization (with branch)
    #[test]
    fn test_secret_accessor_repo_with_branch() {
        let accessor = SecretAccessor::Repo {
            repo: "github.com/user/repo".to_string(),
            branch: Some("main".to_string()),
        };

        let json = serde_json::to_string(&accessor).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed["type"], "Repo");
        assert_eq!(parsed["repo"], "github.com/user/repo");
        assert_eq!(parsed["branch"], "main");
    }

    /// Test SecretAccessor::Repo serialization (without branch - branch should be omitted)
    #[test]
    fn test_secret_accessor_repo_without_branch() {
        let accessor = SecretAccessor::Repo {
            repo: "github.com/user/repo".to_string(),
            branch: None,
        };

        let json = serde_json::to_string(&accessor).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed["type"], "Repo");
        assert_eq!(parsed["repo"], "github.com/user/repo");
        // branch should be omitted (not null) due to skip_serializing_if
        assert!(parsed.get("branch").is_none());
    }

    /// Test SecretAccessor::WasmHash serialization
    #[test]
    fn test_secret_accessor_wasm_hash() {
        let accessor = SecretAccessor::WasmHash {
            hash: "abc123def456".to_string(),
        };

        let json = serde_json::to_string(&accessor).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed["type"], "WasmHash");
        assert_eq!(parsed["hash"], "abc123def456");
    }

    /// Test that serialized JSON is compatible with keystore's expected format
    #[test]
    fn test_secret_accessor_keystore_compatibility() {
        // Repo with branch
        let accessor = SecretAccessor::Repo {
            repo: "github.com/test/project".to_string(),
            branch: Some("develop".to_string()),
        };
        let json = serde_json::to_string(&accessor).unwrap();
        // Keystore expects: {"type": "Repo", "repo": "...", "branch": "..."}
        assert!(json.contains(r#""type":"Repo""#));
        assert!(json.contains(r#""repo":"github.com/test/project""#));
        assert!(json.contains(r#""branch":"develop""#));

        // WasmHash
        let accessor = SecretAccessor::WasmHash {
            hash: "deadbeef".to_string(),
        };
        let json = serde_json::to_string(&accessor).unwrap();
        // Keystore expects: {"type": "WasmHash", "hash": "..."}
        assert!(json.contains(r#""type":"WasmHash""#));
        assert!(json.contains(r#""hash":"deadbeef""#));
    }
}
