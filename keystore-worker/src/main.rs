//! Keystore Worker - TEE-based secret management for NEAR Offshore
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
mod types;
mod utils;

use anyhow::{Context, Result};
use config::Config;
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

    tracing::info!("Starting NEAR Offshore Keystore Worker");

    // Load configuration
    let config = Config::from_env().context("Failed to load configuration")?;
    config.validate().context("Invalid configuration")?;

    tracing::info!(
        server_addr = %config.server_addr,
        tee_mode = ?config.tee_mode,
        contract = %config.offchainvm_contract_id,
        "Configuration loaded"
    );

    // Initialize or load keystore (master secret based)
    let keystore = initialize_keystore(&config).await?;

    tracing::info!("Keystore initialized with master secret derivation");
    tracing::info!("Each repo will have a unique keypair derived from master_secret + seed");

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

    // Create API server
    let app_state = api::AppState::new(keystore, config.clone(), near_client);
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

/// Initialize keystore from environment or generate new one
///
/// In production TEE:
/// - First start: Generate master_secret, seal to TEE storage
/// - Subsequent starts: Load from sealed storage
///
/// For MVP (non-TEE):
/// - Use KEYSTORE_MASTER_SECRET from environment (if set)
/// - Otherwise: Generate new master_secret and warn user to save it
async fn initialize_keystore(_config: &Config) -> Result<Keystore> {
    // Try to load from environment variable
    if let Ok(master_secret_hex) = std::env::var("KEYSTORE_MASTER_SECRET") {
        tracing::info!("Loading keystore from KEYSTORE_MASTER_SECRET");
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
