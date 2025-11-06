//! NEP-297 Event Format Validation
//!
//! Verifies that contract events follow the NEAR Event standard envelope.
//!
//! Reference: https://github.com/near/NEPs/blob/master/neps/nep-0297.md
//!
//! Events are log entries that:
//! 1. Start with the `EVENT_JSON:` prefix
//! 2. Followed by a single valid JSON string
//! 3. JSON must have {standard, version, event} fields (data is optional)

use serde::{Deserialize, Serialize};
use anyhow::Result;

/// NEP-297 Event Log Data
///
/// Interface to capture data about an event:
/// * `standard`: name of standard, e.g. "outlayer" or "nep171"
/// * `version`: e.g. "1.0.0"
/// * `event`: type of the event, e.g. "execution_requested"
/// * `data`: associated event data (optional, strictly typed per standard)
#[derive(Debug, Deserialize, Serialize)]
struct Nep297Event {
    standard: String,
    version: String,
    event: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    data: Option<serde_json::Value>,
}

/// Parse NEP-297 event from log string
///
/// Valid format: `EVENT_JSON:{"standard":"...","version":"...","event":"..."}`
///
/// Returns the parsed event or an error if:
/// - Missing `EVENT_JSON:` prefix
/// - Invalid JSON after prefix
/// - Missing required fields (standard, version, event)
#[allow(dead_code)]
fn parse_nep297_event(log: &str) -> Result<Nep297Event> {
    // Check for EVENT_JSON: prefix
    if !log.starts_with("EVENT_JSON:") {
        anyhow::bail!("Event log missing 'EVENT_JSON:' prefix");
    }

    // Extract JSON part (after prefix)
    let json_str = log.strip_prefix("EVENT_JSON:").unwrap().trim();

    // Parse JSON
    let event: Nep297Event = serde_json::from_str(json_str)
        .map_err(|e| anyhow::anyhow!("Invalid JSON in event log: {e}"))?;

    // Validate required fields are non-empty
    if event.standard.is_empty() {
        anyhow::bail!("Event 'standard' field is empty");
    }
    if event.version.is_empty() {
        anyhow::bail!("Event 'version' field is empty");
    }
    if event.event.is_empty() {
        anyhow::bail!("Event 'event' field is empty");
    }

    Ok(event)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_event_envelope_structure() -> Result<()> {
        // Valid NEP-297 event log (with EVENT_JSON: prefix)
        let log = r#"EVENT_JSON:{"standard":"outlayer","version":"1.0.0","event":"execution_requested","data":{"request_id":"123","requester":"alice.near"}}"#;

        let event = parse_nep297_event(log)?;

        assert_eq!(event.standard, "outlayer", "Event standard must be 'outlayer'");
        assert_eq!(event.version, "1.0.0", "Event version must be '1.0.0'");
        assert_eq!(event.event, "execution_requested", "Event type must match");
        assert!(event.data.is_some(), "Event data should be present");

        println!("✓ NEP-297 event envelope structure validated");

        Ok(())
    }

    #[tokio::test]
    async fn test_event_without_data_field() -> Result<()> {
        // Valid event without optional data field
        let log = r#"EVENT_JSON:{"standard":"outlayer","version":"1.0.0","event":"xyz_triggered"}"#;

        let event = parse_nep297_event(log)?;

        assert_eq!(event.standard, "outlayer");
        assert_eq!(event.version, "1.0.0");
        assert_eq!(event.event, "xyz_triggered");
        assert!(event.data.is_none(), "Data field should be None");

        println!("✓ NEP-297 event without data field validated");

        Ok(())
    }

    #[tokio::test]
    async fn test_event_with_whitespace() -> Result<()> {
        // NEP-297 allows whitespace around JSON
        let log = r#"EVENT_JSON:  {"standard":"outlayer","version":"1.0.0","event":"test"}  "#;

        let event = parse_nep297_event(log)?;

        assert_eq!(event.standard, "outlayer");
        assert_eq!(event.event, "test");

        println!("✓ NEP-297 event with whitespace validated");

        Ok(())
    }

    #[tokio::test]
    async fn test_missing_prefix_rejected() {
        // Missing EVENT_JSON: prefix should fail
        let log = r#"{"standard":"outlayer","version":"1.0.0","event":"test"}"#;

        let result = parse_nep297_event(log);
        assert!(result.is_err(), "Missing prefix should be rejected");

        println!("✓ Missing EVENT_JSON: prefix correctly rejected");
    }

    #[tokio::test]
    async fn test_invalid_json_rejected() {
        // Invalid JSON after prefix should fail
        let log = r#"EVENT_JSON:invalid json"#;

        let result = parse_nep297_event(log);
        assert!(result.is_err(), "Invalid JSON should be rejected");

        println!("✓ Invalid JSON correctly rejected");
    }

    #[tokio::test]
    async fn test_missing_required_fields_rejected() {
        // Missing 'version' field
        let log = r#"EVENT_JSON:{"standard":"outlayer","event":"test"}"#;

        let result = parse_nep297_event(log);
        assert!(result.is_err(), "Missing required field should be rejected");

        println!("✓ Missing required fields correctly rejected");
    }

    #[tokio::test]
    async fn test_execution_requested_event() -> Result<()> {
        // OutLayer-specific event: execution_requested
        let log = r#"EVENT_JSON:{"standard":"outlayer","version":"1.0.0","event":"execution_requested","data":{"request_id":"42","requester":"user.near","code_source":{"repo":"github.com/example/repo","commit":"main"}}}"#;

        let event = parse_nep297_event(log)?;
        assert_eq!(event.standard, "outlayer");
        assert_eq!(event.event, "execution_requested");

        let data = event.data.unwrap();
        assert!(data.get("request_id").is_some());
        assert!(data.get("requester").is_some());
        assert!(data.get("code_source").is_some());

        println!("✓ ExecutionRequested event structure validated");

        Ok(())
    }

    #[tokio::test]
    async fn test_execution_resolved_event() -> Result<()> {
        // OutLayer-specific event: execution_resolved
        let log = r#"EVENT_JSON:{"standard":"outlayer","version":"1.0.0","event":"execution_resolved","data":{"request_id":"42","status":"success","fuel_consumed":1500000,"execution_time_ms":250}}"#;

        let event = parse_nep297_event(log)?;
        assert_eq!(event.standard, "outlayer");
        assert_eq!(event.event, "execution_resolved");

        let data = event.data.unwrap();
        assert!(data.get("request_id").is_some());
        assert!(data.get("status").is_some());
        assert!(data.get("fuel_consumed").is_some());
        assert!(data.get("execution_time_ms").is_some());

        println!("✓ ExecutionResolved event structure validated");

        Ok(())
    }

    #[tokio::test]
    async fn test_multiple_events_in_single_log_rejected() {
        // NEP-297 explicitly forbids multiple events in single log entry
        let log = r#"EVENT_JSON:{"standard":"outlayer","version":"1.0.0","event":"abc"}
EVENT_JSON:{"standard":"outlayer","version":"1.0.0","event":"xyz"}"#;

        // Parsing should fail (only first line parsed, second line causes newline in JSON)
        let result = parse_nep297_event(log);
        assert!(result.is_err(), "Multiple events in single log should be rejected");

        println!("✓ Multiple events in single log correctly rejected");
    }

    #[tokio::test]
    async fn test_pretty_formatted_json() -> Result<()> {
        // NEP-297 allows pretty-formatted JSON (spaces/newlines)
        let log = r#"EVENT_JSON:{
  "standard": "outlayer",
  "version": "1.0.0",
  "event": "test_event",
  "data": {
    "key": "value"
  }
}"#;

        let event = parse_nep297_event(log)?;

        assert_eq!(event.standard, "outlayer");
        assert_eq!(event.event, "test_event");
        assert!(event.data.is_some());

        println!("✓ Pretty-formatted JSON event validated");

        Ok(())
    }
}
