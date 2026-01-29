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
pub fn normalize_repo_url(repo: &str) -> String {
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
    /// Calls: contract.get_secrets(accessor: Repo { repo, branch }, profile, owner)
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
            "accessor": {
                "Repo": {
                    "repo": repo_normalized,
                    "branch": branch,
                }
            },
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

        // Log raw response for debugging
        tracing::debug!(
            raw_response = ?String::from_utf8_lossy(&result),
            raw_len = result.len(),
            "Raw contract response for get_secrets"
        );

        // Parse response (Option<SecretProfile>)
        let secret_profile: Option<serde_json::Value> = serde_json::from_slice(&result)
            .context("Failed to parse contract response")?;

        tracing::debug!(
            is_some = secret_profile.is_some(),
            "Parsed get_secrets response"
        );

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

    /// Read secrets from contract by WASM hash
    ///
    /// Calls: contract.get_secrets(accessor: WasmHash { hash }, profile, owner)
    /// Returns: SecretProfile { encrypted_secrets, access, ... }
    pub async fn get_secrets_by_wasm_hash(
        &self,
        wasm_hash: &str,
        profile: &str,
        owner: &str,
    ) -> Result<Option<serde_json::Value>> {
        tracing::debug!(
            contract = %self.contract_id,
            wasm_hash = %wasm_hash,
            profile = %profile,
            owner = %owner,
            "Reading secrets by wasm_hash from contract"
        );

        let args = json!({
            "accessor": {
                "WasmHash": {
                    "hash": wasm_hash
                }
            },
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

    /// Read secrets from contract by project ID
    ///
    /// Calls: contract.get_secrets(accessor: Project { project_id }, profile, owner)
    /// Returns: SecretProfile { encrypted_secrets, access, ... }
    pub async fn get_secrets_by_project(
        &self,
        project_id: &str,
        profile: &str,
        owner: &str,
    ) -> Result<Option<serde_json::Value>> {
        tracing::debug!(
            contract = %self.contract_id,
            project_id = %project_id,
            profile = %profile,
            owner = %owner,
            "Reading secrets by project_id from contract"
        );

        let args = json!({
            "accessor": {
                "Project": {
                    "project_id": project_id
                }
            },
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

    /// Read secrets from contract by System secret type
    ///
    /// Calls: contract.get_secrets(accessor: System(PaymentKey), profile, owner)
    /// Returns: SecretProfile { encrypted_secrets, access, ... }
    pub async fn get_secrets_by_system(
        &self,
        secret_type: &str,  // "PaymentKey"
        profile: &str,      // nonce for Payment Keys
        owner: &str,
    ) -> Result<Option<serde_json::Value>> {
        tracing::debug!(
            contract = %self.contract_id,
            secret_type = %secret_type,
            profile = %profile,
            owner = %owner,
            "Reading secrets by System from contract"
        );

        let args = json!({
            "accessor": {
                "System": secret_type
            },
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

    /// Check if account is member of DAO role (Sputnik v2 compatible)
    ///
    /// Calls: dao_contract.get_policy()
    /// Returns: Policy { roles: [...], ... }
    ///
    /// Role kinds:
    /// - "Everyone" - all users
    /// - "Group" - explicit list of accounts { "kind": { "Group": ["alice.near", ...] } }
    /// - "Member" - token holders with balance >= threshold { "kind": { "Member": "1000000" } }
    pub async fn check_dao_membership(&self, dao_contract: &str, account_id: &str, role_name: &str) -> Result<bool> {
        let dao_contract_id = AccountId::from_str(dao_contract)
            .context("Invalid DAO contract ID")?;

        tracing::debug!(
            dao_contract = %dao_contract,
            account_id = %account_id,
            role_name = %role_name,
            "Checking DAO membership"
        );

        // Call get_policy()
        let query = methods::query::RpcQueryRequest {
            block_reference: BlockReference::latest(),
            request: near_primitives::views::QueryRequest::CallFunction {
                account_id: dao_contract_id,
                method_name: "get_policy".to_string(),
                args: vec![].into(), // no arguments
            },
        };

        let response = self
            .rpc_client
            .call(query)
            .await
            .context("Failed to query DAO policy")?;

        let result = match response.kind {
            QueryResponseKind::CallResult(result) => result.result,
            _ => anyhow::bail!("Unexpected query response"),
        };

        // Parse policy response
        let policy: serde_json::Value = serde_json::from_slice(&result)
            .context("Failed to parse DAO policy response")?;

        tracing::debug!(
            policy_json = %serde_json::to_string_pretty(&policy).unwrap_or_default(),
            "Received DAO policy"
        );

        // Find role by name in policy.roles array
        let roles = policy
            .get("roles")
            .and_then(|v| v.as_array())
            .context("Policy missing 'roles' array")?;

        for role in roles {
            let name = role.get("name").and_then(|v| v.as_str());
            if name != Some(role_name) {
                continue; // Not the role we're looking for
            }

            tracing::debug!(
                role_name = %role_name,
                role_data = %serde_json::to_string(role).unwrap_or_default(),
                "Found matching role"
            );

            // Check role kind
            let kind = role.get("kind").context("Role missing 'kind' field")?;

            // Check if it's "Everyone" (unit variant as string)
            if kind.is_string() && kind.as_str() == Some("Everyone") {
                tracing::debug!("Role kind is Everyone - access granted");
                return Ok(true);
            }

            // Check if it's "Group" variant { "Group": ["alice.near", ...] }
            if let Some(group_accounts) = kind.get("Group").and_then(|v| v.as_array()) {
                let is_member = group_accounts.iter().any(|acc| {
                    acc.as_str() == Some(account_id)
                });

                tracing::debug!(
                    kind = "Group",
                    accounts_count = group_accounts.len(),
                    is_member = %is_member,
                    "Checked Group membership"
                );

                return Ok(is_member);
            }

            // "Member" variant requires token balance check - not supported
            // Use "Group" role with explicit account list instead
            if kind.get("Member").is_some() {
                anyhow::bail!(
                    "DAO role '{}' uses Member kind which requires token balance checking. \
                    This is not supported. Please use a Group role with explicit account list instead.",
                    role_name
                );
            }

            // Unknown role kind
            tracing::warn!(
                role_kind = %serde_json::to_string(kind).unwrap_or_default(),
                "Unknown role kind in DAO policy"
            );
            return Ok(false);
        }

        // Role not found in policy
        tracing::debug!(
            role_name = %role_name,
            available_roles = ?roles.iter()
                .filter_map(|r| r.get("name").and_then(|v| v.as_str()))
                .collect::<Vec<_>>(),
            "Role not found in DAO policy"
        );

        Ok(false)
    }

    /// Verify that a public key belongs to an account
    ///
    /// Calls: view_access_key_list for account_id
    /// Returns: Ok(()) if public_key is found, Err if not found or RPC error
    pub async fn verify_access_key_owner(
        &self,
        account_id: &str,
        public_key: &str,
    ) -> Result<()> {
        let account_id_parsed = AccountId::from_str(account_id)
            .context("Invalid account ID")?;

        tracing::debug!(
            account_id = %account_id,
            public_key = %public_key,
            "Verifying access key ownership"
        );

        let query = methods::query::RpcQueryRequest {
            block_reference: BlockReference::latest(),
            request: near_primitives::views::QueryRequest::ViewAccessKeyList {
                account_id: account_id_parsed,
            },
        };

        let response = self
            .rpc_client
            .call(query)
            .await
            .context("Failed to query access keys")?;

        let access_keys = match response.kind {
            QueryResponseKind::AccessKeyList(list) => list.keys,
            _ => anyhow::bail!("Unexpected query response"),
        };

        // Check if public_key is in the access key list
        let key_found = access_keys.iter().any(|key| {
            key.public_key.to_string() == public_key
        });

        if key_found {
            tracing::debug!(
                account_id = %account_id,
                public_key = %public_key,
                "✅ Access key verified"
            );
            Ok(())
        } else {
            tracing::warn!(
                account_id = %account_id,
                public_key = %public_key,
                keys_count = access_keys.len(),
                "❌ Access key not found for account"
            );
            anyhow::bail!(
                "Public key {} does not belong to account {}",
                public_key, account_id
            )
        }
    }
}
