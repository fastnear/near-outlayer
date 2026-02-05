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
    /// Keystore TEE session ID (set after challenge-response registration)
    pub keystore_tee_session_id: Option<String>,
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
    /// Create a dev-mode attestation stub
    fn dev_stub() -> Self {
        Self {
            tee_type: "none".to_string(),
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
            "outlayer_tee" => Self::dev_stub(), // Attestation is a no-op; TEE sessions handle auth
            "none" => Self::dev_stub(),
            _ => Self::dev_stub(),
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

    /// Build a keystore request with auth headers (Bearer token + optional X-TEE-Session)
    fn keystore_request(&self, method: reqwest::Method, path: &str) -> reqwest::blocking::RequestBuilder {
        let mut req = self
            .client
            .request(method, format!("{}{}", self.config.keystore_url, path))
            .header("Authorization", format!("Bearer {}", self.config.keystore_token))
            .header("Content-Type", "application/json");
        if let Some(ref session_id) = self.config.keystore_tee_session_id {
            req = req.header("X-TEE-Session", session_id.as_str());
        }
        req
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
            .keystore_request(reqwest::Method::POST, "/storage/encrypt")
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
            .keystore_request(reqwest::Method::POST, "/storage/decrypt")
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
                    .keystore_request(reqwest::Method::POST, "/storage/decrypt")
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

    /// Set worker storage (account_id = "@worker")
    /// is_encrypted: true = encrypt via keystore (default, private to project)
    ///               false = store plaintext (public, readable by other projects)
    pub fn set_worker(&self, key: &str, value: &[u8], is_encrypted: bool) -> Result<()> {
        if is_encrypted {
            // Encrypted: use keystore
            self.set_for_account(key, value, "@worker")
        } else {
            // Public: store plaintext directly (no keystore)
            self.set_public(key, value)
        }
    }

    /// Set public storage (plaintext, no encryption)
    fn set_public(&self, key: &str, value: &[u8]) -> Result<()> {
        let key_hash = self.hash_key(key);

        debug!(
            "storage_set_public: key_hash={}, value_size={}",
            key_hash,
            value.len()
        );

        // Store plaintext key and value directly (no keystore encryption)
        let body = serde_json::json!({
            "project_uuid": &self.config.project_uuid,
            "wasm_hash": self.config.wasm_hash,
            "account_id": "@worker",
            "key_hash": key_hash,
            "encrypted_key": key.as_bytes(),      // plaintext key
            "encrypted_value": value,              // plaintext value
            "is_encrypted": false,
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
            error!("Storage set_public failed: {} - {}", status, error_text);
            anyhow::bail!("Storage set_public failed: {} - {}", status, error_text);
        }

        Ok(())
    }

    /// Get worker storage (account_id = "@worker")
    /// project_uuid: None = read from current project (private or public)
    ///               Some("p0000000000000001") = read public data from another project by UUID
    pub fn get_worker(&self, key: &str, project_uuid: Option<&str>) -> Result<Option<Vec<u8>>> {
        match project_uuid {
            None => {
                // Read from own project - may be encrypted or public
                self.get_worker_own(key)
            }
            Some(uuid) => {
                // Read from another project - only public data allowed
                self.get_worker_public(key, uuid)
            }
        }
    }

    /// Get worker storage from own project (handles both encrypted and public)
    fn get_worker_own(&self, key: &str) -> Result<Option<Vec<u8>>> {
        let key_hash = self.hash_key(key);

        debug!("storage_get_worker_own: key_hash={}", key_hash);

        let body = serde_json::json!({
            "project_uuid": &self.config.project_uuid,
            "account_id": "@worker",
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
            #[serde(default = "default_true")]
            is_encrypted: bool,
        }

        fn default_true() -> bool { true }

        let resp: GetResponse = response.json().context("Failed to parse storage get response")?;

        if !resp.exists {
            return Ok(None);
        }

        match (resp.encrypted_key, resp.encrypted_value) {
            (Some(enc_key), Some(enc_value)) => {
                if resp.is_encrypted {
                    // Encrypted: decrypt via keystore
                    let decrypted = self.decrypt_via_keystore(&enc_key, &enc_value, "@worker")?;
                    Ok(Some(decrypted.value))
                } else {
                    // Public: value is plaintext
                    Ok(Some(enc_value))
                }
            }
            _ => Ok(None),
        }
    }

    /// Get public worker storage from another project by UUID
    fn get_worker_public(&self, key: &str, project_uuid: &str) -> Result<Option<Vec<u8>>> {
        let key_hash = self.hash_key(key);

        debug!(
            "storage_get_worker_public: project_uuid={}, key_hash={}",
            project_uuid, key_hash
        );

        // Request public storage from coordinator
        // Coordinator will check is_encrypted=false before returning
        let body = serde_json::json!({
            "project_uuid": project_uuid,
            "key_hash": key_hash,
        });

        let response = self
            .client
            .post(format!("{}/storage/get-public", self.config.coordinator_url))
            .header("Authorization", format!("Bearer {}", self.config.coordinator_token))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .context("Failed to send storage get-public request")?;

        if !response.status().is_success() {
            let status = response.status();
            if status == reqwest::StatusCode::NOT_FOUND {
                return Ok(None);
            }
            if status == reqwest::StatusCode::FORBIDDEN {
                anyhow::bail!("Storage key '{}' in project '{}' is not public (encrypted)", key, project_uuid);
            }
            let error_text = response.text().unwrap_or_default();
            error!("Storage get-public failed: {} - {}", status, error_text);
            anyhow::bail!("Storage get-public failed: {} - {}", status, error_text);
        }

        #[derive(Deserialize)]
        struct GetPublicResponse {
            exists: bool,
            value: Option<Vec<u8>>,
        }

        let resp: GetPublicResponse = response.json().context("Failed to parse storage get-public response")?;

        if !resp.exists {
            return Ok(None);
        }

        Ok(resp.value)
    }

    // ==================== Conditional Write Operations ====================

    /// Set a key only if it doesn't already exist
    /// Returns true if value was inserted, false if key already existed
    pub fn set_if_absent(&self, key: &str, value: &[u8]) -> Result<bool> {
        // Encrypt via keystore
        let encrypted = self.encrypt_via_keystore(key, value, &self.config.account_id)?;

        debug!(
            "storage_set_if_absent: key_hash={}, account={}, value_size={}",
            encrypted.key_hash,
            self.config.account_id,
            value.len()
        );

        // Try to insert via coordinator
        let body = serde_json::json!({
            "project_uuid": &self.config.project_uuid,
            "wasm_hash": self.config.wasm_hash,
            "account_id": self.config.account_id,
            "key_hash": encrypted.key_hash,
            "encrypted_key": encrypted.encrypted_key,
            "encrypted_value": encrypted.encrypted_value,
        });

        let response = self
            .client
            .post(format!("{}/storage/set-if-absent", self.config.coordinator_url))
            .header("Authorization", format!("Bearer {}", self.config.coordinator_token))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .context("Failed to send storage set-if-absent request")?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().unwrap_or_default();
            anyhow::bail!("Storage set-if-absent failed: {} - {}", status, error_text);
        }

        #[derive(Deserialize)]
        struct SetIfAbsentResponse {
            inserted: bool,
        }

        let resp: SetIfAbsentResponse = response.json().context("Failed to parse set-if-absent response")?;
        Ok(resp.inserted)
    }

    /// Set a key only if current value equals expected (compare-and-swap)
    /// Returns (success, current_value) where current_value is provided for retry on failure
    pub fn set_if_equals(&self, key: &str, expected: &[u8], new_value: &[u8]) -> Result<(bool, Option<Vec<u8>>)> {
        // First, get current encrypted value to pass to coordinator
        let key_hash = self.hash_key(key);

        let get_body = serde_json::json!({
            "project_uuid": &self.config.project_uuid,
            "account_id": self.config.account_id,
            "key_hash": key_hash,
        });

        let get_response = self
            .client
            .post(format!("{}/storage/get", self.config.coordinator_url))
            .header("Authorization", format!("Bearer {}", self.config.coordinator_token))
            .header("Content-Type", "application/json")
            .json(&get_body)
            .send()
            .context("Failed to send storage get request for set_if_equals")?;

        if !get_response.status().is_success() {
            anyhow::bail!("Storage get failed during set_if_equals");
        }

        #[derive(Deserialize)]
        struct GetResponse {
            exists: bool,
            encrypted_key: Option<Vec<u8>>,
            encrypted_value: Option<Vec<u8>>,
        }

        let get_resp: GetResponse = get_response.json().context("Failed to parse get response")?;

        if !get_resp.exists {
            // Key doesn't exist - can't do CAS
            return Ok((false, None));
        }

        let (current_enc_key, current_enc_value) = match (get_resp.encrypted_key, get_resp.encrypted_value) {
            (Some(k), Some(v)) => (k, v),
            _ => return Ok((false, None)),
        };

        // Decrypt current value to compare with expected
        let decrypted = self.decrypt_via_keystore(&current_enc_key, &current_enc_value, &self.config.account_id)?;

        if decrypted.value != expected {
            // Current value doesn't match expected - return current value for retry
            return Ok((false, Some(decrypted.value)));
        }

        // Values match - encrypt new value and try to update
        let new_encrypted = self.encrypt_via_keystore(key, new_value, &self.config.account_id)?;

        let update_body = serde_json::json!({
            "project_uuid": &self.config.project_uuid,
            "wasm_hash": self.config.wasm_hash,
            "account_id": self.config.account_id,
            "key_hash": key_hash,
            "expected_encrypted_value": current_enc_value,
            "new_encrypted_key": new_encrypted.encrypted_key,
            "new_encrypted_value": new_encrypted.encrypted_value,
        });

        let update_response = self
            .client
            .post(format!("{}/storage/set-if-equals", self.config.coordinator_url))
            .header("Authorization", format!("Bearer {}", self.config.coordinator_token))
            .header("Content-Type", "application/json")
            .json(&update_body)
            .send()
            .context("Failed to send storage set-if-equals request")?;

        if !update_response.status().is_success() {
            let status = update_response.status();
            let error_text = update_response.text().unwrap_or_default();
            anyhow::bail!("Storage set-if-equals failed: {} - {}", status, error_text);
        }

        #[derive(Deserialize)]
        struct SetIfEqualsResponse {
            updated: bool,
            current_encrypted_value: Option<Vec<u8>>,
            current_encrypted_key: Option<Vec<u8>>,
        }

        let update_resp: SetIfEqualsResponse = update_response.json().context("Failed to parse set-if-equals response")?;

        if update_resp.updated {
            Ok((true, None))
        } else {
            // Concurrent modification - decrypt current value for retry
            if let (Some(enc_key), Some(enc_value)) = (update_resp.current_encrypted_key, update_resp.current_encrypted_value) {
                let current = self.decrypt_via_keystore(&enc_key, &enc_value, &self.config.account_id)?;
                Ok((false, Some(current.value)))
            } else {
                Ok((false, None))
            }
        }
    }

    /// Atomically increment a numeric value
    /// If key doesn't exist, creates it with delta as initial value
    /// Returns the new value after increment
    pub fn increment(&self, key: &str, delta: i64) -> Result<i64> {
        // MAX_RETRIES needed for CAS (compare-and-swap) pattern: if another execution
        // modifies the same key concurrently, our expected value won't match and we
        // retry with the new value. Common for worker storage shared across executions.
        const MAX_RETRIES: usize = 5;

        for attempt in 0..MAX_RETRIES {
            // Get current value
            let current_opt = self.get(key)?;

            match current_opt {
                None => {
                    // Key doesn't exist - try to create with initial value
                    let new_value = delta;
                    let value_bytes = new_value.to_le_bytes().to_vec();

                    if self.set_if_absent(key, &value_bytes)? {
                        debug!("increment: created key={} with initial value={}", key, new_value);
                        return Ok(new_value);
                    }
                    // Key was created by someone else - retry
                    debug!("increment: concurrent create detected, retrying (attempt {})", attempt + 1);
                }
                Some(current_bytes) => {
                    // Parse current value as i64
                    let current_value = if current_bytes.len() == 8 {
                        i64::from_le_bytes(current_bytes.clone().try_into().unwrap())
                    } else {
                        anyhow::bail!("increment: invalid value format, expected 8 bytes (i64), got {}", current_bytes.len());
                    };

                    let new_value = current_value.checked_add(delta)
                        .context("increment: overflow")?;
                    let new_bytes = new_value.to_le_bytes().to_vec();

                    let (success, _) = self.set_if_equals(key, &current_bytes, &new_bytes)?;
                    if success {
                        debug!("increment: updated key={} from {} to {}", key, current_value, new_value);
                        return Ok(new_value);
                    }
                    // Concurrent modification - retry
                    debug!("increment: concurrent modification detected, retrying (attempt {})", attempt + 1);
                }
            }
        }

        anyhow::bail!("increment: max retries ({}) exceeded for key={}", MAX_RETRIES, key)
    }

    /// Atomically decrement a numeric value
    /// If key doesn't exist, creates it with -delta as initial value
    /// Returns the new value after decrement
    pub fn decrement(&self, key: &str, delta: i64) -> Result<i64> {
        // decrement(delta) is just increment(-delta)
        self.increment(key, -delta)
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
