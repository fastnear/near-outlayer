use anyhow::{Context, Result};
use near_primitives::types::AccountId;
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::time::Duration;
use tokio::time::sleep;
use tracing::{error, info, warn};

use crate::api_client::{ApiClient, CreateTaskParams, ResourceLimits as ApiResourceLimits};

/// Enum for different event types from contract
#[derive(Debug, Clone)]
pub enum ContractEvent {
    ExecutionRequested(ExecutionRequestedEvent),
    ProjectStorageCleanup(ProjectStorageCleanupEvent),
}

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
    #[serde(skip)]
    pub receipt_id: Option<String>,  // Receipt ID from neardata
    #[serde(skip)]
    pub predecessor_id: Option<String>,  // Predecessor from neardata
    #[serde(skip)]
    pub signer_public_key: Option<String>,  // Signer public key from neardata
    #[serde(skip)]
    pub gas_burnt: Option<u64>,  // Gas burnt from neardata
}

/// ProjectStorageCleanup event data from contract (emitted when project is deleted)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectStorageCleanupEvent {
    pub project_id: String,
    pub project_uuid: String,
    pub timestamp: u64,
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
    #[serde(default)]
    pub compile_only: bool,
    #[serde(default)]
    pub force_rebuild: bool,
    #[serde(default)]
    pub store_on_fastfs: bool,
    /// Project UUID for persistent storage (passed from request_execution_project)
    #[serde(default)]
    pub project_uuid: Option<String>,
    /// Project ID for project-based secrets (e.g., "alice.near/my-app")
    #[serde(default)]
    pub project_id: Option<String>,
}

/// Code source - either GitHub repo or pre-compiled WASM URL
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum CodeSource {
    GitHub {
        #[serde(rename = "GitHub")]
        github: GitHubSource,
    },
    WasmUrl {
        #[serde(rename = "WasmUrl")]
        wasm_url: WasmUrlSource,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitHubSource {
    pub repo: String,
    pub commit: String,
    pub build_target: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WasmUrlSource {
    pub url: String,
    pub hash: String,
    pub build_target: Option<String>,
}

impl CodeSource {
    /// Get repo URL (for GitHub sources)
    #[allow(dead_code)]
    pub fn repo(&self) -> Option<&str> {
        match self {
            CodeSource::GitHub { github } => Some(&github.repo),
            CodeSource::WasmUrl { .. } => None,
        }
    }

    /// Get commit (for GitHub sources)
    #[allow(dead_code)]
    pub fn commit(&self) -> Option<&str> {
        match self {
            CodeSource::GitHub { github } => Some(&github.commit),
            CodeSource::WasmUrl { .. } => None,
        }
    }

    /// Get build target
    pub fn build_target(&self) -> Option<&str> {
        match self {
            CodeSource::GitHub { github } => github.build_target.as_deref(),
            CodeSource::WasmUrl { wasm_url } => wasm_url.build_target.as_deref(),
        }
    }

    /// Set build target
    pub fn set_build_target(&mut self, target: String) {
        match self {
            CodeSource::GitHub { github } => github.build_target = Some(target),
            CodeSource::WasmUrl { wasm_url } => wasm_url.build_target = Some(target),
        }
    }

    /// Get display string for logging
    pub fn display(&self) -> String {
        match self {
            CodeSource::GitHub { github } => format!("{}@{}", github.repo, github.commit),
            CodeSource::WasmUrl { wasm_url } => format!("url:{} hash:{}", wasm_url.url, wasm_url.hash),
        }
    }

    /// Convert to api_client::CodeSource
    pub fn to_api_code_source(&self) -> crate::api_client::CodeSource {
        match self {
            CodeSource::GitHub { github } => crate::api_client::CodeSource::GitHub {
                repo: github.repo.clone(),
                commit: github.commit.clone(),
                build_target: github.build_target.clone().unwrap_or_else(|| "wasm32-wasi".to_string()),
            },
            CodeSource::WasmUrl { wasm_url } => crate::api_client::CodeSource::WasmUrl {
                url: wasm_url.url.clone(),
                hash: wasm_url.hash.clone(),
                build_target: wasm_url.build_target.clone().unwrap_or_else(|| "wasm32-wasi".to_string()),
            },
        }
    }
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
    predecessor_id: Option<String>,
    receipt: Option<ReceiptAction>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "Action")]
struct ReceiptAction {
    #[serde(rename = "Action")]
    action: Option<ActionDetails>,
}

#[derive(Debug, Deserialize)]
struct ActionDetails {
    signer_public_key: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ExecutionOutcome {
    outcome: Option<Outcome>,
    #[allow(dead_code)]
    id: Option<String>,  // receipt_id
}

#[derive(Debug, Deserialize)]
struct Outcome {
    logs: Option<Vec<String>>,
    #[allow(dead_code)]
    receipt_ids: Option<Vec<String>>,
    gas_burnt: Option<u64>,
}

/// FastNEAR status response
#[derive(Debug, Deserialize)]
struct FastNearStatus {
    sync_block_height: u64,
}

/// NEAR event monitor that watches neardata.xyz for execution_requested and version_requested events
pub struct EventMonitor {
    api_client: ApiClient,
    neardata_api_url: String,
    #[allow(dead_code)]
    fastnear_api_url: String,
    contract_id: AccountId,
    current_block: u64,
    scan_interval_ms: u64,
    http_client: reqwest::Client,
    event_json_regex: Regex,
    blocks_scanned: u64,
    events_found: u64,
    // Event filters
    event_filter_standard_name: String,
    #[allow(dead_code)]
    event_filter_function_name: String, // Kept for compatibility but we now handle multiple events
    event_filter_min_version: Option<(u64, u64, u64)>, // Parsed semver (major, minor, patch)
}

impl EventMonitor {
    /// Parse semver string like "1.2.3" into (major, minor, patch)
    fn parse_semver(version: &str) -> Option<(u64, u64, u64)> {
        let parts: Vec<&str> = version.split('.').collect();
        if parts.len() >= 3 {
            let major = parts[0].parse().ok()?;
            let minor = parts[1].parse().ok()?;
            let patch = parts[2].parse().ok()?;
            Some((major, minor, patch))
        } else if parts.len() == 2 {
            let major = parts[0].parse().ok()?;
            let minor = parts[1].parse().ok()?;
            Some((major, minor, 0))
        } else if parts.len() == 1 {
            let major = parts[0].parse().ok()?;
            Some((major, 0, 0))
        } else {
            None
        }
    }

    /// Compare two semver tuples: returns true if actual >= required
    fn semver_gte(actual: (u64, u64, u64), required: (u64, u64, u64)) -> bool {
        actual >= required
    }

    pub async fn new(
        api_client: ApiClient,
        neardata_api_url: String,
        fastnear_api_url: String,
        contract_id: AccountId,
        start_block: u64,
        scan_interval_ms: u64,
        event_filter_standard_name: String,
        event_filter_function_name: String,
        event_filter_min_version: Option<String>,
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

        // Parse min version if provided
        let parsed_min_version = event_filter_min_version
            .as_ref()
            .and_then(|v| Self::parse_semver(v));

        info!(
            "Event filter: standard={}, function={}, min_version={:?}",
            event_filter_standard_name, event_filter_function_name, event_filter_min_version
        );

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
            event_filter_standard_name,
            event_filter_function_name,
            event_filter_min_version: parsed_min_version,
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
                            "📦 Block {}: Found {} contract events (total: {} events in {} blocks)",
                            self.current_block,
                            events.len(),
                            self.events_found,
                            self.blocks_scanned
                        );
                    }

                    // Process found events
                    for event in events {
                        match event {
                            ContractEvent::ExecutionRequested(exec_event) => {
                                if let Err(e) = self.handle_execution_requested(exec_event).await {
                                    error!("Failed to handle execution_requested event: {}", e);
                                }
                            }
                            ContractEvent::ProjectStorageCleanup(cleanup_event) => {
                                if let Err(e) = self.handle_project_storage_cleanup(cleanup_event).await {
                                    error!("Failed to handle project_storage_cleanup event: {}", e);
                                }
                            }
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

    /// Scan a single block for contract events
    async fn scan_single_block(&self, block_id: u64) -> Result<Vec<ContractEvent>> {
        let block_data = self.load_block(block_id).await?;

        if block_data.shards.is_none() {
            return Ok(vec![]);
        }

        let events = self.process_shards(&block_data.shards.unwrap(), block_id)?;

        if !events.is_empty() {
            info!(
                "Block {}: found {} contract events",
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
    ) -> Result<Vec<ContractEvent>> {
        let mut events = Vec::new();
        let mut receipts_checked = 0;
        let mut contract_receipts = 0;

        // Process receipt execution outcomes
        for shard in shards {
            if let Some(receipt_outcomes) = &shard.receipt_execution_outcomes {
                for outcome in receipt_outcomes {
                    receipts_checked += 1;

                    // Extract neardata fields
                    let receipt_id = outcome.receipt.as_ref()
                        .and_then(|r| r.receipt_id.clone());
                    let predecessor_id = outcome.receipt.as_ref()
                        .and_then(|r| r.predecessor_id.clone());
                    let signer_public_key = outcome.receipt.as_ref()
                        .and_then(|r| r.receipt.as_ref())
                        .and_then(|action| action.action.as_ref())
                        .and_then(|details| details.signer_public_key.clone());
                    let gas_burnt = outcome.execution_outcome.as_ref()
                        .and_then(|exec| exec.outcome.as_ref())
                        .and_then(|o| o.gas_burnt);
                    let transaction_hash = outcome.tx_hash.clone();

                    // Check receiver_id matches our contract
                    let is_our_contract = if let Some(receipt) = &outcome.receipt {
                        if let Some(receiver_id) = &receipt.receiver_id {
                            if receiver_id == self.contract_id.as_str() {
                                contract_receipts += 1;
                                true
                            } else {
                                false
                            }
                        } else {
                            false
                        }
                    } else {
                        false
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
                                        // Add transaction metadata only for ExecutionRequested events
                                        if let ContractEvent::ExecutionRequested(ref mut exec_event) = event {
                                            exec_event.transaction_hash = transaction_hash.clone();
                                            exec_event.receipt_id = receipt_id.clone();
                                            exec_event.predecessor_id = predecessor_id.clone();
                                            exec_event.signer_public_key = signer_public_key.clone();
                                            exec_event.gas_burnt = gas_burnt;
                                        }
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

    /// Process individual log entry - handles multiple event types
    fn process_log(&self, log: &str, block_height: u64) -> Option<ContractEvent> {
        // Extract EVENT_JSON from log
        let captures = self.event_json_regex.captures(log)?;
        let event_json_str = captures.get(1)?.as_str();

        // Parse JSON
        let event: Value = serde_json::from_str(event_json_str).ok()?;

        // Check standard name
        let standard = event.get("standard")?.as_str()?;
        if standard != self.event_filter_standard_name {
            return None;
        }

        // Check min version (>= comparison)
        if let Some(required_version) = self.event_filter_min_version {
            let event_version_str = event.get("version").and_then(|v| v.as_str());
            match event_version_str.and_then(|v| Self::parse_semver(v)) {
                Some(actual_version) if Self::semver_gte(actual_version, required_version) => {}
                _ => return None,
            }
        }

        let event_name = event.get("event")?.as_str()?;
        let data_array = event.get("data")?.as_array()?;
        if data_array.is_empty() {
            return None;
        }

        // Handle different event types
        match event_name {
            "execution_requested" => {
                // Only process if matches the configured filter (for backwards compatibility)
                if event_name != self.event_filter_function_name {
                    return None;
                }

                let mut event_data: ExecutionRequestedEvent =
                    serde_json::from_value(data_array[0].clone()).ok()?;
                event_data.block_height = block_height;

                // Parse and validate request_data
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

                if request_data.code_source.build_target().is_none() {
                    request_data.code_source.set_build_target("wasm32-wasi".to_string());
                    info!("⚠️  No build_target specified, defaulting to wasm32-wasi");
                } else {
                    info!("📦 build_target specified: {}", request_data.code_source.build_target().unwrap());
                }

                info!(
                    "✅ Found execution_requested event at block {}: request_id={} source={}",
                    block_height, request_data.request_id, request_data.code_source.display()
                );

                Some(ContractEvent::ExecutionRequested(event_data))
            }
            "project_storage_cleanup" => {
                let event_data: ProjectStorageCleanupEvent =
                    serde_json::from_value(data_array[0].clone()).ok()?;

                info!(
                    "✅ Found project_storage_cleanup event at block {}: project_id={} uuid={}",
                    block_height, event_data.project_id, event_data.project_uuid
                );

                Some(ContractEvent::ProjectStorageCleanup(event_data))
            }
            _ => None,
        }
    }

    /// Handle execution_requested event by creating task in coordinator
    async fn handle_execution_requested(&self, event: ExecutionRequestedEvent) -> Result<()> {
        // Log raw event data for debugging
        tracing::debug!("📋 Raw request_data JSON: {}", event.request_data);

        // Parse the nested request_data JSON
        let request_data: RequestData = serde_json::from_str(&event.request_data)
            .context("Failed to parse request_data JSON")?;

        info!(
            "Creating task for execution request: request_id={} source={} sender={} response_format={:?}",
            request_data.request_id,
            request_data.code_source.display(),
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
            transaction_hash: event.transaction_hash.clone(),
            receipt_id: event.receipt_id.clone(),
            predecessor_id: event.predecessor_id.clone(),
            signer_public_key: event.signer_public_key.clone(),
            gas_burnt: event.gas_burnt,
        };

        // Convert code_source to api_client format
        let api_code_source = request_data.code_source.to_api_code_source();

        // Create task in coordinator API
        let params = CreateTaskParams {
            request_id: request_data.request_id,
            data_id: data_id_hex.clone(),
            code_source: api_code_source,
            resource_limits: ApiResourceLimits {
                max_instructions: request_data.resource_limits.max_instructions,
                max_memory_mb: request_data.resource_limits.max_memory_mb,
                max_execution_seconds: request_data.resource_limits.max_execution_seconds,
            },
            input_data: request_data.input_data.clone(),
            secrets_ref: request_data.secrets_ref.clone(),
            response_format: request_data.response_format.clone(),
            context,
            user_account_id: Some(request_data.sender_id.clone()),
            near_payment_yocto: Some(request_data.payment.clone()),
            compile_only: request_data.compile_only,
            force_rebuild: request_data.force_rebuild,
            store_on_fastfs: request_data.store_on_fastfs,
            project_uuid: request_data.project_uuid.clone(),
            project_id: request_data.project_id.clone(),
        };

        match self.api_client.create_task(params).await
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
                    "❌ Failed to create task: {}. data_id={} source={}",
                    e, data_id_hex, request_data.code_source.display()
                );
                return Err(e);
            }
        }

        Ok(())
    }

    /// Handle project_storage_cleanup event by clearing storage in coordinator
    async fn handle_project_storage_cleanup(&self, event: ProjectStorageCleanupEvent) -> Result<()> {
        info!(
            "🧹 Clearing storage for deleted project: project_id={} uuid={}",
            event.project_id, event.project_uuid
        );

        match self.api_client.clear_project_storage(&event.project_uuid).await {
            Ok(()) => {
                info!(
                    "✅ Successfully cleared storage for project: uuid={}",
                    event.project_uuid
                );
                Ok(())
            }
            Err(e) => {
                error!(
                    "❌ Failed to clear storage for project uuid={}: {}",
                    event.project_uuid, e
                );
                Err(e)
            }
        }
    }
}
