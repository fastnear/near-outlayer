# Gmail connector — setting up the Google app from scratch

A runbook: everything needed to create OutLayer's Google OAuth app for the Gmail
connector, connect a first account, and publish the connector. It assumes
nothing exists yet — the test project `outlayer-gmail-test` used on 2026-09-11 is
a sandbox and is not reused.

What you end up with: one Google Cloud project owned by an OutLayer identity, an
OAuth client with the single scope `gmail.send`, published (so tokens do not
expire weekly) but unverified (a warning screen, at most 100 users), and the
connector live at `connectors.outlayer.testnet/gmail`, then `…near/gmail`.

The plan this implements, and the reasons behind each choice, are in
[gmail-onboarding-plan.md](gmail-onboarding-plan.md).

---

## 1. Who owns the app — and why not your personal account

**Yes, use a separate identity. Never your personal Google account.** What can
and cannot go wrong:

- **A user's spam lands on the user's account, not ours.** Every message leaves
  from the connected user's own Gmail address, so Gmail's sending limits and spam
  actions apply to that sender. Our mailbox sends nothing on anyone's behalf.
- **What can hit us is the app.** Google can suspend an OAuth app that breaks its
  policies, and then every connected user stops working at once.
- **A suspended app cannot be swapped quietly.** A refresh token only works with
  the client that issued it, so a new app means every user connects again.
- **Whether an app suspension spreads to the owner's other Google services is not
  stated anywhere in Google's documentation.** That silence is the reason to keep
  the app away from an account whose mail, Drive and logins you depend on.

Two ways to do it:

| option | what it is | trade-off |
|---|---|---|
| **A. A new Google account** | a Gmail address used only for OutLayer's OAuth apps | ten minutes; still one account, so one lost password or lock locks the app |
| **B. Cloud Identity Free on `outlayer.ai`** (recommended) | a free Google organisation for our domain, up to 50 users; the project belongs to the organisation, not a person | one DNS record more; two admins mean no single point of failure |

Whichever you choose: two-factor authentication on, recovery email and phone
that belong to the company, and the account used for nothing else.

What actually keeps the app alive is not the account but what users can do with
it. The connector refuses to send without an owner's policy, enforces a recipient
allowlist and a daily count, caps every wallet at 400 messages a day in its
manifest, and can stamp a subject prefix; the Terms forbid spam. Google judges an
app by what its users can do with it, and that is the part we control.

---

## 2. Before you open Google Cloud

Both must be true, or the consent screen will point at pages that are not there:

1. **The privacy policy and terms are live on the dashboard.** The landing at
   `outlayer.ai` answers 200 to any path with its home page, so a 200 proves
   nothing — check for the text:

   ```bash
   curl -s https://app.outlayer.ai/privacy | grep -c "Limited Use"   # must print 1 or more
   curl -s https://app.outlayer.ai/terms   | grep -c "Terms of Service"
   ```

2. **You know which identity (A or B) you are creating the project from**, and you
   are signed in to Google Cloud as it — in a separate browser profile, so your
   personal account is never the one that clicks *Create*.

---

## 3. Owner identity

**Option A.** Create the account at <https://accounts.google.com/signup>, turn on
2-Step Verification, set company recovery details. Done.

**Option B.**

1. Sign up for Cloud Identity Free with the domain `outlayer.ai`.
2. It asks you to prove the domain with a **TXT record** — add it in Cloudflare
   (zone `outlayer.ai` → DNS → Add record → Type `TXT`, Name `@`, Content as
   given, TTL Auto). It can take up to 72 hours to be seen, usually far less.
3. Create two users, for example `ops@outlayer.ai` and your own
   `vadim@outlayer.ai`, and make both super admins.
4. The Google Cloud organisation is created the first time one of them creates a
   project (§5).

---

## 4. Verify the domain in Search Console

Required for the consent screen's authorised domain.

1. <https://search.google.com/search-console> → **Add property** → **Domain** →
   `outlayer.ai`. Do this as the identity from §3.
2. Copy the `google-site-verification=…` value.
3. Cloudflare → zone `outlayer.ai` → DNS → **Add record**: Type `TXT`, Name `@`,
   Content the value, TTL Auto → Save.
4. Back in Search Console → **Verify**.

(With option B the domain may already count as verified from §3; Search Console
says so when you add it.)

---

## 5. The Google Cloud project

1. <https://console.cloud.google.com/projectcreate> — name `outlayer-gmail`. With
   option B, choose the `outlayer.ai` organisation as its location.
2. <https://console.cloud.google.com/apis/library/gmail.googleapis.com> — with the
   new project selected, **Enable**.

---

## 6. Google Auth Platform (the consent screen)

<https://console.cloud.google.com/auth/overview> with the project selected.

**Branding.** Set these right the first time: changing the name, logo, home page,
privacy link or redirect URIs later needs brand re-verification before users see
the change.

| field | value |
|---|---|
| App name | `OutLayer` |
| User support email | the §3 identity, or `security@outlayer.ai` if it is a Google group you can pick |
| App logo | leave empty |
| Application home page | `https://app.outlayer.ai` |
| Privacy policy | `https://app.outlayer.ai/privacy` |
| Terms of service | `https://app.outlayer.ai/terms` |
| Authorised domains | `outlayer.ai` |
| Developer contact | the §3 identity — Google sends policy and verification mail here, so it must be read |

**Audience.** User type **External**. Then **Publish app** — status *In
production*. Do not leave it in *Testing*: refresh tokens issued in Testing
expire after 7 days. Test users are not needed once published.

**Data access.** Add exactly one scope:
`https://www.googleapis.com/auth/gmail.send`. Nothing else — every extra scope is
one Google will ask us to justify, and the connector has no use for any other.

---

## 7. The OAuth client

<https://console.cloud.google.com/apis/credentials> → **Create credentials** →
**OAuth client ID**.

| field | value |
|---|---|
| Application type | **Web application** |
| Name | `outlayer-gmail-connector` |
| Authorised redirect URIs | `https://developers.google.com/oauthplayground` |

Add `https://app.outlayer.ai/connect/gmail` here on the day the dashboard's
*Connect Gmail* page ships — not before, and remember it counts as a branding
change.

Copy the **Client ID** and **Client secret** into the company password manager.
They never go into a repository, a chat, or an `.env` inside a repository.

---

## 8. Connect a first account

For any Gmail account you want an agent to send from:

1. <https://developers.google.com/oauthplayground/> → gear → **Use your own OAuth
   credentials** → paste the client id and secret.
2. In **Input your own scopes**: `https://www.googleapis.com/auth/gmail.send` →
   **Authorize APIs**.
3. Sign in as that account. Google shows "hasn't verified this app": **Advanced**
   → **Go to OutLayer (unsafe)** → **Allow**.
4. **Exchange authorization code for tokens** → copy the **Refresh token**
   (starts with `1//`). The code shown before it is single-use; do not store it.

Each connected account counts once toward the 100-user cap of an unverified app.

---

## 9. Publish the connector

Testnet first. All from `connectors/gmail-connector`.

```bash
./build.sh                                        # prints the SHA-256

export OUTLAYER_NETWORK=testnet                   # bare `outlayer` follows the default network
outlayer login                                    # as connectors.outlayer.testnet
outlayer upload target/wasm32-wasip2/release/gmail-connector.wasm   # → a FastFS URL
outlayer deploy gmail <the FastFS URL>            # → connectors.outlayer.testnet/gmail

CONTRACT=outlayer.testnet OWNER=owner.outlayer.testnet ./set-prices.sh testnet
```

`set-prices.sh` also refreshes the coordinator's price cache. The coordinator must
be running a build whose connector registry contains `Gmail` — otherwise the
project is called as an ordinary one and nothing is priced.

Then give an agent a credential and a policy, as the **wallet owner**:

```bash
outlayer secrets set-for-agent \
  '{"GMAIL_CLIENT_ID":"…","GMAIL_CLIENT_SECRET":"…","GMAIL_REFRESH_TOKEN":"1//0…",
    "GMAIL_POLICY":"{\"recipient_domains\":[\"example.com\"],\"max_per_day\":20,\"subject_prefix\":\"[agent]\"}"}' \
  --project connectors.outlayer.testnet/gmail --api-key wk_…
```

and call it:

```bash
curl -s https://testnet-api.outlayer.ai/call/connectors.outlayer.testnet/gmail \
  -H "X-Payment-Key: <a key the wallet owns>" -H "X-Wallet-Id: <wallet id>" \
  -H "X-Use-Owner-Secret: 1" -H "Content-Type: application/json" \
  -d '{"input":{"operation":"status"}}'
```

`status` fetching a token is the proof the credential works. Then `send` to an
address the policy allows.

Mainnet is the same with `OUTLAYER_NETWORK=mainnet`, `connectors.outlayer.near`,
`CONTRACT=outlayer.near`, and `./set-prices.sh mainnet`.

---

## 10. Retire the test setup

1. <https://myaccount.google.com/permissions> as the test mailbox → remove
   `outlayer-gmail-test`. Its token can read that mailbox, which nothing needs any
   more.
2. Delete `connectors/gmail-connector/.env.gmail`.
3. Delete the `outlayer-gmail-test` project, or keep it as a sandbox that is never
   given to users.

---

## 11. Later: removing the warning and the cap

When 100 users stops being enough, submit the app for **sensitive-scope
verification** from the Google Auth Platform: 3–5 business days, no fee, no
security assessment. It needs one thing we do not have yet — an unlisted YouTube
video showing a user granting the scope on the consent screen and a message being
sent through the connector — and a one-sentence justification: the scope sends
the email the user's own agent composes, and no narrower scope can send.

---

## 12. If Google suspends the app

Every connected user's sends fail with `credential_rejected` or
`credential_expired`. Read the mail Google sent to the developer contact first;
the console offers an appeal. If the app cannot be restored: a new project and
client under the same identity (§5–§7), and every user connects again (§8),
because their refresh tokens belong to the old client. That cost is the reason
§1 exists.

## 13. What to watch

- Mail to the developer contact address.
- How many accounts have connected, against the 100-user cap.
- `credential_expired` (a token revoked or unused for six months) and
  `scope_missing` (an account connected without `gmail.send`) in connector answers.
