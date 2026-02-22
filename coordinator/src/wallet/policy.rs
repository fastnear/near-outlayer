//! Policy cache and enforcement
//!
//! In-memory HashMap (NOT in DB):
//! - wallet_id -> NoPolicy { since: Instant } — TTL 5 min
//! - Cache miss / expired -> RPC has_wallet_policy() view call
//! - NoPolicy -> cache (zero further calls)
//! - HasPolicy -> keystore check-policy (always fresh, never cached)

use super::types::{
    KeystoreCheckPolicyRequest, KeystoreCheckPolicyResponse, WalletError,
};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{Mutex, RwLock};
use tracing::debug;

/// Negative policy cache entry
struct NoPolicyEntry {
    since: Instant,
}

/// Policy cache TTL (5 minutes)
const NO_POLICY_TTL: Duration = Duration::from_secs(300);

/// Policy cache — coordinator in-memory HashMap
pub struct PolicyCache {
    cache: Mutex<HashMap<String, NoPolicyEntry>>,
}

impl PolicyCache {
    pub fn new() -> Self {
        Self {
            cache: Mutex::new(HashMap::new()),
        }
    }

    /// Check if wallet is in the "no policy" negative cache
    pub async fn is_no_policy(&self, wallet_id: &str) -> bool {
        let cache = self.cache.lock().await;
        if let Some(entry) = cache.get(wallet_id) {
            if entry.since.elapsed() < NO_POLICY_TTL {
                return true;
            }
        }
        false
    }

    /// Mark wallet as having no policy (negative cache)
    pub async fn set_no_policy(&self, wallet_id: &str) {
        let mut cache = self.cache.lock().await;
        cache.insert(
            wallet_id.to_string(),
            NoPolicyEntry {
                since: Instant::now(),
            },
        );
    }

    /// Invalidate cache entry for a wallet
    pub async fn invalidate(&self, wallet_id: &str) {
        let mut cache = self.cache.lock().await;
        cache.remove(wallet_id);
    }

    /// Clear all expired entries (periodic cleanup)
    pub async fn cleanup_expired(&self) {
        let mut cache = self.cache.lock().await;
        cache.retain(|_, entry| entry.since.elapsed() < NO_POLICY_TTL);
    }
}

/// Result of policy check
#[derive(Debug)]
pub enum PolicyDecision {
    /// No policy set — allow (quick onboarding mode)
    NoPolicyAllow,
    /// Policy checked, operation allowed
    Allowed,
    /// Policy checked, requires multisig approval
    RequiresApproval {
        required_approvals: i32,
    },
    /// Policy denied
    Denied(String),
    /// Wallet is frozen
    Frozen,
}

/// Output from policy check (decision + metadata for webhooks)
pub struct PolicyCheckOutput {
    pub decision: PolicyDecision,
    pub webhook_url: Option<String>,
    /// Decrypted policy JSON (if available from keystore)
    pub policy: Option<serde_json::Value>,
}

/// Resolve wallet_id (UUID) → on-chain wallet_pubkey ("ed25519:<hex>")
///
/// Looks up `near_pubkey` from `wallet_accounts` table (populated by GET /address).
pub async fn resolve_wallet_pubkey(
    db: &sqlx::PgPool,
    wallet_id: &str,
) -> Result<String, WalletError> {
    let row: Option<(String,)> = sqlx::query_as(
        "SELECT near_pubkey FROM wallet_accounts WHERE wallet_id = $1 AND near_pubkey IS NOT NULL",
    )
    .bind(wallet_id)
    .fetch_optional(db)
    .await
    .map_err(|e| WalletError::InternalError(format!("DB error resolving wallet pubkey: {}", e)))?;

    match row {
        Some((near_pubkey,)) => Ok(near_pubkey), // DB already stores "ed25519:<hex>"
        None => Err(WalletError::InternalError(
            "Wallet has no NEAR public key. Call GET /wallet/v1/address first.".to_string(),
        )),
    }
}

/// Check wallet policy for an action
///
/// 1. Check negative cache (no policy → skip)
/// 2. RPC has_wallet_policy() view call
/// 3. If no policy → cache and allow
/// 4. If has policy → keystore check-policy (fresh, not cached)
pub async fn check_wallet_policy(
    policy_cache: &PolicyCache,
    near_rpc_url: &str,
    contract_id: &str,
    keystore_url: &str,
    keystore_auth_token: &str,
    wallet_id: &str,
    wallet_pubkey: &str,
    action: serde_json::Value,
) -> Result<PolicyCheckOutput, WalletError> {
    check_wallet_policy_with_overrides(
        policy_cache,
        near_rpc_url,
        contract_id,
        keystore_url,
        keystore_auth_token,
        wallet_id,
        wallet_pubkey,
        action,
        None,
    )
    .await
}

/// Check wallet policy, with optional local override map
///
/// `wallet_id` — coordinator UUID (used for keystore, cache, overrides)
/// `wallet_pubkey` — on-chain key "ed25519:<hex>" (used for contract RPC)
pub async fn check_wallet_policy_with_overrides(
    policy_cache: &PolicyCache,
    near_rpc_url: &str,
    contract_id: &str,
    keystore_url: &str,
    keystore_auth_token: &str,
    wallet_id: &str,
    wallet_pubkey: &str,
    action: serde_json::Value,
    policy_overrides: Option<&Arc<RwLock<HashMap<String, String>>>>,
) -> Result<PolicyCheckOutput, WalletError> {
    // Step 0: Check local overrides (testing / local policy)
    let local_override = if let Some(overrides) = policy_overrides {
        let map = overrides.read().await;
        map.get(wallet_id).cloned()
    } else {
        None
    };

    if local_override.is_some() {
        debug!("Policy override found for {}, using local data", wallet_id);
    } else {
        // Step 1: Check negative cache (only when no local override)
        if policy_cache.is_no_policy(wallet_id).await {
            debug!("Policy cache hit: {} has no policy (cached)", wallet_id);
            return Ok(PolicyCheckOutput {
                decision: PolicyDecision::NoPolicyAllow,
                webhook_url: None,
                policy: None,
            });
        }
    }

    // Step 2: Check if policy exists (local override or on-chain)
    let has_policy = if local_override.is_some() {
        true
    } else {
        rpc_has_wallet_policy(near_rpc_url, contract_id, wallet_pubkey)
            .await
            .map_err(|e| {
                WalletError::InternalError(format!("Failed to check policy on-chain: {}", e))
            })?
    };

    if !has_policy {
        // Step 3: No policy → cache and allow
        debug!("No policy for {}, caching negative result", wallet_id);
        policy_cache.set_no_policy(wallet_id).await;
        return Ok(PolicyCheckOutput {
            decision: PolicyDecision::NoPolicyAllow,
            webhook_url: None,
            policy: None,
        });
    }

    // Step 4: Has policy → keystore check-policy
    debug!(
        "Policy exists for {}, checking with keystore",
        wallet_id
    );
    let check_result = keystore_check_policy(
        keystore_url,
        keystore_auth_token,
        wallet_id,
        action,
        local_override.as_deref(),
    )
    .await
    .map_err(|e| WalletError::KeystoreError(format!("Policy check failed: {}", e)))?;

    // Extract webhook_url from decrypted policy (for event delivery)
    let webhook_url = check_result
        .policy
        .as_ref()
        .and_then(|p| p.get("webhook_url"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let policy = check_result.policy;

    if check_result.frozen {
        return Ok(PolicyCheckOutput {
            decision: PolicyDecision::Frozen,
            webhook_url,
            policy,
        });
    }

    if !check_result.allowed {
        return Ok(PolicyCheckOutput {
            decision: PolicyDecision::Denied(
                check_result
                    .reason
                    .unwrap_or_else(|| "Policy denied".to_string()),
            ),
            webhook_url,
            policy,
        });
    }

    if check_result.requires_approval {
        return Ok(PolicyCheckOutput {
            decision: PolicyDecision::RequiresApproval {
                required_approvals: check_result.required_approvals.unwrap_or(2),
            },
            webhook_url,
            policy,
        });
    }

    Ok(PolicyCheckOutput {
        decision: PolicyDecision::Allowed,
        webhook_url,
        policy,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_cache_miss() {
        let cache = PolicyCache::new();
        assert!(!cache.is_no_policy("unknown-wallet").await);
    }

    #[tokio::test]
    async fn test_cache_set_and_hit() {
        let cache = PolicyCache::new();
        cache.set_no_policy("w1").await;
        assert!(cache.is_no_policy("w1").await);
    }

    #[tokio::test]
    async fn test_cache_invalidate() {
        let cache = PolicyCache::new();
        cache.set_no_policy("w1").await;
        assert!(cache.is_no_policy("w1").await);
        cache.invalidate("w1").await;
        assert!(!cache.is_no_policy("w1").await);
    }

    #[tokio::test]
    async fn test_cache_different_wallets() {
        let cache = PolicyCache::new();
        cache.set_no_policy("w1").await;
        assert!(cache.is_no_policy("w1").await);
        assert!(!cache.is_no_policy("w2").await);
    }
}

/// RPC call to contract: has_wallet_policy(wallet_pubkey) -> bool
async fn rpc_has_wallet_policy(
    near_rpc_url: &str,
    contract_id: &str,
    wallet_id: &str,
) -> anyhow::Result<bool> {
    let client = reqwest::Client::new();

    let args_json = serde_json::json!({
        "wallet_pubkey": wallet_id,
    });
    let args_base64 = base64::Engine::encode(
        &base64::engine::general_purpose::STANDARD,
        serde_json::to_string(&args_json)?.as_bytes(),
    );

    let body = serde_json::json!({
        "jsonrpc": "2.0",
        "id": "wallet-policy-check",
        "method": "query",
        "params": {
            "request_type": "call_function",
            "finality": "optimistic",
            "account_id": contract_id,
            "method_name": "has_wallet_policy",
            "args_base64": args_base64,
        }
    });

    let response = client
        .post(near_rpc_url)
        .json(&body)
        .send()
        .await?;

    let rpc_result: serde_json::Value = response.json().await?;

    // Parse RPC response
    if let Some(error) = rpc_result.get("error") {
        anyhow::bail!("NEAR RPC error: {}", error);
    }

    let result_bytes = rpc_result
        .pointer("/result/result")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_u64().map(|n| n as u8))
                .collect::<Vec<u8>>()
        })
        .unwrap_or_default();

    // Parse JSON boolean from bytes
    let result_str = String::from_utf8(result_bytes)?;
    let has_policy: bool = serde_json::from_str(&result_str)?;

    Ok(has_policy)
}

/// RPC call to contract: get_wallet_policy(wallet_pubkey) -> Option<{encrypted_data, owner, frozen}>
pub async fn rpc_get_wallet_policy(
    near_rpc_url: &str,
    contract_id: &str,
    wallet_pubkey: &str,
) -> anyhow::Result<Option<serde_json::Value>> {
    let client = reqwest::Client::new();

    let args_json = serde_json::json!({
        "wallet_pubkey": wallet_pubkey,
    });
    let args_base64 = base64::Engine::encode(
        &base64::engine::general_purpose::STANDARD,
        serde_json::to_string(&args_json)?.as_bytes(),
    );

    let body = serde_json::json!({
        "jsonrpc": "2.0",
        "id": "wallet-policy-get",
        "method": "query",
        "params": {
            "request_type": "call_function",
            "finality": "optimistic",
            "account_id": contract_id,
            "method_name": "get_wallet_policy",
            "args_base64": args_base64,
        }
    });

    let response = client
        .post(near_rpc_url)
        .json(&body)
        .send()
        .await?;

    let rpc_result: serde_json::Value = response.json().await?;

    if let Some(error) = rpc_result.get("error") {
        anyhow::bail!("NEAR RPC error: {}", error);
    }

    let result_bytes = rpc_result
        .pointer("/result/result")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_u64().map(|n| n as u8))
                .collect::<Vec<u8>>()
        })
        .unwrap_or_default();

    let result_str = String::from_utf8(result_bytes)?;
    let entry: Option<serde_json::Value> = serde_json::from_str(&result_str)?;

    Ok(entry)
}

/// Call keystore check-policy endpoint
async fn keystore_check_policy(
    keystore_url: &str,
    keystore_auth_token: &str,
    wallet_id: &str,
    action: serde_json::Value,
    encrypted_policy_data: Option<&str>,
) -> anyhow::Result<KeystoreCheckPolicyResponse> {
    let client = reqwest::Client::new();

    let payload = KeystoreCheckPolicyRequest {
        wallet_id: wallet_id.to_string(),
        action,
        encrypted_policy_data: encrypted_policy_data.map(|s| s.to_string()),
    };

    let mut request = client
        .post(format!("{}/wallet/check-policy", keystore_url))
        .json(&payload);

    if !keystore_auth_token.is_empty() {
        request = request.header("Authorization", format!("Bearer {}", keystore_auth_token));
    }

    let response = request.send().await?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response
            .text()
            .await
            .unwrap_or_else(|_| "unknown".to_string());
        anyhow::bail!("Keystore check-policy failed ({}): {}", status, body);
    }

    let result: KeystoreCheckPolicyResponse = response.json().await?;
    Ok(result)
}
