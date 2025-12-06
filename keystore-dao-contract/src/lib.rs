use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};
use near_sdk::collections::{LookupMap, UnorderedSet};
use near_sdk::serde::{Deserialize, Serialize};
use near_sdk::{env, ext_contract, near_bindgen, AccountId, Promise, PromiseOrValue, PublicKey, BorshStorageKey, NearToken, Allowance, Gas};
use schemars::JsonSchema;

// Collateral wrapper for TDX verification (from register-contract)
mod collateral;
use collateral::Collateral;

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

// External interface for MPC contract
#[ext_contract(ext_mpc)]
#[allow(dead_code)]
trait ExtMPC {
    fn request_app_private_key(&self, request: CKDRequestArgs) -> CKDResponse;
}

#[derive(BorshSerialize, BorshStorageKey)]
#[borsh(crate = "near_sdk::borsh")]
enum StorageKey {
    ApprovedRtmr3,
    DaoMembers,
    Proposals,
    Votes { proposal_id: u64 },
    ApprovedKeystores,
}

/// Proposal for registering a new keystore
#[derive(BorshDeserialize, BorshSerialize, Serialize, Deserialize, Clone, JsonSchema)]
#[serde(crate = "near_sdk::serde")]
pub struct KeystoreProposal {
    pub id: u64,
    #[schemars(with = "String")]
    pub public_key: PublicKey,
    pub rtmr3: String,
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
    pub rtmr3: String,
    pub approved_at: u64,
    pub proposal_id: u64,
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
/// 3. Contract verifies attestation and creates proposal
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

    /// List of approved RTMR3 measurements
    pub approved_rtmr3: UnorderedSet<String>,

    /// TDX quote collateral (Intel's reference data for verification)
    pub quote_collateral: Option<String>,
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
            approved_rtmr3: UnorderedSet::new(StorageKey::ApprovedRtmr3),
            quote_collateral: None,
        }
    }

    /// Submit keystore registration with TEE attestation
    ///
    /// This method:
    /// 1. Verifies TDX quote signature
    /// 2. Extracts RTMR3 and public key
    /// 3. Creates a proposal for DAO voting
    pub fn submit_keystore_registration(
        &mut self,
        public_key: PublicKey,
        tdx_quote_hex: String,
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

        let (rtmr3, embedded_pubkey) = self.verify_tdx_quote(&tdx_quote_hex, &collateral);

        env::log_str(&format!(
            "TEE Registration Request. RTMR3: {}. Public Key: {:?}",
            rtmr3, embedded_pubkey
        ));

        // CRITICAL: Check if RTMR3 is in approved list
        assert!(
            self.approved_rtmr3.contains(&rtmr3),
            "RTMR3 {} not approved for registration. Contact admin to add this RTMR3.",
            rtmr3
        );

        env::log_str(&format!("✅ RTMR3 {} is in approved list", rtmr3));

        // Verify public key matches quote
        assert_eq!(
            embedded_pubkey, public_key,
            "Public key mismatch: provided key doesn't match TDX quote"
        );

        // Create proposal
        let proposal = KeystoreProposal {
            id: self.next_proposal_id,
            public_key: public_key.clone(),
            rtmr3: rtmr3.clone(),
            submitter: env::predecessor_account_id(),
            created_at: env::block_timestamp(),
            votes_for: self.approval_threshold,
            votes_against: 0,
            status: ProposalStatus::Pending,
        };

        let proposal_id = self.next_proposal_id;
        self.proposals.insert(&proposal_id, &proposal);
        self.next_proposal_id += 1;
        
        env::log_str(&format!(
            "Created proposal {} for keystore registration (RTMR3: {})",
            proposal_id, rtmr3
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

        // Check if threshold reached
        if proposal.votes_for >= self.approval_threshold {
            proposal.status = ProposalStatus::Approved;
                        
            env::log_str(&format!(
                "Proposal {} approved with {} votes",
                proposal_id, proposal.votes_for
            ));

            self.internal_execute_proposal(proposal_id, proposal);
        } else if proposal.votes_against > (self.dao_members.len() as u32 - self.approval_threshold) {
            proposal.status = ProposalStatus::Rejected;
            self.proposals.insert(&proposal_id, &proposal);

            env::log_str(&format!(
                "Proposal {} rejected with {} votes against",
                proposal_id, proposal.votes_against
            ));
        }
        else {
            // Update proposal
            self.proposals.insert(&proposal_id, &proposal);
        }        
    }

    /// Owner: Add approved RTMR3 for auto-approval
    /// If clear_others is true, removes all existing RTMR3s before adding the new one
    pub fn add_approved_rtmr3(&mut self, rtmr3: String, clear_others: Option<bool>) {
        self.assert_owner();
        assert_eq!(rtmr3.len(), 96, "RTMR3 must be 96 hex chars");

        // Clear all existing RTMR3s if requested (useful for testing)
        if clear_others.unwrap_or(false) {
            let count = self.approved_rtmr3.len();
            self.approved_rtmr3.clear();
            env::log_str(&format!("Cleared {} existing RTMR3 entries", count));
        }

        self.approved_rtmr3.insert(&rtmr3);
        env::log_str(&format!("Added approved RTMR3: {}", rtmr3));
        env::log_str(&format!("Total approved RTMR3s: {}", self.approved_rtmr3.len()));
    }

    /// Owner: Clear all approved RTMR3s (useful for testing)
    pub fn clear_all_approved_rtmr3(&mut self) {
        self.assert_owner();

        let count = self.approved_rtmr3.len();
        self.approved_rtmr3.clear();

        env::log_str(&format!("Cleared all {} RTMR3 entries", count));
    }

    /// Owner: Remove specific approved RTMR3
    pub fn remove_approved_rtmr3(&mut self, rtmr3: String) {
        self.assert_owner();
        assert_eq!(rtmr3.len(), 96, "RTMR3 must be 96 hex chars");

        let was_present = self.approved_rtmr3.remove(&rtmr3);

        if was_present {
            env::log_str(&format!("Removed approved RTMR3: {}", rtmr3));
            env::log_str(&format!("Total approved RTMR3s remaining: {}", self.approved_rtmr3.len()));
        } else {
            env::log_str(&format!("RTMR3 not found in approved list: {}", rtmr3));
        }
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

    /// Get approved RTMR3 list
    pub fn get_approved_rtmr3(&self) -> Vec<String> {
        self.approved_rtmr3.to_vec()
    }

    /// Check if RTMR3 is approved
    pub fn is_rtmr3_approved(&self, rtmr3: String) -> bool {
        self.approved_rtmr3.contains(&rtmr3)
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
            "approved_rtmr3_count": self.approved_rtmr3.len(),
            "has_collateral": self.quote_collateral.is_some(),
        })
    }    
    

    /// Request a key from the MPC contract
    /// This function makes a cross-contract call to the MPC contract to derive a private key
    /// The request must come from an approved keystore with a valid access key    
    pub fn request_key(&self, request: CKDRequestArgs) -> PromiseOrValue<CKDResponse> {
        // Make cross-contract call to MPC contract
        // Attach all gas and 1 yoctoNEAR as required by MPC contract
        let promise = ext_mpc::ext(self.mpc_contract_id.clone())
            .with_static_gas(Gas::from_tgas(100)) // Use 100 TGas for the call
            .with_attached_deposit(NearToken::from_yoctonear(1)) // Attach 1 yoctoNEAR
            .request_app_private_key(request);

        // Return the promise - NEAR will handle the callback automatically
        PromiseOrValue::Promise(promise)
    }
}

impl KeystoreDao {
    // ===== Internal Methods =====

    /// Verify TDX quote and extract RTMR3 + public key
    fn verify_tdx_quote(&self, tdx_quote_hex: &str, collateral_json: &str) -> (String, PublicKey) {
        use dcap_qvl::verify;

        // Decode hex quote
        let quote_bytes = hex::decode(tdx_quote_hex).expect("Invalid hex encoding");

        // Parse collateral
        let collateral_value: serde_json::Value = serde_json::from_str(collateral_json)
            .expect("Failed to parse collateral JSON");
        let collateral = Collateral::try_from_json(collateral_value)
            .expect("Failed to parse collateral");

        // Verify quote with dcap-qvl 0.3.2
        let now = env::block_timestamp() / 1_000_000_000; // Convert nanos to seconds
        let result = verify::verify(&quote_bytes, collateral.inner(), now)
            .expect("TDX quote verification failed");

        // Extract RTMR3 from TDX report
        let rtmr3_bytes = result
            .report
            .as_td10()
            .expect("Quote is not TDX format")
            .rt_mr3;
        let rtmr3 = hex::encode(rtmr3_bytes.to_vec());

        // Extract public key from report_data (first 32 bytes)
        let report_data = result.report.as_td10().unwrap().report_data;
        let pubkey_bytes = &report_data[..32];

        // Convert to NEAR PublicKey (add ed25519 prefix)
        let pubkey_with_prefix = [&[0u8], pubkey_bytes].concat();
        let public_key = PublicKey::try_from(pubkey_with_prefix)
            .expect("Invalid ed25519 public key");

        (rtmr3, public_key)
    }

    fn assert_owner(&self) {
        assert_eq!(
            env::predecessor_account_id(),
            self.owner_id,
            "Only owner can call this method"
        );
    }

    /// Execute approved proposal to add keystore access key
    fn internal_execute_proposal(&mut self, proposal_id: u64, mut proposal: KeystoreProposal) {
        // Check status
        assert_eq!(
            proposal.status, ProposalStatus::Approved,
            "Proposal is not approved"
        );

        // Add public key to this contract's account
        // Permission: functional key, only allows to request key from the MPC network
        let allowance = Allowance::limited(NearToken::from_near(1)).unwrap(); // 10 NEAR for MPC operations
        Promise::new(env::current_account_id()).add_access_key_allowance(
            proposal.public_key.clone(),
            allowance,
            env::current_account_id(),
            "request_key".to_string(),
        );         

        // Mark as executed
        proposal.status = ProposalStatus::Executed;
        self.proposals.insert(&proposal_id, &proposal);

        // Add to approved keystores
        self.approved_keystores.insert(&proposal.public_key);

        env::log_str(&format!(
            "✅ Executed proposal {}: Added keystore access key (RTMR3: {})",
            proposal_id, proposal.rtmr3
        ));
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_init() {
        let dao = KeystoreDao::new(
            "owner.near".parse().unwrap(),
            "init.near".parse().unwrap(),
            vec!["member1.near".parse().unwrap(), "member2.near".parse().unwrap()],
            "v1.signer.near".parse().unwrap(),
        );
        assert_eq!(dao.owner_id, "owner.near".parse::<AccountId>().unwrap());
        assert_eq!(dao.dao_members.len(), 2);
        assert_eq!(dao.approval_threshold, 2); // >50% of 2 members
    }
}