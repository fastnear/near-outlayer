//! Contract migration module
//!
//! Migration history (each `migrate()` is single-use; once run, the
//! state shape advances and prior migrate paths cannot be re-applied):
//!
//! * v4 → v5: rename `per_ms_fee_usd` → `per_sec_fee_usd`. (Run.)
//! * v5 → v6: add `wallet_policies`, `wallet_owner_index`. (Run.)
//! * **v6 → v7 (current): add `secret_vault_bindings` (Phase 2 of
//!   per-vault master plan).**
//!
//! Versions ≤ v6 are now historical. The `migrate()` entry point in
//! this file targets v6 → v7 specifically. Production deployments must
//! be on v6 before calling this migration; an earlier-version
//! deployment must first run a v4/v5 → v6 migration from a prior code
//! revision.

use crate::*;
use near_sdk::borsh::BorshDeserialize;
use near_sdk::collections::{LookupMap, UnorderedMap, UnorderedSet};

/// Pre-Phase-2 contract state (v6). Mirrors the `Contract` struct as it
/// existed immediately before the per-vault master Phase 2 changes
/// added `secret_vault_bindings`. All other fields carry over verbatim.
#[derive(BorshDeserialize)]
#[borsh(crate = "near_sdk::borsh")]
#[allow(dead_code)] // fields needed for borsh deserialisation only
pub struct ContractV6 {
    owner_id: AccountId,
    operator_id: AccountId,
    paused: bool,
    event_standard: String,
    event_version: String,

    // NEAR pricing
    base_fee: Balance,
    per_million_instructions_fee: Balance,
    per_ms_fee: Balance,
    per_compile_ms_fee: Balance,

    // USD pricing
    base_fee_usd: u128,
    per_million_instructions_fee_usd: u128,
    per_sec_fee_usd: u128,
    per_compile_ms_fee_usd: u128,

    payment_token_contract: Option<AccountId>,

    next_request_id: u64,
    pending_requests: LookupMap<u64, ExecutionRequest>,

    total_executions: u64,
    total_fees_collected: Balance,

    secrets_storage: LookupMap<SecretKey, SecretProfile>,
    user_secrets_index: LookupMap<AccountId, UnorderedSet<SecretKey>>,

    projects: LookupMap<String, Project>,
    project_versions: LookupMap<String, UnorderedMap<String, VersionInfo>>,
    user_projects_index: LookupMap<AccountId, UnorderedSet<String>>,
    next_project_id: u64,

    developer_earnings: LookupMap<AccountId, u128>,
    user_stablecoin_balances: LookupMap<AccountId, u128>,

    wallet_policies: LookupMap<String, wallet::WalletPolicyEntry>,
    wallet_owner_index: LookupMap<AccountId, UnorderedSet<String>>,
}

#[near_bindgen]
impl Contract {
    /// Migrate from v6 (pre-Phase-2) to v7 (per-vault master Phase 2).
    ///
    /// Adds `secret_vault_bindings` as an empty `LookupMap`. All
    /// existing `SecretProfile` entries deserialise unchanged — old
    /// secrets without a binding are interpreted by the worker as
    /// "encrypted with the default OutLayer master". Customers
    /// migrating to per-vault masters re-encrypt and rebind via
    /// `update_user_secrets` at their own pace.
    #[private]
    #[init(ignore_state)]
    pub fn migrate() -> Self {
        let v6: ContractV6 = env::state_read().expect("failed to read v6 state");

        log!(
            "Migrating contract v6 -> v7 (add secret_vault_bindings): owner={}, total_executions={}",
            v6.owner_id,
            v6.total_executions
        );

        Self {
            owner_id: v6.owner_id,
            operator_id: v6.operator_id,
            paused: v6.paused,
            event_standard: v6.event_standard,
            event_version: v6.event_version,
            base_fee: v6.base_fee,
            per_million_instructions_fee: v6.per_million_instructions_fee,
            per_ms_fee: v6.per_ms_fee,
            per_compile_ms_fee: v6.per_compile_ms_fee,
            base_fee_usd: v6.base_fee_usd,
            per_million_instructions_fee_usd: v6.per_million_instructions_fee_usd,
            per_sec_fee_usd: v6.per_sec_fee_usd,
            per_compile_ms_fee_usd: v6.per_compile_ms_fee_usd,
            payment_token_contract: v6.payment_token_contract,
            next_request_id: v6.next_request_id,
            pending_requests: v6.pending_requests,
            total_executions: v6.total_executions,
            total_fees_collected: v6.total_fees_collected,
            secrets_storage: v6.secrets_storage,
            user_secrets_index: v6.user_secrets_index,
            projects: v6.projects,
            project_versions: v6.project_versions,
            user_projects_index: v6.user_projects_index,
            next_project_id: v6.next_project_id,
            developer_earnings: v6.developer_earnings,
            user_stablecoin_balances: v6.user_stablecoin_balances,
            wallet_policies: v6.wallet_policies,
            wallet_owner_index: v6.wallet_owner_index,
            // ----- v7 -----
            secret_vault_bindings: LookupMap::new(StorageKey::SecretVaultBindings),
        }
    }

    /// Returns the contract's storage-schema version. Bumped each time
    /// `migrate()` advances the layout. Off-chain tooling reads this to
    /// decide whether a deploy needs a migration call.
    pub fn get_storage_version(&self) -> String {
        "7".to_string()
    }
}
