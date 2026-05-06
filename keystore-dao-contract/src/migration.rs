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

#[near_bindgen]
impl KeystoreDao {
    /// Migrate from V1 to V2 (adds vault registry).
    ///
    /// V0 → V1 is no longer reachable from this method; the testnet
    /// deployment already migrated and the V0 struct above remains
    /// only for archaeological reference.
    ///
    /// V2 fields are initialized empty by default. To avoid the
    /// deploy-time race window (vault-checker requires the v1.0 vault
    /// hash to be approved before it can verify any vault), an
    /// operator MAY pass `initial_vault_hash` + `initial_vault_label`
    /// + `initial_vault_audit_url` to seed the whitelist atomically
    /// with the migration. This collapses the original "deploy +
    /// follow-up DAO tx" runbook into one transaction.
    ///
    /// All three optional args must be provided together; `migrate()`
    /// panics if exactly one or two are supplied.
    #[private]
    #[init(ignore_state)]
    pub fn migrate(
        initial_vault_hash: Option<Base58CryptoHash>,
        initial_vault_label: Option<String>,
        initial_vault_audit_url: Option<String>,
    ) -> Self {
        let v1: KeystoreDaoV1 = env::state_read().expect("failed to read V1 state");

        let mut approved_vault_code_hashes =
            UnorderedSet::new(StorageKey::ApprovedVaultCodeHashes);
        let mut vault_versions = LookupMap::new(StorageKey::VaultVersions);

        match (initial_vault_hash, initial_vault_label, initial_vault_audit_url) {
            (Some(hash), Some(label), Some(audit_url)) => {
                assert!(label.len() <= 64, "label must be at most 64 bytes");
                assert!(audit_url.len() <= 256, "audit_url must be at most 256 bytes");
                approved_vault_code_hashes.insert(&hash);
                vault_versions.insert(
                    &hash,
                    &VaultVersionInfo {
                        label,
                        // Seeded audit_url is always Some here.
                        // Operators who want a seeded vault without
                        // an audit URL should pass all migrate args
                        // as None and call approve_vault_version
                        // separately with `audit_url = None`.
                        audit_url: Some(audit_url),
                        deprecated: false,
                        approved_at: env::block_timestamp(),
                    },
                );
                env::log_str(&format!(
                    "vault_version_approved {} (seeded by migrate)",
                    String::from(&hash)
                ));
            }
            (None, None, None) => {
                env::log_str(
                    "migrate: vault registry initialised empty; \
                     remember to call approve_vault_version(...) \
                     before any vault deploys",
                );
            }
            _ => env::panic_str(
                "initial_vault_hash, initial_vault_label, and initial_vault_audit_url \
                 must all be Some(_) together, or all be None",
            ),
        }

        Self {
            dao_members: v1.dao_members,
            approval_threshold: v1.approval_threshold,
            owner_id: v1.owner_id,
            init_account_id: v1.init_account_id,
            mpc_contract_id: v1.mpc_contract_id,
            proposals: v1.proposals,
            next_proposal_id: v1.next_proposal_id,
            votes: v1.votes,
            approved_keystores: v1.approved_keystores,
            approved_measurements: v1.approved_measurements,
            quote_collateral: v1.quote_collateral,
            // ----- v2 -----
            ceased_operations: false,
            approved_vault_code_hashes,
            vault_versions,
            verified_vaults: UnorderedSet::new(StorageKey::VerifiedVaults),
            banned_vaults: UnorderedSet::new(StorageKey::BannedVaults),
            // ----- v3 (vault-version multisig) -----
            //
            // Initialised empty — there can never be a pending vote
            // before these methods are callable, since
            // `approve_vault_version` / `revoke_vault_version` are
            // introduced in the same deploy that ships this migration.
            // Existing testnet state has no votes to migrate.
            //
            // `vault_version_approval_args` locks the first proposer's
            // metadata so quorum can't be bricked by minor
            // `(label, audit_url)` typos from later voters.
            vault_version_votes: LookupMap::new(StorageKey::VaultVersionVotes),
            vault_version_approval_args: LookupMap::new(
                StorageKey::VaultVersionApprovalArgs,
            ),
        }
    }
}
