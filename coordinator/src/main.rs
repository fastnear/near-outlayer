mod auth;
mod config;
mod handlers;
mod models;
mod storage;

use axum::{
    routing::{delete, get, post},
    Router,
};
use std::sync::Arc;
use std::time::Duration;
use tower_http::trace::TraceLayer;
use tracing::info;

use config::Config;
use storage::lru_eviction::LruEviction;

#[derive(Clone)]
pub struct AppState {
    pub db: sqlx::PgPool,
    pub redis: redis::Client,
    pub config: Arc<Config>,
    pub lru_eviction: Arc<LruEviction>,
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

    // Build application state
    let state = AppState {
        db,
        redis,
        config: config.clone(),
        lru_eviction,
    };

    // Build API router
    let app = Router::new()
        // Task endpoints
        .route("/tasks/poll", get(handlers::tasks::poll_task))
        .route("/tasks/complete", post(handlers::tasks::complete_task))
        .route("/tasks/fail", post(handlers::tasks::fail_task))
        .route("/tasks/create", post(handlers::tasks::create_task))
        // WASM cache endpoints
        .route("/wasm/:checksum", get(handlers::wasm::get_wasm))
        .route("/wasm/upload", post(handlers::wasm::upload_wasm))
        .route("/wasm/exists/:checksum", get(handlers::wasm::wasm_exists))
        // Lock endpoints
        .route("/locks/acquire", post(handlers::locks::acquire_lock))
        .route(
            "/locks/release/:lock_key",
            delete(handlers::locks::release_lock),
        )
        // Health check
        .route("/health", get(|| async { "OK" }))
        // Add middleware
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            auth::auth_middleware,
        ))
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    // Start HTTP server
    let addr = format!("{}:{}", config.host, config.port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;

    info!("Coordinator API server listening on {}", addr);
    axum::serve(listener, app).await?;

    Ok(())
}
