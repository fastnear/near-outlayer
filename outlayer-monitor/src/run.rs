//! Top-level run loop wiring source → detector + forwarder → sinks.

use std::sync::Arc;

use anyhow::Result;
use tokio::sync::Mutex;

use crate::detector::Detector;
use crate::sinks::{handle_verdict, ActionSink, Alerter, VaultEventForwarder};
use crate::source::ReceiptSource;
use crate::types::StreamEvent;

pub struct RunConfig {
    pub window_blocks: u64,
}

/// Drives the source → (detector | forwarder) → sinks pipeline until
/// the source is exhausted (test fixtures) or the process is killed
/// (production).
///
/// Two parallel concerns share the same lake-source iteration:
///   * MPC receipts feed the [`Detector`] (race-attack path).
///   * Vault contract logs feed the [`VaultEventForwarder`] (webhook
///     path, with RPC cross-check inside the forwarder).
///
/// `forwarder` may be `None` if the operator only wants race-attack
/// detection without webhook forwarding (single-purpose deploy).
pub async fn run<S, A, R, F>(
    source: S,
    actions: A,
    alerter: R,
    forwarder: Option<F>,
    config: RunConfig,
) -> Result<()>
where
    S: ReceiptSource + 'static,
    A: ActionSink + 'static,
    R: Alerter + 'static,
    F: VaultEventForwarder + 'static,
{
    let detector = Arc::new(Mutex::new(Detector::new(config.window_blocks)));
    let source = Arc::new(source);
    let actions = Arc::new(actions);
    let alerter = Arc::new(alerter);
    let forwarder = forwarder.map(Arc::new);

    loop {
        let event = match source.next().await? {
            Some(r) => r,
            None => {
                tracing::info!("receipt source closed; exiting run loop");
                return Ok(());
            }
        };
        match event {
            StreamEvent::Mcp(receipt) => {
                let verdict = {
                    let mut d = detector.lock().await;
                    d.observe(receipt)
                };
                if let Err(e) =
                    handle_verdict(verdict, actions.as_ref(), alerter.as_ref()).await
                {
                    tracing::error!(error = %e, "verdict handler returned error; continuing");
                }
            }
            StreamEvent::Vault(vault_event) => {
                // Mirror the event to the coordinator's webhook
                // dispatcher (best-effort, customer-facing).
                if let Some(ref f) = forwarder {
                    if let Err(e) = f.forward(&vault_event).await {
                        tracing::warn!(
                            vault_id = %vault_event.vault_id,
                            event_type = %vault_event.event_type,
                            error = %e,
                            "vault event forward failed; continuing"
                        );
                    }
                } else {
                    tracing::debug!(
                        vault_id = %vault_event.vault_id,
                        event_type = %vault_event.event_type,
                        "vault event observed but no forwarder configured; dropping"
                    );
                }
                // Sovereignty cutoff: on `recovery_finalized_*`, ask
                // the keystore to drop the cached per-vault master
                // so signing requests start failing within seconds.
                // The contract has already deleted the on-chain TEE
                // key by this point — this is the cache-eviction
                // half of the same fence.
                // Use `starts_with` instead of exact-match so any future
                // contract-side log suffix (e.g. `_v2`, `_dryrun`) still
                // routes to the eviction path. The contract's emitted
                // event names are the authoritative spelling — kept
                // narrow enough that unrelated `recovery_finalized*`
                // strings (none today) still need an explicit branch.
                let trigger = if vault_event
                    .event_type
                    .starts_with("recovery_finalized_unilateral")
                {
                    Some("unilateral")
                } else if vault_event
                    .event_type
                    .starts_with("recovery_finalized_cessation")
                {
                    Some("cessation")
                } else {
                    None
                };
                if let Some(trigger) = trigger {
                    if let Err(e) = actions
                        .evict_on_recovery_finalize(&vault_event.vault_id, trigger)
                        .await
                    {
                        tracing::warn!(
                            vault_id = %vault_event.vault_id,
                            trigger = %trigger,
                            error = %e,
                            "recovery_finalize evict failed; on-chain key-swap is the authoritative cutoff"
                        );
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sinks::{ActionSink, Alerter};
    use crate::source::MockSource;
    use crate::types::{McpReceipt, VaultEventReceipt, Verdict};
    use async_trait::async_trait;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct CountingActions {
        bans: Arc<AtomicUsize>,
        evicts: Arc<AtomicUsize>,
    }
    #[async_trait]
    impl ActionSink for CountingActions {
        async fn ban_and_evict(&self, _: &McpReceipt, _: &McpReceipt) -> Result<()> {
            self.bans.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
        async fn evict_on_recovery_finalize(&self, _: &str, _: &str) -> Result<()> {
            self.evicts.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
    }
    struct CountingAlerts(Arc<AtomicUsize>);
    #[async_trait]
    impl Alerter for CountingAlerts {
        async fn alert(&self, _: &McpReceipt, _: &McpReceipt) -> Result<()> {
            self.0.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
    }
    struct CountingForwarder(Arc<AtomicUsize>);
    #[async_trait]
    impl VaultEventForwarder for CountingForwarder {
        async fn forward(&self, _: &VaultEventReceipt) -> Result<()> {
            self.0.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
    }

    fn rcpt(vault: &str, dpath: &str, h: u64, t: &str) -> McpReceipt {
        McpReceipt {
            vault_id: vault.into(),
            derivation_path: dpath.into(),
            block_height: h,
            tx_hash: t.into(),
            receipt_id: format!("r-{t}"),
        }
    }

    fn vault_evt(vault: &str, event: &str, h: u64) -> VaultEventReceipt {
        VaultEventReceipt {
            emitting_account: vault.into(),
            vault_id: vault.into(),
            event_type: event.into(),
            raw_log: event.into(),
            block_height: h,
            tx_hash: format!("tx-{h}"),
            receipt_id: format!("r-{h}"),
        }
    }

    #[tokio::test]
    async fn pipeline_detects_race_in_sequence() {
        let bans = Arc::new(AtomicUsize::new(0));
        let alerts = Arc::new(AtomicUsize::new(0));
        let source = MockSource::new(vec![
            rcpt("v.alice.near", "near", 1, "t1"),
            rcpt("v.bob.near", "near", 2, "tb"),
            rcpt("v.alice.near", "near", 5, "t2"), // race
            rcpt("v.alice.near", "evm", 7, "t3"),  // first-seen
        ]);
        run(
            source,
            CountingActions { bans: bans.clone(), evicts: Arc::new(AtomicUsize::new(0)) },
            CountingAlerts(alerts.clone()),
            None::<CountingForwarder>,
            RunConfig { window_blocks: 100 },
        )
        .await
        .unwrap();
        assert_eq!(bans.load(Ordering::SeqCst), 1);
        assert_eq!(alerts.load(Ordering::SeqCst), 1);
    }

    /// Mixed stream — MPC receipts go to detector, vault events to
    /// forwarder; counters land on the right sinks.
    #[tokio::test]
    async fn pipeline_dispatches_mcp_and_vault_events_separately() {
        let bans = Arc::new(AtomicUsize::new(0));
        let alerts = Arc::new(AtomicUsize::new(0));
        let forwards = Arc::new(AtomicUsize::new(0));
        let source = MockSource::from_events(vec![
            StreamEvent::Mcp(rcpt("v.alice.near", "near", 1, "t1")),
            StreamEvent::Vault(vault_evt("v.alice.near", "recovery_initiated_unilateral", 2)),
            StreamEvent::Mcp(rcpt("v.alice.near", "near", 3, "t2")), // race
            StreamEvent::Vault(vault_evt("v.alice.near", "recovery_finalized_unilateral", 4)),
        ]);
        run(
            source,
            CountingActions { bans: bans.clone(), evicts: Arc::new(AtomicUsize::new(0)) },
            CountingAlerts(alerts.clone()),
            Some(CountingForwarder(forwards.clone())),
            RunConfig { window_blocks: 100 },
        )
        .await
        .unwrap();
        assert_eq!(bans.load(Ordering::SeqCst), 1, "race should ban once");
        assert_eq!(alerts.load(Ordering::SeqCst), 1, "race should alert once");
        assert_eq!(forwards.load(Ordering::SeqCst), 2, "two vault events forwarded");
    }

    #[test]
    fn verdict_variants_compile() {
        let _v = Verdict::FirstSeen;
    }
}
