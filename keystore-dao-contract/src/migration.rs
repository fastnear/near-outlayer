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
#[allow(dead_code)] // retained for archaeological reference; V2 → V3 already migrated on-chain
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

// ============================================================
// V3 → V4 (keystore-key revocation proposals)
// ============================================================
//
// V3 is the shape live on `dao.outlayer.{testnet,near}` today — the multi-collateral struct
// produced by the previous migration. V4 appends `revoke_proposals`, the map behind
// `propose_revoke_keystore_keys` / `vote_revoke_keystore_keys`.
//
// Field order MUST match the current on-chain layout exactly: the `KeystoreDao` struct in
// lib.rs minus the new trailing field.

#[derive(BorshDeserialize)]
#[borsh(crate = "near_sdk::borsh")]
pub struct KeystoreDaoV3 {
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
    pub collaterals: Vec<String>,
    pub ceased_operations: bool,
    pub approved_vault_code_hashes: UnorderedSet<Base58CryptoHash>,
    pub vault_versions: LookupMap<Base58CryptoHash, VaultVersionInfo>,
    pub verified_vaults: UnorderedSet<AccountId>,
    pub banned_vaults: UnorderedSet<AccountId>,
    pub vault_version_votes: LookupMap<VaultVersionAction, Vec<AccountId>>,
    pub vault_version_approval_args: LookupMap<Base58CryptoHash, ApprovalArgs>,
}

#[near_bindgen]
impl KeystoreDao {
    /// Migrate V3 (multi-collateral) → V4: adds the empty `revoke_proposals` map. Every other
    /// field is carried through verbatim; no existing value is rewritten.
    ///
    /// **Run once, right after deploying the V4 wasm** — until it runs, every method panics on
    /// state deserialization.
    ///
    /// Earlier migrations (V0→V1, V1→V2, V2→V3) are no longer reachable from this method;
    /// `dao.outlayer.{testnet,near}` already went through them and the structs above remain
    /// only for archaeological reference.
    #[private]
    #[init(ignore_state)]
    pub fn migrate() -> Self {
        let old: KeystoreDaoV3 = env::state_read().expect("failed to read V3 state");

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
            collaterals: old.collaterals,
            ceased_operations: old.ceased_operations,
            approved_vault_code_hashes: old.approved_vault_code_hashes,
            vault_versions: old.vault_versions,
            verified_vaults: old.verified_vaults,
            banned_vaults: old.banned_vaults,
            vault_version_votes: old.vault_version_votes,
            vault_version_approval_args: old.vault_version_approval_args,
            // ----- v4 -----
            revoke_proposals: LookupMap::new(StorageKey::RevokeProposals),
        }
    }
}
