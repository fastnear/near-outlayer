# outlayer-monitor

Off-chain race-attack detector and vault-event forwarder for OutLayer per-customer vaults.

Runs alongside the OutLayer coordinator. Subscribes to FastNEAR's `neardata.xyz` finalized-block feed and reacts to two classes of on-chain events:

1. **MPC receipts** (`request_app_private_key`) — two calls with the same `(vault_id, derivation_path)` inside a configurable dedup window are the signature of a race-attack against the per-vault master. Trips `/admin/ban-vault` + `/admin/evict-customer` on the keystore-worker (or stays alert-only, see below).
2. **Vault-contract / DAO logs** — `recovery_initiated_*`, `exit_window_set_*`, `vault_banned`, `vault_unbanned`, `vault_tee_key_added`. Forwarded to coordinator's `/internal/vault-event` for fan-out to customer webhooks, **after** an independent NEAR RPC cross-check (defense vs neardata compromise).

## Why this exists

Per-vault masters are derived deterministically from `(MPC default_master, predecessor=vault_id, derivation_path)`. The `derivation_path` is `HMAC(default_master, "vault-master:{vault}")`, which is unguessable by the customer until the worker submits the first request — at that moment the path goes on-chain in plaintext. From then on it's public. The mitigation is **first-write-wins**: any second `request_app_private_key` from the same vault with the same path inside the dedup window is treated as evidence of compromise, and the vault is banned before the attacker can use the derived master.

The vault-event forwarder is independent: it gives customers a Slack/webhook-style stream of state transitions without forcing them to run their own indexer.

## Architecture

```
                         neardata.xyz (FastNEAR)
                                  │
                                  ▼
                          src/source.rs  (lake-source feature)
                                  │
                                  ▼
                          src/run.rs     (per-block dispatch)
                          /             \
              StreamEvent::Mcp     StreamEvent::Vault
                    │                       │
              src/detector.rs           (RPC cross-check)
              (dedup window)                │
                    │                       ▼
                    │             CoordinatorVaultEventForwarder
                    ▼                       │
              KeystoreActionSink  ──▶  POST coordinator
              (ban-vault +              /internal/vault-event
              evict-customer)
                    │
                    ▼
              SlackAlerter | TelegramAlerter | StdoutAlerter
```

## Source layout

| File | Role |
|------|------|
| `src/main.rs`     | CLI parsing, network-default resolution, sink wiring |
| `src/run.rs`      | Per-block iteration loop; dispatches Mcp vs Vault events |
| `src/source.rs`   | Neardata HTTP adapter; checkpoint persistence |
| `src/detector.rs` | Sliding-window dedup logic; eviction policy |
| `src/sinks.rs`    | `KeystoreActionSink`, alerters (Slack/Telegram/Stdout), `CoordinatorVaultEventForwarder` |
| `src/types.rs`    | Plain-data types (`McpReceipt`, `VaultEventReceipt`, `Verdict`) |

## Build

```bash
# Production: must enable lake-source so the binary actually subscribes
cargo build --release --features lake-source

# Refusing to start without the feature is intentional — it prevents
# silent "no events ever fired" deploys.
```

## Configuration

All flags can be set via environment variables (see `OUTLAYER_MONITOR_*` in `src/main.rs`). Most have sensible defaults derived from `--network` (`mainnet` / `testnet`).

| Flag | Env | Notes |
|------|-----|-------|
| `--network` | `OUTLAYER_NETWORK` | `mainnet` or `testnet` (default) |
| `--start-block` | `OUTLAYER_MONITOR_START_BLOCK` | **Required.** Set to a recent finalized height on first deploy |
| `--checkpoint-path` | `OUTLAYER_MONITOR_CHECKPOINT_PATH` | File for atomic last-block persistence; **set this for production** |
| `--window-blocks` | `OUTLAYER_MONITOR_WINDOW_BLOCKS` | Dedup window. Default 600 (≈10 min) |
| `--keystore-url` | `OUTLAYER_MONITOR_KEYSTORE_URL` | Internal URL of the keystore-worker |
| `--worker-token` | `OUTLAYER_MONITOR_WORKER_TOKEN` | Bearer token authorised on `/admin/ban-vault` and `/admin/evict-customer` |
| `--auto-ban-enabled` | `OUTLAYER_MONITOR_AUTO_BAN` | Default `false`; flip on once false-positive rate is acceptable |
| `--coordinator-url` | `OUTLAYER_MONITOR_COORDINATOR_URL` | Optional; enables vault-event forwarding when set |
| `--near-rpc-url` | `OUTLAYER_MONITOR_RPC_URL` | Independent RPC for cross-check; **recommend a different infrastructure provider than neardata** |
| `--slack-webhook-url` | `OUTLAYER_MONITOR_SLACK_WEBHOOK` | Optional alerter |
| `--telegram-bot-token` + `--telegram-chat-id` | `OUTLAYER_MONITOR_TELEGRAM_BOT/CHAT` | Optional alerter |

Alerter priority: Slack > Telegram > Stdout.

## Operating posture

Default is alert-only (`--auto-ban-disabled`):

- The detector still observes and emits structured JSON / Slack / Telegram alerts for every duplicate `(vault_id, derivation_path)` it sees.
- `KeystoreActionSink` does **not** call the keystore — operators read alerts and ban manually if needed.
- Use this mode to confirm the false-positive rate is acceptable on your network and traffic profile.

Once confirmed, flip to `--auto-ban-enabled`:

- Each detected duplicate auto-fires `/admin/ban-vault` + `/admin/evict-customer`.
- Alerts continue to fire so on-call still has visibility.

## Run

```bash
OUTLAYER_MONITOR_START_BLOCK=190000000 \
OUTLAYER_MONITOR_CHECKPOINT_PATH=/var/lib/outlayer-monitor/checkpoint \
OUTLAYER_MONITOR_KEYSTORE_URL=https://keystore-abc123.phala.cloud \
OUTLAYER_MONITOR_WORKER_TOKEN=$WORKER_TOKEN \
OUTLAYER_MONITOR_COORDINATOR_URL=https://api.outlayer.fastnear.com \
OUTLAYER_MONITOR_SLACK_WEBHOOK=$SLACK_URL \
./outlayer-monitor --network mainnet
```

Logs go through `tracing` with `RUST_LOG`-style filtering (`outlayer_monitor=info,info` by default).

## Defense-in-depth note

The vault-event forwarder performs an independent NEAR RPC view-call against `--near-rpc-url` BEFORE forwarding any event. This protects customer webhooks from a compromised neardata feed pushing fake `vault_banned` / `vault_unlocked` notifications. The cross-check costs ~1 RPC per forwarded event; vault state transitions are rare (<1/hour at scale), so the cost is acceptable. Recommended: set `--near-rpc-url` to a different infrastructure provider than the neardata feed for stronger isolation.
