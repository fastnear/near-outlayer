//! Idempotency key handling for wallet operations
//!
//! POST requests use X-Idempotency-Key header to prevent duplicate operations.
//! Stored in DB (wallet_requests table) with unique index on (wallet_id, idempotency_key).

use super::types::WalletError;
use sqlx::PgPool;
use tracing::debug;

/// Check if an idempotency key has already been used for this wallet.
/// Returns Some(request_id) if duplicate, None if new.
pub async fn check_idempotency(
    db: &PgPool,
    wallet_id: &str,
    idempotency_key: &str,
) -> Result<Option<String>, WalletError> {
    let row = sqlx::query_scalar::<_, uuid::Uuid>(
        r#"
        SELECT request_id FROM wallet_requests
        WHERE wallet_id = $1 AND idempotency_key = $2
        "#,
    )
    .bind(wallet_id)
    .bind(idempotency_key)
    .fetch_optional(db)
    .await
    .map_err(|e| WalletError::InternalError(format!("Database error: {}", e)))?;

    if let Some(request_id) = row {
        debug!(
            "Idempotency key '{}' already used for wallet '{}', request_id={}",
            idempotency_key, wallet_id, request_id
        );
        return Ok(Some(request_id.to_string()));
    }

    Ok(None)
}
