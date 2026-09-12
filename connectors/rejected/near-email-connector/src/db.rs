//! The near.email database service client.
//!
//! A COPY of `wasi-examples/near-email/wasi-near-email-ark/src/db.rs`: that
//! module serves the website and is not ours to change, and the protocol it
//! speaks is the service's, not ours to redesign. Every request shape here is
//! the one the service accepts today.
//!
//! One rule with security weight: the host is a constant. Nothing a caller
//! sends can point a request anywhere but `mail.near.email`, which is also the
//! only host the manifest declares.
use crate::types::*;
use std::time::Duration;
use wasi_http_client::Client;

/// HTTP request timeout
const TIMEOUT: Duration = Duration::from_secs(30);

/// Send email via SMTP relay
#[allow(dead_code)]
pub fn send_email(
    api_url: &str,
    from_account: &str,
    to: &str,
    subject: &str,
    body_text: &str,
    api_secret: Option<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
    send_email_with_attachments(api_url, from_account, to, subject, body_text, &[], api_secret)
}

/// Send email with attachments via SMTP relay
///
/// Uses chunked HTTP writes to bypass the 4KB limit in wasi-http's blocking_write_and_flush
pub fn send_email_with_attachments(
    api_url: &str,
    from_account: &str,
    to: &str,
    subject: &str,
    body_text: &str,
    attachments: &[crate::types::Attachment],
    api_secret: Option<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
    let url = format!("{}/send", api_url);
    eprintln!("[send_email] url={}, attachments={}", url, attachments.len());

    let payload = serde_json::json!({
        "from_account": from_account,
        "to": to,
        "subject": subject,
        "body": body_text,
        "attachments": attachments,
    });

    let body_data = serde_json::to_vec(&payload)?;
    eprintln!("[send_email] body_len={} bytes ({} KB)", body_data.len(), body_data.len() / 1024);

    // Use chunked HTTP client for large bodies (bypasses 4KB limit)
    let response = crate::http_chunked::post_chunked(
        &url,
        "application/json",
        &body_data,
        TIMEOUT,
        api_secret,
    )?;

    eprintln!("[send_email] response status={}", response.status());

    if response.status() != 200 {
        let body_str = String::from_utf8_lossy(response.body());
        return Err(format!("Send email failed: {} - {}", response.status(), body_str).into());
    }

    Ok(())
}

/// Delete email from database
pub fn delete_email(
    api_url: &str,
    email_id: &str,
    account_id: &str,
    api_secret: Option<&str>,
) -> Result<bool, Box<dyn std::error::Error>> {
    let url = format!("{}/emails/{}", api_url, email_id);

    let payload = serde_json::json!({
        "account_id": account_id,
    });

    let body_data = serde_json::to_vec(&payload)?;
    let mut request = Client::new()
        .delete(&url)
        .header("Content-Type", "application/json")
        .body(&body_data)
        .connect_timeout(TIMEOUT);

    if let Some(secret) = api_secret {
        request = request.header("X-API-Secret", secret);
    }

    let response = request.send()?;

    if response.status() != 200 {
        return Err(format!("Delete email failed: {}", response.status()).into());
    }

    let body = response.body()?;
    let result: DbGenericResponse = serde_json::from_slice(&body)?;

    Ok(result.deleted)
}

/// Store internal email (already encrypted)
/// Used for NEAR-to-NEAR messaging without external SMTP
pub fn store_internal_email(
    api_url: &str,
    recipient: &str,
    sender_email: &str,
    encrypted_data: &[u8],
    api_secret: Option<&str>,
) -> Result<String, Box<dyn std::error::Error>> {
    use base64::{engine::general_purpose::STANDARD, Engine};

    let url = format!("{}/internal-store", api_url);

    let payload = serde_json::json!({
        "recipient": recipient,
        "sender_email": sender_email,
        "encrypted_data": STANDARD.encode(encrypted_data),
    });

    let body_data = serde_json::to_vec(&payload)?;

    // Use chunked HTTP for large bodies
    let response = crate::http_chunked::post_chunked(
        &url,
        "application/json",
        &body_data,
        TIMEOUT,
        api_secret,
    )?;

    if response.status() != 200 {
        return Err(format!("Store internal email failed: {}", response.status()).into());
    }

    // Parse response to get the email ID
    let result: serde_json::Value = serde_json::from_slice(response.body())?;
    let id = result["id"].as_str().unwrap_or("unknown").to_string();

    Ok(id)
}

/// Store sent email (already encrypted)
/// If `email_id` is provided, the database will use it; otherwise it generates a new UUID.
/// This is used for lazy attachment support where we need to pre-store attachments with the email_id.
pub fn store_sent_email(
    api_url: &str,
    sender: &str,
    recipient_email: &str,
    encrypted_data: &[u8],
    tx_hash: Option<&str>,
    email_id: Option<&str>,
    api_secret: Option<&str>,
) -> Result<String, Box<dyn std::error::Error>> {
    use base64::{engine::general_purpose::STANDARD, Engine};

    let url = format!("{}/store-sent", api_url);

    let payload = serde_json::json!({
        "sender": sender,
        "recipient_email": recipient_email,
        "encrypted_data": STANDARD.encode(encrypted_data),
        "tx_hash": tx_hash,
        "id": email_id,
    });

    let body_data = serde_json::to_vec(&payload)?;

    // Use chunked HTTP for large bodies
    let response = crate::http_chunked::post_chunked(
        &url,
        "application/json",
        &body_data,
        TIMEOUT,
        api_secret,
    )?;

    if response.status() != 200 {
        return Err(format!("Store sent email failed: {}", response.status()).into());
    }

    let result: DbStoreSentResponse = serde_json::from_slice(response.body())?;

    Ok(result.id)
}

/// Fetch combined inbox + sent emails in a single request
/// This is more efficient than separate fetch_emails + fetch_sent_emails calls
pub fn request_email(
    api_url: &str,
    account_id: &str,
    inbox_limit: i64,
    inbox_offset: i64,
    sent_limit: i64,
    sent_offset: i64,
    api_secret: Option<&str>,
    need_poll_token: bool,
) -> Result<crate::types::DbRequestEmailResponse, Box<dyn std::error::Error>> {
    let url = format!(
        "{}/request-email?account_id={}&inbox_limit={}&inbox_offset={}&sent_limit={}&sent_offset={}&need_poll_token={}",
        api_url, account_id, inbox_limit, inbox_offset, sent_limit, sent_offset, need_poll_token
    );

    let mut request = Client::new()
        .get(&url)
        .connect_timeout(TIMEOUT);

    if let Some(secret) = api_secret {
        request = request.header("X-API-Secret", secret);
    }

    let response = request.send()?;

    if response.status() != 200 {
        return Err(format!("Database API error: {}", response.status()).into());
    }

    let body = response.body()?;
    let result: crate::types::DbRequestEmailResponse = serde_json::from_slice(&body)?;

    Ok(result)
}

/// Store an attachment for lazy loading
pub fn store_attachment(
    api_url: &str,
    email_id: &str,
    folder: &str,
    recipient: &str,
    filename: &str,
    content_type: &str,
    size: usize,
    encrypted_data: &[u8],
    api_secret: Option<&str>,
) -> Result<String, Box<dyn std::error::Error>> {
    use base64::{engine::general_purpose::STANDARD, Engine};

    let url = format!("{}/attachments", api_url);

    let payload = serde_json::json!({
        "email_id": email_id,
        "folder": folder,
        "recipient": recipient,
        "filename": filename,
        "content_type": content_type,
        "size": size,
        "encrypted_data": STANDARD.encode(encrypted_data),
    });

    let body_data = serde_json::to_vec(&payload)?;

    // Use chunked HTTP for large bodies (attachments can be big)
    let response = crate::http_chunked::post_chunked(
        &url,
        "application/json",
        &body_data,
        TIMEOUT,
        api_secret,
    )?;

    if response.status() != 200 {
        return Err(format!("Store attachment failed: {}", response.status()).into());
    }

    let result: crate::types::DbStoreAttachmentResponse = serde_json::from_slice(response.body())?;

    Ok(result.id)
}

/// Fetch an attachment by ID
pub fn fetch_attachment(
    api_url: &str,
    attachment_id: &str,
    recipient: &str,
    api_secret: Option<&str>,
) -> Result<crate::types::DbAttachmentResponse, Box<dyn std::error::Error>> {
    let url = format!("{}/attachments/{}?recipient={}", api_url, attachment_id, recipient);

    let mut request = Client::new()
        .get(&url)
        .connect_timeout(TIMEOUT);

    if let Some(secret) = api_secret {
        request = request.header("X-API-Secret", secret);
    }

    let response = request.send()?;

    if response.status() == 404 {
        return Err("Attachment not found".into());
    }

    if response.status() != 200 {
        return Err(format!("Fetch attachment failed: {}", response.status()).into());
    }

    let body = response.body()?;
    let result: crate::types::DbAttachmentResponse = serde_json::from_slice(&body)?;

    Ok(result)
}
