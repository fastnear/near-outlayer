use anyhow::{Context, Result};
use reqwest::{Client, StatusCode};
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Task types that worker can receive
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Task {
    Compile {
        request_id: u64,
        data_id: String,
        code_source: CodeSource,
        resource_limits: ResourceLimits,
        input_data: String,
    },
    Execute {
        request_id: u64,
        data_id: String,
        wasm_checksum: String,
        resource_limits: ResourceLimits,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum CodeSource {
    GitHub {
        repo: String,
        commit: String,
        build_target: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceLimits {
    pub max_instructions: u64,
    pub max_memory_mb: u32,
    pub max_execution_seconds: u64,
}

/// Execution result to send back to coordinator
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionResult {
    pub success: bool,
    pub output: Option<Vec<u8>>,
    pub error: Option<String>,
    pub execution_time_ms: u64,
}

/// API client for communicating with Coordinator API
#[derive(Clone)]
pub struct ApiClient {
    client: Client,
    base_url: String,
    auth_token: String,
}

impl ApiClient {
    /// Create a new API client
    pub fn new(base_url: String, auth_token: String) -> Result<Self> {
        let client = Client::builder()
            .timeout(Duration::from_secs(120)) // 2 minutes default timeout
            .build()
            .context("Failed to build HTTP client")?;

        Ok(Self {
            client,
            base_url: base_url.trim_end_matches('/').to_string(),
            auth_token,
        })
    }

    /// Poll for a new task (long-polling)
    ///
    /// # Arguments
    /// * `timeout` - Timeout in seconds for long-polling (max 60)
    ///
    /// # Returns
    /// * `Ok(Some(task))` - New task received
    /// * `Ok(None)` - No task available (timeout reached)
    /// * `Err(_)` - Request failed
    pub async fn poll_task(&self, timeout: u64) -> Result<Option<Task>> {
        let url = format!("{}/tasks/poll?timeout={}", self.base_url, timeout);

        tracing::debug!("🔍 Polling for task: {}", url);

        let response = self
            .client
            .get(&url)
            .bearer_auth(&self.auth_token)
            .timeout(Duration::from_secs(timeout + 10)) // Add buffer to timeout
            .send()
            .await
            .context("Failed to send poll request")?;

        tracing::debug!("📡 Poll response status: {}", response.status());

        match response.status() {
            StatusCode::OK => {
                let body_text = response.text().await?;
                tracing::debug!("📦 Poll response body: {}", body_text);
                let task: Task = serde_json::from_str(&body_text)
                    .context(format!("Failed to parse task JSON: {}", body_text))?;
                Ok(Some(task))
            }
            StatusCode::NO_CONTENT => Ok(None), // No task available
            status => {
                let error_text = response
                    .text()
                    .await
                    .unwrap_or_else(|_| "Unknown error".to_string());
                tracing::error!("❌ Poll failed with status {}: {}", status, error_text);
                anyhow::bail!("Poll failed with status {}: {}", status, error_text)
            }
        }
    }

    /// Complete a task with result
    ///
    /// # Arguments
    /// * `request_id` - ID of the execution request
    /// * `result` - Execution result (success/failure, output, timing)
    pub async fn complete_task(&self, request_id: u64, result: ExecutionResult) -> Result<()> {
        let url = format!("{}/tasks/complete", self.base_url);

        #[derive(Serialize)]
        struct CompleteRequest {
            request_id: u64,
            success: bool,
            output: Option<Vec<u8>>,
            error: Option<String>,
            execution_time_ms: u64,
        }

        let request = CompleteRequest {
            request_id,
            success: result.success,
            output: result.output,
            error: result.error,
            execution_time_ms: result.execution_time_ms,
        };

        let response = self
            .client
            .post(&url)
            .bearer_auth(&self.auth_token)
            .json(&request)
            .send()
            .await
            .context("Failed to send complete request")?;

        if !response.status().is_success() {
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            anyhow::bail!("Complete task failed: {}", error_text)
        }

        Ok(())
    }

    /// Mark a task as failed
    ///
    /// # Arguments
    /// * `request_id` - ID of the execution request
    /// * `error` - Error message describing the failure
    pub async fn fail_task(&self, request_id: u64, error: String) -> Result<()> {
        let url = format!("{}/tasks/fail", self.base_url);

        #[derive(Serialize)]
        struct FailRequest {
            request_id: u64,
            error: String,
        }

        let request = FailRequest { request_id, error };

        let response = self
            .client
            .post(&url)
            .bearer_auth(&self.auth_token)
            .json(&request)
            .send()
            .await
            .context("Failed to send fail request")?;

        if !response.status().is_success() {
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            anyhow::bail!("Fail task failed: {}", error_text)
        }

        Ok(())
    }

    /// Download WASM binary from cache
    ///
    /// # Arguments
    /// * `checksum` - SHA256 checksum of the WASM file
    ///
    /// # Returns
    /// * `Ok(bytes)` - WASM binary data
    /// * `Err(_)` - Download failed or file not found
    pub async fn download_wasm(&self, checksum: &str) -> Result<Vec<u8>> {
        let url = format!("{}/wasm/{}", self.base_url, checksum);

        let response = self
            .client
            .get(&url)
            .bearer_auth(&self.auth_token)
            .send()
            .await
            .context("Failed to download WASM")?;

        if response.status() == StatusCode::NOT_FOUND {
            anyhow::bail!("WASM file not found: {}", checksum)
        }

        if !response.status().is_success() {
            anyhow::bail!("Download failed with status: {}", response.status())
        }

        let bytes = response
            .bytes()
            .await
            .context("Failed to read WASM bytes")?
            .to_vec();

        Ok(bytes)
    }

    /// Upload compiled WASM binary to cache
    ///
    /// # Arguments
    /// * `checksum` - SHA256 checksum of the WASM file
    /// * `repo` - GitHub repository URL
    /// * `commit` - Git commit hash
    /// * `bytes` - WASM binary data
    pub async fn upload_wasm(
        &self,
        checksum: String,
        repo: String,
        commit: String,
        bytes: Vec<u8>,
    ) -> Result<()> {
        let url = format!("{}/wasm/upload", self.base_url);

        // Create multipart form with correct field names (matching coordinator's handler)
        let file_part = reqwest::multipart::Part::bytes(bytes.clone())
            .file_name(format!("{}.wasm", checksum))
            .mime_str("application/wasm")
            .context("Failed to create file part")?;

        let form = reqwest::multipart::Form::new()
            .text("checksum", checksum.clone())
            .text("repo_url", repo.clone())      // coordinator expects "repo_url"
            .text("commit_hash", commit.clone()) // coordinator expects "commit_hash"
            .part("wasm_file", file_part);       // coordinator expects "wasm_file"

        tracing::info!(
            "Uploading WASM: checksum={} size={} bytes repo={} commit={}",
            checksum, bytes.len(), repo, commit
        );

        let response = self
            .client
            .post(&url)
            .bearer_auth(&self.auth_token)
            .multipart(form)
            .send()
            .await
            .context("Failed to upload WASM")?;

        if !response.status().is_success() {
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            anyhow::bail!("Upload failed: {}", error_text)
        }

        Ok(())
    }

    /// Check if WASM file exists in cache
    ///
    /// # Arguments
    /// * `checksum` - SHA256 checksum of the WASM file
    ///
    /// # Returns
    /// * `Ok(true)` - File exists in cache
    /// * `Ok(false)` - File does not exist
    pub async fn wasm_exists(&self, checksum: &str) -> Result<bool> {
        let url = format!("{}/wasm/exists/{}", self.base_url, checksum);

        let response = self
            .client
            .get(&url)
            .bearer_auth(&self.auth_token)
            .send()
            .await
            .context("Failed to check WASM existence")?;

        if !response.status().is_success() {
            anyhow::bail!("Check failed with status: {}", response.status())
        }

        #[derive(Deserialize)]
        struct ExistsResponse {
            exists: bool,
        }

        let result = response
            .json::<ExistsResponse>()
            .await
            .context("Failed to parse exists response")?;

        Ok(result.exists)
    }

    /// Acquire a distributed lock
    ///
    /// # Arguments
    /// * `lock_key` - Unique key for the lock (e.g., "compile:{repo}:{commit}")
    /// * `worker_id` - ID of this worker
    /// * `ttl` - Time-to-live in seconds
    ///
    /// # Returns
    /// * `Ok(true)` - Lock acquired
    /// * `Ok(false)` - Lock already held by another worker
    pub async fn acquire_lock(&self, lock_key: String, worker_id: String, ttl: u64) -> Result<bool> {
        let url = format!("{}/locks/acquire", self.base_url);

        #[derive(Serialize)]
        struct AcquireRequest {
            lock_key: String,
            worker_id: String,
            ttl_seconds: u64,
        }

        let request = AcquireRequest {
            lock_key,
            worker_id,
            ttl_seconds: ttl,
        };

        let response = self
            .client
            .post(&url)
            .bearer_auth(&self.auth_token)
            .json(&request)
            .send()
            .await
            .context("Failed to acquire lock")?;

        if !response.status().is_success() {
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            anyhow::bail!("Lock acquire failed: {}", error_text)
        }

        #[derive(Deserialize)]
        struct AcquireResponse {
            acquired: bool,
        }

        let result = response
            .json::<AcquireResponse>()
            .await
            .context("Failed to parse lock response")?;

        Ok(result.acquired)
    }

    /// Release a distributed lock
    ///
    /// # Arguments
    /// * `lock_key` - Key of the lock to release
    pub async fn release_lock(&self, lock_key: &str) -> Result<()> {
        let url = format!("{}/locks/release/{}", self.base_url, lock_key);

        let response = self
            .client
            .delete(&url)
            .bearer_auth(&self.auth_token)
            .send()
            .await
            .context("Failed to release lock")?;

        if !response.status().is_success() {
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            anyhow::bail!("Lock release failed: {}", error_text)
        }

        Ok(())
    }

    /// Create a new task (used by event monitor)
    ///
    /// # Arguments
    /// * `request_id` - Execution request ID from contract
    /// * `data_id` - Input data identifier (hex string)
    /// * `repo` - GitHub repository URL
    /// * `commit` - Git commit hash
    /// * `max_instructions` - Maximum WASM instructions
    /// * `max_memory_mb` - Maximum memory in MB
    /// * `max_execution_seconds` - Maximum execution time
    pub async fn create_task(
        &self,
        request_id: u64,
        data_id: String,
        repo: String,
        commit: String,
        max_instructions: u64,
        max_memory_mb: u32,
        max_execution_seconds: u64,
        input_data: String,
    ) -> Result<()> {
        let url = format!("{}/tasks/create", self.base_url);

        #[derive(Serialize)]
        struct CreateRequest {
            request_id: u64,
            data_id: String,
            code_source: CodeSource,
            resource_limits: ResourceLimits,
            input_data: String,
        }

        let request = CreateRequest {
            request_id,
            data_id,
            code_source: CodeSource::GitHub {
                repo,
                commit,
                build_target: "wasm32-unknown-unknown".to_string(),
            },
            resource_limits: ResourceLimits {
                max_instructions,
                max_memory_mb,
                max_execution_seconds,
            },
            input_data,
        };

        let response = self
            .client
            .post(&url)
            .bearer_auth(&self.auth_token)
            .json(&request)
            .send()
            .await
            .context("Failed to create task")?;

        if !response.status().is_success() {
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            anyhow::bail!("Create task failed: {}", error_text)
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_api_client_creation() {
        let client = ApiClient::new(
            "http://localhost:8080".to_string(),
            "test-token".to_string(),
        );
        assert!(client.is_ok());
    }

    #[test]
    fn test_base_url_trimming() {
        let client = ApiClient::new(
            "http://localhost:8080/".to_string(),
            "test-token".to_string(),
        )
        .unwrap();
        assert_eq!(client.base_url, "http://localhost:8080");
    }
}
