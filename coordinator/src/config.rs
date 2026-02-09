use std::path::PathBuf;

#[derive(Clone)]
pub struct Config {
    // HTTP server
    pub host: String,
    pub port: u16,

    // PostgreSQL
    pub database_url: String,
    pub db_pool_size: u32,

    // Redis
    pub redis_url: String,
    /// Queue for compilation tasks
    pub redis_queue_compile: String,
    /// Queue for execution tasks
    pub redis_queue_execute: String,

    // WASM cache
    pub wasm_cache_dir: PathBuf,
    pub wasm_cache_max_size_gb: u64,
    pub lru_eviction_check_interval_seconds: u64,

    // Auth
    pub require_auth: bool,

    // Timeouts
    pub task_poll_timeout_seconds: u64,
    pub lock_default_ttl_seconds: u64,

    // NEAR contract integration
    pub near_rpc_url: String,
    pub contract_id: String,

    // Keystore integration
    pub keystore_base_url: Option<String>,
    pub keystore_auth_token: Option<String>,

    // CORS
    pub cors_allowed_origins: Vec<String>,

    // Admin API
    pub admin_bearer_token: String,
    pub expected_worker_measurement: String,

    // Stablecoin configuration
    /// Stablecoin contract address (e.g., "17208628f84f5d6ad33f0da3bbbeb27ffcb398eac501a31bd6ad2011e36133a1" for native USDC on NEAR)
    pub stablecoin_contract: String,
    /// Token decimals (6 for USDC)
    pub stablecoin_decimals: u8,
    /// Token symbol (e.g., "USDC")
    pub stablecoin_symbol: String,

    // HTTPS API settings
    /// Default compute budget in minimal token units (e.g., 10000 = $0.01)
    pub default_compute_limit: u128,
    /// Minimum compute budget (protection against micro-spam)
    pub min_compute_limit: u128,
    /// HTTPS call timeout in seconds
    pub https_call_timeout_seconds: u64,
    /// Max concurrent calls per payment key
    pub max_concurrent_calls_per_key: u32,
    /// Rate limit for payment key (requests per minute)
    pub payment_key_rate_limit_per_minute: u32,

    // TEE session verification
    /// Operator account where TEE worker keys are registered as access keys (e.g., "worker.outlayer.testnet")
    pub operator_account_id: Option<String>,
    /// Require valid TEE session for HTTPS call completion and keystore access
    pub require_tee_session: bool,
}

impl Config {
    pub fn from_env() -> Result<Self, Box<dyn std::error::Error>> {
        dotenv::dotenv().ok();

        let config = Self {
            host: std::env::var("HOST").unwrap_or_else(|_| "0.0.0.0".to_string()),
            port: std::env::var("PORT")
                .unwrap_or_else(|_| "8080".to_string())
                .parse()?,

            database_url: std::env::var("DATABASE_URL")
                .unwrap_or_else(|_| "postgres://postgres:postgres@localhost/offchainvm".to_string()),
            db_pool_size: std::env::var("DB_POOL_SIZE")
                .unwrap_or_else(|_| "50".to_string())
                .parse()?,

            redis_url: std::env::var("REDIS_URL")
                .unwrap_or_else(|_| "redis://localhost:6379".to_string()),
            redis_queue_compile: std::env::var("REDIS_QUEUE_COMPILE")
                .unwrap_or_else(|_| "outlayer:compile".to_string()),
            redis_queue_execute: std::env::var("REDIS_QUEUE_EXECUTE")
                .unwrap_or_else(|_| "outlayer:execute".to_string()),

            wasm_cache_dir: PathBuf::from(
                std::env::var("WASM_CACHE_DIR")
                    .unwrap_or_else(|_| "/var/offchainvm/wasm".to_string()),
            ),
            wasm_cache_max_size_gb: std::env::var("WASM_CACHE_MAX_SIZE_GB")
                .unwrap_or_else(|_| "50".to_string())
                .parse()?,
            lru_eviction_check_interval_seconds: std::env::var("LRU_EVICTION_CHECK_INTERVAL_SECONDS")
                .unwrap_or_else(|_| "3600".to_string())
                .parse()?,

            require_auth: std::env::var("REQUIRE_AUTH")
                .unwrap_or_else(|_| "true".to_string())
                .parse()?,

            task_poll_timeout_seconds: std::env::var("TASK_POLL_TIMEOUT_SECONDS")
                .unwrap_or_else(|_| "60".to_string())
                .parse()?,
            lock_default_ttl_seconds: std::env::var("LOCK_DEFAULT_TTL_SECONDS")
                .unwrap_or_else(|_| "300".to_string())
                .parse()?,

            near_rpc_url: std::env::var("NEAR_RPC_URL")
                .unwrap_or_else(|_| "https://rpc.testnet.near.org".to_string()),
            contract_id: std::env::var("OFFCHAINVM_CONTRACT_ID")
                .unwrap_or_else(|_| "outlayer.testnet".to_string()),

            keystore_base_url: std::env::var("KEYSTORE_BASE_URL").ok(),
            keystore_auth_token: std::env::var("KEYSTORE_AUTH_TOKEN").ok(),

            cors_allowed_origins: std::env::var("CORS_ALLOWED_ORIGINS")
                .unwrap_or_else(|_| "http://localhost:3000".to_string())
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect(),

            admin_bearer_token: std::env::var("ADMIN_BEARER_TOKEN")
                .unwrap_or_else(|_| "change-this-in-production".to_string()),
            expected_worker_measurement: std::env::var("EXPECTED_WORKER_MEASUREMENT")
                .unwrap_or_else(|_| "0".repeat(96)),

            // Stablecoin configuration
            stablecoin_contract: std::env::var("STABLECOIN_CONTRACT")
                .unwrap_or_else(|_| "17208628f84f5d6ad33f0da3bbbeb27ffcb398eac501a31bd6ad2011e36133a1".to_string()),
            stablecoin_decimals: std::env::var("STABLECOIN_DECIMALS")
                .unwrap_or_else(|_| "6".to_string())
                .parse()?,
            stablecoin_symbol: std::env::var("STABLECOIN_SYMBOL")
                .unwrap_or_else(|_| "USDC".to_string()),

            // HTTPS API settings
            default_compute_limit: std::env::var("DEFAULT_COMPUTE_LIMIT")
                .unwrap_or_else(|_| "10000".to_string()) // $0.01 default
                .parse()?,
            min_compute_limit: std::env::var("MIN_COMPUTE_LIMIT")
                .unwrap_or_else(|_| "1000".to_string()) // $0.001 minimum
                .parse()?,
            https_call_timeout_seconds: std::env::var("HTTPS_CALL_TIMEOUT_SECONDS")
                .unwrap_or_else(|_| "300".to_string()) // 5 minutes
                .parse()?,
            max_concurrent_calls_per_key: std::env::var("MAX_CONCURRENT_CALLS_PER_KEY")
                .unwrap_or_else(|_| "10".to_string())
                .parse()?,
            payment_key_rate_limit_per_minute: std::env::var("PAYMENT_KEY_RATE_LIMIT_PER_MINUTE")
                .unwrap_or_else(|_| "1000".to_string())
                .parse()?,

            // TEE session verification
            operator_account_id: std::env::var("OPERATOR_ACCOUNT_ID").ok(),
            require_tee_session: std::env::var("REQUIRE_TEE_SESSION")
                .unwrap_or_else(|_| "false".to_string())
                .parse()?,
        };

        if config.require_tee_session && config.operator_account_id.is_none() {
            return Err("REQUIRE_TEE_SESSION=true requires OPERATOR_ACCOUNT_ID to be set".into());
        }

        Ok(config)
    }

    /// Detect network from contract_id
    /// Returns "testnet" or "mainnet"
    pub fn detect_network(&self) -> &'static str {
        if self.contract_id.ends_with(".testnet") {
            "testnet"
        } else {
            "mainnet"
        }
    }

    /// Generate keystore worker_id based on network
    /// Format: {network}-keystore
    pub fn keystore_worker_id(&self) -> String {
        format!("{}-keystore", self.detect_network())
    }
}
