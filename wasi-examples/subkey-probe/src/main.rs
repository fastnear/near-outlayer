//! subkey-probe — a connector whose only job is exercising EVM sub-keys.
//!
//! A connector holds funds under addresses of its own: one secp256k1 key per
//! `label`, derived by the keystore under a path the WORKER builds from the
//! connector's verified manifest (`connector.{connector_id}.{label}`). Nothing
//! else on testnet imports the wallet interface from a connector, so nothing
//! else can show that path working end to end — or show what is refused:
//!
//! | `operation` | price | shows |
//! |---|---|---|
//! | `address` | free | the empty label is `default`; `default`/`trading`/`bridge` are three addresses, none the wallet's own (`get-address`); a label is one key on every EVM chain |
//! | `sign` | paid | a `personal_sign` under `trading` recovers to the `trading` address; one under the empty label recovers to `default`, never to the wallet's own address |
//! | `foreign_label` | free | a label that could spell another connector, or a sub-key on a non-EVM chain, is refused |
//!
//! It is a connector (manifest with `connector_id`, deployed under
//! `connectors.outlayer.testnet`) and imports `outlayer:wallet`, so every call
//! must carry a wallet: a payment key the wallet owns plus `X-Wallet-Id`.
//! `network` is empty on purpose — the wallet functions are host calls, not
//! HTTP, and this module reaches nothing else.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::io::{Read, Write};

/// The manifest, embedded so it is covered by the wasm hash the contract
/// records. `connector_id` is what makes sub-keys available at all.
#[cfg(target_family = "wasm")]
#[used]
#[link_section = "outlayer.manifest"]
static OUTLAYER_MANIFEST: [u8; include_bytes!("../manifest.json").len()] =
    *include_bytes!("../manifest.json");

/// Wallet host bindings, generated from `wit/wallet.wit` — a copy of the
/// worker's `worker/wit/deps/wallet.wit`; `build.sh` diffs the two.
mod wallet_host {
    wit_bindgen::generate!({
        world: "wallet-host",
        path: "wit",
    });
}
use wallet_host::outlayer::wallet::api as wallet;

/// The EVM chain every sub-key is asked for. Any EVM name derives the same key;
/// `address` also asks `hyperevm` to show exactly that.
const CHAIN: &str = "base";

#[derive(Debug, Deserialize, Default)]
struct Input {
    #[serde(default)]
    operation: String,
}

#[derive(Debug, Serialize, Default)]
struct Refusal {
    label: String,
    chain: String,
    error: String,
}

#[derive(Debug, Serialize, Default)]
struct Output {
    ok: bool,
    operation: String,
    detail: String,
    // ---- address ----
    /// label → address. The empty label is the `default` sub-key.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    addresses: BTreeMap<String, String>,
    /// The wallet's own EVM address (`get-address`), which no sub-key may equal.
    #[serde(skip_serializing_if = "Option::is_none")]
    wallet_own: Option<String>,
    /// The `trading` address asked for on `hyperevm` — must equal the `base` one.
    #[serde(skip_serializing_if = "Option::is_none")]
    trading_on_hyperevm: Option<String>,
    // ---- sign ----
    #[serde(skip_serializing_if = "Option::is_none")]
    signature: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    signer_expected: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    signer_recovered: Option<String>,
    /// Who signed when the label was empty — must be the `default` sub-key.
    #[serde(skip_serializing_if = "Option::is_none")]
    empty_label_signer: Option<String>,
    // ---- foreign_label ----
    /// Every refusal, with what was asked. All of them must be refusals.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    refusals: Vec<Refusal>,
    /// A request that should have been refused but was not.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    accepted_but_should_not_be: Vec<Refusal>,
}

fn main() {
    let mut raw = Vec::new();
    if std::io::stdin().read_to_end(&mut raw).is_err() {
        emit(&Output {
            ok: false,
            detail: "could not read stdin".into(),
            ..Default::default()
        });
        return;
    }
    let input: Input = match serde_json::from_slice(&raw) {
        Ok(i) => i,
        Err(e) => {
            emit(&Output {
                ok: false,
                detail: format!("input is not JSON: {e}"),
                ..Default::default()
            });
            return;
        }
    };
    emit(&run(&input));
}

fn emit(out: &Output) {
    let mut stdout = std::io::stdout();
    let _ = stdout.write_all(serde_json::to_string(out).unwrap_or_default().as_bytes());
    let _ = stdout.flush();
}

fn run(input: &Input) -> Output {
    let op = input.operation.trim();
    match op {
        "address" => address(op),
        "sign" => sign(op),
        "foreign_label" => foreign_label(op),
        "" => Output {
            ok: false,
            operation: op.into(),
            detail: "no `operation` in the input".into(),
            ..Default::default()
        },
        other => Output {
            ok: false,
            operation: other.into(),
            detail: format!("unknown operation `{other}` (address, sign, foreign_label)"),
            ..Default::default()
        },
    }
}

/// The `address` field out of a `get-sub-key-address` answer.
fn address_of(chain: &str, label: &str) -> Result<String, String> {
    let (json, err) = wallet::get_sub_key_address(chain, label);
    if !err.is_empty() {
        return Err(err);
    }
    let v: serde_json::Value =
        serde_json::from_str(&json).map_err(|e| format!("address answer is not JSON: {e}"))?;
    v.get("address")
        .and_then(|a| a.as_str())
        .map(|a| a.to_ascii_lowercase())
        .ok_or_else(|| "address answer carries no `address`".to_string())
}

fn address(op: &str) -> Output {
    let mut out = Output {
        operation: op.into(),
        ..Default::default()
    };
    for label in ["", "default", "trading", "bridge"] {
        match address_of(CHAIN, label) {
            Ok(a) => {
                out.addresses.insert(label.to_string(), a);
            }
            Err(e) => {
                out.detail = format!("get-sub-key-address({CHAIN}, {label:?}) failed: {e}");
                return out;
            }
        }
    }
    match address_of("hyperevm", "trading") {
        Ok(a) => out.trading_on_hyperevm = Some(a),
        Err(e) => {
            out.detail = format!("get-sub-key-address(hyperevm, trading) failed: {e}");
            return out;
        }
    }

    let own = match own_address(CHAIN) {
        Ok(a) => a,
        Err(e) => {
            out.detail = format!("get-address({CHAIN}) failed: {e}");
            return out;
        }
    };
    out.wallet_own = Some(own.clone());

    // "" and "default" are one key; default/trading/bridge are three; none of
    // them is the wallet's own key — the one address a guest must never be
    // able to sign for.
    let empty_is_default = out.addresses.get("") == out.addresses.get("default");
    let distinct: std::collections::BTreeSet<&String> = out.addresses.values().collect();
    let none_is_own = !distinct.contains(&own);
    let same_across_chains = out.trading_on_hyperevm.as_deref() == out.addresses.get("trading").map(String::as_str);
    out.ok = empty_is_default && distinct.len() == 3 && none_is_own && same_across_chains;
    out.detail = if !empty_is_default {
        "the empty label and `default` derived DIFFERENT addresses".into()
    } else if distinct.len() != 3 {
        "two labels derived the SAME address — sub-keys are not distinct".into()
    } else if !none_is_own {
        "a sub-key IS the wallet's own address — a guest can reach the wallet's key".into()
    } else if !same_across_chains {
        "the trading key differs between base and hyperevm — an EVM key must be one key".into()
    } else {
        "the empty label is `default`; three labels are three addresses, none the wallet's own; a label is one key on every EVM chain".into()
    };
    out
}

/// The wallet's own EVM address, from `get-address` — the key no label reaches.
fn own_address(chain: &str) -> Result<String, String> {
    let (json, err) = wallet::get_address(chain);
    if !err.is_empty() {
        return Err(err);
    }
    let v: serde_json::Value =
        serde_json::from_str(&json).map_err(|e| format!("address answer is not JSON: {e}"))?;
    v.get("address")
        .and_then(|a| a.as_str())
        .map(|a| a.to_ascii_lowercase())
        .ok_or_else(|| "address answer carries no `address`".to_string())
}

fn sign(op: &str) -> Output {
    const MESSAGE: &str = "subkey-probe";
    const LABEL: &str = "trading";

    let mut out = Output {
        operation: op.into(),
        ..Default::default()
    };
    let expected = match address_of(CHAIN, LABEL) {
        Ok(a) => a,
        Err(e) => {
            out.detail = format!("get-sub-key-address failed: {e}");
            return out;
        }
    };
    out.signer_expected = Some(expected.clone());

    // What a venue does with the signature: recover the signer from the
    // EIP-191 digest and compare with the address the wallet reported.
    let recovered = match sign_and_recover(MESSAGE, LABEL) {
        Ok((sig, r)) => {
            out.signature = Some(sig);
            r
        }
        Err(e) => {
            out.detail = e;
            return out;
        }
    };
    out.signer_recovered = Some(recovered.clone());
    if recovered != expected {
        out.detail = "the signature recovers to a DIFFERENT address than the trading sub-key".into();
        return out;
    }

    // The empty label signs too — as the `default` sub-key, never as the
    // wallet's own key (`get-address`), the one address a guest must not be
    // able to sign for.
    let (default_addr, own) = match (address_of(CHAIN, ""), own_address(CHAIN)) {
        (Ok(d), Ok(o)) => (d, o),
        (Err(e), _) | (_, Err(e)) => {
            out.detail = format!("address lookup failed: {e}");
            return out;
        }
    };
    out.wallet_own = Some(own.clone());
    let empty_signer = match sign_and_recover("subkey-probe:empty-label", "") {
        Ok((_, r)) => r,
        Err(e) => {
            out.detail = format!("the empty label must sign as `default`, but: {e}");
            return out;
        }
    };
    out.empty_label_signer = Some(empty_signer.clone());
    out.ok = empty_signer == default_addr && empty_signer != own;
    out.detail = if out.ok {
        "the trading signature recovers to the trading sub-key, and the empty label signs as `default`, not as the wallet's own key".into()
    } else if empty_signer == own {
        "the empty label signed with the WALLET'S OWN key — a guest can reach it".into()
    } else {
        "the empty label signed as neither `default` nor the wallet's own key".into()
    };
    out
}

/// `evm-sign-message(utf8)` under `label`, and the address recovered from the
/// signature over the EIP-191 digest.
fn sign_and_recover(message: &str, label: &str) -> Result<(String, String), String> {
    let (json, err) = wallet::evm_sign_message(CHAIN, message, "utf8", label);
    if !err.is_empty() {
        return Err(format!("evm-sign-message failed: {err}"));
    }
    let sig_hex = serde_json::from_str::<serde_json::Value>(&json)
        .ok()
        .and_then(|v| v.get("signature").and_then(|s| s.as_str()).map(str::to_string))
        .ok_or_else(|| "sign answer carries no `signature`".to_string())?;
    let recovered = recover_personal_sign(message.as_bytes(), &sig_hex).map_err(|e| format!("recovery failed: {e}"))?;
    Ok((sig_hex, recovered))
}

fn foreign_label(op: &str) -> Output {
    let mut out = Output {
        operation: op.into(),
        ..Default::default()
    };
    // A label that could spell another connector's path, or a deeper segment,
    // or a non-EVM chain with a sub-key: every one must come back refused.
    let attempts = [
        (CHAIN, "hl.trading"),
        (CHAIN, "near-email:send"),
        (CHAIN, "Trading"),
        (CHAIN, "a b"),
        (CHAIN, "connector.mercury.trading"),
        ("near", "trading"),
        ("solana", "trading"),
    ];
    for (chain, label) in attempts {
        let (json, err) = wallet::get_sub_key_address(chain, label);
        let entry = Refusal {
            label: label.into(),
            chain: chain.into(),
            error: if err.is_empty() { json } else { err.clone() },
        };
        if err.is_empty() {
            out.accepted_but_should_not_be.push(entry);
        } else {
            out.refusals.push(entry);
        }
    }
    out.ok = out.accepted_but_should_not_be.is_empty();
    out.detail = if out.ok {
        "every foreign, malformed or non-EVM sub-key request was refused".into()
    } else {
        "a sub-key request that must be refused was ACCEPTED".into()
    };
    out
}

/// Recover the `0x` address that produced `sig_hex` over the EIP-191 digest of
/// `message` (`keccak256("\x19Ethereum Signed Message:\n{len}{message}")`).
fn recover_personal_sign(message: &[u8], sig_hex: &str) -> Result<String, String> {
    use k256::ecdsa::{RecoveryId, Signature, VerifyingKey};

    let sig = hex::decode(sig_hex.trim_start_matches("0x")).map_err(|e| format!("signature hex: {e}"))?;
    if sig.len() != 65 {
        return Err(format!("signature is {} bytes, expected 65", sig.len()));
    }
    let v = sig[64];
    let recid = RecoveryId::try_from(v.checked_sub(27).ok_or("v below 27")?)
        .map_err(|e| format!("recovery id: {e}"))?;
    let signature = Signature::from_slice(&sig[..64]).map_err(|e| format!("signature: {e}"))?;

    let mut preimage = format!("\x19Ethereum Signed Message:\n{}", message.len()).into_bytes();
    preimage.extend_from_slice(message);
    let digest = keccak256(&preimage);

    let key = VerifyingKey::recover_from_prehash(&digest, &signature, recid)
        .map_err(|e| format!("recover: {e}"))?;
    let uncompressed = key.to_encoded_point(false);
    let hash = keccak256(&uncompressed.as_bytes()[1..]);
    Ok(format!("0x{}", hex::encode(&hash[12..])))
}

fn keccak256(data: &[u8]) -> [u8; 32] {
    use tiny_keccak::{Hasher, Keccak};
    let mut k = Keccak::v256();
    let mut out = [0u8; 32];
    k.update(data);
    k.finalize(&mut out);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use k256::ecdsa::{signature::hazmat::PrehashSigner, SigningKey};

    /// Sign locally the way the keystore does (recoverable, over the EIP-191
    /// digest) and recover: the probe's check must accept exactly the signer.
    #[test]
    fn personal_sign_recovery_round_trips() {
        let key = SigningKey::from_slice(&[0x11u8; 32]).expect("valid scalar");
        let expected = {
            let point = key.verifying_key().to_encoded_point(false);
            let hash = keccak256(&point.as_bytes()[1..]);
            format!("0x{}", hex::encode(&hash[12..]))
        };

        let message = b"subkey-probe";
        let mut preimage = format!("\x19Ethereum Signed Message:\n{}", message.len()).into_bytes();
        preimage.extend_from_slice(message);
        let digest = keccak256(&preimage);
        let (sig, recid): (k256::ecdsa::Signature, k256::ecdsa::RecoveryId) =
            key.sign_prehash(&digest).expect("sign");
        let mut bytes = sig.to_bytes().to_vec();
        bytes.push(27 + recid.to_byte());
        let sig_hex = format!("0x{}", hex::encode(bytes));

        assert_eq!(recover_personal_sign(message, &sig_hex).unwrap(), expected);
        // A different message recovers to some other address, never to ours.
        assert_ne!(recover_personal_sign(b"other", &sig_hex).unwrap(), expected);
    }

    #[test]
    fn malformed_signatures_fail_loudly() {
        assert!(recover_personal_sign(b"x", "0x00").is_err());
        let bad = format!("0x{}", "11".repeat(64)) + "01";
        assert!(recover_personal_sign(b"x", &bad).unwrap_err().contains("v below 27"));
    }
}
