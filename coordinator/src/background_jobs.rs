//! Background jobs for coordinator
//!
//! Contains periodic tasks that run in the background:
//! - Cleanup of stuck reserved balances in payment_key_balances table

use sqlx::PgPool;
use std::time::Duration;
use tracing::{info, warn, error};

/// Configuration for the payment key cleanup job
pub struct PaymentKeyCleanupConfig {
    /// How often to run the cleanup job (default: 5 minutes)
    pub check_interval: Duration,
    /// How old reserved balances must be before being cleared (default: 10 minutes)
    pub stale_threshold: Duration,
}

impl Default for PaymentKeyCleanupConfig {
    fn default() -> Self {
        Self {
            check_interval: Duration::from_secs(5 * 60),    // 5 minutes
            stale_threshold: Duration::from_secs(10 * 60), // 10 minutes
        }
    }
}

/// Run the payment key cleanup job periodically
///
/// This job finds payment_key_balances records where:
/// - reserved != 0
/// - last_reserved_at < NOW() - stale_threshold
///
/// And resets their reserved balance to 0 to prevent "stuck" reservations
/// from blocking future calls.
pub async fn run_payment_key_cleanup(db: PgPool, config: PaymentKeyCleanupConfig) {
    info!(
        "🧹 Payment key cleanup job started (interval={}s, stale_threshold={}s)",
        config.check_interval.as_secs(),
        config.stale_threshold.as_secs()
    );

    loop {
        tokio::time::sleep(config.check_interval).await;

        match cleanup_stuck_reservations(&db, config.stale_threshold).await {
            Ok(count) => {
                if count > 0 {
                    info!("🧹 Cleaned up {} stuck reserved balances", count);
                }
            }
            Err(e) => {
                error!("❌ Payment key cleanup job failed: {}", e);
            }
        }
    }
}

/// Clean up stuck reserved balances
///
/// Returns the number of records that were cleaned up.
async fn cleanup_stuck_reservations(
    db: &PgPool,
    stale_threshold: Duration,
) -> Result<u64, sqlx::Error> {
    let stale_seconds = stale_threshold.as_secs() as i64;

    // Find and update stuck reservations
    // Use INTERVAL for PostgreSQL timestamp arithmetic
    let result = sqlx::query(
        r#"
        UPDATE payment_key_balances
        SET reserved = 0,
            last_reserved_at = NULL
        WHERE reserved != '0'
          AND last_reserved_at IS NOT NULL
          AND last_reserved_at < NOW() - INTERVAL '1 second' * $1
        "#,
    )
    .bind(stale_seconds)
    .execute(db)
    .await?;

    let rows_affected = result.rows_affected();

    if rows_affected > 0 {
        // Log details about what was cleaned up
        warn!(
            "⚠️ Reset {} stuck reserved balances (stale_threshold={}s)",
            rows_affected, stale_seconds
        );
    }

    Ok(rows_affected)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = PaymentKeyCleanupConfig::default();
        assert_eq!(config.check_interval, Duration::from_secs(5 * 60));
        assert_eq!(config.stale_threshold, Duration::from_secs(10 * 60));
    }
}
