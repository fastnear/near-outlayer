use anyhow::{Context, Result};
use near_crypto::InMemorySigner;
use near_jsonrpc_client::{methods, JsonRpcClient};
use near_primitives::transaction::{Action, FunctionCallAction, Transaction, TransactionV0};
use near_primitives::types::{AccountId, BlockReference, Finality};
use near_primitives::views::FinalExecutionOutcomeView;
use serde_json::json;
use std::time::Duration;
use tracing::{debug, info};

use crate::api_client::{ExecutionOutput, ExecutionResult};

/// Delay between submit_execution_output and resolve_execution in 2-call flow (milliseconds)
/// Set to 0 to rely on finalization waiting without additional delay
const TWO_CALL_DELAY_MS: u64 = 0;

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

    /// Submit execution result using 2-call flow with manual nonce management
    ///
    /// This method handles large outputs by:
    /// 1. Getting current nonce and block_hash
    /// 2. Sending submit_execution_output with nonce+1
    /// 3. Sending resolve_execution with nonce+2 (avoiding nonce conflict)
    ///
    /// # Arguments
    /// * `request_id` - Request ID from the contract
    /// * `result` - Execution result with large output
    async fn submit_result_two_call_flow(
        &self,
        request_id: u64,
        result: &ExecutionResult,
    ) -> Result<String> {
        // Get current nonce and block_hash once
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

        info!("📋 2-call flow: current_nonce={}, using nonce={} and nonce={}",
            current_nonce, current_nonce + 1, current_nonce + 2);

        // Step 1: submit_execution_output with nonce+1
        let output = result.output.as_ref().unwrap();
        let output_args = json!({
            "request_id": request_id,
            "output": output,
        });

        let output_args_json = serde_json::to_string(&output_args)
            .context("Failed to serialize submit_execution_output args")?;

        info!("📤 Step 1/2: Submitting large output (size: {} bytes, nonce={})",
            output_args_json.len(), current_nonce + 1);

        let outcome1 = self
            .call_contract_method_with_nonce(
                "submit_execution_output",
                output_args_json.into_bytes(),
                100_000_000_000_000, // 100 TGas
                0,
                current_nonce + 1,
                block_hash,
            )
            .await
            .context("Failed to call submit_execution_output")?;

        info!("✅ Step 1/2 complete: {:?}", outcome1.status);

        // Optional delay
        if TWO_CALL_DELAY_MS > 0 {
            info!("⏳ Waiting {}ms between transactions", TWO_CALL_DELAY_MS);
            tokio::time::sleep(Duration::from_millis(TWO_CALL_DELAY_MS)).await;
        }

        // Step 2: resolve_execution with nonce+2 (no output, already stored)
        let resolve_args = json!({
            "request_id": request_id,
            "response": {
                "success": result.success,
                "output": null, // Output already submitted
                "error": result.error,
                "resources_used": {
                    "instructions": result.instructions,
                    "time_ms": result.execution_time_ms,
                }
            }
        });

        let resolve_args_json = serde_json::to_string(&resolve_args)
            .context("Failed to serialize resolve_execution args")?;

        info!("📤 Step 2/2: Resolving execution (size: {} bytes, nonce={})",
            resolve_args_json.len(), current_nonce + 2);

        let outcome2 = self
            .call_contract_method_with_nonce(
                "resolve_execution",
                resolve_args_json.into_bytes(),
                300_000_000_000_000, // 300 TGas
                0,
                current_nonce + 2,
                block_hash,
            )
            .await
            .context("Failed to call resolve_execution")?;

        info!("✅ Step 2/2 complete: {:?}", outcome2.status);
        info!("   Final transaction ID: {}", outcome2.transaction_outcome.id);

        // Return final transaction hash
        let tx_hash = format!("{}", outcome2.transaction_outcome.id);
        Ok(tx_hash)
    }

    /// Submit large execution output separately (2-call flow)
    ///
    /// Calls `submit_execution_output` on the OffchainVM contract
    /// This is used when output is too large to fit in yield resume payload
    ///
    /// # Arguments
    /// * `request_id` - Request ID from the contract
    /// * `output` - Execution output (bytes, text, or JSON)
    ///
    /// # Returns
    /// * `Ok(tx_hash)` - Transaction hash as hex string
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

        // Optional delay before next transaction to ensure nonce propagation
        if TWO_CALL_DELAY_MS > 0 {
            info!("⏳ Waiting {}ms for nonce propagation", TWO_CALL_DELAY_MS);
            tokio::time::sleep(Duration::from_millis(TWO_CALL_DELAY_MS)).await;
        }

        // Return transaction hash as hex string
        let tx_hash = format!("{}", outcome.transaction_outcome.id);
        Ok(tx_hash)
    }

    /// Submit execution result back to the NEAR contract
    ///
    /// Automatically decides between 1-call or 2-call flow based on payload size:
    /// - If payload < 1024 bytes: calls `resolve_execution` directly (1-call)
    /// - If payload >= 1024 bytes: calls `submit_execution_output` first, then `resolve_execution` (2-call)
    ///
    /// # Arguments
    /// * `request_id` - Request ID from the contract
    /// * `result` - Execution result from WASM executor
    ///
    /// # Returns
    /// * `Ok(tx_hash)` - Transaction hash as hex string
    pub async fn submit_execution_result(
        &self,
        request_id: u64,
        result: &ExecutionResult,
    ) -> Result<String> {
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

        // Return transaction hash as hex string
        let tx_hash = format!("{}", outcome.transaction_outcome.id);
        Ok(tx_hash)
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
                    info!("Transaction finalized: {}", tx_hash);
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
