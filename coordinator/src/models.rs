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
}

/// Reference to secrets stored in contract (repo-based system)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecretsReference {
    pub profile: String,
    pub account_id: String,
}

/// Job types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum JobType {
    Compile,
    Execute,
}

/// Task types (kept for backward compatibility with worker polling)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Task {
    Compile {
        request_id: u64,
        data_id: String,
        code_source: CodeSource,
        resource_limits: ResourceLimits,
        input_data: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        secrets_ref: Option<SecretsReference>,
        #[serde(default)]
        response_format: ResponseFormat,
        #[serde(default)]
        context: ExecutionContext,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        user_account_id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        near_payment_yocto: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        transaction_hash: Option<String>,
    },
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
}

impl CodeSource {
    /// Normalize repo URL to full https:// format for git clone
    /// Examples:
    /// - "github.com/user/repo" -> "https://github.com/user/repo"
    /// - "https://github.com/user/repo" -> "https://github.com/user/repo" (unchanged)
    /// - "user/repo" -> "https://github.com/user/repo"
    pub fn normalize(mut self) -> Self {
        match &mut self {
            CodeSource::GitHub { repo, .. } => {
                // Skip if already has protocol
                if repo.starts_with("https://") || repo.starts_with("http://") {
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
        }
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
        if let CodeSource::GitHub { repo, .. } = normalized {
            assert_eq!(repo, "https://github.com/alice/project");
        } else {
            panic!("Expected GitHub variant");
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
        if let CodeSource::GitHub { repo, .. } = normalized {
            assert_eq!(repo, "http://github.com/alice/project");
        } else {
            panic!("Expected GitHub variant");
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
        if let CodeSource::GitHub { repo, .. } = normalized {
            assert_eq!(repo, "https://github.com/alice/project");
        } else {
            panic!("Expected GitHub variant");
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
        if let CodeSource::GitHub { repo, .. } = normalized {
            assert_eq!(repo, "https://github.com/alice/project");
        } else {
            panic!("Expected GitHub variant");
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
        if let CodeSource::GitHub { repo, .. } = normalized {
            // Invalid format should remain unchanged
            assert_eq!(repo, "invalid");
        } else {
            panic!("Expected GitHub variant");
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
}

/// Legacy: Complete task request
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
    pub resource_limits: ResourceLimits,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_account_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub near_payment_yocto: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transaction_hash: Option<String>,
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
}

#[derive(Debug, Serialize)]
pub struct CreateTaskResponse {
    pub request_id: i64,
    pub created: bool,
}
