//! Client for communicating with keystore worker
//!
//! Handles secret decryption requests and the TEE session handshake.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

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

/// No secret exists for the accessor that was asked for.
///
/// Distinct from every other failure on purpose: it is the only one a caller
/// may respond to by asking a different question.
#[derive(Debug)]
pub struct SecretsNotFound;

impl std::fmt::Display for SecretsNotFound {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "no secret for that accessor")
    }
}

impl std::error::Error for SecretsNotFound {}

impl SecretsNotFound {
    /// Does this failure mean "there is no such secret"?
    ///
    /// The one question a caller may answer by running anyway, with no secrets.
    /// Every other failure — a refusal, an unreadable blob, a keystore that is
    /// down — must stop the job, because running without a credential that was
    /// supposed to be there is worse than not running.
    ///
    /// Asked of the typed marker rather than of the message. The message is
    /// user-facing prose assembled from the keystore's own words, and a refusal
    /// whose text happened to contain "not found" would otherwise be read as
    /// "no secrets" — a job silently running with none, on an ACCESS-CONTROL
    /// refusal of all things.
    pub fn is_missing(error: &anyhow::Error) -> bool {
        error.downcast_ref::<SecretsNotFound>().is_some()
    }
}

/// Client for keystore worker API
#[derive(Clone)]
pub struct KeystoreClient {
    base_url: String,
    auth_token: String,
    http_client: reqwest::Client,
    /// TEE session ID (set after successful challenge-response registration)
    tee_session_id: std::sync::Arc<std::sync::Mutex<Option<String>>>,
    /// TEE signing info for auto-reconnect (public key bytes + signing key)
    tee_signing_info: Option<std::sync::Arc<near_crypto::SecretKey>>,
}

impl KeystoreClient {
    /// Create new keystore client
    pub fn new(base_url: String, auth_token: String) -> Self {
        Self {
            base_url,
            auth_token,
            http_client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .connect_timeout(std::time::Duration::from_secs(10))
                .build()
                .expect("Failed to build keystore HTTP client"),
            tee_session_id: std::sync::Arc::new(std::sync::Mutex::new(None)),
            tee_signing_info: None,
        }
    }

    /// Set TEE signing info for auto-reconnect on session expiry
    pub fn set_tee_signing_info(&mut self, secret_key: near_crypto::SecretKey) {
        self.tee_signing_info = Some(std::sync::Arc::new(secret_key));
    }

    /// Set TEE session ID (called after successful challenge-response registration)
    pub fn set_tee_session_id(&self, session_id: String) {
        *self.tee_session_id.lock().unwrap() = Some(session_id);
    }

    /// Get TEE session ID (for passing to StorageClient)
    pub fn get_tee_session_id(&self) -> Option<String> {
        self.tee_session_id.lock().unwrap().clone()
    }

    /// Register TEE session directly with keystore via challenge-response.
    ///
    /// 1. POST {keystore}/tee-challenge → get challenge
    /// 2. Sign challenge with ed25519 key
    /// 3. POST {keystore}/register-tee → get session_id
    ///
    /// This bypasses the coordinator proxy, ensuring the session is registered
    /// on the same keystore instance that handles /decrypt requests.
    pub async fn register_tee_session(
        &self,
        secret_key: &near_crypto::SecretKey,
    ) -> Result<String> {
        // NEAR canonical form carries the scheme (ed25519 / ml-dsa-65) in its prefix.
        let near_public_key = secret_key.public_key().to_string();

        // 1. Request challenge
        let url = format!("{}/tee-challenge", self.base_url);
        let response = self.http_client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.auth_token))
            .send()
            .await
            .context("Failed to request TEE challenge from keystore")?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            anyhow::bail!("Keystore TEE challenge failed ({}): {}", status, text);
        }

        #[derive(serde::Deserialize)]
        struct ChallengeResponse {
            challenge: String,
        }
        let challenge_resp: ChallengeResponse = response.json().await
            .context("Failed to parse keystore TEE challenge")?;

        // 2. Sign challenge (ed25519 or ml-dsa-65), send signature in NEAR form.
        let challenge_bytes = hex::decode(&challenge_resp.challenge)
            .context("Invalid challenge hex")?;
        let signature = secret_key.sign(&challenge_bytes).to_string();

        // 3. Register with signed challenge
        let url = format!("{}/register-tee", self.base_url);
        let response = self.http_client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.auth_token))
            .json(&serde_json::json!({
                "public_key": near_public_key,
                "challenge": challenge_resp.challenge,
                "signature": signature,
            }))
            .send()
            .await
            .context("Failed to submit keystore TEE registration")?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            anyhow::bail!("Keystore TEE registration failed ({}): {}", status, text);
        }

        #[derive(serde::Deserialize)]
        struct RegisterResponse {
            session_id: String,
        }
        let register_resp: RegisterResponse = response.json().await
            .context("Failed to parse keystore TEE registration response")?;

        self.set_tee_session_id(register_resp.session_id.clone());
        tracing::info!(
            session_id = %register_resp.session_id,
            "TEE session registered directly with keystore"
        );

        Ok(register_resp.session_id)
    }

    /// Check if an HTTP error response indicates TEE session expiry.
    /// Parses JSON `{"error": "..."}` and checks for session-related keywords.
    fn is_tee_session_expired(status: reqwest::StatusCode, body: &str) -> bool {
        if status != reqwest::StatusCode::FORBIDDEN {
            return false;
        }
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(body) {
            if let Some(error) = json.get("error").and_then(|e| e.as_str()) {
                return error.contains("session not found") || error.contains("session expired");
            }
        }
        // Fallback: plain text match
        body.contains("session not found")
    }

    /// Try to re-register TEE session if signing info is available.
    /// Returns Ok(()) on success, Err if reconnect failed or no signing info.
    async fn try_reconnect_tee_session(&self) -> Result<()> {
        let secret_key = self.tee_signing_info.as_ref()
            .context("No TEE signing info for reconnect")?
            .clone();
        tracing::warn!("TEE session expired on keystore, re-registering...");
        let session_id = self.register_tee_session(&secret_key).await?;
        tracing::info!(session_id = %session_id, "TEE session re-registered");
        Ok(())
    }

    /// POST a JSON body, re-establishing the TEE session and retrying once when the keystore
    /// rejects the request because the session is gone.
    ///
    /// Only `/decrypt` used to recover this way; `/encrypt` and `/decrypt-raw` failed hard until
    /// the worker was restarted by hand. One keystore restart invalidates every session at once
    /// (they live in its memory), so these paths recover on their own now.
    ///
    /// Deliberately NOT covering `/storage/*` and `/vrf/generate`: those run inside a WASI
    /// execution on blocking clients that were handed a session id by value at job start, so a
    /// keystore restart mid-job fails that job — accepted behaviour, not an oversight.
    ///
    /// `build_body` is a closure so the retry re-serializes from scratch rather than reusing a
    /// half-consumed request.
    async fn post_with_session_retry_scoped<T: Serialize>(
        &self,
        url: &str,
        build_body: impl Fn() -> Result<T>,
        what: &str,
        vault_id: Option<&str>,
    ) -> Result<reqwest::Response> {
        let response = Self::add_vault_header(self.add_auth_headers(self.http_client.post(url)), vault_id)
            .json(&build_body()?)
            .send()
            .await
            .with_context(|| format!("Failed to send {} request", what))?;

        if response.status().is_success() {
            return Ok(response);
        }

        let status = response.status();
        let error_text = response.text().await.unwrap_or_default();
        if !Self::is_tee_session_expired(status, &error_text) {
            anyhow::bail!("{} request failed ({}): {}", what, status, error_text);
        }

        // Unlike `/decrypt`, which keeps its own inline retry so it can map keystore 4xx into
        // user-facing messages, these endpoints have no such mapping — so the reconnect
        // failure is surfaced as the error itself rather than logged and swallowed.
        self.try_reconnect_tee_session()
            .await
            .with_context(|| format!("{} got {} and the TEE session reconnect failed", what, status))?;

        // The retry carries the SAME scope. Dropping it here would silently
        // reach for the default master on the second attempt, which reads a
        // vault customer's blob with the wrong key and fails in a way that
        // looks like corruption rather than a missing header.
        let retry = Self::add_vault_header(
            self.add_auth_headers(self.http_client.post(url)),
            vault_id,
        )
        .json(&build_body()?)
        .send()
        .await
        .with_context(|| format!("Failed to send {} retry request", what))?;

        if !retry.status().is_success() {
            let retry_status = retry.status();
            let retry_body = retry.text().await.unwrap_or_default();
            anyhow::bail!(
                "{} failed again after session reconnect ({}): {}",
                what,
                retry_status,
                retry_body
            );
        }
        Ok(retry)
    }

    /// Parse a successful decrypt response into a HashMap of env vars.
    fn parse_decrypt_response(response_bytes: &[u8]) -> Result<std::collections::HashMap<String, String>> {
        let decrypt_response: DecryptResponse = serde_json::from_slice(response_bytes)
            .context("Failed to parse decrypt response")?;
        let plaintext = base64::decode(&decrypt_response.plaintext_secrets)
            .context("Failed to decode plaintext secrets")?;
        let plaintext_str = String::from_utf8(plaintext)
            .context("Invalid secrets format: not valid UTF-8 text")?;
        serde_json::from_str(&plaintext_str)
            .context("Invalid secrets format: must be a JSON object with string key-value pairs")
    }

    /// Add the vault scope, when the material belongs to one.
    ///
    /// Absent header means the default master, which is where every key of a
    /// wallet without a vault lives. The header must match how the blob was
    /// written: a mismatch does not corrupt anything, it simply fails to
    /// decrypt — loud, but a lockout.
    fn add_vault_header(
        builder: reqwest::RequestBuilder,
        vault_id: Option<&str>,
    ) -> reqwest::RequestBuilder {
        match vault_id.map(str::trim).filter(|v| !v.is_empty()) {
            Some(vault) => builder.header("X-Customer-Vault", vault),
            None => builder,
        }
    }

    /// Add auth headers: Bearer token + optional X-TEE-Session
    fn add_auth_headers(&self, builder: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        let builder = builder.header("Authorization", format!("Bearer {}", self.auth_token));
        if let Some(session_id) = self.tee_session_id.lock().unwrap().as_ref() {
            builder.header("X-TEE-Session", session_id.as_str())
        } else {
            builder
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

        // Prepare request with accessor
        #[derive(Debug, Serialize)]
        struct DecryptRequest {
            accessor: SecretAccessor,
            profile: String,
            owner: String,
            user_account_id: String,
            task_id: Option<String>,
        }

        let request = DecryptRequest {
            accessor: accessor.clone(),
            profile: profile.to_string(),
            owner: owner.to_string(),
            user_account_id: user_account_id.to_string(),
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

        // Log TEE session info for debugging
        let tee_session = self.tee_session_id.lock().unwrap().clone();
        tracing::info!(
            tee_session_id = ?tee_session,
            url = %url,
            "🔑 Sending decrypt request to keystore"
        );

        let response = self.add_auth_headers(self.http_client.post(&url))
            .json(&request)
            .send()
            .await
            .context("Failed to send decrypt request")?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();

            // Auto-reconnect: if 403 with session expired, re-register and retry once
            if Self::is_tee_session_expired(status, &error_text) {
                // Log why a reconnect failed before falling through. Discarding it (the old
                // `if let Ok(())`) left only the original 403 in the logs, which reads as
                // "the keystore rejected us" even when the real cause is on our side — e.g.
                // no signing key to re-handshake with.
                let reconnected = self.try_reconnect_tee_session().await;
                if let Err(ref e) = reconnected {
                    tracing::error!(error = %e, "TEE session reconnect failed; reporting the original error");
                }
                if reconnected.is_ok() {
                    // Retry the request with new session
                    let retry_request = DecryptRequest {
                        accessor: accessor.clone(),
                        profile: profile.to_string(),
                        owner: owner.to_string(),
                        user_account_id: user_account_id.to_string(),
                        task_id: task_id.map(|s| s.to_string()),
                    };
                    let retry_response = self.add_auth_headers(self.http_client.post(&url))
                        .json(&retry_request)
                        .send()
                        .await
                        .context("Failed to send retry decrypt request")?;

                    if retry_response.status().is_success() {
                        let body = retry_response.bytes().await
                            .context("Failed to read retry decrypt response")?;
                        let env_vars = Self::parse_decrypt_response(&body)?;
                        tracing::info!(
                            accessor = %accessor_desc,
                            profile = %profile,
                            env_count = env_vars.len(),
                            "Successfully decrypted secrets (after reconnect)"
                        );
                        return Ok(env_vars);
                    }
                    let retry_status = retry_response.status();
                    let retry_error = retry_response.text().await.unwrap_or_default();
                    tracing::error!(status = %retry_status, "Decrypt retry also failed after reconnect");
                    anyhow::bail!("Failed to decrypt secrets after TEE session reconnect ({}): {}", retry_status, retry_error);
                }
                // Reconnect failed — fall through to normal error handling
            }

            let truncated_body: String = error_text.chars().take(500).collect();
            tracing::error!(
                status = %status,
                error_body = %truncated_body,
                tee_session_id = ?tee_session,
                "🔒 Keystore /decrypt failed"
            );

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

            // A typed marker for "there is no such secret", so a caller can
            // tell it apart from "the keystore refused" or "the keystore is
            // down". String-matching the message from outside would break the
            // moment the wording changed, and the caller deciding to fall back
            // must never do so because a keystore was merely unreachable.
            let not_found = status == 404 || (status == 400 && error_text.contains("not found"));
            if not_found {
                return Err(anyhow::Error::new(SecretsNotFound).context(user_message));
            }
            anyhow::bail!("{}", user_message);
        }

        let body = response.bytes().await
            .context("Failed to read decrypt response")?;
        let env_vars = Self::parse_decrypt_response(&body)?;

        tracing::info!(
            accessor = %accessor_desc,
            profile = %profile,
            env_count = env_vars.len(),
            "Successfully decrypted secrets"
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

    /// Encrypt data using keystore's derived key
    ///
    /// Used for TopUp flow to re-encrypt Payment Key data with updated balance.
    ///
    /// # Arguments
    /// * `seed` - Seed for key derivation (format: "system:payment_key:{owner}:{nonce}")
    /// * `plaintext` - Raw bytes to encrypt
    ///
    /// # Returns
    /// * `Ok(encrypted_base64)` - Base64 encoded encrypted data
    pub async fn encrypt(
        &self,
        seed: &str,
        plaintext: &[u8],
        vault_id: Option<&str>,
    ) -> Result<String> {
        tracing::info!(
            seed = %seed,
            plaintext_len = plaintext.len(),
            "🔐 Encrypting data via keystore"
        );

        // Prepare request
        #[derive(Debug, Serialize)]
        struct EncryptRequest {
            seed: String,
            plaintext_base64: String,
        }

        #[derive(Debug, Deserialize)]
        struct EncryptResponse {
            encrypted_base64: String,
        }

        let plaintext_base64 = base64::encode(plaintext);

        // Send request to keystore
        let url = format!("{}/encrypt", self.base_url);

        let response = self
            .post_with_session_retry_scoped(
                &url,
                || {
                    Ok(EncryptRequest {
                        seed: seed.to_string(),
                        plaintext_base64: plaintext_base64.clone(),
                    })
                },
                "Encrypt",
                vault_id,
            )
            .await?;

        let encrypt_response: EncryptResponse = response
            .json()
            .await
            .context("Failed to parse encrypt response")?;

        tracing::info!(
            seed = %seed,
            encrypted_len = encrypt_response.encrypted_base64.len(),
            "Successfully encrypted data"
        );

        Ok(encrypt_response.encrypted_base64)
    }

    /// Decrypt raw encrypted data using keystore's derived key
    ///
    /// Used for TopUp flow to decrypt Payment Key data.
    ///
    /// # Arguments
    /// * `seed` - Seed for key derivation (format: "system:payment_key:{owner}:{nonce}")
    /// * `encrypted_base64` - Base64 encoded encrypted data
    ///
    /// # Returns
    /// * `Ok(plaintext)` - Decrypted bytes
    pub async fn decrypt_raw(
        &self,
        seed: &str,
        encrypted_base64: &str,
        vault_id: Option<&str>,
    ) -> Result<Vec<u8>> {
        tracing::info!(
            seed = %seed,
            encrypted_len = encrypted_base64.len(),
            "🔓 Decrypting raw data via keystore"
        );

        // For raw decryption, we need a different approach
        // The keystore's /decrypt endpoint expects accessor/profile/owner
        // For Payment Keys, we need to use the System accessor

        // For TopUp, we pass encrypted data directly (from the event)
        // and keystore decrypts using the seed
        #[derive(Debug, Serialize)]
        struct DecryptRawRequest {
            seed: String,
            encrypted_base64: String,
        }

        #[derive(Debug, Deserialize)]
        struct DecryptRawResponse {
            plaintext_base64: String,
        }

        // Send request to keystore (using /decrypt-raw endpoint for direct decryption)
        let url = format!("{}/decrypt-raw", self.base_url);

        let response = self
            .post_with_session_retry_scoped(
                &url,
                || {
                    Ok(DecryptRawRequest {
                        seed: seed.to_string(),
                        encrypted_base64: encrypted_base64.to_string(),
                    })
                },
                "Decrypt-raw",
                vault_id,
            )
            .await?;

        let decrypt_response: DecryptRawResponse = response
            .json()
            .await
            .context("Failed to parse decrypt-raw response")?;

        let plaintext = base64::decode(&decrypt_response.plaintext_base64)
            .context("Failed to decode plaintext from base64")?;

        tracing::info!(
            seed = %seed,
            plaintext_len = plaintext.len(),
            "Successfully decrypted raw data"
        );

        Ok(plaintext)
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

    // ===== post_with_session_retry: recovery after the keystore forgets its sessions =====

    /// A throwaway keystore: answers the first business request with the keystore's own
    /// session-expired 403, serves the challenge-response handshake, then answers 200.
    ///
    /// Every response closes the connection so the client's pool cannot outlive a step.
    fn fake_keystore(reject_first: bool) -> (String, std::thread::JoinHandle<Vec<String>>) {
        use std::io::{Read, Write};

        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
        let addr = listener.local_addr().expect("addr");
        let handle = std::thread::spawn(move || {
            let mut seen = Vec::new();
            let mut business_calls = 0;
            for stream in listener.incoming().take(if reject_first { 4 } else { 1 }) {
                let mut stream = match stream {
                    Ok(s) => s,
                    Err(_) => break,
                };
                let mut buf = [0u8; 8192];
                let n = stream.read(&mut buf).unwrap_or(0);
                let req = String::from_utf8_lossy(&buf[..n]).into_owned();
                let path = req
                    .split_whitespace()
                    .nth(1)
                    .unwrap_or("/")
                    .to_string();
                seen.push(path.clone());

                let (code, body) = match path.as_str() {
                    "/tee-challenge" => (200, r#"{"challenge":"deadbeef"}"#.to_string()),
                    "/register-tee" => (
                        200,
                        r#"{"session_id":"6f1e9d3a-0000-4000-8000-000000000001"}"#.to_string(),
                    ),
                    _ => {
                        business_calls += 1;
                        if reject_first && business_calls == 1 {
                            (403, r#"{"error":"TEE session not found"}"#.to_string())
                        } else {
                            (200, r#"{"encrypted_base64":"Y2lwaGVy"}"#.to_string())
                        }
                    }
                };

                let response = format!(
                    "HTTP/1.1 {code} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                let _ = stream.write_all(response.as_bytes());
            }
            seen
        });
        (format!("http://{}", addr), handle)
    }

    fn client_with_signing_key(base_url: String) -> KeystoreClient {
        let mut client = KeystoreClient::new(base_url, "test-token".to_string());
        client.set_tee_signing_info(near_crypto::SecretKey::from_random(
            near_crypto::KeyType::ED25519,
        ));
        client.set_tee_session_id("stale-session".to_string());
        client
    }

    /// The keystore restarted and forgot every session. The worker must re-handshake and finish
    /// the original call on its own — before this existed, only `/decrypt` recovered and every
    /// other endpoint stayed broken until someone restarted the worker.
    #[tokio::test]
    async fn encrypt_reestablishes_a_dropped_tee_session_and_succeeds() {
        let (url, server) = fake_keystore(true);
        let client = client_with_signing_key(url);

        let out = client.encrypt("seed", b"plaintext", None).await;

        let seen = server.join().expect("server thread");
        assert!(out.is_ok(), "expected recovery, got {out:?}");
        assert_eq!(
            seen,
            vec![
                "/encrypt".to_string(),
                "/tee-challenge".to_string(),
                "/register-tee".to_string(),
                "/encrypt".to_string(),
            ],
            "expected: rejected call, handshake, then the same call again"
        );
    }

    /// Without a signing key there is nothing to re-handshake with. The failure must name that
    /// cause — the old code discarded it and reported only the keystore's 403, which reads as
    /// "the keystore rejected us" when the problem is on our side.
    #[tokio::test]
    async fn encrypt_reports_why_a_reconnect_could_not_happen() {
        let (url, server) = fake_keystore(true);
        // No `set_tee_signing_info` — this is a worker that never got its key.
        let client = KeystoreClient::new(url, "test-token".to_string());

        let err = client
            .encrypt("seed", b"plaintext", None)
            .await
            .expect_err("must fail without a way to re-handshake");
        let chain = format!("{err:#}");

        assert!(
            chain.contains("No TEE signing info"),
            "the reconnect failure must be in the error chain, got: {chain}"
        );
        assert!(
            chain.contains("403"),
            "the original keystore status should still be visible, got: {chain}"
        );
        drop(server);
    }

    /// A keystore that answers one business call with a chosen status and body.
    fn fake_keystore_answering(code: u16, body: &'static str) -> String {
        use std::io::{Read, Write};

        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
        let addr = listener.local_addr().expect("addr");
        std::thread::spawn(move || {
            if let Some(Ok(mut stream)) = listener.incoming().next() {
                let mut buf = [0u8; 8192];
                let _ = stream.read(&mut buf);
                let response = format!(
                    "HTTP/1.1 {code} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                let _ = stream.write_all(response.as_bytes());
            }
        });
        format!("http://{}", addr)
    }

    async fn decrypt_against(code: u16, body: &'static str) -> anyhow::Error {
        let client = KeystoreClient::new(fake_keystore_answering(code, body), "t".to_string());
        client
            .decrypt_secrets_by_project("a.near/p", "prod", "a.near", "a.near", None)
            .await
            .expect_err("a non-2xx must be an error")
    }

    /// "There is no such secret" is the ONLY failure a job may shrug off and run
    /// without one. It is recognised by TYPE, and the type is what the worker
    /// asks about.
    ///
    /// The trap this guards is narrow and nasty: the user-facing message is
    /// assembled from the keystore's own words, so a REFUSAL whose text happens
    /// to contain "not found" would, under a string test, start the job with no
    /// credential — silently, on an access-control refusal. The 401 case below
    /// says exactly that phrase for exactly that reason.
    #[tokio::test]
    async fn only_a_missing_secret_lets_a_job_continue_without_one() {
        assert!(
            SecretsNotFound::is_missing(&decrypt_against(404, r#"{"error":"nope"}"#).await),
            "a 404 means there is no such secret"
        );
        assert!(
            SecretsNotFound::is_missing(
                &decrypt_against(400, r#"{"error":"secret not found"}"#).await
            ),
            "the keystore also reports it as a 400 saying so"
        );

        let refusal =
            decrypt_against(401, r#"{"error":"agent profile not found on this wallet"}"#).await;
        assert!(
            !SecretsNotFound::is_missing(&refusal),
            "a REFUSAL must stop the job even when its wording contains the words \
             the old string test looked for: {refusal:#}"
        );

        let broken = decrypt_against(500, r#"{"error":"boom"}"#).await;
        assert!(
            !SecretsNotFound::is_missing(&broken),
            "a keystore that is down is not a keystore saying the secret is absent"
        );
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
