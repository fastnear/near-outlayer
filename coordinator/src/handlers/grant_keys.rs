//! Grant Keys API - Admin endpoints for managing non-withdrawable grant keys
//!
//! Grant keys are payment keys that:
//! - Cannot use X-Attached-Deposit (no earnings transfer to developers)
//! - Compute usage is charged normally
//! - Created by admin only, not synced from contract
//! - Cannot be withdrawn (future feature)

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use rand::Rng;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::Row;
use tracing::info;

use crate::AppState;

/// Request to create a new grant key
#[derive(Debug, Deserialize)]
pub struct CreateGrantKeyRequest {
    /// Owner account ID (NEAR account that will use this key)
    pub owner: String,
    /// Initial balance in minimal token units (e.g., 1000000 = $1.00 for 6 decimals)
    pub initial_balance: String,
    /// Allowed project IDs (empty = all projects allowed)
    #[serde(default)]
    pub project_ids: Vec<String>,
    /// Max amount per API call in minimal token units (None = no limit)
    #[serde(default)]
    pub max_per_call: Option<String>,
    /// Optional note for admin reference
    #[serde(default)]
    pub note: Option<String>,
}

/// Response after creating a grant key
#[derive(Debug, Serialize)]
pub struct CreateGrantKeyResponse {
    pub owner: String,
    pub nonce: i32,
    /// The generated key (hex, 64 chars) - only returned once!
    pub key: String,
    pub initial_balance: String,
    pub project_ids: Vec<String>,
    pub max_per_call: Option<String>,
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

/// POST /admin/grant-keys
///
/// Create a new grant key for the specified owner.
/// Returns the generated key - this is the only time the key is visible!
pub async fn create_grant_key(
    State(state): State<AppState>,
    Json(req): Json<CreateGrantKeyRequest>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    // Validate initial_balance is a valid number
    let _balance: u128 = req.initial_balance.parse().map_err(|_| {
        (StatusCode::BAD_REQUEST, "Invalid initial_balance format".to_string())
    })?;

    // Find next available nonce for this owner (across ALL keys, not just grants)
    // Nonce must be unique per owner regardless of key type
    let next_nonce: i32 = sqlx::query_scalar(
        r#"
        SELECT COALESCE(MAX(nonce) + 1, 0)
        FROM payment_keys
        WHERE owner = $1
        "#,
    )
    .bind(&req.owner)
    .fetch_one(&state.db)
    .await
    .map_err(|e| {
        (StatusCode::INTERNAL_SERVER_ERROR, format!("Database error: {}", e))
    })?;

    // Generate random 32-byte key and encode as hex (64 chars)
    let key_bytes: [u8; 32] = rand::thread_rng().gen();
    let key_hex = hex::encode(key_bytes);

    // Calculate SHA256 hash of the key for storage
    let key_hash = hex::encode(Sha256::digest(key_hex.as_bytes()));

    // Insert grant key into database
    sqlx::query(
        r#"
        INSERT INTO payment_keys (owner, nonce, key_hash, initial_balance, project_ids, max_per_call, is_grant)
        VALUES ($1, $2, $3, $4, $5, $6, TRUE)
        "#,
    )
    .bind(&req.owner)
    .bind(next_nonce)
    .bind(&key_hash)
    .bind(&req.initial_balance)
    .bind(&req.project_ids)
    .bind(&req.max_per_call)
    .execute(&state.db)
    .await
    .map_err(|e| {
        (StatusCode::INTERNAL_SERVER_ERROR, format!("Database error: {}", e))
    })?;

    info!(
        "Created grant key: owner={}, nonce={}, balance={}, note={:?}",
        req.owner, next_nonce, req.initial_balance, req.note
    );

    Ok((
        StatusCode::CREATED,
        Json(CreateGrantKeyResponse {
            owner: req.owner,
            nonce: next_nonce,
            key: key_hex,
            initial_balance: req.initial_balance,
            project_ids: req.project_ids,
            max_per_call: req.max_per_call,
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

// =============================================================================
// Future: Payment Key Withdrawal
// =============================================================================

/// Withdraw remaining balance from a payment key (FUTURE - not implemented yet)
///
/// This function is a placeholder for future withdrawal functionality.
/// When implemented, it MUST:
/// 1. Exclude grant keys (is_grant = TRUE) - they cannot be withdrawn
/// 2. Only allow withdrawal of regular payment keys synced from contract
/// 3. Verify ownership via NEAR signature or contract call
/// 4. Transfer stablecoins back to the owner
///
/// Grant keys are excluded because:
/// - They don't have real tokens deposited on the contract
/// - They are virtual balances created by admin for testing/grants
/// - Allowing withdrawal would create tokens out of thin air
#[allow(dead_code)]
pub async fn withdraw_payment_key_balance(
    _state: &AppState,
    _owner: &str,
    _nonce: i32,
) -> Result<(), String> {
    // TODO: Implement when contract has withdrawal event support
    //
    // Pseudocode:
    // 1. Check key exists: SELECT * FROM payment_keys WHERE owner = $1 AND nonce = $2
    // 2. CRITICAL: Verify is_grant = FALSE, otherwise reject:
    //    if key.is_grant {
    //        return Err("Grant keys cannot be withdrawn".to_string());
    //    }
    // 3. Calculate available balance: initial_balance - spent - reserved
    // 4. Verify no in-flight calls (reserved = 0)
    // 5. Call contract method to initiate withdrawal
    // 6. Wait for contract event confirming withdrawal
    // 7. Mark key as deleted or update balance

    Err("Withdrawal not implemented yet - waiting for contract event support".to_string())
}
