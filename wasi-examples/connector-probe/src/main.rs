//! A connector that does nothing useful, so that everything AROUND a connector
//! can be tested.
//!
//! near.email is the only real connector, and it is mainnet-only — which leaves
//! the whole connector path untestable on testnet. This one exists to fill that
//! gap: it is published like a connector, priced like a connector, metered like
//! a connector, and it carries a manifest with a real outbound allowlist. What
//! it does with all that is report back.
//!
//! ## What each operation is for
//!
//! | `operation` | Price | What it proves |
//! |---|---|---|
//! | `ping` | 0 | a free operation is still a REAL price: it runs only on a key that can pay, and it is refused when the key cannot |
//! | `whoami` | 0 | what the guest was told about its caller — the identity the worker injected, and nothing the caller could have chosen |
//! | `secret` | $0.01 | the owner's secret arrived, WITHOUT printing it: presence, length, and a hash prefix |
//! | `burn` | $0.01 | compute actually costs something: burns instructions on demand, so a charge can be watched |
//! | `fetch` | $0.015 | the declared host is reachable |
//! | `forbidden_fetch` | $0.015 | an undeclared host is NOT, and the refusal comes from the worker rather than from politeness here |
//!
//! And one operation that is deliberately absent from the price list —
//! `unpriced` — which the coordinator must refuse BEFORE this module runs. It
//! is not implemented here on purpose: if execution ever reaches it, the
//! fail-closed pricing rule has a hole.
//!
//! ## What it never does
//!
//! It never returns a secret's value, and never accepts a host to fetch from
//! the caller. A test tool that could be pointed at an arbitrary host would be
//! an SSRF gadget with a TEE's network access, and one that echoed secrets
//! would make every test run a leak.

use outlayer::env;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

// ==================== Connector manifest ====================

/// The manifest, embedded in a wasm custom section so it is covered by the
/// wasm hash the contract records for this version.
///
/// It declares the outbound allowlist the TEE worker enforces: this module may
/// reach `rpc.testnet.fastnear.com` and nothing else. `#[used]` keeps the
/// linker from dropping a static nothing references — without it the module
/// would publish with no allowlist and, being a connector, be refused all
/// outbound network. See `wasi-examples/CONNECTOR_MANIFEST.md`.
#[used]
#[link_section = "outlayer.manifest"]
static OUTLAYER_MANIFEST: [u8; include_bytes!("../manifest.json").len()] =
    *include_bytes!("../manifest.json");

/// The one host this connector may reach, and it must match `manifest.json`.
/// A change here without a change there produces a connector that cannot reach
/// its own backend.
const ALLOWED_HOST: &str = "https://rpc.testnet.fastnear.com";

/// A host the manifest does NOT declare. Fixed, not taken from the caller: a
/// connector that fetched a caller-chosen host would be an SSRF gadget running
/// inside a TEE with keys in it.
const UNDECLARED_HOST: &str = "https://example.com";

/// Secret names this connector looks for.
///
/// Ordinary names, not `PROTECTED_` ones: the point is to prove the OWNER's
/// secret reached the guest, and these are what an author would store.
const SECRET_KEYS: [&str; 2] = ["PROBE_TOKEN", "PROBE_SECOND"];

#[derive(Debug, Deserialize)]
struct Input {
    /// Which operation, under the ONE universal field name every connector
    /// uses.
    ///
    /// This is the same string the contract read to price the call and the
    /// coordinator read to bill it — one value, out of these very bytes, so
    /// there is nothing to keep in step. This module is still told nothing
    /// about the price, and must not be: a price inside a guest is a second
    /// copy that wins on money whenever the two disagree.
    ///
    /// It was `op` once. Renamed because the standard is `operation`, the same
    /// word the on-chain price table uses, and `op` was the only place it was
    /// abbreviated.
    #[serde(default)]
    operation: String,
    /// `burn` only: roughly how much work to do. Capped below.
    #[serde(default)]
    rounds: Option<u32>,
}

#[derive(Debug, Serialize)]
struct SecretSeen {
    key: String,
    found: bool,
    /// Length only. Enough to tell "the right secret" from "some secret", and
    /// useless to anybody reading a test log.
    len: Option<usize>,
    /// First eight hex characters of SHA256(value). Lets a test assert WHICH
    /// secret arrived without the value ever leaving the TEE.
    sha256_prefix: Option<String>,
}

#[derive(Debug, Serialize, Default)]
struct Output {
    ok: bool,
    /// Echoed back under the same universal name the request used.
    operation: String,
    /// Free-text, for a human reading a failed run.
    detail: String,
    // ---- whoami ----
    #[serde(skip_serializing_if = "Option::is_none")]
    sender_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    request_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    call_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    network: Option<String>,
    // ---- secret ----
    #[serde(skip_serializing_if = "Vec::is_empty")]
    secrets: Vec<SecretSeen>,
    // ---- burn ----
    #[serde(skip_serializing_if = "Option::is_none")]
    rounds_done: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    checksum: Option<u64>,
    // ---- fetch ----
    #[serde(skip_serializing_if = "Option::is_none")]
    http_status: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    http_error: Option<String>,
}

fn main() {
    let input = match env::input_json::<Input>() {
        Ok(Some(i)) => i,
        Ok(None) => Input {
            operation: String::new(),
            rounds: None,
        },
        Err(e) => {
            let _ = env::output_json(&Output {
                ok: false,
                operation: String::new(),
                detail: format!("input is not JSON: {e}"),
                ..Default::default()
            });
            return;
        }
    };

    let out = run(&input);
    let _ = env::output_json(&out);
}

fn run(input: &Input) -> Output {
    let op = input.operation.trim();
    match op {
        "ping" => Output {
            ok: true,
            operation: op.into(),
            detail: "the key could pay for a free operation".into(),
            ..Default::default()
        },
        "whoami" => whoami(op),
        "secret" => secret(op),
        "burn" => burn(op, input.rounds.unwrap_or(1)),
        "fetch" => fetch(op, ALLOWED_HOST, "the declared host must be reachable"),
        "forbidden_fetch" => fetch(
            op,
            UNDECLARED_HOST,
            "an undeclared host must be refused by the worker, not by this module",
        ),
        "" => Output {
            ok: false,
            operation: op.into(),
            detail: "no `operation` in the input. Every layer prices and refuses \
                     by that one field, so a call reaching this module without \
                     one means something upstream priced nothing."
                .into(),
            ..Default::default()
        },
        other => Output {
            ok: false,
            operation: other.into(),
            detail: format!(
                "unknown operation `{other}`. If this ran at all, the coordinator \
                 priced an operation this module does not implement — which the \
                 fail-closed price lookup is supposed to prevent."
            ),
            ..Default::default()
        },
    }
}

/// What the guest was told about its caller.
///
/// Every value here is injected by the worker. None of it is anything the
/// caller could have set in the request body, which is the point: a test can
/// compare what it sent with what the guest saw.
fn whoami(op: &str) -> Output {
    Output {
        ok: true,
        operation: op.into(),
        detail: "identity as the worker injected it".into(),
        sender_id: env::signer_account_id(),
        request_id: env::request_id(),
        call_id: env::var("OUTLAYER_CALL_ID"),
        network: env::var("NEAR_NETWORK_ID"),
        ..Default::default()
    }
}

/// Did the owner's secret arrive, and which one?
///
/// Reports presence, length and a hash prefix — never the value. That is not
/// squeamishness: these tests are run against real credentials, and a tool that
/// printed them would turn every run into a leak and every log into a store of
/// somebody else's password.
fn secret(op: &str) -> Output {
    let secrets: Vec<SecretSeen> = SECRET_KEYS
        .iter()
        .map(|key| match env::var(key) {
            Some(value) => SecretSeen {
                key: (*key).into(),
                found: true,
                len: Some(value.len()),
                sha256_prefix: Some(hex::encode(Sha256::digest(value.as_bytes()))[..8].to_string()),
            },
            None => SecretSeen {
                key: (*key).into(),
                found: false,
                len: None,
                sha256_prefix: None,
            },
        })
        .collect();

    let found = secrets.iter().filter(|s| s.found).count();
    Output {
        ok: found > 0,
        operation: op.into(),
        detail: if found > 0 {
            format!("{found} of {} secret(s) reached the guest", SECRET_KEYS.len())
        } else {
            "no secret reached the guest. Either none was stored for this agent, \
             or the call did not ask for one (X-Use-Owner-Secret)."
                .into()
        },
        secrets,
        ..Default::default()
    }
}

/// Burn instructions on demand, so a compute charge can be watched moving.
///
/// Capped, and the cap is the point: an uncapped loop in a priced operation is
/// a way for one caller to hold a worker for as long as they like. The
/// execution limits would stop it eventually, but "eventually" is a worker that
/// is not serving anybody else.
fn burn(op: &str, rounds: u32) -> Output {
    const MAX_ROUNDS: u32 = 200;
    let rounds = rounds.min(MAX_ROUNDS);

    // Deliberately not optimised away: each round hashes the previous digest.
    let mut digest = Sha256::digest(b"probe").to_vec();
    for _ in 0..rounds {
        for _ in 0..1_000 {
            digest = Sha256::digest(&digest).to_vec();
        }
    }
    let checksum = u64::from_le_bytes(digest[..8].try_into().unwrap_or([0; 8]));

    Output {
        ok: true,
        operation: op.into(),
        detail: format!("burned {rounds} round(s) of a thousand hashes"),
        rounds_done: Some(rounds),
        checksum: Some(checksum),
        ..Default::default()
    }
}

/// Reach a host, and report what happened.
///
/// `ok` is true only when the request COMPLETED. For `forbidden_fetch` that
/// makes a `false` the passing result: the worker's allowlist is supposed to
/// stop it, and a success there means an undeclared host was reachable from
/// inside a TEE that holds keys.
fn fetch(op: &str, url: &str, detail: &str) -> Output {
    match wasi_http_client::Client::new().get(url).send() {
        Ok(resp) => Output {
            ok: true,
            operation: op.into(),
            detail: detail.into(),
            http_status: Some(resp.status()),
            ..Default::default()
        },
        Err(e) => Output {
            ok: false,
            operation: op.into(),
            detail: detail.into(),
            http_error: Some(format!("{e}")),
            ..Default::default()
        },
    }
}
