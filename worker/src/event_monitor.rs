use anyhow::{Context, Result};
use near_primitives::types::AccountId;
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::time::Duration;
use tokio::time::sleep;
use tracing::{error, info, warn};

use crate::api_client::ApiClient;

/// ExecutionRequested event data from contract (matches contract's event structure)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionRequestedEvent {
    pub request_data: String,  // JSON string containing RequestData
    pub data_id: Vec<u8>,
    pub timestamp: u64,
    #[serde(skip)]
    pub block_height: u64,  // Added locally, not from contract event
    #[serde(skip)]
    pub transaction_hash: Option<String>,  // Original transaction hash from neardata
}

/// Parsed request data from the JSON string
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestData {
    pub request_id: u64,
    pub sender_id: String,
    pub code_source: CodeSource,
    pub resource_limits: ResourceLimits,
    pub input_data: String,
    #[serde(default)]
    pub secrets_ref: Option<crate::api_client::SecretsReference>,
    pub payment: String,
    pub timestamp: u64,
    #[serde(default)]
    pub response_format: crate::api_client::ResponseFormat,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeSource {
    pub repo: String,
    pub commit: String,
    pub build_target: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceLimits {
    pub max_instructions: u64,
    pub max_memory_mb: u32,
    pub max_execution_seconds: u64,
}

/// Block data from neardata.xyz API
#[derive(Debug, Deserialize)]
struct BlockData {
    shards: Option<Vec<ShardData>>,
}

#[derive(Debug, Deserialize)]
struct ShardData {
    receipt_execution_outcomes: Option<Vec<ReceiptExecutionOutcome>>,
}

#[derive(Debug, Deserialize)]
struct ReceiptExecutionOutcome {
    receipt: Option<Receipt>,
    execution_outcome: Option<ExecutionOutcome>,
    tx_hash: Option<String>,
}

#[derive(Debug, Deserialize)]
struct Receipt {
    receiver_id: Option<String>,
    receipt_id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ExecutionOutcome {
    outcome: Option<Outcome>,
    id: Option<String>,  // receipt_id
}

#[derive(Debug, Deserialize)]
struct Outcome {
    logs: Option<Vec<String>>,
    receipt_ids: Option<Vec<String>>,
}

/// FastNEAR status response
#[derive(Debug, Deserialize)]
struct FastNearStatus {
    sync_block_height: u64,
}

/// NEAR event monitor that watches neardata.xyz for execution_requested events
pub struct EventMonitor {
    api_client: ApiClient,
    neardata_api_url: String,
    fastnear_api_url: String,
    contract_id: AccountId,
    current_block: u64,
    scan_interval_ms: u64,
    http_client: reqwest::Client,
    event_json_regex: Regex,
    blocks_scanned: u64,
    events_found: u64,
}

impl EventMonitor {
    pub async fn new(
        api_client: ApiClient,
        neardata_api_url: String,
        fastnear_api_url: String,
        contract_id: AccountId,
        start_block: u64,
        scan_interval_ms: u64,
    ) -> Result<Self> {
        let http_client = reqwest::Client::builder()
            .timeout(Duration::from_secs(10))
            .build()
            .context("Failed to create HTTP client")?;

        // If start_block is 0, fetch latest block from FastNEAR
        let current_block = if start_block == 0 {
            info!("START_BLOCK_HEIGHT=0, fetching latest block from FastNEAR...");
            Self::fetch_latest_block(&http_client, &fastnear_api_url).await?
        } else {
            start_block
        };

        Ok(Self {
            api_client,
            neardata_api_url,
            fastnear_api_url,
            contract_id,
            current_block,
            scan_interval_ms,
            http_client,
            event_json_regex: Regex::new(r"EVENT_JSON:(.*?)$")
                .context("Failed to compile regex")?,
            blocks_scanned: 0,
            events_found: 0,
        })
    }

    /// Fetch latest block height from FastNEAR API
    async fn fetch_latest_block(
        http_client: &reqwest::Client,
        fastnear_api_url: &str,
    ) -> Result<u64> {
        info!("Fetching latest block height from {}", fastnear_api_url);

        let response = http_client
            .get(fastnear_api_url)
            .send()
            .await
            .context("Failed to fetch FastNEAR status")?;

        if !response.status().is_success() {
            anyhow::bail!(
                "FastNEAR API returned status: {}",
                response.status()
            );
        }

        let status: FastNearStatus = response
            .json()
            .await
            .context("Failed to parse FastNEAR status")?;

        info!("Latest block height: {}", status.sync_block_height);
        Ok(status.sync_block_height)
    }

    /// Start continuous monitoring of new blocks
    pub async fn start_monitoring(&mut self) -> Result<()> {
        info!(
            "Starting event monitoring from block {} for contract {}",
            self.current_block, self.contract_id
        );

        let start_block = self.current_block;
        let mut retry_count = 0;
        const MAX_RETRIES: u32 = 3;

        loop {
            match self.scan_single_block(self.current_block).await {
                Ok(events) => {
                    self.blocks_scanned += 1;
                    retry_count = 0; // Reset retry counter on success

                    if !events.is_empty() {
                        self.events_found += events.len() as u64;
                        info!(
                            "📦 Block {}: Found {} execution_requested events (total: {} events in {} blocks)",
                            self.current_block,
                            events.len(),
                            self.events_found,
                            self.blocks_scanned
                        );
                    }

                    // Process found events
                    for event in events {
                        if let Err(e) = self.handle_execution_requested(event).await {
                            error!("Failed to handle execution_requested event: {}", e);
                        }
                    }

                    // Move to next block
                    self.current_block += 1;

                    // Log progress every 100 blocks
                    if self.blocks_scanned % 100 == 0 {
                        info!(
                            "📊 Progress: Scanned blocks {}-{} ({} blocks, {} events found)",
                            start_block,
                            self.current_block - 1,
                            self.blocks_scanned,
                            self.events_found
                        );
                    }

                    // Brief pause between blocks (if configured)
                    if self.scan_interval_ms > 0 {
                        sleep(Duration::from_millis(self.scan_interval_ms)).await;
                    }
                }
                Err(e) => {
                    retry_count += 1;
                    error!(
                        "❌ Error scanning block {} (attempt {}/{}): {}",
                        self.current_block, retry_count, MAX_RETRIES, e
                    );

                    if retry_count >= MAX_RETRIES {
                        warn!(
                            "⚠️  Skipping block {} after {} failed attempts",
                            self.current_block, MAX_RETRIES
                        );
                        // Skip to next block
                        self.current_block += 1;
                        retry_count = 0;
                        sleep(Duration::from_secs(1)).await;
                    } else {
                        // Wait before retrying same block
                        sleep(Duration::from_secs(5)).await;
                    }
                }
            }
        }
    }

    /// Scan a single block for execution_requested events
    async fn scan_single_block(&self, block_id: u64) -> Result<Vec<ExecutionRequestedEvent>> {
        let block_data = self.load_block(block_id).await?;

        if block_data.shards.is_none() {
            return Ok(vec![]);
        }

        let events = self.process_shards(&block_data.shards.unwrap(), block_id)?;

        if !events.is_empty() {
            info!(
                "Block {}: found {} execution_requested events",
                block_id,
                events.len()
            );
        }

        Ok(events)
    }

    /// Load block data from neardata.xyz API
    async fn load_block(&self, block_id: u64) -> Result<BlockData> {
        let url = self.neardata_api_url.replace("{block_id}", &block_id.to_string());

        let response = self
            .http_client
            .get(&url)
            .send()
            .await
            .context("Failed to fetch block")?;

        match response.status() {
            reqwest::StatusCode::OK => {
                // First, get the response body as a raw string.
                // This helps in debugging if JSON parsing fails later.
                let response_text = response
                    .text()
                    .await
                    .context("Failed to read response body as text")?;

                // Handle null response (block not yet indexed)
                if response_text.trim() == "null" {
                    info!("⏳ Block {} returned null (not indexed yet)", block_id);
                    return Ok(BlockData { shards: None });
                }

                // Now, try to parse the string. If it fails, the error
                // message can include the raw text that caused the issue.
                let block_data: BlockData = serde_json::from_str(&response_text)
                    .with_context(|| format!("Failed to parse block data from JSON. Raw text (truncated): '{}'",
                        if response_text.len() > 200 { &response_text[..200] } else { &response_text }))?;

                // The rest of your logic remains unchanged.
                let shard_count = block_data.shards.as_ref().map(|s| s.len()).unwrap_or(0);
                if self.blocks_scanned % 10 == 0 {
                    // Assuming `block_id` is available in this scope.
                    info!("📥 Block {}: Fetched from neardata ({} shards)", block_id, shard_count);
                }

                Ok(block_data)
            }
            reqwest::StatusCode::NOT_FOUND => {
                info!("⏳ Block {} not found yet (waiting for neardata indexing)", block_id);
                // Block not found yet, return empty data
                Ok(BlockData { shards: None })
            }
            status => {
                anyhow::bail!("HTTP {} for block {}", status, block_id);
            }
        }
    }

    /// Process shards from block data
    fn process_shards(
        &self,
        shards: &[ShardData],
        block_height: u64,
    ) -> Result<Vec<ExecutionRequestedEvent>> {
        let mut events = Vec::new();
        let mut receipts_checked = 0;
        let mut contract_receipts = 0;

        // Process receipt execution outcomes
        for shard in shards {
            if let Some(receipt_outcomes) = &shard.receipt_execution_outcomes {
                for outcome in receipt_outcomes {
                    receipts_checked += 1;

                    // Check receiver_id matches our contract and get tx_hash
                    let (is_our_contract, transaction_hash) = if let Some(receipt) = &outcome.receipt {
                        if let Some(receiver_id) = &receipt.receiver_id {
                            if receiver_id == self.contract_id.as_str() {
                                contract_receipts += 1;
                                (true, outcome.tx_hash.clone())
                            } else {
                                (false, None)
                            }
                        } else {
                            (false, None)
                        }
                    } else {
                        (false, None)
                    };

                    if !is_our_contract {
                        continue;
                    }

                    // Process logs from our contract
                    if let Some(execution) = &outcome.execution_outcome {
                        if let Some(outcome_data) = &execution.outcome {
                            if let Some(logs) = &outcome_data.logs {
                                for log in logs {
                                    if let Some(mut event) =
                                        self.process_log(log, block_height)
                                    {
                                        event.transaction_hash = transaction_hash.clone();
                                        events.push(event);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        // Log detailed stats every 10 blocks if we found receipts for our contract
        if contract_receipts > 0 || (self.blocks_scanned % 50 == 0 && receipts_checked > 0) {
            info!(
                "🔍 Block {}: Checked {} receipts, {} for contract {}, {} events",
                block_height,
                receipts_checked,
                contract_receipts,
                self.contract_id,
                events.len()
            );
        }

        Ok(events)
    }

    /// Process individual log entry
    fn process_log(&self, log: &str, block_height: u64) -> Option<ExecutionRequestedEvent> {
        // Extract EVENT_JSON from log
        let captures = self.event_json_regex.captures(log)?;
        let event_json_str = captures.get(1)?.as_str();

        // Parse JSON
        let event: Value = serde_json::from_str(event_json_str).ok()?;

        // Check if this is our execution_requested event
        if event.get("standard")?.as_str()? != "near-offshore" {
            return None;
        }

        if event.get("event")?.as_str()? != "execution_requested" {
            return None;
        }

        let data_array = event.get("data")?.as_array()?;
        if data_array.is_empty() {
            return None;
        }

        // Parse the first data entry
        let mut event_data: ExecutionRequestedEvent =
            serde_json::from_value(data_array[0].clone()).ok()?;

        // Set block_height from parameter
        event_data.block_height = block_height;

        // Parse the nested request_data JSON string
        let mut request_data: RequestData = match serde_json::from_str(&event_data.request_data) {
            Ok(data) => data,
            Err(e) => {
                error!(
                    "Failed to parse request_data JSON at block {}: {}. Raw: {}",
                    block_height, e, event_data.request_data
                );
                return None;
            }
        };

        info!("request_data secrets_ref: {:?}", request_data.secrets_ref);

        // Default to wasm32-wasi if not specified (will be normalized to wasm32-wasip1 by compiler)
        if request_data.code_source.build_target.is_none() {
            request_data.code_source.build_target = Some("wasm32-wasi".to_string());
            info!("⚠️  No build_target specified, defaulting to wasm32-wasi");
        } else {
            info!("📦 build_target specified: {}", request_data.code_source.build_target.as_ref().unwrap());
        }

        info!(
            "✅ Found execution_requested event at block {}: request_id={} repo={} commit={}",
            block_height, request_data.request_id, request_data.code_source.repo, request_data.code_source.commit
        );

        Some(event_data)
    }

    /// Handle execution_requested event by creating task in coordinator
    async fn handle_execution_requested(&self, event: ExecutionRequestedEvent) -> Result<()> {
        // Log raw event data for debugging
        tracing::debug!("📋 Raw request_data JSON: {}", event.request_data);

        // Parse the nested request_data JSON
        let request_data: RequestData = serde_json::from_str(&event.request_data)
            .context("Failed to parse request_data JSON")?;

        info!(
            "Creating task for execution request: request_id={} repo={} commit={} sender={} response_format={:?}",
            request_data.request_id,
            request_data.code_source.repo,
            request_data.code_source.commit,
            request_data.sender_id,
            request_data.response_format
        );

        // Convert data_id Vec<u8> to hex string
        let data_id_hex = hex::encode(&event.data_id);

        // Build execution context
        let context = crate::api_client::ExecutionContext {
            sender_id: Some(request_data.sender_id.clone()),
            block_height: Some(event.block_height),
            block_timestamp: Some(event.timestamp),
            contract_id: Some(self.contract_id.to_string()),
        };

        // Create task in coordinator API
        match self.api_client
            .create_task(
                request_data.request_id,              // request_id from contract
                data_id_hex.clone(),
                request_data.code_source.repo.clone(),
                request_data.code_source.commit.clone(),
                request_data.code_source.build_target.clone().unwrap_or_else(|| "wasm32-wasi".to_string()),
                request_data.resource_limits.max_instructions,
                request_data.resource_limits.max_memory_mb,
                request_data.resource_limits.max_execution_seconds,
                request_data.input_data.clone(),
                request_data.secrets_ref.clone(),
                request_data.response_format.clone(),
                context,
                Some(request_data.sender_id.clone()), // user_account_id
                Some(request_data.payment.clone()),   // near_payment_yocto
                event.transaction_hash.clone(),       // transaction_hash from neardata
            )
            .await
        {
            Ok(Some(request_id)) => {
                info!("✅ Task created in coordinator: request_id={} data_id={}",
                    request_id, data_id_hex);
            }
            Ok(None) => {
                info!("ℹ️  Task already exists (duplicate): data_id={}", data_id_hex);
            }
            Err(e) => {
                error!(
                    "❌ Failed to create task: {}. data_id={} repo={} commit={}",
                    e, data_id_hex,
                    request_data.code_source.repo, request_data.code_source.commit
                );
                return Err(e);
            }
        }

        Ok(())
    }
}
