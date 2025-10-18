mod api_client;
mod compiler;
mod config;
mod event_monitor;
mod executor;
mod keystore_client;
mod near_client;

use anyhow::{Context, Result};
use tracing::{error, info, warn};

use api_client::{ApiClient, CodeSource, ExecutionResult, JobInfo, JobType, Task};
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

    info!("📨 Received task: {:?}", task);

    // Extract task details
    let (request_id, data_id, code_source, resource_limits, input_data, encrypted_secrets, response_format, context, user_account_id, near_payment_yocto, transaction_hash) = match &task {
        Task::Compile {
            request_id,
            data_id,
            code_source,
            resource_limits,
            input_data,
            encrypted_secrets,
            response_format,
            context,
            user_account_id,
            near_payment_yocto,
            transaction_hash,
            ..
        } => (
            *request_id,
            data_id.clone(),
            code_source.clone(),
            resource_limits.clone(),
            input_data.clone(),
            encrypted_secrets.clone(),
            response_format.clone(),
            context.clone(),
            user_account_id.clone(),
            near_payment_yocto.clone(),
            transaction_hash.clone(),
        ),
        Task::Execute { .. } => {
            warn!("⚠️ Received Execute task directly - this should not happen in job-based workflow");
            return Ok(true);
        }
    };

    // Claim jobs for this task
    info!("🎯 Claiming jobs for request_id={} data_id={}", request_id, data_id);
    let claim_response = match api_client
        .claim_job(
            request_id,
            data_id.clone(),
            config.worker_id.clone(),
            &code_source,
            &resource_limits,
            user_account_id.clone(),
            near_payment_yocto.clone(),
            transaction_hash.clone(),
        )
        .await
    {
        Ok(response) => response,
        Err(e) => {
            warn!("⚠️ Failed to claim job (likely already claimed): {}", e);
            return Ok(true); // Not an error, just means another worker got it first
        }
    };

    if claim_response.jobs.is_empty() {
        warn!("⚠️ No jobs returned for request_id={}", request_id);
        return Ok(true);
    }

    info!("✅ Claimed {} job(s) for request_id={}", claim_response.jobs.len(), request_id);

    // Extract pricing from response
    let pricing = &claim_response.pricing;
    info!(
        "💰 Pricing: per_compile_ms={} max_compile_sec={}",
        pricing.per_compile_ms_fee, pricing.max_compilation_seconds
    );

    // Local WASM cache - if we compile, we keep it in memory for execute job
    let mut compiled_wasm: Option<(String, Vec<u8>, u64)> = None; // (checksum, bytes, compile_time_ms)

    // Process each job in order
    for job in claim_response.jobs {
        info!("🔧 Processing job_id={} type={:?}", job.job_id, job.job_type);

        if !job.allowed {
            warn!("⚠️ Job {} not allowed (already completed or failed)", job.job_id);
            continue;
        }

        match job.job_type {
            JobType::Compile => {
                match handle_compile_job(
                    api_client,
                    compiler,
                    &job,
                    &code_source,
                    pricing,
                    near_payment_yocto.as_ref(),
                )
                .await {
                    Ok((checksum, wasm_bytes, compile_time_ms)) => {
                        // Store in local cache for execute job (including compile time)
                        compiled_wasm = Some((checksum, wasm_bytes, compile_time_ms));
                    }
                    Err(e) => {
                        // Compilation failed - notify contract
                        error!("❌ Compilation failed, notifying contract: {}", e);

                        let error_message = format!("Compilation failed: {}", e);
                        let execution_result = ExecutionResult {
                            success: false,
                            output: None,
                            error: Some(error_message),
                            execution_time_ms: 0,
                            instructions: 0,
                            compile_time_ms: None,
                        };

                        if let Err(submit_err) = near_client
                            .submit_execution_result(request_id, &execution_result)
                            .await
                        {
                            error!("❌ Failed to submit compilation error to contract: {}", submit_err);
                        } else {
                            info!("✅ Compilation error submitted to contract");
                        }

                        // Don't process execute job - compilation failed
                        return Err(e);
                    }
                }
            }
            JobType::Execute => {
                handle_execute_job(
                    api_client,
                    executor,
                    near_client,
                    keystore_client,
                    &job,
                    &code_source,
                    &resource_limits,
                    &input_data,
                    encrypted_secrets.as_ref(),
                    &response_format,
                    &context,
                    user_account_id.as_ref(),
                    near_payment_yocto.as_ref(),
                    request_id,
                    &data_id,
                    compiled_wasm.as_ref(), // Pass local WASM cache
                )
                .await?;
            }
        }
    }

    // Upload WASM to coordinator after all work is done (non-critical)
    if let Some((checksum, wasm_bytes, _compile_time_ms)) = compiled_wasm {
        info!("📤 Uploading compiled WASM to coordinator (background)");
        if let Err(e) = api_client.upload_wasm(checksum, code_source.repo().to_string(), code_source.commit().to_string(), wasm_bytes).await {
            warn!("⚠️ Failed to upload WASM to coordinator: {}", e);
            // Not critical - execution already completed and submitted to NEAR
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

/// Handle a compile job
/// Returns (checksum, wasm_bytes, compile_time_ms) for local caching
async fn handle_compile_job(
    api_client: &ApiClient,
    compiler: &Compiler,
    job: &JobInfo,
    code_source: &CodeSource,
    pricing: &api_client::PricingConfig,
    user_payment: Option<&String>,
) -> Result<(String, Vec<u8>, u64)> {
    info!("🔨 Starting compilation job_id={}", job.job_id);

    // Validate compilation budget
    if let Some(payment_str) = user_payment {
        // Parse pricing and payment
        let per_compile_ms_fee: u128 = pricing.per_compile_ms_fee.parse()
            .context("Failed to parse per_compile_ms_fee")?;
        let user_payment_yocto: u128 = payment_str.parse()
            .context("Failed to parse user_payment")?;

        // Calculate max affordable compilation time
        let max_affordable_seconds = if per_compile_ms_fee > 0 {
            (user_payment_yocto / per_compile_ms_fee / 1000) as u64
        } else {
            u64::MAX
        };

        info!(
            "💰 Compilation budget check: payment={} yoctoNEAR, max_affordable={}s, contract_limit={}s",
            user_payment_yocto, max_affordable_seconds, pricing.max_compilation_seconds
        );

        // Check if user's payment covers at least minimum compilation time (30 seconds)
        const MIN_COMPILATION_SECONDS: u64 = 30;
        if max_affordable_seconds < MIN_COMPILATION_SECONDS {
            let min_payment = (MIN_COMPILATION_SECONDS as u128) * 1000 * per_compile_ms_fee;
            let error_msg = format!(
                "Insufficient payment for compilation: payment covers only {}s but minimum is {}s. Need at least {} yoctoNEAR",
                max_affordable_seconds,
                MIN_COMPILATION_SECONDS,
                min_payment
            );
            error!("❌ {}", error_msg);

            // Report budget error to coordinator
            if let Err(e) = api_client
                .complete_job(job.job_id, false, None, Some(error_msg.clone()), 0, 0, None, None, None)
                .await
            {
                warn!("⚠️ Failed to report budget error: {}", e);
            }

            return Err(anyhow::anyhow!(error_msg));
        }
    }

    let start_time = std::time::Instant::now();

    // Calculate timeout: min(contract_limit, user_budget_limit)
    let timeout_seconds = if let Some(payment_str) = user_payment {
        let per_compile_ms_fee: u128 = pricing.per_compile_ms_fee.parse().unwrap_or(1);
        let user_payment_yocto: u128 = payment_str.parse().unwrap_or(0);
        let budget_limit = if per_compile_ms_fee > 0 {
            (user_payment_yocto / per_compile_ms_fee / 1000) as u64
        } else {
            pricing.max_compilation_seconds
        };
        Some(std::cmp::min(budget_limit, pricing.max_compilation_seconds))
    } else {
        Some(pricing.max_compilation_seconds)
    };

    // Compile the code with timeout (returns checksum and bytes, does NOT upload yet)
    let compile_result = compiler.compile_local(code_source, timeout_seconds).await;
    let compile_time_ms = start_time.elapsed().as_millis() as u64;

    match compile_result {
        Ok((checksum, wasm_bytes)) => {
            info!("✅ Compilation successful: checksum={} size={} bytes time={}ms",
                checksum, wasm_bytes.len(), compile_time_ms);

            // Calculate compilation cost: compile_time_ms * per_compile_ms_fee
            let per_compile_ms_fee: u128 = pricing.per_compile_ms_fee.parse()
                .unwrap_or_else(|_| {
                    warn!("Failed to parse per_compile_ms_fee, using 0");
                    0
                });
            let compile_cost_yocto = compile_time_ms as u128 * per_compile_ms_fee;

            if compile_cost_yocto > 0 {
                info!("💰 Compilation cost: {} yoctoNEAR ({:.6} NEAR) = {}ms * {} yoctoNEAR/ms",
                    compile_cost_yocto,
                    compile_cost_yocto as f64 / 1e24,
                    compile_time_ms,
                    per_compile_ms_fee
                );
            }

            // Report completion to coordinator (non-critical)
            if let Err(e) = api_client
                .complete_job(
                    job.job_id,
                    true,
                    None,
                    None,
                    compile_time_ms,
                    0, // No instructions for compilation
                    Some(checksum.clone()),
                    None, // No actual_cost for compile jobs
                    Some(compile_cost_yocto.to_string()), // Send compile cost
                )
                .await
            {
                warn!("⚠️ Failed to report compile job completion: {}", e);
                // Continue anyway - will upload later
            }

            Ok((checksum, wasm_bytes, compile_time_ms))
        }
        Err(e) => {
            let error_msg = format!("Compilation failed: {}", e);
            warn!("❌ {}", error_msg);

            // Report failure to coordinator
            if let Err(report_err) = api_client
                .complete_job(
                    job.job_id,
                    false,
                    None,
                    Some(error_msg.clone()),
                    compile_time_ms,
                    0,
                    None,
                    None,
                    None,
                )
                .await
            {
                warn!("⚠️ Failed to report compile job failure: {}", report_err);
            }

            Err(e)
        }
    }
}

/// Handle an execute job
async fn handle_execute_job(
    api_client: &ApiClient,
    executor: &Executor,
    near_client: &NearClient,
    keystore_client: Option<&KeystoreClient>,
    job: &JobInfo,
    code_source: &CodeSource,
    resource_limits: &api_client::ResourceLimits,
    input_data: &str,
    encrypted_secrets: Option<&Vec<u8>>,
    response_format: &api_client::ResponseFormat,
    context: &api_client::ExecutionContext,
    user_account_id: Option<&String>,
    near_payment_yocto: Option<&String>,
    request_id: u64,
    data_id: &str,
    compiled_wasm: Option<&(String, Vec<u8>, u64)>, // Local cache from compile job (checksum, bytes, compile_time_ms)
) -> Result<()> {
    info!("⚙️ Starting execution job_id={}", job.job_id);

    // Get WASM checksum from job
    let wasm_checksum = job.wasm_checksum.as_ref()
        .ok_or_else(|| anyhow::anyhow!("Execute job missing wasm_checksum"))?;

    // Get WASM bytes and compile time - use local cache if available, otherwise download
    let (wasm_bytes, compile_time_ms) = if let Some((cached_checksum, cached_bytes, cached_compile_time)) = compiled_wasm {
        if cached_checksum == wasm_checksum {
            info!("✅ Using locally compiled WASM: {} bytes (cache hit!) compiled in {}ms", cached_bytes.len(), cached_compile_time);
            (cached_bytes.clone(), Some(*cached_compile_time))
        } else {
            warn!("⚠️ Checksum mismatch - downloading from coordinator");
            info!("📥 Downloading WASM: checksum={}", wasm_checksum);
            let bytes = api_client.download_wasm(wasm_checksum).await
                .context("Failed to download WASM")?;
            info!("✅ Downloaded WASM: {} bytes", bytes.len());
            (bytes, None) // Unknown compile time for cached WASM
        }
    } else {
        // No local cache - download from coordinator
        info!("📥 Downloading WASM: checksum={}", wasm_checksum);
        let bytes = api_client.download_wasm(wasm_checksum).await
            .context("Failed to download WASM")?;
        info!("✅ Downloaded WASM: {} bytes", bytes.len());
        (bytes, None) // Unknown compile time for cached WASM
    };

    // Decrypt secrets if provided
    let user_secrets = if let (Some(encrypted), Some(keystore)) = (encrypted_secrets, keystore_client) {
        info!("🔐 Decrypting secrets via keystore...");
        match keystore.decrypt_secrets(encrypted, Some(data_id)).await {
            Ok(secrets) => {
                info!("✅ Secrets decrypted successfully");
                Some(secrets)
            }
            Err(e) => {
                let error_msg = format!("Failed to decrypt secrets: {}", e);
                error!("❌ {}", error_msg);

                // Fail the job
                api_client
                    .complete_job(job.job_id, false, None, Some(error_msg), 0, 0, None, None, None)
                    .await?;
                return Ok(());
            }
        }
    } else {
        None
    };

    // Merge environment variables
    let env_vars = merge_env_vars(user_secrets, context, resource_limits, request_id);

    // Get build target from code source
    let build_target = match code_source {
        CodeSource::GitHub { build_target, .. } => Some(build_target.as_str()),
    };

    // Execute WASM
    info!("🚀 Executing WASM...");
    let exec_result = executor
        .execute(
            &wasm_bytes,
            input_data.as_bytes(),
            resource_limits,
            Some(env_vars),
            build_target,
            response_format,
        )
        .await;

    match exec_result {
        Ok(mut execution_result) => {
            // Add compilation time if WASM was compiled in this execution
            execution_result.compile_time_ms = compile_time_ms;

            if let Some(ct) = compile_time_ms {
                info!(
                    "✅ Execution successful: compile={}ms execute={}ms instructions={}",
                    ct, execution_result.execution_time_ms, execution_result.instructions
                );
            } else {
                info!(
                    "✅ Execution successful: time={}ms instructions={} (using cached WASM)",
                    execution_result.execution_time_ms, execution_result.instructions
                );
            }

            // Submit result to NEAR contract (critical path - highest priority)
            info!("📤 Submitting result to NEAR contract...");
            let near_result = near_client.submit_execution_result(request_id, &execution_result).await;

            // Report to coordinator (can wait, non-critical)
            match near_result {
                Ok((tx_hash, outcome)) => {
                    info!("✅ Result submitted to NEAR: tx_hash={}", tx_hash);

                    // Extract actual cost from contract logs
                    let actual_cost = NearClient::extract_payment_from_logs(&outcome);
                    if actual_cost > 0 {
                        info!("💰 Extracted execution cost from contract: {} yoctoNEAR ({:.6} NEAR)",
                            actual_cost, actual_cost as f64 / 1e24);
                    }

                    // Report success to coordinator (async, can fail without breaking flow)
                    if let Err(e) = api_client
                        .complete_job(
                            job.job_id,
                            true,
                            execution_result.output.clone(),
                            None,
                            execution_result.execution_time_ms,
                            execution_result.instructions,
                            None,
                            if actual_cost > 0 { Some(actual_cost.to_string()) } else { None },
                            None, // No compile_cost for execute jobs
                        )
                        .await
                    {
                        warn!("⚠️ Failed to report execute job completion: {}", e);
                        // Continue anyway - NEAR transaction is already submitted
                    }
                }
                Err(e) => {
                    let error_msg = format!("Failed to submit to NEAR: {}", e);
                    error!("❌ {}", error_msg);

                    // Report failure to coordinator
                    if let Err(report_err) = api_client
                        .complete_job(
                            job.job_id,
                            false,
                            None,
                            Some(error_msg.clone()),
                            execution_result.execution_time_ms,
                            execution_result.instructions,
                            None,
                            None,
                            None,
                        )
                        .await
                    {
                        warn!("⚠️ Failed to report execute job failure: {}", report_err);
                    }

                    return Err(e);
                }
            }
        }
        Err(e) => {
            let error_msg = format!("Execution failed: {}", e);
            error!("❌ {}", error_msg);

            let result = ExecutionResult {
                success: false,
                output: None,
                error: Some(error_msg.clone()),
                execution_time_ms: 0,
                instructions: 0,
                compile_time_ms,
            };

            // Submit error to NEAR contract (critical path)
            let near_result = near_client.submit_execution_result(request_id, &result).await;

            // Report to coordinator (non-critical)
            if let Err(report_err) = api_client
                .complete_job(
                    job.job_id,
                    false,
                    None,
                    Some(error_msg.clone()),
                    0,
                    0,
                    None,
                    None,
                    None,
                )
                .await
            {
                warn!("⚠️ Failed to report execute job failure: {}", report_err);
            }

            // Check if NEAR submission succeeded
            match near_result {
                Ok((tx_hash, _outcome)) => {
                    info!("✅ Error submitted to NEAR: tx_hash={}", tx_hash);
                }
                Err(submit_err) => {
                    error!("❌ Failed to submit error to NEAR: {}", submit_err);
                }
            }

            return Err(e);
        }
    }

    Ok(())
}

/// OLD HANDLERS - TO BE REMOVED AFTER TESTING

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
    user_account_id: Option<String>,
    near_payment_yocto: Option<String>,
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
                compile_time_ms: None, // Compilation failed before execute
            };

            // Submit error to NEAR contract
            match near_client.submit_execution_result(request_id, &error_result).await {
                Ok((tx_hash, _outcome)) => {
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
        Ok((tx_hash, outcome)) => {
            info!("✅ Successfully submitted to NEAR: tx_hash={}", tx_hash);

            // Extract actual cost from contract logs
            let actual_cost = NearClient::extract_payment_from_logs(&outcome);
            let actual_cost_near = actual_cost as f64 / 1e24;
            info!("💰 Extracted execution cost from contract logs: {} yoctoNEAR ({:.6} NEAR)",
                actual_cost, actual_cost_near);

            // Extract GitHub repo and commit from code_source
            let (github_repo, github_commit) = match &code_source {
                CodeSource::GitHub { repo, commit, .. } => {
                    (Some(repo.clone()), Some(commit.clone()))
                }
            };

            // Mark task as complete in coordinator with cost from contract
            api_client
                .complete_task(
                    request_id,
                    Some(data_id.clone()),
                    result,
                    Some(tx_hash),
                    user_account_id,
                    Some(actual_cost.to_string()), // Send cost extracted from contract logs
                    config.worker_id.clone(),
                    github_repo,
                    github_commit,
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
    user_account_id: Option<String>,
    near_payment_yocto: Option<String>,
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
        Ok((tx_hash, outcome)) => {
            info!("Successfully submitted result to NEAR for request_id={}, tx_hash={}", request_id, tx_hash);

            // Extract actual cost from contract logs (contains estimated_cost from contract calculation)
            let actual_cost = NearClient::extract_payment_from_logs(&outcome);
            let actual_cost_near = actual_cost as f64 / 1e24;
            info!("💰 Extracted execution cost from contract logs: {} yoctoNEAR ({:.6} NEAR)",
                actual_cost, actual_cost_near);

            // Mark task as complete in coordinator with cost from contract
            api_client
                .complete_task(
                    request_id,
                    Some(data_id.clone()),
                    result,
                    Some(tx_hash),
                    user_account_id,
                    Some(actual_cost.to_string()), // Send cost extracted from contract logs
                    config.worker_id.clone(),
                    None, // No github_repo for Execute tasks
                    None, // No github_commit for Execute tasks
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
