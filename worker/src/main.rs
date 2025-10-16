mod api_client;
mod compiler;
mod config;
mod event_monitor;
mod executor;
mod keystore_client;
mod near_client;

use anyhow::{Context, Result};
use tracing::{error, info, warn};

use api_client::{ApiClient, CodeSource, ExecutionResult, Task};
use compiler::Compiler;
use config::Config;
use event_monitor::EventMonitor;
use executor::Executor;
use keystore_client::KeystoreClient;
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

    // Initialize keystore client (optional)
    let keystore_client = if let (Some(keystore_url), Some(keystore_token)) = (
        &config.keystore_base_url,
        &config.keystore_auth_token,
    ) {
        info!("Keystore configured at: {}", keystore_url);
        info!("TEE mode: {}", config.tee_mode);
        Some(KeystoreClient::new(
            keystore_url.clone(),
            keystore_token.clone(),
            config.tee_mode.clone(),
        ))
    } else {
        info!("Keystore not configured - encrypted secrets will not be supported");
        None
    };

    // Initialize NEAR client
    let near_client = NearClient::new(
        config.near_rpc_url.clone(),
        config.operator_signer.clone(),
        config.offchainvm_contract_id.clone(),
    )
    .context("Failed to create NEAR client")?;

    // Start heartbeat task
    let heartbeat_api_client = api_client.clone();
    let heartbeat_worker_id = config.worker_id.clone();
    let heartbeat_worker_name = format!("worker-{}", config.worker_id);
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(30));
        loop {
            interval.tick().await;
            if let Err(e) = heartbeat_api_client
                .send_heartbeat(
                    heartbeat_worker_id.clone(),
                    heartbeat_worker_name.clone(),
                    "online",
                    None,
                )
                .await
            {
                warn!("Failed to send heartbeat: {}", e);
            }
        }
    });
    info!("Heartbeat task started (every 30 seconds)");

    // Start event monitor if enabled
    if config.enable_event_monitor {
        let event_api_client = api_client.clone();
        let neardata_url = config.neardata_api_url.clone();
        let fastnear_url = config.fastnear_api_url.clone();
        let contract_id = config.offchainvm_contract_id.clone();
        let start_block = config.start_block_height;
        let scan_interval_ms = config.scan_interval_ms;

        tokio::spawn(async move {
            info!("Starting event monitor...");
            match EventMonitor::new(
                event_api_client,
                neardata_url,
                fastnear_url,
                contract_id,
                start_block,
                scan_interval_ms,
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
            keystore_client.as_ref(),
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
    keystore_client: Option<&KeystoreClient>,
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
            encrypted_secrets,
            response_format,
            context,
        } => {
            handle_compile_task(
                api_client,
                compiler,
                executor,
                near_client,
                keystore_client,
                config,
                request_id,
                data_id,
                code_source,
                resource_limits,
                input_data,
                encrypted_secrets,
                response_format,
                context,
            )
            .await?;
        }
        Task::Execute {
            request_id,
            data_id,
            wasm_checksum,
            resource_limits,
            input_data,
            encrypted_secrets,
            build_target,
            response_format,
            context,
        } => {
            handle_execute_task(
                api_client,
                executor,
                near_client,
                keystore_client,
                config,
                request_id,
                data_id,
                wasm_checksum,
                resource_limits,
                input_data,
                encrypted_secrets,
                build_target,
                response_format,
                context,
            )
            .await?;
        }
    }

    Ok(true)
}

/// Merge user secrets with system environment variables
fn merge_env_vars(
    user_secrets: Option<std::collections::HashMap<String, String>>,
    context: &api_client::ExecutionContext,
    resource_limits: &api_client::ResourceLimits,
    request_id: u64,
) -> std::collections::HashMap<String, String> {

    let mut env_vars = user_secrets.unwrap_or_default();

    // Add execution context
    if let Some(ref sender_id) = context.sender_id {
        env_vars.insert("NEAR_SENDER_ID".to_string(), sender_id.clone());
    }
    if let Some(ref contract_id) = context.contract_id {
        env_vars.insert("NEAR_CONTRACT_ID".to_string(), contract_id.clone());
    }
    if let Some(block_height) = context.block_height {
        env_vars.insert("NEAR_BLOCK_HEIGHT".to_string(), block_height.to_string());
    }
    if let Some(block_timestamp) = context.block_timestamp {
        env_vars.insert("NEAR_BLOCK_TIMESTAMP".to_string(), block_timestamp.to_string());
    }

    // Add resource limits
    env_vars.insert("NEAR_MAX_INSTRUCTIONS".to_string(), resource_limits.max_instructions.to_string());
    env_vars.insert("NEAR_MAX_MEMORY_MB".to_string(), resource_limits.max_memory_mb.to_string());
    env_vars.insert("NEAR_MAX_EXECUTION_SECONDS".to_string(), resource_limits.max_execution_seconds.to_string());

    // Add request ID
    env_vars.insert("NEAR_REQUEST_ID".to_string(), request_id.to_string());

    env_vars
}

/// Handle a compilation task - now also executes and submits result
async fn handle_compile_task(
    api_client: &ApiClient,
    compiler: &Compiler,
    executor: &Executor,
    near_client: &NearClient,
    keystore_client: Option<&KeystoreClient>,
    config: &Config,
    request_id: u64,
    data_id: String,
    code_source: CodeSource,
    resource_limits: api_client::ResourceLimits,
    input_data: String,
    encrypted_secrets: Option<Vec<u8>>,
    response_format: api_client::ResponseFormat,
    context: api_client::ExecutionContext,
) -> Result<()> {
    info!("🔨 Starting compilation for request_id={}", request_id);

    // Step 1: Compile the code
    let checksum = match compiler.compile(&code_source).await {
        Ok(checksum) => {
            info!("✅ Compilation successful: checksum={}", checksum);
            checksum
        }
        Err(e) => {
            let error_msg = format!("Compilation failed: {}", e);
            warn!("❌ {}", error_msg);

            // Create error result to submit to NEAR
            let error_result = ExecutionResult {
                success: false,
                output: None,
                error: Some(error_msg.clone()),
                execution_time_ms: 0,
                instructions: 0,
            };

            // Submit error to NEAR contract
            match near_client.submit_execution_result(request_id, &error_result).await {
                Ok(tx_hash) => {
                    info!("✅ Compilation error submitted to NEAR: tx_hash={}", tx_hash);
                }
                Err(submit_err) => {
                    error!("❌ Failed to submit compilation error to NEAR: {}", submit_err);
                }
            }

            // Mark task as failed in coordinator
            api_client
                .fail_task(request_id, error_msg)
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

    // Step 3: Decrypt secrets if provided
    let env_vars = if let Some(encrypted) = &encrypted_secrets {
        info!("🔐 Found encrypted_secrets field: {} bytes", encrypted.len());
        if let Some(keystore) = keystore_client {
            info!("🔑 Keystore client configured, attempting decryption...");
            match keystore.decrypt_secrets(encrypted, Some(&request_id.to_string())).await {
                Ok(secrets) => {
                    info!("✅ Secrets decrypted successfully! {} environment variables", secrets.len());
                    info!("📝 Environment variables: {:?}", secrets.keys().collect::<Vec<_>>());
                    Some(secrets)
                }
                Err(e) => {
                    let error_msg = format!("Failed to decrypt secrets: {}", e);
                    warn!("❌ {}", error_msg);
                    api_client.fail_task(request_id, error_msg).await?;
                    return Ok(());
                }
            }
        } else {
            warn!("⚠️  Encrypted secrets provided ({} bytes) but keystore not configured - ignoring", encrypted.len());
            None
        }
    } else {
        info!("ℹ️  No encrypted_secrets in task");
        None
    };

    // Step 4: Execute the WASM
    info!("⚙️  Executing WASM for request_id={} (size={} bytes)", request_id, wasm_bytes.len());

    // Use input_data from contract request
    info!("📝 Using input from contract: {}", input_data);
    if env_vars.is_some() {
        info!("🔑 Secrets available for execution (will be passed via WASI env)");
    }
    info!("📊 Resource limits: max_instructions={}, max_memory={}MB, max_time={}s",
        resource_limits.max_instructions,
        resource_limits.max_memory_mb,
        resource_limits.max_execution_seconds);

    let input_bytes = input_data.as_bytes().to_vec();
    info!("🚀 Starting WASM execution now...");

    // Extract build_target for optimized executor selection
    let build_target = match &code_source {
        CodeSource::GitHub { build_target, .. } => Some(build_target.as_str()),
    };

    // Merge user secrets with system environment variables
    let merged_env_vars = merge_env_vars(env_vars, &context, &resource_limits, request_id);
    info!("🌍 Environment variables: {} total", merged_env_vars.len());

    let result = match executor.execute(&wasm_bytes, &input_bytes, &resource_limits, Some(merged_env_vars), build_target, &response_format).await {
        Ok(result) => {
            info!("✅ WASM Execution completed: success={}, time={}ms, error={:?}",
                result.success,
                result.execution_time_ms,
                result.error);
            if let Some(ref output) = &result.output {
                use api_client::ExecutionOutput;
                let output_preview = match output {
                    ExecutionOutput::Bytes(data) => format!("Bytes({} bytes)", data.len()),
                    ExecutionOutput::Text(data) => {
                        let preview = if data.len() > 200 { &data[..200] } else { data };
                        format!("Text: {}", preview)
                    }
                    ExecutionOutput::Json(data) => format!("Json: {}", serde_json::to_string(data).unwrap_or_default()),
                };
                info!("📤 WASM Output: {}", output_preview);
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
    info!("   request_id={}", request_id);
    info!("   data_id={}", data_id);
    info!("   success={}", result.success);
    match near_client.submit_execution_result(request_id, &result).await {
        Ok(tx_hash) => {
            info!("✅ Successfully submitted to NEAR: tx_hash={}", tx_hash);

            // Mark task as complete in coordinator
            api_client
                .complete_task(
                    request_id,
                    Some(data_id.clone()),
                    result,
                    Some(tx_hash),
                    None, // TODO: Extract user_account_id from contract request event
                    None, // TODO: Extract near_payment_yocto from contract request event
                    config.worker_id.clone(),
                )
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
    keystore_client: Option<&KeystoreClient>,
    config: &Config,
    request_id: u64,
    data_id: String,
    wasm_checksum: String,
    resource_limits: api_client::ResourceLimits,
    input_data: String,
    encrypted_secrets: Option<Vec<u8>>,
    build_target: Option<String>,
    response_format: api_client::ResponseFormat,
    context: api_client::ExecutionContext,
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

    // Decrypt secrets if provided
    let env_vars = if let Some(encrypted) = &encrypted_secrets {
        info!("🔐 Found encrypted_secrets field: {} bytes", encrypted.len());
        if let Some(keystore) = keystore_client {
            info!("🔑 Keystore client configured, attempting decryption...");
            match keystore.decrypt_secrets(encrypted, Some(&request_id.to_string())).await {
                Ok(secrets) => {
                    info!("✅ Secrets decrypted successfully! {} environment variables", secrets.len());
                    info!("📝 Environment variables: {:?}", secrets.keys().collect::<Vec<_>>());
                    Some(secrets)
                }
                Err(e) => {
                    let error_msg = format!("Failed to decrypt secrets: {}", e);
                    warn!("❌ {}", error_msg);
                    api_client.fail_task(request_id, error_msg).await?;
                    return Ok(());
                }
            }
        } else {
            warn!("⚠️  Encrypted secrets provided ({} bytes) but keystore not configured - ignoring", encrypted.len());
            None
        }
    } else {
        info!("ℹ️  No encrypted_secrets in task");
        None
    };

    // Use input data from task
    info!("📝 Using input from task: {}", input_data);
    if env_vars.is_some() {
        info!("🔑 Secrets will be passed via WASI environment");
    }
    let input_bytes = input_data.as_bytes().to_vec();

    // Merge user secrets with system environment variables
    let merged_env_vars = merge_env_vars(env_vars, &context, &resource_limits, request_id);
    info!("🌍 Environment variables: {} total", merged_env_vars.len());

    // Execute WASM with environment variables and build target hint
    let result = executor
        .execute(&wasm_bytes, &input_bytes, &resource_limits, Some(merged_env_vars), build_target.as_deref(), &response_format)
        .await
        .context("Failed to execute WASM")?;

    info!(
        "Execution completed for request_id={}, success={}",
        request_id, result.success
    );

    // Submit result to NEAR contract using request_id
    match near_client.submit_execution_result(request_id, &result).await {
        Ok(tx_hash) => {
            info!("Successfully submitted result to NEAR for request_id={}, tx_hash={}", request_id, tx_hash);

            // Mark task as complete in coordinator
            api_client
                .complete_task(
                    request_id,
                    Some(data_id.clone()),
                    result,
                    Some(tx_hash),
                    None, // TODO: Extract from contract event
                    None, // TODO: Extract from contract event
                    config.worker_id.clone(),
                )
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
