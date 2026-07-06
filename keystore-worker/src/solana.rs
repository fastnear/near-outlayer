//! Solana signing helpers — a reject-only transaction-message guard,
//! dependency-free, runs inside the TEE.
//!
//! The Solana signing model mirrors EVM: the client builds and broadcasts,
//! the keystore only signs the supplied bytes with the wallet's ed25519 key
//! (Solana signs the raw serialized message — there is no digest step). The
//! policy gate distinguishes message signing (base `solana_sign` capability)
//! from transaction signing (`solana_sign.raw_tx` sub-flag, default-OFF).
//!
//! Unlike EVM, Solana has no EIP-191-style prefix separating messages from
//! transactions: a "message" whose bytes happen to be a valid serialized
//! transaction message would, once signed, be broadcastable — silently
//! bypassing the `raw_tx` sub-flag. Wallets (Phantom, Solflare) close this
//! by refusing to `signMessage` bytes that parse as a transaction message;
//! [`parses_as_transaction_message`] implements the same reject-only check.
//! It never interprets the payload beyond "could a node accept this as a
//! transaction message" — blind signing stays blind.
//!
//! Why hand-rolled (not `solana-sdk`): same reasoning as `eip712.rs` — the
//! keystore runs in an enclave where every dependency widens the
//! attestation/reproducible-build surface. The wire format is small and
//! fully specified; it is pinned against `@solana/web3.js`-generated
//! reference vectors (`solana_vectors.json`).
//!
//! Wire format (legacy):
//! `[header: 3×u8][account keys: compact-u16 count, count×32B]
//!  [recent blockhash: 32B][instructions: compact-u16 count, each:
//!  u8 program_id_index, compact-u16 accounts, compact-u16 data]`
//! Versioned (v0) prepends a prefix byte `0x80 | version` and appends
//! `[address table lookups: compact-u16 count, each: 32B account,
//!  compact-u16 writable indexes, compact-u16 readonly indexes]`.
//!
//! Spec: <https://solana.com/docs/core/transactions>.

/// Maximum serialized transaction size a Solana node accepts
/// (`PACKET_DATA_SIZE` = 1280 − 40 (IPv6 header) − 8 (fragment header)).
/// A *message* is strictly smaller than the transaction that carries it,
/// so anything above this can never be broadcast and is rejected outright
/// by the sign-transaction endpoint.
pub const MAX_TX_MESSAGE_LEN: usize = 1232;

/// Size cap for the sign-message endpoint. Arbitrary but generous —
/// off-chain messages (SIWS, ToS acknowledgements) are tiny; the cap only
/// bounds enclave work on garbage input.
pub const MAX_MESSAGE_LEN: usize = 64 * 1024;

/// Returns `true` when `bytes` fully parse as a valid Solana transaction
/// message (legacy or versioned) — i.e. when an ed25519 signature over them
/// could be attached to a broadcastable transaction.
///
/// Used as a REJECT check by the message-signing path only; it never gates
/// the transaction-signing path and never interprets instruction contents.
///
/// Deliberate asymmetry in strictness:
/// * A missed transaction (false negative) is a `raw_tx`-bypass, so the
///   parser is *lenient about encoding* (accepts any ≤3-byte compact-u16,
///   any version byte `0x80..=0xFF` with a v0-shaped body — a hypothetical
///   future version that keeps the v0 layout is still caught).
/// * A rejected genuine message (false positive) only costs UX, and the
///   header/index sanity checks below mirror what a node's `sanitize()`
///   enforces — bytes failing them can never execute, so it is safe to
///   sign them as a message.
///
/// The signature covers ALL supplied bytes, so trailing garbage after a
/// parseable prefix makes the signature useless for the embedded
/// transaction — hence the exact-consumption requirement.
pub fn parses_as_transaction_message(bytes: &[u8]) -> bool {
    if bytes.is_empty() || bytes.len() > MAX_TX_MESSAGE_LEN {
        return false;
    }
    let mut c = Cursor { buf: bytes, pos: 0 };
    // Versioned message: prefix byte with the MSB set (legacy headers can't
    // collide — num_required_signatures is bounded far below 128).
    //
    // ⚠️ FUTURE MESSAGE VERSIONS: any version byte (0x80..=0xFF) is parsed
    // against the v0 LAYOUT below. Today that is airtight — nodes reject
    // every version except 0 (`UnsupportedVersion`), so a payload this parser
    // misses cannot be broadcast. THE DAY SOLANA SHIPS A v1 MESSAGE FORMAT
    // WITH A DIFFERENT LAYOUT, THIS GUARD MUST LEARN IT *BEFORE* MAINNET
    // NODES ACCEPT IT — otherwise a v1 transaction could slip through
    // sign-message and bypass the `solana_sign.raw_tx` gate.
    let versioned = bytes[0] & 0x80 != 0;
    if versioned {
        c.pos = 1;
    }
    parse_message_body(&mut c, versioned).is_some() && c.pos == bytes.len()
}

struct Cursor<'a> {
    buf: &'a [u8],
    pos: usize,
}

impl<'a> Cursor<'a> {
    fn u8(&mut self) -> Option<u8> {
        let b = *self.buf.get(self.pos)?;
        self.pos += 1;
        Some(b)
    }

    fn skip(&mut self, n: usize) -> Option<()> {
        let end = self.pos.checked_add(n)?;
        if end > self.buf.len() {
            return None;
        }
        self.pos = end;
        Some(())
    }

    /// Solana `short_vec` compact-u16: little-endian base-128 varint,
    /// at most 3 bytes, value ≤ u16::MAX. Lenient: non-canonical
    /// encodings (e.g. `0x80 0x00` for 0) are accepted — see the
    /// false-negative rationale on [`parses_as_transaction_message`].
    fn compact_u16(&mut self) -> Option<usize> {
        let mut value: usize = 0;
        for i in 0..3 {
            let b = self.u8()?;
            value |= ((b & 0x7f) as usize) << (7 * i);
            if b & 0x80 == 0 {
                return if value <= u16::MAX as usize { Some(value) } else { None };
            }
        }
        None // >3 bytes: not a valid short_vec length
    }
}

/// Parse a message body (header onward; the version prefix, if any, is
/// already consumed). Returns `Some(())` only for a shape a node could
/// accept per `SanitizedMessage` rules.
fn parse_message_body(c: &mut Cursor, versioned: bool) -> Option<()> {
    let num_required = c.u8()? as usize;
    let ro_signed = c.u8()? as usize;
    let ro_unsigned = c.u8()? as usize;

    let num_static_keys = c.compact_u16()?;
    // sanitize() invariants: a fee-payer signer must exist, be writable,
    // and all key ranges must fit inside the static key list.
    if num_required == 0
        || num_required > num_static_keys
        || ro_signed >= num_required
        || ro_unsigned > num_static_keys - num_required
    {
        return None;
    }
    c.skip(num_static_keys * 32)?; // static account keys
    c.skip(32)?; // recent blockhash

    let num_instructions = c.compact_u16()?;
    let mut max_index_used: usize = 0;
    for _ in 0..num_instructions {
        let program_id_index = c.u8()? as usize;
        max_index_used = max_index_used.max(program_id_index);
        let num_accounts = c.compact_u16()?;
        for _ in 0..num_accounts {
            max_index_used = max_index_used.max(c.u8()? as usize);
        }
        let data_len = c.compact_u16()?;
        c.skip(data_len)?;
    }

    let mut total_keys = num_static_keys;
    if versioned {
        let num_lookups = c.compact_u16()?;
        for _ in 0..num_lookups {
            c.skip(32)?; // lookup table account
            let writable = c.compact_u16()?;
            c.skip(writable)?;
            let readonly = c.compact_u16()?;
            c.skip(readonly)?;
            total_keys += writable + readonly;
        }
    }

    // An instruction index pointing past every available key can never
    // execute → not a broadcastable transaction.
    if num_instructions > 0 && max_index_used >= total_keys {
        return None;
    }
    Some(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    /// Guard + signing vectors generated by `@solana/web3.js`
    /// (`scripts/gen_solana_vectors.mjs`). Every `tx_message` case must be
    /// caught by the guard; every `not_tx` case must pass it.
    #[test]
    fn matches_web3js_reference_vectors() {
        let raw = include_str!("solana_vectors.json");
        let vectors: Value = serde_json::from_str(raw).expect("parse vectors");
        let (mut tx, mut not_tx) = (0, 0);
        for c in vectors["guard_cases"].as_array().expect("guard_cases") {
            let label = c["label"].as_str().unwrap();
            let bytes = decode_b64(c["bytes_base64"].as_str().unwrap());
            let want_tx = c["is_tx_message"].as_bool().unwrap();
            assert_eq!(
                parses_as_transaction_message(&bytes),
                want_tx,
                "guard mismatch: {label}"
            );
            if want_tx {
                tx += 1;
            } else {
                not_tx += 1;
            }
        }
        assert!(tx >= 3 && not_tx >= 4, "missing vectors: {tx} tx / {not_tx} not-tx");
    }

    /// Byte-exact cross-signing: the keystore's ed25519 signature over each
    /// vector payload must be IDENTICAL to what `@solana/web3.js` /
    /// `nacl.sign.detached` produced for the same derived key (ed25519 is
    /// deterministic), the derived address must match, and splicing our
    /// signature into the wire format must reproduce the web3.js-signed
    /// transaction byte-for-byte — i.e. our output is broadcastable as-is.
    ///
    /// The vector master secret and keys are PUBLIC test fixtures that hold
    /// no funds and never will.
    #[test]
    fn signatures_match_solana_tooling_byte_for_byte() {
        let raw = include_str!("solana_vectors.json");
        let vectors: Value = serde_json::from_str(raw).expect("parse vectors");
        let signing = &vectors["signing"];

        let ks = crate::crypto::Keystore::from_master_secret_hex(
            signing["test_master_hex"].as_str().unwrap(),
        )
        .unwrap();
        let seed = signing["seed"].as_str().unwrap();
        // The vector seed must be exactly what the API derives for this wallet.
        assert_eq!(
            seed,
            crate::api::wallet_seed(signing["wallet_id"].as_str().unwrap(), "solana")
        );

        let (_, verifying_key) = ks.derive_keypair(None, seed).unwrap();
        assert_eq!(
            bs58::encode(verifying_key.as_bytes()).into_string(),
            signing["address_base58"].as_str().unwrap(),
            "derived Solana address diverged from web3.js"
        );

        for c in signing["cases"].as_array().unwrap() {
            let label = c["label"].as_str().unwrap();
            let payload = decode_b64(c["payload_base64"].as_str().unwrap());

            let sig = ks.sign(None, seed, &payload).unwrap();
            assert_eq!(
                bs58::encode(sig.to_bytes()).into_string(),
                c["signature_base58"].as_str().unwrap(),
                "signature mismatch vs solana tooling: {label}"
            );

            // For transactions: single-signer wire format is
            // `compact-u16 sig count (1) ‖ 64-byte sig ‖ message bytes`.
            // Assembling it with OUR signature must reproduce the
            // web3.js-signed transaction exactly.
            if let Some(signed_tx) = c["signed_tx_base64"].as_str() {
                let mut wire = Vec::with_capacity(1 + 64 + payload.len());
                wire.push(1);
                wire.extend_from_slice(&sig.to_bytes());
                wire.extend_from_slice(&payload);
                assert_eq!(
                    wire,
                    decode_b64(signed_tx),
                    "assembled signed tx mismatch: {label}"
                );
            }

            // The guard must agree with the vector's kind: tx payloads are
            // caught, message payloads pass.
            assert_eq!(
                parses_as_transaction_message(&payload),
                c["kind"] == "transaction",
                "guard disagrees with vector kind: {label}"
            );
        }
    }

    /// The minimal hand-built legacy tx message used by
    /// `tests/wallet_solana_sign_e2e.sh` (test 3) must trip the guard — keeps
    /// the e2e script and this parser from silently disagreeing.
    #[test]
    fn guard_catches_minimal_handbuilt_tx() {
        let mut msg = vec![1u8, 0, 1]; // header: 1 required sig, 0 ro-signed, 1 ro-unsigned
        msg.push(2); // 2 static account keys
        msg.extend_from_slice(&[0u8; 32]); // key 0: fee payer
        msg.extend((0u8..32).collect::<Vec<_>>()); // key 1: program
        msg.extend_from_slice(&[7u8; 32]); // recent blockhash
        msg.push(1); // 1 instruction
        msg.push(1); // program_id_index = 1
        msg.extend_from_slice(&[1, 0]); // 1 account index: [0]
        msg.extend_from_slice(&[4, 2, 0, 0, 0]); // 4-byte instruction data
        assert!(parses_as_transaction_message(&msg));
        // ...and stays caught only as an EXACT parse.
        msg.push(0);
        assert!(!parses_as_transaction_message(&msg), "trailing byte → not a tx");
    }

    #[test]
    fn guard_rejects_structural_garbage() {
        // Empty, oversized, truncated, and text payloads are all "not a tx".
        assert!(!parses_as_transaction_message(b""));
        assert!(!parses_as_transaction_message(&[0u8; MAX_TX_MESSAGE_LEN + 1]));
        assert!(!parses_as_transaction_message(b"hello solana"));
        assert!(!parses_as_transaction_message(
            "Приложение просит подписать это сообщение".as_bytes()
        ));
        // Header claims one signer but there are no account keys.
        assert!(!parses_as_transaction_message(&[1, 0, 0, 0]));
        // Solana off-chain message preamble (\xff"solana offchain") must not
        // register as a transaction.
        let mut offchain = vec![0xffu8];
        offchain.extend_from_slice(b"solana offchain");
        offchain.extend_from_slice(&[0u8; 20]);
        assert!(!parses_as_transaction_message(&offchain));
    }

    fn decode_b64(s: &str) -> Vec<u8> {
        use base64::Engine as _;
        base64::engine::general_purpose::STANDARD
            .decode(s)
            .expect("base64 vector")
    }
}
