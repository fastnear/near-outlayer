mod api_client;
mod compiler;
mod config;
mod event_monitor;
mod executor;
mod near_client;

use anyhow::{Context, Result};
use tracing::{error, info, warn};

use api_client::{ApiClient, CodeSource, ExecutionResult, Task};
use compiler::Compiler;
use config::Config;
use event_monitor::EventMonitor;
use executor::Executor;
use near_client::NearClient;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "offchainvm_worker=info".into()),
        )
        .init();

    info!("OffchainVM Worker starting...");

    // Load configuration
    let config = Config::from_env().context("Failed to load configuration")?;
    config.validate().context("Invalid configuration")?;

    info!("Worker ID: {}", config.worker_id);
    info!("Coordinator API: {}", config.api_base_url);
    info!("NEAR RPC: {}", config.near_rpc_url);
    info!("Contract ID: {}", config.offchainvm_contract_id);
    info!("Event monitor enabled: {}", config.enable_event_monitor);

    // Initialize API client
    let api_client = ApiClient::new(config.api_base_url.clone(), config.api_auth_token.clone())
        .context("Failed to create API client")?;

    // Initialize compiler
    let compiler = Compiler::new(api_client.clone(), config.clone())
        .context("Failed to create compiler")?;

    // Initialize executor
    let executor = Executor::new(config.default_max_instructions);

    // Initialize NEAR client
    let near_client = NearClient::new(
        config.near_rpc_url.clone(),
        config.operator_signer.clone(),
        config.offchainvm_contract_id.clone(),
    )
    .context("Failed to create NEAR client")?;

    // Start event monitor if enabled
    if config.enable_event_monitor {
        let event_api_client = api_client.clone();
        let neardata_url = config.neardata_api_url.clone();
        let fastnear_url = config.fastnear_api_url.clone();
        let contract_id = config.offchainvm_contract_id.clone();
        let start_block = config.start_block_height;
        let scan_interval = config.scan_interval_seconds;

        tokio::spawn(async move {
            info!("Starting event monitor...");
            match EventMonitor::new(
                event_api_client,
                neardata_url,
                fastnear_url,
                contract_id,
                start_block,
                scan_interval,
            )
            .await
            {
                Ok(mut monitor) => {
                    if let Err(e) = monitor.start_monitoring().await {
                        error!("Event monitor failed: {}", e);
                    }
                }
                Err(e) => {
                    error!("Failed to create event monitor: {}", e);
                }
            }
        });
    }

    // Main worker loop
    info!("Starting worker loop...");
    loop {
        match worker_iteration(
            &api_client,
            &compiler,
            &executor,
            &near_client,
            &config,
        )
        .await
        {
            Ok(processed) => {
                if !processed {
                    // No task available, short sleep before next poll
                    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
                }
            }
            Err(e) => {
                error!("Worker iteration failed: {}", e);
                // Sleep before retry to avoid tight error loop
                tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
            }
        }
    }
}

/// Single iteration of the worker loop
///
/// Returns Ok(true) if a task was processed, Ok(false) if no task available
async fn worker_iteration(
    api_client: &ApiClient,
    compiler: &Compiler,
    executor: &Executor,
    near_client: &NearClient,
    config: &Config,
) -> Result<bool> {
    // Poll for a task (with long-polling)
    let task = api_client
        .poll_task(config.poll_timeout_seconds)
        .await
        .context("Failed to poll for task")?;

    let Some(task) = task else {
        // No task available
        return Ok(false);
    };

    info!("Received task: {:?}", task);

    // Process task based on type
    match task {
        Task::Compile {
            request_id,
            data_id,
            code_source,
            resource_limits,
            input_data,
        } => {
            handle_compile_task(
                api_client,
                compiler,
                executor,
                near_client,
                request_id,
                data_id,
                code_source,
                resource_limits,
                input_data,
            )
            .await?;
        }
        Task::Execute {
            request_id,
            data_id,
            wasm_checksum,
            resource_limits,
        } => {
            handle_execute_task(
                api_client,
                executor,
                near_client,
                request_id,
                data_id,
                wasm_checksum,
                resource_limits,
            )
            .await?;
        }
    }

    Ok(true)
}

/// Handle a compilation task - now also executes and submits result
async fn handle_compile_task(
    api_client: &ApiClient,
    compiler: &Compiler,
    executor: &Executor,
    near_client: &NearClient,
    request_id: u64,
    data_id: String,
    code_source: CodeSource,
    resource_limits: api_client::ResourceLimits,
    input_data: String,
) -> Result<()> {
    info!("🔨 Starting compilation for request_id={}", request_id);

    // Step 1: Compile the code
    let checksum = match compiler.compile(&code_source).await {
        Ok(checksum) => {
            info!("✅ Compilation successful: checksum={}", checksum);
            checksum
        }
        Err(e) => {
            warn!("❌ Compilation failed for request_id={}: {}", request_id, e);
            api_client
                .fail_task(request_id, format!("Compilation failed: {}", e))
                .await
                .context("Failed to fail compile task")?;
            return Ok(());
        }
    };

    // Step 2: Download the compiled WASM
    info!("📥 Downloading compiled WASM: checksum={}", checksum);
    let wasm_bytes = match api_client.download_wasm(&checksum).await {
        Ok(bytes) => {
            info!("✅ Downloaded WASM: {} bytes", bytes.len());
            bytes
        }
        Err(e) => {
            warn!("❌ Failed to download WASM: {}", e);
            api_client
                .fail_task(request_id, format!("Failed to download WASM: {}", e))
                .await?;
            return Ok(());
        }
    };

    // Step 3: Execute the WASM
    info!("⚙️  Executing WASM for request_id={} (size={} bytes)", request_id, wasm_bytes.len());

    // Use input_data from contract request
    info!("📝 Using input from contract: {}", input_data);
    info!("📊 Resource limits: max_instructions={}, max_memory={}MB, max_time={}s",
        resource_limits.max_instructions,
        resource_limits.max_memory_mb,
        resource_limits.max_execution_seconds);

    let input_bytes = input_data.as_bytes().to_vec();
    info!("🚀 Starting WASM execution now...");

    let result = match executor.execute(&wasm_bytes, &input_bytes, &resource_limits).await {
        Ok(result) => {
            info!("✅ WASM Execution completed: success={}, time={}ms, output_len={:?}, error={:?}",
                result.success,
                result.execution_time_ms,
                result.output.as_ref().map(|o| o.len()),
                result.error);
            if let Some(ref output) = result.output {
                info!("📤 WASM Output (first 200 bytes): {:?}",
                    String::from_utf8_lossy(&output[..output.len().min(200)]));
            }
            result
        }
        Err(e) => {
            warn!("❌ WASM Execution failed: {}", e);
            api_client
                .fail_task(request_id, format!("Execution failed: {}", e))
                .await?;
            return Ok(());
        }
    };

    // Step 4: Submit result to NEAR contract (promise_yield_resume)
    info!("📤 Submitting result to NEAR contract via promise_yield_resume");
    info!("   data_id={}", data_id);
    info!("   success={}, output_len={:?}", result.success, result.output.as_ref().map(|o| o.len()));
    match near_client.submit_execution_result(&data_id, &result).await {
        Ok(tx_hash) => {
            info!("✅ Successfully submitted to NEAR: tx_hash={}", tx_hash);

            // Mark task as complete in coordinator
            api_client
                .complete_task(request_id, result)
                .await
                .context("Failed to complete task in coordinator")?;

            info!("🎉 Task completed end-to-end for request_id={}", request_id);
        }
        Err(e) => {
            error!("❌ Failed to submit result to NEAR: {}", e);
            error!("Full error: {:?}", e);
            // Print error chain
            for (i, cause) in e.chain().enumerate() {
                error!("  [{}] {}", i, cause);
            }
            api_client
                .fail_task(request_id, format!("Failed to submit to NEAR: {}", e))
                .await?;
        }
    }

    Ok(())
}

/// Handle an execution task
async fn handle_execute_task(
    api_client: &ApiClient,
    executor: &Executor,
    near_client: &NearClient,
    request_id: u64,
    data_id: String,
    wasm_checksum: String,
    resource_limits: api_client::ResourceLimits,
) -> Result<()> {
    info!(
        "Executing WASM for request_id={}, data_id={}, checksum={}",
        request_id, data_id, wasm_checksum
    );

    // Download WASM from coordinator
    let wasm_bytes = match api_client.download_wasm(&wasm_checksum).await {
        Ok(bytes) => bytes,
        Err(e) => {
            warn!(
                "Failed to download WASM for request_id={}: {}",
                request_id, e
            );
            api_client
                .fail_task(request_id, format!("Failed to download WASM: {}", e))
                .await?;
            return Ok(());
        }
    };

    // Get input data (for now, use empty data - in real implementation, fetch from data_id)
    let input_data = vec![]; // TODO: Fetch actual input data using data_id

    // Execute WASM
    let result = executor
        .execute(&wasm_bytes, &input_data, &resource_limits)
        .await
        .context("Failed to execute WASM")?;

    info!(
        "Execution completed for request_id={}, success={}",
        request_id, result.success
    );

    // Submit result to NEAR contract using data_id
    match near_client.submit_execution_result(&data_id, &result).await {
        Ok(_) => {
            info!("Successfully submitted result to NEAR for request_id={}", request_id);

            // Mark task as complete in coordinator
            api_client
                .complete_task(request_id, result)
                .await
                .context("Failed to complete execute task")?;
        }
        Err(e) => {
            warn!(
                "Failed to submit result to NEAR for request_id={}: {}",
                request_id, e
            );

            // Mark task as failed
            api_client
                .fail_task(
                    request_id,
                    format!("Failed to submit result to NEAR: {}", e),
                )
                .await
                .context("Failed to fail execute task")?;
        }
    }

    Ok(())
}
