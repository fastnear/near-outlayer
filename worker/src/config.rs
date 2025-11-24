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
    pub operator_signer: Option<InMemorySigner>,

    // Worker settings
    pub worker_id: String,
    pub enable_event_monitor: bool,
    pub poll_timeout_seconds: u64,
    pub scan_interval_ms: u64,

    // Compilation mode
    // "docker" - use Docker containers (requires Docker socket)
    // "native" - use native Rust toolchain with bubblewrap (for TEE/Phala)
    pub compilation_mode: String,

    // Docker for compilation (only used in "docker" mode)
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

    // Worker registration mode
    // If true - use TEE registration flow (requires REGISTER_CONTRACT_ID and INIT_ACCOUNT_*)
    // If false - use legacy mode with OPERATOR_PRIVATE_KEY (for testnet without TEE)
    // CRITICAL: OPERATOR_PRIVATE_KEY is ONLY for resolve_execution(), NEVER for user transactions!
    pub use_tee_registration: bool,

    // Worker registration (optional - for TEE key registration)
    pub register_contract_id: Option<AccountId>,

    // Init account (optional - for paying gas on worker registration)
    // If not set, operator_signer will be used for registration
    pub init_account_id: Option<AccountId>,
    pub init_account_signer: Option<InMemorySigner>,

    // Debug logging (admin only - stores raw stderr/stdout in system_hidden_logs table)
    // WARNING: These logs should NEVER be exposed via public API for security reasons
    // Set to false in production to disable raw log storage
    pub save_system_hidden_logs_to_debug: bool,

    // Print WASM stderr to worker logs (for debugging WASM execution)
    // Set to false in production to reduce log noise
    pub print_wasm_stderr: bool,

    // Worker capabilities (what this worker can do)
    pub capabilities: WorkerCapabilities,

    // FastFS receiver contract (optional - for storing compiled WASM)
    pub fastfs_receiver: Option<String>,

    // FastFS sender account (optional - separate account for paying FastFS storage)
    pub fastfs_sender_signer: Option<InMemorySigner>,

    // RPC Proxy configuration (for WASM host functions)
    #[allow(dead_code)]
    pub rpc_proxy: RpcProxyConfig,
}

/// RPC Proxy configuration for WASM host functions
#[derive(Debug, Clone)]
pub struct RpcProxyConfig {
    /// Enable RPC proxy host functions for WASM
    #[allow(dead_code)]
    pub enabled: bool,
    /// RPC URL with API key (separate from worker's NEAR_RPC_URL)
    /// If not set, falls back to worker's near_rpc_url
    pub rpc_url: Option<String>,
    /// Maximum RPC calls per execution (rate limiting)
    pub max_calls_per_execution: u32,
    /// Allow transaction methods (send_tx, broadcast_tx_*)
    /// If false, only view methods are allowed
    pub allow_transactions: bool,
}

/// Worker capabilities - what jobs this worker can handle
#[derive(Debug, Clone)]
pub struct WorkerCapabilities {
    pub compilation: bool, // Can compile GitHub repos to WASM
    pub execution: bool,   // Can execute WASM code
}

impl WorkerCapabilities {
    /// Convert capabilities to string array for API
    pub fn to_array(&self) -> Vec<String> {
        let mut result = Vec::new();
        if self.compilation {
            result.push("compilation".to_string());
        }
        if self.execution {
            result.push("execution".to_string());
        }
        result
    }

    /// Check if worker can handle compilation
    pub fn can_compile(&self) -> bool {
        self.compilation
    }

    /// Check if worker can handle execution
    #[allow(dead_code)]
    pub fn can_execute(&self) -> bool {
        self.execution
    }
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
    ///   ⚠️ CRITICAL: This key is ONLY for calling resolve_execution() on OutLayer contract!
    ///   This key is NEVER passed to WASM and NEVER used for signing user transactions.
    ///   User transactions are signed with keys provided by WASM via secrets mechanism.
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

        // Check if TEE registration is enabled
        let use_tee_registration = env::var("USE_TEE_REGISTRATION")
            .unwrap_or_else(|_| "true".to_string()) // Default: true (TEE mode for production)
            .parse::<bool>()
            .context("USE_TEE_REGISTRATION must be 'true' or 'false'")?;

        // Load operator_signer based on registration mode
        // CRITICAL: operator_signer is ONLY used for calling resolve_execution() on OutLayer contract
        // This key is NEVER passed to WASM and NEVER used for user transactions!
        // User transactions are signed with keys from WASM (provided via secrets mechanism).
        let operator_signer = if use_tee_registration {
            // TEE mode: operator_signer will be set after registration in main.rs
            None
        } else {
            // Legacy mode: load OPERATOR_PRIVATE_KEY from env
            // CRITICAL: This key is ONLY for resolve_execution(), NOT for user transactions!
            let operator_private_key = env::var("OPERATOR_PRIVATE_KEY")
                .context("OPERATOR_PRIVATE_KEY is required when USE_TEE_REGISTRATION=false")?;

            let secret_key: SecretKey = operator_private_key
                .parse()
                .context("Invalid OPERATOR_PRIVATE_KEY format (expected ed25519:...)")?;

            Some(InMemorySigner::from_secret_key(
                operator_account_id.clone(),
                secret_key,
            ))
        };

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

        // Compilation mode: docker (default) or native (bubblewrap)
        let compilation_mode = env::var("COMPILATION_MODE")
            .unwrap_or_else(|_| "docker".to_string())
            .to_lowercase();

        // Validate compilation mode
        if !["docker", "native"].contains(&compilation_mode.as_str()) {
            anyhow::bail!("Invalid COMPILATION_MODE: '{}'. Must be 'docker' or 'native'", compilation_mode);
        }

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

        let tee_mode_raw = env::var("TEE_MODE")
            .unwrap_or_else(|_| "none".to_string());
        // Remove quotes if present (Phala Cloud may add them)
        let tee_mode = tee_mode_raw
            .trim()
            .trim_matches('"')
            .trim_matches('\'')
            .to_lowercase();

        // Validate TEE mode (case-insensitive, trimmed)
        if !["tdx", "sgx", "sev", "simulated", "none"].contains(&tee_mode.as_str()) {
            anyhow::bail!(
                "Invalid TEE_MODE: received '{}' (raw: '{:?}'), must be one of: tdx, sgx, sev, simulated, none (case-insensitive)",
                tee_mode,
                tee_mode_raw
            );
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

        // Worker capabilities - what this worker can do
        let compilation_enabled = env::var("COMPILATION_ENABLED")
            .unwrap_or_else(|_| "true".to_string())
            .parse::<bool>()
            .context("COMPILATION_ENABLED must be 'true' or 'false'")?;

        let execution_enabled = env::var("EXECUTION_ENABLED")
            .unwrap_or_else(|_| "true".to_string())
            .parse::<bool>()
            .context("EXECUTION_ENABLED must be 'true' or 'false'")?;

        // At least one capability must be enabled
        if !compilation_enabled && !execution_enabled {
            anyhow::bail!("At least one capability must be enabled (COMPILATION_ENABLED or EXECUTION_ENABLED)");
        }

        let capabilities = WorkerCapabilities {
            compilation: compilation_enabled,
            execution: execution_enabled,
        };

        // FastFS receiver contract (optional)
        let fastfs_receiver = env::var("FASTFS_RECEIVER").ok();

        // FastFS sender account (optional - separate account for paying storage)
        let fastfs_sender_signer = if let Ok(sender_account_id) = env::var("FASTFS_SENDER_ACCOUNT_ID") {
            let sender_private_key = env::var("FASTFS_SENDER_PRIVATE_KEY")
                .context("FASTFS_SENDER_PRIVATE_KEY is required when FASTFS_SENDER_ACCOUNT_ID is set")?;

            let sender_account = AccountId::from_str(&sender_account_id)
                .context("Invalid FASTFS_SENDER_ACCOUNT_ID format")?;

            let secret_key: SecretKey = sender_private_key
                .parse()
                .context("Invalid FASTFS_SENDER_PRIVATE_KEY format (expected ed25519:...)")?;

            Some(InMemorySigner::from_secret_key(sender_account, secret_key))
        } else {
            None
        };

        // Worker registration configuration (optional)
        let register_contract_id = env::var("REGISTER_CONTRACT_ID")
            .ok()
            .map(|id| AccountId::from_str(&id))
            .transpose()
            .context("Invalid REGISTER_CONTRACT_ID format")?;

        // Init account (optional - for paying gas on worker registration)
        let (init_account_id, init_account_signer) = if let Ok(init_account_str) = env::var("INIT_ACCOUNT_ID") {
            let init_account_id = AccountId::from_str(&init_account_str)
                .context("Invalid INIT_ACCOUNT_ID format")?;

            let init_private_key_str = env::var("INIT_ACCOUNT_PRIVATE_KEY")
                .context("INIT_ACCOUNT_PRIVATE_KEY is required when INIT_ACCOUNT_ID is set")?;

            let init_private_key = SecretKey::from_str(&init_private_key_str)
                .context("Invalid INIT_ACCOUNT_PRIVATE_KEY format")?;

            let init_signer = InMemorySigner::from_secret_key(
                init_account_id.clone(),
                init_private_key,
            );

            (Some(init_account_id), Some(init_signer))
        } else {
            (None, None)
        };

        // NEAR RPC Proxy configuration (for WASM host functions)
        let rpc_proxy_enabled = env::var("NEAR_RPC_PROXY_ENABLED")
            .unwrap_or_else(|_| "true".to_string())
            .parse::<bool>()
            .context("NEAR_RPC_PROXY_ENABLED must be 'true' or 'false'")?;

        let rpc_proxy_url = env::var("NEAR_RPC_PROXY_URL").ok();

        let rpc_proxy_max_calls = env::var("NEAR_RPC_PROXY_MAX_CALLS")
            .unwrap_or_else(|_| "100".to_string())
            .parse::<u32>()
            .context("NEAR_RPC_PROXY_MAX_CALLS must be a valid number")?;

        let rpc_proxy_allow_transactions = env::var("NEAR_RPC_PROXY_ALLOW_TRANSACTIONS")
            .unwrap_or_else(|_| "true".to_string())
            .parse::<bool>()
            .context("NEAR_RPC_PROXY_ALLOW_TRANSACTIONS must be 'true' or 'false'")?;

        let rpc_proxy = RpcProxyConfig {
            enabled: rpc_proxy_enabled,
            rpc_url: rpc_proxy_url,
            max_calls_per_execution: rpc_proxy_max_calls,
            allow_transactions: rpc_proxy_allow_transactions,
        };

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
            compilation_mode,
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
            use_tee_registration,
            register_contract_id,
            init_account_id,
            init_account_signer,
            save_system_hidden_logs_to_debug,
            print_wasm_stderr,
            capabilities,
            fastfs_receiver,
            fastfs_sender_signer,
            rpc_proxy,
        })
    }

    /// Set operator signer after registration
    pub fn set_operator_signer(&mut self, signer: InMemorySigner) {
        self.operator_signer = Some(signer);
    }

    /// Get operator signer (panics if not set)
    pub fn get_operator_signer(&self) -> &InMemorySigner {
        self.operator_signer.as_ref().expect("Operator signer not set - registration must have failed")
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

        // Security check: native compilation mode must be isolated
        // because malicious build scripts can steal secrets via environment variables
        if self.compilation_mode == "native" {
            // Native compilation requires strict isolation
            if self.capabilities.execution {
                anyhow::bail!(
                    "Security error: Native compilation mode (COMPILATION_MODE=native) \
                     must NOT have EXECUTION_ENABLED=true. \
                     Malicious build scripts can steal secrets from environment variables. \
                     Set EXECUTION_ENABLED=false for native compiler workers."
                );
            }
            if self.init_account_signer.is_some() {
                anyhow::bail!(
                    "Security error: Native compilation mode (COMPILATION_MODE=native) \
                     must NOT have INIT_ACCOUNT_PRIVATE_KEY set. \
                     Malicious build scripts can steal secrets from environment variables. \
                     Remove INIT_ACCOUNT_PRIVATE_KEY from .env for native compiler workers."
                );
            }
            if self.operator_signer.is_some() {
                anyhow::bail!(
                    "Security error: Native compilation mode (COMPILATION_MODE=native) \
                     must NOT have OPERATOR_PRIVATE_KEY set. \
                     Malicious build scripts can steal secrets from environment variables. \
                     Remove OPERATOR_PRIVATE_KEY from .env for native compiler workers."
                );
            }
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
            operator_signer: Some(InMemorySigner::from_secret_key(
                "worker.testnet".parse().unwrap(),
                "ed25519:3D4YudUahN1nawWvHfEKBGpmJLfbCTbvdXDJKqfLhQ98XewyWK4tEDWvmAYPZqcgz7qfkCEHyWD15m8JVVWJ3LXD".parse().unwrap(),
            )),
            worker_id: "test-worker".to_string(),
            enable_event_monitor: false,
            poll_timeout_seconds: 60,
            scan_interval_ms: 0,
            compilation_mode: "docker".to_string(),
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
            use_tee_registration: false, // Test mode: use legacy with OPERATOR_PRIVATE_KEY
            register_contract_id: None,
            init_account_id: None,
            init_account_signer: None,
            save_system_hidden_logs_to_debug: true, // Default: enabled for debugging
            print_wasm_stderr: false,
            capabilities: WorkerCapabilities {
                compilation: true,
                execution: true,
            },
            fastfs_receiver: None,
            fastfs_sender_signer: None,
            rpc_proxy: RpcProxyConfig {
                enabled: true,
                rpc_url: None,
                max_calls_per_execution: 100,
                allow_transactions: true,
            },
        }
    }
}
