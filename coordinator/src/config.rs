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
    pub redis_task_queue: String,

    // WASM cache
    pub wasm_cache_dir: PathBuf,
    pub wasm_cache_max_size_gb: u64,
    pub lru_eviction_check_interval_seconds: u64,

    // Auth
    pub require_auth: bool,

    // Timeouts
    pub task_poll_timeout_seconds: u64,
    pub lock_default_ttl_seconds: u64,
}

impl Config {
    pub fn from_env() -> Result<Self, Box<dyn std::error::Error>> {
        dotenv::dotenv().ok();

        Ok(Self {
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
            redis_task_queue: std::env::var("REDIS_TASK_QUEUE")
                .unwrap_or_else(|_| "offchainvm:tasks".to_string()),

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
        })
    }
}
