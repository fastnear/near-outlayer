//! Configuration management for keystore worker
//!
//! Loads configuration from environment variables with validation.

use anyhow::{Context, Result};
use std::net::SocketAddr;

#[derive(Debug, Clone)]
pub struct Config {
    /// Server bind address
    pub server_addr: SocketAddr,

    /// NEAR network (testnet, mainnet)
    #[allow(dead_code)]
    pub near_network: String,

    /// NEAR RPC URL
    #[allow(dead_code)]
    pub near_rpc_url: String,

    /// OffchainVM contract account ID
    pub offchainvm_contract_id: String,

    /// Allowed worker token hashes (SHA256) - for TEE workers only
    /// Grants access to: /decrypt, /encrypt, /decrypt-raw, /storage/*
    pub allowed_worker_token_hashes: Vec<String>,

    /// Allowed coordinator token hashes (SHA256) - for coordinator only
    /// Grants access to: /add_generated_secret, /update_user_secrets
    pub allowed_coordinator_token_hashes: Vec<String>,

    /// TEE mode (sgx, sev, simulated, none)
    pub tee_mode: TeeMode,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TeeMode {
    /// Intel SGX
    Sgx,
    /// AMD SEV-SNP
    Sev,
    /// Intel TDX (Trust Domain Extensions)
    Tdx,
    /// Simulated TEE for testing
    Simulated,
    /// No TEE (dev mode)
    None,
}

impl std::fmt::Display for TeeMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TeeMode::Sgx => write!(f, "sgx"),
            TeeMode::Sev => write!(f, "sev"),
            TeeMode::Tdx => write!(f, "tdx"),
            TeeMode::Simulated => write!(f, "simulated"),
            TeeMode::None => write!(f, "none"),
        }
    }
}

impl Config {
    /// Load configuration from environment variables
    pub fn from_env() -> Result<Self> {
        dotenvy::dotenv().ok();

        let host = std::env::var("SERVER_HOST").unwrap_or_else(|_| "0.0.0.0".to_string());
        let port = std::env::var("SERVER_PORT")
            .unwrap_or_else(|_| "8081".to_string())
            .parse::<u16>()
            .context("Invalid SERVER_PORT")?;

        let server_addr = format!("{}:{}", host, port)
            .parse()
            .context("Invalid server address")?;

        let near_network = std::env::var("NEAR_NETWORK").unwrap_or_else(|_| "testnet".to_string());
        let near_rpc_url = std::env::var("NEAR_RPC_URL")
            .context("NEAR_RPC_URL is required")?;
        let offchainvm_contract_id = std::env::var("OFFCHAINVM_CONTRACT_ID")
            .context("OFFCHAINVM_CONTRACT_ID is required")?;

        let allowed_worker_token_hashes = std::env::var("ALLOWED_WORKER_TOKEN_HASHES")
            .unwrap_or_default()
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();

        let allowed_coordinator_token_hashes = std::env::var("ALLOWED_COORDINATOR_TOKEN_HASHES")
            .unwrap_or_default()
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();

        let tee_mode = match std::env::var("TEE_MODE").unwrap_or_else(|_| "none".to_string()).as_str() {
            "sgx" => TeeMode::Sgx,
            "sev" => TeeMode::Sev,
            "tdx" => TeeMode::Tdx,
            "simulated" => TeeMode::Simulated,
            "none" => TeeMode::None,
            other => anyhow::bail!("Invalid TEE_MODE: {}", other),
        };

        Ok(Config {
            server_addr,
            near_network,
            near_rpc_url,
            offchainvm_contract_id,
            allowed_worker_token_hashes,
            allowed_coordinator_token_hashes,
            tee_mode,
        })
    }

    /// Validate configuration
    pub fn validate(&self) -> Result<()> {
        if self.allowed_worker_token_hashes.is_empty() {
            tracing::warn!("No worker token hashes configured - worker endpoints will reject all requests");
        }

        if self.allowed_coordinator_token_hashes.is_empty() {
            tracing::warn!("No coordinator token hashes configured - coordinator endpoints will reject all requests");
        }

        Ok(())
    }
}
