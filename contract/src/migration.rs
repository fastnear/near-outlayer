//! Contract migration module
//!
//! This module handles migration of contract state when storage structures change.
//!
//! Migration v2 -> v3: Added Project system
//! New fields: projects, project_versions, user_projects_index, next_project_id

use crate::*;
use near_sdk::borsh::BorshDeserialize;
use near_sdk::collections::{LookupMap, UnorderedSet};

/// Contract state version 2 (before Project system)
#[derive(BorshDeserialize)]
#[borsh(crate = "near_sdk::borsh")]
#[allow(dead_code)] // Fields needed for Borsh deserialization during migration
pub struct ContractV2 {
    // Contract configuration
    owner_id: AccountId,
    operator_id: AccountId,
    paused: bool,

    // Pricing
    base_fee: Balance,
    per_million_instructions_fee: Balance,
    per_ms_fee: Balance,
    per_compile_ms_fee: Balance,

    // Request tracking
    next_request_id: u64,
    pending_requests: LookupMap<u64, ExecutionRequest>,

    // Statistics
    total_executions: u64,
    total_fees_collected: Balance,

    // Secrets storage
    secrets_storage: LookupMap<SecretKey, SecretProfile>,

    // User secrets index
    user_secrets_index: LookupMap<AccountId, UnorderedSet<SecretKey>>,
}

#[near_bindgen]
impl Contract {
    /// Migrate contract from version 2 to version 3 (add Project system)
    ///
    /// This migration:
    /// 1. Reads old contract state (v2)
    /// 2. Preserves all existing data
    /// 3. Adds new Project storage collections
    ///
    /// # Safety
    /// - Only owner can call this
    /// - Should only be called once after contract upgrade
    #[private]
    #[init(ignore_state)]
    pub fn migrate() -> Self {
        let old_state: ContractV2 = env::state_read().expect("Failed to read old state");

        // Log migration
        log!(
            "Migrating contract v2 -> v3 (Project system): owner={}, total_executions={}",
            old_state.owner_id,
            old_state.total_executions
        );

        Self {
            owner_id: old_state.owner_id,
            operator_id: old_state.operator_id,
            paused: old_state.paused,
            event_standard: "near-outlayer".to_string(),
            event_version: "1.0.0".to_string(),
            // NEAR pricing
            base_fee: old_state.base_fee,
            per_million_instructions_fee: old_state.per_million_instructions_fee,
            per_ms_fee: old_state.per_ms_fee,
            per_compile_ms_fee: old_state.per_compile_ms_fee,
            // USD pricing (defaults for HTTPS API)
            base_fee_usd: 10_000,
            per_million_instructions_fee_usd: 1,
            per_ms_fee_usd: 10,
            per_compile_ms_fee_usd: 10,
            payment_token_contract: None,
            next_request_id: old_state.next_request_id,
            pending_requests: old_state.pending_requests,
            total_executions: old_state.total_executions,
            total_fees_collected: old_state.total_fees_collected,
            // Preserve existing secrets storage
            secrets_storage: old_state.secrets_storage,
            user_secrets_index: old_state.user_secrets_index,
            // New Project system collections
            projects: LookupMap::new(StorageKey::Projects),
            project_versions: LookupMap::new(b"pv".to_vec()),
            user_projects_index: LookupMap::new(StorageKey::UserProjects),
            next_project_id: 0,
        }
    }

    /// Check if contract needs migration
    /// Returns the current storage version
    pub fn get_storage_version(&self) -> String {
        // Version 3: Project system added
        "3".to_string()
    }
}
