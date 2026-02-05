use axum::{
    extract::State,
    http::StatusCode,
    Json,
};
use serde::Serialize;
use tracing::{error, warn};

use crate::AppState;

#[derive(Debug, Serialize)]
pub struct DetailedHealth {
    pub status: String,
    pub timestamp: i64,
    pub checks: HealthChecks,
}

#[derive(Debug, Serialize)]
pub struct HealthChecks {
    pub database: ServiceCheck,
    pub redis: ServiceCheck,
    pub keystore: ServiceCheck,
    pub workers: WorkersCheck,
    pub event_monitor: EventMonitorCheck,
    pub tee_attestation: TeeAttestationCheck,
}

#[derive(Debug, Serialize)]
pub struct ServiceCheck {
    pub status: String,
    pub latency_ms: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct WorkersCheck {
    pub status: String,
    pub active: i64,
    pub total: i64,
    pub details: Vec<WorkerDetail>,
}

#[derive(Debug, Serialize)]
pub struct WorkerDetail {
    pub worker_id: String,
    pub worker_name: String,
    pub status: String,
    pub last_heartbeat_secs_ago: i64,
}

#[derive(Debug, Serialize)]
pub struct EventMonitorCheck {
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chain_tip_block: Option<i64>,
    pub workers: Vec<EventMonitorWorkerDetail>,
}

#[derive(Debug, Serialize)]
pub struct EventMonitorWorkerDetail {
    pub worker_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_block: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub blocks_behind: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_update_secs_ago: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct TeeAttestationCheck {
    pub status: String,
    pub workers: Vec<TeeWorkerDetail>,
}

#[derive(Debug, Serialize)]
pub struct TeeWorkerDetail {
    pub worker_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_attestation_secs_ago: Option<i64>,
}

// Thresholds
const WORKER_HEARTBEAT_WARNING_SECS: i64 = 120; // 2 minutes
const EVENT_MONITOR_WARNING_SECS: i64 = 300; // 5 minutes
const EVENT_MONITOR_BLOCKS_BEHIND_WARNING: i64 = 100;
const TEE_ATTESTATION_WARNING_SECS: i64 = 3600; // 1 hour

/// Detailed health check endpoint
///
/// Returns structured health information about all system components.
/// Used by Uptime Kuma for monitoring and alerting.
///
/// - HTTP 200 + "healthy": all checks pass
/// - HTTP 200 + "degraded": some warnings (stale heartbeat, event monitor lag)
/// - HTTP 503 + "unhealthy": critical issues (DB/Redis down, no active workers)
pub async fn health_detailed(
    State(state): State<AppState>,
) -> (StatusCode, Json<DetailedHealth>) {
    let timestamp = chrono::Utc::now().timestamp();
    let mut is_unhealthy = false;
    let mut is_degraded = false;

    // Run independent checks in parallel (DB, Redis, keystore, and DB-dependent checks)
    let (db_check, redis_check, keystore_check, workers_check, event_monitor_check, tee_check) = tokio::join!(
        check_database(&state),
        check_redis(&state),
        check_keystore(&state),
        check_workers(&state),
        check_event_monitor(&state),
        check_tee_attestation(&state),
    );

    if db_check.status == "error" {
        is_unhealthy = true;
    }

    if redis_check.status == "error" {
        is_unhealthy = true;
    }

    if keystore_check.status == "error" {
        is_degraded = true;
    }

    match workers_check.status.as_str() {
        "critical" | "error" => is_unhealthy = true,
        "warning" => is_degraded = true,
        _ => {}
    }

    match event_monitor_check.status.as_str() {
        "error" | "critical" => is_degraded = true,
        "warning" => is_degraded = true,
        _ => {}
    }

    match tee_check.status.as_str() {
        "error" => is_degraded = true,
        "warning" => is_degraded = true,
        _ => {}
    }

    let status = if is_unhealthy {
        "unhealthy"
    } else if is_degraded {
        "degraded"
    } else {
        "healthy"
    };

    let http_status = if is_unhealthy {
        StatusCode::SERVICE_UNAVAILABLE
    } else {
        StatusCode::OK
    };

    (
        http_status,
        Json(DetailedHealth {
            status: status.to_string(),
            timestamp,
            checks: HealthChecks {
                database: db_check,
                redis: redis_check,
                keystore: keystore_check,
                workers: workers_check,
                event_monitor: event_monitor_check,
                tee_attestation: tee_check,
            },
        }),
    )
}

async fn check_database(state: &AppState) -> ServiceCheck {
    let start = std::time::Instant::now();
    match sqlx::query_as::<_, (i32,)>("SELECT 1")
        .fetch_one(&state.db)
        .await
    {
        Ok(_) => ServiceCheck {
            status: "ok".to_string(),
            latency_ms: Some(start.elapsed().as_millis() as i64),
            error: None,
        },
        Err(e) => {
            error!("Health check: database error: {}", e);
            ServiceCheck {
                status: "error".to_string(),
                latency_ms: None,
                error: Some("database unavailable".to_string()),
            }
        }
    }
}

async fn check_redis(state: &AppState) -> ServiceCheck {
    let start = std::time::Instant::now();
    match state.redis.get_multiplexed_async_connection().await {
        Ok(mut conn) => {
            match redis::cmd("PING")
                .query_async::<_, String>(&mut conn)
                .await
            {
                Ok(_) => ServiceCheck {
                    status: "ok".to_string(),
                    latency_ms: Some(start.elapsed().as_millis() as i64),
                    error: None,
                },
                Err(e) => {
                    error!("Health check: Redis PING failed: {}", e);
                    ServiceCheck {
                        status: "error".to_string(),
                        latency_ms: None,
                        error: Some("redis unavailable".to_string()),
                    }
                }
            }
        }
        Err(e) => {
            error!("Health check: Redis connection failed: {}", e);
            ServiceCheck {
                status: "error".to_string(),
                latency_ms: None,
                error: Some("redis unavailable".to_string()),
            }
        }
    }
}

async fn check_keystore(state: &AppState) -> ServiceCheck {
    let keystore_url = match &state.config.keystore_base_url {
        Some(url) => url,
        None => {
            return ServiceCheck {
                status: "skipped".to_string(),
                latency_ms: None,
                error: Some("KEYSTORE_BASE_URL not configured".to_string()),
            };
        }
    };

    let start = std::time::Instant::now();
    let client = crate::near_client::health_check_client();

    match client.get(format!("{}/health", keystore_url)).send().await {
        Ok(resp) => {
            let latency = start.elapsed().as_millis() as i64;
            if resp.status().is_success() {
                ServiceCheck {
                    status: "ok".to_string(),
                    latency_ms: Some(latency),
                    error: None,
                }
            } else {
                warn!("Health check: keystore returned HTTP {}", resp.status());
                ServiceCheck {
                    status: "error".to_string(),
                    latency_ms: Some(latency),
                    error: Some("keystore unhealthy".to_string()),
                }
            }
        }
        Err(e) => {
            error!("Health check: keystore unreachable: {}", e);
            ServiceCheck {
                status: "error".to_string(),
                latency_ms: None,
                error: Some("keystore unreachable".to_string()),
            }
        }
    }
}

#[derive(sqlx::FromRow)]
struct WorkerRow {
    worker_id: String,
    worker_name: String,
    status: String,
    heartbeat_secs_ago: Option<i64>,
}

async fn check_workers(state: &AppState) -> WorkersCheck {
    let workers: Vec<WorkerRow> = match sqlx::query_as(
        r#"
        SELECT
            worker_id,
            worker_name,
            status,
            EXTRACT(EPOCH FROM (NOW() - last_heartbeat_at))::BIGINT as heartbeat_secs_ago
        FROM worker_status
        WHERE last_heartbeat_at > NOW() - INTERVAL '24 hours'
        ORDER BY last_heartbeat_at DESC
        "#,
    )
    .fetch_all(&state.db)
    .await
    {
        Ok(rows) => rows,
        Err(e) => {
            error!("Health check: failed to query workers: {}", e);
            return WorkersCheck {
                status: "error".to_string(),
                active: 0,
                total: 0,
                details: vec![],
            };
        }
    };

    let total = workers.len() as i64;
    let active = workers
        .iter()
        .filter(|w| {
            (w.status == "online" || w.status == "busy")
                && w.heartbeat_secs_ago.unwrap_or(i64::MAX) < WORKER_HEARTBEAT_WARNING_SECS
        })
        .count() as i64;

    let has_stale = workers.iter().any(|w| {
        (w.status == "online" || w.status == "busy")
            && w.heartbeat_secs_ago.unwrap_or(i64::MAX) >= WORKER_HEARTBEAT_WARNING_SECS
    });

    let status = if active == 0 {
        "critical"
    } else if has_stale {
        "warning"
    } else {
        "ok"
    };

    let details = workers
        .into_iter()
        .map(|w| WorkerDetail {
            worker_id: w.worker_id,
            worker_name: w.worker_name,
            status: w.status,
            last_heartbeat_secs_ago: w.heartbeat_secs_ago.unwrap_or(-1),
        })
        .collect();

    WorkersCheck {
        status: status.to_string(),
        active,
        total,
        details,
    }
}

#[derive(sqlx::FromRow)]
struct EventMonitorRow {
    worker_id: String,
    event_monitor_block_height: Option<i64>,
    monitor_secs_ago: Option<i64>,
}

async fn check_event_monitor(state: &AppState) -> EventMonitorCheck {
    // Fetch chain tip from NEAR RPC (non-blocking, best-effort)
    let chain_tip = match crate::near_client::fetch_latest_block_height(&state.config.near_rpc_url).await {
        Ok(height) => Some(height as i64),
        Err(e) => {
            warn!("Health check: failed to fetch NEAR chain tip: {}", e);
            None
        }
    };

    let monitors: Vec<EventMonitorRow> = match sqlx::query_as(
        r#"
        SELECT
            worker_id,
            event_monitor_block_height,
            EXTRACT(EPOCH FROM (NOW() - event_monitor_updated_at))::BIGINT as monitor_secs_ago
        FROM worker_status
        WHERE status IN ('online', 'busy')
          AND last_heartbeat_at > NOW() - INTERVAL '5 minutes'
          AND event_monitor_block_height IS NOT NULL
        ORDER BY event_monitor_block_height DESC
        "#,
    )
    .fetch_all(&state.db)
    .await
    {
        Ok(rows) => rows,
        Err(e) => {
            error!("Health check: failed to query event monitors: {}", e);
            return EventMonitorCheck {
                status: "error".to_string(),
                chain_tip_block: chain_tip,
                workers: vec![],
            };
        }
    };

    let has_stale = monitors.iter().any(|m| {
        m.monitor_secs_ago.unwrap_or(i64::MAX) >= EVENT_MONITOR_WARNING_SECS
    });

    let has_lag = chain_tip.is_some() && monitors.iter().any(|m| {
        if let (Some(tip), Some(current)) = (chain_tip, m.event_monitor_block_height) {
            (tip - current).max(0) > EVENT_MONITOR_BLOCKS_BEHIND_WARNING
        } else {
            false
        }
    });

    let status = if monitors.is_empty() {
        // No event monitors reporting — could be OK if not configured
        "unknown"
    } else if has_stale || has_lag {
        "warning"
    } else {
        "ok"
    };

    let workers = monitors
        .into_iter()
        .map(|m| {
            let blocks_behind = chain_tip.and_then(|tip| {
                m.event_monitor_block_height.map(|current| (tip - current).max(0))
            });
            EventMonitorWorkerDetail {
                worker_id: m.worker_id,
                current_block: m.event_monitor_block_height,
                blocks_behind,
                last_update_secs_ago: m.monitor_secs_ago,
            }
        })
        .collect();

    EventMonitorCheck {
        status: status.to_string(),
        chain_tip_block: chain_tip,
        workers,
    }
}

#[derive(sqlx::FromRow)]
struct TeeRow {
    worker_name: String,
    attestation_secs_ago: Option<i64>,
}

async fn check_tee_attestation(state: &AppState) -> TeeAttestationCheck {
    let workers: Vec<TeeRow> = match sqlx::query_as(
        r#"
        SELECT
            wat.worker_name,
            EXTRACT(EPOCH FROM (NOW() - wat.last_attestation_at))::BIGINT as attestation_secs_ago
        FROM worker_auth_tokens wat
        INNER JOIN worker_status ws ON wat.worker_name = ws.worker_name
        WHERE ws.status IN ('online', 'busy')
          AND ws.last_heartbeat_at > NOW() - INTERVAL '5 minutes'
          AND wat.is_active = true
        "#,
    )
    .fetch_all(&state.db)
    .await
    {
        Ok(rows) => rows,
        Err(e) => {
            error!("Health check: failed to query TEE attestation: {}", e);
            return TeeAttestationCheck {
                status: "error".to_string(),
                workers: vec![],
            };
        }
    };

    let has_stale = workers.iter().any(|w| {
        w.attestation_secs_ago.unwrap_or(i64::MAX) >= TEE_ATTESTATION_WARNING_SECS
    });

    let status = if has_stale { "warning" } else { "ok" };

    let details = workers
        .into_iter()
        .map(|w| TeeWorkerDetail {
            worker_name: w.worker_name,
            last_attestation_secs_ago: w.attestation_secs_ago,
        })
        .collect();

    TeeAttestationCheck {
        status: status.to_string(),
        workers: details,
    }
}
