use anyhow::Result;
use sqlx::PgPool;

use crate::health_types::DetailedHealth;

pub async fn init_schema(pool: &PgPool) -> Result<()> {
    let statements = [
        r#"CREATE TABLE IF NOT EXISTS health_snapshots (
            id              BIGSERIAL PRIMARY KEY,
            collected_at    TIMESTAMPTZ NOT NULL DEFAULT NOW(),
            network         TEXT NOT NULL,
            status          TEXT NOT NULL,
            db_status       TEXT NOT NULL,
            db_latency_ms   BIGINT,
            redis_status    TEXT NOT NULL,
            redis_latency_ms BIGINT,
            keystore_status TEXT NOT NULL,
            keystore_latency_ms BIGINT,
            workers_status  TEXT NOT NULL,
            workers_active  BIGINT NOT NULL,
            workers_total   BIGINT NOT NULL,
            event_monitor_status TEXT NOT NULL,
            chain_tip_block BIGINT,
            tee_status      TEXT NOT NULL,
            fetch_error     TEXT
        )"#,
        r#"CREATE TABLE IF NOT EXISTS worker_snapshots (
            id              BIGSERIAL PRIMARY KEY,
            collected_at    TIMESTAMPTZ NOT NULL DEFAULT NOW(),
            network         TEXT NOT NULL,
            worker_id       TEXT NOT NULL,
            worker_name     TEXT NOT NULL,
            status          TEXT NOT NULL,
            heartbeat_secs_ago BIGINT NOT NULL
        )"#,
        r#"CREATE TABLE IF NOT EXISTS event_monitor_snapshots (
            id                  BIGSERIAL PRIMARY KEY,
            collected_at        TIMESTAMPTZ NOT NULL DEFAULT NOW(),
            network             TEXT NOT NULL,
            worker_id           TEXT NOT NULL,
            current_block       BIGINT,
            blocks_behind       BIGINT,
            last_update_secs_ago BIGINT
        )"#,
        r#"CREATE TABLE IF NOT EXISTS tee_attestation_snapshots (
            id                      BIGSERIAL PRIMARY KEY,
            collected_at            TIMESTAMPTZ NOT NULL DEFAULT NOW(),
            network                 TEXT NOT NULL,
            worker_name             TEXT NOT NULL,
            last_attestation_secs_ago BIGINT
        )"#,
        r#"CREATE TABLE IF NOT EXISTS alert_state (
            network         TEXT PRIMARY KEY,
            last_status     TEXT NOT NULL,
            last_changed_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
        )"#,
        "CREATE INDEX IF NOT EXISTS idx_health_snapshots_time ON health_snapshots (collected_at, network)",
        "CREATE INDEX IF NOT EXISTS idx_worker_snapshots_time ON worker_snapshots (collected_at, network, worker_id)",
        "CREATE INDEX IF NOT EXISTS idx_event_monitor_snapshots_time ON event_monitor_snapshots (collected_at, network, worker_id)",
        "CREATE INDEX IF NOT EXISTS idx_tee_attestation_snapshots_time ON tee_attestation_snapshots (collected_at, network, worker_name)",
    ];

    for sql in &statements {
        sqlx::query(sql).execute(pool).await?;
    }

    tracing::info!("Database schema initialized");
    Ok(())
}

pub async fn insert_snapshot(
    pool: &PgPool,
    network: &str,
    health: &DetailedHealth,
) -> Result<()> {
    let mut tx = pool.begin().await?;

    // 1. Health snapshot
    sqlx::query(
        r#"
        INSERT INTO health_snapshots (
            network, status,
            db_status, db_latency_ms,
            redis_status, redis_latency_ms,
            keystore_status, keystore_latency_ms,
            workers_status, workers_active, workers_total,
            event_monitor_status, chain_tip_block,
            tee_status
        ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14)
        "#,
    )
    .bind(network)
    .bind(&health.status)
    .bind(&health.checks.database.status)
    .bind(health.checks.database.latency_ms)
    .bind(&health.checks.redis.status)
    .bind(health.checks.redis.latency_ms)
    .bind(&health.checks.keystore.status)
    .bind(health.checks.keystore.latency_ms)
    .bind(&health.checks.workers.status)
    .bind(health.checks.workers.active)
    .bind(health.checks.workers.total)
    .bind(&health.checks.event_monitor.status)
    .bind(health.checks.event_monitor.chain_tip_block)
    .bind(&health.checks.tee_attestation.status)
    .execute(&mut *tx)
    .await?;

    // 2. Worker snapshots
    for w in &health.checks.workers.details {
        sqlx::query(
            r#"
            INSERT INTO worker_snapshots (network, worker_id, worker_name, status, heartbeat_secs_ago)
            VALUES ($1, $2, $3, $4, $5)
            "#,
        )
        .bind(network)
        .bind(&w.worker_id)
        .bind(&w.worker_name)
        .bind(&w.status)
        .bind(w.last_heartbeat_secs_ago)
        .execute(&mut *tx)
        .await?;
    }

    // 3. Event monitor snapshots
    for m in &health.checks.event_monitor.workers {
        sqlx::query(
            r#"
            INSERT INTO event_monitor_snapshots (network, worker_id, current_block, blocks_behind, last_update_secs_ago)
            VALUES ($1, $2, $3, $4, $5)
            "#,
        )
        .bind(network)
        .bind(&m.worker_id)
        .bind(m.current_block)
        .bind(m.blocks_behind)
        .bind(m.last_update_secs_ago)
        .execute(&mut *tx)
        .await?;
    }

    // 4. TEE attestation snapshots
    for t in &health.checks.tee_attestation.workers {
        sqlx::query(
            r#"
            INSERT INTO tee_attestation_snapshots (network, worker_name, last_attestation_secs_ago)
            VALUES ($1, $2, $3)
            "#,
        )
        .bind(network)
        .bind(&t.worker_name)
        .bind(t.last_attestation_secs_ago)
        .execute(&mut *tx)
        .await?;
    }

    tx.commit().await?;
    Ok(())
}

pub async fn insert_error_snapshot(pool: &PgPool, network: &str, error: &str) -> Result<()> {
    sqlx::query(
        r#"
        INSERT INTO health_snapshots (
            network, status,
            db_status, redis_status, keystore_status,
            workers_status, workers_active, workers_total,
            event_monitor_status, tee_status, fetch_error
        ) VALUES ($1, 'unreachable', '', '', '', '', 0, 0, '', '', $2)
        "#,
    )
    .bind(network)
    .bind(error)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn cleanup_old_data(pool: &PgPool, retention_days: u32) -> Result<()> {
    let interval = format!("{} days", retention_days);

    let mut deleted_total: u64 = 0;
    for table in &[
        "health_snapshots",
        "worker_snapshots",
        "event_monitor_snapshots",
        "tee_attestation_snapshots",
    ] {
        let result = sqlx::query(&format!(
            "DELETE FROM {} WHERE collected_at < NOW() - $1::INTERVAL",
            table
        ))
        .bind(&interval)
        .execute(pool)
        .await?;
        deleted_total += result.rows_affected();
    }

    if deleted_total > 0 {
        tracing::info!("Cleanup: deleted {} old rows", deleted_total);
    }
    Ok(())
}

/// Get the last known alert status for a network. Returns None if no prior state.
pub async fn get_alert_state(pool: &PgPool, network: &str) -> Result<Option<String>> {
    let row: Option<(String,)> = sqlx::query_as(
        "SELECT last_status FROM alert_state WHERE network = $1",
    )
    .bind(network)
    .fetch_optional(pool)
    .await?;
    Ok(row.map(|r| r.0))
}

/// Update the alert state for a network.
pub async fn set_alert_state(pool: &PgPool, network: &str, status: &str) -> Result<()> {
    sqlx::query(
        r#"
        INSERT INTO alert_state (network, last_status, last_changed_at)
        VALUES ($1, $2, NOW())
        ON CONFLICT (network)
        DO UPDATE SET last_status = $2, last_changed_at = NOW()
        "#,
    )
    .bind(network)
    .bind(status)
    .execute(pool)
    .await?;
    Ok(())
}
