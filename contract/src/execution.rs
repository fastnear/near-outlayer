use crate::*;
use near_sdk::serde_json::json;

#[near_bindgen]
impl Contract {
    /// Request off-chain execution
    ///
    /// # Arguments
    /// * `code_source` - GitHub repository and commit to compile
    /// * `resource_limits` - Optional resource limits for execution (default: 1B instructions, 128MB, 60s)
    /// * `input_data` - Optional input data for the WASM program (default: empty string)
    /// * `secrets_ref` - Optional reference to repo-based secrets (profile + account_id)
    /// * `response_format` - Optional output format: Bytes, Text, or Json (default: Text)
    ///
    /// # Repo-Based Secrets (New Approach)
    /// Secrets are now stored in contract per repository and accessed via references:
    /// 1. Store secrets once: `store_secrets(repo, branch, profile, encrypted_data, access_rules)`
    /// 2. Reference them in execution: `secrets_ref: { profile: "default", account_id: "alice.near" }`
    /// 3. Worker will fetch secrets from contract via keystore
    #[payable]
    pub fn request_execution(
        &mut self,
        code_source: CodeSource,
        resource_limits: Option<ResourceLimits>,
        input_data: Option<String>,
        secrets_ref: Option<SecretsReference>,
        response_format: Option<ResponseFormat>,
    ) {
        self.assert_not_paused();

        let limits = resource_limits.unwrap_or_default();

        // Validate resource limits against hard caps
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

        let estimated_cost = self.estimate_cost(&limits);
        let payment = env::attached_deposit().as_yoctonear();

        assert!(
            payment >= estimated_cost,
            "Insufficient payment: required {} yoctoNEAR for requested limits (max_instructions: {:?}, max_execution_seconds: {:?}), got {} yoctoNEAR",
            estimated_cost,
            limits.max_instructions,
            limits.max_execution_seconds,
            payment
        );

        let request_id = self.next_request_id;
        self.next_request_id += 1;

        let sender_id = env::predecessor_account_id();
        let format = response_format.unwrap_or_default();

        // Create execution request data for yield
        let request_data = json!({
            "request_id": request_id,
            "sender_id": sender_id,
            "code_source": code_source,
            "resource_limits": limits,
            "input_data": input_data.as_ref().cloned().unwrap_or_default(),
            "secrets_ref": secrets_ref.as_ref(),
            "response_format": format,
            "payment": U128::from(payment),
            "timestamp": env::block_timestamp()
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
        let execution_request = ExecutionRequest {
            request_id,
            data_id,
            sender_id: sender_id.clone(),
            code_source: code_source.clone(),
            resource_limits: limits.clone(),
            payment,
            timestamp: env::block_timestamp(),
            secrets_ref,
            response_format: format.clone(),
            input_data,
            pending_output: None,
            output_submitted: false,
        };

        self.pending_requests
            .insert(&request_id, &execution_request);

        // Emit event for workers to catch
        events::emit::execution_requested(&request_data.to_string(), data_id);

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
                        if let Some(stored_output) = request.pending_output {
                            let output: crate::ExecutionOutput = stored_output.into();
                            exec_response.output = Some(output);
                        }
                    }

                    if exec_response.success {
                        // Calculate actual cost
                        let cost = self.calculate_cost(&exec_response.resources_used);

                        // Refund excess payment
                        let refund = payment.0.saturating_sub(cost);
                        if refund > 0 {
                            // Transfer refund back to sender
                            near_sdk::Promise::new(sender_id.clone())
                                .transfer(NearToken::from_yoctonear(refund));
                        }

                        // Collect fee
                        self.total_fees_collected += cost;

                        // Log payment charged in easy-to-parse format for worker
                        log!("[[yNEAR charged: \"{}\"]]", cost);

                        // Emit success event
                        events::emit::execution_completed(
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

                            // Log for debugging (with type info)
                            let log_preview = match &output {
                                ExecutionOutput::Bytes(bytes) => format!("Bytes({} bytes)", bytes.len()),
                                ExecutionOutput::Text(text) => {
                                    let preview: String = text.chars().take(100).collect();
                                    format!("Text: {}", preview)
                                }
                                ExecutionOutput::Json(value) => {
                                    let json_str = serde_json::to_string(value).unwrap_or_default();
                                    format!("Json: {}", json_str)
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
                        // Execution failed - refund everything except base fee
                        let refund = payment.0.saturating_sub(self.base_fee);
                        if refund > 0 {
                            near_sdk::Promise::new(sender_id.clone())
                                .transfer(NearToken::from_yoctonear(refund));
                        }

                        self.total_fees_collected += self.base_fee;

                        // Get error message for event and panic
                        let error_msg = exec_response.error.unwrap_or("Unknown error".to_string());

                        // Emit failure event with error details
                        events::emit::execution_completed(
                            &sender_id,
                            &code_source,
                            &exec_response.resources_used,
                            false,
                            Some(&error_msg),
                            U128(self.base_fee),  // payment_charged (only base fee)
                            U128(refund),         // payment_refunded
                            exec_response.compilation_note.as_deref(),
                        );

                        env::panic_str(&format!(
                            "Execution failed: {}. Resources: {{ instructions: {}, time_ms: {} }}. Refunded {} yoctoNEAR",
                            error_msg,
                            exec_response.resources_used.instructions,
                            exec_response.resources_used.time_ms,
                            refund
                        ));
                    }
                }
                Err(promise_error) => {
                    // Promise failed - refund everything except base fee
                    let refund = payment.0.saturating_sub(self.base_fee);
                    if refund > 0 {
                        near_sdk::Promise::new(sender_id.clone())
                            .transfer(NearToken::from_yoctonear(refund));
                    }

                    self.total_fees_collected += self.base_fee;

                    env::panic_str(&format!(
                        "Execution promise failed: {:?}. Refunded {} yoctoNEAR",
                        promise_error, refund
                    ));
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

        // Remove the request and refund the user
        if let Some(stale_request) = self.pending_requests.remove(&request_id) {
            near_sdk::Promise::new(stale_request.sender_id.clone())
                .transfer(NearToken::from_yoctonear(stale_request.payment));

            log!(
                "Cancelled stale execution {} and refunded user {}",
                request_id,
                stale_request.sender_id
            );
        }
    }
}

impl Contract {
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
