//! Payment Keys with Intents - WASI module for swapping tokens to USDC for payment key top-up
//!
//! This module:
//! 1. Reads token price from oracle-ark storage
//! 2. Validates minimum value ($0.01 USDC)
//! 3. Swaps token to USDC via 1Click API
//! 4. Sends USDC to outlayer.near via ft_transfer_call with payment key nonce in msg

#[allow(dead_code, deprecated)]
mod near_tx;

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::env;
use std::io::{self, Read, Write};
use std::time::Duration;
use wasi_http_client::Client;

// ============================================================================
// Configuration
// ============================================================================

const ONECLICK_BASE_URL: &str = "https://1click.chaindefuser.com";
const INTENTS_CONTRACT: &str = "intents.near";
const OUTLAYER_CONTRACT: &str = "outlayer.near";
const USDC_CONTRACT: &str = "17208628f84f5d6ad33f0da3bbbeb27ffcb398eac501a31bd6ad2011e36133a1";
const USDC_DEFUSE_ASSET: &str = "nep141:17208628f84f5d6ad33f0da3bbbeb27ffcb398eac501a31bd6ad2011e36133a1";
const MIN_USDC_AMOUNT: u128 = 10_000; // $0.01 with 6 decimals
const ORACLE_PROJECT_UUID: &str = "p0000000000000003";

// Token whitelist embedded at compile time
const TOKENS_JSON: &str = include_str!("../tokens.json");

// ============================================================================
// Input/Output Types
// ============================================================================

#[derive(Deserialize, Debug)]
struct Input {
    owner: String,
    nonce: u32,
    token_id: String,        // e.g. "wrap.near"
    amount: String,          // token amount in minimal units
    swap_contract_id: String, // Account that holds tokens and executes swap
}

#[derive(Serialize, Debug)]
struct Output {
    success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    usdc_amount: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tx_hashes: Option<Vec<String>>,
    logs: Vec<String>,
}

// ============================================================================
// Token Whitelist Types
// ============================================================================

#[derive(Deserialize, Debug)]
struct TokenConfig {
    oracle_key: String,
    decimals: u32,
    defuse_asset_id: String,
}

// ============================================================================
// Oracle Price Types (from oracle-ark)
// ============================================================================

#[derive(Deserialize, Debug)]
struct StoredPrice {
    price: f64,
    timestamp: u64,
    #[allow(dead_code)]
    sources: Vec<SourceInfo>,
    #[allow(dead_code)]
    aggregation_method: String,
}

#[derive(Deserialize, Debug)]
struct SourceInfo {
    #[allow(dead_code)]
    name: String,
    #[allow(dead_code)]
    price: f64,
    #[allow(dead_code)]
    timestamp: Option<u64>,
}

// ============================================================================
// 1Click API Types (matching coordinator's backend/mod.rs)
// ============================================================================

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct OneClickQuoteRequest {
    dry: bool,
    swap_type: String,
    slippage_tolerance: u32,
    origin_asset: String,
    deposit_type: String,
    destination_asset: String,
    amount: String,
    refund_to: String,
    refund_type: String,
    recipient: String,
    recipient_type: String,
    deadline: String,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct OneClickQuoteResponse {
    #[allow(dead_code)]
    correlation_id: String,
    quote: OneClickQuote,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
struct OneClickQuote {
    deposit_address: String,
    #[allow(dead_code)]
    amount_in: String,
    amount_out: String,
    #[allow(dead_code)]
    min_amount_out: String,
    #[allow(dead_code)]
    deadline: String,
    #[serde(default)]
    #[allow(dead_code)]
    time_estimate: Option<u64>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct OneClickSubmitDeposit {
    tx_hash: String,
    deposit_address: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    near_sender_account: Option<String>,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct OneClickStatusResponse {
    status: String,
    #[serde(default)]
    swap_details: Option<OneClickSwapDetails>,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct OneClickSwapDetails {
    #[serde(default)]
    amount_out: Option<String>,
    #[serde(default)]
    #[allow(dead_code)]
    intent_hashes: Vec<String>,
    #[serde(default)]
    #[allow(dead_code)]
    near_tx_hashes: Vec<String>,
}

// ============================================================================
// Main Entry Point
// ============================================================================

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut input_string = String::new();
    io::stdin().read_to_string(&mut input_string)?;

    eprintln!("Input: {}", input_string);

    let input: Input = serde_json::from_str(&input_string)?;

    eprintln!(
        "Processing payment key top-up: owner={}, nonce={}, token={}, amount={}",
        input.owner, input.nonce, input.token_id, input.amount
    );

    let mut logs = Vec::new();
    let output = match execute_topup(&input, &mut logs) {
        Ok(result) => result,
        Err(e) => {
            eprintln!("Top-up execution failed: {:?}", e);
            logs.push(format!("FAILED: {}", e));
            Output {
                success: false,
                usdc_amount: None,
                error: Some(format!("{}", e)),
                tx_hashes: None,
                logs,
            }
        }
    };

    print!("{}", serde_json::to_string(&output)?);
    io::stdout().flush()?;

    Ok(())
}

fn execute_topup(input: &Input, logs: &mut Vec<String>) -> Result<Output, Box<dyn std::error::Error>> {
    let mut tx_hashes = Vec::new();

    // Step 1: Load token whitelist and validate token
    let tokens: HashMap<String, TokenConfig> = serde_json::from_str(TOKENS_JSON)?;

    let token_config = tokens.get(&input.token_id).ok_or_else(|| {
        format!(
            "Token {} is not in whitelist. Allowed: {:?}",
            input.token_id,
            tokens.keys().collect::<Vec<_>>()
        )
    })?;

    logs.push(format!(
        "1. Token validated: {} (oracle_key={}, decimals={})",
        input.token_id, token_config.oracle_key, token_config.decimals
    ));

    // Step 2: Get token price from oracle storage
    let token_price = get_token_price(&token_config.oracle_key)?;
    logs.push(format!("2. Oracle price: ${:.4}", token_price));

    // Step 3: Calculate expected USDC and validate minimum
    let amount: u128 = input.amount.parse()?;
    let token_in_decimals = amount as f64 / 10f64.powi(token_config.decimals as i32);
    let expected_usdc = token_in_decimals * token_price;
    let expected_usdc_minimal = (expected_usdc * 1_000_000.0) as u128; // 6 decimals for USDC

    if expected_usdc_minimal < MIN_USDC_AMOUNT {
        return Err(format!(
            "Deposit too small: ${:.4} USDC expected, minimum is $0.01",
            expected_usdc
        )
        .into());
    }

    // Apply 2% slippage tolerance for min_amount_out
    let min_usdc_out = (expected_usdc_minimal as f64 * 0.98) as u128;

    logs.push(format!(
        "3. Expected: {} {} = ~${:.2} USDC (min_out={})",
        token_in_decimals, input.token_id, expected_usdc, min_usdc_out
    ));

    // Step 4: Get swap contract credentials
    let swap_contract_id = &input.swap_contract_id;
    let swap_contract_private_key = env::var("SWAP_CONTRACT_PRIVATE_KEY")
        .map_err(|_| "SWAP_CONTRACT_PRIVATE_KEY not found in environment")?;
    let oneclick_jwt = env::var("ONECLICK_JWT")
        .map_err(|_| "ONECLICK_JWT not found in environment")?;
    let rpc_url = env::var("NEAR_RPC_URL")
        .unwrap_or_else(|_| "https://rpc.mainnet.fastnear.com".to_string());

    logs.push(format!("4. Swap contract: {}", swap_contract_id));

    // Step 5: Get 1Click quote
    let quote_resp = get_oneclick_quote(
        &oneclick_jwt,
        &token_config.defuse_asset_id,
        USDC_DEFUSE_ASSET,
        &input.amount,
        swap_contract_id,
    )?;

    let deposit_address = &quote_resp.quote.deposit_address;
    let quote_amount_out: u128 = quote_resp.quote.amount_out.parse()?;

    if quote_amount_out < min_usdc_out {
        return Err(format!(
            "Quote too low: {} USDC < {} minimum",
            quote_amount_out, min_usdc_out
        )
        .into());
    }

    logs.push(format!(
        "5. 1Click quote: {} {} -> {} USDC (deposit_address={})",
        input.amount, input.token_id, quote_resp.quote.amount_out, deposit_address
    ));

    // Step 6: Deposit tokens to intents.near via ft_transfer_call
    let token_contract = &input.token_id;

    let deposit_tx = near_tx::ft_transfer_call(
        &rpc_url,
        swap_contract_id,
        &swap_contract_private_key,
        token_contract,
        INTENTS_CONTRACT,
        &input.amount,
        "",
    )?;
    tx_hashes.push(deposit_tx.clone());
    logs.push(format!("6. Deposit to intents.near: tx={}", deposit_tx));

    // Step 7: mt_transfer on intents.near — move tokens to 1Click deposit address
    let mt_args = serde_json::json!({
        "receiver_id": deposit_address,
        "token_id": token_config.defuse_asset_id,
        "amount": input.amount,
    });

    let mt_tx = near_tx::call(
        &rpc_url,
        swap_contract_id,
        &swap_contract_private_key,
        INTENTS_CONTRACT,
        "mt_transfer",
        &mt_args.to_string(),
        100_000_000_000_000, // 100 TGas
        1,                   // 1 yoctoNEAR
    )?;
    tx_hashes.push(mt_tx.clone());
    logs.push(format!("7. mt_transfer to deposit address: tx={}", mt_tx));

    // Step 8: Notify 1Click about the deposit (best-effort, non-fatal)
    if let Err(e) = submit_oneclick_deposit(
        &oneclick_jwt,
        &mt_tx,
        deposit_address,
        Some(swap_contract_id),
    ) {
        eprintln!("Warning: Failed to submit deposit to 1Click (non-fatal): {}", e);
    }
    logs.push("8. 1Click deposit notification sent".to_string());

    // Step 9: Poll 1Click status until terminal state
    let status_resp = poll_oneclick_status(&oneclick_jwt, deposit_address)?;

    match status_resp.status.as_str() {
        "SUCCESS" => {
            logs.push("9. 1Click swap SETTLED".to_string());
        }
        "FAILED" => {
            logs.push("9. 1Click swap FAILED".to_string());
            return Err("1Click swap failed".into());
        }
        "REFUNDED" => {
            logs.push("9. 1Click swap REFUNDED".to_string());
            return Err("1Click swap was refunded — tokens returned to wallet".into());
        }
        other => {
            logs.push(format!("9. 1Click swap still processing: {}", other));
            return Err(format!("1Click swap timed out (status: {})", other).into());
        }
    }

    // Step 10: Send USDC from swap_contract to outlayer.near with payment key msg
    // 1Click delivered USDC to swap_contract_id. Now ft_transfer_call to outlayer.near.
    let actual_usdc = status_resp.swap_details
        .as_ref()
        .and_then(|d| d.amount_out.clone())
        .unwrap_or_else(|| quote_resp.quote.amount_out.clone());

    let withdrawal_msg = serde_json::json!({
        "action": "top_up_payment_key",
        "nonce": input.nonce,
        "owner": input.owner
    })
    .to_string();

    logs.push(format!(
        "10. ft_transfer_call: {} USDC -> {} msg={}",
        actual_usdc, OUTLAYER_CONTRACT, withdrawal_msg
    ));

    let topup_tx = near_tx::ft_transfer_call(
        &rpc_url,
        swap_contract_id,
        &swap_contract_private_key,
        USDC_CONTRACT,
        OUTLAYER_CONTRACT,
        &actual_usdc,
        &withdrawal_msg,
    )?;
    tx_hashes.push(topup_tx.clone());

    logs.push(format!(
        "11. Done: owner={} nonce={} usdc={} tx={}",
        input.owner, input.nonce, actual_usdc, topup_tx
    ));

    Ok(Output {
        success: true,
        usdc_amount: Some(actual_usdc),
        error: None,
        tx_hashes: Some(tx_hashes),
        logs: logs.clone(),
    })
}

// ============================================================================
// Oracle Functions
// ============================================================================

fn get_token_price(oracle_key: &str) -> Result<f64, Box<dyn std::error::Error>> {
    let storage_key = format!("price:{}", oracle_key);

    match outlayer::storage::get_worker_from_project(&storage_key, Some(ORACLE_PROJECT_UUID)) {
        Ok(Some(data)) => {
            let stored: StoredPrice = serde_json::from_slice(&data)?;

            // Check freshness (max 5 minutes old)
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)?
                .as_secs();

            if now.saturating_sub(stored.timestamp) > 300 {
                eprintln!(
                    "Warning: Price is {} seconds old",
                    now.saturating_sub(stored.timestamp)
                );
            }

            Ok(stored.price)
        }
        Ok(None) => Err(format!("Price not found in oracle storage: {}", storage_key).into()),
        Err(e) => Err(format!("Failed to read oracle storage: {}", e).into()),
    }
}

// ============================================================================
// 1Click API Functions
// ============================================================================

fn get_oneclick_quote(
    jwt: &str,
    token_in: &str,
    token_out: &str,
    amount_in: &str,
    swap_contract_id: &str,
) -> Result<OneClickQuoteResponse, Box<dyn std::error::Error>> {
    let deadline = get_deadline_iso8601(300);

    let request = OneClickQuoteRequest {
        dry: false,
        swap_type: "EXACT_INPUT".to_string(),
        slippage_tolerance: 100, // 1%
        origin_asset: token_in.to_string(),
        deposit_type: "INTENTS".to_string(),
        destination_asset: token_out.to_string(),
        amount: amount_in.to_string(),
        refund_to: swap_contract_id.to_string(),
        refund_type: "INTENTS".to_string(),
        recipient: swap_contract_id.to_string(),
        recipient_type: "DESTINATION_CHAIN".to_string(),
        deadline,
    };

    let url = format!("{}/v0/quote", ONECLICK_BASE_URL);
    let body = serde_json::to_string(&request)?;

    const MAX_RETRIES: u32 = 3;
    let mut last_error = String::from("no attempts made");

    for attempt in 1..=MAX_RETRIES {
        eprintln!("1Click quote attempt {}/{}", attempt, MAX_RETRIES);

        match Client::new()
            .post(&url)
            .header("Content-Type", "application/json")
            .header("Authorization", format!("Bearer {}", jwt).as_str())
            .connect_timeout(Duration::from_secs(15))
            .body(body.as_bytes())
            .send()
        {
            Ok(response) => {
                let status = response.status();
                match response.body() {
                    Ok(resp_body) => {
                        let resp_str = String::from_utf8_lossy(&resp_body);
                        if status / 100 != 2 {
                            last_error = format!("HTTP {}: {}", status, &resp_str[..resp_str.len().min(500)]);
                            eprintln!("Attempt {}: {}", attempt, last_error);
                        } else {
                            match serde_json::from_slice::<OneClickQuoteResponse>(&resp_body) {
                                Ok(quote_resp) => return Ok(quote_resp),
                                Err(e) => {
                                    last_error = format!("JSON parse error: {} body={}", e, &resp_str[..resp_str.len().min(500)]);
                                    eprintln!("Attempt {}: {}", attempt, last_error);
                                }
                            }
                        }
                    }
                    Err(e) => {
                        last_error = format!("Failed to read response body: {}", e);
                        eprintln!("Attempt {}: {}", attempt, last_error);
                    }
                }
            }
            Err(e) => {
                last_error = format!("HTTP request failed: {}", e);
                eprintln!("Attempt {}: {}", attempt, last_error);
            }
        }

        if attempt < MAX_RETRIES {
            std::thread::sleep(Duration::from_secs(2));
        }
    }

    Err(format!("1Click quote failed after {} retries. Last error: {}", MAX_RETRIES, last_error).into())
}

fn submit_oneclick_deposit(
    jwt: &str,
    tx_hash: &str,
    deposit_address: &str,
    near_sender_account: Option<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
    let request = OneClickSubmitDeposit {
        tx_hash: tx_hash.to_string(),
        deposit_address: deposit_address.to_string(),
        near_sender_account: near_sender_account.map(|s| s.to_string()),
    };

    let url = format!("{}/v0/deposit/submit", ONECLICK_BASE_URL);

    let response = Client::new()
        .post(&url)
        .header("Content-Type", "application/json")
        .header("Authorization", format!("Bearer {}", jwt).as_str())
        .connect_timeout(Duration::from_secs(10))
        .body(serde_json::to_string(&request)?.as_bytes())
        .send()?;

    let status = response.status();
    if status / 100 != 2 {
        let body = response.body().unwrap_or_default();
        let body_str = String::from_utf8_lossy(&body);
        return Err(format!("1Click deposit/submit returned HTTP {}: {}", status, body_str).into());
    }

    eprintln!("1Click deposit submitted: tx={}, deposit_addr={}", tx_hash, deposit_address);
    Ok(())
}

fn poll_oneclick_status(
    jwt: &str,
    deposit_address: &str,
) -> Result<OneClickStatusResponse, Box<dyn std::error::Error>> {
    const POLL_INTERVAL_MS: u64 = 2_000;
    const POLL_TIMEOUT_MS: u64 = 120_000;
    let max_attempts = POLL_TIMEOUT_MS / POLL_INTERVAL_MS;

    let url = format!("{}/v0/status?depositAddress={}", ONECLICK_BASE_URL, deposit_address);

    for attempt in 0..max_attempts {
        if attempt > 0 {
            std::thread::sleep(Duration::from_millis(POLL_INTERVAL_MS));
        }

        match Client::new()
            .get(&url)
            .header("Authorization", format!("Bearer {}", jwt).as_str())
            .connect_timeout(Duration::from_secs(10))
            .send()
        {
            Ok(response) => {
                if response.status() / 100 != 2 {
                    eprintln!("1Click status poll attempt {}: HTTP {}", attempt + 1, response.status());
                    continue;
                }

                match response.body() {
                    Ok(body) => {
                        match serde_json::from_slice::<OneClickStatusResponse>(&body) {
                            Ok(status_resp) => {
                                eprintln!("1Click status (attempt {}): {}", attempt + 1, status_resp.status);

                                match status_resp.status.as_str() {
                                    "SUCCESS" | "FAILED" | "REFUNDED" => return Ok(status_resp),
                                    _ => {} // Continue polling
                                }
                            }
                            Err(e) => {
                                eprintln!("1Click status parse error (attempt {}): {}", attempt + 1, e);
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("1Click status body read error (attempt {}): {}", attempt + 1, e);
                    }
                }
            }
            Err(e) => {
                eprintln!("1Click status request error (attempt {}): {}", attempt + 1, e);
            }
        }
    }

    // Timeout — return processing status
    Ok(OneClickStatusResponse {
        status: "PROCESSING".to_string(),
        swap_details: None,
    })
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Format a deadline as ISO 8601 UTC string, `seconds_from_now` seconds in the future.
/// Uses Howard Hinnant's civil_from_days algorithm for correct leap year handling.
fn get_deadline_iso8601(seconds_from_now: u64) -> String {
    use std::time::{SystemTime, UNIX_EPOCH};

    let total_secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
        + seconds_from_now;

    let days = (total_secs / 86400) as i64;
    let time_of_day = total_secs % 86400;

    // Howard Hinnant's algorithm (basis of C++20 chrono / Rust chrono)
    let z = days + 719468;
    let era = (if z >= 0 { z } else { z - 146096 }) / 146097;
    let doe = (z - era * 146097) as u32;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };

    let hours = time_of_day / 3600;
    let minutes = (time_of_day % 3600) / 60;
    let seconds = time_of_day % 60;

    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}.000Z",
        y, m, d, hours, minutes, seconds
    )
}
