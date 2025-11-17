use axum::{
    extract::{Request, State},
    http::StatusCode,
    middleware::Next,
    response::Response,
};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use crate::config::Config;

/// Simple in-memory rate limiter (per API key)
#[derive(Clone)]
pub struct RateLimiter {
    counters: Arc<Mutex<HashMap<i64, (u32, std::time::Instant)>>>,
}

impl RateLimiter {
    pub fn new() -> Self {
        Self {
            counters: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Check if request is within rate limit
    pub async fn check_rate_limit(
        &self,
        api_key_id: i64,
        limit_per_minute: u32,
    ) -> Result<(), StatusCode> {
        let mut counters = self.counters.lock().await;
        let now = std::time::Instant::now();

        let entry = counters.entry(api_key_id).or_insert((0, now));

        // Reset counter if more than 1 minute passed
        if now.duration_since(entry.1).as_secs() >= 60 {
            *entry = (0, now);
        }

        // Check limit
        if entry.0 >= limit_per_minute {
            tracing::warn!(
                "Rate limit exceeded for API key ID {}: {} requests in last minute",
                api_key_id,
                entry.0
            );
            return Err(StatusCode::TOO_MANY_REQUESTS);
        }

        // Increment counter
        entry.0 += 1;

        Ok(())
    }
}

/// Middleware for rate limiting based on API key
pub async fn rate_limit_middleware(
    State((rate_limiter, config)): State<(Arc<RateLimiter>, Arc<Config>)>,
    request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    // Skip rate limiting if API key check is disabled
    if !config.require_attestation_api_key {
        tracing::debug!("Rate limiting skipped (REQUIRE_ATTESTATION_API_KEY=false)");
        return Ok(next.run(request).await);
    }

    // Get API key ID from extensions (set by api_key_auth middleware)
    let api_key_id = request
        .extensions()
        .get::<i64>()
        .copied()
        .ok_or(StatusCode::UNAUTHORIZED)?;

    // Get rate limit from extensions
    let rate_limit = request
        .extensions()
        .get::<i32>()
        .copied()
        .ok_or(StatusCode::INTERNAL_SERVER_ERROR)?;

    // Check rate limit
    rate_limiter
        .check_rate_limit(api_key_id, rate_limit as u32)
        .await?;

    Ok(next.run(request).await)
}
