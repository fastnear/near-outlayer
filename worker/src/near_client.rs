use anyhow::{Context, Result};
use near_crypto::InMemorySigner;
use near_jsonrpc_client::{methods, JsonRpcClient};
use near_primitives::transaction::{Action, FunctionCallAction, Transaction, TransactionV0};
use near_primitives::types::{AccountId, BlockReference, Finality};
use near_primitives::views::FinalExecutionOutcomeView;
use serde_json::json;
use tracing::{debug, info};

use crate::api_client::ExecutionResult;

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

    /// Submit execution result back to the NEAR contract
    ///
    /// Calls `resolve_execution` on the OffchainVM contract
    ///
    /// # Arguments
    /// * `data_id` - Data ID from the yield promise (32 bytes hex string)
    /// * `result` - Execution result from WASM executor
    ///
    /// # Returns
    /// * `Ok(tx_hash)` - Transaction hash as hex string
    pub async fn submit_execution_result(
        &self,
        data_id: &str,
        result: &ExecutionResult,
    ) -> Result<String> {
        info!(
            "📡 Submitting execution result: data_id={}, success={}, output_len={:?}",
            data_id, result.success, result.output.as_ref().map(|o| o.len())
        );

        // Parse data_id from hex string to [u8; 32]
        let data_id_bytes = hex::decode(data_id)
            .context("Failed to decode data_id from hex")?;

        if data_id_bytes.len() != 32 {
            anyhow::bail!("data_id must be 32 bytes, got {}", data_id_bytes.len());
        }

        info!("📦 data_id bytes (first 8): {:?}", &data_id_bytes[..8]);

        // Prepare method arguments matching contract signature:
        // resolve_execution(data_id: [u8; 32], response: ExecutionResponse)
        // Note: data_id must be sent as array, not base58 string
        let args = json!({
            "data_id": data_id_bytes,
            "response": {
                "success": result.success,
                "return_value": result.output,
                "error": result.error,
                "resources_used": {
                    "instructions": result.instructions,
                    "time_ms": result.execution_time_ms,
                }
            }
        });

        let args_json = serde_json::to_string(&args).context("Failed to serialize args")?;
        info!("📤 Full args for resolve_execution: {}", args_json);

        // Send transaction
        info!("🔗 Sending transaction:");
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
