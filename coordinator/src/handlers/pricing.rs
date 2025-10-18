use axum::{extract::State, http::StatusCode, Json};
use std::time::{Duration, SystemTime};
use tracing::{info, warn};

use crate::{models::PricingConfig, near_client, AppState};

/// Refresh pricing from contract (rate-limited to once per hour)
pub async fn refresh_pricing(
    State(state): State<AppState>,
) -> Result<Json<PricingConfig>, StatusCode> {
    const MIN_UPDATE_INTERVAL: Duration = Duration::from_secs(3600); // 1 hour (60 minutes)

    // Check last update time
    let last_update = *state.pricing_updated_at.read().await;
    let elapsed = SystemTime::now()
        .duration_since(last_update)
        .unwrap_or(Duration::from_secs(0));

    if elapsed < MIN_UPDATE_INTERVAL {
        let remaining_secs = MIN_UPDATE_INTERVAL.saturating_sub(elapsed).as_secs();
        let remaining_mins = remaining_secs / 60;

        warn!(
            "⚠️ Rate limit: Pricing was updated {} minutes ago. Next update allowed in {} minutes.",
            elapsed.as_secs() / 60,
            remaining_mins
        );

        // Return current pricing without updating
        let current_pricing = state.pricing.read().await.clone();
        return Ok(Json(current_pricing));
    }

    // Fetch new pricing
    info!("📡 Refreshing pricing from NEAR contract...");
    match near_client::fetch_pricing_from_contract(
        &state.config.near_rpc_url,
        &state.config.contract_id,
    )
    .await
    {
        Ok(new_pricing) => {
            info!(
                "✅ Pricing updated: base={} per_compile_ms={} max_compile_sec={}",
                new_pricing.base_fee,
                new_pricing.per_compile_ms_fee,
                new_pricing.max_compilation_seconds
            );

            // Update state
            *state.pricing.write().await = new_pricing.clone();
            *state.pricing_updated_at.write().await = SystemTime::now();

            Ok(Json(new_pricing))
        }
        Err(e) => {
            tracing::error!("❌ Failed to refresh pricing: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// Get current pricing (readonly, no update)
pub async fn get_pricing(State(state): State<AppState>) -> Json<PricingConfig> {
    let pricing = state.pricing.read().await.clone();
    Json(pricing)
}
