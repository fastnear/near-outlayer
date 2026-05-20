//! Pure detection state machine.
//!
//! The detector is a stateful observer over a stream of [`McpReceipt`].
//! It groups receipts by `(vault_id, derivation_path)` and flags any
//! second occurrence within the dedup window as a race attack.
//!
//! ## Why "block height window" not wall-clock seconds
//!
//! The race-attack threat model is: a malicious customer atomically
//! deploys a vault, then races the legitimate TEE worker to drive the
//! MPC `request_app_private_key` call themselves (using a backup key
//! they sneaked into the deploy tx). `vault-checker` rejects vaults
//! with extra access keys, so the only way for a malicious second
//! call to land is during the ~minute-window between deploy-tx
//! finality and `mark_vault_verified` landing.
//!
//! At ~1 NEAR block per second, a ~600-block dedup window comfortably
//! covers that minute plus reorg / indexer-lag headroom. Using block
//! height (not wall clock) means restarts of the monitor pick up
//! exactly where they left off without time-skew bugs.

use std::collections::HashMap;

use crate::types::{BlockHeight, DerivationPath, McpReceipt, VaultId, Verdict};

/// In-memory state: the most recent receipt for each
/// `(vault_id, derivation_path)` pair currently inside the dedup
/// window. Receipts older than the window are evicted on access.
#[derive(Debug)]
pub struct Detector {
    /// Sliding-window memory keyed by (vault, dpath).
    seen: HashMap<(VaultId, DerivationPath), McpReceipt>,
    /// Receipts older than `latest_block - window_blocks` are
    /// evicted. With `window_blocks = 600` and 1-second blocks that
    /// is a 10-minute window.
    window_blocks: BlockHeight,
    /// Highest block height observed so far, used for window
    /// computation. Receipts arriving out-of-order BELOW this height
    /// are still observed (we do not assume strict monotonicity at
    /// receipt level — only at lake/block level).
    high_water: BlockHeight,
}

impl Detector {
    pub fn new(window_blocks: BlockHeight) -> Self {
        assert!(window_blocks > 0, "window_blocks must be > 0");
        Self {
            seen: HashMap::new(),
            window_blocks,
            high_water: 0,
        }
    }

    /// Number of currently-tracked pairs. Used by the metrics
    /// endpoint and tests; not part of the detection contract.
    pub fn tracked_pairs(&self) -> usize {
        self.seen.len()
    }

    /// Process one receipt. The detector is single-threaded by design —
    /// near-lake delivers receipts in order on a single tokio task.
    pub fn observe(&mut self, receipt: McpReceipt) -> Verdict {
        if receipt.block_height > self.high_water {
            self.high_water = receipt.block_height;
            self.evict_aged_out();
        }

        let key = (receipt.vault_id.clone(), receipt.derivation_path.clone());
        match self.seen.get(&key).cloned() {
            None => {
                self.seen.insert(key, receipt);
                Verdict::FirstSeen
            }
            Some(previous) => {
                // We hit `seen` only AFTER `evict_aged_out` ran (when
                // high_water bumped). So if a previous entry is here,
                // it is still inside the dedup window — race.
                //
                // Out-of-order delivery: lake-framework usually streams
                // in canonical block order, but at-finality replays or
                // shard-level reordering can land a receipt with a
                // LOWER block_height than the current `seen` entry.
                // In that case the labels would be misleading
                // ("current" older than "previous"). Normalise so the
                // tuple always reflects chronological order — the
                // post-mortem reason string then tells the truth.
                let (older, newer) = if receipt.block_height < previous.block_height {
                    (receipt, previous)
                } else {
                    (previous, receipt)
                };
                // Keep the OLDER entry surviving so a third call still
                // cites the original duplicate.
                self.seen.insert(key, older.clone());
                Verdict::RaceDetected {
                    previous: older,
                    current: newer,
                }
            }
        }
    }

    /// Drop entries whose block_height is older than the window.
    /// O(n) in seen.len(); for the targeted scale (a few hundred
    /// active customers) this is fine. If state grows, switch to a
    /// per-bucket time wheel.
    fn evict_aged_out(&mut self) {
        let lower_bound = self.high_water.saturating_sub(self.window_blocks);
        self.seen.retain(|_, r| r.block_height >= lower_bound);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rcpt(vault: &str, dpath: &str, height: BlockHeight, tx: &str) -> McpReceipt {
        McpReceipt {
            vault_id: vault.to_string(),
            derivation_path: dpath.to_string(),
            block_height: height,
            tx_hash: tx.to_string(),
            receipt_id: format!("rcpt-{tx}"),
        }
    }

    #[test]
    fn first_seen_is_clean() {
        let mut d = Detector::new(100);
        assert_eq!(d.observe(rcpt("v.alice.near", "near", 1, "t1")), Verdict::FirstSeen);
        assert_eq!(d.tracked_pairs(), 1);
    }

    #[test]
    fn duplicate_inside_window_flags_race() {
        let mut d = Detector::new(100);
        d.observe(rcpt("v.alice.near", "near", 1, "t1"));
        let v = d.observe(rcpt("v.alice.near", "near", 50, "t2"));
        match v {
            Verdict::RaceDetected { previous, current } => {
                assert_eq!(previous.tx_hash, "t1");
                assert_eq!(current.tx_hash, "t2");
            }
            _ => panic!("expected race detected, got {v:?}"),
        }
    }

    #[test]
    fn different_dpaths_are_independent() {
        let mut d = Detector::new(100);
        assert_eq!(d.observe(rcpt("v.alice.near", "near", 1, "t1")), Verdict::FirstSeen);
        assert_eq!(d.observe(rcpt("v.alice.near", "evm", 2, "t2")), Verdict::FirstSeen);
        assert_eq!(d.observe(rcpt("v.alice.near", "near", 3, "t3")), Verdict::RaceDetected {
            previous: rcpt("v.alice.near", "near", 1, "t1"),
            current: rcpt("v.alice.near", "near", 3, "t3"),
        });
    }

    #[test]
    fn different_vaults_are_independent() {
        let mut d = Detector::new(100);
        assert_eq!(d.observe(rcpt("v.alice.near", "near", 1, "t1")), Verdict::FirstSeen);
        assert_eq!(d.observe(rcpt("v.bob.near", "near", 2, "t2")), Verdict::FirstSeen);
    }

    #[test]
    fn post_window_repeat_surfaces_as_first_seen() {
        // After the window aging-out, the previous entry is gone —
        // a second call from the same vault looks like a fresh
        // observation, NOT a "benign repeat" sentinel. This is the
        // correct semantics: from the monitor's POV a vault re-deriving
        // after the window is indistinguishable from a brand-new one.
        let mut d = Detector::new(10);
        d.observe(rcpt("v.alice.near", "near", 1, "t1"));
        // Bump high_water past the window so t1 is aged out.
        d.observe(rcpt("v.bob.near", "near", 100, "tb"));
        assert_eq!(
            d.observe(rcpt("v.alice.near", "near", 105, "t2")),
            Verdict::FirstSeen,
        );
    }

    #[test]
    fn third_call_still_pairs_with_first() {
        let mut d = Detector::new(100);
        d.observe(rcpt("v.alice.near", "near", 1, "t1"));
        let v = d.observe(rcpt("v.alice.near", "near", 5, "t2"));
        assert!(matches!(v, Verdict::RaceDetected { .. }));
        // A third call inside the window should still cite t1 as
        // previous, not t2 — the original race is the meaningful one.
        let v3 = d.observe(rcpt("v.alice.near", "near", 9, "t3"));
        match v3 {
            Verdict::RaceDetected { previous, current } => {
                assert_eq!(previous.tx_hash, "t1");
                assert_eq!(current.tx_hash, "t3");
            }
            _ => panic!("expected race detected on third call"),
        }
    }

    #[test]
    fn out_of_order_below_high_water_still_observed() {
        let mut d = Detector::new(100);
        d.observe(rcpt("v.alice.near", "near", 50, "t1"));
        // A receipt at block 30 (below high_water 50) still observed
        // — the dedup is keyed on (vault, dpath), not chronology.
        assert_eq!(
            d.observe(rcpt("v.alice.near", "evm", 30, "t2")),
            Verdict::FirstSeen,
        );
    }

    #[test]
    fn eviction_keeps_state_bounded() {
        let mut d = Detector::new(10);
        d.observe(rcpt("v.alice.near", "near", 1, "t1"));
        d.observe(rcpt("v.alice.near", "evm", 2, "t2"));
        d.observe(rcpt("v.bob.near", "near", 3, "t3"));
        assert_eq!(d.tracked_pairs(), 3);
        // Bump high_water past the window; all three pairs age out.
        d.observe(rcpt("v.carol.near", "near", 100, "t4"));
        assert_eq!(d.tracked_pairs(), 1, "older pairs evicted");
    }
}
