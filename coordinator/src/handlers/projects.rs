//! Project UUID resolution handler
//!
//! Provides endpoint for workers to resolve project_id → project_uuid.
//! Uses Redis cache (forever) since UUIDs never change.

use axum::{
    extract::{Query, State},
    http::StatusCode,
    Json,
};
use redis::AsyncCommands;
use serde::{Deserialize, Serialize};
use tracing::{debug, info, warn};

use crate::{near_client, AppState};

/// Redis key prefix for project UUID cache
const PROJECT_UUID_CACHE_PREFIX: &str = "project_uuid:";

#[derive(Debug, Deserialize)]
pub struct ResolveProjectQuery {
    /// Project ID in format "owner.near/name"
    pub project_id: String,
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

/// Resolve project_id to project_uuid
///
/// GET /projects/uuid?project_id=alice.near/my-app
///
/// Uses Redis cache - UUIDs are cached forever since they never change.
/// Only invalidated by project deletion (which is a separate cleanup process).
pub async fn resolve_project_uuid(
    State(state): State<AppState>,
    Query(query): Query<ResolveProjectQuery>,
) -> Result<Json<ProjectUuidResponse>, (StatusCode, Json<ProjectErrorResponse>)> {
    let project_id = &query.project_id;
    let cache_key = format!("{}{}", PROJECT_UUID_CACHE_PREFIX, project_id);

    // Try Redis cache first
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

    // Check cache
    let cached: Option<String> = redis_conn.get(&cache_key).await.unwrap_or(None);

    if let Some(cached_json) = cached {
        // Parse cached data
        if let Ok(cached_data) = serde_json::from_str::<CachedProjectData>(&cached_json) {
            debug!("Cache hit for project: {}", project_id);
            return Ok(Json(ProjectUuidResponse {
                project_id: project_id.clone(),
                uuid: cached_data.uuid,
                active_version: cached_data.active_version,
                cached: true,
            }));
        }
    }

    // Cache miss - fetch from contract
    info!("Cache miss for project: {}, fetching from contract", project_id);

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
            // Cache the result (forever - UUIDs don't change)
            let cache_data = CachedProjectData {
                uuid: project_info.uuid.clone(),
                active_version: project_info.active_version.clone(),
            };

            if let Ok(cache_json) = serde_json::to_string(&cache_data) {
                let _: Result<(), _> = redis_conn.set(&cache_key, &cache_json).await;
                debug!("Cached project UUID: {} -> {}", project_id, project_info.uuid);
            }

            Ok(Json(ProjectUuidResponse {
                project_id: project_id.clone(),
                uuid: project_info.uuid,
                active_version: project_info.active_version,
                cached: false,
            }))
        }
        None => {
            Err((
                StatusCode::NOT_FOUND,
                Json(ProjectErrorResponse {
                    error: format!("Project not found: {}", project_id),
                }),
            ))
        }
    }
}

/// Invalidate project cache (called when project is deleted)
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

/// Cached project data (stored in Redis)
#[derive(Debug, Serialize, Deserialize)]
struct CachedProjectData {
    uuid: String,
    active_version: String,
}
