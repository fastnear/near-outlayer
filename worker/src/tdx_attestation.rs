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
    pub fn generate_registration_quote(&self, public_key_bytes: &[u8; 32]) -> Result<String> {
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
                    .context("Failed to generate TDX quote via dstack socket")?;

                tracing::info!("✅ Generated TDX quote (size: {} bytes)", tdx_quote.len());

                // Return hex-encoded quote (register contract expects hex string)
                Ok(hex::encode(&tdx_quote))
            }
            "simulated" => {
                // Simulated mode: Create fake quote with public key embedded
                tracing::warn!("Using SIMULATED attestation (dev only!)");

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

                tracing::info!("✅ Generated simulated quote (size: {} bytes)", fake_quote.len());

                // Return hex-encoded fake quote
                Ok(hex::encode(fake_quote.as_bytes()))
            }
            "none" => {
                // No attestation mode: Create minimal fake quote
                tracing::warn!("Using NO-ATTESTATION mode (dev only!)");

                let fake_quote = format!(
                    "NO_ATTESTATION:pubkey={}",
                    hex::encode(public_key_bytes)
                );

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
    pub fn generate_task_attestation(
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

    /// Call Phala dstack socket to generate TDX quote
    ///
    /// This is the real TDX attestation generation for production Phala Cloud deployment.
    ///
    /// Phala dstack socket API:
    /// - Unix socket: /var/run/dstack.sock
    /// - HTTP POST with report_data (64 bytes)
    /// - Returns TDX quote (5-8KB binary)
    fn call_phala_dstack_socket(&self, report_data: &[u8; 64]) -> Result<Vec<u8>> {
        #[cfg(unix)]
        {
            use std::os::unix::net::UnixStream;
            use std::io::{Read, Write};

            tracing::info!("Calling Phala dstack socket for TDX quote generation");

            // Connect to Unix socket
            let mut stream = UnixStream::connect("/var/run/dstack.sock")
                .context("Failed to connect to /var/run/dstack.sock - is Phala dstack running?")?;

            // Prepare HTTP POST request
            let body = base64::encode(report_data);
            let request = format!(
                "POST /tdx/quote HTTP/1.1\r\n\
                 Host: localhost\r\n\
                 Content-Type: application/json\r\n\
                 Content-Length: {}\r\n\
                 \r\n\
                 {{\"report_data\":\"{}\"}}",
                body.len() + 17, // length of JSON with quotes
                body
            );

            // Send request
            stream.write_all(request.as_bytes())
                .context("Failed to send request to dstack socket")?;

            // Read response
            let mut response = Vec::new();
            stream.read_to_end(&mut response)
                .context("Failed to read response from dstack socket")?;

            // Parse HTTP response
            let response_str = String::from_utf8_lossy(&response);

            // Find JSON body (after \r\n\r\n)
            let body_start = response_str.find("\r\n\r\n")
                .context("Invalid HTTP response from dstack socket")?;
            let body = &response_str[body_start + 4..];

            // Parse JSON response
            let json: serde_json::Value = serde_json::from_str(body)
                .context("Failed to parse JSON response from dstack socket")?;

            let quote_base64 = json["quote"]
                .as_str()
                .context("Missing 'quote' field in dstack response")?;

            // Decode TDX quote from base64
            let tdx_quote = base64::decode(quote_base64)
                .context("Failed to decode TDX quote from base64")?;

            tracing::info!(
                quote_size = tdx_quote.len(),
                "Successfully generated TDX quote via dstack socket"
            );

            Ok(tdx_quote)
        }

        #[cfg(not(unix))]
        {
            anyhow::bail!(
                "TDX mode requires Unix socket support (Linux/macOS). \
                 For Windows development, use TEE_MODE=simulated instead."
            )
        }
    }
}

// Base64 encoding/decoding helpers
mod base64 {
    use ::base64::Engine;
    use ::base64::engine::general_purpose::STANDARD;

    pub fn encode<T: AsRef<[u8]>>(input: T) -> String {
        STANDARD.encode(input)
    }

    pub fn decode<T: AsRef<[u8]>>(input: T) -> Result<Vec<u8>, ::base64::DecodeError> {
        STANDARD.decode(input)
    }
}
