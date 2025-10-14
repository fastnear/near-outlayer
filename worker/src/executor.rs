use anyhow::{Context, Result};
use std::collections::HashMap;
use std::time::{Duration, Instant};
use tracing::{debug, info};
use wasmtime::component::{Component, Linker};
use wasmtime::*;
use wasmtime_wasi::{DirPerms, FilePerms, ResourceTable, WasiCtx, WasiCtxBuilder, WasiView};
use wasmtime_wasi::bindings::Command;
use wasmtime_wasi_http::{WasiHttpCtx, WasiHttpView};

use crate::api_client::{ExecutionResult, ResourceLimits};

/// Host state for WASM execution with WASI support
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

/// WASM executor with wasmtime (WASI P2) and wasmi (WASI P1) support
pub struct Executor {
    /// Maximum instructions allowed per execution (default)
    _default_max_instructions: u64,
}

impl Executor {
    /// Create a new executor
    pub fn new(default_max_instructions: u64) -> Self {
        Self {
            _default_max_instructions: default_max_instructions,
        }
    }

    /// Execute WASM binary with resource limits
    ///
    /// # Arguments
    /// * `wasm_bytes` - Compiled WASM binary (component for P2, module for P1)
    /// * `input_data` - Input data to pass to the WASM via stdin
    /// * `limits` - Resource limits for execution
    /// * `env_vars` - Environment variables (from decrypted secrets)
    ///
    /// # Returns
    /// * `Ok(ExecutionResult)` - Execution completed (success or error)
    /// * `Err(_)` - Fatal execution error
    pub async fn execute(
        &self,
        wasm_bytes: &[u8],
        input_data: &[u8],
        limits: &ResourceLimits,
        env_vars: Option<HashMap<String, String>>,
    ) -> Result<ExecutionResult> {
        let start_time = Instant::now();

        info!(
            "Starting WASM execution: {} instructions, {} MB memory, {} seconds",
            limits.max_instructions, limits.max_memory_mb, limits.max_execution_seconds
        );

        // Execute with timeout
        let timeout = Duration::from_secs(limits.max_execution_seconds);
        let result = tokio::time::timeout(
            timeout,
            Self::execute_async(wasm_bytes, input_data, limits, env_vars),
        )
        .await;

        let execution_time_ms = start_time.elapsed().as_millis() as u64;

        match result {
            Ok(Ok((output, instructions))) => {
                info!(
                    "WASM execution succeeded in {} ms, consumed {} instructions",
                    execution_time_ms, instructions
                );
                Ok(ExecutionResult {
                    success: true,
                    output: Some(output),
                    error: None,
                    execution_time_ms,
                    instructions,
                })
            }
            Ok(Err(e)) => {
                info!("WASM execution failed: {}", e);
                Ok(ExecutionResult {
                    success: false,
                    output: None,
                    error: Some(e.to_string()),
                    execution_time_ms,
                    instructions: 0,
                })
            }
            Err(_) => {
                // Timeout exceeded
                Ok(ExecutionResult {
                    success: false,
                    output: None,
                    error: Some(format!(
                        "Execution timeout exceeded ({} seconds)",
                        limits.max_execution_seconds
                    )),
                    execution_time_ms,
                    instructions: 0,
                })
            }
        }
    }

    /// Async execution - tries WASI P2 (wasmtime), falls back to WASI P1 (wasmi)
    async fn execute_async(
        wasm_bytes: &[u8],
        input_data: &[u8],
        limits: &ResourceLimits,
        env_vars: Option<HashMap<String, String>>,
    ) -> Result<(Vec<u8>, u64)> {
        // Configure wasmtime engine for WASI Preview 2
        let mut config = Config::new();
        config.wasm_component_model(true); // WASI P2 components
        config.async_support(true); // HTTP requires async
        config.consume_fuel(true); // Instruction metering

        let engine = Engine::new(&config)?;

        // Try to load as WASI P2 component first
        if let Ok(component) = Component::from_binary(&engine, wasm_bytes) {
            debug!("Loading as WASI Preview 2 component");
            return Self::execute_wasi_p2(&engine, &component, input_data, limits, env_vars).await;
        }

        // Try to load as WASI P1 module with wasmtime (new binary format)
        let mut module_config = Config::new();
        module_config.async_support(true);
        module_config.consume_fuel(true);
        let module_engine = Engine::new(&module_config)?;

        if let Ok(module) = wasmtime::Module::from_binary(&module_engine, wasm_bytes) {
            debug!("Loading as WASI Preview 1 module (wasmtime)");
            return Self::execute_wasi_p1_wasmtime(&module_engine, &module, input_data, limits, env_vars).await;
        }

        // If nothing worked, return error
        anyhow::bail!("Failed to load WASM binary: not a valid WASI P2 component or WASI P1 module")
    }

    /// Execute WASI Preview 2 component with wasmtime
    async fn execute_wasi_p2(
        engine: &Engine,
        component: &Component,
        input_data: &[u8],
        limits: &ResourceLimits,
        env_vars: Option<HashMap<String, String>>,
    ) -> Result<(Vec<u8>, u64)> {
        // Create linker with WASI and WASI-HTTP support
        let mut linker = Linker::new(engine);
        wasmtime_wasi::add_to_linker_async(&mut linker)?;
        wasmtime_wasi_http::add_only_http_to_linker_async(&mut linker)?;

        // Prepare stdin with input data
        let stdin_pipe = wasmtime_wasi::pipe::MemoryInputPipe::new(input_data.to_vec());

        // Prepare stdout capture
        let stdout_pipe = wasmtime_wasi::pipe::MemoryOutputPipe::new(
            (limits.max_memory_mb as usize) * 1024 * 1024
        );

        // Build WASI context
        let mut wasi_builder = WasiCtxBuilder::new();
        wasi_builder.stdin(stdin_pipe);
        wasi_builder.stdout(stdout_pipe.clone());
        wasi_builder.stderr(wasmtime_wasi::pipe::MemoryOutputPipe::new(1024 * 1024));

        // Add preopened directory (required for WASI P2 filesystem interface)
        // WASM component needs filesystem preopens to initialize even if not using files
        // This satisfies wasi:filesystem/preopens@0.2.2 interface requirement
        wasi_builder.preopened_dir(
            "/tmp",        // host_path - temporary directory on host
            ".",           // guest_path - available as "." in WASM
            DirPerms::all(),   // full directory permissions
            FilePerms::all(),  // full file permissions
        )?;

        // Add environment variables (from encrypted secrets)
        if let Some(env_map) = env_vars {
            for (key, value) in env_map {
                wasi_builder.env(&key, &value);
                debug!("Added env var: {}", key);
            }
        }

        let host_state = HostState {
            wasi_ctx: wasi_builder.build(),
            wasi_http_ctx: WasiHttpCtx::new(),
            table: ResourceTable::new(),
        };

        // Create store with fuel limit
        let mut store = Store::new(engine, host_state);
        store.set_fuel(limits.max_instructions)?;

        // Instantiate as Command component
        debug!("Instantiating WASI command component");
        let command = Command::instantiate_async(&mut store, component, &linker)
            .await
            .context("Failed to instantiate WASM command component")?;

        // Call wasi:cli/run
        debug!("Calling wasi:cli/run");
        let result = command
            .wasi_cli_run()
            .call_run(&mut store)
            .await;

        match result {
            Ok(res) => {
                debug!("wasi:cli/run returned: {:?}", res);
                res.map_err(|_| anyhow::anyhow!("WASM program returned error exit code"))?;
            }
            Err(e) => {
                return Err(anyhow::anyhow!("Failed to call wasi:cli/run: {:?}", e));
            }
        }

        debug!("WASI command execution completed");

        // Get results
        let fuel_consumed = limits.max_instructions - store.get_fuel().unwrap_or(0);
        debug!("WASM execution consumed {} instructions", fuel_consumed);

        // Read output from stdout
        let output = stdout_pipe.contents().to_vec();

        Ok((output, fuel_consumed))
    }

    /// Execute WASI Preview 1 module with wasmtime (new binary format with _start)
    async fn execute_wasi_p1_wasmtime(
        engine: &Engine,
        module: &wasmtime::Module,
        input_data: &[u8],
        limits: &ResourceLimits,
        env_vars: Option<HashMap<String, String>>,
    ) -> Result<(Vec<u8>, u64)> {
        use wasmtime_wasi::preview1::{self, WasiP1Ctx};

        // Create linker for WASI P1
        let mut linker = wasmtime::Linker::new(engine);
        preview1::add_to_linker_async(&mut linker, |t: &mut WasiP1Ctx| t)?;

        // Prepare stdin with input data
        let stdin_pipe = wasmtime_wasi::pipe::MemoryInputPipe::new(input_data.to_vec());

        // Prepare stdout capture
        let stdout_pipe = wasmtime_wasi::pipe::MemoryOutputPipe::new(
            (limits.max_memory_mb as usize) * 1024 * 1024
        );

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
        let mut store = Store::new(engine, wasi_p1_ctx);
        store.set_fuel(limits.max_instructions)?;

        // Instantiate module
        debug!("Instantiating WASI P1 module");
        let instance = linker.instantiate_async(&mut store, module).await
            .context("Failed to instantiate WASI P1 module")?;

        // Get and call _start function (WASI entry point)
        debug!("Calling _start");
        let start = instance
            .get_typed_func::<(), ()>(&mut store, "_start")
            .context("Failed to find _start function")?;

        start.call_async(&mut store, ()).await
            .context("Failed to call _start")?;

        debug!("WASI P1 module execution completed");

        // Get results
        let fuel_consumed = limits.max_instructions - store.get_fuel().unwrap_or(0);
        debug!("WASM execution consumed {} instructions", fuel_consumed);

        // Read output from stdout
        let output = stdout_pipe.contents().to_vec();

        Ok((output, fuel_consumed))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_executor_creation() {
        let executor = Executor::new(10_000_000_000);
        assert_eq!(executor._default_max_instructions, 10_000_000_000);
    }
}
