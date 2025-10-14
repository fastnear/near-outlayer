use serde::{Deserialize, Serialize};

/// Task types
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
        encrypted_secrets: Option<Vec<u8>>,
    },
    Execute {
        request_id: u64,
        data_id: String,
        wasm_checksum: String,
        resource_limits: ResourceLimits,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        encrypted_secrets: Option<Vec<u8>>,
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

fn default_build_target() -> String {
    "wasm32-wasip1".to_string()
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
}

#[derive(Debug, Deserialize)]
pub struct CompleteTaskRequest {
    pub request_id: u64,
    pub success: bool,
    pub output: Option<Vec<u8>>,
    pub error: Option<String>,
    pub execution_time_ms: u64,
}

#[derive(Debug, Deserialize)]
pub struct FailTaskRequest {
    pub request_id: u64,
    pub error: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateTaskRequest {
    pub request_id: u64,
    pub code_source: CodeSource,
    pub resource_limits: ResourceLimits,
    pub input_data: String,
    pub data_id: String,
    #[serde(default)]
    pub encrypted_secrets: Option<Vec<u8>>,
}
