//! IP-based rate limiting middleware
//!
//! Protects public endpoints from spam/DoS by limiting requests per IP address.
//! Used for /secrets/* endpoints that proxy to keystore (which runs in TEE and may be slow).

use axum::{
    extract::{ConnectInfo, Request, State},
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Response},
};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::Mutex;

/// IP-based rate limiter
#[derive(Clone)]
pub struct IpRateLimiter {
    /// Map of IP -> (request_count, window_start)
    counters: Arc<Mutex<HashMap<String, (u32, Instant)>>>,
    /// Requests allowed per minute
    limit_per_minute: u32,
}

impl IpRateLimiter {
    pub fn new(limit_per_minute: u32) -> Self {
        Self {
            counters: Arc::new(Mutex::new(HashMap::new())),
            limit_per_minute,
        }
    }

    /// Check if request from IP is within rate limit
    pub async fn check(&self, ip: &str) -> Result<(), (StatusCode, String)> {
        let mut counters = self.counters.lock().await;
        let now = Instant::now();

        // Lazy cleanup: if HashMap gets too large, remove old entries
        // This only triggers during attacks, not normal operation
        if counters.len() > 1000 {
            let before = counters.len();
            counters.retain(|_, (_, window_start)| {
                now.duration_since(*window_start).as_secs() < 120
            });
            tracing::info!(
                before = before,
                after = counters.len(),
                "IP rate limiter cleanup triggered"
            );
        }

        let entry = counters.entry(ip.to_string()).or_insert((0, now));

        // Reset counter if more than 1 minute passed
        if now.duration_since(entry.1).as_secs() >= 60 {
            *entry = (0, now);
        }

        // Check limit
        if entry.0 >= self.limit_per_minute {
            tracing::warn!(
                ip = %ip,
                requests = entry.0,
                limit = self.limit_per_minute,
                "IP rate limit exceeded"
            );
            return Err((
                StatusCode::TOO_MANY_REQUESTS,
                format!(
                    "Rate limit exceeded: {} requests/minute allowed. Try again later.",
                    self.limit_per_minute
                ),
            ));
        }

        // Increment counter
        entry.0 += 1;

        tracing::debug!(
            ip = %ip,
            requests = entry.0,
            limit = self.limit_per_minute,
            "IP rate limit check passed"
        );

        Ok(())
    }
}

/// Extract client IP from request
/// Checks X-Forwarded-For header first (for reverse proxy), then falls back to connection IP
fn get_client_ip(request: &Request) -> String {
    // Try X-Forwarded-For header (common with reverse proxies like nginx)
    if let Some(forwarded_for) = request.headers().get("x-forwarded-for") {
        if let Ok(value) = forwarded_for.to_str() {
            // Take first IP in the chain (original client)
            if let Some(ip) = value.split(',').next() {
                return ip.trim().to_string();
            }
        }
    }

    // Try X-Real-IP header (nginx specific)
    if let Some(real_ip) = request.headers().get("x-real-ip") {
        if let Ok(value) = real_ip.to_str() {
            return value.trim().to_string();
        }
    }

    // Fall back to connection info
    if let Some(connect_info) = request.extensions().get::<ConnectInfo<SocketAddr>>() {
        return connect_info.0.ip().to_string();
    }

    // Ultimate fallback
    "unknown".to_string()
}

/// Middleware for IP-based rate limiting
pub async fn ip_rate_limit_middleware(
    State(rate_limiter): State<Arc<IpRateLimiter>>,
    request: Request,
    next: Next,
) -> Result<Response, Response> {
    let ip = get_client_ip(&request);

    rate_limiter
        .check(&ip)
        .await
        .map_err(|(status, msg)| (status, msg).into_response())?;

    Ok(next.run(request).await)
}
