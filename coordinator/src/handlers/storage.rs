use axum::{
    extract::{Query, State},
    http::StatusCode,
    Json,
};
use serde::{Deserialize, Serialize};
use tracing::{debug, error, info};

use crate::AppState;

/// Request to set a storage key
#[derive(Debug, Deserialize)]
pub struct StorageSetRequest {
    pub project_uuid: String,          // Required - storage only allowed for projects
    pub wasm_hash: String,             // Which version is writing
    pub account_id: String,            // NEAR account or "@worker"
    pub key_hash: String,              // SHA256 of plaintext key
    pub encrypted_key: Vec<u8>,        // Encrypted key (or plaintext if is_encrypted=false)
    pub encrypted_value: Vec<u8>,      // Encrypted value (or plaintext if is_encrypted=false)
    #[serde(default = "default_true")]
    pub is_encrypted: bool,            // false = public storage, readable by other projects
}

fn default_true() -> bool { true }

/// Request to get a storage key
#[derive(Debug, Deserialize)]
pub struct StorageGetRequest {
    pub project_uuid: String,          // Required - storage only allowed for projects
    pub account_id: String,
    pub key_hash: String,
}

/// Request to get storage by version (for migration support)
#[derive(Debug, Deserialize)]
pub struct StorageGetByVersionRequest {
    pub wasm_hash: String,
    pub account_id: String,
    pub key_hash: String,
}

/// Request to delete a storage key
#[derive(Debug, Deserialize)]
pub struct StorageDeleteRequest {
    pub project_uuid: String,  // Required - storage only allowed for projects
    pub account_id: String,
    pub key_hash: String,
}

/// Request to list storage keys
#[derive(Debug, Deserialize)]
pub struct StorageListQuery {
    pub project_uuid: String,  // Required - storage only allowed for projects
    pub account_id: String,
    #[allow(dead_code)]
    pub prefix_hash: Option<String>,  // Optional prefix filter (SHA256 of prefix)
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

/// Request to clear all storage for a project/account
#[derive(Debug, Deserialize)]
pub struct StorageClearRequest {
    pub project_uuid: String,
    pub account_id: String,
}

/// Request to clear storage for a specific version
#[derive(Debug, Deserialize)]
pub struct StorageClearVersionRequest {
    pub wasm_hash: String,
    pub account_id: String,
}

/// Request to clear ALL storage for a project (all accounts)
#[derive(Debug, Deserialize)]
pub struct StorageClearProjectRequest {
    pub project_uuid: String,
}

/// Request to get public storage from another project
#[derive(Debug, Deserialize)]
pub struct StorageGetPublicRequest {
    pub owner: String,              // Project owner (NEAR account)
    pub project_id: String,         // Project ID
    pub key_hash: String,           // SHA256 of plaintext key
}

/// Response for public storage get
#[derive(Debug, Serialize)]
pub struct StorageGetPublicResponse {
    pub exists: bool,
    pub value: Option<Vec<u8>>,     // Plaintext value (only for is_encrypted=false)
}

/// Response for storage get
#[derive(Debug, Serialize)]
pub struct StorageGetResponse {
    pub exists: bool,
    pub encrypted_key: Option<Vec<u8>>,
    pub encrypted_value: Option<Vec<u8>>,
    pub wasm_hash: Option<String>,  // Which version wrote this
    #[serde(default)]
    pub is_encrypted: bool,         // false = plaintext public storage
}


/// Response for storage list
#[derive(Debug, Serialize)]
pub struct StorageListResponse {
    pub keys: Vec<StorageKeyInfo>,
    pub total: i64,
}

#[derive(Debug, Serialize)]
pub struct StorageKeyInfo {
    pub key_hash: String,
    pub encrypted_key: Vec<u8>,
    pub encrypted_value: Vec<u8>,
    pub wasm_hash: String,
}

/// Response for storage usage
#[derive(Debug, Serialize)]
pub struct StorageUsageResponse {
    pub total_bytes: i64,
    pub key_count: i32,
}

/// Response for has key check
#[derive(Debug, Serialize)]
pub struct StorageHasResponse {
    pub exists: bool,
}

/// Request for set_if_absent (insert only if key doesn't exist)
#[derive(Debug, Deserialize)]
pub struct StorageSetIfAbsentRequest {
    pub project_uuid: String,
    pub wasm_hash: String,
    pub account_id: String,
    pub key_hash: String,
    pub encrypted_key: Vec<u8>,
    pub encrypted_value: Vec<u8>,
}

/// Response for set_if_absent
#[derive(Debug, Serialize)]
pub struct StorageSetIfAbsentResponse {
    /// true if value was inserted, false if key already existed
    pub inserted: bool,
}

/// Request for set_if_equals (update only if current value matches expected)
#[derive(Debug, Deserialize)]
pub struct StorageSetIfEqualsRequest {
    pub project_uuid: String,
    pub wasm_hash: String,
    pub account_id: String,
    pub key_hash: String,
    pub expected_encrypted_value: Vec<u8>,  // Expected current value
    pub new_encrypted_key: Vec<u8>,         // New key (for re-encryption)
    pub new_encrypted_value: Vec<u8>,       // New value
}

/// Response for set_if_equals
#[derive(Debug, Serialize)]
pub struct StorageSetIfEqualsResponse {
    /// true if value was updated, false if current value didn't match expected
    pub updated: bool,
    /// Current encrypted value (for retry on failure)
    pub current_encrypted_value: Option<Vec<u8>>,
    /// Current encrypted key (for retry on failure)
    pub current_encrypted_key: Option<Vec<u8>>,
}

/// Set a storage key
pub async fn storage_set(
    State(state): State<AppState>,
    Json(req): Json<StorageSetRequest>,
) -> Result<StatusCode, StatusCode> {
    debug!(
        "storage_set: project_uuid={:?}, account={}, key_hash={}, is_encrypted={}",
        req.project_uuid, req.account_id, req.key_hash, req.is_encrypted
    );

    let data_size = (req.encrypted_key.len() + req.encrypted_value.len()) as i64;

    // Insert or update storage data
    let result = sqlx::query(
        r#"
        INSERT INTO storage_data (project_uuid, wasm_hash, account_id, key_hash, encrypted_key, encrypted_value, is_encrypted)
        VALUES ($1, $2, $3, $4, $5, $6, $7)
        ON CONFLICT (project_uuid, account_id, key_hash)
        DO UPDATE SET
            encrypted_value = $6,
            wasm_hash = $2,
            is_encrypted = $7,
            updated_at = NOW()
        "#,
    )
    .bind(Some(&req.project_uuid))
    .bind(&req.wasm_hash)
    .bind(&req.account_id)
    .bind(&req.key_hash)
    .bind(&req.encrypted_key)
    .bind(&req.encrypted_value)
    .bind(req.is_encrypted)
    .execute(&state.db)
    .await;

    if let Err(e) = result {
        error!("Failed to set storage: {}", e);
        return Err(StatusCode::INTERNAL_SERVER_ERROR);
    }

    // Update usage tracking (always by project since project_uuid is required)
    update_project_usage(&state.db, &req.project_uuid, &req.account_id).await;

    info!(
        "storage_set success: project_uuid={:?}, account={}, key_hash={}, size={}, is_encrypted={}",
        req.project_uuid, req.account_id, req.key_hash, data_size, req.is_encrypted
    );
    Ok(StatusCode::OK)
}

/// Set a storage key only if it doesn't already exist
pub async fn storage_set_if_absent(
    State(state): State<AppState>,
    Json(req): Json<StorageSetIfAbsentRequest>,
) -> Json<StorageSetIfAbsentResponse> {
    debug!(
        "storage_set_if_absent: project_uuid={}, account={}, key_hash={}",
        req.project_uuid, req.account_id, req.key_hash
    );

    // Try to insert, do nothing on conflict
    let result = sqlx::query(
        r#"
        INSERT INTO storage_data (project_uuid, wasm_hash, account_id, key_hash, encrypted_key, encrypted_value)
        VALUES ($1, $2, $3, $4, $5, $6)
        ON CONFLICT (project_uuid, account_id, key_hash) DO NOTHING
        "#,
    )
    .bind(&req.project_uuid)
    .bind(&req.wasm_hash)
    .bind(&req.account_id)
    .bind(&req.key_hash)
    .bind(&req.encrypted_key)
    .bind(&req.encrypted_value)
    .execute(&state.db)
    .await;

    match result {
        Ok(res) => {
            let inserted = res.rows_affected() > 0;
            if inserted {
                update_project_usage(&state.db, &req.project_uuid, &req.account_id).await;
                info!(
                    "storage_set_if_absent: inserted key_hash={} for project={}",
                    req.key_hash, req.project_uuid
                );
            } else {
                debug!(
                    "storage_set_if_absent: key already exists key_hash={} for project={}",
                    req.key_hash, req.project_uuid
                );
            }
            Json(StorageSetIfAbsentResponse { inserted })
        }
        Err(e) => {
            error!("storage_set_if_absent error: {}", e);
            Json(StorageSetIfAbsentResponse { inserted: false })
        }
    }
}

/// Set a storage key only if current value matches expected value (compare-and-swap)
pub async fn storage_set_if_equals(
    State(state): State<AppState>,
    Json(req): Json<StorageSetIfEqualsRequest>,
) -> Json<StorageSetIfEqualsResponse> {
    debug!(
        "storage_set_if_equals: project_uuid={}, account={}, key_hash={}",
        req.project_uuid, req.account_id, req.key_hash
    );

    // Try to update only if encrypted_value matches expected
    let result = sqlx::query(
        r#"
        UPDATE storage_data
        SET encrypted_key = $5,
            encrypted_value = $6,
            wasm_hash = $7,
            updated_at = NOW()
        WHERE project_uuid = $1
          AND account_id = $2
          AND key_hash = $3
          AND encrypted_value = $4
        "#,
    )
    .bind(&req.project_uuid)
    .bind(&req.account_id)
    .bind(&req.key_hash)
    .bind(&req.expected_encrypted_value)
    .bind(&req.new_encrypted_key)
    .bind(&req.new_encrypted_value)
    .bind(&req.wasm_hash)
    .execute(&state.db)
    .await;

    match result {
        Ok(res) => {
            if res.rows_affected() > 0 {
                // Update succeeded - value matched
                update_project_usage(&state.db, &req.project_uuid, &req.account_id).await;
                info!(
                    "storage_set_if_equals: updated key_hash={} for project={}",
                    req.key_hash, req.project_uuid
                );
                Json(StorageSetIfEqualsResponse {
                    updated: true,
                    current_encrypted_value: None,
                    current_encrypted_key: None,
                })
            } else {
                // Update failed - fetch current value for retry
                debug!(
                    "storage_set_if_equals: value mismatch for key_hash={}, fetching current",
                    req.key_hash
                );
                let current = sqlx::query_as::<_, (Vec<u8>, Vec<u8>)>(
                    r#"
                    SELECT encrypted_key, encrypted_value
                    FROM storage_data
                    WHERE project_uuid = $1 AND account_id = $2 AND key_hash = $3
                    "#,
                )
                .bind(&req.project_uuid)
                .bind(&req.account_id)
                .bind(&req.key_hash)
                .fetch_optional(&state.db)
                .await;

                match current {
                    Ok(Some((enc_key, enc_value))) => Json(StorageSetIfEqualsResponse {
                        updated: false,
                        current_encrypted_value: Some(enc_value),
                        current_encrypted_key: Some(enc_key),
                    }),
                    _ => Json(StorageSetIfEqualsResponse {
                        updated: false,
                        current_encrypted_value: None,
                        current_encrypted_key: None,
                    }),
                }
            }
        }
        Err(e) => {
            error!("storage_set_if_equals error: {}", e);
            Json(StorageSetIfEqualsResponse {
                updated: false,
                current_encrypted_value: None,
                current_encrypted_key: None,
            })
        }
    }
}

/// Get a storage key
pub async fn storage_get(
    State(state): State<AppState>,
    Json(req): Json<StorageGetRequest>,
) -> Json<StorageGetResponse> {
    debug!(
        "storage_get: project_uuid={:?}, account={}, key_hash={}",
        req.project_uuid, req.account_id, req.key_hash
    );

    let result = sqlx::query_as::<_, (Vec<u8>, Vec<u8>, String, bool)>(
        r#"
        SELECT encrypted_key, encrypted_value, wasm_hash, is_encrypted
        FROM storage_data
        WHERE project_uuid = $1
          AND account_id = $2
          AND key_hash = $3
        "#,
    )
    .bind(&req.project_uuid)
    .bind(&req.account_id)
    .bind(&req.key_hash)
    .fetch_optional(&state.db)
    .await;

    match result {
        Ok(Some((encrypted_key, encrypted_value, wasm_hash, is_encrypted))) => {
            debug!("storage_get found: key_hash={}, is_encrypted={}", req.key_hash, is_encrypted);
            Json(StorageGetResponse {
                exists: true,
                encrypted_key: Some(encrypted_key),
                encrypted_value: Some(encrypted_value),
                wasm_hash: Some(wasm_hash),
                is_encrypted,
            })
        }
        Ok(None) => {
            debug!("storage_get not found: key_hash={}", req.key_hash);
            Json(StorageGetResponse {
                exists: false,
                encrypted_key: None,
                encrypted_value: None,
                wasm_hash: None,
                is_encrypted: true,
            })
        }
        Err(e) => {
            error!("storage_get error: {}", e);
            Json(StorageGetResponse {
                exists: false,
                encrypted_key: None,
                encrypted_value: None,
                wasm_hash: None,
                is_encrypted: true,
            })
        }
    }
}

/// Get storage by specific version (for migration between versions)
pub async fn storage_get_by_version(
    State(state): State<AppState>,
    Json(req): Json<StorageGetByVersionRequest>,
) -> Json<StorageGetResponse> {
    debug!(
        "storage_get_by_version: wasm_hash={}, account={}, key_hash={}",
        req.wasm_hash, req.account_id, req.key_hash
    );

    let result = sqlx::query_as::<_, (Vec<u8>, Vec<u8>, String, bool)>(
        r#"
        SELECT encrypted_key, encrypted_value, wasm_hash, is_encrypted
        FROM storage_data
        WHERE wasm_hash = $1
          AND account_id = $2
          AND key_hash = $3
        "#,
    )
    .bind(&req.wasm_hash)
    .bind(&req.account_id)
    .bind(&req.key_hash)
    .fetch_optional(&state.db)
    .await;

    match result {
        Ok(Some((encrypted_key, encrypted_value, wasm_hash, is_encrypted))) => {
            Json(StorageGetResponse {
                exists: true,
                encrypted_key: Some(encrypted_key),
                encrypted_value: Some(encrypted_value),
                wasm_hash: Some(wasm_hash),
                is_encrypted,
            })
        }
        Ok(None) => Json(StorageGetResponse {
            exists: false,
            encrypted_key: None,
            encrypted_value: None,
            wasm_hash: None,
            is_encrypted: true,
        }),
        Err(e) => {
            error!("storage_get_by_version error: {}", e);
            Json(StorageGetResponse {
                exists: false,
                encrypted_key: None,
                encrypted_value: None,
                wasm_hash: None,
                is_encrypted: true,
            })
        }
    }
}

/// Check if a storage key exists
pub async fn storage_has(
    State(state): State<AppState>,
    Json(req): Json<StorageGetRequest>,
) -> Json<StorageHasResponse> {
    let result = sqlx::query_as::<_, (i64,)>(
        r#"
        SELECT COUNT(*) FROM storage_data
        WHERE project_uuid = $1
          AND account_id = $2
          AND key_hash = $3
        "#,
    )
    .bind(&req.project_uuid)
    .bind(&req.account_id)
    .bind(&req.key_hash)
    .fetch_one(&state.db)
    .await;

    let exists = matches!(result, Ok((count,)) if count > 0);
    Json(StorageHasResponse { exists })
}

/// Delete a storage key
pub async fn storage_delete(
    State(state): State<AppState>,
    Json(req): Json<StorageDeleteRequest>,
) -> Result<StatusCode, StatusCode> {
    debug!(
        "storage_delete: project_uuid={:?}, account={}, key_hash={}",
        req.project_uuid, req.account_id, req.key_hash
    );

    let result = sqlx::query(
        r#"
        DELETE FROM storage_data
        WHERE project_uuid = $1
          AND account_id = $2
          AND key_hash = $3
        "#,
    )
    .bind(&req.project_uuid)
    .bind(&req.account_id)
    .bind(&req.key_hash)
    .execute(&state.db)
    .await;

    match result {
        Ok(res) => {
            if res.rows_affected() > 0 {
                // Update usage tracking
                update_project_usage(&state.db, &req.project_uuid, &req.account_id).await;
                info!("storage_delete success: key_hash={}", req.key_hash);
                Ok(StatusCode::OK)
            } else {
                Ok(StatusCode::NOT_FOUND)
            }
        }
        Err(e) => {
            error!("storage_delete error: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// List storage keys for a project/account
pub async fn storage_list(
    State(state): State<AppState>,
    Query(query): Query<StorageListQuery>,
) -> Json<StorageListResponse> {
    let limit = query.limit.unwrap_or(100).min(1000);
    let offset = query.offset.unwrap_or(0);

    debug!(
        "storage_list: project_uuid={:?}, account={}, limit={}, offset={}",
        query.project_uuid, query.account_id, limit, offset
    );

    // Get total count
    let count_result = sqlx::query_as::<_, (i64,)>(
        r#"
        SELECT COUNT(*) FROM storage_data
        WHERE project_uuid = $1
          AND account_id = $2
        "#,
    )
    .bind(&query.project_uuid)
    .bind(&query.account_id)
    .fetch_one(&state.db)
    .await;

    let total = count_result.map(|(c,)| c).unwrap_or(0);

    // Get keys with encrypted values (needed for keystore decryption)
    let keys_result = sqlx::query_as::<_, (String, Vec<u8>, Vec<u8>, String)>(
        r#"
        SELECT key_hash, encrypted_key, encrypted_value, wasm_hash
        FROM storage_data
        WHERE project_uuid = $1
          AND account_id = $2
        ORDER BY created_at
        LIMIT $3 OFFSET $4
        "#,
    )
    .bind(&query.project_uuid)
    .bind(&query.account_id)
    .bind(limit)
    .bind(offset)
    .fetch_all(&state.db)
    .await;

    let keys = keys_result
        .map(|rows| {
            rows.into_iter()
                .map(|(key_hash, encrypted_key, encrypted_value, wasm_hash)| StorageKeyInfo {
                    key_hash,
                    encrypted_key,
                    encrypted_value,
                    wasm_hash,
                })
                .collect()
        })
        .unwrap_or_default();

    Json(StorageListResponse { keys, total })
}

/// Get storage usage for a project
pub async fn storage_usage(
    State(state): State<AppState>,
    Query(query): Query<StorageListQuery>,
) -> Json<StorageUsageResponse> {
    let result = sqlx::query_as::<_, (i64, i32)>(
        r#"
        SELECT COALESCE(total_bytes, 0), COALESCE(key_count, 0)
        FROM storage_usage
        WHERE project_uuid = $1 AND account_id = $2
        "#,
    )
    .bind(&query.project_uuid)
    .bind(&query.account_id)
    .fetch_optional(&state.db)
    .await;

    match result {
        Ok(Some((total_bytes, key_count))) => Json(StorageUsageResponse {
            total_bytes,
            key_count,
        }),
        _ => Json(StorageUsageResponse {
            total_bytes: 0,
            key_count: 0,
        }),
    }
}

/// Clear all storage for a project/account
pub async fn storage_clear_all(
    State(state): State<AppState>,
    Json(req): Json<StorageClearRequest>,
) -> Result<StatusCode, StatusCode> {
    info!(
        "storage_clear_all: project_uuid={}, account={}",
        req.project_uuid, req.account_id
    );

    let result = sqlx::query(
        r#"
        DELETE FROM storage_data
        WHERE project_uuid = $1 AND account_id = $2
        "#,
    )
    .bind(&req.project_uuid)
    .bind(&req.account_id)
    .execute(&state.db)
    .await;

    match result {
        Ok(res) => {
            // Update usage to 0
            let _ = sqlx::query(
                r#"
                UPDATE storage_usage
                SET total_bytes = 0, key_count = 0, updated_at = NOW()
                WHERE project_uuid = $1 AND account_id = $2
                "#,
            )
            .bind(&req.project_uuid)
            .bind(&req.account_id)
            .execute(&state.db)
            .await;

            info!(
                "storage_clear_all: deleted {} rows for project={}",
                res.rows_affected(),
                req.project_uuid
            );
            Ok(StatusCode::OK)
        }
        Err(e) => {
            error!("storage_clear_all error: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// Clear storage for a specific WASM version
pub async fn storage_clear_version(
    State(state): State<AppState>,
    Json(req): Json<StorageClearVersionRequest>,
) -> Result<StatusCode, StatusCode> {
    info!(
        "storage_clear_version: wasm_hash={}, account={}",
        req.wasm_hash, req.account_id
    );

    let result = sqlx::query(
        r#"
        DELETE FROM storage_data
        WHERE wasm_hash = $1 AND account_id = $2
        "#,
    )
    .bind(&req.wasm_hash)
    .bind(&req.account_id)
    .execute(&state.db)
    .await;

    match result {
        Ok(res) => {
            info!(
                "storage_clear_version: deleted {} rows for wasm_hash={}",
                res.rows_affected(),
                req.wasm_hash
            );
            Ok(StatusCode::OK)
        }
        Err(e) => {
            error!("storage_clear_version error: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

// Helper: Update usage for project-based storage
async fn update_project_usage(db: &sqlx::PgPool, project_uuid: &str, account_id: &str) {
    let _ = sqlx::query(
        r#"
        INSERT INTO storage_usage (project_uuid, account_id, total_bytes, key_count, updated_at)
        SELECT
            $1,
            $2,
            COALESCE(SUM(LENGTH(encrypted_key) + LENGTH(encrypted_value)), 0),
            COUNT(*)::INT,
            NOW()
        FROM storage_data
        WHERE project_uuid = $1 AND account_id = $2
        ON CONFLICT (project_uuid, account_id)
        DO UPDATE SET
            total_bytes = EXCLUDED.total_bytes,
            key_count = EXCLUDED.key_count,
            updated_at = NOW()
        "#,
    )
    .bind(project_uuid)
    .bind(account_id)
    .execute(db)
    .await;
}

/// Clear ALL storage for a project (all accounts) - called when project is deleted
/// This is an internal endpoint called by workers when they detect project_deleted event
pub async fn storage_clear_project(
    State(state): State<AppState>,
    Json(req): Json<StorageClearProjectRequest>,
) -> Result<StatusCode, StatusCode> {
    info!("storage_clear_project: project_uuid={}", req.project_uuid);

    // Delete all storage data for this project
    let data_result = sqlx::query(
        r#"
        DELETE FROM storage_data
        WHERE project_uuid = $1
        "#,
    )
    .bind(&req.project_uuid)
    .execute(&state.db)
    .await;

    // Delete all usage records for this project
    let usage_result = sqlx::query(
        r#"
        DELETE FROM storage_usage
        WHERE project_uuid = $1
        "#,
    )
    .bind(&req.project_uuid)
    .execute(&state.db)
    .await;

    match (data_result, usage_result) {
        (Ok(data_res), Ok(usage_res)) => {
            info!(
                "storage_clear_project: deleted {} data rows, {} usage rows for project={}",
                data_res.rows_affected(),
                usage_res.rows_affected(),
                req.project_uuid
            );
            Ok(StatusCode::OK)
        }
        (Err(e), _) | (_, Err(e)) => {
            error!("storage_clear_project error: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// Get public storage from another project (cross-project read)
/// Only returns data if is_encrypted=false
pub async fn storage_get_public(
    State(state): State<AppState>,
    Json(req): Json<StorageGetPublicRequest>,
) -> Result<Json<StorageGetPublicResponse>, StatusCode> {
    debug!(
        "storage_get_public: owner={}, project_id={}, key_hash={}",
        req.owner, req.project_id, req.key_hash
    );

    // First, look up project_uuid from owner and project_id
    let project_result = sqlx::query_as::<_, (String,)>(
        r#"
        SELECT uuid FROM projects
        WHERE owner = $1 AND project_id = $2
        "#,
    )
    .bind(&req.owner)
    .bind(&req.project_id)
    .fetch_optional(&state.db)
    .await;

    let project_uuid = match project_result {
        Ok(Some((uuid,))) => uuid,
        Ok(None) => {
            debug!("storage_get_public: project not found owner={} project_id={}", req.owner, req.project_id);
            return Ok(Json(StorageGetPublicResponse {
                exists: false,
                value: None,
            }));
        }
        Err(e) => {
            error!("storage_get_public: failed to lookup project: {}", e);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };

    // Now get the storage value, but only if it's public (is_encrypted=false)
    let result = sqlx::query_as::<_, (Vec<u8>, bool)>(
        r#"
        SELECT encrypted_value, is_encrypted
        FROM storage_data
        WHERE project_uuid = $1
          AND account_id = '@worker'
          AND key_hash = $2
        "#,
    )
    .bind(&project_uuid)
    .bind(&req.key_hash)
    .fetch_optional(&state.db)
    .await;

    match result {
        Ok(Some((value, is_encrypted))) => {
            if is_encrypted {
                // Data exists but is encrypted - not accessible cross-project
                debug!(
                    "storage_get_public: key exists but is encrypted, denying access: owner={} project_id={} key_hash={}",
                    req.owner, req.project_id, req.key_hash
                );
                return Err(StatusCode::FORBIDDEN);
            }
            // Public data - return plaintext value
            debug!(
                "storage_get_public: found public data: owner={} project_id={} key_hash={}",
                req.owner, req.project_id, req.key_hash
            );
            Ok(Json(StorageGetPublicResponse {
                exists: true,
                value: Some(value),
            }))
        }
        Ok(None) => {
            debug!(
                "storage_get_public: key not found: owner={} project_id={} key_hash={}",
                req.owner, req.project_id, req.key_hash
            );
            Ok(Json(StorageGetPublicResponse {
                exists: false,
                value: None,
            }))
        }
        Err(e) => {
            error!("storage_get_public error: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}
