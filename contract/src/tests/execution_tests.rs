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

        contract.request_execution(code_source, None, None);
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

        contract.request_execution(code_source, None, None);
    }
}
