//! WASI Preview 2 (Component Model) executor
//!
//! Executes WASM components compiled with wasm32-wasip2 target.
//!
//! ## Features
//! - Component model with typed interfaces
//! - HTTP/HTTPS requests via wasi-http
//! - NEAR RPC proxy via host functions (when ExecutionContext is provided)
//! - Advanced filesystem operations
//! - Async execution
//!
//! ## Requirements
//! - wasmtime 28+
//! - WASM component format (not core module)
//! - wasi:cli/run interface

use anyhow::{Context, Result};
use std::collections::HashMap;
use tracing::debug;
use wasmtime::component::{Component, Linker};
use wasmtime::*;
use wasmtime_wasi::{DirPerms, FilePerms, ResourceTable, WasiCtx, WasiCtxBuilder, WasiView};
use wasmtime_wasi::bindings::Command;
use wasmtime_wasi_http::{WasiHttpCtx, WasiHttpView};

use crate::api_client::ResourceLimits;
use crate::outlayer_rpc::{RpcHostState, RpcProxy};

use super::ExecutionContext;

/// Host state for WASI P2 execution
///
/// Contains WASI context, HTTP context, and optionally RPC proxy state.
struct HostState {
    wasi_ctx: WasiCtx,
    wasi_http_ctx: WasiHttpCtx,
    table: ResourceTable,
    /// RPC proxy state (only present if ExecutionContext has outlayer_rpc)
    rpc_state: Option<RpcHostState>,
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

impl HostState {
    /// Get RPC host state (for host function callbacks)
    fn rpc_state_mut(&mut self) -> &mut RpcHostState {
        self.rpc_state.as_mut().expect("RPC state not initialized")
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
/// * `exec_ctx` - Execution context with optional RPC proxy
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
    exec_ctx: Option<&ExecutionContext>,
) -> Result<(Vec<u8>, u64)> {
    // Configure wasmtime engine for WASI Preview 2
    let mut config = Config::new();
    config.wasm_component_model(true); // Enable component model
    config.async_support(true); // HTTP requires async
    config.consume_fuel(true); // Instruction metering

    let engine = Engine::new(&config)?;

    // Try to load as component
    let component = Component::from_binary(&engine, wasm_bytes)
        .context("Not a valid WASI Preview 2 component")?;

    debug!("Loaded as WASI Preview 2 component");

    // Create linker with WASI and HTTP support
    let mut linker: Linker<HostState> = Linker::new(&engine);
    wasmtime_wasi::add_to_linker_async(&mut linker)?;
    wasmtime_wasi_http::add_only_http_to_linker_async(&mut linker)?;

    // Add NEAR RPC host functions if context has RPC proxy
    let rpc_state = if let Some(ctx) = exec_ctx {
        if let Some(outlayer_rpc) = &ctx.outlayer_rpc {
            debug!("Adding NEAR RPC host functions to linker");

            // Clone the proxy for use in host state
            let proxy_clone = RpcProxy::new(
                crate::config::RpcProxyConfig {
                    enabled: true,
                    rpc_url: None, // Will use the original proxy's URL
                    max_calls_per_execution: 100,
                    allow_transactions: true,
                },
                &outlayer_rpc.get_rpc_url_masked(),
            )?;

            // Add RPC host functions to linker
            // Note: This adds the near:rpc/api interface that WASM can import
            crate::outlayer_rpc::add_rpc_to_linker(&mut linker, |state: &mut HostState| {
                state.rpc_state_mut()
            })?;

            Some(RpcHostState::new(proxy_clone, ctx.runtime_handle.clone()))
        } else {
            debug!("No RPC proxy in execution context");
            None
        }
    } else {
        debug!("No execution context provided");
        None
    };

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

    // Add environment variables (from encrypted secrets)
    if let Some(env_map) = env_vars {
        for (key, value) in env_map {
            wasi_builder.env(&key, &value);
            debug!("Added env var: {}", key);
        }
    }

    // Add indicator that RPC proxy is available
    if rpc_state.is_some() {
        wasi_builder.env("NEAR_RPC_PROXY_AVAILABLE", "1");
        debug!("Added env var: NEAR_RPC_PROXY_AVAILABLE=1");
    }

    let host_state = HostState {
        wasi_ctx: wasi_builder.build(),
        wasi_http_ctx: WasiHttpCtx::new(),
        table: ResourceTable::new(),
        rpc_state,
    };

    // Create store with fuel limit
    let mut store = Store::new(&engine, host_state);
    store.set_fuel(limits.max_instructions)?;

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

    // Get fuel consumed before checking result
    let fuel_consumed = limits.max_instructions - store.get_fuel().unwrap_or(0);
    debug!("Component consumed {} instructions", fuel_consumed);

    // Log RPC call count if available
    if let Some(ref rpc_state) = store.data().rpc_state {
        let call_count = rpc_state.proxy.try_lock()
            .map(|p| p.get_call_count())
            .unwrap_or(0);
        if call_count > 0 {
            debug!("Component made {} RPC calls", call_count);
        }
    }

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
