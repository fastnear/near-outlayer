// A verbatim copy: unused items are kept rather than trimmed, so a diff against
// the website's module stays readable and nothing of the wire is lost by accident.
#![allow(dead_code)]

//! The near.email behaviour an operation needs: reading a mailbox, building a
//! message, laying out the sent folder.
//!
//! A COPY of the helper half of
//! `wasi-examples/near-email/wasi-near-email-ark/src/main.rs`. That module
//! serves the website and is not ours to change, and this is its behaviour, not
//! a reinterpretation of it: the MIME it builds, the lazy-attachment layout of
//! the sent folder, the signature line, the size budget a reply must fit. A
//! connector that built these differently would produce mail the web UI shows
//! wrongly, or a sent folder it cannot read.

use crate::types::{self, *};
use crate::{crypto, db};

/// Where the mailbox service lives. A constant, so nothing a caller sends can
/// point a request elsewhere; the manifest declares this host and no other.
pub const DATABASE_API_URL: &str = "https://mail.near.email";

/// Appended to an outgoing body, with `%account%` replaced by the sender.
pub const EMAIL_SIGNATURE: Option<&str> = Some("Sent onchain by %account% via OutLayer");

/// How much of a reply may be filled with mail: 1.5 MB.
pub const DEFAULT_MAX_OUTPUT_SIZE: usize = 1_500_000;

/// `.near` on mainnet, `.testnet` elsewhere. The suffix decides which accounts
/// may have an address at all.
pub fn account_suffix() -> &'static str {
    match std::env::var("NEAR_NETWORK_ID").as_deref() {
        Ok("mainnet") => ".near",
        _ => ".testnet",
    }
}

/// Fetch result with counts
pub struct FetchResult {
    pub email_data: EmailData,
    pub inbox_next: Option<i64>,
    pub sent_next: Option<i64>,
    pub inbox_count: i64,
    pub sent_count: i64,
}

/// Fetch combined inbox + sent emails with size limit
/// Returns FetchResult with emails, next offsets, and total counts
/// Attachments are lazy-loaded by ID (stored by SMTP server)
pub fn fetch_request_email(
    api_url: &str,
    account_id: &str,
    user_privkey: &libsecp256k1::SecretKey,
    max_size: usize,
    inbox_offset: i64,
    sent_offset: i64,
    api_secret: Option<&str>,
    need_poll_token: bool,
) -> Result<FetchResult, Box<dyn std::error::Error>> {
    // Reserve some space for JSON overhead and encryption
    let effective_max = max_size.saturating_sub(10_000);
    let mut current_size: usize = 0;

    // Fetch both inbox and sent emails in a single HTTP request (includes counts)
    let combined = db::request_email(api_url, account_id, 100, inbox_offset, 100, sent_offset, api_secret, need_poll_token)?;
    let inbox_count = combined.inbox_count;
    let sent_count = combined.sent_count;
    let poll_token = combined.poll_token;

    let mut inbox_emails = Vec::new();
    let mut inbox_next: Option<i64> = None;

    for (idx, enc_email) in combined.inbox.into_iter().enumerate() {
        match crypto::decrypt_email(user_privkey, &enc_email.encrypted_data) {
            Ok(decrypted) => {
                let parsed = match parse_email(&decrypted) {
                    Ok(p) => p,
                    Err(e) => {
                        eprintln!("Failed to parse inbox email {}: {}", enc_email.id, e);
                        continue;
                    }
                };

                // Attachments are already processed by SMTP, just use them directly
                let attachments = parsed.attachments;

                // Calculate email size (all attachments are lazy, only metadata counts)
                let attachments_size: usize = attachments.iter()
                    .map(|a| {
                        let base = a.filename.len() + a.content_type.len() + 50;
                        if let Some(ref data) = a.data {
                            base + data.len()
                        } else {
                            base + 50  // Just metadata for lazy attachments
                        }
                    })
                    .sum();

                let full_email_size = enc_email.id.len() + enc_email.sender_email.len() +
                    parsed.subject.len() + parsed.body.len() + enc_email.received_at.len() +
                    attachments_size + 100;

                // Size of a truncated email (without body content and attachments)
                let truncated_size = enc_email.id.len() + enc_email.sender_email.len() +
                    parsed.subject.len() + 100 + enc_email.received_at.len() + 100;

                if current_size + full_email_size <= effective_max {
                    // Full email fits
                    let email = Email {
                        id: enc_email.id,
                        from: enc_email.sender_email,
                        subject: parsed.subject,
                        body: parsed.body,
                        received_at: enc_email.received_at,
                        attachments,
                    };
                    current_size += full_email_size;
                    inbox_emails.push(email);
                } else if current_size + truncated_size <= effective_max {
                    // Email too large, show truncated version
                    let att_count = attachments.len();
                    let att_info = if att_count > 0 {
                        format!("\n\n[{} attachment(s) not shown]", att_count)
                    } else {
                        String::new()
                    };

                    let email = Email {
                        id: enc_email.id,
                        from: enc_email.sender_email,
                        subject: parsed.subject,
                        body: format!("[Email too large to display in this view]{}", att_info),
                        received_at: enc_email.received_at,
                        attachments: Vec::new(),
                    };
                    current_size += truncated_size;
                    inbox_emails.push(email);
                } else {
                    // Can't fit even truncated version, stop here
                    inbox_next = Some(inbox_offset + idx as i64);
                    break;
                }
            }
            Err(e) => {
                eprintln!("Failed to decrypt inbox email {}: {}", enc_email.id, e);
            }
        }
    }

    // Check if there are more inbox emails
    if inbox_next.is_none() && inbox_emails.len() == 100 {
        // Fetched max limit, there might be more
        inbox_next = Some(inbox_offset + 100);
    }

    // Process sent emails (already fetched in combined request)
    let mut sent_emails = Vec::new();
    let mut sent_next: Option<i64> = None;

    for (idx, enc_email) in combined.sent.into_iter().enumerate() {
        match crypto::decrypt_email(user_privkey, &enc_email.encrypted_data) {
            Ok(decrypted) => {
                let parsed = match parse_email(&decrypted) {
                    Ok(p) => p,
                    Err(e) => {
                        eprintln!("Failed to parse sent email {}: {}", enc_email.id, e);
                        continue;
                    }
                };

                // Attachments are already processed, just use them directly
                let attachments = parsed.attachments;

                // Calculate email size (all attachments are lazy, only metadata counts)
                let attachments_size: usize = attachments.iter()
                    .map(|a| {
                        let base = a.filename.len() + a.content_type.len() + 50;
                        if let Some(ref data) = a.data {
                            base + data.len()
                        } else {
                            base + 50  // Just metadata for lazy attachments
                        }
                    })
                    .sum();

                let full_email_size = enc_email.id.len() + enc_email.recipient_email.len() +
                    parsed.subject.len() + parsed.body.len() + enc_email.sent_at.len() +
                    attachments_size + 100;

                let truncated_size = enc_email.id.len() + enc_email.recipient_email.len() +
                    parsed.subject.len() + 100 + enc_email.sent_at.len() + 100;

                if current_size + full_email_size <= effective_max {
                    // Full email fits
                    let email = SentEmail {
                        id: enc_email.id,
                        to: enc_email.recipient_email,
                        subject: parsed.subject,
                        body: parsed.body,
                        tx_hash: enc_email.tx_hash,
                        sent_at: enc_email.sent_at,
                        attachments,
                    };
                    current_size += full_email_size;
                    sent_emails.push(email);
                } else if current_size + truncated_size <= effective_max {
                    // Email too large, show truncated version
                    let att_count = attachments.len();
                    let att_info = if att_count > 0 {
                        format!("\n\n[{} attachment(s) not shown]", att_count)
                    } else {
                        String::new()
                    };

                    let email = SentEmail {
                        id: enc_email.id,
                        to: enc_email.recipient_email,
                        subject: parsed.subject,
                        body: format!("[Email too large to display in this view]{}", att_info),
                        tx_hash: enc_email.tx_hash,
                        sent_at: enc_email.sent_at,
                        attachments: Vec::new(),
                    };
                    current_size += truncated_size;
                    sent_emails.push(email);
                } else {
                    // Can't fit even truncated version, stop here
                    sent_next = Some(sent_offset + idx as i64);
                    break;
                }
            }
            Err(e) => {
                eprintln!("Failed to decrypt sent email {}: {}", enc_email.id, e);
            }
        }
    }

    // Check if there are more sent emails
    if sent_next.is_none() && sent_emails.len() == 100 {
        sent_next = Some(sent_offset + 100);
    }

    Ok(FetchResult {
        email_data: EmailData {
            inbox: inbox_emails,
            sent: sent_emails,
            poll_token,
        },
        inbox_next,
        sent_next,
        inbox_count,
        sent_count,
    })
}

/// JSON format for emails (outlayer-email)
#[derive(Debug, serde::Deserialize)]
struct EmailJson {
    format: String,
    subject: String,
    body: String,
    #[serde(default)]
    attachments: Vec<AttachmentJson>,
}

#[derive(Debug, serde::Deserialize)]
struct AttachmentJson {
    id: String,
    filename: String,
    content_type: String,
    size: usize,
}

/// Parse decrypted email content (outlayer-email JSON or MIME format)
pub fn parse_email(data: &[u8]) -> Result<ParsedEmail, Box<dyn std::error::Error>> {
    // First try JSON format (outlayer-email)
    if let Ok(json) = serde_json::from_slice::<EmailJson>(data) {
        if json.format == "outlayer-email" {
            let attachments = json.attachments.into_iter().map(|att| types::Attachment {
                filename: att.filename,
                content_type: att.content_type,
                data: None,
                size: att.size,
                attachment_id: Some(att.id),
            }).collect();

            return Ok(ParsedEmail {
                subject: json.subject,
                body: json.body,
                attachments,
            });
        }
    }

    // Fallback: try MIME format (for internal NEAR-to-NEAR emails)
    let text = String::from_utf8(data.to_vec())
        .map_err(|e| format!("Invalid UTF-8: {}", e))?;

    parse_mime_email(&text)
}

/// Parse MIME format email (RFC822 style headers + body)
pub fn parse_mime_email(text: &str) -> Result<ParsedEmail, Box<dyn std::error::Error>> {
    // Split headers and body by blank line (\r\n\r\n or \n\n)
    let (headers_str, body) = if let Some(pos) = text.find("\r\n\r\n") {
        (&text[..pos], &text[pos + 4..])
    } else if let Some(pos) = text.find("\n\n") {
        (&text[..pos], &text[pos + 2..])
    } else {
        return Err("Invalid MIME: no header/body separator found".into());
    };

    // Parse Subject header (handles multi-line folding)
    let mut subject = String::new();
    let mut in_subject = false;

    for line in headers_str.lines() {
        if line.to_lowercase().starts_with("subject:") {
            subject = line[8..].trim().to_string();
            in_subject = true;
        } else if in_subject && (line.starts_with(' ') || line.starts_with('\t')) {
            // Continuation of subject (folded header)
            subject.push(' ');
            subject.push_str(line.trim());
        } else {
            in_subject = false;
        }
    }

    // For MIME emails, we don't have lazy attachments - return empty
    // (attachments would need multipart parsing which is complex)
    Ok(ParsedEmail {
        subject,
        body: body.to_string(),
        attachments: Vec::new(),
    })
}

/// Result of building sent email content with lazy attachments
pub struct SentEmailContent {
    /// JSON string to be encrypted and stored
    pub json_content: String,
    /// Processed attachments with lazy loading info (for immediate response)
    pub attachments: Vec<types::Attachment>,
}

/// Build sent email content - stores attachments separately via db-api
pub fn build_sent_email_with_lazy_attachments(
    api_url: &str,
    email_id: &str,
    account_id: &str,
    master_privkey: &libsecp256k1::SecretKey,
    _from: &str,
    _to: &str,
    subject: &str,
    body: &str,
    attachments: &[Attachment],
    api_secret: Option<&str>,
) -> Result<SentEmailContent, Box<dyn std::error::Error>> {
    use base64::{engine::general_purpose::STANDARD, Engine};

    let mut json_attachments = Vec::new();
    let mut result_attachments = Vec::new();

    for att in attachments {
        let data_bytes = match &att.data {
            Some(b64) => STANDARD.decode(b64)?,
            None => continue,
        };

        // Always store attachments separately (same as SMTP)
        let encrypted = crypto::encrypt_for_account(master_privkey, account_id, &data_bytes)?;

        let att_id = db::store_attachment(
            api_url,
            email_id,
            "sent",
            account_id,
            &att.filename,
            &att.content_type,
            att.size,
            &encrypted,
            api_secret,
        )?;

        json_attachments.push(serde_json::json!({
            "id": att_id,
            "filename": att.filename,
            "content_type": att.content_type,
            "size": att.size
        }));
        result_attachments.push(types::Attachment {
            filename: att.filename.clone(),
            content_type: att.content_type.clone(),
            data: None,
            size: att.size,
            attachment_id: Some(att_id),
        });
    }

    let email_json = serde_json::json!({
        "format": "outlayer-email",
        "subject": subject,
        "body": body,
        "attachments": json_attachments
    });

    Ok(SentEmailContent {
        json_content: email_json.to_string(),
        attachments: result_attachments,
    })
}

pub struct ParsedEmail {
    pub subject: String,
    pub body: String,
    pub attachments: Vec<types::Attachment>,
}

/// Validate that account_id is allowed to send emails
pub fn validate_sender_account(account_id: &str, expected_suffix: &str) -> Result<(), Box<dyn std::error::Error>> {
    if account_id.len() == 64 && account_id.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(format!(
            "Implicit accounts cannot send emails. Please use a named account ending with {}",
            expected_suffix
        ).into());
    }

    if !account_id.ends_with(expected_suffix) {
        return Err(format!(
            "Account '{}' cannot send emails. Only accounts ending with '{}' are supported.",
            account_id, expected_suffix
        ).into());
    }

    Ok(())
}

/// Extract NEAR account ID from email address
pub fn extract_account_from_email(email: &str, email_suffix: &str, default_account_suffix: &str) -> String {
    let email_lower = email.to_lowercase();
    let local_part = email_lower
        .strip_suffix(email_suffix)
        .unwrap_or(&email_lower)
        .to_string();

    if local_part.contains('.') {
        local_part
    } else {
        format!("{}{}", local_part, default_account_suffix)
    }
}

/// Build email content with optional attachments (MIME multipart if needed)
pub fn build_email_content(
    from: &str,
    to: &str,
    subject: &str,
    body: &str,
    attachments: &[Attachment],
) -> String {
    if attachments.is_empty() {
        // Simple text email
        format!(
            "From: {}\r\nTo: {}\r\nSubject: {}\r\nContent-Type: text/plain; charset=utf-8\r\n\r\n{}",
            from, to, subject, body
        )
    } else {
        // MIME multipart email with attachments
        let boundary = format!("----=_Part_{:016x}", rand_u64());

        let mut result = String::new();
        result.push_str(&format!("From: {}\r\n", from));
        result.push_str(&format!("To: {}\r\n", to));
        result.push_str(&format!("Subject: {}\r\n", subject));
        result.push_str("MIME-Version: 1.0\r\n");
        result.push_str(&format!("Content-Type: multipart/mixed; boundary=\"{}\"\r\n", boundary));
        result.push_str("\r\n");

        // Body part
        result.push_str(&format!("--{}\r\n", boundary));
        result.push_str("Content-Type: text/plain; charset=utf-8\r\n");
        result.push_str("Content-Transfer-Encoding: 8bit\r\n");
        result.push_str("\r\n");
        result.push_str(body);
        result.push_str("\r\n");

        // Attachment parts
        for att in attachments {
            // For sending, attachments must have inline data
            let data = match &att.data {
                Some(d) => d,
                None => continue,  // Skip attachments without data (shouldn't happen for sending)
            };

            result.push_str(&format!("--{}\r\n", boundary));
            result.push_str(&format!("Content-Type: {}; name=\"{}\"\r\n", att.content_type, att.filename));
            result.push_str("Content-Transfer-Encoding: base64\r\n");
            result.push_str(&format!("Content-Disposition: attachment; filename=\"{}\"\r\n", att.filename));
            result.push_str("\r\n");
            // Split base64 into 76-char lines
            for chunk in data.as_bytes().chunks(76) {
                result.push_str(std::str::from_utf8(chunk).unwrap_or(""));
                result.push_str("\r\n");
            }
        }

        // End boundary
        result.push_str(&format!("--{}--\r\n", boundary));

        result
    }
}

/// Simple pseudo-random u64 for boundary generation
pub fn rand_u64() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    now.as_nanos() as u64 ^ 0xDEADBEEF
}

/// Get current timestamp in ISO 8601 format
pub fn get_current_timestamp() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let secs = now.as_secs();

    // Convert to date/time components (simplified UTC)
    let days = secs / 86400;
    let time_secs = secs % 86400;
    let hours = time_secs / 3600;
    let minutes = (time_secs % 3600) / 60;
    let seconds = time_secs % 60;

    // Calculate year/month/day from days since epoch (1970-01-01)
    let mut y = 1970;
    let mut remaining_days = days as i64;

    loop {
        let days_in_year = if is_leap_year(y) { 366 } else { 365 };
        if remaining_days < days_in_year {
            break;
        }
        remaining_days -= days_in_year;
        y += 1;
    }

    let days_in_months = if is_leap_year(y) {
        [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    } else {
        [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    };

    let mut m = 1;
    for days_in_month in days_in_months.iter() {
        if remaining_days < *days_in_month {
            break;
        }
        remaining_days -= days_in_month;
        m += 1;
    }
    let d = remaining_days + 1;

    format!("{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z", y, m, d, hours, minutes, seconds)
}

fn is_leap_year(year: i64) -> bool {
    (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0)
}

/// Insert signature before quoted text markers
pub fn insert_signature_before_quote(body: &str, signature: &str) -> String {
    let quote_markers = [
        "-------- Original Message --------",
        "---------- Forwarded message ---------",
    ];

    let mut earliest_pos: Option<usize> = None;

    for marker in &quote_markers {
        if let Some(pos) = body.find(marker) {
            earliest_pos = Some(earliest_pos.map_or(pos, |e| e.min(pos)));
        }
    }

    if let Some(on_pos) = body.find("\nOn ") {
        let after_on = &body[on_pos..];
        if after_on.contains("wrote:") || after_on.contains("написал:") {
            earliest_pos = Some(earliest_pos.map_or(on_pos, |e| e.min(on_pos)));
        }
    }

    if let Some(pos) = earliest_pos {
        let (before, after) = body.split_at(pos);
        let before_trimmed = before.trim_end();
        format!("{}\r\n\r\n--\r\n{}\r\n\r\n{}", before_trimmed, signature, after)
    } else {
        format!("{}\r\n\r\n--\r\n{}", body, signature)
    }
}

