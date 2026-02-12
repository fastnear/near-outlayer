//! Project UUID resolution handler
//!
//! Provides endpoint for workers to resolve project_id → project_uuid + active_version.
//! UUID is cached in Redis (never changes). active_version is always fetched from contract.

use axum::{
    extract::{Query, State},
    http::StatusCode,
    Json,
};
use redis::AsyncCommands;
use serde::{Deserialize, Serialize};
use tracing::{debug, info, warn};

use crate::{near_client, AppState};

/// Redis key prefix for project UUID cache (UUID only, not active_version)
const PROJECT_UUID_CACHE_PREFIX: &str = "project_by_uuid:";

#[derive(Debug, Deserialize)]
pub struct ResolveProjectQuery {
    /// Project ID in format "owner.near/name"
    pub project_id: String,
    /// If version is already known, skip contract call and return cached UUID only
    pub version: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ProjectUuidResponse {
    pub project_id: String,
    pub uuid: String,
    pub active_version: String,
    pub cached: bool,
}

#[derive(Debug, Serialize)]
pub struct ProjectErrorResponse {
    pub error: String,
}

/// Resolve project_id to project_uuid and active_version
///
/// GET /projects/uuid?project_id=alice.near/my-app
/// GET /projects/uuid?project_id=alice.near/my-app&version=abc123
///
/// UUID is cached in Redis (it never changes).
/// If `version` param is provided, returns cached UUID + that version (no contract call).
/// If `version` is omitted, always calls contract to get the current active_version.
pub async fn resolve_project_uuid(
    State(state): State<AppState>,
    Query(query): Query<ResolveProjectQuery>,
) -> Result<Json<ProjectUuidResponse>, (StatusCode, Json<ProjectErrorResponse>)> {
    let project_id = &query.project_id;

    // If version is already known, try to return cached UUID without contract call
    if let Some(ref version) = query.version {
        if let Some(uuid) = get_cached_uuid(&state, project_id).await {
            debug!("Cache hit for project UUID: {} -> {}", project_id, uuid);
            return Ok(Json(ProjectUuidResponse {
                project_id: project_id.clone(),
                uuid,
                active_version: version.clone(),
                cached: true,
            }));
        }
    }

    // Fetch from contract (always when version is None, or on cache miss)
    let project = near_client::fetch_project_from_contract(
        &state.config.near_rpc_url,
        &state.config.contract_id,
        project_id,
    )
    .await
    .map_err(|e| {
        warn!("Failed to fetch project from contract: {}", e);
        (
            StatusCode::BAD_GATEWAY,
            Json(ProjectErrorResponse {
                error: format!("Contract query failed: {}", e),
            }),
        )
    })?;

    match project {
        Some(project_info) => {
            // Cache UUID only (it never changes)
            cache_uuid(&state, project_id, &project_info.uuid).await;

            let active_version = query.version.unwrap_or(project_info.active_version);

            Ok(Json(ProjectUuidResponse {
                project_id: project_id.clone(),
                uuid: project_info.uuid,
                active_version,
                cached: false,
            }))
        }
        None => Err((
            StatusCode::NOT_FOUND,
            Json(ProjectErrorResponse {
                error: format!("Project not found: {}", project_id),
            }),
        )),
    }
}

/// Get cached UUID from Redis (returns None on miss or error)
async fn get_cached_uuid(state: &AppState, project_id: &str) -> Option<String> {
    let cache_key = format!("{}{}", PROJECT_UUID_CACHE_PREFIX, project_id);
    let mut conn = state.redis.get_multiplexed_async_connection().await.ok()?;
    conn.get(&cache_key).await.unwrap_or(None)
}

/// Cache UUID in Redis (forever — UUIDs never change)
async fn cache_uuid(state: &AppState, project_id: &str, uuid: &str) {
    let cache_key = format!("{}{}", PROJECT_UUID_CACHE_PREFIX, project_id);
    if let Ok(mut conn) = state.redis.get_multiplexed_async_connection().await {
        let _: Result<(), _> = conn.set(&cache_key, uuid).await;
        info!("Cached project UUID: {} -> {}", project_id, uuid);
    }
}

/// Invalidate project UUID cache
///
/// DELETE /projects/cache?project_id=alice.near/my-app
pub async fn invalidate_project_cache(
    State(state): State<AppState>,
    Query(query): Query<ResolveProjectQuery>,
) -> Result<StatusCode, (StatusCode, Json<ProjectErrorResponse>)> {
    let project_id = &query.project_id;
    let cache_key = format!("{}{}", PROJECT_UUID_CACHE_PREFIX, project_id);

    let mut redis_conn = state
        .redis
        .get_multiplexed_async_connection()
        .await
        .map_err(|e| {
            warn!("Redis connection error: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ProjectErrorResponse {
                    error: "Cache unavailable".to_string(),
                }),
            )
        })?;

    let _: Result<(), _> = redis_conn.del(&cache_key).await;
    info!("Invalidated cache for project: {}", project_id);

    Ok(StatusCode::NO_CONTENT)
}

/// Resolve project_uuid from Redis cache, falling back to contract.
/// Used by HTTPS call handler — only needs UUID, not active_version.
pub async fn resolve_project_uuid_for_call(
    state: &AppState,
    project_id: &str,
) -> Result<String, String> {
    // Try cache first
    if let Some(uuid) = get_cached_uuid(state, project_id).await {
        return Ok(uuid);
    }

    // Cache miss — fetch from contract
    info!("Cache miss for project UUID: {}, fetching from contract", project_id);

    let project = near_client::fetch_project_from_contract(
        &state.config.near_rpc_url,
        &state.config.contract_id,
        project_id,
    )
    .await
    .map_err(|e| format!("Contract query failed: {}", e))?;

    match project {
        Some(project_info) => {
            cache_uuid(state, project_id, &project_info.uuid).await;
            Ok(project_info.uuid)
        }
        None => Err(format!("Project not found: {}", project_id)),
    }
}
