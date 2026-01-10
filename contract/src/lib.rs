use near_sdk::borsh::{BorshDeserialize, BorshSerialize};
use near_sdk::collections::{LookupMap, UnorderedMap, UnorderedSet};
use near_sdk::json_types::U128;
use near_sdk::serde::Serialize;
use near_sdk::{
    env, log, near, near_bindgen, AccountId, BorshStorageKey, Gas, GasWeight, NearToken,
    PanicOnDefault, PromiseError,
};
use std::convert::TryInto;

mod admin;
mod events;
mod execution;
mod migration;
mod payment;
mod projects;
mod secrets;
mod types;
mod views;

pub type Balance = u128;
pub type CryptoHash = [u8; 32];

// Gas constants
pub const MIN_RESPONSE_GAS: Gas = Gas::from_tgas(50);
pub const DATA_ID_REGISTER: u64 = 37;

// Timeout for stale execution cancellation (10 minutes)
pub const EXECUTION_TIMEOUT: u64 = 600 * 1_000_000_000;

// Maximum resource limits (hard caps)
pub const MAX_INSTRUCTIONS: u64 = 500_000_000_000; // 500 billion instructions
pub const MAX_EXECUTION_SECONDS: u64 = 180; // 180 seconds
pub const MAX_COMPILATION_SECONDS: u64 = 300; // 5 minutes max compilation time

#[derive(BorshSerialize, BorshStorageKey)]
#[borsh(crate = "near_sdk::borsh")]
enum StorageKey {
    PendingRequests,
    SecretsStorage,
    UserSecretsIndex,
    UserSecretsList { account_id: AccountId },
    // Project storage
    Projects,
    ProjectVersions { project_uuid: String },
    UserProjects,
    UserProjectsList { account_id: AccountId },
}

/// Execution source - GitHub repo, pre-compiled WASM URL, or project reference
#[derive(Clone, Debug)]
#[near(serializers = [borsh, json])]
pub enum ExecutionSource {
    /// GitHub repository with source code to compile
    GitHub {
        repo: String,
        commit: String,
        build_target: Option<String>, // e.g., "wasm32-wasip1"
    },
    /// Pre-compiled WASM file accessible via URL
    /// Worker downloads from URL, verifies SHA256 hash, then executes without compilation
    WasmUrl {
        url: String,           // URL for downloading (https://, ipfs://, ar://)
        hash: String,          // SHA256 hash for verification (hex encoded)
        build_target: Option<String>, // e.g., "wasm32-wasip1", "wasm32-wasip2"
    },
    /// Project reference - uses registered project's code
    /// If version_key is None, uses active version
    Project {
        project_id: String,              // "alice.near/my-app"
        version_key: Option<String>,     // None = active version, Some = specific version
    },
}

/// Resolved code source for worker (GitHub or WasmUrl only, no Project)
/// This is what gets sent to worker after resolving Project references
#[derive(Clone, Debug)]
#[near(serializers = [borsh, json])]
pub enum CodeSource {
    GitHub {
        repo: String,
        commit: String,
        build_target: Option<String>,
    },
    WasmUrl {
        url: String,
        hash: String,
        build_target: Option<String>,
    },
}

/// Optional request parameters for additional options
#[derive(Clone, Debug, Default)]
#[near(serializers = [borsh, json])]
pub struct RequestParams {
    /// Force recompilation even if WASM exists in cache
    #[serde(default)]
    pub force_rebuild: bool,

    /// Store compiled WASM to FastFS after compilation
    /// Path will be: /{checksum}.wasm
    #[serde(default)]
    pub store_on_fastfs: bool,

    /// Compile only flag. Also set = true if resource_limits is none
    #[serde(default)]
    pub compile_only: bool,

    /// Project UUID for project-based execution
    /// Set automatically by request_execution_project
    /// Used by worker to enable persistent storage for the project
    #[serde(default)]
    pub project_uuid: Option<String>,
}

/// Response format for execution output
#[derive(Clone, Debug, PartialEq, Eq)]
#[near(serializers = [borsh, json])]
pub enum ResponseFormat {
    /// Raw bytes - no parsing
    Bytes,
    /// UTF-8 text string (default)
    Text,
    /// Parse stdout as JSON
    Json,
}

impl Default for ResponseFormat {
    fn default() -> Self {
        Self::Text
    }
}

/// Resource limits for execution
#[derive(Clone, Debug)]
#[near(serializers = [borsh, json])]
pub struct ResourceLimits {
    pub max_instructions: Option<u64>,
    pub max_memory_mb: Option<u32>,
    pub max_execution_seconds: Option<u64>,
}

impl Default for ResourceLimits {
    fn default() -> Self {
        Self {
            max_instructions: Some(1_000_000_000), // 1B instructions
            max_memory_mb: Some(128),              // 128 MB
            max_execution_seconds: Some(60),       // 60 seconds
        }
    }
}

/// Execution request stored in contract
#[derive(Clone, Debug)]
#[near(serializers = [borsh, json])]
pub struct ExecutionRequest {
    pub request_id: u64,
    pub data_id: CryptoHash,
    pub sender_id: AccountId,
    pub execution_source: ExecutionSource,  // Original source (may be Project)
    pub resolved_source: CodeSource,         // Resolved source for worker (GitHub/WasmUrl only)
    pub resource_limits: ResourceLimits,
    pub payment: Balance,
    pub timestamp: u64,
    pub secrets_ref: Option<SecretsReference>, // Reference to repo-based secrets
    pub response_format: ResponseFormat,
    pub input_data: Option<String>, // Optional input data for execution
    pub payer_account_id: AccountId, // Account to receive refunds (explicit or defaults to sender)

    // Large output handling (2-call flow)
    pub pending_output: Option<StoredOutput>, // Temporary storage for large output data
    pub output_submitted: bool, // Flag indicating output data has been submitted
}

/// Execution output - can be bytes, text, or parsed JSON
#[derive(Clone, Debug)]
#[near(serializers = [json])]
pub enum ExecutionOutput {
    Bytes(Vec<u8>),
    Text(String),
    Json(serde_json::Value),
}

/// Internal storage format for ExecutionOutput (Borsh-compatible)
/// Stores all data as Vec<u8> for efficient serialization
#[derive(Clone, Debug)]
#[near(serializers = [borsh, json])]
pub enum StoredOutput {
    Bytes(Vec<u8>),
    Text(Vec<u8>),      // UTF-8 bytes
    Json(Vec<u8>),      // JSON string as UTF-8 bytes
}

impl From<ExecutionOutput> for StoredOutput {
    fn from(output: ExecutionOutput) -> Self {
        match output {
            ExecutionOutput::Bytes(bytes) => StoredOutput::Bytes(bytes),
            ExecutionOutput::Text(text) => StoredOutput::Text(text.into_bytes()),
            ExecutionOutput::Json(value) => {
                let json_str = serde_json::to_string(&value).unwrap_or_default();
                StoredOutput::Json(json_str.into_bytes())
            }
        }
    }
}

impl From<StoredOutput> for ExecutionOutput {
    fn from(stored: StoredOutput) -> Self {
        match stored {
            StoredOutput::Bytes(bytes) => ExecutionOutput::Bytes(bytes),
            StoredOutput::Text(bytes) => ExecutionOutput::Text(
                String::from_utf8(bytes).unwrap_or_else(|_| String::from("[invalid UTF-8]"))
            ),
            StoredOutput::Json(bytes) => {
                let json_str = String::from_utf8(bytes).unwrap_or_default();
                ExecutionOutput::Json(
                    serde_json::from_str(&json_str).unwrap_or(serde_json::Value::Null)
                )
            }
        }
    }
}

/// Execution response from worker
#[derive(Clone, Debug)]
#[near(serializers = [json])]
pub struct ExecutionResponse {
    pub success: bool,
    pub output: Option<ExecutionOutput>,
    pub error: Option<String>,
    pub resources_used: ResourceMetrics,
    pub compilation_note: Option<String>, // e.g., "Cached WASM from 2025-01-10 14:30 UTC"
}

/// Resource usage metrics
#[derive(Clone, Debug)]
#[near(serializers = [json])]
pub struct ResourceMetrics {
    pub instructions: u64,        // Instructions used during WASM execution
    pub time_ms: u64,              // Execution time in milliseconds
    pub compile_time_ms: Option<u64>, // Compilation time in milliseconds (if compiled)
}

/// Reference to secrets stored in contract (new approach)
#[derive(Clone, Debug)]
#[near(serializers = [borsh, json])]
pub struct SecretsReference {
    pub profile: String,      // Profile name (e.g., "default", "premium")
    pub account_id: AccountId, // Account that owns the secrets
}

/// Secret profile stored in contract (internal storage)
#[derive(Clone, Debug)]
#[near(serializers = [borsh])]
pub struct SecretProfile {
    pub encrypted_secrets: String,      // base64-encoded encrypted secrets
    pub access: types::AccessCondition, // Access control rules
    pub created_at: u64,                // Timestamp when created
    pub updated_at: u64,                // Timestamp when last updated
    pub storage_deposit: Balance,       // Storage staking amount (u128 for cheaper storage)
}

/// Secret profile for JSON view (returned from view methods)
#[derive(Clone, Debug)]
#[near(serializers = [json])]
pub struct SecretProfileView {
    pub encrypted_secrets: String,      // base64-encoded encrypted secrets
    pub access: types::AccessCondition, // Access control rules
    pub created_at: u64,                // Timestamp when created
    pub updated_at: u64,                // Timestamp when last updated
    pub storage_deposit: U128,          // Storage staking amount (U128 for JSON)
    pub accessor: SecretAccessor,       // What code can access this secret (Repo or WasmHash)
}


/// System secret types (Payment Keys, etc.)
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[near(serializers = [borsh, json])]
pub enum SystemSecretType {
    /// Payment Key for HTTPS API
    PaymentKey,
}

/// Secret accessor - defines what code can access/decrypt the secret
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[near(serializers = [borsh, json])]
pub enum SecretAccessor {
    /// Secrets bound to a GitHub repository and optional branch
    Repo {
        repo: String,           // Normalized repo path: "github.com/owner/repo"
        branch: Option<String>, // Branch name or None for all branches
    },
    /// Secrets bound to a specific WASM hash (for WasmUrl sources)
    WasmHash {
        hash: String,           // SHA256 hash of the WASM binary
    },
    /// Secrets bound to a project (available to all versions)
    Project {
        project_id: String,     // "alice.near/my-app"
    },
    /// System secrets (Payment Keys for HTTPS API)
    System(SystemSecretType),
}

/// Composite key for secrets storage
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[near(serializers = [borsh])]
pub struct SecretKey {
    pub accessor: SecretAccessor, // What code can access this secret (Repo, WasmHash, or Project)
    pub profile: String,          // Profile name: "default", "premium", etc.
    pub owner: AccountId,         // Account that created these secrets
}

// ============================================================================
// Project System - Persistent Storage for WASM Applications
// ============================================================================

/// Project stored in contract
#[derive(Clone, Debug)]
#[near(serializers = [borsh, json])]
pub struct Project {
    pub uuid: String,              // Internal UUID: "proj_a1b2c3d4"
    pub owner: AccountId,          // alice.near
    pub name: String,              // "my-app"
    pub active_version: String,    // wasm_hash of active version
    pub created_at: u64,
    pub storage_deposit: Balance,  // Storage staking for contract data
}

/// Version info stored per project
#[derive(Clone, Debug)]
#[near(serializers = [borsh, json])]
pub struct VersionInfo {
    pub source: CodeSource,
    pub added_at: u64,
    pub storage_deposit: Balance,  // Storage staking for this version entry
}

/// Pending version request (for yield/resume flow)
#[derive(Clone, Debug)]
#[near(serializers = [borsh])]
pub struct PendingVersion {
    pub project_uuid: String,
    pub project_id: String,        // "alice.near/my-app" for metadata verification
    pub source: CodeSource,
    pub set_active: bool,
    pub requested_at: u64,
    pub data_id: CryptoHash,
}

/// Project view for JSON responses
#[derive(Clone, Debug)]
#[near(serializers = [json])]
pub struct ProjectView {
    pub uuid: String,
    pub owner: AccountId,
    pub name: String,
    pub project_id: String,        // "alice.near/my-app"
    pub active_version: String,
    pub created_at: u64,
    pub storage_deposit: U128,
}

/// Version view for JSON responses
#[derive(Clone, Debug)]
#[near(serializers = [json])]
pub struct VersionView {
    pub wasm_hash: String,
    pub source: CodeSource,
    pub added_at: u64,
    pub is_active: bool,
}

/// Pricing view for JSON responses (includes both NEAR and USD pricing)
#[derive(Clone, Debug)]
#[near(serializers = [json])]
pub struct PricingView {
    // NEAR pricing (for blockchain transactions)
    pub base_fee: U128,
    pub per_million_instructions_fee: U128,
    pub per_ms_fee: U128,
    pub per_compile_ms_fee: U128,
    // USD pricing (for HTTPS API, in minimal token units)
    pub base_fee_usd: U128,
    pub per_million_instructions_fee_usd: U128,
    pub per_ms_fee_usd: U128,
    pub per_compile_ms_fee_usd: U128,
}

#[derive(BorshDeserialize, BorshSerialize, PanicOnDefault)]
#[borsh(crate = "near_sdk::borsh")]
#[near_bindgen]
pub struct Contract {
    // Contract configuration
    owner_id: AccountId,
    operator_id: AccountId,
    paused: bool,

    // Event metadata (for NEP-297 events)
    event_standard: String,
    event_version: String,

    // Pricing (NEAR) - for blockchain transactions
    base_fee: Balance,
    per_million_instructions_fee: Balance,
    per_ms_fee: Balance,                 // Execution time cost
    per_compile_ms_fee: Balance,         // Compilation time cost

    // Pricing (USD) - for HTTPS API (in minimal token units, e.g., 1 = 0.000001 USDT)
    base_fee_usd: u128,                      // e.g., 10000 = $0.01
    per_million_instructions_fee_usd: u128,  // e.g., 1 = $0.000001 per 1M instructions
    per_ms_fee_usd: u128,                    // e.g., 10 = $0.00001 per ms execution
    per_compile_ms_fee_usd: u128,            // e.g., 10 = $0.00001 per ms compilation

    // Payment token for HTTPS API (e.g., "usdt.tether-token.near")
    payment_token_contract: Option<AccountId>,

    // Request tracking
    next_request_id: u64,
    pending_requests: LookupMap<u64, ExecutionRequest>,

    // Statistics
    total_executions: u64,
    total_fees_collected: Balance,

    // Repo-based secrets storage
    secrets_storage: LookupMap<SecretKey, SecretProfile>,

    // User secrets index: account_id -> set of SecretKey
    user_secrets_index: LookupMap<AccountId, UnorderedSet<SecretKey>>,

    // Project storage: project_id ("alice.near/my-app") -> Project
    projects: LookupMap<String, Project>,

    // Project versions: project_uuid -> (wasm_hash -> VersionInfo)
    project_versions: LookupMap<String, UnorderedMap<String, VersionInfo>>,

    // User projects index: account_id -> set of project_id
    user_projects_index: LookupMap<AccountId, UnorderedSet<String>>,

    // Next project UUID counter
    next_project_id: u64,
}

#[near_bindgen]
impl Contract {
    #[init]
    pub fn new(owner_id: AccountId, operator_id: Option<AccountId>, event_standard: Option<String>,
        event_version: Option<String>) -> Self {
        Self {
            owner_id: owner_id.clone(),
            operator_id: operator_id.unwrap_or(owner_id),
            paused: false,
            event_standard: event_standard.unwrap_or("near-outlayer".to_string()),
            event_version: event_version.unwrap_or("1.0.0".to_string()),
            // NEAR pricing
            base_fee: 1_000_000_000_000_000_000_000, // 0.001 NEAR
            per_million_instructions_fee: 100_000_000_000_000, // 0.0000001 NEAR per million instructions
            per_ms_fee: 100_000_000_000_000_000, // 0.0001 NEAR per second (execution)
            per_compile_ms_fee: 100_000_000_000_000_000, // 0.0001 NEAR per second (compilation)
            // USD pricing (for HTTPS API, using USDT with 6 decimals)
            base_fee_usd: 10_000,             // $0.01 base fee
            per_million_instructions_fee_usd: 1, // $0.000001 per million instructions
            per_ms_fee_usd: 10,               // $0.00001 per ms execution
            per_compile_ms_fee_usd: 10,       // $0.00001 per ms compilation
            // Payment token (set via admin method)
            payment_token_contract: None,
            next_request_id: 0,
            pending_requests: LookupMap::new(StorageKey::PendingRequests),
            total_executions: 0,
            total_fees_collected: 0,
            secrets_storage: LookupMap::new(StorageKey::SecretsStorage),
            user_secrets_index: LookupMap::new(StorageKey::UserSecretsIndex),
            // Project system
            projects: LookupMap::new(StorageKey::Projects),
            project_versions: LookupMap::new(b"pv".to_vec()),
            user_projects_index: LookupMap::new(StorageKey::UserProjects),
            next_project_id: 0,
        }
    }
}

impl Contract {
    fn assert_not_paused(&self) {
        assert!(!self.paused, "Contract is paused");
    }

    fn assert_operator(&self) {
        assert_eq!(
            env::predecessor_account_id(),
            self.operator_id,
            "Only operator can call this"
        );
    }

    fn calculate_cost(&self, metrics: &ResourceMetrics) -> Balance {
        let instruction_cost =
            (metrics.instructions / 1_000_000) as u128 * self.per_million_instructions_fee;
        let time_cost = metrics.time_ms as u128 * self.per_ms_fee;

        // Add compilation cost if compilation occurred (uses separate, higher rate)
        let compile_cost = metrics.compile_time_ms
            .map(|ms| ms as u128 * self.per_compile_ms_fee)
            .unwrap_or(0);

        self.base_fee + instruction_cost + time_cost + compile_cost
    }

    /// Estimate cost based on resource limits
    fn estimate_cost(&self, limits: &ResourceLimits) -> Balance {
        // Use requested limits or defaults
        let max_instructions = limits.max_instructions.unwrap_or(1_000_000_000);
        let max_execution_seconds = limits.max_execution_seconds.unwrap_or(60);
        let max_time_ms = max_execution_seconds * 1000;

        // Calculate worst-case cost
        let instruction_cost = (max_instructions / 1_000_000) as u128 * self.per_million_instructions_fee;
        let time_cost = max_time_ms as u128 * self.per_ms_fee;

        self.base_fee + instruction_cost + time_cost
    }
}

#[cfg(test)]
mod tests;
