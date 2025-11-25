//! RPC Test Ark - Test WASM for OutLayer RPC host functions
//!
//! This WASM component tests the near:rpc/api host functions provided by OutLayer worker.
//! It performs various RPC calls and outputs the results as JSON.
//!
//! For transaction tests (call/transfer): Requires env NEAR_SENDER_ID, NEAR_SENDER_PRIVATE_KEY
//! For view-only tests: No credentials required

use serde::Serialize;
use std::env;
use std::io::Write;

// Generate bindings for near:rpc/api interface
wit_bindgen::generate!({
    world: "rpc-test",
    path: "wit",
});

#[derive(Debug, Serialize)]
struct Output {
    success: bool,
    results: Vec<TestResult>,
}

#[derive(Debug, Serialize)]
struct TestResult {
    name: String,
    success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

fn main() {
    println!("=== NEAR RPC Test Suite ===\n");

    let mut results = Vec::new();
    let mut passed = 0;
    let mut failed = 0;

    // Get signer from env (for transaction tests)
    let signer_id = env::var("NEAR_SENDER_ID").unwrap_or_else(|_| "test.testnet".to_string());
    let signer_key = env::var("NEAR_SENDER_PRIVATE_KEY").unwrap_or_default();

    // Run all tests with default values
    let tests: Vec<TestResult> = vec![
        test_view_account("outlayer.testnet"),
        test_block(),
        test_gas_price(),
        test_view_call("wrap.testnet", "ft_metadata", "{}"),
        test_raw(),
        test_transfer(&signer_id, &signer_key, "outlayer.testnet", "1"),
    ];

    for test in tests {
        if test.success {
            passed += 1;
            print!("[PASS] {}", test.name);
            if let Some(ref result) = test.result {
                // Show tx_hash for transfer
                if let Some(tx_hash) = result.get("tx_hash") {
                    print!(" -> tx: {}", tx_hash.as_str().unwrap_or("?"));
                }
            }
            println!();
        } else {
            failed += 1;
            println!("[FAIL] {} - {}", test.name, test.error.as_deref().unwrap_or("unknown error"));
        }
        results.push(test);
    }

    println!("\n=== Results: {}/{} passed ===", passed, passed + failed);

    let all_success = failed == 0;

    // Also output JSON for machine parsing
    let output = Output {
        success: all_success,
        results,
    };

    println!("\nJSON output:");
    let output_json = serde_json::to_string(&output).expect("Failed to serialize output");
    print!("{}", output_json);
    std::io::stdout().flush().expect("Failed to flush stdout");
}

fn test_view_account(account_id: &str) -> TestResult {
    eprintln!("Testing view_account for: {}", account_id);

    let (result, error) = near::rpc::api::view_account(account_id);

    if !error.is_empty() {
        return TestResult {
            name: "view_account".to_string(),
            success: false,
            result: None,
            error: Some(error),
        };
    }

    // Parse the result as JSON
    match serde_json::from_str::<serde_json::Value>(&result) {
        Ok(json) => {
            // Extract key fields for validation
            let has_amount = json.get("result").and_then(|r| r.get("amount")).is_some();
            let has_block_height = json.get("result").and_then(|r| r.get("block_height")).is_some();

            if has_amount && has_block_height {
                TestResult {
                    name: "view_account".to_string(),
                    success: true,
                    result: Some(json),
                    error: None,
                }
            } else {
                TestResult {
                    name: "view_account".to_string(),
                    success: false,
                    result: Some(json),
                    error: Some("Missing expected fields (amount, block_height)".to_string()),
                }
            }
        }
        Err(e) => TestResult {
            name: "view_account".to_string(),
            success: false,
            result: None,
            error: Some(format!("Failed to parse result: {}", e)),
        },
    }
}

fn test_block() -> TestResult {
    eprintln!("Testing block (final)");

    let (result, error) = near::rpc::api::block("final");

    if !error.is_empty() {
        return TestResult {
            name: "block".to_string(),
            success: false,
            result: None,
            error: Some(error),
        };
    }

    match serde_json::from_str::<serde_json::Value>(&result) {
        Ok(json) => {
            // Check for block header
            let has_header = json.get("result").and_then(|r| r.get("header")).is_some();
            let has_chunks = json.get("result").and_then(|r| r.get("chunks")).is_some();

            if has_header && has_chunks {
                // Extract height for summary
                let height = json
                    .get("result")
                    .and_then(|r| r.get("header"))
                    .and_then(|h| h.get("height"));

                TestResult {
                    name: "block".to_string(),
                    success: true,
                    result: Some(serde_json::json!({
                        "height": height,
                        "has_header": true,
                        "has_chunks": true
                    })),
                    error: None,
                }
            } else {
                TestResult {
                    name: "block".to_string(),
                    success: false,
                    result: Some(json),
                    error: Some("Missing expected fields (header, chunks)".to_string()),
                }
            }
        }
        Err(e) => TestResult {
            name: "block".to_string(),
            success: false,
            result: None,
            error: Some(format!("Failed to parse result: {}", e)),
        },
    }
}

fn test_gas_price() -> TestResult {
    eprintln!("Testing gas_price");

    let (result, error) = near::rpc::api::gas_price();

    if !error.is_empty() {
        return TestResult {
            name: "gas_price".to_string(),
            success: false,
            result: None,
            error: Some(error),
        };
    }

    // Result should be a gas price value
    if result.is_empty() {
        return TestResult {
            name: "gas_price".to_string(),
            success: false,
            result: None,
            error: Some("Empty gas price result".to_string()),
        };
    }

    // Try to parse as number or string
    let gas_price: serde_json::Value = serde_json::from_str(&result)
        .unwrap_or_else(|_| serde_json::json!(result));

    TestResult {
        name: "gas_price".to_string(),
        success: true,
        result: Some(serde_json::json!({ "gas_price": gas_price })),
        error: None,
    }
}

fn test_view_call(contract_id: &str, method_name: &str, args_json: &str) -> TestResult {
    eprintln!(
        "Testing view call: {}.{}({})",
        contract_id, method_name, args_json
    );

    let (result, error) = near::rpc::api::view(contract_id, method_name, args_json);

    if !error.is_empty() {
        return TestResult {
            name: format!("view_call({}.{})", contract_id, method_name),
            success: false,
            result: None,
            error: Some(error),
        };
    }

    // Try to parse result as JSON
    let parsed: serde_json::Value = serde_json::from_str(&result)
        .unwrap_or_else(|_| serde_json::json!(result));

    TestResult {
        name: format!("view_call({}.{})", contract_id, method_name),
        success: true,
        result: Some(parsed),
        error: None,
    }
}

fn test_raw() -> TestResult {
    eprintln!("Testing raw RPC (status)");

    let (result, error) = near::rpc::api::raw("status", "[]");

    if !error.is_empty() {
        return TestResult {
            name: "raw(status)".to_string(),
            success: false,
            result: None,
            error: Some(error),
        };
    }

    match serde_json::from_str::<serde_json::Value>(&result) {
        Ok(json) => {
            let chain_id = json.get("result").and_then(|r| r.get("chain_id"));
            let version = json.get("result").and_then(|r| r.get("version"));

            TestResult {
                name: "raw(status)".to_string(),
                success: true,
                result: Some(serde_json::json!({
                    "chain_id": chain_id,
                    "version": version
                })),
                error: None,
            }
        }
        Err(e) => TestResult {
            name: "raw(status)".to_string(),
            success: false,
            result: None,
            error: Some(format!("Failed to parse result: {}", e)),
        },
    }
}

fn test_transfer(signer_id: &str, signer_key: &str, receiver_id: &str, amount_yocto: &str) -> TestResult {
    eprintln!("Testing transfer: {} yoctoNEAR from {} to {}", amount_yocto, signer_id, receiver_id);

    let (tx_hash, error) = near::rpc::api::transfer(signer_id, signer_key, receiver_id, amount_yocto);

    if !error.is_empty() {
        return TestResult {
            name: format!("transfer({})", receiver_id),
            success: false,
            result: None,
            error: Some(error),
        };
    }

    TestResult {
        name: format!("transfer({})", receiver_id),
        success: true,
        result: Some(serde_json::json!({ "tx_hash": tx_hash })),
        error: None,
    }
}
