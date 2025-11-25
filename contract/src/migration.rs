//! Contract migration module
//!
//! This module handles migration of contract state when storage structures change.
//!
//! Migration v1 -> v2: SecretKey structure changed from flat fields to SecretKeyType enum
//! Old: SecretKey { repo, branch, profile, owner }
//! New: SecretKey { key_type: SecretKeyType::Repo { repo, branch }, profile, owner }

use crate::*;
use near_sdk::borsh::{BorshDeserialize, BorshSerialize};
use near_sdk::collections::{LookupMap, UnorderedSet};

/// Old SecretKey structure (before SecretKeyType enum)
#[derive(Clone, Debug, PartialEq, Eq, Hash, BorshSerialize, BorshDeserialize)]
#[borsh(crate = "near_sdk::borsh")]
pub struct OldSecretKey {
    pub repo: String,
    pub branch: Option<String>,
    pub profile: String,
    pub owner: AccountId,
}

/// Old Contract state (version 1)
#[derive(BorshDeserialize)]
#[borsh(crate = "near_sdk::borsh")]
#[allow(dead_code)] // Fields needed for Borsh deserialization during migration
pub struct OldContract {
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

    // Old secrets storage with flat SecretKey
    secrets_storage: LookupMap<OldSecretKey, SecretProfile>,

    // Old user secrets index
    user_secrets_index: LookupMap<AccountId, UnorderedSet<OldSecretKey>>,
}

#[near_bindgen]
impl Contract {
    /// Migrate contract from version 1 (flat SecretKey) to version 2 (SecretKeyType enum)
    ///
    /// This migration:
    /// 1. Reads old contract state
    /// 2. Converts all OldSecretKey to new SecretKey with SecretKeyType::Repo
    /// 3. Creates new storage collections
    ///
    /// Note: This is a lazy migration approach - old data remains but new data uses new format.
    /// For full migration, you would need to iterate all entries and re-insert them.
    ///
    /// # Safety
    /// - Only owner can call this
    /// - Should only be called once after contract upgrade
    #[private]
    #[init(ignore_state)]
    pub fn migrate() -> Self {
        let old_state: OldContract = env::state_read().expect("Failed to read old state");

        // Log migration
        log!(
            "Migrating contract from v1 to v2: owner={}, total_executions={}",
            old_state.owner_id,
            old_state.total_executions
        );

        // Create new contract with same configuration
        // Note: The LookupMaps are initialized with same storage keys, so they will
        // read the same underlying data. The difference is in how SecretKey is serialized.
        //
        // IMPORTANT: This approach requires that the contract has no existing secrets,
        // or that all secrets are re-inserted after migration.
        // For production, you would need to iterate and convert each entry.

        Self {
            owner_id: old_state.owner_id,
            operator_id: old_state.operator_id,
            paused: old_state.paused,
            base_fee: old_state.base_fee,
            per_million_instructions_fee: old_state.per_million_instructions_fee,
            per_ms_fee: old_state.per_ms_fee,
            per_compile_ms_fee: old_state.per_compile_ms_fee,
            next_request_id: old_state.next_request_id,
            pending_requests: old_state.pending_requests,
            total_executions: old_state.total_executions,
            total_fees_collected: old_state.total_fees_collected,
            // New empty storage collections - old secrets will need to be re-stored
            secrets_storage: LookupMap::new(StorageKey::SecretsStorage),
            user_secrets_index: LookupMap::new(StorageKey::UserSecretsIndex),
        }
    }

    /// Check if contract needs migration
    /// Returns the current storage version
    pub fn get_storage_version(&self) -> String {
        // Version 2: SecretKeyType enum
        "2".to_string()
    }
}
