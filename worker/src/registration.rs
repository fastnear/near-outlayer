use anyhow::{Context, Result};
use near_crypto::{PublicKey, SecretKey};
use near_jsonrpc_client::{methods, JsonRpcClient};
use near_primitives::transaction::{Action, FunctionCallAction, Transaction, TransactionV0};
use near_primitives::types::{AccountId, BlockReference, Finality};
use serde_json::json;
use std::fs;
use std::path::Path;
use tracing::info;

use crate::tdx_attestation::TdxClient;

/// Worker registration client
///
/// Handles worker keypair generation and registration with the register contract
pub struct RegistrationClient {
    rpc_client: JsonRpcClient,
    register_contract_id: AccountId,
    init_account_id: AccountId,
    init_secret_key: SecretKey,
}

impl RegistrationClient {
    /// Create a new registration client
    pub fn new(
        near_rpc_url: String,
        register_contract_id: AccountId,
        init_account_id: AccountId,
        init_secret_key: SecretKey,
    ) -> Self {
        let rpc_client = JsonRpcClient::connect(&near_rpc_url);

        Self {
            rpc_client,
            register_contract_id,
            init_account_id,
            init_secret_key,
        }
    }

    /// Load or generate worker keypair
    ///
    /// Keypair is stored at ~/.near-credentials/worker-keypair.json
    /// Format: {"public_key": "ed25519:...", "private_key": "ed25519:..."}
    pub fn load_or_generate_keypair(&self) -> Result<(PublicKey, SecretKey)> {
        let keypair_path = Self::get_keypair_path();

        if keypair_path.exists() {
            info!("📂 Loading existing worker keypair from: {}", keypair_path.display());
            self.load_keypair(&keypair_path)
        } else {
            info!("🔑 Generating new worker keypair...");
            let (public_key, secret_key) = self.generate_keypair()?;
            self.save_keypair(&keypair_path, &public_key, &secret_key)?;
            Ok((public_key, secret_key))
        }
    }

    /// Generate a new ed25519 keypair
    fn generate_keypair(&self) -> Result<(PublicKey, SecretKey)> {
        use ed25519_dalek::SigningKey;
        use rand::rngs::OsRng;

        let signing_key = SigningKey::generate(&mut OsRng);
        let secret_bytes = signing_key.to_bytes(); // 32 bytes
        let public_bytes = signing_key.verifying_key().to_bytes(); // 32 bytes

        // NEAR ED25519SecretKey requires 64 bytes: [secret_key (32 bytes)][public_key (32 bytes)]
        let mut keypair_bytes = [0u8; 64];
        keypair_bytes[..32].copy_from_slice(&secret_bytes);
        keypair_bytes[32..].copy_from_slice(&public_bytes);

        // Create NEAR SecretKey from 64-byte keypair
        let secret_key = SecretKey::ED25519(near_crypto::ED25519SecretKey(keypair_bytes));
        let public_key = secret_key.public_key();

        info!("✅ Generated new keypair: {}", public_key);

        Ok((public_key, secret_key))
    }

    /// Save keypair to file
    fn save_keypair(&self, path: &Path, public_key: &PublicKey, secret_key: &SecretKey) -> Result<()> {
        // Create parent directory if it doesn't exist
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .context("Failed to create keypair directory")?;
        }

        let keypair_json = json!({
            "public_key": public_key.to_string(),
            "private_key": secret_key.to_string(),
        });

        fs::write(path, serde_json::to_string_pretty(&keypair_json)?)
            .context("Failed to write keypair file")?;

        info!("💾 Saved worker keypair to: {}", path.display());

        Ok(())
    }

    /// Load keypair from file
    fn load_keypair(&self, path: &Path) -> Result<(PublicKey, SecretKey)> {
        let contents = fs::read_to_string(path)
            .context("Failed to read keypair file")?;

        let keypair: serde_json::Value = serde_json::from_str(&contents)
            .context("Failed to parse keypair JSON")?;

        let public_key_str = keypair["public_key"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing public_key field"))?;

        let private_key_str = keypair["private_key"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing private_key field"))?;

        let public_key: PublicKey = public_key_str.parse()
            .context("Failed to parse public_key")?;

        let secret_key: SecretKey = private_key_str.parse()
            .context("Failed to parse private_key")?;

        info!("✅ Loaded keypair: {}", public_key);

        Ok((public_key, secret_key))
    }

    /// Get keypair file path
    fn get_keypair_path() -> std::path::PathBuf {
        let home = std::env::var("HOME")
            .unwrap_or_else(|_| "/root".to_string());

        std::path::PathBuf::from(home)
            .join(".near-credentials")
            .join("worker-keypair.json")
    }

    /// Register worker public key with TDX attestation
    ///
    /// This method:
    /// 1. Generates a TDX quote with the public key embedded in report_data
    /// 2. Calls register_worker_key on the register contract
    /// 3. The contract verifies the quote and adds the key to the operator account
    pub async fn register_worker_key(
        &self,
        public_key: &PublicKey,
        tdx_client: &TdxClient,
        collateral_json: Option<String>,
    ) -> Result<String> {
        info!("🔐 Registering worker key with register contract...");
        info!("   Public key: {}", public_key);
        info!("   Register contract: {}", self.register_contract_id);
        info!("   Init account (gas payer): {}", self.init_account_id);

        // Extract raw ed25519 public key bytes (32 bytes, without the 0x00 prefix)
        let public_key_bytes = match public_key {
            PublicKey::ED25519(key) => key.0,
            _ => anyhow::bail!("Only ed25519 keys are supported"),
        };

        info!("   Public key bytes (hex): {}", hex::encode(&public_key_bytes));

        // Generate TDX quote with public key embedded in report_data
        // TdxClient will put public_key_bytes into the first 32 bytes of report_data
        let tdx_quote_hex = tdx_client
            .generate_registration_quote(&public_key_bytes)
            .context("Failed to generate TDX quote for registration")?;

        info!("✅ Generated TDX quote (length: {} bytes)", tdx_quote_hex.len() / 2);

        // Call register_worker_key on the register contract
        let args = json!({
            "public_key": public_key.to_string(),
            "tdx_quote_hex": tdx_quote_hex,
            "collateral_json": collateral_json,
        });

        let args_json = serde_json::to_string(&args)
            .context("Failed to serialize register_worker_key args")?;

        info!("📤 Calling register_worker_key on {}...", self.register_contract_id);

        // Call contract method
        let outcome = self
            .call_contract_method(
                &self.register_contract_id,
                "register_worker_key",
                args_json.into_bytes(),
                300_000_000_000_000, // 300 TGas
                0, // No deposit
            )
            .await
            .context("Failed to call register_worker_key")?;

        let tx_hash = format!("{}", outcome.transaction_outcome.id);
        info!("✅ Worker key registered successfully!");
        info!("   Transaction: {}", tx_hash);

        Ok(tx_hash)
    }


    /// Call a contract method
    async fn call_contract_method(
        &self,
        contract_id: &AccountId,
        method_name: &str,
        args: Vec<u8>,
        gas: u64,
        deposit: u128,
    ) -> Result<near_primitives::views::FinalExecutionOutcomeView> {
        use near_crypto::InMemorySigner;

        // Create signer from init account credentials (for gas payment)
        let signer = InMemorySigner::from_secret_key(
            self.init_account_id.clone(),
            self.init_secret_key.clone(),
        );

        // Get account access key for nonce
        let access_key_query = methods::query::RpcQueryRequest {
            block_reference: BlockReference::Finality(Finality::Final),
            request: near_primitives::views::QueryRequest::ViewAccessKey {
                account_id: signer.account_id.clone(),
                public_key: signer.public_key(),
            },
        };

        let access_key_response = self
            .rpc_client
            .call(access_key_query)
            .await
            .context("Failed to query access key")?;

        let current_nonce = match access_key_response.kind {
            near_jsonrpc_primitives::types::query::QueryResponseKind::AccessKey(access_key) => {
                access_key.nonce
            }
            _ => anyhow::bail!("Unexpected query response"),
        };

        // Get latest block hash
        let block_query = methods::block::RpcBlockRequest {
            block_reference: BlockReference::Finality(Finality::Final),
        };

        let block = self
            .rpc_client
            .call(block_query)
            .await
            .context("Failed to query block")?;

        let block_hash = block.header.hash;

        // Create transaction
        let transaction_v0 = TransactionV0 {
            signer_id: signer.account_id.clone(),
            public_key: signer.public_key(),
            nonce: current_nonce + 1,
            receiver_id: contract_id.clone(),
            block_hash,
            actions: vec![Action::FunctionCall(Box::new(FunctionCallAction {
                method_name: method_name.to_string(),
                args,
                gas,
                deposit,
            }))],
        };

        let transaction = Transaction::V0(transaction_v0);

        // Sign transaction
        let signature = signer.sign(transaction.get_hash_and_size().0.as_ref());
        let signed_transaction = near_primitives::transaction::SignedTransaction::new(
            signature,
            transaction,
        );

        // Broadcast transaction with commit (wait for finality)
        let tx_request = methods::broadcast_tx_commit::RpcBroadcastTxCommitRequest {
            signed_transaction,
        };

        let outcome = self
            .rpc_client
            .call(tx_request)
            .await
            .context("Failed to broadcast transaction and wait for commit")?;

        Ok(outcome)
    }
}

/// Register worker on startup
///
/// This function should be called ONCE when the worker starts up.
/// It will:
/// 1. Load or generate a worker keypair
/// 2. Register the public key with the register contract using TDX attestation
/// 3. Store the keypair for future use
///
/// Returns the worker's public key and secret key
pub async fn register_worker_on_startup(
    near_rpc_url: String,
    register_contract_id: AccountId,
    init_account_id: AccountId,
    init_secret_key: SecretKey,
    tdx_client: &TdxClient,
    collateral_json: Option<String>,
) -> Result<(PublicKey, SecretKey)> {
    info!("🚀 Starting worker registration flow...");

    let registration_client = RegistrationClient::new(
        near_rpc_url,
        register_contract_id,
        init_account_id,
        init_secret_key,
    );

    // Load or generate keypair
    let (public_key, secret_key) = registration_client.load_or_generate_keypair()?;

    // Register worker key - fail fast if registration fails
    info!("📝 Registering worker key with TDX attestation...");

    registration_client
        .register_worker_key(&public_key, tdx_client, collateral_json)
        .await
        .context("Failed to register worker key with register contract")?;

    Ok((public_key, secret_key))
}
