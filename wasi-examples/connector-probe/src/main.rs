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
//! | `env` | 0 | EVERY system variable the worker is supposed to inject, present or missing — so one going quiet is a failed probe rather than a connector that misbehaves months later |
//!
//! (Prices above are the ones in `scripts/set_connector_prices_testnet.sh`.
//! Note `whoami` is listed there at $0.01, not 0 — the table row is stale and
//! the script is what the coordinator actually charges.)
//! | `secret` | $0.01 | the owner's secret arrived, WITHOUT printing it: presence, length, and a hash prefix |
//! | `burn` | $0.01 | compute actually costs something: burns instructions on demand, so a charge can be watched |
//! | `fetch` | $0.015 | the declared host is reachable |
//! | `forbidden_fetch` | $0.015 | an undeclared host is NOT, and the refusal comes from the worker rather than from politeness here |
//! | `vrf` | $0.01 | the ALPHA the randomness is bound to — the same seed through both doors must name the same account, differing only in the request id |
//! | `refund` | $0.01 | money handed back reaches `earnings_history`: a non-zero `refund_usd` and an `amount` reduced by it. The worker computed this for a while and never sent it |
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

/// Host bindings for VRF and refunds, generated from `wit/` — copies of the
/// worker's own `worker/wit/deps/{vrf,payment}.wit`.
///
/// Copies rather than path references because `wasi-examples/` is what an
/// author copies from, and an example that reaches up into the worker's tree
/// cannot be lifted out. `build.sh` diffs both, so drift is reported rather
/// than discovered.
///
/// WHAT IMPORTING THESE COSTS, because it is not free and the wallet interface
/// is the cautionary tale. The worker links a host interface only into a
/// component that imports it, and for two of the three it also REFUSES to run
/// such a component when the interface is unavailable:
///
///   * `near:payment/api` — always linked, even at `attached_usd = 0`, where a
///     refund simply comes back as an error. Importing it risks nothing.
///   * `near:vrf/api` — refused outright when the worker has no keystore
///     CONFIGURED. Configured, not reachable: `KEYSTORE_BASE_URL` is read from
///     the environment once, so a keystore that is down still leaves the
///     component instantiable and fails only the VRF call itself. The failure
///     mode is therefore a worker deployed without that variable — where every
///     operation here would stop, including `ping`. That is a uniform property
///     of a deployment rather than a per-request one, so it is accepted; if a
///     keystore-less worker is ever meant to serve this connector, move `vrf`
///     to its own module the way `wallet-probe` was split off.
///   * `outlayer:wallet/api` — refused whenever the request names no wallet,
///     which is MOST calls to this connector. That one stays out, and lives in
///     `wallet-probe` instead.
mod probe_host {
    wit_bindgen::generate!({
        world: "probe-host",
        path: "wit",
        generate_all,
    });
}
use probe_host::near::vrf::api as vrf;
use probe_host::near::payment::api as payment;

/// Every environment variable the worker injects, as of this build.
///
/// This list is the POINT of the `env` operation. `merge_env_vars` in the
/// worker is unit-tested, but nothing checked the whole chain — worker →
/// wasmtime → guest — so a variable dropped anywhere downstream would surface
/// as a connector quietly acting on the wrong identity, months later, with no
/// error anywhere. Here it surfaces as `found: false`.
///
/// Blank is NOT missing. On an HTTPS call the worker deliberately sets the
/// blockchain-only fields to empty strings, because a guest that reads
/// `NEAR_BLOCK_HEIGHT` should see "no block" rather than fall through to
/// whatever the host had. So the report distinguishes the two.
///
/// When the worker gains a variable, add it here. When one of these disappears
/// from the worker on purpose, delete it here in the same change — a list that
/// drifts is a list nobody trusts.
const SYSTEM_VARS: &[&str] = &[
    // Identity — who the guest acts as, and who pays.
    "NEAR_SENDER_ID",
    "NEAR_USER_ACCOUNT_ID",
    "NEAR_PREDECESSOR_ID",
    "NEAR_SIGNER_PUBLIC_KEY",
    // Where and what kind of run.
    "NEAR_NETWORK_ID",
    "OUTLAYER_EXECUTION_TYPE",
    // Correlation.
    "NEAR_REQUEST_ID",
    "OUTLAYER_CALL_ID",
    "NEAR_TRANSACTION_HASH",
    "NEAR_RECEIPT_ID",
    // On-chain context (empty on an HTTPS call).
    "NEAR_CONTRACT_ID",
    "NEAR_BLOCK_HEIGHT",
    "NEAR_BLOCK_TIMESTAMP",
    "NEAR_GAS_BURNT",
    // Money.
    "NEAR_PAYMENT_YOCTO",
    "ATTACHED_USD",
    "USD_PAYMENT",
    // Limits.
    "NEAR_MAX_INSTRUCTIONS",
    "NEAR_MAX_MEMORY_MB",
    "NEAR_MAX_EXECUTION_SECONDS",
    // Project.
    "OUTLAYER_PROJECT_ID",
    "OUTLAYER_PROJECT_UUID",
    "OUTLAYER_PROJECT_NAME",
    "OUTLAYER_PROJECT_OWNER",
    // Custody. Injected outside `merge_env_vars`, which is why it was missing
    // from every list until the worker's own `SYSTEM_ENV_VARS` was written
    // down and compared against the source.
    "WALLET_ID",
    // Capability advertisement, written by the P2 executor onto the WASI
    // builder. This probe found it on 2026-08-21 by reporting it as a name it
    // did not know — at which point it turned out to be in no list at all:
    // neither stripped from a caller's secrets nor refused as a secret key.
    "NEAR_RPC_PROXY_AVAILABLE",
];

/// The subset of [`SYSTEM_VARS`] whose ABSENCE is lawful, per the table in
/// `wasi-examples/WASM_ENV_VARS.md`: those are documented "If project" and
/// "If wallet", not "Yes".
///
/// A run that legitimately has neither still has to report every OTHER variable,
/// so these are excluded from the missing list rather than the report — they are
/// still shown, with `found: false`, which is the honest answer to "did it
/// arrive".
///
/// `WALLET_ID` earns its place here the hard way: an on-chain `request_execution`
/// reaches the worker through the event monitor, which carries the caller's
/// account and no wallet at all, so the variable is legitimately absent on that
/// door. Treating it as mandatory made this probe fail a healthy stack.
const CONDITIONAL_VARS: &[&str] = &[
    // "If project"
    "OUTLAYER_PROJECT_ID",
    "OUTLAYER_PROJECT_UUID",
    "OUTLAYER_PROJECT_NAME",
    "OUTLAYER_PROJECT_OWNER",
    // "If wallet"
    "WALLET_ID",
    // "If proxy" — absent whenever the run has no RPC proxy attached.
    "NEAR_RPC_PROXY_AVAILABLE",
];

/// Prefixes the WORKER owns. A variable with one of these that is not in
/// [`SYSTEM_VARS`] is reported BY NAME, because that is how a rename shows
/// itself: the old name goes missing and a new one appears in the same run.
const SYSTEM_PREFIXES: &[&str] = &["NEAR_", "OUTLAYER_", "ATTACHED_", "USD_"];

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
    /// `vrf` only: the caller's half of alpha. The host prepends the request
    /// id, which the guest cannot influence — that is what makes the result
    /// unmanipulable, and why the same seed on two doors is a fair comparison.
    #[serde(default)]
    seed: Option<String>,
    /// `refund` only: how much of `ATTACHED_USD` to hand back, in minimal
    /// units. Absent means all of it.
    #[serde(default)]
    refund_usd: Option<u64>,
}

/// One system variable, as the guest actually received it.
#[derive(Debug, Serialize)]
struct VarSeen {
    key: String,
    /// The worker set it. `false` means it never arrived — the thing this
    /// operation exists to catch.
    found: bool,
    /// Set but empty. The worker does this deliberately for blockchain fields
    /// on an HTTPS call, so it is a different fact from missing.
    blank: bool,
    /// The value. Safe to print because the worker STRIPS every name in
    /// `SYSTEM_ENV_VARS` from the owner's secrets before writing its own, so
    /// whatever arrives under one of these names came from the worker: an
    /// identifier, a network name, a number or a hash. That strip is what makes
    /// the claim true — while the guarantee was "system values are written
    /// last", the conditionally-written names (`OUTLAYER_PROJECT_OWNER` and
    /// friends) could still carry a secret straight into this field. The
    /// owner's secrets themselves are counted, not shown.
    #[serde(skip_serializing_if = "Option::is_none")]
    value: Option<String>,
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
    /// Who PAID, as opposed to who the guest acts as. Reported next to
    /// `sender_id` so a bound run is self-evident: the two differ exactly when
    /// a bound identity is in effect, and billing stays on this one.
    #[serde(skip_serializing_if = "Option::is_none")]
    user_account_id: Option<String>,
    /// `HTTPS` or `NEAR` — which door this run came through.
    #[serde(skip_serializing_if = "Option::is_none")]
    execution_type: Option<String>,
    // ---- env ----
    #[serde(skip_serializing_if = "Vec::is_empty")]
    system_env: Vec<VarSeen>,
    /// Names carrying a worker-owned prefix that this build does not know.
    /// Populated when the worker renames or adds a variable.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    unknown_system_vars: Vec<String>,
    /// Everything else in the environment — the owner's own secrets. COUNTED,
    /// never named and never valued.
    #[serde(skip_serializing_if = "Option::is_none")]
    other_var_count: Option<usize>,
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
    // ---- vrf ----
    /// The full alpha the HOST built. The whole point of the operation: it
    /// names the identity the randomness is bound to, and the two doors must
    /// agree on that name.
    #[serde(skip_serializing_if = "Option::is_none")]
    vrf_alpha: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    vrf_output: Option<String>,
    /// Present so a caller can verify the proof without this module being
    /// trusted about any of it.
    #[serde(skip_serializing_if = "Option::is_none")]
    vrf_signature: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    vrf_pubkey: Option<String>,
    // ---- refund ----
    /// What was asked back, and what the host said about it. `refund_error`
    /// empty means the refund stands.
    #[serde(skip_serializing_if = "Option::is_none")]
    refunded_usd: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    refund_error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    attached_usd: Option<String>,
}

/// Verifiable randomness, and — the reason this exists — the ALPHA it was
/// bound to.
///
/// `output` and `signature` are reported so a caller can verify the proof
/// without trusting this module about any of it. But the probe is `alpha`: the
/// host builds it as `vrf:{request_id}:{user_seed}` around an identity the
/// guest cannot choose, and a defect once made that identity the payment key's
/// owner on one door and the guest's borrowed name on the other. One module,
/// one flag, different randomness depending on how it was started.
///
/// So the same seed through both doors must produce an alpha naming the SAME
/// account, differing only in the request id.
fn vrf_report(op: &str, seed: &str) -> Output {
    let (output, signature, alpha, error) = vrf::generate(seed);
    if !error.is_empty() {
        return Output {
            ok: false,
            operation: op.into(),
            detail: format!("vrf::generate failed: {error}"),
            ..Default::default()
        };
    }
    // Absent rather than fatal: the proof above is complete without it, and a
    // key that cannot be read says nothing about the alpha under test.
    let (pubkey, pk_err) = vrf::pubkey();
    Output {
        ok: !alpha.is_empty(),
        operation: op.into(),
        detail: if alpha.is_empty() {
            "vrf::generate returned no alpha — there is nothing to bind the randomness to".into()
        } else {
            format!("alpha `{alpha}`")
        },
        vrf_alpha: Some(alpha),
        vrf_output: Some(output),
        vrf_signature: Some(signature),
        vrf_pubkey: pk_err.is_empty().then_some(pubkey),
        ..Default::default()
    }
}

/// Hand part of the attached payment back.
///
/// What this is for is the LEDGER, not the guest: a refund has to reach
/// `earnings_history` and reduce what the author is credited, and for a while
/// it did not — the worker computed it and never sent it, so the author was
/// paid for money the guest had returned. Nothing failed, and the figure was
/// simply wrong.
///
/// Absent `refund_usd` means "all of it", read from `ATTACHED_USD` — the same
/// value the host checks the refund against, so the ordinary call is a full
/// refund rather than an amount this module invented.
fn refund(op: &str, amount: Option<u64>) -> Output {
    let attached = env::var("ATTACHED_USD").unwrap_or_default();
    let want = match amount {
        Some(a) => a,
        None => attached.trim().parse::<u64>().unwrap_or(0),
    };
    let error = payment::refund_usd(want);
    Output {
        // A REFUSED refund is still a well-formed answer — over-refunding is
        // supposed to fail — so `ok` follows whether the host said anything
        // coherent, and the caller reads `refund_error` for the outcome.
        ok: true,
        operation: op.into(),
        detail: if error.is_empty() {
            format!("refunded {want} of {attached}")
        } else {
            format!("refund of {want} refused: {error}")
        },
        refunded_usd: error.is_empty().then_some(want),
        refund_error: (!error.is_empty()).then_some(error),
        attached_usd: Some(attached),
        ..Default::default()
    }
}

fn main() {
    let input = match env::input_json::<Input>() {
        Ok(Some(i)) => i,
        Ok(None) => Input {
            operation: String::new(),
            rounds: None,
            seed: None,
            refund_usd: None,
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
        "env" => env_report(op),
        "secret" => secret(op),
        "burn" => burn(op, input.rounds.unwrap_or(1)),
        "vrf" => vrf_report(op, input.seed.as_deref().unwrap_or("probe")),
        "refund" => refund(op, input.refund_usd),
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
/// Who the guest was told it is — BOTH identities, because Agent Connect
/// splits them and the whole point of `use_bound_identity` is that only one
/// of the two moves.
///
/// * `sender_id` (`NEAR_SENDER_ID`) is who the guest ACTS AS. near-email turns
///   it into the mailbox it sends from. This is the one a binding renames.
/// * `user_account_id` (`NEAR_USER_ACCOUNT_ID`) is who PAID. It must never
///   follow a binding — if it did, usage would be attributed to an account
///   that paid nothing.
///
/// `execution_type` is here so one probe answers for both entry paths: the
/// same request run over HTTPS and over an on-chain transaction has to give
/// the same `sender_id`, and without this field a run tells you nothing about
/// which path produced it.
fn whoami(op: &str) -> Output {
    let sender = env::signer_account_id();
    let payer = env::var("NEAR_USER_ACCOUNT_ID");
    // Both empty-or-missing is its own fact: it means the worker told the guest
    // nothing about who it is, which no run should ever look like.
    let detail = match (sender.as_deref(), payer.as_deref()) {
        (None, _) => "no sender was injected at all — this run has no identity".to_string(),
        (Some(s), Some(p)) if s == p => {
            format!("acting as the caller '{s}' — no bound identity in effect")
        }
        (Some(s), Some(p)) => {
            format!("acting as '{s}', paid by '{p}' — a bound identity IS in effect")
        }
        (Some(s), None) => format!("acting as '{s}'; no payer was injected"),
    };
    Output {
        ok: sender.is_some(),
        operation: op.into(),
        detail,
        sender_id: sender,
        user_account_id: payer,
        execution_type: env::var("OUTLAYER_EXECUTION_TYPE"),
        request_id: env::request_id(),
        call_id: env::var("OUTLAYER_CALL_ID"),
        network: env::var("NEAR_NETWORK_ID"),
        ..Default::default()
    }
}

/// Every system variable, present or missing — and nothing else.
///
/// Reads the environment DIRECTLY rather than through the known list alone, so
/// the report answers two questions at once: did anything we expect go
/// missing, and did anything we do not expect appear. A rename shows as both
/// at the same time, in one run.
///
/// **It cannot leak a secret, by construction.** The owner's secrets are in
/// this same environment — the worker merges them first and writes the system
/// variables over them — so a naive dump would turn every test run into a
/// leak, which is exactly what this module's rules forbid. Three tiers
/// instead: known system variables get their value (none of them ever carries
/// a secret), unknown variables carrying a worker-owned PREFIX get their name
/// only (enough to spot a rename), and everything else is counted and never
/// named.
fn env_report(op: &str) -> Output {
    let system_env: Vec<VarSeen> = SYSTEM_VARS
        .iter()
        .map(|key| match env::var(key) {
            Some(value) => VarSeen {
                key: (*key).into(),
                found: true,
                blank: value.is_empty(),
                value: Some(value),
            },
            None => VarSeen {
                key: (*key).into(),
                found: false,
                blank: false,
                value: None,
            },
        })
        .collect();

    let mut unknown_system_vars = Vec::new();
    let mut other_var_count = 0usize;
    for (key, _) in std::env::vars() {
        if SYSTEM_VARS.contains(&key.as_str()) {
            continue;
        }
        if SYSTEM_PREFIXES.iter().any(|p| key.starts_with(p)) {
            unknown_system_vars.push(key);
        } else {
            other_var_count += 1;
        }
    }
    unknown_system_vars.sort();

    let missing: Vec<&str> = system_env
        .iter()
        .filter(|v| !v.found && !CONDITIONAL_VARS.contains(&v.key.as_str()))
        .map(|v| v.key.as_str())
        .collect();
    let detail = if missing.is_empty() && unknown_system_vars.is_empty() {
        "every system variable this build knows about arrived".to_string()
    } else if missing.is_empty() {
        format!(
            "all known variables arrived, but the worker also sent {} this build \
             does not know: {}. Add them to SYSTEM_VARS — or, if one replaced a \
             variable that is now missing, that was a rename.",
            unknown_system_vars.len(),
            unknown_system_vars.join(", ")
        )
    } else {
        format!(
            "MISSING: {}. A guest reading one of these gets nothing, and nothing \
             upstream reports an error — which is why this probe exists.",
            missing.join(", ")
        )
    };

    Output {
        // `ok` is about the ENVIRONMENT, not about the call: a run that
        // reaches this code and reports a missing variable succeeded as a
        // call and failed as a probe, and the test wants the second answer.
        ok: missing.is_empty(),
        operation: op.into(),
        detail,
        system_env,
        unknown_system_vars,
        other_var_count: Some(other_var_count),
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
