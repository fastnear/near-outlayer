use crate::*;
use near_sdk::serde_json::json;

const EVENT_STANDARD: &str = "near-outlayer";
const EVENT_STANDARD_VERSION: &str = "1.0.0";

pub mod emit {
    use super::*;
    use near_sdk::{env, log};

    #[derive(Serialize)]
    #[serde(crate = "near_sdk::serde")]
    struct ExecutionRequestedEventData<'a> {
        pub request_data: &'a String,
        pub data_id: CryptoHash,
        pub timestamp: u64,
    }

    #[derive(Serialize)]
    #[serde(crate = "near_sdk::serde")]
    struct ExecutionCompletedEventData<'a> {
        pub sender_id: &'a AccountId,
        pub code_source: &'a CodeSource,
        pub resources_used: &'a ResourceMetrics,
        pub success: bool,
        pub error_message: Option<&'a str>,
        pub payment_charged: U128,    // Actual amount charged (after refund)
        pub payment_refunded: U128,   // Amount refunded to user
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

    /// Emit event when worker should process execution request
    pub fn execution_requested(request_data: &String, data_id: CryptoHash) {
        log_event(
            "execution_requested",
            ExecutionRequestedEventData {
                request_data,
                data_id,
                timestamp: env::block_timestamp(),
            },
        );
    }

    /// Emit event when execution is completed (success or failure)
    pub fn execution_completed(
        sender_id: &AccountId,
        code_source: &CodeSource,
        resources_used: &ResourceMetrics,
        success: bool,
        error_message: Option<&str>,
        payment_charged: U128,
        payment_refunded: U128,
        compilation_note: Option<&str>,
    ) {
        log_event(
            "execution_completed",
            ExecutionCompletedEventData {
                sender_id,
                code_source,
                resources_used,
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
