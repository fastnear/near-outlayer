//! Contract migration module
//!
//! Migration history (each `migrate()` is single-use; once run, the
//! state shape advances and prior migrate paths cannot be re-applied):
//!
//! * v4 → v5: rename `per_ms_fee_usd` → `per_sec_fee_usd`. (Run.)
//! * v5 → v6: add `wallet_policies`, `wallet_owner_index`. (Run.)
//! * v6 → v7: add `secret_vault_bindings` (Phase 2 of per-vault master
//!   plan). (Run.)
//! * **v7 → v8 (current): add `subscription_plans` — what a
//!   subscription costs — AND `project_pricing` — what a curated
//!   project charges per operation, and whom to credit for it.**
//!
//! Two field sets, ONE migration, because they ship in one deploy and
//! are therefore one generation of the schema. There is no version
//! between them, and none should be written: `migrate()` must expect a
//! shape some chain actually holds.
//!
//! **`ContractV7` mirrors what is DEPLOYED, not this working tree minus
//! the new fields.** Check it against `git show HEAD:` before touching
//! it. The two are the same only when the previous migration is already
//! committed, and it is the DEPLOYED shape that `state_read` will be
//! handed.
//!
//! Versions ≤ v7 are historical. Production deployments must be on v7
//! before calling this migration; an earlier-version deployment must
//! first run the prior migrations from an earlier code revision.

use crate::*;
use near_sdk::borsh::BorshDeserialize;
use near_sdk::collections::{LookupMap, UnorderedMap, UnorderedSet};

/// Contract state as DEPLOYED at v7 — the last shape any chain has held.
///
/// Mirrors `Contract` as it was before either `subscription_plans` or
/// `project_pricing` existed; every field carries over verbatim. Check it
/// against `git show HEAD:contract/src/lib.rs`, not against the struct in this
/// working tree: the two differ by exactly the fields this migration adds.
///
/// A new field means a new state SHAPE, and borsh reads by position — so a
/// deploy without this migration would read the old bytes into the new struct
/// and run off the end. That is the whole reason this file exists.
#[derive(BorshDeserialize)]
#[borsh(crate = "near_sdk::borsh")]
#[allow(dead_code)] // fields needed for borsh deserialisation only
pub struct ContractV7 {
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

    secret_vault_bindings: LookupMap<SecretKey, AccountId>,
}

#[near_bindgen]
impl Contract {
    /// Migrate from v7 to v8: add the subscription price list and the
    /// per-project price table.
    ///
    /// Both start EMPTY, and empty is the safe direction for each. An empty
    /// plan list sells nothing, so a deployment whose operator has not set
    /// prices yet refuses purchases and hands the money back rather than
    /// granting a subscription at a price nobody chose. An empty price table
    /// prices nothing, so every project keeps behaving exactly as it does
    /// today — admission only applies to a project that has a price.
    ///
    /// Filled in afterwards with `set_subscription_plans` and
    /// `set_project_pricing`. Note what is NOT carried over: there is no old
    /// copy of the connector prices anywhere on chain to import. They live in
    /// the coordinator's `connector_operation_prices` today, and moving them is
    /// an operator step against the live deployment, not a state rewrite.
    #[private]
    #[init(ignore_state)]
    pub fn migrate() -> Self {
        let v7: ContractV7 = env::state_read().expect("failed to read v7 state");

        log!(
            "Migrating contract v7 -> v8 (subscription_plans + project_pricing): owner={}, total_executions={}",
            v7.owner_id,
            v7.total_executions
        );

        Self {
            owner_id: v7.owner_id,
            operator_id: v7.operator_id,
            paused: v7.paused,
            event_standard: v7.event_standard,
            event_version: v7.event_version,
            base_fee: v7.base_fee,
            per_million_instructions_fee: v7.per_million_instructions_fee,
            per_ms_fee: v7.per_ms_fee,
            per_compile_ms_fee: v7.per_compile_ms_fee,
            base_fee_usd: v7.base_fee_usd,
            per_million_instructions_fee_usd: v7.per_million_instructions_fee_usd,
            per_sec_fee_usd: v7.per_sec_fee_usd,
            per_compile_ms_fee_usd: v7.per_compile_ms_fee_usd,
            payment_token_contract: v7.payment_token_contract,
            next_request_id: v7.next_request_id,
            pending_requests: v7.pending_requests,
            total_executions: v7.total_executions,
            total_fees_collected: v7.total_fees_collected,
            secrets_storage: v7.secrets_storage,
            user_secrets_index: v7.user_secrets_index,
            projects: v7.projects,
            project_versions: v7.project_versions,
            user_projects_index: v7.user_projects_index,
            next_project_id: v7.next_project_id,
            developer_earnings: v7.developer_earnings,
            user_stablecoin_balances: v7.user_stablecoin_balances,
            wallet_policies: v7.wallet_policies,
            wallet_owner_index: v7.wallet_owner_index,
            secret_vault_bindings: v7.secret_vault_bindings,
            // ----- v8 -----
            subscription_plans: Vec::new(),
            project_pricing: UnorderedMap::new(StorageKey::ProjectPricing),
        }
    }

    /// Returns the contract's storage-schema version. Bumped each time
    /// `migrate()` advances the layout. Off-chain tooling reads this to
    /// decide whether a deploy needs a migration call.
    pub fn get_storage_version(&self) -> String {
        "8".to_string()
    }
}
