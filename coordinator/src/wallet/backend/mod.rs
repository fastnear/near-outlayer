//! Wallet backend trait — modular execution layer
//!
//! Currently: NEAR Intents (meta-transactions / relayer)
//! Future: native blockchain operations, additional chain backends

pub mod intents;

use anyhow::Result;
use serde::{Deserialize, Serialize};

/// Information about a wallet for backend operations
#[derive(Debug, Clone)]
pub struct WalletInfo {
    pub wallet_id: String,
    pub chain: String,
    pub chain_address: String,
    pub chain_public_key: String,
}

/// Withdraw request for backend
#[derive(Debug, Clone, Serialize)]
pub struct BackendWithdrawRequest {
    pub to: String,
    pub amount: String,
    pub token: Option<String>,
    pub chain: String,
}

/// Withdraw result from backend
#[derive(Debug, Clone, Deserialize)]
pub struct BackendWithdrawResponse {
    pub operation_id: String,
    pub status: String,
    #[serde(default)]
    pub tx_hash: Option<String>,
    #[serde(default)]
    pub fee: Option<String>,
    #[serde(default)]
    pub fee_token: Option<String>,
}

/// Deposit quote request
#[derive(Debug, Clone, Serialize)]
pub struct BackendDepositRequest {
    pub source_chain: String,
    pub token: String,
    pub amount: String,
    pub destination_address: String,
}

/// Deposit quote response from backend
#[derive(Debug, Clone, Deserialize)]
pub struct BackendDepositResponse {
    pub operation_id: String,
    pub deposit_address: String,
    pub chain: String,
    pub status: String,
    #[serde(default)]
    pub expires_at: Option<String>,
}

/// Operation status
#[derive(Debug, Clone, Deserialize)]
pub struct OperationStatus {
    pub operation_id: String,
    pub status: String,
    #[serde(default)]
    pub tx_hash: Option<String>,
    #[serde(default)]
    pub fee: Option<String>,
    #[serde(default)]
    pub fee_token: Option<String>,
    #[serde(default)]
    pub error: Option<String>,
}

/// Token info from backend
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackendTokenInfo {
    pub id: String,
    pub symbol: String,
    pub chains: Vec<String>,
}

/// Wallet backend trait — pluggable execution layer
#[async_trait::async_trait]
pub trait WalletBackend: Send + Sync {
    /// Execute a withdrawal (sign + submit transaction)
    async fn withdraw(
        &self,
        wallet: &WalletInfo,
        req: BackendWithdrawRequest,
        signed_tx_base64: &str,
    ) -> Result<BackendWithdrawResponse>;

    /// Get deposit quote (cross-chain deposit via Intents)
    async fn deposit_quote(
        &self,
        wallet: &WalletInfo,
        req: BackendDepositRequest,
    ) -> Result<BackendDepositResponse>;

    /// Poll operation status
    async fn operation_status(&self, operation_id: &str) -> Result<OperationStatus>;

    /// List available tokens
    async fn list_tokens(&self) -> Result<Vec<BackendTokenInfo>>;
}
