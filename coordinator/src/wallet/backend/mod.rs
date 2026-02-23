//! Wallet backend trait — modular execution layer
//!
//! Swap: 1Click REST API (https://1click.chaindefuser.com)
//! Withdraw: direct ft_withdraw on intents.near (single NEAR transaction)
//! List tokens: static list of supported tokens

pub mod intents;

use anyhow::Result;
use serde::{Deserialize, Serialize};

/// Token info from backend
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackendTokenInfo {
    pub id: String,
    pub symbol: String,
    pub chains: Vec<String>,
}

/// 1Click API quote request
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OneClickQuoteRequest {
    pub dry: bool,
    pub swap_type: String,
    pub slippage_tolerance: u32,
    pub origin_asset: String,
    pub deposit_type: String,
    pub destination_asset: String,
    pub amount: String,
    pub refund_to: String,
    pub refund_type: String,
    pub recipient: String,
    pub recipient_type: String,
    pub deadline: String,
}

/// 1Click API quote response
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OneClickQuoteResponse {
    pub correlation_id: String,
    pub quote: OneClickQuote,
}

/// 1Click quote details
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OneClickQuote {
    pub deposit_address: String,
    pub amount_in: String,
    pub amount_out: String,
    pub min_amount_out: String,
    pub deadline: String,
    #[serde(default)]
    pub time_estimate: Option<u64>,
}

/// 1Click deposit submission request
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OneClickSubmitDeposit {
    pub tx_hash: String,
    pub deposit_address: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub near_sender_account: Option<String>,
}

/// 1Click status response
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OneClickStatusResponse {
    pub status: String,
    #[serde(default)]
    pub swap_details: Option<OneClickSwapDetails>,
}

/// 1Click swap details
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OneClickSwapDetails {
    #[serde(default)]
    pub amount_out: Option<String>,
    #[serde(default)]
    pub amount_out_formatted: Option<String>,
    #[serde(default)]
    pub intent_hashes: Vec<String>,
    #[serde(default)]
    pub near_tx_hashes: Vec<String>,
}

/// Wallet backend trait — pluggable execution layer
#[async_trait::async_trait]
pub trait WalletBackend: Send + Sync {
    /// List available tokens
    async fn list_tokens(&self) -> Result<Vec<BackendTokenInfo>>;

    /// Get a 1Click swap quote (returns deposit address + pricing)
    async fn oneclick_quote(
        &self,
        _req: OneClickQuoteRequest,
    ) -> Result<OneClickQuoteResponse> {
        anyhow::bail!("1Click quotes are not supported by this backend");
    }

    /// Notify 1Click that deposit was made (speeds up processing)
    async fn oneclick_submit_deposit(
        &self,
        _req: OneClickSubmitDeposit,
    ) -> Result<()> {
        anyhow::bail!("1Click deposit submission is not supported by this backend");
    }

    /// Poll 1Click swap status by deposit address
    async fn oneclick_poll_status(
        &self,
        _deposit_address: &str,
    ) -> Result<OneClickStatusResponse> {
        anyhow::bail!("1Click status polling is not supported by this backend");
    }
}
