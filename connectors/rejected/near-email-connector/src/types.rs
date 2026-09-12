//! The wire types the near.email database service speaks.
//!
//! A COPY of the matching types in
//! `wasi-examples/near-email/wasi-near-email-ark/src/types.rs`, trimmed to what
//! travels to or from that service. The old module's own request and response
//! vocabulary is deliberately absent: this connector has its own door.
//!
//! These must stay in step with the service. A field renamed there and not here
//! fails silently, because serde fills a default — which is why nothing below
//! is "tidied up" from what the service documents.

use serde::{Deserialize, Serialize};

/// Deserialize a base64 string as bytes, as the service sends attachment data.
fn deserialize_base64<'de, D>(deserializer: D) -> Result<Vec<u8>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use base64::{engine::general_purpose::STANDARD, Engine};
    let s: String = Deserialize::deserialize(deserializer)?;
    STANDARD.decode(&s).map_err(serde::de::Error::custom)
}

/// Attachment metadata with base64-encoded content or lazy loading reference
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Attachment {
    pub filename: String,
    pub content_type: String,
    /// Base64-encoded attachment data (for small attachments < 2KB)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<String>,
    /// Size in bytes. Serialised always, because the service and the web UI
    /// read it; accepted as absent, because a caller that already supplied
    /// `data` should not have to count it — this connector fills it in
    /// (`main.rs`, `fill_attachment_sizes`) before anything is stored, since the
    /// number decides whether an attachment is stored inline or on its own.
    #[serde(default)]
    pub size: usize,
    /// Attachment ID for lazy loading (for large attachments >= 2KB)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attachment_id: Option<String>,
}

/// Decrypted payload for SendEmail request
/// Contains all email fields encrypted together to keep recipient private
#[derive(Debug, Deserialize)]
pub struct SendEmailPayload {
    pub to: String,
    pub subject: String,
    pub body: String,
    #[serde(default)]
    pub attachments: Vec<Attachment>,
}

#[derive(Debug, Serialize)]
pub struct Email {
    pub id: String,
    pub from: String,
    pub subject: String,
    pub body: String,
    pub received_at: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub attachments: Vec<Attachment>,
}

#[derive(Debug, Serialize)]
pub struct SentEmail {
    pub id: String,
    pub to: String,
    pub subject: String,
    pub body: String,
    pub tx_hash: Option<String>,
    pub sent_at: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub attachments: Vec<Attachment>,
}

#[derive(Debug, Deserialize)]
pub struct EncryptedEmail {
    pub id: String,
    pub sender_email: String,
    #[serde(deserialize_with = "deserialize_base64")]
    pub encrypted_data: Vec<u8>,
    pub received_at: String,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct DbGenericResponse {
    pub success: bool,
    #[serde(default)]
    pub deleted: bool,
}

#[derive(Debug, Deserialize)]
pub struct EncryptedSentEmail {
    pub id: String,
    pub recipient_email: String,
    #[serde(deserialize_with = "deserialize_base64")]
    pub encrypted_data: Vec<u8>,
    pub tx_hash: Option<String>,
    pub sent_at: String,
}

/// Combined inbox + sent emails response from /request-email endpoint
#[derive(Debug, Deserialize)]
pub struct DbRequestEmailResponse {
    pub inbox: Vec<EncryptedEmail>,
    pub sent: Vec<EncryptedSentEmail>,
    pub inbox_count: i64,
    pub sent_count: i64,
    /// Poll token for lightweight /poll/count endpoint (if requested via need_poll_token)
    #[serde(default)]
    pub poll_token: Option<String>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct DbStoreSentResponse {
    pub success: bool,
    pub id: String,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct DbStoreAttachmentResponse {
    pub success: bool,
    pub id: String,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct DbAttachmentResponse {
    pub id: String,
    pub email_id: String,
    pub folder: String,
    pub filename: String,
    pub content_type: String,
    pub size: i32,
    #[serde(deserialize_with = "deserialize_base64")]
    pub encrypted_data: Vec<u8>,
}

/// Combined email data returned in encrypted_data field
#[derive(Debug, Serialize)]
pub struct EmailData {
    pub inbox: Vec<Email>,
    pub sent: Vec<SentEmail>,
    /// Poll token for lightweight /poll/count endpoint (encrypted along with emails)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub poll_token: Option<String>,
}
