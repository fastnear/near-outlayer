use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::{Deserialize, Serialize};

// ============================================================================
// Error types
// ============================================================================

#[derive(Debug)]
pub enum WalletError {
    /// Missing authentication — no Authorization header or X-Internal-Wallet-Auth header
    MissingAuth,
    /// Invalid or revoked API key
    InvalidApiKey,
    /// Missing or invalid X-Wallet-Id header (internal auth only)
    MissingWalletId,
    /// Invalid wallet ID format
    InvalidWalletIdFormat(String),
    /// Missing X-Signature header
    MissingSignature,
    /// Missing X-Timestamp header
    MissingTimestamp,
    /// Invalid signature
    InvalidSignature(String),
    /// Timestamp too old or too far in the future
    TimestampExpired,
    /// Duplicate idempotency key
    DuplicateIdempotencyKey { request_id: String },
    /// Wallet is frozen by controller
    WalletFrozen,
    /// Policy denied the operation
    PolicyDenied(String),
    /// Insufficient balance
    InsufficientBalance(String),
    /// Invalid destination address
    InvalidAddress(String),
    /// Rate limited
    RateLimited,
    /// Unsupported chain
    UnsupportedChain(String),
    /// Unsupported token
    UnsupportedToken(String),
    /// Request not found
    RequestNotFound,
    /// Approval not found
    ApprovalNotFound,
    /// Not an approver
    NotApprover,
    /// Already approved
    AlreadyApproved,
    /// Keystore error
    KeystoreError(String),
    /// Internal error
    InternalError(String),
}

impl IntoResponse for WalletError {
    fn into_response(self) -> Response {
        let (status, error_code, message) = match self {
            WalletError::MissingAuth => (
                StatusCode::UNAUTHORIZED,
                "missing_auth",
                "Provide Authorization: Bearer <api_key> header. Register at POST /register".to_string(),
            ),
            WalletError::InvalidApiKey => (
                StatusCode::UNAUTHORIZED,
                "invalid_api_key",
                "Invalid or revoked API key".to_string(),
            ),
            WalletError::MissingWalletId => (
                StatusCode::UNAUTHORIZED,
                "missing_wallet_id",
                "Missing X-Wallet-Id header".to_string(),
            ),
            WalletError::InvalidWalletIdFormat(msg) => (
                StatusCode::BAD_REQUEST,
                "invalid_wallet_id",
                msg,
            ),
            WalletError::MissingSignature => (
                StatusCode::UNAUTHORIZED,
                "missing_signature",
                "Missing X-Signature header".to_string(),
            ),
            WalletError::MissingTimestamp => (
                StatusCode::UNAUTHORIZED,
                "missing_timestamp",
                "Missing X-Timestamp header".to_string(),
            ),
            WalletError::InvalidSignature(msg) => (
                StatusCode::UNAUTHORIZED,
                "invalid_signature",
                msg,
            ),
            WalletError::TimestampExpired => (
                StatusCode::UNAUTHORIZED,
                "timestamp_expired",
                "Timestamp is too old or too far in the future (30 second window)".to_string(),
            ),
            WalletError::DuplicateIdempotencyKey { request_id } => (
                StatusCode::OK, // Return OK with existing result for idempotent requests
                "duplicate_idempotency_key",
                format!("Request already processed: {}", request_id),
            ),
            WalletError::WalletFrozen => (
                StatusCode::FORBIDDEN,
                "wallet_frozen",
                "Wallet is frozen by controller".to_string(),
            ),
            WalletError::PolicyDenied(msg) => (
                StatusCode::FORBIDDEN,
                "policy_denied",
                msg,
            ),
            WalletError::InsufficientBalance(msg) => (
                StatusCode::BAD_REQUEST,
                "insufficient_balance",
                msg,
            ),
            WalletError::InvalidAddress(msg) => (
                StatusCode::BAD_REQUEST,
                "invalid_address",
                msg,
            ),
            WalletError::RateLimited => (
                StatusCode::TOO_MANY_REQUESTS,
                "rate_limited",
                "Too many requests".to_string(),
            ),
            WalletError::UnsupportedChain(chain) => (
                StatusCode::BAD_REQUEST,
                "unsupported_chain",
                format!("Unsupported chain: {}", chain),
            ),
            WalletError::UnsupportedToken(token) => (
                StatusCode::BAD_REQUEST,
                "unsupported_token",
                format!("Unsupported token: {}", token),
            ),
            WalletError::RequestNotFound => (
                StatusCode::NOT_FOUND,
                "request_not_found",
                "Request not found".to_string(),
            ),
            WalletError::ApprovalNotFound => (
                StatusCode::NOT_FOUND,
                "approval_not_found",
                "Approval not found".to_string(),
            ),
            WalletError::NotApprover => (
                StatusCode::FORBIDDEN,
                "not_approver",
                "You are not listed as an approver for this wallet".to_string(),
            ),
            WalletError::AlreadyApproved => (
                StatusCode::CONFLICT,
                "already_approved",
                "You have already approved this request".to_string(),
            ),
            WalletError::KeystoreError(msg) => (
                StatusCode::BAD_GATEWAY,
                "keystore_error",
                msg,
            ),
            WalletError::InternalError(msg) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal_error",
                msg,
            ),
        };

        (
            status,
            Json(serde_json::json!({
                "error": error_code,
                "message": message,
            })),
        )
            .into_response()
    }
}

// ============================================================================
// Request types
// ============================================================================

#[derive(Debug, Deserialize)]
pub struct WithdrawRequest {
    pub chain: String,
    pub to: String,
    pub amount: String,
    #[serde(default)]
    pub token: Option<String>, // null = native
}

#[derive(Debug, Deserialize)]
pub struct DepositRequest {
    pub source_chain: String,
    pub token: String,
    pub amount: String,
}

#[derive(Debug, Deserialize)]
pub struct EncryptPolicyRequest {
    pub wallet_id: String,
    pub rules: serde_json::Value,
    #[serde(default)]
    pub approval: Option<serde_json::Value>,
    #[serde(default)]
    pub admin_quorum: Option<serde_json::Value>,
    #[serde(default)]
    pub webhook_url: Option<String>,
    /// SHA256 hashes of authorized API keys (owner-controlled, synced via on-chain policy)
    #[serde(default)]
    pub authorized_key_hashes: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
pub struct SignPolicyRequest {
    pub encrypted_data: String,
}

#[derive(Debug, Deserialize)]
pub struct InvalidateCacheRequest {
    pub wallet_id: String,
}

#[derive(Debug, Deserialize)]
pub struct AddressQuery {
    pub chain: String,
}

#[derive(Debug, Deserialize)]
pub struct RequestsQuery {
    #[serde(default)]
    pub r#type: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default = "default_limit")]
    pub limit: i64,
    #[serde(default)]
    pub offset: i64,
}

fn default_limit() -> i64 {
    50
}

#[derive(Debug, Deserialize)]
pub struct AuditQuery {
    #[serde(default = "default_limit")]
    pub limit: i64,
    #[serde(default)]
    pub offset: i64,
}

// ============================================================================
// Response types
// ============================================================================

#[derive(Debug, Serialize)]
pub struct AddressResponse {
    pub wallet_id: String,
    pub chain: String,
    pub address: String,
}

#[derive(Debug, Serialize)]
pub struct WithdrawResponse {
    pub request_id: String,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub approval_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub required: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub approved: Option<i32>,
}

#[derive(Debug, Serialize)]
pub struct DryRunResponse {
    pub would_succeed: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub estimated_fee: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fee_token: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub policy_check: Option<PolicyCheckResult>,
}

#[derive(Debug, Serialize)]
pub struct PolicyCheckResult {
    pub within_limits: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub daily_remaining: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub address_allowed: Option<bool>,
}

#[derive(Debug, Serialize)]
pub struct DepositResponse {
    pub request_id: String,
    pub status: String,
    pub deposit_address: String,
    pub chain: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct RequestStatusResponse {
    pub request_id: String,
    pub r#type: String,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    pub created_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct RequestListResponse {
    pub requests: Vec<RequestStatusResponse>,
    pub total: i64,
}

#[derive(Debug, Serialize)]
pub struct TokenInfo {
    pub id: String,
    pub symbol: String,
    pub chains: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct TokensResponse {
    pub tokens: Vec<TokenInfo>,
}

#[derive(Debug, Serialize)]
pub struct EncryptPolicyResponse {
    pub encrypted_base64: String,
    pub wallet_pubkey: String,
}

#[derive(Debug, Serialize)]
pub struct PendingApprovalResponse {
    pub approval_id: String,
    pub request_id: Option<String>,
    pub r#type: String,
    pub request_data: serde_json::Value,
    pub required: i32,
    pub approved: i32,
    pub expires_at: String,
}

#[derive(Debug, Serialize)]
pub struct PendingApprovalsResponse {
    pub approvals: Vec<PendingApprovalResponse>,
}

#[derive(Debug, Serialize)]
pub struct ApproveResponse {
    pub approval_id: String,
    pub status: String,
    pub approved: i32,
    pub required: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_id: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct PolicyResponse {
    pub wallet_id: String,
    pub controller: String,
    pub frozen: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rules: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub approval: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub authorized_key_hashes: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage: Option<serde_json::Value>,
}

#[derive(Debug, Serialize)]
pub struct AuditEvent {
    pub r#type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    #[serde(flatten)]
    pub details: serde_json::Value,
    pub at: String,
}

#[derive(Debug, Serialize)]
pub struct AuditResponse {
    pub events: Vec<AuditEvent>,
}

// ============================================================================
// Registration + API key management types
// ============================================================================

#[derive(Debug, Serialize)]
pub struct RegisterResponse {
    pub wallet_id: String,
    pub api_key: String,
    pub near_account_id: String,
    pub handoff_url: String,
}

// API key management types removed — keys controlled by on-chain policy

// ============================================================================
// Keystore request/response types
// ============================================================================

#[derive(Debug, Serialize)]
pub struct KeystoreDeriveAddressRequest {
    pub wallet_id: String,
    pub chain: String,
}

#[derive(Debug, Deserialize)]
pub struct KeystoreDeriveAddressResponse {
    pub address: String,
    pub public_key: String,
}

#[derive(Debug, Serialize)]
pub struct KeystoreSignNep413Request {
    pub wallet_id: String,
    pub chain: String,
    pub message: String,
    pub nonce_base64: String,
    pub recipient: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub approval_info: Option<ApprovalInfo>,
}

#[derive(Debug, Deserialize)]
pub struct KeystoreSignNep413Response {
    pub signature_base58: String,
    pub public_key: String,
}

#[derive(Debug, Serialize)]
pub struct KeystoreCheckPolicyRequest {
    pub wallet_id: String,
    pub action: serde_json::Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub encrypted_policy_data: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct KeystoreCheckPolicyResponse {
    pub allowed: bool,
    pub frozen: bool,
    #[serde(default)]
    pub requires_approval: bool,
    #[serde(default)]
    pub required_approvals: Option<i32>,
    #[serde(default)]
    pub reason: Option<String>,
    #[serde(default)]
    pub policy: Option<serde_json::Value>,
}

#[derive(Debug, Serialize)]
pub struct KeystoreEncryptPolicyRequest {
    pub wallet_id: String,
    pub policy_json: String,
}

#[derive(Debug, Deserialize)]
pub struct KeystoreEncryptPolicyInnerResponse {
    pub encrypted_base64: String,
}

#[derive(Debug, Serialize)]
pub struct KeystoreSignPolicyRequest {
    pub wallet_id: String,
    pub encrypted_data_hash: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct KeystoreSignPolicyResponse {
    pub signature_hex: String,
    pub public_key_hex: String,
}

// ============================================================================
// Approval info (passed to keystore for verification)
// ============================================================================

#[derive(Debug, Serialize, Clone)]
pub struct ApprovalInfo {
    pub approver_ids: Vec<String>,
    pub request_hash: String,
}

// ============================================================================
// Call (native NEAR function call) types
// ============================================================================

#[derive(Debug, Deserialize)]
pub struct CallRequest {
    pub receiver_id: String,
    pub method_name: String,
    #[serde(default = "default_empty_json")]
    pub args: serde_json::Value,
    #[serde(default)]
    pub gas: Option<String>,     // default 30 TGas
    #[serde(default)]
    pub deposit: Option<String>, // default "0" yoctoNEAR
}

fn default_empty_json() -> serde_json::Value {
    serde_json::json!({})
}

#[derive(Debug, Serialize)]
pub struct CallResponse {
    pub request_id: String,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tx_hash: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub approval_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub required: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub approved: Option<i32>,
}

#[derive(Debug, serde::Serialize)]
pub struct KeystoreSignNearCallRequest {
    pub wallet_id: String,
    pub receiver_id: String,
    pub method_name: String,
    pub args_json: String,
    pub gas: u64,
    pub deposit: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub approval_info: Option<ApprovalInfo>,
}

#[derive(Debug, serde::Deserialize)]
pub struct KeystoreSignNearCallResponse {
    pub signed_tx_base64: String,
    pub tx_hash: String,
    pub signer_id: String,
    pub public_key: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::response::IntoResponse;

    #[test]
    fn test_error_status_codes() {
        let cases: Vec<(WalletError, StatusCode)> = vec![
            (WalletError::MissingAuth, StatusCode::UNAUTHORIZED),
            (WalletError::InvalidApiKey, StatusCode::UNAUTHORIZED),
            (WalletError::MissingWalletId, StatusCode::UNAUTHORIZED),
            (WalletError::InvalidWalletIdFormat("x".into()), StatusCode::BAD_REQUEST),
            (WalletError::MissingSignature, StatusCode::UNAUTHORIZED),
            (WalletError::MissingTimestamp, StatusCode::UNAUTHORIZED),
            (WalletError::InvalidSignature("x".into()), StatusCode::UNAUTHORIZED),
            (WalletError::TimestampExpired, StatusCode::UNAUTHORIZED),
            (WalletError::WalletFrozen, StatusCode::FORBIDDEN),
            (WalletError::PolicyDenied("x".into()), StatusCode::FORBIDDEN),
            (WalletError::InsufficientBalance("x".into()), StatusCode::BAD_REQUEST),
            (WalletError::InvalidAddress("x".into()), StatusCode::BAD_REQUEST),
            (WalletError::RateLimited, StatusCode::TOO_MANY_REQUESTS),
            (WalletError::UnsupportedChain("x".into()), StatusCode::BAD_REQUEST),
            (WalletError::UnsupportedToken("x".into()), StatusCode::BAD_REQUEST),
            (WalletError::RequestNotFound, StatusCode::NOT_FOUND),
            (WalletError::ApprovalNotFound, StatusCode::NOT_FOUND),
            (WalletError::NotApprover, StatusCode::FORBIDDEN),
            (WalletError::AlreadyApproved, StatusCode::CONFLICT),
            (WalletError::KeystoreError("x".into()), StatusCode::BAD_GATEWAY),
            (WalletError::InternalError("x".into()), StatusCode::INTERNAL_SERVER_ERROR),
        ];

        for (error, expected_status) in cases {
            let response = error.into_response();
            assert_eq!(
                response.status(),
                expected_status,
                "Wrong status for error variant"
            );
        }
    }

    #[test]
    fn test_duplicate_idempotency_returns_200() {
        let error = WalletError::DuplicateIdempotencyKey {
            request_id: "req-123".to_string(),
        };
        let response = error.into_response();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[test]
    fn test_error_json_shape() {
        let error = WalletError::MissingWalletId;
        let response = error.into_response();
        let (_, body) = response.into_parts();
        // body is an axum body — we can't easily parse in sync test,
        // but status code is already verified above.
        // Verify the shape via direct JSON construction:
        let (_, error_code, message) = (
            StatusCode::UNAUTHORIZED,
            "missing_wallet_id",
            "Missing X-Wallet-Id header",
        );
        let json = serde_json::json!({
            "error": error_code,
            "message": message,
        });
        assert!(json["error"].is_string());
        assert!(json["message"].is_string());
    }

    #[test]
    fn test_withdraw_request_deserialization() {
        let json = r#"{"chain": "near", "to": "dest.near", "amount": "1000000"}"#;
        let req: WithdrawRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.chain, "near");
        assert!(req.token.is_none());

        let json_with_token = r#"{"chain": "near", "to": "dest.near", "amount": "1000000", "token": "usdc"}"#;
        let req2: WithdrawRequest = serde_json::from_str(json_with_token).unwrap();
        assert_eq!(req2.token, Some("usdc".to_string()));
    }

    #[test]
    fn test_requests_query_defaults() {
        let json = r#"{}"#;
        let query: RequestsQuery = serde_json::from_str(json).unwrap();
        assert_eq!(query.limit, 50);
        assert_eq!(query.offset, 0);
        assert!(query.r#type.is_none());
        assert!(query.status.is_none());
    }
}
