//! Payment Keys with Intents - WASI module for swapping tokens to USDC for payment key top-up
//!
//! This module:
//! 1. Reads token price from oracle-ark storage
//! 2. Validates minimum value ($1 USDC)
//! 3. Swaps token to USDC via NEAR Intents
//! 4. Withdraws USDC to outlayer.near with payment key nonce in msg

mod crypto;
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

const INTENTS_API_URL: &str = "https://solver-relay-v2.chaindefuser.com/rpc";
const INTENTS_CONTRACT: &str = "intents.near";
const OUTLAYER_CONTRACT: &str = "outlayer.near";
const USDC_CONTRACT: &str = "17208628f84f5d6ad33f0da3bbbeb27ffcb398eac501a31bd6ad2011e36133a1";
const USDC_DEFUSE_ASSET: &str = "nep141:17208628f84f5d6ad33f0da3bbbeb27ffcb398eac501a31bd6ad2011e36133a1";
const MIN_USDC_AMOUNT: u128 = 1_000_000; // $1.00 with 6 decimals

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
    usdc_amount: Option<String>,
    error_message: Option<String>,
    tx_hashes: Option<Vec<String>>,
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
// NEAR Intents API Types
// ============================================================================

#[derive(Serialize)]
struct JsonRpcRequest<T> {
    id: u32,
    jsonrpc: String,
    method: String,
    params: Vec<T>,
}

#[derive(Serialize)]
struct QuoteParams {
    defuse_asset_identifier_in: String,
    defuse_asset_identifier_out: String,
    exact_amount_in: String,
}

#[derive(Deserialize)]
struct JsonRpcResponse<T> {
    result: Option<T>,
    error: Option<JsonRpcError>,
}

#[derive(Deserialize, Debug)]
struct JsonRpcError {
    message: String,
}

#[derive(Deserialize, Debug, Clone)]
struct Quote {
    amount_in: String,
    amount_out: String,
    expiration_time: String,
    quote_hash: String,
}

#[derive(Serialize)]
struct PublishIntentParams {
    signed_data: SignedData,
    quote_hashes: Option<Vec<String>>,
}

#[derive(Serialize)]
struct SignedData {
    payload: Payload,
    standard: String,
    signature: String,
    public_key: String,
}

#[derive(Serialize)]
struct Payload {
    message: String,
    nonce: String,
    recipient: String,
}

#[derive(Deserialize, Debug)]
struct PublishIntentResult {
    status: String,
    intent_hash: Option<String>,
}

#[derive(Serialize)]
struct IntentMessage {
    signer_id: String,
    deadline: String,
    intents: Vec<IntentAction>,
}

#[derive(Serialize)]
#[serde(tag = "intent")]
enum IntentAction {
    #[serde(rename = "token_diff")]
    TokenDiff { diff: serde_json::Value },
    #[serde(rename = "ft_withdraw")]
    FtWithdraw {
        token: String,
        receiver_id: String,
        amount: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        msg: Option<String>,
    },
}

#[derive(Serialize)]
struct GetStatusParams {
    intent_hash: String,
}

#[derive(Deserialize)]
struct GetStatusResult {
    status: String,
}

// ============================================================================
// Main Entry Point
// ============================================================================

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Read input from stdin
    let mut input_string = String::new();
    io::stdin().read_to_string(&mut input_string)?;

    eprintln!("Input: {}", input_string);

    // Parse input JSON
    let input: Input = serde_json::from_str(&input_string)?;

    eprintln!(
        "Processing payment key top-up: owner={}, nonce={}, token={}, amount={}",
        input.owner, input.nonce, input.token_id, input.amount
    );

    // Execute the swap flow
    let output = match execute_topup(&input) {
        Ok(result) => result,
        Err(e) => {
            eprintln!("Top-up execution failed: {:?}", e);
            Output {
                success: false,
                usdc_amount: None,
                error_message: Some(format!("{}", e)),
                tx_hashes: None,
            }
        }
    };

    // Output result
    print!("{}", serde_json::to_string(&output)?);
    io::stdout().flush()?;

    Ok(())
}

fn execute_topup(input: &Input) -> Result<Output, Box<dyn std::error::Error>> {
    let mut tx_hashes = Vec::new();

    // Step 1: Load token whitelist and validate token
    eprintln!("Step 1: Validating token whitelist...");
    let tokens: HashMap<String, TokenConfig> = serde_json::from_str(TOKENS_JSON)?;

    let token_config = tokens.get(&input.token_id).ok_or_else(|| {
        format!(
            "Token {} is not in whitelist. Allowed tokens: {:?}",
            input.token_id,
            tokens.keys().collect::<Vec<_>>()
        )
    })?;

    eprintln!("Token {} found in whitelist: oracle_key={}, decimals={}",
        input.token_id, token_config.oracle_key, token_config.decimals);

    // Step 2: Get token price from oracle storage
    eprintln!("Step 2: Fetching token price from oracle storage...");
    let token_price = get_token_price(&token_config.oracle_key)?;
    eprintln!("Token price: ${:.4}", token_price);

    // Step 3: Calculate expected USDC and validate minimum
    eprintln!("Step 3: Validating minimum value...");
    let amount: u128 = input.amount.parse()?;
    let token_in_decimals = amount as f64 / 10f64.powi(token_config.decimals as i32);
    let expected_usdc = token_in_decimals * token_price;
    let expected_usdc_minimal = (expected_usdc * 1_000_000.0) as u128; // 6 decimals for USDC

    eprintln!(
        "{} {} = ${:.2} USDC (expected)",
        token_in_decimals, input.token_id, expected_usdc
    );

    if expected_usdc_minimal < MIN_USDC_AMOUNT {
        return Err(format!(
            "Deposit too small: ${:.2} USDC expected, minimum is $1.00",
            expected_usdc
        )
        .into());
    }

    // Apply 2% slippage tolerance for min_amount_out
    let min_usdc_out = (expected_usdc_minimal as f64 * 0.98) as u128;

    // Step 4: Get swap contract credentials
    eprintln!("Step 4: Getting swap contract credentials...");
    let swap_contract_id = &input.swap_contract_id;
    let swap_contract_private_key = env::var("SWAP_CONTRACT_PRIVATE_KEY")
        .map_err(|_| "SWAP_CONTRACT_PRIVATE_KEY not found in environment")?;
    let rpc_url = env::var("NEAR_RPC_URL")
        .unwrap_or_else(|_| "https://rpc.mainnet.near.org".to_string());

    eprintln!("Swap contract: {}", swap_contract_id);

    // Step 5: Get quote from Intents API
    eprintln!("Step 5: Getting quote from NEAR Intents API...");
    let quote = get_quote(
        &token_config.defuse_asset_id,
        USDC_DEFUSE_ASSET,
        &input.amount,
    )?;

    let quote_amount_out: u128 = quote.amount_out.parse()?;
    if quote_amount_out < min_usdc_out {
        return Err(format!(
            "Quote too low: {} USDC < {} minimum",
            quote_amount_out, min_usdc_out
        )
        .into());
    }

    eprintln!(
        "Quote received: {} {} -> {} USDC",
        quote.amount_in, input.token_id, quote.amount_out
    );

    // Step 6: Deposit tokens to intents.near
    eprintln!("Step 6: Depositing tokens to intents.near...");
    let token_contract = &input.token_id;

    let deposit_tx = near_tx::ft_transfer_call(
        &rpc_url,
        &swap_contract_id,
        &swap_contract_private_key,
        token_contract,
        INTENTS_CONTRACT,
        &input.amount,
        "",
    )?;
    tx_hashes.push(deposit_tx.clone());
    eprintln!("Deposit TX: {}", deposit_tx);

    // Step 7: Publish swap intent
    eprintln!("Step 7: Publishing swap intent...");
    let intent_hash = publish_swap_intent(
        &swap_contract_id,
        &swap_contract_private_key,
        &token_config.defuse_asset_id,
        USDC_DEFUSE_ASSET,
        &quote,
    )?;
    eprintln!("Intent hash: {}", intent_hash);

    // Step 8: Wait for settlement
    eprintln!("Step 8: Waiting for intent settlement...");
    let settled = wait_for_settlement(&intent_hash)?;

    if !settled {
        return Err("Intent failed to settle within timeout".into());
    }

    eprintln!("Intent settled!");

    // Step 9: Withdraw USDC to outlayer.near with payment key msg
    eprintln!("Step 9: Withdrawing USDC to outlayer.near...");

    // Build the msg for ft_on_transfer callback
    let withdrawal_msg = serde_json::json!({
        "action": "top_up_payment_key",
        "nonce": input.nonce
    })
    .to_string();

    let withdraw_success = withdraw_tokens_with_msg(
        &swap_contract_id,
        &swap_contract_private_key,
        USDC_CONTRACT,
        OUTLAYER_CONTRACT,
        &quote.amount_out,
        &withdrawal_msg,
    )?;

    if !withdraw_success {
        return Err("Failed to withdraw USDC to outlayer.near".into());
    }

    eprintln!(
        "Successfully topped up payment key {}:{} with {} USDC",
        input.owner, input.nonce, quote.amount_out
    );

    Ok(Output {
        success: true,
        usdc_amount: Some(quote.amount_out.clone()),
        error_message: None,
        tx_hashes: Some(tx_hashes),
    })
}

// ============================================================================
// Oracle Functions
// ============================================================================

fn get_token_price(oracle_key: &str) -> Result<f64, Box<dyn std::error::Error>> {
    let storage_key = format!("price:{}", oracle_key);

    match outlayer::storage::get_worker(&storage_key) {
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
// NEAR Intents API Functions
// ============================================================================

fn get_quote(
    token_in: &str,
    token_out: &str,
    amount_in: &str,
) -> Result<Quote, Box<dyn std::error::Error>> {
    let request = JsonRpcRequest {
        id: 1,
        jsonrpc: "2.0".to_string(),
        method: "quote".to_string(),
        params: vec![QuoteParams {
            defuse_asset_identifier_in: token_in.to_string(),
            defuse_asset_identifier_out: token_out.to_string(),
            exact_amount_in: amount_in.to_string(),
        }],
    };

    const MAX_RETRIES: u32 = 3;

    for attempt in 1..=MAX_RETRIES {
        eprintln!("Quote API attempt {}/{}", attempt, MAX_RETRIES);

        match Client::new()
            .post(INTENTS_API_URL)
            .header("Content-Type", "application/json")
            .connect_timeout(Duration::from_secs(10))
            .body(serde_json::to_string(&request)?.as_bytes())
            .send()
        {
            Ok(response) => {
                if response.status() == 200 {
                    if let Ok(body) = response.body() {
                        if let Ok(json_response) =
                            serde_json::from_slice::<JsonRpcResponse<Vec<Quote>>>(&body)
                        {
                            if let Some(quotes) = json_response.result {
                                if let Some(best_quote) = quotes
                                    .into_iter()
                                    .max_by_key(|q| q.amount_out.parse::<u128>().unwrap_or(0))
                                {
                                    return Ok(best_quote);
                                }
                            }
                        }
                    }
                }
            }
            Err(e) => {
                eprintln!("Attempt {} failed: {}", attempt, e);
            }
        }

        if attempt < MAX_RETRIES {
            std::thread::sleep(Duration::from_millis(1000));
        }
    }

    Err("Quote API failed after retries".into())
}

fn publish_swap_intent(
    signer_id: &str,
    private_key: &str,
    token_in: &str,
    token_out: &str,
    quote: &Quote,
) -> Result<String, Box<dyn std::error::Error>> {
    let diff = serde_json::json!({
        token_in: format!("-{}", quote.amount_in),
        token_out: quote.amount_out.clone()
    });

    let intent_message = IntentMessage {
        signer_id: signer_id.to_string(),
        deadline: quote.expiration_time.clone(),
        intents: vec![IntentAction::TokenDiff { diff }],
    };

    let message_str = serde_json::to_string(&intent_message)?;
    let message_str = message_str.replace("\":", "\": ");

    let nonce = generate_nonce();
    let signature = sign_intent(&message_str, &nonce, private_key)?;

    let params = PublishIntentParams {
        signed_data: SignedData {
            payload: Payload {
                message: message_str,
                nonce: nonce.clone(),
                recipient: INTENTS_CONTRACT.to_string(),
            },
            standard: "nep413".to_string(),
            signature: format!("ed25519:{}", signature),
            public_key: derive_public_key(private_key)?,
        },
        quote_hashes: Some(vec![quote.quote_hash.clone()]),
    };

    let request = JsonRpcRequest {
        id: 1,
        jsonrpc: "2.0".to_string(),
        method: "publish_intent".to_string(),
        params: vec![params],
    };

    let response = Client::new()
        .post(INTENTS_API_URL)
        .header("Content-Type", "application/json")
        .connect_timeout(Duration::from_secs(10))
        .body(serde_json::to_string(&request)?.as_bytes())
        .send()?;

    if response.status() != 200 {
        return Err(format!("Publish intent API returned status {}", response.status()).into());
    }

    let body = response.body()?;
    let json_response: JsonRpcResponse<PublishIntentResult> = serde_json::from_slice(&body)?;

    if let Some(error) = json_response.error {
        return Err(format!("Publish intent API error: {}", error.message).into());
    }

    let result = json_response.result.ok_or("No result from publish_intent")?;

    if result.status != "OK" {
        return Err(format!("Intent publish failed with status: {}", result.status).into());
    }

    result.intent_hash.ok_or("No intent_hash returned".into())
}

fn wait_for_settlement(intent_hash: &str) -> Result<bool, Box<dyn std::error::Error>> {
    const MAX_ATTEMPTS: u32 = 120; // 30 seconds

    for attempt in 0..MAX_ATTEMPTS {
        if attempt > 0 {
            std::thread::sleep(Duration::from_millis(250));
        }

        let request = JsonRpcRequest {
            id: 1,
            jsonrpc: "2.0".to_string(),
            method: "get_status".to_string(),
            params: vec![GetStatusParams {
                intent_hash: intent_hash.to_string(),
            }],
        };

        let response = Client::new()
            .post(INTENTS_API_URL)
            .header("Content-Type", "application/json")
            .connect_timeout(Duration::from_secs(5))
            .body(serde_json::to_string(&request)?.as_bytes())
            .send()?;

        if response.status() != 200 {
            continue;
        }

        let body = response.body()?;
        let json_response: JsonRpcResponse<GetStatusResult> = serde_json::from_slice(&body)?;

        if let Some(result) = json_response.result {
            eprintln!("Intent status (attempt {}): {}", attempt + 1, result.status);

            match result.status.as_str() {
                "SETTLED" => return Ok(true),
                "NOT_FOUND_OR_NOT_VALID_ANYMORE" | "NOT_FOUND_OR_NOT_VALID" | "FAILED" => {
                    return Ok(false);
                }
                _ => {} // Continue polling
            }
        }
    }

    Ok(false) // Timeout
}

fn withdraw_tokens_with_msg(
    signer_id: &str,
    private_key: &str,
    token: &str,
    receiver_id: &str,
    amount: &str,
    msg: &str,
) -> Result<bool, Box<dyn std::error::Error>> {
    // Build withdraw intent with msg
    let intent_message = IntentMessage {
        signer_id: signer_id.to_string(),
        deadline: get_deadline_180s(),
        intents: vec![IntentAction::FtWithdraw {
            token: token.to_string(),
            receiver_id: receiver_id.to_string(),
            amount: amount.to_string(),
            msg: Some(msg.to_string()),
        }],
    };

    let message_str = serde_json::to_string(&intent_message)?;
    let message_str = message_str.replace("\":", "\": ");

    eprintln!("Withdraw message: {}", message_str);

    let nonce = generate_nonce();
    let signature = sign_intent(&message_str, &nonce, private_key)?;

    let params = PublishIntentParams {
        signed_data: SignedData {
            payload: Payload {
                message: message_str,
                nonce: nonce.clone(),
                recipient: INTENTS_CONTRACT.to_string(),
            },
            standard: "nep413".to_string(),
            signature: format!("ed25519:{}", signature),
            public_key: derive_public_key(private_key)?,
        },
        quote_hashes: None,
    };

    let request = JsonRpcRequest {
        id: 1,
        jsonrpc: "2.0".to_string(),
        method: "publish_intent".to_string(),
        params: vec![params],
    };

    let response = Client::new()
        .post(INTENTS_API_URL)
        .header("Content-Type", "application/json")
        .connect_timeout(Duration::from_secs(10))
        .body(serde_json::to_string(&request)?.as_bytes())
        .send()?;

    if response.status() != 200 {
        return Err(format!("Withdraw API returned status {}", response.status()).into());
    }

    let body = response.body()?;
    let json_response: JsonRpcResponse<PublishIntentResult> = serde_json::from_slice(&body)?;

    if let Some(error) = json_response.error {
        return Err(format!("Withdraw API error: {}", error.message).into());
    }

    let result = json_response.result.ok_or("No result from withdraw")?;
    let intent_hash = result.intent_hash.ok_or("No intent_hash for withdraw")?;

    // Wait for withdrawal settlement
    wait_for_settlement(&intent_hash)
}

// ============================================================================
// Helper Functions
// ============================================================================

fn generate_nonce() -> String {
    use sha2::{Digest, Sha256};
    use std::time::{SystemTime, UNIX_EPOCH};

    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos()
        .to_string();

    let mut hasher = Sha256::new();
    hasher.update(timestamp.as_bytes());
    let result = hasher.finalize();

    base64::encode(result)
}

fn get_deadline_180s() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};

    let now_plus_180 = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
        + 180;

    let total_seconds = now_plus_180;
    let seconds = total_seconds % 60;
    let total_minutes = total_seconds / 60;
    let minutes = total_minutes % 60;
    let total_hours = total_minutes / 60;
    let hours = total_hours % 24;
    let total_days = total_hours / 24;

    let year = 1970 + (total_days / 365);
    let day_of_year = total_days % 365;
    let month = (day_of_year / 30) + 1;
    let day = (day_of_year % 30) + 1;

    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}.000Z",
        year, month, day, hours, minutes, seconds
    )
}

fn sign_intent(
    message: &str,
    nonce: &str,
    private_key: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    let key_base58 = if private_key.starts_with("ed25519:") {
        &private_key[8..]
    } else {
        private_key
    };

    let (signature, _public_key) =
        crypto::sign_nep413_intent(message, nonce, INTENTS_CONTRACT, key_base58)?;

    Ok(signature)
}

fn derive_public_key(private_key: &str) -> Result<String, Box<dyn std::error::Error>> {
    let key_base58 = if private_key.starts_with("ed25519:") {
        &private_key[8..]
    } else {
        private_key
    };

    let dummy_nonce = base64::encode(&[0u8; 32]);
    let (_signature, public_key) =
        crypto::sign_nep413_intent("{}", &dummy_nonce, INTENTS_CONTRACT, key_base58)?;

    Ok(format!("ed25519:{}", public_key))
}
