//! HTTP handlers for wallet API endpoints
//!
//! All endpoints under /wallet/v1/

use super::audit;
use super::auth::{self, authenticate};
use super::backend::{BackendDepositRequest, BackendWithdrawRequest, WalletInfo};
use super::idempotency;
use super::policy::{self, PolicyDecision};
use super::types::*;
use super::webhooks;
use super::WalletState;
use axum::extract::{Path, Query, State};
use axum::http::HeaderMap;
use axum::Json;
use chrono::Utc;
use sha2::{Digest, Sha256};
use tracing::{debug, info, warn};
use uuid::Uuid;

// ============================================================================
// POST /register — create new wallet account, return API key
// ============================================================================

/// Register a new wallet account and return an API key.
///
/// The API key is generated in plaintext and returned once — it is NOT stored
/// in the database (only its SHA-256 hash is persisted). The coordinator is not
/// running inside TEE, so the plaintext key exists briefly in server memory.
///
/// Security recommendation: after setting up a policy via the handoff link,
/// the human controller should revoke the initial API key (DELETE /wallet/v1/api-keys/:hash)
/// and create a new one (POST /wallet/v1/api-keys). The new key is generated under
/// policy control — even if compromised, operations are limited by policy rules (limits,
/// freeze, multisig). This is recommended but not enforced.
pub async fn register(
    State(state): State<WalletState>,
    headers: HeaderMap,
) -> Result<Json<RegisterResponse>, WalletError> {
    // Stricter rate limit for registration (10/min per IP, on top of global 100/min)
    let ip = extract_client_ip(&headers);
    state
        .register_rate_limiter
        .check(&ip)
        .await
        .map_err(|_| WalletError::RateLimited)?;

    let wallet_id = Uuid::new_v4().to_string();
    let api_key = format!("wk_{}", hex::encode(rand::random::<[u8; 32]>()));

    let key_hash = {
        let mut hasher = Sha256::new();
        hasher.update(api_key.as_bytes());
        hex::encode(hasher.finalize())
    };

    // Insert account + API key atomically
    let mut tx = state.db.begin().await
        .map_err(|e| WalletError::InternalError(format!("DB error: {}", e)))?;

    sqlx::query(
        "INSERT INTO wallet_accounts (wallet_id) VALUES ($1)",
    )
    .bind(&wallet_id)
    .execute(&mut *tx)
    .await
    .map_err(|e| WalletError::InternalError(format!("DB error: {}", e)))?;

    sqlx::query(
        "INSERT INTO wallet_api_keys (key_hash, wallet_id, label) VALUES ($1, $2, 'primary')",
    )
    .bind(&key_hash)
    .bind(&wallet_id)
    .execute(&mut *tx)
    .await
    .map_err(|e| WalletError::InternalError(format!("DB error: {}", e)))?;

    tx.commit().await
        .map_err(|e| WalletError::InternalError(format!("DB error: {}", e)))?;

    // Derive NEAR address from keystore and save near_pubkey for policy lookups
    let near_account_id = match keystore_derive_address(&state, &wallet_id, "near").await {
        Ok(resp) => {
            // Save near_pubkey so policy checks work immediately (without GET /address first)
            if !resp.public_key.is_empty() {
                let _ = sqlx::query(
                    "UPDATE wallet_accounts SET near_pubkey = $1 WHERE wallet_id = $2",
                )
                .bind(&resp.public_key)
                .bind(&wallet_id)
                .execute(&state.db)
                .await;
            }
            resp.address
        }
        Err(e) => {
            warn!("Failed to derive NEAR address during registration: {:?}", e);
            String::new()
        }
    };

    let handoff_url = format!(
        "https://outlayer.fastnear.com/wallet?key={}",
        api_key
    );

    // Log only masked key
    let masked = mask_api_key(&api_key);
    info!("New wallet registered: wallet_id={}, near_account_id={}, api_key={}", wallet_id, near_account_id, masked);

    Ok(Json(RegisterResponse {
        wallet_id,
        api_key,
        near_account_id,
        handoff_url,
    }))
}

// API key management endpoints removed — keys are now controlled by on-chain policy
// (authorized_key_hashes inside encrypted policy, synced via WalletPolicyUpdated SystemEvent)

/// Mask API key for logging: show "wk_ab...ef12" (prefix + first 2 hex + last 4)
fn mask_api_key(key: &str) -> String {
    if key.len() > 11 {
        format!("{}...{}", &key[..5], &key[key.len() - 4..])
    } else {
        "wk_***".to_string()
    }
}

/// Helper: call keystore derive-address
async fn keystore_derive_address(
    state: &WalletState,
    wallet_id: &str,
    chain: &str,
) -> Result<KeystoreDeriveAddressResponse, WalletError> {
    let keystore_url = state.keystore_base_url.as_ref().ok_or_else(|| {
        WalletError::InternalError("Keystore URL not configured".to_string())
    })?;

    let payload = KeystoreDeriveAddressRequest {
        wallet_id: wallet_id.to_string(),
        chain: chain.to_string(),
    };

    let mut request = state
        .http_client
        .post(format!("{}/wallet/derive-address", keystore_url))
        .json(&payload);

    if let Some(ref token) = state.keystore_auth_token {
        request = request.header("Authorization", format!("Bearer {}", token));
    }

    let response = request.send().await.map_err(|e| {
        WalletError::KeystoreError(format!("Failed to connect to keystore: {}", e))
    })?;

    if !response.status().is_success() {
        let body = response.text().await.unwrap_or_default();
        return Err(WalletError::KeystoreError(format!(
            "Keystore derive-address failed: {}",
            body
        )));
    }

    response.json().await.map_err(|e| {
        WalletError::KeystoreError(format!("Invalid keystore response: {}", e))
    })
}

/// Helper: call keystore sign-nep413 (for NEAR Intents protocol)
async fn keystore_sign_nep413(
    state: &WalletState,
    wallet_id: &str,
    chain: &str,
    message: &str,
    nonce_base64: &str,
    recipient: &str,
    approval_info: Option<&ApprovalInfo>,
) -> Result<KeystoreSignNep413Response, WalletError> {
    let keystore_url = state.keystore_base_url.as_ref().ok_or_else(|| {
        WalletError::InternalError("Keystore URL not configured".to_string())
    })?;

    let payload = KeystoreSignNep413Request {
        wallet_id: wallet_id.to_string(),
        chain: chain.to_string(),
        message: message.to_string(),
        nonce_base64: nonce_base64.to_string(),
        recipient: recipient.to_string(),
        approval_info: approval_info.cloned(),
    };

    let mut request = state
        .http_client
        .post(format!("{}/wallet/sign-nep413", keystore_url))
        .json(&payload);

    if let Some(ref token) = state.keystore_auth_token {
        request = request.header("Authorization", format!("Bearer {}", token));
    }

    let response = request.send().await.map_err(|e| {
        WalletError::KeystoreError(format!("Failed to connect to keystore: {}", e))
    })?;

    if !response.status().is_success() {
        let body = response.text().await.unwrap_or_default();
        return Err(WalletError::KeystoreError(format!(
            "Keystore sign-nep413 failed: {}",
            body
        )));
    }

    response.json().await.map_err(|e| {
        WalletError::KeystoreError(format!("Invalid keystore response: {}", e))
    })
}

/// Helper: call keystore encrypt-policy
async fn keystore_encrypt_policy(
    state: &WalletState,
    wallet_id: &str,
    policy_json: &str,
) -> Result<KeystoreEncryptPolicyInnerResponse, WalletError> {
    let keystore_url = state.keystore_base_url.as_ref().ok_or_else(|| {
        WalletError::InternalError("Keystore URL not configured".to_string())
    })?;

    let payload = KeystoreEncryptPolicyRequest {
        wallet_id: wallet_id.to_string(),
        policy_json: policy_json.to_string(),
    };

    let mut request = state
        .http_client
        .post(format!("{}/wallet/encrypt-policy", keystore_url))
        .json(&payload);

    if let Some(ref token) = state.keystore_auth_token {
        request = request.header("Authorization", format!("Bearer {}", token));
    }

    let response = request.send().await.map_err(|e| {
        WalletError::KeystoreError(format!("Failed to connect to keystore: {}", e))
    })?;

    if !response.status().is_success() {
        let body = response.text().await.unwrap_or_default();
        return Err(WalletError::KeystoreError(format!(
            "Keystore encrypt-policy failed: {}",
            body
        )));
    }

    response.json().await.map_err(|e| {
        WalletError::KeystoreError(format!("Invalid keystore response: {}", e))
    })
}

/// Helper: record usage in DB
async fn record_usage(
    db: &sqlx::PgPool,
    wallet_id: &str,
    token: &str,
    amount: &str,
) -> Result<(), WalletError> {
    let now = Utc::now();
    let daily_period = format!("daily:{}", now.format("%Y-%m-%d"));
    let hourly_period = format!("hourly:{}", now.format("%Y-%m-%dT%H"));
    let monthly_period = format!("monthly:{}", now.format("%Y-%m"));

    for period in &[&daily_period, &hourly_period, &monthly_period] {
        sqlx::query(
            r#"
            INSERT INTO wallet_usage (wallet_id, token, period, total_amount, tx_count)
            VALUES ($1, $2, $3, $4, 1)
            ON CONFLICT (wallet_id, token, period)
            DO UPDATE SET
                total_amount = (wallet_usage.total_amount::numeric + $4::numeric)::text,
                tx_count = wallet_usage.tx_count + 1
            "#,
        )
        .bind(wallet_id)
        .bind(token)
        .bind(*period)
        .bind(amount)
        .execute(db)
        .await
        .map_err(|e| WalletError::InternalError(format!("Usage recording failed: {}", e)))?;
    }

    Ok(())
}

/// Get current usage for a wallet (passed to keystore for velocity limit checks)
async fn get_current_usage(
    db: &sqlx::PgPool,
    wallet_id: &str,
) -> Result<serde_json::Value, WalletError> {
    let now = Utc::now();
    let daily_period = format!("daily:{}", now.format("%Y-%m-%d"));
    let hourly_period = format!("hourly:{}", now.format("%Y-%m-%dT%H"));
    let monthly_period = format!("monthly:{}", now.format("%Y-%m"));

    let rows = sqlx::query_as::<_, (String, String, String, i32)>(
        "SELECT token, period, total_amount, tx_count FROM wallet_usage WHERE wallet_id = $1 AND (period = $2 OR period = $3 OR period = $4)",
    )
    .bind(wallet_id)
    .bind(&daily_period)
    .bind(&hourly_period)
    .bind(&monthly_period)
    .fetch_all(db)
    .await
    .map_err(|e| WalletError::InternalError(format!("Usage query failed: {}", e)))?;

    let mut daily = serde_json::Map::new();
    let mut hourly = serde_json::Map::new();
    let mut monthly = serde_json::Map::new();
    let mut hourly_tx_count = 0i32;

    for (token, period, amount, tx_count) in rows {
        if period.starts_with("daily:") {
            daily.insert(token, serde_json::Value::String(amount));
        } else if period.starts_with("hourly:") {
            hourly.insert(token, serde_json::Value::String(amount));
            hourly_tx_count += tx_count;
        } else if period.starts_with("monthly:") {
            monthly.insert(token, serde_json::Value::String(amount));
        }
    }

    Ok(serde_json::json!({
        "daily": daily,
        "hourly": hourly,
        "monthly": monthly,
        "hourly_tx_count": hourly_tx_count,
    }))
}

// ============================================================================
// GET /address?chain={chain}
// ============================================================================
pub async fn get_address(
    State(state): State<WalletState>,
    headers: HeaderMap,
    Query(query): Query<AddressQuery>,
) -> Result<Json<AddressResponse>, WalletError> {
    let auth = authenticate(&headers, &state.allowed_worker_token_hashes, &state.db, &state.api_key_cache).await?;

    // Validate chain
    let chain = query.chain.to_lowercase();
    validate_chain(&chain)?;

    let derived = keystore_derive_address(&state, &auth.wallet_id, &chain).await?;

    // Save near_pubkey for dashboard lookup (idempotent)
    if chain == "near" && !derived.public_key.is_empty() {
        let _ = sqlx::query(
            "UPDATE wallet_accounts SET near_pubkey = $1 WHERE wallet_id = $2 AND near_pubkey IS NULL",
        )
        .bind(&derived.public_key)
        .bind(&auth.wallet_id)
        .execute(&state.db)
        .await;
    }

    Ok(Json(AddressResponse {
        wallet_id: auth.wallet_id,
        chain,
        address: derived.address,
    }))
}

// ============================================================================
// GET /tokens
// ============================================================================
pub async fn get_tokens(
    State(state): State<WalletState>,
    headers: HeaderMap,
) -> Result<Json<TokensResponse>, WalletError> {
    let _auth = authenticate(&headers, &state.allowed_worker_token_hashes, &state.db, &state.api_key_cache).await?;

    let tokens = state.backend.list_tokens().await.map_err(|e| {
        WalletError::InternalError(format!("Failed to list tokens: {}", e))
    })?;

    Ok(Json(TokensResponse {
        tokens: tokens
            .into_iter()
            .map(|t| TokenInfo {
                id: t.id,
                symbol: t.symbol,
                chains: t.chains,
            })
            .collect(),
    }))
}

// ============================================================================
// POST /withdraw
// ============================================================================
pub async fn withdraw(
    State(state): State<WalletState>,
    headers: HeaderMap,
    body: String,
) -> Result<Json<WithdrawResponse>, WalletError> {
    let auth = authenticate(&headers, &state.allowed_worker_token_hashes, &state.db, &state.api_key_cache).await?;

    // Idempotency key: use provided header, or generate random UUID if absent
    let idempotency_key = if auth.is_internal {
        auth.idempotency_key.clone()
    } else {
        Some(auth.idempotency_key.clone()
            .unwrap_or_else(|| uuid::Uuid::new_v4().to_string()))
    };

    // Check idempotency
    if let Some(ref key) = idempotency_key {
        if let Some(existing_id) = idempotency::check_idempotency(&state.db, &auth.wallet_id, key).await? {
            return Err(WalletError::DuplicateIdempotencyKey {
                request_id: existing_id,
            });
        }
    }

    let req: WithdrawRequest =
        serde_json::from_str(&body).map_err(|e| WalletError::InternalError(format!("Invalid request body: {}", e)))?;

    let chain = req.chain.to_lowercase();
    validate_chain(&chain)?;

    let token_key = req.token.as_deref().unwrap_or("native").to_string();

    // Get current usage for velocity limit checks
    let current_usage = get_current_usage(&state.db, &auth.wallet_id).await.unwrap_or_default();

    // Check policy
    let action = serde_json::json!({
        "type": "withdraw",
        "chain": chain,
        "to": req.to,
        "amount": req.amount,
        "token": token_key,
        "current_usage": current_usage,
    });

    let wallet_pubkey = policy::resolve_wallet_pubkey(&state.db, &auth.wallet_id).await?;
    let policy_output = policy::check_wallet_policy_with_overrides(
        &state.policy_cache,
        &state.near_rpc_url,
        &state.contract_id,
        state.keystore_base_url.as_deref().unwrap_or(""),
        state.keystore_auth_token.as_deref().unwrap_or(""),
        &auth.wallet_id,
        &wallet_pubkey,
        action,
        Some(&state.policy_overrides),
    )
    .await?;

    let webhook_url = policy_output.webhook_url.clone();

    match policy_output.decision {
        PolicyDecision::Frozen => return Err(WalletError::WalletFrozen),
        PolicyDecision::Denied(reason) => return Err(WalletError::PolicyDenied(reason)),
        PolicyDecision::RequiresApproval { required_approvals } => {
            // Create pending approval
            let request_id = Uuid::new_v4();
            let approval_id = Uuid::new_v4();

            let request_data = serde_json::json!({
                "chain": chain,
                "to": req.to,
                "amount": req.amount,
                "token": token_key,
            });

            let request_hash = sha256_hex(&serde_json::to_string(&request_data).unwrap());

            // Insert approval
            sqlx::query(
                r#"
                INSERT INTO wallet_pending_approvals (id, wallet_id, request_type, request_data, request_hash, required_approvals, expires_at)
                VALUES ($1, $2, 'withdraw', $3, $4, $5, NOW() + INTERVAL '24 hours')
                "#,
            )
            .bind(approval_id)
            .bind(&auth.wallet_id)
            .bind(&request_data)
            .bind(&request_hash)
            .bind(required_approvals)
            .execute(&state.db)
            .await
            .map_err(|e| WalletError::InternalError(format!("DB error: {}", e)))?;

            // Insert request
            sqlx::query(
                r#"
                INSERT INTO wallet_requests (request_id, wallet_id, request_type, chain, request_data, approval_id, status, idempotency_key)
                VALUES ($1, $2, 'withdraw', $3, $4, $5, 'pending_approval', $6)
                "#,
            )
            .bind(request_id)
            .bind(&auth.wallet_id)
            .bind(&chain)
            .bind(&request_data)
            .bind(approval_id)
            .bind(&idempotency_key)
            .execute(&state.db)
            .await
            .map_err(|e| WalletError::InternalError(format!("DB error: {}", e)))?;

            // Audit log
            audit::record_audit_event(
                &state.db,
                &auth.wallet_id,
                "withdraw_pending_approval",
                &auth.wallet_id,
                serde_json::json!({"to": req.to, "amount": req.amount, "chain": chain}),
                Some(&request_id.to_string()),
            )
            .await;

            // Webhook: approval_needed
            if let Some(ref url) = webhook_url {
                let _ = webhooks::enqueue_webhook(
                    &state.db,
                    &auth.wallet_id,
                    "approval_needed",
                    serde_json::json!({
                        "request_id": request_id.to_string(),
                        "approval_id": approval_id.to_string(),
                        "type": "withdraw",
                        "request_data": request_data,
                        "required_approvals": required_approvals,
                    }),
                    url,
                )
                .await;
            }

            // Record usage even for pending requests (enforces rate limits)
            record_usage(&state.db, &auth.wallet_id, &token_key, &req.amount).await?;

            return Ok(Json(WithdrawResponse {
                request_id: request_id.to_string(),
                status: "pending_approval".to_string(),
                approval_id: Some(approval_id.to_string()),
                required: Some(required_approvals),
                approved: Some(0),
            }));
        }
        PolicyDecision::NoPolicyAllow | PolicyDecision::Allowed => {
            // Proceed with withdrawal
        }
    }

    // Acquire nonce lock for this wallet
    let nonce_lock = state.nonce_locks.get_lock(&auth.wallet_id).await;
    let _nonce_guard = nonce_lock.lock().await;

    // Derive chain address (= signer_id for intents)
    let derived = keystore_derive_address(&state, &auth.wallet_id, &chain).await?;

    // Strip nep141: prefix from token if present (intents ft_withdraw uses plain token ID)
    let token_for_intent = req.token.as_deref().unwrap_or("native").to_string();
    let token_for_intent = token_for_intent
        .strip_prefix("nep141:")
        .unwrap_or(&token_for_intent)
        .to_string();

    // Build NEP-413 intent message for ft_withdraw
    let deadline = (Utc::now() + chrono::Duration::seconds(180))
        .format("%Y-%m-%dT%H:%M:%S%.3fZ")
        .to_string();

    let intent_message = serde_json::json!({
        "signer_id": derived.address,
        "deadline": deadline,
        "intents": [{
            "intent": "ft_withdraw",
            "token": token_for_intent,
            "receiver_id": req.to,
            "amount": req.amount,
        }]
    });

    // Serialize with spaces after colons (intents protocol format, matches intents-ark)
    let message_str = serde_json::to_string(&intent_message)
        .map_err(|e| WalletError::InternalError(format!("Failed to serialize intent: {}", e)))?
        .replace("\":", "\": ");

    // Generate nonce: SHA256(timestamp_nanos) -> base64
    let nonce_base64 = {
        let timestamp_nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
            .to_string();
        let hash = Sha256::digest(timestamp_nanos.as_bytes());
        base64::Engine::encode(&base64::engine::general_purpose::STANDARD, hash)
    };

    // Sign intent via keystore NEP-413 endpoint
    let sign_result = keystore_sign_nep413(
        &state,
        &auth.wallet_id,
        &chain,
        &message_str,
        &nonce_base64,
        "intents.near",
        None,
    )
    .await?;

    // Package signed intent data for backend
    let signed_intent = serde_json::json!({
        "message": message_str,
        "nonce": nonce_base64,
        "recipient": "intents.near",
        "signature": sign_result.signature_base58,
        "public_key": sign_result.public_key,
        "standard": "nep413",
    });

    let signed_intent_base64 = base64::Engine::encode(
        &base64::engine::general_purpose::STANDARD,
        serde_json::to_string(&signed_intent).unwrap().as_bytes(),
    );

    // Create request record
    let request_id = Uuid::new_v4();
    let request_data = serde_json::json!({
        "chain": chain,
        "to": req.to,
        "amount": req.amount,
        "token": token_key,
    });

    sqlx::query(
        r#"
        INSERT INTO wallet_requests (request_id, wallet_id, request_type, chain, request_data, status, idempotency_key)
        VALUES ($1, $2, 'withdraw', $3, $4, 'processing', $5)
        "#,
    )
    .bind(request_id)
    .bind(&auth.wallet_id)
    .bind(&chain)
    .bind(&request_data)
    .bind(&idempotency_key)
    .execute(&state.db)
    .await
    .map_err(|e| WalletError::InternalError(format!("DB error: {}", e)))?;

    // Submit to backend (solver-relay)
    let wallet_info = WalletInfo {
        wallet_id: auth.wallet_id.clone(),
        chain: chain.clone(),
        chain_address: derived.address.clone(),
        chain_public_key: derived.public_key.clone(),
    };

    let backend_req = BackendWithdrawRequest {
        to: req.to.clone(),
        amount: req.amount.clone(),
        token: req.token.clone(),
        chain: chain.clone(),
    };

    let (status, result_data, operation_id) = match state
        .backend
        .withdraw(&wallet_info, backend_req, &signed_intent_base64)
        .await
    {
        Ok(result) => {
            let status = if result.status == "success" {
                "success"
            } else {
                "processing"
            };
            let data = serde_json::json!({
                "intent_hash": result.operation_id,
                "tx_hash": result.tx_hash,
                "fee": result.fee,
                "fee_token": result.fee_token,
            });
            (status.to_string(), data, Some(result.operation_id))
        }
        Err(e) => {
            let full_error = format!("{:#}", e);
            warn!("Withdraw backend error for request {}: {}", request_id, full_error);
            let data = serde_json::json!({
                "error": full_error,
            });
            ("failed".to_string(), data, None)
        }
    };

    // Update request with result
    sqlx::query(
        "UPDATE wallet_requests SET status = $2, intents_ref = $3, result_data = $4, updated_at = NOW() WHERE request_id = $1",
    )
    .bind(request_id)
    .bind(&status)
    .bind(&operation_id)
    .bind(&result_data)
    .execute(&state.db)
    .await
    .map_err(|e| WalletError::InternalError(format!("DB error: {}", e)))?;

    // Record usage only for non-failed operations (failed withdrawals should
    // not count against velocity limits)
    if status != "failed" {
        record_usage(&state.db, &auth.wallet_id, &token_key, &req.amount).await?;
    }

    // Audit log
    audit::record_audit_event(
        &state.db,
        &auth.wallet_id,
        "withdraw",
        &auth.wallet_id,
        serde_json::json!({"to": req.to, "amount": req.amount, "chain": chain, "status": status}),
        Some(&request_id.to_string()),
    )
    .await;

    // Webhook: request_completed
    if let Some(ref url) = webhook_url {
        let _ = webhooks::enqueue_webhook(
            &state.db,
            &auth.wallet_id,
            "request_completed",
            serde_json::json!({
                "request_id": request_id.to_string(),
                "type": "withdraw",
                "status": status,
                "result": result_data,
            }),
            url,
        )
        .await;
    }

    Ok(Json(WithdrawResponse {
        request_id: request_id.to_string(),
        status,
        approval_id: None,
        required: None,
        approved: None,
    }))
}

// ============================================================================
// POST /withdraw/dry-run
// ============================================================================
pub async fn withdraw_dry_run(
    State(state): State<WalletState>,
    headers: HeaderMap,
    body: String,
) -> Result<Json<DryRunResponse>, WalletError> {
    let auth = authenticate(&headers, &state.allowed_worker_token_hashes, &state.db, &state.api_key_cache).await?;

    let req: WithdrawRequest =
        serde_json::from_str(&body).map_err(|e| WalletError::InternalError(format!("Invalid request body: {}", e)))?;

    let chain = req.chain.to_lowercase();
    validate_chain(&chain)?;

    let token_key = req.token.as_deref().unwrap_or("native").to_string();

    // Get current usage for velocity limit checks
    let current_usage = get_current_usage(&state.db, &auth.wallet_id).await.unwrap_or_default();

    // Check policy
    let action = serde_json::json!({
        "type": "withdraw",
        "chain": chain,
        "to": req.to,
        "amount": req.amount,
        "token": token_key,
        "dry_run": true,
        "current_usage": current_usage,
    });

    let wallet_pubkey = policy::resolve_wallet_pubkey(&state.db, &auth.wallet_id).await?;
    let policy_output = policy::check_wallet_policy_with_overrides(
        &state.policy_cache,
        &state.near_rpc_url,
        &state.contract_id,
        state.keystore_base_url.as_deref().unwrap_or(""),
        state.keystore_auth_token.as_deref().unwrap_or(""),
        &auth.wallet_id,
        &wallet_pubkey,
        action,
        Some(&state.policy_overrides),
    )
    .await?;

    match policy_output.decision {
        PolicyDecision::Frozen => {
            return Ok(Json(DryRunResponse {
                would_succeed: false,
                reason: Some("wallet_frozen".to_string()),
                message: Some("Wallet is frozen by controller".to_string()),
                estimated_fee: None,
                fee_token: None,
                policy_check: None,
            }));
        }
        PolicyDecision::Denied(reason) => {
            return Ok(Json(DryRunResponse {
                would_succeed: false,
                reason: Some("policy_denied".to_string()),
                message: Some(reason),
                estimated_fee: None,
                fee_token: None,
                policy_check: Some(PolicyCheckResult {
                    within_limits: false,
                    daily_remaining: None,
                    address_allowed: None,
                }),
            }));
        }
        PolicyDecision::RequiresApproval { .. } => {
            return Ok(Json(DryRunResponse {
                would_succeed: true,
                reason: None,
                message: Some("Operation would require multisig approval".to_string()),
                estimated_fee: Some("0.002".to_string()),
                fee_token: Some("NEAR".to_string()),
                policy_check: Some(PolicyCheckResult {
                    within_limits: true,
                    daily_remaining: None,
                    address_allowed: Some(true),
                }),
            }));
        }
        PolicyDecision::NoPolicyAllow | PolicyDecision::Allowed => {
            return Ok(Json(DryRunResponse {
                would_succeed: true,
                reason: None,
                message: None,
                estimated_fee: Some("0.002".to_string()),
                fee_token: Some("NEAR".to_string()),
                policy_check: Some(PolicyCheckResult {
                    within_limits: true,
                    daily_remaining: None,
                    address_allowed: Some(true),
                }),
            }));
        }
    }
}

// ============================================================================
// POST /deposit
// ============================================================================
pub async fn deposit(
    State(state): State<WalletState>,
    headers: HeaderMap,
    body: String,
) -> Result<Json<DepositResponse>, WalletError> {
    let auth = authenticate(&headers, &state.allowed_worker_token_hashes, &state.db, &state.api_key_cache).await?;

    let idempotency_key = auth
        .idempotency_key
        .clone()
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());

    // Check idempotency
    if let Some(existing_id) = idempotency::check_idempotency(&state.db, &auth.wallet_id, &idempotency_key).await? {
        return Err(WalletError::DuplicateIdempotencyKey {
            request_id: existing_id,
        });
    }

    let req: DepositRequest =
        serde_json::from_str(&body).map_err(|e| WalletError::InternalError(format!("Invalid request body: {}", e)))?;

    // For deposits, we need the destination address on the target chain (NEAR by default)
    let target_chain = "near";
    let derived = keystore_derive_address(&state, &auth.wallet_id, target_chain).await?;

    let wallet_info = WalletInfo {
        wallet_id: auth.wallet_id.clone(),
        chain: target_chain.to_string(),
        chain_address: derived.address.clone(),
        chain_public_key: derived.public_key.clone(),
    };

    let backend_req = BackendDepositRequest {
        source_chain: req.source_chain.clone(),
        token: req.token.clone(),
        amount: req.amount.clone(),
        destination_address: derived.address.clone(),
    };

    // Try to get deposit quote from backend; fallback to direct deposit address
    let (deposit_address, deposit_chain, expires_at, intents_ref) = match state
        .backend
        .deposit_quote(&wallet_info, backend_req)
        .await
    {
        Ok(result) => (
            result.deposit_address,
            result.chain,
            result.expires_at,
            Some(result.operation_id),
        ),
        Err(e) => {
            // Backend unavailable — use wallet's own address for direct deposit
            warn!("Deposit quote backend unavailable, using direct address: {}", e);
            (
                derived.address.clone(),
                req.source_chain.clone(),
                None,
                None,
            )
        }
    };

    // Create request record
    let request_id = Uuid::new_v4();
    let request_data = serde_json::json!({
        "source_chain": req.source_chain,
        "token": req.token,
        "amount": req.amount,
        "deposit_address": deposit_address,
    });

    sqlx::query(
        r#"
        INSERT INTO wallet_requests (request_id, wallet_id, request_type, chain, request_data, intents_ref, status, idempotency_key)
        VALUES ($1, $2, 'deposit', $3, $4, $5, 'pending_deposit', $6)
        "#,
    )
    .bind(request_id)
    .bind(&auth.wallet_id)
    .bind(&req.source_chain)
    .bind(&request_data)
    .bind(&intents_ref)
    .bind(idempotency_key)
    .execute(&state.db)
    .await
    .map_err(|e| WalletError::InternalError(format!("DB error: {}", e)))?;

    // Audit log
    audit::record_audit_event(
        &state.db,
        &auth.wallet_id,
        "deposit",
        &auth.wallet_id,
        serde_json::json!({"source_chain": req.source_chain, "token": req.token, "amount": req.amount}),
        Some(&request_id.to_string()),
    )
    .await;

    Ok(Json(DepositResponse {
        request_id: request_id.to_string(),
        status: "pending_deposit".to_string(),
        deposit_address,
        chain: deposit_chain,
        expires_at,
    }))
}

// ============================================================================
// GET /requests/{request_id}
// ============================================================================
pub async fn get_request_status(
    State(state): State<WalletState>,
    headers: HeaderMap,
    Path(request_id): Path<String>,
) -> Result<Json<RequestStatusResponse>, WalletError> {
    let auth = authenticate(&headers, &state.allowed_worker_token_hashes, &state.db, &state.api_key_cache).await?;

    let request_uuid: Uuid = request_id
        .parse()
        .map_err(|_| WalletError::RequestNotFound)?;

    let row = sqlx::query_as::<_, RequestRow>(
        r#"
        SELECT request_id, wallet_id, request_type, status, result_data, intents_ref, created_at, updated_at
        FROM wallet_requests
        WHERE request_id = $1 AND wallet_id = $2
        "#,
    )
    .bind(request_uuid)
    .bind(&auth.wallet_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| WalletError::InternalError(format!("DB error: {}", e)))?
    .ok_or(WalletError::RequestNotFound)?;

    // If still processing, check backend for updated status
    if row.status == "processing" {
        if let Some(ref intents_ref) = row.intents_ref {
            if let Ok(op_status) = state.backend.operation_status(intents_ref).await {
                if op_status.status != "processing" {
                    let new_status = &op_status.status;
                    let result_data = serde_json::json!({
                        "tx_hash": op_status.tx_hash,
                        "fee": op_status.fee,
                        "fee_token": op_status.fee_token,
                        "error": op_status.error,
                    });

                    let _ = sqlx::query(
                        "UPDATE wallet_requests SET status = $2, result_data = $3, updated_at = NOW() WHERE request_id = $1",
                    )
                    .bind(request_uuid)
                    .bind(new_status)
                    .bind(&result_data)
                    .execute(&state.db)
                    .await;

                    return Ok(Json(RequestStatusResponse {
                        request_id: row.request_id.to_string(),
                        r#type: row.request_type,
                        status: new_status.clone(),
                        result: Some(result_data),
                        created_at: row.created_at.to_rfc3339(),
                        updated_at: Some(Utc::now().to_rfc3339()),
                    }));
                }
            }
        }
    }

    Ok(Json(RequestStatusResponse {
        request_id: row.request_id.to_string(),
        r#type: row.request_type,
        status: row.status,
        result: row.result_data,
        created_at: row.created_at.to_rfc3339(),
        updated_at: row.updated_at.map(|t| t.to_rfc3339()),
    }))
}

// ============================================================================
// GET /requests
// ============================================================================
pub async fn list_requests(
    State(state): State<WalletState>,
    headers: HeaderMap,
    Query(query): Query<RequestsQuery>,
) -> Result<Json<RequestListResponse>, WalletError> {
    let auth = authenticate(&headers, &state.allowed_worker_token_hashes, &state.db, &state.api_key_cache).await?;

    let limit = query.limit.min(100).max(1);
    let offset = query.offset.max(0);

    // Validate filter values against whitelist to prevent SQL injection
    const ALLOWED_TYPES: &[&str] = &["withdraw", "deposit", "call"];
    const ALLOWED_STATUSES: &[&str] = &[
        "processing", "success", "failed", "pending_approval",
        "approved", "rejected", "pending_deposit",
    ];

    if let Some(ref t) = query.r#type {
        if !ALLOWED_TYPES.contains(&t.as_str()) {
            return Err(WalletError::InternalError(format!(
                "Invalid request type filter: '{}'. Allowed: {:?}", t, ALLOWED_TYPES
            )));
        }
    }
    if let Some(ref s) = query.status {
        if !ALLOWED_STATUSES.contains(&s.as_str()) {
            return Err(WalletError::InternalError(format!(
                "Invalid status filter: '{}'. Allowed: {:?}", s, ALLOWED_STATUSES
            )));
        }
    }

    // Build query with fully parameterized bindings (limit/offset included as params)
    let (type_filter, status_filter) = (query.r#type.is_some(), query.status.is_some());

    // Compute param positions: $1=wallet_id, optionally $2/$3 for type/status, then limit/offset
    let (base_where, count_where, limit_param, offset_param) = match (type_filter, status_filter) {
        (true, true)   => (" AND request_type = $2 AND status = $3", " AND request_type = $2 AND status = $3", "$4", "$5"),
        (true, false)  => (" AND request_type = $2", " AND request_type = $2", "$3", "$4"),
        (false, true)  => (" AND status = $2", " AND status = $2", "$3", "$4"),
        (false, false) => ("", "", "$2", "$3"),
    };

    let sql = format!(
        "SELECT request_id, wallet_id, request_type, status, result_data, intents_ref, created_at, updated_at \
         FROM wallet_requests WHERE wallet_id = $1{} ORDER BY created_at DESC LIMIT {} OFFSET {}",
        base_where, limit_param, offset_param
    );
    let count_sql = format!(
        "SELECT COUNT(*) FROM wallet_requests WHERE wallet_id = $1{}", count_where
    );

    // Bind params dynamically (limit/offset as i64 params instead of format! interpolation)
    let limit_i64 = limit as i64;
    let offset_i64 = offset as i64;

    let rows: Vec<RequestRow> = match (query.r#type.as_deref(), query.status.as_deref()) {
        (Some(t), Some(s)) => {
            sqlx::query_as(&sql).bind(&auth.wallet_id).bind(t).bind(s).bind(limit_i64).bind(offset_i64)
                .fetch_all(&state.db).await
        }
        (Some(t), None) => {
            sqlx::query_as(&sql).bind(&auth.wallet_id).bind(t).bind(limit_i64).bind(offset_i64)
                .fetch_all(&state.db).await
        }
        (None, Some(s)) => {
            sqlx::query_as(&sql).bind(&auth.wallet_id).bind(s).bind(limit_i64).bind(offset_i64)
                .fetch_all(&state.db).await
        }
        (None, None) => {
            sqlx::query_as(&sql).bind(&auth.wallet_id).bind(limit_i64).bind(offset_i64)
                .fetch_all(&state.db).await
        }
    }.map_err(|e| WalletError::InternalError(format!("DB error: {}", e)))?;

    let total: (i64,) = match (query.r#type.as_deref(), query.status.as_deref()) {
        (Some(t), Some(s)) => {
            sqlx::query_as(&count_sql).bind(&auth.wallet_id).bind(t).bind(s)
                .fetch_one(&state.db).await
        }
        (Some(t), None) => {
            sqlx::query_as(&count_sql).bind(&auth.wallet_id).bind(t)
                .fetch_one(&state.db).await
        }
        (None, Some(s)) => {
            sqlx::query_as(&count_sql).bind(&auth.wallet_id).bind(s)
                .fetch_one(&state.db).await
        }
        (None, None) => {
            sqlx::query_as(&count_sql).bind(&auth.wallet_id)
                .fetch_one(&state.db).await
        }
    }.map_err(|e| WalletError::InternalError(format!("DB error: {}", e)))?;

    Ok(Json(RequestListResponse {
        requests: rows
            .into_iter()
            .map(|r| RequestStatusResponse {
                request_id: r.request_id.to_string(),
                r#type: r.request_type,
                status: r.status,
                result: r.result_data,
                created_at: r.created_at.to_rfc3339(),
                updated_at: r.updated_at.map(|t| t.to_rfc3339()),
            })
            .collect(),
        total: total.0,
    }))
}

// ============================================================================
// POST /encrypt-policy
// ============================================================================
pub async fn encrypt_policy(
    State(state): State<WalletState>,
    body: String,
) -> Result<Json<EncryptPolicyResponse>, WalletError> {
    // Public endpoint — encrypts policy JSON via keystore (pure data transform).
    // The encrypted result is useless without sign-policy (which requires auth)
    // and store_wallet_policy on-chain (which requires NEAR wallet signature).
    // Rate-limited by the global IP-based rate limiter.

    let req: EncryptPolicyRequest =
        serde_json::from_str(&body).map_err(|e| WalletError::InternalError(format!("Invalid request body: {}", e)))?;

    // Build canonical policy JSON
    let policy = serde_json::json!({
        "version": 1,
        "frozen": false,
        "rules": req.rules,
        "approval": req.approval,
        "admin_quorum": req.admin_quorum,
        "webhook_url": req.webhook_url,
        "authorized_key_hashes": req.authorized_key_hashes,
    });

    let policy_json = serde_json::to_string(&policy)
        .map_err(|e| WalletError::InternalError(format!("JSON serialization error: {}", e)))?;

    // Encrypt via keystore
    let encrypted = keystore_encrypt_policy(&state, &req.wallet_id, &policy_json).await?;

    info!(
        "Policy encrypted for wallet={}, encrypted_len={}",
        req.wallet_id,
        encrypted.encrypted_base64.len()
    );

    Ok(Json(EncryptPolicyResponse {
        encrypted_base64: encrypted.encrypted_base64,
        wallet_pubkey: req.wallet_id,
    }))
}

// ============================================================================
// POST /sign-policy — sign encrypted_data with wallet's ed25519 key (for on-chain store_wallet_policy)
// ============================================================================
pub async fn sign_policy(
    State(state): State<WalletState>,
    headers: HeaderMap,
    Json(req): Json<SignPolicyRequest>,
) -> Result<Json<KeystoreSignPolicyResponse>, WalletError> {
    use sha2::{Sha256, Digest};

    let auth = authenticate(&headers, &state.allowed_worker_token_hashes, &state.db, &state.api_key_cache).await?;

    // Compute SHA256 of the encrypted_data
    let mut hasher = Sha256::new();
    hasher.update(req.encrypted_data.as_bytes());
    let hash_hex = hex::encode(hasher.finalize());

    let keystore_url = state.keystore_base_url.as_ref().ok_or_else(|| {
        WalletError::InternalError("Keystore URL not configured".to_string())
    })?;

    let payload = KeystoreSignPolicyRequest {
        wallet_id: auth.wallet_id.clone(),
        encrypted_data_hash: hash_hex,
    };

    let mut request = state
        .http_client
        .post(format!("{}/wallet/sign-policy", keystore_url))
        .json(&payload);

    if let Some(ref token) = state.keystore_auth_token {
        request = request.header("Authorization", format!("Bearer {}", token));
    }

    let response = request.send().await.map_err(|e| {
        WalletError::KeystoreError(format!("Failed to connect to keystore: {}", e))
    })?;

    if !response.status().is_success() {
        let body = response.text().await.unwrap_or_default();
        return Err(WalletError::KeystoreError(format!(
            "Keystore sign-policy failed: {}",
            body
        )));
    }

    let result: KeystoreSignPolicyResponse = response.json().await.map_err(|e| {
        WalletError::KeystoreError(format!("Invalid keystore response: {}", e))
    })?;

    info!(
        "Policy signed for wallet={}, pubkey=ed25519:{}",
        auth.wallet_id,
        &result.public_key_hex[..8]
    );

    Ok(Json(result))
}

// ============================================================================
// POST /invalidate-cache
// ============================================================================
pub async fn invalidate_cache(
    State(state): State<WalletState>,
    body: String,
) -> Result<Json<serde_json::Value>, WalletError> {
    // Public endpoint — flushes negative policy cache for a wallet_id.
    // Does not change policy or enforcement, only triggers a fresh on-chain check.
    // Rate-limited by the global IP-based rate limiter.

    let req: InvalidateCacheRequest =
        serde_json::from_str(&body).map_err(|e| WalletError::InternalError(format!("Invalid request body: {}", e)))?;

    state.policy_cache.invalidate(&req.wallet_id).await;

    // Policy sync (key hashes, frozen status) is handled by the worker's
    // on-chain event monitor via internal_wallet_policy_sync endpoint.
    // No policy_json accepted here to prevent unauthenticated key injection.

    debug!("Cache invalidated for wallet={}", req.wallet_id);

    Ok(Json(serde_json::json!({"ok": true})))
}

// ============================================================================
// GET /policy
// ============================================================================
pub async fn get_policy(
    State(state): State<WalletState>,
    headers: HeaderMap,
) -> Result<Json<PolicyResponse>, WalletError> {
    let auth = authenticate(&headers, &state.allowed_worker_token_hashes, &state.db, &state.api_key_cache).await?;

    // Read policy and frozen status from DB (synced via internal_wallet_policy_sync)
    let row = sqlx::query_as::<_, (Option<serde_json::Value>, bool)>(
        "SELECT policy_json, frozen FROM wallet_accounts WHERE wallet_id = $1",
    )
    .bind(&auth.wallet_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| WalletError::InternalError(format!("DB error: {}", e)))?
    .ok_or_else(|| WalletError::InternalError("Wallet not found".to_string()))?;

    let (mut policy_json, frozen) = row;

    // Lazy-sync: if policy_json is NULL, fetch from contract and decrypt via keystore
    if policy_json.is_none() {
        let wallet_pubkey = policy::resolve_wallet_pubkey(&state.db, &auth.wallet_id).await?;
        match policy::rpc_get_wallet_policy(&state.near_rpc_url, &state.contract_id, &wallet_pubkey).await {
            Ok(Some(on_chain)) => {
                if let Some(encrypted_data) = on_chain.get("encrypted_data").and_then(|v| v.as_str()) {
                    match keystore_decrypt_policy(&state, &auth.wallet_id, encrypted_data).await {
                        Ok(decrypted) => {
                            // Save to DB for next time
                            let _ = sqlx::query(
                                "UPDATE wallet_accounts SET policy_json = $1 WHERE wallet_id = $2",
                            )
                            .bind(&decrypted)
                            .bind(&auth.wallet_id)
                            .execute(&state.db)
                            .await;

                            info!("Lazy-synced policy for wallet={}", auth.wallet_id);
                            policy_json = Some(decrypted);
                        }
                        Err(e) => {
                            warn!("Lazy-sync decrypt failed for wallet={}: {:?}", auth.wallet_id, e);
                        }
                    }
                }
            }
            Ok(None) => { /* no policy on-chain */ }
            Err(e) => {
                warn!("Lazy-sync RPC failed for wallet={}: {}", auth.wallet_id, e);
            }
        }
    }

    // Extract fields from stored policy
    let (rules, approval, authorized_key_hashes) = match &policy_json {
        Some(policy) => {
            let rules = policy.get("rules").cloned();
            let approval = policy.get("approval").cloned();
            let key_hashes = policy
                .get("authorized_key_hashes")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(|s| s.to_string()))
                        .collect()
                });
            (rules, approval, key_hashes)
        }
        None => (None, None, None),
    };

    // Get usage data from DB
    let now = Utc::now();
    let daily_period = format!("daily:{}", now.format("%Y-%m-%d"));
    let hourly_period = format!("hourly:{}", now.format("%Y-%m-%dT%H"));

    let usage_rows = sqlx::query_as::<_, (String, String, String)>(
        "SELECT token, period, total_amount FROM wallet_usage WHERE wallet_id = $1 AND (period = $2 OR period = $3)",
    )
    .bind(&auth.wallet_id)
    .bind(&daily_period)
    .bind(&hourly_period)
    .fetch_all(&state.db)
    .await
    .map_err(|e| WalletError::InternalError(format!("DB error: {}", e)))?;

    let mut usage_map = serde_json::Map::new();
    for (token, period, amount) in usage_rows {
        let period_type = if period.starts_with("daily:") {
            "daily"
        } else {
            "hourly"
        };

        let period_entry = usage_map
            .entry(period_type.to_string())
            .or_insert_with(|| serde_json::Value::Object(serde_json::Map::new()));

        if let serde_json::Value::Object(ref mut map) = period_entry {
            map.insert(
                token,
                serde_json::json!({"spent": amount}),
            );
        }
    }

    // Resolve controller (owner) from near_pubkey
    let wallet_pubkey = policy::resolve_wallet_pubkey(&state.db, &auth.wallet_id).await?;

    Ok(Json(PolicyResponse {
        wallet_id: auth.wallet_id,
        controller: wallet_pubkey,
        frozen,
        rules,
        approval,
        authorized_key_hashes,
        usage: Some(serde_json::Value::Object(usage_map)),
    }))
}

// ============================================================================
// GET /pending_approvals
// ============================================================================
pub async fn get_pending_approvals(
    State(state): State<WalletState>,
    headers: HeaderMap,
) -> Result<Json<PendingApprovalsResponse>, WalletError> {
    let auth = authenticate(&headers, &state.allowed_worker_token_hashes, &state.db, &state.api_key_cache).await?;

    let rows = sqlx::query_as::<_, ApprovalRow>(
        r#"
        SELECT pa.id, pa.wallet_id, pa.request_type, pa.request_data,
               pa.required_approvals, pa.status, pa.expires_at, pa.created_at,
               wr.request_id as linked_request_id,
               (SELECT COUNT(*) FROM wallet_approval_signatures WHERE approval_id = pa.id)::int as approval_count
        FROM wallet_pending_approvals pa
        LEFT JOIN wallet_requests wr ON wr.approval_id = pa.id
        WHERE pa.wallet_id = $1 AND pa.status = 'pending' AND pa.expires_at > NOW()
        ORDER BY pa.created_at DESC
        "#,
    )
    .bind(&auth.wallet_id)
    .fetch_all(&state.db)
    .await
    .map_err(|e| WalletError::InternalError(format!("DB error: {}", e)))?;

    Ok(Json(PendingApprovalsResponse {
        approvals: rows
            .into_iter()
            .map(|r| PendingApprovalResponse {
                approval_id: r.id.to_string(),
                request_id: r.linked_request_id.map(|id| id.to_string()),
                r#type: r.request_type,
                request_data: r.request_data,
                required: r.required_approvals,
                approved: r.approval_count,
                expires_at: r.expires_at.to_rfc3339(),
            })
            .collect(),
    }))
}

// ============================================================================
// POST /approve/{approval_id}
// ============================================================================
pub async fn approve(
    State(state): State<WalletState>,
    Path(approval_id_str): Path<String>,
    Json(body): Json<serde_json::Value>,
) -> Result<Json<ApproveResponse>, WalletError> {
    // No API key auth — approver proves identity via NEAR wallet signature (NEP-413).
    // This prevents the agent from approving its own multisig transactions.
    let signature = body.get("signature").and_then(|v| v.as_str())
        .ok_or_else(|| WalletError::InternalError("signature is required".to_string()))?;
    let public_key = body.get("public_key").and_then(|v| v.as_str())
        .ok_or_else(|| WalletError::InternalError("public_key is required".to_string()))?;
    let account_id = body.get("account_id").and_then(|v| v.as_str())
        .ok_or_else(|| WalletError::InternalError("account_id is required".to_string()))?;
    let nonce = body.get("nonce").and_then(|v| v.as_str())
        .ok_or_else(|| WalletError::InternalError("nonce is required".to_string()))?;

    let approver_id = account_id;

    let approval_id: Uuid = approval_id_str
        .parse()
        .map_err(|_| WalletError::ApprovalNotFound)?;

    // Get the approval
    let approval = sqlx::query_as::<_, (Uuid, String, String, serde_json::Value, i32, String, String)>(
        r#"
        SELECT id, wallet_id, request_type, request_data, required_approvals, status, request_hash
        FROM wallet_pending_approvals
        WHERE id = $1
        "#,
    )
    .bind(approval_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| WalletError::InternalError(format!("DB error: {}", e)))?
    .ok_or(WalletError::ApprovalNotFound)?;

    if approval.5 != "pending" {
        return Err(WalletError::InternalError(format!(
            "Approval is already {}",
            approval.5
        )));
    }

    // Verify NEP-413 signature: message = "approve:{approval_id}:{request_hash}"
    let message = format!("approve:{}:{}", approval_id, approval.6);
    auth::verify_nep413_signature(&message, signature, public_key, nonce, &state.contract_id)?;

    // Check if approver already signed
    let existing = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM wallet_approval_signatures WHERE approval_id = $1 AND approver_id = $2",
    )
    .bind(approval_id)
    .bind(approver_id)
    .fetch_one(&state.db)
    .await
    .map_err(|e| WalletError::InternalError(format!("DB error: {}", e)))?;

    if existing > 0 {
        return Err(WalletError::AlreadyApproved);
    }

    // Verify approver is in the policy's approvers list
    let approval_wallet_pubkey = policy::resolve_wallet_pubkey(&state.db, &approval.1).await?;
    let policy_output = policy::check_wallet_policy(
        &state.policy_cache,
        &state.near_rpc_url,
        &state.contract_id,
        state.keystore_base_url.as_deref().unwrap_or(""),
        state.keystore_auth_token.as_deref().unwrap_or(""),
        &approval.1, // wallet_id that owns the approval
        &approval_wallet_pubkey,
        serde_json::json!({"type": "get_policy"}),
    )
    .await
    .ok();

    if let Some(ref output) = policy_output {
        // If policy exists and has approvers list, verify the caller is in it
        if let Some(ref policy_json) = output.policy {
            if let Some(approvers) = policy_json.pointer("/approval/approvers").and_then(|v| v.as_array()) {
                let is_approver = approvers.iter().any(|a| {
                    let id = a.get("id").and_then(|v| v.as_str()).unwrap_or("");
                    // Match by: account_id (NEAR account) or public_key
                    id == approver_id || id == public_key
                });

                if !is_approver {
                    return Err(WalletError::NotApprover);
                }
            }
        }
    }

    // Insert approval signature (store the actual ed25519 signature)
    sqlx::query(
        r#"
        INSERT INTO wallet_approval_signatures (approval_id, approver_id, approver_role, signature)
        VALUES ($1, $2, $3, $4)
        "#,
    )
    .bind(approval_id)
    .bind(approver_id)
    .bind("signer")
    .bind(signature)
    .execute(&state.db)
    .await
    .map_err(|e| WalletError::InternalError(format!("DB error: {}", e)))?;

    // Count current approvals
    let current_count: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM wallet_approval_signatures WHERE approval_id = $1",
    )
    .bind(approval_id)
    .fetch_one(&state.db)
    .await
    .map_err(|e| WalletError::InternalError(format!("DB error: {}", e)))?;

    let approved = current_count.0 as i32;
    let required = approval.4;

    // Audit log
    audit::record_audit_event(
        &state.db,
        &approval.1,
        "approval",
        approver_id,
        serde_json::json!({"approval_id": approval_id.to_string(), "approved": approved, "required": required}),
        None,
    )
    .await;

    // Get webhook_url from policy (best-effort, for event delivery)
    let webhook_url = policy::check_wallet_policy(
        &state.policy_cache,
        &state.near_rpc_url,
        &state.contract_id,
        state.keystore_base_url.as_deref().unwrap_or(""),
        state.keystore_auth_token.as_deref().unwrap_or(""),
        &approval.1,
        &approval_wallet_pubkey,
        serde_json::json!({"type": "get_policy"}),
    )
    .await
    .ok()
    .and_then(|o| o.webhook_url);

    // Webhook: approval_received
    if let Some(ref url) = webhook_url {
        let _ = webhooks::enqueue_webhook(
            &state.db,
            &approval.1,
            "approval_received",
            serde_json::json!({
                "approval_id": approval_id.to_string(),
                "approver": approver_id,
                "approved": approved,
                "required": required,
            }),
            url,
        )
        .await;
    }

    // Check if threshold met
    if approved >= required {
        // Mark approval as approved
        sqlx::query("UPDATE wallet_pending_approvals SET status = 'approved' WHERE id = $1")
            .bind(approval_id)
            .execute(&state.db)
            .await
            .ok();

        // Mark the linked request as processing
        sqlx::query(
            "UPDATE wallet_requests SET status = 'processing', updated_at = NOW() WHERE approval_id = $1",
        )
        .bind(approval_id)
        .execute(&state.db)
        .await
        .ok();

        // Get linked request_id
        let linked_request = sqlx::query_scalar::<_, Uuid>(
            "SELECT request_id FROM wallet_requests WHERE approval_id = $1",
        )
        .bind(approval_id)
        .fetch_optional(&state.db)
        .await
        .ok()
        .flatten();

        // Auto-execute the approved operation in background
        if let Some(request_id) = linked_request {
            let wallet_id = approval.1.clone();
            let request_type = approval.2.clone();
            let request_data = approval.3.clone();
            let request_hash = approval.6.clone();
            let state_clone = state.clone();
            let wh_url = webhook_url.clone();

            // Load all approver_ids for this approval
            let approver_ids = sqlx::query_scalar::<_, String>(
                "SELECT approver_id FROM wallet_approval_signatures WHERE approval_id = $1",
            )
            .bind(approval_id)
            .fetch_all(&state.db)
            .await
            .unwrap_or_default();

            let approval_info = ApprovalInfo {
                approver_ids,
                request_hash,
            };

            tokio::spawn(async move {
                if request_type == "withdraw" {
                    execute_approved_withdraw(
                        state_clone,
                        wallet_id,
                        request_id,
                        request_data,
                        wh_url,
                        approval_info,
                    )
                    .await;
                } else if request_type == "call" {
                    execute_approved_call(
                        state_clone,
                        wallet_id,
                        request_id,
                        request_data,
                        wh_url,
                        approval_info,
                    )
                    .await;
                } else {
                    warn!(
                        "Auto-execute not implemented for request type: {}",
                        request_type
                    );
                }
            });
        }

        return Ok(Json(ApproveResponse {
            approval_id: approval_id.to_string(),
            // Return "approved" — execution happens asynchronously via tokio::spawn.
            // Client should poll GET /requests/{id} for the final status.
            status: "approved".to_string(),
            approved,
            required,
            request_id: linked_request.map(|id| id.to_string()),
        }));
    }

    Ok(Json(ApproveResponse {
        approval_id: approval_id.to_string(),
        status: "pending".to_string(),
        approved,
        required,
        request_id: None,
    }))
}

// ============================================================================
// Reject
// ============================================================================
pub async fn reject(
    State(state): State<WalletState>,
    headers: HeaderMap,
    Path(approval_id_str): Path<String>,
    body: Option<Json<serde_json::Value>>,
) -> Result<Json<serde_json::Value>, WalletError> {
    let auth = authenticate(&headers, &state.allowed_worker_token_hashes, &state.db, &state.api_key_cache).await?;

    let approver_account = body
        .as_ref()
        .and_then(|b| b.get("approver_account"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let approver_id = approver_account.as_deref().unwrap_or(&auth.wallet_id);

    let approval_id: Uuid = approval_id_str
        .parse()
        .map_err(|_| WalletError::ApprovalNotFound)?;

    // Get the approval
    let approval = sqlx::query_as::<_, (Uuid, String, String, String)>(
        "SELECT id, wallet_id, request_type, status FROM wallet_pending_approvals WHERE id = $1",
    )
    .bind(approval_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| WalletError::InternalError(format!("DB error: {}", e)))?
    .ok_or(WalletError::ApprovalNotFound)?;

    if approval.3 != "pending" {
        return Err(WalletError::InternalError(format!(
            "Approval is already {}",
            approval.3
        )));
    }

    // Verify the caller is an approver in the policy
    let approval_wallet_pubkey = policy::resolve_wallet_pubkey(&state.db, &approval.1).await?;
    let policy_output = policy::check_wallet_policy(
        &state.policy_cache,
        &state.near_rpc_url,
        &state.contract_id,
        state.keystore_base_url.as_deref().unwrap_or(""),
        state.keystore_auth_token.as_deref().unwrap_or(""),
        &approval.1,
        &approval_wallet_pubkey,
        serde_json::json!({"type": "get_policy"}),
    )
    .await
    .ok();

    if let Some(ref output) = policy_output {
        if let Some(ref policy_json) = output.policy {
            if let Some(approvers) = policy_json.pointer("/approval/approvers").and_then(|v| v.as_array()) {
                let wallet_pubkey = sqlx::query_scalar::<_, String>(
                    "SELECT near_pubkey FROM wallet_accounts WHERE wallet_id = $1",
                )
                .bind(&auth.wallet_id)
                .fetch_optional(&state.db)
                .await
                .ok()
                .flatten();

                let is_approver = approvers.iter().any(|a| {
                    let id = a.get("id").and_then(|v| v.as_str()).unwrap_or("");
                    id == approver_id
                        || id == auth.wallet_id
                        || wallet_pubkey.as_deref().is_some_and(|pk| id == pk)
                });

                if !is_approver {
                    return Err(WalletError::NotApprover);
                }
            }
        }
    }

    // Mark as rejected
    sqlx::query("UPDATE wallet_pending_approvals SET status = 'rejected' WHERE id = $1")
        .bind(approval_id)
        .execute(&state.db)
        .await
        .map_err(|e| WalletError::InternalError(format!("DB error: {}", e)))?;

    // Also reject the linked request
    sqlx::query(
        "UPDATE wallet_requests SET status = 'rejected', updated_at = NOW() WHERE approval_id = $1",
    )
    .bind(approval_id)
    .execute(&state.db)
    .await
    .ok();

    // Audit log
    audit::record_audit_event(
        &state.db,
        &approval.1,
        "rejection",
        approver_id,
        serde_json::json!({"approval_id": approval_id.to_string()}),
        None,
    )
    .await;

    Ok(Json(serde_json::json!({
        "approval_id": approval_id.to_string(),
        "status": "rejected",
    })))
}

/// Execute an approved withdraw operation (called after multisig threshold met)
async fn execute_approved_withdraw(
    state: WalletState,
    wallet_id: String,
    request_id: Uuid,
    request_data: serde_json::Value,
    webhook_url: Option<String>,
    approval_info: ApprovalInfo,
) {
    // Verify the approval hasn't expired between threshold check and execution
    let still_valid = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM wallet_pending_approvals WHERE id = (SELECT approval_id FROM wallet_requests WHERE request_id = $1) AND status = 'approved' AND expires_at > NOW()",
    )
    .bind(request_id)
    .fetch_one(&state.db)
    .await
    .unwrap_or(0);

    if still_valid == 0 {
        warn!("Auto-execute aborted for {}: approval expired or invalid", request_id);
        let _ = sqlx::query(
            "UPDATE wallet_requests SET status = 'failed', result_data = $2, updated_at = NOW() WHERE request_id = $1",
        )
        .bind(request_id)
        .bind(serde_json::json!({"error": "Approval expired before execution"}))
        .execute(&state.db)
        .await;
        return;
    }

    let chain = request_data["chain"]
        .as_str()
        .unwrap_or("near")
        .to_string();
    let to = request_data["to"].as_str().unwrap_or("").to_string();
    let amount = request_data["amount"]
        .as_str()
        .unwrap_or("0")
        .to_string();
    let token = request_data["token"].as_str().map(|s| s.to_string());
    let token_key = token.as_deref().unwrap_or("native").to_string();

    // Strip nep141: prefix (intents ft_withdraw uses plain token ID)
    let token_for_intent = token_key
        .strip_prefix("nep141:")
        .unwrap_or(&token_key)
        .to_string();

    // Acquire nonce lock
    let nonce_lock = state.nonce_locks.get_lock(&wallet_id).await;
    let _nonce_guard = nonce_lock.lock().await;

    // Derive address
    let derived = match keystore_derive_address(&state, &wallet_id, &chain).await {
        Ok(d) => d,
        Err(e) => {
            warn!(
                "Auto-execute derive-address failed for {}: {:?}",
                request_id, e
            );
            let _ = sqlx::query(
                "UPDATE wallet_requests SET status = 'failed', result_data = $2, updated_at = NOW() WHERE request_id = $1",
            )
            .bind(request_id)
            .bind(serde_json::json!({"error": format!("{:?}", e)}))
            .execute(&state.db)
            .await;
            return;
        }
    };

    // Build NEP-413 intent message (same as normal withdraw flow)
    let deadline = (Utc::now() + chrono::Duration::seconds(180))
        .format("%Y-%m-%dT%H:%M:%S%.3fZ")
        .to_string();

    let intent_message = serde_json::json!({
        "signer_id": derived.address,
        "deadline": deadline,
        "intents": [{
            "intent": "ft_withdraw",
            "token": token_for_intent,
            "receiver_id": to,
            "amount": amount,
        }]
    });

    // Serialize with spaces after colons (intents protocol format)
    let message_str = serde_json::to_string(&intent_message)
        .unwrap()
        .replace("\":", "\": ");

    // Generate nonce: SHA256(timestamp_nanos) -> base64
    let nonce_base64 = {
        let timestamp_nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
            .to_string();
        let hash = Sha256::digest(timestamp_nanos.as_bytes());
        base64::Engine::encode(&base64::engine::general_purpose::STANDARD, hash)
    };

    // Sign intent via keystore NEP-413 endpoint
    let sign_result =
        match keystore_sign_nep413(&state, &wallet_id, &chain, &message_str, &nonce_base64, "intents.near", Some(&approval_info)).await {
            Ok(s) => s,
            Err(e) => {
                warn!("Auto-execute sign failed for {}: {:?}", request_id, e);
                let _ = sqlx::query(
                    "UPDATE wallet_requests SET status = 'failed', result_data = $2, updated_at = NOW() WHERE request_id = $1",
                )
                .bind(request_id)
                .bind(serde_json::json!({"error": format!("{:?}", e)}))
                .execute(&state.db)
                .await;
                return;
            }
        };

    // Package signed intent (same format as normal withdraw)
    let signed_intent = serde_json::json!({
        "message": message_str,
        "nonce": nonce_base64,
        "recipient": "intents.near",
        "signature": sign_result.signature_base58,
        "public_key": sign_result.public_key,
        "standard": "nep413",
    });

    let signed_intent_base64 = base64::Engine::encode(
        &base64::engine::general_purpose::STANDARD,
        serde_json::to_string(&signed_intent).unwrap().as_bytes(),
    );

    // Submit to backend
    let wallet_info = WalletInfo {
        wallet_id: wallet_id.clone(),
        chain: chain.clone(),
        chain_address: derived.address.clone(),
        chain_public_key: derived.public_key.clone(),
    };

    let backend_req = BackendWithdrawRequest {
        to: to.clone(),
        amount: amount.clone(),
        token,
        chain: chain.clone(),
    };

    match state
        .backend
        .withdraw(&wallet_info, backend_req, &signed_intent_base64)
        .await
    {
        Ok(result) => {
            let status = if result.status == "success" {
                "success"
            } else {
                "processing"
            };

            let result_data = serde_json::json!({
                "tx_hash": result.tx_hash,
                "fee": result.fee,
                "fee_token": result.fee_token,
            });

            let _ = sqlx::query(
                "UPDATE wallet_requests SET status = $2, intents_ref = $3, result_data = $4, updated_at = NOW() WHERE request_id = $1",
            )
            .bind(request_id)
            .bind(status)
            .bind(&result.operation_id)
            .bind(&result_data)
            .execute(&state.db)
            .await;

            // Usage already recorded when pending_approval was created (handlers.rs line 533).
            // Recording again here would double-count against velocity limits.

            // Audit
            audit::record_audit_event(
                &state.db,
                &wallet_id,
                "withdraw_auto_executed",
                "system",
                serde_json::json!({"to": to, "amount": amount, "chain": chain, "status": status}),
                Some(&request_id.to_string()),
            )
            .await;

            // Webhook: request_completed
            if let Some(ref url) = webhook_url {
                let _ = webhooks::enqueue_webhook(
                    &state.db,
                    &wallet_id,
                    "request_completed",
                    serde_json::json!({
                        "request_id": request_id.to_string(),
                        "type": "withdraw",
                        "status": status,
                        "result": result_data,
                    }),
                    url,
                )
                .await;
            }
        }
        Err(e) => {
            warn!("Auto-execute backend failed for {}: {}", request_id, e);
            let _ = sqlx::query(
                "UPDATE wallet_requests SET status = 'failed', result_data = $2, updated_at = NOW() WHERE request_id = $1",
            )
            .bind(request_id)
            .bind(serde_json::json!({"error": e.to_string()}))
            .execute(&state.db)
            .await;
        }
    }
}

// ============================================================================
// Helper: call keystore sign-near-call
// ============================================================================

async fn keystore_sign_near_call(
    state: &WalletState,
    wallet_id: &str,
    receiver_id: &str,
    method_name: &str,
    args_json: &str,
    gas: u64,
    deposit: &str,
    approval_info: Option<&ApprovalInfo>,
) -> Result<KeystoreSignNearCallResponse, WalletError> {
    let keystore_url = state.keystore_base_url.as_ref().ok_or_else(|| {
        WalletError::InternalError("Keystore URL not configured".to_string())
    })?;

    let payload = KeystoreSignNearCallRequest {
        wallet_id: wallet_id.to_string(),
        receiver_id: receiver_id.to_string(),
        method_name: method_name.to_string(),
        args_json: args_json.to_string(),
        gas,
        deposit: deposit.to_string(),
        approval_info: approval_info.cloned(),
    };

    let mut request = state
        .http_client
        .post(format!("{}/wallet/sign-near-call", keystore_url))
        .json(&payload);

    if let Some(ref token) = state.keystore_auth_token {
        request = request.header("Authorization", format!("Bearer {}", token));
    }

    let response = request.send().await.map_err(|e| {
        WalletError::KeystoreError(format!("Failed to connect to keystore: {}", e))
    })?;

    if !response.status().is_success() {
        let body = response.text().await.unwrap_or_default();
        return Err(WalletError::KeystoreError(format!(
            "Keystore sign-near-call failed: {}",
            body
        )));
    }

    response.json().await.map_err(|e| {
        WalletError::KeystoreError(format!("Invalid keystore response: {}", e))
    })
}

/// Broadcast a signed transaction to NEAR RPC and parse the outcome.
///
/// Returns (status, tx_hash, result_value) where result_value is the decoded
/// return value from FunctionCall if successful.
async fn broadcast_near_tx(
    http_client: &reqwest::Client,
    rpc_url: &str,
    signed_tx_base64: &str,
) -> Result<(String, String, Option<serde_json::Value>), WalletError> {
    let body = serde_json::json!({
        "jsonrpc": "2.0",
        "id": "wallet-call",
        "method": "broadcast_tx_commit",
        "params": [signed_tx_base64],
    });

    let response = http_client
        .post(rpc_url)
        .json(&body)
        .timeout(std::time::Duration::from_secs(60))
        .send()
        .await
        .map_err(|e| WalletError::InternalError(format!("NEAR RPC error: {}", e)))?;

    let rpc_result: serde_json::Value = response
        .json()
        .await
        .map_err(|e| WalletError::InternalError(format!("Invalid RPC response: {}", e)))?;

    // Check for JSON-RPC error
    if let Some(err) = rpc_result.get("error") {
        return Err(WalletError::InternalError(format!(
            "NEAR RPC error: {}",
            err
        )));
    }

    let result = rpc_result
        .get("result")
        .ok_or_else(|| WalletError::InternalError("Missing result in RPC response".to_string()))?;

    // Extract tx hash
    let tx_hash = result
        .pointer("/transaction_outcome/id")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    // Check top-level status
    let status = result.get("status").unwrap_or(&serde_json::Value::Null);
    if let Some(failure) = status.get("Failure") {
        return Ok(("failed".to_string(), tx_hash, Some(failure.clone())));
    }

    // Check transaction_outcome for failure
    if let Some(failure) = result.pointer("/transaction_outcome/outcome/status/Failure") {
        return Ok(("failed".to_string(), tx_hash, Some(failure.clone())));
    }

    // Check receipts for failures
    if let Some(receipts) = result.get("receipts_outcome").and_then(|v| v.as_array()) {
        for receipt in receipts {
            if let Some(failure) = receipt.pointer("/outcome/status/Failure") {
                return Ok(("failed".to_string(), tx_hash, Some(failure.clone())));
            }
        }
    }

    // Success — extract return value from status.SuccessValue
    let return_value = status
        .get("SuccessValue")
        .and_then(|v| v.as_str())
        .and_then(|b64| {
            base64::Engine::decode(&base64::engine::general_purpose::STANDARD, b64).ok()
        })
        .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok());

    Ok(("success".to_string(), tx_hash, return_value))
}

// ============================================================================
// POST /call — native NEAR function call
// ============================================================================

const DEFAULT_GAS: u64 = 30_000_000_000_000; // 30 TGas

pub async fn call(
    State(state): State<WalletState>,
    headers: HeaderMap,
    body: String,
) -> Result<Json<CallResponse>, WalletError> {
    let auth = authenticate(&headers, &state.allowed_worker_token_hashes, &state.db, &state.api_key_cache).await?;

    // Idempotency key: use provided header, or generate random UUID if absent
    let idempotency_key = if auth.is_internal {
        auth.idempotency_key.clone()
    } else {
        Some(auth.idempotency_key.clone()
            .unwrap_or_else(|| uuid::Uuid::new_v4().to_string()))
    };

    if let Some(ref key) = idempotency_key {
        if let Some(existing_id) = idempotency::check_idempotency(&state.db, &auth.wallet_id, key).await? {
            return Err(WalletError::DuplicateIdempotencyKey {
                request_id: existing_id,
            });
        }
    }

    // Parse request
    let req: CallRequest = serde_json::from_str(&body).map_err(|e| {
        WalletError::InternalError(format!("Invalid request: {}", e))
    })?;

    let gas: u64 = req.gas.as_deref().unwrap_or("30000000000000").parse().unwrap_or(DEFAULT_GAS);
    let deposit = req.deposit.as_deref().unwrap_or("0").to_string();
    let args_json = serde_json::to_string(&req.args).unwrap_or_else(|_| "{}".to_string());

    // Policy check
    let action = serde_json::json!({
        "type": "call",
        "receiver_id": req.receiver_id,
        "method_name": req.method_name,
        "deposit": deposit,
    });

    let wallet_pubkey = policy::resolve_wallet_pubkey(&state.db, &auth.wallet_id).await?;
    let policy_output = policy::check_wallet_policy_with_overrides(
        &state.policy_cache,
        &state.near_rpc_url,
        &state.contract_id,
        state.keystore_base_url.as_deref().unwrap_or(""),
        state.keystore_auth_token.as_deref().unwrap_or(""),
        &auth.wallet_id,
        &wallet_pubkey,
        action,
        Some(&state.policy_overrides),
    )
    .await?;

    let webhook_url = policy_output.webhook_url.clone();
    match policy_output.decision {
        PolicyDecision::Frozen => return Err(WalletError::WalletFrozen),
        PolicyDecision::Denied(reason) => return Err(WalletError::PolicyDenied(reason)),
        PolicyDecision::RequiresApproval { required_approvals } => {
            // Create pending approval
            let request_id = Uuid::new_v4();
            let approval_id = Uuid::new_v4();
            let request_data = serde_json::json!({
                "receiver_id": req.receiver_id,
                "method_name": req.method_name,
                "args": req.args,
                "gas": gas.to_string(),
                "deposit": deposit,
            });

            let request_hash = sha256_hex(&serde_json::to_string(&request_data).unwrap_or_default());

            sqlx::query(
                r#"
                INSERT INTO wallet_pending_approvals (id, wallet_id, request_type, request_data, request_hash, required_approvals, expires_at)
                VALUES ($1, $2, 'call', $3, $4, $5, NOW() + INTERVAL '24 hours')
                "#,
            )
            .bind(approval_id)
            .bind(&auth.wallet_id)
            .bind(&request_data)
            .bind(&request_hash)
            .bind(required_approvals)
            .execute(&state.db)
            .await
            .map_err(|e| WalletError::InternalError(format!("DB error: {}", e)))?;

            sqlx::query(
                r#"
                INSERT INTO wallet_requests (request_id, wallet_id, request_type, chain, request_data, approval_id, status, idempotency_key)
                VALUES ($1, $2, 'call', 'near', $3, $4, 'pending_approval', $5)
                "#,
            )
            .bind(request_id)
            .bind(&auth.wallet_id)
            .bind(&request_data)
            .bind(approval_id)
            .bind(idempotency_key.as_deref())
            .execute(&state.db)
            .await
            .map_err(|e| WalletError::InternalError(format!("DB error: {}", e)))?;

            audit::record_audit_event(
                &state.db,
                &auth.wallet_id,
                "call_pending_approval",
                &auth.wallet_id,
                serde_json::json!({"receiver_id": req.receiver_id, "method_name": req.method_name, "deposit": deposit}),
                Some(&request_id.to_string()),
            )
            .await;

            if let Some(ref url) = webhook_url {
                let _ = webhooks::enqueue_webhook(
                    &state.db,
                    &auth.wallet_id,
                    "approval_needed",
                    serde_json::json!({
                        "request_id": request_id.to_string(),
                        "approval_id": approval_id.to_string(),
                        "type": "call",
                        "required": required_approvals,
                    }),
                    url,
                )
                .await;
            }

            return Ok(Json(CallResponse {
                request_id: request_id.to_string(),
                status: "pending_approval".to_string(),
                tx_hash: None,
                result: None,
                approval_id: Some(approval_id.to_string()),
                required: Some(required_approvals),
                approved: Some(0),
            }));
        }
        PolicyDecision::NoPolicyAllow | PolicyDecision::Allowed => {}
    }

    // Execute the call
    let nonce_lock = state.nonce_locks.get_lock(&auth.wallet_id).await;
    let _nonce_guard = nonce_lock.lock().await;

    let sign_result = keystore_sign_near_call(
        &state,
        &auth.wallet_id,
        &req.receiver_id,
        &req.method_name,
        &args_json,
        gas,
        &deposit,
        None,
    )
    .await?;

    // Create request record
    let request_id = Uuid::new_v4();
    let request_data = serde_json::json!({
        "receiver_id": req.receiver_id,
        "method_name": req.method_name,
        "args": req.args,
        "gas": gas.to_string(),
        "deposit": deposit,
    });

    sqlx::query(
        r#"
        INSERT INTO wallet_requests (request_id, wallet_id, request_type, chain, request_data, status, idempotency_key)
        VALUES ($1, $2, 'call', 'near', $3, 'processing', $4)
        "#,
    )
    .bind(request_id)
    .bind(&auth.wallet_id)
    .bind(&request_data)
    .bind(idempotency_key.as_deref())
    .execute(&state.db)
    .await
    .map_err(|e| WalletError::InternalError(format!("DB error: {}", e)))?;

    // Broadcast
    let (status, tx_hash, result_value) =
        broadcast_near_tx(&state.http_client, &state.near_rpc_url, &sign_result.signed_tx_base64).await?;

    let result_data = serde_json::json!({
        "tx_hash": tx_hash,
        "signer_id": sign_result.signer_id,
        "result": result_value,
    });

    // Update request
    sqlx::query(
        "UPDATE wallet_requests SET status = $2, result_data = $3, updated_at = NOW() WHERE request_id = $1",
    )
    .bind(request_id)
    .bind(&status)
    .bind(&result_data)
    .execute(&state.db)
    .await
    .ok();

    // Audit
    audit::record_audit_event(
        &state.db,
        &auth.wallet_id,
        "call",
        &auth.wallet_id,
        serde_json::json!({
            "receiver_id": req.receiver_id,
            "method_name": req.method_name,
            "deposit": deposit,
            "status": status,
            "tx_hash": tx_hash,
        }),
        Some(&request_id.to_string()),
    )
    .await;

    // Webhook
    if let Some(ref url) = webhook_url {
        let _ = webhooks::enqueue_webhook(
            &state.db,
            &auth.wallet_id,
            "request_completed",
            serde_json::json!({
                "request_id": request_id.to_string(),
                "type": "call",
                "status": status,
                "result": result_data,
            }),
            url,
        )
        .await;
    }

    Ok(Json(CallResponse {
        request_id: request_id.to_string(),
        status,
        tx_hash: Some(tx_hash),
        result: result_value,
        approval_id: None,
        required: None,
        approved: None,
    }))
}

/// Execute an approved call operation (called after multisig threshold met)
async fn execute_approved_call(
    state: WalletState,
    wallet_id: String,
    request_id: Uuid,
    request_data: serde_json::Value,
    webhook_url: Option<String>,
    approval_info: ApprovalInfo,
) {
    // Verify the approval hasn't expired between threshold check and execution
    let still_valid = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM wallet_pending_approvals WHERE id = (SELECT approval_id FROM wallet_requests WHERE request_id = $1) AND status = 'approved' AND expires_at > NOW()",
    )
    .bind(request_id)
    .fetch_one(&state.db)
    .await
    .unwrap_or(0);

    if still_valid == 0 {
        warn!("Auto-execute call aborted for {}: approval expired or invalid", request_id);
        let _ = sqlx::query(
            "UPDATE wallet_requests SET status = 'failed', result_data = $2, updated_at = NOW() WHERE request_id = $1",
        )
        .bind(request_id)
        .bind(serde_json::json!({"error": "Approval expired before execution"}))
        .execute(&state.db)
        .await;
        return;
    }

    let receiver_id = request_data["receiver_id"].as_str().unwrap_or("").to_string();
    let method_name = request_data["method_name"].as_str().unwrap_or("").to_string();
    let args = &request_data["args"];
    let args_json = serde_json::to_string(args).unwrap_or_else(|_| "{}".to_string());
    let gas: u64 = request_data["gas"]
        .as_str()
        .and_then(|s| s.parse().ok())
        .unwrap_or(DEFAULT_GAS);
    let deposit = request_data["deposit"].as_str().unwrap_or("0").to_string();

    let nonce_lock = state.nonce_locks.get_lock(&wallet_id).await;
    let _nonce_guard = nonce_lock.lock().await;

    match keystore_sign_near_call(&state, &wallet_id, &receiver_id, &method_name, &args_json, gas, &deposit, Some(&approval_info)).await {
        Ok(sign_result) => {
            match broadcast_near_tx(&state.http_client, &state.near_rpc_url, &sign_result.signed_tx_base64).await {
                Ok((status, tx_hash, result_value)) => {
                    let result_data = serde_json::json!({
                        "tx_hash": tx_hash,
                        "signer_id": sign_result.signer_id,
                        "result": result_value,
                    });

                    let _ = sqlx::query(
                        "UPDATE wallet_requests SET status = $2, result_data = $3, updated_at = NOW() WHERE request_id = $1",
                    )
                    .bind(request_id)
                    .bind(&status)
                    .bind(&result_data)
                    .execute(&state.db)
                    .await;

                    audit::record_audit_event(
                        &state.db,
                        &wallet_id,
                        "call_auto_executed",
                        &wallet_id,
                        serde_json::json!({"receiver_id": receiver_id, "method_name": method_name, "status": status, "tx_hash": tx_hash}),
                        Some(&request_id.to_string()),
                    )
                    .await;

                    if let Some(ref url) = webhook_url {
                        let _ = webhooks::enqueue_webhook(
                            &state.db,
                            &wallet_id,
                            "request_completed",
                            serde_json::json!({
                                "request_id": request_id.to_string(),
                                "type": "call",
                                "status": status,
                                "result": result_data,
                            }),
                            url,
                        )
                        .await;
                    }
                }
                Err(e) => {
                    warn!("Auto-execute call broadcast failed for {}: {:?}", request_id, e);
                    let _ = sqlx::query(
                        "UPDATE wallet_requests SET status = 'failed', result_data = $2, updated_at = NOW() WHERE request_id = $1",
                    )
                    .bind(request_id)
                    .bind(serde_json::json!({"error": format!("{:?}", e)}))
                    .execute(&state.db)
                    .await;
                }
            }
        }
        Err(e) => {
            warn!("Auto-execute call signing failed for {}: {:?}", request_id, e);
            let _ = sqlx::query(
                "UPDATE wallet_requests SET status = 'failed', result_data = $2, updated_at = NOW() WHERE request_id = $1",
            )
            .bind(request_id)
            .bind(serde_json::json!({"error": format!("{:?}", e)}))
            .execute(&state.db)
            .await;
        }
    }
}

// ============================================================================
// GET /audit
// ============================================================================
pub async fn get_audit(
    State(state): State<WalletState>,
    headers: HeaderMap,
    Query(query): Query<AuditQuery>,
) -> Result<Json<AuditResponse>, WalletError> {
    let auth = authenticate(&headers, &state.allowed_worker_token_hashes, &state.db, &state.api_key_cache).await?;

    let limit = query.limit.min(100).max(1);
    let offset = query.offset.max(0);

    let rows = audit::get_audit_events(&state.db, &auth.wallet_id, limit, offset)
        .await
        .map_err(|e| WalletError::InternalError(format!("DB error: {}", e)))?;

    Ok(Json(AuditResponse {
        events: rows
            .into_iter()
            .map(|r| AuditEvent {
                r#type: r.event_type,
                request_id: r.request_id,
                status: None,
                details: r.details,
                at: r.created_at.to_rfc3339(),
            })
            .collect(),
    }))
}

/// Verify internal auth: requires X-Internal-Wallet-Auth header with a valid worker token.
/// Returns Err(WalletError) if auth fails.
fn verify_internal_auth(
    headers: &HeaderMap,
    allowed_worker_token_hashes: &[String],
) -> Result<(), WalletError> {
    let token = headers
        .get("x-internal-wallet-auth")
        .and_then(|v| v.to_str().ok())
        .ok_or(WalletError::MissingAuth)?;

    let token_hash = {
        let mut hasher = Sha256::new();
        hasher.update(token.as_bytes());
        hex::encode(hasher.finalize())
    };

    if allowed_worker_token_hashes.iter().any(|h| h == &token_hash) {
        Ok(())
    } else {
        Err(WalletError::InvalidSignature(
            "Invalid internal wallet auth token".to_string(),
        ))
    }
}

// ============================================================================
// Internal wallet check endpoint (for worker WASI calls)
// ============================================================================
pub async fn internal_wallet_check(
    State(state): State<WalletState>,
    headers: HeaderMap,
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> Result<Json<serde_json::Value>, WalletError> {
    verify_internal_auth(&headers, &state.allowed_worker_token_hashes)?;

    // Support multiple query modes:
    // 1. ?wallet_id=X — usage data + pending approvals for a specific wallet
    // 2. ?approver_id=X — pending approvals where X is listed as an approver
    // 3. ?approval_id=X — single approval details

    if let Some(approval_id) = params.get("approval_id") {
        // Single approval detail
        let approval_uuid: Uuid = approval_id.parse().map_err(|_| WalletError::ApprovalNotFound)?;
        let approval = sqlx::query_as::<_, (Uuid, String, String, serde_json::Value, String, i32, String, chrono::DateTime<chrono::Utc>, chrono::DateTime<chrono::Utc>)>(
            r#"
            SELECT id, wallet_id, request_type, request_data, status, required_approvals, request_hash, expires_at, created_at
            FROM wallet_pending_approvals WHERE id = $1
            "#,
        )
        .bind(approval_uuid)
        .fetch_optional(&state.db)
        .await
        .map_err(|e| WalletError::InternalError(format!("DB error: {}", e)))?
        .ok_or(WalletError::ApprovalNotFound)?;

        // Get approvers
        let approvers = sqlx::query_as::<_, (String, String, String, chrono::DateTime<chrono::Utc>)>(
            "SELECT approver_id, approver_role, signature, created_at FROM wallet_approval_signatures WHERE approval_id = $1",
        )
        .bind(approval_uuid)
        .fetch_all(&state.db)
        .await
        .map_err(|e| WalletError::InternalError(format!("DB error: {}", e)))?;

        return Ok(Json(serde_json::json!({
            "id": approval.0.to_string(),
            "wallet_id": approval.1,
            "request_type": approval.2,
            "request_data": approval.3,
            "status": approval.4,
            "required_approvals": approval.5,
            "request_hash": approval.6,
            "expires_at": approval.7.to_rfc3339(),
            "created_at": approval.8.to_rfc3339(),
            "approvers": approvers.iter().map(|a| serde_json::json!({
                "approver_id": a.0,
                "approver_role": a.1,
                "signature": a.2,
                "created_at": a.3.to_rfc3339(),
            })).collect::<Vec<_>>(),
        })));
    }

    if let Some(near_pubkey) = params.get("near_pubkey") {
        // Lookup wallet_id by on-chain pubkey, then return pending approvals
        let wallet_id = sqlx::query_scalar::<_, String>(
            "SELECT wallet_id FROM wallet_accounts WHERE near_pubkey = $1",
        )
        .bind(near_pubkey)
        .fetch_optional(&state.db)
        .await
        .map_err(|e| WalletError::InternalError(format!("DB error: {}", e)))?;

        let Some(wallet_id) = wallet_id else {
            return Ok(Json(serde_json::json!({
                "near_pubkey": near_pubkey,
                "wallet_id": null,
                "pending_approvals": [],
            })));
        };

        let approvals = sqlx::query_as::<_, (Uuid, String, String, serde_json::Value, i32, chrono::DateTime<chrono::Utc>, chrono::DateTime<chrono::Utc>, i64)>(
            r#"
            SELECT pa.id, pa.wallet_id, pa.request_type, pa.request_data,
                   pa.required_approvals, pa.expires_at, pa.created_at,
                   (SELECT COUNT(*) FROM wallet_approval_signatures WHERE approval_id = pa.id) as approved_count
            FROM wallet_pending_approvals pa
            WHERE pa.wallet_id = $1 AND pa.status = 'pending' AND pa.expires_at > NOW()
            ORDER BY pa.created_at DESC
            "#,
        )
        .bind(&wallet_id)
        .fetch_all(&state.db)
        .await
        .map_err(|e| WalletError::InternalError(format!("DB error: {}", e)))?;

        let pending: Vec<serde_json::Value> = approvals
            .iter()
            .map(|a| {
                serde_json::json!({
                    "id": a.0.to_string(),
                    "wallet_id": a.1,
                    "request_type": a.2,
                    "request_data": a.3,
                    "required_approvals": a.4,
                    "approved_count": a.7,
                    "expires_at": a.5.to_rfc3339(),
                    "created_at": a.6.to_rfc3339(),
                })
            })
            .collect();

        return Ok(Json(serde_json::json!({
            "near_pubkey": near_pubkey,
            "wallet_id": wallet_id,
            "pending_approvals": pending,
        })));
    }

    if let Some(approver_id) = params.get("approver_id") {
        // Find pending approvals where this pubkey is a known approver
        // (check approval_signatures for wallets this pubkey has approved before,
        // plus all pending approvals for those wallets)
        let approvals = sqlx::query_as::<_, (Uuid, String, String, serde_json::Value, i32, chrono::DateTime<chrono::Utc>, chrono::DateTime<chrono::Utc>, i64)>(
            r#"
            SELECT pa.id, pa.wallet_id, pa.request_type, pa.request_data,
                   pa.required_approvals, pa.expires_at, pa.created_at,
                   (SELECT COUNT(*) FROM wallet_approval_signatures WHERE approval_id = pa.id) as approved_count
            FROM wallet_pending_approvals pa
            WHERE pa.status = 'pending' AND pa.expires_at > NOW()
            AND EXISTS (
                SELECT 1 FROM wallet_approval_signatures was2
                WHERE was2.approval_id IN (SELECT id FROM wallet_pending_approvals WHERE wallet_id = pa.wallet_id)
                AND was2.approver_id = $1
            )
            ORDER BY pa.created_at DESC
            "#,
        )
        .bind(approver_id)
        .fetch_all(&state.db)
        .await
        .map_err(|e| WalletError::InternalError(format!("DB error: {}", e)))?;

        let pending: Vec<serde_json::Value> = approvals
            .iter()
            .map(|a| {
                serde_json::json!({
                    "id": a.0.to_string(),
                    "wallet_id": a.1,
                    "request_type": a.2,
                    "request_data": a.3,
                    "required_approvals": a.4,
                    "approved_count": a.7,
                    "expires_at": a.5.to_rfc3339(),
                    "created_at": a.6.to_rfc3339(),
                })
            })
            .collect();

        return Ok(Json(serde_json::json!({
            "approver_id": approver_id,
            "pending_approvals": pending,
        })));
    }

    // Default: wallet_id mode — usage + pending approvals
    let wallet_id = params
        .get("wallet_id")
        .ok_or(WalletError::MissingWalletId)?;

    let now = Utc::now();
    let daily_period = format!("daily:{}", now.format("%Y-%m-%d"));

    let usage_rows = sqlx::query_as::<_, (String, String, i32)>(
        "SELECT token, total_amount, tx_count FROM wallet_usage WHERE wallet_id = $1 AND period = $2",
    )
    .bind(wallet_id)
    .bind(&daily_period)
    .fetch_all(&state.db)
    .await
    .map_err(|e| WalletError::InternalError(format!("DB error: {}", e)))?;

    let mut usage = serde_json::Map::new();
    let mut total_tx_count = 0;
    for (token, amount, count) in usage_rows {
        usage.insert(token, serde_json::json!({"spent": amount, "count": count}));
        total_tx_count += count;
    }

    // Also fetch pending approvals for this wallet
    let approvals = sqlx::query_as::<_, (Uuid, String, String, serde_json::Value, i32, chrono::DateTime<chrono::Utc>, chrono::DateTime<chrono::Utc>, i64)>(
        r#"
        SELECT pa.id, pa.wallet_id, pa.request_type, pa.request_data,
               pa.required_approvals, pa.expires_at, pa.created_at,
               (SELECT COUNT(*) FROM wallet_approval_signatures WHERE approval_id = pa.id) as approved_count
        FROM wallet_pending_approvals pa
        WHERE pa.wallet_id = $1 AND pa.status = 'pending' AND pa.expires_at > NOW()
        ORDER BY pa.created_at DESC
        "#,
    )
    .bind(wallet_id)
    .fetch_all(&state.db)
    .await
    .map_err(|e| WalletError::InternalError(format!("DB error: {}", e)))?;

    let pending: Vec<serde_json::Value> = approvals
        .iter()
        .map(|a| {
            serde_json::json!({
                "id": a.0.to_string(),
                "wallet_id": a.1,
                "request_type": a.2,
                "request_data": a.3,
                "required_approvals": a.4,
                "approved_count": a.7,
                "expires_at": a.5.to_rfc3339(),
                "created_at": a.6.to_rfc3339(),
            })
        })
        .collect();

    Ok(Json(serde_json::json!({
        "wallet_id": wallet_id,
        "daily_usage": usage,
        "total_tx_count_today": total_tx_count,
        "pending_approvals": pending,
    })))
}

/// Internal audit endpoint (for worker WASI calls, async, non-blocking)
pub async fn internal_wallet_audit(
    State(state): State<WalletState>,
    headers: HeaderMap,
    Json(payload): Json<serde_json::Value>,
) -> Result<Json<serde_json::Value>, WalletError> {
    verify_internal_auth(&headers, &state.allowed_worker_token_hashes)?;
    let wallet_id = payload["wallet_id"].as_str().unwrap_or("");
    let event_type = payload["event_type"].as_str().unwrap_or("unknown");
    let actor = payload["actor"].as_str().unwrap_or(wallet_id);

    audit::record_audit_event(
        &state.db,
        wallet_id,
        event_type,
        actor,
        payload.clone(),
        payload["request_id"].as_str(),
    )
    .await;

    Ok(Json(serde_json::json!({"ok": true})))
}

// ============================================================================
// POST /internal/activate-policy
// ============================================================================
/// Activate a policy locally without on-chain storage.
/// Stores encrypted_base64 in memory so check_wallet_policy can use it.
pub async fn internal_activate_policy(
    State(state): State<WalletState>,
    headers: HeaderMap,
    Json(payload): Json<serde_json::Value>,
) -> Result<Json<serde_json::Value>, WalletError> {
    verify_internal_auth(&headers, &state.allowed_worker_token_hashes)?;
    let wallet_id = payload["wallet_id"]
        .as_str()
        .ok_or_else(|| WalletError::InternalError("missing wallet_id".to_string()))?;
    let encrypted_base64 = payload["encrypted_base64"]
        .as_str()
        .ok_or_else(|| WalletError::InternalError("missing encrypted_base64".to_string()))?;

    // Store override
    {
        let mut overrides = state.policy_overrides.write().await;
        overrides.insert(wallet_id.to_string(), encrypted_base64.to_string());
    }

    // Invalidate negative cache so next check goes through policy engine
    state.policy_cache.invalidate(wallet_id).await;

    info!("Policy activated locally for wallet={}", wallet_id);

    Ok(Json(serde_json::json!({"ok": true, "wallet_id": wallet_id})))
}

// ============================================================================
// POST /internal/wallet-policy-sync — worker notifies of on-chain policy update
// ============================================================================
/// Called by worker when WalletPolicyUpdated SystemEvent is detected.
/// Decrypts policy via keystore, extracts authorized_key_hashes, stores in DB.
pub async fn internal_wallet_policy_sync(
    State(state): State<WalletState>,
    headers: HeaderMap,
    Json(payload): Json<serde_json::Value>,
) -> Result<Json<serde_json::Value>, WalletError> {
    verify_internal_auth(&headers, &state.allowed_worker_token_hashes)?;
    let wallet_pubkey = payload["wallet_pubkey"]
        .as_str()
        .ok_or_else(|| WalletError::InternalError("missing wallet_pubkey".to_string()))?;
    let _owner = payload["owner"]
        .as_str()
        .ok_or_else(|| WalletError::InternalError("missing owner".to_string()))?;
    let encrypted_data = payload["encrypted_data"]
        .as_str()
        .ok_or_else(|| WalletError::InternalError("missing encrypted_data".to_string()))?;

    // Lookup wallet_id from near_pubkey
    let wallet_id = match sqlx::query_as::<_, (String,)>(
        "SELECT wallet_id FROM wallet_accounts WHERE near_pubkey = $1",
    )
    .bind(wallet_pubkey)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| WalletError::InternalError(format!("DB error: {}", e)))?
    {
        Some((wid,)) => wid,
        None => {
            info!(
                "Wallet policy sync skipped: wallet_pubkey={} not registered in coordinator",
                wallet_pubkey
            );
            return Ok(Json(serde_json::json!({"ok": true, "skipped": true})));
        }
    };

    // Decrypt policy via keystore check-policy (returns decrypted policy in response)
    let decrypted_policy = keystore_decrypt_policy(&state, &wallet_id, encrypted_data).await?;

    // Extract authorized_key_hashes from decrypted policy
    let key_hashes: Vec<String> = decrypted_policy
        .get("authorized_key_hashes")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default();

    info!(
        "Policy sync for wallet={}: {} authorized key hashes",
        wallet_id,
        key_hashes.len()
    );

    // Extract frozen status from payload (from on-chain event)
    let frozen = payload["frozen"].as_bool().unwrap_or(false);

    // Update DB: save policy + replace policy-sourced keys
    let mut tx = state
        .db
        .begin()
        .await
        .map_err(|e| WalletError::InternalError(format!("DB error: {}", e)))?;

    // Save decrypted policy and frozen status to wallet_accounts
    sqlx::query(
        "UPDATE wallet_accounts SET policy_json = $1, frozen = $2 WHERE wallet_id = $3",
    )
    .bind(&decrypted_policy)
    .bind(frozen)
    .bind(&wallet_id)
    .execute(&mut *tx)
    .await
    .map_err(|e| WalletError::InternalError(format!("DB error: {}", e)))?;

    // Revoke all existing policy-sourced keys for this wallet
    sqlx::query(
        "UPDATE wallet_api_keys SET revoked_at = NOW() WHERE wallet_id = $1 AND source = 'policy' AND revoked_at IS NULL",
    )
    .bind(&wallet_id)
    .execute(&mut *tx)
    .await
    .map_err(|e| WalletError::InternalError(format!("DB error: {}", e)))?;

    // Insert new policy-sourced keys
    for key_hash in &key_hashes {
        sqlx::query(
            "INSERT INTO wallet_api_keys (key_hash, wallet_id, label, source) VALUES ($1, $2, 'policy', 'policy') ON CONFLICT (key_hash) DO UPDATE SET revoked_at = NULL, source = 'policy'",
        )
        .bind(key_hash)
        .bind(&wallet_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| WalletError::InternalError(format!("DB error: {}", e)))?;
    }

    if !key_hashes.is_empty() {
        // Policy has key hashes → revoke bootstrap keys (policy takes over)
        sqlx::query(
            "UPDATE wallet_api_keys SET revoked_at = NOW() WHERE wallet_id = $1 AND source = 'bootstrap' AND revoked_at IS NULL",
        )
        .bind(&wallet_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| WalletError::InternalError(format!("DB error: {}", e)))?;
    } else {
        // No policy key hashes → restore bootstrap keys so owner isn't locked out
        sqlx::query(
            "UPDATE wallet_api_keys SET revoked_at = NULL WHERE wallet_id = $1 AND source = 'bootstrap'",
        )
        .bind(&wallet_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| WalletError::InternalError(format!("DB error: {}", e)))?;
    }

    tx.commit()
        .await
        .map_err(|e| WalletError::InternalError(format!("DB error: {}", e)))?;

    // Invalidate caches
    state.api_key_cache.invalidate_wallet(&wallet_id).await;
    state.policy_cache.invalidate(&wallet_id).await;

    info!(
        "Policy sync complete: wallet={}, policy_keys={}, bootstrap_revoked={}",
        wallet_id,
        key_hashes.len(),
        !key_hashes.is_empty()
    );

    Ok(Json(serde_json::json!({"ok": true, "wallet_id": wallet_id, "key_hashes_synced": key_hashes.len()})))
}

// ============================================================================
// POST /internal/wallet-policy-delete — worker notifies of on-chain policy deletion
// ============================================================================
pub async fn internal_wallet_policy_delete(
    State(state): State<WalletState>,
    headers: HeaderMap,
    Json(payload): Json<serde_json::Value>,
) -> Result<Json<serde_json::Value>, WalletError> {
    verify_internal_auth(&headers, &state.allowed_worker_token_hashes)?;
    let wallet_pubkey = payload["wallet_pubkey"]
        .as_str()
        .ok_or_else(|| WalletError::InternalError("missing wallet_pubkey".to_string()))?;

    let wallet_id = match sqlx::query_as::<_, (String,)>(
        "SELECT wallet_id FROM wallet_accounts WHERE near_pubkey = $1",
    )
    .bind(wallet_pubkey)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| WalletError::InternalError(format!("DB error: {}", e)))?
    {
        Some((wid,)) => wid,
        None => {
            return Ok(Json(serde_json::json!({"ok": true, "skipped": true})));
        }
    };

    // Clear policy_json and reset frozen
    sqlx::query(
        "UPDATE wallet_accounts SET policy_json = NULL, frozen = FALSE WHERE wallet_id = $1",
    )
    .bind(&wallet_id)
    .execute(&state.db)
    .await
    .map_err(|e| WalletError::InternalError(format!("DB error: {}", e)))?;

    // Revoke all policy-sourced keys
    sqlx::query(
        "UPDATE wallet_api_keys SET revoked_at = NOW() WHERE wallet_id = $1 AND source = 'policy' AND revoked_at IS NULL",
    )
    .bind(&wallet_id)
    .execute(&state.db)
    .await
    .map_err(|e| WalletError::InternalError(format!("DB error: {}", e)))?;

    // Restore bootstrap keys so owner isn't locked out
    sqlx::query(
        "UPDATE wallet_api_keys SET revoked_at = NULL WHERE wallet_id = $1 AND source = 'bootstrap'",
    )
    .bind(&wallet_id)
    .execute(&state.db)
    .await
    .map_err(|e| WalletError::InternalError(format!("DB error: {}", e)))?;

    // Invalidate caches
    state.api_key_cache.invalidate_wallet(&wallet_id).await;
    state.policy_cache.invalidate(&wallet_id).await;

    info!("Policy deleted sync: wallet={}, bootstrap keys restored", wallet_id);

    Ok(Json(serde_json::json!({"ok": true, "wallet_id": wallet_id})))
}

// ============================================================================
// POST /internal/wallet-frozen-change — worker notifies of freeze/unfreeze
// ============================================================================
pub async fn internal_wallet_frozen_change(
    State(state): State<WalletState>,
    headers: HeaderMap,
    Json(payload): Json<serde_json::Value>,
) -> Result<Json<serde_json::Value>, WalletError> {
    verify_internal_auth(&headers, &state.allowed_worker_token_hashes)?;
    let wallet_pubkey = payload["wallet_pubkey"]
        .as_str()
        .ok_or_else(|| WalletError::InternalError("missing wallet_pubkey".to_string()))?;
    let frozen = payload["frozen"]
        .as_bool()
        .ok_or_else(|| WalletError::InternalError("missing frozen".to_string()))?;

    let wallet_id = match sqlx::query_as::<_, (String,)>(
        "SELECT wallet_id FROM wallet_accounts WHERE near_pubkey = $1",
    )
    .bind(wallet_pubkey)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| WalletError::InternalError(format!("DB error: {}", e)))?
    {
        Some((wid,)) => wid,
        None => {
            return Ok(Json(serde_json::json!({"ok": true, "skipped": true})));
        }
    };

    // Update frozen status in DB
    sqlx::query("UPDATE wallet_accounts SET frozen = $1 WHERE wallet_id = $2")
        .bind(frozen)
        .bind(&wallet_id)
        .execute(&state.db)
        .await
        .map_err(|e| WalletError::InternalError(format!("DB error: {}", e)))?;

    // Invalidate policy cache so next check-policy picks up new freeze status from contract
    state.policy_cache.invalidate(&wallet_id).await;

    info!(
        "Wallet frozen change sync: wallet={} frozen={}",
        wallet_id, frozen
    );

    Ok(Json(serde_json::json!({"ok": true, "wallet_id": wallet_id, "frozen": frozen})))
}

/// Helper: decrypt wallet policy via keystore check-policy endpoint.
/// Returns the decrypted policy JSON.
async fn keystore_decrypt_policy(
    state: &WalletState,
    wallet_id: &str,
    encrypted_data: &str,
) -> Result<serde_json::Value, WalletError> {
    let keystore_url = state.keystore_base_url.as_ref().ok_or_else(|| {
        WalletError::InternalError("Keystore URL not configured".to_string())
    })?;

    // Use check-policy with inline encrypted_policy_data and a noop action
    let payload = KeystoreCheckPolicyRequest {
        wallet_id: wallet_id.to_string(),
        action: serde_json::json!({"type": "sync"}),
        encrypted_policy_data: Some(encrypted_data.to_string()),
    };

    let mut request = state
        .http_client
        .post(format!("{}/wallet/check-policy", keystore_url))
        .json(&payload);

    if let Some(ref token) = state.keystore_auth_token {
        request = request.header("Authorization", format!("Bearer {}", token));
    }

    let response = request.send().await.map_err(|e| {
        WalletError::KeystoreError(format!("Failed to connect to keystore: {}", e))
    })?;

    if !response.status().is_success() {
        let body = response.text().await.unwrap_or_default();
        return Err(WalletError::KeystoreError(format!(
            "Keystore check-policy failed: {}",
            body
        )));
    }

    let check_response: KeystoreCheckPolicyResponse =
        response.json().await.map_err(|e| {
            WalletError::KeystoreError(format!("Invalid keystore response: {}", e))
        })?;

    check_response.policy.ok_or_else(|| {
        WalletError::KeystoreError("Keystore returned no policy data".to_string())
    })
}

// ============================================================================
// Helpers
// ============================================================================

/// Extract client IP from request headers (X-Forwarded-For, X-Real-IP, fallback "unknown")
fn extract_client_ip(headers: &HeaderMap) -> String {
    if let Some(forwarded) = headers.get("x-forwarded-for").and_then(|v| v.to_str().ok()) {
        if let Some(ip) = forwarded.split(',').next() {
            return ip.trim().to_string();
        }
    }
    if let Some(real_ip) = headers.get("x-real-ip").and_then(|v| v.to_str().ok()) {
        return real_ip.trim().to_string();
    }
    "unknown".to_string()
}

pub(crate) fn validate_chain(chain: &str) -> Result<(), WalletError> {
    match chain {
        // v1: Only NEAR is supported. Ethereum derivation uses Ed25519 (incorrect —
        // Ethereum requires secp256k1), and Solana via Intents is untested.
        // Funds sent to derived Ethereum addresses would be permanently lost.
        "near" => Ok(()),
        "ethereum" | "solana" => Err(WalletError::UnsupportedChain(format!(
            "{} chain is not yet supported in wallet v1. Only 'near' is available.",
            chain,
        ))),
        _ => Err(WalletError::UnsupportedChain(chain.to_string())),
    }
}

pub(crate) fn sha256_hex(data: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data.as_bytes());
    hex::encode(hasher.finalize())
}

// ============================================================================
// DB row types
// ============================================================================

#[derive(Debug, sqlx::FromRow)]
struct RequestRow {
    request_id: Uuid,
    #[allow(dead_code)]
    wallet_id: String,
    request_type: String,
    status: String,
    result_data: Option<serde_json::Value>,
    intents_ref: Option<String>,
    created_at: chrono::DateTime<chrono::Utc>,
    updated_at: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Debug, sqlx::FromRow)]
struct ApprovalRow {
    id: Uuid,
    #[allow(dead_code)]
    wallet_id: String,
    request_type: String,
    request_data: serde_json::Value,
    required_approvals: i32,
    #[allow(dead_code)]
    status: String,
    expires_at: chrono::DateTime<chrono::Utc>,
    #[allow(dead_code)]
    created_at: chrono::DateTime<chrono::Utc>,
    linked_request_id: Option<Uuid>,
    approval_count: i32,
}

// ============================================================================
// GET /pending_approvals_by_pubkey?near_pubkey=... — public, read-only
// ============================================================================
/// Public endpoint for dashboard badge: returns pending approval count
/// for wallets owned by a given on-chain public key. Rate-limited by IP.
/// No auth required — only returns non-sensitive metadata (approval IDs, types, counts).
pub async fn pending_approvals_by_pubkey(
    State(state): State<WalletState>,
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> Result<Json<serde_json::Value>, WalletError> {
    let near_pubkey = params
        .get("near_pubkey")
        .ok_or_else(|| WalletError::InternalError("Missing near_pubkey parameter".to_string()))?;

    let wallet_id = sqlx::query_scalar::<_, String>(
        "SELECT wallet_id FROM wallet_accounts WHERE near_pubkey = $1",
    )
    .bind(near_pubkey)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| WalletError::InternalError(format!("DB error: {}", e)))?;

    let Some(wallet_id) = wallet_id else {
        return Ok(Json(serde_json::json!({
            "near_pubkey": near_pubkey,
            "pending_approvals": [],
        })));
    };

    let approvals = sqlx::query_as::<_, (Uuid, String, i32, chrono::DateTime<chrono::Utc>, i64, String, chrono::DateTime<chrono::Utc>)>(
        r#"
        SELECT pa.id, pa.request_type, pa.required_approvals, pa.expires_at,
               (SELECT COUNT(*) FROM wallet_approval_signatures WHERE approval_id = pa.id) as approved_count,
               pa.request_hash, pa.created_at
        FROM wallet_pending_approvals pa
        WHERE pa.wallet_id = $1 AND pa.status = 'pending' AND pa.expires_at > NOW()
        ORDER BY pa.created_at DESC
        LIMIT 20
        "#,
    )
    .bind(&wallet_id)
    .fetch_all(&state.db)
    .await
    .map_err(|e| WalletError::InternalError(format!("DB error: {}", e)))?;

    let pending: Vec<serde_json::Value> = approvals
        .iter()
        .map(|a| {
            serde_json::json!({
                "id": a.0.to_string(),
                "request_type": a.1,
                "required_approvals": a.2,
                "approved_count": a.4,
                "expires_at": a.3.to_rfc3339(),
                "request_hash": a.5,
                "created_at": a.6.to_rfc3339(),
            })
        })
        .collect();

    Ok(Json(serde_json::json!({
        "near_pubkey": near_pubkey,
        "pending_approvals": pending,
    })))
}

// ============================================================================
// GET /approval/:approval_id — public, read-only detail for a single approval
// ============================================================================
/// Public endpoint for dashboard approval detail page.
/// Returns non-sensitive metadata: type, status, approvers, expiry.
/// No auth required — rate-limited by IP.
pub async fn get_approval_detail(
    State(state): State<WalletState>,
    axum::extract::Path(approval_id): axum::extract::Path<String>,
) -> Result<Json<serde_json::Value>, WalletError> {
    let approval_uuid: Uuid = approval_id
        .parse()
        .map_err(|_| WalletError::ApprovalNotFound)?;

    let approval = sqlx::query_as::<_, (Uuid, String, String, serde_json::Value, String, i32, String, chrono::DateTime<chrono::Utc>, chrono::DateTime<chrono::Utc>)>(
        r#"
        SELECT id, wallet_id, request_type, request_data, status, required_approvals, request_hash, expires_at, created_at
        FROM wallet_pending_approvals WHERE id = $1
        "#,
    )
    .bind(approval_uuid)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| WalletError::InternalError(format!("DB error: {}", e)))?
    .ok_or(WalletError::ApprovalNotFound)?;

    let approvers = sqlx::query_as::<_, (String, String, String, chrono::DateTime<chrono::Utc>)>(
        "SELECT approver_id, approver_role, signature, created_at FROM wallet_approval_signatures WHERE approval_id = $1",
    )
    .bind(approval_uuid)
    .fetch_all(&state.db)
    .await
    .map_err(|e| WalletError::InternalError(format!("DB error: {}", e)))?;

    Ok(Json(serde_json::json!({
        "id": approval.0.to_string(),
        "wallet_id": approval.1,
        "request_type": approval.2,
        "request_data": approval.3,
        "status": approval.4,
        "required_approvals": approval.5,
        "request_hash": approval.6,
        "expires_at": approval.7.to_rfc3339(),
        "created_at": approval.8.to_rfc3339(),
        "approvers": approvers.iter().map(|a| serde_json::json!({
            "approver_id": a.0,
            "approver_role": a.1,
            "signature": a.2,
            "created_at": a.3.to_rfc3339(),
        })).collect::<Vec<_>>(),
    })))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_chain_valid() {
        assert!(validate_chain("near").is_ok());
    }

    #[test]
    fn test_validate_chain_disabled() {
        // Ethereum and Solana are disabled in v1 (derivation not production-ready)
        assert!(matches!(validate_chain("ethereum"), Err(WalletError::UnsupportedChain(_))));
        assert!(matches!(validate_chain("solana"), Err(WalletError::UnsupportedChain(_))));
    }

    #[test]
    fn test_validate_chain_invalid() {
        let result = validate_chain("bitcoin");
        assert!(matches!(result, Err(WalletError::UnsupportedChain(_))));
        let result = validate_chain("");
        assert!(matches!(result, Err(WalletError::UnsupportedChain(_))));
    }

    #[test]
    fn test_sha256_hex_known_value() {
        // SHA-256("test") = 9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08
        let hash = sha256_hex("test");
        assert_eq!(
            hash,
            "9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08"
        );
    }
}
