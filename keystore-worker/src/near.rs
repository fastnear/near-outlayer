//! NEAR blockchain client
//!
//! Handles interaction with NEAR contract to publish public key.

use anyhow::{Context, Result};
use near_crypto::{InMemorySigner, SecretKey};
use near_jsonrpc_client::{methods, JsonRpcClient};
use near_jsonrpc_primitives::types::query::QueryResponseKind;
use near_primitives::{
    transaction::{Action, FunctionCallAction, SignedTransaction, Transaction, TransactionV0},
    types::{AccountId, BlockReference},
};
use serde_json::json;
use std::str::FromStr;

/// NEAR client for publishing keystore public key to contract
pub struct NearClient {
    /// JSON-RPC client
    rpc_client: JsonRpcClient,
    /// Signer for transactions
    signer: InMemorySigner,
    /// Contract account ID
    contract_id: AccountId,
}

impl NearClient {
    /// Create new NEAR client
    pub fn new(
        rpc_url: &str,
        account_id: &str,
        private_key: &str,
        contract_id: &str,
    ) -> Result<Self> {
        let rpc_client = JsonRpcClient::connect(rpc_url);

        let account_id_parsed = AccountId::from_str(account_id)
            .context("Invalid account ID")?;

        let secret_key = SecretKey::from_str(private_key)
            .context("Invalid private key")?;

        let signer = InMemorySigner::from_secret_key(account_id_parsed.clone(), secret_key);

        let contract_id = AccountId::from_str(contract_id)
            .context("Invalid contract ID")?;

        Ok(Self {
            rpc_client,
            signer,
            contract_id,
        })
    }

    /// Publish public key to contract
    ///
    /// Calls: contract.set_keystore_pubkey(pubkey_base58)
    pub async fn publish_public_key(&self, pubkey_hex: &str) -> Result<()> {
        tracing::info!(
            contract = %self.contract_id,
            pubkey = %pubkey_hex,
            "Publishing public key to contract"
        );

        // Get current access key nonce
        let access_key_query = methods::query::RpcQueryRequest {
            block_reference: BlockReference::latest(),
            request: near_primitives::views::QueryRequest::ViewAccessKey {
                account_id: self.signer.account_id.clone(),
                public_key: self.signer.public_key.clone(),
            },
        };

        let access_key_response = self
            .rpc_client
            .call(access_key_query)
            .await
            .context("Failed to query access key")?;

        let nonce = match access_key_response.kind {
            QueryResponseKind::AccessKey(access_key) => access_key.nonce,
            _ => anyhow::bail!("Unexpected query response"),
        };

        // Get latest block hash
        let block_query = methods::block::RpcBlockRequest {
            block_reference: BlockReference::latest(),
        };

        let block = self
            .rpc_client
            .call(block_query)
            .await
            .context("Failed to query block")?;

        let block_hash = block.header.hash;

        // Build transaction
        let transaction = Transaction::V0(TransactionV0 {
            signer_id: self.signer.account_id.clone(),
            public_key: self.signer.public_key.clone(),
            nonce: nonce + 1,
            receiver_id: self.contract_id.clone(),
            block_hash,
            actions: vec![Action::FunctionCall(Box::new(FunctionCallAction {
                method_name: "set_keystore_pubkey".to_string(),
                args: json!({
                    "pubkey": pubkey_hex  // Contract now accepts both hex and base58
                })
                .to_string()
                .into_bytes(),
                gas: 30_000_000_000_000, // 30 TGas
                deposit: 0,
            }))],
        });

        // Sign transaction
        let signature = self.signer.sign(transaction.get_hash_and_size().0.as_ref());
        let signed_tx = SignedTransaction::new(signature, transaction);

        // Send transaction
        let tx_request = methods::broadcast_tx_commit::RpcBroadcastTxCommitRequest {
            signed_transaction: signed_tx,
        };

        let response = self
            .rpc_client
            .call(tx_request)
            .await
            .context("Failed to send transaction")?;

        if let near_primitives::views::FinalExecutionStatus::Failure(err) = response.status {
            anyhow::bail!("Transaction failed: {:?}", err);
        }

        tracing::info!("Successfully published public key to contract");

        Ok(())
    }

    /// Check if current public key matches contract's stored key
    pub async fn verify_public_key(&self, expected_pubkey_hex: &str) -> Result<bool> {
        let query = methods::query::RpcQueryRequest {
            block_reference: BlockReference::latest(),
            request: near_primitives::views::QueryRequest::CallFunction {
                account_id: self.contract_id.clone(),
                method_name: "get_keystore_pubkey".to_string(),
                args: vec![].into(),
            },
        };

        let response = self
            .rpc_client
            .call(query)
            .await
            .context("Failed to query contract")?;

        let result = match response.kind {
            QueryResponseKind::CallResult(result) => result.result,
            _ => anyhow::bail!("Unexpected query response"),
        };

        // Parse response (JSON string or null)
        let stored_pubkey: Option<String> = serde_json::from_slice(&result)
            .context("Failed to parse contract response")?;

        match stored_pubkey {
            Some(ref pubkey) if pubkey == expected_pubkey_hex => {
                tracing::info!("Public key matches contract");
                Ok(true)
            }
            Some(ref pubkey) => {
                tracing::error!(
                    expected = %expected_pubkey_hex,
                    stored = %pubkey,
                    "Public key mismatch!"
                );
                Ok(false)
            }
            None => {
                tracing::warn!("No public key set in contract");
                Ok(false)
            }
        }
    }
}
