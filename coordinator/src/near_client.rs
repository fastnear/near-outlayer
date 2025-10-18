use anyhow::{Context, Result};
use reqwest::Client;
use serde_json::json;
use tracing::{info, warn};

use crate::models::PricingConfig;

/// Fetch pricing configuration from NEAR contract
pub async fn fetch_pricing_from_contract(
    rpc_url: &str,
    contract_id: &str,
) -> Result<PricingConfig> {
    info!("📡 Fetching pricing from contract: {}", contract_id);

    let client = Client::new();

    // Call get_pricing view method
    let pricing_response = client
        .post(rpc_url)
        .json(&json!({
            "jsonrpc": "2.0",
            "id": "dontcare",
            "method": "query",
            "params": {
                "request_type": "call_function",
                "finality": "final",
                "account_id": contract_id,
                "method_name": "get_pricing",
                "args_base64": ""
            }
        }))
        .send()
        .await
        .context("Failed to call get_pricing")?;

    let pricing_json: serde_json::Value = pricing_response
        .json()
        .await
        .context("Failed to parse get_pricing response")?;

    let pricing_result = pricing_json["result"]["result"]
        .as_array()
        .context("Invalid get_pricing response format")?;

    // Parse pricing tuple: (base_fee, per_instruction_fee, per_ms_fee, per_compile_ms_fee)
    let pricing_str = String::from_utf8(
        pricing_result
            .iter()
            .map(|v| v.as_u64().unwrap() as u8)
            .collect(),
    )
    .context("Failed to decode pricing result")?;

    let pricing: (String, String, String, String) = serde_json::from_str(&pricing_str)
        .context("Failed to parse pricing JSON")?;

    // Call get_max_limits view method
    let limits_response = client
        .post(rpc_url)
        .json(&json!({
            "jsonrpc": "2.0",
            "id": "dontcare",
            "method": "query",
            "params": {
                "request_type": "call_function",
                "finality": "final",
                "account_id": contract_id,
                "method_name": "get_max_limits",
                "args_base64": ""
            }
        }))
        .send()
        .await
        .context("Failed to call get_max_limits")?;

    let limits_json: serde_json::Value = limits_response
        .json()
        .await
        .context("Failed to parse get_max_limits response")?;

    let limits_result = limits_json["result"]["result"]
        .as_array()
        .context("Invalid get_max_limits response format")?;

    let limits_str = String::from_utf8(
        limits_result
            .iter()
            .map(|v| v.as_u64().unwrap() as u8)
            .collect(),
    )
    .context("Failed to decode limits result")?;

    let limits: (u64, u64, u64) = serde_json::from_str(&limits_str)
        .context("Failed to parse limits JSON")?;

    let config = PricingConfig {
        base_fee: pricing.0,
        per_instruction_fee: pricing.1,
        per_ms_fee: pricing.2,
        per_compile_ms_fee: pricing.3,
        max_compilation_seconds: limits.2,
        max_instructions: limits.0,
        max_execution_seconds: limits.1,
    };

    info!(
        "✅ Fetched pricing: base={} per_inst={} per_ms={} per_compile_ms={} max_compile_sec={} max_inst={} max_exec_sec={}",
        config.base_fee,
        config.per_instruction_fee,
        config.per_ms_fee,
        config.per_compile_ms_fee,
        config.max_compilation_seconds,
        config.max_instructions,
        config.max_execution_seconds
    );

    Ok(config)
}

/// Get default pricing (fallback if contract fetch fails)
pub fn get_default_pricing() -> PricingConfig {
    warn!("⚠️ Using default pricing (failed to fetch from contract)");
    PricingConfig {
        base_fee: "10000000000000000000000".to_string(),       // 0.01 NEAR
        per_instruction_fee: "1000000000000000".to_string(),   // 0.000001 NEAR per million instructions
        per_ms_fee: "1000000000000000000".to_string(),         // 0.001 NEAR per ms
        per_compile_ms_fee: "2000000000000000000".to_string(), // 0.002 NEAR per ms (compilation)
        max_compilation_seconds: 300,                           // 5 minutes
        max_instructions: 100_000_000_000,                      // 100B instructions
        max_execution_seconds: 60,                              // 60 seconds
    }
}
