use anyhow::Result;
use sqlx::PgPool;

use crate::alerting::Alerter;
use crate::config::CoordinatorTarget;
use crate::db;
use crate::health_types::DetailedHealth;

pub async fn poll_target(
    client: &reqwest::Client,
    pool: &PgPool,
    target: &CoordinatorTarget,
    alerter: &Alerter,
) {
    let result = fetch_and_store(client, pool, target).await;

    let new_status = match &result {
        Ok(health) => health.status.clone(),
        Err(e) => {
            // Store error snapshot so Grafana shows the gap
            let _ = db::insert_error_snapshot(pool, &target.label, &e.to_string()).await;
            "unreachable".to_string()
        }
    };

    // Check alert state and send telegram if status changed
    match db::get_alert_state(pool, &target.label).await {
        Ok(prev_status) => {
            let changed = prev_status.as_deref() != Some(&new_status);
            if changed {
                // Build alert context
                let context = match &result {
                    Ok(health) => format_health_context(health),
                    Err(e) => format!("Error: {}", e),
                };

                alerter
                    .send_alert(&target.label, &new_status, &context)
                    .await;

                if let Err(e) = db::set_alert_state(pool, &target.label, &new_status).await {
                    tracing::error!(
                        network = %target.label,
                        error = %e,
                        "Failed to update alert state"
                    );
                }
            }
        }
        Err(e) => {
            tracing::error!(
                network = %target.label,
                error = %e,
                "Failed to read alert state"
            );
        }
    }

    if let Err(e) = &result {
        tracing::warn!(
            network = %target.label,
            error = %e,
            "Failed to collect health data"
        );
    }
}

async fn fetch_and_store(
    client: &reqwest::Client,
    pool: &PgPool,
    target: &CoordinatorTarget,
) -> Result<DetailedHealth> {
    let url = format!("{}/health/detailed", target.url);

    let resp = client.get(&url).send().await?;
    let status_code = resp.status();

    // 503 is expected for "unhealthy" — still valid data
    if !status_code.is_success() && status_code.as_u16() != 503 {
        let body = resp.text().await.unwrap_or_default();
        anyhow::bail!("HTTP {} from {}: {}", status_code, target.label, body);
    }

    let health: DetailedHealth = resp.json().await?;

    db::insert_snapshot(pool, &target.label, &health).await?;

    tracing::debug!(
        network = %target.label,
        status = %health.status,
        workers_active = health.checks.workers.active,
        "Health collected"
    );

    Ok(health)
}

fn format_health_context(health: &DetailedHealth) -> String {
    let mut parts = Vec::new();

    // Service statuses
    let db = &health.checks.database;
    let redis = &health.checks.redis;
    let ks = &health.checks.keystore;
    parts.push(format!(
        "DB: {} {} | Redis: {} {} | Keystore: {}{}",
        db.status,
        db.latency_ms.map(|ms| format!("({}ms)", ms)).unwrap_or_default(),
        redis.status,
        redis.latency_ms.map(|ms| format!("({}ms)", ms)).unwrap_or_default(),
        ks.status,
        ks.latency_ms.map(|ms| format!(" ({}ms)", ms)).unwrap_or_default(),
    ));

    // Workers
    let w = &health.checks.workers;
    parts.push(format!("Workers: {} active / {} total", w.active, w.total));

    // Stale workers
    for worker in &w.details {
        if worker.last_heartbeat_secs_ago > 120 {
            parts.push(format!(
                "  {} heartbeat stale ({}s)",
                worker.worker_name, worker.last_heartbeat_secs_ago
            ));
        }
    }

    // Event monitor
    let em = &health.checks.event_monitor;
    if !em.workers.is_empty() {
        for m in &em.workers {
            if let Some(behind) = m.blocks_behind {
                if behind > 100 {
                    parts.push(format!(
                        "Event monitor {}: {} blocks behind",
                        m.worker_id, behind
                    ));
                }
            }
        }
    }

    // TEE
    for t in &health.checks.tee_attestation.workers {
        if let Some(secs) = t.last_attestation_secs_ago {
            if secs > 3600 {
                parts.push(format!(
                    "TEE {}: attestation stale ({}s)",
                    t.worker_name, secs
                ));
            }
        }
    }

    parts.join("\n")
}
