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

/// Response from get_pricing_full contract view method
#[derive(Debug, Clone, serde::Deserialize)]
struct PricingFullResponse {
    // NEAR pricing
    pub base_fee: String,
    pub per_million_instructions_fee: String,
    pub per_ms_fee: String,
    pub per_compile_ms_fee: String,
    // USD pricing
    pub base_fee_usd: String,
    pub per_million_instructions_fee_usd: String,
    pub per_sec_fee_usd: String,
    pub per_compile_ms_fee_usd: String,
}

/// Fetch pricing configuration from NEAR contract
pub async fn fetch_pricing_from_contract(
    rpc_url: &str,
    contract_id: &str,
) -> Result<PricingConfig> {
    info!("📡 Fetching pricing from contract: {}", contract_id);

    let client = Client::new();

    // Call get_pricing_full view method (includes USD pricing)
    let pricing: PricingFullResponse = call_view(&client, rpc_url, contract_id, "get_pricing_full", None).await?;

    // Call get_max_limits view method
    let limits: (u64, u64, u64) = call_view(&client, rpc_url, contract_id, "get_max_limits", None).await?;

    let config = PricingConfig {
        // NEAR pricing
        base_fee: pricing.base_fee,
        per_instruction_fee: pricing.per_million_instructions_fee,
        per_ms_fee: pricing.per_ms_fee,
        per_compile_ms_fee: pricing.per_compile_ms_fee,
        // USD pricing
        base_fee_usd: pricing.base_fee_usd,
        per_instruction_fee_usd: pricing.per_million_instructions_fee_usd,
        per_sec_fee_usd: pricing.per_sec_fee_usd,
        per_compile_ms_fee_usd: pricing.per_compile_ms_fee_usd,
        // Limits
        max_compilation_seconds: limits.2,
        max_instructions: limits.0,
        max_execution_seconds: limits.1,
    };

    info!(
        "✅ Fetched pricing: base={} per_inst={} per_ms={} per_compile_ms={} base_usd={} per_inst_usd={} per_sec_usd={} per_compile_ms_usd={} max_compile_sec={} max_inst={} max_exec_sec={}",
        config.base_fee,
        config.per_instruction_fee,
        config.per_ms_fee,
        config.per_compile_ms_fee,
        config.base_fee_usd,
        config.per_instruction_fee_usd,
        config.per_sec_fee_usd,
        config.per_compile_ms_fee_usd,
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
        // NEAR pricing
        base_fee: "10000000000000000000000".to_string(),       // 0.01 NEAR
        per_instruction_fee: "1000000000000000".to_string(),   // 0.000001 NEAR per million instructions
        per_ms_fee: "1000000000000000000".to_string(),         // 0.001 NEAR per ms
        per_compile_ms_fee: "2000000000000000000".to_string(), // 0.002 NEAR per ms (compilation)
        // USD pricing (in minimal token units, e.g. 1 = 0.000001 USDT)
        base_fee_usd: "1000".to_string(),                      // $0.001
        per_instruction_fee_usd: "1".to_string(),              // $0.000001 per million instructions
        per_sec_fee_usd: "1".to_string(),                       // $0.000001 per sec (execution)
        per_compile_ms_fee_usd: "10".to_string(),              // $0.00001 per ms (compilation)
        // Limits
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

/// Full project info including code source (for HTTPS API)
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ProjectFullInfo {
    pub uuid: String,
    pub owner: String,
    pub name: String,
    pub project_id: String,
    pub active_version: String,
    #[serde(default)]
    pub repo: Option<String>,
    #[serde(default)]
    pub commit_hash: Option<String>,
    #[serde(default)]
    pub build_target: Option<String>,
}

/// Fetch full project info from NEAR contract
/// Returns project with active version's code source
pub async fn fetch_project_full_from_contract(
    rpc_url: &str,
    contract_id: &str,
    project_id: &str,
) -> Result<Option<ProjectFullInfo>> {
    let client = Client::new();
    let args = json!({ "project_id": project_id });

    // First get basic project info
    let project: Option<ProjectInfo> = call_view(&client, rpc_url, contract_id, "get_project", Some(&args)).await?;

    match project {
        Some(p) => {
            // Now get the active version's code source
            let version_args = json!({
                "project_id": project_id,
                "version": p.active_version
            });

            let code_source: Option<CodeSourceInfo> = call_view(
                &client, rpc_url, contract_id, "get_project_version", Some(&version_args)
            ).await.unwrap_or(None);

            Ok(Some(ProjectFullInfo {
                uuid: p.uuid,
                owner: p.owner,
                name: p.name,
                project_id: p.project_id,
                active_version: p.active_version,
                repo: code_source.as_ref().and_then(|cs| cs.repo.clone()),
                commit_hash: code_source.as_ref().and_then(|cs| cs.commit.clone()),
                build_target: code_source.as_ref().and_then(|cs| cs.build_target.clone()),
            }))
        }
        None => Ok(None),
    }
}

/// Code source info from project version
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CodeSourceInfo {
    pub repo: Option<String>,
    pub commit: Option<String>,
    pub build_target: Option<String>,
}

