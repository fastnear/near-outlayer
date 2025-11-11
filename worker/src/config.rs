use anyhow::{Context, Result};
use near_crypto::{InMemorySigner, SecretKey};
use near_primitives::types::AccountId;
use std::env;
use std::str::FromStr;

/// Worker configuration loaded from environment variables
#[derive(Debug, Clone)]
pub struct Config {
    // Coordinator API
    pub api_base_url: String,
    pub api_auth_token: String,

    // NEAR configuration
    pub near_rpc_url: String,
    pub neardata_api_url: String,
    pub fastnear_api_url: String,
    pub start_block_height: u64,
    pub offchainvm_contract_id: AccountId,
    #[allow(dead_code)]
    pub operator_account_id: AccountId,
    pub operator_signer: InMemorySigner,

    // Worker settings
    pub worker_id: String,
    pub enable_event_monitor: bool,
    pub poll_timeout_seconds: u64,
    pub scan_interval_ms: u64,

    // Docker for compilation
    pub docker_image: String,
    pub compile_timeout_seconds: u64,
    pub compile_memory_limit_mb: u64,
    pub compile_cpu_limit: f64,

    // WASM execution limits (defaults)
    pub default_max_instructions: u64,
    #[allow(dead_code)]
    pub default_max_memory_mb: u32,
    #[allow(dead_code)]
    pub default_max_execution_seconds: u64,

    // Keystore worker (optional - for secret decryption)
    pub keystore_base_url: Option<String>,
    pub keystore_auth_token: Option<String>,
    pub tee_mode: String,

    // Debug logging (admin only - stores raw stderr/stdout in system_hidden_logs table)
    // WARNING: These logs should NEVER be exposed via public API for security reasons
    // Set to false in production to disable raw log storage
    pub save_system_hidden_logs_to_debug: bool,

    // Print WASM stderr to worker logs (for debugging WASM execution)
    // Set to false in production to reduce log noise
    pub print_wasm_stderr: bool,
}

impl Config {
    /// Load configuration from environment variables
    ///
    /// Required environment variables:
    /// - API_BASE_URL: Coordinator API URL (e.g., http://localhost:8080)
    /// - API_AUTH_TOKEN: Bearer token for API authentication
    /// - NEAR_RPC_URL: NEAR RPC endpoint (e.g., https://rpc.testnet.near.org)
    /// - OFFCHAINVM_CONTRACT_ID: OffchainVM contract account ID
    /// - OPERATOR_ACCOUNT_ID: Worker operator account ID
    /// - OPERATOR_PRIVATE_KEY: Worker operator private key (ed25519:...)
    ///
    /// Optional environment variables (with defaults):
    /// - WORKER_ID: Unique worker identifier (default: random UUID)
    /// - ENABLE_EVENT_MONITOR: Enable NEAR event monitoring (default: false)
    /// - POLL_TIMEOUT_SECONDS: Long-poll timeout (default: 60)
    /// - DOCKER_IMAGE: Docker image for compilation (default: rust:1.75)
    /// - COMPILE_TIMEOUT_SECONDS: Compilation timeout (default: 300)
    /// - COMPILE_MEMORY_LIMIT_MB: Compilation memory limit (default: 2048)
    /// - COMPILE_CPU_LIMIT: Compilation CPU limit (default: 2.0)
    /// - DEFAULT_MAX_INSTRUCTIONS: Default instruction limit (default: 10_000_000_000)
    /// - DEFAULT_MAX_MEMORY_MB: Default memory limit (default: 128)
    /// - DEFAULT_MAX_EXECUTION_SECONDS: Default execution timeout (default: 60)
    pub fn from_env() -> Result<Self> {
        // Load .env file if present
        dotenv::dotenv().ok();

        // Required fields
        let api_base_url = env::var("API_BASE_URL")
            .context("API_BASE_URL environment variable is required")?;

        let api_auth_token = env::var("API_AUTH_TOKEN")
            .context("API_AUTH_TOKEN environment variable is required")?;

        let near_rpc_url = env::var("NEAR_RPC_URL")
            .context("NEAR_RPC_URL environment variable is required")?;

        let neardata_api_url = env::var("NEARDATA_API_URL")
            .unwrap_or_else(|_| "https://testnet.neardata.xyz/v0/block".to_string());

        let fastnear_api_url = env::var("FASTNEAR_API_URL")
            .unwrap_or_else(|_| {
                // Auto-detect based on neardata URL
                if neardata_api_url.contains("mainnet") {
                    "https://api.fastnear.com/status".to_string()
                } else {
                    "https://test.api.fastnear.com/status".to_string()
                }
            });

        let start_block_height = env::var("START_BLOCK_HEIGHT")
            .unwrap_or_else(|_| "0".to_string())
            .parse::<u64>()
            .context("START_BLOCK_HEIGHT must be a valid number")?;

        let offchainvm_contract_id = env::var("OFFCHAINVM_CONTRACT_ID")
            .context("OFFCHAINVM_CONTRACT_ID environment variable is required")?;
        let offchainvm_contract_id = AccountId::from_str(&offchainvm_contract_id)
            .context("Invalid OFFCHAINVM_CONTRACT_ID format")?;

        let operator_account_id = env::var("OPERATOR_ACCOUNT_ID")
            .context("OPERATOR_ACCOUNT_ID environment variable is required")?;
        let operator_account_id = AccountId::from_str(&operator_account_id)
            .context("Invalid OPERATOR_ACCOUNT_ID format")?;

        let operator_private_key = env::var("OPERATOR_PRIVATE_KEY")
            .context("OPERATOR_PRIVATE_KEY environment variable is required")?;
        let operator_secret_key = SecretKey::from_str(&operator_private_key)
            .context("Invalid OPERATOR_PRIVATE_KEY format (expected ed25519:...)")?;

        let operator_signer = InMemorySigner::from_secret_key(
            operator_account_id.clone(),
            operator_secret_key,
        );

        // Optional fields with defaults
        let worker_id = env::var("WORKER_ID")
            .unwrap_or_else(|_| uuid::Uuid::new_v4().to_string());

        let enable_event_monitor = env::var("ENABLE_EVENT_MONITOR")
            .unwrap_or_else(|_| "false".to_string())
            .parse::<bool>()
            .context("ENABLE_EVENT_MONITOR must be 'true' or 'false'")?;

        let poll_timeout_seconds = env::var("POLL_TIMEOUT_SECONDS")
            .unwrap_or_else(|_| "60".to_string())
            .parse::<u64>()
            .context("POLL_TIMEOUT_SECONDS must be a valid number")?;

        let scan_interval_ms = env::var("SCAN_INTERVAL_MS")
            .unwrap_or_else(|_| "0".to_string())
            .parse::<u64>()
            .context("SCAN_INTERVAL_MS must be a valid number")?;

        let docker_image = env::var("DOCKER_IMAGE")
            .unwrap_or_else(|_| "zavodil/wasmedge-compiler:latest".to_string());

        let compile_timeout_seconds = env::var("COMPILE_TIMEOUT_SECONDS")
            .unwrap_or_else(|_| "300".to_string())
            .parse::<u64>()
            .context("COMPILE_TIMEOUT_SECONDS must be a valid number")?;

        let compile_memory_limit_mb = env::var("COMPILE_MEMORY_LIMIT_MB")
            .unwrap_or_else(|_| "2048".to_string())
            .parse::<u64>()
            .context("COMPILE_MEMORY_LIMIT_MB must be a valid number")?;

        let compile_cpu_limit = env::var("COMPILE_CPU_LIMIT")
            .unwrap_or_else(|_| "2.0".to_string())
            .parse::<f64>()
            .context("COMPILE_CPU_LIMIT must be a valid number")?;

        let default_max_instructions = env::var("DEFAULT_MAX_INSTRUCTIONS")
            .unwrap_or_else(|_| "10000000000".to_string())
            .parse::<u64>()
            .context("DEFAULT_MAX_INSTRUCTIONS must be a valid number")?;

        let default_max_memory_mb = env::var("DEFAULT_MAX_MEMORY_MB")
            .unwrap_or_else(|_| "128".to_string())
            .parse::<u32>()
            .context("DEFAULT_MAX_MEMORY_MB must be a valid number")?;

        let default_max_execution_seconds = env::var("DEFAULT_MAX_EXECUTION_SECONDS")
            .unwrap_or_else(|_| "60".to_string())
            .parse::<u64>()
            .context("DEFAULT_MAX_EXECUTION_SECONDS must be a valid number")?;

        // Keystore configuration (optional)
        let keystore_base_url = env::var("KEYSTORE_BASE_URL").ok();
        let keystore_auth_token = env::var("KEYSTORE_AUTH_TOKEN").ok();

        let tee_mode = env::var("TEE_MODE")
            .unwrap_or_else(|_| "none".to_string());

        // Validate TEE mode
        if !["sgx", "sev", "simulated", "none"].contains(&tee_mode.as_str()) {
            anyhow::bail!("Invalid TEE_MODE: must be one of: sgx, sev, simulated, none");
        }

        // Debug logging flag (default: true = enabled)
        // WARNING: Stores raw stderr/stdout in system_hidden_logs table
        // Set SAVE_SYSTEM_HIDDEN_LOGS_TO_DEBUG=false in production to disable
        let save_system_hidden_logs_to_debug = env::var("SAVE_SYSTEM_HIDDEN_LOGS_TO_DEBUG")
            .unwrap_or_else(|_| "true".to_string())
            .parse::<bool>()
            .unwrap_or(true);

        // Set PRINT_WASM_STDERR=true to see WASM stderr in worker logs
        let print_wasm_stderr = env::var("PRINT_WASM_STDERR")
            .unwrap_or_else(|_| "false".to_string())
            .parse::<bool>()
            .unwrap_or(false);

        Ok(Self {
            api_base_url,
            api_auth_token,
            near_rpc_url,
            neardata_api_url,
            fastnear_api_url,
            start_block_height,
            offchainvm_contract_id,
            operator_account_id,
            operator_signer,
            worker_id,
            enable_event_monitor,
            poll_timeout_seconds,
            scan_interval_ms,
            docker_image,
            compile_timeout_seconds,
            compile_memory_limit_mb,
            compile_cpu_limit,
            default_max_instructions,
            default_max_memory_mb,
            default_max_execution_seconds,
            keystore_base_url,
            keystore_auth_token,
            tee_mode,
            save_system_hidden_logs_to_debug,
            print_wasm_stderr,
        })
    }

    /// Validate configuration
    pub fn validate(&self) -> Result<()> {
        if self.api_base_url.is_empty() {
            anyhow::bail!("API base URL cannot be empty");
        }

        if self.api_auth_token.is_empty() {
            anyhow::bail!("API auth token cannot be empty");
        }

        if self.near_rpc_url.is_empty() {
            anyhow::bail!("NEAR RPC URL cannot be empty");
        }

        if self.poll_timeout_seconds == 0 || self.poll_timeout_seconds > 300 {
            anyhow::bail!("Poll timeout must be between 1 and 300 seconds");
        }

        if self.compile_timeout_seconds == 0 || self.compile_timeout_seconds > 3600 {
            anyhow::bail!("Compile timeout must be between 1 and 3600 seconds");
        }

        if self.compile_memory_limit_mb < 512 {
            anyhow::bail!("Compile memory limit must be at least 512 MB");
        }

        if self.compile_cpu_limit <= 0.0 {
            anyhow::bail!("Compile CPU limit must be positive");
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_validation() {
        let mut config = create_test_config();
        assert!(config.validate().is_ok());

        // Test invalid poll timeout
        config.poll_timeout_seconds = 0;
        assert!(config.validate().is_err());

        config.poll_timeout_seconds = 60;
        assert!(config.validate().is_ok());

        // Test invalid memory limit
        config.compile_memory_limit_mb = 100;
        assert!(config.validate().is_err());
    }

    fn create_test_config() -> Config {
        Config {
            api_base_url: "http://localhost:8080".to_string(),
            api_auth_token: "test-token".to_string(),
            near_rpc_url: "https://rpc.testnet.near.org".to_string(),
            neardata_api_url: "https://testnet.neardata.xyz/v0/block".to_string(),
            fastnear_api_url: "https://test.api.fastnear.com/status".to_string(),
            start_block_height: 0,
            offchainvm_contract_id: "outlayer.testnet".parse().unwrap(),
            operator_account_id: "worker.testnet".parse().unwrap(),
            operator_signer: InMemorySigner::from_secret_key(
                "worker.testnet".parse().unwrap(),
                "ed25519:3D4YudUahN1nawWvHfEKBGpmJLfbCTbvdXDJKqfLhQ98XewyWK4tEDWvmAYPZqcgz7qfkCEHyWD15m8JVVWJ3LXD".parse().unwrap(),
            ),
            worker_id: "test-worker".to_string(),
            enable_event_monitor: false,
            poll_timeout_seconds: 60,
            scan_interval_ms: 0,
            docker_image: "rust:1.75".to_string(),
            compile_timeout_seconds: 300,
            compile_memory_limit_mb: 2048,
            compile_cpu_limit: 2.0,
            default_max_instructions: 10_000_000_000,
            default_max_memory_mb: 128,
            default_max_execution_seconds: 60,
            keystore_base_url: None,
            keystore_auth_token: None,
            tee_mode: "none".to_string(),
            save_system_hidden_logs_to_debug: true, // Default: enabled for debugging
            print_wasm_stderr: false,
        }
    }
}
