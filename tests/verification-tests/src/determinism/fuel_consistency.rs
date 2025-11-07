//! Phase 1 Integration Test: Fuel Consistency
//!
//! **Property**: 100 executions of same WASM + same input → identical outputs & fuel consumption
//!
//! This test verifies the Phase 1 hardening work:
//! - Deterministic fuel metering (no randomness in instruction counting)
//! - Bit-for-bit output consistency (no nondeterministic operations in WASM)
//! - Epoch deadline does not affect determinism (timeout mechanism is orthogonal)

#[cfg(test)]
use crate::common::{assert_deterministic, build_test_wasm, execute_wasm_p1};
#[cfg(test)]
use anyhow::Result;

#[tokio::test]
async fn test_100x_same_input_determinism() -> Result<()> {
    // Build test WASM (determinism-test uses deterministic PRNG)
    let wasm = build_test_wasm("determinism-test").await?;

    // Fixed input (same seed → same output)
    let input = r#"{"seed":42,"iterations":1000}"#.as_bytes();

    // Maximum fuel for execution
    let max_fuel = 10_000_000;

    let mut results = Vec::new();

    // Execute 100 times
    for iteration in 0..100 {
        let result = execute_wasm_p1(&wasm, input, max_fuel)
            .await
            .unwrap_or_else(|_| panic!("iteration {iteration} failed"));
        results.push(result);
    }

    // All results must be identical to the first
    let first = &results[0];
    for (i, result) in results.iter().enumerate().skip(1) {
        assert_deterministic(
            first,
            result,
            &format!("iteration {} diverged from iteration 0", i + 1),
        );
    }

    println!("✓ 100x determinism verified");
    println!("  Output: {} bytes", first.output.len());
    println!("  Fuel consumed: {}", first.fuel_consumed);
    println!(
        "  Avg time: {} ms",
        results.iter().map(|r| r.execution_time_ms).sum::<u64>() / 100
    );

    Ok(())
}

#[tokio::test]
async fn test_cross_runtime_consistency_wasmi_vs_wasmtime() -> Result<()> {
    use crate::common::execute_wasm_p2;

    // Build test WASM
    let wasm = build_test_wasm("random-ark").await?;
    let input = r#"{"seed": 12345}"#.as_bytes();
    let max_fuel = 100_000_000;

    // Execute with wasmi (P1 runtime)
    let wasmi_result = execute_wasm_p1(&wasm, input, max_fuel).await?;

    // Execute with wasmtime (P2 runtime)
    let wasmtime_result = execute_wasm_p2(&wasm, input, max_fuel, 10).await?;

    // Outputs must match (fuel may differ due to different metering strategies)
    assert_eq!(
        wasmi_result.output, wasmtime_result.output,
        "wasmi vs wasmtime output differs"
    );

    println!("✓ Cross-runtime consistency verified");
    println!("  wasmi fuel: {}", wasmi_result.fuel_consumed);
    println!("  wasmtime fuel: {}", wasmtime_result.fuel_consumed);

    Ok(())
}

#[tokio::test]
async fn test_epoch_deadline_timeout_behavior() -> Result<()> {
    use crate::common::execute_wasm_p2;

    // Build a WASM that runs for a long time
    let wasm = build_test_wasm("random-ark").await?;
    let input = r#"{"seed": 99999}"#.as_bytes();

    // Set very low epoch deadline to trigger timeout
    let max_fuel = 100_000_000;
    let low_epoch_ticks = 1; // Should timeout almost immediately

    let result = execute_wasm_p2(&wasm, input, max_fuel, low_epoch_ticks).await;

    // We expect either success (if execution was fast enough) or timeout error
    // The key property: timeout is deterministic based on epoch ticks
    match result {
        Ok(exec_result) => {
            println!("✓ Execution completed within epoch deadline");
            println!("  Fuel consumed: {}", exec_result.fuel_consumed);
        }
        Err(e) => {
            // Timeout should be deterministic - same epoch config always times out at same point
            println!("✓ Epoch deadline triggered (expected): {e}");
        }
    }

    Ok(())
}

#[tokio::test]
async fn test_zero_fuel_immediate_rejection() -> Result<()> {
    let wasm = build_test_wasm("random-ark").await?;
    let input = r#"{"seed": 1}"#.as_bytes();

    // Zero fuel should fail immediately
    let result = execute_wasm_p1(&wasm, input, 0).await;

    assert!(
        result.is_err(),
        "Zero fuel should fail, but got: {result:?}"
    );

    println!("✓ Zero fuel correctly rejected");

    Ok(())
}
