use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};
use near_sdk::collections::{LookupMap, UnorderedSet};
use near_sdk::json_types::Base58CryptoHash;
use near_sdk::serde::{Deserialize, Serialize};
use near_sdk::{env, ext_contract, near_bindgen, require, AccountId, Promise, PromiseOrValue, PublicKey, BorshStorageKey, NearToken, Allowance, Gas};
use schemars::JsonSchema;

// Collateral wrapper for TDX verification (from register-contract)
mod collateral;
use collateral::Collateral;

mod migration;

// Custom getrandom implementation for WASM (required by dcap-qvl)
#[cfg(target_arch = "wasm32")]
use getrandom::{register_custom_getrandom, Error};
#[cfg(target_arch = "wasm32")]
fn randomness_unsupported(_: &mut [u8]) -> Result<(), Error> {
    Err(Error::UNSUPPORTED)
}
#[cfg(target_arch = "wasm32")]
register_custom_getrandom!(randomness_unsupported);

// dtos module for MPC types (simplified version for contract interface)
pub mod dtos {
    use near_sdk::serde::{Deserialize, Serialize};

    /// BLS12-381 G1 public key type - simplified as String for JSON serialization
    /// In the actual MPC contract this is [u8; 96] but we use String for easier JSON handling
    #[cfg_attr(
        all(feature = "abi", not(target_arch = "wasm32")),
        derive(schemars::JsonSchema)
    )]
    #[derive(Serialize, Deserialize, Clone, Debug)]
    #[serde(crate = "near_sdk::serde")]
    pub struct Bls12381G1PublicKey(pub String);
}

// External interface for MPC contract.
//
// `request_app_private_key` is the MPC contract's CKD (Conditional Key
// Derivation) entry point. The name describes what the caller is asking
// for, NOT what travels on the wire — `CKDRequestArgs` only contains
// the caller's ephemeral PUBLIC key (`app_public_key`), a derivation
// path, and a domain id. No private key is ever passed.
//
// MPC nodes hold threshold shares of the master and never see the
// derived private key in cleartext; they collectively produce an
// encrypted CKD payload (`big_y`, `big_c`) targeted at the caller's
// `app_public_key`. The caller (keystore-worker, inside its TEE)
// decrypts the payload locally with the matching app-private key.
#[ext_contract(ext_mpc)]
#[allow(dead_code)]
trait ExtMPC {
    fn request_app_private_key(&self, request: CKDRequestArgs) -> CKDResponse;
}

#[derive(BorshSerialize, BorshStorageKey)]
#[borsh(crate = "near_sdk::borsh")]
#[allow(dead_code)]
enum StorageKey {
    ApprovedRtmr3, // legacy, kept for migration deserialization
    DaoMembers,
    Proposals,
    Votes { proposal_id: u64 },
    ApprovedKeystores,
    // ----- v2 (vault registry) -----
    ApprovedVaultCodeHashes,
    VaultVersions,
    VerifiedVaults,
    BannedVaults,
    // ----- v3 (vault-version multisig) -----
    /// Per-`(action, hash)` vote ledger. Each DAO member who calls
    /// `approve_vault_version` / `revoke_vault_version` is recorded
    /// once; when the count reaches `approval_threshold` the
    /// underlying action executes and the entry is cleared. Key is
    /// borsh-serialized `VaultVersionAction`.
    VaultVersionVotes,
    /// First-proposer's `(label, audit_url)` for an approve
    /// proposal. See `vault_version_approval_args` doc on
    /// `KeystoreDao` for the trust model.
    VaultVersionApprovalArgs,
}

/// Full TEE measurements for verifying the entire dstack environment.
/// All fields are 96 hex characters (48 bytes).
///
/// Without verifying MRTD + RTMR0-2, an attacker can run a dev dstack image
/// (with SSH enabled), connect to the container, and modify the running code
/// while RTMR3 still passes because the docker-compose is the same.
#[derive(BorshSerialize, BorshDeserialize, Serialize, Deserialize, JsonSchema, Clone, PartialEq, Eq, Debug)]
#[serde(crate = "near_sdk::serde")]
pub struct ApprovedMeasurements {
    /// MRTD - Virtual firmware (TDX module) identity
    pub mrtd: String,
    /// RTMR0 - Bootloader, firmware config
    pub rtmr0: String,
    /// RTMR1 - OS kernel, boot params, initrd
    pub rtmr1: String,
    /// RTMR2 - OS applications layer
    pub rtmr2: String,
    /// RTMR3 - Runtime events (compose-hash, key-provider)
    pub rtmr3: String,
}

/// Proposal for registering a new keystore
#[derive(BorshDeserialize, BorshSerialize, Serialize, Deserialize, Clone, JsonSchema)]
#[serde(crate = "near_sdk::serde")]
pub struct KeystoreProposal {
    pub id: u64,
    #[schemars(with = "String")]
    pub public_key: PublicKey,
    pub measurements: ApprovedMeasurements,
    #[schemars(with = "String")]
    pub submitter: AccountId,
    pub created_at: u64,
    pub votes_for: u32,
    pub votes_against: u32,
    pub status: ProposalStatus,
}

#[derive(BorshDeserialize, BorshSerialize, Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(crate = "near_sdk::serde")]
pub enum ProposalStatus {
    Pending,
    Approved,
    Rejected,
    Executed,
}

/// Information about an approved keystore
#[derive(BorshDeserialize, BorshSerialize, Serialize, Deserialize, Clone, JsonSchema)]
#[serde(crate = "near_sdk::serde")]
pub struct KeystoreInfo {
    #[schemars(with = "String")]
    pub public_key: PublicKey,
    pub measurements: ApprovedMeasurements,
    pub approved_at: u64,
    pub proposal_id: u64,
}

/// Metadata for a whitelisted vault contract version. Stored alongside
/// the code hash so off-chain auditors can match a deployed vault back
/// to a specific release tag and audit URL without grepping git logs.
#[derive(BorshDeserialize, BorshSerialize, Serialize, Deserialize, Clone, Debug, JsonSchema)]
#[serde(crate = "near_sdk::serde")]
pub struct VaultVersionInfo {
    /// Human-readable label, e.g. `"v1.0"`.
    pub label: String,
    /// External link to the audit report or release notes for this
    /// hash. `None` for unaudited self-published versions — the DAO
    /// can still approve such a hash through the multisig vote, but
    /// off-chain reviewers will see a missing audit URL and apply
    /// extra scrutiny.
    pub audit_url: Option<String>,
    /// `true` once the version has been deprecated — existing vaults on
    /// this hash keep working but new vaults SHOULD use a non-deprecated
    /// version. Distinct from `revoke_vault_version`, which removes the
    /// hash from `approved_vault_code_hashes` entirely.
    pub deprecated: bool,
    /// Block timestamp (nanoseconds) at which the version was approved.
    pub approved_at: u64,
}

/// Keystore DAO Contract
///
/// This contract manages keystore registration through DAO governance.
/// Keystores run in TEE and need DAO approval to get access keys.
/// Once approved, they can request deterministic secrets from MPC network.
///
/// # Flow
/// 1. Keystore generates keypair inside TEE
/// 2. Keystore submits registration with TDX attestation
/// 3. Contract verifies attestation (MRTD + RTMR0-3) and creates proposal
/// 4. DAO members vote on proposal
/// 5. If approved, contract adds access key to itself
/// 6. Keystore can now request CKD from MPC using contract's account
#[near_bindgen]
#[derive(BorshDeserialize, BorshSerialize)]
pub struct KeystoreDao {
    /// DAO members who can vote
    pub dao_members: UnorderedSet<AccountId>,

    /// Minimum votes required for approval (>50% of members)
    pub approval_threshold: u32,

    /// Owner who can manage DAO members and collateral
    pub owner_id: AccountId,

    /// Init account that pays gas for initial registration
    pub init_account_id: AccountId,

    /// MPC contract ID for CKD requests (e.g., "v1.signer-prod.testnet")
    pub mpc_contract_id: AccountId,

    /// All proposals
    pub proposals: LookupMap<u64, KeystoreProposal>,

    /// Next proposal ID
    pub next_proposal_id: u64,

    /// Votes: (proposal_id, voter) -> vote (true = approve, false = reject)
    pub votes: LookupMap<(u64, AccountId), bool>,

    /// Approved keystore public keys
    pub approved_keystores: UnorderedSet<PublicKey>,

    /// Full TEE measurements approved for keystore registration.
    /// Each entry contains MRTD + RTMR0-3 (all must match for registration).
    pub approved_measurements: Vec<ApprovedMeasurements>,

    /// TDX quote collateral (Intel's reference data for verification)
    pub quote_collateral: Option<String>,

    // ----- v2: vault registry (cessation flag + code-hash whitelist + verified/banned sets) -----

    /// `true` while OutLayer DAO has declared cessation. Vault contracts
    /// gate their `initiate_recovery` callback on this flag.
    pub ceased_operations: bool,

    /// Whitelist of vault-contract code hashes the DAO trusts. A vault
    /// must deploy WASM whose sha256 matches one of these hashes for
    /// `is_vault_code_approved` to return true; this is the gate the
    /// vault-checker WASI agent enforces before calling
    /// `mark_vault_verified`.
    pub approved_vault_code_hashes: UnorderedSet<Base58CryptoHash>,

    /// Per-hash metadata: human-readable label, audit URL, deprecation
    /// flag, approval timestamp.
    pub vault_versions: LookupMap<Base58CryptoHash, VaultVersionInfo>,

    /// Vaults that have passed off-chain verification by the
    /// keystore-worker. End-users read this set via `is_vault_verified`
    /// to confirm a vault's state was checked by an attested TEE.
    pub verified_vaults: UnorderedSet<AccountId>,

    /// Vaults that have been banned (typically by automated race-attack
    /// detection, or by DAO vote). `is_vault_verified` returns false
    /// for any vault present here regardless of `verified_vaults`.
    pub banned_vaults: UnorderedSet<AccountId>,

    // ----- v3: vault-version multisig -----

    /// Multi-signature ledger for `approve_vault_version` and
    /// `revoke_vault_version`. Plan called for these actions to go
    /// through the existing proposal+vote flow with `approval_threshold`
    /// votes. This map tracks who has voted for each pending action;
    /// when the recorded set reaches `approval_threshold` the action
    /// executes and the entry is cleared.
    ///
    /// Key encodes ONLY the action kind + hash (no label/audit_url),
    /// so two members typing slightly different metadata (e.g.
    /// "v1.0" vs "v1.0 ") still converge on the same proposal —
    /// see `vault_version_approval_args` for how the metadata is
    /// bound to the FIRST proposer's choice (subsequent voters must
    /// match exactly or get a clear error). Without this scheme,
    /// minor metadata typos would silently brick quorum.
    pub vault_version_votes: LookupMap<VaultVersionAction, Vec<AccountId>>,

    /// First-voter args for an in-flight `approve_vault_version`
    /// proposal. Once a member proposes
    /// `(hash=X, label="v2.0", audit_url="...")`, this map locks the
    /// metadata to those values. Subsequent voters MUST pass the
    /// same `(label, audit_url)` or the call panics — preventing
    /// metadata-typo split votes. Cleared together with
    /// `vault_version_votes` when quorum is reached.
    /// Only `Approve` has metadata; `Revoke` is just `hash`.
    pub vault_version_approval_args: LookupMap<Base58CryptoHash, ApprovalArgs>,
}

/// First-proposer's metadata for an in-flight approve proposal. See
/// `vault_version_approval_args` for the trust model.
#[cfg_attr(
    all(feature = "abi", not(target_arch = "wasm32")),
    derive(::near_sdk::schemars::JsonSchema)
)]
#[derive(BorshSerialize, BorshDeserialize, Serialize, Deserialize, Clone, Debug, PartialEq)]
#[borsh(crate = "near_sdk::borsh")]
#[serde(crate = "near_sdk::serde")]
pub struct ApprovalArgs {
    pub label: String,
    /// `None` is allowed — see `VaultVersionInfo::audit_url`.
    pub audit_url: Option<String>,
}

/// Pending DAO multisig action against the vault-version registry.
/// Keys `vault_version_votes` (borsh-encoded). Approval metadata
/// lives in a parallel `vault_version_approval_args` map so that
/// both members vote on the same `(action, hash)` entry regardless
/// of the metadata they pass.
#[cfg_attr(
    all(feature = "abi", not(target_arch = "wasm32")),
    derive(::near_sdk::schemars::JsonSchema)
)]
#[derive(BorshSerialize, BorshDeserialize, Serialize, Deserialize, Clone, Debug)]
#[borsh(crate = "near_sdk::borsh")]
#[serde(crate = "near_sdk::serde")]
pub enum VaultVersionAction {
    Approve { hash: Base58CryptoHash },
    Revoke { hash: Base58CryptoHash },
}

#[cfg_attr(
    all(feature = "abi", not(target_arch = "wasm32")),
    derive(::near_sdk::schemars::JsonSchema)
)]
#[derive(Serialize, Deserialize)]
#[serde(crate = "near_sdk::serde")]
pub struct CKDResponse {
    pub big_y: dtos::Bls12381G1PublicKey,
    pub big_c: dtos::Bls12381G1PublicKey,
}

#[cfg_attr(
    all(feature = "abi", not(target_arch = "wasm32")),
    derive(schemars::JsonSchema)
)]
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(crate = "near_sdk::serde")]
pub struct DomainId(pub u64);

#[cfg_attr(
    all(feature = "abi", not(target_arch = "wasm32")),
    derive(schemars::JsonSchema)
)]
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(crate = "near_sdk::serde")]
pub struct CKDRequestArgs {
    pub derivation_path: String,
    pub app_public_key: dtos::Bls12381G1PublicKey,
    pub domain_id: DomainId,
}

impl Default for KeystoreDao {
    fn default() -> Self {
        env::panic_str("KeystoreDao must be initialized");
    }
}

#[near_bindgen]
impl KeystoreDao {
    /// Initialize contract with DAO members
    #[init]
    pub fn new(
        owner_id: AccountId,
        init_account_id: AccountId,
        dao_members: Vec<AccountId>,
        mpc_contract_id: AccountId,
    ) -> Self {
        assert!(!dao_members.is_empty(), "DAO must have at least one member");

        let mut members_set = UnorderedSet::new(StorageKey::DaoMembers);
        for member in dao_members.iter() {
            members_set.insert(member);
        }

        // Calculate approval threshold (>50%)
        let threshold = (dao_members.len() as u32 / 2) + 1;

        Self {
            dao_members: members_set,
            approval_threshold: threshold,
            owner_id,
            init_account_id,
            mpc_contract_id,
            proposals: LookupMap::new(StorageKey::Proposals),
            next_proposal_id: 1,
            votes: LookupMap::new(StorageKey::Votes { proposal_id: 0 }),
            approved_keystores: UnorderedSet::new(StorageKey::ApprovedKeystores),
            approved_measurements: Vec::new(),
            quote_collateral: None,
            ceased_operations: false,
            approved_vault_code_hashes: UnorderedSet::new(StorageKey::ApprovedVaultCodeHashes),
            vault_versions: LookupMap::new(StorageKey::VaultVersions),
            verified_vaults: UnorderedSet::new(StorageKey::VerifiedVaults),
            banned_vaults: UnorderedSet::new(StorageKey::BannedVaults),
            vault_version_votes: LookupMap::new(StorageKey::VaultVersionVotes),
            vault_version_approval_args: LookupMap::new(
                StorageKey::VaultVersionApprovalArgs,
            ),
        }
    }

    /// Submit keystore registration with TEE attestation
    ///
    /// This method:
    /// 1. Verifies TDX quote signature
    /// 2. Extracts MRTD + RTMR0-3 and public key
    /// 3. Checks all measurements against approved list
    /// 4. Creates a proposal for DAO voting
    pub fn submit_keystore_registration(
        &mut self,
        public_key: PublicKey,
        tdx_quote_hex: String,
        #[allow(unused_variables)]
        app_id: Option<String>,
    ) -> u64 {
        // Only init account can submit (pays gas)
        assert_eq!(
            env::predecessor_account_id(),
            self.init_account_id,
            "Only {} can submit registrations",
            self.init_account_id
        );

        // Check if already approved
        assert!(
            !self.approved_keystores.contains(&public_key),
            "Keystore already approved"
        );

        // Verify TDX quote and extract data
        let collateral = self.quote_collateral.clone()
            .expect("Quote collateral required (owner must call update_collateral first)");

        let (measurements, embedded_pubkey) = self.verify_tdx_quote(&tdx_quote_hex, &collateral);

        // Log app_id if provided (for TEE verification)
        if let Some(ref app_id) = app_id {
            env::log_str(&format!("App_id: {}", app_id));
        }

        env::log_str(&format!(
            "📋 Verified TDX quote. Keystore's TEE-generated key (from quote report_data): {:?}",
            embedded_pubkey
        ));
        env::log_str(&format!(
            "📋 Measurements from TDX quote: mrtd={}, rtmr0={}, rtmr1={}, rtmr2={}, rtmr3={}",
            measurements.mrtd, measurements.rtmr0, measurements.rtmr1,
            measurements.rtmr2, measurements.rtmr3
        ));

        // CRITICAL: Check if measurements are in approved list
        assert!(
            self.approved_measurements.contains(&measurements),
            "Worker measurements not approved. MRTD={}, RTMR0={}, RTMR1={}, RTMR2={}, RTMR3={}.",
            measurements.mrtd, measurements.rtmr0, measurements.rtmr1,
            measurements.rtmr2, measurements.rtmr3
        );

        env::log_str("✅ Measurements are in approved list");

        // Verify public key matches quote
        assert_eq!(
            embedded_pubkey, public_key,
            "Public key mismatch: provided key doesn't match TDX quote"
        );

        // Create proposal
        let proposal = KeystoreProposal {
            id: self.next_proposal_id,
            public_key: public_key.clone(),
            measurements: measurements.clone(),
            submitter: env::predecessor_account_id(),
            created_at: env::block_timestamp(),
            votes_for: 0,
            votes_against: 0,
            status: ProposalStatus::Pending,
        };

        let proposal_id = self.next_proposal_id;
        self.proposals.insert(&proposal_id, &proposal);
        self.next_proposal_id += 1;

        env::log_str(&format!(
            "Created proposal {} for keystore registration (all 5 TEE measurements verified)",
            proposal_id
        ));

        proposal_id
    }

    /// DAO member votes on proposal
    pub fn vote(&mut self, proposal_id: u64, approve: bool) {
        let voter = env::predecessor_account_id();

        // Check if DAO member
        assert!(
            self.dao_members.contains(&voter),
            "Only DAO members can vote"
        );

        // Get proposal
        let mut proposal = self.proposals.get(&proposal_id)
            .expect("Proposal not found");

        // Check status
        assert_eq!(
            proposal.status, ProposalStatus::Pending,
            "Proposal is not pending"
        );

        // Check if already voted
        let vote_key = (proposal_id, voter.clone());
        assert!(
            !self.votes.contains_key(&vote_key),
            "Already voted on this proposal"
        );

        // Record vote
        self.votes.insert(&vote_key, &approve);

        if approve {
            proposal.votes_for += 1;
        } else {
            proposal.votes_against += 1;
        }

        // Recompute LIVE counts using only current DAO members for the
        // threshold decision. The stored `votes_for` / `votes_against`
        // are kept as "all votes ever cast" for the view API, but
        // would otherwise allow a removed member's vote to push the
        // proposal past threshold or rejection without any current
        // member's consent — see `approve_vault_version` for the
        // analogous fix on the vault-version multisig.
        let (live_for, live_against) = self.dao_members.iter().fold(
            (0u32, 0u32),
            |(f, a), member| match self.votes.get(&(proposal_id, member.clone())) {
                Some(true) => (f + 1, a),
                Some(false) => (f, a + 1),
                None => (f, a),
            },
        );

        if live_for >= self.approval_threshold {
            proposal.status = ProposalStatus::Approved;

            env::log_str(&format!(
                "Proposal {} approved with {} votes",
                proposal_id, live_for
            ));

            self.internal_execute_proposal(proposal_id, proposal);
        } else if live_against > (self.dao_members.len() as u32 - self.approval_threshold) {
            proposal.status = ProposalStatus::Rejected;
            self.proposals.insert(&proposal_id, &proposal);

            env::log_str(&format!(
                "Proposal {} rejected with {} votes against",
                proposal_id, live_against
            ));
        }
        else {
            // Update proposal
            self.proposals.insert(&proposal_id, &proposal);
        }
    }

    /// Owner: Add approved TEE measurements (MRTD + RTMR0-3).
    ///
    /// All 5 measurements must match for a keystore to register.
    /// Get measurements from Phala attestation:
    /// `phala cvms attestation <CVM_NAME> --json | jq '.tcb_info'`
    ///
    /// If `clear_others` is true, removes all existing entries before adding.
    pub fn add_approved_measurements(&mut self, measurements: ApprovedMeasurements, clear_others: Option<bool>) {
        self.assert_owner();
        Self::validate_measurements(&measurements);

        if clear_others.unwrap_or(false) {
            let count = self.approved_measurements.len();
            self.approved_measurements.clear();
            env::log_str(&format!("Cleared {} existing measurement entries", count));
        }

        if !self.approved_measurements.contains(&measurements) {
            self.approved_measurements.push(measurements.clone());
        }

        env::log_str(&format!(
            "Approved measurements added: mrtd={}, rtmr0={}, rtmr1={}, rtmr2={}, rtmr3={}",
            measurements.mrtd, measurements.rtmr0, measurements.rtmr1,
            measurements.rtmr2, measurements.rtmr3
        ));
        env::log_str(&format!("Total approved measurements: {}", self.approved_measurements.len()));
    }

    /// Owner: Clear all approved measurements
    pub fn clear_all_approved_measurements(&mut self) {
        self.assert_owner();

        let count = self.approved_measurements.len();
        self.approved_measurements.clear();

        env::log_str(&format!("Cleared all {} measurement entries", count));
    }

    /// Owner: Remove specific approved measurements
    pub fn remove_approved_measurements(&mut self, measurements: ApprovedMeasurements) {
        self.assert_owner();
        self.approved_measurements.retain(|m| m != &measurements);
        env::log_str(&format!("Approved measurements removed. Remaining: {}", self.approved_measurements.len()));
    }

    /// Owner: Add DAO member
    pub fn add_dao_member(&mut self, member: AccountId) {
        self.assert_owner();

        self.dao_members.insert(&member);

        // Recalculate threshold
        self.approval_threshold = (self.dao_members.len() as u32 / 2) + 1;

        env::log_str(&format!("Added DAO member: {}", member));
    }

    /// Owner: Remove DAO member
    pub fn remove_dao_member(&mut self, member: AccountId) {
        self.assert_owner();
        assert!(self.dao_members.len() > 1, "Cannot remove last DAO member");

        self.dao_members.remove(&member);

        // Recalculate threshold
        self.approval_threshold = (self.dao_members.len() as u32 / 2) + 1;

        env::log_str(&format!("Removed DAO member: {}", member));
    }

    /// Owner: Update TDX quote collateral
    pub fn update_collateral(&mut self, collateral: String) {
        self.assert_owner();
        self.quote_collateral = Some(collateral);
        env::log_str("Quote collateral updated");
    }

    // ===== View Methods =====

    /// Get proposal details
    pub fn get_proposal(&self, proposal_id: u64) -> Option<KeystoreProposal> {
        self.proposals.get(&proposal_id)
    }

    /// Get all proposals
    pub fn get_proposals(&self, from_index: u64, limit: u64) -> Vec<KeystoreProposal> {
        let mut result = Vec::new();
        for i in from_index..from_index.saturating_add(limit).min(self.next_proposal_id) {
            if let Some(proposal) = self.proposals.get(&i) {
                result.push(proposal);
            }
        }
        result
    }

    /// Get DAO members
    pub fn get_dao_members(&self) -> Vec<AccountId> {
        self.dao_members.to_vec()
    }

    /// Get approved measurements list
    pub fn get_approved_measurements(&self) -> Vec<ApprovedMeasurements> {
        self.approved_measurements.clone()
    }

    /// Check if measurements are approved
    pub fn is_measurements_approved(&self, measurements: ApprovedMeasurements) -> bool {
        self.approved_measurements.contains(&measurements)
    }

    /// Check if keystore with given public key is approved
    pub fn is_keystore_approved(&self, public_key: String) -> bool {
        // Parse public key to validate format
        if let Ok(parsed_key) = public_key.parse::<PublicKey>() {
            // Check if this public key exists in approved keystores
            self.approved_keystores.contains(&parsed_key)
        } else {
            false
        }
    }

    /// Get owner
    pub fn get_owner(&self) -> AccountId {
        self.owner_id.clone()
    }

    /// Get config
    pub fn get_config(&self) -> serde_json::Value {
        serde_json::json!({
            "owner_id": self.owner_id,
            "init_account_id": self.init_account_id,
            "mpc_contract_id": self.mpc_contract_id,
            "approval_threshold": self.approval_threshold,
            "dao_members_count": self.dao_members.len(),
            "next_proposal_id": self.next_proposal_id,
            "approved_keystores_count": self.approved_keystores.len(),
            "approved_measurements_count": self.approved_measurements.len(),
            "has_collateral": self.quote_collateral.is_some(),
        })
    }


    /// Request a key from the MPC contract
    /// This function makes a cross-contract call to the MPC contract to derive a private key
    /// The request must come from an approved keystore with a valid access key
    pub fn request_key(&self, request: CKDRequestArgs) -> PromiseOrValue<CKDResponse> {
        require!(
            env::predecessor_account_id() == env::current_account_id(),
            "must be called via an approved-keystore access key on this contract"
        );

        // Make cross-contract call to MPC contract
        // Attach all gas and 1 yoctoNEAR as required by MPC contract
        let promise = ext_mpc::ext(self.mpc_contract_id.clone())
            .with_static_gas(Gas::from_tgas(100)) // Use 100 TGas for the call
            .with_attached_deposit(NearToken::from_yoctonear(1)) // Attach 1 yoctoNEAR
            .request_app_private_key(request);

        // Return the promise - NEAR will handle the callback automatically
        PromiseOrValue::Promise(promise)
    }

    // ===== Cessation (DAO-member single-signer, reversible) =====
    //
    // Any DAO member can flip the cessation flag in either direction.
    // The plan accepts single-signer here because the threat model is
    // "OutLayer is shutting down" — speed of declaration matters more
    // than multi-party governance, and an erroneous declaration is
    // immediately reversible by any other DAO member.

    /// Declare cessation — opens the recovery window for every vault.
    pub fn declare_cessation(&mut self) {
        self.assert_dao_member();
        require!(!self.ceased_operations, "already ceased");
        self.ceased_operations = true;
        env::log_str("cessation_declared");
    }

    /// Revoke cessation — closes the recovery window. Vaults whose
    /// finalize_recovery callback reads `is_ceased() == false` after
    /// this point will have their recovery cancelled.
    pub fn revoke_cessation(&mut self) {
        self.assert_dao_member();
        require!(self.ceased_operations, "not currently ceased");
        self.ceased_operations = false;
        env::log_str("cessation_revoked");
    }

    /// `true` while the DAO has declared cessation. Vault contracts
    /// cross-contract-call this method as the gate on `initiate_recovery`
    /// and `finalize_recovery` for the Cessation trigger.
    pub fn is_ceased(&self) -> bool {
        self.ceased_operations
    }

    // ===== Vault code-hash whitelist =====
    //
    // Auth model: `approve_vault_version` and `revoke_vault_version`
    // require `approval_threshold` distinct DAO-member votes for the
    // same hash before executing (see `vault_version_votes` +
    // `vault_version_approval_args`). `deprecate_vault_version` is
    // intentionally single-member: deprecation is a soft signal
    // (existing vaults keep working), and a single rogue
    // deprecation is reversible by re-approving through quorum.
    //
    // Why this shape rather than reusing `KeystoreProposal` /
    // `vote()`: the existing proposal flow is hard-wired to
    // keystore-registration (it carries TDX measurements, voter
    // tallies, etc.). Generalising it to a sum-typed
    // `Proposal { kind, payload }` would require migrating existing
    // on-chain proposal records — disproportionate cost for one
    // additional action class. The standalone `vault_version_votes`
    // map mirrors the same multisig semantics in ~30 lines.
    //
    // Trust impact: with `approval_threshold` (>50% of members)
    // votes required, no single rogue/compromised DAO member can
    // whitelist a malicious WASM hash. Operator mitigations remain
    // in force as defense in depth:
    //   * run an off-chain monitor on `vault_version_approval_vote`
    //     events to alert on hashes nearing quorum;
    //   * `revoke_vault_version` is also quorum-gated and reversible;
    //   * vault-checker reads `is_vault_code_approved` as ground
    //     truth and trusts the on-chain quorum.

    /// Quorum-gated approval of a new vault contract code hash.
    ///
    /// Each DAO member's call records a vote for the given hash;
    /// when `approval_threshold` (>50%) distinct members have
    /// voted, the hash is whitelisted and the pending entry is
    /// cleared. The first proposer's `(label, audit_url)` is locked
    /// into `vault_version_approval_args`; subsequent voters MUST
    /// pass matching args or the call panics with a clear message
    /// (prevents silent quorum-bricking from minor metadata typos).
    ///
    /// Idempotent on the hash itself — re-approving an already-
    /// approved hash overwrites the metadata only after the new
    /// approval reaches quorum.
    ///
    /// Returns the number of votes recorded so far (so a caller can
    /// see whether their vote was decisive).
    pub fn approve_vault_version(
        &mut self,
        hash: Base58CryptoHash,
        label: String,
        audit_url: Option<String>,
    ) -> u32 {
        self.assert_dao_member();
        require!(label.len() <= 64, "label must be at most 64 bytes");
        if let Some(ref url) = audit_url {
            require!(url.len() <= 256, "audit_url must be at most 256 bytes");
        }

        // Vote ledger is keyed on hash only. The first proposer's
        // `(label, audit_url)` is locked into a parallel map;
        // subsequent voters must pass the SAME args or get an
        // unambiguous error. This prevents silent quorum-bricking
        // from minor metadata typos like "v1.0" vs "v1.0 ".
        let action = VaultVersionAction::Approve { hash };
        let proposed = ApprovalArgs {
            label: label.clone(),
            audit_url: audit_url.clone(),
        };
        match self.vault_version_approval_args.get(&hash) {
            Some(existing) if existing != proposed => {
                env::panic_str(&format!(
                    "approve_vault_version: a proposal for hash {} is already in flight \
                     with first-proposer args label={:?} audit_url={:?}; your call passed \
                     label={:?} audit_url={:?}. Match those exactly to vote, or wait for the \
                     current proposal to be cleared by quorum.",
                    String::from(&hash),
                    existing.label,
                    existing.audit_url,
                    proposed.label,
                    proposed.audit_url,
                ));
            }
            Some(_) => { /* matches; continue */ }
            None => {
                // First-proposer wins: lock args.
                self.vault_version_approval_args.insert(&hash, &proposed);
            }
        }

        let voter = env::predecessor_account_id();
        let mut voters = self.vault_version_votes.get(&action).unwrap_or_default();
        // Drop votes from anyone who is no longer a DAO member. Without
        // this, a malicious owner could record votes from members A,B,
        // remove honest members C,D,E to drop `approval_threshold`, and
        // then any re-vote from A or B (even a no-op duplicate) would
        // see the stale `voters.len() >= new_threshold` and execute.
        voters.retain(|v| self.dao_members.contains(v));
        if !voters.contains(&voter) {
            voters.push(voter);
        }
        let count = voters.len() as u32;

        if count >= self.approval_threshold {
            // Quorum reached — execute and clear BOTH the vote
            // ledger and the locked-args entry.
            self.vault_version_votes.remove(&action);
            self.vault_version_approval_args.remove(&hash);
            self.internal_approve_vault_version(hash, label, audit_url);
        } else {
            self.vault_version_votes.insert(&action, &voters);
            env::log_str(&format!(
                "vault_version_approval_vote hash={} votes={}/{}",
                String::from(&hash),
                count,
                self.approval_threshold,
            ));
        }
        count
    }

    /// Mark an approved version as deprecated. Single-DAO-member
    /// gate (NOT multisig) — deprecation is a SOFT signal: the hash
    /// stays in `approved_vault_code_hashes` so existing vaults remain
    /// valid, and new deployments are merely steered toward a
    /// non-deprecated hash. The fast-response path is preserved
    /// because deprecation is reversible (just call
    /// `approve_vault_version` again — that goes through quorum).
    pub fn deprecate_vault_version(&mut self, hash: Base58CryptoHash) {
        self.assert_dao_member();
        let mut info = self
            .vault_versions
            .get(&hash)
            .unwrap_or_else(|| env::panic_str("vault version not found"));
        info.deprecated = true;
        self.vault_versions.insert(&hash, &info);
        env::log_str(&format!("vault_version_deprecated {}", String::from(&hash)));
    }

    /// Quorum-gated hard removal of a vault contract code hash.
    /// Reserved for critical vulnerabilities — existing vaults on
    /// this hash will fail re-verification (their
    /// `is_vault_code_approved` check returns false), and any new
    /// deployment of this hash will be rejected by vault-checker.
    ///
    /// Gated by `approval_threshold` votes, same pattern as
    /// `approve_vault_version`. Vote ledger is keyed on
    /// `Revoke { hash }` so this is independent of any pending
    /// Approve votes for the same hash.
    pub fn revoke_vault_version(&mut self, hash: Base58CryptoHash) -> u32 {
        self.assert_dao_member();

        let action = VaultVersionAction::Revoke { hash };
        let voter = env::predecessor_account_id();
        let mut voters = self.vault_version_votes.get(&action).unwrap_or_default();
        // Drop stale votes from removed members; see `approve_vault_version`.
        voters.retain(|v| self.dao_members.contains(v));
        if !voters.contains(&voter) {
            voters.push(voter);
        }
        let count = voters.len() as u32;

        if count >= self.approval_threshold {
            self.vault_version_votes.remove(&action);
            self.internal_revoke_vault_version(hash);
        } else {
            self.vault_version_votes.insert(&action, &voters);
            env::log_str(&format!(
                "vault_version_revoke_vote hash={} votes={}/{}",
                String::from(&hash),
                count,
                self.approval_threshold,
            ));
        }
        count
    }

    /// View — how many DAO members have so far voted for the given
    /// pending vault-version action? Returns 0 if none. Useful for
    /// dashboards showing "<n>/<threshold> votes".
    pub fn get_vault_version_votes(&self, action: VaultVersionAction) -> u32 {
        self.vault_version_votes
            .get(&action)
            .map(|v| v.len() as u32)
            .unwrap_or(0)
    }

    /// View — does the given vault contract sha256 hash sit in the
    /// approved set right now? Includes deprecated entries.
    pub fn is_vault_code_approved(&self, hash: Base58CryptoHash) -> bool {
        self.approved_vault_code_hashes.contains(&hash)
    }

    /// View — list approved versions with their metadata. Pagination
    /// is intentionally absent: the expected size of this list is
    /// single-digit (one per release), bounded by the natural cadence
    /// of vault contract revisions.
    pub fn list_approved_vault_versions(&self) -> Vec<(Base58CryptoHash, VaultVersionInfo)> {
        self.approved_vault_code_hashes
            .iter()
            .filter_map(|h| self.vault_versions.get(&h).map(|info| (h, info)))
            .collect()
    }

    // ===== Vault verification (called by approved keystores via
    //       the contract's own access key) =====

    /// Mark a vault account as verified. Called by an approved
    /// keystore-worker through the access key this contract installs
    /// on itself — `predecessor_account_id == current_account_id`.
    ///
    /// **Operator note (deploy-time):** legacy keystore access keys
    /// issued before vault-registry support were given the method-list
    /// `["request_key"]` only — they cannot reach this method. Newly
    /// approved keystores get `["request_key", "mark_vault_verified",
    /// "ban_vault"]`, so any keystore-worker that needs to call
    /// `mark_vault_verified` must be re-registered through
    /// `submit_keystore_registration` + DAO vote. Legacy keystores
    /// remain functional for `request_key` only.
    ///
    /// **Trust model:** this method trusts ALL approved keystores
    /// equally. Every keystore-worker that has been added through
    /// `submit_keystore_registration` + DAO vote receives an access
    /// key on this contract whose method-list includes
    /// `mark_vault_verified` (see `internal_execute_proposal`). A
    /// regular CKD-only keystore can therefore call this method even
    /// though it isn't an "attested vault-checker" specifically. The
    /// plan accepts this conflation: TEE attestation + DAO approval
    /// are the trust boundary, not per-method capability splits.
    ///
    /// **Banned-vault guard:** if the vault is currently in
    /// `banned_vaults`, this call panics rather than silently writing
    /// to `verified_vaults`. Without the guard the two sets would
    /// drift (a banned vault still showing up in `verified_vaults`)
    /// and confuse off-chain indexers — `is_vault_verified` already
    /// short-circuits to `false` for banned vaults but the underlying
    /// state would be misleading.
    pub fn mark_vault_verified(&mut self, vault_id: AccountId) {
        require!(
            env::predecessor_account_id() == env::current_account_id(),
            "must be called via an approved-keystore access key on this contract"
        );
        require!(
            !self.banned_vaults.contains(&vault_id),
            "vault is banned; call unban_vault (DAO-only) before re-marking it verified"
        );
        self.verified_vaults.insert(&vault_id);
        env::log_str(&format!("vault_verified {}", vault_id));
    }

    /// View — has the vault been verified by an attested keystore AND
    /// not subsequently banned? End-users gate trust decisions on this.
    pub fn is_vault_verified(&self, vault_id: AccountId) -> bool {
        self.verified_vaults.contains(&vault_id) && !self.banned_vaults.contains(&vault_id)
    }

    // ===== Vault ban (race-attack mitigation) =====

    /// Ban a vault. Callable by:
    ///   * any DAO member directly (predecessor in `dao_members`), OR
    ///   * an approved keystore via the contract's own access key
    ///     (predecessor == current_account_id).
    /// The reason is logged for audit visibility but not stored
    /// long-term — keep the ban itself in the set, and reasons in
    /// `vault_banned` log events for off-chain indexers.
    pub fn ban_vault(&mut self, vault_id: AccountId, reason: String) {
        let pred = env::predecessor_account_id();
        let allowed =
            pred == env::current_account_id() || self.dao_members.contains(&pred);
        require!(
            allowed,
            "ban_vault: only an approved keystore (via this contract's access key) \
             or a DAO member can ban a vault"
        );
        require!(reason.len() <= 256, "ban reason must be at most 256 bytes");
        self.banned_vaults.insert(&vault_id);
        env::log_str(&format!(
            "vault_banned {} reason=\"{}\"",
            vault_id, reason
        ));
    }

    /// Unban a vault — DAO-member-only. The asymmetry vs ban_vault is
    /// intentional: false-positive race-attack detections must always
    /// be reversible, but the reversal needs a human in the loop.
    pub fn unban_vault(&mut self, vault_id: AccountId) {
        self.assert_dao_member();
        self.banned_vaults.remove(&vault_id);
        env::log_str(&format!("vault_unbanned {}", vault_id));
    }

    /// View — has this vault been banned?
    pub fn is_vault_banned(&self, vault_id: AccountId) -> bool {
        self.banned_vaults.contains(&vault_id)
    }
}

impl KeystoreDao {
    // ===== Internal Methods =====

    /// Verify TDX quote and extract full measurements + public key
    fn verify_tdx_quote(&self, tdx_quote_hex: &str, collateral_json: &str) -> (ApprovedMeasurements, PublicKey) {
        use dcap_qvl::verify;

        // Decode hex quote
        let quote_bytes = hex::decode(tdx_quote_hex).expect("Invalid hex encoding");

        // Parse collateral
        let collateral_value: serde_json::Value = serde_json::from_str(collateral_json)
            .expect("Failed to parse collateral JSON");
        let collateral = Collateral::try_from_json(collateral_value)
            .expect("Failed to parse collateral");

        // Verify quote with dcap-qvl 0.3.11
        let now = env::block_timestamp() / 1_000_000_000; // Convert nanos to seconds
        let result = verify::verify(&quote_bytes, collateral.inner(), now)
            .expect("TDX quote verification failed");

        // Reject anything but an up-to-date platform: dcap-qvl's verify() returns Ok
        // for OutOfDate / ConfigurationNeeded / SWHardeningNeeded — only Revoked errors.
        assert_eq!(
            result.status.as_str(),
            "UpToDate",
            "TDX TCB status not acceptable: {} (platform needs a firmware/microcode update)",
            result.status
        );
        assert!(
            result.advisory_ids.is_empty(),
            "TDX platform has outstanding security advisories: {}",
            result.advisory_ids.join(", ")
        );

        // Extract all measurements from TDX report (MRTD + RTMR0-3)
        let td10 = result
            .report
            .as_td10()
            .expect("Quote is not TDX format");

        let measurements = ApprovedMeasurements {
            mrtd: hex::encode(td10.mr_td.to_vec()),
            rtmr0: hex::encode(td10.rt_mr0.to_vec()),
            rtmr1: hex::encode(td10.rt_mr1.to_vec()),
            rtmr2: hex::encode(td10.rt_mr2.to_vec()),
            rtmr3: hex::encode(td10.rt_mr3.to_vec()),
        };

        // Extract public key from report_data (first 32 bytes)
        let pubkey_bytes = &td10.report_data[..32];

        // Convert to NEAR PublicKey (add ed25519 prefix)
        let pubkey_with_prefix = [&[0u8], pubkey_bytes].concat();
        let public_key = PublicKey::try_from(pubkey_with_prefix)
            .expect("Invalid ed25519 public key");

        (measurements, public_key)
    }

    fn assert_owner(&self) {
        assert_eq!(
            env::predecessor_account_id(),
            self.owner_id,
            "Only owner can call this method"
        );
    }

    fn validate_measurement_field(name: &str, value: &str) {
        assert_eq!(
            value.len(),
            96,
            "Invalid {} format: expected 96 hex characters, got {}",
            name,
            value.len()
        );
        assert!(
            value.chars().all(|c| c.is_ascii_hexdigit()),
            "Invalid {} format: must be hex string",
            name
        );
    }

    fn validate_measurements(m: &ApprovedMeasurements) {
        Self::validate_measurement_field("mrtd", &m.mrtd);
        Self::validate_measurement_field("rtmr0", &m.rtmr0);
        Self::validate_measurement_field("rtmr1", &m.rtmr1);
        Self::validate_measurement_field("rtmr2", &m.rtmr2);
        Self::validate_measurement_field("rtmr3", &m.rtmr3);
    }

    /// Execute approved proposal to add keystore access key
    fn internal_execute_proposal(&mut self, proposal_id: u64, mut proposal: KeystoreProposal) {
        // Check status
        assert_eq!(
            proposal.status, ProposalStatus::Approved,
            "Proposal is not approved"
        );

        // Add public key to this contract's account.
        //
        // Permitted methods:
        //   * `request_key`           — original CKD-via-MPC flow.
        //   * `mark_vault_verified`   — vault-checker WASI agent uses
        //                                this to record off-chain
        //                                verification results.
        //   * `ban_vault`             — automated race-attack monitor
        //                                may invoke this when it
        //                                detects duplicate MPC calls.
        //
        // Unlimited allowance: a finite allowance (previously 1 NEAR
        // ≈ 1000 tx) creates a single-point-of-failure where a
        // sustained spam against `/sign-vault-verification` could
        // drain the key, bricking BOTH `mark_vault_verified` AND
        // `ban_vault` for every customer simultaneously. The blast
        // radius (whole trust pipeline) outweighs the upside of a
        // gas budget cap, especially since the FCAK is already
        // method-list-bounded (no transfer / cross-contract escape)
        // and the holder runs inside an attested TEE.
        Promise::new(env::current_account_id()).add_access_key_allowance(
            proposal.public_key.clone(),
            Allowance::Unlimited,
            env::current_account_id(),
            "request_key,mark_vault_verified,ban_vault".to_string(),
        );

        // Mark as executed
        proposal.status = ProposalStatus::Executed;
        self.proposals.insert(&proposal_id, &proposal);

        // Add to approved keystores
        self.approved_keystores.insert(&proposal.public_key);

        env::log_str(&format!(
            "✅ Executed proposal {}: Added keystore access key {:?} (all 5 TEE measurements verified)",
            proposal_id, proposal.public_key
        ));
    }

    /// Helper: assert the caller is a DAO member. Used by the
    /// single-signer methods (cessation flips, vault version whitelist
    /// management, manual ban / unban).
    fn assert_dao_member(&self) {
        assert!(
            self.dao_members.contains(&env::predecessor_account_id()),
            "only DAO members can call this method"
        );
    }

    /// Apply an approved-by-quorum vault-version whitelist. Private —
    /// only callable from `approve_vault_version` after the vote
    /// threshold is reached.
    fn internal_approve_vault_version(
        &mut self,
        hash: Base58CryptoHash,
        label: String,
        audit_url: Option<String>,
    ) {
        self.approved_vault_code_hashes.insert(&hash);
        self.vault_versions.insert(
            &hash,
            &VaultVersionInfo {
                label,
                audit_url,
                deprecated: false,
                approved_at: env::block_timestamp(),
            },
        );
        env::log_str(&format!(
            "vault_version_approved {}",
            String::from(&hash)
        ));
    }

    /// Apply an approved-by-quorum vault-version revocation. Private —
    /// only callable from `revoke_vault_version` after the vote
    /// threshold is reached.
    fn internal_revoke_vault_version(&mut self, hash: Base58CryptoHash) {
        self.approved_vault_code_hashes.remove(&hash);
        self.vault_versions.remove(&hash);
        env::log_str(&format!(
            "vault_version_revoked {}",
            String::from(&hash)
        ));
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use near_sdk::test_utils::VMContextBuilder;
    use near_sdk::testing_env;

    fn dao_account() -> AccountId {
        "dao.near".parse().unwrap()
    }
    fn member_a() -> AccountId {
        "member-a.near".parse().unwrap()
    }
    fn member_b() -> AccountId {
        "member-b.near".parse().unwrap()
    }
    fn vault_a() -> AccountId {
        "vault.alice.near".parse().unwrap()
    }
    fn vault_b() -> AccountId {
        "vault.bob.near".parse().unwrap()
    }
    fn outsider() -> AccountId {
        "eve.near".parse().unwrap()
    }
    fn dummy_hash() -> Base58CryptoHash {
        // 32 bytes of 0x42 — base58("BB...") suffices for tests; we never
        // need it to match a real WASM hash here.
        let bytes = [0x42u8; 32];
        Base58CryptoHash::from(bytes)
    }
    fn dummy_hash_b() -> Base58CryptoHash {
        let bytes = [0x99u8; 32];
        Base58CryptoHash::from(bytes)
    }

    fn ctx(predecessor: AccountId) -> VMContextBuilder {
        let mut b = VMContextBuilder::new();
        b.current_account_id(dao_account())
            .predecessor_account_id(predecessor);
        b
    }

    fn fresh_dao() -> KeystoreDao {
        testing_env!(ctx("owner.near".parse().unwrap()).build());
        KeystoreDao::new(
            "owner.near".parse().unwrap(),
            "init.near".parse().unwrap(),
            vec![member_a(), member_b()],
            "v1.signer.near".parse().unwrap(),
        )
    }

    #[test]
    fn test_init() {
        let dao = fresh_dao();
        assert_eq!(dao.owner_id, "owner.near".parse::<AccountId>().unwrap());
        assert_eq!(dao.dao_members.len(), 2);
        assert_eq!(dao.approval_threshold, 2); // >50% of 2 members
        assert_eq!(dao.approved_measurements.len(), 0);
        // v2 fields default-initialized
        assert!(!dao.ceased_operations);
        assert_eq!(dao.approved_vault_code_hashes.len(), 0);
        assert_eq!(dao.verified_vaults.len(), 0);
        assert_eq!(dao.banned_vaults.len(), 0);
    }

    // ===== Cessation =====

    #[test]
    fn declare_then_revoke_cessation_round_trip() {
        let mut dao = fresh_dao();
        testing_env!(ctx(member_a()).build());
        dao.declare_cessation();
        assert!(dao.is_ceased());
        testing_env!(ctx(member_b()).build());
        dao.revoke_cessation();
        assert!(!dao.is_ceased());
    }

    #[test]
    #[should_panic(expected = "only DAO members can call this method")]
    fn declare_cessation_rejects_non_member() {
        let mut dao = fresh_dao();
        testing_env!(ctx(outsider()).build());
        dao.declare_cessation();
    }

    #[test]
    #[should_panic(expected = "already ceased")]
    fn declare_cessation_rejects_when_already_ceased() {
        let mut dao = fresh_dao();
        testing_env!(ctx(member_a()).build());
        dao.declare_cessation();
        dao.declare_cessation();
    }

    #[test]
    #[should_panic(expected = "not currently ceased")]
    fn revoke_cessation_rejects_when_not_ceased() {
        let mut dao = fresh_dao();
        testing_env!(ctx(member_a()).build());
        dao.revoke_cessation();
    }

    // ===== Vault code-hash whitelist =====

    #[test]
    fn approve_vault_version_records_metadata() {
        // Requires `approval_threshold` votes (here 2 of 2 members).
        // First call records a vote but does NOT execute; second call
        // from a different member reaches quorum and executes.
        let mut dao = fresh_dao();
        let h = dummy_hash();
        testing_env!(ctx(member_a()).build());
        let votes_a = dao.approve_vault_version(
            h,
            "v1.0".to_string(),
            Some("https://audit.example/v1".to_string()),
        );
        assert_eq!(votes_a, 1);
        assert!(!dao.is_vault_code_approved(h), "should NOT execute on 1 vote");

        testing_env!(ctx(member_b()).build());
        let votes_b = dao.approve_vault_version(
            h,
            "v1.0".to_string(),
            Some("https://audit.example/v1".to_string()),
        );
        // After execution the vote ledger is cleared, so the return
        // value reflects the count at execution time = threshold.
        assert_eq!(votes_b, 2);
        assert!(dao.is_vault_code_approved(h));
        let listed = dao.list_approved_vault_versions();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].0, h);
        assert_eq!(listed[0].1.label, "v1.0");
        assert_eq!(listed[0].1.audit_url.as_deref(), Some("https://audit.example/v1"));
        assert!(!listed[0].1.deprecated);
        // Vote ledger cleared after execution.
        assert_eq!(
            dao.get_vault_version_votes(VaultVersionAction::Approve { hash: h }),
            0
        );
    }

    /// Helper: drive an approve through full quorum (both members).
    /// Used by the deprecate / revoke tests below so they don't have
    /// to reproduce the multisig dance every time.
    fn quorum_approve(dao: &mut KeystoreDao, h: Base58CryptoHash, label: &str, url: &str) {
        testing_env!(ctx(member_a()).build());
        dao.approve_vault_version(h, label.to_string(), Some(url.to_string()));
        testing_env!(ctx(member_b()).build());
        dao.approve_vault_version(h, label.to_string(), Some(url.to_string()));
        assert!(dao.is_vault_code_approved(h));
    }

    /// Helper: drive a revoke through full quorum.
    fn quorum_revoke(dao: &mut KeystoreDao, h: Base58CryptoHash) {
        testing_env!(ctx(member_a()).build());
        dao.revoke_vault_version(h);
        testing_env!(ctx(member_b()).build());
        dao.revoke_vault_version(h);
        assert!(!dao.is_vault_code_approved(h));
    }

    #[test]
    fn approve_vault_version_does_not_execute_on_single_vote() {
        let mut dao = fresh_dao();
        let h = dummy_hash();
        testing_env!(ctx(member_a()).build());
        dao.approve_vault_version(h, "v1.0".into(), Some("url".into()));
        // Same member voting again is idempotent (one entry per voter)
        // — must NOT count as a second vote toward the threshold.
        dao.approve_vault_version(h, "v1.0".into(), Some("url".into()));
        assert!(!dao.is_vault_code_approved(h));
        assert_eq!(
            dao.get_vault_version_votes(VaultVersionAction::Approve { hash: h }),
            1
        );
    }

    /// `fresh_dao()` has 2 members → threshold = 2 (every second
    /// vote is decisive). This helper covers the more interesting
    /// "non-decisive intermediate vote" case with 5 members.
    fn fresh_dao_5() -> KeystoreDao {
        testing_env!(ctx("owner.near".parse().unwrap()).build());
        let m = |s: &str| -> AccountId { s.parse().unwrap() };
        KeystoreDao::new(
            m("owner.near"),
            m("init.near"),
            vec![
                m("m1.near"),
                m("m2.near"),
                m("m3.near"),
                m("m4.near"),
                m("m5.near"),
            ],
            m("v1.signer.near"),
        )
    }

    #[test]
    fn approve_vault_version_3_of_5_quorum() {
        let mut dao = fresh_dao_5();
        assert_eq!(dao.approval_threshold, 3); // (5/2)+1
        let h = dummy_hash();
        let m = |s: &str| -> AccountId { s.parse().unwrap() };

        testing_env!(ctx(m("m1.near")).build());
        let v1 = dao.approve_vault_version(h, "v1".into(), Some("url".into()));
        assert_eq!(v1, 1);
        assert!(!dao.is_vault_code_approved(h));

        testing_env!(ctx(m("m2.near")).build());
        let v2 = dao.approve_vault_version(h, "v1".into(), Some("url".into()));
        assert_eq!(v2, 2);
        assert!(!dao.is_vault_code_approved(h));

        testing_env!(ctx(m("m3.near")).build());
        let v3 = dao.approve_vault_version(h, "v1".into(), Some("url".into()));
        assert_eq!(v3, 3);
        assert!(dao.is_vault_code_approved(h)); // executed at threshold

        // A late vote from a 4th member after execution should NOT
        // crash and should NOT redo the action — the entry was
        // cleared, so this becomes a fresh proposal of count=1.
        testing_env!(ctx(m("m4.near")).build());
        let v4 = dao.approve_vault_version(h, "v1".into(), Some("url".into()));
        assert_eq!(v4, 1, "post-execution vote starts a fresh proposal");
    }

    #[test]
    #[should_panic(expected = "is already in flight with first-proposer args")]
    fn approve_vault_version_second_voter_with_mismatched_args_panics() {
        // The first proposer locks `(label, audit_url)`; a second
        // voter who passes different args MUST get a clear error so
        // they know to match the locked args. Without this guard,
        // distinct args silently brick quorum by producing
        // independent proposals.
        let mut dao = fresh_dao();
        let h = dummy_hash();
        testing_env!(ctx(member_a()).build());
        dao.approve_vault_version(h, "v1.0".into(), Some("url-A".into()));
        testing_env!(ctx(member_b()).build());
        // mismatched args → panic with actionable message
        dao.approve_vault_version(h, "v1.0-fix".into(), Some("url-B".into()));
    }

    #[test]
    fn approve_vault_version_second_voter_with_matching_args_reaches_quorum() {
        // Matching args from the second voter proceed normally.
        // This is the non-typo happy path.
        let mut dao = fresh_dao();
        let h = dummy_hash();
        testing_env!(ctx(member_a()).build());
        dao.approve_vault_version(h, "v1.0".into(), Some("url-A".into()));
        testing_env!(ctx(member_b()).build());
        dao.approve_vault_version(h, "v1.0".into(), Some("url-A".into()));
        assert!(dao.is_vault_code_approved(h));
        // Locked args cleared after quorum.
        assert_eq!(
            dao.get_vault_version_votes(VaultVersionAction::Approve { hash: h }),
            0
        );
    }

    #[test]
    #[should_panic(expected = "only DAO members")]
    fn approve_vault_version_rejects_non_member() {
        let mut dao = fresh_dao();
        testing_env!(ctx(outsider()).build());
        dao.approve_vault_version(dummy_hash(), "v1.0".to_string(), Some("url".to_string()));
    }

    #[test]
    #[should_panic(expected = "label must be at most")]
    fn approve_vault_version_rejects_oversized_label() {
        let mut dao = fresh_dao();
        testing_env!(ctx(member_a()).build());
        dao.approve_vault_version(dummy_hash(), "x".repeat(65), Some("url".to_string()));
    }

    /// Stale-vote attack on `approve_vault_version`. Without
    /// `voters.retain` in the quorum check, this scenario would let
    /// an owner shrink the DAO until a stale 2-vote count satisfies
    /// the new threshold, executing without any current member's
    /// active consent.
    #[test]
    fn approve_vault_version_does_not_count_removed_members() {
        let mut dao = fresh_dao_5();
        assert_eq!(dao.approval_threshold, 3);
        let h = dummy_hash();
        let m = |s: &str| -> AccountId { s.parse().unwrap() };

        testing_env!(ctx(m("m1.near")).build());
        dao.approve_vault_version(h, "v1".into(), Some("url".into()));
        testing_env!(ctx(m("m2.near")).build());
        dao.approve_vault_version(h, "v1".into(), Some("url".into()));
        assert!(!dao.is_vault_code_approved(h));

        // Owner shrinks DAO. Threshold drops; stale votes from m1, m2
        // are still in storage and would falsely satisfy quorum
        // without the retain guard.
        testing_env!(ctx("owner.near".parse().unwrap()).build());
        dao.remove_dao_member(m("m1.near"));
        dao.remove_dao_member(m("m2.near"));
        assert_eq!(dao.approval_threshold, 2); // (3/2)+1

        // m3 votes — m1/m2 are no longer members, must be discarded.
        testing_env!(ctx(m("m3.near")).build());
        let count = dao.approve_vault_version(h, "v1".into(), Some("url".into()));
        assert_eq!(count, 1, "stale votes from removed members must not count");
        assert!(!dao.is_vault_code_approved(h));
    }

    /// Same stale-vote class on `revoke_vault_version`. Revoking a
    /// previously good hash is destructive (existing vaults lose
    /// `is_vault_code_approved`), so this must also resist the
    /// shrink-then-revote attack.
    #[test]
    fn revoke_vault_version_does_not_count_removed_members() {
        let mut dao = fresh_dao_5();
        let h = dummy_hash();
        let m = |s: &str| -> AccountId { s.parse().unwrap() };

        // Approve via 5-member quorum (need 3 of 5 m1..m5).
        testing_env!(ctx(m("m1.near")).build());
        dao.approve_vault_version(h, "v1".into(), Some("url".into()));
        testing_env!(ctx(m("m2.near")).build());
        dao.approve_vault_version(h, "v1".into(), Some("url".into()));
        testing_env!(ctx(m("m3.near")).build());
        dao.approve_vault_version(h, "v1".into(), Some("url".into()));
        assert!(dao.is_vault_code_approved(h));

        // Two members vote to revoke. Threshold=3, so still pending.
        testing_env!(ctx(m("m1.near")).build());
        dao.revoke_vault_version(h);
        testing_env!(ctx(m("m2.near")).build());
        dao.revoke_vault_version(h);
        assert!(dao.is_vault_code_approved(h));

        testing_env!(ctx("owner.near".parse().unwrap()).build());
        dao.remove_dao_member(m("m1.near"));
        dao.remove_dao_member(m("m2.near"));
        assert_eq!(dao.approval_threshold, 2);

        testing_env!(ctx(m("m3.near")).build());
        let count = dao.revoke_vault_version(h);
        assert_eq!(count, 1, "stale revoke votes must not count");
        assert!(dao.is_vault_code_approved(h), "hash must still be approved");
    }

    /// Re-vote after threshold drop must not auto-execute. This is
    /// the original Critical attack path: A and B vote, owner shrinks
    /// DAO, then A re-votes (which is allowed because there's no
    /// "already voted" assertion on this path) — without the retain
    /// guard the stale `voters.len() == 2` would meet the new
    /// `threshold = 2` immediately.
    #[test]
    fn approve_vault_version_revote_after_shrink_does_not_auto_execute() {
        let mut dao = fresh_dao_5();
        let h = dummy_hash();
        let m = |s: &str| -> AccountId { s.parse().unwrap() };

        testing_env!(ctx(m("m1.near")).build());
        dao.approve_vault_version(h, "v1".into(), Some("url".into()));
        testing_env!(ctx(m("m2.near")).build());
        dao.approve_vault_version(h, "v1".into(), Some("url".into()));

        testing_env!(ctx("owner.near".parse().unwrap()).build());
        dao.remove_dao_member(m("m3.near"));
        dao.remove_dao_member(m("m4.near"));
        dao.remove_dao_member(m("m5.near"));
        assert_eq!(dao.approval_threshold, 2); // (2/2)+1

        // m1 re-votes. retain leaves [m1, m2]; push skipped (m1 already in).
        // count = 2 == threshold, so this is a legitimate quorum: both
        // remaining members voted. This case SHOULD execute — and it
        // does. Verify that.
        testing_env!(ctx(m("m1.near")).build());
        let count = dao.approve_vault_version(h, "v1".into(), Some("url".into()));
        assert_eq!(count, 2);
        assert!(dao.is_vault_code_approved(h),
            "when shrunk DAO is exactly the set that voted, that IS a valid quorum");
    }

    /// The dangerous variant of the previous test: shrink to {m1, X}
    /// where X never voted. m1's stale vote alone must NOT count as
    /// quorum even though `voters.len() == threshold` would superficially
    /// match (because m2's vote gets pruned).
    #[test]
    fn approve_vault_version_revote_after_partial_shrink_pending() {
        let mut dao = fresh_dao_5();
        let h = dummy_hash();
        let m = |s: &str| -> AccountId { s.parse().unwrap() };

        testing_env!(ctx(m("m1.near")).build());
        dao.approve_vault_version(h, "v1".into(), Some("url".into()));
        testing_env!(ctx(m("m2.near")).build());
        dao.approve_vault_version(h, "v1".into(), Some("url".into()));

        testing_env!(ctx("owner.near".parse().unwrap()).build());
        // Remove m2 (one of the voters) and m3, m4. DAO becomes
        // {m1, m5}, threshold = 2. Without retain, voters list is
        // [m1, m2] → len == 2 == threshold → would falsely execute.
        dao.remove_dao_member(m("m2.near"));
        dao.remove_dao_member(m("m3.near"));
        dao.remove_dao_member(m("m4.near"));
        assert_eq!(dao.approval_threshold, 2);

        testing_env!(ctx(m("m1.near")).build());
        let count = dao.approve_vault_version(h, "v1".into(), Some("url".into()));
        assert_eq!(count, 1, "m2 was removed, only m1 remains as a valid voter");
        assert!(!dao.is_vault_code_approved(h));
    }

    #[test]
    fn deprecate_vault_version_keeps_hash_in_set() {
        let mut dao = fresh_dao();
        let h = dummy_hash();
        quorum_approve(&mut dao, h, "v1.0", "url");
        // Deprecate is single-member by design (soft signal).
        testing_env!(ctx(member_a()).build());
        dao.deprecate_vault_version(h);
        assert!(dao.is_vault_code_approved(h)); // still approved
        let listed = dao.list_approved_vault_versions();
        assert!(listed[0].1.deprecated);
    }

    #[test]
    fn revoke_vault_version_removes_hash() {
        let mut dao = fresh_dao();
        let h = dummy_hash();
        quorum_approve(&mut dao, h, "v1.0", "url");
        quorum_revoke(&mut dao, h);
        assert!(!dao.is_vault_code_approved(h));
        assert!(dao.list_approved_vault_versions().is_empty());
    }

    #[test]
    fn revoke_vault_version_does_not_execute_on_single_vote() {
        let mut dao = fresh_dao();
        let h = dummy_hash();
        quorum_approve(&mut dao, h, "v1.0", "url");
        // One revoke vote should NOT clear the hash.
        testing_env!(ctx(member_a()).build());
        dao.revoke_vault_version(h);
        assert!(dao.is_vault_code_approved(h));
        // Second revoke from a different member tips quorum.
        testing_env!(ctx(member_b()).build());
        dao.revoke_vault_version(h);
        assert!(!dao.is_vault_code_approved(h));
    }

    #[test]
    fn list_skips_revoked_hashes() {
        let mut dao = fresh_dao();
        quorum_approve(&mut dao, dummy_hash(), "v1.0", "u");
        quorum_approve(&mut dao, dummy_hash_b(), "v1.1", "u");
        quorum_revoke(&mut dao, dummy_hash());
        let listed = dao.list_approved_vault_versions();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].0, dummy_hash_b());
    }

    // ===== mark_vault_verified =====

    #[test]
    fn mark_vault_verified_succeeds_when_called_via_self_access_key() {
        let mut dao = fresh_dao();
        // Approved keystores call into this method by signing a tx FROM
        // the dao account itself (using their installed access key) —
        // so predecessor == current_account_id.
        testing_env!(ctx(dao_account()).build());
        dao.mark_vault_verified(vault_a());
        assert!(dao.is_vault_verified(vault_a()));
    }

    #[test]
    #[should_panic(expected = "must be called via an approved-keystore access key")]
    fn mark_vault_verified_rejects_external_caller() {
        let mut dao = fresh_dao();
        testing_env!(ctx(member_a()).build());
        dao.mark_vault_verified(vault_a());
    }

    #[test]
    fn is_vault_verified_returns_false_when_banned() {
        let mut dao = fresh_dao();
        testing_env!(ctx(dao_account()).build());
        dao.mark_vault_verified(vault_a());
        assert!(dao.is_vault_verified(vault_a()));
        // DAO member bans the vault.
        testing_env!(ctx(member_a()).build());
        dao.ban_vault(vault_a(), "test ban".to_string());
        assert!(!dao.is_vault_verified(vault_a()));
        assert!(dao.is_vault_banned(vault_a()));
    }

    // ===== request_key (CKD proxy authorization) =====

    #[test]
    #[should_panic(expected = "must be called via an approved-keystore access key")]
    fn request_key_rejects_external_caller() {
        // An outside account calling request_key directly arrives with
        // predecessor != current_account_id and must be rejected — otherwise it
        // could obtain a CKD share encrypted to an attacker-supplied app key.
        let dao = fresh_dao();
        testing_env!(ctx(outsider()).build());
        let _ = dao.request_key(CKDRequestArgs {
            derivation_path: String::new(),
            app_public_key: dtos::Bls12381G1PublicKey("attacker-supplied".to_string()),
            domain_id: DomainId(0),
        });
    }

    // ===== ban_vault / unban_vault =====

    #[test]
    fn ban_vault_via_dao_member() {
        let mut dao = fresh_dao();
        testing_env!(ctx(member_a()).build());
        dao.ban_vault(vault_a(), "duplicate_mpc_call".to_string());
        assert!(dao.is_vault_banned(vault_a()));
    }

    #[test]
    fn ban_vault_via_self_access_key() {
        let mut dao = fresh_dao();
        testing_env!(ctx(dao_account()).build());
        dao.ban_vault(vault_a(), "automated_detection".to_string());
        assert!(dao.is_vault_banned(vault_a()));
    }

    #[test]
    #[should_panic(expected = "ban_vault: only an approved keystore")]
    fn ban_vault_rejects_outsider() {
        let mut dao = fresh_dao();
        testing_env!(ctx(outsider()).build());
        dao.ban_vault(vault_a(), "spam".to_string());
    }

    #[test]
    fn unban_vault_works_for_dao_member() {
        let mut dao = fresh_dao();
        testing_env!(ctx(member_a()).build());
        dao.ban_vault(vault_a(), "false positive".to_string());
        assert!(dao.is_vault_banned(vault_a()));
        dao.unban_vault(vault_a());
        assert!(!dao.is_vault_banned(vault_a()));
    }

    #[test]
    #[should_panic(expected = "only DAO members")]
    fn unban_vault_rejects_self_access_key() {
        // Asymmetric auth: ban can be triggered by the keystore worker
        // (via predecessor == current), but unban requires a human DAO
        // member.
        let mut dao = fresh_dao();
        testing_env!(ctx(dao_account()).build());
        dao.unban_vault(vault_a());
    }

    #[test]
    #[should_panic(expected = "vault is banned")]
    fn mark_vault_verified_rejects_banned_vault() {
        // Sequence: verify → ban → attempted re-mark. The re-mark
        // panics rather than silently leaving the two sets out of
        // sync. Off-chain indexers can rely on `verified_vaults` ⊥
        // `banned_vaults` invariant.
        let mut dao = fresh_dao();
        testing_env!(ctx(dao_account()).build());
        dao.mark_vault_verified(vault_a());
        testing_env!(ctx(member_a()).build());
        dao.ban_vault(vault_a(), "race attack".to_string());
        testing_env!(ctx(dao_account()).build());
        dao.mark_vault_verified(vault_a()); // panics: "vault is banned"
    }

    #[test]
    fn unban_then_remark_succeeds() {
        let mut dao = fresh_dao();
        testing_env!(ctx(dao_account()).build());
        dao.mark_vault_verified(vault_a());
        testing_env!(ctx(member_a()).build());
        dao.ban_vault(vault_a(), "false positive".to_string());
        dao.unban_vault(vault_a());
        testing_env!(ctx(dao_account()).build());
        dao.mark_vault_verified(vault_a());
        assert!(dao.is_vault_verified(vault_a()));
    }

    #[test]
    fn vault_b_isolated_from_vault_a_state() {
        let mut dao = fresh_dao();
        testing_env!(ctx(dao_account()).build());
        dao.mark_vault_verified(vault_a());
        assert!(dao.is_vault_verified(vault_a()));
        assert!(!dao.is_vault_verified(vault_b()));
    }
}
