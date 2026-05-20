//! Plain-data types shared by the detector, sinks, and event sources.
//!
//! Kept dependency-free (no `near-lake-framework`, no `near_primitives`)
//! so the detector can be unit-tested without an indexer in the loop.

use serde::{Deserialize, Serialize};

/// Account id of the *predecessor* of an MPC `request_app_private_key`
/// receipt. For an attested vault this is the vault account; for any
/// other origin we ignore the receipt (out of scope).
pub type VaultId = String;

/// `derivation_path` argument extracted from the
/// `request_app_private_key` call. The race-attack detection key is
/// `(vault_id, derivation_path)` — two calls with the same pair within
/// the dedup window are the smoking gun.
pub type DerivationPath = String;

/// Block height — used as the dedup window's time axis. We don't
/// need wall-clock time; near-lake delivers receipts in
/// canonical block order so monotonicity is sufficient for ordering.
pub type BlockHeight = u64;

/// One filtered MPC call observed in a finalized block. Built from a
/// `near-lake-framework` `ReceiptEnumView` after the indexer-side
/// filter passes (`receiver_id == mpc_contract_id`,
/// `method_name == "request_app_private_key"`).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct McpReceipt {
    pub vault_id: VaultId,
    pub derivation_path: DerivationPath,
    pub block_height: BlockHeight,
    pub tx_hash: String,
    /// The full receipt id, useful for linking to an explorer.
    pub receipt_id: String,
}

/// A vault contract log line observed in a finalized block, plus
/// enough context to forward to `coordinator /internal/vault-event`.
/// Emitted by the same lake-source pipeline as [`McpReceipt`] but
/// carries different payload — separate enum variant in
/// [`StreamEvent`] so the run loop can dispatch to the right sink.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct VaultEventReceipt {
    /// Account that emitted the log — vault account for vault-side
    /// events (`recovery_*`, `exit_window_set`), keystore-DAO
    /// account for DAO-side events (`vault_banned`, `vault_unbanned`).
    pub emitting_account: VaultId,
    /// Vault id the event refers to. For vault-emitted events this
    /// is the same as `emitting_account`; for DAO-emitted
    /// `vault_banned <vault_id>` it's the parsed argument.
    pub vault_id: VaultId,
    /// Stable event id used by coordinator's whitelist and the
    /// customer's webhook payload (e.g. `recovery_initiated_unilateral`).
    pub event_type: String,
    /// Original log line for forwarding context (capped to 256 bytes
    /// — same cap the keystore-dao enforces on `ban_vault`).
    pub raw_log: String,
    pub block_height: BlockHeight,
    pub tx_hash: String,
    pub receipt_id: String,
}

/// Anything the lake source can yield. Keeps the per-call shape
/// `Result<Option<StreamEvent>>` simple — caller dispatches in one
/// `match`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StreamEvent {
    Mcp(McpReceipt),
    Vault(VaultEventReceipt),
}

/// Detection verdict returned by [`crate::detector::Detector::observe`].
///
/// Note: there is no separate "benign repeat after window expiry"
/// variant. The detector's eviction policy runs synchronously on each
/// `observe` so any out-of-window entry is GONE by the time we look up
/// the pair, and a post-window second call surfaces as `FirstSeen` —
/// which is the correct semantics (a vault re-deriving after a long
/// gap is, from the monitor's perspective, indistinguishable from a
/// new vault).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verdict {
    /// First time seeing this `(vault_id, derivation_path)` pair, or
    /// the previous occurrence has aged out of the dedup window.
    FirstSeen,
    /// Repeat call inside the dedup window — race attack.
    RaceDetected {
        previous: McpReceipt,
        current: McpReceipt,
    },
}

/// Reason string shipped to the keystore-worker's `/admin/ban-vault`
/// and `/admin/evict-customer` endpoints. We keep this short and
/// machine-greppable; a richer post-mortem is logged separately.
///
/// **Length-bounded** at 256 bytes (the `keystore-dao.ban_vault`
/// contract's hard cap). Without truncation, an attacker who controls
/// the on-chain `derivation_path` could pick a 200+ byte string,
/// inflating the reason past the limit and making the ban tx revert
/// — i.e. the attacker could block their own ban. We truncate the
/// derivation_path slice to a fixed budget so the prefix and tx
/// hashes always fit.
pub fn ban_reason(previous: &McpReceipt, current: &McpReceipt) -> String {
    // Budget: total <= 256 bytes.
    //   prefix "duplicate_mpc_call previous= current= dpath="  ≈ 47
    //   2× tx hash, ed25519 → base58, ≤44 each              ≈ 88
    //   safety margin                                       ≈ 5
    //   ⇒ derivation_path budget                            = 256 - 140 = 116
    const DPATH_MAX: usize = 116;
    let dpath = if current.derivation_path.len() <= DPATH_MAX {
        current.derivation_path.as_str()
    } else {
        // char_indices to avoid splitting a UTF-8 codepoint mid-byte.
        // derivation_path is conventionally ASCII but defensive.
        let cut = current
            .derivation_path
            .char_indices()
            .take_while(|(i, _)| *i <= DPATH_MAX)
            .last()
            .map(|(i, _)| i)
            .unwrap_or(0);
        &current.derivation_path[..cut]
    };
    let s = format!(
        "duplicate_mpc_call previous={} current={} dpath={}",
        previous.tx_hash, current.tx_hash, dpath,
    );
    // Belt and suspenders: cap on the formatted result too in case
    // any of the upstream fields turn out larger than expected.
    if s.len() > 256 {
        let cut = s
            .char_indices()
            .take_while(|(i, _)| *i < 256)
            .last()
            .map(|(i, _)| i)
            .unwrap_or(0);
        s[..cut].to_string()
    } else {
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rcpt(vault: &str, dpath: &str, h: u64, t: &str) -> McpReceipt {
        McpReceipt {
            vault_id: vault.into(),
            derivation_path: dpath.into(),
            block_height: h,
            tx_hash: t.into(),
            receipt_id: format!("r-{t}"),
        }
    }

    #[test]
    fn ban_reason_within_256_for_normal_input() {
        let r = ban_reason(
            &rcpt("v.alice.near", "near", 1, "5d6P9hZ8wJYcFaQRn3eXdM7yKBvLwTjPpUcGzKxV4uWQ"),
            &rcpt("v.alice.near", "near", 5, "9hPJ4mN2rQzY8tKvBaXcDfEgHjLnMpRwSuTuVwXyZ12ab"),
        );
        assert!(r.len() <= 256, "got {} bytes", r.len());
        assert!(r.contains("duplicate_mpc_call"));
    }

    #[test]
    fn ban_reason_truncates_long_derivation_path() {
        let long = "a".repeat(500);
        let r = ban_reason(
            &rcpt("v.alice.near", &long, 1, "tx1"),
            &rcpt("v.alice.near", &long, 5, "tx2"),
        );
        assert!(r.len() <= 256, "got {} bytes", r.len());
    }
}
