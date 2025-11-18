mod api_client;
mod collateral_fetcher;
mod compiler;
mod config;
mod event_monitor;
mod executor;
mod keystore_client;
mod near_client;
mod registration;
mod tdx_attestation;

use anyhow::{Context, Result};
use tracing::{error, info, warn};

use api_client::{ApiClient, CodeSource, ExecutionResult, JobInfo, JobType};
use collateral_fetcher::fetch_collateral_from_phala;
use compiler::Compiler;
use config::Config;
use event_monitor::EventMonitor;
use executor::Executor;
use keystore_client::KeystoreClient;
use near_client::NearClient;
use tdx_attestation::TdxClient;

/// Generate a dummy TDX quote and fetch collateral from Phala Cloud API
///
/// This is used when registration fails with "Quote collateral required" error.
/// We generate a fresh quote (with dummy data) just to get the collateral JSON.
async fn generate_dummy_quote_and_fetch_collateral(tdx_client: &TdxClient) -> Result<String> {
    info!("Generating dummy TDX quote for collateral fetching...");

    // Generate quote with dummy 32-byte data
    let dummy_data = [0u8; 32];
    let tdx_quote_hex = tdx_client
        .generate_registration_quote(&dummy_data)
        .await
        .context("Failed to generate dummy TDX quote")?;

    info!("   Quote generated: {} bytes", tdx_quote_hex.len() / 2);

    // Fetch collateral from Phala API
    let collateral_json = fetch_collateral_from_phala(&tdx_quote_hex)
        .await
        .context("Failed to fetch collateral from Phala API")?;

    Ok(collateral_json)
}

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
    info!("Worker capabilities: {:?}", config.capabilities.to_array());

    // Initialize API client
    let api_client = ApiClient::new(config.api_base_url.clone(), config.api_auth_token.clone())
        .context("Failed to create API client")?;

    // Initialize compiler (only if compilation capability enabled)
    let compiler = if config.capabilities.can_compile() {
        info!("✅ Compilation capability enabled - initializing compiler");
        Some(Compiler::new(api_client.clone(), config.clone())
            .context("Failed to create compiler")?)
    } else {
        info!("⚠️  Compilation capability disabled - will only handle Execute jobs");
        None
    };

    // Initialize executor
    let executor = Executor::new(config.default_max_instructions, config.print_wasm_stderr);

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

    // Initialize TDX client for task attestations
    let tdx_client = tdx_attestation::TdxClient::new(config.tee_mode.clone());

    // IMPORTANT: Worker registration MUST happen BEFORE creating NearClient
    // because NearClient requires operator_signer which is generated during registration
    // Make config mutable so we can set operator_signer after registration
    let mut config = config;

    // Check registration mode: TEE or legacy
    if config.use_tee_registration {
        info!("🔐 TEE registration mode enabled (USE_TEE_REGISTRATION=true)");

        // Register worker key with register contract (if configured)
        // This MUST happen before creating NearClient
        if let Some(ref register_contract_id) = config.register_contract_id {
            info!("🔑 Worker registration enabled - attempting to register with {}...", register_contract_id);

            // Use init account for gas payment if configured
            let (init_account_id, init_secret_key) = if let (Some(init_id), Some(init_signer)) =
                (&config.init_account_id, &config.init_account_signer) {
                info!("   Using init account for gas payment: {}", init_id);
                (init_id.clone(), init_signer.secret_key.clone())
            } else {
                error!("❌ REGISTER_CONTRACT_ID is set but init account credentials missing");
                error!("   When using worker registration, you must provide:");
                error!("   - INIT_ACCOUNT_ID");
                error!("   - INIT_ACCOUNT_PRIVATE_KEY");
                return Err(anyhow::anyhow!("Init account credentials required for worker registration"));
            };

            // Attempt registration (only once - fail fast if it fails)
            let (_public_key, secret_key, tdx_quote_hex) = match registration::register_worker_on_startup(
                config.near_rpc_url.clone(),
                register_contract_id.clone(),
                config.operator_account_id.clone(),
                init_account_id.clone(),
                init_secret_key.clone(),
                &tdx_client,
            ).await {
                Ok(result) => {
                    info!("✅ Worker keypair ready: {}", result.0);
                    info!("   Key registered and ready for signing execution results");
                    result
                }
                Err(e) => {
                    error!("❌ Worker registration flow failed: {:?}", e);
                    error!("   Worker CANNOT start without registered key");
                    error!("   Error chain:");
                    for (i, cause) in e.chain().enumerate() {
                        error!("      {}: {}", i, cause);
                    }

                    // Auto-fetch collateral from Phala Cloud for ANY registration error
                    // This helps diagnose all issues (missing collateral, wrong RTMR3, etc.)
                    error!("");
                    error!("🔍 Fetching collateral from Phala Cloud API for diagnostics...");
                    error!("");

                    match generate_dummy_quote_and_fetch_collateral(&tdx_client).await {
                        Ok(collateral_json) => {
                            error!("✅ Successfully fetched collateral from Phala Cloud!");
                            error!("");
                            error!("📋 COLLATERAL JSON (copy this for update_collateral call):");
                            error!("");
                            error!("{}", collateral_json);
                            error!("");
                            error!("📝 To cache this collateral in the register contract, run:");
                            error!("");
                            error!("   COLLATERAL=$(cat <<'EOF'");
                            error!("{}", collateral_json);
                            error!("EOF");
                            error!("   )");
                            error!("");
                            error!("   near call {} update_collateral \\", register_contract_id);
                            error!("     \"{{\\\"collateral\\\":$COLLATERAL}}\" \\");
                            error!("     --accountId outlayer.testnet \\");
                            error!("     --gas 300000000000000");
                            error!("");
                        }
                        Err(fetch_err) => {
                            error!("⚠️  Failed to auto-fetch collateral: {:?}", fetch_err);
                            error!("   (This is OK if you already have collateral cached)");
                            error!("");
                        }
                    }

                    error!("📝 Common issues:");
                    error!("   - Missing collateral: Cache collateral JSON above via update_collateral");
                    error!("   - RTMR3 not approved: Check contract logs for RTMR3 and add via add_approved_rtmr3");
                    error!("   - Init account balance: Verify init-worker.outlayer.testnet has funds");
                    error!("");
                    error!("⏹️  Worker stopped - fix the issue and restart");

                    return Err(anyhow::anyhow!("Worker registration failed: {:?}", e));
                }
            };

            // Set operator signer with generated keypair
            let operator_signer = near_crypto::InMemorySigner::from_secret_key(
                config.operator_account_id.clone(),
                secret_key,
            );
            config.set_operator_signer(operator_signer);
            info!("✅ Operator signer configured for account: {}", config.operator_account_id);

            // Send startup attestation to coordinator (using TDX quote from registration)
            info!("📤 Sending startup attestation to coordinator...");
            if let Err(e) = send_startup_attestation_with_quote(&api_client, &tdx_quote_hex, &config).await {
                error!("❌ Failed to send startup attestation to coordinator: {}", e);
                error!("   This is required for coordinator to track worker RTMR3");
                error!("   Common causes:");
                error!("   - Coordinator not accessible (check API_BASE_URL)");
                error!("   - Worker auth token invalid (check API_AUTH_TOKEN)");
                error!("   - Database migration not applied (check coordinator logs)");
                error!("");
                error!("⏹️  Worker stopped - fix the issue and restart");
                return Err(anyhow::anyhow!("Startup attestation failed: {:?}", e));
            }
            info!("✅ Startup attestation sent successfully - worker registered with coordinator");
        } else {
            error!("❌ Worker registration disabled - REGISTER_CONTRACT_ID not set");
            error!("   Worker MUST use registration flow to generate ephemeral keys in TEE");
            error!("   Set REGISTER_CONTRACT_ID or use USE_TEE_REGISTRATION=false for legacy mode");
            return Err(anyhow::anyhow!("Worker registration required - REGISTER_CONTRACT_ID must be set"));
        }
    } else {
        info!("🔓 Legacy mode enabled (USE_TEE_REGISTRATION=false)");
        info!("   Using OPERATOR_PRIVATE_KEY from .env for all transactions");
        info!("   ⚠️  This mode is for testnet only - use TEE registration for production!");
    }

    // Create NearClient with operator signer from registration
    let near_client = NearClient::new(
        config.near_rpc_url.clone(),
        config.get_operator_signer().clone(),
        config.offchainvm_contract_id.clone(),
    )
    .context("Failed to create NEAR client")?;
    info!("NEAR client initialized");

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
            compiler.as_ref(),
            &executor,
            &near_client,
            keystore_client.as_ref(),
            &tdx_client,
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
    compiler: Option<&Compiler>,
    executor: &Executor,
    near_client: &NearClient,
    keystore_client: Option<&KeystoreClient>,
    tdx_client: &tdx_attestation::TdxClient,
    config: &Config,
) -> Result<bool> {
    // Poll for a task (with long-polling) - specify capabilities to poll correct queue
    let capabilities = config.capabilities.to_array();
    let task = api_client
        .poll_task(config.poll_timeout_seconds, &capabilities)
        .await
        .context("Failed to poll for task")?;

    let Some(execution_request) = task else {
        // No execution request available
        return Ok(false);
    };

    info!("📨 Received execution request: {:?}", execution_request);

    // Extract request details
    let request_id = execution_request.request_id;
    let data_id = execution_request.data_id.clone();
    let code_source = execution_request.code_source.clone();
    let resource_limits = execution_request.resource_limits.clone();
    let input_data = execution_request.input_data.clone();
    let secrets_ref = execution_request.secrets_ref.clone();
    let response_format = execution_request.response_format.clone();
    let context = execution_request.context.clone();
    let user_account_id = execution_request.user_account_id.clone();
    let near_payment_yocto = execution_request.near_payment_yocto.clone();
    let transaction_hash = context.transaction_hash.clone();

    // Claim jobs for this task with worker capabilities
    info!("🎯 Claiming jobs for request_id={} data_id={} with capabilities={:?}",
          request_id, data_id, config.capabilities.to_array());
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
            config.capabilities.to_array(),
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
    let mut compiled_wasm: Option<(String, Vec<u8>, u64, Option<String>)> = None; // (checksum, bytes, compile_time_ms, created_at)

    // Process each job in order
    for job in claim_response.jobs {
        info!("🔧 Processing job_id={} type={:?}", job.job_id, job.job_type);

        if !job.allowed {
            warn!("⚠️ Job {} not allowed (already completed or failed)", job.job_id);
            continue;
        }

        match job.job_type {
            JobType::Compile => {
                // Skip compile jobs if compilation capability is disabled
                let Some(compiler_ref) = compiler else {
                    warn!("⚠️ Skipping Compile job {} - compilation capability disabled (COMPILATION_ENABLED=false)", job.job_id);
                    continue;
                };

                match handle_compile_job(
                    api_client,
                    compiler_ref,
                    keystore_client,
                    tdx_client,
                    &job,
                    &code_source,
                    &context,
                    &user_account_id,
                    pricing,
                    near_payment_yocto.as_ref(),
                    request_id,
                    config,
                )
                .await {
                    Ok((checksum, wasm_bytes, compile_time_ms, created_at)) => {
                        // Store in local cache for execute job (including compile time and created_at)
                        compiled_wasm = Some((checksum, wasm_bytes, compile_time_ms, created_at));
                    }
                    Err(e) => {
                        // Compilation failed - complete_job already called in handle_compile_job
                        // Coordinator will create execute task with compile_error for executor to report
                        error!("❌ Compilation failed: {}", e);
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
                    tdx_client,
                    &job,
                    &code_source,
                    &resource_limits,
                    &input_data,
                    secrets_ref.as_ref(),
                    &response_format,
                    &context,
                    user_account_id.as_ref(),
                    near_payment_yocto.as_ref(),
                    transaction_hash.as_ref(),
                    request_id,
                    &data_id,
                    compiled_wasm.as_ref().map(|(cs, b, ct, ca)| (cs, b, ct, ca.as_deref())), // Pass local WASM cache
                )
                .await?;
            }
        }
    }

    // Note: WASM upload now happens inside handle_compile_job BEFORE complete_job
    // This ensures WASM exists on coordinator when execute task is created

    Ok(true)
}

/// Merge user secrets with system environment variables
fn merge_env_vars(
    user_secrets: Option<std::collections::HashMap<String, String>>,
    context: &api_client::ExecutionContext,
    resource_limits: &api_client::ResourceLimits,
    request_id: u64,
    user_account_id: Option<&String>,
    near_payment_yocto: Option<&String>,
    transaction_hash: Option<&String>,
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
    if let Some(ref receipt_id) = context.receipt_id {
        env_vars.insert("NEAR_RECEIPT_ID".to_string(), receipt_id.clone());
    }
    if let Some(ref predecessor_id) = context.predecessor_id {
        env_vars.insert("NEAR_PREDECESSOR_ID".to_string(), predecessor_id.clone());
    }
    if let Some(ref signer_public_key) = context.signer_public_key {
        env_vars.insert("NEAR_SIGNER_PUBLIC_KEY".to_string(), signer_public_key.clone());
    }
    if let Some(gas_burnt) = context.gas_burnt {
        env_vars.insert("NEAR_GAS_BURNT".to_string(), gas_burnt.to_string());
    }

    // Add user account and payment info
    if let Some(user_id) = user_account_id {
        env_vars.insert("NEAR_USER_ACCOUNT_ID".to_string(), user_id.clone());
    }
    if let Some(payment) = near_payment_yocto {
        env_vars.insert("NEAR_PAYMENT_YOCTO".to_string(), payment.clone());
    }
    if let Some(tx_hash) = transaction_hash {
        env_vars.insert("NEAR_TRANSACTION_HASH".to_string(), tx_hash.clone());
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
    _keystore_client: Option<&KeystoreClient>,
    tdx_client: &tdx_attestation::TdxClient,
    job: &JobInfo,
    code_source: &CodeSource,
    context: &api_client::ExecutionContext,
    user_account_id: &Option<String>,
    pricing: &api_client::PricingConfig,
    user_payment: Option<&String>,
    request_id: u64,
    config: &Config,
) -> Result<(String, Vec<u8>, u64, Option<String>)> {
    info!("🔨 Starting compilation job_id={} request_id={}", job.job_id, request_id);

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
                .complete_job(job.job_id, false, None, Some(error_msg.clone()), 0, 0, None, None, None, Some(api_client::JobStatus::InsufficientPayment))
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
        Ok((checksum, wasm_bytes, created_at)) => {
            info!("✅ Compilation successful: checksum={} size={} bytes time={}ms cached={:?}",
                checksum, wasm_bytes.len(), compile_time_ms, created_at.is_some());

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

            // Upload WASM to coordinator BEFORE complete_job
            // This ensures WASM exists when coordinator creates execute task
            info!("📤 Uploading compiled WASM to coordinator...");
            if let Err(e) = api_client
                .upload_wasm(
                    checksum.clone(),
                    code_source.repo().to_string(),
                    code_source.commit().to_string(),
                    code_source.build_target().to_string(),
                    wasm_bytes.clone(),
                )
                .await
            {
                warn!("⚠️ Failed to upload WASM to coordinator: {}", e);
                // Continue anyway - complete_job will still work, but execute task may fail
            } else {
                info!("✅ WASM uploaded successfully");
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
                    None, // No error category for success
                )
                .await
            {
                warn!("⚠️ Failed to report compile job completion: {}", e);
                // Continue anyway - will upload later
            }

            // Generate and store TDX attestation
            match tdx_client.generate_task_attestation(
                "compile",
                job.job_id,
                Some(code_source.repo()),
                Some(code_source.commit()),
                Some(code_source.build_target()),
                None, // No wasm_hash for compile (we produce it)
                None, // No input_hash for compile
                &checksum, // output_hash is the compiled WASM checksum
                context.block_height,
            ).await {
                Ok(tdx_quote) => {
                    // Send attestation to coordinator
                    let attestation_request = api_client::StoreAttestationRequest {
                        task_id: job.job_id,
                        task_type: api_client::TaskType::Compile,
                        tdx_quote,
                        request_id: Some(request_id as i64),
                        caller_account_id: user_account_id.clone(),
                        transaction_hash: context.transaction_hash.clone(),
                        block_height: context.block_height,
                        repo_url: Some(code_source.repo().to_string()),
                        commit_hash: Some(code_source.commit().to_string()),
                        build_target: Some(code_source.build_target().to_string()),
                        wasm_hash: None,
                        input_hash: None,
                        output_hash: checksum.clone(),
                    };

                    if let Err(e) = api_client.store_attestation(attestation_request).await {
                        warn!("⚠️ Failed to store compilation attestation: {}", e);
                        // Non-critical - continue anyway
                    } else {
                        info!("✅ Stored compilation attestation for task_id={}", job.job_id);
                    }
                }
                Err(e) => {
                    warn!("⚠️ Failed to generate TDX attestation for compilation: {}", e);
                    // Non-critical - continue anyway
                }
            }

            Ok((checksum, wasm_bytes, compile_time_ms, created_at))
        }
        Err(e) => {
            let error_msg = e.to_string();
            warn!("❌ Compilation failed: {}", error_msg);

            // Check if this is a CompilationError with raw logs
            if let Some(comp_err) = e.downcast_ref::<compiler::CompilationError>() {
                // Store raw logs for admin debugging ONLY if enabled
                // WARNING: system_hidden_logs table should NEVER be exposed via public API
                if config.save_system_hidden_logs_to_debug {
                    if let Err(log_err) = api_client
                        .store_system_log(
                            request_id,
                            Some(job.job_id),
                            "compilation",
                            Some(comp_err.stderr.clone()),
                            Some(comp_err.stdout.clone()),
                            comp_err.exit_code,
                            None,
                        )
                        .await
                    {
                        warn!("⚠️ Failed to store compilation logs: {}", log_err);
                    }
                }
            }

            // Report failure to coordinator with detailed user-facing error message
            // The error_msg already contains safe, user-facing description from classify_compilation_error()
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
                    Some(api_client::JobStatus::CompilationFailed),
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
    tdx_client: &tdx_attestation::TdxClient,
    job: &JobInfo,
    code_source: &CodeSource,
    resource_limits: &api_client::ResourceLimits,
    input_data: &str,
    secrets_ref: Option<&api_client::SecretsReference>,
    response_format: &api_client::ResponseFormat,
    context: &api_client::ExecutionContext,
    user_account_id: Option<&String>,
    near_payment_yocto: Option<&String>,
    transaction_hash: Option<&String>,
    request_id: u64,
    data_id: &str,
    compiled_wasm: Option<(&String, &Vec<u8>, &u64, Option<&str>)>, // Local cache from compile job (checksum, bytes, compile_time_ms, created_at)
) -> Result<()> {
    info!("⚙️ Starting execution job_id={}", job.job_id);

    // Check if compilation failed - if so, just report the error to contract
    if let Some(compile_error) = &job.compile_error {
        info!("❌ Compilation failed, reporting error to contract: {}", compile_error);

        // Calculate compile cost (from job info)
        let compile_cost: u128 = job.compile_cost_yocto
            .as_ref()
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);

        // Create execution result with compilation error
        let error_result = api_client::ExecutionResult {
            success: false,
            output: None,
            error: Some(compile_error.clone()),
            execution_time_ms: 0,
            instructions: 0,
            compile_time_ms: None,
            compilation_note: Some("Compilation failed".to_string()),
        };

        let near_result = near_client
            .submit_execution_result(request_id, &error_result)
            .await;

        match near_result {
            Ok((tx_hash, _outcome)) => {
                info!("✅ Compilation error submitted to NEAR successfully: tx_hash={}", tx_hash);

                // Report to coordinator
                if let Err(e) = api_client
                    .complete_job(
                        job.job_id,
                        false,
                        None,
                        Some(compile_error.clone()),
                        0,
                        0,
                        None,
                        Some(compile_cost.to_string()),
                        None, // compile_cost already included in actual_cost
                        Some(api_client::JobStatus::CompilationFailed),
                    )
                    .await
                {
                    warn!("⚠️ Failed to report job completion: {}", e);
                }
            }
            Err(e) => {
                error!("❌ Failed to submit compilation error to contract: {}", e);
                // Report failure to coordinator
                if let Err(report_err) = api_client
                    .complete_job(
                        job.job_id,
                        false,
                        None,
                        Some(format!("Failed to submit to contract: {}", e)),
                        0,
                        0,
                        None,
                        None,
                        None,
                        Some(api_client::JobStatus::Failed),
                    )
                    .await
                {
                    warn!("⚠️ Failed to report job failure: {}", report_err);
                }
            }
        }

        return Ok(());
    }

    // Extract compile_cost from job (if compilation was done)
    let compile_cost: u128 = job.compile_cost_yocto
        .as_ref()
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);

    if compile_cost > 0 {
        info!("💰 Compile cost from compiler: {} yoctoNEAR ({:.6} NEAR)",
            compile_cost, compile_cost as f64 / 1e24);
    }

    // Get WASM checksum from job
    let wasm_checksum = job.wasm_checksum.as_ref()
        .ok_or_else(|| anyhow::anyhow!("Execute job missing wasm_checksum"))?;

    // Get WASM bytes, compile time, and creation timestamp - use local cache if available, otherwise download
    let (wasm_bytes, compile_time_ms, created_at) = if let Some((cached_checksum, cached_bytes, cached_compile_time, cached_created_at)) = compiled_wasm {
        if cached_checksum == wasm_checksum {
            info!("✅ Using locally compiled WASM: {} bytes (cache hit!) compiled in {}ms", cached_bytes.len(), cached_compile_time);
            (cached_bytes.clone(), Some(*cached_compile_time), cached_created_at.map(|s| s.to_string()))
        } else {
            warn!("⚠️ Checksum mismatch - downloading from coordinator");
            // Check cache metadata to get created_at timestamp
            let (_exists, created_at) = match api_client.wasm_exists(wasm_checksum).await {
                Ok(result) => result,
                Err(e) => {
                    let error_msg = format!("Failed to check WASM existence: {}", e);
                    error!("❌ {}", error_msg);
                    api_client.complete_job(job.job_id, false, None, Some(error_msg), 0, 0, None, None, None, Some(api_client::JobStatus::Failed)).await?;
                    return Ok(());
                }
            };
            info!("📥 Downloading WASM: checksum={} (cached since: {:?})", wasm_checksum, created_at);
            let bytes = match api_client.download_wasm(wasm_checksum).await {
                Ok(bytes) => bytes,
                Err(e) => {
                    let error_msg = format!("Failed to download WASM: {}", e);
                    error!("❌ {}", error_msg);
                    api_client.complete_job(job.job_id, false, None, Some(error_msg), 0, 0, None, None, None, Some(api_client::JobStatus::Failed)).await?;
                    return Ok(());
                }
            };
            info!("✅ Downloaded WASM: {} bytes", bytes.len());
            (bytes, None, created_at)
        }
    } else {
        // No local cache - download from coordinator
        // Check cache metadata to get created_at timestamp
        let (_exists, created_at) = match api_client.wasm_exists(wasm_checksum).await {
            Ok(result) => result,
            Err(e) => {
                let error_msg = format!("Failed to check WASM existence: {}", e);
                error!("❌ {}", error_msg);
                api_client.complete_job(job.job_id, false, None, Some(error_msg), 0, 0, None, None, None, Some(api_client::JobStatus::Failed)).await?;
                return Ok(());
            }
        };
        info!("📥 Downloading WASM: checksum={} (cached since: {:?})", wasm_checksum, &created_at);
        let bytes = match api_client.download_wasm(wasm_checksum).await {
            Ok(bytes) => bytes,
            Err(e) => {
                let error_msg = format!("Failed to download WASM: {}", e);
                error!("❌ {}", error_msg);
                api_client.complete_job(job.job_id, false, None, Some(error_msg), 0, 0, None, None, None, Some(api_client::JobStatus::Failed)).await?;
                return Ok(());
            }
        };
        info!("✅ Downloaded WASM: {} bytes, created_at={:?}", bytes.len(), &created_at);
        (bytes, None, created_at)
    };

    // Decrypt secrets from contract if provided (new repo-based system)
    info!("🔍 DEBUG secrets_ref: {:?}", secrets_ref);
    info!("🔍 DEBUG keystore_client: {}", if keystore_client.is_some() { "Some" } else { "None" });

    let user_secrets = if let (Some(secrets_ref), Some(keystore)) = (secrets_ref, keystore_client) {
        info!("🔐 Decrypting repo-based secrets: profile={}, owner={}", secrets_ref.profile, secrets_ref.account_id);

        // Get repo from code_source
        let repo = code_source.repo();

        // Resolve branch from commit via coordinator API (with caching)
        let commit = code_source.commit();
        let branch = match api_client.resolve_branch(repo, commit).await {
            Ok(b) => {
                if let Some(ref branch_name) = b {
                    info!("✅ Coordinator resolved '{}' → branch '{}'", commit, branch_name);
                } else {
                    info!("⚠️  Coordinator: commit '{}' not found, using wildcard (branch=None)", commit);
                }
                b
            }
            Err(e) => {
                warn!("⚠️  Coordinator API failed: {}, using wildcard (branch=None)", e);
                None
            }
        };

        // Call keystore to decrypt secrets from contract
        // user_account_id is the account that requested execution (used for access control)
        let caller = user_account_id.map(|s| s.as_str()).unwrap_or(&secrets_ref.account_id);
        match keystore.decrypt_secrets_from_contract(repo, branch.as_deref(), &secrets_ref.profile, &secrets_ref.account_id, caller, Some(data_id)).await {
            Ok(secrets) => {
                info!("✅ Secrets decrypted successfully: {} environment variables", secrets.len());
                Some(secrets)
            }
            Err(e) => {
                // Error message already user-friendly from keystore_client
                let error_msg = e.to_string();
                error!("❌ Secrets decryption failed: {}", error_msg);

                // Determine error category based on error message
                let error_category = if error_msg.contains("Access") && error_msg.contains("denied") {
                    api_client::JobStatus::AccessDenied
                } else if error_msg.contains("not found") || error_msg.contains("Invalid secrets format") {
                    api_client::JobStatus::Custom // Secrets not found or invalid format - user configuration issue
                } else {
                    api_client::JobStatus::Failed // Generic secret error - infrastructure issue
                };

                // Send error to NEAR contract
                let error_result = ExecutionResult {
                    success: false,
                    output: None,
                    error: Some(error_msg.clone()),
                    execution_time_ms: 0,
                    instructions: 0,
                    compile_time_ms: None,
                    compilation_note: None,
                };

                // Extract actual cost from contract logs (base_fee on failure)
                let actual_cost = match near_client.submit_execution_result(request_id, &error_result).await {
                    Ok((tx_hash, outcome)) => {
                        info!("✅ Failure reported to NEAR contract (contract panicked as expected): tx_hash={}", tx_hash);
                        let cost = NearClient::extract_payment_from_logs(&outcome);
                        if cost > 0 {
                            info!("💰 Extracted cost from contract: {} yoctoNEAR ({:.6} NEAR)",
                                cost, cost as f64 / 1e24);
                        }
                        cost
                    }
                    Err(e) => {
                        error!("❌ Failed to report failure to NEAR: {}", e);
                        0
                    }
                };

                // Fail the job in coordinator with actual cost
                api_client
                    .complete_job(
                        job.job_id,
                        false,
                        None,
                        Some(error_msg),
                        0,
                        0,
                        None,
                        if actual_cost > 0 { Some(actual_cost.to_string()) } else { None },
                        None,
                        Some(error_category)
                    )
                    .await?;
                return Ok(());
            }
        }
    } else {
        None
    };

    // Merge environment variables
    let env_vars = merge_env_vars(user_secrets, context, resource_limits, request_id, user_account_id, near_payment_yocto, transaction_hash);

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

            // Add compilation note if WASM was from cache
            execution_result.compilation_note = created_at.as_ref().map(|timestamp| {
                format!("Cached WASM from {}", timestamp)
            });

            info!("🔍 DEBUG: created_at={:?}, compilation_note={:?}", &created_at, &execution_result.compilation_note);

            if let Some(ct) = compile_time_ms {
                info!(
                    "✅ Execution successful: compile={}ms execute={}ms instructions={}",
                    ct, execution_result.execution_time_ms, execution_result.instructions
                );
            } else {
                info!(
                    "✅ Execution successful: time={}ms instructions={} (using cached WASM{})",
                    execution_result.execution_time_ms,
                    execution_result.instructions,
                    created_at.as_ref().map(|t| format!(" from {}", t)).unwrap_or_default()
                );
            }

            // Submit result to NEAR contract (critical path - highest priority)
            info!("📤 Submitting result to NEAR contract...");
            let near_result = near_client.submit_execution_result(request_id, &execution_result).await;

            // Report to coordinator (can wait, non-critical)
            match near_result {
                Ok((tx_hash, outcome)) => {
                    // Check if contract panicked (shouldn't happen for success=true!)
                    if matches!(outcome.status, near_primitives::views::FinalExecutionStatus::Failure(_)) {
                        error!("⚠️  WARNING: Contract panicked unexpectedly on successful execution! tx_hash={}", tx_hash);
                        error!("    This should NOT happen - contract should only panic on failures!");
                    } else {
                        info!("✅ Result submitted to NEAR successfully: tx_hash={}", tx_hash);
                    }

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
                            if compile_cost > 0 { Some(compile_cost.to_string()) } else { None },
                            None, // No error category for success
                        )
                        .await
                    {
                        warn!("⚠️ Failed to report execute job completion: {}", e);
                        // Continue anyway - NEAR transaction is already submitted
                    }

                    // Generate and store TDX attestation
                    {
                        // Calculate output hash (SHA256 of output)
                        use sha2::{Digest, Sha256};
                        let output_hash = if let Some(ref output) = execution_result.output {
                            let mut hasher = Sha256::new();
                            match output {
                                api_client::ExecutionOutput::Bytes(bytes) => hasher.update(bytes),
                                api_client::ExecutionOutput::Text(text) => hasher.update(text.as_bytes()),
                                api_client::ExecutionOutput::Json(json) => hasher.update(json.to_string().as_bytes()),
                            }
                            hex::encode(hasher.finalize())
                        } else {
                            "no-output".to_string()
                        };

                        // Calculate input hash
                        let mut input_hasher = Sha256::new();
                        input_hasher.update(input_data.as_bytes());
                        let input_hash = hex::encode(input_hasher.finalize());

                        match tdx_client.generate_task_attestation(
                            "execute",
                            job.job_id,
                            Some(code_source.repo()),
                            Some(code_source.commit()),
                            Some(code_source.build_target()),
                            Some(wasm_checksum),
                            Some(&input_hash),
                            &output_hash,
                            context.block_height,
                        ).await {
                            Ok(tdx_quote) => {
                                // Send attestation to coordinator
                                let attestation_request = api_client::StoreAttestationRequest {
                                    task_id: job.job_id,
                                    task_type: api_client::TaskType::Execute,
                                    tdx_quote,
                                    request_id: Some(request_id as i64),
                                    caller_account_id: user_account_id.cloned(),
                                    transaction_hash: transaction_hash.cloned(),
                                    block_height: context.block_height,
                                    repo_url: Some(code_source.repo().to_string()),
                                    commit_hash: Some(code_source.commit().to_string()),
                                    build_target: Some(code_source.build_target().to_string()),
                                    wasm_hash: Some(wasm_checksum.clone()),
                                    input_hash: Some(input_hash),
                                    output_hash,
                                };

                                if let Err(e) = api_client.store_attestation(attestation_request).await {
                                    warn!("⚠️ Failed to store execution attestation: {}", e);
                                    // Non-critical - continue anyway
                                } else {
                                    info!("✅ Stored execution attestation for task_id={}", job.job_id);
                                }
                            }
                            Err(e) => {
                                warn!("⚠️ Failed to generate TDX attestation for execution: {}", e);
                                // Non-critical - continue anyway
                            }
                        }
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
                            Some(api_client::JobStatus::Failed), // Infrastructure error - can't reach NEAR
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
                compilation_note: None,
            };

            // Submit error to NEAR contract (critical path) and extract actual cost
            let actual_cost = match near_client.submit_execution_result(request_id, &result).await {
                Ok((tx_hash, outcome)) => {
                    info!("✅ Failure reported to NEAR contract (contract panicked as expected): tx_hash={}", tx_hash);
                    let cost = NearClient::extract_payment_from_logs(&outcome);
                    if cost > 0 {
                        info!("💰 Extracted cost from contract: {} yoctoNEAR ({:.6} NEAR)",
                            cost, cost as f64 / 1e24);
                    }
                    cost
                }
                Err(submit_err) => {
                    error!("❌ Failed to report failure to NEAR: {}", submit_err);
                    0
                }
            };

            // Report to coordinator with actual cost (non-critical)
            if let Err(report_err) = api_client
                .complete_job(
                    job.job_id,
                    false,
                    None,
                    Some(error_msg.clone()),
                    0,
                    0,
                    None,
                    if actual_cost > 0 { Some(actual_cost.to_string()) } else { None },
                    None,
                    Some(api_client::JobStatus::ExecutionFailed), // WASM execution error (panic, trap, timeout)
                )
                .await
            {
                warn!("⚠️ Failed to report execute job failure: {}", report_err);
            }

            return Err(e);
        }
    }

    Ok(())
}

// Tests moved to coordinator/src/handlers/github.rs
// Branch resolution is now done via coordinator API with Redis caching

/// Send startup attestation to coordinator using pre-generated TDX quote
///
/// This registers the worker with the coordinator and updates its RTMR3 measurement.
/// Called ONCE on worker startup after successful key registration.
/// If this fails, the worker will stop (no retries).
///
/// # Arguments
/// * `tdx_quote_hex` - Pre-generated TDX quote from registration (hex-encoded)
async fn send_startup_attestation_with_quote(
    api_client: &ApiClient,
    tdx_quote_hex: &str,
    config: &Config,
) -> Result<()> {
    use api_client::{StoreAttestationRequest, TaskType};

    info!("Using TDX quote from registration (length: {} bytes)", tdx_quote_hex.len() / 2);

    // Create attestation request with pre-generated quote
    let request = StoreAttestationRequest {
        task_id: -1,
        task_type: TaskType::Execute, // Use Execute type for startup
        tdx_quote: tdx_quote_hex.to_string(),
        request_id: None,
        caller_account_id: None,
        transaction_hash: None,
        block_height: None,
        repo_url: Some(format!("worker://{}", config.worker_id)),
        commit_hash: Some("startup".to_string()),
        build_target: Some(config.tee_mode.clone()),
        wasm_hash: None,
        input_hash: None, // Not required for startup attestation (task_id = -1)
        output_hash: "worker_startup".to_string(),
    };

    // Send to coordinator (fail fast - no retries)
    api_client
        .store_attestation(request)
        .await
        .context("Failed to store startup attestation")?;

    Ok(())
}
