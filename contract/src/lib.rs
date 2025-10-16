use near_sdk::borsh::{BorshDeserialize, BorshSerialize};
use near_sdk::collections::LookupMap;
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
mod views;

pub type Balance = u128;
pub type CryptoHash = [u8; 32];

// Gas constants
pub const MIN_RESPONSE_GAS: Gas = Gas::from_tgas(50);
pub const DATA_ID_REGISTER: u64 = 37;

// Timeout for stale execution cancellation (10 minutes)
pub const EXECUTION_TIMEOUT: u64 = 600 * 1_000_000_000;

// Maximum resource limits (hard caps)
pub const MAX_INSTRUCTIONS: u64 = 100_000_000_000; // 100 billion instructions
pub const MAX_EXECUTION_SECONDS: u64 = 60; // 60 seconds

#[derive(BorshSerialize, BorshStorageKey)]
#[borsh(crate = "near_sdk::borsh")]
enum StorageKey {
    PendingRequests,
}

/// Code source specification
#[derive(Clone, Debug)]
#[near(serializers = [borsh, json])]
pub struct CodeSource {
    pub repo: String,
    pub commit: String,
    pub build_target: Option<String>, // e.g., "wasm32-wasi"
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
    pub code_source: CodeSource,
    pub resource_limits: ResourceLimits,
    pub payment: Balance,
    pub timestamp: u64,
    pub encrypted_secrets: Option<Vec<u8>>, // Secrets encrypted with keystore pubkey
    pub response_format: ResponseFormat,
}

/// Execution output - can be bytes, text, or parsed JSON
#[derive(Clone, Debug)]
#[near(serializers = [json])]
pub enum ExecutionOutput {
    Bytes(Vec<u8>),
    Text(String),
    Json(serde_json::Value),
}

/// Execution response from worker
#[derive(Clone, Debug)]
#[near(serializers = [json])]
pub struct ExecutionResponse {
    pub success: bool,
    pub output: Option<ExecutionOutput>,
    pub error: Option<String>,
    pub resources_used: ResourceMetrics,
}

/// Resource usage metrics
#[derive(Clone, Debug)]
#[near(serializers = [json])]
pub struct ResourceMetrics {
    pub instructions: u64,
    pub time_ms: u64,
}

#[derive(BorshDeserialize, BorshSerialize, PanicOnDefault)]
#[borsh(crate = "near_sdk::borsh")]
#[near_bindgen]
pub struct Contract {
    // Contract configuration
    owner_id: AccountId,
    operator_id: AccountId,
    paused: bool,

    // Pricing
    base_fee: Balance,
    per_instruction_fee: Balance,
    per_ms_fee: Balance,

    // Request tracking
    next_request_id: u64,
    pending_requests: LookupMap<u64, ExecutionRequest>,

    // Statistics
    total_executions: u64,
    total_fees_collected: Balance,

    // Keystore integration
    keystore_account_id: Option<AccountId>,
    keystore_pubkey: Option<String>, // hex encoded public key
}

#[near_bindgen]
impl Contract {
    #[init]
    pub fn new(owner_id: AccountId, operator_id: Option<AccountId>) -> Self {
        Self {
            owner_id: owner_id.clone(),
            operator_id: operator_id.unwrap_or(owner_id),
            paused: false,
            base_fee: 10_000_000_000_000_000_000_000, // 0.01 NEAR
            per_instruction_fee: 1_000_000_000_000_000, // 0.000001 NEAR per million instructions
            per_ms_fee: 1_000_000_000_000_000_000, // 0.000001 NEAR per millisecond
            next_request_id: 0,
            pending_requests: LookupMap::new(StorageKey::PendingRequests),
            total_executions: 0,
            total_fees_collected: 0,
            keystore_account_id: None,
            keystore_pubkey: None,
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
            (metrics.instructions / 1_000_000) as u128 * self.per_instruction_fee;
        let time_cost = metrics.time_ms as u128 * self.per_ms_fee;

        self.base_fee + instruction_cost + time_cost
    }

    /// Estimate cost based on resource limits
    fn estimate_cost(&self, limits: &ResourceLimits) -> Balance {
        // Use requested limits or defaults
        let max_instructions = limits.max_instructions.unwrap_or(1_000_000_000);
        let max_execution_seconds = limits.max_execution_seconds.unwrap_or(60);
        let max_time_ms = max_execution_seconds * 1000;

        // Calculate worst-case cost
        let instruction_cost = (max_instructions / 1_000_000) as u128 * self.per_instruction_fee;
        let time_cost = max_time_ms as u128 * self.per_ms_fee;

        self.base_fee + instruction_cost + time_cost
    }
}

#[cfg(test)]
mod tests;
