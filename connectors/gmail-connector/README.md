# gmail-connector

An agent sends mail from the owner's own Gmail address, under a policy the owner
stored. The credential never leaves the enclave and never appears in an answer.

It **only sends**. The credential carries the single scope `gmail.send`, which
authorises sending and nothing else — not reading the mailbox, not even reading
back which address it belongs to. So this connector cannot see a single message
the owner has, and an agent gets nothing out of the mailbox, by construction. A
message goes without a `From` header and Gmail fills in the connected account
(verified against a live mailbox).

## Why the REST API and not SMTP

The executor gives a guest no sockets at all: TCP, UDP and name lookup are off.
So SMTP cannot be spoken from inside the enclave whatever credential one holds,
and an app password is of no use here. The manifest allows exactly two hosts:
`oauth2.googleapis.com` for the token and `gmail.googleapis.com` for mail.

## Operations

| `operation` | class | what it does |
|---|---|---|
| `status` | read | whether the credential works (it fetches an access token), the policy's caps, today's send count |
| `send` | write | `to`, `cc`, `subject`, `body`, `attachments` — policy-checked, then sent |

`to` and `cc` take one address as a string or several as an array, and every
entry must be a **bare** address — `name@example.com`, no display name, no angle
brackets, ASCII only. A display name is the one place a second recipient can hide
from a policy check (`victim@evil.com, <ok@good.com>` is two addresses to Gmail
and one to a check that looks inside the brackets), so the only form accepted is
the one whose checked value is the value sent. A string is never split on
commas: a comma makes it a malformed address, not a list.

## The credential

Three values, stored once by the wallet owner, all of them the caller's own —
this connector holds no credential of ours, so it has no `author_secrets`:

```bash
outlayer secrets set-for-agent \
  '{"GMAIL_CLIENT_ID":"…apps.googleusercontent.com",
    "GMAIL_CLIENT_SECRET":"…",
    "GMAIL_REFRESH_TOKEN":"1//0…"}' \
  --project connectors.outlayer.near/gmail --api-key wk_…
```

The call must carry `X-Use-Owner-Secret: 1` to bring them into the run. The
refresh token stays in the enclave; what leaves is one request to Google for an
access token, which is cached in project storage until shortly before it expires.

One mailbox for several agents: store the same three values once under YOUR
account instead, and whitelist the agents' wallet accounts — a date after an
account makes that grant lapse on its own:

```bash
outlayer secrets set '{"GMAIL_CLIENT_ID":"…","GMAIL_CLIENT_SECRET":"…","GMAIL_REFRESH_TOKEN":"…"}' \
  --project connectors.outlayer.near/gmail --profile gmail \
  --access whitelist:you.near,<agent-wallet-account>@2026-12-31
```

Each agent then names the row in its call —
`"secrets_ref": {"account_id": "you.near", "profile": "gmail"}` — and
`outlayer secrets access … --access whitelist:you.near` takes it back. The
ciphertext never moves.

### Connecting an account

The connector needs nothing but those three values: storing them is the whole of
"connecting". Getting them is a Google consent flow for the scope
`https://www.googleapis.com/auth/gmail.send`, and somebody has to receive
Google's redirect:

1. **Google Cloud console** → create or pick a project.
2. **APIs & Services → Library** → enable **Gmail API**.
3. **OAuth consent screen** → External; add the account as a *Test user*, then set
   **Publishing status: In production**. A client left in *Testing* issues refresh
   tokens that expire in 7 days. An unverified production app works for up to
   100 users, each clicking through Google's "hasn't verified this app" screen.
4. **Credentials → Create credentials → OAuth client ID.**
   - *Web application*, with `https://developers.google.com/oauthplayground` as an
     authorised redirect URI, to use the [OAuth Playground](https://developers.google.com/oauthplayground/):
     gear → *Use your own OAuth credentials* → the scope above → *Authorize APIs* →
     *Exchange authorization code for tokens* → copy the refresh token.
   - or *Desktop app*, to use the helper here instead:
     ```bash
     python3 get-refresh-token.py --client-id …apps.googleusercontent.com --client-secret …
     ```
5. Store the three values as the agent's secret — in the dashboard's secrets
   form, which encrypts them in the browser to the keystore's key, or with the
   command above.

When Google later refuses a refresh, the answer says `credential_expired` and
that retrying will not help, because it will not: mint a new token the same way.
The path to a one-click flow under our own client is in
`docs/design/gmail-onboarding-plan.md`.

## Policy (`GMAIL_POLICY`, an agent secret)

```json
{
  "recipient_domains": ["example.com"],
  "recipients": ["boss@other.org"],
  "max_per_day": 20,
  "max_recipients": 5,
  "max_attachment_kb": 2048,
  "subject_prefix": "[agent]"
}
```

This is the point of the connector rather than handing an agent a token. An agent
that can send from a real person's address can phish in their name, so the owner
says who may be written to and how much, and the connector enforces it inside the
enclave before anything reaches Google.

Fail-closed: no policy means nothing is sent. `max_per_day` is the owner's own
cap on a runaway agent, counted per calling wallet, and it is optional: the
platform's cap is the manifest's `send` limit, which the coordinator enforces
for every wallet because every user sends through the same published OAuth
client, and Google caps the account itself. Silence about attachments is not
permission — without
`max_attachment_kb` the agent may not send files. `recipient_domains: ["any"]`,
or naming neither list, means anywhere, which is a choice the owner has to write
down. An unknown field is a parse error. `subject_prefix` is added once when it
is missing, so a recipient can tell agent mail from its owner's.

The day's count lives in project storage, per agent, in UTC days. A message's
place is **reserved atomically before it is sent** and given back if the send
does not happen — the way the platform reserves money before a call runs — so two
calls at once cannot both take the last place, and a message Google refused costs
nothing from the budget. The manifest also caps `send` at 400 a day for everyone, so the connector
refuses before Gmail's own daily limit does.

## Build

```bash
./build.sh
```

Checks that the artefact carries the `outlayer.manifest` section, that the
manifest's operations are exactly the ones the code dispatches on, that the limit
words are ones the platform knows, and that no `author_secrets` crept in. Prints
the SHA-256 to publish.

## Prices

```bash
CONTRACT=outlayer.testnet OWNER=owner.outlayer.testnet ./set-prices.sh testnet
```

Sets the on-chain price row, then tells the coordinator to re-read it. The admin
token and the coordinator URL come from the main repo's `scripts/.env`.

## Layout

| file | what it is |
|---|---|
| `src/main.rs` | the two operations and the input shape |
| `src/oauth.rs` | refresh token to access token, cached; Google's refusals translated |
| `src/gmail.rs` | the send call, with Google's errors turned into what to do about them |
| `src/mime.rs` | building an RFC 2822 message |
| `src/policy.rs` | the owner's rules, address parsing, the day's count |
