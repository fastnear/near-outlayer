use crate::config::TelegramConfig;

pub struct Alerter {
    config: Option<TelegramConfig>,
    client: reqwest::Client,
}

impl Alerter {
    pub fn new(config: Option<TelegramConfig>, client: reqwest::Client) -> Self {
        Self { config, client }
    }

    pub async fn send_alert(&self, network: &str, status: &str, context: &str) {
        let emoji = match status {
            "healthy" => "\u{2705}",     // green checkmark
            "degraded" => "\u{1f7e1}",   // yellow circle
            "unhealthy" => "\u{1f534}",  // red circle
            "unreachable" => "\u{1f534}", // red circle
            _ => "\u{2753}",             // question mark
        };

        let status_upper = status.to_uppercase();
        let message = format!(
            "{} [{}] Coordinator is {}\n{}",
            emoji, network, status_upper, context
        );

        tracing::info!(
            network = %network,
            status = %status,
            "Alert: status changed"
        );

        if let Some(config) = &self.config {
            if let Err(e) = self.send_telegram(config, &message).await {
                tracing::error!(
                    error = %e,
                    "Failed to send Telegram alert"
                );
            }
        }
    }

    async fn send_telegram(&self, config: &TelegramConfig, text: &str) -> anyhow::Result<()> {
        let url = format!(
            "https://api.telegram.org/bot{}/sendMessage",
            config.bot_token
        );

        let resp = self
            .client
            .post(&url)
            .json(&serde_json::json!({
                "chat_id": config.chat_id,
                "text": text,
                "disable_web_page_preview": true
            }))
            .send()
            .await?;

        if !resp.status().is_success() {
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!("Telegram API error: {}", body);
        }

        tracing::debug!("Telegram alert sent");
        Ok(())
    }
}
