//! Browser-side TDX quote verification.
//!
//! Wraps `dcap-qvl` so the dashboard can run the real Intel checks in the visitor's own browser:
//! signature chain to the Intel root that is embedded in the crate, TCB status, and the
//! measurements decoded from the verified quote. Nothing here trusts the coordinator — the quote
//! and the collateral are inputs, and a tampered one fails.
//!
//! Every entry point is total: errors come back as `{ ok: false, error }`, never as a panic that
//! would surface as an opaque "unreachable executed" in the console.

use serde::Serialize;
use wasm_bindgen::prelude::*;

/// Measurements as decoded from a quote. Lowercase hex, Intel-style.
#[derive(Serialize, Default)]
pub struct Measurements {
    pub mrtd: String,
    pub rtmr0: String,
    pub rtmr1: String,
    pub rtmr2: String,
    pub rtmr3: String,
}

#[derive(Serialize)]
pub struct ParseResult {
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// Platform id, lowercase hex — the key for fetching the right collateral.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fmspc: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub measurements: Option<Measurements>,
    /// First 32 bytes of report_data, hex: the commitment the TEE made to the task.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub report_data_prefix: Option<String>,
    /// Remaining 32 bytes, hex. Expected to be all zeros.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub report_data_suffix: Option<String>,
}

#[derive(Serialize)]
pub struct VerifyResult {
    /// True only when the Intel signature chain and the TCB material both check out.
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// Platform TCB status verbatim. `verify()` returns Ok for `OutOfDate` and
    /// `ConfigurationNeeded` too — only `Revoked` errors — so the caller must read this rather than
    /// treat a successful call as "fully up to date".
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    #[serde(default)]
    pub advisory_ids: Vec<String>,
    /// Measurements taken from the VERIFIED quote, not from any surrounding metadata.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub measurements: Option<Measurements>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub report_data_prefix: Option<String>,
}

fn measurements_of(report: &dcap_qvl::quote::Report) -> Option<Measurements> {
    let td = report.as_td10()?;
    Some(Measurements {
        mrtd: hex::encode(td.mr_td),
        rtmr0: hex::encode(td.rt_mr0),
        rtmr1: hex::encode(td.rt_mr1),
        rtmr2: hex::encode(td.rt_mr2),
        rtmr3: hex::encode(td.rt_mr3),
    })
}

fn to_js<T: Serialize>(value: &T) -> JsValue {
    serde_wasm_bindgen::to_value(value).unwrap_or(JsValue::NULL)
}

/// Decode a quote without verifying it.
///
/// Used for one thing only: learning the FMSPC so the caller can fetch the collateral that matches
/// this platform. Everything it returns is unverified by definition — display nothing from here as
/// established fact.
#[wasm_bindgen]
pub fn parse_quote(quote: &[u8]) -> JsValue {
    let parsed = match dcap_qvl::quote::Quote::parse(quote) {
        Ok(q) => q,
        Err(e) => {
            return to_js(&ParseResult {
                ok: false,
                error: Some(format!("quote could not be decoded: {e:?}")),
                fmspc: None,
                measurements: None,
                report_data_prefix: None,
                report_data_suffix: None,
            })
        }
    };

    let fmspc = dcap_qvl::intel::quote_fmspc(&parsed)
        .ok()
        .map(|f| hex::encode(f).to_lowercase());

    let (prefix, suffix) = match parsed.report.as_td10() {
        Some(td) => (
            Some(hex::encode(&td.report_data[..32])),
            Some(hex::encode(&td.report_data[32..])),
        ),
        None => (None, None),
    };

    to_js(&ParseResult {
        ok: fmspc.is_some(),
        error: fmspc
            .is_none()
            .then(|| "quote carries no FMSPC (not a TDX DCAP quote?)".to_string()),
        fmspc,
        measurements: measurements_of(&parsed.report),
        report_data_prefix: prefix,
        report_data_suffix: suffix,
    })
}

/// Verify a quote against Intel-issued collateral, as of `now_secs`.
///
/// `now_secs` must be the moment the attestation was produced, not the wall clock: collateral is
/// only valid inside its own window, so a past execution has to be judged against the collateral
/// that was current when it ran.
#[wasm_bindgen]
pub fn verify_quote(quote: &[u8], collateral_json: &str, now_secs: f64) -> JsValue {
    let fail = |msg: String| {
        to_js(&VerifyResult {
            ok: false,
            error: Some(msg),
            status: None,
            advisory_ids: Vec::new(),
            measurements: None,
            report_data_prefix: None,
        })
    };

    let collateral: dcap_qvl::QuoteCollateralV3 = match serde_json::from_str(collateral_json) {
        Ok(c) => c,
        Err(e) => return fail(format!("collateral could not be parsed: {e}")),
    };

    if !(now_secs.is_finite() && now_secs >= 0.0) {
        return fail("invalid verification timestamp".to_string());
    }

    match dcap_qvl::verify::verify(quote, &collateral, now_secs as u64) {
        Ok(report) => to_js(&VerifyResult {
            ok: true,
            error: None,
            status: Some(report.status.clone()),
            advisory_ids: report.advisory_ids.clone(),
            measurements: measurements_of(&report.report),
            report_data_prefix: report
                .report
                .as_td10()
                .map(|td| hex::encode(&td.report_data[..32])),
        }),
        Err(e) => fail(format!("{e:?}")),
    }
}
