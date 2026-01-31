//! Contract migration module
//!
//! This module handles migration of contract state when storage structures change.
//!
//! Migration v4 -> v5: Changed per_ms_fee_usd to per_sec_fee_usd
//! - Renamed field: per_ms_fee_usd -> per_sec_fee_usd
//! - USDC has 6 decimals, per_ms pricing was too expensive

use crate::*;
use near_sdk::borsh::BorshDeserialize;
use near_sdk::collections::{LookupMap, UnorderedMap, UnorderedSet};

/// Contract state version 4 (before per_sec_fee_usd change)
#[derive(BorshDeserialize)]
#[borsh(crate = "near_sdk::borsh")]
#[allow(dead_code)] // Fields needed for Borsh deserialization during migration
pub struct ContractV4 {
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

    // Pricing (USD) - old field name: per_ms_fee_usd
    base_fee_usd: u128,
    per_million_instructions_fee_usd: u128,
    per_ms_fee_usd: u128,
    per_compile_ms_fee_usd: u128,

    // Payment token
    payment_token_contract: Option<AccountId>,

    // Request tracking
    next_request_id: u64,
    pending_requests: LookupMap<u64, ExecutionRequest>,

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

    // Developer earnings (stablecoin)
    developer_earnings: LookupMap<AccountId, u128>,

    // User stablecoin balances
    user_stablecoin_balances: LookupMap<AccountId, u128>,
}

#[near_bindgen]
impl Contract {
    /// Migrate contract from version 4 to version 5 (per_ms_fee_usd -> per_sec_fee_usd)
    ///
    /// This migration:
    /// 1. Reads old contract state (v4)
    /// 2. Preserves all existing data
    /// 3. Changes per_ms_fee_usd to per_sec_fee_usd with value 1
    ///
    /// # Safety
    /// - Only owner can call this
    /// - Should only be called once after contract upgrade
    #[private]
    #[init(ignore_state)]
    pub fn migrate() -> Self {
        let old_state: ContractV4 = env::state_read().expect("Failed to read old state");

        // Log migration
        log!(
            "Migrating contract v4 -> v5 (per_ms_fee_usd -> per_sec_fee_usd): owner={}, total_executions={}",
            old_state.owner_id,
            old_state.total_executions
        );

        Self {
            owner_id: old_state.owner_id,
            operator_id: old_state.operator_id,
            paused: old_state.paused,
            event_standard: old_state.event_standard,
            event_version: old_state.event_version,
            // NEAR pricing (unchanged)
            base_fee: old_state.base_fee,
            per_million_instructions_fee: old_state.per_million_instructions_fee,
            per_ms_fee: old_state.per_ms_fee,
            per_compile_ms_fee: old_state.per_compile_ms_fee,
            // USD pricing (per_ms_fee_usd -> per_sec_fee_usd)
            base_fee_usd: old_state.base_fee_usd,
            per_million_instructions_fee_usd: old_state.per_million_instructions_fee_usd,
            per_sec_fee_usd: 1, // Set to 1 ($0.000001 per second)
            per_compile_ms_fee_usd: old_state.per_compile_ms_fee_usd,
            payment_token_contract: old_state.payment_token_contract,
            next_request_id: old_state.next_request_id,
            pending_requests: old_state.pending_requests,
            total_executions: old_state.total_executions,
            total_fees_collected: old_state.total_fees_collected,
            secrets_storage: old_state.secrets_storage,
            user_secrets_index: old_state.user_secrets_index,
            // Project system
            projects: old_state.projects,
            project_versions: old_state.project_versions,
            user_projects_index: old_state.user_projects_index,
            next_project_id: old_state.next_project_id,
            // Developer earnings
            developer_earnings: old_state.developer_earnings,
            user_stablecoin_balances: old_state.user_stablecoin_balances,
        }
    }

    /// Check if contract needs migration
    /// Returns the current storage version
    pub fn get_storage_version(&self) -> String {
        // Version 5: per_ms_fee_usd -> per_sec_fee_usd
        "5".to_string()
    }
}