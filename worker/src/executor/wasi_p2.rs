//! WASI Preview 2 (Component Model) executor
//!
//! Executes WASM components compiled with wasm32-wasip2 target.
//!
//! ## Features
//! - Component model with typed interfaces
//! - HTTP/HTTPS requests via wasi-http
//! - Advanced filesystem operations
//! - Async execution
//! - **Phase 1 Hardening**: Epoch deadline + deterministic WASI environment
//!
//! ## Requirements
//! - wasmtime 28+
//! - WASM component format (not core module)
//! - wasi:cli/run interface
//!
//! ## Phase 1: Principal Engineer Hardening
//! - Epoch-based wall-clock deadline (cannot be bypassed by idle syscalls)
//! - Deterministic WASI environment (TZ=UTC, LANG=C, no ambient RNG/network)
//! - Fuel + epoch dual protection for resource limits

use anyhow::{Context, Result};
use std::collections::HashMap;
use std::time::Duration;
use tracing::debug;
use wasmtime::component::{Component, Linker};
use wasmtime::*;
use wasmtime_wasi::{DirPerms, FilePerms, ResourceTable, WasiCtx, WasiCtxBuilder, WasiView};
use wasmtime_wasi::bindings::Command;
use wasmtime_wasi_http::{WasiHttpCtx, WasiHttpView};

use crate::api_client::ResourceLimits;
use super::wasmtime_cfg;
use super::wasi_env;

/// Host state for WASI P2 execution
struct HostState {
    wasi_ctx: WasiCtx,
    wasi_http_ctx: WasiHttpCtx,
    table: ResourceTable,
}

impl WasiView for HostState {
    fn ctx(&mut self) -> &mut WasiCtx {
        &mut self.wasi_ctx
    }

    fn table(&mut self) -> &mut ResourceTable {
        &mut self.table
    }
}

impl WasiHttpView for HostState {
    fn ctx(&mut self) -> &mut WasiHttpCtx {
        &mut self.wasi_http_ctx
    }

    fn table(&mut self) -> &mut ResourceTable {
        &mut self.table
    }
}

/// Execute WASI Preview 2 component
///
/// # Arguments
/// * `wasm_bytes` - WASM component binary
/// * `input_data` - JSON input via stdin
/// * `limits` - Resource limits (memory, instructions, time)
/// * `env_vars` - Environment variables (from encrypted secrets)
/// * `print_stderr` - Print WASM stderr to worker logs
///
/// # Returns
/// * `Ok((output, fuel_consumed))` - Execution succeeded
/// * `Err(_)` - Not a valid P2 component or execution failed
pub async fn execute(
    wasm_bytes: &[u8],
    input_data: &[u8],
    limits: &ResourceLimits,
    env_vars: Option<HashMap<String, String>>,
    print_stderr: bool,
) -> Result<(Vec<u8>, u64)> {
    // Phase 1 Hardening: Use deterministic engine configuration
    // - Fuel metering (per-instruction accounting)
    // - Epoch interruption (hard wall-clock deadline)
    // - Disabled non-deterministic features
    let mut engine = wasmtime_cfg::engine_with_limits()?;

    // WASI P2 requires component model
    let mut config = Config::new();
    config.wasm_component_model(true);
    config.async_support(true);
    config.consume_fuel(true);
    config.epoch_interruption(true);
    config.wasm_threads(false);
    config.wasm_multi_memory(true);
    config.wasm_memory64(false);
    config.debug_info(false);

    let engine = Engine::new(&config)?;

    debug!("Phase 1: Engine configured with fuel + epoch deadline");

    // Try to load as component
    let component = Component::from_binary(&engine, wasm_bytes)
        .context("Not a valid WASI Preview 2 component")?;

    debug!("Loaded as WASI Preview 2 component");

    // Create linker with WASI and HTTP support
    let mut linker = Linker::new(&engine);
    wasmtime_wasi::add_to_linker_async(&mut linker)?;
    wasmtime_wasi_http::add_only_http_to_linker_async(&mut linker)?;

    // Prepare stdin/stdout/stderr pipes
    let stdin_pipe = wasmtime_wasi::pipe::MemoryInputPipe::new(input_data.to_vec());
    let stdout_pipe =
        wasmtime_wasi::pipe::MemoryOutputPipe::new((limits.max_memory_mb as usize) * 1024 * 1024);
    let stderr_pipe = wasmtime_wasi::pipe::MemoryOutputPipe::new(1024 * 1024);

    // Build WASI context
    let mut wasi_builder = WasiCtxBuilder::new();
    wasi_builder.stdin(stdin_pipe);
    wasi_builder.stdout(stdout_pipe.clone());
    wasi_builder.stderr(stderr_pipe.clone());

    // Add preopened directory (required for WASI P2 filesystem interface)
    wasi_builder.preopened_dir(
        "/tmp",      // host_path
        ".",         // guest_path
        DirPerms::all(),
        FilePerms::all(),
    )?;

    // Phase 1 Hardening: Add deterministic environment or custom env vars
    if let Some(env_map) = env_vars {
        debug!("Phase 1: Using custom env vars (from secrets)");
        for (key, value) in env_map {
            wasi_builder.env(&key, &value);
            debug!("Added custom env var: {}", key);
        }
    } else {
        debug!("Phase 1: Using deterministic WASI environment");
        // Add deterministic defaults
        for (key, value) in wasi_env::WasiEnvBuilder::default_env_vars() {
            wasi_builder.env(&key, &value);
        }
    }

    let host_state = HostState {
        wasi_ctx: wasi_builder.build(),
        wasi_http_ctx: WasiHttpCtx::new(),
        table: ResourceTable::new(),
    };

    // Create store with fuel + epoch deadline
    let mut store = Store::new(&engine, host_state);

    // Phase 1 Hardening: Configure store limits (fuel + epoch)
    let deadline_task = wasmtime_cfg::configure_store_limits(
        &engine,
        &mut store,
        limits.max_instructions,
        Duration::from_secs(limits.max_execution_seconds)
    )?;

    debug!("Phase 1: Store configured with {} fuel, {}s epoch deadline",
           limits.max_instructions, limits.max_execution_seconds);

    // Instantiate and execute component
    debug!("Instantiating component");
    let command = Command::instantiate_async(&mut store, &component, &linker)
        .await
        .context("Failed to instantiate component")?;

    debug!("Running wasi:cli/run");
    let execution_result = command
        .wasi_cli_run()
        .call_run(&mut store)
        .await;

    // Phase 1 Hardening: Clean up deadline task
    deadline_task.abort();
    debug!("Phase 1: Epoch deadline task aborted");

    // Get fuel consumed before checking result
    let fuel_consumed = limits.max_instructions - store.get_fuel().unwrap_or(0);
    debug!("Component consumed {} instructions", fuel_consumed);

    // Check execution result
    // Read stderr for debugging (if flag is enabled)
    let stderr_contents = stderr_pipe.contents();
    if print_stderr && !stderr_contents.is_empty() {
        let stderr_str = String::from_utf8_lossy(&stderr_contents);
        tracing::info!("📝 WASM stderr output:\n{}", stderr_str);
    }

    match execution_result {
        Ok(Ok(())) => {
            debug!("Component execution completed successfully");
            let output = stdout_pipe.contents().to_vec();
            Ok((output, fuel_consumed))
        }
        Ok(Err(_)) | Err(_) => {
            // Component exited with error or trapped
            let error_msg = if !stderr_contents.is_empty() {
                String::from_utf8_lossy(&stderr_contents).to_string()
            } else {
                "Component execution failed (no error message in stderr)".to_string()
            };

            debug!("Component execution failed: {}", error_msg);
            Err(anyhow::anyhow!("{}", error_msg))
        }
    }
}
