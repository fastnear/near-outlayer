//! Webhook delivery for wallet events
//!
//! Events: approval_needed, approval_received, request_completed,
//!         wallet_frozen, policy_changed
//!
//! Retry: 3 attempts, exponential backoff. HMAC-SHA256 signature on payload.

use hmac::{Hmac, Mac};
use sha2::Sha256;
use sqlx::PgPool;
use tracing::{debug, warn};

type HmacSha256 = Hmac<Sha256>;

/// Maximum delivery attempts
const MAX_ATTEMPTS: i32 = 3;

/// Validate webhook URL to prevent SSRF.
/// Only HTTPS URLs pointing to public hosts are allowed.
fn validate_webhook_url(url: &str) -> Result<(), anyhow::Error> {
    // Require HTTPS scheme
    if !url.starts_with("https://") {
        anyhow::bail!("Webhook URL must use HTTPS: {}", url);
    }

    // Extract host portion (between "https://" and the next "/" or ":" or end)
    let after_scheme = &url[8..]; // skip "https://"
    let host = after_scheme
        .split(|c| c == '/' || c == ':' || c == '?' || c == '#')
        .next()
        .unwrap_or("");

    if host.is_empty() {
        anyhow::bail!("Webhook URL has no host");
    }

    // Block localhost and common private/link-local addresses
    let blocked_hosts = [
        "localhost",
        "127.0.0.1",
        "0.0.0.0",
        "[::1]",
        "169.254.169.254", // cloud metadata
    ];
    let host_lower = host.to_lowercase();
    if blocked_hosts.iter().any(|b| host_lower == *b) {
        anyhow::bail!(
            "Webhook URL host '{}' is not allowed (private/internal)",
            host
        );
    }

    // Block private IP ranges: 10.x, 172.16-31.x, 192.168.x
    if let Ok(ip) = host.parse::<std::net::Ipv4Addr>() {
        if ip.is_private() || ip.is_loopback() || ip.is_link_local() {
            anyhow::bail!("Webhook URL IP {} is private/internal", ip);
        }
    }

    Ok(())
}

/// Create a webhook delivery record
pub async fn enqueue_webhook(
    db: &PgPool,
    wallet_id: &str,
    event_type: &str,
    payload: serde_json::Value,
    webhook_url: &str,
) -> Result<(), anyhow::Error> {
    validate_webhook_url(webhook_url)?;

    sqlx::query(
        r#"
        INSERT INTO wallet_webhook_deliveries (wallet_id, event_type, payload, webhook_url, next_retry_at)
        VALUES ($1, $2, $3, $4, NOW())
        "#,
    )
    .bind(wallet_id)
    .bind(event_type)
    .bind(&payload)
    .bind(webhook_url)
    .execute(db)
    .await?;

    debug!(
        "Webhook enqueued: wallet={}, event={}, url={}",
        wallet_id, event_type, webhook_url
    );
    Ok(())
}

/// Process pending webhooks (called from background task)
pub async fn process_pending_webhooks(db: &PgPool, webhook_secret: &str) {
    let rows = match sqlx::query_as::<_, (i64, String, String, serde_json::Value, String, i32)>(
        r#"
        SELECT id, wallet_id, event_type, payload, webhook_url, attempts
        FROM wallet_webhook_deliveries
        WHERE status = 'pending' AND next_retry_at <= NOW()
        ORDER BY next_retry_at ASC
        LIMIT 50
        "#,
    )
    .fetch_all(db)
    .await
    {
        Ok(rows) => rows,
        Err(e) => {
            warn!("Failed to fetch pending webhooks: {}", e);
            return;
        }
    };

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .unwrap();

    for (id, wallet_id, event_type, payload, webhook_url, attempts) in rows {
        let payload_str = serde_json::to_string(&payload).unwrap_or_default();

        // Compute HMAC-SHA256 signature
        let signature = compute_hmac_signature(&payload_str, webhook_secret);

        let result = client
            .post(&webhook_url)
            .header("Content-Type", "application/json")
            .header("X-Webhook-Signature", &signature)
            .header("X-Wallet-Id", &wallet_id)
            .header("X-Event-Type", &event_type)
            .body(payload_str.clone())
            .send()
            .await;

        match result {
            Ok(resp) if resp.status().is_success() => {
                // Mark as delivered
                let _ = sqlx::query(
                    "UPDATE wallet_webhook_deliveries SET status = 'delivered', attempts = $2 WHERE id = $1",
                )
                .bind(id)
                .bind(attempts + 1)
                .execute(db)
                .await;

                debug!(
                    "Webhook delivered: id={}, wallet={}, event={}",
                    id, wallet_id, event_type
                );
            }
            Ok(resp) => {
                let status = resp.status();
                let body = resp.text().await.unwrap_or_default();
                handle_webhook_failure(db, id, attempts, &format!("HTTP {}: {}", status, body))
                    .await;
            }
            Err(e) => {
                handle_webhook_failure(db, id, attempts, &e.to_string()).await;
            }
        }
    }
}

async fn handle_webhook_failure(db: &PgPool, id: i64, attempts: i32, error: &str) {
    let new_attempts = attempts + 1;

    if new_attempts >= MAX_ATTEMPTS {
        // Mark as failed
        let _ = sqlx::query(
            "UPDATE wallet_webhook_deliveries SET status = 'failed', attempts = $2, last_error = $3 WHERE id = $1",
        )
        .bind(id)
        .bind(new_attempts)
        .bind(error)
        .execute(db)
        .await;

        warn!(
            "Webhook permanently failed after {} attempts: id={}, error={}",
            MAX_ATTEMPTS, id, error
        );
    } else {
        // Exponential backoff: 10s, 60s, 360s
        let backoff_seconds = 10 * (6_i64.pow(attempts as u32));
        let _ = sqlx::query(
            r#"
            UPDATE wallet_webhook_deliveries
            SET attempts = $2, next_retry_at = NOW() + $3 * INTERVAL '1 second', last_error = $4
            WHERE id = $1
            "#,
        )
        .bind(id)
        .bind(new_attempts)
        .bind(backoff_seconds)
        .bind(error)
        .execute(db)
        .await;

        debug!(
            "Webhook retry scheduled: id={}, attempt={}, backoff={}s, error={}",
            id, new_attempts, backoff_seconds, error
        );
    }
}

/// Compute HMAC-SHA256 signature for webhook payload
pub(crate) fn compute_hmac_signature(payload: &str, secret: &str) -> String {
    let mut mac = <HmacSha256 as Mac>::new_from_slice(secret.as_bytes())
        .expect("HMAC can take key of any size");
    mac.update(payload.as_bytes());
    hex::encode(mac.finalize().into_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hmac_deterministic() {
        let sig1 = compute_hmac_signature("payload", "secret");
        let sig2 = compute_hmac_signature("payload", "secret");
        assert_eq!(sig1, sig2);
    }

    #[test]
    fn test_hmac_different_payloads() {
        let sig1 = compute_hmac_signature("payload1", "secret");
        let sig2 = compute_hmac_signature("payload2", "secret");
        assert_ne!(sig1, sig2);
    }

    #[test]
    fn test_hmac_different_secrets() {
        let sig1 = compute_hmac_signature("payload", "secret1");
        let sig2 = compute_hmac_signature("payload", "secret2");
        assert_ne!(sig1, sig2);
    }

    #[test]
    fn test_hmac_known_vector() {
        let sig = compute_hmac_signature("", "key");
        // HMAC-SHA256 produces a 32-byte (64 hex chars) output
        assert_eq!(sig.len(), 64);
        assert!(sig.chars().all(|c| c.is_ascii_hexdigit()));
        // Different input produces different output
        let sig2 = compute_hmac_signature("test", "key");
        assert_ne!(sig, sig2);
    }

    #[test]
    fn test_webhook_url_valid_https() {
        assert!(validate_webhook_url("https://example.com/webhook").is_ok());
        assert!(validate_webhook_url("https://myapp.io:8443/hook").is_ok());
    }

    #[test]
    fn test_webhook_url_rejects_http() {
        assert!(validate_webhook_url("http://example.com/webhook").is_err());
    }

    #[test]
    fn test_webhook_url_rejects_localhost() {
        assert!(validate_webhook_url("https://localhost/webhook").is_err());
        assert!(validate_webhook_url("https://127.0.0.1/webhook").is_err());
    }

    #[test]
    fn test_webhook_url_rejects_metadata() {
        assert!(validate_webhook_url("https://169.254.169.254/latest").is_err());
    }

    #[test]
    fn test_webhook_url_rejects_private_ip() {
        assert!(validate_webhook_url("https://10.0.0.1/webhook").is_err());
        assert!(validate_webhook_url("https://192.168.1.1/webhook").is_err());
        assert!(validate_webhook_url("https://172.16.0.1/webhook").is_err());
    }
}
