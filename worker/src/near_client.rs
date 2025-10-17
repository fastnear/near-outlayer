use anyhow::{Context, Result};
use near_crypto::InMemorySigner;
use near_jsonrpc_client::{methods, JsonRpcClient};
use near_primitives::transaction::{Action, FunctionCallAction, Transaction, TransactionV0};
use near_primitives::types::{AccountId, BlockReference, Finality};
use near_primitives::views::FinalExecutionOutcomeView;
use serde_json::{json, Value};
use std::time::Duration;
use tracing::{debug, info, warn};

use crate::api_client::{ExecutionOutput, ExecutionResult};

/// NEAR blockchain client for worker operations
pub struct NearClient {
    client: JsonRpcClient,
    signer: InMemorySigner,
    contract_id: AccountId,
}

impl NearClient {
    /// Create a new NEAR client
    ///
    /// # Arguments
    /// * `rpc_url` - NEAR RPC endpoint URL
    /// * `signer` - Signer for transactions
    /// * `contract_id` - OffchainVM contract account ID
    pub fn new(rpc_url: String, signer: InMemorySigner, contract_id: AccountId) -> Result<Self> {
        let client = JsonRpcClient::connect(&rpc_url);

        Ok(Self {
            client,
            signer,
            contract_id,
        })
    }

    /// Extract cost from transaction logs (parses [[yNEAR charged: "..."]] or estimated_cost)
    /// Parses the "Resolving execution" log from contract which contains estimated_cost
    /// Returns 0 if not found (will show as 0 NEAR in dashboard)
    #[allow(dead_code)]
    pub fn extract_payment_from_logs(outcome: &FinalExecutionOutcomeView) -> u128 {
        // Collect all logs from transaction and receipt outcomes
        let mut all_logs = Vec::new();

        info!("📋 Extracting estimated_cost from transaction logs...");

        // Logs from transaction itself
        info!("   Transaction outcome logs: {}", outcome.transaction_outcome.outcome.logs.len());
        all_logs.extend(outcome.transaction_outcome.outcome.logs.clone());

        // Logs from all receipts
        info!("   Receipt outcomes: {}", outcome.receipts_outcome.len());
        for (i, receipt_outcome) in outcome.receipts_outcome.iter().enumerate() {
            info!("   Receipt #{} executor: {}, logs: {}",
                i,
                receipt_outcome.outcome.executor_id,
                receipt_outcome.outcome.logs.len()
            );
            for (j, log) in receipt_outcome.outcome.logs.iter().enumerate() {
                let preview = if log.len() > 300 {
                    format!("{}...", &log[..300])
                } else {
                    log.clone()
                };
                info!("      Receipt #{} Log #{}: {}", i, j, preview);
            }
            all_logs.extend(receipt_outcome.outcome.logs.clone());
        }

        info!("   Total logs to parse: {}", all_logs.len());

        // Try to find "[[yNEAR charged: \"...\"]]" log (most reliable, set after refund calculation)
        for (i, log) in all_logs.iter().enumerate() {
            info!("   Log #{}: {}", i, if log.len() > 200 { &log[..200] } else { log });

            // Parse log format: [[yNEAR charged: "123456789"]] (exact final cost after refunds)
            if let Some(start) = log.find("[[yNEAR charged: \"") {
                info!("   ✓ Found '[[yNEAR charged]]' log");

                let after_prefix = &log[start + "[[yNEAR charged: \"".len()..];
                // Find closing quote
                if let Some(quote_end) = after_prefix.find('"') {
                    let cost_str = &after_prefix[..quote_end];

                    match cost_str.parse::<u128>() {
                        Ok(cost) => {
                            info!("💰 Successfully extracted yNEAR charged: {} yoctoNEAR ({:.6} NEAR)",
                                cost, cost as f64 / 1e24);
                            return cost;
                        }
                        Err(e) => {
                            warn!("   ❌ Failed to parse yNEAR charged '{}' as u128: {}", cost_str, e);
                        }
                    }
                }
            }

            // Fallback: Parse "estimated_cost" from resolve_execution log (before callback)
            if log.contains("Resolving execution") && log.contains("estimated_cost:") {
                info!("   ✓ Found 'Resolving execution' log with estimated_cost");

                // Extract the cost value using string parsing
                // Format: "estimated_cost: 12345678 yoctoNEAR"
                if let Some(cost_start) = log.find("estimated_cost: ") {
                    let after_prefix = &log[cost_start + "estimated_cost: ".len()..];
                    if let Some(space_pos) = after_prefix.find(' ') {
                        let cost_str = &after_prefix[..space_pos];
                        match cost_str.parse::<u128>() {
                            Ok(cost) => {
                                info!("💰 Successfully extracted estimated_cost: {} yoctoNEAR ({:.6} NEAR)",
                                    cost, cost as f64 / 1e24);
                                return cost;
                            }
                            Err(e) => {
                                warn!("   ❌ Failed to parse estimated_cost '{}' as u128: {}", cost_str, e);
                            }
                        }
                    }
                }
            }

            // Fallback: Also try EVENT_JSON for backwards compatibility
            if let Some(event_json) = log.strip_prefix("EVENT_JSON:") {
                info!("   ✓ Found EVENT_JSON, parsing...");

                match serde_json::from_str::<Value>(event_json) {
                    Ok(event) => {
                        if let Some(event_type) = event.get("event").and_then(|e| e.as_str()) {
                            if event_type == "execution_completed" {
                                info!("   ✓ Found execution_completed event!");

                                if let Some(data) = event.get("data").and_then(|d| d.as_array()) {
                                    if let Some(first_data) = data.first() {
                                        if let Some(payment_str) = first_data.get("payment_charged").and_then(|p| p.as_str()) {
                                            info!("   ✓ Found payment_charged: {}", payment_str);

                                            match payment_str.parse::<u128>() {
                                                Ok(payment) => {
                                                    info!("💰 Successfully extracted payment_charged from event: {} yoctoNEAR ({:.6} NEAR)",
                                                        payment, payment as f64 / 1e24);
                                                    return payment;
                                                }
                                                Err(e) => {
                                                    warn!("   ❌ Failed to parse payment_charged: {}", e);
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    Err(e) => {
                        warn!("   ❌ Failed to parse EVENT_JSON: {}", e);
                    }
                }
            }
        }

        warn!("⚠️  Contract did not provide estimated_cost in logs - will record as 0 NEAR");
        0
    }

    /// Submit execution result using optimized 1-transaction flow
    ///
    /// This method handles large outputs efficiently by calling the combined
    /// submit_execution_output_and_resolve method which:
    /// 1. Stores large output in contract storage
    /// 2. Creates internal promise to resolve_execution
    ///
    /// This saves ~1-2 seconds compared to two separate transactions.
    ///
    /// # Arguments
    /// * `request_id` - Request ID from the contract
    /// * `result` - Execution result with large output
    async fn submit_result_two_call_flow(
        &self,
        request_id: u64,
        result: &ExecutionResult,
    ) -> Result<(String, FinalExecutionOutcomeView)> {
        let output = result.output.as_ref().unwrap();

        // Prepare arguments for submit_execution_output_and_resolve
        let args = json!({
            "request_id": request_id,
            "output": output,
            "success": result.success,
            "error": result.error,
            "resources_used": {
                "instructions": result.instructions,
                "time_ms": result.execution_time_ms,
            }
        });

        let args_json = serde_json::to_string(&args)
            .context("Failed to serialize submit_execution_output_and_resolve args")?;

        info!(
            "📤 Submitting large output + resolve in ONE transaction (size: {} bytes)",
            args_json.len()
        );

        // Call the combined method (400 TGas: 100 for submit + 300 for internal resolve)
        let outcome = self
            .call_contract_method(
                "submit_execution_output_and_resolve",
                args_json.into_bytes(),
                300_000_000_000_000, // 300 TGas total
                0,
            )
            .await
            .context("Failed to call submit_execution_output_and_resolve")?;

        info!("✅ Combined transaction complete: {:?}", outcome.status);
        info!("   Transaction ID: {}", outcome.transaction_outcome.id);
        info!("   Initial receipt outcomes: {}", outcome.receipts_outcome.len());

        // Fetch full transaction status to get all nested receipts (including callbacks)
        info!("🔍 Fetching full transaction status with ALL nested receipts...");
        let full_outcome = self.fetch_all_receipts(&outcome.transaction_outcome.id, &self.signer.account_id).await?;
        info!("   Total receipt outcomes (including nested): {}", full_outcome.receipts_outcome.len());

        // Return transaction hash and FULL outcome
        let tx_hash = format!("{}", outcome.transaction_outcome.id);
        Ok((tx_hash, full_outcome))
    }

    /// Submit large execution output separately (legacy 2-call flow)
    ///
    /// This is kept as a fallback option. The recommended approach is to use
    /// submit_execution_output_and_resolve for better performance.
    ///
    /// # Arguments
    /// * `request_id` - Request ID from the contract
    /// * `output` - Execution output (bytes, text, or JSON)
    ///
    /// # Returns
    /// * `Ok(tx_hash)` - Transaction hash as hex string
    #[allow(dead_code)]
    async fn submit_execution_output(
        &self,
        request_id: u64,
        output: &ExecutionOutput,
    ) -> Result<String> {
        info!(
            "📤 Submitting large execution output separately: request_id={}",
            request_id
        );

        // Prepare method arguments matching contract signature:
        // submit_execution_output(request_id: u64, output: ExecutionOutput)
        let args = json!({
            "request_id": request_id,
            "output": output,
        });

        let args_json = serde_json::to_string(&args).context("Failed to serialize args")?;
        info!("📤 Full args for submit_execution_output (size: {} bytes)", args_json.len());

        // Send transaction (no deposit needed)
        let outcome = self
            .call_contract_method(
                "submit_execution_output",
                args_json.into_bytes(),
                100_000_000_000_000, // 100 TGas
                0, // No deposit
            )
            .await
            .context("Failed to call submit_execution_output")?;

        info!("✅ submit_execution_output transaction outcome status: {:?}", outcome.status);
        info!("   Transaction ID: {}", outcome.transaction_outcome.id);

        // Return transaction hash as hex string
        let tx_hash = format!("{}", outcome.transaction_outcome.id);
        Ok(tx_hash)
    }

    /// Submit execution result back to the NEAR contract
    ///
    /// Automatically decides between 1-call or 2-call flow based on payload size:
    /// - If payload < 1024 bytes: calls `resolve_execution` directly (1-call)
    /// - If payload >= 1024 bytes: calls `submit_execution_output_and_resolve` (optimized 1-transaction flow)
    ///
    /// # Arguments
    /// * `request_id` - Request ID from the contract
    /// * `result` - Execution result from WASM executor
    ///
    /// # Returns
    /// * `Ok((tx_hash, outcome))` - Transaction hash and full execution outcome
    pub async fn submit_execution_result(
        &self,
        request_id: u64,
        result: &ExecutionResult,
    ) -> Result<(String, FinalExecutionOutcomeView)> {
        info!(
            "📡 Submitting execution result: request_id={}, success={}",
            request_id, result.success
        );

        // Check payload size to decide between 1-call or 2-call flow
        // Build full ExecutionResponse to estimate payload size
        let full_response = json!({
            "success": result.success,
            "output": result.output,
            "error": result.error,
            "resources_used": {
                "instructions": result.instructions,
                "time_ms": result.execution_time_ms,
            }
        });

        let response_json = serde_json::to_string(&full_response)
            .context("Failed to serialize response")?;

        const PAYLOAD_LIMIT: usize = 1024;
        let payload_size = response_json.len();

        info!("📊 Response payload size: {} bytes (limit: {} bytes)", payload_size, PAYLOAD_LIMIT);

        // Decide flow based on payload size
        let use_two_call_flow = payload_size >= PAYLOAD_LIMIT && result.success && result.output.is_some();

        if use_two_call_flow {
            info!("⚠️  Payload exceeds limit, using 2-call flow (submit_execution_output + resolve_execution)");

            // For 2-call flow, we need to manage nonce manually to avoid nonce conflicts
            // Get current nonce and block_hash once, then use nonce+1 for second transaction
            return self.submit_result_two_call_flow(request_id, result).await;
        } else {
            info!("✅ Payload size OK, using 1-call flow (resolve_execution only)");
        }

        // 1-call flow: Prepare method arguments for resolve_execution with output
        let args = json!({
            "request_id": request_id,
            "response": {
                "success": result.success,
                "output": result.output,
                "error": result.error,
                "resources_used": {
                    "instructions": result.instructions,
                    "time_ms": result.execution_time_ms,
                }
            }
        });

        let args_json = serde_json::to_string(&args).context("Failed to serialize args")?;
        info!("📤 resolve_execution args (1-call flow, with output): size={} bytes", args_json.len());

        // Send transaction
        info!("🔗 Sending resolve_execution transaction:");
        info!("   Contract: {}", self.contract_id);
        info!("   Signer: {}", self.signer.account_id);
        info!("   Method: resolve_execution");
        info!("   Gas: 300 TGas");

        let outcome = self
            .call_contract_method(
                "resolve_execution",
                args_json.into_bytes(),
                300_000_000_000_000, // 300 TGas (increased for yield resume)
                0,                    // No attached deposit
            )
            .await
            .context("Failed to call resolve_execution")?;

        info!("✅ Transaction outcome status: {:?}", outcome.status);
        info!("   Transaction ID: {}", outcome.transaction_outcome.id);
        info!("   Receipt outcomes: {}", outcome.receipts_outcome.len());

        // Log receipt details for debugging
        for (i, receipt) in outcome.receipts_outcome.iter().enumerate() {
            info!("   Receipt #{}: executor={}, logs={}",
                i, receipt.outcome.executor_id, receipt.outcome.logs.len());
            for (j, log) in receipt.outcome.logs.iter().enumerate() {
                info!("      Log #{}: {}", j, log);
            }
        }

        // Return transaction hash and outcome with receipt logs
        // Note: The estimated_cost is in the resolve_execution receipt logs
        let tx_hash = format!("{}", outcome.transaction_outcome.id);
        Ok((tx_hash, outcome))
    }

    /// Call a contract method with explicit nonce
    async fn call_contract_method_with_nonce(
        &self,
        method_name: &str,
        args: Vec<u8>,
        gas: u64,
        deposit: u128,
        nonce: u64,
        block_hash: near_primitives::hash::CryptoHash,
    ) -> Result<FinalExecutionOutcomeView> {
        // Create transaction using V0 format (no priority_fee)
        let transaction_v0 = TransactionV0 {
            signer_id: self.signer.account_id.clone(),
            public_key: self.signer.public_key(),
            nonce,
            receiver_id: self.contract_id.clone(),
            block_hash,
            actions: vec![Action::FunctionCall(Box::new(FunctionCallAction {
                method_name: method_name.to_string(),
                args,
                gas,
                deposit,
            }))],
        };

        let transaction = Transaction::V0(transaction_v0);

        // Sign transaction
        let signature = self.signer.sign(transaction.get_hash_and_size().0.as_ref());
        let signed_transaction = near_primitives::transaction::SignedTransaction::new(
            signature,
            transaction,
        );
        let hash = signed_transaction.get_hash();

        // Broadcast transaction with commit (wait for finality)
        let tx_request = methods::broadcast_tx_commit::RpcBroadcastTxCommitRequest {
            signed_transaction,
        };

        debug!("Broadcasting transaction with commit: {:?}", hash);

        let outcome = self
            .client
            .call(tx_request)
            .await
            .context("Failed to broadcast transaction and wait for commit")?;

        debug!("Transaction committed: {:?}", hash);

        Ok(outcome)
    }

    /// Call a contract method
    async fn call_contract_method(
        &self,
        method_name: &str,
        args: Vec<u8>,
        gas: u64,
        deposit: u128,
    ) -> Result<FinalExecutionOutcomeView> {
        // Get account access key for nonce
        let access_key_query = methods::query::RpcQueryRequest {
            block_reference: BlockReference::Finality(Finality::Final),
            request: near_primitives::views::QueryRequest::ViewAccessKey {
                account_id: self.signer.account_id.clone(),
                public_key: self.signer.public_key(),
            },
        };

        let access_key_response = self
            .client
            .call(access_key_query)
            .await
            .context("Failed to query access key")?;

        let current_nonce = match access_key_response.kind {
            near_jsonrpc_primitives::types::query::QueryResponseKind::AccessKey(access_key) => {
                access_key.nonce
            }
            _ => anyhow::bail!("Unexpected query response"),
        };

        // Get latest block hash
        let block_query = methods::block::RpcBlockRequest {
            block_reference: BlockReference::Finality(Finality::Final),
        };

        let block = self
            .client
            .call(block_query)
            .await
            .context("Failed to query block")?;

        let block_hash = block.header.hash;

        // Create transaction using V0 format (no priority_fee)
        let transaction_v0 = TransactionV0 {
            signer_id: self.signer.account_id.clone(),
            public_key: self.signer.public_key(),
            nonce: current_nonce + 1,
            receiver_id: self.contract_id.clone(),
            block_hash,
            actions: vec![Action::FunctionCall(Box::new(FunctionCallAction {
                method_name: method_name.to_string(),
                args,
                gas,
                deposit,
            }))],
        };

        let transaction = Transaction::V0(transaction_v0);

        // Sign transaction
        let signature = self.signer.sign(transaction.get_hash_and_size().0.as_ref());
        let signed_transaction = near_primitives::transaction::SignedTransaction::new(
            signature,
            transaction,
        );
        let hash = signed_transaction.get_hash();

        // Broadcast transaction with commit (wait for finality)
        let tx_request = methods::broadcast_tx_commit::RpcBroadcastTxCommitRequest {
            signed_transaction,
        };

        debug!("Broadcasting transaction with commit: {:?}", hash);

        let outcome = self
            .client
            .call(tx_request)
            .await
            .context("Failed to broadcast transaction and wait for commit")?;

        debug!("Transaction committed: {:?}", hash);

        Ok(outcome)
    }

    /// Fetch transaction with retry to get ALL receipts including callbacks
    /// promise_yield_resume creates nested callbacks that may not be included immediately
    async fn fetch_all_receipts(
        &self,
        tx_hash: &near_primitives::hash::CryptoHash,
        sender_id: &AccountId,
    ) -> Result<FinalExecutionOutcomeView> {
        // Wait for initial finalization
        let mut outcome = self.wait_for_transaction(tx_hash, sender_id).await?;
        let initial_receipts = outcome.receipts_outcome.len();

        info!("🔄 Waiting for all nested receipts to complete...");
        info!("   Initial receipts: {}", initial_receipts);

        // Retry fetching transaction status to get nested receipts
        // Nested receipts (callbacks) may take additional time to execute
        for retry in 0..10 {
            tokio::time::sleep(Duration::from_millis(500)).await;

            match self.wait_for_transaction(tx_hash, sender_id).await {
                Ok(new_outcome) => {
                    let new_count = new_outcome.receipts_outcome.len();

                    if new_count > outcome.receipts_outcome.len() {
                        info!("   Retry #{}: Found {} receipts (was {})", retry + 1, new_count, outcome.receipts_outcome.len());
                        outcome = new_outcome;

                        // If we found the execution_completed event, we're done
                        if Self::has_execution_completed_event(&outcome) {
                            info!("   ✓ Found execution_completed event!");
                            break;
                        }
                    }
                }
                Err(e) => {
                    warn!("   Retry #{}: Failed to fetch: {}", retry + 1, e);
                }
            }
        }

        info!("   Final receipts: {}", outcome.receipts_outcome.len());
        Ok(outcome)
    }

    /// Check if outcome contains execution_completed event or yNEAR charged log
    fn has_execution_completed_event(outcome: &FinalExecutionOutcomeView) -> bool {
        for receipt_outcome in &outcome.receipts_outcome {
            for log in &receipt_outcome.outcome.logs {
                // Check for yNEAR charged log (appears first, most reliable)
                if log.contains("[[yNEAR charged:") {
                    return true;
                }
                // Fallback: check for EVENT_JSON
                if log.contains("EVENT_JSON:") && log.contains("execution_completed") {
                    return true;
                }
            }
        }
        false
    }

    /// Wait for transaction to be finalized
    async fn wait_for_transaction(
        &self,
        tx_hash: &near_primitives::hash::CryptoHash,
        sender_id: &AccountId,
    ) -> Result<FinalExecutionOutcomeView> {
        // Poll for transaction result (up to 60 seconds)
        for _ in 0..60 {
            let tx_request = methods::tx::RpcTransactionStatusRequest {
                transaction_info: methods::tx::TransactionInfo::TransactionId {
                    tx_hash: tx_hash.clone(),
                    sender_account_id: sender_id.clone(),
                },
                wait_until: near_primitives::views::TxExecutionStatus::Final,
            };

            match self.client.call(tx_request).await {
                Ok(outcome) => {
                    debug!("Transaction finalized: {}", tx_hash);
                    // Extract FinalExecutionOutcomeView from the response
                    match outcome.final_execution_outcome {
                        Some(near_primitives::views::FinalExecutionOutcomeViewEnum::FinalExecutionOutcome(view)) => {
                            return Ok(view);
                        }
                        Some(near_primitives::views::FinalExecutionOutcomeViewEnum::FinalExecutionOutcomeWithReceipt(view)) => {
                            return Ok(view.final_outcome);
                        }
                        None => {
                            anyhow::bail!("No final execution outcome in response");
                        }
                    }
                }
                Err(e) => {
                    // Check if it's a timeout error (transaction not yet processed)
                    if e.to_string().contains("UNKNOWN_TRANSACTION")
                        || e.to_string().contains("timeout")
                    {
                        debug!("Transaction not yet processed, waiting...");
                        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
                        continue;
                    }
                    return Err(e).context("Transaction failed");
                }
            }
        }

        anyhow::bail!("Transaction timeout: {}", tx_hash)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use near_crypto::SecretKey;

    #[test]
    fn test_near_client_creation() {
        let signer = InMemorySigner::from_secret_key(
            "worker.testnet".parse().unwrap(),
            "ed25519:3D4YudUahN1nawWvHfEKBGpmJLfbCTbvdXDJKqfLhQ98XewyWK4tEDWvmAYPZqcgz7qfkCEHyWD15m8JVVWJ3LXD"
                .parse::<SecretKey>()
                .unwrap(),
        );

        let client = NearClient::new(
            "https://rpc.testnet.near.org".to_string(),
            signer,
            "offchainvm.testnet".parse().unwrap(),
        );

        assert!(client.is_ok());
    }
}
