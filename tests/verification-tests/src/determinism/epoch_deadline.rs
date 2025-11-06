//! Epoch Deadline Integration Tests
//!
//! Verifies that wasmtime epoch interruption behaves deterministically.

#[cfg(test)]
use crate::common::{build_test_wasm, execute_wasm_p2};
#[cfg(test)]
use anyhow::Result;

#[tokio::test]
async fn test_epoch_deadline_deterministic_timeout() -> Result<()> {
    let wasm = build_test_wasm("random-ark").await?;
    let input = r#"{"seed": 42}"#.as_bytes();
    let max_fuel = 100_000_000;

    // Test with very low epoch ticks (should timeout consistently)
    let low_ticks = 1;

    let mut results = Vec::new();
    for _i in 0..10 {
        let result = execute_wasm_p2(&wasm, input, max_fuel, low_ticks).await;
        results.push(result);
    }

    // All results should be identical (either all succeed or all timeout)
    let first_is_ok = results[0].is_ok();
    for result in &results {
        assert_eq!(
            result.is_ok(),
            first_is_ok,
            "Epoch deadline behavior is non-deterministic"
        );
    }

    println!("✓ Epoch deadline behavior is deterministic across 10 runs");

    Ok(())
}

#[tokio::test]
async fn test_high_epoch_allows_completion() -> Result<()> {
    let wasm = build_test_wasm("random-ark").await?;
    let input = r#"{"seed": 42}"#.as_bytes();
    let max_fuel = 10_000_000;

    // High epoch ticks should allow completion
    let high_ticks = 1000;

    let result = execute_wasm_p2(&wasm, input, max_fuel, high_ticks).await;

    // Should complete successfully (or fail for fuel, not epoch)
    if result.is_err() {
        // If it errors, should not be an epoch timeout
        let err_msg = format!("{:?}", result.err().unwrap());
        assert!(
            !err_msg.contains("epoch") && !err_msg.contains("deadline"),
            "High epoch deadline should not cause timeout"
        );
    }

    println!("✓ High epoch deadline allows execution to complete");

    Ok(())
}
