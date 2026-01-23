//! Payment Key Top-Up via NEP-141 ft_transfer_call
//!
//! This module handles topping up Payment Keys with stablecoins (stablecoins).
//! Uses yield/resume mechanism similar to request_execution.

use crate::*;
use near_sdk::serde_json::json;
use near_sdk::{env, log, near_bindgen, AccountId, Gas, GasWeight};

/// Minimum top-up amount: $1.00 (1_000_000 for USDT with 6 decimals)
pub const MIN_TOP_UP_AMOUNT: u128 = 1_000_000;

/// Gas for on_top_up_response callback
pub const TOP_UP_CALLBACK_GAS: Gas = Gas::from_tgas(30);

/// Action for ft_on_transfer msg field
#[derive(Clone, Debug)]
#[near(serializers = [json])]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum FtTransferAction {
    /// Top up a Payment Key balance
    TopUpPaymentKey { nonce: u32 },
    /// Deposit stablecoin to user's balance (for attached_usd payments)
    DepositBalance,
}

/// Result of top-up operation (sent via yield/resume)
#[derive(Clone, Debug)]
#[near(serializers = [borsh, json])]
pub enum TopUpResult {
    /// Success - contains new encrypted secret data
    Success { new_encrypted_data: String },
    /// Error - contains error message
    Error { message: String },
}

/// Result of delete operation (sent via yield/resume)
#[derive(Clone, Debug)]
#[near(serializers = [borsh, json])]
pub enum DeletePaymentKeyResult {
    /// Success - key deleted from coordinator PostgreSQL
    Success,
    /// Error - contains error message
    Error { message: String },
}

/// Gas for on_delete_payment_key_response callback
pub const DELETE_CALLBACK_GAS: Gas = Gas::from_tgas(30);

/// System event for workers to process
#[derive(Clone, Debug)]
#[near(serializers = [json])]
pub enum SystemEvent {
    /// Payment Key top-up request
    TopUpPaymentKey {
        data_id: CryptoHash,
        owner: AccountId,
        nonce: u32,
        amount: U128,
        encrypted_data: String, // current encrypted secret (base64)
    },
    /// Payment Key delete request
    DeletePaymentKey {
        data_id: CryptoHash,
        owner: AccountId,
        nonce: u32,
    },
}

#[near_bindgen]
impl Contract {
    /// NEP-141 callback for receiving fungible tokens
    ///
    /// msg format: {"action": "top_up_payment_key", "nonce": 0}
    ///
    /// Uses yield/resume: waits for worker to update the encrypted secret
    /// with new balance, then returns 0 (accept) or amount (refund)
    pub fn ft_on_transfer(
        &mut self,
        sender_id: AccountId,
        amount: U128,
        msg: String,
    ) {
        let token_contract = env::predecessor_account_id();

        // Check that payment token is configured
        let configured_token = self.payment_token_contract.as_ref()
            .expect("Payment token contract not configured");

        // Check that token matches configured contract
        assert!(
            &token_contract == configured_token,
            "Invalid token contract. Expected: {}, got: {}",
            configured_token,
            token_contract
        );

        // Parse action from msg
        let action: FtTransferAction = serde_json::from_str(&msg)
            .expect("Invalid msg format. Expected: {\"action\": \"top_up_payment_key\", \"nonce\": 0}");

        match action {
            FtTransferAction::TopUpPaymentKey { nonce } => {
                self.handle_top_up(sender_id, amount, nonce)
            }
            FtTransferAction::DepositBalance => {
                self.handle_deposit_balance(sender_id, amount)
            }
        }
    }

    /// Handle stablecoin deposit to user's balance
    /// Used for attached_usd payments to project developers
    fn handle_deposit_balance(&mut self, sender_id: AccountId, amount: U128) {
        // Add to user's stablecoin balance
        let current = self.user_stablecoin_balances.get(&sender_id).unwrap_or(0);
        self.user_stablecoin_balances.insert(&sender_id, &(current + amount.0));

        log!(
            "Deposited {} stablecoin to {} (new balance: {})",
            amount.0,
            sender_id,
            current + amount.0
        );
    }

    /// Handle Payment Key top-up with yield/resume
    fn handle_top_up(
        &mut self,
        sender_id: AccountId,
        amount: U128,
        nonce: u32,
    ) {
        // Check minimum amount
        assert!(
            amount.0 >= MIN_TOP_UP_AMOUNT,
            "Minimum top-up is $1.00 ({} minimal units)",
            MIN_TOP_UP_AMOUNT
        );

        // Build secret key for Payment Key
        let secret_key = SecretKey {
            accessor: SecretAccessor::System(SystemSecretType::PaymentKey),
            profile: nonce.to_string(),
            owner: sender_id.clone(),
        };

        // Get existing secret
        let secret_profile = self.secrets_storage.get(&secret_key)
            .expect("Payment key not found. Create it first with store_secrets()");

        // Create callback data
        let callback_data = json!({
            "owner": sender_id,
            "nonce": nonce,
            "amount": amount,
        });

        // Create yield - wait for worker to re-encrypt secret with new balance
        let promise_idx = env::promise_yield_create(
            "on_top_up_response",
            &callback_data.to_string().into_bytes(),
            TOP_UP_CALLBACK_GAS,
            GasWeight(1),
            DATA_ID_REGISTER,
        );

        // Get data_id for resume
        let data_id: CryptoHash = env::read_register(DATA_ID_REGISTER)
            .expect("Failed to read data_id")
            .try_into()
            .expect("Invalid data_id");

        // Emit event for worker to process
        self.emit_system_event(SystemEvent::TopUpPaymentKey {
            data_id,
            owner: sender_id.clone(),
            nonce,
            amount,
            encrypted_data: secret_profile.encrypted_secrets,
        });

        log!(
            "TopUp requested: owner={}, nonce={}, amount={}",
            sender_id,
            nonce,
            amount.0
        );

        // Return the yield promise (will be resumed by worker)
        env::promise_return(promise_idx);
    }

    /// Callback after worker processes top-up via yield/resume
    /// Returns U128 - amount to refund (0 = accept all, amount = refund all)
    #[private]
    pub fn on_top_up_response(
        &mut self,
        owner: AccountId,
        nonce: u32,
        amount: U128, // amount to refund on error
        #[callback_result] result: Result<TopUpResult, PromiseError>,
    ) -> U128 {
        match result {
            Ok(TopUpResult::Success { new_encrypted_data }) => {
                // Build secret key
                let secret_key = SecretKey {
                    accessor: SecretAccessor::System(SystemSecretType::PaymentKey),
                    profile: nonce.to_string(),
                    owner: owner.clone(),
                };

                // Get existing profile to preserve metadata
                if let Some(mut profile) = self.secrets_storage.get(&secret_key) {
                    log!(
                        "Updating Payment Key secret: owner={}, nonce={}, old_len={}, new_len={}",
                        owner,
                        nonce,
                        profile.encrypted_secrets.len(),
                        new_encrypted_data.len()
                    );

                    // Update encrypted data with new balance
                    profile.encrypted_secrets = new_encrypted_data;
                    profile.updated_at = env::block_timestamp();

                    // Save updated profile
                    self.secrets_storage.insert(&secret_key, &profile);

                    log!(
                        "Payment key topped up: owner={}, nonce={}, amount={}",
                        owner,
                        nonce,
                        amount.0
                    );

                    U128(0) // Accept all tokens
                } else {
                    log!("Payment key not found during callback: owner={}, nonce={}", owner, nonce);
                    amount // Refund all tokens
                }
            }
            Ok(TopUpResult::Error { message }) => {
                log!(
                    "TopUp failed: owner={}, nonce={}, error={}",
                    owner,
                    nonce,
                    message
                );
                amount // Refund all tokens
            }
            Err(_) => {
                log!(
                    "TopUp callback timeout: owner={}, nonce={}",
                    owner,
                    nonce
                );
                amount // Refund all tokens
            }
        }
    }

    /// Resume a TopUp yield promise with the result
    ///
    /// Called by the worker (operator) after processing the TopUp:
    /// 1. Worker decrypts current Payment Key data
    /// 2. Worker adds topup amount to balance
    /// 3. Worker re-encrypts data
    /// 4. Worker calls this method to resume the yield
    ///
    /// # Arguments
    /// * `data_id` - CryptoHash from the yield promise (hex encoded)
    /// * `result` - TopUpResult with new encrypted data or error
    pub fn resume_topup(&mut self, data_id: String, result: TopUpResult) {
        // Only operator can resume
        assert!(
            env::predecessor_account_id() == self.operator_id,
            "Only operator can resume topup"
        );

        // Decode data_id from hex
        let data_id_bytes = hex::decode(&data_id)
            .expect("Invalid data_id hex");
        let data_id_hash: CryptoHash = data_id_bytes
            .try_into()
            .expect("Invalid data_id length");

        // Serialize result for yield resume (callback_result expects JSON)
        let result_bytes = serde_json::to_vec(&result)
            .expect("Failed to serialize TopUpResult");

        // Resume the yield promise
        let success = env::promise_yield_resume(&data_id_hash, &result_bytes);

        if success {
            log!("TopUp yield resumed: data_id={}", data_id);
        } else {
            log!("TopUp yield resume failed (timeout?): data_id={}", data_id);
        }
    }

    // =========================================================================
    // Delete Payment Key (with yield/resume)
    // =========================================================================

    /// Delete a Payment Key using yield/resume mechanism
    ///
    /// Flow:
    /// 1. User calls delete_payment_key(nonce)
    /// 2. Contract emits DeletePaymentKey event
    /// 3. Worker receives event, deletes key from coordinator PostgreSQL
    /// 4. Worker calls resume_delete_payment_key with Success
    /// 5. Contract callback deletes secret from storage and refunds deposit
    ///
    /// # Arguments
    /// * `nonce` - Payment Key nonce to delete
    #[payable]
    pub fn delete_payment_key(&mut self, nonce: u32) {
        let caller = env::predecessor_account_id();

        // Require 1 yoctoNEAR for security (prevent accidental calls)
        assert!(
            env::attached_deposit().as_yoctonear() >= 1,
            "Requires attached deposit of at least 1 yoctoNEAR"
        );

        // Build secret key for Payment Key
        let secret_key = SecretKey {
            accessor: SecretAccessor::System(SystemSecretType::PaymentKey),
            profile: nonce.to_string(),
            owner: caller.clone(),
        };

        // Verify Payment Key exists
        assert!(
            self.secrets_storage.get(&secret_key).is_some(),
            "Payment key not found: owner={}, nonce={}",
            caller,
            nonce
        );

        // Create callback data
        let callback_data = json!({
            "owner": caller,
            "nonce": nonce,
        });

        // Create yield - wait for worker to delete from coordinator
        let promise_idx = env::promise_yield_create(
            "on_delete_payment_key_response",
            &callback_data.to_string().into_bytes(),
            DELETE_CALLBACK_GAS,
            GasWeight(1),
            DATA_ID_REGISTER,
        );

        // Get data_id for resume
        let data_id: CryptoHash = env::read_register(DATA_ID_REGISTER)
            .expect("Failed to read data_id")
            .try_into()
            .expect("Invalid data_id");

        // Emit event for worker to process
        self.emit_system_event(SystemEvent::DeletePaymentKey {
            data_id,
            owner: caller.clone(),
            nonce,
        });

        log!(
            "DeletePaymentKey requested: owner={}, nonce={}",
            caller,
            nonce
        );

        // Return the yield promise (will be resumed by worker)
        env::promise_return(promise_idx);
    }

    /// Callback after worker processes delete via yield/resume
    #[private]
    pub fn on_delete_payment_key_response(
        &mut self,
        owner: AccountId,
        nonce: u32,
        #[callback_result] result: Result<DeletePaymentKeyResult, PromiseError>,
    ) {
        match result {
            Ok(DeletePaymentKeyResult::Success) => {
                // Build secret key
                let secret_key = SecretKey {
                    accessor: SecretAccessor::System(SystemSecretType::PaymentKey),
                    profile: nonce.to_string(),
                    owner: owner.clone(),
                };

                log!(
                    "Deleting Payment Key secret from storage: owner={}, nonce={}",
                    owner,
                    nonce
                );

                // Delete secret from contract storage (refunds storage deposit)
                self.delete_secrets_internal(secret_key, &owner);

                log!(
                    "Payment key deleted: owner={}, nonce={}",
                    owner,
                    nonce
                );
            }
            Ok(DeletePaymentKeyResult::Error { message }) => {
                log!(
                    "DeletePaymentKey failed: owner={}, nonce={}, error={}",
                    owner,
                    nonce,
                    message
                );
                // Don't delete on error - key remains valid
            }
            Err(_) => {
                log!(
                    "DeletePaymentKey callback timeout: owner={}, nonce={}",
                    owner,
                    nonce
                );
                // Don't delete on timeout - key remains valid
            }
        }
    }

    /// Resume a DeletePaymentKey yield promise with the result
    ///
    /// Called by the worker (operator) after deleting the key from coordinator:
    /// 1. Worker receives DeletePaymentKey event
    /// 2. Worker calls POST /payment-keys/delete on coordinator
    /// 3. Worker calls this method to resume the yield
    ///
    /// # Arguments
    /// * `data_id` - CryptoHash from the yield promise (hex encoded)
    /// * `result` - DeletePaymentKeyResult (Success or Error)
    pub fn resume_delete_payment_key(&mut self, data_id: String, result: DeletePaymentKeyResult) {
        // Only operator can resume
        assert!(
            env::predecessor_account_id() == self.operator_id,
            "Only operator can resume delete_payment_key"
        );

        // Decode data_id from hex
        let data_id_bytes = hex::decode(&data_id)
            .expect("Invalid data_id hex");
        let data_id_hash: CryptoHash = data_id_bytes
            .try_into()
            .expect("Invalid data_id length");

        // Serialize result for yield resume (callback_result expects JSON)
        let result_bytes = serde_json::to_vec(&result)
            .expect("Failed to serialize DeletePaymentKeyResult");

        // Resume the yield promise
        let success = env::promise_yield_resume(&data_id_hash, &result_bytes);

        if success {
            log!("DeletePaymentKey yield resumed: data_id={}", data_id);
        } else {
            log!("DeletePaymentKey yield resume failed (timeout?): data_id={}", data_id);
        }
    }

    /// Emit system event for workers
    fn emit_system_event(&self, event: SystemEvent) {
        let event_json = json!({
            "standard": self.event_standard,
            "version": self.event_version,
            "event": "system_event",
            "data": [event]
        });

        log!("EVENT_JSON:{}", event_json.to_string());
    }
}
