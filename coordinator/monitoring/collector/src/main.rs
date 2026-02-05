mod alerting;
mod collector;
mod config;
mod db;
mod health_types;

use std::time::Duration;

use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "outlayer_health_collector=info".into()),
        )
        .init();

    let config = config::Config::from_env()?;

    // Connect to PostgreSQL with retry
    let pool = connect_with_retry(&config.database_url).await?;
    db::init_schema(&pool).await?;

    let http_client = reqwest::Client::builder()
        .timeout(Duration::from_secs(config.request_timeout_seconds))
        .build()?;

    let alerter = alerting::Alerter::new(config.telegram, http_client.clone());

    tracing::info!(
        targets = config.targets.len(),
        poll_interval = config.poll_interval_seconds,
        retention_days = config.retention_days,
        telegram = alerter_has_telegram(&alerter),
        "Health collector started"
    );

    let mut poll_interval = tokio::time::interval(Duration::from_secs(config.poll_interval_seconds));
    let mut cleanup_interval = tokio::time::interval(Duration::from_secs(3600));

    loop {
        tokio::select! {
            _ = poll_interval.tick() => {
                for target in &config.targets {
                    collector::poll_target(&http_client, &pool, target, &alerter).await;
                }
            }
            _ = cleanup_interval.tick() => {
                if let Err(e) = db::cleanup_old_data(&pool, config.retention_days).await {
                    tracing::warn!(error = %e, "Failed to clean up old data");
                }
            }
        }
    }
}

async fn connect_with_retry(database_url: &str) -> Result<sqlx::PgPool> {
    let delays = [2, 4, 8];
    let mut last_error = None;

    for (attempt, delay_secs) in delays.iter().enumerate() {
        match sqlx::PgPool::connect(database_url).await {
            Ok(pool) => {
                tracing::info!("Connected to database");
                return Ok(pool);
            }
            Err(e) => {
                tracing::warn!(
                    attempt = attempt + 1,
                    delay = delay_secs,
                    error = %e,
                    "Database connection failed, retrying..."
                );
                last_error = Some(e);
                tokio::time::sleep(Duration::from_secs(*delay_secs)).await;
            }
        }
    }

    // Final attempt
    match sqlx::PgPool::connect(database_url).await {
        Ok(pool) => {
            tracing::info!("Connected to database");
            Ok(pool)
        }
        Err(e) => {
            Err(last_error.unwrap_or(e).into())
        }
    }
}

// Helper to check if alerter has telegram configured (for startup log)
fn alerter_has_telegram(_alerter: &alerting::Alerter) -> bool {
    // Check via env var since Alerter doesn't expose config
    std::env::var("TELEGRAM_BOT_TOKEN")
        .ok()
        .filter(|s| !s.is_empty())
        .is_some()
}
