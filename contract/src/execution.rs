use crate::*;
use near_sdk::serde_json::json;

#[near_bindgen]
impl Contract {
    /// Request off-chain execution
    #[payable]
    pub fn request_execution(
        &mut self,
        code_source: CodeSource,
        resource_limits: Option<ResourceLimits>,
        input_data: Option<String>,
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

        // Create execution request data for yield
        let request_data = json!({
            "request_id": request_id,
            "sender_id": sender_id,
            "code_source": code_source,
            "resource_limits": limits,
            "input_data": input_data.unwrap_or_else(|| "{}".to_string()),
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
        };

        self.pending_requests
            .insert(&request_id, &execution_request);

        // Emit event for workers to catch
        events::emit::execution_requested(&request_data.to_string(), data_id);

        // Return the promise to pause execution
        env::promise_return(promise_idx)
    }

    /// Worker calls this to provide execution result
    pub fn resolve_execution(&mut self, data_id: CryptoHash, response: ExecutionResponse) {
        // Only operator can resolve executions
        self.assert_operator();

        log!(
            "Resolving execution with data_id: {:?}, success: {}, resources_used: {{ instructions: {}, time_ms: {} }}",
            data_id,
            response.success,
            response.resources_used.instructions,
            response.resources_used.time_ms
        );

        // Resume the yield promise with the response
        if !env::promise_yield_resume(&data_id, &serde_json::to_vec(&response).unwrap()) {
            env::panic_str("Unable to resume execution promise");
        }
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
    ) -> Option<String> {
        // Remove the pending request
        if let Some(_request) = self.pending_requests.remove(&request_id) {
            self.total_executions += 1;

            match response {
                Ok(exec_response) => {
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

                        // Emit success event
                        events::emit::execution_completed(
                            &sender_id,
                            &code_source,
                            &exec_response.resources_used,
                            true,
                        );

                        // Log the execution result with resources used
                        if let Some(return_value) = &exec_response.return_value {
                            match String::from_utf8(return_value.clone()) {
                                Ok(result_str) => {
                                    log!(
                                        "Execution completed successfully. Result: {}, Resources: {{ instructions: {}, time_ms: {} }}, Cost: {} yoctoNEAR, Refund: {} yoctoNEAR",
                                        result_str,
                                        exec_response.resources_used.instructions,
                                        exec_response.resources_used.time_ms,
                                        cost,
                                        refund
                                    );

                                    Some(result_str)
                                }
                                Err(e) => {
                                    log!(
                                        "Execution returned non-UTF8 data: {:?}. Resources: {{ instructions: {}, time_ms: {} }}, Cost: {} yoctoNEAR, Refund: {} yoctoNEAR",
                                        e,
                                        exec_response.resources_used.instructions,
                                        exec_response.resources_used.time_ms,
                                        cost,
                                        refund
                                    );

                                    None
                                }
                            }
                        } else {
                            log!(
                                "Execution has no output value. Resources: {{ instructions: {}, time_ms: {} }}, Cost: {} yoctoNEAR, Refund: {} yoctoNEAR",
                                exec_response.resources_used.instructions,
                                exec_response.resources_used.time_ms,
                                cost,
                                refund
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

                        // Emit failure event
                        events::emit::execution_completed(
                            &sender_id,
                            &code_source,
                            &exec_response.resources_used,
                            false,
                        );

                        env::panic_str(&format!(
                            "Execution failed: {}. Resources: {{ instructions: {}, time_ms: {} }}. Refunded {} yoctoNEAR",
                            exec_response.error.unwrap_or("Unknown error".to_string()),
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
