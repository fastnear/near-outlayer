use crate::*;
use near_sdk::borsh::BorshDeserialize;
use near_sdk::collections::{LookupMap, UnorderedSet};

// ============================================================
// V0 → V1 (legacy: rtmr3 → full ApprovedMeasurements)
// ============================================================
//
// Kept in this file as historical reference — the testnet deployment
// already migrated through this path. Mainnet has not deployed V0, so
// V0 → V1 is dead-code-ish but cheap to retain.

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

// ============================================================
// V1 → V2 (vault registry — cessation, vault-code whitelist,
// verified/banned vaults)
// ============================================================
//
// V1 is the shape that mainnet (when deployed) and testnet hold today.
// V2 adds the vault-registry surface required by vault contracts
// (cross-contract `is_ceased` / `is_keystore_approved`) and the
// vault-checker (`mark_vault_verified` / `is_vault_verified`).

#[derive(BorshDeserialize)]
#[borsh(crate = "near_sdk::borsh")]
#[allow(dead_code)] // retained for archaeological reference; V1 → V2 already migrated on-chain
pub struct KeystoreDaoV1 {
    pub dao_members: UnorderedSet<AccountId>,
    pub approval_threshold: u32,
    pub owner_id: AccountId,
    pub init_account_id: AccountId,
    pub mpc_contract_id: AccountId,
    pub proposals: LookupMap<u64, KeystoreProposal>,
    pub next_proposal_id: u64,
    pub votes: LookupMap<(u64, AccountId), bool>,
    pub approved_keystores: UnorderedSet<PublicKey>,
    pub approved_measurements: Vec<ApprovedMeasurements>,
    pub quote_collateral: Option<String>,
}

// ============================================================
// V2 → V3 (multi-collateral / FMSPC-match)
// ============================================================
//
// V2 is the shape currently live on `dao.outlayer.{testnet,near}`: the
// full vault-registry struct (cessation + vault-code whitelist +
// verified/banned + vault-version multisig) carrying a SINGLE
// `quote_collateral: Option<String>`. This `migrate()` replaces that
// single Option with a multi-slot `collaterals: Vec<String>` (one slot
// per platform/FMSPC) so a mixed Phala + self-hosted fleet can register.
//
// Field order MUST match the current on-chain serialized layout exactly
// (same as the `KeystoreDao` struct in lib.rs, with `quote_collateral`
// in place of `collaterals`). Every other field is carried through
// verbatim — the vault registry is already on-chain and is NOT
// re-seeded here.

#[derive(BorshDeserialize)]
#[borsh(crate = "near_sdk::borsh")]
pub struct KeystoreDaoV2 {
    pub dao_members: UnorderedSet<AccountId>,
    pub approval_threshold: u32,
    pub owner_id: AccountId,
    pub init_account_id: AccountId,
    pub mpc_contract_id: AccountId,
    pub proposals: LookupMap<u64, KeystoreProposal>,
    pub next_proposal_id: u64,
    pub votes: LookupMap<(u64, AccountId), bool>,
    pub approved_keystores: UnorderedSet<PublicKey>,
    pub approved_measurements: Vec<ApprovedMeasurements>,
    /// Old single-collateral slot — migrated into `collaterals[0]`.
    pub quote_collateral: Option<String>,
    // ----- v2: vault registry -----
    pub ceased_operations: bool,
    pub approved_vault_code_hashes: UnorderedSet<Base58CryptoHash>,
    pub vault_versions: LookupMap<Base58CryptoHash, VaultVersionInfo>,
    pub verified_vaults: UnorderedSet<AccountId>,
    pub banned_vaults: UnorderedSet<AccountId>,
    // ----- v3: vault-version multisig -----
    pub vault_version_votes: LookupMap<VaultVersionAction, Vec<AccountId>>,
    pub vault_version_approval_args: LookupMap<Base58CryptoHash, ApprovalArgs>,
}

#[near_bindgen]
impl KeystoreDao {
    /// Migrate the live (vault-registry) state to the multi-collateral
    /// layout: `quote_collateral: Option<String>` → `collaterals:
    /// Vec<String>`. An existing `Some(c)` (e.g. the Phala 20a06f000000
    /// collateral) is carried into slot 0; `None` becomes an empty vec.
    /// The owner then adds the self-hosted FMSPC via
    /// `update_collateral(collateral, 1)`. All vault-registry fields are
    /// preserved verbatim.
    ///
    /// V0 → V1 and V1 → V2 are no longer reachable from this method;
    /// `dao.outlayer.{testnet,near}` already migrated through those and
    /// the old structs above remain only for archaeological reference.
    #[private]
    #[init(ignore_state)]
    pub fn migrate() -> Self {
        let old: KeystoreDaoV2 = env::state_read().expect("failed to read V2 state");

        Self {
            dao_members: old.dao_members,
            approval_threshold: old.approval_threshold,
            owner_id: old.owner_id,
            init_account_id: old.init_account_id,
            mpc_contract_id: old.mpc_contract_id,
            proposals: old.proposals,
            next_proposal_id: old.next_proposal_id,
            votes: old.votes,
            approved_keystores: old.approved_keystores,
            approved_measurements: old.approved_measurements,
            // Move the single cached collateral into slot 0; owner adds
            // others (self-hosted FMSPC) via `update_collateral(c, 1)`.
            collaterals: old.quote_collateral.map(|c| vec![c]).unwrap_or_default(),
            // ----- v2: carried through verbatim -----
            ceased_operations: old.ceased_operations,
            approved_vault_code_hashes: old.approved_vault_code_hashes,
            vault_versions: old.vault_versions,
            verified_vaults: old.verified_vaults,
            banned_vaults: old.banned_vaults,
            // ----- v3: carried through verbatim -----
            vault_version_votes: old.vault_version_votes,
            vault_version_approval_args: old.vault_version_approval_args,
        }
    }
}
