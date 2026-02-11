//! Payment Key Top-Up via NEP-141 ft_transfer_call
//!
//! This module handles topping up Payment Keys with stablecoins (stablecoins).
//! Uses yield/resume mechanism similar to request_execution.
//!
//! Also supports top-up with NEAR or other tokens via swap to USDC.

use crate::*;
use near_sdk::serde_json::json;
use near_sdk::{env, log, near_bindgen, AccountId, Gas, GasWeight, NearToken, Promise};

/// Minimum top-up amount: $0.01 (10_000 for USDT with 6 decimals)
pub const MIN_TOP_UP_AMOUNT: u128 = 10_000;

/// Minimum NEAR deposit: 0.01 NEAR
pub const MIN_NEAR_DEPOSIT: u128 = 10_000_000_000_000_000_000_000; // 0.01 NEAR

/// wNEAR contract on mainnet
pub const WNEAR_CONTRACT: &str = "wrap.near";

/// Cost for OutLayer execution (covers base_fee + 1 yoctoNEAR for ft_transfer)
/// Must be >= base_fee + 1. Currently set to 0.01 NEAR to have margin.
pub const EXECUTION_COST: u128 = 10_000_000_000_000_000_000_000; // 0.01 NEAR

/// Gas for wrap.near calls
pub const WRAP_GAS: Gas = Gas::from_tgas(10);

/// Gas for ft_transfer calls
pub const FT_TRANSFER_GAS: Gas = Gas::from_tgas(30);

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
            "Minimum top-up is $0.01 ({} minimal units)",
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
    /// pub(crate) to allow calling from secrets.rs for PaymentKey creation
    pub(crate) fn emit_system_event(&self, event: SystemEvent) {
        let event_json = json!({
            "standard": self.event_standard,
            "version": self.event_version,
            "event": "system_event",
            "data": [event]
        });

        log!("EVENT_JSON:{}", event_json.to_string());
    }

    // =========================================================================
    // Top-up Payment Key with NEAR (swapped to USDC via Intents)
    // =========================================================================
    //
    // MAINNET ONLY: NEAR Intents protocol is only available on mainnet.
    // These methods will fail on testnet because:
    // - wrap.near doesn't exist on testnet (use wrap.testnet)
    // - v1.publishintent.near doesn't exist on testnet
    // - Intents API (api.defuse.org) only supports mainnet
    //
    // UI should hide "Top Up with NEAR" button on testnet.
    // =========================================================================

    /// Top up a Payment Key with NEAR (MAINNET ONLY)
    ///
    /// This is a convenience wrapper that:
    /// 1. Wraps NEAR to wNEAR
    /// 2. Transfers wNEAR to swap_contract_id
    /// 3. Calls WASI to swap wNEAR -> USDC via Intents
    ///
    /// The token will be swapped to USDC via NEAR Intents protocol.
    /// Minimum deposit: 0.1 NEAR (after subtracting execution cost).
    ///
    /// # Arguments
    /// * `nonce` - Payment Key nonce to top up (must already exist)
    /// * `swap_contract_id` - Account that will execute the swap (e.g., "v1.publishintent.near")
    ///
    /// # Panics
    /// * Payment key not found
    /// * Deposit below minimum (0.1 NEAR + execution cost)
    ///
    /// # Note
    /// This method only works on mainnet. NEAR Intents are not available on testnet.
    #[payable]
    pub fn top_up_payment_key_with_near(
        &mut self,
        nonce: u32,
        swap_contract_id: AccountId,
    ) -> Promise {
        self.assert_not_paused();

        let caller = env::predecessor_account_id();
        let deposit = env::attached_deposit();

        // Reserve NEAR for execution cost
        let wrap_amount = deposit
            .as_yoctonear()
            .saturating_sub(EXECUTION_COST);

        // Check minimum deposit (after subtracting execution cost)
        assert!(
            wrap_amount >= MIN_NEAR_DEPOSIT,
            "Minimum deposit is {} yoctoNEAR (0.01 NEAR) + {} yoctoNEAR (execution cost), got {} yoctoNEAR",
            MIN_NEAR_DEPOSIT,
            EXECUTION_COST,
            deposit.as_yoctonear()
        );

        // Verify payment key exists
        let secret_key = SecretKey {
            accessor: SecretAccessor::System(SystemSecretType::PaymentKey),
            profile: nonce.to_string(),
            owner: caller.clone(),
        };

        assert!(
            self.secrets_storage.get(&secret_key).is_some(),
            "Payment key not found. Create it first with store_secrets()"
        );

        // Verify we have enough reserved for ft_transfer (1 yocto) + base_fee
        let ft_transfer_deposit: u128 = 1;
        assert!(
            EXECUTION_COST >= self.base_fee + ft_transfer_deposit,
            "EXECUTION_COST ({}) must cover base_fee ({}) + ft_transfer deposit (1)",
            EXECUTION_COST,
            self.base_fee
        );

        log!(
            "TopUpWithNear: owner={}, nonce={}, wrap_amount={}, swap_contract={}",
            caller,
            nonce,
            wrap_amount,
            swap_contract_id
        );

        let wnear_contract: AccountId = WNEAR_CONTRACT.parse().unwrap();

        // Step 1: Wrap NEAR to wNEAR (wrap_amount, keep EXECUTION_COST for later)
        // Step 2: Transfer wNEAR to swap_contract_id
        // Step 3: Call request_execution for payment-keys-with-intents WASI
        Promise::new(wnear_contract.clone())
            .function_call(
                "near_deposit".to_string(),
                vec![],
                NearToken::from_yoctonear(wrap_amount),
                WRAP_GAS,
            )
            .then(
                Promise::new(wnear_contract)
                    .function_call(
                        "ft_transfer".to_string(),
                        json!({
                            "receiver_id": swap_contract_id,
                            "amount": wrap_amount.to_string(),
                        })
                        .to_string()
                        .into_bytes(),
                        NearToken::from_yoctonear(1), // 1 yoctoNEAR for ft_transfer
                        FT_TRANSFER_GAS,
                    ),
            )
            .then(self.internal_request_token_swap(
                caller,
                nonce,
                WNEAR_CONTRACT.to_string(),
                wrap_amount.to_string(),
                swap_contract_id.to_string(),
            ))
    }

    /// Top up a Payment Key with any whitelisted token (MAINNET ONLY)
    ///
    /// The token will be swapped to USDC via NEAR Intents protocol.
    /// Whitelist is maintained in the payment-keys-with-intents WASI.
    ///
    /// # Prerequisites
    /// The token must already be transferred to swap_contract_id.
    /// This can be done via ft_transfer before calling this method.
    ///
    /// # Arguments
    /// * `nonce` - Payment Key nonce to top up (must already exist)
    /// * `token_id` - Token contract address (e.g., "wrap.near")
    /// * `amount` - Token amount in minimal units
    /// * `swap_contract_id` - Account that will execute the swap
    ///
    /// # Panics
    /// * Payment key not found
    /// * Token not in whitelist (will fail in WASI)
    ///
    /// # Note
    /// This method only works on mainnet. NEAR Intents are not available on testnet.
    pub fn top_up_payment_key_with_token(
        &mut self,
        nonce: u32,
        token_id: AccountId,
        amount: U128,
        swap_contract_id: AccountId,
    ) -> Promise {
        self.assert_not_paused();

        let caller = env::predecessor_account_id();

        // Verify payment key exists
        let secret_key = SecretKey {
            accessor: SecretAccessor::System(SystemSecretType::PaymentKey),
            profile: nonce.to_string(),
            owner: caller.clone(),
        };

        assert!(
            self.secrets_storage.get(&secret_key).is_some(),
            "Payment key not found. Create it first with store_secrets()"
        );

        log!(
            "TopUpWithToken: owner={}, nonce={}, token={}, amount={}, swap_contract={}",
            caller,
            nonce,
            token_id,
            amount.0,
            swap_contract_id
        );

        // Token should already be at the swap contract
        // Call request_execution for payment-keys-with-intents WASI
        self.internal_request_token_swap(
            caller,
            nonce,
            token_id.to_string(),
            amount.0.to_string(),
            swap_contract_id.to_string(),
        )
    }

    /// Internal: Request execution of payment-keys-with-intents WASI
    fn internal_request_token_swap(
        &self,
        owner: AccountId,
        nonce: u32,
        token_id: String,
        amount: String,
        swap_contract_id: String,
    ) -> Promise {
        // Build input for the WASI
        let input_data = json!({
            "owner": owner,
            "nonce": nonce,
            "token_id": token_id,
            "amount": amount,
            "swap_contract_id": swap_contract_id,
        })
        .to_string();

        // Build execution source - project reference
        let source = json!({
            "Project": {
                "project_id": "publishintent.near/payment-keys-with-intents",
                "version_key": null
            }
        });

        // Call request_execution on ourselves
        // Note: This is an internal call, so we use env::current_account_id()
        // Use function_call_weight to give ALL remaining gas to this call
        Promise::new(env::current_account_id()).function_call_weight(
            "request_execution".to_string(),
            json!({
                "source": source,
                "resource_limits": {
                    "max_instructions": 10_000_000_000_u64, // 10B instructions
                    "max_memory_mb": 256_u32,
                    "max_execution_seconds": 120_u64, // 2 minutes for swap
                },
                "input_data": input_data,
                // Secrets owner for swap contract private key (SWAP_CONTRACT_PRIVATE_KEY)
                // This account must have stored secrets with profile "intents-swap"
                "secrets_ref": {
                    "profile": "intents-swap",
                    "account_id": "publishintent.near", // Hardcoded: secrets owner for intents swap
                },
                "response_format": "Json",
                "payer_account_id": null,
                "params": null,
            })
            .to_string()
            .into_bytes(),
            NearToken::from_yoctonear(15000000000000000000000), // Pay base fee for execution
            Gas::from_tgas(0), // minimum gas (will get all remaining)
            GasWeight(1),      // weight = 1, gets all remaining gas
        )
    }
}
