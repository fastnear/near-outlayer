---
name: gmail-connector
description: Send email from the owner's own Gmail address through the `gmail` connector — to the recipients the owner's policy allows, with attachments if it allows them. Use when an agent with an OutLayer wallet needs to write to someone by email as its owner. It cannot read the mailbox.
---

# Gmail connector

You send mail as the owner's Gmail account. You never see the credential, you can
only write where the owner's policy allows, and you cannot read the mailbox —
there is no operation for it, and the credential could not do it anyway.

## Call shape

```
POST https://api.outlayer.ai/call/connectors.outlayer.near/gmail
X-Payment-Key: <a payment key the custody wallet owns>
X-Wallet-Id: <the wallet id>
X-Use-Owner-Secret: 1
Content-Type: application/json

{"input": {"operation": "<op>", ...}}
```

`X-Use-Owner-Secret: 1` brings the Gmail credential and policy stored under
your own wallet into the run. If the owner stored them under THEIR account and
whitelisted your wallet, name that row instead:
`{"input": {...}, "secrets_ref": {"account_id": "owner.near", "profile": "gmail"}}`.
With neither, nothing works and the answer says so. Connecting an
account is nothing more than the owner storing three secrets for this connector;
if `status` says the credential is missing, that is what has not happened yet.

## Start here

`{"operation": "status"}` — free. Says whether the credential works, what the
policy allows, and how many messages you have already sent today. If
`policy.present` is false, nothing can be sent; ask the owner.

## Sending

```json
{"operation": "send", "to": "boss@example.com", "subject": "Weekly numbers",
 "body": "Revenue is up 4%.\n\n— the agent"}
```

* `to` and `cc` take one address as a string, or an array for several:
  `"to": ["a@example.com", "b@example.com"]`. Bare addresses only — no display
  name, no angle brackets. A comma inside a string is refused, not split.
* `attachments`: `[{"filename": "r.csv", "content_type": "text/csv", "data": "<base64>"}]`.
  Allowed only if the owner set `max_attachment_kb`.
* The sender is always the owner's own address. There is no field for changing it.

The answer carries the new `message_id`, `thread_id`, and how many of today's
messages are left. The subject may come back with a prefix the owner's policy
adds.

## What the policy means

`status` reports it. Sending needs a policy; `max_per_day` in it is the owner's
optional cap, counted per calling wallet.
`recipient_domains` and `recipients` decide who may be written to, and **one
disallowed address refuses the whole message** — a message goes to all of its
recipients or none. `max_recipients` counts `to` and `cc` together.
`max_attachment_kb` is the total of a message's files, and without it attachments
are refused. Size the request inside those limits rather than discovering them by
refusal: a refused call still ran and still cost its fee.

## Costs

| operation | price |
|---|---|
| `status` | free |
| `send` | $0.01 |

Plus compute, about $0.001 a call. A run that started is charged even when Gmail
then refuses it. `send` is also capped at 400 a day, below Gmail's own daily
limit.

## When it refuses

* `this wallet has no Gmail credential` — the owner has not stored it, or the
  call lacks `X-Use-Owner-Secret: 1`.
* `credential_expired` — Google rejected the refresh token for good. The owner
  must mint a new one; retrying is pointless.
* `scope_missing` — the credential was not granted `gmail.send`. The owner must
  consent again.
* `rate_limited` — Gmail is throttling. Wait, then retry; nothing was sent.
* `policy_denied: …` — the owner's caps. Change the request, or ask the owner.
* `is not a bare email address` — an entry in `to` or `cc` has a display name,
  angle brackets, a comma, a second `@`, or characters an address cannot have.
