use anyhow::{bail, Context, Result};

pub struct Config {
    pub targets: Vec<CoordinatorTarget>,
    pub database_url: String,
    pub poll_interval_seconds: u64,
    pub retention_days: u32,
    pub request_timeout_seconds: u64,
    pub telegram: Option<TelegramConfig>,
}

pub struct CoordinatorTarget {
    pub label: String,
    pub url: String,
}

pub struct TelegramConfig {
    pub bot_token: String,
    pub chat_id: String,
}

impl Config {
    pub fn from_env() -> Result<Self> {
        let targets_str = std::env::var("COLLECTOR_TARGETS")
            .context("COLLECTOR_TARGETS is required (e.g. mainnet=https://api.outlayer.fastnear.com)")?;

        let targets = parse_targets(&targets_str)?;
        if targets.is_empty() {
            bail!("COLLECTOR_TARGETS must contain at least one target");
        }

        let database_url = std::env::var("COLLECTOR_DATABASE_URL")
            .unwrap_or_else(|_| "postgres://collector:collector@collector-db:5432/health".to_string());

        let poll_interval_seconds = std::env::var("COLLECTOR_POLL_INTERVAL")
            .unwrap_or_else(|_| "30".to_string())
            .parse()
            .context("COLLECTOR_POLL_INTERVAL must be a number")?;

        let retention_days = std::env::var("COLLECTOR_RETENTION_DAYS")
            .unwrap_or_else(|_| "90".to_string())
            .parse()
            .context("COLLECTOR_RETENTION_DAYS must be a number")?;

        let request_timeout_seconds = std::env::var("COLLECTOR_REQUEST_TIMEOUT")
            .unwrap_or_else(|_| "15".to_string())
            .parse()
            .context("COLLECTOR_REQUEST_TIMEOUT must be a number")?;

        let telegram = match (
            std::env::var("TELEGRAM_BOT_TOKEN").ok().filter(|s| !s.is_empty()),
            std::env::var("TELEGRAM_CHAT_ID").ok().filter(|s| !s.is_empty()),
        ) {
            (Some(bot_token), Some(chat_id)) => Some(TelegramConfig { bot_token, chat_id }),
            (Some(_), None) => bail!("TELEGRAM_BOT_TOKEN is set but TELEGRAM_CHAT_ID is missing"),
            _ => None,
        };

        Ok(Config {
            targets,
            database_url,
            poll_interval_seconds,
            retention_days,
            request_timeout_seconds,
            telegram,
        })
    }
}

/// Parse "mainnet=https://api.example.com,testnet=https://testnet.example.com"
fn parse_targets(s: &str) -> Result<Vec<CoordinatorTarget>> {
    let mut targets = Vec::new();
    for part in s.split(',') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        let (label, url) = part
            .split_once('=')
            .with_context(|| format!("Invalid target format: '{}'. Expected 'label=url'", part))?;
        targets.push(CoordinatorTarget {
            label: label.trim().to_string(),
            url: url.trim().to_string(),
        });
    }
    Ok(targets)
}
