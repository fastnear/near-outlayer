use crate::*;

impl Contract {
    pub(crate) fn assert_owner(&self) {
        assert_eq!(
            env::predecessor_account_id(),
            self.owner_id,
            "Only owner can call this method"
        );
    }
}

#[near_bindgen]
impl Contract {
    /// Set new owner (only current owner can call)
    pub fn set_owner(&mut self, new_owner_id: AccountId) {
        self.assert_owner();
        let old_owner = self.owner_id.clone();
        self.owner_id = new_owner_id.clone();

        log!("Owner changed from {} to {}", old_owner, new_owner_id);
    }

    /// Set new operator (only owner can call)
    pub fn set_operator(&mut self, new_operator_id: AccountId) {
        self.assert_owner();
        let old_operator = self.operator_id.clone();
        self.operator_id = new_operator_id.clone();

        log!(
            "Operator changed from {} to {}",
            old_operator,
            new_operator_id
        );
    }

    /// Pause/unpause contract (only owner can call)
    pub fn set_paused(&mut self, paused: bool) {
        self.assert_owner();
        self.paused = paused;

        log!("Contract {}", if paused { "paused" } else { "unpaused" });
    }

    /// Update pricing (only owner can call)
    pub fn set_pricing(
        &mut self,
        base_fee: Option<U128>,
        per_instruction_fee: Option<U128>,
        per_ms_fee: Option<U128>,
    ) {
        self.assert_owner();

        if let Some(fee) = base_fee {
            self.base_fee = fee.0;
            log!("Base fee updated to {}", fee.0);
        }
        if let Some(fee) = per_instruction_fee {
            self.per_instruction_fee = fee.0;
            log!("Per instruction fee updated to {}", fee.0);
        }
        if let Some(fee) = per_ms_fee {
            self.per_ms_fee = fee.0;
            log!("Per millisecond fee updated to {}", fee.0);
        }
    }

    /// Emergency function to cancel pending execution and refund user (only owner can call)
    pub fn emergency_cancel_execution(&mut self, request_id: u64) {
        self.assert_owner();

        if let Some(request) = self.pending_requests.remove(&request_id) {
            // Refund payment to user
            near_sdk::Promise::new(request.sender_id.clone())
                .transfer(NearToken::from_yoctonear(request.payment));

            log!(
                "Emergency cancelled execution {} and refunded {} yoctoNEAR to {}",
                request_id,
                request.payment,
                request.sender_id
            );
        } else {
            env::panic_str("Execution request not found");
        }
    }

    /// Set keystore account ID and public key (only owner or keystore can call)
    ///
    /// This method allows the keystore worker to register itself with the contract.
    /// The public key is stored on-chain so users can encrypt secrets before calling request_execution.
    ///
    /// # Arguments
    /// * `pubkey` - Public key in hex format (64 chars) or NEAR format (ed25519:base58)
    ///
    /// # Examples
    /// - Hex: "53965a1377f93aa3ed819339a973d9438410e33bf43b3efc2b965a96a9af7595"
    /// - NEAR: "ed25519:6dHp2jhzeGPgMTXACkHYNE4nYo8smp9vZoDVDyEGnoqe"
    pub fn set_keystore_pubkey(&mut self, pubkey: String) {
        let caller = env::predecessor_account_id();

        // Allow owner to set any keystore account
        // Or allow keystore account itself to update its pubkey
        let is_authorized = caller == self.owner_id
            || self
                .keystore_account_id
                .as_ref()
                .map(|k| caller == *k)
                .unwrap_or(false);

        assert!(is_authorized, "Only owner or keystore can set pubkey");

        // Parse and normalize to hex format
        let pubkey_hex = if pubkey.starts_with("ed25519:") {
            // NEAR base58 format - convert to hex
            let base58_part = pubkey.strip_prefix("ed25519:").unwrap();

            // Decode base58 to bytes
            let bytes = bs58::decode(base58_part)
                .into_vec()
                .expect("Invalid base58 encoding");

            assert_eq!(bytes.len(), 32, "Public key must be 32 bytes");

            // Convert to hex
            hex::encode(bytes)
        } else if pubkey.len() == 64 && pubkey.chars().all(|c| c.is_ascii_hexdigit()) {
            // Already in hex format
            pubkey
        } else {
            env::panic_str("Public key must be either 64 hex characters or NEAR format (ed25519:base58)")
        };

        // If keystore account not set yet, set it to caller
        if self.keystore_account_id.is_none() {
            self.keystore_account_id = Some(caller.clone());
            log!("Keystore account set to {}", caller);
        }

        self.keystore_pubkey = Some(pubkey_hex.clone());
        log!("Keystore public key updated: {}", pubkey_hex);
    }

    /// Set keystore account ID (only owner can call)
    pub fn set_keystore_account(&mut self, keystore_account_id: AccountId) {
        self.assert_owner();
        self.keystore_account_id = Some(keystore_account_id.clone());
        log!("Keystore account set to {}", keystore_account_id);
    }
}
