use anyhow::{Context, Result};
use reqwest::Client;
use serde::de::DeserializeOwned;
use serde_json::json;
use tracing::{info, warn};

use crate::models::PricingConfig;

/// Call a view method on NEAR contract
async fn call_view<T: DeserializeOwned>(
    client: &Client,
    rpc_url: &str,
    contract_id: &str,
    method: &str,
    args: Option<&serde_json::Value>,
) -> Result<T> {
    let args_base64 = match args {
        Some(a) => base64::Engine::encode(
            &base64::engine::general_purpose::STANDARD,
            a.to_string().as_bytes(),
        ),
        None => String::new(),
    };

    let response = client
        .post(rpc_url)
        .json(&json!({
            "jsonrpc": "2.0",
            "id": "dontcare",
            "method": "query",
            "params": {
                "request_type": "call_function",
                "finality": "final",
                "account_id": contract_id,
                "method_name": method,
                "args_base64": args_base64
            }
        }))
        .send()
        .await
        .with_context(|| format!("Failed to call {}", method))?;

    let json: serde_json::Value = response
        .json()
        .await
        .with_context(|| format!("Failed to parse {} response", method))?;

    if let Some(error) = json.get("error") {
        anyhow::bail!("RPC error calling {}: {:?}", method, error);
    }

    let result = json["result"]["result"]
        .as_array()
        .with_context(|| format!("Invalid {} response format", method))?;

    let result_str = String::from_utf8(
        result.iter().map(|v| v.as_u64().unwrap() as u8).collect(),
    )
    .with_context(|| format!("Failed to decode {} result", method))?;

    serde_json::from_str(&result_str)
        .with_context(|| format!("Failed to parse {} JSON: {}", method, result_str))
}

/// Fetch pricing configuration from NEAR contract
pub async fn fetch_pricing_from_contract(
    rpc_url: &str,
    contract_id: &str,
) -> Result<PricingConfig> {
    info!("📡 Fetching pricing from contract: {}", contract_id);

    let client = Client::new();

    // Call get_pricing view method
    let pricing_array: Vec<String> = call_view(&client, rpc_url, contract_id, "get_pricing", None).await?;

    if pricing_array.len() != 4 {
        anyhow::bail!("Expected 4 pricing values, got {}", pricing_array.len());
    }

    let pricing = (
        pricing_array[0].clone(),
        pricing_array[1].clone(),
        pricing_array[2].clone(),
        pricing_array[3].clone(),
    );

    // Call get_max_limits view method
    let limits: (u64, u64, u64) = call_view(&client, rpc_url, contract_id, "get_max_limits", None).await?;

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

/// Project info from contract
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ProjectInfo {
    pub uuid: String,
    pub owner: String,
    pub name: String,
    pub project_id: String,
    pub active_version: String,
}

/// Fetch project info from NEAR contract by project_id
pub async fn fetch_project_from_contract(
    rpc_url: &str,
    contract_id: &str,
    project_id: &str,
) -> Result<Option<ProjectInfo>> {
    let client = Client::new();
    let args = json!({ "project_id": project_id });
    call_view(&client, rpc_url, contract_id, "get_project", Some(&args)).await
}
