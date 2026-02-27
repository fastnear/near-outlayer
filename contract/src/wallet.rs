use crate::*;
use near_sdk::env;
use sha2::{Digest, Sha256};

/// Storage cost per byte in NEAR
const STORAGE_PRICE_PER_BYTE: Balance = 10_000_000_000_000_000_000; // 0.00001 NEAR per byte

/// Wallet policy entry stored on-chain
/// Encrypted by keystore (TEE), only keystore can decrypt
#[derive(Clone, Debug)]
#[near(serializers = [borsh])]
pub struct WalletPolicyEntry {
    /// Controller NEAR account (who created/manages this policy)
    pub owner: AccountId,
    /// Encrypted policy data (encrypted by keystore)
    pub encrypted_data: String,
    /// Emergency freeze flag (set by controller without wallet sig)
    pub frozen: bool,
    /// Block timestamp of last update
    pub updated_at: u64,
    /// Storage deposit staked for this entry
    pub storage_deposit: Balance,
}

/// Wallet policy view for JSON responses
#[derive(Clone, Debug)]
#[near(serializers = [json])]
pub struct WalletPolicyView {
    pub owner: AccountId,
    pub encrypted_data: String,
    pub frozen: bool,
    pub updated_at: u64,
}

/// Lightweight view for listing wallets by owner
#[derive(Clone, Debug)]
#[near(serializers = [json])]
pub struct WalletPolicyListItem {
    pub wallet_pubkey: String,
    pub owner: AccountId,
    pub frozen: bool,
    pub updated_at: u64,
}

/// Parse wallet pubkey string into (key_type, raw_bytes)
/// Formats: "ed25519:<hex32>" or "secp256k1:<hex33>"
fn parse_wallet_pubkey(wallet_pubkey: &str) -> (String, Vec<u8>) {
    let parts: Vec<&str> = wallet_pubkey.splitn(2, ':').collect();
    assert!(
        parts.len() == 2,
        "Invalid wallet_pubkey format. Expected 'ed25519:<hex>' or 'secp256k1:<hex>'"
    );

    let key_type = parts[0];
    let hex_str = parts[1];

    assert!(
        key_type == "ed25519" || key_type == "secp256k1",
        "Unsupported key type '{}'. Must be 'ed25519' or 'secp256k1'",
        key_type
    );

    let raw_bytes = hex::decode(hex_str).unwrap_or_else(|_| {
        env::panic_str("Invalid hex encoding in wallet_pubkey");
    });

    match key_type {
        "ed25519" => {
            assert!(
                raw_bytes.len() == 32,
                "Ed25519 public key must be 32 bytes, got {}",
                raw_bytes.len()
            );
        }
        "secp256k1" => {
            assert!(
                raw_bytes.len() == 33,
                "Secp256k1 compressed public key must be 33 bytes, got {}",
                raw_bytes.len()
            );
            assert!(
                raw_bytes[0] == 0x02 || raw_bytes[0] == 0x03,
                "Secp256k1 compressed key must start with 0x02 or 0x03"
            );
        }
        _ => unreachable!(),
    }

    (key_type.to_string(), raw_bytes)
}

/// Verify wallet signature on-chain using native host functions
/// message_hash: SHA256 hash of the data being signed (32 bytes)
fn verify_wallet_signature(
    wallet_pubkey: &str,
    message_hash: &[u8; 32],
    wallet_signature: &str,
) {
    let (key_type, pubkey_bytes) = parse_wallet_pubkey(wallet_pubkey);
    let sig_bytes = hex::decode(wallet_signature).unwrap_or_else(|_| {
        env::panic_str("Invalid hex encoding in wallet_signature");
    });

    match key_type.as_str() {
        "ed25519" => {
            assert!(
                sig_bytes.len() == 64,
                "Ed25519 signature must be 64 bytes, got {}",
                sig_bytes.len()
            );
            let sig: [u8; 64] = sig_bytes.try_into().unwrap();
            let pk: [u8; 32] = pubkey_bytes.try_into().unwrap();
            // ed25519_verify(signature, message, public_key) -> bool
            let valid = env::ed25519_verify(&sig, message_hash, &pk);
            assert!(valid, "Invalid Ed25519 wallet signature");
        }
        "secp256k1" => {
            // Signature: 64 bytes (r || s) + 1 byte recovery id (v)
            assert!(
                sig_bytes.len() == 65,
                "Secp256k1 signature must be 65 bytes (64 sig + 1 recovery id), got {}",
                sig_bytes.len()
            );
            let sig: [u8; 64] = sig_bytes[..64].try_into().unwrap();
            let v = sig_bytes[64];
            assert!(v <= 1, "Recovery id (v) must be 0 or 1, got {}", v);

            // ecrecover returns uncompressed public key (64 bytes: x || y)
            let recovered = env::ecrecover(message_hash, &sig, v, true)
                .unwrap_or_else(|| env::panic_str("Secp256k1 signature recovery failed"));

            // Compress recovered key and compare with stored compressed key
            let x = &recovered[0..32];
            let y_last_byte = recovered[63];
            let prefix = if y_last_byte % 2 == 0 { 0x02 } else { 0x03 };

            let mut compressed = Vec::with_capacity(33);
            compressed.push(prefix);
            compressed.extend_from_slice(x);

            assert!(
                compressed == pubkey_bytes,
                "Recovered secp256k1 public key does not match wallet_pubkey"
            );
        }
        _ => unreachable!(),
    }
}

#[near_bindgen]
impl Contract {
    /// Store or update wallet policy
    ///
    /// Requires wallet_signature = sign(sha256(encrypted_data)) from the wallet's key.
    /// This proves the caller has API key access to the wallet (keystore signs on behalf).
    /// If entry already exists, caller must be the same controller (owner).
    /// Requires attached storage deposit.
    ///
    /// # Arguments
    /// * `wallet_pubkey` - "ed25519:<hex32>" or "secp256k1:<hex33>"
    /// * `encrypted_data` - Policy encrypted by keystore
    /// * `wallet_signature` - Hex-encoded signature proving API key ownership
    #[payable]
    pub fn store_wallet_policy(
        &mut self,
        wallet_pubkey: String,
        encrypted_data: String,
        wallet_signature: String,
    ) {
        self.assert_not_paused();

        let caller = env::predecessor_account_id();
        let attached_deposit = env::attached_deposit().as_yoctonear();

        // Validate inputs
        assert!(!wallet_pubkey.is_empty(), "wallet_pubkey cannot be empty");
        assert!(!encrypted_data.is_empty(), "encrypted_data cannot be empty");
        assert!(
            encrypted_data.len() <= 100_000,
            "encrypted_data too large (max 100KB)"
        );
        assert!(!wallet_signature.is_empty(), "wallet_signature is required");

        // Parse and validate wallet pubkey
        parse_wallet_pubkey(&wallet_pubkey);

        // Verify wallet signature on-chain
        let mut hasher = Sha256::new();
        hasher.update(encrypted_data.as_bytes());
        let message_hash: [u8; 32] = hasher.finalize().into();

        verify_wallet_signature(&wallet_pubkey, &message_hash, &wallet_signature);

        // Calculate storage cost
        let storage_size = self.calculate_wallet_policy_storage_size(&wallet_pubkey, &encrypted_data);
        let required_deposit = storage_size as u128 * STORAGE_PRICE_PER_BYTE;

        // Check ownership and handle deposit
        let is_new = if let Some(existing) = self.wallet_policies.get(&wallet_pubkey) {
            // Update: caller must be the same controller
            assert!(
                existing.owner == caller,
                "Only the original controller ({}) can update this wallet policy",
                existing.owner
            );

            let total_available = attached_deposit + existing.storage_deposit;
            assert!(
                total_available >= required_deposit,
                "Insufficient deposit for update. Required: {}, available (attached {} + old {}): {}",
                required_deposit,
                attached_deposit,
                existing.storage_deposit,
                total_available
            );

            // Refund excess
            let refund = total_available - required_deposit;
            if refund > 0 {
                near_sdk::Promise::new(caller.clone())
                    .transfer(NearToken::from_yoctonear(refund));
            }
            false
        } else {
            // New entry
            assert!(
                attached_deposit >= required_deposit,
                "Insufficient storage deposit. Required: {} yoctoNEAR, attached: {}",
                required_deposit,
                attached_deposit
            );

            // Refund excess
            if attached_deposit > required_deposit {
                let refund = attached_deposit - required_deposit;
                near_sdk::Promise::new(caller.clone())
                    .transfer(NearToken::from_yoctonear(refund));
            }
            true
        };

        // Store wallet policy entry
        let entry = WalletPolicyEntry {
            owner: caller.clone(),
            encrypted_data,
            frozen: false, // Never frozen on store/update
            updated_at: env::block_timestamp(),
            storage_deposit: required_deposit,
        };

        self.wallet_policies.insert(&wallet_pubkey, &entry);

        // Add to owner index (new entries only — updates keep the same owner)
        if is_new {
            let mut owner_set = self
                .wallet_owner_index
                .get(&caller)
                .unwrap_or_else(|| {
                    UnorderedSet::new(StorageKey::WalletOwnerList {
                        account_id: caller.clone(),
                    })
                });
            owner_set.insert(&wallet_pubkey);
            self.wallet_owner_index.insert(&caller, &owner_set);
        }

        self.emit_system_event(crate::payment::SystemEvent::WalletPolicyUpdated {
            wallet_pubkey,
            owner: caller,
            encrypted_data: entry.encrypted_data.clone(),
            frozen: entry.frozen,
        });
    }

    /// Freeze wallet (controller-only, NO wallet signature required)
    ///
    /// Only the controller (caller == entry.owner) can freeze.
    /// Does not require wallet key signature — enables emergency freeze
    /// even if agent's key is compromised.
    pub fn freeze_wallet(&mut self, wallet_pubkey: String) {
        self.assert_not_paused();

        let caller = env::predecessor_account_id();
        let mut entry = self
            .wallet_policies
            .get(&wallet_pubkey)
            .unwrap_or_else(|| env::panic_str("Wallet policy not found"));

        assert!(
            entry.owner == caller,
            "Only the controller ({}) can freeze this wallet",
            entry.owner
        );

        assert!(!entry.frozen, "Wallet is already frozen");

        entry.frozen = true;
        entry.updated_at = env::block_timestamp();
        self.wallet_policies.insert(&wallet_pubkey, &entry);

        self.emit_system_event(crate::payment::SystemEvent::WalletFrozenChanged {
            wallet_pubkey,
            owner: caller,
            frozen: true,
        });
    }

    /// Unfreeze wallet (controller-only, NO wallet signature required)
    ///
    /// Only the controller (caller == entry.owner) can unfreeze.
    /// If admin_quorum is set in policy, keystore will additionally verify
    /// quorum approval during check-policy.
    pub fn unfreeze_wallet(&mut self, wallet_pubkey: String) {
        self.assert_not_paused();

        let caller = env::predecessor_account_id();
        let mut entry = self
            .wallet_policies
            .get(&wallet_pubkey)
            .unwrap_or_else(|| env::panic_str("Wallet policy not found"));

        assert!(
            entry.owner == caller,
            "Only the controller ({}) can unfreeze this wallet",
            entry.owner
        );

        assert!(entry.frozen, "Wallet is not frozen");

        entry.frozen = false;
        entry.updated_at = env::block_timestamp();
        self.wallet_policies.insert(&wallet_pubkey, &entry);

        self.emit_system_event(crate::payment::SystemEvent::WalletFrozenChanged {
            wallet_pubkey,
            owner: caller,
            frozen: false,
        });
    }

    /// Delete wallet policy and refund storage deposit
    ///
    /// Only the controller can delete.
    pub fn delete_wallet_policy(&mut self, wallet_pubkey: String) {
        let caller = env::predecessor_account_id();
        let entry = self
            .wallet_policies
            .get(&wallet_pubkey)
            .unwrap_or_else(|| env::panic_str("Wallet policy not found"));

        assert!(
            entry.owner == caller,
            "Only the controller ({}) can delete this wallet policy",
            entry.owner
        );

        self.wallet_policies.remove(&wallet_pubkey);

        // Remove from owner index
        if let Some(mut owner_set) = self.wallet_owner_index.get(&caller) {
            owner_set.remove(&wallet_pubkey);
            if owner_set.is_empty() {
                self.wallet_owner_index.remove(&caller);
            } else {
                self.wallet_owner_index.insert(&caller, &owner_set);
            }
        }

        // Refund storage deposit
        if entry.storage_deposit > 0 {
            near_sdk::Promise::new(caller.clone())
                .transfer(NearToken::from_yoctonear(entry.storage_deposit));
        }

        self.emit_system_event(crate::payment::SystemEvent::WalletPolicyDeleted {
            wallet_pubkey,
            owner: caller,
        });
    }
}

// View methods
#[near_bindgen]
impl Contract {
    /// Check if a wallet policy exists (view, no decryption needed)
    /// Used by coordinator for negative cache check (free RPC call)
    pub fn has_wallet_policy(&self, wallet_pubkey: String) -> bool {
        self.wallet_policies.get(&wallet_pubkey).is_some()
    }

    /// Get wallet policy entry (view)
    /// Returns owner, encrypted_data, frozen flag, updated_at
    /// Keystore decrypts encrypted_data for policy rules
    pub fn get_wallet_policy(&self, wallet_pubkey: String) -> Option<WalletPolicyView> {
        self.wallet_policies.get(&wallet_pubkey).map(|entry| {
            WalletPolicyView {
                owner: entry.owner,
                encrypted_data: entry.encrypted_data,
                frozen: entry.frozen,
                updated_at: entry.updated_at,
            }
        })
    }

    /// List all wallet policies owned by an account
    pub fn get_wallet_policies_by_owner(&self, owner: AccountId) -> Vec<WalletPolicyListItem> {
        let Some(owner_set) = self.wallet_owner_index.get(&owner) else {
            return vec![];
        };
        owner_set
            .iter()
            .filter_map(|wallet_pubkey| {
                self.wallet_policies.get(&wallet_pubkey).map(|entry| {
                    WalletPolicyListItem {
                        wallet_pubkey,
                        owner: entry.owner,
                        frozen: entry.frozen,
                        updated_at: entry.updated_at,
                    }
                })
            })
            .collect()
    }

    /// Estimate storage cost for wallet policy (before storing)
    pub fn estimate_wallet_policy_cost(
        &self,
        wallet_pubkey: String,
        encrypted_data: String,
    ) -> near_sdk::json_types::U128 {
        let storage_bytes =
            self.calculate_wallet_policy_storage_size(&wallet_pubkey, &encrypted_data);
        near_sdk::json_types::U128((storage_bytes as u128) * STORAGE_PRICE_PER_BYTE)
    }
}

impl Contract {
    /// Calculate storage size for wallet policy entry
    fn calculate_wallet_policy_storage_size(
        &self,
        wallet_pubkey: &str,
        encrypted_data: &str,
    ) -> u64 {
        const BASE_OVERHEAD: u64 = 40; // LookupMap entry overhead

        let key_size = (4 + wallet_pubkey.len()) as u64; // String with length prefix

        // WalletPolicyEntry fields:
        // owner (AccountId = String): 4 + len
        // encrypted_data (String): 4 + len
        // frozen (bool): 1
        // updated_at (u64): 8
        // storage_deposit (u128): 16
        let owner_estimate = 4 + 64; // AccountId max ~64 chars
        let value_size = (owner_estimate
            + 4
            + encrypted_data.len()
            + 1  // frozen
            + 8  // updated_at
            + 16 // storage_deposit
        ) as u64;

        BASE_OVERHEAD + key_size + value_size
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use near_sdk::test_utils::{accounts, VMContextBuilder};
    use near_sdk::testing_env;

    fn get_context(predecessor: AccountId, attached_deposit: NearToken) -> VMContextBuilder {
        let mut builder = VMContextBuilder::new();
        builder
            .predecessor_account_id(predecessor)
            .attached_deposit(attached_deposit);
        builder
    }

    #[test]
    fn test_parse_wallet_pubkey_ed25519() {
        let hex_key = "a".repeat(64); // 32 bytes
        let wallet_pubkey = format!("ed25519:{}", hex_key);
        let (key_type, raw_bytes) = parse_wallet_pubkey(&wallet_pubkey);
        assert_eq!(key_type, "ed25519");
        assert_eq!(raw_bytes.len(), 32);
    }

    #[test]
    fn test_parse_wallet_pubkey_secp256k1() {
        // Valid compressed secp256k1 key (prefix 02 + 32 bytes)
        let hex_key = format!("02{}", "b".repeat(64));
        let wallet_pubkey = format!("secp256k1:{}", hex_key);
        let (key_type, raw_bytes) = parse_wallet_pubkey(&wallet_pubkey);
        assert_eq!(key_type, "secp256k1");
        assert_eq!(raw_bytes.len(), 33);
    }

    #[test]
    #[should_panic(expected = "Unsupported key type")]
    fn test_parse_invalid_key_type() {
        parse_wallet_pubkey("rsa:abcdef");
    }

    #[test]
    #[should_panic(expected = "Unsupported key type")]
    fn test_parse_account_type_rejected() {
        parse_wallet_pubkey("account:some-uuid");
    }

    #[test]
    #[should_panic(expected = "Ed25519 public key must be 32 bytes")]
    fn test_parse_ed25519_wrong_length() {
        let hex_key = "ab".repeat(16); // 16 bytes, not 32
        parse_wallet_pubkey(&format!("ed25519:{}", hex_key));
    }

    #[test]
    fn test_has_wallet_policy_empty() {
        let owner = accounts(0);
        let context = get_context(owner.clone(), NearToken::from_near(0));
        testing_env!(context.build());

        let contract = Contract::new(owner, None, None, None);
        assert!(!contract.has_wallet_policy("ed25519:aaaa".to_string()));
    }

    #[test]
    fn test_freeze_unfreeze() {
        let owner = accounts(0);
        let controller = accounts(1);

        let context = get_context(owner.clone(), NearToken::from_near(0));
        testing_env!(context.build());

        let mut contract = Contract::new(owner, None, None, None);

        // Manually insert a policy entry for testing (bypassing signature verification)
        let wallet_pubkey = format!("ed25519:{}", "a".repeat(64));
        let entry = WalletPolicyEntry {
            owner: controller.clone(),
            encrypted_data: "test_encrypted".to_string(),
            frozen: false,
            updated_at: 0,
            storage_deposit: 0,
        };
        contract.wallet_policies.insert(&wallet_pubkey, &entry);

        // Freeze
        let context = get_context(controller.clone(), NearToken::from_near(0));
        testing_env!(context.build());
        contract.freeze_wallet(wallet_pubkey.clone());

        let view = contract.get_wallet_policy(wallet_pubkey.clone()).unwrap();
        assert!(view.frozen);

        // Unfreeze
        contract.unfreeze_wallet(wallet_pubkey.clone());
        let view = contract.get_wallet_policy(wallet_pubkey).unwrap();
        assert!(!view.frozen);
    }

    #[test]
    #[should_panic(expected = "Only the controller")]
    fn test_freeze_wrong_controller() {
        let owner = accounts(0);
        let controller = accounts(1);
        let attacker = accounts(2);

        let context = get_context(owner.clone(), NearToken::from_near(0));
        testing_env!(context.build());

        let mut contract = Contract::new(owner, None, None, None);

        let wallet_pubkey = format!("ed25519:{}", "a".repeat(64));
        let entry = WalletPolicyEntry {
            owner: controller,
            encrypted_data: "test".to_string(),
            frozen: false,
            updated_at: 0,
            storage_deposit: 0,
        };
        contract.wallet_policies.insert(&wallet_pubkey, &entry);

        // Attacker tries to freeze
        let context = get_context(attacker, NearToken::from_near(0));
        testing_env!(context.build());
        contract.freeze_wallet(wallet_pubkey);
    }
}
