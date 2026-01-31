//! Grant Keys API - Admin endpoints for managing grant payment keys
//!
//! Grant keys are payment keys that:
//! - Cannot use X-Attached-Deposit (no earnings transfer to developers)
//! - Compute usage is charged normally (only for gas/compute)
//! - Cannot be withdrawn
//!
//! SECURITY: Admin cannot create new keys - only grant balance to EXISTING keys.
//! User must first create the key via store_secrets (they control the secret).
//! Admin can only fund keys that have zero balance (not yet topped up by user).

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};
use sqlx::Row;
use tracing::info;

use crate::AppState;

/// Request to grant balance to an existing payment key
#[derive(Debug, Deserialize)]
pub struct GrantPaymentKeyRequest {
    /// Owner account ID (NEAR account that owns the key)
    pub owner: String,
    /// Payment key nonce
    pub nonce: u32,
    /// Amount to grant in minimal token units (e.g., 1000000 = $1.00 for 6 decimals)
    pub amount: String,
    /// Optional note for admin reference
    #[serde(default)]
    pub note: Option<String>,
}

/// Response after granting to a payment key
#[derive(Debug, Serialize)]
pub struct GrantPaymentKeyResponse {
    pub owner: String,
    pub nonce: i32,
    pub initial_balance: String,
    pub is_grant: bool,
}

/// Grant key info for listing
#[derive(Debug, Serialize)]
pub struct GrantKeyInfo {
    pub owner: String,
    pub nonce: i32,
    pub initial_balance: String,
    /// Amount spent from this key
    pub spent: String,
    /// Amount currently reserved for in-flight calls
    pub reserved: String,
    /// Available balance (initial - spent - reserved)
    pub available: String,
    pub project_ids: Vec<String>,
    pub max_per_call: Option<String>,
    pub created_at: String,
}

/// POST /admin/grant-payment-key
///
/// Grant balance to an EXISTING payment key with ZERO balance.
/// Sets is_grant=true - key cannot withdraw or use X-Attached-Deposit.
///
/// SECURITY: Admin cannot create keys - only fund existing ones.
/// User controls the secret (created via store_secrets), admin only adds balance.
///
/// is_grant=true means:
/// - Balance cannot be withdrawn
/// - X-Attached-Deposit is forbidden (cannot pay developers)
/// - Can only be used for compute (gas)
pub async fn grant_payment_key(
    State(state): State<AppState>,
    Json(req): Json<GrantPaymentKeyRequest>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    // Validate amount is a valid number
    let amount: u128 = req.amount.parse().map_err(|_| {
        (StatusCode::BAD_REQUEST, "Invalid amount format".to_string())
    })?;

    if amount == 0 {
        return Err((StatusCode::BAD_REQUEST, "Amount must be greater than 0".to_string()));
    }

    // Check that key exists and has zero balance (not yet funded)
    let row = sqlx::query(
        r#"
        SELECT initial_balance, is_grant
        FROM payment_keys
        WHERE owner = $1 AND nonce = $2 AND deleted_at IS NULL
        "#,
    )
    .bind(&req.owner)
    .bind(req.nonce as i32)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| {
        (StatusCode::INTERNAL_SERVER_ERROR, format!("Database error: {}", e))
    })?;

    let row = row.ok_or_else(|| {
        (StatusCode::NOT_FOUND, format!(
            "Payment key not found: owner={} nonce={}. User must create it first via store_secrets.",
            req.owner, req.nonce
        ))
    })?;

    let current_balance: String = row.get("initial_balance");
    let is_already_grant: bool = row.get("is_grant");
    let current_balance_u128: u128 = current_balance.parse().unwrap_or(0);

    // If key has balance but is NOT a grant - it was funded by user, cannot grant
    if current_balance_u128 > 0 && !is_already_grant {
        return Err((StatusCode::CONFLICT, format!(
            "Cannot grant to user-funded key. Current balance: {}. \
             Grants can only be applied to unfunded keys or existing grant keys.",
            current_balance
        )));
    }

    // Calculate new balance (add to existing for grant top-up)
    let new_balance = current_balance_u128 + amount;

    // Update key: set/add balance and mark as grant
    sqlx::query(
        r#"
        UPDATE payment_keys
        SET initial_balance = $3, is_grant = TRUE, updated_at = NOW()
        WHERE owner = $1 AND nonce = $2 AND deleted_at IS NULL
        "#,
    )
    .bind(&req.owner)
    .bind(req.nonce as i32)
    .bind(new_balance.to_string())
    .execute(&state.db)
    .await
    .map_err(|e| {
        (StatusCode::INTERNAL_SERVER_ERROR, format!("Database error: {}", e))
    })?;

    if is_already_grant {
        info!(
            "Grant key topped up: owner={}, nonce={}, added={}, new_balance={}, note={:?}",
            req.owner, req.nonce, req.amount, new_balance, req.note
        );
    } else {
        info!(
            "Granted to payment key: owner={}, nonce={}, amount={}, note={:?}",
            req.owner, req.nonce, req.amount, req.note
        );
    }

    Ok((
        StatusCode::OK,
        Json(GrantPaymentKeyResponse {
            owner: req.owner,
            nonce: req.nonce as i32,
            initial_balance: new_balance.to_string(),
            is_grant: true,
        }),
    ))
}

/// GET /admin/grant-keys
///
/// List all grant keys with their balances
pub async fn list_grant_keys(
    State(state): State<AppState>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let rows = sqlx::query(
        r#"
        SELECT
            pk.owner,
            pk.nonce,
            pk.initial_balance,
            pk.project_ids,
            pk.max_per_call,
            pk.created_at::TEXT as created_at,
            COALESCE(pkb.spent, 0) as spent,
            COALESCE(pkb.reserved, 0) as reserved
        FROM payment_keys pk
        LEFT JOIN payment_key_balances pkb ON pk.owner = pkb.owner AND pk.nonce = pkb.nonce
        WHERE pk.is_grant = TRUE AND pk.deleted_at IS NULL
        ORDER BY pk.created_at DESC
        "#,
    )
    .fetch_all(&state.db)
    .await
    .map_err(|e| {
        (StatusCode::INTERNAL_SERVER_ERROR, format!("Database error: {}", e))
    })?;

    let keys: Vec<GrantKeyInfo> = rows
        .into_iter()
        .map(|row| {
            let initial_balance: String = row.get("initial_balance");
            let spent: sqlx::types::BigDecimal = row.get("spent");
            let reserved: sqlx::types::BigDecimal = row.get("reserved");

            let initial: u128 = initial_balance.parse().unwrap_or(0);
            let spent_u128: u128 = spent.to_string().parse().unwrap_or(0);
            let reserved_u128: u128 = reserved.to_string().parse().unwrap_or(0);
            let available = initial.saturating_sub(spent_u128).saturating_sub(reserved_u128);

            GrantKeyInfo {
                owner: row.get("owner"),
                nonce: row.get("nonce"),
                initial_balance,
                spent: spent_u128.to_string(),
                reserved: reserved_u128.to_string(),
                available: available.to_string(),
                project_ids: row.get("project_ids"),
                max_per_call: row.get("max_per_call"),
                created_at: row.get("created_at"),
            }
        })
        .collect();

    Ok(Json(keys))
}

/// DELETE /admin/grant-keys/{owner}/{nonce}
///
/// Delete a grant key (soft delete). Only works for grant keys (is_grant=true).
/// Use this when a grant is finished and the key is no longer needed.
pub async fn delete_grant_key(
    State(state): State<AppState>,
    Path((owner, nonce)): Path<(String, i32)>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    // Check that key exists and is a grant key
    let result = sqlx::query(
        r#"
        UPDATE payment_keys
        SET deleted_at = NOW()
        WHERE owner = $1 AND nonce = $2 AND is_grant = TRUE AND deleted_at IS NULL
        "#,
    )
    .bind(&owner)
    .bind(nonce)
    .execute(&state.db)
    .await
    .map_err(|e| {
        (StatusCode::INTERNAL_SERVER_ERROR, format!("Database error: {}", e))
    })?;

    if result.rows_affected() == 0 {
        return Err((
            StatusCode::NOT_FOUND,
            "Grant key not found or already deleted".to_string(),
        ));
    }

    info!("Deleted grant key: owner={}, nonce={}", owner, nonce);

    Ok(StatusCode::NO_CONTENT)
}
