//! Wallet module — Fireblocks-style custody for AI agents
//!
//! REST API + WASI host functions for wallet operations.
//! Agents authenticate with API keys (register → get key → use Authorization: Bearer header).
//! Policy engine with multisig, limits, freeze, and audit.

pub mod auth;
pub mod audit;
pub mod backend;
pub mod handlers;
pub mod idempotency;
pub mod nonce;
pub mod policy;
pub mod types;
pub mod webhooks;

use axum::routing::{get, post};
use axum::Router;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use auth::ApiKeyCache;
use backend::intents::IntentsBackend;
use backend::WalletBackend;
use nonce::WalletNonceLocks;
use policy::PolicyCache;

use crate::middleware::ip_rate_limit::IpRateLimiter;

/// Shared state for wallet endpoints
#[derive(Clone)]
pub struct WalletState {
    pub db: sqlx::PgPool,
    pub http_client: reqwest::Client,
    pub policy_cache: Arc<PolicyCache>,
    pub nonce_locks: Arc<WalletNonceLocks>,
    pub backend: Arc<dyn WalletBackend>,
    pub keystore_base_url: Option<String>,
    pub keystore_auth_token: Option<String>,
    pub near_rpc_url: String,
    pub contract_id: String,
    pub webhook_secret: String,
    /// SHA256 hashes of allowed worker tokens (for internal WASI wallet auth)
    pub allowed_worker_token_hashes: Vec<String>,
    /// Local policy overrides: wallet_id → encrypted_base64 (for testing without on-chain storage)
    pub policy_overrides: Arc<RwLock<HashMap<String, String>>>,
    /// Stricter rate limiter for POST /register (10 req/min per IP)
    pub register_rate_limiter: Arc<IpRateLimiter>,
    /// In-memory cache for API key → wallet_id (avoids DB query per request)
    pub api_key_cache: ApiKeyCache,
}

/// Build the wallet router (/wallet/v1/*)
pub fn router<S>(state: WalletState) -> Router<S> {
    Router::new()
        // Registration (no auth required)
        .route("/register", post(handlers::register))
        // API key management removed — keys are now controlled by on-chain policy
        // (authorized_key_hashes inside encrypted policy, synced via SystemEvent)
        // Public-facing endpoints (wallet auth via headers)
        .route("/wallet/v1/address", get(handlers::get_address))
        .route("/wallet/v1/tokens", get(handlers::get_tokens))
        .route("/wallet/v1/call", post(handlers::call))
        .route("/wallet/v1/transfer", post(handlers::transfer))
        .route("/wallet/v1/delete", post(handlers::delete))
        .route("/wallet/v1/balance", get(handlers::get_balance))
        .route("/wallet/v1/deposit", post(handlers::deposit))
        .route("/wallet/v1/intents/withdraw", post(handlers::withdraw))
        .route("/wallet/v1/intents/withdraw/dry-run", post(handlers::withdraw_dry_run))
        .route("/wallet/v1/intents/deposit", post(handlers::intents_deposit))
        .route("/wallet/v1/intents/swap", post(handlers::swap))
        .route("/wallet/v1/requests/:request_id", get(handlers::get_request_status))
        .route("/wallet/v1/requests", get(handlers::list_requests))
        .route("/wallet/v1/encrypt-policy", post(handlers::encrypt_policy))
        .route("/wallet/v1/sign-policy", post(handlers::sign_policy))
        .route("/wallet/v1/invalidate-cache", post(handlers::invalidate_cache))
        .route("/wallet/v1/policy", get(handlers::get_policy))
        .route("/wallet/v1/pending_approvals", get(handlers::get_pending_approvals))
        .route("/wallet/v1/approve/:approval_id", post(handlers::approve))
        .route("/wallet/v1/reject/:approval_id", post(handlers::reject))
        .route("/wallet/v1/audit", get(handlers::get_audit))
        // Public read-only endpoints for dashboard (no auth, rate-limited by IP)
        .route("/wallet/v1/stats", get(handlers::wallet_stats))
        .route("/wallet/v1/pending_approvals_by_pubkey", get(handlers::pending_approvals_by_pubkey))
        .route("/wallet/v1/approval/:approval_id", get(handlers::get_approval_detail))
        // Internal endpoints (for worker WASI calls, requires X-Internal-Wallet-Auth)
        .route("/internal/wallet-check", get(handlers::internal_wallet_check))
        .route("/internal/wallet-audit", post(handlers::internal_wallet_audit))
        .route("/internal/activate-policy", post(handlers::internal_activate_policy))
        // Internal endpoints for on-chain policy sync (called by worker on SystemEvent)
        .route("/internal/wallet-policy-sync", post(handlers::internal_wallet_policy_sync))
        .route("/internal/wallet-policy-delete", post(handlers::internal_wallet_policy_delete))
        .route("/internal/wallet-frozen-change", post(handlers::internal_wallet_frozen_change))
        .with_state(state)
}

/// Create WalletState from coordinator's AppState and config
pub fn create_wallet_state(
    db: sqlx::PgPool,
    keystore_base_url: Option<String>,
    keystore_auth_token: Option<String>,
    near_rpc_url: String,
    contract_id: String,
    oneclick_base_url: String,
    oneclick_jwt: Option<String>,
    webhook_secret: String,
    allowed_worker_token_hashes: Vec<String>,
) -> WalletState {
    let backend = IntentsBackend::new(oneclick_base_url, oneclick_jwt);

    WalletState {
        db,
        http_client: reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .expect("Failed to create HTTP client"),
        policy_cache: Arc::new(PolicyCache::new()),
        nonce_locks: Arc::new(WalletNonceLocks::new()),
        backend: Arc::new(backend),
        keystore_base_url,
        keystore_auth_token,
        near_rpc_url,
        contract_id,
        webhook_secret,
        allowed_worker_token_hashes,
        policy_overrides: Arc::new(RwLock::new(HashMap::new())),
        register_rate_limiter: Arc::new(IpRateLimiter::new(10)),
        api_key_cache: ApiKeyCache::new(),
    }
}
