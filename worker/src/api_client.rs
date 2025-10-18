use anyhow::{Context, Result};
use reqwest::{Client, StatusCode};
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Response format for execution output
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub enum ResponseFormat {
    Bytes,
    #[default]
    Text,
    Json,
}

/// Execution context metadata passed to WASM via environment variables
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ExecutionContext {
    #[serde(default)]
    pub sender_id: Option<String>,
    #[serde(default)]
    pub block_height: Option<u64>,
    #[serde(default)]
    pub block_timestamp: Option<u64>,
    #[serde(default)]
    pub contract_id: Option<String>,
}

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
        #[serde(default)]
        encrypted_secrets: Option<Vec<u8>>,
        #[serde(default)]
        response_format: ResponseFormat,
        #[serde(default)]
        context: ExecutionContext,
        #[serde(default)]
        user_account_id: Option<String>,
        #[serde(default)]
        near_payment_yocto: Option<String>,
        #[serde(default)]
        transaction_hash: Option<String>,
    },
    Execute {
        request_id: u64,
        data_id: String,
        wasm_checksum: String,
        resource_limits: ResourceLimits,
        input_data: String,
        #[serde(default)]
        encrypted_secrets: Option<Vec<u8>>,
        #[serde(default)]
        build_target: Option<String>,
        #[serde(default)]
        response_format: ResponseFormat,
        #[serde(default)]
        context: ExecutionContext,
        #[serde(default)]
        user_account_id: Option<String>,
        #[serde(default)]
        near_payment_yocto: Option<String>,
        #[serde(default)]
        transaction_hash: Option<String>,
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

impl CodeSource {
    pub fn repo(&self) -> &str {
        match self {
            CodeSource::GitHub { repo, .. } => repo,
        }
    }

    pub fn commit(&self) -> &str {
        match self {
            CodeSource::GitHub { commit, .. } => commit,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceLimits {
    pub max_instructions: u64,
    pub max_memory_mb: u32,
    pub max_execution_seconds: u64,
}

/// Execution output - can be bytes, text, or parsed JSON
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ExecutionOutput {
    Bytes(Vec<u8>),
    Text(String),
    Json(serde_json::Value),
}

/// Execution result to send back to coordinator
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionResult {
    pub success: bool,
    pub output: Option<ExecutionOutput>,
    pub error: Option<String>,
    pub execution_time_ms: u64,
    pub instructions: u64,
    pub compile_time_ms: Option<u64>, // Compilation time if WASM was compiled in this execution
    pub compilation_note: Option<String>, // e.g., "Cached WASM from 2025-01-10 14:30 UTC"
}

/// Job type - compile or execute
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum JobType {
    Compile,
    Execute,
}

/// Job information returned by claim_job
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobInfo {
    pub job_id: i64,
    pub job_type: JobType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wasm_checksum: Option<String>,
    pub allowed: bool,
}

/// Pricing configuration from coordinator (fetched from NEAR contract)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PricingConfig {
    pub base_fee: String,                // yoctoNEAR
    pub per_instruction_fee: String,     // yoctoNEAR per million instructions
    pub per_ms_fee: String,              // yoctoNEAR per millisecond (execution)
    pub per_compile_ms_fee: String,      // yoctoNEAR per millisecond (compilation)
    pub max_compilation_seconds: u64,    // Maximum compilation time
}

/// Response from claim_job endpoint
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClaimJobResponse {
    pub jobs: Vec<JobInfo>,
    pub pricing: PricingConfig,
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
    /// * `data_id` - Data ID from blockchain
    /// * `result` - Execution result (success/failure, output, timing)
    /// * `resolve_tx_id` - Transaction ID of resolve_execution call
    /// * `user_account_id` - User who requested execution
    /// * `near_payment_yocto` - Payment amount in yoctoNEAR
    /// * `worker_id` - This worker's ID
    pub async fn complete_task(
        &self,
        request_id: u64,
        data_id: Option<String>,
        result: ExecutionResult,
        resolve_tx_id: Option<String>,
        user_account_id: Option<String>,
        near_payment_yocto: Option<String>,
        worker_id: String,
        github_repo: Option<String>,
        github_commit: Option<String>,
    ) -> Result<()> {
        let url = format!("{}/tasks/complete", self.base_url);

        #[derive(Serialize)]
        struct CompleteRequest {
            request_id: u64,
            success: bool,
            output: Option<ExecutionOutput>,
            error: Option<String>,
            execution_time_ms: u64,
            instructions: u64,
            data_id: Option<String>,
            resolve_tx_id: Option<String>,
            user_account_id: Option<String>,
            near_payment_yocto: Option<String>,
            worker_id: Option<String>,
            github_repo: Option<String>,
            github_commit: Option<String>,
        }

        let request = CompleteRequest {
            request_id,
            success: result.success,
            output: result.output.clone(),
            error: result.error,
            execution_time_ms: result.execution_time_ms,
            instructions: result.instructions,
            data_id,
            resolve_tx_id,
            user_account_id,
            near_payment_yocto,
            worker_id: Some(worker_id),
            github_repo,
            github_commit,
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

    /// Claim job(s) for a task
    ///
    /// # Arguments
    /// * `request_id` - Request ID from contract
    /// * `data_id` - Data ID from contract event
    /// * `worker_id` - This worker's ID
    /// * `code_source` - Code source details
    /// * `resource_limits` - Resource limits for execution
    /// * `user_account_id` - Optional user account ID from contract
    /// * `near_payment_yocto` - Optional payment amount from contract
    /// * `transaction_hash` - Optional transaction hash from contract
    ///
    /// # Returns
    /// * `Ok(jobs)` - Array of jobs to process (compile and/or execute)
    /// * `Err(_)` - Request failed or task already claimed
    pub async fn claim_job(
        &self,
        request_id: u64,
        data_id: String,
        worker_id: String,
        code_source: &CodeSource,
        resource_limits: &ResourceLimits,
        user_account_id: Option<String>,
        near_payment_yocto: Option<String>,
        transaction_hash: Option<String>,
    ) -> Result<ClaimJobResponse> {
        let url = format!("{}/jobs/claim", self.base_url);

        #[derive(Serialize)]
        struct ClaimRequest {
            request_id: u64,
            data_id: String,
            worker_id: String,
            code_source: CodeSource,
            resource_limits: ResourceLimits,
            #[serde(skip_serializing_if = "Option::is_none")]
            user_account_id: Option<String>,
            #[serde(skip_serializing_if = "Option::is_none")]
            near_payment_yocto: Option<String>,
            #[serde(skip_serializing_if = "Option::is_none")]
            transaction_hash: Option<String>,
        }

        let request = ClaimRequest {
            request_id,
            data_id,
            worker_id,
            code_source: code_source.clone(),
            resource_limits: resource_limits.clone(),
            user_account_id,
            near_payment_yocto,
            transaction_hash,
        };

        tracing::debug!("🎯 Claiming job for request_id={}", request_id);

        let response = self
            .client
            .post(&url)
            .bearer_auth(&self.auth_token)
            .json(&request)
            .send()
            .await
            .context("Failed to send claim job request")?;

        match response.status() {
            StatusCode::OK => {
                let claim_response: ClaimJobResponse = response
                    .json()
                    .await
                    .context("Failed to parse claim job response")?;
                tracing::info!(
                    "✅ Claimed {} job(s) for request_id={}",
                    claim_response.jobs.len(),
                    request_id
                );
                Ok(claim_response)
            }
            StatusCode::CONFLICT => {
                anyhow::bail!("Task already claimed by another worker")
            }
            status => {
                let error_text = response
                    .text()
                    .await
                    .unwrap_or_else(|_| "Unknown error".to_string());
                tracing::error!("❌ Claim job failed with status {}: {}", status, error_text);
                anyhow::bail!("Claim job failed with status {}: {}", status, error_text)
            }
        }
    }

    /// Complete a job with result
    ///
    /// # Arguments
    /// * `job_id` - Job ID from coordinator
    /// * `success` - Whether job succeeded
    /// * `output` - Execution output (for execute jobs)
    /// * `error` - Error message (if failed)
    /// * `time_ms` - Time taken in milliseconds
    /// * `instructions` - Instructions consumed (for execute jobs, 0 for compile)
    /// * `wasm_checksum` - WASM checksum (for compile jobs)
    /// * `actual_cost_yocto` - Total cost from contract (for execute jobs)
    /// * `compile_cost_yocto` - Compilation cost calculated by worker (for compile jobs)
    pub async fn complete_job(
        &self,
        job_id: i64,
        success: bool,
        output: Option<ExecutionOutput>,
        error: Option<String>,
        time_ms: u64,
        instructions: u64,
        wasm_checksum: Option<String>,
        actual_cost_yocto: Option<String>,
        compile_cost_yocto: Option<String>,
    ) -> Result<()> {
        let url = format!("{}/jobs/complete", self.base_url);

        #[derive(Serialize)]
        struct CompleteJobRequest {
            job_id: i64,
            success: bool,
            output: Option<ExecutionOutput>,
            error: Option<String>,
            time_ms: u64,
            instructions: u64,
            wasm_checksum: Option<String>,
            actual_cost_yocto: Option<String>,
            compile_cost_yocto: Option<String>,
        }

        let request = CompleteJobRequest {
            job_id,
            success,
            output,
            error: error.clone(),
            time_ms,
            instructions,
            wasm_checksum,
            actual_cost_yocto,
            compile_cost_yocto,
        };

        tracing::debug!(
            "📤 Completing job_id={} success={} time_ms={}",
            job_id,
            success,
            time_ms
        );

        let response = self
            .client
            .post(&url)
            .bearer_auth(&self.auth_token)
            .json(&request)
            .send()
            .await
            .context("Failed to send complete job request")?;

        if !response.status().is_success() {
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            tracing::error!("❌ Complete job failed: {}", error_text);
            anyhow::bail!("Complete job failed: {}", error_text)
        }

        tracing::info!("✅ Job {} completed successfully", job_id);
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
    /// * `Ok((exists, created_at))` - Whether file exists and optional creation timestamp
    pub async fn wasm_exists(&self, checksum: &str) -> Result<(bool, Option<String>)> {
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
            created_at: Option<String>,
        }

        let result = response
            .json::<ExistsResponse>()
            .await
            .context("Failed to parse exists response")?;

        Ok((result.exists, result.created_at))
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
    /// * `input_data` - Input data JSON string
    /// * `encrypted_secrets` - Optional encrypted secrets
    /// * `user_account_id` - User who requested execution
    /// * `near_payment_yocto` - Payment amount in yoctoNEAR
    ///
    /// Returns `Ok(Some(request_id))` if task was created, `Ok(None)` if duplicate
    pub async fn create_task(
        &self,
        request_id: u64,
        data_id: String,
        repo: String,
        commit: String,
        build_target: String,
        max_instructions: u64,
        max_memory_mb: u32,
        max_execution_seconds: u64,
        input_data: String,
        encrypted_secrets: Option<Vec<u8>>,
        response_format: ResponseFormat,
        context: ExecutionContext,
        user_account_id: Option<String>,
        near_payment_yocto: Option<String>,
        transaction_hash: Option<String>,
    ) -> Result<Option<u64>> {
        let url = format!("{}/tasks/create", self.base_url);

        #[derive(Serialize)]
        struct CreateRequest {
            request_id: u64,
            data_id: String,
            code_source: CodeSource,
            resource_limits: ResourceLimits,
            input_data: String,
            #[serde(skip_serializing_if = "Option::is_none")]
            encrypted_secrets: Option<Vec<u8>>,
            response_format: ResponseFormat,
            context: ExecutionContext,
            #[serde(skip_serializing_if = "Option::is_none")]
            user_account_id: Option<String>,
            #[serde(skip_serializing_if = "Option::is_none")]
            near_payment_yocto: Option<String>,
            #[serde(skip_serializing_if = "Option::is_none")]
            transaction_hash: Option<String>,
        }

        #[derive(Deserialize)]
        struct CreateResponse {
            request_id: i64,
            created: bool,
        }

        let request = CreateRequest {
            request_id,
            data_id,
            code_source: CodeSource::GitHub {
                repo,
                commit,
                build_target,
            },
            resource_limits: ResourceLimits {
                max_instructions,
                max_memory_mb,
                max_execution_seconds,
            },
            input_data,
            encrypted_secrets,
            response_format,
            context,
            user_account_id,
            near_payment_yocto,
            transaction_hash,
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

        let create_response: CreateResponse = response.json().await.context("Failed to parse create task response")?;

        if create_response.created {
            Ok(Some(create_response.request_id as u64))
        } else {
            Ok(None)
        }
    }

    /// Send heartbeat to coordinator
    ///
    /// # Arguments
    /// * `worker_id` - Unique worker identifier
    /// * `worker_name` - Human-readable worker name
    /// * `status` - Current status (online, busy, offline)
    /// * `current_task_id` - ID of currently executing task (if any)
    pub async fn send_heartbeat(
        &self,
        worker_id: String,
        worker_name: String,
        status: &str,
        current_task_id: Option<i64>,
    ) -> Result<()> {
        let url = format!("{}/workers/heartbeat", self.base_url);

        #[derive(Serialize)]
        struct HeartbeatRequest {
            worker_id: String,
            worker_name: String,
            status: String,
            current_task_id: Option<i64>,
        }

        let request = HeartbeatRequest {
            worker_id,
            worker_name,
            status: status.to_string(),
            current_task_id,
        };

        let response = self
            .client
            .post(&url)
            .bearer_auth(&self.auth_token)
            .json(&request)
            .send()
            .await
            .context("Failed to send heartbeat")?;

        if !response.status().is_success() {
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            anyhow::bail!("Heartbeat failed: {}", error_text)
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
