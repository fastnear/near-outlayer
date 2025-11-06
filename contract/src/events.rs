//! NEP-297 Event Format
//!
//! Phase 1 Hardening: All events follow NEP-297 envelope:
//! ```json
//! {
//!   "standard": "near-outlayer",
//!   "version": "1.0.0",
//!   "event": "execution_requested",
//!   "data": [{ ... }]
//! }
//! ```
//!
//! Events include:
//! - request_id: Unique identifier for each execution request
//! - payer: AccountId that paid for execution
//! - limits: Requested resource limits (max_instructions, max_memory, max_time)
//! - actuals: Actual resource usage (instructions, memory, time)
//! - wasm_checksum: SHA256 of compiled WASM (for cache verification)

use crate::*;
use near_sdk::serde_json::json;

const EVENT_STANDARD: &str = "near-outlayer";
const EVENT_STANDARD_VERSION: &str = "1.0.0";

pub mod emit {
    use super::*;
    use near_sdk::{env, log};

    /// Phase 1: Enhanced with request_id, payer, limits for observability
    #[derive(Serialize)]
    #[serde(crate = "near_sdk::serde")]
    struct ExecutionRequestedEventData<'a> {
        pub request_id: u64,              // Phase 1: Unique request identifier
        pub payer: &'a AccountId,         // Phase 1: Who paid for this execution
        pub code_source: &'a CodeSource,  // Phase 1: Full code source info
        pub limits: &'a ResourceLimits,   // Phase 1: Requested limits
        pub request_data: &'a String,     // Original request JSON (for workers)
        pub data_id: CryptoHash,          // Hash of request data
        pub timestamp: u64,
    }

    /// Phase 1: Enhanced with request_id, actuals vs limits, wasm_checksum
    #[derive(Serialize)]
    #[serde(crate = "near_sdk::serde")]
    struct ExecutionCompletedEventData<'a> {
        pub request_id: u64,              // Phase 1: Match with ExecutionRequestedEvent
        pub payer: &'a AccountId,         // Phase 1: Who paid (may differ from sender)
        pub sender_id: &'a AccountId,     // Original requester
        pub code_source: &'a CodeSource,  // Code that was executed
        pub limits: &'a ResourceLimits,   // Phase 1: Requested limits
        pub actuals: &'a ResourceMetrics, // Phase 1: Actual usage (renamed from resources_used)
        pub wasm_checksum: Option<&'a str>, // Phase 1: SHA256 of compiled WASM
        pub success: bool,
        pub error_message: Option<&'a str>,
        pub payment_charged: U128,        // Actual amount charged (after refund)
        pub payment_refunded: U128,       // Amount refunded to user
        pub compilation_note: Option<&'a str>, // e.g., "Cached WASM from 2025-01-10 14:30 UTC"
        pub timestamp: u64,
    }

    fn log_event<T: Serialize>(event: &str, data: T) {
        let event = json!({
            "standard": EVENT_STANDARD,
            "version": EVENT_STANDARD_VERSION,
            "event": event,
            "data": [data]
        });

        log!("EVENT_JSON:{}", event.to_string());
    }

    /// Phase 1: Enhanced execution_requested event with full observability
    pub fn execution_requested(
        request_id: u64,
        payer: &AccountId,
        code_source: &CodeSource,
        limits: &ResourceLimits,
        request_data: &String,
        data_id: CryptoHash,
    ) {
        log_event(
            "execution_requested",
            ExecutionRequestedEventData {
                request_id,
                payer,
                code_source,
                limits,
                request_data,
                data_id,
                timestamp: env::block_timestamp(),
            },
        );
    }

    /// Phase 1: Enhanced execution_completed event with limits vs actuals comparison
    #[allow(clippy::too_many_arguments)]
    pub fn execution_completed(
        request_id: u64,
        payer: &AccountId,
        sender_id: &AccountId,
        code_source: &CodeSource,
        limits: &ResourceLimits,
        actuals: &ResourceMetrics,
        wasm_checksum: Option<&str>,
        success: bool,
        error_message: Option<&str>,
        payment_charged: U128,
        payment_refunded: U128,
        compilation_note: Option<&str>,
    ) {
        log_event(
            "execution_completed",
            ExecutionCompletedEventData {
                request_id,
                payer,
                sender_id,
                code_source,
                limits,
                actuals,
                wasm_checksum,
                success,
                error_message,
                payment_charged,
                payment_refunded,
                compilation_note,
                timestamp: env::block_timestamp(),
            },
        );
    }
}
