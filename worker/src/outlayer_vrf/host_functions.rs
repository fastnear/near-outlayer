//! VRF host functions for WASM components
//!
//! Implements the `near:vrf/api` WIT interface.
//! Uses Ed25519 deterministic signatures via keystore-worker.

use anyhow::Result;
use tracing::debug;
use wasmtime::component::Linker;

use crate::keystore_client::Attestation;

// Generate bindings from WIT
wasmtime::component::bindgen!({
    path: "wit",
    world: "near:vrf/vrf-host",
});

/// VRF result: (output_hex, signature_hex, alpha, error)
type VrfResult = (String, String, String, String);

/// Host state for VRF functions
pub struct VrfHostState {
    /// Request ID from the execution context (auto-prepended to seed)
    request_id: u64,
    /// Blocking HTTP client for keystore calls
    http_client: reqwest::blocking::Client,
    /// Keystore base URL
    keystore_url: String,
    /// Auth token for keystore
    auth_token: String,
    /// TEE session ID (provides actual auth via X-TEE-Session header)
    tee_session_id: Option<String>,
    /// Call counter for rate limiting
    call_count: u32,
    /// Max VRF calls per execution
    max_calls: u32,
}

impl VrfHostState {
    /// Create VRF host state
    ///
    /// `tee_session_id` provides the actual auth — attestation in the body is a stub
    /// (same pattern as storage client: TEE sessions handle auth).
    pub fn new(
        request_id: u64,
        keystore_url: &str,
        auth_token: &str,
        tee_session_id: Option<String>,
    ) -> Self {
        let http_client = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .connect_timeout(std::time::Duration::from_secs(5))
            .build()
            .expect("failed to build VRF HTTP client");

        Self {
            request_id,
            http_client,
            keystore_url: keystore_url.to_string(),
            auth_token: auth_token.to_string(),
            tee_session_id,
            call_count: 0,
            max_calls: 10,
        }
    }

    /// Stub attestation — real auth is via X-TEE-Session header
    /// (same approach as outlayer_storage::client::Attestation::for_mode)
    fn stub_attestation() -> Attestation {
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        Attestation {
            tee_type: "none".to_string(),
            quote: base64_encode(b"session-auth"),
            worker_pubkey: None,
            timestamp,
        }
    }
}

impl near::vrf::api::Host for VrfHostState {
    fn generate(&mut self, user_seed: String) -> VrfResult {
        debug!(
            "vrf::generate user_seed={}, request_id={}",
            user_seed, self.request_id
        );

        // Rate limit
        if self.call_count >= self.max_calls {
            return (
                String::new(),
                String::new(),
                String::new(),
                format!(
                    "VRF rate limit exceeded: {} calls (max: {})",
                    self.call_count + 1,
                    self.max_calls
                ),
            );
        }
        self.call_count += 1;

        // Construct alpha with auto-prepended request_id
        let alpha = format!("vrf:{}:{}", self.request_id, user_seed);

        // Call keystore (TEE session header provides auth, attestation is a stub)
        let attestation = Self::stub_attestation();

        let url = format!("{}/vrf/generate", self.keystore_url);

        let mut request = self
            .http_client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.auth_token))
            .json(&serde_json::json!({
                "alpha": alpha,
                "attestation": serde_json::json!({
                    "tee_type": attestation.tee_type,
                    "quote": attestation.quote,
                    "worker_pubkey": attestation.worker_pubkey,
                    "timestamp": attestation.timestamp,
                }),
            }));

        if let Some(ref session_id) = self.tee_session_id {
            request = request.header("X-TEE-Session", session_id.as_str());
        }

        match request.send() {
            Ok(response) => {
                if !response.status().is_success() {
                    let status = response.status();
                    let error = response.text().unwrap_or_default();
                    return (
                        String::new(),
                        String::new(),
                        alpha,
                        format!("Keystore VRF failed ({}): {}", status, error),
                    );
                }

                match response.json::<serde_json::Value>() {
                    Ok(json) => {
                        let output_hex = json["output_hex"].as_str().unwrap_or("").to_string();
                        let signature_hex =
                            json["signature_hex"].as_str().unwrap_or("").to_string();
                        debug!(
                            "vrf::generate success, output_len={}",
                            output_hex.len()
                        );
                        (output_hex, signature_hex, alpha, String::new())
                    }
                    Err(e) => (
                        String::new(),
                        String::new(),
                        alpha,
                        format!("Failed to parse VRF response: {}", e),
                    ),
                }
            }
            Err(e) => (
                String::new(),
                String::new(),
                alpha,
                format!("VRF request failed: {}", e),
            ),
        }
    }
}

/// Add VRF host functions to a wasmtime component linker
pub fn add_vrf_to_linker<T: Send + 'static>(
    linker: &mut Linker<T>,
    get_state: impl Fn(&mut T) -> &mut VrfHostState + Send + Sync + Copy + 'static,
) -> Result<()> {
    near::vrf::api::add_to_linker(linker, get_state)
}

fn base64_encode(data: &[u8]) -> String {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD.encode(data)
}
