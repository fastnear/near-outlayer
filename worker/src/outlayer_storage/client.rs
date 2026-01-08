//! HTTP client for Coordinator storage API with Keystore encryption
//!
//! This client handles communication with:
//! - Keystore: encrypt/decrypt data
//! - Coordinator: store/retrieve encrypted data
//!
//! All encryption/decryption is done by keystore (TEE), not locally.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::time::Duration;
use tracing::{debug, error, warn};

/// Storage client configuration
#[derive(Clone)]
pub struct StorageConfig {
    /// Coordinator API base URL
    pub coordinator_url: String,
    /// Auth token for coordinator API
    pub coordinator_token: String,
    /// Keystore API base URL
    pub keystore_url: String,
    /// Auth token for keystore API
    pub keystore_token: String,
    /// Project UUID - required for storage
    pub project_uuid: String,
    /// WASM hash (SHA256 of current WASM binary)
    pub wasm_hash: String,
    /// Account ID of the signer (NEAR account)
    pub account_id: String,
    /// TEE mode for attestation
    pub tee_mode: String,
}

/// Attestation for keystore requests
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Attestation {
    pub tee_type: String,
    pub quote: String,
    pub measurements: serde_json::Value,
    pub timestamp: u64,
}

impl Attestation {
    /// Create a simulated attestation (for dev mode)
    pub fn simulated() -> Self {
        Self {
            tee_type: "simulated".to_string(),
            quote: "".to_string(),
            measurements: serde_json::json!({}),
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        }
    }

    /// Create attestation based on TEE mode
    pub fn for_mode(tee_mode: &str) -> Self {
        match tee_mode {
            "none" | "simulated" => Self::simulated(),
            // TODO: Add real TDX/SGX attestation
            _ => Self::simulated(),
        }
    }
}

/// HTTP client for coordinator storage API
pub struct StorageClient {
    client: reqwest::blocking::Client,
    config: StorageConfig,
}

impl StorageClient {
    /// Create a new storage client
    pub fn new(config: StorageConfig) -> Result<Self> {
        let client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(30))
            .connect_timeout(Duration::from_secs(10))
            .build()
            .context("Failed to create HTTP client")?;

        Ok(Self { client, config })
    }

    /// Hash a key for storage lookup
    fn hash_key(&self, key: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(key.as_bytes());
        hex::encode(hasher.finalize())
    }

    /// Encrypt data via keystore
    fn encrypt_via_keystore(&self, key: &str, value: &[u8], account_id: &str) -> Result<EncryptedData> {
        let body = serde_json::json!({
            "project_uuid": self.config.project_uuid,
            "wasm_hash": self.config.wasm_hash,
            "account_id": account_id,
            "key": key,
            "value_base64": base64_encode(value),
            "attestation": Attestation::for_mode(&self.config.tee_mode),
        });

        let response = self
            .client
            .post(format!("{}/storage/encrypt", self.config.keystore_url))
            .header("Authorization", format!("Bearer {}", self.config.keystore_token))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .context("Failed to send keystore encrypt request")?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().unwrap_or_default();
            error!("Keystore encrypt failed: {} - {}", status, error_text);
            anyhow::bail!("Keystore encrypt failed: {} - {}", status, error_text);
        }

        #[derive(Deserialize)]
        struct EncryptResponse {
            encrypted_key_base64: String,
            encrypted_value_base64: String,
            key_hash: String,
        }

        let resp: EncryptResponse = response.json().context("Failed to parse keystore encrypt response")?;

        Ok(EncryptedData {
            encrypted_key: base64_decode(&resp.encrypted_key_base64)?,
            encrypted_value: base64_decode(&resp.encrypted_value_base64)?,
            key_hash: resp.key_hash,
        })
    }

    /// Decrypt data via keystore
    fn decrypt_via_keystore(&self, encrypted_key: &[u8], encrypted_value: &[u8], account_id: &str) -> Result<DecryptedData> {
        let body = serde_json::json!({
            "project_uuid": self.config.project_uuid,
            "wasm_hash": self.config.wasm_hash,
            "account_id": account_id,
            "encrypted_key_base64": base64_encode(encrypted_key),
            "encrypted_value_base64": base64_encode(encrypted_value),
            "attestation": Attestation::for_mode(&self.config.tee_mode),
        });

        let response = self
            .client
            .post(format!("{}/storage/decrypt", self.config.keystore_url))
            .header("Authorization", format!("Bearer {}", self.config.keystore_token))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .context("Failed to send keystore decrypt request")?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().unwrap_or_default();
            error!("Keystore decrypt failed: {} - {}", status, error_text);
            anyhow::bail!("Keystore decrypt failed: {} - {}", status, error_text);
        }

        #[derive(Deserialize)]
        struct DecryptResponse {
            key: String,
            value_base64: String,
        }

        let resp: DecryptResponse = response.json().context("Failed to parse keystore decrypt response")?;

        Ok(DecryptedData {
            key: resp.key,
            value: base64_decode(&resp.value_base64)?,
        })
    }

    /// Set a storage key-value pair
    pub fn set(&self, key: &str, value: &[u8]) -> Result<()> {
        self.set_for_account(key, value, &self.config.account_id)
    }

    /// Set a storage key-value pair for a specific account
    pub fn set_for_account(&self, key: &str, value: &[u8], account_id: &str) -> Result<()> {
        // Encrypt via keystore
        let encrypted = self.encrypt_via_keystore(key, value, account_id)?;

        debug!(
            "storage_set: key_hash={}, account={}, value_size={}",
            encrypted.key_hash,
            account_id,
            value.len()
        );

        // Store in coordinator
        let body = serde_json::json!({
            "project_uuid": &self.config.project_uuid,
            "wasm_hash": self.config.wasm_hash,
            "account_id": account_id,
            "key_hash": encrypted.key_hash,
            "encrypted_key": encrypted.encrypted_key,
            "encrypted_value": encrypted.encrypted_value,
        });

        let response = self
            .client
            .post(format!("{}/storage/set", self.config.coordinator_url))
            .header("Authorization", format!("Bearer {}", self.config.coordinator_token))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .context("Failed to send storage set request")?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().unwrap_or_default();
            error!("Storage set failed: {} - {}", status, error_text);
            anyhow::bail!("Storage set failed: {} - {}", status, error_text);
        }

        Ok(())
    }

    /// Get a storage value by key
    pub fn get(&self, key: &str) -> Result<Option<Vec<u8>>> {
        self.get_for_account(key, &self.config.account_id)
    }

    /// Get a storage value for a specific account
    pub fn get_for_account(&self, key: &str, account_id: &str) -> Result<Option<Vec<u8>>> {
        let key_hash = self.hash_key(key);

        debug!("storage_get: key_hash={}, account={}", key_hash, account_id);

        let body = serde_json::json!({
            "project_uuid": &self.config.project_uuid,
            "account_id": account_id,
            "key_hash": key_hash,
        });

        let response = self
            .client
            .post(format!("{}/storage/get", self.config.coordinator_url))
            .header("Authorization", format!("Bearer {}", self.config.coordinator_token))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .context("Failed to send storage get request")?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().unwrap_or_default();
            error!("Storage get failed: {} - {}", status, error_text);
            anyhow::bail!("Storage get failed: {} - {}", status, error_text);
        }

        #[derive(Deserialize)]
        struct GetResponse {
            exists: bool,
            encrypted_key: Option<Vec<u8>>,
            encrypted_value: Option<Vec<u8>>,
        }

        let resp: GetResponse = response.json().context("Failed to parse storage get response")?;

        if !resp.exists {
            return Ok(None);
        }

        match (resp.encrypted_key, resp.encrypted_value) {
            (Some(enc_key), Some(enc_value)) => {
                // Decrypt via keystore
                let decrypted = self.decrypt_via_keystore(&enc_key, &enc_value, account_id)?;
                Ok(Some(decrypted.value))
            }
            _ => Ok(None),
        }
    }

    /// Check if a key exists
    pub fn has(&self, key: &str) -> Result<bool> {
        let key_hash = self.hash_key(key);

        let body = serde_json::json!({
            "project_uuid": &self.config.project_uuid,
            "account_id": self.config.account_id,
            "key_hash": key_hash,
        });

        let response = self
            .client
            .post(format!("{}/storage/has", self.config.coordinator_url))
            .header("Authorization", format!("Bearer {}", self.config.coordinator_token))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .context("Failed to send storage has request")?;

        if !response.status().is_success() {
            return Ok(false);
        }

        #[derive(Deserialize)]
        struct HasResponse {
            exists: bool,
        }

        let resp: HasResponse = response.json().unwrap_or(HasResponse { exists: false });
        Ok(resp.exists)
    }

    /// Delete a key
    pub fn delete(&self, key: &str) -> Result<bool> {
        let key_hash = self.hash_key(key);

        let body = serde_json::json!({
            "project_uuid": &self.config.project_uuid,
            "account_id": self.config.account_id,
            "key_hash": key_hash,
        });

        let response = self
            .client
            .post(format!("{}/storage/delete", self.config.coordinator_url))
            .header("Authorization", format!("Bearer {}", self.config.coordinator_token))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .context("Failed to send storage delete request")?;

        Ok(response.status().is_success())
    }

    /// List keys with optional prefix filter
    pub fn list_keys(&self, prefix: &str) -> Result<String> {
        // Note: prefix filtering requires decryption of all keys first,
        // then we filter client-side after decryption

        let url = format!(
            "{}/storage/list?account_id={}&project_uuid={}",
            self.config.coordinator_url,
            self.config.account_id,
            self.config.project_uuid
        );

        let response = self
            .client
            .get(&url)
            .header("Authorization", format!("Bearer {}", self.config.coordinator_token))
            .send()
            .context("Failed to send storage list request")?;

        if !response.status().is_success() {
            return Ok("[]".to_string());
        }

        #[derive(Deserialize)]
        struct ListResponse {
            keys: Vec<KeyInfo>,
        }

        #[derive(Deserialize)]
        struct KeyInfo {
            #[allow(dead_code)]
            key_hash: String,
            encrypted_key: Vec<u8>,
            encrypted_value: Vec<u8>,
        }

        let resp: ListResponse = response.json().unwrap_or(ListResponse { keys: vec![] });

        // Decrypt all keys via keystore and filter by prefix
        let mut decrypted_keys: Vec<String> = Vec::new();
        for key_info in resp.keys {
            match self.decrypt_via_keystore(&key_info.encrypted_key, &key_info.encrypted_value, &self.config.account_id) {
                Ok(decrypted) => {
                    // Apply prefix filter after decryption
                    if prefix.is_empty() || decrypted.key.starts_with(prefix) {
                        decrypted_keys.push(decrypted.key);
                    }
                }
                Err(e) => {
                    warn!("Failed to decrypt key during list: {}", e);
                }
            }
        }

        serde_json::to_string(&decrypted_keys).context("Failed to serialize keys")
    }

    /// Get value from a specific WASM version (for migration)
    pub fn get_by_version(&self, key: &str, wasm_hash: &str) -> Result<Option<Vec<u8>>> {
        let key_hash = self.hash_key(key);

        let body = serde_json::json!({
            "wasm_hash": wasm_hash,
            "account_id": self.config.account_id,
            "key_hash": key_hash,
        });

        let response = self
            .client
            .post(format!("{}/storage/get-by-version", self.config.coordinator_url))
            .header("Authorization", format!("Bearer {}", self.config.coordinator_token))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .context("Failed to send storage get-by-version request")?;

        if !response.status().is_success() {
            return Ok(None);
        }

        #[derive(Deserialize)]
        struct GetResponse {
            exists: bool,
            encrypted_key: Option<Vec<u8>>,
            encrypted_value: Option<Vec<u8>>,
        }

        let resp: GetResponse = response.json().context("Failed to parse response")?;

        if !resp.exists {
            return Ok(None);
        }

        match (resp.encrypted_key, resp.encrypted_value) {
            (Some(enc_key), Some(enc_value)) => {
                // For version-specific get, we need to use the old wasm_hash for decryption
                let body = serde_json::json!({
                    "project_uuid": self.config.project_uuid,
                    "wasm_hash": wasm_hash, // Use old wasm_hash
                    "account_id": self.config.account_id,
                    "encrypted_key_base64": base64_encode(&enc_key),
                    "encrypted_value_base64": base64_encode(&enc_value),
                    "attestation": Attestation::for_mode(&self.config.tee_mode),
                });

                let response = self
                    .client
                    .post(format!("{}/storage/decrypt", self.config.keystore_url))
                    .header("Authorization", format!("Bearer {}", self.config.keystore_token))
                    .header("Content-Type", "application/json")
                    .json(&body)
                    .send()
                    .context("Failed to send keystore decrypt request")?;

                if !response.status().is_success() {
                    return Ok(None);
                }

                #[derive(Deserialize)]
                struct DecryptResponse {
                    #[allow(dead_code)]
                    key: String,
                    value_base64: String,
                }

                let resp: DecryptResponse = response.json().context("Failed to parse decrypt response")?;
                Ok(Some(base64_decode(&resp.value_base64)?))
            }
            _ => Ok(None),
        }
    }

    /// Clear all storage for this project/account
    pub fn clear_all(&self) -> Result<()> {
        let body = serde_json::json!({
            "project_uuid": &self.config.project_uuid,
            "account_id": self.config.account_id,
        });

        let response = self
            .client
            .post(format!("{}/storage/clear-all", self.config.coordinator_url))
            .header("Authorization", format!("Bearer {}", self.config.coordinator_token))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .context("Failed to send storage clear-all request")?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().unwrap_or_default();
            anyhow::bail!("Storage clear-all failed: {} - {}", status, error_text);
        }

        Ok(())
    }

    /// Clear storage for a specific WASM version
    pub fn clear_version(&self, wasm_hash: &str) -> Result<()> {
        let body = serde_json::json!({
            "wasm_hash": wasm_hash,
            "account_id": self.config.account_id,
        });

        let response = self
            .client
            .post(format!("{}/storage/clear-version", self.config.coordinator_url))
            .header("Authorization", format!("Bearer {}", self.config.coordinator_token))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .context("Failed to send storage clear-version request")?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().unwrap_or_default();
            anyhow::bail!("Storage clear-version failed: {} - {}", status, error_text);
        }

        Ok(())
    }

    /// Set worker-private storage (account_id = "@worker")
    pub fn set_worker(&self, key: &str, value: &[u8]) -> Result<()> {
        self.set_for_account(key, value, "@worker")
    }

    /// Get worker-private storage (account_id = "@worker")
    pub fn get_worker(&self, key: &str) -> Result<Option<Vec<u8>>> {
        self.get_for_account(key, "@worker")
    }
}

/// Encrypted data from keystore
struct EncryptedData {
    encrypted_key: Vec<u8>,
    encrypted_value: Vec<u8>,
    key_hash: String,
}

/// Decrypted data from keystore
struct DecryptedData {
    key: String,
    value: Vec<u8>,
}

// Base64 helpers
fn base64_encode(data: &[u8]) -> String {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD.encode(data)
}

fn base64_decode(data: &str) -> Result<Vec<u8>> {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD
        .decode(data)
        .context("Invalid base64")
}
