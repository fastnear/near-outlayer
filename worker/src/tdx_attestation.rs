//! TDX Attestation Generation
//!
//! This module handles Intel TDX quote generation for worker registration
//! and task attestations. It communicates with Phala dstack socket or
//! directly with TDX hardware.

use anyhow::{Context, Result};
use sha2::{Digest, Sha256};

/// TDX attestation client
pub struct TdxClient {
    tee_mode: String,
}

impl TdxClient {
    /// Create new TDX client
    pub fn new(tee_mode: String) -> Self {
        Self { tee_mode }
    }

    /// Generate TDX quote for worker registration with public key embedded
    ///
    /// This method embeds the worker's public key (32 bytes) into the first
    /// 32 bytes of the TDX quote's report_data field. This allows the register
    /// contract to cryptographically verify that the public key was generated
    /// inside the TEE.
    ///
    /// # Arguments
    /// * `public_key_bytes` - Raw ed25519 public key bytes (32 bytes)
    ///
    /// # Returns
    /// * Hex-encoded TDX quote (ready to pass to register_worker_key)
    pub async fn generate_registration_quote(&self, public_key_bytes: &[u8; 32]) -> Result<String> {
        tracing::info!("🔐 Generating registration TDX quote with embedded public key");
        tracing::info!("   Public key (hex): {}", hex::encode(public_key_bytes));

        match self.tee_mode.as_str() {
            "tdx" => {
                // TDX mode: Generate real TDX quote with custom report_data
                tracing::info!("Using TDX attestation (Phala dstack socket)");

                // Create report_data: first 32 bytes = public key, rest = zeros
                let mut report_data = [0u8; 64];
                report_data[..32].copy_from_slice(public_key_bytes);

                // Call Phala dstack socket to generate TDX quote
                let tdx_quote = self.call_phala_dstack_socket(&report_data)
                    .await
                    .context("Failed to generate TDX quote via dstack socket")?;

                tracing::info!("✅ Generated real TDX quote (size: {} bytes)", tdx_quote.len());

                // Return hex-encoded quote (register contract expects hex string)
                Ok(hex::encode(&tdx_quote))
            }
            "simulated" => {
                // Simulated mode: Create fake quote with public key embedded
                tracing::warn!("⚠️  Using SIMULATED attestation (dev only!)");

                // Format: "SIMULATED:pubkey_hex:measurement_hex"
                let binary_path = std::env::current_exe()
                    .context("Failed to get current executable path")?;

                let binary = std::fs::read(&binary_path)
                    .context("Failed to read worker binary")?;

                let mut hasher = Sha256::new();
                hasher.update(&binary);
                let measurement = hasher.finalize();

                // Create fake quote structure (this won't verify with real dcap-qvl!)
                // For simulated mode, we create a minimal fake TDX quote structure
                // that includes the public key in report_data
                let fake_quote = format!(
                    "SIMULATED_TDX_QUOTE:pubkey={}:measurement={}",
                    hex::encode(public_key_bytes),
                    hex::encode(measurement)
                );

                tracing::info!("✅ Generated SIMULATED quote (hex-encoded, size: {} bytes)", fake_quote.len());

                // Return hex-encoded fake quote
                Ok(hex::encode(fake_quote.as_bytes()))
            }
            "none" => {
                // No attestation mode: Create minimal fake quote
                tracing::warn!("⚠️  Using NO-ATTESTATION mode (dev only!)");

                let fake_quote = format!(
                    "NO_ATTESTATION:pubkey={}",
                    hex::encode(public_key_bytes)
                );

                tracing::info!("✅ Generated NO-ATTESTATION stub (hex-encoded, size: {} bytes)", hex::encode(fake_quote.as_bytes()).len());

                Ok(hex::encode(fake_quote.as_bytes()))
            }
            other => {
                anyhow::bail!(
                    "Unsupported TEE mode for registration: {}. Use 'tdx', 'simulated', or 'none'",
                    other
                );
            }
        }
    }

    /// Generate TDX quote for task attestation
    ///
    /// This is used to create attestations for compilation and execution tasks.
    /// The report_data contains a hash of task parameters.
    ///
    /// # Arguments
    /// * `task_type` - Type of task (compile, execute, startup)
    /// * `task_id` - Unique task identifier
    /// * `repo` - GitHub repository URL
    /// * `commit` - Git commit hash
    /// * `build_target` - WASM build target
    /// * `wasm_hash` - SHA256 hash of compiled WASM
    /// * `input_hash` - SHA256 hash of execution input
    /// * `output_hash` - SHA256 hash of execution output
    /// * `block_height` - NEAR block height
    ///
    /// # Returns
    /// * Base64-encoded TDX quote
    pub async fn generate_task_attestation(
        &self,
        task_type: &str,
        task_id: i64,
        repo: Option<&str>,
        commit: Option<&str>,
        build_target: Option<&str>,
        wasm_hash: Option<&str>,
        input_hash: Option<&str>,
        output_hash: &str,
        block_height: Option<u64>,
    ) -> Result<String> {
        // Build report_data from task parameters
        let mut hasher = Sha256::new();
        hasher.update(task_type.as_bytes());
        hasher.update(&task_id.to_le_bytes());
        if let Some(r) = repo {
            hasher.update(r.as_bytes());
        }
        if let Some(c) = commit {
            hasher.update(c.as_bytes());
        }
        if let Some(bt) = build_target {
            hasher.update(bt.as_bytes());
        }
        if let Some(wh) = wasm_hash {
            hasher.update(wh.as_bytes());
        }
        if let Some(ih) = input_hash {
            hasher.update(ih.as_bytes());
        }
        hasher.update(output_hash.as_bytes());
        if let Some(bh) = block_height {
            hasher.update(&bh.to_le_bytes());
        }

        let task_hash = hasher.finalize();

        // Create report_data: [task_hash (32 bytes)][zeros (32 bytes)]
        let mut report_data = [0u8; 64];
        report_data[..32].copy_from_slice(&task_hash);

        match self.tee_mode.as_str() {
            "tdx" => {
                // Real TDX attestation
                let tdx_quote = self.call_phala_dstack_socket(&report_data)
                    .await
                    .context("Failed to generate TDX quote for task")?;
                Ok(base64::encode(&tdx_quote))
            }
            "simulated" => {
                // Simulated mode
                tracing::debug!("Generating simulated attestation for task {}", task_id);
                let binary_path = std::env::current_exe()
                    .context("Failed to get current executable path")?;
                let binary = std::fs::read(&binary_path)
                    .context("Failed to read worker binary")?;
                let mut measurement_hasher = Sha256::new();
                measurement_hasher.update(&binary);
                let measurement = measurement_hasher.finalize();

                let fake_quote = format!(
                    "SIMULATED:measurement={}:task_hash={}",
                    hex::encode(measurement),
                    hex::encode(task_hash)
                );
                Ok(base64::encode(fake_quote.as_bytes()))
            }
            "none" => {
                // Dev mode: Minimal fake quote
                tracing::warn!("Using no-attestation mode (dev only!)");
                Ok(base64::encode(b"no-attestation-dev-mode"))
            }
            other => {
                anyhow::bail!("Unsupported TEE mode for task attestation: {}", other);
            }
        }
    }

    /// Call Phala dstack SDK to generate TDX quote
    ///
    /// This is the real TDX attestation generation for production Phala Cloud deployment.
    ///
    /// Uses dstack-sdk library (same as MPC Node) which communicates with:
    /// - Unix socket: /var/run/dstack.sock (default on Linux)
    /// - HTTP API: http://localhost:8090 (if DSTACK_SIMULATOR_ENDPOINT is set)
    ///
    /// Returns HEX-encoded TDX quote (not base64!)
    async fn call_phala_dstack_socket(&self, report_data: &[u8; 64]) -> Result<Vec<u8>> {
        use dstack_sdk::dstack_client::DstackClient;

        tracing::info!("🔌 Calling Phala dstack SDK for TDX quote generation");
        tracing::debug!("   report_data (hex): {}", hex::encode(report_data));

        // Create dstack client (auto-detects Unix socket or HTTP endpoint)
        let client = DstackClient::new(None); // None = use default /var/run/dstack.sock

        // Request TDX quote with report_data
        let response = client.get_quote(report_data.to_vec())
            .await
            .context("Failed to call dstack-sdk get_quote")?;

        tracing::info!("✅ Received TDX quote from dstack-sdk");
        tracing::debug!("   quote (hex, first 100 chars): {}",
            if response.quote.len() > 100 { &response.quote[..100] } else { &response.quote });

        // Decode HEX quote to bytes (dstack-sdk returns HEX string, not base64)
        let tdx_quote = hex::decode(&response.quote)
            .context("Failed to decode TDX quote from HEX")?;

        tracing::info!(
            quote_size = tdx_quote.len(),
            "Successfully generated TDX quote via dstack-sdk"
        );

        // Debug: Extract and log RTMR3 from quote
        const RTMR3_OFFSET: usize = 256;
        const RTMR3_SIZE: usize = 48;
        if tdx_quote.len() >= RTMR3_OFFSET + RTMR3_SIZE {
            let rtmr3_bytes = &tdx_quote[RTMR3_OFFSET..RTMR3_OFFSET + RTMR3_SIZE];
            let rtmr3_hex = hex::encode(rtmr3_bytes);
            tracing::info!("📏 RTMR3 extracted from quote (offset {}, {} bytes): {}", RTMR3_OFFSET, RTMR3_SIZE, rtmr3_hex);
        } else {
            tracing::warn!("⚠️  Quote too short to extract RTMR3: {} bytes (need {})", tdx_quote.len(), RTMR3_OFFSET + RTMR3_SIZE);
        }

        Ok(tdx_quote)
    }
}

// Base64 encoding/decoding helpers
mod base64 {
    use ::base64::Engine;
    use ::base64::engine::general_purpose::STANDARD;

    pub fn encode<T: AsRef<[u8]>>(input: T) -> String {
        STANDARD.encode(input)
    }

    #[allow(dead_code)]
    pub fn decode<T: AsRef<[u8]>>(input: T) -> Result<Vec<u8>, ::base64::DecodeError> {
        STANDARD.decode(input)
    }
}
