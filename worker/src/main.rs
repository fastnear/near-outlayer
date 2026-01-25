mod api_client;
mod collateral_fetcher;
mod compiled_cache;
mod compiler;
mod config;
mod event_monitor;
mod executor;
mod fastfs;
mod keystore_client;
mod near_client;
mod registration;
mod outlayer_rpc;
mod outlayer_storage;
mod outlayer_payment;
mod tdx_attestation;
mod wasm_cache;

use anyhow::{Context, Result};
use std::sync::{Arc, Mutex};
use tracing::{debug, error, info, warn};

use api_client::{ApiClient, CodeSource, ExecutionResult, JobInfo, JobStatus, JobType};
use compiled_cache::CompiledCache;
use wasm_cache::WasmCache;
use collateral_fetcher::fetch_collateral_from_phala;
use compiler::Compiler;
use config::Config;
use event_monitor::EventMonitor;
use executor::{Executor, ExecutionContext};
use keystore_client::KeystoreClient;
use near_client::NearClient;
use outlayer_storage::StorageConfig;
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

    // Initialize WASM cache (if enabled)
    let wasm_cache = if config.wasm_cache_max_size_mb > 0 {
        // Determine cache directory - try configured dir first, fall back to /tmp if not writable
        let cache_dir = {
            let configured_dir = std::path::PathBuf::from(&config.wasm_cache_dir);

            // Try to create and write to configured directory
            let configured_ok = std::fs::create_dir_all(&configured_dir).is_ok() && {
                let test_file = configured_dir.join(".write_test");
                let write_ok = std::fs::write(&test_file, b"test").is_ok();
                let _ = std::fs::remove_file(&test_file);
                write_ok
            };

            if configured_ok {
                info!("📦 WASM cache directory: {} (writable)", configured_dir.display());
                configured_dir
            } else {
                // Fall back to /tmp subdirectory (TEE environment like Phala)
                // Security note: WASI P2 has access to /tmp, but cache has checksum verification
                // so tampering would be detected. In TEE only /tmp is writable.
                let fallback_dir = std::path::PathBuf::from("/tmp/outlayer-wasm-cache");
                warn!(
                    "⚠️  Configured cache directory '{}' is not writable, falling back to '{}'",
                    configured_dir.display(),
                    fallback_dir.display()
                );
                warn!("   Note: /tmp fallback is less secure - WASI has access to /tmp");
                warn!("   Cache integrity is protected by checksum verification.");
                fallback_dir
            }
        };

        info!("📦 WASM cache enabled: max_size={}MB, dir={}", config.wasm_cache_max_size_mb, cache_dir.display());
        let cache = WasmCache::new(
            cache_dir,
            config.wasm_cache_max_size_mb,
        ).context("Failed to initialize WASM cache")?;
        Some(Arc::new(Mutex::new(cache)))
    } else {
        info!("📦 WASM cache disabled (WASM_CACHE_MAX_SIZE_MB=0)");
        None
    };

    // Initialize compiler (only if compilation capability enabled)
    let compiler = if config.capabilities.can_compile() {
        info!("✅ Compilation capability enabled - initializing compiler");
        Some(Compiler::new(api_client.clone(), config.clone())
            .context("Failed to create compiler")?)
    } else {
        info!("⚠️  Compilation capability disabled - will only handle Execute jobs");
        None
    };

    // Initialize RPC proxy if enabled
    let rpc_proxy = if config.rpc_proxy.enabled {
        info!("🔧 Initializing NEAR RPC proxy...");
        let proxy = outlayer_rpc::RpcProxy::new(
            config.rpc_proxy.clone(),
            &config.near_rpc_url,
        )?;
        info!("✅ RPC proxy initialized: {}", proxy.get_rpc_url_masked());
        Some(proxy)
    } else {
        info!("⚠️  RPC proxy disabled - WASM modules cannot make NEAR RPC calls");
        None
    };

    // NOTE: Executor creation moved to after registration (needs secret key for compiled cache)

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

    // Initialize compiled cache (requires secret key from registration/config)
    // This caches pre-compiled wasmtime components for ~10x faster WASM startup
    let compiled_cache: Option<Arc<Mutex<CompiledCache>>> = if config.wasm_cache_max_size_mb > 0 {
        // Get secret key bytes from operator signer
        let secret_key = &config.get_operator_signer().secret_key;
        let secret_key_bytes: [u8; 32] = match secret_key {
            near_crypto::SecretKey::ED25519(ed_key) => {
                // ED25519 secret key is 64 bytes (seed + public), we need first 32 (seed)
                let mut bytes = [0u8; 32];
                bytes.copy_from_slice(&ed_key.0[..32]);
                bytes
            }
            _ => {
                warn!("⚠️ Compiled cache requires ED25519 key, skipping");
                [0u8; 32] // Won't be used
            }
        };

        // Only create cache if we have a valid key
        if secret_key_bytes != [0u8; 32] {
            let compiled_cache_dir = std::path::PathBuf::from(&config.wasm_cache_dir).join("compiled");
            match CompiledCache::new(compiled_cache_dir.clone(), config.wasm_cache_max_size_mb, &secret_key_bytes) {
                Ok(cache) => {
                    info!("⚡ Compiled cache enabled: dir={}, max_size={}MB",
                        compiled_cache_dir.display(), config.wasm_cache_max_size_mb);
                    Some(Arc::new(Mutex::new(cache)))
                }
                Err(e) => {
                    warn!("⚠️ Failed to initialize compiled cache: {}", e);
                    None
                }
            }
        } else {
            None
        }
    } else {
        info!("⚡ Compiled cache disabled (WASM_CACHE_MAX_SIZE_MB=0)");
        None
    };

    // Initialize executor with RPC proxy and compiled cache
    let executor = {
        let runtime_handle = tokio::runtime::Handle::current();
        let mut exec_context = ExecutionContext::new(runtime_handle);

        if let Some(proxy) = rpc_proxy {
            exec_context = exec_context.with_outlayer_rpc(proxy);
        }

        if let Some(ref cache) = compiled_cache {
            exec_context = exec_context.with_compiled_cache(cache.clone());
        }

        Executor::new(config.default_max_instructions, config.print_wasm_stderr)
            .with_context(exec_context)
    };

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
        let event_filter_standard_name = config.event_filter_standard_name.clone();
        let event_filter_function_name = config.event_filter_function_name.clone();
        let event_filter_min_version = config.event_filter_min_version.clone();

        tokio::spawn(async move {
            info!("Starting event monitor...");
            match EventMonitor::new(
                event_api_client,
                neardata_url,
                fastnear_url,
                contract_id,
                start_block,
                scan_interval_ms,
                event_filter_standard_name,
                event_filter_function_name,
                event_filter_min_version,
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

    // Start Contract System Callbacks Handler
    // This handles contract business logic that requires yield/resume (TopUp, Delete, etc.)
    // Separated from main worker loop to avoid blocking WASM execution tasks
    // Only workers with "execution" capability should poll system callbacks
    if config.capabilities.to_array().contains(&"execution".to_string()) {
        let callbacks_api_client = api_client.clone();
        let callbacks_keystore_client = keystore_client.clone();
        let callbacks_near_client = near_client.clone();
        let callbacks_capabilities = config.capabilities.to_array();

        tokio::spawn(async move {
            run_contract_system_callbacks_handler(
                callbacks_api_client,
                callbacks_keystore_client,
                callbacks_near_client,
                callbacks_capabilities,
            )
            .await;
        });
        info!("📋 Contract System Callbacks Handler started");
    } else {
        info!("📋 Contract System Callbacks Handler skipped (no 'execution' capability)");
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
            wasm_cache.as_ref(),
            compiled_cache.as_ref(),
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
    wasm_cache: Option<&Arc<Mutex<WasmCache>>>,
    compiled_cache: Option<&Arc<Mutex<CompiledCache>>>,
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

    info!("📨 Received execution request: request_id={} project_uuid={:?} project_id={:?}",
        execution_request.request_id, execution_request.project_uuid, execution_request.project_id);

    // Extract request details
    let request_id = execution_request.request_id;
    let data_id = execution_request.data_id.clone();
    let resource_limits = execution_request.resource_limits.clone();
    let input_data = execution_request.input_data.clone();
    let secrets_ref = execution_request.secrets_ref.clone();
    let response_format = execution_request.response_format.clone();
    let context = execution_request.context.clone();
    let user_account_id = execution_request.user_account_id.clone();
    let near_payment_yocto = execution_request.near_payment_yocto.clone();
    let attached_usd = execution_request.attached_usd.clone();
    let transaction_hash = context.transaction_hash.clone();
    let store_on_fastfs = execution_request.store_on_fastfs;
    let compile_only = execution_request.compile_only;
    let force_rebuild = execution_request.force_rebuild;
    let compile_result = execution_request.compile_result.clone();
    let is_https_call = execution_request.is_https_call;
    let call_id = execution_request.call_id.clone();
    let payment_key_owner = execution_request.payment_key_owner.clone();
    let payment_key_nonce = execution_request.payment_key_nonce;
    let usd_payment = execution_request.usd_payment.clone();

    /// Result of resolving project: code_source + project_uuid
    struct ResolvedProject {
        code_source: api_client::CodeSource,
        project_uuid: String,
    }

    // Helper to resolve code_source from project_id
    async fn resolve_code_source_from_project(
        near_client: &near_client::NearClient,
        project_id: &str,
    ) -> Result<ResolvedProject> {
        info!("📦 Resolving code_source from project_id: {}", project_id);

        // Fetch project from contract
        let project = near_client.fetch_project(project_id).await?
            .ok_or_else(|| anyhow::anyhow!("Project not found: {}", project_id))?;

        let project_uuid = project.uuid.clone();

        // Fetch version info
        let version_view = near_client.fetch_project_version(project_id, &project.active_version).await?
            .ok_or_else(|| anyhow::anyhow!("Project version not found: {} @ {}", project_id, project.active_version))?;

        // Convert contract's CodeSource to worker's api_client::CodeSource
        let code_source = match version_view.source {
            near_client::ContractCodeSource::GitHub { repo, commit, build_target } => {
                let build_target = build_target.unwrap_or_else(|| "wasm32-wasip1".to_string());
                info!("✅ Resolved code_source: repo={} commit={} target={}", repo, commit, build_target);
                api_client::CodeSource::GitHub { repo, commit, build_target }
            }
            near_client::ContractCodeSource::WasmUrl { url, hash, build_target } => {
                let build_target = build_target.unwrap_or_else(|| "wasm32-wasip1".to_string());
                info!("✅ Resolved code_source: url={} hash={} target={}", url, hash, build_target);
                api_client::CodeSource::WasmUrl { url, hash, build_target }
            }
        };

        Ok(ResolvedProject { code_source, project_uuid })
    }

    // Resolve code_source: either from request directly, or from project_id via contract
    // Also resolve project_uuid if resolving from project
    // Always normalize to ensure repo URL has https:// prefix
    let (code_source, resolved_project_uuid): (api_client::CodeSource, Option<String>) = match execution_request.code_source.clone() {
        Some(cs) => (cs.normalize(), None), // code_source provided directly, no uuid from resolution
        None => {
            // No code_source - resolve from project_id (HTTPS API flow)
            let project_id = execution_request.project_id.as_ref()
                .ok_or_else(|| anyhow::anyhow!("No code_source and no project_id in request"))?;

            match resolve_code_source_from_project(near_client, project_id).await {
                Ok(resolved) => {
                    info!("✅ Resolved project_uuid={} from project_id={}", resolved.project_uuid, project_id);
                    (resolved.code_source.normalize(), Some(resolved.project_uuid))
                }
                Err(e) => {
                    // If this is an HTTPS call, report the error back to coordinator
                    if is_https_call {
                        if let Some(ref cid) = call_id {
                            error!("❌ Failed to resolve project for HTTPS call {}: {}", cid, e);
                            if let Err(report_err) = api_client.complete_https_call(
                                cid,
                                false,
                                None,
                                Some(format!("Failed to resolve project: {}", e)),
                                0,
                                0,
                            ).await {
                                error!("❌ Failed to report HTTPS call error: {}", report_err);
                            }
                        }
                    }
                    return Err(e);
                }
            }
        }
    };

    // Claim jobs for this task with worker capabilities
    let has_compile_result = compile_result.is_some();

    // Log if task appears to be misrouted (useful for debugging)
    if force_rebuild && !has_compile_result && !config.capabilities.can_compile() {
        warn!("⚠️ Task with force_rebuild=true but no compile_result routed to executor - may be waiting for compiler");
    }

    // Get project_uuid: prefer resolved from contract, fallback to execution_request
    let project_uuid = resolved_project_uuid.or(execution_request.project_uuid.clone());
    let project_id = execution_request.project_id.clone();

    info!("🎯 Claiming jobs for request_id={} data_id={} with capabilities={:?} compile_only={} force_rebuild={} has_compile_result={} project_uuid={:?}",
          request_id, data_id, config.capabilities.to_array(), compile_only, force_rebuild, has_compile_result, project_uuid);
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
            compile_only,
            force_rebuild,
            has_compile_result,
            project_uuid,
            project_id,
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
    let mut compiled_wasm: Option<(String, Vec<u8>, u64, Option<String>, Option<String>)> = None; // (checksum, bytes, compile_time_ms, created_at, published_url)

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
                    near_client,
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
                    store_on_fastfs,
                    force_rebuild,
                    compile_only,
                )
                .await {
                    Ok((checksum, wasm_bytes, compile_time_ms, created_at, published_url)) => {
                        // Store in local cache for execute job (including compile time, created_at, and published_url)
                        compiled_wasm = Some((checksum, wasm_bytes, compile_time_ms, created_at, published_url));
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
                    attached_usd.as_ref(),
                    transaction_hash.as_ref(),
                    request_id,
                    &data_id,
                    compiled_wasm.as_ref().map(|(cs, b, ct, ca, pu)| (cs, b, ct, ca.as_deref(), pu.as_deref())), // Pass local WASM cache with published_url
                    compile_result.as_ref(), // Pass compile_result (published_url or result for compile_only)
                    compile_only,
                    config.use_tee_registration,
                    config,
                    is_https_call,
                    call_id.as_ref(),
                    payment_key_owner.as_ref(),
                    payment_key_nonce,
                    usd_payment.as_ref(),
                    wasm_cache,
                    compiled_cache,
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
///
/// For HTTPS calls, blockchain-related env vars are set to empty strings
/// and additional HTTPS-specific vars are added (OUTLAYER_EXECUTION_TYPE, USD_PAYMENT, etc.)
fn merge_env_vars(
    user_secrets: Option<std::collections::HashMap<String, String>>,
    context: &api_client::ExecutionContext,
    resource_limits: &api_client::ResourceLimits,
    request_id: u64,
    user_account_id: Option<&String>,
    near_payment_yocto: Option<&String>,
    attached_usd: Option<&String>,
    transaction_hash: Option<&String>,
    project_id: Option<&String>,
    project_uuid: Option<&String>,
    // HTTPS-specific parameters
    is_https_call: bool,
    call_id: Option<&String>,
    payment_key_owner: Option<&String>,
    usd_payment: Option<&String>,
    // Network configuration
    near_rpc_url: &str,
) -> std::collections::HashMap<String, String> {

    let mut env_vars = user_secrets.unwrap_or_default();

    // Determine network from RPC URL
    let network_id = if near_rpc_url.contains("mainnet") { "mainnet" } else { "testnet" };
    env_vars.insert("NEAR_NETWORK_ID".to_string(), network_id.to_string());

    // Set execution type
    env_vars.insert(
        "OUTLAYER_EXECUTION_TYPE".to_string(),
        if is_https_call { "HTTPS".to_string() } else { "NEAR".to_string() }
    );

    if is_https_call {
        // HTTPS mode: use payment key owner as sender, set blockchain vars to empty

        // NEAR_SENDER_ID = Payment Key owner
        if let Some(owner) = payment_key_owner {
            env_vars.insert("NEAR_SENDER_ID".to_string(), owner.clone());
            env_vars.insert("NEAR_USER_ACCOUNT_ID".to_string(), owner.clone());
        }

        // Blockchain vars = empty strings (not available for HTTPS)
        env_vars.insert("NEAR_CONTRACT_ID".to_string(), "".to_string());
        env_vars.insert("NEAR_BLOCK_HEIGHT".to_string(), "".to_string());
        env_vars.insert("NEAR_BLOCK_TIMESTAMP".to_string(), "".to_string());
        env_vars.insert("NEAR_RECEIPT_ID".to_string(), "".to_string());
        env_vars.insert("NEAR_PREDECESSOR_ID".to_string(), "".to_string());
        env_vars.insert("NEAR_SIGNER_PUBLIC_KEY".to_string(), "".to_string());
        env_vars.insert("NEAR_GAS_BURNT".to_string(), "".to_string());
        env_vars.insert("NEAR_TRANSACTION_HASH".to_string(), "".to_string());
        env_vars.insert("NEAR_REQUEST_ID".to_string(), "".to_string());

        // HTTPS has no NEAR payment or attached deposit
        env_vars.insert("NEAR_PAYMENT_YOCTO".to_string(), "0".to_string());
        env_vars.insert("ATTACHED_USD".to_string(), "0".to_string());

        // HTTPS-specific: USD payment to project owner
        env_vars.insert(
            "USD_PAYMENT".to_string(),
            usd_payment.cloned().unwrap_or_else(|| "0".to_string())
        );

        // HTTPS-specific: call ID (UUID)
        if let Some(cid) = call_id {
            env_vars.insert("OUTLAYER_CALL_ID".to_string(), cid.clone());
        } else {
            env_vars.insert("OUTLAYER_CALL_ID".to_string(), "".to_string());
        }
    } else {
        // NEAR transaction mode: use context values

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
        if let Some(deposit) = attached_usd {
            env_vars.insert("ATTACHED_USD".to_string(), deposit.clone());
        } else {
            env_vars.insert("ATTACHED_USD".to_string(), "0".to_string());
        }
        if let Some(tx_hash) = transaction_hash {
            env_vars.insert("NEAR_TRANSACTION_HASH".to_string(), tx_hash.clone());
        }

        // Add request ID
        env_vars.insert("NEAR_REQUEST_ID".to_string(), request_id.to_string());

        // NEAR mode doesn't have USD payment or call_id
        env_vars.insert("USD_PAYMENT".to_string(), "0".to_string());
        env_vars.insert("OUTLAYER_CALL_ID".to_string(), "".to_string());
    }

    // Add resource limits (same for both modes)
    env_vars.insert("NEAR_MAX_INSTRUCTIONS".to_string(), resource_limits.max_instructions.to_string());
    env_vars.insert("NEAR_MAX_MEMORY_MB".to_string(), resource_limits.max_memory_mb.to_string());
    env_vars.insert("NEAR_MAX_EXECUTION_SECONDS".to_string(), resource_limits.max_execution_seconds.to_string());

    // Add project context (same for both modes)
    if let Some(proj_id) = project_id {
        env_vars.insert("OUTLAYER_PROJECT_ID".to_string(), proj_id.clone());
        // Split by first '/' only (project name may contain '/')
        if let Some(slash_pos) = proj_id.find('/') {
            let owner = &proj_id[..slash_pos];
            let name = &proj_id[slash_pos + 1..];
            env_vars.insert("OUTLAYER_PROJECT_OWNER".to_string(), owner.to_string());
            env_vars.insert("OUTLAYER_PROJECT_NAME".to_string(), name.to_string());
        }
    }
    if let Some(proj_uuid) = project_uuid {
        env_vars.insert("OUTLAYER_PROJECT_UUID".to_string(), proj_uuid.clone());
    }

    env_vars
}

/// Fetch WASM bytes from coordinator or local cache
///
/// For P1: uses WasmCache (raw bytes LRU cache)
/// For P2: downloads directly (CompiledCache handles caching in executor)
async fn fetch_wasm_bytes(
    api_client: &ApiClient,
    wasm_checksum: &str,
    wasm_cache: Option<&Arc<Mutex<WasmCache>>>,
    is_p2: bool,
    created_at: &Option<String>,
) -> Result<Vec<u8>> {
    // P1: try WasmCache first (raw bytes cache)
    if !is_p2 {
        if let Some(cache) = wasm_cache {
            if let Some(cached_bytes) = cache.lock().ok().and_then(|mut c| c.get(wasm_checksum)) {
                info!("✅ WASM LRU cache hit (P1): {} ({}KB)", wasm_checksum, cached_bytes.len() / 1024);
                return Ok(cached_bytes);
            }
        }
    }

    // Download from coordinator
    info!("📥 Downloading WASM: checksum={} (cached since: {:?}) is_p2={}", wasm_checksum, created_at, is_p2);
    let bytes = api_client.download_wasm(wasm_checksum).await
        .map_err(|e| {
            error!("❌ Failed to download WASM: {}", e);
            anyhow::anyhow!("Failed to download WASM: {}", e)
        })?;

    info!("✅ Downloaded WASM: {} bytes", bytes.len());
    Ok(bytes)
}

/// Handle a compile job
/// Returns (checksum, wasm_bytes, compile_time_ms) for local caching
async fn handle_compile_job(
    api_client: &ApiClient,
    compiler: &Compiler,
    _near_client: &NearClient,
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
    store_on_fastfs: bool,
    force_rebuild: bool,
    _compile_only: bool,
) -> Result<(String, Vec<u8>, u64, Option<String>, Option<String>)> {
    // Returns (checksum, wasm_bytes, compile_time_ms, created_at, published_url)
    info!("🔨 Starting compilation job_id={} request_id={}", job.job_id, request_id);

    // Check if this is a WasmUrl source - if so, download instead of compile
    if let CodeSource::WasmUrl { url, hash, build_target } = code_source {
        info!("📥 WasmUrl source detected - downloading from URL instead of compiling");
        info!("   URL: {}", url);
        info!("   Hash: {}", hash);

        let start_time = std::time::Instant::now();

        // Download and cache WASM
        let result = compiler.download_and_cache_wasm(url, hash, build_target).await;
        let download_time_ms = start_time.elapsed().as_millis() as u64;

        match result {
            Ok((checksum, wasm_bytes, created_at)) => {
                info!("✅ WASM downloaded: checksum={} size={} bytes time={}ms cached={:?}",
                    checksum, wasm_bytes.len(), download_time_ms, created_at.is_some());

                // Report completion to coordinator
                // Note: For downloads, we report 0 compile_cost (no compilation happened)
                if let Err(e) = api_client
                    .complete_job(
                        job.job_id,
                        true,
                        None,
                        None,
                        download_time_ms,
                        0, // No instructions for download
                        Some(checksum.clone()),
                        None, // No actual_cost for download jobs
                        Some("0".to_string()), // Zero compile cost - just download
                        None, // No error category for success
                        None, // No compile_result
                    )
                    .await
                {
                    warn!("⚠️ Failed to report download job completion: {}", e);
                }

                return Ok((checksum, wasm_bytes, download_time_ms, created_at, None)); // No FastFS for WasmUrl downloads
            }
            Err(e) => {
                let error_msg = e.to_string();
                warn!("❌ WASM download failed: {}", error_msg);

                // Report failure to coordinator
                if let Err(report_err) = api_client
                    .complete_job(
                        job.job_id,
                        false,
                        None,
                        Some(error_msg.clone()),
                        download_time_ms,
                        0,
                        None,
                        None,
                        None,
                        Some(api_client::JobStatus::CompilationFailed), // Reuse status for download failures
                        None, // No compile_result
                    )
                    .await
                {
                    warn!("⚠️ Failed to report download job failure: {}", report_err);
                }

                return Err(e);
            }
        }
    }

    // Extract GitHub fields - compile jobs for GitHub source
    let (repo, commit, build_target) = match code_source {
        CodeSource::GitHub { repo, commit, build_target } => {
            // If commit is empty, use "main" as default
            let commit_str = if commit.is_empty() {
                info!("⚠️ Commit is empty, using 'main' as default branch");
                "main"
            } else {
                commit.as_str()
            };
            (repo.as_str(), commit_str, build_target.as_str())
        },
        CodeSource::WasmUrl { .. } => unreachable!("WasmUrl handled above"),
    };

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
                .complete_job(job.job_id, false, None, Some(error_msg.clone()), 0, 0, None, None, None, Some(api_client::JobStatus::InsufficientPayment), None)
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
    let compile_result = compiler.compile_local_with_options(code_source, timeout_seconds, force_rebuild).await;
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
                    repo.to_string(),
                    commit.to_string(),
                    build_target.to_string(),
                    wasm_bytes.clone(),
                )
                .await
            {
                warn!("⚠️ Failed to upload WASM to coordinator: {}", e);
                // Continue anyway - complete_job will still work, but execute task may fail
            } else {
                info!("✅ WASM uploaded successfully");
            }

            // Upload to FastFS if requested (before complete_job to include result)
            let mut compile_result_for_executor: Option<String> = None;
            let mut published_url: Option<String> = None;

            if store_on_fastfs {
                if let Some(ref fastfs_receiver) = config.fastfs_receiver {
                    // Use dedicated FastFS sender if configured, otherwise use operator
                    let signer = config.fastfs_sender_signer.clone()
                        .unwrap_or_else(|| config.get_operator_signer().clone());

                    info!("📦 Uploading compiled WASM to FastFS...");
                    info!("   Sender: {}", signer.account_id);
                    info!("   Receiver: {}", fastfs_receiver);

                    let fastfs_client = fastfs::FastFsClient::new(
                        &config.near_rpc_url,
                        signer.clone(),
                        fastfs_receiver,
                    );

                    // Build the FastFS URL (same format regardless of transaction success)
                    let fastfs_url = format!(
                        "https://{}.fastfs.io/{}/{}.wasm",
                        signer.account_id,
                        fastfs_receiver,
                        checksum
                    );

                    match fastfs_client.upload_wasm(&wasm_bytes, &checksum).await {
                        Ok(url) => {
                            info!("✅ FastFS upload successful: {}", url);
                        }
                        Err(_) => {
                            // FastFS transaction "fails" but indexer picks up the file - this is expected
                            // The info message was already logged in fastfs.rs
                            info!("📁 FastFS URL: {}", fastfs_url);
                        }
                    }

                    // Always save published URL for compilation_note
                    published_url = Some(fastfs_url.clone());

                    // Always pass the URL to executor via compile_result
                    // For compile_only: executor sends URL to contract as result
                    // For normal: executor uses URL in compilation_note
                    info!("📤 Setting compile_result for executor: {}", fastfs_url);
                    compile_result_for_executor = Some(fastfs_url);
                } else {
                    warn!("⚠️ store_on_fastfs=true but FASTFS_RECEIVER not configured, skipping upload");
                }
            }

            // Report completion to coordinator
            // If compile_result is set, coordinator will create execute task for executor to send result
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
                    compile_result_for_executor, // Pass FastFS URL to executor
                )
                .await
            {
                warn!("⚠️ Failed to report compile job completion: {}", e);
                // Continue anyway - will upload later
            }

            // Generate and store TDX attestation only if TEE registration is enabled
            if config.use_tee_registration {
                match tdx_client.generate_task_attestation(
                    "compile",
                    job.job_id,
                    Some(repo),
                    Some(commit),
                    Some(build_target),
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
                            // HTTPS call context - None for NEAR calls
                            call_id: None,
                            payment_key_owner: None,
                            payment_key_nonce: None,
                            repo_url: Some(repo.to_string()),
                            commit_hash: Some(commit.to_string()),
                            build_target: Some(build_target.to_string()),
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
            } else {
                debug!("Skipping attestation generation (USE_TEE_REGISTRATION=false)");
            }

            Ok((checksum, wasm_bytes, compile_time_ms, created_at, published_url))
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
                    None, // No compile_result
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
#[allow(clippy::too_many_arguments)]
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
    attached_usd: Option<&String>,
    transaction_hash: Option<&String>,
    request_id: u64,
    data_id: &str,
    compiled_wasm: Option<(&String, &Vec<u8>, &u64, Option<&str>, Option<&str>)>, // Local cache from compile job (checksum, bytes, compile_time_ms, created_at, published_url)
    compile_result: Option<&String>, // Result from compile job (published_url or result for compile_only)
    compile_only: bool,
    use_tee_registration: bool,
    config: &Config, // For storage config
    is_https_call: bool, // HTTPS API call - skip NEAR contract, call coordinator
    call_id: Option<&String>, // HTTPS call ID for coordinator completion
    payment_key_owner: Option<&String>, // Payment Key owner for HTTPS calls
    payment_key_nonce: Option<i32>, // Payment Key nonce for HTTPS calls
    usd_payment: Option<&String>, // USD payment amount for HTTPS calls
    wasm_cache: Option<&Arc<Mutex<WasmCache>>>, // Local WASM LRU cache (P1 only)
    compiled_cache: Option<&Arc<Mutex<CompiledCache>>>, // Compiled component cache (P2 only)
) -> Result<()> {
    info!("⚙️ Starting execution job_id={}", job.job_id);

    // Extract published_url from compile_result (if set by compiler)
    // For compile_only=true: send as result, for compile_only=false: use in compilation_note
    let published_url_from_compile_result = compile_result.cloned();

    // Check if this is a result-only task (compile_only=true)
    // Compiler passed the result (e.g., FastFS URL or wasm_hash) for executor to send to contract
    if compile_only && compile_result.is_some() {
        let result_to_send = compile_result.unwrap();
        info!("📤 Result-only execute task: sending compile_result to contract: {}", result_to_send);

        // Use consistent "Published to" format for URLs
        let compilation_note = if result_to_send.starts_with("http") {
            format!("Published to {}", result_to_send)
        } else {
            format!("Result from compilation: {}", result_to_send)
        };

        let result = api_client::ExecutionResult {
            success: true,
            output: Some(api_client::ExecutionOutput::Text(result_to_send.clone())),
            error: None,
            execution_time_ms: 0, // No execution
            instructions: 0, // No execution
            compile_time_ms: None, // Already counted in compile job
            compilation_note: Some(compilation_note),
            refund_usd: None,
        };

        match near_client.submit_execution_result(request_id, &result).await {
            Ok((tx_hash, outcome)) => {
                info!("✅ Compile result submitted to NEAR successfully: tx_hash={}", tx_hash);

                // Extract actual cost from contract logs
                let actual_cost = NearClient::extract_payment_from_logs(&outcome);
                if actual_cost > 0 {
                    info!("💰 Extracted cost from contract: {} yoctoNEAR ({:.6} NEAR)",
                        actual_cost, actual_cost as f64 / 1e24);
                }

                // Report success to coordinator
                if let Err(e) = api_client
                    .complete_job(
                        job.job_id,
                        true,
                        Some(api_client::ExecutionOutput::Text(result_to_send.clone())),
                        None,
                        0, // No execution time
                        0, // No instructions
                        None,
                        if actual_cost > 0 { Some(actual_cost.to_string()) } else { None },
                        None, // compile_cost already reported by compile job
                        None, // No error category for success
                        None, // No compile_result to pass on
                    )
                    .await
                {
                    warn!("⚠️ Failed to report execute job completion: {}", e);
                }
            }
            Err(e) => {
                error!("❌ Failed to submit compile result to contract: {}", e);

                // Report failure to coordinator
                if let Err(report_err) = api_client
                    .complete_job(
                        job.job_id,
                        false,
                        None,
                        Some(format!("Failed to submit result to contract: {}", e)),
                        0,
                        0,
                        None,
                        None,
                        None,
                        Some(api_client::JobStatus::Failed),
                        None,
                    )
                    .await
                {
                    warn!("⚠️ Failed to report execute job failure: {}", report_err);
                }
            }
        }

        return Ok(());
    }

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
            refund_usd: None,
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
                        None, // No compile_result
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
                        None, // No compile_result
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

    // Get build target early to decide caching strategy
    // P2 uses CompiledCache (native code), P1 uses WasmCache (raw bytes)
    let build_target = match code_source {
        CodeSource::GitHub { build_target, .. } => build_target.as_str(),
        CodeSource::WasmUrl { build_target, .. } => build_target.as_str(),
    };
    let is_p2 = build_target == "wasm32-wasip2";

    // For P2: validate CompiledCache entry BEFORE downloading raw bytes
    // validate_entry() checks: files exist + signature valid
    // If valid, skip download - executor will load from compiled cache
    // If invalid (missing files, bad signature), entry is removed - must download WASM
    let compiled_cache_valid = if is_p2 {
        compiled_cache
            .and_then(|c| c.lock().ok())
            .map(|mut c| c.validate_entry(wasm_checksum))
            .unwrap_or(false)
    } else {
        false
    };

    // Get WASM bytes, compile time, creation timestamp, published URL
    // P2 + valid compiled cache: skip download (executor loads from cache)
    // P2 + invalid/no cache: download from coordinator
    // P1: use WasmCache (raw bytes LRU)
    let (wasm_bytes, compile_time_ms, created_at, published_url) = if let Some((cached_checksum, cached_bytes, cached_compile_time, cached_created_at, cached_published_url)) = compiled_wasm {
        // Local compile cache from same execution (freshly compiled)
        if cached_checksum == wasm_checksum {
            info!("✅ Using locally compiled WASM: {} bytes (freshly compiled!) compiled in {}ms", cached_bytes.len(), cached_compile_time);
            (cached_bytes.clone(), Some(*cached_compile_time), cached_created_at.map(|s| s.to_string()), cached_published_url.map(|s| s.to_string()))
        } else {
            warn!("⚠️ Checksum mismatch - need to fetch WASM");
            let created_at = api_client.wasm_exists(wasm_checksum).await.ok().and_then(|(_, ca)| ca);
            match fetch_wasm_bytes(api_client, wasm_checksum, wasm_cache, is_p2, &created_at).await {
                Ok(bytes) => (bytes, job.compile_time_ms, created_at, None),
                Err(e) => {
                    let error_msg = format!("Failed to download WASM: {}", e);
                    error!("❌ {}", error_msg);
                    api_client.complete_job(job.job_id, false, None, Some(error_msg), 0, 0, None, None, None, Some(api_client::JobStatus::Failed), None).await?;
                    return Ok(());
                }
            }
        }
    } else if compiled_cache_valid {
        // P2 compiled cache is valid - skip download
        info!("⚡ CompiledCache valid for {} - skipping WASM download", wasm_checksum);
        let created_at = api_client.wasm_exists(wasm_checksum).await.ok().and_then(|(_, ca)| ca);
        (Vec::new(), job.compile_time_ms, created_at, None)
    } else {
        // Need to fetch WASM bytes (P1, or P2 with no/invalid cache)
        let created_at = api_client.wasm_exists(wasm_checksum).await.ok().and_then(|(_, ca)| ca);
        match fetch_wasm_bytes(api_client, wasm_checksum, wasm_cache, is_p2, &created_at).await {
            Ok(bytes) => (bytes, job.compile_time_ms, created_at, None),
            Err(e) => {
                let error_msg = format!("Failed to download WASM: {}", e);
                error!("❌ {}", error_msg);
                api_client.complete_job(job.job_id, false, None, Some(error_msg), 0, 0, None, None, None, Some(api_client::JobStatus::Failed), None).await?;
                return Ok(());
            }
        }
    };

    // Project UUID comes from the contract via coordinator - no need to extract from WASM metadata
    // The contract determines which CodeSource to use for a project, and the coordinator passes project_uuid
    // This is secure because WASM cannot fake its project - the binding is enforced by the contract
    let project_uuid = job.project_uuid.clone();
    if let Some(ref uuid) = project_uuid {
        info!("📋 Running in project context: project_id={:?}, project_uuid={}", job.project_id, uuid);
    } else {
        debug!("No project context - running as standalone WASM (storage disabled)");
    }

    // Decrypt secrets from contract if provided (new repo-based system)
    info!("🔍 DEBUG secrets_ref: {:?}", secrets_ref);
    info!("🔍 DEBUG keystore_client: {}", if keystore_client.is_some() { "Some" } else { "None" });

    let user_secrets = if let (Some(secrets_ref), Some(keystore)) = (secrets_ref, keystore_client) {
        info!("🔐 Decrypting secrets: profile={}, owner={}", secrets_ref.profile, secrets_ref.account_id);

        // user_account_id is the account that requested execution (used for access control)
        let caller = user_account_id.map(|s| s.as_str()).unwrap_or(&secrets_ref.account_id);

        // Decrypt secrets based on project_id (if present) or code_source type
        let secrets_result = if let Some(ref proj_id) = job.project_id {
            // Project-based execution: use project-scoped secrets
            info!("📦 Decrypting project-based secrets for project: {}", proj_id);
            keystore.decrypt_secrets_by_project(proj_id, &secrets_ref.profile, &secrets_ref.account_id, caller, Some(data_id)).await
        } else {
            // Non-project execution: use code_source type for secrets
            match code_source {
                CodeSource::GitHub { repo, commit, .. } => {
                    info!("📦 Decrypting repo-based secrets for GitHub source");

                    // Resolve branch from commit via coordinator API (with caching)
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

                    // Call keystore to decrypt secrets by repo
                    keystore.decrypt_secrets_from_contract(repo, branch.as_deref(), &secrets_ref.profile, &secrets_ref.account_id, caller, Some(data_id)).await
                }
                CodeSource::WasmUrl { hash, .. } => {
                    info!("📦 Decrypting wasm_hash-based secrets for WasmUrl source: {}", hash);

                    // Call keystore to decrypt secrets by wasm_hash
                    keystore.decrypt_secrets_by_wasm_hash(hash, &secrets_ref.profile, &secrets_ref.account_id, caller, Some(data_id)).await
                }
            }
        };

        match secrets_result {
            Ok(secrets) => {
                info!("✅ Secrets decrypted successfully: {} environment variables", secrets.len());
                Some(secrets)
            }
            Err(e) => {
                // Error message already user-friendly from keystore_client
                let error_msg = e.to_string();

                // "Secrets not found" is not a fatal error - WASM may not need secrets
                // Continue with empty secrets map
                if error_msg.contains("not found") {
                    info!("ℹ️  No secrets configured for this project/source, continuing without secrets");
                    Some(std::collections::HashMap::new())
                } else {
                    error!("❌ Secrets decryption failed: {}", error_msg);

                    // Determine error category based on error message
                    let error_category = if error_msg.contains("Access") && error_msg.contains("denied") {
                        api_client::JobStatus::AccessDenied
                    } else if error_msg.contains("Invalid secrets format") {
                        api_client::JobStatus::Custom // Invalid format - user configuration issue
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
                    refund_usd: None,
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
                        Some(error_category),
                        None, // No compile_result
                    )
                    .await?;
                    return Ok(());
                }
            }
        }
    } else {
        None
    };

    // Merge environment variables
    let env_vars = merge_env_vars(
        user_secrets,
        context,
        resource_limits,
        request_id,
        user_account_id,
        near_payment_yocto,
        attached_usd,
        transaction_hash,
        job.project_id.as_ref(),
        project_uuid.as_ref(),
        // HTTPS-specific parameters
        is_https_call,
        call_id,
        payment_key_owner,
        usd_payment,
        // Network configuration
        &config.near_rpc_url,
    );

    // Get build target from code source
    let build_target = match code_source {
        CodeSource::GitHub { build_target, .. } => Some(build_target.as_str()),
        CodeSource::WasmUrl { build_target, .. } => Some(build_target.as_str()),
    };

    // Create storage config if keystore is configured AND project_uuid exists
    // Storage requires both: keystore for encryption/decryption AND project for data organization
    let storage_config = match (&config.keystore_base_url, &config.keystore_auth_token, &project_uuid) {
        (Some(keystore_url), Some(keystore_token), Some(uuid)) => {
            // Determine account_id for storage (user who triggered execution)
            let storage_account_id = user_account_id
                .cloned()
                .unwrap_or_else(|| "anonymous".to_string());

            info!(
                "📦 Storage enabled: project_uuid={}, wasm_hash={}, account={}",
                uuid, wasm_checksum, storage_account_id
            );

            Some(StorageConfig {
                coordinator_url: config.api_base_url.clone(),
                coordinator_token: config.api_auth_token.clone(),
                keystore_url: keystore_url.clone(),
                keystore_token: keystore_token.clone(),
                project_uuid: uuid.clone(),
                wasm_hash: wasm_checksum.clone(),
                account_id: storage_account_id,
                tee_mode: config.tee_mode.clone(),
            })
        }
        (None, _, _) | (_, None, _) => {
            debug!("Storage not enabled (keystore not configured)");
            None
        }
        (_, _, None) => {
            debug!("Storage not enabled (no project - WASM running standalone)");
            None
        }
    };

    // Execute WASM
    info!("🚀 Executing WASM...");
    let exec_result = executor
        .execute(
            &wasm_bytes,
            Some(wasm_checksum),
            input_data.as_bytes(),
            resource_limits,
            Some(env_vars),
            build_target,
            response_format,
            storage_config,
        )
        .await;

    // Cache raw WASM after execution - only for P1 (P2 uses CompiledCache for native code)
    // This is a security measure: WASI P2 has access to /tmp, so we cache only after WASI exits
    if !is_p2 && !wasm_bytes.is_empty() {
        if let Some(cache) = wasm_cache {
            if let Ok(mut c) = cache.lock() {
                if let Err(e) = c.put(wasm_checksum, &wasm_bytes) {
                    warn!("Failed to cache WASM after execution: {}", e);
                } else {
                    info!("📦 Cached WASM after execution: {} ({}KB)", wasm_checksum, wasm_bytes.len() / 1024);
                }
            }
        }
    }

    match exec_result {
        Ok(mut execution_result) => {
            // Add compilation time if WASM was compiled in this execution
            execution_result.compile_time_ms = compile_time_ms;

            // Add compilation note - prioritize published_url, then check if freshly compiled or cached
            // published_url can come from local cache (compiled_wasm) or from compile_result (separate executor)
            let effective_published_url = published_url.or(published_url_from_compile_result.clone());

            execution_result.compilation_note = if let Some(url) = &effective_published_url {
                // WASM was published to FastFS/IPFS
                Some(format!("Published to {}", url))
            } else if compile_time_ms.is_some() {
                // Freshly compiled in this task (no published_url)
                Some("Freshly compiled".to_string())
            } else if compile_cost > 0 {
                // Compiled by separate compiler worker (compile_cost indicates compilation happened)
                Some("Freshly compiled".to_string())
            } else if let Some(timestamp) = &created_at {
                // Downloaded from cache
                Some(format!("Cached WASM from {}", timestamp))
            } else {
                None
            };

            info!("🔍 DEBUG: effective_published_url={:?}, compile_time_ms={:?}, compile_cost={}, created_at={:?}, compilation_note={:?}",
                &effective_published_url, &compile_time_ms, compile_cost, &created_at, &execution_result.compilation_note);

            // Check if WASM execution actually succeeded (executor returns Ok even for WASM errors)
            if !execution_result.success {
                let error_msg = execution_result.error.clone().unwrap_or_else(|| "Unknown error".to_string());
                error!("❌ WASM execution failed: {}", error_msg);

                // HTTPS calls: report error to coordinator
                if is_https_call {
                    if let Some(ref call_id_str) = call_id {
                        info!("📤 HTTPS call: reporting WASM error to coordinator (call_id={})", call_id_str);
                        if let Err(https_err) = api_client.complete_https_call(
                            call_id_str,
                            false,
                            None,
                            Some(error_msg.clone()),
                            execution_result.instructions,
                            execution_result.execution_time_ms,
                        ).await {
                            error!("❌ Failed to report HTTPS call error: {}", https_err);
                        }

                        // Report failure to coordinator job tracking
                        if let Err(e) = api_client
                            .complete_job(
                                job.job_id,
                                false,
                                None,
                                Some(error_msg),
                                execution_result.execution_time_ms,
                                execution_result.instructions,
                                None,
                                None,
                                if compile_cost > 0 { Some(compile_cost.to_string()) } else { None },
                                Some(JobStatus::ExecutionFailed),
                                None,
                            )
                            .await
                        {
                            warn!("⚠️ Failed to report execute job failure: {}", e);
                        }

                        return Ok(());
                    }
                }

                // NEAR contract calls: continue to normal flow (will be handled below)
                // The result (including success=false) will be submitted to NEAR contract
            }

            // Log execution result (only log success for successful executions)
            if execution_result.success {
                if let Some(ct) = compile_time_ms {
                    info!(
                        "✅ Execution successful: compile={}ms execute={}ms instructions={}{}",
                        ct, execution_result.execution_time_ms, execution_result.instructions,
                        effective_published_url.as_ref().map(|u| format!(" published: {}", u)).unwrap_or_default()
                    );
                } else {
                    info!(
                        "✅ Execution successful: time={}ms instructions={} (using cached WASM{})",
                        execution_result.execution_time_ms,
                        execution_result.instructions,
                        created_at.as_ref().map(|t| format!(" from {}", t)).unwrap_or_default()
                    );
                }
            }

            // HTTPS calls go to coordinator, not NEAR contract
            if is_https_call {
                let call_id_str = call_id.ok_or_else(|| anyhow::anyhow!("HTTPS call missing call_id"))?;
                info!("📤 HTTPS call: submitting result to coordinator (call_id={})", call_id_str);

                // Convert ExecutionOutput to serde_json::Value
                let output_json = execution_result.output.as_ref().map(|out| match out {
                    api_client::ExecutionOutput::Bytes(bytes) => {
                        use base64::{engine::general_purpose::STANDARD, Engine};
                        serde_json::Value::String(STANDARD.encode(bytes))
                    }
                    api_client::ExecutionOutput::Text(text) => {
                        serde_json::Value::String(text.clone())
                    }
                    api_client::ExecutionOutput::Json(json) => json.clone(),
                });

                match api_client.complete_https_call(
                    call_id_str,
                    true,
                    output_json.clone(),
                    None,
                    execution_result.instructions,
                    execution_result.execution_time_ms,
                ).await {
                    Ok(()) => {
                        info!("✅ HTTPS call result submitted to coordinator successfully");

                        // Report success to coordinator job tracking
                        if let Err(e) = api_client
                            .complete_job(
                                job.job_id,
                                true,
                                execution_result.output.clone(),
                                None,
                                execution_result.execution_time_ms,
                                execution_result.instructions,
                                None,
                                None, // No cost extraction for HTTPS calls - handled by coordinator
                                if compile_cost > 0 { Some(compile_cost.to_string()) } else { None },
                                None,
                                None,
                            )
                            .await
                        {
                            warn!("⚠️ Failed to report execute job completion: {}", e);
                        }

                        // Generate and store attestation for HTTPS call
                        if use_tee_registration {
                            use sha2::{Sha256, Digest};

                            // Compute hashes
                            let mut input_hasher = Sha256::new();
                            input_hasher.update(input_data.as_bytes());
                            let input_hash = hex::encode(input_hasher.finalize());

                            let output_hash = if let Some(ref json) = output_json {
                                let mut output_hasher = Sha256::new();
                                output_hasher.update(json.to_string().as_bytes());
                                hex::encode(output_hasher.finalize())
                            } else {
                                // Empty output
                                let mut output_hasher = Sha256::new();
                                output_hasher.update(b"");
                                hex::encode(output_hasher.finalize())
                            };

                            // Generate TDX quote
                            match tdx_client.generate_task_attestation(
                                "execute",
                                job.job_id,
                                code_source.repo(),
                                code_source.commit(),
                                code_source.build_target(),
                                Some(wasm_checksum),
                                Some(&input_hash),
                                &output_hash,
                                None, // No block_height for HTTPS calls
                            ).await {
                                Ok(tdx_quote) => {
                                    // Send attestation to coordinator with HTTPS fields
                                    let attestation_request = api_client::StoreAttestationRequest {
                                        task_id: job.job_id,
                                        task_type: api_client::TaskType::Execute,
                                        tdx_quote,
                                        // NEAR context - None for HTTPS calls
                                        request_id: None,
                                        caller_account_id: None,
                                        transaction_hash: None,
                                        block_height: None,
                                        // HTTPS call context
                                        call_id: Some(call_id_str.to_string()),
                                        payment_key_owner: payment_key_owner.cloned(),
                                        payment_key_nonce,
                                        repo_url: code_source.repo().map(|s| s.to_string()),
                                        commit_hash: code_source.commit().map(|s| s.to_string()),
                                        build_target: code_source.build_target().map(|s| s.to_string()),
                                        wasm_hash: Some(wasm_checksum.clone()),
                                        input_hash: Some(input_hash),
                                        output_hash,
                                    };

                                    if let Err(e) = api_client.store_attestation(attestation_request).await {
                                        warn!("⚠️ Failed to store HTTPS execution attestation: {}", e);
                                        // Non-critical - continue anyway
                                    } else {
                                        info!("✅ Stored HTTPS execution attestation for call_id={}", call_id_str);
                                    }
                                }
                                Err(e) => {
                                    warn!("⚠️ Failed to generate TDX attestation for HTTPS execution: {}", e);
                                    // Non-critical - continue anyway
                                }
                            }
                        } else {
                            debug!("Skipping attestation generation for HTTPS call (USE_TEE_REGISTRATION=false)");
                        }
                    }
                    Err(e) => {
                        error!("❌ Failed to submit HTTPS call result: {}", e);

                        if let Err(report_err) = api_client
                            .complete_job(
                                job.job_id,
                                false,
                                None,
                                Some(format!("Failed to submit HTTPS call result: {}", e)),
                                execution_result.execution_time_ms,
                                execution_result.instructions,
                                None,
                                None,
                                None,
                                Some(api_client::JobStatus::Failed),
                                None,
                            )
                            .await
                        {
                            warn!("⚠️ Failed to report execute job failure: {}", report_err);
                        }

                        return Err(e);
                    }
                }

                return Ok(());
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

                    // Report to coordinator (async, can fail without breaking flow)
                    if let Err(e) = api_client
                        .complete_job(
                            job.job_id,
                            execution_result.success,
                            execution_result.output.clone(),
                            execution_result.error.clone(),
                            execution_result.execution_time_ms,
                            execution_result.instructions,
                            None,
                            if actual_cost > 0 { Some(actual_cost.to_string()) } else { None },
                            if compile_cost > 0 { Some(compile_cost.to_string()) } else { None },
                            None, // No error category for success
                            None, // No compile_result
                        )
                        .await
                    {
                        warn!("⚠️ Failed to report execute job completion: {}", e);
                        // Continue anyway - NEAR transaction is already submitted
                    }

                    // Generate and store TDX attestation
                    {
                        // Calculate output hash to match what the contract returns to the user
                        // The contract converts ExecutionOutput to serde_json::Value and returns it
                        use sha2::{Digest, Sha256};
                        let output_hash = if let Some(ref output) = execution_result.output {
                            let mut hasher = Sha256::new();

                            // Hash the JSON value that the contract returns (see contract/src/execution.rs:308-322)
                            let json_value = match output {
                                api_client::ExecutionOutput::Bytes(bytes) => {
                                    // Contract returns base64-encoded string for bytes
                                    use base64::{engine::general_purpose::STANDARD, Engine};
                                    serde_json::Value::String(STANDARD.encode(bytes))
                                },
                                api_client::ExecutionOutput::Text(text) => {
                                    // Contract returns text as JSON string
                                    serde_json::Value::String(text.clone())
                                },
                                api_client::ExecutionOutput::Json(json) => {
                                    // Contract returns JSON value directly
                                    json.clone()
                                },
                            };

                            // Serialize the JSON value to string (this is what gets returned from contract)
                            let json_string = serde_json::to_string(&json_value)
                                .unwrap_or_else(|_| "null".to_string());
                            hasher.update(json_string.as_bytes());

                            hex::encode(hasher.finalize())
                        } else {
                            "no-output".to_string()
                        };

                        // Generate and store TDX attestation only if TEE registration is enabled
                        if use_tee_registration {
                            // Calculate input hash
                            let mut input_hasher = Sha256::new();
                            input_hasher.update(input_data.as_bytes());
                            let input_hash = hex::encode(input_hasher.finalize());

                            match tdx_client.generate_task_attestation(
                                "execute",
                                job.job_id,
                                code_source.repo(),
                                code_source.commit(),
                                code_source.build_target(),
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
                                        // HTTPS call context - None for NEAR calls
                                        call_id: None,
                                        payment_key_owner: None,
                                        payment_key_nonce: None,
                                        repo_url: code_source.repo().map(|s| s.to_string()),
                                        commit_hash: code_source.commit().map(|s| s.to_string()),
                                        build_target: code_source.build_target().map(|s| s.to_string()),
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
                        } else {
                            debug!("Skipping attestation generation (USE_TEE_REGISTRATION=false)");
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
                            None, // No compile_result
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

            // Handle HTTPS call errors
            if is_https_call {
                if let Some(call_id_str) = call_id {
                    info!("📤 HTTPS call: reporting error to coordinator (call_id={})", call_id_str);

                    if let Err(https_err) = api_client.complete_https_call(
                        call_id_str,
                        false,
                        None,
                        Some(error_msg.clone()),
                        0,
                        0,
                    ).await {
                        error!("❌ Failed to report HTTPS call error: {}", https_err);
                    }

                    // Report to coordinator job tracking
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
                            Some(api_client::JobStatus::ExecutionFailed),
                            None,
                        )
                        .await
                    {
                        warn!("⚠️ Failed to report execute job failure: {}", report_err);
                    }

                    return Err(e);
                }
            }

            let result = ExecutionResult {
                success: false,
                output: None,
                error: Some(error_msg.clone()),
                execution_time_ms: 0,
                instructions: 0,
                compile_time_ms,
                compilation_note: None,
                refund_usd: None,
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
                    None, // No compile_result
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
        // HTTPS call context - None for startup
        call_id: None,
        payment_key_owner: None,
        payment_key_nonce: None,
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

// =============================================================================
// Contract System Callbacks Handler
// =============================================================================
//
// This handler processes contract business logic that requires yield/resume:
// - TopUp: decrypt Payment Key → add balance → re-encrypt → resume on contract
// - Delete: delete from coordinator PostgreSQL → resume on contract
// - (Future: Withdraw, UpdateLimits, etc.)
//
// Separated from main worker loop to avoid blocking WASM compile/execute tasks.
// This is NOT directly related to running user code - it's contract system operations.
// =============================================================================

/// Contract System Callbacks Handler - processes TopUp, Delete, and other contract callbacks
///
/// Polls multiple task queues and processes contract system operations that require
/// yield/resume mechanism. These are business logic operations, not WASM execution.
async fn run_contract_system_callbacks_handler(
    api_client: ApiClient,
    keystore_client: Option<KeystoreClient>,
    near_client: NearClient,
    capabilities: Vec<String>,
) {
    use api_client::SystemCallbackTask;

    info!("📋 Contract System Callbacks Handler loop started (unified queue, 60s timeout)");

    loop {
        // Poll unified queue for any system callback task (blocking, 60s timeout like execution queue)
        match api_client.poll_system_callback_task(60, &capabilities).await {
            Ok(Some(task)) => {
                match task {
                    // =================================================================
                    // TopUp Payment Key - requires keystore
                    // =================================================================
                    SystemCallbackTask::TopUp(payload) => {
                        info!(
                            "💰 Processing TopUp task: owner={} nonce={} amount={}",
                            payload.owner, payload.nonce, payload.amount
                        );

                        // TopUp requires keystore
                        if let Some(ref ks_client) = keystore_client {
                            // Convert payload to the format expected by process_topup_task
                            let task_data = api_client::TopUpTaskData {
                                data_id: payload.data_id.clone(),
                                owner: payload.owner.clone(),
                                nonce: payload.nonce,
                                amount: payload.amount.clone(),
                                encrypted_data: payload.encrypted_data.clone(),
                            };

                            match process_topup_task(ks_client, &near_client, &task_data).await {
                                Ok(result) => {
                                    info!(
                                        "✅ TopUp completed: owner={} nonce={} tx={} new_balance={}",
                                        payload.owner, payload.nonce, result.tx_hash, result.new_balance
                                    );

                                    // Notify coordinator of payment key metadata (non-critical)
                                    if let Err(e) = api_client
                                        .complete_topup(
                                            &payload.owner,
                                            payload.nonce,
                                            &result.new_balance,
                                            &result.key_hash,
                                            &result.project_ids,
                                            result.max_per_call.as_deref(),
                                        )
                                        .await
                                    {
                                        warn!(
                                            "Failed to notify coordinator of payment key update (non-critical): {}",
                                            e
                                        );
                                    }
                                }
                                Err(e) => {
                                    error!(
                                        "❌ TopUp failed: owner={} nonce={} error={}",
                                        payload.owner, payload.nonce, e
                                    );

                                    // Try to resume with error so contract doesn't hang
                                    if let Err(resume_err) = near_client
                                        .resume_topup_error(&payload.data_id, &format!("TopUp failed: {}", e))
                                        .await
                                    {
                                        error!(
                                            "❌ Failed to resume TopUp with error: {} (original error: {})",
                                            resume_err, e
                                        );
                                    }
                                }
                            }
                        } else {
                            error!(
                                "❌ TopUp task received but keystore not configured! owner={} nonce={}",
                                payload.owner, payload.nonce
                            );

                            // Resume with error so contract doesn't hang
                            if let Err(e) = near_client
                                .resume_topup_error(&payload.data_id, "Keystore not configured on this worker")
                                .await
                            {
                                error!(
                                    "❌ Failed to resume TopUp with error: {}",
                                    e
                                );
                            }
                        }
                    }

                    // =================================================================
                    // Delete Payment Key - doesn't require keystore
                    // =================================================================
                    SystemCallbackTask::DeletePaymentKey(payload) => {
                        info!(
                            "🗑️ Processing DeletePaymentKey task: owner={} nonce={}",
                            payload.owner, payload.nonce
                        );

                        // Step 1: Delete from coordinator PostgreSQL
                        if let Err(e) = api_client
                            .delete_payment_key(&payload.owner, payload.nonce)
                            .await
                        {
                            error!(
                                "❌ Failed to delete payment key from coordinator: owner={} nonce={} error={}",
                                payload.owner, payload.nonce, e
                            );

                            // Resume with error so contract doesn't delete the secret
                            if let Err(e) = near_client
                                .resume_delete_payment_key_error(
                                    &payload.data_id,
                                    &format!("Failed to delete from coordinator: {}", e),
                                )
                                .await
                            {
                                error!(
                                    "❌ Failed to resume delete with error: data_id={} error={}",
                                    payload.data_id, e
                                );
                            }
                            continue;
                        }

                        // Step 2: Call resume_delete_payment_key on contract
                        match near_client.resume_delete_payment_key(&payload.data_id).await {
                            Ok(tx_hash) => {
                                info!(
                                    "✅ DeletePaymentKey completed: owner={} nonce={} tx={}",
                                    payload.owner, payload.nonce, tx_hash
                                );
                            }
                            Err(e) => {
                                error!(
                                    "❌ Failed to resume DeletePaymentKey on contract: owner={} nonce={} error={}",
                                    payload.owner, payload.nonce, e
                                );
                            }
                        }
                    }

                    // =================================================================
                    // Project Storage Cleanup - clear compiled WASM and storage
                    // =================================================================
                    SystemCallbackTask::ProjectStorageCleanup(payload) => {
                        info!(
                            "🧹 Processing ProjectStorageCleanup task: project_id={} uuid={}",
                            payload.project_id, payload.project_uuid
                        );

                        match api_client.clear_project_storage(&payload.project_uuid).await {
                            Ok(()) => {
                                info!(
                                    "✅ ProjectStorageCleanup completed: uuid={}",
                                    payload.project_uuid
                                );
                            }
                            Err(e) => {
                                error!(
                                    "❌ Failed to clear project storage: uuid={} error={}",
                                    payload.project_uuid, e
                                );
                            }
                        }
                    }
                }
            }
            Ok(None) => {
                // Timeout - blocking BRPOP already waited 60s, continue immediately
            }
            Err(e) => {
                error!("❌ Failed to poll system callback task: {}", e);
                // Sleep before retry to avoid tight error loop
                tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
            }
        }
    }
}

/// TopUp result containing tx_hash, new balance, and key metadata for coordinator
struct TopUpResult {
    tx_hash: String,
    new_balance: String,
    key_hash: String,
    project_ids: Vec<String>,
    max_per_call: Option<String>,
}

/// Process a single TopUp task
///
/// 1. Decrypt current Payment Key data via keystore
/// 2. Parse JSON and update initial_balance
/// 3. Re-encrypt via keystore
/// 4. Call resume_topup on contract
///
/// Returns: tx_hash and new_balance for coordinator notification
async fn process_topup_task(
    keystore_client: &KeystoreClient,
    near_client: &NearClient,
    task: &api_client::TopUpTaskData,
) -> Result<TopUpResult> {
    // 1. Decrypt current Payment Key data
    // Seed format: "system:payment_key:{owner}:{nonce}"
    let seed = format!("system:payment_key:{}:{}", task.owner, task.nonce);

    let decrypted_bytes = keystore_client
        .decrypt_raw(&seed, &task.encrypted_data)
        .await
        .context("Failed to decrypt Payment Key data")?;

    let decrypted_str = String::from_utf8(decrypted_bytes)
        .context("Payment Key data is not valid UTF-8")?;

    // 2. Parse JSON and extract fields
    // Payment Key format: {"key":"base64_key","initial_balance":"123","project_ids":[],"max_per_call":"1000"}
    let mut payment_key_data: serde_json::Value = serde_json::from_str(&decrypted_str)
        .context("Failed to parse Payment Key JSON")?;

    // Extract key for hash computation
    let key = payment_key_data
        .get("key")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("Missing key field"))?
        .to_string();

    // Compute key_hash = SHA256(key)
    use sha2::{Sha256, Digest};
    let key_hash = hex::encode(Sha256::digest(key.as_bytes()));

    // Extract project_ids
    let project_ids: Vec<String> = payment_key_data
        .get("project_ids")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();

    // Extract max_per_call
    let max_per_call = payment_key_data
        .get("max_per_call")
        .and_then(|v| v.as_str())
        .map(String::from);

    let current_balance = payment_key_data
        .get("initial_balance")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("Missing initial_balance field"))?;

    let current_balance_u128: u128 = current_balance
        .parse()
        .context("Failed to parse initial_balance as u128")?;

    let topup_amount: u128 = task.amount.parse()
        .context("Failed to parse topup amount as u128")?;

    let new_balance = current_balance_u128 + topup_amount;

    info!(
        "💰 Updating balance: {} + {} = {} (key_hash={}...)",
        current_balance_u128, topup_amount, new_balance, &key_hash[..8]
    );

    payment_key_data["initial_balance"] = serde_json::json!(new_balance.to_string());

    // 3. Re-encrypt via keystore
    let updated_json = serde_json::to_string(&payment_key_data)
        .context("Failed to serialize updated Payment Key")?;

    let new_encrypted_data = keystore_client
        .encrypt(&seed, updated_json.as_bytes())
        .await
        .context("Failed to encrypt updated Payment Key data")?;

    // 4. Call resume_topup on contract
    let tx_hash = near_client
        .resume_topup(&task.data_id, &new_encrypted_data)
        .await
        .context("Failed to resume TopUp on contract")?;

    Ok(TopUpResult {
        tx_hash,
        new_balance: new_balance.to_string(),
        key_hash,
        project_ids,
        max_per_call,
    })
}

