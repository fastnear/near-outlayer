//! WASI Preview 1 executor
//!
//! Executes WASM modules compiled with wasm32-wasip1 or wasm32-wasi targets.
//!
//! ## Features
//! - Standard WASI functions (stdio, random, environment)
//! - Binary format with `main()` entry point
//! - Fuel metering for instruction counting
//!
//! ## Requirements
//! - wasmtime 28+ with WASI P1 compatibility layer
//! - Core WASM module (not component)
//! - `_start` export (created by Rust from `fn main()`)

use anyhow::{Context, Result};
use std::collections::HashMap;
use tracing::debug;
use wasmtime::*;
use wasmtime_wasi::preview1::{self, WasiP1Ctx};
use wasmtime_wasi::WasiCtxBuilder;

use crate::api_client::ResourceLimits;

/// Execute WASI Preview 1 module
///
/// # Arguments
/// * `wasm_bytes` - WASM module binary
/// * `input_data` - JSON input via stdin
/// * `limits` - Resource limits (memory, instructions, time)
/// * `env_vars` - Environment variables (from encrypted secrets)
///
/// # Returns
/// * `Ok((output, fuel_consumed))` - Execution succeeded
/// * `Err(_)` - Not a valid P1 module or execution failed
pub async fn execute(
    wasm_bytes: &[u8],
    input_data: &[u8],
    limits: &ResourceLimits,
    env_vars: Option<HashMap<String, String>>,
) -> Result<(Vec<u8>, u64)> {
    // Configure wasmtime engine for WASI Preview 1
    let mut config = Config::new();
    config.async_support(true);
    config.consume_fuel(true);

    let engine = Engine::new(&config)?;

    // Try to load as module
    let module = wasmtime::Module::from_binary(&engine, wasm_bytes)
        .context("Not a valid WASI Preview 1 module")?;

    debug!("Loaded as WASI Preview 1 module (wasmtime)");

    // Create linker for WASI P1
    let mut linker = wasmtime::Linker::new(&engine);
    preview1::add_to_linker_async(&mut linker, |t: &mut WasiP1Ctx| t)?;

    // Prepare stdin/stdout pipes
    let stdin_pipe = wasmtime_wasi::pipe::MemoryInputPipe::new(input_data.to_vec());
    let stdout_pipe =
        wasmtime_wasi::pipe::MemoryOutputPipe::new((limits.max_memory_mb as usize) * 1024 * 1024);

    // Build WASI P1 context
    let mut wasi_builder = WasiCtxBuilder::new();
    wasi_builder.stdin(stdin_pipe);
    wasi_builder.stdout(stdout_pipe.clone());
    wasi_builder.stderr(wasmtime_wasi::pipe::MemoryOutputPipe::new(1024 * 1024));

    // Add environment variables (from encrypted secrets)
    if let Some(env_map) = env_vars {
        for (key, value) in env_map {
            wasi_builder.env(&key, &value);
            debug!("Added env var: {}", key);
        }
    }

    let wasi_p1_ctx = wasi_builder.build_p1();

    // Create store with fuel limit
    let mut store = Store::new(&engine, wasi_p1_ctx);
    store.set_fuel(limits.max_instructions)?;

    // Instantiate module
    debug!("Instantiating WASI P1 module");
    let instance = linker
        .instantiate_async(&mut store, &module)
        .await
        .context("Failed to instantiate WASI P1 module")?;

    // Get and call _start function (WASI entry point from main())
    debug!("Calling _start");
    let start = instance
        .get_typed_func::<(), ()>(&mut store, "_start")
        .context(
            "Failed to find _start function. \
             Make sure you're using [[bin]] format with fn main(), not [lib] with cdylib",
        )?;

    start
        .call_async(&mut store, ())
        .await
        .context("WASI P1 module execution failed")?;

    debug!("WASI P1 module execution completed");

    // Get results
    let fuel_consumed = limits.max_instructions - store.get_fuel().unwrap_or(0);
    debug!("WASM execution consumed {} instructions", fuel_consumed);

    let output = stdout_pipe.contents().to_vec();

    Ok((output, fuel_consumed))
}
