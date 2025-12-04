//! Keystore Worker - TEE-based secret management for NEAR OutLayer
//!
//! This worker runs in a Trusted Execution Environment (TEE) and provides
//! secure decryption of user secrets for executor workers.
//!
//! Architecture:
//! 1. Keystore worker has a master_secret that NEVER leaves TEE
//! 2. Per-repo keypairs are derived using HMAC-SHA256(master_secret, seed)
//! 3. Seed format: "github.com/owner/repo:account_id[:branch]"
//! 4. Users encrypt secrets with repo-specific public key from coordinator
//! 5. Executor workers request decryption with TEE attestation proof
//! 6. Keystore verifies attestation and decrypts secrets
//! 7. Secrets are returned only to verified TEE workers
//!
//! Security guarantees:
//! - Master secret NEVER leaves TEE memory
//! - Each repo/branch/owner gets a unique keypair
//! - Only verified workers (via attestation) can decrypt
//! - All operations are async and non-blocking
//! - Token-based authentication for API access

mod api;
mod attestation;
mod config;
mod crypto;
mod near;
mod secret_generation;
mod types;
mod utils;
mod mpc_ckd;
mod tee_registration;
mod tdx_attestation;

use anyhow::{Context, Result};
use config::{Config, TeeMode};
use crypto::Keystore;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,keystore_worker=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    tracing::info!("Starting NEAR OutLayer Keystore Worker");

    // Load configuration
    let config = Config::from_env().context("Failed to load configuration")?;
    config.validate().context("Invalid configuration")?;

    tracing::info!(
        server_addr = %config.server_addr,
        tee_mode = ?config.tee_mode,
        contract = %config.offchainvm_contract_id,
        "Configuration loaded"
    );

    // Initialize NEAR RPC client for reading secrets from contract
    // Only NEAR_RPC_URL and NEAR_CONTRACT_ID are required (read-only)
    let near_client = if let (Ok(rpc_url), Ok(contract_id)) = (
        std::env::var("NEAR_RPC_URL"),
        std::env::var("NEAR_CONTRACT_ID"),
    ) {
        tracing::info!("Initializing NEAR RPC client (read-only)");

        match near::NearClient::new(&rpc_url, &contract_id) {
            Ok(client) => {
                tracing::info!("✅ NEAR RPC client initialized");
                Some(client)
            }
            Err(e) => {
                tracing::warn!("❌ Failed to initialize NEAR client: {}", e);
                tracing::warn!("   Secrets reading from contract will not work");
                None
            }
        }
    } else {
        tracing::warn!("NEAR_RPC_URL or NEAR_CONTRACT_ID not set");
        tracing::warn!("Secrets reading from contract will be disabled");
        tracing::warn!("Required env vars: NEAR_RPC_URL, NEAR_CONTRACT_ID");
        None
    };

    // Check if we're in TEE registration mode
    let use_tee_registration = std::env::var("USE_TEE_REGISTRATION")
        .unwrap_or_else(|_| "false".to_string())
        .parse::<bool>()
        .unwrap_or(false);

    // Validate configuration: KEYSTORE_MASTER_SECRET is incompatible with TEE registration
    if use_tee_registration && std::env::var("KEYSTORE_MASTER_SECRET").is_ok() {
        tracing::error!("❌ Configuration error: KEYSTORE_MASTER_SECRET cannot be used with USE_TEE_REGISTRATION=true");
        tracing::error!("   When using TEE registration, the master secret comes from MPC CKD after DAO approval");
        tracing::error!("   Please remove KEYSTORE_MASTER_SECRET from your .env file");
        return Err(anyhow::anyhow!("Incompatible configuration: KEYSTORE_MASTER_SECRET with USE_TEE_REGISTRATION=true"));
    }

    // Initialize keystore (temporary if TEE mode)
    let initial_keystore = if use_tee_registration {
        tracing::info!("🔐 TEE registration mode - starting with temporary keystore");
        tracing::info!("   API will be blocked until DAO approval and MPC key obtained");
        crypto::Keystore::generate() // Temporary keystore
    } else {
        // Initialize normal keystore for non-TEE mode
        let keystore = initialize_keystore(&config).await?;
        tracing::info!("Keystore initialized with master secret derivation");
        tracing::info!("Each repo will have a unique keypair derived from master_secret + seed");
        keystore
    };

    // Create API server
    let app_state = api::AppState::new(initial_keystore, config.clone(), near_client);

    // If in TEE mode, spawn task to handle registration and MPC key retrieval
    if use_tee_registration {
        let state_clone = app_state.clone();
        let config_clone = config.clone();

        tokio::spawn(async move {
            tracing::info!("🔐 Starting TEE registration process in background");

            match perform_tee_registration(&config_clone).await {
                Ok(real_keystore) => {
                    // Replace the temporary keystore with the real one
                    state_clone.replace_keystore(real_keystore).await;
                    state_clone.mark_ready();
                    tracing::info!("✅ TEE registration complete! Keystore is now ready to serve requests");
                }
                Err(e) => {
                    tracing::error!("❌ TEE registration failed: {}", e);

                    // Enhanced error debugging when LOG_MASTER_KEY_HASH is set
                    if std::env::var("LOG_MASTER_KEY_HASH").unwrap_or_default() == "true" {
                        tracing::error!("🔍 DEBUG: Full error chain:");
                        let mut source = e.source();
                        let mut level = 1;
                        while let Some(err) = source {
                            tracing::error!("   Level {}: {}", level, err);
                            source = err.source();
                            level += 1;
                        }
                        tracing::error!("🔍 DEBUG: Error Debug format: {:?}", e);
                    }

                    tracing::error!("   Keystore will remain in not-ready state");
                    tracing::error!("   Fix the issue and restart the service");
                }
            }
        });
    }

    let router = api::create_router(app_state);

    // Start server
    let listener = tokio::net::TcpListener::bind(&config.server_addr)
        .await
        .context("Failed to bind server")?;

    tracing::info!(
        addr = %config.server_addr,
        "Keystore worker API server started"
    );
    tracing::info!("Ready to serve decryption requests from executor workers");

    // Run server (this blocks until shutdown)
    axum::serve(listener, router)
        .await
        .context("Server error")?;

    Ok(())
}

/// Perform TEE registration and get MPC-derived keystore
async fn perform_tee_registration(config: &Config) -> Result<Keystore> {
    tracing::info!("🔐 Starting TEE registration flow with retry logic");

    // Check required environment variables - NO FALLBACKS!
    let dao_contract = std::env::var("KEYSTORE_DAO_CONTRACT")
        .context("KEYSTORE_DAO_CONTRACT not set")?;
    let init_account_id = std::env::var("INIT_ACCOUNT_ID")
        .context("INIT_ACCOUNT_ID not set")?;
    let init_private_key = std::env::var("INIT_ACCOUNT_PRIVATE_KEY")
        .context("INIT_ACCOUNT_PRIVATE_KEY not set")?;
    let near_rpc_url = std::env::var("NEAR_RPC_URL")
        .context("NEAR_RPC_URL is required for TEE registration")?;

    // Create registration client
    let registration = tee_registration::RegistrationClient::new(
        near_rpc_url.clone(),
        dao_contract.parse()?,
        init_account_id.parse()?,
        init_private_key.parse()?,
    )?;

    // Load or generate keypair
    // In TEE mode, generate ephemeral keypair in memory only
    let is_tee_mode = config.tee_mode != TeeMode::None;
    let (public_key, secret_key) = registration.load_or_generate_keypair(is_tee_mode)?;
    tracing::info!("📂 Using keystore public key: {}", public_key);

    // Check if already approved
    let approved = match mpc_ckd::check_keystore_approval(
        &near_rpc_url,
        &dao_contract,
        &public_key.to_string(),
    ).await {
        Ok(approved) => approved,
        Err(e) => {
            let error_str = format!("{:?}", e);
            if error_str.contains("MethodNotFound") {
                tracing::warn!("⚠️ Method 'is_keystore_approved' not found on DAO contract");
                tracing::warn!("   This might be an older version of the contract");
                tracing::warn!("   Assuming keystore is NOT approved and proceeding with registration");
                false // Assume not approved if method doesn't exist
            } else {
                return Err(e);
            }
        }
    };

    if !approved {
        tracing::info!("📝 Keystore not yet approved, submitting registration to DAO");

        // Debug log the registration parameters if LOG_MASTER_KEY_HASH is set
        if std::env::var("LOG_MASTER_KEY_HASH").unwrap_or_default() == "true" {
            tracing::info!("🔍 DEBUG: Registration parameters:");
            tracing::info!("   DAO contract: {}", dao_contract);
            tracing::info!("   Init account: {}", init_account_id);
            tracing::info!("   Public key: {}", public_key);
            tracing::info!("   TEE mode: {:?}", config.tee_mode);
        }

        // Generate attestation using new TdxClient
        use crate::tdx_attestation::TdxClient;
        let tdx_client = TdxClient::new(config.tee_mode.to_string());

        // Extract ED25519 public key bytes
        let pubkey_bytes = match &public_key {
            near_crypto::PublicKey::ED25519(key) => key.0,
            _ => anyhow::bail!("Only ED25519 keys supported"),
        };

        let tdx_quote = tdx_client.generate_registration_quote(&pubkey_bytes).await?;
        tracing::info!("📡 Generated TEE attestation (mode: {:?})", config.tee_mode);
        tracing::info!("   Quote will be verified by DAO contract against approved RTMR3 list");

        // Submit to DAO (contract will verify the quote)
        let proposal_id = match registration.submit_registration(public_key.clone(), tdx_quote).await {
            Ok(id) => id,
            Err(e) => {
                let error_str = e.to_string();

                // Check if error is due to RTMR3 not being approved
                if error_str.contains("not approved") || error_str.contains("RTMR3") {
                    tracing::error!("❌ Registration rejected by DAO contract");
                    tracing::error!("   RTMR3 not in approved list");
                    tracing::error!("");
                    tracing::error!("📝 Solution:");
                    tracing::error!("   1. Check DAO contract logs for the extracted RTMR3");
                    tracing::error!("   2. Admin needs to add RTMR3 to approved list");
                    tracing::error!("   3. Restart keystore worker after RTMR3 is approved");
                    tracing::error!("");
                    tracing::error!("⏹️  Keystore stopped - fix the issue and restart");
                } else if std::env::var("LOG_MASTER_KEY_HASH").unwrap_or_default() == "true" {
                    tracing::error!("🔍 DEBUG: Failed to submit registration");
                    tracing::error!("   Error: {:?}", e);
                    tracing::error!("   Check that dao.outlayer.testnet has 'submit_keystore_registration' method");
                    tracing::error!("   You can verify with: near view dao.outlayer.testnet get_config");
                }
                return Err(e);
            }
        };
        tracing::info!("📤 Registration submitted! Proposal ID: {}", proposal_id);

        // Wait for approval
        tracing::info!("⏳ Waiting for DAO approval (this may take a while)...");
        tracing::info!("   DAO members need to vote on proposal #{}", proposal_id);
        registration.wait_for_approval(proposal_id, &public_key).await?;
    } else {
        tracing::info!("✅ Keystore already approved by DAO");
    }

    // Now we're approved, get MPC-derived secret
    tracing::info!("🔑 Requesting master secret from MPC network via CKD");
    let keystore = mpc_ckd::initialize_mpc_keystore(dao_contract, secret_key).await?;

    tracing::info!("✅ Successfully obtained MPC-derived master secret");
    Ok(keystore)
}

/// Initialize keystore from environment or generate new one
///
/// For non-TEE mode only:
/// - Use KEYSTORE_MASTER_SECRET from environment (if set)
/// - Otherwise: Generate new master_secret and warn user to save it
async fn initialize_keystore(_config: &Config) -> Result<Keystore> {
    // Non-TEE mode: use environment variable or generate
    if let Ok(master_secret_hex) = std::env::var("KEYSTORE_MASTER_SECRET") {
        tracing::info!("Loading keystore from KEYSTORE_MASTER_SECRET");

        // Log master key hash if configured to do so
        if std::env::var("LOG_MASTER_KEY_HASH")
            .unwrap_or_else(|_| "false".to_string())
            .parse::<bool>()
            .unwrap_or(false)
        {
            use sha2::{Sha256, Digest};
            let mut hasher = Sha256::new();
            hasher.update(master_secret_hex.as_bytes());
            let hash = hasher.finalize();
            tracing::info!("Master key hash (SHA256): {}", hex::encode(hash));
        }

        Keystore::from_master_secret_hex(&master_secret_hex)
            .context("Failed to load keystore from master secret")
    } else {
        // Generate new master secret
        tracing::warn!("KEYSTORE_MASTER_SECRET not found - generating new master secret");
        let keystore = Keystore::generate();

        // Get hex representation for user to save
        let master_hex = keystore.master_secret_hex();

        tracing::warn!("");
        tracing::warn!("=================================================================");
        tracing::warn!("IMPORTANT: Save this master secret to persist keystore:");
        tracing::warn!("KEYSTORE_MASTER_SECRET={}", master_hex);
        tracing::warn!("");
        tracing::warn!("Add this to your .env file to avoid generating a new secret");
        tracing::warn!("on next restart (which would invalidate all encrypted secrets)");
        tracing::warn!("=================================================================");
        tracing::warn!("");

        Ok(keystore)
    }
}
