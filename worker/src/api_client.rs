use anyhow::{Context, Result};
use reqwest::{Client, StatusCode};
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Job status for error classification
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum JobStatus {
    Pending,
    InProgress,
    Completed,
    Failed,
    CompilationFailed,
    ExecutionFailed,
    AccessDenied,
    InsufficientPayment,
    Custom,
}

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
    #[serde(default)]
    pub transaction_hash: Option<String>,
    #[serde(default)]
    pub receipt_id: Option<String>,
    #[serde(default)]
    pub predecessor_id: Option<String>,
    #[serde(default)]
    pub signer_public_key: Option<String>,
    #[serde(default)]
    pub gas_burnt: Option<u64>,
}

/// Request from user to execute WASM code off-chain
///
/// Flow:
/// 1. Event Monitor detects on-chain execution request
/// 2. Sends ExecutionRequest to Coordinator API
/// 3. Coordinator places in Redis queue for workers
/// 4. Worker polls and receives ExecutionRequest
/// 5. Worker claims jobs via coordinator (which decides: compile+execute or just execute)
/// 6. Worker processes jobs and returns result to contract
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionRequest {
    pub request_id: u64,
    pub data_id: String,
    /// Code source - optional for HTTPS calls where project_id is used instead
    /// Worker resolves project_id -> code_source from contract
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub code_source: Option<CodeSource>,
    pub resource_limits: ResourceLimits,
    pub input_data: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub secrets_ref: Option<SecretsReference>,
    #[serde(default)]
    pub response_format: ResponseFormat,
    #[serde(default)]
    pub context: ExecutionContext,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_account_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub near_payment_yocto: Option<String>,
    /// Payment to project developer (stablecoin, minimal token units)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attached_usd: Option<String>,
    /// If true, only compile the code without executing
    #[serde(default)]
    pub compile_only: bool,
    /// Force recompilation even if WASM exists in cache
    #[serde(default)]
    pub force_rebuild: bool,
    /// Store compiled WASM to FastFS after compilation
    #[serde(default)]
    pub store_on_fastfs: bool,
    /// Result from compile job to pass to executor (e.g., FastFS URL or compilation error)
    /// When set, executor should call resolve_execution with this value without running WASM
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub compile_result: Option<String>,
    /// Project UUID for persistent storage (None for standalone WASM)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_uuid: Option<String>,
    /// Project ID for project-based secrets (e.g., "alice.near/my-app")
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_id: Option<String>,
    /// Version key for specific project version (if None, uses active_version)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version_key: Option<String>,
    /// HTTPS API call flag - if true, don't call contract, call coordinator instead
    #[serde(default)]
    pub is_https_call: bool,
    /// HTTPS API call ID - used to complete the call on coordinator
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub call_id: Option<String>,
    /// Payment Key owner for HTTPS calls (NEAR account ID)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub payment_key_owner: Option<String>,
    /// Payment Key nonce for HTTPS calls
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub payment_key_nonce: Option<i32>,
    /// USD payment amount for HTTPS calls (X-Attached-Deposit, in minimal token units)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub usd_payment: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum CodeSource {
    GitHub {
        repo: String,
        commit: String,
        build_target: String,
    },
    /// Pre-compiled WASM file accessible via URL
    /// Worker downloads from URL, verifies SHA256 hash, then executes without compilation
    WasmUrl {
        url: String,           // URL for downloading (https://, ipfs://, ar://)
        hash: String,          // SHA256 hash for verification (hex encoded)
        build_target: String,
    },
}

/// Reference to secrets stored in contract (new repo-based system)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecretsReference {
    pub profile: String,
    pub account_id: String,
}

impl SecretsReference {
    /// Format as "{account_id}/{profile}" for attestation hash.
    /// Returns None if either field is empty.
    pub fn as_attestation_ref(&self) -> Option<String> {
        if self.account_id.is_empty() || self.profile.is_empty() {
            None
        } else {
            Some(format!("{}/{}", self.account_id, self.profile))
        }
    }
}

impl CodeSource {
    pub fn repo(&self) -> Option<&str> {
        match self {
            CodeSource::GitHub { repo, .. } => Some(repo),
            CodeSource::WasmUrl { .. } => None,
        }
    }

    pub fn commit(&self) -> Option<&str> {
        match self {
            CodeSource::GitHub { commit, .. } => Some(commit),
            CodeSource::WasmUrl { .. } => None,
        }
    }

    pub fn build_target(&self) -> Option<&str> {
        match self {
            CodeSource::GitHub { build_target, .. } => Some(build_target),
            CodeSource::WasmUrl { build_target, .. } => Some(build_target),
        }
    }

    /// Get the hash for WasmUrl sources (used for verification)
    #[allow(dead_code)]
    pub fn hash(&self) -> Option<&str> {
        match self {
            CodeSource::GitHub { .. } => None,
            CodeSource::WasmUrl { hash, .. } => Some(hash),
        }
    }

    /// Get the URL for WasmUrl sources
    #[allow(dead_code)]
    pub fn url(&self) -> Option<&str> {
        match self {
            CodeSource::GitHub { .. } => None,
            CodeSource::WasmUrl { url, .. } => Some(url),
        }
    }

    /// Check if this is a WasmUrl source (pre-compiled, no compilation needed)
    #[allow(dead_code)]
    pub fn is_wasm_url(&self) -> bool {
        matches!(self, CodeSource::WasmUrl { .. })
    }

    /// Normalize repo URL to full https:// format for git clone
    /// Examples:
    /// - "github.com/user/repo" -> "https://github.com/user/repo"
    /// - "https://github.com/user/repo" -> "https://github.com/user/repo" (unchanged)
    /// - "user/repo" -> "https://github.com/user/repo"
    /// - "git@github.com:user/repo.git" -> "https://github.com/user/repo"
    /// - "ssh://git@github.com/user/repo" -> "https://github.com/user/repo"
    pub fn normalize(mut self) -> Self {
        match &mut self {
            CodeSource::GitHub { repo, .. } => {
                // Skip if already has https/http protocol
                if repo.starts_with("https://") || repo.starts_with("http://") {
                    return self;
                }

                // Handle SSH format: git@github.com:user/repo.git
                if repo.starts_with("git@github.com:") {
                    let path = repo.strip_prefix("git@github.com:").unwrap();
                    let path = path.strip_suffix(".git").unwrap_or(path);
                    *repo = format!("https://github.com/{}", path);
                    return self;
                }

                // Handle SSH URL format: ssh://git@github.com/user/repo
                if repo.starts_with("ssh://git@github.com/") {
                    let path = repo.strip_prefix("ssh://git@github.com/").unwrap();
                    let path = path.strip_suffix(".git").unwrap_or(path);
                    *repo = format!("https://github.com/{}", path);
                    return self;
                }

                // Handle ssh:// without git@ prefix
                if repo.starts_with("ssh://") {
                    // Leave as is, will fail later with better error
                    return self;
                }

                // Add https:// prefix
                if repo.starts_with("github.com/") {
                    *repo = format!("https://{}", repo);
                } else if !repo.contains('/') {
                    // Invalid format - leave as is, will fail later with better error
                    return self;
                } else {
                    // Assume it's "user/repo" format
                    *repo = format!("https://github.com/{}", repo);
                }

                self
            }
            CodeSource::WasmUrl { .. } => {
                // WasmUrl already has full URL, no normalization needed
                self
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceLimits {
    pub max_instructions: u64,
    pub max_memory_mb: u32,
    pub max_execution_seconds: u64,
}

/// Parameters for creating a new task in coordinator
#[derive(Debug, Clone)]
pub struct CreateTaskParams {
    pub request_id: u64,
    pub data_id: String,
    pub code_source: CodeSource,
    pub resource_limits: ResourceLimits,
    pub input_data: String,
    pub secrets_ref: Option<SecretsReference>,
    pub response_format: ResponseFormat,
    pub context: ExecutionContext,
    pub user_account_id: Option<String>,
    pub near_payment_yocto: Option<String>,
    /// Payment to project developer (stablecoin, minimal token units)
    pub attached_usd: Option<String>,
    pub compile_only: bool,
    pub force_rebuild: bool,
    pub store_on_fastfs: bool,
    /// Project UUID for persistent storage (from request_execution_project)
    pub project_uuid: Option<String>,
    /// Project ID for project-based secrets (e.g., "alice.near/my-app")
    pub project_id: Option<String>,
}

/// Execution output - can be bytes, text, or parsed JSON
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ExecutionOutput {
    Bytes(Vec<u8>),
    Text(String),
    Json(serde_json::Value),
}

/// Project UUID info from coordinator
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectUuidInfo {
    pub project_id: String,
    pub uuid: String,
    pub active_version: String,
    pub cached: bool,
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
    /// Refund amount to return to user from attached_usd (stablecoin, minimal token units)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refund_usd: Option<u64>,
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
    /// Compilation cost from compile job (for execute jobs to include in total cost)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compile_cost_yocto: Option<String>,
    /// Compilation error message (for execute jobs to report failure to contract)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compile_error: Option<String>,
    /// Compilation time in milliseconds from compile job
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compile_time_ms: Option<u64>,
    /// Project UUID for persistent storage (None for standalone WASM)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_uuid: Option<String>,
    /// Project ID for project-based secrets (e.g., "alice.near/my-app")
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_id: Option<String>,
    /// Job creation timestamp (unix seconds) - used for attestation V1 format
    #[serde(default)]
    pub created_at: i64,
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

    /// Poll for a new execution request (long-polling)
    ///
    /// # Arguments
    /// * `timeout` - Timeout in seconds for long-polling (max 60)
    /// * `capabilities` - Worker capabilities (e.g., ["compilation", "execution"])
    ///
    /// # Returns
    /// * `Ok(Some(request))` - New execution request received
    /// * `Ok(None)` - No request available (timeout reached)
    /// * `Err(_)` - Request failed
    pub async fn poll_task(&self, timeout: u64, capabilities: &[String]) -> Result<Option<ExecutionRequest>> {
        // Build URL with query parameters
        let capabilities_param = capabilities.join(",");
        let url = format!(
            "{}/executions/poll?timeout={}&capabilities={}",
            self.base_url, timeout, capabilities_param
        );

        tracing::debug!("🔍 Polling for execution request: {}", url);

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
                let request: ExecutionRequest = serde_json::from_str(&body_text)
                    .context(format!("Failed to parse execution request JSON: {}", body_text))?;
                Ok(Some(request))
            }
            StatusCode::NO_CONTENT => Ok(None), // No request available
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
    #[allow(dead_code)]
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
    #[allow(dead_code)]
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
        capabilities: Vec<String>,
        compile_only: bool,
        force_rebuild: bool,
        has_compile_result: bool,
        project_uuid: Option<String>,
        project_id: Option<String>,
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
            capabilities: Vec<String>,
            compile_only: bool,
            force_rebuild: bool,
            has_compile_result: bool,
            #[serde(skip_serializing_if = "Option::is_none")]
            project_uuid: Option<String>,
            #[serde(skip_serializing_if = "Option::is_none")]
            project_id: Option<String>,
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
            capabilities,
            compile_only,
            force_rebuild,
            has_compile_result,
            project_uuid,
            project_id,
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
    /// * `compile_result` - Result to pass to executor (e.g., FastFS URL)
    #[allow(clippy::too_many_arguments)]
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
        error_category: Option<JobStatus>,
        compile_result: Option<String>,
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
            #[serde(skip_serializing_if = "Option::is_none")]
            error_category: Option<JobStatus>,
            #[serde(skip_serializing_if = "Option::is_none")]
            compile_result: Option<String>,
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
            error_category,
            compile_result,
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

    /// Complete an HTTPS API call (without NEAR contract interaction)
    ///
    /// Used for HTTPS /call endpoint where results go directly to coordinator
    /// instead of being submitted to the NEAR contract.
    ///
    /// # Arguments
    /// * `call_id` - UUID of the HTTPS call
    /// * `success` - Whether execution succeeded
    /// * `output` - Execution output (JSON)
    /// * `error` - Error message (if failed)
    /// * `instructions` - Instructions consumed
    /// * `time_ms` - Execution time in milliseconds
    /// * `job_id` - Job ID for attestation linking
    pub async fn complete_https_call(
        &self,
        call_id: &str,
        success: bool,
        output: Option<serde_json::Value>,
        error: Option<String>,
        instructions: u64,
        time_ms: u64,
        job_id: Option<i64>,
    ) -> Result<()> {
        let url = format!("{}/https-calls/complete", self.base_url);

        #[derive(Serialize)]
        struct CompleteHttpsCallRequest {
            call_id: String,
            success: bool,
            #[serde(skip_serializing_if = "Option::is_none")]
            output: Option<serde_json::Value>,
            #[serde(skip_serializing_if = "Option::is_none")]
            error: Option<String>,
            instructions: u64,
            time_ms: u64,
            #[serde(skip_serializing_if = "Option::is_none")]
            job_id: Option<i64>,
        }

        let request = CompleteHttpsCallRequest {
            call_id: call_id.to_string(),
            success,
            output,
            error: error.clone(),
            instructions,
            time_ms,
            job_id,
        };

        tracing::info!(
            "📤 Completing HTTPS call: call_id={} success={} instructions={} time_ms={}",
            call_id, success, instructions, time_ms
        );

        let response = self
            .client
            .post(&url)
            .bearer_auth(&self.auth_token)
            .json(&request)
            .send()
            .await
            .context("Failed to send complete HTTPS call request")?;

        if !response.status().is_success() {
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            tracing::error!("❌ Complete HTTPS call failed: {}", error_text);
            anyhow::bail!("Complete HTTPS call failed: {}", error_text)
        }

        tracing::info!("✅ HTTPS call {} completed successfully", call_id);
        Ok(())
    }

    /// Store system logs (compilation/execution) for admin debugging
    /// This endpoint does NOT require authentication (internal endpoint)
    ///
    /// # Arguments
    /// * `request_id` - Execution request ID
    /// * `job_id` - Job ID (optional)
    /// * `log_type` - "compilation" or "execution"
    /// * `stderr` - Raw stderr output
    /// * `stdout` - Raw stdout output
    /// * `exit_code` - Exit code from process
    /// * `execution_error` - WASM execution error (optional)
    pub async fn store_system_log(
        &self,
        request_id: u64,
        job_id: Option<i64>,
        log_type: &str,
        stderr: Option<String>,
        stdout: Option<String>,
        exit_code: Option<i32>,
        execution_error: Option<String>,
    ) -> Result<()> {
        let url = format!("{}/internal/system-logs", self.base_url);

        let payload = serde_json::json!({
            "request_id": request_id,
            "job_id": job_id,
            "log_type": log_type,
            "stderr": stderr,
            "stdout": stdout,
            "exit_code": exit_code,
            "execution_error": execution_error,
        });

        let response = self
            .client
            .post(&url)
            .json(&payload)
            .send()
            .await
            .context("Failed to store system log")?;

        if !response.status().is_success() {
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            tracing::warn!("⚠️ Failed to store system log: {}", error_text);
            // Don't fail the job if logging fails - just warn
            return Ok(());
        }

        tracing::debug!("📝 Stored system log ({}) for request_id={}", log_type, request_id);
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
    /// * `build_target` - Build target (e.g., wasm32-wasip1, wasm32-wasip2)
    /// * `bytes` - WASM binary data
    pub async fn upload_wasm(
        &self,
        checksum: String,
        repo: String,
        commit: String,
        build_target: String,
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
            .text("repo_url", repo.clone())         // coordinator expects "repo_url"
            .text("commit_hash", commit.clone())    // coordinator expects "commit_hash"
            .text("build_target", build_target.clone()) // coordinator expects "build_target"
            .part("wasm_file", file_part);          // coordinator expects "wasm_file"

        tracing::info!(
            "Uploading WASM: checksum={} size={} bytes repo={} commit={} target={}",
            checksum, bytes.len(), repo, commit, build_target
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
        // URL-encode the lock key to handle special characters like : and /
        let encoded_key = urlencoding::encode(lock_key);
        let url = format!("{}/locks/release/{}", self.base_url, encoded_key);

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

    /// Create a new execution request (used by event monitor)
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
    /// * `secrets_ref` - Optional reference to secrets stored in contract
    /// * `context` - Execution context (includes transaction_hash from neardata)
    /// * `user_account_id` - User who requested execution
    /// * `near_payment_yocto` - Payment amount in yoctoNEAR
    ///
    /// Returns `Ok(Some(request_id))` if request was created, `Ok(None)` if duplicate
    pub async fn create_task(&self, params: CreateTaskParams) -> Result<Option<u64>> {
        let url = format!("{}/executions/create", self.base_url);

        #[derive(Serialize)]
        struct CreateRequest {
            request_id: u64,
            data_id: String,
            code_source: CodeSource,
            resource_limits: ResourceLimits,
            input_data: String,
            #[serde(skip_serializing_if = "Option::is_none")]
            secrets_ref: Option<SecretsReference>,
            response_format: ResponseFormat,
            context: ExecutionContext,
            #[serde(skip_serializing_if = "Option::is_none")]
            user_account_id: Option<String>,
            #[serde(skip_serializing_if = "Option::is_none")]
            near_payment_yocto: Option<String>,
            #[serde(skip_serializing_if = "Option::is_none")]
            attached_usd: Option<String>,
            compile_only: bool,
            force_rebuild: bool,
            store_on_fastfs: bool,
            #[serde(skip_serializing_if = "Option::is_none")]
            project_uuid: Option<String>,
            #[serde(skip_serializing_if = "Option::is_none")]
            project_id: Option<String>,
        }

        #[derive(Deserialize)]
        struct CreateResponse {
            request_id: i64,
            created: bool,
        }

        let request = CreateRequest {
            request_id: params.request_id,
            data_id: params.data_id,
            code_source: params.code_source,
            resource_limits: params.resource_limits,
            input_data: params.input_data,
            secrets_ref: params.secrets_ref,
            response_format: params.response_format,
            context: params.context,
            user_account_id: params.user_account_id,
            near_payment_yocto: params.near_payment_yocto,
            attached_usd: params.attached_usd,
            compile_only: params.compile_only,
            force_rebuild: params.force_rebuild,
            store_on_fastfs: params.store_on_fastfs,
            project_uuid: params.project_uuid,
            project_id: params.project_id,
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

    /// Create a TopUp task for Payment Key balance update
    ///
    /// This is used when ft_on_transfer emits SystemEvent::TopUpPaymentKey
    /// Worker will decrypt/update balance/encrypt and call promise_yield_resume
    pub async fn create_topup_task(&self, params: TopUpTaskData) -> Result<Option<i64>> {
        let url = format!("{}/topup/create", self.base_url);

        #[derive(Serialize)]
        struct CreateTopUpRequest {
            data_id: String,
            owner: String,
            nonce: u32,
            amount: String,
            encrypted_data: String,
        }

        #[derive(Deserialize)]
        struct CreateTopUpResponse {
            task_id: i64,
            created: bool,
        }

        let request = CreateTopUpRequest {
            data_id: params.data_id.clone(),
            owner: params.owner,
            nonce: params.nonce,
            amount: params.amount,
            encrypted_data: params.encrypted_data,
        };

        let response = self
            .client
            .post(&url)
            .bearer_auth(&self.auth_token)
            .json(&request)
            .send()
            .await
            .context("Failed to create topup task")?;

        if !response.status().is_success() {
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            anyhow::bail!("Create topup task failed: {}", error_text)
        }

        let create_response: CreateTopUpResponse = response.json().await
            .context("Failed to parse create topup task response")?;

        if create_response.created {
            tracing::info!(
                task_id = create_response.task_id,
                data_id = %params.data_id,
                "TopUp task created in coordinator"
            );
            Ok(Some(create_response.task_id))
        } else {
            tracing::info!(
                data_id = %params.data_id,
                "TopUp task already exists (duplicate)"
            );
            Ok(None)
        }
    }

    /// Complete TopUp - notify coordinator of new balance
    ///
    /// Called after successfully calling resume_topup on the contract.
    /// This stores payment key metadata in coordinator's PostgreSQL for validation.
    ///
    /// # Arguments
    /// * `owner` - Payment Key owner (NEAR account)
    /// * `nonce` - Payment Key nonce
    /// * `new_initial_balance` - The new total balance after top-up
    /// * `key_hash` - SHA256 hash of the key (hex encoded) for validation
    /// * `project_ids` - List of allowed project IDs (empty = all projects)
    /// * `max_per_call` - Max amount per API call (optional)
    pub async fn complete_topup(
        &self,
        owner: &str,
        nonce: u32,
        new_initial_balance: &str,
        key_hash: &str,
        project_ids: &[String],
        max_per_call: Option<&str>,
    ) -> Result<()> {
        let url = format!("{}/topup/complete", self.base_url);

        #[derive(Serialize)]
        struct CompleteTopUpRequest {
            owner: String,
            nonce: u32,
            new_initial_balance: String,
            key_hash: String,
            project_ids: Vec<String>,
            max_per_call: Option<String>,
        }

        let request = CompleteTopUpRequest {
            owner: owner.to_string(),
            nonce,
            new_initial_balance: new_initial_balance.to_string(),
            key_hash: key_hash.to_string(),
            project_ids: project_ids.to_vec(),
            max_per_call: max_per_call.map(String::from),
        };

        tracing::info!(
            "📊 Notifying coordinator of TopUp completion: owner={} nonce={} balance={} key_hash={}...",
            owner, nonce, new_initial_balance, &key_hash[..8.min(key_hash.len())]
        );

        let response = self
            .client
            .post(&url)
            .bearer_auth(&self.auth_token)
            .json(&request)
            .send()
            .await
            .context("Failed to notify coordinator of topup completion")?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            tracing::warn!(
                "Failed to update balance cache (non-critical): {}",
                error_text
            );
            // Non-critical error - TopUp still succeeded on contract
            return Ok(());
        }

        tracing::info!("Balance cache updated successfully");
        Ok(())
    }

    /// Delete payment key from coordinator PostgreSQL (soft delete)
    ///
    /// Called when processing DeletePaymentKey event from contract.
    /// This marks the key as deleted in PostgreSQL so it's no longer valid for HTTPS API calls.
    ///
    /// # Arguments
    /// * `owner` - Payment Key owner (NEAR account)
    /// * `nonce` - Payment Key nonce
    pub async fn delete_payment_key(&self, owner: &str, nonce: u32) -> Result<()> {
        let url = format!("{}/payment-keys/delete", self.base_url);

        #[derive(Serialize)]
        struct DeletePaymentKeyRequest {
            owner: String,
            nonce: u32,
        }

        let request = DeletePaymentKeyRequest {
            owner: owner.to_string(),
            nonce,
        };

        tracing::info!(
            "🗑️ Deleting payment key from coordinator: owner={} nonce={}",
            owner, nonce
        );

        let response = self
            .client
            .post(&url)
            .bearer_auth(&self.auth_token)
            .json(&request)
            .send()
            .await
            .context("Failed to delete payment key from coordinator")?;

        if !response.status().is_success() {
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            anyhow::bail!("Delete payment key failed: {}", error_text)
        }

        tracing::info!(
            "✅ Payment key deleted from coordinator: owner={} nonce={}",
            owner, nonce
        );
        Ok(())
    }

    /// Create a DeletePaymentKey task in coordinator queue
    ///
    /// Called by event monitor when DeletePaymentKey event is detected.
    /// Worker will poll for these tasks and process them.
    pub async fn create_delete_payment_key_task(
        &self,
        params: DeletePaymentKeyTaskData,
    ) -> Result<Option<i64>> {
        let url = format!("{}/payment-keys/delete-task/create", self.base_url);

        #[derive(Serialize)]
        struct CreateDeleteTaskRequest {
            data_id: String,
            owner: String,
            nonce: u32,
        }

        #[derive(Deserialize)]
        struct CreateDeleteTaskResponse {
            task_id: i64,
            created: bool,
        }

        let request = CreateDeleteTaskRequest {
            data_id: params.data_id.clone(),
            owner: params.owner,
            nonce: params.nonce,
        };

        tracing::info!(
            "📝 Creating DeletePaymentKey task: data_id={} owner={} nonce={}",
            &params.data_id[..8.min(params.data_id.len())],
            request.owner,
            request.nonce
        );

        let response = self
            .client
            .post(&url)
            .bearer_auth(&self.auth_token)
            .json(&request)
            .send()
            .await
            .context("Failed to create delete payment key task")?;

        if !response.status().is_success() {
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            anyhow::bail!("Create delete task failed: {}", error_text)
        }

        let result: CreateDeleteTaskResponse = response.json().await?;
        if result.created {
            Ok(Some(result.task_id))
        } else {
            Ok(None)
        }
    }

    /// Create a project storage cleanup task in coordinator queue
    ///
    /// Called by event monitor when ProjectStorageCleanup event is detected.
    /// Worker will poll for these tasks and process them.
    pub async fn create_project_storage_cleanup_task(
        &self,
        project_id: &str,
        project_uuid: &str,
    ) -> Result<Option<i64>> {
        let url = format!("{}/projects/cleanup-task/create", self.base_url);

        #[derive(Serialize)]
        struct CreateCleanupTaskRequest {
            project_id: String,
            project_uuid: String,
        }

        #[derive(Deserialize)]
        struct CreateCleanupTaskResponse {
            task_id: i64,
            created: bool,
        }

        let request = CreateCleanupTaskRequest {
            project_id: project_id.to_string(),
            project_uuid: project_uuid.to_string(),
        };

        tracing::info!(
            "📝 Creating ProjectStorageCleanup task: project_id={} uuid={}",
            project_id, project_uuid
        );

        let response = self
            .client
            .post(&url)
            .bearer_auth(&self.auth_token)
            .json(&request)
            .send()
            .await
            .context("Failed to create project storage cleanup task")?;

        if !response.status().is_success() {
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            anyhow::bail!("Create cleanup task failed: {}", error_text)
        }

        let result: CreateCleanupTaskResponse = response.json().await?;
        if result.created {
            Ok(Some(result.task_id))
        } else {
            Ok(None)
        }
    }

    // =========================================================================
    // Unified System Callbacks Polling
    // =========================================================================

    /// Poll for ANY system callback task (TopUp, Delete, etc.) from unified queue
    ///
    /// This is the preferred method - replaces separate poll_topup_task and
    /// poll_delete_payment_key_task methods. Worker polls a single queue and
    /// dispatches based on task type.
    ///
    /// # Arguments
    /// * `timeout` - Seconds to wait (0 for non-blocking, max 120)
    ///
    /// # Returns
    /// * `Ok(Some(task))` - Task received, dispatch based on task_type
    /// * `Ok(None)` - No tasks available (timeout)
    /// * `Err(_)` - Request failed
    pub async fn poll_system_callback_task(&self, timeout: u64, capabilities: &[String]) -> Result<Option<SystemCallbackTask>> {
        let capabilities_param = capabilities.join(",");
        let url = format!("{}/system-callbacks/poll?timeout={}&capabilities={}", self.base_url, timeout, capabilities_param);

        let response = self
            .client
            .get(&url)
            .bearer_auth(&self.auth_token)
            .send()
            .await
            .context("Failed to poll system callback task")?;

        if !response.status().is_success() {
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            anyhow::bail!("Poll system callback task failed: {}", error_text)
        }

        let task: Option<SystemCallbackTask> = response.json().await
            .context("Failed to parse system callback task response")?;

        if let Some(ref t) = task {
            match t {
                SystemCallbackTask::TopUp(payload) => {
                    tracing::info!(
                        "📥 System callback: TopUp task_id={} owner={} nonce={}",
                        payload.task_id, payload.owner, payload.nonce
                    );
                }
                SystemCallbackTask::DeletePaymentKey(payload) => {
                    tracing::info!(
                        "📥 System callback: Delete task_id={} owner={} nonce={}",
                        payload.task_id, payload.owner, payload.nonce
                    );
                }
                SystemCallbackTask::ProjectStorageCleanup(payload) => {
                    tracing::info!(
                        "📥 System callback: ProjectStorageCleanup task_id={} project_id={} uuid={}",
                        payload.task_id, payload.project_id, payload.project_uuid
                    );
                }
            }
        }

        Ok(task)
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

    /// Resolve which branch contains a specific commit hash
    ///
    /// Calls coordinator's GitHub API integration with caching.
    ///
    /// # Arguments
    /// * `repo` - Repository URL (e.g., "alice/project" or full URL)
    /// * `commit` - Commit hash or branch name
    ///
    /// # Returns
    /// * `Ok(Some(branch))` - Branch name found
    /// * `Ok(None)` - Commit not found or branch could not be determined
    /// * `Err(_)` - API error
    pub async fn resolve_branch(&self, repo: &str, commit: &str) -> Result<Option<String>> {
        let url = format!("{}/github/resolve-branch", self.base_url);

        #[derive(Deserialize)]
        struct ResolveBranchResponse {
            branch: Option<String>,
            #[allow(dead_code)]
            repo_normalized: String,
            #[allow(dead_code)]
            cached: bool,
        }

        let response = self
            .client
            .get(&url)
            .query(&[("repo", repo), ("commit", commit)])
            .send() // Note: This endpoint is public (no auth token needed)
            .await
            .context("Failed to send resolve-branch request")?;

        match response.status() {
            StatusCode::OK => {
                let data: ResolveBranchResponse = response
                    .json()
                    .await
                    .context("Failed to parse resolve-branch response")?;
                Ok(data.branch)
            }
            StatusCode::NOT_FOUND => Ok(None), // Commit not found
            status => {
                let error_text = response
                    .text()
                    .await
                    .unwrap_or_else(|_| "Unknown error".to_string());
                anyhow::bail!(
                    "Resolve-branch failed with status {}: {}",
                    status,
                    error_text
                )
            }
        }
    }

    /// Resolve project_id to project_uuid via coordinator
    ///
    /// Calls coordinator's project API with Redis caching.
    /// UUIDs are cached forever since they never change.
    ///
    /// # Arguments
    /// * `project_id` - Project ID in format "owner.near/name"
    ///
    /// # Returns
    /// * `Ok(Some(ProjectInfo))` - Project found with UUID and active version
    /// * `Ok(None)` - Project not found
    /// * `Err(_)` - API error
    pub async fn resolve_project_uuid(&self, project_id: &str) -> Result<Option<ProjectUuidInfo>> {
        let url = format!("{}/projects/uuid", self.base_url);

        let response = self
            .client
            .get(&url)
            .header("Authorization", format!("Bearer {}", self.auth_token))
            .query(&[("project_id", project_id)])
            .send()
            .await
            .context("Failed to send resolve-project-uuid request")?;

        match response.status() {
            StatusCode::OK => {
                let data: ProjectUuidInfo = response
                    .json()
                    .await
                    .context("Failed to parse project-uuid response")?;
                Ok(Some(data))
            }
            StatusCode::NOT_FOUND => Ok(None),
            status => {
                let error_text = response
                    .text()
                    .await
                    .unwrap_or_else(|_| "Unknown error".to_string());
                anyhow::bail!(
                    "Resolve-project-uuid failed with status {}: {}",
                    status,
                    error_text
                )
            }
        }
    }

    /// Store task attestation in coordinator
    ///
    /// Sends TDX quote and task metadata to coordinator for public verification.
    /// This endpoint requires worker auth token.
    ///
    /// # Arguments
    /// * `request` - Attestation data with TDX quote and task metadata
    ///
    /// # Returns
    /// * `Ok(())` if attestation was stored successfully
    /// * `Err` if request failed
    pub async fn store_attestation(&self, request: StoreAttestationRequest) -> Result<()> {
        let url = format!("{}/attestations", self.base_url);

        tracing::debug!(
            task_id = request.task_id,
            task_type = %request.task_type,
            "Storing attestation in coordinator"
        );

        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.auth_token))
            .json(&request)
            .send()
            .await
            .context("Failed to send attestation to coordinator")?;

        match response.status() {
            StatusCode::CREATED => {
                tracing::info!(
                    task_id = request.task_id,
                    task_type = %request.task_type,
                    "Successfully stored attestation in coordinator"
                );
                Ok(())
            }
            StatusCode::BAD_REQUEST => {
                let error_text = response
                    .text()
                    .await
                    .unwrap_or_else(|_| "Invalid request".to_string());
                anyhow::bail!("Attestation validation failed: {}", error_text)
            }
            StatusCode::UNAUTHORIZED => {
                anyhow::bail!("Worker authentication failed - check API_AUTH_TOKEN")
            }
            status => {
                let error_text = response
                    .text()
                    .await
                    .unwrap_or_else(|_| "Unknown error".to_string());
                anyhow::bail!(
                    "Failed to store attestation with status {}: {}",
                    status,
                    error_text
                )
            }
        }
    }

    /// Clear all storage for a project (called when project is deleted)
    pub async fn clear_project_storage(&self, project_uuid: &str) -> Result<()> {
        let url = format!("{}/storage/clear-project", self.base_url);

        tracing::info!(
            project_uuid = project_uuid,
            "Clearing project storage in coordinator"
        );

        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.auth_token))
            .json(&serde_json::json!({ "project_uuid": project_uuid }))
            .send()
            .await
            .context("Failed to send clear-project request to coordinator")?;

        match response.status() {
            StatusCode::OK => {
                tracing::info!(
                    project_uuid = project_uuid,
                    "Successfully cleared project storage"
                );
                Ok(())
            }
            StatusCode::UNAUTHORIZED => {
                anyhow::bail!("Worker authentication failed - check API_AUTH_TOKEN")
            }
            status => {
                let error_text = response
                    .text()
                    .await
                    .unwrap_or_else(|_| "Unknown error".to_string());
                anyhow::bail!(
                    "Failed to clear project storage with status {}: {}",
                    status,
                    error_text
                )
            }
        }
    }
}

/// Task type enum matching coordinator's TaskType
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TaskType {
    Compile,
    Execute,
    /// TopUp Payment Key - process yield/resume for balance update
    Topup,
}

impl std::fmt::Display for TaskType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TaskType::Compile => write!(f, "compile"),
            TaskType::Execute => write!(f, "execute"),
            TaskType::Topup => write!(f, "topup"),
        }
    }
}

/// Parameters for creating a TopUp task
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TopUpTaskData {
    /// data_id for yield/resume (hex encoded)
    pub data_id: String,
    /// Payment Key owner
    pub owner: String,
    /// Payment Key nonce (profile)
    pub nonce: u32,
    /// TopUp amount in minimal token units
    pub amount: String,
    /// Current encrypted secret (base64)
    pub encrypted_data: String,
}

/// Data for a DeletePaymentKey task
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeletePaymentKeyTaskData {
    /// data_id for yield/resume (hex encoded)
    pub data_id: String,
    /// Payment Key owner
    pub owner: String,
    /// Payment Key nonce (profile)
    pub nonce: u32,
}

// =============================================================================
// Unified System Callback Task Type
// =============================================================================

/// Unified system callback task - matches coordinator's SystemCallbackTask
///
/// All contract business logic that requires yield/resume is processed through this.
/// Workers poll a single queue and dispatch based on task_type.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "task_type", rename_all = "snake_case")]
pub enum SystemCallbackTask {
    /// TopUp Payment Key - requires keystore to decrypt/re-encrypt
    TopUp(TopUpTaskPayload),
    /// Delete Payment Key - no keystore needed
    DeletePaymentKey(DeletePaymentKeyPayload),
    /// Project Storage Cleanup - clear compiled WASM and storage for deleted project
    ProjectStorageCleanup(ProjectStorageCleanupPayload),
}

/// TopUp task payload from unified queue
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TopUpTaskPayload {
    pub task_id: i64,
    pub data_id: String,
    pub owner: String,
    pub nonce: u32,
    pub amount: String,
    pub encrypted_data: String,
    pub created_at: i64,
}

/// DeletePaymentKey task payload from unified queue
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeletePaymentKeyPayload {
    pub task_id: i64,
    pub data_id: String,
    pub owner: String,
    pub nonce: u32,
    pub created_at: i64,
}

/// ProjectStorageCleanup task payload from unified queue
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectStorageCleanupPayload {
    pub task_id: i64,
    pub project_id: String,
    pub project_uuid: String,
    pub created_at: i64,
}

/// Request to store task attestation in coordinator
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoreAttestationRequest {
    pub task_id: i64,
    pub task_type: TaskType,

    // TDX attestation data
    pub tdx_quote: String, // base64 encoded

    // NEAR context (NULL for HTTPS calls)
    pub request_id: Option<i64>,
    pub caller_account_id: Option<String>,
    pub transaction_hash: Option<String>,
    pub block_height: Option<u64>,

    // HTTPS call context (NULL for NEAR calls)
    pub call_id: Option<String>,
    pub payment_key_owner: Option<String>,
    pub payment_key_nonce: Option<i32>,

    // Code source
    pub repo_url: Option<String>,
    pub commit_hash: Option<String>,
    pub build_target: Option<String>,

    // Task data hashes
    pub wasm_hash: Option<String>,
    pub input_hash: Option<String>,
    pub output_hash: String,

    // V1 attestation fields
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub secrets_ref: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attached_usd: Option<String>,
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
