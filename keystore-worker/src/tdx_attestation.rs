//! TDX Attestation Generation for Keystore
//!
//! This module handles Intel TDX quote generation for keystore registration
//! with DAO contract. It communicates with Phala dstack socket.

use anyhow::{Context, Result};
use tracing::{info, warn};

// TDX Quote v4 = Header (48 bytes) + TD10 Report Body (584 bytes) + Auth Data
// Absolute offsets for measurements within the raw quote bytes:
const MEASUREMENT_SIZE: usize = 48; // Each measurement is 48 bytes (96 hex chars)
const MRTD_OFFSET: usize = 184;     // 48 (header) + 136 (body offset for mr_td)
const RTMR0_OFFSET: usize = 376;    // 48 (header) + 328 (body offset for rt_mr0)
const RTMR1_OFFSET: usize = 424;    // 48 (header) + 376 (body offset for rt_mr1)
const RTMR2_OFFSET: usize = 472;    // 48 (header) + 424 (body offset for rt_mr2)
const RTMR3_OFFSET: usize = 520;    // 48 (header) + 472 (body offset for rt_mr3)

/// All TEE measurements extracted from a TDX quote
#[derive(Debug, Clone)]
pub struct TdxMeasurements {
    pub mrtd: String,
    pub rtmr0: String,
    pub rtmr1: String,
    pub rtmr2: String,
    pub rtmr3: String,
}

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
    /// * `public_key_bytes` - 32-byte report_data binding (see tee_registration::report_data_binding)
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

        // Extract and log measurements (debug level — logged at info during registration)
        if let Some(m) = extract_all_measurements_from_bytes(&tdx_quote) {
            // INFO, not DEBUG, and deliberately so: `scripts/deploy_tdx.sh` step [3/6] greps this
            // line out of the keystore's log to learn which measurements to approve. At DEBUG it
            // is only emitted when the env filter happens to include `keystore_worker=debug` —
            // the process default does, but an explicit `RUST_LOG=info` silently drops it and the
            // deploy then waits ~4 minutes and aborts with "Could not read measurements". The
            // values are public (they end up on chain), so there is nothing to hide at INFO.
            info!("{}", format_measurements_line(&m));
        } else {
            warn!("⚠️ Quote too short to extract measurements: {} bytes (need {})", tdx_quote.len(), RTMR3_OFFSET + MEASUREMENT_SIZE);
        }

        Ok(tdx_quote)
    }
}

/// The one place the measurement line's wire format is defined.
///
/// `scripts/deploy_tdx.sh` parses this with `grep -oE 'MRTD=[a-f0-9]{96}'` (and the same per
/// RTMR), so the `KEY=<96 hex>` shape and the comma separator are a contract with the deploy
/// flow, not cosmetics. Note it is the `=` form — the worker logs a different `MRTD:  <hex>`
/// colon form, and the script's own comment calls out the difference. Covered by
/// `measurement_line_matches_what_the_deploy_script_greps`.
pub fn format_measurements_line(m: &TdxMeasurements) -> String {
    format!(
        "TDX Measurements: MRTD={}, RTMR0={}, RTMR1={}, RTMR2={}, RTMR3={}",
        m.mrtd, m.rtmr0, m.rtmr1, m.rtmr2, m.rtmr3
    )
}

/// Ready-to-paste `measurements` argument for the DAO's `add_approved_measurements`.
///
/// Printed when registration is blocked so the operator can approve without reassembling five
/// hex strings by hand. Field names match the contract's `ApprovedMeasurements`.
pub fn measurements_approval_args(m: &TdxMeasurements) -> String {
    format!(
        r#"{{"measurements":{{"mrtd":"{}","rtmr0":"{}","rtmr1":"{}","rtmr2":"{}","rtmr3":"{}"}},"clear_others":false}}"#,
        m.mrtd, m.rtmr0, m.rtmr1, m.rtmr2, m.rtmr3
    )
}

/// Extract all TEE measurements (MRTD + RTMR0-3) from raw TDX quote bytes.
fn extract_all_measurements_from_bytes(tdx_quote: &[u8]) -> Option<TdxMeasurements> {
    if tdx_quote.len() < RTMR3_OFFSET + MEASUREMENT_SIZE {
        return None;
    }
    Some(TdxMeasurements {
        mrtd: hex::encode(&tdx_quote[MRTD_OFFSET..MRTD_OFFSET + MEASUREMENT_SIZE]),
        rtmr0: hex::encode(&tdx_quote[RTMR0_OFFSET..RTMR0_OFFSET + MEASUREMENT_SIZE]),
        rtmr1: hex::encode(&tdx_quote[RTMR1_OFFSET..RTMR1_OFFSET + MEASUREMENT_SIZE]),
        rtmr2: hex::encode(&tdx_quote[RTMR2_OFFSET..RTMR2_OFFSET + MEASUREMENT_SIZE]),
        rtmr3: hex::encode(&tdx_quote[RTMR3_OFFSET..RTMR3_OFFSET + MEASUREMENT_SIZE]),
    })
}

/// Extract all TEE measurements from a hex-encoded TDX quote.
///
/// Returns None if the quote is too short or not a real TDX quote.
pub fn extract_all_measurements_from_quote_hex(tdx_quote_hex: &str) -> Option<TdxMeasurements> {
    let tdx_quote = hex::decode(tdx_quote_hex).ok()?;
    extract_all_measurements_from_bytes(&tdx_quote)
}
#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> TdxMeasurements {
        // 96 hex chars each — the real width of MRTD/RTMR0-3 (48 bytes).
        TdxMeasurements {
            mrtd: "7f".repeat(48),
            rtmr0: "a0".repeat(48),
            rtmr1: "b1".repeat(48),
            rtmr2: "c2".repeat(48),
            rtmr3: "d3".repeat(48),
        }
    }

    /// Emulate `grep -oE '<KEY>=[a-f0-9]{96}'` — what `scripts/deploy_tdx.sh` step [3/6] runs
    /// against this line to learn which measurements to approve.
    fn grep_measurement(line: &str, key: &str) -> Option<String> {
        let start = line.find(&format!("{key}="))? + key.len() + 1;
        let value: String = line[start..]
            .chars()
            .take_while(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
            .collect();
        (value.len() == 96).then_some(value)
    }

    /// The deploy flow parses this line out of the keystore's log; the format is therefore a
    /// contract, not cosmetics. If it drifts, `deploy_tdx.sh` does not fail fast — it polls for
    /// ~4 minutes and then aborts with "Could not read measurements", which reads like a broken
    /// node rather than a renamed log field. This test is here so that becomes a failing build.
    #[test]
    fn measurement_line_matches_what_the_deploy_script_greps() {
        let m = sample();
        let line = format_measurements_line(&m);

        assert_eq!(grep_measurement(&line, "MRTD").as_ref(), Some(&m.mrtd));
        assert_eq!(grep_measurement(&line, "RTMR0").as_ref(), Some(&m.rtmr0));
        assert_eq!(grep_measurement(&line, "RTMR1").as_ref(), Some(&m.rtmr1));
        assert_eq!(grep_measurement(&line, "RTMR2").as_ref(), Some(&m.rtmr2));
        assert_eq!(grep_measurement(&line, "RTMR3").as_ref(), Some(&m.rtmr3));

        // The `=` form specifically. The worker logs a `MRTD:  <hex>` colon form and the deploy
        // script's own comment calls out that these must not be confused; "harmonising" them
        // would silently break the keystore path.
        assert!(line.contains("MRTD="), "must use the = form: {line}");
        assert!(!line.contains("MRTD:"), "must NOT use the worker's colon form: {line}");
    }

    /// The printed approval command has to be pasteable as-is; a wrong field name means an
    /// operator approves a set that never matches and the keystore keeps refusing to register.
    #[test]
    fn approval_args_match_the_contract_argument_shape() {
        let m = sample();
        let v: serde_json::Value =
            serde_json::from_str(&measurements_approval_args(&m)).expect("must be valid JSON");

        assert_eq!(v["measurements"]["mrtd"], m.mrtd);
        assert_eq!(v["measurements"]["rtmr0"], m.rtmr0);
        assert_eq!(v["measurements"]["rtmr1"], m.rtmr1);
        assert_eq!(v["measurements"]["rtmr2"], m.rtmr2);
        assert_eq!(v["measurements"]["rtmr3"], m.rtmr3);
        // Approving a new set must never wipe the ones already approved: other live instances
        // are still registering against them.
        assert_eq!(v["clear_others"], false);
    }

    /// The line is built from the same values the pre-check sends to the DAO, so an operator
    /// approving what they read in the log approves exactly what the keystore will ask about.
    #[test]
    fn the_logged_line_and_the_approval_args_agree() {
        let m = sample();
        let line = format_measurements_line(&m);
        let v: serde_json::Value = serde_json::from_str(&measurements_approval_args(&m)).unwrap();

        for (key, field) in [
            ("MRTD", "mrtd"),
            ("RTMR0", "rtmr0"),
            ("RTMR1", "rtmr1"),
            ("RTMR2", "rtmr2"),
            ("RTMR3", "rtmr3"),
        ] {
            assert_eq!(
                grep_measurement(&line, key).as_deref(),
                v["measurements"][field].as_str(),
                "{key} differs between the log line and the approval args"
            );
        }
    }
}
