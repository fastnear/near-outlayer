//! `outlayer-monitor` — race-attack detection + vault event forwarding.
//!
//! Runs alongside the OutLayer coordinator. Subscribes to FastNEAR's
//! `neardata.xyz` finalized-block feed, two filter passes share the
//! iteration:
//!
//! 1. **MPC receipts** — `request_app_private_key` calls. Two from
//!    the same `(vault, derivation_path)` inside the dedup window
//!    fire `/admin/ban-vault` + alerter (race-attack detection).
//! 2. **Vault contract logs** — `recovery_*`, `vault_banned`,
//!    `exit_window_set`, etc. Forwarded to coordinator's
//!    `/internal/vault-event` after RPC cross-check (defense vs
//!    neardata compromise).
//!
//! Launch posture is alert-only by default (`--auto-ban-disabled`);
//! flip `--auto-ban-enabled` once real-world false-positive rate is
//! acceptable.

mod detector;
mod run;
mod sinks;
mod source;
mod types;

use anyhow::Result;
use clap::Parser;

use crate::run::{run, RunConfig};
use crate::sinks::{
    CoordinatorVaultEventForwarder, KeystoreActionSink, SlackAlerter, StdoutAlerter,
    TelegramAlerter,
};
use crate::source::{LakeSource, LakeSourceConfig};

#[derive(Debug, Parser)]
#[command(
    name = "outlayer-monitor",
    about = "Race-attack detection + vault event forwarding for OutLayer per-customer vaults"
)]
struct Cli {
    /// Network — selects neardata subdomain + default contract ids.
    #[arg(long, env = "OUTLAYER_NETWORK", default_value = "testnet")]
    network: String,

    /// MPC contract id to filter receipts on. Defaults from the
    /// network when unset (mainnet → v1.signer, testnet →
    /// v1.signer-prod.testnet).
    #[arg(long, env = "OUTLAYER_MONITOR_MPC_CONTRACT")]
    mpc_contract: Option<String>,

    /// keystore-DAO contract id — used both as a log-source filter
    /// (`vault_banned`, `vault_unbanned`, `vault_verified` come from
    /// here) and as a target for RPC cross-checks. Defaults from
    /// network: `dao.outlayer.near` (mainnet) / `dao.outlayer.testnet`
    /// (testnet). Set to empty string to disable vault-event
    /// forwarding entirely.
    #[arg(long, env = "OUTLAYER_MONITOR_KEYSTORE_DAO")]
    keystore_dao: Option<String>,

    /// Block height to start indexing from. On a fresh deploy set to
    /// a recent finality height; the operator should persist the
    /// last-processed height in storage and resume on restart.
    #[arg(long, env = "OUTLAYER_MONITOR_START_BLOCK")]
    start_block: u64,

    /// Dedup window in blocks. Two `request_app_private_key` calls
    /// from the same `(vault, derivation_path)` inside this window
    /// trip the detector. Default 600 blocks ≈ 10 minutes at 1 block
    /// per second — covers the deploy → mark_vault_verified gap with
    /// reorg headroom.
    #[arg(long, env = "OUTLAYER_MONITOR_WINDOW_BLOCKS", default_value = "600")]
    window_blocks: u64,

    /// Internal URL of the keystore-worker. Same address the
    /// coordinator uses (KEYSTORE_BASE_URL env on coordinator).
    #[arg(long, env = "OUTLAYER_MONITOR_KEYSTORE_URL")]
    keystore_url: String,

    /// Worker token authorised to call `/admin/ban-vault` and
    /// `/admin/evict-customer`. Same shape as
    /// `KEYSTORE_AUTH_TOKEN` on a worker (worker-side hash must be
    /// in `ALLOWED_WORKER_TOKEN_HASHES`).
    #[arg(long, env = "OUTLAYER_MONITOR_WORKER_TOKEN")]
    worker_token: String,

    /// Launch posture: alert-only by default. Pass
    /// `--auto-ban-enabled` once real-world data confirms the
    /// detector's false-positive rate is acceptable.
    #[arg(long, env = "OUTLAYER_MONITOR_AUTO_BAN", default_value = "false")]
    auto_ban_enabled: bool,

    /// Slack incoming-webhook URL for race-attack alerts. Mutually
    /// exclusive with `--telegram-bot-token` (use whichever channel
    /// your ops pipeline cares about). Without either, the monitor
    /// falls back to stdout JSON.
    #[arg(long, env = "OUTLAYER_MONITOR_SLACK_WEBHOOK")]
    slack_webhook_url: Option<String>,

    /// Telegram bot token for race-attack alerts. Pair with
    /// `--telegram-chat-id`.
    #[arg(long, env = "OUTLAYER_MONITOR_TELEGRAM_BOT")]
    telegram_bot_token: Option<String>,

    /// Telegram chat id (group/channel) where alerts post. Required
    /// if `--telegram-bot-token` is set.
    #[arg(long, env = "OUTLAYER_MONITOR_TELEGRAM_CHAT")]
    telegram_chat_id: Option<String>,

    /// Path to the checkpoint file (last_processed_block). Survives
    /// process restarts so the monitor doesn't replay history. The
    /// file is written atomically (tmp + rename) after every block.
    /// Recommended: a path on local persistent storage. Leave unset
    /// to disable persistence (test setups only).
    #[arg(long, env = "OUTLAYER_MONITOR_CHECKPOINT_PATH")]
    checkpoint_path: Option<std::path::PathBuf>,

    /// Coordinator base URL where vault contract events are
    /// forwarded. POSTs land at `${coordinator}/internal/vault-event`.
    /// Leave unset to disable forwarding (race-attack monitor still
    /// works).
    #[arg(long, env = "OUTLAYER_MONITOR_COORDINATOR_URL")]
    coordinator_url: Option<String>,

    /// Independent NEAR RPC URL used for the cross-check before
    /// forwarding vault events (defense vs neardata compromise).
    /// Defaults from network: `https://rpc.mainnet.fastnear.com`
    /// (mainnet) / `https://rpc.testnet.fastnear.com` (testnet).
    /// Recommend a different infrastructure than the neardata feed
    /// (e.g. self-hosted RPC, official near.org RPC) for stronger
    /// defense.
    #[arg(long, env = "OUTLAYER_MONITOR_RPC_URL")]
    near_rpc_url: Option<String>,
}

fn default_mpc_contract(network: &str) -> &'static str {
    match network {
        "mainnet" => "v1.signer",
        _ => "v1.signer-prod.testnet",
    }
}

fn default_keystore_dao(network: &str) -> &'static str {
    match network {
        "mainnet" => "dao.outlayer.near",
        _ => "dao.outlayer.testnet",
    }
}

fn default_rpc_url(network: &str) -> &'static str {
    match network {
        "mainnet" => "https://rpc.mainnet.fastnear.com",
        _ => "https://rpc.testnet.fastnear.com",
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "outlayer_monitor=info,info".into()),
        )
        .init();

    let cli = Cli::parse();
    let mpc_contract = cli
        .mpc_contract
        .clone()
        .unwrap_or_else(|| default_mpc_contract(&cli.network).to_string());
    let keystore_dao_id = cli
        .keystore_dao
        .clone()
        .unwrap_or_else(|| default_keystore_dao(&cli.network).to_string());
    let near_rpc_url = cli
        .near_rpc_url
        .clone()
        .unwrap_or_else(|| default_rpc_url(&cli.network).to_string());

    tracing::info!(
        network = %cli.network,
        mpc_contract = %mpc_contract,
        keystore_dao = %keystore_dao_id,
        start_block = cli.start_block,
        window_blocks = cli.window_blocks,
        keystore_url = %cli.keystore_url,
        coordinator_url = ?cli.coordinator_url,
        rpc_url = %near_rpc_url,
        auto_ban_enabled = cli.auto_ban_enabled,
        "outlayer-monitor starting"
    );

    if cli.checkpoint_path.is_none() {
        tracing::warn!(
            "--checkpoint-path NOT set; source will replay from --start-block on restart. \
             Set OUTLAYER_MONITOR_CHECKPOINT_PATH for production deploys."
        );
    }
    if cli.coordinator_url.is_none() {
        tracing::warn!(
            "--coordinator-url NOT set; vault contract events will NOT be forwarded to \
             customer webhooks. Set OUTLAYER_MONITOR_COORDINATOR_URL to enable forwarding."
        );
    }
    if cli.slack_webhook_url.is_some() && cli.telegram_bot_token.is_some() {
        tracing::warn!(
            "Both --slack-webhook-url and --telegram-bot-token set; using Slack. \
             Pick one for unambiguous alerting."
        );
    }
    if cli.telegram_bot_token.is_some() && cli.telegram_chat_id.is_none() {
        eprintln!("--telegram-bot-token requires --telegram-chat-id");
        std::process::exit(2);
    }

    #[cfg(not(feature = "lake-source"))]
    {
        let _ = (mpc_contract, keystore_dao_id, near_rpc_url, cli);
        eprintln!(
            "outlayer-monitor: built without `lake-source` feature. \
             Rebuild with `cargo build --release --features lake-source` \
             to enable the FastNEAR neardata adapter. Refusing to start."
        );
        std::process::exit(2);
    }

    #[cfg(feature = "lake-source")]
    {
        let source = LakeSource::new(LakeSourceConfig {
            mpc_contract_id: mpc_contract,
            keystore_dao_id: keystore_dao_id.clone(),
            start_block_height: cli.start_block,
            network: cli.network.clone(),
            checkpoint_path: cli.checkpoint_path.clone(),
        });
        let actions =
            KeystoreActionSink::new(cli.keystore_url, cli.worker_token.clone(), cli.auto_ban_enabled);
        let forwarder = cli.coordinator_url.clone().map(|url| {
            CoordinatorVaultEventForwarder::new(
                url,
                cli.worker_token.clone(),
                near_rpc_url,
                keystore_dao_id,
            )
        });
        let cfg = RunConfig { window_blocks: cli.window_blocks };

        // Pick alerter via priority: Slack > Telegram > Stdout. We
        // monomorphise via match arms so `run` stays generic.
        match (cli.slack_webhook_url, cli.telegram_bot_token, cli.telegram_chat_id) {
            (Some(url), _, _) => {
                run(source, actions, SlackAlerter::new(url), forwarder, cfg).await
            }
            (None, Some(token), Some(chat)) => {
                run(source, actions, TelegramAlerter::new(token, chat), forwarder, cfg).await
            }
            _ => run(source, actions, StdoutAlerter, forwarder, cfg).await,
        }
    }
}
