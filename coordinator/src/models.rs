use serde::{Deserialize, Serialize};

/// Pricing configuration from NEAR contract
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PricingConfig {
    pub base_fee: String,                // yoctoNEAR
    pub per_instruction_fee: String,     // yoctoNEAR per million instructions
    pub per_ms_fee: String,              // yoctoNEAR per millisecond (execution)
    pub per_compile_ms_fee: String,      // yoctoNEAR per millisecond (compilation)
    pub max_compilation_seconds: u64,    // Maximum compilation time (from pricing)
    pub max_instructions: u64,           // Hard cap on instructions
    pub max_execution_seconds: u64,      // Hard cap on execution time
}

/// Job status enumeration
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum JobStatus {
    Pending,              // Waiting to be picked up by worker
    InProgress,           // Currently being processed
    Completed,            // Successfully completed
    Failed,               // Technical/infrastructure error
    CompilationFailed,    // Compilation error (repo doesn't exist, syntax error, build failed)
    ExecutionFailed,      // WASM execution error (panic, trap, timeout)
    AccessDenied,         // Access denied to secrets
    InsufficientPayment,  // Not enough payment for requested resources
    Custom,               // Custom error (see error_details)
}

impl JobStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            JobStatus::Pending => "pending",
            JobStatus::InProgress => "in_progress",
            JobStatus::Completed => "completed",
            JobStatus::Failed => "failed",
            JobStatus::CompilationFailed => "compilation_failed",
            JobStatus::ExecutionFailed => "execution_failed",
            JobStatus::AccessDenied => "access_denied",
            JobStatus::InsufficientPayment => "insufficient_payment",
            JobStatus::Custom => "custom",
        }
    }
}

/// Response format for execution output
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub enum ResponseFormat {
    Bytes,
    #[default]
    Text,
    Json,
}

/// Execution context metadata
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

/// Reference to secrets stored in contract (repo-based system)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecretsReference {
    pub profile: String,
    pub account_id: String,
}

/// Type of work to be performed by worker
/// Created by coordinator when breaking down ExecutionRequest into specific tasks
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum JobType {
    Compile,
    Execute,
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
    /// Code source - optional for HTTPS calls where project_id is used instead.
    /// Worker will resolve project_id → code_source from contract.
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transaction_hash: Option<String>,
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

    // ===== HTTPS API fields =====
    /// HTTPS API call flag - if true, report result to coordinator, not contract
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
    /// USD payment amount for HTTPS calls (in minimal token units)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub usd_payment: Option<String>,
    /// Compute limit in USD for HTTPS calls (X-Compute-Limit header value)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub compute_limit_usd: Option<String>,
    /// Attached deposit in USD for HTTPS calls (X-Attached-Deposit header value)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attached_deposit_usd: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum CodeSource {
    GitHub {
        repo: String,
        commit: String,
        #[serde(default = "default_build_target")]
        build_target: String,
    },
    /// Pre-compiled WASM file accessible via URL
    /// Worker downloads from URL, verifies SHA256 hash, then executes without compilation
    WasmUrl {
        url: String,           // URL for downloading (https://, ipfs://, ar://)
        hash: String,          // SHA256 hash for verification (hex encoded)
        #[serde(default = "default_build_target")]
        build_target: String,
    },
}

impl CodeSource {
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

    /// Get the hash for WasmUrl sources
    #[allow(dead_code)]
    pub fn hash(&self) -> Option<&str> {
        match self {
            CodeSource::GitHub { .. } => None,
            CodeSource::WasmUrl { hash, .. } => Some(hash),
        }
    }

    /// Check if this is a WasmUrl source (pre-compiled, no compilation needed)
    #[allow(dead_code)]
    pub fn is_wasm_url(&self) -> bool {
        matches!(self, CodeSource::WasmUrl { .. })
    }
}

fn default_build_target() -> String {
    "wasm32-wasip1".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_repo_url_with_https() {
        let source = CodeSource::GitHub {
            repo: "https://github.com/alice/project".to_string(),
            commit: "main".to_string(),
            build_target: "wasm32-wasip1".to_string(),
        };
        let normalized = source.normalize();
        match normalized {
            CodeSource::GitHub { repo, .. } => assert_eq!(repo, "https://github.com/alice/project"),
            _ => panic!("Expected GitHub variant"),
        }
    }

    #[test]
    fn test_normalize_repo_url_with_http() {
        let source = CodeSource::GitHub {
            repo: "http://github.com/alice/project".to_string(),
            commit: "main".to_string(),
            build_target: "wasm32-wasip1".to_string(),
        };
        let normalized = source.normalize();
        match normalized {
            CodeSource::GitHub { repo, .. } => assert_eq!(repo, "http://github.com/alice/project"),
            _ => panic!("Expected GitHub variant"),
        }
    }

    #[test]
    fn test_normalize_repo_url_github_com_prefix() {
        let source = CodeSource::GitHub {
            repo: "github.com/alice/project".to_string(),
            commit: "main".to_string(),
            build_target: "wasm32-wasip1".to_string(),
        };
        let normalized = source.normalize();
        match normalized {
            CodeSource::GitHub { repo, .. } => assert_eq!(repo, "https://github.com/alice/project"),
            _ => panic!("Expected GitHub variant"),
        }
    }

    #[test]
    fn test_normalize_repo_url_short_format() {
        let source = CodeSource::GitHub {
            repo: "alice/project".to_string(),
            commit: "main".to_string(),
            build_target: "wasm32-wasip1".to_string(),
        };
        let normalized = source.normalize();
        match normalized {
            CodeSource::GitHub { repo, .. } => assert_eq!(repo, "https://github.com/alice/project"),
            _ => panic!("Expected GitHub variant"),
        }
    }

    #[test]
    fn test_normalize_repo_url_invalid_format() {
        let source = CodeSource::GitHub {
            repo: "invalid".to_string(),
            commit: "main".to_string(),
            build_target: "wasm32-wasip1".to_string(),
        };
        let normalized = source.normalize();
        match normalized {
            // Invalid format should remain unchanged
            CodeSource::GitHub { repo, .. } => assert_eq!(repo, "invalid"),
            _ => panic!("Expected GitHub variant"),
        }
    }

    #[test]
    fn test_normalize_repo_url_ssh_format() {
        let source = CodeSource::GitHub {
            repo: "git@github.com:zavodil/private-ft.git".to_string(),
            commit: "main".to_string(),
            build_target: "wasm32-wasip1".to_string(),
        };
        let normalized = source.normalize();
        match normalized {
            CodeSource::GitHub { repo, .. } => assert_eq!(repo, "https://github.com/zavodil/private-ft"),
            _ => panic!("Expected GitHub variant"),
        }
    }

    #[test]
    fn test_normalize_repo_url_ssh_format_no_git_suffix() {
        let source = CodeSource::GitHub {
            repo: "git@github.com:alice/project".to_string(),
            commit: "main".to_string(),
            build_target: "wasm32-wasip1".to_string(),
        };
        let normalized = source.normalize();
        match normalized {
            CodeSource::GitHub { repo, .. } => assert_eq!(repo, "https://github.com/alice/project"),
            _ => panic!("Expected GitHub variant"),
        }
    }

    #[test]
    fn test_normalize_repo_url_ssh_url_format() {
        let source = CodeSource::GitHub {
            repo: "ssh://git@github.com/alice/project.git".to_string(),
            commit: "main".to_string(),
            build_target: "wasm32-wasip1".to_string(),
        };
        let normalized = source.normalize();
        match normalized {
            CodeSource::GitHub { repo, .. } => assert_eq!(repo, "https://github.com/alice/project"),
            _ => panic!("Expected GitHub variant"),
        }
    }

    #[test]
    fn test_normalize_wasm_url_source() {
        let source = CodeSource::WasmUrl {
            url: "https://example.com/my.wasm".to_string(),
            hash: "abc123def456".to_string(),
            build_target: "wasm32-wasip1".to_string(),
        };
        let normalized = source.normalize();
        match normalized {
            CodeSource::WasmUrl { url, hash, .. } => {
                assert_eq!(url, "https://example.com/my.wasm");
                assert_eq!(hash, "abc123def456");
            }
            _ => panic!("Expected WasmUrl variant"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceLimits {
    pub max_instructions: u64,
    pub max_memory_mb: u32,
    pub max_execution_seconds: u64,
}

/// API request/response types

#[derive(Debug, Deserialize)]
pub struct AcquireLockRequest {
    pub lock_key: String,
    pub worker_id: String,
    pub ttl_seconds: u64,
}

#[derive(Debug, Serialize)]
pub struct AcquireLockResponse {
    pub acquired: bool,
}

#[derive(Debug, Serialize)]
pub struct WasmExistsResponse {
    pub exists: bool,
    pub created_at: Option<String>, // ISO 8601 timestamp when WASM was compiled
}

/// Execution output - can be bytes, text, or parsed JSON
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ExecutionOutput {
    Bytes(Vec<u8>),
    Text(String),
    Json(serde_json::Value),
}

/// Complete job request - worker finished a job
#[derive(Debug, Deserialize)]
pub struct CompleteJobRequest {
    pub job_id: i64,
    pub success: bool,
    #[allow(dead_code)]
    pub output: Option<ExecutionOutput>,
    pub error: Option<String>,
    pub time_ms: u64,
    #[serde(default)]
    pub instructions: u64,
    #[serde(default)]
    pub wasm_checksum: Option<String>,
    #[serde(default)]
    pub actual_cost_yocto: Option<String>,
    #[serde(default)]
    pub compile_cost_yocto: Option<String>,
    /// Error category for better failure classification (optional, only when success=false)
    #[serde(default)]
    pub error_category: Option<JobStatus>,
    /// Result from compile job to pass to executor (e.g., FastFS URL or compilation error)
    /// When set, executor should call resolve_execution with this value without running WASM
    #[serde(default)]
    pub compile_result: Option<String>,
}

/// Legacy: Complete task request
#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct CompleteTaskRequest {
    pub request_id: u64,
    pub success: bool,
    pub output: Option<ExecutionOutput>,
    pub error: Option<String>,
    pub execution_time_ms: u64,
    #[serde(default)]
    pub instructions: u64,
    #[serde(default)]
    pub data_id: Option<String>,
    #[serde(default)]
    pub resolve_tx_id: Option<String>,
    #[serde(default)]
    pub user_account_id: Option<String>,
    #[serde(default)]
    pub near_payment_yocto: Option<String>,
    #[serde(default)]
    pub worker_id: Option<String>,
    #[serde(default)]
    pub github_repo: Option<String>,
    #[serde(default)]
    pub github_commit: Option<String>,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct FailTaskRequest {
    pub request_id: u64,
    pub error: String,
}

/// Claim job request - worker wants to claim work for a task
#[derive(Debug, Deserialize)]
pub struct ClaimJobRequest {
    pub request_id: u64,
    pub data_id: String,
    pub worker_id: String,
    pub code_source: CodeSource,
    #[allow(dead_code)]
    pub resource_limits: ResourceLimits,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_account_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub near_payment_yocto: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transaction_hash: Option<String>,
    /// Worker capabilities - what this worker can do
    /// Examples: ["compilation", "execution"], ["execution"], ["compilation"]
    #[serde(default)]
    pub capabilities: Vec<String>,
    /// Only compile, don't execute
    #[serde(default)]
    pub compile_only: bool,
    /// Force recompilation even if WASM exists in cache
    #[serde(default)]
    pub force_rebuild: bool,
    /// Whether compile_result exists (from ExecutionRequest)
    /// When true, executor should accept the task even with compile_only=true
    #[serde(default)]
    pub has_compile_result: bool,
    /// Project UUID for persistent storage (None for standalone WASM)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_uuid: Option<String>,
    /// Project ID for project-based secrets (e.g., "alice.near/my-app")
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_id: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ClaimJobResponse {
    pub jobs: Vec<JobInfo>,
    pub pricing: PricingConfig,  // Pricing from contract for budget validation
}

#[derive(Debug, Serialize)]
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
    /// Project UUID for storage (None for standalone WASM)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project_uuid: Option<String>,
    /// Project ID for project-based secrets (e.g., "alice.near/my-app")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project_id: Option<String>,
}

/// Create task request (event monitor)
#[derive(Debug, Deserialize)]
pub struct CreateTaskRequest {
    pub request_id: u64,
    pub code_source: CodeSource,
    pub resource_limits: ResourceLimits,
    pub input_data: String,
    pub data_id: String,
    #[serde(default)]
    pub secrets_ref: Option<SecretsReference>, // Reference to contract-stored secrets
    #[serde(default)]
    pub response_format: ResponseFormat,
    #[serde(default)]
    pub context: ExecutionContext,
    #[serde(default)]
    pub user_account_id: Option<String>,
    #[serde(default)]
    pub near_payment_yocto: Option<String>,
    #[serde(default)]
    pub transaction_hash: Option<String>,
    /// If true, only compile the code without executing
    #[serde(default)]
    pub compile_only: bool,
    /// Force recompilation even if WASM exists in cache
    #[serde(default)]
    pub force_rebuild: bool,
    /// Store compiled WASM to FastFS after compilation
    #[serde(default)]
    pub store_on_fastfs: bool,
    /// Project UUID for persistent storage (None for standalone WASM)
    #[serde(default)]
    pub project_uuid: Option<String>,
    /// Project ID for project-based secrets (e.g., "alice.near/my-app")
    #[serde(default)]
    pub project_id: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct CreateTaskResponse {
    pub request_id: i64,
    pub created: bool,
}

/// System hidden log entry (for admin debugging only - raw stderr/stdout/errors)
#[derive(Debug, Serialize, Deserialize)]
pub struct SystemHiddenLog {
    pub id: i64,
    pub request_id: i64,
    pub job_id: Option<i64>,
    pub log_type: String, // 'compilation' or 'execution'
    pub stderr: Option<String>,
    pub stdout: Option<String>,
    pub exit_code: Option<i32>,
    pub execution_error: Option<String>,
    pub created_at: String,
}

/// Request to store system hidden logs (from worker)
#[derive(Debug, Deserialize)]
pub struct StoreSystemLogRequest {
    pub request_id: i64,
    pub job_id: Option<i64>,
    pub log_type: String, // 'compilation' or 'execution'
    #[serde(default)]
    pub stderr: Option<String>,
    #[serde(default)]
    pub stdout: Option<String>,
    #[serde(default)]
    pub exit_code: Option<i32>,
    #[serde(default)]
    pub execution_error: Option<String>,
}

// ============================================================================
// ATTESTATION MODELS
// ============================================================================

/// Task type for attestation (unified for both Compile and Execute)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum TaskType {
    Compile,
    Execute,
}

impl TaskType {
    pub fn as_str(&self) -> &'static str {
        match self {
            TaskType::Compile => "compile",
            TaskType::Execute => "execute",
        }
    }
}

impl std::str::FromStr for TaskType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "compile" => Ok(TaskType::Compile),
            "execute" => Ok(TaskType::Execute),
            _ => Err(format!("Invalid task type: {}", s)),
        }
    }
}

/// Task attestation record from database (unified format)
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct TaskAttestation {
    pub id: i64,
    pub task_id: i64,
    pub task_type: String,

    // TDX attestation data
    pub tdx_quote: Vec<u8>,
    pub worker_measurement: String,

    // NEAR context (NULL for HTTPS calls)
    pub request_id: Option<i64>,
    pub caller_account_id: Option<String>,
    pub transaction_hash: Option<String>,
    pub block_height: Option<i64>,

    // HTTPS call context (NULL for NEAR calls)
    pub call_id: Option<uuid::Uuid>,
    pub payment_key_owner: Option<String>,
    pub payment_key_nonce: Option<i32>,

    // Code source (both task types have this)
    pub repo_url: Option<String>,
    pub commit_hash: Option<String>,
    pub build_target: Option<String>,

    // Task data hashes (unified)
    pub wasm_hash: Option<String>,
    pub input_hash: Option<String>,  // NULL for Compile, present for Execute
    pub output_hash: String,

    pub created_at: Option<chrono::NaiveDateTime>,
}

/// API response for attestation queries
#[derive(Debug, Clone, Serialize)]
pub struct AttestationResponse {
    pub id: i64,
    pub task_id: i64,
    pub task_type: String,

    // TDX attestation data
    pub tdx_quote: String,  // base64 encoded
    pub worker_measurement: String,

    // NEAR context (NULL for HTTPS calls)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_id: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub caller_account_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transaction_hash: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub block_height: Option<i64>,

    // HTTPS call context (NULL for NEAR calls)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub call_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payment_key_owner: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payment_key_nonce: Option<i32>,

    // Code source
    #[serde(skip_serializing_if = "Option::is_none")]
    pub repo_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub commit_hash: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub build_target: Option<String>,

    // Task data hashes
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wasm_hash: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input_hash: Option<String>,
    pub output_hash: String,

    pub timestamp: i64,  // Unix timestamp
}

impl From<TaskAttestation> for AttestationResponse {
    fn from(att: TaskAttestation) -> Self {
        use base64::Engine;

        Self {
            id: att.id,
            task_id: att.task_id,
            task_type: att.task_type,
            tdx_quote: base64::engine::general_purpose::STANDARD.encode(&att.tdx_quote),
            worker_measurement: att.worker_measurement,
            request_id: att.request_id,
            caller_account_id: att.caller_account_id,
            transaction_hash: att.transaction_hash,
            block_height: att.block_height,
            // HTTPS call context
            call_id: att.call_id.map(|id| id.to_string()),
            payment_key_owner: att.payment_key_owner,
            payment_key_nonce: att.payment_key_nonce,
            repo_url: att.repo_url,
            commit_hash: att.commit_hash,
            build_target: att.build_target,
            wasm_hash: att.wasm_hash,
            input_hash: att.input_hash,
            output_hash: att.output_hash,
            timestamp: att.created_at.map(|dt| dt.and_utc().timestamp()).unwrap_or(0),
        }
    }
}

/// Request to store attestation (from worker - unified format)
#[derive(Debug, Clone, Deserialize)]
pub struct StoreAttestationRequest {
    pub task_id: i64,
    pub task_type: TaskType,

    // TDX attestation data
    pub tdx_quote: String,  // base64 encoded

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
}

impl StoreAttestationRequest {
    /// Validate the request based on task type
    pub fn validate(&self) -> Result<(), String> {
        match self.task_type {
            TaskType::Execute => {
                // input_hash is optional for startup/special tasks (task_id = -1)
                // but should be present for normal execution tasks
                if self.task_id >= 0 && self.input_hash.is_none() {
                    return Err("Execute tasks must have input_hash (except startup tasks)".to_string());
                }
            }
            TaskType::Compile => {
                // Compile tasks don't need input_hash
            }
        }

        // commit_hash validation - can be Git SHA1 (40 chars) or branch name (any length)
        // No strict length validation needed

        // Skip hash length validation for special tasks (task_id = -1) like startup
        if self.task_id >= 0 {
            if let Some(ref hash) = self.wasm_hash {
                if hash.len() != 64 {
                    return Err("wasm_hash must be 64 characters (SHA256 hex)".to_string());
                }
            }

            if let Some(ref hash) = self.input_hash {
                if hash.len() != 64 {
                    return Err("input_hash must be 64 characters (SHA256 hex)".to_string());
                }
            }

            if self.output_hash.len() != 64 {
                return Err("output_hash must be 64 characters (SHA256 hex)".to_string());
            }
        }

        Ok(())
    }
}

// ============================================================================
// API KEY MODELS
// ============================================================================

/// API key record from database
#[allow(dead_code)]
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ApiKey {
    pub id: i64,
    pub api_key: String,  // SHA256 hash of the actual key
    pub near_account_id: String,
    pub key_name: Option<String>,
    pub is_active: bool,
    pub rate_limit_per_minute: i32,
    pub created_at: chrono::NaiveDateTime,
    pub last_used_at: Option<chrono::NaiveDateTime>,
}

/// Request to create a new API key (admin endpoint)
#[derive(Debug, Clone, Deserialize)]
pub struct CreateApiKeyRequest {
    pub near_account_id: String,
    #[serde(default)]
    pub key_name: Option<String>,
    #[serde(default)]
    pub rate_limit_per_minute: Option<i32>,
}

impl CreateApiKeyRequest {
    /// Validate NEAR account ID format
    pub fn validate(&self) -> Result<(), String> {
        // Basic NEAR account validation
        if self.near_account_id.is_empty() {
            return Err("near_account_id cannot be empty".to_string());
        }

        if self.near_account_id.len() > 64 {
            return Err("near_account_id too long (max 64 characters)".to_string());
        }

        // Check if it's a valid NEAR account format (simplified)
        let is_valid = self.near_account_id.ends_with(".near")
            || self.near_account_id.ends_with(".testnet")
            || (self.near_account_id.len() >= 2
                && self.near_account_id.len() <= 64
                && self
                    .near_account_id
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '_' || c == '-'));

        if !is_valid {
            return Err("Invalid NEAR account ID format".to_string());
        }

        // Validate rate limit if provided
        if let Some(limit) = self.rate_limit_per_minute {
            if limit <= 0 || limit > 600 {
                return Err("rate_limit_per_minute must be between 1 and 600".to_string());
            }
        }

        Ok(())
    }
}

/// Response when creating a new API key
#[derive(Debug, Clone, Serialize)]
pub struct CreateApiKeyResponse {
    pub api_key: String,  // Plaintext key (only shown ONCE!)
    pub near_account_id: String,
    pub rate_limit_per_minute: i32,
    pub created_at: i64,  // Unix timestamp
}

/// Public API key info (without the actual key)
#[allow(dead_code)]
#[derive(Debug, Clone, Serialize)]
pub struct ApiKeyInfo {
    pub id: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key_name: Option<String>,
    pub near_account_id: String,
    pub is_active: bool,
    pub rate_limit_per_minute: i32,
    pub created_at: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_used_at: Option<i64>,
}

impl From<ApiKey> for ApiKeyInfo {
    fn from(key: ApiKey) -> Self {
        Self {
            id: key.id,
            key_name: key.key_name,
            near_account_id: key.near_account_id,
            is_active: key.is_active,
            rate_limit_per_minute: key.rate_limit_per_minute,
            created_at: key.created_at.and_utc().timestamp(),
            last_used_at: key.last_used_at.map(|dt| dt.and_utc().timestamp()),
        }
    }
}

/// Request to generate API key from dashboard (with NEAR wallet signature)
#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize)]
pub struct GenerateApiKeyRequest {
    pub near_account_id: String,
    pub signature: String,  // Signature from NEAR wallet
    pub message: String,    // Message that was signed
    pub public_key: String, // Public key that signed the message
    #[serde(default)]
    pub key_name: Option<String>,
}

impl GenerateApiKeyRequest {
    /// Verify the signature matches the account
    #[allow(dead_code)]
    pub fn verify(&self) -> Result<(), String> {
        // TODO: Implement NEAR signature verification
        // For now, just validate the account format
        if self.near_account_id.is_empty() {
            return Err("near_account_id cannot be empty".to_string());
        }

        // Message should contain the account ID
        if !self.message.contains(&self.near_account_id) {
            return Err("Message must contain the NEAR account ID".to_string());
        }

        Ok(())
    }
}
