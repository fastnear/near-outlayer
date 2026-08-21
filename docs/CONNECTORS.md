# Building a connector

A guide for developers writing connectors on OutLayer. It covers what a
connector is, what being one changes, how secrets reach your code, and every way
a call to you can be limited.

The manifest format itself has its own document —
[`wasi-examples/CONNECTOR_MANIFEST.md`](../wasi-examples/CONNECTOR_MANIFEST.md).
Read this first for the model, that one for the field-by-field reference.

---

## 1. What a connector is

**A connector is an ordinary project that we curated and priced.** There is no
separate connector runtime, no special deployment, no second API. You write a
WASI module, publish it like any project, and what makes it a connector is
decided from two facts:

* it is published under the curated namespace (`connectors.outlayer.near` on
  mainnet, `connectors.outlayer.testnet` on testnet), and
* its wasm carries a manifest declaring a `connector_id`.

Membership is a comparison against the owner account of a `project_id`, which
makes "is this a connector" a structural fact rather than something a project
claims about itself.

### Why the category exists at all

An ordinary project runs your code for you. A connector runs your code **for
somebody else's agent**, inside a TEE that holds custody keys, and lets it reach
the internet. Three things follow, and they are the whole difference:

| | ordinary project | connector |
|---|---|---|
| outbound network | open | **only the hosts your manifest declares** |
| operation naming | your business | a required top-level `operation` string |
| pricing | per call | **per operation, on chain** |

Each is a constraint on you. Together they are what lets a stranger's agent call
your code with money attached and a wallet in the room.

### What a connector is NOT

It is not a plugin, an extension, or anything that runs inside another program.
Your module is a normal WASI guest with a normal entry point. It cannot see
other calls, other agents' secrets, or the wallet's keys — the same isolation
every project gets.

It is also not a way to charge whatever you like. Prices live on chain and
nowhere else, and the contract refuses a call that does not attach the exact
price of the operation it names. A price baked into your wasm could only ever
disagree with the one that decides.

---

## 2. The shape of a call

Every connector call names its operation in one place:

```json
{ "operation": "send", "to": "someone@example.com", "subject": "hello" }
```

Over HTTPS that object is the `input` of `POST /call/{owner}/{project}`; on
chain it is `input_data` in `request_execution`. Same bytes either way, and four
readers take the operation out of them: the contract prices the call, the
coordinator bills it and picks which limit applies, the worker refuses a
connector call that names none, and your guest dispatches on it.

**Fail-closed, before your code runs.** Absent, blank, not a string, nested, or
spelled `op` — all refused. None of them defaults, because a defaulted operation
is a defaulted price and the cheapest one is what an attacker would pick.

An operation with no on-chain price is refused too. Unpriced is not free.

Two constraints come with the format: a priced project's request must be a JSON
object, and on chain it must be at most **10 KB** — the contract parses it, and
the caller's gas pays for that. A connector that moves more than that takes a
reference, not the bytes.

### Answering

Return JSON on stdout. The convention the playground and the connectors follow:

```json
{ "success": true, "output": { … }, "logs": [], "error": null }
```

The field is `error`, not `error_message`.

---

## 3. Network: you declare it, the worker enforces it

A connector reaches only the hosts listed in `capabilities.network` in its
manifest. Exact hostnames, case-insensitive, **no implicit subdomain wildcard**:
`example.com` does not permit `evil.example.com`.

The manifest lives in a wasm custom section, so it is covered by the SHA256 the
contract records for the version. Nobody — not you after publishing, not the
coordinator operator — can widen it without publishing a new version that users
have to move to.

A connector with no manifest section reaches **nothing**. That is the
fail-closed direction, and `build.sh` in `connector-probe` fails the build when
the section is missing so you find out at your desk.

Every outbound attempt is reported to the coordinator with whether the allowlist
permitted it. The coordinator stores that trail and decides nothing — the
allowlist is enforced inside the worker, where the keys are.

---

## 4. Secrets

This is the part most connectors get wrong, because there are two entirely
different secrets involved and they belong to different people.

### 4.1 Your credential (the connector author's)

Your SMTP password, your upstream API key — a credential that belongs to **you**
and is the same for every caller.

Store it with `store_secrets` under your own account and point calls at it with
`secrets_ref`:

```json
{
  "input": { "operation": "send", … },
  "secrets_ref": { "account_id": "you.near", "profile": "prod" }
}
```

Access to a stored secret is governed on chain by an `AccessCondition`, which is
richer than a list: `AllowAll`, `Whitelist`, `AccountPattern`, `NearBalance`,
`FtBalance`, `NftOwned`, `DaoMember`, and `Logic`/`Not` to combine them. That is
how you can hand a credential to a class of callers — everyone holding a
particular NFT, every member of a DAO role — without naming them.

Decryption happens in the keystore TEE and the plaintext exists only inside the
worker that runs your module. The coordinator never sees it.

### 4.2 The caller's credential (the agent's own)

An agent may have a secret of its own that YOUR connector needs to act on its
behalf — its account on your service, its own upstream token.

The agent asks for it with a header:

```
X-Use-Owner-Secret: 1
```

Nothing is looked up unless the call asks, because most connectors need no
secret and a lookup costs a keystore round trip plus a "not found" that means
nothing.

Where it is looked up is not negotiable and not in the request: both the profile
and the owner are the agent's **own account**, taken from the payment key's row.
The secret was stored BY that wallet, so it is owned by the same account it is
named after — which is what makes it unforgeable, since only the wallet's key
can make that wallet sign.

A header rather than a body field on purpose: on the connector path the body is
your input, and reserving a key inside it would collide with whatever you
already accept.

The header is meaningful only for a key owned by a custody wallet. An ordinary
payment key's holder addresses their own secrets through the body, as always.

### 4.3 How secrets reach your code

As environment variables. Read them with `std::env::var`.

Two rules protect you from a caller who tries to impersonate the platform:

**Reserved names are refused at storage time.** `store_secrets` rejects any key
the worker itself injects — `NEAR_SENDER_ID`, `NEAR_USER_ACCOUNT_ID`,
`OUTLAYER_PROJECT_OWNER`, `WALLET_ID` and the rest. You get an error naming the
offending keys.

**And the worker strips them anyway.** Before writing a single system value,
`merge_env_vars` removes every system name from the merged secrets. So whatever
arrives under a system name came from the worker, not from whoever supplied the
secrets — regardless of storage-time checks.

Absent stays distinct from empty: a variable the worker does not set for this
run is *missing*, not blank, so `env::var("OUTLAYER_PROJECT_OWNER").ok()` still
means "no project".

`PROTECTED_` is a reserved prefix for secrets the keystore generates. Manual
secrets cannot use it.

The full list of injected variables, and what each means, is in
[`wasi-examples/WASM_ENV_VARS.md`](../wasi-examples/WASM_ENV_VARS.md).

### 4.4 Who your caller is

`NEAR_SENDER_ID` is the identity the guest acts as. It is injected by the worker
and cannot be chosen by the caller — that is what the two rules above are for.
near.email turns it into the mailbox it sends from; treat it as the account you
are acting for.

`NEAR_USER_ACCOUNT_ID` is who **paid**. The two are the same unless the caller
is an Agent Connect wallet running under a bound account's name, which is opt-in
per call. Bill and attribute against the payer; act as the sender.

---

## 5. Limits

Four independent mechanisms can refuse a call to you. They are ANDed — every
applicable one must pass — and none of them can raise another.

### 5.1 Price (the contract)

Per operation, in the contract's pricing table, with the author's share and the
account it pays to alongside it. The chain enforces it: `request_execution`
refuses a call that does not attach the operation's exact price.

You do not set this in your manifest. A manifest may state a *recommended*
price; the on-chain one is what is charged.

### 5.2 Operation limits (the coordinator)

`(operation, window, max, who it applies to)`. One primitive for every "no more
than N per period" rule.

* **window** — `day`, `week`, `month`. Rolling from first use, **not
  calendar-aligned**: a calendar month resets for everybody at midnight on the
  1st, which turns a monthly cap into a stampede. "This month" means the 30 days
  since you started.
* **applies** — `everyone`, `unpaid` (no purchased subscription), `covered`
  (calls paid from an allowance — trial and gift included).
* **operation** — exact (`near-email:send`) or a whole-segment wildcard
  (`near-email:*`). No general globbing: `near-email:*` matches
  `near-email:send` and not `near-emailx:send`. These are the coordinator's own
  rules, written with the connector id; **in your manifest you write it
  without** — see §5.3.

When one is exceeded the caller is told the number, the period, and how many
seconds until the counter expires — because the window rolls from first use and
nobody can work that out from the outside.

### 5.3 Limits your connector declares about itself

Your manifest may carry `limits`. They are **unioned** with the coordinator's
rules, never compared: since every applicable rule must pass, declaring
something stricter gets you the stricter number and declaring something looser
changes nothing. There is no comparison to get wrong and no drift to detect.

**Write the operation WITHOUT your connector id.** The id is prefixed for you
when the declaration is stored:

```jsonc
// in your manifest
{ "operation": "send:external", "window": "day", "max_count": 3, "applies": "covered" }
// what it becomes, and what the counter is keyed by
"near-email:send:external"
```

Write the prefix yourself and you get `near-email:near-email:send:external`.
Rules are matched exactly, so it caps nothing — and nothing tells you: no error,
no log, no failing call. You would ship believing you had limited yourself. The
id is added rather than accepted so a manifest cannot declare limits about a
*different* connector.

The rest of the name is yours and may be finer than the operation. `send:external`
is not a second operation — it is what a `send` counts as when any recipient is
outside near.email, a distinction only the connector can make.

Use this for a cap that protects something only you know about — mail
deliverability is the canonical case. It travels inside the wasm and is covered
by the on-chain hash, so it holds even where the coordinator's own table does
not.

One sharp edge worth knowing: the numbers in force are the ones the last binary
that **ran** reported, keyed by project rather than by hash. A rollback restores
the old numbers when the old binary next runs, not the moment it is published.

A word the platform does not recognise is read at its **strictest**, not
dropped — an unknown `window` reads as `month`, an unknown `applies` as
`everyone`. Check your own manifest at build time; `connector-probe/build.sh`
shows how.

### 5.4 What the caller's key allows

Independent of anything you declare:

* **scope** — a payment key lists the projects it may call, as `owner/project`
  or `owner/*`. Empty means any project. The wildcard is a whole trailing
  segment only: `owner/pre*` does not match `owner/prefix`.
* **balance or subscription** — a call is paid from the key's money or from an
  allowance. A subscription runs **one call at a time** per key; a second
  concurrent one falls back to money if the key has any, and is refused with
  `call_already_in_flight` (retryable) if it does not. Money-paid calls have no
  concurrency limit and never occupy the subscription's slot.
* **compute** — resource limits per call, clamped to the tier's ceilings.

---

## 6. Testing before you ship

`wasi-examples/connector-probe` exists for exactly this. It is published,
priced, metered and manifested like a real connector, and every operation
reports one fact about the platform rather than doing work:

| operation | what it proves |
|---|---|
| `ping` | a free operation is still a real price: it runs only on a key that can pay |
| `whoami` | both identities the worker injected — who you act as, and who paid |
| `env` | every system variable, present or missing, so one going quiet is a failed probe |
| `secret` | the owner's secret arrived — presence, length and a hash prefix, never the value |
| `burn` | compute costs something |
| `fetch` | your declared host is reachable |
| `forbidden_fetch` | an undeclared host is not, and the refusal comes from the worker |

Copy its shape. In particular copy two habits: it never returns a secret's
value, and it never accepts a host to fetch from the caller. A connector that
fetched a caller-chosen host would be an SSRF gadget with a TEE's network
access; one that echoed secrets would make every test run a leak.

---

## 7. Checklist

1. Write the guest. Dispatch on a top-level `operation` string.
2. Embed a manifest with `connector_id` and every host you need in
   `capabilities.network`. No manifest means no network.
3. Fail the build if the custom section is missing.
4. Publish under the curated namespace.
5. Price every operation on chain, including the free ones — unpriced is
   refused, not free.
6. Decide whose secret you need: yours (`secrets_ref` + an `AccessCondition`) or
   the caller's (`X-Use-Owner-Secret`). Most connectors need neither.
7. Declare a `limits` entry for anything only you know is fragile.
8. Handle errors. Read `NEAR_SENDER_ID` for who you act as and
   `NEAR_USER_ACCOUNT_ID` for who paid; never take either from the input.

## See also

* [`wasi-examples/CONNECTOR_MANIFEST.md`](../wasi-examples/CONNECTOR_MANIFEST.md) — manifest reference
* [`wasi-examples/WASI_TUTORIAL.md`](../wasi-examples/WASI_TUTORIAL.md) — writing and building a WASI guest
* [`wasi-examples/WASM_ENV_VARS.md`](../wasi-examples/WASM_ENV_VARS.md) — every injected variable
* [`wasi-examples/connector-probe/`](../wasi-examples/connector-probe/) — a working connector to copy
