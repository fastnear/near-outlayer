use crate::*;
use near_sdk::borsh::BorshDeserialize;
use near_sdk::collections::{LookupMap, UnorderedSet};

/// Old KeystoreProposal with rtmr3: String (before full measurements)
#[derive(BorshDeserialize)]
#[borsh(crate = "near_sdk::borsh")]
#[allow(dead_code)]
pub struct KeystoreProposalV0 {
    pub id: u64,
    pub public_key: PublicKey,
    pub rtmr3: String,
    pub submitter: AccountId,
    pub created_at: u64,
    pub votes_for: u32,
    pub votes_against: u32,
    pub status: ProposalStatus,
}

#[derive(BorshDeserialize)]
#[borsh(crate = "near_sdk::borsh")]
#[allow(dead_code)]
pub struct KeystoreDaoV0 {
    pub dao_members: UnorderedSet<AccountId>,
    pub approval_threshold: u32,
    pub owner_id: AccountId,
    pub init_account_id: AccountId,
    pub mpc_contract_id: AccountId,
    pub proposals: LookupMap<u64, KeystoreProposalV0>,
    pub next_proposal_id: u64,
    pub votes: LookupMap<(u64, AccountId), bool>,
    pub approved_keystores: UnorderedSet<PublicKey>,
    pub approved_rtmr3: UnorderedSet<String>,
    pub quote_collateral: Option<String>,
}

#[near_bindgen]
impl KeystoreDao {
    /// Migrate from V0 (approved_rtmr3) to V1 (approved_measurements).
    ///
    /// Old approved_rtmr3 data is dropped — admin adds full measurements after migration.
    /// Old proposals with rtmr3-only schema become inaccessible (already executed/irrelevant).
    #[private]
    #[init(ignore_state)]
    pub fn migrate() -> Self {
        let old_state: KeystoreDaoV0 = env::state_read().expect("Failed to read old state");

        Self {
            dao_members: old_state.dao_members,
            approval_threshold: old_state.approval_threshold,
            owner_id: old_state.owner_id,
            init_account_id: old_state.init_account_id,
            mpc_contract_id: old_state.mpc_contract_id,
            // Reset proposals — old ones have incompatible schema (rtmr3 vs measurements)
            proposals: LookupMap::new(StorageKey::Proposals),
            next_proposal_id: old_state.next_proposal_id,
            votes: old_state.votes,
            approved_keystores: old_state.approved_keystores,
            approved_measurements: Vec::new(),
            quote_collateral: old_state.quote_collateral,
        }
    }
}
