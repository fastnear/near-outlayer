//! TDX Attestation Generation for Keystore
//!
//! This module handles Intel TDX quote generation for keystore registration
//! with DAO contract. It communicates with Phala dstack socket.

use anyhow::{Context, Result};
use tracing::{info, warn};

/// Information from Phala dstack about the running app
#[derive(Debug, Clone)]
pub struct PhalaAppInfo {
    pub app_id: String,
}

/// Get Phala app info (app_id) from dstack socket
///
/// Returns None if not running in Phala TEE environment or if dstack is unavailable
pub async fn get_phala_app_info() -> Option<PhalaAppInfo> {
    use dstack_sdk::dstack_client::DstackClient;

    let client = DstackClient::new(None);
    match client.info().await {
        Ok(info) => {
            info!("📱 Phala app_id: {}", info.app_id);
            Some(PhalaAppInfo {
                app_id: info.app_id,
            })
        }
        Err(e) => {
            tracing::debug!("Could not get Phala app info (not in TEE?): {}", e);
            None
        }
    }
}

/// TDX attestation client
pub struct TdxClient {
    tee_mode: String,
}

impl TdxClient {
    /// Create new TDX client
    pub fn new(tee_mode: String) -> Self {
        Self { tee_mode }
    }

    /// Generate TDX quote for keystore registration with public key embedded
    ///
    /// This method embeds the keystore's public key (32 bytes) into the first
    /// 32 bytes of the TDX quote's report_data field. This allows the DAO
    /// contract to cryptographically verify that the public key was generated
    /// inside the TEE.
    ///
    /// # Arguments
    /// * `public_key_bytes` - Raw ed25519 public key bytes (32 bytes)
    ///
    /// # Returns
    /// * Hex-encoded TDX quote (ready to pass to submit_keystore_registration)
    pub async fn generate_registration_quote(&self, public_key_bytes: &[u8; 32]) -> Result<String> {
        info!("🔐 Generating registration TDX quote with embedded public key");
        info!("   Public key (hex): {}", hex::encode(public_key_bytes));

        match self.tee_mode.as_str() {
            "outlayer_tee" => {
                // OutLayer TEE mode: Generate real TDX quote with custom report_data
                info!("Using TDX attestation (Phala dstack socket)");

                // Create report_data: first 32 bytes = public key, rest = zeros
                let mut report_data = [0u8; 64];
                report_data[..32].copy_from_slice(public_key_bytes);

                // Call Phala dstack socket to generate TDX quote
                let tdx_quote = self.call_phala_dstack_socket(&report_data)
                    .await
                    .context("Failed to generate TDX quote via dstack socket")?;

                info!("✅ Generated real TDX quote (size: {} bytes)", tdx_quote.len());

                // Return hex-encoded quote (DAO contract expects hex string)
                Ok(hex::encode(&tdx_quote))
            }
            "none" => {
                // No attestation mode: Create minimal fake quote
                warn!("⚠️ Using NO-ATTESTATION mode (dev only!)");

                let fake_quote = format!(
                    "NO_ATTESTATION:pubkey={}",
                    hex::encode(public_key_bytes)
                );

                info!("✅ Generated NO-ATTESTATION stub (hex-encoded, size: {} bytes)", hex::encode(fake_quote.as_bytes()).len());

                Ok(hex::encode(fake_quote.as_bytes()))
            }
            other => {
                anyhow::bail!(
                    "Unsupported TEE mode for registration: {}. Use 'outlayer_tee' or 'none'",
                    other
                );
            }
        }
    }

    /// Call Phala dstack SDK to generate TDX quote
    ///
    /// This is the real TDX attestation generation for production Phala Cloud deployment.
    ///
    /// Uses dstack-sdk library which communicates with:
    /// - Unix socket: /var/run/dstack.sock (default on Linux)
    /// - HTTP API: http://localhost:8090 (if DSTACK_SIMULATOR_ENDPOINT is set)
    ///
    /// Returns raw TDX quote bytes (not hex!)
    async fn call_phala_dstack_socket(&self, report_data: &[u8; 64]) -> Result<Vec<u8>> {
        use dstack_sdk::dstack_client::DstackClient;

        info!("🔌 Calling Phala dstack SDK for TDX quote generation");
        info!("   report_data (hex): {}", hex::encode(report_data));

        // Create dstack client (auto-detects Unix socket or HTTP endpoint)
        let client = DstackClient::new(None); // None = use default /var/run/dstack.sock

        // Request TDX quote with report_data
        let response = client.get_quote(report_data.to_vec())
            .await
            .context("Failed to call dstack-sdk get_quote")?;

        info!("✅ Received TDX quote from dstack-sdk");
        info!("   quote (hex, first 100 chars): {}",
            if response.quote.len() > 100 { &response.quote[..100] } else { &response.quote });

        // Decode HEX quote to bytes (dstack-sdk returns HEX string, not base64)
        let tdx_quote = hex::decode(&response.quote)
            .context("Failed to decode TDX quote from HEX")?;

        info!("   TDX quote size: {} bytes", tdx_quote.len());

        // Debug: Extract and log RTMR3 from quote
        const RTMR3_OFFSET: usize = 256;
        const RTMR3_SIZE: usize = 48;
        if tdx_quote.len() >= RTMR3_OFFSET + RTMR3_SIZE {
            let rtmr3_bytes = &tdx_quote[RTMR3_OFFSET..RTMR3_OFFSET + RTMR3_SIZE];
            let rtmr3_hex = hex::encode(rtmr3_bytes);
            info!("📏 RTMR3 extracted from quote (offset {}, {} bytes): {}", RTMR3_OFFSET, RTMR3_SIZE, rtmr3_hex);
            info!("   ⚠️ Make sure this RTMR3 is in the DAO pre-approved list!");
            info!("   If not, run: near call dao.outlayer.testnet add_approved_rtmr3 '{{\"rtmr3\":\"{}\"}}'", rtmr3_hex);
        } else {
            warn!("⚠️ Quote too short to extract RTMR3: {} bytes (need {})", tdx_quote.len(), RTMR3_OFFSET + RTMR3_SIZE);
        }

        Ok(tdx_quote)
    }
}