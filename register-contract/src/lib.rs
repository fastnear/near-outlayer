use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};
use near_sdk::collections::UnorderedSet;
use near_sdk::serde::{Deserialize, Serialize};
use near_sdk::{env, near_bindgen, AccountId, Allowance, NearToken, Promise, PublicKey, BorshStorageKey};
use schemars::JsonSchema;

// Collateral wrapper with Borsh support (same approach as MPC Node)
mod collateral;
use collateral::Collateral;

mod migration;

// Custom getrandom implementation for WASM (same as MPC Node)
// We don't need actual randomness in this contract (only verification)
#[cfg(target_arch = "wasm32")]
use getrandom::{register_custom_getrandom, Error};
#[cfg(target_arch = "wasm32")]
fn randomness_unsupported(_: &mut [u8]) -> Result<(), Error> {
    Err(Error::UNSUPPORTED)
}
#[cfg(target_arch = "wasm32")]
register_custom_getrandom!(randomness_unsupported);

#[derive(BorshSerialize, BorshStorageKey)]
#[borsh(crate = "near_sdk::borsh")]
enum StorageKey {
    ApprovedRtmr3
}

/// Register Contract - TEE Worker Key Registration
///
/// This contract verifies TDX attestations and registers worker public keys
/// by adding them as access keys to its own account (the worker account).
///
/// # Security Model
/// 1. Worker generates keypair INSIDE TEE (private key never leaves TEE)
/// 2. Worker generates TDX quote with public key embedded in report_data
/// 3. This contract verifies TDX quote signature (Intel cryptographic proof)
/// 4. This contract extracts RTMR3 and checks it against approved list
/// 5. This contract adds public key to itself (current_account_id) with permissions for offchainvm_contract_id
/// 6. Result: Public key is cryptographically proven to be generated in approved TEE
///
/// # Usage
/// 1. Deploy contract: `near deploy worker.outlayer.testnet --wasmFile register.wasm`
/// 2. Initialize: `near call worker.outlayer.testnet new '{"owner_id":"outlayer.testnet","init_worker_account":"init-worker.outlayer.testnet"}' --accountId outlayer.testnet`
/// 3. Add approved RTMR3: `near call worker.outlayer.testnet add_approved_rtmr3 '{"rtmr3":"..."}' --accountId outlayer.testnet`
/// 4. Init worker calls: `near call worker.outlayer.testnet register_worker_key '{"public_key":"...","tdx_quote_hex":"..."}' --accountId init-worker.outlayer.testnet`
/// 5. Contract verifies and adds key to worker.outlayer.testnet with permissions for outlayer.testnet
#[near_bindgen]
#[derive(BorshDeserialize, BorshSerialize)]
pub struct RegisterContract {
    /// Owner who can manage approved RTMR3 list
    pub owner_id: AccountId,

    /// Init worker account that can call register_worker_key
    pub init_worker_account: AccountId,

    /// List of approved RTMR3 measurements for worker registration
    /// Format: 96 hex characters (48 bytes)
    pub approved_rtmr3: UnorderedSet<String>,

    /// Quote collateral data (cached, updated periodically by owner)
    /// This is Intel's reference data needed for TDX quote verification
    pub quote_collateral: Option<String>,

    pub outlayer_contract_id: AccountId
}

impl Default for RegisterContract {
    fn default() -> Self {
        env::panic_str("RegisterContract must be initialized");
    }
}

#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(crate = "near_sdk::serde")]
pub struct WorkerKeyInfo {
    #[schemars(with = "String")]
    pub public_key: PublicKey,
    pub rtmr3: String,
    pub registered_at: u64,
}

#[near_bindgen]
impl RegisterContract {
    /// Initialize contract
    #[init]
    pub fn new(owner_id: AccountId, init_worker_account: AccountId, outlayer_contract_id: AccountId) -> Self {
        Self {
            owner_id,
            init_worker_account,
            approved_rtmr3: UnorderedSet::new(StorageKey::ApprovedRtmr3),
            quote_collateral: None,
            outlayer_contract_id
        }
    }

    /// Register worker public key with TEE attestation
    ///
    /// This method:
    /// 1. Verifies TDX quote signature (Intel cryptographic proof)
    /// 2. Extracts RTMR3 from quote
    /// 3. Checks RTMR3 is in approved list
    /// 4. Extracts public key from quote report_data
    /// 5. Verifies public_key parameter matches embedded key
    /// 6. Adds access key to this contract's account (self)
    ///
    /// # Arguments
    /// * `public_key` - Worker's ed25519 public key (generated in TEE)
    /// * `tdx_quote_hex` - Hex-encoded TDX quote from Phala dstack
    ///
    /// # Returns
    /// Promise to add access key to this account
    ///
    /// # Panics
    /// - If TDX quote verification fails
    /// - If RTMR3 not in approved list
    /// - If public key doesn't match quote
    /// - If collateral not cached (use update_collateral first)
    pub fn register_worker_key(
        &mut self,
        public_key: PublicKey,
        tdx_quote_hex: String,
    ) -> Promise {
        // Check that caller is init_worker_account
        assert_eq!(
            env::predecessor_account_id(),
            self.init_worker_account,
            "Only {} can call register_worker_key",
            self.init_worker_account
        );

        // Use ONLY cached collateral (security: prevent custom collateral bypass)
        let collateral = self.quote_collateral.clone()
            .expect("Quote collateral required (cache via update_collateral)");

        // 1. Verify TDX quote and extract RTMR3 + embedded public key
        let (rtmr3, embedded_pubkey) =
            self.verify_worker_registration(&tdx_quote_hex, &collateral);

        // Log RTMR3 for admin to approve (visible even if check fails)
        env::log_str(&format!(
            "📋 Extracted RTMR3 from TDX quote: {}",
            rtmr3
        ));
        env::log_str(&format!(
            "📋 Extracted public key from quote: {:?}",
            embedded_pubkey
        ));

        // 2. Check RTMR3 is approved
        assert!(
            self.approved_rtmr3.contains(&rtmr3),
            "Worker RTMR3 {} not approved for registration. Contact admin to add this RTMR3.",
            rtmr3
        );

        // 3. Verify public_key matches the one embedded in TDX quote
        assert_eq!(
            embedded_pubkey, public_key,
            "Public key mismatch: provided key doesn't match TDX quote report_data"
        );

        env::log_str(&format!(
            "✅ TEE verification passed: rtmr3={}, pubkey={:?}",
            rtmr3, public_key
        ));

        // 4. Add access key to this contract's account (worker account)
        // Permission: Function call to offchainvm_contract_id::resolve_execution and submit_execution_output_and_resolve
        let allowance: Allowance = Allowance::limited(NearToken::from_near(10)).unwrap(); // 10 NEAR for gas
        let method_names = "resolve_execution,submit_execution_output_and_resolve,resume_topup,resume_delete_payment_key".to_string();
        let current_account = env::current_account_id();
        
        env::log_str(&format!(
            "Adding access key to {}: pubkey={:?}, allowance=10 NEAR, methods={}, receiver={}",
            current_account.clone(), public_key, method_names, self.outlayer_contract_id
        ));

        // Add key to this account (self) with permissions for offchainvm_contract_id
        Promise::new(current_account.clone()).add_access_key_allowance(
            public_key,
            allowance,
            self.outlayer_contract_id.clone(),
            method_names,
        )
    }

    /// Verify TDX quote and extract RTMR3 + embedded public key
    ///
    /// Uses dcap-qvl library to:
    /// - Parse TDX quote structure
    /// - Verify Intel's cryptographic signature
    /// - Extract RTMR3 (TEE measurement)
    /// - Extract public key from report_data (first 32 bytes)
    fn verify_worker_registration(
        &self,
        quote_hex: &str,
        collateral_str: &str,
    ) -> (String, PublicKey) {
        use dcap_qvl::verify;
        use hex::decode;

        // Decode hex quote
        let quote_bytes = decode(quote_hex).expect("Invalid quote hex encoding");

        // Parse collateral JSON using MPC Node's Collateral wrapper (audited code)
        // Full structure with 9 fields including CRL
        let collateral_json: serde_json::Value =
            serde_json::from_str(collateral_str).expect("Invalid collateral JSON format");
        let collateral: Collateral = Collateral::try_from_json(collateral_json)
            .expect("Failed to parse collateral");

        // Verify quote with Intel's cryptographic signature
        // Uses dcap_qvl 0.3.2 with "contract" feature (NEAR-compatible, same as MPC Node)
        let now = env::block_timestamp() / 1_000_000_000; // Convert to seconds
        let result = verify::verify(&quote_bytes, collateral.inner(), now)
            .expect("TDX quote verification failed - invalid signature or expired TCB");

        // Extract RTMR3 (TEE measurement register)
        // TDX 1.0 format: result.report.as_td10().rt_mr3
        let rtmr3_bytes = result
            .report
            .as_td10()
            .expect("Quote is not TDX format (expected TD10)")
            .rt_mr3;
        let rtmr3 = hex::encode(rtmr3_bytes.to_vec());

        // Extract public key from report_data (first 32 bytes)
        let report_data = result.report.as_td10().unwrap().report_data;
        let pubkey_bytes = &report_data[..32]; // ed25519 public key (32 bytes)

        // Convert bytes to NEAR PublicKey
        let pubkey_with_prefix = [&[0u8], pubkey_bytes].concat(); // Add ed25519 prefix
        let public_key = PublicKey::try_from(pubkey_with_prefix)
            .expect("Invalid ed25519 public key in quote report_data");

        (rtmr3, public_key)
    }

    /// Update cached quote collateral
    ///
    /// Collateral contains Intel's reference data (certificates, TCB info, CRL)
    /// Should be updated when Intel releases new TCB versions (~ monthly)
    ///
    /// Get collateral from: https://api.trustedservices.intel.com/sgx/certification/v4/
    /// or via Phala's dcap-qvl CLI tool
    pub fn update_collateral(&mut self, collateral: String) {
        self.assert_owner();
        self.quote_collateral = Some(collateral);
        env::log_str("Quote collateral updated");
    }

    /// Add approved RTMR3 measurement
    ///
    /// RTMR3 uniquely identifies a TEE worker image (Docker + Phala config)
    /// Get RTMR3 from first worker deployment via coordinator database:
    /// `SELECT last_seen_rtmr3 FROM worker_auth_tokens WHERE worker_name = 'worker-1'`
    pub fn add_approved_rtmr3(&mut self, rtmr3: String) {
        self.assert_owner();

        // Validate RTMR3 format (96 hex chars = 48 bytes)
        assert_eq!(
            rtmr3.len(),
            96,
            "Invalid RTMR3 format: expected 96 hex characters"
        );
        assert!(
            rtmr3.chars().all(|c| c.is_ascii_hexdigit()),
            "Invalid RTMR3 format: must be hex string"
        );

        self.approved_rtmr3.insert(&rtmr3);
        env::log_str(&format!("Approved RTMR3 added: {}", rtmr3));
    }

    /// Remove approved RTMR3
    pub fn remove_approved_rtmr3(&mut self, rtmr3: String) {
        self.assert_owner();
        self.approved_rtmr3.remove(&rtmr3);
        env::log_str(&format!("Approved RTMR3 removed: {}", rtmr3));
    }

    /// Transfer ownership
    pub fn transfer_ownership(&mut self, new_owner: AccountId) {
        self.assert_owner();
        env::log_str(&format!(
            "Ownership transferred: {} -> {}",
            self.owner_id, new_owner
        ));
        self.owner_id = new_owner;
    }

    // ========== View methods ==========

    /// Get list of approved RTMR3 measurements
    pub fn get_approved_rtmr3(&self) -> Vec<String> {
        self.approved_rtmr3.iter().collect()
    }

    /// Check if RTMR3 is approved
    pub fn is_rtmr3_approved(&self, rtmr3: String) -> bool {
        self.approved_rtmr3.contains(&rtmr3)
    }

    /// Get cached collateral (if any)
    pub fn get_collateral(&self) -> Option<String> {
        self.quote_collateral.clone()
    }

    /// Get init worker account
    pub fn get_init_worker_account(&self) -> AccountId {
        self.init_worker_account.clone()
    }

    // ========== Internal ==========

    fn assert_owner(&self) {
        assert_eq!(
            env::predecessor_account_id(),
            self.owner_id,
            "Only owner can call this method"
        );
    }
}



#[cfg(test)]
mod tests {
    use super::*;
    use near_sdk::test_utils::{accounts, VMContextBuilder};
    use near_sdk::testing_env;

    fn get_context() -> VMContextBuilder {
        let mut builder = VMContextBuilder::new();
        builder
            .current_account_id(accounts(0))
            .predecessor_account_id(accounts(1))
            .signer_account_id(accounts(1));
        builder
    }

    #[test]
    fn test_init() {
        let context = get_context();
        testing_env!(context.build());

        let contract = RegisterContract::new(accounts(1), accounts(2));
        assert_eq!(contract.owner_id, accounts(1));
        assert_eq!(contract.init_worker_account, accounts(2));
        assert_eq!(contract.get_approved_rtmr3().len(), 0);
    }

    #[test]
    fn test_add_approved_rtmr3() {
        let context = get_context();
        testing_env!(context.build());

        let mut contract = RegisterContract::new(accounts(1), accounts(2));

        let rtmr3 = "a".repeat(96); // Valid 96 hex chars
        contract.add_approved_rtmr3(rtmr3.clone());

        assert!(contract.is_rtmr3_approved(rtmr3));
        assert_eq!(contract.get_approved_rtmr3().len(), 1);
    }

    #[test]
    #[should_panic(expected = "Invalid RTMR3 format")]
    fn test_invalid_rtmr3_length() {
        let context = get_context();
        testing_env!(context.build());

        let mut contract = RegisterContract::new(accounts(1), accounts(2));
        contract.add_approved_rtmr3("abc".to_string()); // Too short
    }

    #[test]
    #[should_panic(expected = "Only owner")]
    fn test_non_owner_cannot_add_rtmr3() {
        let mut context = get_context();
        testing_env!(context.build());

        let mut contract = RegisterContract::new(accounts(1), accounts(2));

        // Change predecessor to non-owner
        testing_env!(context.predecessor_account_id(accounts(3)).build());

        contract.add_approved_rtmr3("a".repeat(96));
    }
}
