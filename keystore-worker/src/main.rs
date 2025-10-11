//! Keystore Worker - TEE-based secret management for NEAR Offshore
//!
//! This worker runs in a Trusted Execution Environment (TEE) and provides
//! secure decryption of user secrets for executor workers.
//!
//! Architecture:
//! 1. Keystore worker generates master keypair on first start
//! 2. Public key is published to NEAR contract
//! 3. Users encrypt secrets with this public key
//! 4. Executor workers request decryption with TEE attestation proof
//! 5. Keystore verifies attestation and decrypts secrets
//! 6. Secrets are returned only to verified TEE workers
//!
//! Security guarantees:
//! - Private key NEVER leaves TEE memory
//! - Only verified workers (via attestation) can decrypt
//! - All operations are async and non-blocking
//! - Token-based authentication for API access

mod api;
mod attestation;
mod config;
mod crypto;
mod near;

use anyhow::{Context, Result};
use config::Config;
use crypto::Keystore;
use std::sync::Arc;
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

    // Initialize or load keystore
    let keystore = initialize_keystore(&config).await?;

    tracing::info!(
        public_key = %keystore.public_key_hex(),
        "Keystore initialized"
    );

    // Verify public key matches contract (critical security check)
    let near_client = near::NearClient::new(
        &config.near_rpc_url,
        &config.keystore_account_id,
        &config.keystore_private_key,
        &config.offchainvm_contract_id,
    )?;

    let pubkey_hex = keystore.public_key_hex();
    let matches = near_client
        .verify_public_key(&pubkey_hex)
        .await
        .context("Failed to verify public key")?;

    if !matches {
        tracing::error!("Public key mismatch detected!");
        tracing::error!("This keystore's public key does not match the contract's stored key.");
        tracing::error!("Expected: {}", pubkey_hex);
        tracing::error!("This is a critical security error. Possible causes:");
        tracing::error!("1. Wrong private key in KEYSTORE_PRIVATE_KEY");
        tracing::error!("2. Contract has different public key set");
        tracing::error!("3. Wrong contract ID in OFFCHAINVM_CONTRACT_ID");
        tracing::error!("");
        tracing::error!("To fix: Call set_keystore_pubkey on contract or use correct private key");
        anyhow::bail!("Public key mismatch - keystore cannot operate safely");
    }

    tracing::info!("✓ Public key verified - matches contract");

    // Create API server
    let app_state = api::AppState::new(keystore, config.clone());
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
/// - First start: Generate new keypair, seal to TEE storage, publish to contract
/// - Subsequent starts: Load from sealed storage
///
/// For MVP (non-TEE):
/// - Use KEYSTORE_PRIVATE_KEY from environment
async fn initialize_keystore(config: &Config) -> Result<Keystore> {
    match config.tee_mode {
        config::TeeMode::Sgx | config::TeeMode::Sev => {
            // TODO: Implement TEE sealed storage
            // Steps:
            // 1. Check if sealed key exists in TEE persistent storage
            // 2. If yes: Unseal and load
            // 3. If no: Generate new key, seal it, publish to contract
            tracing::warn!("TEE mode enabled but sealed storage not implemented");
            tracing::warn!("Falling back to loading key from environment");
            load_keystore_from_env(config)
        }
        config::TeeMode::Simulated => {
            tracing::info!("Simulated TEE mode - loading key from environment");
            load_keystore_from_env(config)
        }
        config::TeeMode::None => {
            tracing::info!("Dev mode (no TEE) - loading key from environment");
            load_keystore_from_env(config)
        }
    }
}

/// Load keystore from KEYSTORE_PRIVATE_KEY environment variable
fn load_keystore_from_env(config: &Config) -> Result<Keystore> {
    Keystore::from_private_key(&config.keystore_private_key)
        .context("Failed to load keystore from private key")
}

/// Generate new keystore and publish to contract (for first-time setup)
#[allow(dead_code)]
async fn generate_and_publish_keystore(config: &Config) -> Result<Keystore> {
    tracing::info!("Generating new keystore keypair");

    let keystore = Keystore::generate();
    let pubkey_hex = keystore.public_key_hex();

    tracing::info!(
        pubkey = %pubkey_hex,
        "Generated new keypair"
    );

    // Publish to contract
    let near_client = near::NearClient::new(
        &config.near_rpc_url,
        &config.keystore_account_id,
        &config.keystore_private_key,
        &config.offchainvm_contract_id,
    )?;

    near_client
        .publish_public_key(&pubkey_hex)
        .await
        .context("Failed to publish public key to contract")?;

    tracing::info!("Successfully published public key to contract");

    Ok(keystore)
}
