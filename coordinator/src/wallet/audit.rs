//! Audit log recording for wallet operations
//!
//! Records all wallet actions: withdraw, deposit, policy_change,
//! approval, freeze, unfreeze

use sqlx::PgPool;
use tracing::{debug, warn};

/// Record an audit event
pub async fn record_audit_event(
    db: &PgPool,
    wallet_id: &str,
    event_type: &str,
    actor: &str,
    details: serde_json::Value,
    request_id: Option<&str>,
) {
    let result = sqlx::query(
        r#"
        INSERT INTO wallet_audit_log (wallet_id, event_type, actor, details, request_id)
        VALUES ($1, $2, $3, $4, $5)
        "#,
    )
    .bind(wallet_id)
    .bind(event_type)
    .bind(actor)
    .bind(&details)
    .bind(request_id)
    .execute(db)
    .await;

    match result {
        Ok(_) => debug!(
            "Audit event recorded: wallet={}, type={}, actor={}",
            wallet_id, event_type, actor
        ),
        Err(e) => warn!(
            "Failed to record audit event: wallet={}, type={}, error={}",
            wallet_id, event_type, e
        ),
    }
}

/// Get audit events for a wallet
pub async fn get_audit_events(
    db: &PgPool,
    wallet_id: &str,
    limit: i64,
    offset: i64,
) -> Result<Vec<AuditRow>, sqlx::Error> {
    let rows = sqlx::query_as::<_, AuditRow>(
        r#"
        SELECT event_type, actor, details, request_id, created_at
        FROM wallet_audit_log
        WHERE wallet_id = $1
        ORDER BY created_at DESC
        LIMIT $2 OFFSET $3
        "#,
    )
    .bind(wallet_id)
    .bind(limit)
    .bind(offset)
    .fetch_all(db)
    .await?;

    Ok(rows)
}

#[derive(Debug, sqlx::FromRow)]
pub struct AuditRow {
    pub event_type: String,
    pub actor: String,
    pub details: serde_json::Value,
    pub request_id: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}
