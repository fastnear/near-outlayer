//! Idempotency-Key Middleware
//!
//! Phase 1 Hardening: Prevent duplicate request processing.
//!
//! ## Protocol
//!
//! Client sends: `Idempotency-Key: uuid-v4` (or any unique string)
//! Server stores: (key, method, path, response_status, response_body)
//! TTL: 10 minutes (configurable)
//!
//! ## Behavior
//! - First request with key: Process normally, cache response
//! - Duplicate request with key (within TTL): Return cached response (409 CONFLICT or original)
//! - After TTL: Key expires, next request is treated as new
//!
//! ## Use Cases
//! - Network retries (client resends same request)
//! - Job claim atomicity (multiple workers trying to claim same job)
//! - Payment deduplication (user clicks "submit" multiple times)
//!
//! ## Storage
//! - Redis with TTL for production
//! - In-memory HashMap for development

use axum::{
    body::Body,
    extract::{Request, State},
    http::{Response, StatusCode},
    middleware::Next,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;
use tracing::{debug, warn};

/// Cached response for idempotency
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CachedResponse {
    pub status: u16,
    pub body: Vec<u8>,
    pub timestamp: u64,
}

/// Idempotency store (in-memory for Phase 1, Redis for Phase 2)
pub struct IdempotencyStore {
    /// In-memory cache: idempotency_key → cached response
    cache: Arc<RwLock<std::collections::HashMap<String, CachedResponse>>>,
    /// TTL for cached responses (seconds)
    ttl_seconds: u64,
}

impl IdempotencyStore {
    /// Create new idempotency store with TTL
    pub fn new(ttl_seconds: u64) -> Self {
        Self {
            cache: Arc::new(RwLock::new(std::collections::HashMap::new())),
            ttl_seconds,
        }
    }

    /// Get cached response for idempotency key (if exists and not expired)
    pub async fn get(&self, key: &str) -> Option<CachedResponse> {
        let cache = self.cache.read().await;
        let cached = cache.get(key)?;

        // Check if expired
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        if now - cached.timestamp > self.ttl_seconds {
            debug!("Idempotency key '{}' expired (age: {}s)", key, now - cached.timestamp);
            drop(cache);
            // Remove expired entry
            self.cache.write().await.remove(key);
            return None;
        }

        debug!("Idempotency key '{}' found (age: {}s)", key, now - cached.timestamp);
        Some(cached.clone())
    }

    /// Store response for idempotency key
    pub async fn set(&self, key: String, response: CachedResponse) {
        debug!("Storing idempotency key '{}' (status: {})", key, response.status);
        self.cache.write().await.insert(key, response);
    }

    /// Clean up expired entries (run periodically)
    pub async fn cleanup_expired(&self) {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let mut cache = self.cache.write().await;
        let before_count = cache.len();

        cache.retain(|_, response| {
            now - response.timestamp <= self.ttl_seconds
        });

        let removed = before_count - cache.len();
        if removed > 0 {
            debug!("Cleaned up {} expired idempotency keys", removed);
        }
    }
}

/// Axum middleware for idempotency
pub async fn idempotency_middleware(
    State(store): State<Arc<IdempotencyStore>>,
    req: Request,
    next: Next,
) -> Result<Response<Body>, StatusCode> {
    // Extract Idempotency-Key header (optional)
    let idempotency_key = req
        .headers()
        .get("Idempotency-Key")
        .and_then(|v| v.to_str().ok());

    // If no idempotency key, process normally
    let Some(key) = idempotency_key else {
        return Ok(next.run(req).await);
    };

    // Check if we've seen this key before
    if let Some(cached) = store.get(key).await {
        debug!("Returning cached response for idempotency key: {}", key);

        // Return cached response
        let mut response = Response::new(Body::from(cached.body));
        *response.status_mut() = StatusCode::from_u16(cached.status).unwrap_or(StatusCode::OK);

        // Add header to indicate this was a cached response
        response.headers_mut().insert(
            "X-Idempotency-Replay",
            "true".parse().unwrap(),
        );

        return Ok(response);
    }

    // Process request normally
    let response = next.run(req).await;

    // Cache the response
    let (parts, body) = response.into_parts();
    let body_bytes = axum::body::to_bytes(body, usize::MAX)
        .await
        .map_err(|e| {
            warn!("Failed to read response body for idempotency caching: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    let cached = CachedResponse {
        status: parts.status.as_u16(),
        body: body_bytes.to_vec(),
        timestamp: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs(),
    };

    store.set(key.to_string(), cached).await;

    // Reconstruct response
    let response = Response::from_parts(parts, Body::from(body_bytes));
    Ok(response)
}

/// Spawn background task to clean up expired idempotency keys
pub fn spawn_cleanup_task(store: Arc<IdempotencyStore>, interval: Duration) {
    tokio::spawn(async move {
        let mut interval_timer = tokio::time::interval(interval);
        loop {
            interval_timer.tick().await;
            store.cleanup_expired().await;
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_idempotency_store() {
        let store = IdempotencyStore::new(600); // 10 minute TTL

        // No cached response initially
        assert!(store.get("test-key").await.is_none());

        // Store response
        let cached = CachedResponse {
            status: 200,
            body: b"test response".to_vec(),
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        };

        store.set("test-key".to_string(), cached.clone()).await;

        // Retrieve cached response
        let retrieved = store.get("test-key").await.unwrap();
        assert_eq!(retrieved.status, 200);
        assert_eq!(retrieved.body, b"test response");
    }

    #[tokio::test]
    async fn test_expiration() {
        let store = IdempotencyStore::new(1); // 1 second TTL

        let cached = CachedResponse {
            status: 200,
            body: b"test".to_vec(),
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs()
                - 2, // 2 seconds ago (expired)
        };

        store.set("expired-key".to_string(), cached).await;

        // Should return None (expired)
        assert!(store.get("expired-key").await.is_none());
    }

    #[tokio::test]
    async fn test_cleanup() {
        let store = IdempotencyStore::new(1); // 1 second TTL

        // Add expired entry
        let expired = CachedResponse {
            status: 200,
            body: b"expired".to_vec(),
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs()
                - 10,
        };

        store.set("old".to_string(), expired).await;

        // Add valid entry
        let valid = CachedResponse {
            status: 200,
            body: b"valid".to_vec(),
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        };

        store.set("new".to_string(), valid).await;

        // Cleanup should remove expired, keep valid
        store.cleanup_expired().await;

        assert!(store.get("old").await.is_none());
        assert!(store.get("new").await.is_some());
    }
}
