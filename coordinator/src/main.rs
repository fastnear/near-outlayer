mod auth;
mod background_jobs;
mod config;
mod github;
mod handlers;
mod middleware;
mod models;
mod near_client;
mod storage;

use axum::{
    routing::{delete, get, post},
    Router,
    extract::DefaultBodyLimit,
    http::{HeaderValue, Method},
};
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use tokio::sync::RwLock;
use tower_http::{cors::CorsLayer, trace::TraceLayer};
use tracing::info;

use config::Config;
use storage::lru_eviction::LruEviction;
use models::PricingConfig;

#[derive(Clone)]
pub struct AppState {
    pub db: sqlx::PgPool,
    pub redis: redis::Client,
    pub config: Arc<Config>,
    pub lru_eviction: Arc<LruEviction>,
    pub pricing: Arc<RwLock<PricingConfig>>,  // Pricing from contract
    pub pricing_updated_at: Arc<RwLock<SystemTime>>,  // Last update time
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "offchainvm_coordinator=debug,tower_http=debug".into()),
        )
        .init();

    // Load config
    let config = Arc::new(Config::from_env()?);
    info!("Config loaded successfully");

    // Initialize database pool
    let db = sqlx::postgres::PgPoolOptions::new()
        .max_connections(config.db_pool_size)
        .connect(&config.database_url)
        .await?;
    info!("Database connected");

    // Run migrations
    sqlx::migrate!("./migrations").run(&db).await?;
    info!("Database migrations completed");

    // Initialize Redis client
    let redis = redis::Client::open(config.redis_url.clone())?;
    info!("Redis client initialized");

    // Create WASM cache directory if it doesn't exist
    tokio::fs::create_dir_all(&config.wasm_cache_dir).await?;
    info!("WASM cache directory: {:?}", config.wasm_cache_dir);

    // Initialize LRU eviction
    let lru_eviction = Arc::new(LruEviction::new(
        db.clone(),
        config.wasm_cache_dir.clone(),
        config.wasm_cache_max_size_gb * 1024 * 1024 * 1024,
    ));

    // Start LRU eviction background task
    let eviction_interval = Duration::from_secs(config.lru_eviction_check_interval_seconds);
    let lru_eviction_clone = lru_eviction.clone();
    tokio::spawn(async move {
        lru_eviction_clone.run_periodic_check(eviction_interval).await;
    });
    info!("LRU eviction task started");

    // Start payment key cleanup background task
    let db_for_cleanup = db.clone();
    tokio::spawn(async move {
        background_jobs::run_payment_key_cleanup(
            db_for_cleanup,
            background_jobs::PaymentKeyCleanupConfig::default(),
        )
        .await;
    });
    info!("Payment key cleanup task started (every 5 min, stale threshold 10 min)");

    // Start TEE challenge cleanup background task (removes expired challenges)
    let db_for_tee_cleanup = db.clone();
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_secs(60)).await;
            let _ = sqlx::query("DELETE FROM tee_challenges WHERE created_at < NOW() - INTERVAL '60 seconds'")
                .execute(&db_for_tee_cleanup)
                .await;
        }
    });
    info!("TEE challenge cleanup task started (every 60s)");

    // Fetch pricing from contract
    info!("📡 Fetching initial pricing from NEAR contract...");
    let initial_pricing = match near_client::fetch_pricing_from_contract(
        &config.near_rpc_url,
        &config.contract_id,
    )
    .await
    {
        Ok(pricing) => pricing,
        Err(e) => {
            tracing::warn!("⚠️ Failed to fetch pricing from contract: {}. Using defaults.", e);
            near_client::get_default_pricing()
        }
    };

    // Build application state
    let state = AppState {
        db,
        redis,
        config: config.clone(),
        lru_eviction,
        pricing: Arc::new(RwLock::new(initial_pricing)),
        pricing_updated_at: Arc::new(RwLock::new(SystemTime::now())),
    };

    // Initialize IP rate limiter for public endpoints (10 requests/minute)
    // Protects keystore (which runs in TEE) from spam/DoS
    // Cleanup happens lazily when HashMap exceeds 1000 entries
    let ip_rate_limiter = Arc::new(middleware::ip_rate_limit::IpRateLimiter::new(10));
    info!("IP rate limiter initialized (10 req/min for /secrets/*)");

    // Initialize IP rate limiter for public storage endpoints (100 requests/minute)
    let public_storage_rate_limiter = Arc::new(middleware::ip_rate_limit::IpRateLimiter::new(100));
    info!("IP rate limiter initialized (100 req/min for /public/storage/*)");

    // Build protected routes (require auth)
    let protected = Router::new()
        // Job endpoints (protected)
        .route("/jobs/claim", post(handlers::jobs::claim_job))
        .route("/jobs/complete", post(handlers::jobs::complete_job))
        // Execution request endpoints (protected) - for Redis queue management
        .route("/executions/poll", get(handlers::tasks::poll_task))
        .route("/executions/create", post(handlers::tasks::create_task))
        // WASM cache endpoints (protected)
        .route("/wasm/:checksum", get(handlers::wasm::get_wasm))
        .route("/wasm/upload", post(handlers::wasm::upload_wasm))
        .route("/wasm/exists/:checksum", get(handlers::wasm::wasm_exists))
        // Lock endpoints (protected)
        .route("/locks/acquire", post(handlers::locks::acquire_lock))
        .route(
            "/locks/release/:lock_key",
            delete(handlers::locks::release_lock),
        )
        // Worker endpoints (protected)
        .route("/workers/heartbeat", post(handlers::workers::heartbeat))
        .route(
            "/workers/task-completion",
            post(handlers::workers::notify_task_completion),
        )
        // TEE session management
        .route("/workers/tee-challenge", post(handlers::workers::tee_challenge))
        .route("/workers/register-tee", post(handlers::workers::register_tee))
        // Attestation storage endpoint (worker-protected)
        .route("/attestations", post(handlers::attestations::store_attestation))
        // GitHub API endpoint (protected - only workers need it)
        .route("/github/resolve-branch", get(handlers::github::resolve_branch))
        // Storage endpoints (worker-protected)
        .route("/storage/set", post(handlers::storage::storage_set))
        .route("/storage/set-if-absent", post(handlers::storage::storage_set_if_absent))
        .route("/storage/set-if-equals", post(handlers::storage::storage_set_if_equals))
        .route("/storage/get", post(handlers::storage::storage_get))
        .route("/storage/get-by-version", post(handlers::storage::storage_get_by_version))
        .route("/storage/has", post(handlers::storage::storage_has))
        .route("/storage/delete", post(handlers::storage::storage_delete))
        .route("/storage/list", get(handlers::storage::storage_list))
        .route("/storage/usage", get(handlers::storage::storage_usage))
        .route("/storage/clear-all", post(handlers::storage::storage_clear_all))
        .route("/storage/clear-version", post(handlers::storage::storage_clear_version))
        .route("/storage/clear-project", post(handlers::storage::storage_clear_project))
        .route("/storage/get-public", post(handlers::storage::storage_get_public))
        // Project endpoints (worker-protected)
        .route("/projects/uuid", get(handlers::projects::resolve_project_uuid))
        .route("/projects/cache", delete(handlers::projects::invalidate_project_cache))
        // TopUp endpoints (worker-protected)
        .route("/topup/create", post(handlers::topup::create_topup_task))
        .route("/topup/complete", post(handlers::topup::complete_topup))
        // DeletePaymentKey task endpoints (worker-protected)
        .route("/payment-keys/delete-task/create", post(handlers::topup::create_delete_payment_key_task))
        // ProjectStorageCleanup task endpoints (worker-protected)
        .route("/projects/cleanup-task/create", post(handlers::topup::create_project_storage_cleanup_task))
        // Unified system callbacks poll endpoint
        .route("/system-callbacks/poll", get(handlers::topup::poll_system_callback_task))
        // HTTPS call completion endpoint (worker-protected)
        .route("/https-calls/complete", post(handlers::call::complete_https_call))
        // Payment key deletion (worker-protected, called after Delete events)
        .route("/payment-keys/delete", post(handlers::topup::delete_payment_key))
        // Payment key initialization (worker-protected, called on store_secrets with amount=0)
        .route("/payment-keys/init", post(handlers::topup::init_payment_key))
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            auth::auth_middleware,
        ))
        .layer(DefaultBodyLimit::max(10 * 1024 * 1024)); // 10 MB for worker responses

    // Build secrets routes (rate limited to protect keystore)
    // These endpoints proxy to keystore which runs in TEE and may be slow
    let secrets_routes = Router::new()
        .route("/secrets/pubkey", post(handlers::github::get_secrets_pubkey))
        .route("/secrets/add_generated_secret", post(handlers::github::add_generated_secret))
        .route("/secrets/update_user_secrets", post(handlers::github::update_user_secrets))
        .layer(axum::middleware::from_fn_with_state(
            ip_rate_limiter.clone(),
            middleware::ip_rate_limit::ip_rate_limit_middleware,
        ))
        .with_state(state.clone());

    // Build public routes (no auth required)
    let public = Router::new()
        .route("/public/workers", get(handlers::public::list_workers))
        .route("/public/jobs", get(handlers::public::list_jobs))
        .route("/public/stats", get(handlers::public::get_stats))
        .route("/public/repos/popular", get(handlers::public::get_popular_repos))
        .route("/public/wasm/info", get(handlers::public::get_wasm_info))
        .route("/public/wasm/exists/:checksum", get(handlers::wasm::wasm_exists))
        .route("/public/pricing", get(handlers::pricing::get_pricing))
        .route("/public/pricing/refresh", post(handlers::pricing::refresh_pricing))
        .route(
            "/public/users/:user_account_id/earnings",
            get(handlers::public::get_user_earnings),
        )
        .route("/public/projects/storage", get(handlers::public::get_project_storage))
        // Payment Key balance and usage (public - no auth required)
        .route(
            "/public/payment-keys/:owner/:nonce/balance",
            get(handlers::call::get_payment_key_balance),
        )
        .route(
            "/public/payment-keys/:owner/:nonce/usage",
            get(handlers::call::get_payment_key_usage),
        )
        // Project Owner Earnings (public - no auth required)
        .route(
            "/public/project-earnings/:project_owner",
            get(handlers::public::get_project_owner_earnings),
        )
        .route(
            "/public/project-earnings/:project_owner/history",
            get(handlers::public::get_project_owner_earnings_history),
        )
        .route("/health", get(|| async { "OK" }))
        .route("/health/detailed", get(handlers::health::health_detailed))
        // Attestation endpoint (public with IP rate limiting)
        .route("/attestations/:job_id", get(handlers::attestations::get_attestation));

    // Build public storage routes (rate limited - 100 req/min per IP, permissive CORS)
    let cors_public_storage = CorsLayer::new()
        .allow_methods([Method::GET, Method::POST, Method::OPTIONS])
        .allow_headers([axum::http::header::CONTENT_TYPE])
        .allow_origin(tower_http::cors::Any);
    let public_storage = Router::new()
        .route("/public/storage/get", get(handlers::public::get_public_storage))
        .route("/public/storage/batch", post(handlers::public::batch_get_public_storage))
        .layer(axum::middleware::from_fn_with_state(
            public_storage_rate_limiter.clone(),
            middleware::ip_rate_limit::ip_rate_limit_middleware,
        ))
        .layer(cors_public_storage)
        .with_state(state.clone());

    // Build internal routes (no auth - for worker communication only)
    // These endpoints are NOT exposed externally, workers use internal network
    let internal = Router::new()
        .route("/internal/system-logs", post(handlers::internal::store_system_log))
        .with_state(state.clone());

    // Build admin routes (require admin bearer token)
    let admin = Router::new()
        .route("/admin/compile-logs/:job_id", get(handlers::internal::get_compile_logs))
        // Grant keys management
        .route("/admin/grant-payment-key", post(handlers::grant_keys::grant_payment_key))
        .route("/admin/grant-keys", get(handlers::grant_keys::list_grant_keys))
        .route("/admin/grant-keys/:owner/:nonce", delete(handlers::grant_keys::delete_grant_key))
        // Worker management
        .route("/admin/workers/:worker_id", delete(handlers::workers::delete_worker))
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            middleware::admin_auth::admin_auth,
        ))
        .with_state(state.clone());

    // Build HTTPS API routes (Payment Key authenticated)
    // Rate limited by IP (100 req/min) before payment key validation
    // These routes have permissive CORS (allow any origin) since they're public API
    let https_ip_rate_limiter = Arc::new(middleware::ip_rate_limit::IpRateLimiter::new(100));
    let cors_permissive = CorsLayer::new()
        .allow_methods([Method::GET, Method::POST, Method::OPTIONS])
        .allow_headers([
            axum::http::header::CONTENT_TYPE,
            axum::http::HeaderName::from_static("x-payment-key"),
            axum::http::HeaderName::from_static("x-compute-limit"),
            axum::http::HeaderName::from_static("x-attached-deposit"),
        ])
        .allow_origin(tower_http::cors::Any);
    let https_api = Router::new()
        // HTTPS API call endpoint: POST /call/{project_owner}/{project_name}
        .route("/call/:project_owner/:project_name", post(handlers::call::https_call))
        // Poll for async call result: GET /calls/{call_id}
        .route("/calls/:call_id", get(handlers::call::get_call_result))
        // Payment key balance (authenticated via X-Payment-Key header)
        .route("/payment-keys/balance", get(handlers::call::get_payment_key_balance_auth))
        .layer(axum::middleware::from_fn_with_state(
            https_ip_rate_limiter.clone(),
            middleware::ip_rate_limit::ip_rate_limit_middleware,
        ))
        .layer(cors_permissive)
        .layer(DefaultBodyLimit::max(10 * 1024 * 1024)) // 10 MB for large attachments
        .with_state(state.clone());
    info!("HTTPS API routes initialized (100 req/min IP rate limit, permissive CORS, 10MB body limit)");

    // Configure CORS with allowed origins from config (for dashboard/internal routes)
    let cors_restricted = CorsLayer::new()
        .allow_methods([Method::GET, Method::POST, Method::DELETE, Method::OPTIONS])
        .allow_headers([
            axum::http::header::CONTENT_TYPE,
            axum::http::header::AUTHORIZATION,
            axum::http::HeaderName::from_static("x-payment-key"),
            axum::http::HeaderName::from_static("x-compute-limit"),
            axum::http::HeaderName::from_static("x-attached-deposit"),
        ])
        .allow_origin(
            config.cors_allowed_origins
                .iter()
                .filter_map(|origin| origin.parse::<HeaderValue>().ok())
                .collect::<Vec<_>>()
        );

    // Combine routers
    // Note: https_api and public_storage have their own permissive CORS layers
    // They must be merged AFTER cors_restricted to avoid being overridden
    let app = Router::new()
        .merge(protected)
        .merge(public)
        .merge(secrets_routes)
        .merge(internal)
        .merge(admin)
        .layer(cors_restricted)
        .merge(https_api)
        .merge(public_storage)
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    // Start HTTP server
    let addr = format!("{}:{}", config.host, config.port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;

    info!("Coordinator API server listening on {}", addr);
    axum::serve(listener, app).await?;

    Ok(())
}
