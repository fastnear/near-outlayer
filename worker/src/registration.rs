use anyhow::{Context, Result};
use near_crypto::{InMemorySigner, PublicKey, SecretKey};
use near_jsonrpc_client::{methods, JsonRpcClient};
use near_jsonrpc_primitives::types::query::RpcQueryError;
use near_primitives::types::{AccountId, BlockReference, Finality};
use near_primitives::views::QueryRequest;
use serde_json::json;
use std::fs;
use std::path::Path;
use tracing::info;

use crate::near_client::NearClient;
use crate::tdx_attestation::TdxClient;

/// Worker registration client
///
/// Handles worker keypair generation and registration via the register-contract
/// deployed at the operator account (OPERATOR_ACCOUNT_ID).
pub struct RegistrationClient {
    near_client: NearClient,
    /// Operator account where register-contract is deployed and keys are stored
    operator_account_id: AccountId,
    rpc_client: JsonRpcClient,
}

impl RegistrationClient {
    /// Create a new registration client
    ///
    /// Register-contract is deployed at `operator_account_id`.
    /// Uses the init account for gas payment.
    pub fn new(
        near_rpc_url: String,
        operator_account_id: AccountId,
        init_account_id: AccountId,
        init_secret_key: SecretKey,
    ) -> Result<Self> {
        // Create signer for init account (pays gas for registration)
        let signer = InMemorySigner::from_secret_key(
            init_account_id,
            init_secret_key,
        );

        // Create NearClient pointing to operator account (register-contract deployed there)
        let near_client = NearClient::new(
            near_rpc_url.clone(),
            signer,
            operator_account_id.clone(),
        )?;

        // Create RPC client for queries (view access keys)
        let rpc_client = JsonRpcClient::connect(&near_rpc_url);

        Ok(Self {
            near_client,
            operator_account_id,
            rpc_client,
        })
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

    /// Check if an access key exists on the operator account
    ///
    /// This queries the NEAR blockchain to check if the given public key
    /// is already registered as an access key on the operator account.
    ///
    /// # Arguments
    /// * `public_key` - The public key to check
    ///
    /// # Returns
    /// * `Ok(true)` - Key exists
    /// * `Ok(false)` - Key does not exist
    /// * `Err(_)` - Failed to query the blockchain
    pub async fn check_access_key_exists(&self, public_key: &PublicKey) -> Result<bool> {
        info!("🔍 Checking if key exists on operator account: {}", self.operator_account_id);
        info!("   Public key: {}", public_key);

        let request = methods::query::RpcQueryRequest {
            block_reference: BlockReference::Finality(Finality::Final),
            request: QueryRequest::ViewAccessKey {
                account_id: self.operator_account_id.clone(),
                public_key: public_key.clone(),
            },
        };

        match self.rpc_client.call(request).await {
            Ok(_response) => {
                info!("✅ Access key found on operator account");
                Ok(true)
            }
            Err(e) => {
                // Check if this is UnknownAccessKey error (key not found - which is OK)
                if let Some(handler_error) = e.handler_error() {
                    match handler_error {
                        RpcQueryError::UnknownAccessKey { public_key, .. } => {
                            info!("ℹ️  Access key NOT found on operator account: {}", public_key);
                            info!("   Will need to register via contract");
                            return Ok(false);
                        }
                        _ => {
                            // Other query errors (e.g., network issues, invalid account, etc.)
                            anyhow::bail!("Failed to query access key: {:?}", handler_error)
                        }
                    }
                }

                // Non-handler errors (network, parse errors, etc.)
                anyhow::bail!("Failed to query access key (network/transport error): {:?}", e)
            }
        }
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
    ) -> Result<(String, String)> {
        info!("🔐 Registering worker key with register contract...");
        info!("   Public key: {}", public_key);
        info!("   Register contract: {}", self.operator_account_id);

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
            .await
            .context("Failed to generate TDX quote for registration")?;

        info!("✅ Generated TDX quote (length: {} bytes)", tdx_quote_hex.len() / 2);
        info!("   TDX quote hex (first 100 chars): {}...",
            if tdx_quote_hex.len() > 100 { &tdx_quote_hex[..100] } else { &tdx_quote_hex });

        // Call register_worker_key on the register contract
        // Note: Contract ONLY uses cached collateral (security: prevent bypass)
        // If registration fails with "Quote collateral required", fetch collateral and cache it via update_collateral
        let args = json!({
            "public_key": public_key.to_string(),
            "tdx_quote_hex": tdx_quote_hex,
        });

        let args_json = serde_json::to_string(&args)
            .context("Failed to serialize register_worker_key args")?;

        info!("📤 Calling register_worker_key on {}...", self.operator_account_id);
        info!("   Args size: {} bytes", args_json.len());

        // Call contract method using NearClient (reuses working transaction logic)
        let outcome = self
            .near_client
            .call_contract(
                &self.operator_account_id,
                "register_worker_key",
                args_json.into_bytes(),
                300_000_000_000_000, // 300 TGas
                0, // No deposit
            )
            .await
            .context("Failed to call register_worker_key")?;

        info!("📋 Transaction outcome status: {:?}", outcome.status);
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

        let tx_hash = format!("{}", outcome.transaction_outcome.id);

        // Check if transaction succeeded
        use near_primitives::views::FinalExecutionStatus;
        match &outcome.status {
            FinalExecutionStatus::SuccessValue(_) => {
                info!("✅ Worker key registered successfully!");
                info!("   Transaction: {}", tx_hash);
                Ok((tx_hash, tdx_quote_hex))
            }
            FinalExecutionStatus::Failure(err) => {
                anyhow::bail!("Transaction failed: {:?}", err)
            }
            other => {
                anyhow::bail!("Unexpected transaction status: {:?}", other)
            }
        }
    }
}

/// Register worker on startup
///
/// This function should be called ONCE when the worker starts up.
/// It will:
/// 1. Load or generate a worker keypair
/// 2. Check if the key is already registered on the operator account
/// 3. Register the public key with the register contract using TDX attestation (if not already registered)
/// 4. Store the keypair for future use
///
/// Returns the worker's public key and secret key
pub async fn register_worker_on_startup(
    near_rpc_url: String,
    operator_account_id: AccountId,
    init_account_id: AccountId,
    init_secret_key: SecretKey,
    tdx_client: &TdxClient,
) -> Result<(PublicKey, SecretKey, String)> {
    info!("🚀 Starting worker registration flow...");

    let registration_client = RegistrationClient::new(
        near_rpc_url,
        operator_account_id,
        init_account_id,
        init_secret_key,
    )
    .context("Failed to create registration client")?;

    // Load or generate keypair
    let (public_key, secret_key) = registration_client.load_or_generate_keypair()?;

    // Check if key is already registered on the operator account
    let key_exists = registration_client
        .check_access_key_exists(&public_key)
        .await
        .context("Failed to check if access key exists on operator account")?;

    let tdx_quote_hex = if key_exists {
        info!("✅ Worker key already registered on operator account - skipping registration");
        info!("   Using existing key for signing execution results");

        // Generate a TDX quote anyway for coordinator attestation (not sent to contract)
        let public_key_bytes = match &public_key {
            PublicKey::ED25519(key) => key.0,
            _ => anyhow::bail!("Only ed25519 keys are supported"),
        };

        tdx_client
            .generate_registration_quote(&public_key_bytes)
            .await
            .context("Failed to generate TDX quote for coordinator")?
    } else {
        // Register worker key - fail fast if registration fails
        info!("📝 Registering worker key with TDX attestation...");

        let (_tx_hash, tdx_quote) = registration_client
            .register_worker_key(&public_key, tdx_client)
            .await
            .context("Failed to register worker key with register contract")?;

        tdx_quote
    };

    Ok((public_key, secret_key, tdx_quote_hex))
}
