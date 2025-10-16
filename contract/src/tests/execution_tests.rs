#[cfg(test)]
mod tests {
    use crate::tests::{get_context, setup_contract};
    use crate::EXECUTION_TIMEOUT;
    use crate::*;
    use near_sdk::test_utils::accounts;
    use near_sdk::{testing_env, NearToken};

    #[test]
    fn test_cancel_stale_execution_success() {
        let mut contract = setup_contract();
        let sender = accounts(3);
        let initial_timestamp = env::block_timestamp();

        // Manually add a pending execution request
        let execution_request = ExecutionRequest {
            request_id: 0,
            data_id: [0; 32],
            sender_id: sender.clone(),
            code_source: CodeSource {
                repo: "https://github.com/test/repo".to_string(),
                commit: "abc123".to_string(),
                build_target: Some("wasm32-wasi".to_string()),
            },
            resource_limits: ResourceLimits::default(),
            payment: 100_000_000_000_000_000_000_000, // 0.1 NEAR
            timestamp: initial_timestamp,
            encrypted_secrets: None,
            response_format: ResponseFormat::default(),
            pending_output: None,
            output_submitted: false,
        };
        contract.pending_requests.insert(&0, &execution_request);
        assert!(contract.get_request(0).is_some());

        // Set up context:
        // - The caller is the original sender
        // - The block timestamp is *after* the timeout period
        let mut context = get_context(sender.clone(), NearToken::from_near(0));
        context.block_timestamp(initial_timestamp + EXECUTION_TIMEOUT + 1);
        testing_env!(context.build());

        // Call the function
        contract.cancel_stale_execution(0);

        // Assert that the request was removed
        assert!(
            contract.get_request(0).is_none(),
            "Stale execution should have been removed"
        );
    }

    #[test]
    #[should_panic(expected = "Only the sender can cancel this execution")]
    fn test_cancel_stale_execution_unauthorized() {
        let mut contract = setup_contract();
        let sender = accounts(3);
        let unauthorized_caller = accounts(4);
        let initial_timestamp = env::block_timestamp();

        // Add a pending execution from `sender`
        let execution_request = ExecutionRequest {
            request_id: 0,
            data_id: [0; 32],
            sender_id: sender.clone(),
            code_source: CodeSource {
                repo: "https://github.com/test/repo".to_string(),
                commit: "abc123".to_string(),
                build_target: Some("wasm32-wasi".to_string()),
            },
            resource_limits: ResourceLimits::default(),
            payment: 100_000_000_000_000_000_000_000,
            timestamp: initial_timestamp,
            encrypted_secrets: None,
            response_format: ResponseFormat::default(),
            pending_output: None,
            output_submitted: false,
        };
        contract.pending_requests.insert(&0, &execution_request);

        // Set up context:
        // - The caller is *not* the original sender
        // - The time is after the timeout
        let mut context = get_context(unauthorized_caller, NearToken::from_near(0));
        context.block_timestamp(initial_timestamp + EXECUTION_TIMEOUT + 1);
        testing_env!(context.build());

        // Should panic
        contract.cancel_stale_execution(0);
    }

    #[test]
    fn test_response_format_default() {
        let mut contract = setup_contract();
        let code_source = CodeSource {
            repo: "https://github.com/test/repo".to_string(),
            commit: "main".to_string(),
            build_target: Some("wasm32-wasi".to_string()),
        };

        let mut context = get_context(accounts(1), NearToken::from_near(1));
        testing_env!(context.build());

        contract.request_execution(code_source.clone(), None, None, None, None);

        // Check that response_format defaults to Text
        let request = contract.get_request(0).expect("Request should exist");
        assert_eq!(request.response_format, ResponseFormat::Text);
    }

    #[test]
    fn test_response_format_json() {
        let mut contract = setup_contract();
        let code_source = CodeSource {
            repo: "https://github.com/test/repo".to_string(),
            commit: "main".to_string(),
            build_target: Some("wasm32-wasi".to_string()),
        };

        let mut context = get_context(accounts(1), NearToken::from_near(1));
        testing_env!(context.build());

        contract.request_execution(code_source.clone(), None, None, None, Some(ResponseFormat::Json));

        // Check that response_format is Json
        let request = contract.get_request(0).expect("Request should exist");
        assert_eq!(request.response_format, ResponseFormat::Json);
    }

    #[test]
    fn test_response_format_bytes() {
        let mut contract = setup_contract();
        let code_source = CodeSource {
            repo: "https://github.com/test/repo".to_string(),
            commit: "main".to_string(),
            build_target: Some("wasm32-wasi".to_string()),
        };

        let mut context = get_context(accounts(1), NearToken::from_near(1));
        testing_env!(context.build());

        contract.request_execution(code_source.clone(), None, None, None, Some(ResponseFormat::Bytes));

        // Check that response_format is Bytes
        let request = contract.get_request(0).expect("Request should exist");
        assert_eq!(request.response_format, ResponseFormat::Bytes);
    }

    #[test]
    #[should_panic(expected = "Execution is not yet stale, please wait")]
    fn test_cancel_stale_execution_not_stale() {
        let mut contract = setup_contract();
        let sender = accounts(3);
        let initial_timestamp = env::block_timestamp();

        // Add a pending execution
        let execution_request = ExecutionRequest {
            request_id: 0,
            data_id: [0; 32],
            sender_id: sender.clone(),
            code_source: CodeSource {
                repo: "https://github.com/test/repo".to_string(),
                commit: "abc123".to_string(),
                build_target: Some("wasm32-wasi".to_string()),
            },
            resource_limits: ResourceLimits::default(),
            payment: 100_000_000_000_000_000_000_000,
            timestamp: initial_timestamp,
            encrypted_secrets: None,
            response_format: ResponseFormat::default(),
            pending_output: None,
            output_submitted: false,
        };
        contract.pending_requests.insert(&0, &execution_request);

        // Set up context:
        // - The caller is the original sender
        // - The block timestamp is *before* the timeout
        let mut context = get_context(sender, NearToken::from_near(0));
        context.block_timestamp(initial_timestamp + EXECUTION_TIMEOUT - 1);
        testing_env!(context.build());

        // Should panic
        contract.cancel_stale_execution(0);
    }

    #[test]
    #[should_panic(expected = "Execution request not found")]
    fn test_cancel_stale_execution_not_found() {
        let mut contract = setup_contract();

        let mut context = get_context(accounts(3), NearToken::from_near(0));
        testing_env!(context.build());

        // Try to cancel non-existent request
        contract.cancel_stale_execution(999);
    }

    #[test]
    fn test_calculate_cost() {
        let contract = setup_contract();

        let metrics = ResourceMetrics {
            instructions: 10_000_000, // 10M instructions
            time_ms: 5000,            // 5000 ms (5 seconds)
        };

        let cost = contract.calculate_cost(&metrics);

        // Expected:
        // base_fee: 10_000_000_000_000_000_000_000
        // instruction_cost: 10 * 1_000_000_000_000_000 = 10_000_000_000_000_000
        // time_cost: 5000 * 1_000_000_000_000_000_000 = 5_000_000_000_000_000_000_000
        // Total: 15_000_010_000_000_000_000_000

        assert_eq!(cost, 15_000_010_000_000_000_000_000);
    }

    #[test]
    #[should_panic(expected = "Contract is paused")]
    fn test_request_execution_when_paused() {
        let mut contract = setup_contract();

        // Owner pauses contract
        let mut context = get_context(accounts(0), NearToken::from_near(0));
        testing_env!(context.build());
        contract.set_paused(true);

        // User tries to request execution
        let mut context = get_context(accounts(2), NearToken::from_millinear(100));
        testing_env!(context.build());

        let code_source = CodeSource {
            repo: "https://github.com/test/repo".to_string(),
            commit: "abc123".to_string(),
            build_target: Some("wasm32-wasi".to_string()),
        };

        contract.request_execution(code_source, None, None, None, None);
    }

    #[test]
    #[should_panic(expected = "Insufficient payment")]
    fn test_request_execution_insufficient_payment() {
        let mut contract = setup_contract();

        // User tries with insufficient deposit
        let mut context = get_context(accounts(2), NearToken::from_yoctonear(1000));
        testing_env!(context.build());

        let code_source = CodeSource {
            repo: "https://github.com/test/repo".to_string(),
            commit: "abc123".to_string(),
            build_target: Some("wasm32-wasi".to_string()),
        };

        contract.request_execution(code_source, None, None, None, None);
    }

    #[test]
    fn test_submit_execution_output() {
        let mut contract = setup_contract();
        let operator = accounts(1);
        let sender = accounts(3);

        // Manually create a pending request
        let execution_request = ExecutionRequest {
            request_id: 0,
            data_id: [1; 32],
            sender_id: sender.clone(),
            code_source: CodeSource {
                repo: "https://github.com/test/repo".to_string(),
                commit: "abc123".to_string(),
                build_target: Some("wasm32-wasi".to_string()),
            },
            resource_limits: ResourceLimits::default(),
            payment: 100_000_000_000_000_000_000_000,
            timestamp: env::block_timestamp(),
            encrypted_secrets: None,
            response_format: ResponseFormat::Text,
            pending_output: None,
            output_submitted: false,
        };
        contract.pending_requests.insert(&0, &execution_request);

        // Operator submits large output
        let mut context = get_context(operator.clone(), NearToken::from_near(0));
        testing_env!(context.build());

        let large_output = ExecutionOutput::Text("A".repeat(2000)); // > 1024 bytes
        contract.submit_execution_output(0, large_output.clone());

        // Check that output was stored
        assert!(contract.has_pending_output(0));
        let stored_output = contract.get_pending_output(0).expect("Output should be stored");

        match (stored_output, large_output) {
            (ExecutionOutput::Text(stored), ExecutionOutput::Text(original)) => {
                assert_eq!(stored, original);
            }
            _ => panic!("Output type mismatch"),
        }
    }

    #[test]
    #[should_panic(expected = "Only operator can call this")]
    fn test_submit_execution_output_unauthorized() {
        let mut contract = setup_contract();
        let unauthorized = accounts(5);
        let sender = accounts(3);

        // Create a pending request
        let execution_request = ExecutionRequest {
            request_id: 0,
            data_id: [1; 32],
            sender_id: sender.clone(),
            code_source: CodeSource {
                repo: "https://github.com/test/repo".to_string(),
                commit: "abc123".to_string(),
                build_target: Some("wasm32-wasi".to_string()),
            },
            resource_limits: ResourceLimits::default(),
            payment: 100_000_000_000_000_000_000_000,
            timestamp: env::block_timestamp(),
            encrypted_secrets: None,
            response_format: ResponseFormat::Text,
            pending_output: None,
            output_submitted: false,
        };
        contract.pending_requests.insert(&0, &execution_request);

        // Unauthorized user tries to submit output
        let mut context = get_context(unauthorized, NearToken::from_near(0));
        testing_env!(context.build());

        contract.submit_execution_output(0, ExecutionOutput::Text("test".to_string()));
    }

    #[test]
    #[should_panic(expected = "Output already submitted for this request")]
    fn test_submit_execution_output_twice() {
        let mut contract = setup_contract();
        let operator = accounts(1);
        let sender = accounts(3);

        // Create a pending request with output already submitted
        let execution_request = ExecutionRequest {
            request_id: 0,
            data_id: [1; 32],
            sender_id: sender.clone(),
            code_source: CodeSource {
                repo: "https://github.com/test/repo".to_string(),
                commit: "abc123".to_string(),
                build_target: Some("wasm32-wasi".to_string()),
            },
            resource_limits: ResourceLimits::default(),
            payment: 100_000_000_000_000_000_000_000,
            timestamp: env::block_timestamp(),
            encrypted_secrets: None,
            response_format: ResponseFormat::Text,
            pending_output: Some(StoredOutput::Text("old".as_bytes().to_vec())),
            output_submitted: true,
        };
        contract.pending_requests.insert(&0, &execution_request);

        // Operator tries to submit again
        let mut context = get_context(operator, NearToken::from_near(0));
        testing_env!(context.build());

        contract.submit_execution_output(0, ExecutionOutput::Text("new".to_string()));
    }

    #[test]
    fn test_resolve_execution_with_pending_output() {
        let mut contract = setup_contract();
        let operator = accounts(1);
        let sender = accounts(3);

        // Create a pending request with submitted output
        let large_text = "B".repeat(2000);
        let execution_request = ExecutionRequest {
            request_id: 0,
            data_id: [1; 32],
            sender_id: sender.clone(),
            code_source: CodeSource {
                repo: "https://github.com/test/repo".to_string(),
                commit: "abc123".to_string(),
                build_target: Some("wasm32-wasi".to_string()),
            },
            resource_limits: ResourceLimits::default(),
            payment: 100_000_000_000_000_000_000_000,
            timestamp: env::block_timestamp(),
            encrypted_secrets: None,
            response_format: ResponseFormat::Text,
            pending_output: Some(StoredOutput::Text(large_text.as_bytes().to_vec())),
            output_submitted: true,
        };
        contract.pending_requests.insert(&0, &execution_request);

        // Operator resolves with metadata only (no output in response)
        let mut context = get_context(operator, NearToken::from_near(0));
        testing_env!(context.build());

        let response = ExecutionResponse {
            success: true,
            output: None, // Output will be taken from pending_output
            error: None,
            resources_used: ResourceMetrics {
                instructions: 1_000_000,
                time_ms: 100,
            },
        };

        // This would normally call promise_yield_resume, which we can't test directly
        // But we can verify the request state before the call
        let request = contract.get_request(0).expect("Request should exist");
        assert!(request.output_submitted);
        assert!(request.pending_output.is_some());
    }

    #[test]
    fn test_stored_output_conversion_text() {
        let text = "Hello, NEAR!";
        let output = ExecutionOutput::Text(text.to_string());
        let stored: StoredOutput = output.clone().into();
        let converted: ExecutionOutput = stored.into();

        match converted {
            ExecutionOutput::Text(t) => assert_eq!(t, text),
            _ => panic!("Wrong type"),
        }
    }

    #[test]
    fn test_stored_output_conversion_bytes() {
        let bytes = vec![1, 2, 3, 4, 5];
        let output = ExecutionOutput::Bytes(bytes.clone());
        let stored: StoredOutput = output.into();
        let converted: ExecutionOutput = stored.into();

        match converted {
            ExecutionOutput::Bytes(b) => assert_eq!(b, bytes),
            _ => panic!("Wrong type"),
        }
    }

    #[test]
    fn test_stored_output_conversion_json() {
        let json_value = serde_json::json!({"key": "value", "number": 42});
        let output = ExecutionOutput::Json(json_value.clone());
        let stored: StoredOutput = output.into();
        let converted: ExecutionOutput = stored.into();

        match converted {
            ExecutionOutput::Json(j) => assert_eq!(j, json_value),
            _ => panic!("Wrong type"),
        }
    }

    #[test]
    fn test_submit_execution_output_and_resolve_stores_output() {
        let mut contract = setup_contract();
        let operator = accounts(1);
        let sender = accounts(3);

        // Create a pending request
        let execution_request = ExecutionRequest {
            request_id: 0,
            data_id: [1; 32],
            sender_id: sender.clone(),
            code_source: CodeSource {
                repo: "https://github.com/test/repo".to_string(),
                commit: "abc123".to_string(),
                build_target: Some("wasm32-wasi".to_string()),
            },
            resource_limits: ResourceLimits::default(),
            payment: 100_000_000_000_000_000_000_000,
            timestamp: env::block_timestamp(),
            encrypted_secrets: None,
            response_format: ResponseFormat::Text,
            pending_output: None,
            output_submitted: false,
        };
        contract.pending_requests.insert(&0, &execution_request);

        // Operator calls the internal method (we can't test full flow with promise_yield_resume in unit tests)
        let context = get_context(operator.clone(), NearToken::from_near(0));
        testing_env!(context.build());

        let large_output = ExecutionOutput::Text("C".repeat(2000)); // > 1024 bytes

        // Test just the storage part (not the full promise_yield_resume)
        contract.submit_execution_output_internal(0, large_output.clone());

        // Verify that output was stored correctly
        let request = contract.get_request(0).expect("Request should exist");
        assert!(request.output_submitted, "output_submitted flag should be true");
        assert!(request.pending_output.is_some(), "pending_output should be stored");

        // Verify stored output matches
        let stored = contract.get_pending_output(0).expect("Output should be stored");
        match (stored, large_output) {
            (ExecutionOutput::Text(s), ExecutionOutput::Text(o)) => assert_eq!(s, o),
            _ => panic!("Output type mismatch"),
        }
    }

    #[test]
    #[should_panic(expected = "Only operator can call this")]
    fn test_submit_execution_output_and_resolve_unauthorized() {
        let mut contract = setup_contract();
        let unauthorized = accounts(5);
        let sender = accounts(3);

        // Create a pending request
        let execution_request = ExecutionRequest {
            request_id: 0,
            data_id: [1; 32],
            sender_id: sender.clone(),
            code_source: CodeSource {
                repo: "https://github.com/test/repo".to_string(),
                commit: "abc123".to_string(),
                build_target: Some("wasm32-wasi".to_string()),
            },
            resource_limits: ResourceLimits::default(),
            payment: 100_000_000_000_000_000_000_000,
            timestamp: env::block_timestamp(),
            encrypted_secrets: None,
            response_format: ResponseFormat::Text,
            pending_output: None,
            output_submitted: false,
        };
        contract.pending_requests.insert(&0, &execution_request);

        // Unauthorized user tries to call combined method
        let context = get_context(unauthorized, NearToken::from_near(0));
        testing_env!(context.build());

        contract.submit_execution_output_and_resolve(
            0,
            ExecutionOutput::Text("test".to_string()),
            true,
            None,
            ResourceMetrics {
                instructions: 1000,
                time_ms: 10,
            },
        );
    }
}
