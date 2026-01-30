use crate::*;
use near_sdk::serde_json::json;

#[near_bindgen]
impl Contract {
    /// Request off-chain execution
    ///
    /// # Arguments
    /// * `source` - Execution source: GitHub repo, WasmUrl, or Project reference
    /// * `resource_limits` - Optional resource limits for execution (default: 1B instructions, 128MB, 60s)
    ///                      If None, only compilation is performed (compile-only mode)
    /// * `input_data` - Optional input data for the WASM program (default: empty string)
    /// * `secrets_ref` - Optional reference to secrets (profile + account_id)
    /// * `response_format` - Optional output format: Bytes, Text, or Json (default: Text)
    /// * `payer_account_id` - Optional account to receive refunds (default: sender)
    /// * `params` - Optional request parameters (force_rebuild, store_on_fastfs)
    ///
    /// # Execution Source
    /// You can specify code in three ways:
    /// - `GitHub { repo, commit, build_target }` - Compile from GitHub repository
    /// - `WasmUrl { url, hash, build_target }` - Use pre-compiled WASM from URL
    /// - `Project { project_id, version_key }` - Use registered project (version_key=None for active version)
    ///
    /// # Compile-only Mode
    /// If `resource_limits` is None, only compilation is performed:
    /// - WASM is compiled and cached
    /// - No execution occurs
    /// - Useful for pre-compiling expensive builds
    ///
    /// # Secrets
    /// Secrets are stored in contract and accessed via references:
    /// 1. Store secrets once: `store_secrets(accessor, profile, encrypted_data, access_rules)`
    /// 2. Reference them in execution: `secrets_ref: { profile: "default", account_id: "alice.near" }`
    /// 3. Worker will fetch and decrypt secrets via keystore
    #[payable]
    pub fn request_execution(
        &mut self,
        source: ExecutionSource,
        resource_limits: Option<ResourceLimits>,
        input_data: Option<String>,
        secrets_ref: Option<SecretsReference>,
        response_format: Option<ResponseFormat>,
        payer_account_id: Option<AccountId>,
        params: Option<RequestParams>,
    ) {
        self.assert_not_paused();

        // Resolve ExecutionSource to CodeSource (and get project_uuid if applicable)
        let (resolved_source, project_uuid) = self.resolve_execution_source(&source);

        // Use provided limits or defaults (for execute mode)
        let limits = resource_limits.clone().unwrap_or_default();

        // Get params or defaults, but override project_uuid if resolved from Project source
        let mut request_params = params.unwrap_or_default();
        if project_uuid.is_some() {
            request_params.project_uuid = project_uuid;
        }

        // Determine if this is compile-only mode
        let compile_only = request_params.compile_only || resource_limits.is_none();

        // Validate: WasmUrl source cannot have force_rebuild
        if matches!(resolved_source, CodeSource::WasmUrl { .. }) && request_params.force_rebuild {
            env::panic_str("force_rebuild is not applicable for WasmUrl code source");
        }

        // Validate: store_on_fastfs requires force_rebuild (to ensure fresh compilation)
        if request_params.store_on_fastfs && !request_params.force_rebuild {
            env::panic_str("store_on_fastfs requires force_rebuild to ensure fresh compilation and upload");
        }

        // Validate: compile_only mode should not have input_data
        if compile_only && input_data.is_some() && !input_data.as_ref().unwrap().is_empty() {
            env::panic_str("input_data must be empty for compile_only mode - compilation does not use input_data");
        }

        // Validate resource limits against hard caps (only in execute mode)
        if !compile_only {
            let max_instructions = limits.max_instructions.unwrap_or_default();
            let max_execution_seconds = limits.max_execution_seconds.unwrap_or_default();

            assert!(
                max_instructions <= MAX_INSTRUCTIONS,
                "Requested max_instructions {} exceeds hard limit of {}",
                max_instructions,
                MAX_INSTRUCTIONS
            );

            assert!(
                max_execution_seconds <= MAX_EXECUTION_SECONDS,
                "Requested max_execution_seconds {} exceeds hard limit of {} seconds",
                max_execution_seconds,
                MAX_EXECUTION_SECONDS
            );
        }

        // Calculate cost: base fee for compile-only, full estimate for execute
        let estimated_cost = if compile_only {
            self.base_fee // Only base fee for compile-only
        } else {
            self.estimate_cost(&limits)
        };

        // Parse attached_usd for project owner (developer payment in stablecoin)
        let attached_usd = request_params.attached_usd.map(|d| d.0).unwrap_or(0);

        // Validate: attached_usd only valid for Project source
        if attached_usd > 0 {
            assert!(
                matches!(source, ExecutionSource::Project { .. }),
                "attached_usd is only valid for Project execution source"
            );

            // Check user has enough stablecoin balance
            let caller = env::predecessor_account_id();
            let user_balance = self.user_stablecoin_balances.get(&caller).unwrap_or(0);
            assert!(
                user_balance >= attached_usd,
                "Insufficient stablecoin balance. Required: {}, available: {}",
                attached_usd,
                user_balance
            );

            // Deduct from user's stablecoin balance
            self.user_stablecoin_balances.insert(&caller, &(user_balance - attached_usd));
            log!(
                "Deducted {} stablecoin from {} for developer payment (remaining: {})",
                attached_usd,
                caller,
                user_balance - attached_usd
            );
        }

        // NEAR payment is only for compute costs now
        let payment = env::attached_deposit().as_yoctonear();

        assert!(
            payment >= estimated_cost,
            "Insufficient payment: required {} yoctoNEAR for compute, got {} yoctoNEAR",
            estimated_cost,
            payment
        );

        let request_id = self.next_request_id;
        self.next_request_id += 1;

        // predecessor_id = contract that called OutLayer (e.g. token.near)
        // signer_id = real user who signed the transaction (e.g. alice.near)
        let predecessor_id = env::predecessor_account_id();
        let signer_id = env::signer_account_id();

        // Payer: explicitly provided account or fallback to predecessor
        let payer_account_id = payer_account_id.unwrap_or_else(|| predecessor_id.clone());
        let format = response_format.unwrap_or_default();

        // Extract project_id from ExecutionSource if it's a Project source
        let project_id = match &source {
            ExecutionSource::Project { project_id, .. } => Some(project_id.clone()),
            _ => None,
        };

        // Check if input_data is too large for event log (NEAR has 16KB limit per log)
        // Large payloads are stored in state only, worker fetches via get_request()
        let input_data_in_state = input_data
            .as_ref()
            .map(|d| d.len() >= INPUT_DATA_EVENT_THRESHOLD)
            .unwrap_or(false);

        // For large payloads, don't include in event - worker will fetch from state
        let input_data_for_event = if input_data_in_state {
            String::new()
        } else {
            input_data.as_ref().cloned().unwrap_or_default()
        };

        // Create execution request data for yield (send resolved_source to worker)
        let request_data = json!({
            "request_id": request_id,
            "sender_id": signer_id,
            "predecessor_id": predecessor_id,
            "code_source": resolved_source,
            "resource_limits": limits,
            "input_data": input_data_for_event,
            "input_data_in_state": input_data_in_state,
            "secrets_ref": secrets_ref.as_ref(),
            "response_format": format,
            "payment": U128::from(payment),
            "attached_usd": U128::from(attached_usd),
            "timestamp": env::block_timestamp(),
            "compile_only": compile_only,
            "force_rebuild": request_params.force_rebuild,
            "store_on_fastfs": request_params.store_on_fastfs,
            "project_uuid": request_params.project_uuid,
            "project_id": project_id
        });

        // Create yield promise to pause execution
        let promise_idx = env::promise_yield_create(
            "on_execution_response",
            &request_data.to_string().into_bytes(),
            MIN_RESPONSE_GAS,
            GasWeight::default(),
            DATA_ID_REGISTER,
        );

        // Get data_id for the yield promise
        let data_id: CryptoHash = env::read_register(DATA_ID_REGISTER)
            .expect("Register is empty")
            .try_into()
            .expect("Wrong register length");

        // Store the pending execution request
        // Note: sender_id in ExecutionRequest stores predecessor (contract that called us)
        // This is used for authorization checks (cancel_stale_execution)
        let execution_request = ExecutionRequest {
            request_id,
            data_id,
            sender_id: predecessor_id.clone(),
            execution_source: source.clone(),
            resolved_source: resolved_source.clone(),
            resource_limits: limits.clone(),
            payment,
            timestamp: env::block_timestamp(),
            secrets_ref,
            response_format: format.clone(),
            input_data,
            payer_account_id,
            attached_usd,
            pending_output: None,
            output_submitted: false,
        };

        self.pending_requests
            .insert(&request_id, &execution_request);

        // Emit event for workers to catch
        events::emit::execution_requested(&self.event_standard, &self.event_version, &request_data.to_string(), data_id);

        // Return the promise to pause execution
        env::promise_return(promise_idx)
    }



    /// Worker calls this to submit large execution output (> 1024 bytes)
    /// This is the first step of 2-call flow for large outputs
    pub fn submit_execution_output(&mut self, request_id: u64, output: ExecutionOutput) {
        // Only operator can submit execution data
        self.assert_operator();

        self.submit_execution_output_internal(request_id, output);
    }

    /// Worker calls this to submit large output AND resolve in one transaction (recommended)
    ///
    /// This method combines submit_execution_output + resolve_execution into a single call:
    /// 1. Stores the large output in contract storage
    /// 2. Immediately calls resolve_execution_internal with metadata only
    ///
    /// This saves ~1-2 seconds compared to two separate transactions.
    ///
    /// # Arguments
    /// * `request_id` - Request ID
    /// * `output` - Large execution output (> 1024 bytes)
    /// * `success` - Whether execution succeeded
    /// * `error` - Error message if failed
    /// * `resources_used` - Actual resource consumption
    pub fn submit_execution_output_and_resolve(
        &mut self,
        request_id: u64,
        output: ExecutionOutput,
        success: bool,
        error: Option<String>,
        resources_used: ResourceMetrics,
        compilation_note: Option<String>,
    ) {
        // Only operator can submit execution data
        self.assert_operator();

        // Step 1: Store the large output
        self.submit_execution_output_internal(request_id, output);

        // Step 2: Immediately resolve with metadata only (no Promise needed!)
        let response = ExecutionResponse {
            success,
            output: None, // Output already stored above
            error,
            resources_used,
            compilation_note,
            refund_usd: None, // Large output flow doesn't support refund
        };

        log!(
            "Resolving execution for request_id: {} (combined flow)",
            request_id
        );

        // Call resolve directly in the same function call
        self.resolve_execution_internal(request_id, response);
    }



    /// Worker calls this to resolve execution (small output) or finalize after submit_execution_output (large output)
    ///
    /// For outputs <= 1024 bytes: Call this directly with output in response
    /// For outputs > 1024 bytes: Call submit_execution_output first, then call this
    /// Or use submit_execution_output_and_resolve for optimized 1-call flow
    pub fn resolve_execution(&mut self, request_id: u64, response: ExecutionResponse) {
        // Only operator can resolve executions
        self.assert_operator();

        self.resolve_execution_internal(request_id, response);
    }

    #[allow(unused_variables)]
    #[private]
    /// Callback function to handle execution completion
    pub fn on_execution_response(
        &mut self,
        request_id: u64,
        sender_id: AccountId,
        code_source: CodeSource,
        resource_limits: ResourceLimits,
        payment: U128,
        #[callback_result] response: Result<ExecutionResponse, PromiseError>,
    ) -> Option<serde_json::Value> {
        // Remove the pending request and check if output was submitted separately
        if let Some(request) = self.pending_requests.remove(&request_id) {
            self.total_executions += 1;

            match response {
                Ok(mut exec_response) => {
                    // If output was submitted separately, retrieve it from storage
                    if request.output_submitted && exec_response.success {
                        log!("Retrieving large output from storage for request_id: {}", request_id);
                        if let Some(stored_output) = request.pending_output.clone() {
                            let output: crate::ExecutionOutput = stored_output.into();
                            exec_response.output = Some(output);
                        }
                    }

                    if exec_response.success {
                        // Calculate actual cost (NEAR only)
                        let cost = self.calculate_cost(&exec_response.resources_used);

                        // Handle stablecoin payment with refund support
                        if request.attached_usd > 0 {
                            // Get refund amount from WASM (if called refund_usd())
                            let refund_usd = exec_response.refund_usd.map(|r| r as u128).unwrap_or(0);
                            // Clamp refund to attached amount (safety check)
                            let refund_usd = refund_usd.min(request.attached_usd);
                            let developer_amount = request.attached_usd.saturating_sub(refund_usd);

                            // Refund stablecoin to user's balance
                            if refund_usd > 0 {
                                let current_balance = self.user_stablecoin_balances.get(&request.sender_id).unwrap_or(0);
                                self.user_stablecoin_balances.insert(&request.sender_id, &(current_balance + refund_usd));
                                log!(
                                    "Refunded {} stablecoin to {} (WASM requested partial refund)",
                                    refund_usd,
                                    request.sender_id
                                );
                            }

                            // Credit developer earnings if developer_amount > 0
                            if developer_amount > 0 {
                                if let ExecutionSource::Project { project_id, .. } = &request.execution_source {
                                    if let Some(project) = self.projects.get(project_id) {
                                        let current = self.developer_earnings.get(&project.owner).unwrap_or(0);
                                        self.developer_earnings.insert(&project.owner, &(current + developer_amount));
                                        log!(
                                            "Credited {} stablecoin to developer {} for project {} (attached={}, refund={})",
                                            developer_amount, project.owner, project_id, request.attached_usd, refund_usd
                                        );
                                    }
                                }
                            }
                        }

                        // Refund excess NEAR payment (minus compute cost only, stablecoin is separate)
                        let refund = payment.0.saturating_sub(cost);
                        if refund > 0 {
                            // Transfer refund to payer account
                            near_sdk::Promise::new(request.payer_account_id.clone())
                                .transfer(NearToken::from_yoctonear(refund));
                        }

                        // Collect fee
                        self.total_fees_collected += cost;

                        // Log payment charged in easy-to-parse format for worker
                        log!("[[yNEAR charged: \"{}\"]]", cost);

                        // Emit success event
                        events::emit::execution_completed(
                            &self.event_standard,
                            &self.event_version,
                            &sender_id,
                            &code_source,
                            &exec_response.resources_used,
                            true,
                            None,
                            U128(cost),    // payment_charged
                            U128(refund),  // payment_refunded
                            exec_response.compilation_note.as_deref(),
                        );

                        // Log the execution result with resources used
                        if let Some(output) = exec_response.output {
                            // Convert ExecutionOutput to plain JSON value (without enum wrapper)
                            let json_value = match &output {
                                ExecutionOutput::Bytes(bytes) => {
                                    // For bytes, encode as base64 string
                                    use near_sdk::base64::{engine::general_purpose::STANDARD, Engine};
                                    serde_json::Value::String(STANDARD.encode(bytes))
                                }
                                ExecutionOutput::Text(text) => {
                                    // For text, return as JSON string
                                    serde_json::Value::String(text.clone())
                                }
                                ExecutionOutput::Json(value) => {
                                    // For JSON, return the value directly
                                    value.clone()
                                }
                            };

                            // Log for debugging (with type info, truncated to avoid log limit)
                            let log_preview = match &output {
                                ExecutionOutput::Bytes(bytes) => format!("Bytes({} bytes)", bytes.len()),
                                ExecutionOutput::Text(text) => {
                                    let preview: String = text.chars().take(100).collect();
                                    if text.len() > 100 {
                                        format!("Text({} bytes): {}...", text.len(), preview)
                                    } else {
                                        format!("Text: {}", text)
                                    }
                                }
                                ExecutionOutput::Json(value) => {
                                    let json_str = serde_json::to_string(value).unwrap_or_default();
                                    let preview: String = json_str.chars().take(100).collect();
                                    if json_str.len() > 100 {
                                        format!("Json({} bytes): {}...", json_str.len(), preview)
                                    } else {
                                        format!("Json: {}", json_str)
                                    }
                                }
                            };

                            let compilation_info = exec_response.compilation_note
                                .as_ref()
                                .map(|note| format!(", {}", note))
                                .unwrap_or_default();

                            log!(
                                "Execution completed successfully. Output: {}, Resources: {{ instructions: {}, time_ms: {} }}, Cost: {} yoctoNEAR, Refund: {} yoctoNEAR{}",
                                log_preview,
                                exec_response.resources_used.instructions,
                                exec_response.resources_used.time_ms,
                                cost,
                                refund,
                                compilation_info
                            );

                            Some(json_value)
                        } else {
                            let compilation_info = exec_response.compilation_note
                                .as_ref()
                                .map(|note| format!(", {}", note))
                                .unwrap_or_default();

                            log!(
                                "Execution has no output value. Resources: {{ instructions: {}, time_ms: {} }}, Cost: {} yoctoNEAR, Refund: {} yoctoNEAR{}",
                                exec_response.resources_used.instructions,
                                exec_response.resources_used.time_ms,
                                cost,
                                refund,
                                compilation_info
                            );

                            None
                        }
                    } else {
                        // Execution failed - refund NEAR (except base fee) and stablecoin
                        // Developer gets nothing on failure

                        // Refund NEAR (minus base fee)
                        let refund = payment.0.saturating_sub(self.base_fee);
                        if refund > 0 {
                            near_sdk::Promise::new(request.payer_account_id.clone())
                                .transfer(NearToken::from_yoctonear(refund));
                        }

                        // Refund stablecoin to user's balance
                        if request.attached_usd > 0 {
                            let current = self.user_stablecoin_balances.get(&request.sender_id).unwrap_or(0);
                            self.user_stablecoin_balances.insert(&request.sender_id, &(current + request.attached_usd));
                            log!(
                                "Refunded {} stablecoin to {} (execution failed)",
                                request.attached_usd,
                                request.sender_id
                            );
                        }

                        self.total_fees_collected += self.base_fee;

                        // Log payment charged in easy-to-parse format for worker (only base fee charged on failure)
                        log!("[[yNEAR charged: \"{}\"]]", self.base_fee);

                        // Get error message for event
                        let error_msg = exec_response.error.unwrap_or("Unknown error".to_string());

                        // Emit failure event with error details
                        events::emit::execution_completed(
                            &self.event_standard,
                            &self.event_version,
                            &sender_id,
                            &code_source,
                            &exec_response.resources_used,
                            false,
                            Some(&error_msg),
                            U128(self.base_fee),  // payment_charged (only base fee)
                            U128(refund),         // payment_refunded
                            exec_response.compilation_note.as_deref(),
                        );

                        // Log the failure (don't panic - state changes must persist!)
                        log!(
                            "Execution failed: {}. Resources: {{ instructions: {}, time_ms: {} }}. Refunded {} yoctoNEAR",
                            error_msg,
                            exec_response.resources_used.instructions,
                            exec_response.resources_used.time_ms,
                            refund
                        );

                        // Return None to indicate failure to calling contract
                        None
                    }
                }
                Err(promise_error) => {
                    // Promise failed - refund NEAR (except base fee) and stablecoin
                    // Developer gets nothing on failure

                    // Refund NEAR (minus base fee)
                    let refund = payment.0.saturating_sub(self.base_fee);
                    if refund > 0 {
                        near_sdk::Promise::new(request.payer_account_id.clone())
                            .transfer(NearToken::from_yoctonear(refund));
                    }

                    // Refund stablecoin to user's balance
                    if request.attached_usd > 0 {
                        let current = self.user_stablecoin_balances.get(&request.sender_id).unwrap_or(0);
                        self.user_stablecoin_balances.insert(&request.sender_id, &(current + request.attached_usd));
                        log!(
                            "Refunded {} stablecoin to {} (promise failed)",
                            request.attached_usd,
                            request.sender_id
                        );
                    }

                    self.total_fees_collected += self.base_fee;

                    // Log payment charged in easy-to-parse format for worker (only base fee charged on promise failure)
                    log!("[[yNEAR charged: \"{}\"]]", self.base_fee);

                    // Log the promise failure (don't panic - state changes must persist!)
                    log!(
                        "Execution promise failed: {:?}. Refunded {} yoctoNEAR",
                        promise_error, refund
                    );

                    // Return None to indicate failure to calling contract
                    None
                }
            }
        } else {
            log!(
                "Warning: Execution request {} not found in pending requests",
                request_id
            );

            None
        }
    }

    /// Cancel stale execution request if timeout has passed
    pub fn cancel_stale_execution(&mut self, request_id: u64) {
        let request = self
            .pending_requests
            .get(&request_id)
            .expect("Execution request not found");

        // Ensure the caller is the original sender
        assert_eq!(
            env::predecessor_account_id(),
            request.sender_id,
            "Only the sender can cancel this execution"
        );

        // Check if the timeout period has passed
        let is_stale = env::block_timestamp() > request.timestamp + EXECUTION_TIMEOUT;
        assert!(is_stale, "Execution is not yet stale, please wait");

        // Remove the request and refund the payer
        if let Some(stale_request) = self.pending_requests.remove(&request_id) {
            near_sdk::Promise::new(stale_request.payer_account_id.clone())
                .transfer(NearToken::from_yoctonear(stale_request.payment));

            log!(
                "Cancelled stale execution {} and refunded payer {}",
                request_id,
                stale_request.payer_account_id
            );
        }
    }
}

// ============================================================================
// Execution Source Resolution
// ============================================================================

impl Contract {
    /// Resolve ExecutionSource to CodeSource for worker
    /// Returns (resolved_source, project_uuid)
    /// Secrets are passed as-is through secrets_ref parameter, no auto-lookup
    fn resolve_execution_source(
        &self,
        source: &ExecutionSource,
    ) -> (CodeSource, Option<String>) {
        match source {
            ExecutionSource::GitHub { repo, commit, build_target } => {
                (
                    CodeSource::GitHub {
                        repo: repo.clone(),
                        commit: commit.clone(),
                        build_target: build_target.clone(),
                    },
                    None,
                )
            }
            ExecutionSource::WasmUrl { url, hash, build_target } => {
                (
                    CodeSource::WasmUrl {
                        url: url.clone(),
                        hash: hash.clone(),
                        build_target: build_target.clone(),
                    },
                    None,
                )
            }
            ExecutionSource::Project { project_id, version_key } => {
                // Get project
                let project = self.projects.get(project_id)
                    .expect("Project not found");

                // Determine which version to use
                let version_to_use = version_key.clone().unwrap_or_else(|| {
                    assert!(
                        !project.active_version.is_empty(),
                        "Project has no active version"
                    );
                    project.active_version.clone()
                });

                // Get version info to get CodeSource
                let versions = self.project_versions.get(&project.uuid)
                    .expect("Project versions not found");

                let version_info = versions.get(&version_to_use)
                    .expect("Version not found");

                log!(
                    "Resolved project: {}, version: {}, source: {:?}, uuid: {}",
                    project_id, version_to_use, version_info.source, project.uuid
                );

                (version_info.source.clone(), Some(project.uuid.clone()))
            }
        }
    }

    /// Internal helper to submit execution output (used by both public methods)
    pub(crate) fn submit_execution_output_internal(&mut self, request_id: u64, output: ExecutionOutput) {
        // Get the pending request
        let mut request = self
            .pending_requests
            .get(&request_id)
            .expect("Execution request not found");

        // Ensure output was not already submitted
        assert!(
            !request.output_submitted,
            "Output already submitted for this request"
        );

        // Store the output in the request (convert to internal storage format)
        let stored_output: crate::StoredOutput = output.into();
        request.pending_output = Some(stored_output);
        request.output_submitted = true;

        // Save updated request
        self.pending_requests.insert(&request_id, &request);

        log!(
            "Stored pending output for request_id: {}, data_id: {:?}",
            request_id,
            request.data_id
        );
    }

    /// Internal helper to resolve execution (no operator check)
    fn resolve_execution_internal(&mut self, request_id: u64, response: ExecutionResponse) {
        // Get the pending request
        let request = self
            .pending_requests
            .get(&request_id)
            .expect("Execution request not found");

        let data_id = request.data_id;

        // Calculate estimated cost for logging
        let estimated_cost = self.calculate_cost(&response.resources_used);

        log!(
            "Resolving execution for request_id: {}, data_id: {:?}, success: {}, output_submitted: {}, resources_used: {{ instructions: {}, time_ms: {}, compile_time_ms: {:?} }}",
            request_id,
            data_id,
            response.success,
            request.output_submitted,
            response.resources_used.instructions,
            response.resources_used.time_ms,
            response.resources_used.compile_time_ms
        );

        // Log cost in easy-to-parse format for worker
        log!("[[yNEAR charged: \"{}\"]]", estimated_cost);

        // For large outputs, we only pass metadata through resume (output stays in storage)
        // The callback will retrieve it from pending_output field
        // This avoids the 1024 byte limit of promise_yield_resume
        if !env::promise_yield_resume(&data_id, &serde_json::to_vec(&response).unwrap()) {
            env::panic_str("Unable to resume execution promise");
        }
    }
}
