---
name: near-email-connector
description: Send and read email from a NEAR account's own @near.email address through the `near-email` connector — sealed in the enclave, priced per operation, attachments included. Use when an agent with an OutLayer wallet must email a human or another agent, or read the replies.
---

# near.email connector

Your NEAR account has an address: `alice.near` sends and receives as
`alice@near.email`. The mailbox is sealed to a key that exists only inside the
enclave, so you read it through this connector and nothing else can — not the
service that stores it, not us.

## Call shape

```
POST https://api.outlayer.ai/call/connectors.outlayer.near/near-email
X-Payment-Key: <a payment key owned by the account whose mailbox this is>
Content-Type: application/json

{"input": {"operation": "<op>", ...}}
```

Answers are `{"success": bool, "output": {...}, "error": "..."}`. Mainnet only:
the address derives from a `.near` account.

**The mailbox is the payment key's owner.** Over HTTPS the acting account is
that owner and nothing else; on chain it is the signer of the transaction.
There is no field for naming an account, so you cannot read or send as someone
else, and a key belonging to another account opens that account's mailbox
instead of yours. No other secret, policy or wallet id is needed.

## Start here

`{"operation": "status"}` — free. Gives your `address`, `can_send`, and how
many messages are in `inbox_count` / `sent_count`. `can_send: false` means the
account is implicit (64-hex) and has no address; nothing else will work.

## Reading

Reads come back sealed to a throwaway key you generate per call, so a reply is
readable only by whoever asked for it.

1. Make a secp256k1 keypair. Send the 33-byte compressed public key as hex.
2. `{"operation": "list", "ephemeral_pubkey": "02ab…"}` — free. Returns
   `encrypted_data` (ECIES to your key, base64), plus counts and
   `inbox_next_offset` / `sent_next_offset` for paging.
3. Decrypt with the private half. Inside: inbox and sent messages, with
   headers, body, and attachments either inline or named by `attachment_id`.
4. `{"operation": "read_attachment", "attachment_id": "…", "ephemeral_pubkey": "02ab…"}`
   — free — for a named one. `encrypted_data` is the file's bytes, sealed.

`max_output_size` caps how much mail one `list` returns (bytes). Large
attachments are named rather than inlined so a listing stays inside that
budget.

## Sending

Seal the message, so the recipient, subject and body never travel in the clear:

1. `{"operation": "send_pubkey"}` — free. Returns `send_pubkey`, the same key
   for your account forever. Cache it.
2. ECIES-encrypt `{"to": "...", "subject": "...", "body": "...", "attachments": [...]}`
   to that key, base64 it, and send
   `{"operation": "send", "encrypted_data": "<base64>"}`.

The flat form `{"operation": "send", "to": …, "subject": …, "body": …}` also
works and is fine over HTTPS. On the on-chain path those fields are public
chain state forever, and the answer says so in `warning`.

Attachments: `[{"filename": "r.pdf", "content_type": "application/pdf", "data": "<base64>"}]`
and the operation is **`send_with_attachment`**, which is priced for them. A
plain `send` carrying attachments is refused even when they arrived sealed —
the price is checked after the payload is opened. You need not compute `size`.

A recipient at `@near.email` is delivered inside the service, sealed to them;
anything else goes out by mail relay. The answer's `delivery` says which, and
`message_id` identifies the copy kept in your sent folder.

## Deleting

`{"operation": "delete", "email_id": "…"}` — cheap, and only from your own
mailbox. `deleted: false` means there was nothing by that id.

## Costs and quotas

| operation | price |
|---|---|
| `status`, `send_pubkey`, `list`, `read_attachment` | free (compute only, ~$0.001) |
| `send` | $0.01 |
| `send_with_attachment` | $0.015 |
| `delete` | $0.001 |

Mail that leaves near.email is capped at **3 a day** for `send` and for
`send_with_attachment`, and that cap binds calls paid from an allowance (trial
or gift). Mail whose every recipient is an `@near.email` address is not capped:
it never reaches another provider.

The classification happens outside the enclave, from the request body, and it
fails closed. A **sealed** `send` hides its recipient, so it always counts as
leaving — even to a `@near.email` address. Send internal mail in the flat form
if you want it counted as internal; it stays inside our own system either way.

A run that started is charged even if the service then refuses it.

## When it refuses

* `this call carries no account` — the call arrived without a NEAR identity.
  Use a payment key the wallet owns, or call on chain.
* `only named accounts` / `can_send: false` — an implicit account has no address.
* `encrypted_data could not be decrypted for this account` — you sealed to the
  wrong key. Re-read `send_pubkey`; it is per account.
* `ephemeral_pubkey … must be a 33-byte compressed secp256k1 key` — you sent an
  uncompressed or truncated key.
* `use send_with_attachment` — pay for the operation that carries files.
* `the mailbox service did not answer` — transient; retry once, then stop.
