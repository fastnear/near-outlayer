//! near.email as an OutLayer connector: an agent sends mail from its NEAR
//! account's own address and reads that account's mailbox, priced per named
//! operation.
//!
//! The mailbox is sealed. What the service stores is encrypted to a key this
//! enclave derives from the account (`crypto.rs`), so a call that cannot
//! authenticate as the account reads nothing — and neither can the service
//! itself, nor we.
//!
//! | `operation` | class | what it does |
//! |---|---|---|
//! | `status` | read | how many messages the mailbox holds, and that the service answers |
//! | `send_pubkey` | read | the key to seal a message to before `send` |
//! | `list` | read | inbox and sent, sealed to an ephemeral key the caller supplies |
//! | `read_attachment` | read | one attachment, sealed the same way |
//! | `send` | write | send a message with no attachments |
//! | `send_with_attachment` | write | the same, carrying attachments |
//! | `delete` | write | delete one message |
//!
//! The author's own credential — the master key every derivation starts from —
//! is named by the manifest (`author_secrets`) and decrypted into this run by
//! the worker. No caller supplies it and no caller can ask for it.
//!
//! What this connector never does: reach a host the caller names (the service
//! is a constant), echo a key, or offer the owner's key maintenance as an
//! operation. A connector operation is callable by anybody who pays, and key
//! maintenance is not for sale.

mod crypto;
mod db;
mod http_chunked;
mod mail;
mod types;

use base64::{engine::general_purpose::STANDARD, Engine};
use libsecp256k1::SecretKey;
use outlayer::env;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use types::*;

/// The manifest, in a custom section so the on-chain hash covers it: the
/// connector id, the one host this module may reach, the operations it sells
/// and the author secret it needs.
///
/// Gated to wasm because Mach-O rejects a section name with no segment; the
/// artefact that ships is the wasm, and `build.sh` fails when the section is
/// missing from it.
#[cfg(target_family = "wasm")]
#[used]
#[link_section = "outlayer.manifest"]
static OUTLAYER_MANIFEST: [u8; include_bytes!("../manifest.json").len()] =
    *include_bytes!("../manifest.json");

/// The author's secret holding the master key, as the manifest's
/// `author_secrets` profile delivers it.
///
/// It is the SAME key the near.email service has always derived from: every
/// mailbox is sealed to a key derived from it, so a different one would read
/// nothing and write what nothing else can read.
const MASTER_KEY_ENV: &str = "PROTECTED_MASTER_KEY";
/// Optional shared secret the mailbox service checks on writes.
const API_SECRET_ENV: &str = "API_SECRET";

#[derive(Serialize)]
struct Envelope {
    success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    output: Option<Value>,
    logs: Vec<Value>,
    operation: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
struct Input {
    operation: String,
    /// `list`, `read_attachment`, `delete`: the caller's throwaway key, so the
    /// reply is readable only by whoever asked.
    ephemeral_pubkey: Option<String>,
    /// `send`: a sealed `{to, subject, body, attachments}`. Preferred — with it
    /// the recipient and the body never leave the enclave in the clear.
    encrypted_data: Option<String>,
    /// `send`: the same fields in the open. On the chain path they become
    /// public chain state forever.
    to: Option<String>,
    subject: Option<String>,
    body: Option<String>,
    #[serde(default)]
    attachments: Vec<Attachment>,
    /// `read_attachment`
    attachment_id: Option<String>,
    /// `delete`
    email_id: Option<String>,
    /// `list`: paging and how much of the reply may be mail.
    inbox_offset: Option<i64>,
    sent_offset: Option<i64>,
    max_output_size: Option<usize>,
}

fn main() {
    // Storage is imported by being called: a project context needs it.
    let _ = outlayer::storage::has("_init");

    let raw = env::input();
    let envelope = match serde_json::from_slice::<Input>(&raw) {
        Err(e) => Envelope {
            success: false,
            operation: String::new(),
            error: Some(format!("input is not the expected JSON: {e}")),
            output: None,
            logs: Vec::new(),
        },
        Ok(input) => {
            let op = input.operation.trim().to_string();
            match run(&op, &input) {
                Ok(output) => Envelope { success: true, operation: op, error: None, output: Some(output), logs: Vec::new() },
                Err(e) => Envelope { success: false, operation: op, error: Some(e), output: None, logs: Vec::new() },
            }
        }
    };
    let _ = env::output_json(&envelope);
}

/// Everything this connector sells. A name not here is refused with the list,
/// which is also the check that the price table and this code agree: the
/// coordinator would have refused an unpriced operation first, so reaching
/// here means they have drifted.
const OPERATIONS: &[&str] = &[
    "status",
    "send_pubkey",
    "list",
    "read_attachment",
    "send",
    "send_with_attachment",
    "delete",
];

fn run(op: &str, input: &Input) -> Result<Value, String> {
    match op {
        "status" => status(),
        "send_pubkey" => send_pubkey(),
        "list" => list(input),
        "read_attachment" => read_attachment(input),
        "send" => send(input, false),
        "send_with_attachment" => send(input, true),
        "delete" => delete(input),
        "" => Err(format!("no `operation` in the input. This connector sells: {}", OPERATIONS.join(", "))),
        other => Err(format!("unknown operation `{other}`. This connector sells: {}", OPERATIONS.join(", "))),
    }
}

// ==================== what every operation needs ====================

/// The account this call acts for. Set by the platform from the call's own
/// credential, never by the body: an agent cannot send as somebody else.
fn signer() -> Result<String, String> {
    env::signer_account_id().ok_or_else(|| {
        "this call carries no account. near.email addresses belong to NEAR accounts, so the \
         call must come from one (a NEAR transaction, or an HTTPS call whose payment key names it)"
            .to_string()
    })
}

/// The master key, from the author's secret the manifest names.
fn master_key() -> Result<SecretKey, String> {
    let hex = std::env::var(MASTER_KEY_ENV).map_err(|_| {
        "the master key is missing from this run. It is the AUTHOR's secret, stored under the \
         accessor Project(<this project>) with the owner and profile the manifest's \
         author_secrets names, and decrypted by the worker on every run. Nothing a caller sends \
         can stand in for it."
            .to_string()
    })?;
    crypto::parse_private_key(&hex).map_err(|e| format!("the master key could not be read: {e}"))
}

fn api_secret() -> Option<String> {
    std::env::var(API_SECRET_ENV).ok().filter(|s| !s.is_empty())
}

/// The account must be a named one on this network: an implicit (64-hex)
/// account has no address to speak of.
fn sender_account() -> Result<String, String> {
    let account = signer()?;
    mail::validate_sender_account(&account, mail::account_suffix()).map_err(|e| e.to_string())?;
    Ok(account)
}

fn required<'a>(field: Option<&'a String>, name: &str) -> Result<&'a str, String> {
    field
        .map(String::as_str)
        .filter(|s| !s.trim().is_empty())
        .ok_or_else(|| format!("`{name}` is required"))
}

/// The caller's throwaway public key, which the reply is sealed to.
fn ephemeral(input: &Input) -> Result<Vec<u8>, String> {
    let hex_key = required(input.ephemeral_pubkey.as_ref(), "ephemeral_pubkey")?;
    let bytes = hex::decode(hex_key).map_err(|e| format!("`ephemeral_pubkey` is not hex: {e}"))?;
    if bytes.len() != 33 {
        return Err(format!(
            "`ephemeral_pubkey` is {} bytes; it must be a 33-byte compressed secp256k1 key",
            bytes.len()
        ));
    }
    Ok(bytes)
}

fn seal_to_caller(ephemeral_pubkey: &[u8], plaintext: &[u8]) -> Result<String, String> {
    let sealed = ecies::encrypt(ephemeral_pubkey, plaintext)
        .map_err(|e| format!("the reply could not be sealed to `ephemeral_pubkey`: {e}"))?;
    Ok(STANDARD.encode(&sealed))
}

// ==================== reads ====================

fn status() -> Result<Value, String> {
    let account = signer()?;
    let counts = db::request_email(mail::DATABASE_API_URL, &account, 0, 0, 0, 0, api_secret().as_deref(), false)
        .map_err(|e| format!("the mailbox service did not answer: {e}"))?;
    let named = mail::validate_sender_account(&account, mail::account_suffix()).is_ok();
    let address = named.then(|| {
        let local = account.strip_suffix(mail::account_suffix()).unwrap_or(&account);
        format!("{local}@near.email")
    });
    Ok(json!({
        "account_id": account,
        "address": address,
        "can_send": named,
        "inbox_count": counts.inbox_count,
        "sent_count": counts.sent_count,
        "note": if named { "ready" } else { "this account cannot have an address: only named accounts of this network can" },
    }))
}

fn send_pubkey() -> Result<Value, String> {
    let account = sender_account()?;
    let key = crypto::derive_user_pubkey(&master_key()?, &account)
        .map_err(|e| format!("the send key could not be derived: {e}"))?;
    Ok(json!({
        "account_id": account,
        "send_pubkey": hex::encode(&key),
        "note": "seal {to, subject, body, attachments} to this key and pass it as `encrypted_data`; it is the same key for this account forever, so it can be cached",
    }))
}

fn list(input: &Input) -> Result<Value, String> {
    let account = signer()?;
    let ephemeral_pubkey = ephemeral(input)?;
    let max_size = input.max_output_size.unwrap_or(mail::DEFAULT_MAX_OUTPUT_SIZE);
    let user_privkey = crypto::derive_user_privkey(&master_key()?, &account)
        .map_err(|e| format!("the mailbox key could not be derived: {e}"))?;
    let result = mail::fetch_request_email(
        mail::DATABASE_API_URL,
        &account,
        &user_privkey,
        max_size,
        input.inbox_offset.unwrap_or(0),
        input.sent_offset.unwrap_or(0),
        api_secret().as_deref(),
        false,
    )
    .map_err(|e| format!("the mailbox could not be read: {e}"))?;
    let plaintext = serde_json::to_vec(&result.email_data).map_err(|e| format!("reply encoding: {e}"))?;
    Ok(json!({
        "account_id": account,
        "inbox_count": result.inbox_count,
        "sent_count": result.sent_count,
        "inbox_next_offset": result.inbox_next,
        "sent_next_offset": result.sent_next,
        "encrypted_data": seal_to_caller(&ephemeral_pubkey, &plaintext)?,
        "note": "decrypt `encrypted_data` with the private half of `ephemeral_pubkey`; large attachments are named, not inlined — fetch one with read_attachment",
    }))
}

fn read_attachment(input: &Input) -> Result<Value, String> {
    let account = signer()?;
    let ephemeral_pubkey = ephemeral(input)?;
    let attachment_id = required(input.attachment_id.as_ref(), "attachment_id")?;
    let user_privkey = crypto::derive_user_privkey(&master_key()?, &account)
        .map_err(|e| format!("the mailbox key could not be derived: {e}"))?;
    let attachment = db::fetch_attachment(mail::DATABASE_API_URL, attachment_id, &account, api_secret().as_deref())
        .map_err(|e| format!("the attachment could not be fetched: {e}"))?;
    let plaintext = crypto::decrypt_email(&user_privkey, &attachment.encrypted_data)
        .map_err(|e| format!("the attachment could not be decrypted for this account: {e}"))?;
    Ok(json!({
        "filename": attachment.filename,
        "content_type": attachment.content_type,
        "size": attachment.size,
        "encrypted_data": seal_to_caller(&ephemeral_pubkey, &plaintext)?,
    }))
}

// ==================== writes ====================

fn delete(input: &Input) -> Result<Value, String> {
    let account = signer()?;
    let email_id = required(input.email_id.as_ref(), "email_id")?;
    let deleted = db::delete_email(mail::DATABASE_API_URL, email_id, &account, api_secret().as_deref())
        .map_err(|e| format!("the message could not be deleted: {e}"))?;
    Ok(json!({"email_id": email_id, "deleted": deleted}))
}

/// What a send is made of, however the caller supplied it.
#[derive(Debug)]
struct Outgoing {
    to: String,
    subject: String,
    body: String,
    attachments: Vec<Attachment>,
    /// True when the fields arrived in the open, which on the chain path means
    /// they are public chain state forever.
    was_plaintext: bool,
}

/// Read the message out of the request: a sealed payload if there is one, the
/// flat fields otherwise.
fn outgoing(input: &Input, master: &SecretKey, account: &str) -> Result<Outgoing, String> {
    match input.encrypted_data.as_deref() {
        Some(sealed) if !sealed.trim().is_empty() => {
            let ciphertext = STANDARD.decode(sealed).map_err(|e| format!("`encrypted_data` is not base64: {e}"))?;
            let user_privkey = crypto::derive_user_privkey(master, account)
                .map_err(|e| format!("the sending key could not be derived: {e}"))?;
            let plaintext = crypto::decrypt_email(&user_privkey, &ciphertext).map_err(|e| {
                format!("`encrypted_data` could not be decrypted for this account: {e}. Seal it to the key `send_pubkey` returns")
            })?;
            let payload: SendEmailPayload = serde_json::from_slice(&plaintext)
                .map_err(|e| format!("the sealed payload is not {{to, subject, body, attachments}}: {e}"))?;
            Ok(Outgoing {
                to: payload.to,
                subject: payload.subject,
                body: payload.body,
                attachments: payload.attachments,
                was_plaintext: false,
            })
        }
        _ => Ok(Outgoing {
            to: required(input.to.as_ref(), "to")?.to_string(),
            subject: input.subject.clone().unwrap_or_default(),
            body: input.body.clone().unwrap_or_default(),
            attachments: input.attachments.clone(),
            was_plaintext: true,
        }),
    }
}

/// Fill in any attachment size the caller left out, from the data it supplied.
///
/// The number is not cosmetic: it decides whether an attachment is stored
/// inline or on its own, so a zero on a real attachment would put a large blob
/// into a mailbox listing. Counting it here means a caller never has to.
fn fill_attachment_sizes(attachments: &mut [Attachment]) -> Result<(), String> {
    for att in attachments.iter_mut() {
        if att.size == 0 {
            if let Some(data) = att.data.as_deref() {
                att.size = STANDARD
                    .decode(data)
                    .map_err(|e| format!("attachment `{}` is not base64: {e}", att.filename))?
                    .len();
            }
        }
    }
    Ok(())
}

/// Send one message.
///
/// `with_attachments` is the operation the caller paid for, and it is checked
/// AFTER a sealed payload is opened: until then the attachments are ciphertext,
/// so a `send` could otherwise carry them at the cheaper price.
fn send(input: &Input, with_attachments: bool) -> Result<Value, String> {
    let account = sender_account()?;
    let master = master_key()?;
    let api_secret = api_secret();
    let mut message = outgoing(input, &master, &account)?;
    fill_attachment_sizes(&mut message.attachments)?;

    if !with_attachments && !message.attachments.is_empty() {
        return Err(format!(
            "`send` carries no attachments (this message has {}); use `send_with_attachment`, which is priced for them",
            message.attachments.len()
        ));
    }
    if message.to.trim().is_empty() {
        return Err("`to` is required".to_string());
    }

    let local = account.strip_suffix(mail::account_suffix()).unwrap_or(&account);
    let from = format!("{local}@near.email");
    let internal = message.to.trim().to_ascii_lowercase().ends_with("@near.email");

    // The body as it will be read, with the signature line the product adds.
    let body = match mail::EMAIL_SIGNATURE {
        Some(signature) => mail::insert_signature_before_quote(&message.body, &signature.replace("%account%", &account)),
        None => message.body.clone(),
    };
    let content = mail::build_email_content(&from, &message.to, &message.subject, &body, &message.attachments);

    // The sent-folder copy, sealed to the sender, with large attachments stored
    // on their own so a mailbox listing stays inside its size budget.
    let email_id = format!(
        "{:016x}{:016x}",
        mail::rand_u64(),
        mail::rand_u64()
    );
    let sent = mail::build_sent_email_with_lazy_attachments(
        mail::DATABASE_API_URL,
        &email_id,
        &account,
        &master,
        &from,
        &message.to,
        &message.subject,
        &body,
        &message.attachments,
        api_secret.as_deref(),
    )
    .map_err(|e| format!("the sent copy could not be built: {e}"))?;
    // The copy is sealed to the sender's own key: only this account reads it.
    let sealed_sent = crypto::encrypt_for_account(&master, &account, sent.json_content.as_bytes())
        .map_err(|e| format!("the sent copy could not be sealed: {e}"))?;

    let delivered = if internal {
        // Internal mail never leaves the service: it is sealed to the
        // RECIPIENT's derived key and stored.
        let recipient_account = mail::extract_account_from_email(&message.to, "@near.email", mail::account_suffix());
        let sealed_for_recipient = crypto::encrypt_for_account(&master, &recipient_account, content.as_bytes())
            .map_err(|e| format!("the message could not be sealed for the recipient: {e}"))?;
        db::store_internal_email(
            mail::DATABASE_API_URL,
            &recipient_account,
            &from,
            &sealed_for_recipient,
            api_secret.as_deref(),
        )
        .map_err(|e| format!("the message could not be delivered internally: {e}"))?;
        "internal"
    } else {
        db::send_email_with_attachments(
            mail::DATABASE_API_URL,
            &account,
            &message.to,
            &message.subject,
            // The body the recipient reads is the one with the product's
            // signature line, the same text the sent copy keeps.
            &body,
            &message.attachments,
            api_secret.as_deref(),
        )
        .map_err(|e| format!("the mailbox service refused to send it: {e}"))?;
        "external"
    };

    // The sent copy is stored after delivery: a copy of a message that never
    // went would be worse than no copy.
    let sent_stored = db::store_sent_email(
        mail::DATABASE_API_URL,
        &account,
        &message.to,
        &sealed_sent,
        env::transaction_hash().as_deref(),
        Some(&email_id),
        api_secret.as_deref(),
    )
    .is_ok();

    Ok(json!({
        "message_id": email_id,
        "from": from,
        "to": message.to,
        "delivery": delivered,
        "attachments": message.attachments.len(),
        "sealed": !message.was_plaintext,
        "sent_copy_stored": sent_stored,
        "warning": message.was_plaintext.then_some(
            "the recipient, subject and body were supplied in the open; on the on-chain path they are public chain state forever. Seal them with `encrypted_data` next time."
        ),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_operations_this_code_runs_are_the_ones_it_advertises() {
        // Every name in OPERATIONS dispatches, and the refusal for anything
        // else names the list — the same list `build.sh` checks the manifest
        // against, so the three cannot drift apart.
        for op in OPERATIONS {
            let err = run(op, &Input::default()).unwrap_err();
            assert!(!err.starts_with("unknown operation"), "{op} must dispatch, got: {err}");
        }
        for bad in ["", "migrate_master_key", "get_master_public_key", "send_email"] {
            let err = run(bad, &Input::default()).unwrap_err();
            assert!(err.contains("This connector sells: status, send_pubkey"), "{bad} → {err}");
        }
    }

    #[test]
    fn a_reply_is_sealed_only_to_a_real_key() {
        let mut input = Input::default();
        assert!(ephemeral(&input).unwrap_err().contains("required"));
        input.ephemeral_pubkey = Some("zz".into());
        assert!(ephemeral(&input).unwrap_err().contains("not hex"));
        input.ephemeral_pubkey = Some("00".repeat(20));
        assert!(ephemeral(&input).unwrap_err().contains("33-byte"));
    }

    #[test]
    fn a_plaintext_send_is_read_from_the_flat_fields() {
        let master = SecretKey::parse_slice(&[3u8; 32]).unwrap();
        let mut input = Input { to: Some("a@b.c".into()), subject: Some("s".into()), body: Some("b".into()), ..Default::default() };
        let out = outgoing(&input, &master, "alice.near").unwrap();
        assert!(out.was_plaintext);
        assert_eq!((out.to.as_str(), out.subject.as_str(), out.body.as_str()), ("a@b.c", "s", "b"));
        // A sealed payload this account cannot open is refused, and the refusal
        // says where the right key comes from.
        input.encrypted_data = Some(STANDARD.encode([1u8; 80]));
        let err = outgoing(&input, &master, "alice.near").unwrap_err();
        assert!(err.contains("send_pubkey"), "{err}");
    }

    /// An attachment's size decides whether it is stored inline or on its own,
    /// so a caller leaving it out must not mean "zero bytes".
    #[test]
    fn a_missing_attachment_size_is_counted_not_assumed() {
        let mut atts = vec![
            Attachment { filename: "a.txt".into(), content_type: "text/plain".into(), data: Some(STANDARD.encode(vec![0u8; 3000])), size: 0, attachment_id: None },
            Attachment { filename: "b.txt".into(), content_type: "text/plain".into(), data: Some(STANDARD.encode(b"hi")), size: 7, attachment_id: None },
            Attachment { filename: "c.txt".into(), content_type: "text/plain".into(), data: None, size: 0, attachment_id: Some("id".into()) },
        ];
        fill_attachment_sizes(&mut atts).unwrap();
        assert_eq!(atts[0].size, 3000, "counted from the data");
        assert_eq!(atts[1].size, 7, "a size the caller gave is left alone");
        assert_eq!(atts[2].size, 0, "nothing to count for a lazy one");
        let mut bad = vec![Attachment { filename: "x".into(), content_type: "text/plain".into(), data: Some("not base64!".into()), size: 0, attachment_id: None }];
        assert!(fill_attachment_sizes(&mut bad).unwrap_err().contains("not base64"));
    }

    #[test]
    fn a_sealed_send_round_trips_and_the_cheap_one_refuses_attachments() {
        let master = SecretKey::parse_slice(&[4u8; 32]).unwrap();
        let payload = json!({
            "to": "someone@example.com", "subject": "s", "body": "b",
            "attachments": [{"filename": "f.txt", "content_type": "text/plain", "data": "eA=="}]
        });
        let sealed = crypto::encrypt_for_account(&master, "alice.near", &serde_json::to_vec(&payload).unwrap()).unwrap();
        let input = Input { encrypted_data: Some(STANDARD.encode(&sealed)), ..Default::default() };
        let out = outgoing(&input, &master, "alice.near").unwrap();
        assert!(!out.was_plaintext);
        assert_eq!(out.attachments.len(), 1, "the attachment was inside the ciphertext");
        assert_eq!(out.to, "someone@example.com");
    }
}
