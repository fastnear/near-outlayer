//! TEE Registration Module
//!
//! Handles keystore registration with DAO contract:
//! 1. Generate or load NEAR keypair
//! 2. Generate TEE attestation
//! 3. Submit registration to DAO
//! 4. Wait for DAO approval

use anyhow::{Context, Result};
use near_crypto::{InMemorySigner, PublicKey, SecretKey};
use near_jsonrpc_client::{methods, JsonRpcClient};
use near_jsonrpc_primitives::types::query::QueryResponseKind;
use near_primitives::transaction::{Action, FunctionCallAction, Transaction, TransactionV0};
use near_primitives::types::{AccountId, BlockReference, Finality};
use near_primitives::views::{QueryRequest, CallResult, FinalExecutionStatus};
use serde_json::json;
use std::fs;
use std::path::PathBuf;
use std::time::Duration;
use tokio::time::sleep;
use tracing::{info, warn, error};

/// Keystore registration client
pub struct RegistrationClient {
    /// NEAR RPC client
    rpc_client: JsonRpcClient,

    /// DAO contract ID
    dao_contract_id: AccountId,

    /// Init account for gas payment
    init_signer: InMemorySigner,

    /// Path to store keypair
    keypair_path: PathBuf,
}

impl RegistrationClient {
    /// Create new registration client
    pub fn new(
        near_rpc_url: String,
        dao_contract_id: AccountId,
        init_account_id: AccountId,
        init_secret_key: SecretKey,
    ) -> Result<Self> {
        let rpc_client = JsonRpcClient::connect(&near_rpc_url);

        let init_signer = InMemorySigner::from_secret_key(
            init_account_id,
            init_secret_key,
        );

        let keypair_path = Self::get_keypair_path();

        Ok(Self {
            rpc_client,
            dao_contract_id,
            init_signer,
            keypair_path,
        })
    }

    /// Get standard path for keypair storage
    fn get_keypair_path() -> PathBuf {
        let mut path = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
        path.push(".near-credentials");
        path.push("keystore-keypair.json");
        path
    }

    /// Load or generate keystore keypair
    ///
    /// In TEE mode: Always generates new keypair in memory (never saves to disk)
    /// In non-TEE mode: Loads from disk if exists, otherwise generates and saves
    pub fn load_or_generate_keypair(&self, is_tee_mode: bool) -> Result<(PublicKey, SecretKey)> {
        if is_tee_mode {
            // TEE MODE: Always generate in memory, NEVER save to disk
            info!("🔐 TEE Mode: Generating ephemeral keypair in memory (not saved to disk)");
            let (public_key, secret_key) = self.generate_keypair()?;
            info!("✅ TEE keypair generated: {} (exists only in memory)", public_key);
            Ok((public_key, secret_key))
        } else {
            // Non-TEE mode: Can use persistent storage
            if self.keypair_path.exists() {
                info!("📂 Loading existing keystore keypair from: {}", self.keypair_path.display());
                self.load_keypair()
            } else {
                info!("🔑 Generating new keystore keypair...");
                let (public_key, secret_key) = self.generate_keypair()?;
                self.save_keypair(&public_key, &secret_key)?;
                Ok((public_key, secret_key))
            }
        }
    }

    /// Generate new ED25519 keypair
    fn generate_keypair(&self) -> Result<(PublicKey, SecretKey)> {
        use ed25519_dalek::SigningKey;
        use rand::rngs::OsRng;

        let signing_key = SigningKey::generate(&mut OsRng);
        let secret_bytes = signing_key.to_bytes();
        let public_bytes = signing_key.verifying_key().to_bytes();

        // NEAR ED25519SecretKey requires 64 bytes: [secret_key (32 bytes)][public_key (32 bytes)]
        let mut keypair_bytes = [0u8; 64];
        keypair_bytes[..32].copy_from_slice(&secret_bytes);
        keypair_bytes[32..].copy_from_slice(&public_bytes);

        let secret_key = SecretKey::ED25519(near_crypto::ED25519SecretKey(keypair_bytes));
        let public_key = secret_key.public_key();

        info!("✅ Generated new keypair: {}", public_key);

        Ok((public_key, secret_key))
    }

    /// Save keypair to file
    fn save_keypair(&self, public_key: &PublicKey, secret_key: &SecretKey) -> Result<()> {
        // Create directory if it doesn't exist
        if let Some(parent) = self.keypair_path.parent() {
            fs::create_dir_all(parent)?;
        }

        // Save as JSON
        let keypair_json = json!({
            "public_key": public_key.to_string(),
            "private_key": secret_key.to_string(),
        });

        fs::write(&self.keypair_path, serde_json::to_string_pretty(&keypair_json)?)
            .context("Failed to save keypair")?;

        info!("💾 Saved keypair to: {}", self.keypair_path.display());
        Ok(())
    }

    /// Load keypair from file
    fn load_keypair(&self) -> Result<(PublicKey, SecretKey)> {
        let content = fs::read_to_string(&self.keypair_path)
            .context("Failed to read keypair file")?;

        let keypair_json: serde_json::Value = serde_json::from_str(&content)
            .context("Failed to parse keypair JSON")?;

        let public_key_str = keypair_json["public_key"]
            .as_str()
            .context("Missing public_key in keypair file")?;

        let private_key_str = keypair_json["private_key"]
            .as_str()
            .context("Missing private_key in keypair file")?;

        let public_key = public_key_str.parse()
            .context("Invalid public key format")?;

        let secret_key = private_key_str.parse()
            .context("Invalid private key format")?;

        Ok((public_key, secret_key))
    }


    /// Submit registration to DAO contract
    pub async fn submit_registration(
        &self,
        public_key: PublicKey,
        tdx_quote_hex: String,
    ) -> Result<u64> {
        info!("📤 Submitting keystore registration to DAO contract");

        // Prepare function call
        let args = json!({
            "public_key": public_key.to_string(),
            "tdx_quote_hex": tdx_quote_hex,
        });

        // Convert to JSON string first, then to bytes (NEAR expects JSON text, not MessagePack)
        let args_json = serde_json::to_string(&args)
            .context("Failed to serialize args to JSON")?;

        // Debug logging if LOG_MASTER_KEY_HASH is set
        if std::env::var("LOG_MASTER_KEY_HASH").unwrap_or_default() == "true" {
            info!("🔍 DEBUG: Registration transaction details:");
            info!("   Contract ID: {}", self.dao_contract_id);
            info!("   Method name: submit_keystore_registration");
            info!("   Signer account: {}", self.init_signer.account_id);
            info!("   Signer public key: {}", self.init_signer.public_key);
            info!("   Args size: {} bytes", args_json.len());
            info!("   Args JSON (first 500 chars): {}",
                if args_json.len() > 500 { &args_json[..500] } else { &args_json });
            info!("   Public key in args: {}", public_key.to_string());
            info!("   TDX quote hex length: {} chars", tdx_quote_hex.len());
        }

        // Get current nonce
        let access_key_query = methods::query::RpcQueryRequest {
            block_reference: BlockReference::Finality(Finality::Final),
            request: QueryRequest::ViewAccessKey {
                account_id: self.init_signer.account_id.clone(),
                public_key: self.init_signer.public_key.clone(),
            },
        };

        // Debug logging for access key query
        if std::env::var("LOG_MASTER_KEY_HASH").unwrap_or_default() == "true" {
            info!("🔍 DEBUG: Querying access key for transaction nonce");
            info!("   Account: {}", self.init_signer.account_id);
            info!("   Public key: {}", self.init_signer.public_key);
        }

        let access_key_response = match self.rpc_client.call(access_key_query).await {
            Ok(response) => response,
            Err(e) => {
                if std::env::var("LOG_MASTER_KEY_HASH").unwrap_or_default() == "true" {
                    warn!("🔍 DEBUG: Failed to query access key!");
                    warn!("   Error: {:?}", e);
                    warn!("   This might mean:");
                    warn!("   1. Account {} doesn't exist", self.init_signer.account_id);
                    warn!("   2. Public key {} is not added to the account", self.init_signer.public_key);
                    warn!("   3. The private key doesn't match the public key");

                    // Check if this is the actual MethodNotFound error
                    let error_str = format!("{:?}", e);
                    if error_str.contains("MethodNotFound") {
                        warn!("   ⚠️ MethodNotFound during ACCESS KEY query (not contract call!)");
                        warn!("   This is very unusual - RPC issue?");
                    }
                }
                return Err(anyhow::anyhow!("Failed to query access key: {:?}", e));
            }
        };

        let nonce = if let QueryResponseKind::AccessKey(key) = access_key_response.kind {
            if std::env::var("LOG_MASTER_KEY_HASH").unwrap_or_default() == "true" {
                info!("🔍 DEBUG: Access key found, nonce: {}", key.nonce);
            }
            key.nonce + 1
        } else {
            if std::env::var("LOG_MASTER_KEY_HASH").unwrap_or_default() == "true" {
                warn!("🔍 DEBUG: Unexpected response type for access key query");
            }
            1
        };

        // Get latest block hash
        let block = self.rpc_client
            .call(methods::block::RpcBlockRequest {
                block_reference: BlockReference::Finality(Finality::Final),
            })
            .await?;

        // Create transaction using V0 format
        let transaction_v0 = TransactionV0 {
            signer_id: self.init_signer.account_id.clone(),
            public_key: self.init_signer.public_key.clone(),
            nonce,
            receiver_id: self.dao_contract_id.clone(),
            block_hash: block.header.hash,
            actions: vec![Action::FunctionCall(Box::new(FunctionCallAction {
                method_name: "submit_keystore_registration".to_string(),
                args: args_json.into_bytes(),  // Use JSON string as bytes, not MessagePack
                gas: 300_000_000_000_000, // 300 TGas (matching worker registration)
                deposit: 0,
            }))],
        };

        let transaction = Transaction::V0(transaction_v0);

        // Get transaction hash before moving transaction
        let tx_hash = transaction.get_hash_and_size().0;

        // Sign and send
        let signature = self.init_signer.sign(tx_hash.as_ref());
        let signed_tx = near_primitives::transaction::SignedTransaction::new(
            signature,
            transaction,
        );
        let request = methods::broadcast_tx_commit::RpcBroadcastTxCommitRequest {
            signed_transaction: signed_tx,
        };

        // Debug logging before sending transaction
        if std::env::var("LOG_MASTER_KEY_HASH").unwrap_or_default() == "true" {
            info!("🔍 DEBUG: About to send transaction to NEAR RPC");
            info!("   Transaction hash: {}", tx_hash);
            info!("   Nonce: {}", nonce);
            info!("   Block hash: {}", block.header.hash);
        }

        let outcome = match self.rpc_client.call(request).await {
            Ok(outcome) => outcome,
            Err(e) => {
                // Enhanced error logging
                if std::env::var("LOG_MASTER_KEY_HASH").unwrap_or_default() == "true" {
                    warn!("🔍 DEBUG: Transaction failed with error: {:?}", e);
                    warn!("   Error type: {}", std::any::type_name_of_val(&e));
                    warn!("   Contract tried: {}", self.dao_contract_id);
                    warn!("   Method tried: submit_keystore_registration");
                    warn!("   Signer: {}", self.init_signer.account_id);

                    // Try to extract more details from the error
                    let error_str = format!("{:?}", e);
                    if error_str.contains("MethodNotFound") {
                        warn!("   ⚠️  MethodNotFound: The contract doesn't have 'submit_keystore_registration' method");
                        warn!("   ⚠️  Possible causes:");
                        warn!("      1. Wrong contract deployed at {}", self.dao_contract_id);
                        warn!("      2. Method name typo (should be 'submit_keystore_registration')");
                        warn!("      3. Arguments format issue (expecting JSON string as bytes)");
                    }
                }
                return Err(anyhow::anyhow!("Transaction failed: {:?}", e));
            }
        };

        // Log transaction outcome details (similar to worker)
        info!("📋 Transaction outcome status: {:?}", outcome.status);
        info!("   Transaction ID: {:?}", outcome.transaction.hash);
        info!("   Transaction logs: {}", outcome.transaction_outcome.outcome.logs.len());
        for (i, log) in outcome.transaction_outcome.outcome.logs.iter().enumerate() {
            info!("      Log #{}: {}", i, log);
        }
        for (i, receipt) in outcome.receipts_outcome.iter().enumerate() {
            info!("   Receipt #{}: executor={}, logs={}",
                i, receipt.outcome.executor_id, receipt.outcome.logs.len());
            for (j, log) in receipt.outcome.logs.iter().enumerate() {
                info!("      Receipt #{} Log #{}: {}", i, j, log);
            }
        }

        // Check transaction status
        use near_primitives::views::FinalExecutionStatus;
        match &outcome.status {
            FinalExecutionStatus::SuccessValue(value) => {
                // Extract proposal ID from return value
                let proposal_id: u64 = serde_json::from_slice(value)
                    .context("Failed to parse proposal ID from transaction result")?;
                info!("✅ Registration submitted successfully! Proposal ID: {}", proposal_id);
                info!("   Transaction: {:?}", outcome.transaction.hash);
                Ok(proposal_id)
            }
            FinalExecutionStatus::Failure(err) => {
                // Parse the error to provide better feedback
                let err_str = format!("{:?}", err);

                if err_str.contains("Smart contract panicked") {
                    error!("❌ Smart contract panicked!");

                    if err_str.contains("TDX quote verification failed") {
                        error!("   ⚠️  TDX quote verification failed in contract");
                        error!("   ⚠️  This happens when using MOCK mode with a contract expecting real TDX quotes");
                        error!("   ⚠️  Solutions:");
                        error!("      1. Add RTMR3 to pre-approved list: near call {} add_approved_rtmr3", self.dao_contract_id);
                        error!("      2. Or switch to real TDX mode: TEE_MODE=tdx");

                        if err_str.contains("Unsupported quote version") {
                            error!("   ⚠️  The MOCK quote format is not recognized by the contract");
                            error!("   ⚠️  The contract expects a real Intel TDX quote, but received 'MOCK' (0x4d4f434b)");
                        }
                    } else if err_str.contains("RTMR3 must be 96 hex chars") {
                        error!("   ⚠️  RTMR3 format error - must be exactly 96 hex characters");
                    } else if err_str.contains("Keystore already approved") {
                        error!("   ⚠️  This keystore public key is already approved");
                    }

                    error!("   Full error: {}", err_str);
                }

                Err(anyhow::anyhow!("Transaction failed with status: {:?}", err))
            }
            other => {
                warn!("⚠️ Unexpected transaction status: {:?}", other);
                Err(anyhow::anyhow!("Unexpected transaction status: {:?}", other))
            }
        }
    }


    /// Wait for DAO approval and execute proposal when approved
    pub async fn wait_for_approval(&self, proposal_id: u64, public_key: &PublicKey) -> Result<()> {
        info!("⏳ Waiting for DAO approval of proposal #{}...", proposal_id);
        info!("   DAO members need to vote to approve the keystore");

        let mut attempts = 0;
        const MAX_ATTEMPTS: u32 = 360; // 30 minutes with 5 second intervals
        let mut executed = false;

        loop {
            // Check if keystore is already approved (proposal executed)
            if self.check_approval_status(public_key).await? {
                info!("✅ Keystore approved by DAO!");
                return Ok(());
            }

            // Check proposal status
            let proposal_status = self.get_proposal_status(proposal_id).await?;
            match proposal_status.as_str() {
                "Approved" => {
                    if !executed {
                        info!("✅ Proposal approved! Waiting for an execution");                        
                    }
                }
                "Executed" => {
                    // Proposal already executed, wait for keystore to be approved
                    info!("✅ Proposal already executed, waiting for keystore approval...");
                }
                "Rejected" => {
                    anyhow::bail!("❌ Proposal rejected by DAO");
                }
                "Pending" => {
                    if attempts % 12 == 0 { // Log every minute
                        info!("   Still waiting for votes... ({}/{})",
                            attempts * 5, MAX_ATTEMPTS * 5);
                    }
                }
                _ => {}
            }

            attempts += 1;
            if attempts >= MAX_ATTEMPTS {
                anyhow::bail!("Timeout waiting for DAO approval");
            }

            sleep(Duration::from_secs(5)).await;
        }
    }

    /* REMOVE
    /// Execute approved proposal
    async fn execute_proposal(&self, proposal_id: u64) -> Result<()> {
        info!("📤 Executing proposal #{}", proposal_id);

        // Get access key information
        let access_key_query = methods::query::RpcQueryRequest {
            block_reference: BlockReference::Finality(Finality::Final),
            request: QueryRequest::ViewAccessKey {
                account_id: self.init_signer.account_id.clone(),
                public_key: self.init_signer.public_key.clone(),
            },
        };

        let access_key_response = self.rpc_client.call(access_key_query).await
            .context("Failed to query access key")?;

        let nonce = if let QueryResponseKind::AccessKey(access_key_view) = access_key_response.kind {
            access_key_view.nonce + 1
        } else {
            anyhow::bail!("Failed to get access key nonce");
        };

        // Get current block hash
        let block = self.rpc_client
            .call(methods::block::RpcBlockRequest {
                block_reference: BlockReference::Finality(Finality::Final),
            })
            .await
            .context("Failed to get latest block")?;

        // Create transaction to execute proposal
        let args = json!({
            "proposal_id": proposal_id,
        });

        let args_json = serde_json::to_string(&args)?;

        let transaction_v0 = TransactionV0 {
            signer_id: self.init_signer.account_id.clone(),
            public_key: self.init_signer.public_key.clone(),
            nonce,
            receiver_id: self.dao_contract_id.clone(),
            block_hash: block.header.hash,
            actions: vec![Action::FunctionCall(Box::new(FunctionCallAction {
                method_name: "execute_proposal".to_string(),
                args: args_json.into_bytes(),
                gas: 100_000_000_000_000, // 100 TGas
                deposit: 0,
            }))],
        };

        let transaction = Transaction::V0(transaction_v0);

        // Sign and send transaction
        let signature = self.init_signer.sign(transaction.get_hash_and_size().0.as_ref());
        let request = methods::broadcast_tx_commit::RpcBroadcastTxCommitRequest {
            signed_transaction: near_primitives::transaction::SignedTransaction::new(
                signature,
                transaction,
            ),
        };

        let outcome = self.rpc_client.call(request).await
            .context("Failed to execute proposal")?;

        // Check transaction status
        match &outcome.status {
            FinalExecutionStatus::SuccessValue(_) => {
                info!("✅ Proposal executed successfully");
                Ok(())
            }
            FinalExecutionStatus::Failure(err) => {
                Err(anyhow::anyhow!("Transaction failed: {:?}", err))
            }
            other => {
                Err(anyhow::anyhow!("Unexpected transaction status: {:?}", other))
            }
        }
    }
    */

    /// Check if keystore is approved
    async fn check_approval_status(&self, public_key: &PublicKey) -> Result<bool> {
        let args = json!({
            "public_key": public_key.to_string(),
        });

        let request = methods::query::RpcQueryRequest {
            block_reference: BlockReference::Finality(Finality::Final),
            request: QueryRequest::CallFunction {
                account_id: self.dao_contract_id.clone(),
                method_name: "is_keystore_approved".to_string(),
                args: serde_json::to_vec(&args)?.into(),
            },
        };

        let response = self.rpc_client.call(request).await?;

        if let QueryResponseKind::CallResult(CallResult { result, .. }) = response.kind {
            let approved: bool = serde_json::from_slice(&result)?;
            Ok(approved)
        } else {
            Ok(false)
        }
    }

    /// Get proposal status
    async fn get_proposal_status(&self, proposal_id: u64) -> Result<String> {
        let args = json!({
            "proposal_id": proposal_id,
        });

        let request = methods::query::RpcQueryRequest {
            block_reference: BlockReference::Finality(Finality::Final),
            request: QueryRequest::CallFunction {
                account_id: self.dao_contract_id.clone(),
                method_name: "get_proposal".to_string(),
                args: serde_json::to_vec(&args)?.into(),
            },
        };

        let response = self.rpc_client.call(request).await?;

        if let QueryResponseKind::CallResult(CallResult { result, .. }) = response.kind {
            let proposal: serde_json::Value = serde_json::from_slice(&result)?;
            Ok(proposal["status"].as_str().unwrap_or("Unknown").to_string())
        } else {
            Ok("Unknown".to_string())
        }
    }
}