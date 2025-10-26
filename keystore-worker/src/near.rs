//! NEAR blockchain client
//!
//! Handles reading secrets from NEAR contract (read-only).

use anyhow::{Context, Result};
use near_jsonrpc_client::{methods, JsonRpcClient};
use near_jsonrpc_primitives::types::query::QueryResponseKind;
use near_primitives::types::{AccountId, BlockReference};
use serde_json::json;
use std::str::FromStr;

/// Normalize repository URL to consistent format (domain.com/owner/repo)
///
/// Examples:
/// - "https://github.com/alice/project" → "github.com/alice/project"
/// - "http://github.com/alice/project" → "github.com/alice/project"
/// - "github.com/alice/project" → "github.com/alice/project"
/// - "gitlab.com/alice/project" → "gitlab.com/alice/project"
fn normalize_repo_url(repo: &str) -> String {
    let repo = repo.trim();

    // Remove protocol (https:// or http://)
    let repo = repo
        .strip_prefix("https://")
        .or_else(|| repo.strip_prefix("http://"))
        .unwrap_or(repo);

    repo.to_string()
}

/// NEAR client for reading secrets from contract (read-only)
pub struct NearClient {
    /// JSON-RPC client
    rpc_client: JsonRpcClient,
    /// Contract account ID
    contract_id: AccountId,
}

impl NearClient {
    /// Create new NEAR client (read-only)
    ///
    /// Only needs RPC URL and contract ID - no private key required for reading.
    pub fn new(rpc_url: &str, contract_id: &str) -> Result<Self> {
        let rpc_client = JsonRpcClient::connect(rpc_url);

        let contract_id = AccountId::from_str(contract_id)
            .context("Invalid contract ID")?;

        Ok(Self {
            rpc_client,
            contract_id,
        })
    }

    /// Read secrets from contract
    ///
    /// Calls: contract.get_secrets(repo, branch, profile, owner)
    /// Returns: SecretProfile { encrypted_secrets, access, created_at, updated_at, storage_deposit }
    ///
    /// Note: The contract's get_secrets method automatically falls back to branch=null
    /// if secrets with specific branch are not found (wildcard secrets).
    pub async fn get_secrets(
        &self,
        repo: &str,
        branch: Option<&str>,
        profile: &str,
        owner: &str,
    ) -> Result<Option<serde_json::Value>> {
        // Normalize repo URL to match format used when storing
        let repo_normalized = normalize_repo_url(repo);

        tracing::debug!(
            contract = %self.contract_id,
            repo_input = %repo,
            repo_normalized = %repo_normalized,
            branch = ?branch,
            profile = %profile,
            owner = %owner,
            "Reading secrets from contract"
        );

        let args = json!({
            "repo": repo_normalized,
            "branch": branch,
            "profile": profile,
            "owner": owner,
        });

        let query = methods::query::RpcQueryRequest {
            block_reference: BlockReference::latest(),
            request: near_primitives::views::QueryRequest::CallFunction {
                account_id: self.contract_id.clone(),
                method_name: "get_secrets".to_string(),
                args: args.to_string().into_bytes().into(),
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

        // Parse response (Option<SecretProfile>)
        let secret_profile: Option<serde_json::Value> = serde_json::from_slice(&result)
            .context("Failed to parse contract response")?;

        Ok(secret_profile)
    }

    /// Get account NEAR balance in yoctoNEAR
    pub async fn get_account_balance(&self, account_id: &str) -> Result<u128> {
        let account_id_parsed = AccountId::from_str(account_id)
            .context("Invalid account ID")?;

        let query = methods::query::RpcQueryRequest {
            block_reference: BlockReference::latest(),
            request: near_primitives::views::QueryRequest::ViewAccount {
                account_id: account_id_parsed,
            },
        };

        let response = self
            .rpc_client
            .call(query)
            .await
            .context("Failed to query account")?;

        match response.kind {
            QueryResponseKind::ViewAccount(account_view) => Ok(account_view.amount),
            _ => anyhow::bail!("Unexpected query response"),
        }
    }

    /// Get fungible token balance
    ///
    /// Calls: ft_contract.ft_balance_of({"account_id": account_id})
    /// Returns: Balance as u128 (from JSON string)
    pub async fn get_ft_balance(&self, ft_contract: &str, account_id: &str) -> Result<u128> {
        let ft_contract_id = AccountId::from_str(ft_contract)
            .context("Invalid FT contract ID")?;

        let args = json!({
            "account_id": account_id,
        });

        let query = methods::query::RpcQueryRequest {
            block_reference: BlockReference::latest(),
            request: near_primitives::views::QueryRequest::CallFunction {
                account_id: ft_contract_id,
                method_name: "ft_balance_of".to_string(),
                args: args.to_string().into_bytes().into(),
            },
        };

        let response = self
            .rpc_client
            .call(query)
            .await
            .context("Failed to query FT balance")?;

        let result = match response.kind {
            QueryResponseKind::CallResult(result) => result.result,
            _ => anyhow::bail!("Unexpected query response"),
        };

        // FT balance is returned as JSON string: "123456789"
        let balance_str: String = serde_json::from_slice(&result)
            .context("Failed to parse FT balance response")?;

        balance_str.parse::<u128>()
            .context("Failed to parse balance as u128")
    }

    /// Check if account owns NFT(s) from given contract
    ///
    /// If token_id is Some("123"), checks ownership of that specific token
    /// If token_id is None, checks if account owns any token from this contract
    ///
    /// Calls:
    /// - token_id = Some: nft_contract.nft_token({"token_id": "123"})
    /// - token_id = None: nft_contract.nft_tokens_for_owner({"account_id": account_id, "limit": 1})
    pub async fn check_nft_ownership(&self, nft_contract: &str, account_id: &str, token_id: Option<&str>) -> Result<bool> {
        let nft_contract_id = AccountId::from_str(nft_contract)
            .context("Invalid NFT contract ID")?;

        if let Some(specific_token_id) = token_id {
            // Check specific token ownership
            let args = json!({
                "token_id": specific_token_id,
            });

            let query = methods::query::RpcQueryRequest {
                block_reference: BlockReference::latest(),
                request: near_primitives::views::QueryRequest::CallFunction {
                    account_id: nft_contract_id,
                    method_name: "nft_token".to_string(),
                    args: args.to_string().into_bytes().into(),
                },
            };

            let response = self
                .rpc_client
                .call(query)
                .await
                .context("Failed to query specific NFT token")?;

            let result = match response.kind {
                QueryResponseKind::CallResult(result) => result.result,
                _ => anyhow::bail!("Unexpected query response"),
            };

            // NFT standard returns: {token_id, owner_id, ...} or null if not found
            let token: Option<serde_json::Value> = serde_json::from_slice(&result)
                .context("Failed to parse NFT token response")?;

            if let Some(token_data) = token {
                // Check if owner_id matches account_id
                if let Some(owner_id) = token_data.get("owner_id").and_then(|v| v.as_str()) {
                    return Ok(owner_id == account_id);
                }
            }

            Ok(false)
        } else {
            // Check if owns any token from this contract
            let args = json!({
                "account_id": account_id,
                "limit": 1,  // We only need to check if at least one NFT exists
            });

            let query = methods::query::RpcQueryRequest {
                block_reference: BlockReference::latest(),
                request: near_primitives::views::QueryRequest::CallFunction {
                    account_id: nft_contract_id,
                    method_name: "nft_tokens_for_owner".to_string(),
                    args: args.to_string().into_bytes().into(),
                },
            };

            let response = self
                .rpc_client
                .call(query)
                .await
                .context("Failed to query NFT ownership")?;

            let result = match response.kind {
                QueryResponseKind::CallResult(result) => result.result,
                _ => anyhow::bail!("Unexpected query response"),
            };

            // NFT standard returns array of tokens: [{token_id, owner_id, ...}, ...]
            let tokens: Vec<serde_json::Value> = serde_json::from_slice(&result)
                .context("Failed to parse NFT tokens response")?;

            Ok(!tokens.is_empty())
        }
    }
}
