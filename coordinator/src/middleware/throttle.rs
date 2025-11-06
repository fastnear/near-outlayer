use axum::{
    extract::{Request, State},
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Response},
};
use governor::{
    clock::DefaultClock,
    state::{InMemoryState, NotKeyed},
    Quota, RateLimiter,
};
use nonzero_ext::*;
use std::collections::HashMap;
use std::num::NonZeroU32;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, warn};

/// Rate limit profile for a bucket
#[derive(Clone, Debug)]
pub struct RateLimitProfile {
    /// Requests per second
    pub rps: u32,
    /// Burst capacity (max tokens)
    pub burst: u32,
    /// Maximum concurrent in-flight requests
    pub concurrent: u32,
}

impl Default for RateLimitProfile {
    fn default() -> Self {
        Self {
            rps: 5,
            burst: 10,
            concurrent: 4,
        }
    }
}

/// Token bucket rate limiter with concurrency control
pub struct TokenBucket {
    limiter: RateLimiter<NotKeyed, InMemoryState, DefaultClock>,
    profile: RateLimitProfile,
    in_flight: Arc<RwLock<u32>>,
}

impl TokenBucket {
    pub fn new(profile: RateLimitProfile) -> Self {
        let quota = Quota::per_second(nonzero!(profile.rps))
            .allow_burst(NonZeroU32::new(profile.burst).unwrap());

        Self {
            limiter: RateLimiter::direct(quota),
            profile,
            in_flight: Arc::new(RwLock::new(0)),
        }
    }

    /// Check if request can proceed (token + concurrency)
    pub async fn check_rate_limit(&self) -> Result<(), String> {
        // Check concurrency limit first
        let current_in_flight = *self.in_flight.read().await;
        if current_in_flight >= self.profile.concurrent {
            return Err(format!(
                "Concurrency limit reached ({}/{})",
                current_in_flight, self.profile.concurrent
            ));
        }

        // Check token bucket
        match self.limiter.check() {
            Ok(_) => {
                // Increment in-flight counter
                *self.in_flight.write().await += 1;
                Ok(())
            }
            Err(_) => Err(format!(
                "Rate limit exceeded ({}rps)",
                self.profile.rps
            )),
        }
    }

    /// Decrement in-flight counter when request completes
    pub async fn release(&self) {
        let mut in_flight = self.in_flight.write().await;
        if *in_flight > 0 {
            *in_flight -= 1;
        }
    }

    /// Get current state for metrics
    pub async fn get_state(&self) -> (u32, u32, u32) {
        let in_flight = *self.in_flight.read().await;
        (self.profile.rps, self.profile.burst, in_flight)
    }
}

/// Throttle manager - manages multiple buckets per route/auth level
pub struct ThrottleManager {
    /// Anonymous user profile (default)
    pub anon_profile: RateLimitProfile,
    /// API-key authenticated users profile
    pub keyed_profile: RateLimitProfile,
    /// Buckets by route pattern + auth level
    /// Key format: "{route}:{auth_level}"
    buckets: Arc<RwLock<HashMap<String, Arc<TokenBucket>>>>,
}

impl ThrottleManager {
    pub fn new(anon_profile: RateLimitProfile, keyed_profile: RateLimitProfile) -> Self {
        Self {
            anon_profile,
            keyed_profile,
            buckets: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Get or create bucket for route + auth level
    async fn get_bucket(&self, route: &str, has_api_key: bool) -> Arc<TokenBucket> {
        let auth_level = if has_api_key { "keyed" } else { "anon" };
        let bucket_key = format!("{}:{}", route, auth_level);

        // Try to get existing bucket
        {
            let buckets = self.buckets.read().await;
            if let Some(bucket) = buckets.get(&bucket_key) {
                return Arc::clone(bucket);
            }
        }

        // Create new bucket
        let profile = if has_api_key {
            self.keyed_profile.clone()
        } else {
            self.anon_profile.clone()
        };

        let bucket = Arc::new(TokenBucket::new(profile));

        // Store in map
        let mut buckets = self.buckets.write().await;
        buckets.insert(bucket_key, Arc::clone(&bucket));

        bucket
    }

    /// Check rate limit for a request
    pub async fn check(&self, route: &str, has_api_key: bool) -> Result<Arc<TokenBucket>, String> {
        let bucket = self.get_bucket(route, has_api_key).await;
        bucket.check_rate_limit().await?;
        Ok(bucket)
    }

    /// Get metrics for all buckets
    pub async fn get_metrics(&self) -> HashMap<String, (u32, u32, u32)> {
        let buckets = self.buckets.read().await;
        let mut metrics = HashMap::new();

        for (key, bucket) in buckets.iter() {
            let state = bucket.get_state().await;
            metrics.insert(key.clone(), state);
        }

        metrics
    }
}

/// Axum middleware for throttling
pub async fn throttle_middleware(
    State(manager): State<Arc<ThrottleManager>>,
    request: Request,
    next: Next,
) -> Response {
    // Extract route path
    let route = request.uri().path().to_string();

    // Detect API key from Authorization header or query params
    let has_api_key = request
        .headers()
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .map(|v| v.starts_with("Bearer "))
        .unwrap_or(false)
        || request
            .uri()
            .query()
            .unwrap_or("")
            .contains("api_key=")
        || request.uri().query().unwrap_or("").contains("apikey=");

    debug!(
        "Throttle check: route={}, has_api_key={}",
        route, has_api_key
    );

    // Check rate limit
    match manager.check(&route, has_api_key).await {
        Ok(bucket) => {
            // Request allowed - execute and then release
            let response = next.run(request).await;

            // Release concurrency slot
            bucket.release().await;

            response
        }
        Err(reason) => {
            warn!("Rate limit exceeded: route={}, reason={}", route, reason);

            // Return 429 with Retry-After header
            (
                StatusCode::TOO_MANY_REQUESTS,
                [
                    ("Retry-After", "5"),
                    (
                        "X-RateLimit-Limit",
                        if has_api_key { "20" } else { "5" },
                    ),
                ],
                format!("Rate limit exceeded: {}", reason),
            )
                .into_response()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_token_bucket_basic() {
        let profile = RateLimitProfile {
            rps: 2,
            burst: 4,
            concurrent: 2,
        };

        let bucket = TokenBucket::new(profile);

        // Should allow first request
        assert!(bucket.check_rate_limit().await.is_ok());

        // Release
        bucket.release().await;
    }

    #[tokio::test]
    async fn test_concurrency_limit() {
        let profile = RateLimitProfile {
            rps: 100, // High RPS so we hit concurrency first
            burst: 100,
            concurrent: 2,
        };

        let bucket = TokenBucket::new(profile);

        // Allow 2 concurrent
        assert!(bucket.check_rate_limit().await.is_ok());
        assert!(bucket.check_rate_limit().await.is_ok());

        // 3rd should fail
        assert!(bucket.check_rate_limit().await.is_err());

        // Release one
        bucket.release().await;

        // Now should succeed
        assert!(bucket.check_rate_limit().await.is_ok());
    }

    #[tokio::test]
    async fn test_throttle_manager() {
        let anon = RateLimitProfile {
            rps: 5,
            burst: 10,
            concurrent: 4,
        };
        let keyed = RateLimitProfile {
            rps: 20,
            burst: 40,
            concurrent: 8,
        };

        let manager = ThrottleManager::new(anon, keyed);

        // Check anonymous
        let bucket_anon = manager.check("/test", false).await.unwrap();
        let (rps, _, _) = bucket_anon.get_state().await;
        assert_eq!(rps, 5);

        bucket_anon.release().await;

        // Check keyed
        let bucket_keyed = manager.check("/test", true).await.unwrap();
        let (rps, _, _) = bucket_keyed.get_state().await;
        assert_eq!(rps, 20);
    }
}
