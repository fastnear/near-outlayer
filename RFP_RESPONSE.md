# Oracle RFP — Implementation Report

**Submitted by:** FastNEAR Team
**Original Proposal:** [ORACLE_RFP_PROPOSAL.md](ORACLE_RFP_PROPOSAL.md)
**Repository:** https://github.com/zavodil/oracle-ark
**Date:** February 2026

---

## Executive Summary

All **Critical** deliverables from the proposal have been implemented and deployed on testnet:

| # | Deliverable | Status |
|---|-------------|--------|
| 1 | Simple wrapper contract | Deployed as `price-oracle-wrapper.testnet` |
| 2 | Pyth-compatible wrapper contract | Deployed as `price-oracle-pyth.testnet` |
| 3 | Caching oracle contract | Deployed as `price-oracle.testnet` |
| 4 | Integration documentation | 16 documentation files |
| 5 | Testnet deployment & verification | All 3 contracts live |
| 6 | Mainnet deployment & verification | TBD |

Repository: https://github.com/zavodil/oracle-ark

---

## Delivered Components

### 1. Oracle-Ark WASI Module (core engine)

**Source:** [src/](https://github.com/zavodil/oracle-ark/tree/main/src)

The core price-fetching engine runs inside a TEE (Intel TDX via Phala Cloud) as a WASI P2 binary. It fetches prices from external APIs, aggregates them, and returns validated results.

#### Price Sources (9 exchanges + custom)

List of required assets was provided by RHEA team.

| # | Source | API Type | Tokens Supported |
|---|--------|----------|------------------|
| 1 | **CoinGecko** | REST | 12 tokens (NEAR, ETH, BTC, USDT, USDC, WBTC, DAI, AURORA, FRAX, WOO, SOL, ZEC) |
| 2 | **Binance** | REST | 9 tokens (NEAR, USDC, ETH, BTC, WBTC, DAI, WOO, SOL, ZEC) |
| 3 | **Binance US** | REST | 7 tokens (NEAR, USDC, ETH, BTC, SOL, DAI, ZEC) |
| 4 | **Pyth Network** | REST | 12 tokens (all mapped via Pyth price feed IDs) |
| 5 | **Huobi** | REST | 5 tokens (NEAR, BTC, ETH, SOL, ZEC) |
| 6 | **KuCoin** | REST | 7 tokens (NEAR, BTC, ETH, DAI, SOL, ZEC, WOO) |
| 7 | **Gate.io** | REST | 6 tokens (NEAR, BTC, ETH, SOL, ZEC, AURORA) |
| 8 | **Crypto.com** | REST | 4 tokens (NEAR, BTC, ETH, SOL) |
| 9 | **Binance Alpha** | REST | 1 token (RHEA — BSC contract address) |
| 10 | **Custom** | Any HTTP | Unlimited — any URL with JSON path extraction, supports GET/POST |

Source code: [sources/](https://github.com/zavodil/oracle-ark/tree/main/sources) (shared library used by both WASI and scheduler)

#### Supported Tokens (13 tokens configured)

| Token | NEAR Contract | Sources |
|-------|---------------|---------|
| **NEAR** | `wrap.near` | CoinGecko, Binance, Binance US, Pyth, Huobi, KuCoin, Gate, Crypto.com (8) |
| **ETH** | `aurora` | CoinGecko, Binance, Binance US, Pyth, Huobi, KuCoin, Gate, Crypto.com (8) |
| **BTC** | `nbtc.bridge.near` | CoinGecko, Binance, Binance US, Pyth, Huobi, KuCoin, Gate, Crypto.com (8) |
| **USDT** | `usdt.tether-token.near` | CoinGecko, Pyth (2) |
| **USDC** | `17208628f84f5d6ad...` | CoinGecko, Binance, Pyth, Crypto.com, KuCoin (5) |
| **WBTC** | `2260fac5e5...factory.bridge.near` | CoinGecko, Binance, Pyth, Huobi, Crypto.com, KuCoin, Gate (7) |
| **DAI** | `6b175474e8...factory.bridge.near` | CoinGecko, Binance, Binance US, Pyth, Huobi, Gate (6) |
| **AURORA** | `aaaaaa20d9...factory.bridge.near` | CoinGecko, Pyth, Crypto.com, Huobi, KuCoin, Gate (6) |
| **WOO** | `4691937a75...factory.bridge.near` | CoinGecko, Binance, Pyth, Huobi, Crypto.com, KuCoin, Gate (7) |
| **FRAX** | `853d955ace...factory.bridge.near` | CoinGecko, Pyth (2) |
| **SOL** | `22.contract.portalbridge.near` | CoinGecko, Binance, Binance US, Pyth, Huobi, KuCoin, Gate, Crypto.com (8) |
| **ZEC** | `zec.omft.near` | CoinGecko, Binance, Binance US, Pyth, Huobi, KuCoin, Gate (7) |
| **RHEA** | `token.rhealab.near` | Binance Alpha, Pyth (2) |

Configuration: [tokens.json](https://github.com/zavodil/oracle-ark/blob/main/tokens.json)

#### Aggregation Methods

| Method | Description |
|--------|-------------|
| **Median** (default) | Middle value — resistant to outliers, recommended for DeFi |
| **Average** | Arithmetic mean of all source prices |
| **Weighted Average** | Sources weighted by reliability |

#### WASI Commands

| Command | Description |
|---------|-------------|
| `UpdatePrices` | Fetch prices from all sources, store in TEE public storage |
| `GetPrices` | Return cached prices (or fetch fresh if stale) |
| `ForceUpdate` | Always fetch fresh prices (bypass cache) |
| `FetchExternal` | Fetch from a single specific source (no storage) |
| `FetchCustomData` | Fetch any external data via custom URL + JSON path |
| `TestTelegram` | Send a test alert to configured monitoring channel |

#### Security

- **SSRF Protection**: All outgoing HTTP requests validated — blocks localhost, private IPs, Docker DNS, Kubernetes DNS, file:// and unix:// protocols
- **TEE Isolation**: WASI binary runs in Intel TDX sandbox — cannot access host filesystem
- **Price Validation**: Configurable `max_price_deviation_percent` rejects data when sources disagree beyond threshold
- **Minimum Sources**: Configurable `min_sources_num` rejects results with too few responding sources

---

### 2. Oracle Smart Contract (caching layer)

**Source:** [contract/](https://github.com/zavodil/oracle-ark/tree/main/contract)
**Deployed:** `price-oracle.testnet`

On-chain contract that caches prices and integrates with the OutLayer platform for on-demand TEE execution.

#### Public Methods (for DeFi protocols)

| Method | Type | Description |
|--------|------|-------------|
| `get_price_data(asset_ids)` | View (free) | Get cached prices — no gas cost |
| `oracle_call(receiver_id, asset_ids, msg)` | Call | Get prices with cross-contract callback to `receiver_id` |
| `request_price_data(asset_ids)` | Call | Get prices directly (returns `PriceData`, no callback) |
| `custom_call(receiver_id, request, msg)` | Call | Fetch any external data, callback with result |
| `request_custom_data(request)` | Call | Fetch custom data directly (returns result) |
| `get_oracle_price_data(asset_ids, ...)` | View | For specific provider, dackward-compatible with `priceoracle.near` API |

#### Admin Methods

| Method | Description |
|--------|-------------|
| `add_asset(asset_id)` | Register new asset |
| `remove_asset(asset_id)` | Remove asset |
| `add_oracle(account_id)` | Register oracle provider |
| `remove_oracle(account_id)` | Remove oracle provider |
| `configure_outlayer(...)` | Set OutLayer contract, code source, secrets |
| `set_subsidize_outlayer_calls(enabled)` | Enable/disable free oracle for users |
| `set_recency_duration_sec(sec)` | Set staleness tolerance |
| `add_asset_ema(asset_id, period)` | Add EMA calculation for asset |
| `remove_asset_ema(asset_id, period)` | Remove EMA calculation |

#### Key Features

- **Subsidized mode**: When enabled and contract balance > 20 NEAR, the contract pays for OutLayer execution — users get prices for free
- **`can_subsidize_outlayer_calls()`**: View method to check if subsidy is active
- **Automatic freshness**: `oracle_call` returns cached prices if fresh, or triggers OutLayer fetch if stale
- **EMA support**: Exponential Moving Average calculations per asset, configurable periods
- **Backward compatibility**: Implements same `report_prices` / `get_price_data` API as `priceoracle.near`
- **ExecutionSource variants**: `GitHub` (compile from repo), `WasmUrl` (IPFS/FastFS immutable URL), `Project` (pre-uploaded immutable WASM by project ID)
- **Secrets support**: `secrets_profile` + `secrets_account_id` for encrypted API keys in TEE (optional)

#### Contract Versions

| Version | File |
|---------|------|
| Latest | `contract/res/price_oracle.wasm` |

---

### 3. Simple Wrapper Contract

**Source:** [wrapper-contract/](https://github.com/zavodil/oracle-ark/tree/main/wrapper-contract)
**Deployed:** `price-oracle-wrapper.testnet`

Simple contract that demonstrates how to integrate with the oracle. Also includes a prediction market example.

#### Methods

| Method | Description |
|--------|-------------|
| `get_price(token_id)` | Call oracle, get price via callback. Contract pays 0.02 NEAR per call. |
| `predict(token_id, predicted_price)` | Prediction market: guess a token price in USD |
| `resolve()` | Fetch actual price, compare with prediction. Verdict: "correct" (within 1%), "higher", "lower" |
| `oracle_on_call(sender_id, data, msg)` | Callback from oracle contract with price data |

The prediction market demonstrates cross-contract calls, oracle callbacks, and `msg`-based routing in the callback handler.

---

### 4. Pyth-Compatible Wrapper Contract

**Source:** [pyth-compatible-wrapper/](https://github.com/zavodil/oracle-ark/tree/main/pyth-compatible-wrapper)
**Deployed:** `price-oracle-pyth.testnet`

Drop-in replacement for `pyth-oracle.near`. Implements the same public API so DeFi protocols can switch from Pyth to Oracle-Ark with **zero code changes** — just change the contract address.

#### Pyth API Implemented

| Method | Type | Description |
|--------|------|-------------|
| `get_price(price_identifier)` | View | Get price with staleness check |
| `get_price_unsafe(price_identifier)` | View | Get price without staleness check |
| `get_price_no_older_than(price_id, age)` | View | Get price with custom max age |
| `get_ema_price(price_id)` | View | Get EMA price with staleness check |
| `get_ema_price_unsafe(price_id)` | View | Get EMA price without staleness check |
| `get_ema_price_no_older_than(price_id, age)` | View | Get EMA price with custom max age |
| `list_prices(price_ids)` | View | Batch: get multiple prices |
| `list_prices_unsafe(price_ids)` | View | Batch: without staleness check |
| `list_prices_no_older_than(price_ids)` | View | Batch: with staleness check |
| `price_feed_exists(price_identifier)` | View | Check if price feed is configured |
| `get_stale_threshold()` | View | Get staleness threshold in seconds |
| `update_price_feeds(data)` | Call | Accepts Pyth call pattern, triggers Oracle-Ark refresh |
| `get_update_fee_estimate(data)` | View | Returns 1 yoctoNEAR (effectively free) |
| `refresh_prices()` | Call | Anyone can trigger price refresh from Oracle-Ark |

#### Pyth Price Type Compatibility

```
Pyth format:    { price: i64, conf: u64, expo: i32, publish_time: i64 }
Oracle-Ark:     { multiplier: u128, decimals: u8, timestamp: u64 }
Conversion:     price = multiplier, conf = 0, expo = -decimals, publish_time = timestamp/1e9
```

#### Pre-configured Price Feed Mappings

| Asset | Pyth Price ID (hex) | Oracle-Ark Asset |
|-------|---------------------|------------------|
| NEAR/USD | `c415de8d2efa7db2...e226750` | `wrap.near` |
| ETH/USD | `ff61491a93111...fd0ace` | `aurora` |
| BTC/USD | `e62df6c8b4a85...f0f4a415b43` | `nbtc.bridge.near` |
| USDT/USD | `2b89b9dc8fdf...e2e53b` | `usdt.tether-token.near` |
| USDC/USD | `eaa020c61cc4...9e9c94a` | `17208628f84f5d6ad...` |

Additional mappings can be added via `add_price_mapping(price_id_hex, asset_id)` admin method.

#### Migration Guide for DeFi Protocols

```rust
// Before (Pyth):
const PYTH_CONTRACT: &str = "pyth-oracle.near";

// After (Oracle-Ark — same API, no other changes):
const PYTH_CONTRACT: &str = "price-oracle-pyth.near";
```

Reference: [Pyth receiver contract ext.rs](https://github.com/pyth-network/pyth-crosschain/blob/main/target_chains/near/receiver/src/ext.rs)

---

### 5. Scheduler (proactive price updates)

**Source:** [scheduler/](https://github.com/zavodil/oracle-ark/tree/main/scheduler)
**Status:** Running on VPS (Docker, `restart: unless-stopped`)

#### Architecture

The scheduler solves a key problem: how to keep TEE worker prices fresh so that any incoming request gets an **instant response with up-to-date data**, without spending gas on every update.

**How it works:**

1. **TEE worker** (WASI binary running inside Intel TDX enclave) holds fresh prices in its public storage. When any user or contract requests a price, the TEE worker returns the cached result immediately — no external API calls needed at request time.

2. **Scheduler** runs **outside** TEE on a separate VPS. Every 10 seconds it:
   - Fetches current prices from external sources (CoinGecko, Binance, Pyth, etc.) for comparison
   - Reads the TEE worker's stored prices via OutLayer public storage batch API (`/public/storage/batch`)
   - Compares the two: if the price **deviation exceeds the threshold** (default 1%) or if stored prices are **older than the interval** (default 60s), it triggers an update

3. **Triggering an update** means the scheduler sends a `call` request to the OutLayer coordinator with command `update_prices` and the list of tokens to update. Crucially, **the scheduler does NOT send price data** — it only tells the TEE worker *which* tokens need refreshing. The TEE worker then fetches prices from all configured sources independently inside the enclave, aggregates them, and writes the result to public storage.

This design ensures:
- **Trust model is preserved** — the scheduler never provides data that needs to be trusted; all price fetching and aggregation happens inside TEE
- **Gas-free operation** — prices stay fresh in TEE public storage without any on-chain transactions
- **Instant responses** — any WASI call requesting prices gets pre-computed results immediately
- **Efficient updates** — only tokens with significant price changes or stale data get updated

#### Optional: on-chain contract push

The scheduler supports an optional mode (`UPDATE_CONTRACT_ENABLED=true`) where the TEE worker not only updates its public storage but also calls `report_prices` on the oracle smart contract. This writes prices on-chain, making them available via contract view methods without an OutLayer call. However, each on-chain update costs gas, so this mode is **disabled by default** and should only be enabled when on-chain price availability is required.

#### Data flow diagram

```
External APIs                    Scheduler (VPS)                TEE Worker (Phala Cloud)
─────────────                    ───────────────                ────────────────────────
CoinGecko   ─┐                     ┌─────────────┐               ┌──────────────────────┐
Binance     ─┤  compare prices     │ Poll loop   │  read stored  │ Public Storage       │
Pyth        ─┼──────────────────>  │ (every 10s) │ <──────────── │  price:wrap.near     │
KuCoin      ─┤                     │             │   prices      │  price:aurora        │
Gate.io     ─┤                     │  if delta > │               │  price:nbtc...       │
Huobi       ─┤                     │  threshold: │               │                      │
Crypto.com. ─┤                     │             │  trigger      │ WASI Binary          │
Binance US  ─┤                     │  call WASI  │ ────────────> │  fetches own prices  │
BinanceAlpha─┘                     │  (no data!) │  update       │  from all 9 sources  │
                                   └─────────────┘               │  aggregates (median) │
                                                                 │  writes to storage   │
                                                                 └──────────────────────┘
```

#### Update triggers

| Trigger | Condition | Default |
|---------|-----------|---------|
| **Price deviation** | `abs(current - stored) / stored > threshold` | 1% (`PRICE_DIFF_THRESHOLD_PERCENT`) |
| **Time interval** | Time since last update > interval | 60s (`UPDATE_INTERVAL_SECS`) |
| **Missing price** | Token has no stored price yet | Always triggers |

Either trigger is sufficient — if a price moves fast, it gets updated before the interval expires.

#### Monitoring

Built-in alerting (currently Telegram, production monitoring TBD):
- **3+ consecutive poll failures** — alerts with error details
- **No prices available** — alerts when all external sources fail
- **WASI update failure** — alerts with token list and error

#### Configuration

| Variable | Default | Description |
|----------|---------|-------------|
| `COORDINATOR_URL` | `https://api.outlayer.fastnear.com` | OutLayer API URL |
| `PROJECT_OWNER` | required | NEAR account owning the project |
| `PROJECT_NAME` | required | OutLayer project name |
| `PROJECT_UUID` | required | Project UUID for public storage reads |
| `PAYMENT_KEY` | required | Payment key for WASI calls (format: `owner:nonce:secret`) |
| `TOKENS_CONFIG` | `../tokens.json` | Path to shared token configuration |
| `UPDATE_INTERVAL_SECS` | 60 | Maximum age of stored prices before refresh |
| `PRICE_DIFF_THRESHOLD_PERCENT` | 1.0 | Price change % that triggers immediate refresh |
| `UPDATE_CONTRACT_ENABLED` | false | Also push prices to on-chain contract (costs gas) |
| `ORACLE_CONTRACT_ID` | — | Contract to update (required if above is true) |
| `AGGREGATION_METHOD` | median | Price aggregation: median / average / weighted_average |
| `MIN_SOURCES_NUM` | 1 | Minimum sources required for valid price |
| `TELEGRAM_BOT_TOKEN` | — | Monitoring alerts (interim) |
| `TELEGRAM_CHAT_ID` | — | Monitoring alerts chat ID |
| `API_KEY` | — | API key for premium price sources |
| `RUST_LOG` | info | Log level (trace/debug/info/warn/error) |

---

### 6. Price Dashboard

**Source:** [oracle-prices-ui/](https://github.com/zavodil/oracle-ark/tree/main/oracle-prices-ui)

Standalone web dashboard showing live oracle prices. Deployed as a separate app for independent maintenance.

#### Features

- Live price display for all configured tokens
- Auto-refresh every 30 seconds with circular countdown timer
- Batch API for efficient data fetching (single request for all prices)
- Built-in CORS proxy (Python Flask server)
- Configurable via `.env` (API URL, project UUID, assets, port)

---

### 7. Documentation (16 files)

| Document | Description | Link |
|----------|-------------|------|
| **README.md** | Project overview, features, quick start, API reference | [Link](https://github.com/zavodil/oracle-ark/blob/main/README.md) |
| **integration.md** | Integration guide: view methods, call methods, custom data, Rust/JS examples | [Link](https://github.com/zavodil/oracle-ark/blob/main/integration.md) |
| **sdk.md** | SDK reference: data types, all contract methods, code snippets | [Link](https://github.com/zavodil/oracle-ark/blob/main/sdk.md) |
| **contract/README.md** | Contract API: backward-compatible methods, new methods, admin | [Link](https://github.com/zavodil/oracle-ark/blob/main/contract/README.md) |
| **wrapper-contract/README.md** | Simple wrapper usage guide | [Link](https://github.com/zavodil/oracle-ark/blob/main/wrapper-contract/README.md) |
| **pyth-compatible-wrapper/README.md** | Pyth wrapper usage + migration guide | [Link](https://github.com/zavodil/oracle-ark/blob/main/pyth-compatible-wrapper/README.md) |
| **pyth-compatible-wrapper/SPEC.md** | Technical specification for Pyth wrapper | [Link](https://github.com/zavodil/oracle-ark/blob/main/pyth-compatible-wrapper/SPEC.md) |
| **DEPLOY.md** | Full deployment guide (contract, WASI, scheduler, costs) | [Link](https://github.com/zavodil/oracle-ark/blob/main/DEPLOY.md) |
| **TROUBLESHOOTING.md** | Operational runbook (scheduler, contract, WASI, UI, API issues) | [Link](https://github.com/zavodil/oracle-ark/blob/main/TROUBLESHOOTING.md) |
| **SECURITY.md** | Security best practices (TEE, key management, access control, DeFi) | [Link](https://github.com/zavodil/oracle-ark/blob/main/SECURITY.md) |
| **SECURITY_PR.md** | URL validation / SSRF protection details | [Link](https://github.com/zavodil/oracle-ark/blob/main/SECURITY_PR.md) |
| **SOURCES.md** | Price sources reference | [Link](https://github.com/zavodil/oracle-ark/blob/main/SOURCES.md) |
| **PARALLEL_EXECUTION.md** | Parallel execution details | [Link](https://github.com/zavodil/oracle-ark/blob/main/PARALLEL_EXECUTION.md) |
| **CUSTOM_POST_BODY.md** | Custom POST request examples | [Link](https://github.com/zavodil/oracle-ark/blob/main/CUSTOM_POST_BODY.md) |
| **contract/UPGRADE.md** | Contract upgrade documentation | [Link](https://github.com/zavodil/oracle-ark/blob/main/contract/UPGRADE.md) |
| **oracle-prices-ui/README.md** | Dashboard setup guide | [Link](https://github.com/zavodil/oracle-ark/blob/main/oracle-prices-ui/README.md) |

---

## RFP Requirements Compliance

### Smart Contract Requirements

| Requirement | Status | Implementation |
|-------------|--------|----------------|
| **Pyth-compatible interface** | DONE | `pyth-compatible-wrapper/` — full Pyth receiver API (13 methods) |
| **Node operator allowlist** | By design | OutLayer `register-contract` maintains TDX measurements whitelist (MRTD + RTMR0-3) — only TEE-attested workers with approved binary can register. See [Architectural Principle](#architectural-principle-tee-proof-vs-operator-consensus) |
| **Multisig admin (Oracle DAO)** | By design | Not required — admin methods only change operational settings (assets, oracles, OutLayer config). No funds at risk from configuration changes. Contract balance protected by 20 NEAR minimum for subsidy mode |
| **Timelock for critical functions** | By design | Not required — same reasoning. Contract upgrade is the only destructive admin action, and it requires owner key + 1 yoctoNEAR deposit |
| **Pause functionality** | DONE | Owner can remove oracles/assets to effectively pause |

### Oracle Node Requirements

| Requirement | Status | Implementation |
|-------------|--------|----------------|
| **Fetch from 10+ APIs (min 5)** | DONE | 9 built-in exchanges + unlimited custom sources. Major tokens (NEAR, ETH, BTC, SOL) have 8 sources each |
| **Run in TEE** | DONE | WASI binary executes in Phala Cloud TEE workers (Intel TDX) |
| **Provide attestation** | DONE | Verified at worker registration via `register_worker_key()` — Intel DCAP-QVL TDX Quote verification + 5-measurement whitelist (MRTD + RTMR0-3). Access key cryptographically bound to TEE instance |
| **Global distribution** | DONE | OutLayer worker pool — multiple TEE workers available, any free worker responds. See [Architectural Principle](#architectural-principle-tee-proof-vs-operator-consensus) |

### Documentation Requirements

| Requirement | Status | Implementation |
|-------------|--------|----------------|
| **Integration guides + API docs** | DONE | `integration.md` (step-by-step guide), `sdk.md` (full API reference), `contract/README.md` |
| **NEAR docs contribution** | Not done | Planned: add entry to docs.near.org/primitives/oracles |
| **Node operator setup instructions** | DONE | `DEPLOY.md` (deployment), `TROUBLESHOOTING.md` (runbook), `SECURITY.md` (best practices) |
| **End-user documentation** | DONE | `wrapper-contract/README.md`, `pyth-compatible-wrapper/README.md`, code examples |

### Dashboard Requirements

| Requirement | Status | Implementation |
|-------------|--------|----------------|
| **Public website with prices** | DONE | `oracle-prices-ui/` — standalone live price dashboard with auto-refresh |
| **Worker node status** | DONE | Available in OutLayer dashboard: https://outlayer.fastnear.com/workers |
| **Historical price charts** | By design | Not storing historical data — would require disproportionate storage costs for minimal benefit. Current prices are always fresh via TEE |

---

## Deliverables by Phase

### Phase 1: Integration & Documentation — DONE

| Deliverable | Status | Details |
|-------------|--------|---------|
| Simple wrapper contract | Deployed | `price-oracle-wrapper.testnet` — `get_price(token_id)`, prediction market (`predict`/`resolve`) |
| Pyth-compatible wrapper contract | Deployed | `price-oracle-pyth.testnet` — full Pyth API (13 methods), 5 pre-configured assets |
| Caching oracle contract | Deployed | `price-oracle.testnet` — auto-caching, subsidized mode, OutLayer integration |
| Integration guide | Done | `integration.md` — Rust/JS examples, view methods, call methods, custom data |
| API reference | Done | `sdk.md` + `contract/README.md` — all methods, types, schemas |
| Example integrations | Done | wrapper-contract (basic), prediction market (predict/resolve), custom data fetching |
| Oracle dashboard | Done | `oracle-prices-ui/` — standalone live price dashboard |

### Phase 2: TEE Integration — DONE

| Deliverable | Status | Details |
|-------------|--------|---------|
| WASI execution in TEE | Done | oracle-ark.wasm runs in Phala Cloud TEE workers |
| Immutable WASM storage | Done | `ExecutionSource::WasmUrl` (IPFS/FastFS with hash), `ExecutionSource::Project` |
| Proactive pushing | Done | `scheduler/` — time-based + price deviation triggers |
| Operator documentation | Done | `DEPLOY.md`, `TROUBLESHOOTING.md`, `SECURITY.md` |
| Attestation verification | Done | Verified at worker registration via Intel DCAP-QVL (OutLayer `register-contract`). Per-request re-verification not needed — access key is cryptographically bound to TEE instance |
| Worker reputation tracking | Done | Handled by OutLayer platform (worker status, execution history) |
| Fallback mechanisms | Done | OutLayer coordinator handles worker pool assignment and automatic failover |
| Production monitoring | In progress | Interim Telegram alerts configured; production-grade monitoring planned |

### Phase 3: Governance & Decentralization — Addressed by Design

| Deliverable | Status | Details |
|-------------|--------|---------|
| Sputnik DAO deployment | By design | Not required — admin methods change operational settings only, no funds at risk. See [Architectural Principle](#architectural-principle-tee-proof-vs-operator-consensus) |
| Timelock integration | By design | Not required — contract upgrade is the only destructive action, requires owner key |
| Operator onboarding | By design | Not required — TEE proof replaces operator consensus. OutLayer worker pool provides redundancy |

---

## Testnet Deployments

| Contract | Account | Purpose |
|----------|---------|---------|
| Oracle caching contract | `price-oracle.testnet` | Main oracle — caches prices, integrates with OutLayer |
| Simple wrapper | `price-oracle-wrapper.testnet` | Demo: get_price + prediction market |
| Pyth-compatible wrapper | `price-oracle-pyth.testnet` | Pyth API drop-in replacement |

---

## Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                        DeFi Protocols                            │
│                                                                   │
│  Using oracle directly:        Using Pyth API:                   │
│  price-oracle.testnet          price-oracle-pyth.testnet         │
│  ├── get_price_data() [view]   ├── get_price(price_id) [view]   │
│  ├── oracle_call()             ├── list_prices(ids) [view]       │
│  └── request_price_data()      └── update_price_feeds()          │
│                                                                   │
│  Simple wrapper:                                                 │
│  price-oracle-wrapper.testnet                                    │
│  ├── get_price(token_id)                                         │
│  ├── predict(token_id, price)                                    │
│  └── resolve()                                                   │
└──────────────────────┬──────────────────────────────────────────┘
                       │
                       ▼
┌─────────────────────────────────────────────────────────────────┐
│             OutLayer Platform (outlayer.near)                     │
│             Request execution → TEE workers                      │
└──────────────────────┬──────────────────────────────────────────┘
                       │
          ┌────────────┼────────────┐
          ▼            ▼            ▼
┌──────────────┐ ┌──────────┐ ┌──────────────┐
│  TEE Worker  │ │  Worker  │ │  Scheduler   │
│  (Phala)     │ │  (Phala) │ │  (VPS)       │
│              │ │          │ │              │
│ oracle-ark   │ │ oracle-  │ │ Monitors     │
│ .wasm        │ │ ark.wasm │ │ prices,      │
│              │ │          │ │ triggers     │
│ Fetches:     │ │          │ │ updates      │
│ CoinGecko    │ │          │ │              │
│ Binance      │ │          │ │              │
│ Pyth         │ │          │ │              │
│ KuCoin       │ │          │ │              │
│ Gate.io      │ │          │ │              │
│ Huobi        │ │          │ │              │
│ Crypto.com   │ │          │ │              │
│ Custom APIs  │ │          │ │              │
└──────────────┘ └──────────┘ └──────────────┘
```

---

## Code Statistics

| Component | Language | Files | Lines (approx) |
|-----------|----------|-------|----------------|
| WASI module (src/) | Rust | 7 | ~1,100 |
| Shared sources (sources/) | Rust | 3 | ~400 |
| Oracle contract (contract/) | Rust | 7 | ~1,500 |
| Wrapper contract | Rust | 1 | ~250 |
| Pyth wrapper | Rust | 1 | ~520 |
| Scheduler | Rust | 4 | ~500 |
| Dashboard (oracle-prices-ui/) | HTML/Python | 4 | ~400 |
| Documentation | Markdown | 16 | ~2,500 |
| **Total** | | **~43** | **~7,200** |

---

## Remaining Work

### High Priority
- [ ] NEAR docs contribution (docs.near.org/primitives/oracles)
- [ ] Deploy all contracts to mainnet

### Medium Priority
- [ ] Production-grade monitoring (interim Telegram alerts configured)

---

## Architectural Principle: TEE Proof vs Operator Consensus

Oracle-Ark's architecture is fundamentally different from traditional oracle designs, which explains why several RFP items (DAO governance, operator onboarding, multi-node consensus, per-request attestation) are addressed differently than originally planned.

### Traditional oracle vs Oracle-Ark

| | Traditional Oracle (e.g., Chainlink) | Oracle-Ark |
|--|--------------------------------------|------------|
| **Trust source** | Economic incentives — N operators stake collateral, majority must collude to manipulate | Cryptographic proof — Intel TDX attestation proves correct code ran on genuine hardware |
| **Price submission** | Each operator independently submits prices on-chain | Single TEE-verified computation fetches from 9+ sources, aggregates inside enclave |
| **Consensus** | On-chain aggregation of N submissions | Median aggregation of 9 sources inside TEE |
| **Cost per update** | N gas transactions (one per operator) | 0 gas (prices stored in TEE public storage) or 1 gas transaction (optional on-chain push) |
| **Operator overhead** | Recruit, train, monitor operators; coordinate upgrades | Zero — OutLayer worker pool is shared infrastructure |

### Why this makes certain RFP items unnecessary

**No independent operators needed.** Multiple OutLayer workers exist in a pool, but they are interchangeable executors — whoever is free responds. All workers have TEE attestations (verified at registration). Trust comes from cryptographic proof, not from counting independent submissions. Adding 5 operators who all run the same deterministic code in TEE adds no security — it only multiplies gas costs.

**No DAO governance for oracle settings.** Admin methods change operational parameters: add/remove assets, configure OutLayer integration, enable/disable subsidy. These don't put funds at risk — the contract's 20 NEAR minimum balance acts as a circuit breaker for subsidy mode. Contract upgrade is the only destructive action and already requires owner key + deposit.

**No timelock needed.** Configuration changes are non-destructive and immediately reversible. A 7-day delay for adding a new asset or changing the OutLayer config would only slow down operations without security benefit.

**No multi-node consensus (min_confirmations).** If TEE is trusted (verified via DCAP attestation), one execution is sufficient. If TEE is compromised (Intel vulnerability), running N compromised TEEs provides no additional security. The correct mitigation for TEE compromise is measurements rotation (deploy new binary, update approved measurements), not redundant execution.

---

## Addressed by OutLayer Platform

Several items from the original RFP are implemented at the OutLayer platform level. Oracle-Ark runs on OutLayer, which provides these capabilities for all WASI projects.

### TEE Attestation Verification — Done (OutLayer register-contract)

Attestation is verified **at worker registration** via `register_worker_key()` in the OutLayer register-contract:

1. Worker generates an ED25519 keypair **inside** the Intel TDX enclave
2. Worker produces a TDX Quote embedding the public key in `report_data` (first 32 bytes)
3. `register_worker_key()` verifies the TDX Quote using Intel DCAP-QVL (`dcap_qvl::verify::verify()`)
4. Extracts all 5 TDX measurements (MRTD + RTMR0-3) and checks them against the pre-approved list (`approved_measurements`)
5. Verifies the public key in the quote matches the submitted key — cryptographic proof the key was generated inside TEE
6. On success, adds the key as a function-call access key limited to `resolve_execution`, `submit_execution_output_and_resolve`, etc.

The resulting access key is cryptographically bound to that specific TEE instance. If the enclave is destroyed, the key is lost. Per-request re-verification is not needed — the one-time registration already proves the worker identity, and the 5-measurement whitelist ensures only approved binary versions can register.

### Worker Node Status & Reputation — Done (OutLayer Dashboard)

Worker status monitoring, execution history, and reputation tracking are provided by the OutLayer platform: https://outlayer.fastnear.com/workers

### Worker Whitelisting — Done (OutLayer)

Two-tier whitelisting:

1. **Worker execution keys** (`register-contract`): Owner maintains `approved_measurements` whitelist (all 5 TDX measurements must match). Only workers running an approved binary can register.

2. **Keystore secret access** (`keystore-dao-contract`): Full DAO governance with proposal/vote mechanism. DAO members vote on keystore registrations. Approval threshold is >50% of members. Approved keystores get access to MPC-derived secrets.

### Fallback Mechanisms — Done (OutLayer Coordinator)

The OutLayer coordinator manages the worker pool: task assignment, automatic failover to available workers, retry on failure. Oracle-Ark does not need its own failover logic.

### MPC Secret Derivation — Done (OutLayer keystore-worker)

MPC Chain Key Derivation (CKD) is fully implemented:

1. Keystore worker generates ephemeral BLS12-381 keypair inside TEE
2. Calls MPC contract via DAO proxy: `request_key(CkdRequestArgs)` with derivation path
3. MPC network returns encrypted response (`big_y`, `big_c` — BLS12-381 G1 points)
4. Keystore decrypts using ephemeral private key (ECIES scheme)
5. Derives master secret via HKDF-SHA256
6. Master secret never leaves TEE memory — repo-specific keypairs derived via HMAC-SHA256
