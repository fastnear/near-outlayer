//! Hand-rolled EIP-712 (typed-data) and EIP-191 (`personal_sign`) digest
//! computation — dependency-free, runs inside the TEE.
//!
//! These produce the 32-byte **digest** that the EVM signer signs via
//! [`crate::crypto::Keystore::sign_secp256k1_prehash`]. We compute the
//! digest server-side from the structured request (the full
//! `eth_signTypedData_v4` object for EIP-712, or the raw/utf-8 message
//! for EIP-191) rather than trusting a client-supplied hash, so the
//! signed bytes are exactly what the request describes.
//!
//! Why hand-rolled (not `alloy-dyn-abi`/`ethers`): the keystore runs in
//! an enclave where every dependency widens the attestation/reproducible-
//! build surface. EIP-712 encoding is small and fully specified, so we
//! implement the general algorithm here and pin it against viem-generated
//! reference vectors (`eip712_vectors.json`) covering nested structs,
//! arrays, dynamic `bytes`/`string`, `bool`, `address`, `uintN`/`bytesN`,
//! plus the real structs we sign (Polymarket Order, EIP-3009
//! `TransferWithAuthorization`, EIP-2612 `Permit`).
//!
//! Spec: <https://eips.ethereum.org/EIPS/eip-712>,
//! <https://eips.ethereum.org/EIPS/eip-191>.

use anyhow::{anyhow, bail, Context, Result};
use serde_json::Value;
use sha3::{Digest, Keccak256};
use std::collections::{BTreeMap, BTreeSet};

/// Struct type definitions: type name → ordered `(field_name, field_type)`.
type Types = BTreeMap<String, Vec<(String, String)>>;

fn keccak(bytes: &[u8]) -> [u8; 32] {
    let mut out = [0u8; 32];
    out.copy_from_slice(&Keccak256::digest(bytes));
    out
}

// ====================== EIP-191 personal_sign ===========================

/// EIP-191 `personal_sign` digest:
/// `keccak256("\x19Ethereum Signed Message:\n" + len(msg) + msg)`.
pub fn eip191_digest(message: &[u8]) -> [u8; 32] {
    let prefix = format!("\x19Ethereum Signed Message:\n{}", message.len());
    let mut buf = Vec::with_capacity(prefix.len() + message.len());
    buf.extend_from_slice(prefix.as_bytes());
    buf.extend_from_slice(message);
    keccak(&buf)
}

/// EIP-191 `personal_sign` digest for an EXPLICITLY-encoded message.
///
/// The caller states the encoding — we do NOT sniff the content (which would
/// silently sign a hex-looking text string as bytes):
/// * `hex = false` → sign the UTF-8 bytes of `message` (the default; matches
///   viem `hashMessage(string)` / MetaMask `personal_sign`).
/// * `hex = true`  → `message` is hex (`0x`-prefixed or bare); sign the DECODED
///   bytes (viem `hashMessage({ raw })`). Malformed hex (odd length / non-hex)
///   is rejected, not silently reinterpreted.
pub fn eip191_digest_for(message: &str, hex: bool) -> Result<[u8; 32]> {
    let bytes = if hex {
        hex_bytes(&Value::String(message.to_string()))?
    } else {
        message.as_bytes().to_vec()
    };
    Ok(eip191_digest(&bytes))
}

// ========================= EIP-712 typed data ===========================

/// Compute the EIP-712 signing digest for a full `eth_signTypedData_v4`
/// object: `keccak256(0x1901 ‖ domainSeparator ‖ hashStruct(primary, msg))`.
///
/// `typed_data` must have `domain`, `types` (incl. `EIP712Domain`),
/// `primaryType`, and `message`. The `EIP712Domain` type must enumerate
/// exactly the domain fields present (standard client behavior).
pub fn eip712_digest(typed_data: &Value) -> Result<[u8; 32]> {
    let domain = typed_data.get("domain").context("typed_data.domain missing")?;
    let primary = typed_data
        .get("primaryType")
        .and_then(Value::as_str)
        .context("typed_data.primaryType missing")?;
    let message = typed_data.get("message").context("typed_data.message missing")?;
    let types = parse_types(typed_data.get("types").context("typed_data.types missing")?)?;

    let domain_separator = hash_struct("EIP712Domain", domain, &types)
        .context("computing EIP-712 domain separator")?;
    let message_hash = hash_struct(primary, message, &types)
        .with_context(|| format!("computing EIP-712 hashStruct for '{primary}'"))?;

    let mut buf = Vec::with_capacity(66);
    buf.push(0x19);
    buf.push(0x01);
    buf.extend_from_slice(&domain_separator);
    buf.extend_from_slice(&message_hash);
    Ok(keccak(&buf))
}

fn parse_types(types: &Value) -> Result<Types> {
    let obj = types.as_object().context("types must be an object")?;
    let mut map = Types::new();
    for (name, fields) in obj {
        let arr = fields
            .as_array()
            .with_context(|| format!("type '{name}' must be an array of fields"))?;
        let mut out = Vec::with_capacity(arr.len());
        for f in arr {
            let fname = f.get("name").and_then(Value::as_str).context("field.name")?;
            let ftype = f.get("type").and_then(Value::as_str).context("field.type")?;
            out.push((fname.to_string(), ftype.to_string()));
        }
        map.insert(name.clone(), out);
    }
    Ok(map)
}

/// Strip array suffixes to the base type: `Foo[]`→`Foo`, `uint8[3]`→`uint8`.
fn base_type(t: &str) -> &str {
    match t.find('[') {
        Some(i) => &t[..i],
        None => t,
    }
}

/// Strip exactly the LAST array dimension: `uint[]`→`Some("uint")`,
/// `Foo[][2]`→`Some("Foo[]")`, `address`→`None`.
fn array_inner(t: &str) -> Option<String> {
    if t.ends_with(']') {
        let open = t.rfind('[')?;
        Some(t[..open].to_string())
    } else {
        None
    }
}

/// Transitively collect struct types referenced by `primary` (incl. itself).
fn collect_deps(primary: &str, types: &Types, deps: &mut BTreeSet<String>) {
    if deps.contains(primary) || !types.contains_key(primary) {
        return;
    }
    deps.insert(primary.to_string());
    for (_, ftype) in &types[primary] {
        let base = base_type(ftype);
        if types.contains_key(base) {
            collect_deps(base, types, deps);
        }
    }
}

/// `encodeType`: primary type first, then referenced struct types sorted
/// alphabetically — e.g. `Mail(Person from,...)Person(string name,...)`.
fn encode_type(primary: &str, types: &Types) -> Result<String> {
    let mut deps = BTreeSet::new();
    collect_deps(primary, types, &mut deps);
    if !deps.contains(primary) {
        bail!("unknown struct type '{primary}'");
    }
    deps.remove(primary);

    let mut out = String::new();
    for name in std::iter::once(primary.to_string()).chain(deps) {
        let fields = types.get(&name).ok_or_else(|| anyhow!("unknown type '{name}'"))?;
        out.push_str(&name);
        out.push('(');
        let parts: Vec<String> =
            fields.iter().map(|(n, t)| format!("{t} {n}")).collect();
        out.push_str(&parts.join(","));
        out.push(')');
    }
    Ok(out)
}

fn type_hash(primary: &str, types: &Types) -> Result<[u8; 32]> {
    Ok(keccak(encode_type(primary, types)?.as_bytes()))
}

/// `hashStruct(type, data) = keccak(typeHash(type) ‖ encodeData(type, data))`.
fn hash_struct(type_name: &str, data: &Value, types: &Types) -> Result<[u8; 32]> {
    let th = type_hash(type_name, types)?;
    let fields = types
        .get(type_name)
        .ok_or_else(|| anyhow!("unknown struct type '{type_name}'"))?;
    let obj = data
        .as_object()
        .ok_or_else(|| anyhow!("expected an object for struct '{type_name}'"))?;

    let mut enc = Vec::with_capacity(32 + fields.len() * 32);
    enc.extend_from_slice(&th);
    for (fname, ftype) in fields {
        let value = obj
            .get(fname)
            .ok_or_else(|| anyhow!("missing field '{fname}' in '{type_name}'"))?;
        enc.extend_from_slice(&encode_field(ftype, value, types)?);
    }
    Ok(keccak(&enc))
}

/// `encodeData` for a single field → its 32-byte encoding.
fn encode_field(ftype: &str, value: &Value, types: &Types) -> Result<[u8; 32]> {
    if let Some(inner) = array_inner(ftype) {
        let arr = value
            .as_array()
            .with_context(|| format!("expected an array for type '{ftype}'"))?;
        // Fixed-size array `T[n]` must carry exactly n elements (EIP-712 treats
        // it as an n-arity type; viem rejects a length mismatch).
        if let Some(fixed) = array_fixed_len(ftype)? {
            if arr.len() != fixed {
                bail!("fixed-size array '{ftype}' expects {fixed} elements, got {}", arr.len());
            }
        }
        let mut buf = Vec::with_capacity(arr.len() * 32);
        for el in arr {
            buf.extend_from_slice(&encode_field(&inner, el, types)?);
        }
        return Ok(keccak(&buf));
    }
    if types.contains_key(ftype) {
        return hash_struct(ftype, value, types);
    }
    encode_atomic(ftype, value)
}

fn encode_atomic(ftype: &str, value: &Value) -> Result<[u8; 32]> {
    let mut out = [0u8; 32];
    match ftype {
        "string" => {
            return Ok(keccak(value.as_str().context("expected string")?.as_bytes()))
        }
        "bytes" => return Ok(keccak(&hex_bytes(value)?)),
        "bool" => {
            if value.as_bool().context("expected bool")? {
                out[31] = 1;
            }
            return Ok(out);
        }
        "address" => {
            let b = hex_bytes(value)?;
            if b.len() != 20 {
                bail!("address must be 20 bytes, got {}", b.len());
            }
            out[12..].copy_from_slice(&b);
            return Ok(out);
        }
        _ => {}
    }
    if let Some(bits) = ftype.strip_prefix("uint") {
        let n = parse_int_bits(bits)?;
        let v = parse_uint256_be(value)?;
        check_uint_fits(&v, n)?;
        return Ok(v);
    }
    if let Some(bits) = ftype.strip_prefix("int") {
        let n = parse_int_bits(bits)?;
        let v = parse_int256_be(value)?;
        check_int_fits(&v, n)?;
        return Ok(v);
    }
    if let Some(n) = ftype.strip_prefix("bytes") {
        // Canonical bytesN only: decimal 1..=32, no leading zero (reject "bytes01").
        let canonical = n.bytes().all(|b| b.is_ascii_digit()) && !(n.len() > 1 && n.starts_with('0'));
        if let (true, Ok(nn)) = (canonical, n.parse::<usize>()) {
            if (1..=32).contains(&nn) {
                let b = hex_bytes(value)?;
                if b.len() > nn {
                    bail!("bytes{nn} value is {} bytes (too long)", b.len());
                }
                out[..b.len()].copy_from_slice(&b); // fixed bytes are left-aligned
                return Ok(out);
            }
        }
    }
    bail!("unsupported EIP-712 type: '{ftype}'")
}

/// Decode a `0x`-hex JSON string to bytes. Rejects odd-length hex: for the
/// byte-string types (`bytes`/`bytesN`/`address`) an odd nibble count is
/// unambiguously malformed (viem's `hexToBytes` rejects it too), and silently
/// left-padding it would sign different bytes than the caller wrote. The
/// numeric `uint*` hex path normalizes odd length itself (see `parse_uint256_be`).
fn hex_bytes(value: &Value) -> Result<Vec<u8>> {
    let s = value.as_str().context("expected a 0x-hex string")?;
    let body = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")).unwrap_or(s);
    if body.len() % 2 == 1 {
        bail!("odd-length hex string '{s}'");
    }
    hex::decode(body).context("invalid hex string")
}

/// EIP-712 integer width suffix: empty ⇒ 256, else must be a canonical decimal
/// in {8,16,…,256}. Rejects non-canonical spellings (`008`, `0256`, `+8`) that
/// Rust's lax `parse` would otherwise accept but viem/Solidity reject.
fn parse_int_bits(suffix: &str) -> Result<u32> {
    if suffix.is_empty() {
        return Ok(256);
    }
    if !suffix.bytes().all(|b| b.is_ascii_digit()) || (suffix.len() > 1 && suffix.starts_with('0')) {
        bail!("non-canonical integer width '{suffix}'");
    }
    let n: u32 = suffix.parse().map_err(|_| anyhow!("invalid integer width '{suffix}'"))?;
    if n == 0 || n > 256 || n % 8 != 0 {
        bail!("invalid integer width {n} (must be 8..=256 in steps of 8)");
    }
    Ok(n)
}

/// A 32-byte big-endian unsigned value must fit in `bits`: the high `256-bits`
/// bits (top `(256-bits)/8` bytes) are all zero.
fn check_uint_fits(v: &[u8; 32], bits: u32) -> Result<()> {
    let zero_prefix = 32 - (bits as usize / 8);
    if v[..zero_prefix].iter().any(|&b| b != 0) {
        bail!("value does not fit in uint{bits}");
    }
    Ok(())
}

/// A 32-byte two's-complement value must fit in signed `bits`: every bit at
/// index ≥ `bits` equals the sign bit (bit `bits-1`) — i.e. valid sign-extension.
fn check_int_fits(v: &[u8; 32], bits: u32) -> Result<()> {
    if bits == 256 {
        return Ok(());
    }
    let bit = |i: usize| -> u8 { (v[31 - i / 8] >> (i % 8)) & 1 };
    let sign = bit((bits - 1) as usize);
    for i in (bits as usize)..256 {
        if bit(i) != sign {
            bail!("value does not fit in int{bits}");
        }
    }
    Ok(())
}

/// For a fixed-size array type `T[n]`, return `Some(n)`; for a dynamic `T[]`,
/// `None`. Bails on malformed bracket content (`T[abc]`).
fn array_fixed_len(t: &str) -> Result<Option<usize>> {
    if !t.ends_with(']') {
        return Ok(None);
    }
    let open = t.rfind('[').context("malformed array type")?;
    let inner = &t[open + 1..t.len() - 1];
    if inner.is_empty() {
        return Ok(None);
    }
    if !inner.bytes().all(|b| b.is_ascii_digit()) || (inner.len() > 1 && inner.starts_with('0')) {
        bail!("malformed array size in '{t}'");
    }
    let n: usize = inner.parse().map_err(|_| anyhow!("malformed array size in '{t}'"))?;
    if n == 0 {
        bail!("zero-length fixed array '{t}'");
    }
    Ok(Some(n))
}

/// Parse an unsigned integer (decimal string, JSON number, or `0x`-hex)
/// into a 32-byte big-endian value.
fn parse_uint256_be(value: &Value) -> Result<[u8; 32]> {
    let s = match value {
        Value::String(s) => s.trim().to_string(),
        Value::Number(n) => n.to_string(),
        _ => bail!("uint must be a string or number"),
    };
    let mut acc = [0u8; 32];
    if let Some(hex_body) = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
        if hex_body.is_empty() {
            bail!("empty integer hex value");
        }
        // Numeric hex: odd length is fine (0x123 == 0x0123) — left-pad a nibble.
        let normalized;
        let hex_body = if hex_body.len() % 2 == 1 {
            normalized = format!("0{hex_body}");
            normalized.as_str()
        } else {
            hex_body
        };
        let bytes = hex::decode(hex_body).context("invalid integer hex")?;
        if bytes.len() > 32 {
            bail!("uint256 hex overflow ({} bytes)", bytes.len());
        }
        acc[32 - bytes.len()..].copy_from_slice(&bytes);
        return Ok(acc);
    }
    if s.is_empty() {
        bail!("empty integer value");
    }
    // Decimal: acc = acc * 10 + digit, big-endian, with overflow detection.
    for ch in s.chars() {
        let digit = ch.to_digit(10).ok_or_else(|| anyhow!("invalid decimal digit '{ch}'"))? as u16;
        let mut carry = digit;
        for byte in acc.iter_mut().rev() {
            let v = (*byte as u16) * 10 + carry;
            *byte = (v & 0xff) as u8;
            carry = v >> 8;
        }
        if carry != 0 {
            bail!("uint256 decimal overflow");
        }
    }
    Ok(acc)
}

/// Parse a signed integer (optional leading `-`) into 32-byte two's-complement.
fn parse_int256_be(value: &Value) -> Result<[u8; 32]> {
    let s = match value {
        Value::String(s) => s.trim().to_string(),
        Value::Number(n) => n.to_string(),
        _ => bail!("int must be a string or number"),
    };
    let (negative, magnitude) = match s.strip_prefix('-') {
        Some(rest) => (true, rest.to_string()),
        None => (false, s),
    };
    let mut bytes = parse_uint256_be(&Value::String(magnitude))?;
    if negative {
        for b in bytes.iter_mut() {
            *b = !*b;
        }
        let mut carry = 1u16;
        for b in bytes.iter_mut().rev() {
            let v = (*b as u16) + carry;
            *b = (v & 0xff) as u8;
            carry = v >> 8;
            if carry == 0 {
                break;
            }
        }
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every digest must match the viem `hashTypedData` / `hashMessage`
    /// reference (`eip712_vectors.json`). Covers nested structs, arrays,
    /// dynamic bytes/string, bool, address, uintN, bytes32, the real
    /// Polymarket Order / EIP-3009 / EIP-2612 structs, and EIP-191.
    #[test]
    fn matches_viem_reference_vectors() {
        let raw = include_str!("eip712_vectors.json");
        let cases: Vec<Value> = serde_json::from_str(raw).expect("parse vectors");
        let (mut typed, mut msg) = (0, 0);
        for c in &cases {
            let label = c["label"].as_str().unwrap();
            let want = c["digest"].as_str().unwrap().trim_start_matches("0x").to_lowercase();
            match c["kind"].as_str().unwrap() {
                "typed" => {
                    let got = eip712_digest(&c["typedData"])
                        .unwrap_or_else(|e| panic!("{label}: {e:#}"));
                    assert_eq!(hex::encode(got), want, "EIP-712 digest mismatch: {label}");
                    typed += 1;
                }
                kind @ ("msg_utf8" | "msg_hex") => {
                    let got = eip191_digest_for(c["input"].as_str().unwrap(), kind == "msg_hex")
                        .unwrap();
                    assert_eq!(hex::encode(got), want, "EIP-191 digest mismatch: {label}");
                    msg += 1;
                }
                other => panic!("unknown vector kind: {other}"),
            }
        }
        assert!(typed >= 5 && msg >= 2, "missing vectors: {typed} typed / {msg} msg");
    }

    #[test]
    fn encode_type_matches_eip712_spec_example() {
        // The canonical `encodeType(Mail)` from the EIP-712 spec:
        // primary type first, referenced struct types sorted by name.
        let types = parse_types(&serde_json::json!({
            "Person": [{"name":"name","type":"string"},{"name":"wallet","type":"address"}],
            "Mail": [{"name":"from","type":"Person"},{"name":"to","type":"Person"},{"name":"contents","type":"string"}]
        }))
        .unwrap();
        assert_eq!(
            encode_type("Mail", &types).unwrap(),
            "Mail(Person from,Person to,string contents)Person(string name,address wallet)"
        );
    }

    #[test]
    fn rejects_malformed_typed_data() {
        use serde_json::json;
        // One-field typed-data carrying `ftype = val`; digest must bail on bad input.
        let td = |ftype: &str, val: serde_json::Value| {
            json!({
                "domain": {},
                "types": { "EIP712Domain": [], "T": [ { "name": "f", "type": ftype } ] },
                "primaryType": "T",
                "message": { "f": val }
            })
        };
        let bad: &[(&str, serde_json::Value)] = &[
            ("uint8", json!("256")),              // out of uint8 range
            ("uint8", json!("999")),
            ("uint999", json!("1")),              // invalid width (>256)
            ("uint7", json!("1")),                // invalid width (not /8)
            ("int8", json!("128")),               // > int8 max (127)
            ("int8", json!("-129")),              // < int8 min (-128)
            ("uint256", json!("")),               // empty integer
            ("uint256", json!("0x")),             // empty hex
            ("bytes32", json!("0x123")),          // odd-length hex
            ("bytes", json!("0xabc")),            // odd-length hex
            ("uint256[2]", json!(["1"])),         // fixed-array too short
            ("uint256[2]", json!(["1", "2", "3"])), // fixed-array too long
            ("uint008", json!("1")),              // non-canonical width (leading zero)
            ("uint+8", json!("1")),               // non-canonical width (sign)
            ("bytes01", json!("0x00")),           // non-canonical bytesN
            ("uint256[0]", json!([])),            // zero-length fixed array
            ("uint256[01]", json!(["1"])),        // non-canonical array size
        ];
        for (ft, v) in bad {
            assert!(eip712_digest(&td(ft, v.clone())).is_err(), "must reject {ft} = {v}");
        }
        let ok: &[(&str, serde_json::Value)] = &[
            ("uint8", json!("255")),
            ("int8", json!("-128")),
            ("int8", json!("127")),
            ("uint256[2]", json!(["1", "2"])),
            ("bytes32", json!("0x00")),
        ];
        for (ft, v) in ok {
            assert!(eip712_digest(&td(ft, v.clone())).is_ok(), "must accept {ft} = {v}");
        }
    }
}
