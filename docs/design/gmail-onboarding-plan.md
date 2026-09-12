# Gmail connector — one-click onboarding

**Status:** plan. DECIDED 2026-09-11: the connector **sends only** — reading is
out, and so are paid security assessments and demo videos. The send path was
verified against a live mailbox on 2026-09-11 with the connector's own MIME
bytes. What does not exist yet is an onboarding a non-developer can finish.

## 1. Where onboarding stands

The connector needs three agent secrets — `GMAIL_CLIENT_ID`,
`GMAIL_CLIENT_SECRET`, `GMAIL_REFRESH_TOKEN` — granted the single scope
`gmail.send`, and nothing else: storing them is the whole of "connecting". Today every user brings their own OAuth client, which
means creating a Google Cloud project, enabling the Gmail API, configuring a
consent screen and minting a token through the OAuth Playground or the helper
script. That is a developer's path, not a product.

The product path moves that work to us, once: **we** own one verified OAuth
client, and a user sees a *Connect Gmail* button and Google's consent screen.

## 2. The flow

```
dashboard: Connect Gmail
   │  redirect to accounts.google.com with OUR client id, scopes,
   │  access_type=offline, prompt=consent, state
   ▼
Google consent screen (user approves)
   │  redirect back to app.outlayer.ai/connect/gmail?code=…&state=…
   ▼
exchange the code for a refresh token          ← see 2.1 for WHERE
   │
   ▼
seal the token to the keystore's agent-secret key   (existing: eciesEncrypt)
   │
   ▼
POST /wallet/v1/agent-secret/prepare → the keystore signs the call
   │
   ▼
the owner's NEAR wallet sends store_agent_secret and pays the deposit
```

Everything from "seal" onward already exists in
`app/secrets/components/AgentSecretForm.tsx`: the dashboard fetches
`/wallet/v1/agent-secret/pubkey`, encrypts in the browser, asks the coordinator
to prepare the call, checks what came back, and has the owner's wallet send it.
The user signs exactly one NEAR transaction.

The owner-side route is the simpler alternative: store the sealed token under
the OWNER's own account with plain `store_secrets`, condition
`Whitelist[owner, agent…]`, and have each agent name `{owner, profile}` in
`secrets_ref`. No `prepare` step, no per-agent copy, one place to revoke or to
add the next agent, and `ValidUntil` sizes a grant to a lease. The agent-secret
flow stays for an agent that should hold its own credential.

### 2.1 Where the code is exchanged

The authorisation code is useless without the client secret, and Google requires
the client secret for a web client — so the exchange cannot happen in the
browser. Three places it can happen:

| where | who sees the refresh token in plaintext | cost |
|---|---|---|
| a route handler in the dashboard (it runs under `next start`) | the dashboard process, in memory | smallest |
| an endpoint in the coordinator | the coordinator process, in memory | small |
| **inside the enclave**, as a connector operation | nobody outside the TEE | a new operation and an author secret |

**Target: the enclave.** An operation (working name `connect`) receives the code,
exchanges it with OUR client secret held as the connector's author secret, and
returns the refresh token already sealed to the keystore's agent-secret key. The
dashboard never holds a usable token, only ciphertext it passes to `prepare`.
That is the custody story this platform sells, applied to its own onboarding.
The dashboard route is an acceptable stepping stone if speed matters, provided
the terms say our server handles the token momentarily and never stores it.

### 2.2 What the connector must change

1. **Two credential shapes at once.** Users who connected with their own client
   keep working; users who connected through ours carry only a refresh token. Our
   client id and secret go into the manifest's `author_secrets` under names the
   agent's secret never uses (e.g. `GMAIL_OAUTH_CLIENT_ID`), because a name
   defined on both sides refuses the run. The user's own client, when present,
   wins.
2. **The `connect` operation** from 2.1, free: it is the door, not the room.

The connector already sends with a `gmail.send`-only credential: it never calls
`users.getProfile` (which needs a read scope) and leaves the `From` header to
Gmail, which fills in the authenticated account — verified live on 2026-09-11.

## 3. Publishing the app

### 3.1 What `gmail.send` costs, and what it costs to skip the video

`gmail.send` is a **sensitive** scope. Its verification takes 3–5 business days,
needs no third-party security assessment and no payment — but it does need a
**demo video** showing the consent grant and the send.

Without verification the app still works, published (status *In production*,
not *Testing*):

- every user sees Google's "hasn't verified this app" screen and clicks through
  *Advanced*;
- the app can be connected by **100 new users in total**, ever, until verified;
- refresh tokens do NOT expire weekly — the 7-day expiry applies to *Testing*
  status only.

That is the chosen state for now. The video is the single step between it and an
app with no warning and no cap, and it can be recorded whenever the cap starts to
matter.

| scope | class | used |
|---|---|---|
| `gmail.send` | sensitive | yes — `send` |
| `gmail.readonly`, `gmail.modify`, `gmail.compose`, `mail.google.com` | restricted | no, and deliberately not requested |

### 3.2 Prerequisites, and what is missing today

| requirement | state |
|---|---|
| homepage on a domain we verified in Search Console | `app.outlayer.ai` exists; `outlayer.ai` is **not** verified yet (DNS TXT, Cloudflare — Vadim) |
| privacy policy on the same domain as the homepage, saying how Google user data is accessed, used, stored and shared | **does not exist.** `outlayer.ai/privacy` answers 200 only because the landing server serves `index.html` for every path; `app.outlayer.ai/privacy` is 404; no such page ever existed in the dashboard, the landing or the monorepo |
| terms of service | **does not exist**, same reason |
| demo video (unlisted YouTube) showing the consent grant and the send | **skipped for now** — see §3.1 for what that costs |
| written justification per scope | one scope, one sentence: sending the email the user's agent composes |
| app logo, support email | to prepare |

The legal pages go on the dashboard first (`app.outlayer.ai/privacy`,
`app.outlayer.ai/terms`), because the OAuth homepage will be the dashboard and
the policy must share its domain. The dashboard footer (`components/shell/AppShell.tsx`)
links to them, and the landing links to them too.

### 3.3 Reading mail is out, and so is its policy question

Reading would need a restricted scope (a paid annual CASA Tier 2 assessment), and
would send mailbox contents to whatever model drives the agent — a transfer
Google's Workspace user-data policy allows only for a user-facing feature with
consent, and never for training. Send-only raises neither: the agent writes mail;
nothing is read from Google except the credential itself.

## 4. Scopes: request exactly what is implemented

A wider scope "just in case" is the wrong trade:

- Google rejects verification for scopes that no visible feature uses: "We
  require that you only request scope access for implemented features."
- Adding a scope later is ordinary: the new scope needs verification before the
  code uses it, and incremental authorisation (`include_granted_scopes=true`)
  lets an existing user consent to the new scope alone.
- Changing the app's name, logo, homepage, privacy link or redirect URI needs
  brand re-verification, but the change is simply held back until approved; it
  does not trigger the unverified screen or the user cap.

### 4.1 What else is as cheap as sending

Only **non-sensitive** scopes avoid both the video and the cap. An app that uses
nothing else needs just *brand* verification — the verified domain and the
privacy policy, no video, minutes to 2–3 business days — and shows no unverified
screen at all. The useful one is **`drive.file`**: create files in the user's
Drive and edit the files the app created or the user opened with it. Through the
Sheets and Docs APIs that covers "the agent keeps a spreadsheet / writes a report
in my Drive". The broad `spreadsheets`, `documents` and `drive` scopes are
sensitive or restricted and must not be used for that.

Keep such a connector in its **own** OAuth app: one sensitive scope in an app
puts the whole app under the video requirement.

Calendar and Tasks scopes are not classified in Google's documentation; the
console shows each scope's class when it is added to a consent screen, which is
the place to check before designing anything on them.

## 5. Which Google account owns it

Not a new personal Gmail. A personal account cannot own an organisation, cannot
mark an app *Internal*, and if that one account is locked the project is locked
with it — Google's policy text is silent on whether an app suspension spreads to
the owner's other services, which is a reason to keep them apart, not a reason
to assume it does not.

Instead:

1. **Cloud Identity Free on `outlayer.ai`** — free, up to 50 users, proves the
   domain with a DNS TXT record. Creating a project from it creates a Google Cloud
   organisation that owns the project, rather than a person.
2. **At least two admin accounts on the domain** (role accounts such as
   `ops@outlayer.ai`, not personal ones), both project owners, so losing one
   locks nothing. Google mails project owners about verification and policy;
   those addresses must be read.
3. The test project `outlayer-gmail-test` stays a personal sandbox; the
   production client is created fresh under the organisation. Earlier projects
   can be moved into the organisation if ever useful.
4. The organisation also allows an *Internal* OAuth app for our own staff testing:
   no unverified screen, no user cap.

## 6. Order of work

| # | step | who |
|---|---|---|
| 1 | Search Console: verify the domain `outlayer.ai` with its DNS TXT record | Vadim |
| 2 | privacy policy and terms on `app.outlayer.ai` (written, not yet deployed), footer links, a link from the landing | Claude; legal read and deploy by Vadim |
| 3 | connector: the two credential shapes and the `connect` operation; tests | Claude |
| 4 | dashboard: *Connect Gmail* page — redirect, callback, `connect`, then the existing prepare-and-sign path | Claude |
| 5 | production OAuth client, preferably under an organisation (§5): consent screen with homepage, privacy and terms on `app.outlayer.ai`, authorised domain `outlayer.ai`, the single scope `gmail.send`; **publish** (In production) | Vadim |
| 6 | launch: unverified screen, 100-user cap, no weekly expiry | — |
| 7 | when the cap matters: record the demo video, submit sensitive-scope verification (3–5 business days, no fee) | later |

Steps 1–2 and 3–4 run in parallel; 5 needs 1–2; 6 needs 3–5.

## 7. Sources

- Gmail API scopes and their classes — https://developers.google.com/workspace/gmail/api/auth/scopes
- `users.getProfile` scopes (no `gmail.send`) — https://developers.google.com/workspace/gmail/api/reference/rest/v1/users/getProfile
- `users.messages.send` scopes — https://developers.google.com/workspace/gmail/api/reference/rest/v1/users.messages/send
- Sensitive-scope verification — https://developers.google.com/identity/protocols/oauth2/production-readiness/sensitive-scope-verification
- Restricted-scope verification and the annual assessment — https://developers.google.com/identity/protocols/oauth2/production-readiness/restricted-scope-verification
- Verification FAQ (Google charges no assessment fee) — https://support.google.com/cloud/answer/13463817
- Unverified apps and the 100-user cap — https://support.google.com/cloud/answer/7454865
- When verification is not needed — https://support.google.com/cloud/answer/13464323
- Changes to an approved app — https://support.google.com/cloud/answer/13464018
- Requesting minimum scopes — https://support.google.com/cloud/answer/13807380
- Refresh-token expiry (7 days in Testing) — https://developers.google.com/identity/protocols/oauth2
- Incremental authorisation, offline access, client secret on a server — https://developers.google.com/identity/protocols/oauth2/web-server
- API Services user-data policy (Limited Use) — https://developers.google.com/terms/api-services-user-data-policy
- Workspace API user-data policy (AI/ML) — https://developers.google.com/workspace/workspace-api-user-data-developer-policy
- OAuth 2.0 policies (owners, least privilege) — https://developers.google.com/identity/protocols/oauth2/policies
- Cloud Identity editions — https://docs.cloud.google.com/identity/docs/editions
- Organisation resource creation — https://docs.cloud.google.com/resource-manager/docs/creating-managing-organization
- CASA Tier 2 cost and timeline, for the reading path we did not take (third-party, moderate confidence) — https://www.unipile.com/integrating-google-oauth-2-0-user-authentication-into-your-app/
- Drive scopes (`drive.file` non-sensitive) — https://developers.google.com/workspace/drive/api/guides/api-specific-auth
- Sheets and Docs scopes (`drive.file` works with both) — https://developers.google.com/workspace/sheets/api/scopes , https://developers.google.com/workspace/docs/api/auth
- Brand verification (no video for non-sensitive scopes) — https://developers.google.com/identity/protocols/oauth2/production-readiness/brand-verification
