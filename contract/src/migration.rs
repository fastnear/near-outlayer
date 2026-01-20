//! Contract migration module
//!
//! This module handles migration of contract state when storage structures change.
//!
//! Migration v3 -> v4: Added Stablecoin Developer Payments
//! New fields:
//! - developer_earnings: LookupMap<AccountId, u128> - stablecoin earnings for project owners
//! - user_stablecoin_balances: LookupMap<AccountId, u128> - user deposits for attached_usd

use crate::*;
use near_sdk::borsh::BorshDeserialize;
use near_sdk::collections::{LookupMap, UnorderedMap, UnorderedSet};

/// Contract state version 3 (before Developer Earnings)
#[derive(BorshDeserialize)]
#[borsh(crate = "near_sdk::borsh")]
#[allow(dead_code)] // Fields needed for Borsh deserialization during migration
pub struct ContractV3 {
    // Contract configuration
    owner_id: AccountId,
    operator_id: AccountId,
    paused: bool,

    // Event metadata
    event_standard: String,
    event_version: String,

    // Pricing (NEAR)
    base_fee: Balance,
    per_million_instructions_fee: Balance,
    per_ms_fee: Balance,
    per_compile_ms_fee: Balance,

    // Pricing (USD)
    base_fee_usd: u128,
    per_million_instructions_fee_usd: u128,
    per_ms_fee_usd: u128,
    per_compile_ms_fee_usd: u128,

    // Payment token
    payment_token_contract: Option<AccountId>,

    // Request tracking
    next_request_id: u64,
    pending_requests: LookupMap<u64, ExecutionRequestV3>,

    // Statistics
    total_executions: u64,
    total_fees_collected: Balance,

    // Secrets storage
    secrets_storage: LookupMap<SecretKey, SecretProfile>,
    user_secrets_index: LookupMap<AccountId, UnorderedSet<SecretKey>>,

    // Project system
    projects: LookupMap<String, Project>,
    project_versions: LookupMap<String, UnorderedMap<String, VersionInfo>>,
    user_projects_index: LookupMap<AccountId, UnorderedSet<String>>,
    next_project_id: u64,
}

/// ExecutionRequest v3 (before attached_deposit field)
#[derive(Clone, Debug)]
#[near(serializers = [borsh])]
#[allow(dead_code)]
pub struct ExecutionRequestV3 {
    pub request_id: u64,
    pub data_id: CryptoHash,
    pub sender_id: AccountId,
    pub execution_source: ExecutionSource,
    pub resolved_source: CodeSource,
    pub resource_limits: ResourceLimits,
    pub payment: Balance,
    pub timestamp: u64,
    pub secrets_ref: Option<SecretsReference>,
    pub response_format: ResponseFormat,
    pub input_data: Option<String>,
    pub payer_account_id: AccountId,
    pub pending_output: Option<StoredOutput>,
    pub output_submitted: bool,
}

#[near_bindgen]
impl Contract {
    /// Migrate contract from version 3 to version 4 (add Stablecoin Developer Payments)
    ///
    /// This migration:
    /// 1. Reads old contract state (v3)
    /// 2. Preserves all existing data
    /// 3. Adds new developer_earnings storage (stablecoin-based)
    /// 4. Adds new user_stablecoin_balances storage
    ///
    /// Note: pending_requests with old ExecutionRequest format will be
    /// incompatible - ensure no pending requests exist before migration
    ///
    /// # Safety
    /// - Only owner can call this
    /// - Should only be called once after contract upgrade
    #[private]
    #[init(ignore_state)]
    pub fn migrate() -> Self {
        let old_state: ContractV3 = env::state_read().expect("Failed to read old state");

        // Log migration
        log!(
            "Migrating contract v3 -> v4 (Stablecoin Developer Payments): owner={}, total_executions={}",
            old_state.owner_id,
            old_state.total_executions
        );

        Self {
            owner_id: old_state.owner_id,
            operator_id: old_state.operator_id,
            paused: old_state.paused,
            event_standard: old_state.event_standard,
            event_version: old_state.event_version,
            // NEAR pricing
            base_fee: old_state.base_fee,
            per_million_instructions_fee: old_state.per_million_instructions_fee,
            per_ms_fee: old_state.per_ms_fee,
            per_compile_ms_fee: old_state.per_compile_ms_fee,
            // USD pricing
            base_fee_usd: old_state.base_fee_usd,
            per_million_instructions_fee_usd: old_state.per_million_instructions_fee_usd,
            per_ms_fee_usd: old_state.per_ms_fee_usd,
            per_compile_ms_fee_usd: old_state.per_compile_ms_fee_usd,
            payment_token_contract: old_state.payment_token_contract,
            next_request_id: old_state.next_request_id,
            // Note: pending_requests storage key is reused, but format changed
            // Old requests without attached_deposit field will fail to deserialize
            pending_requests: LookupMap::new(StorageKey::PendingRequests),
            total_executions: old_state.total_executions,
            total_fees_collected: old_state.total_fees_collected,
            secrets_storage: old_state.secrets_storage,
            user_secrets_index: old_state.user_secrets_index,
            // Project system
            projects: old_state.projects,
            project_versions: old_state.project_versions,
            user_projects_index: old_state.user_projects_index,
            next_project_id: old_state.next_project_id,
            // NEW: Stablecoin developer payments
            developer_earnings: LookupMap::new(StorageKey::DeveloperEarnings),
            user_stablecoin_balances: LookupMap::new(StorageKey::UserStablecoinBalances),
        }
    }

    /// Check if contract needs migration
    /// Returns the current storage version
    pub fn get_storage_version(&self) -> String {
        // Version 4: Stablecoin developer payments (developer_earnings, user_stablecoin_balances)
        "4".to_string()
    }
}
