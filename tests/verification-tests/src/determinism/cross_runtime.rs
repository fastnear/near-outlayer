//! Cross-Runtime Consistency Tests
//!
//! Verifies that wasmi (P1) and wasmtime (P2) produce identical outputs for the same input.

#[cfg(test)]
use crate::common::{build_test_wasm, execute_wasm_p1, execute_wasm_p2};
#[cfg(test)]
use anyhow::Result;

#[tokio::test]
async fn test_wasmi_wasmtime_output_consistency() -> Result<()> {
    let wasm = build_test_wasm("random-ark").await?;
    let input = r#"{"seed": 999}"#.as_bytes();
    let max_fuel = 100_000_000;

    // Execute with both runtimes
    let wasmi_result = execute_wasm_p1(&wasm, input, max_fuel).await?;
    let wasmtime_result = execute_wasm_p2(&wasm, input, max_fuel, 100).await?;

    // Outputs must match (fuel may differ due to different metering strategies)
    assert_eq!(
        wasmi_result.output, wasmtime_result.output,
        "wasmi and wasmtime produced different outputs for same input"
    );

    println!("✓ wasmi and wasmtime produce identical output");
    println!("  wasmi fuel: {}", wasmi_result.fuel_consumed);
    println!("  wasmtime fuel: {}", wasmtime_result.fuel_consumed);

    Ok(())
}

#[tokio::test]
async fn test_multiple_inputs_cross_runtime() -> Result<()> {
    let wasm = build_test_wasm("random-ark").await?;
    let max_fuel = 100_000_000;

    let test_inputs = vec![r#"{"seed": 1}"#, r#"{"seed": 100}"#, r#"{"seed": 65535}"#];

    for input_str in test_inputs {
        let input = input_str.as_bytes();

        let wasmi_result = execute_wasm_p1(&wasm, input, max_fuel).await?;
        let wasmtime_result = execute_wasm_p2(&wasm, input, max_fuel, 100).await?;

        assert_eq!(
            wasmi_result.output, wasmtime_result.output,
            "Output mismatch for input: {input_str}"
        );
    }

    println!("✓ Cross-runtime consistency verified for 3 different inputs");

    Ok(())
}
