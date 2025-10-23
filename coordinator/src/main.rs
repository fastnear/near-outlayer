mod auth;
mod config;
mod github;
mod handlers;
mod models;
mod near_client;
mod storage;

use axum::{
    routing::{delete, get, post},
    Router,
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

    // Build protected routes (require auth)
    let protected = Router::new()
        // Job endpoints (protected)
        .route("/jobs/claim", post(handlers::jobs::claim_job))
        .route("/jobs/complete", post(handlers::jobs::complete_job))
        // Task endpoints (protected) - for Redis queue management
        .route("/tasks/poll", get(handlers::tasks::poll_task))
        .route("/tasks/create", post(handlers::tasks::create_task))
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
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            auth::auth_middleware,
        ));

    // Build public routes (no auth required)
    let public = Router::new()
        .route("/public/workers", get(handlers::public::list_workers))
        .route("/public/jobs", get(handlers::public::list_jobs))
        .route("/public/stats", get(handlers::public::get_stats))
        .route("/public/repos/popular", get(handlers::public::get_popular_repos))
        .route("/public/wasm/info", get(handlers::public::get_wasm_info))
        .route("/public/pricing", get(handlers::pricing::get_pricing))
        .route("/public/pricing/refresh", post(handlers::pricing::refresh_pricing))
        .route(
            "/public/users/:user_account_id/earnings",
            get(handlers::public::get_user_earnings),
        )
        .route("/github/resolve-branch", get(handlers::github::resolve_branch))
        .route("/secrets/pubkey", get(handlers::github::get_secrets_pubkey))
        .route("/health", get(|| async { "OK" }));

    // Configure CORS with allowed origins from config
    let cors = CorsLayer::new()
        .allow_methods([Method::GET, Method::POST, Method::DELETE, Method::OPTIONS])
        .allow_headers(tower_http::cors::Any)
        .allow_origin(
            config.cors_allowed_origins
                .iter()
                .filter_map(|origin| origin.parse::<HeaderValue>().ok())
                .collect::<Vec<_>>()
        );

    // Combine routers
    let app = Router::new()
        .merge(protected)
        .merge(public)
        .layer(cors)
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    // Start HTTP server
    let addr = format!("{}:{}", config.host, config.port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;

    info!("Coordinator API server listening on {}", addr);
    axum::serve(listener, app).await?;

    Ok(())
}
