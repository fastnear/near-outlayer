# Coordinator admin endpoints

Everything under `/admin/*` on the coordinator, what it does, and what it costs
if the credential leaks. Operational recipes with real tokens and hosts live in
`.idea/TESTING-WITH-ADMIN.md`; this file is the map and the security model.

## Authentication

One bearer token, `ADMIN_BEARER_TOKEN`, checked by `middleware::admin_auth` in
front of every route below. There is no second factor, no per-route scope and no
audit of who called what — the token IS the authorization.

```
Authorization: Bearer $ADMIN_BEARER_TOKEN
```

Three properties the code enforces:

* **The coordinator refuses to start** when the token is unset or shorter than
  24 characters. An unset variable falls back to a placeholder that ships in
  this repository — not a weak password but a published one — and nothing else
  about such a deploy looks wrong, so nothing else would say so.
  (`config::check_admin_token`)
* **The comparison is constant-time.** `==` on strings stops at the first
  differing byte, which turns one credential into a per-byte guessing game for
  anyone who can measure response times. (`middleware::admin_auth`)
* **A wrong token is `403`, a missing one `401`**, and neither says which.

What is NOT enforced, and matters when deciding who holds the token:

* No rate limit on `/admin/*`. The IP limiter sits on the HTTPS API routes.
* No per-route scoping. Anyone who can read `/admin/earnings` can also call
  `DELETE /admin/workers/{id}`.
* Nothing records which operator acted. `tracing` logs the action, not a person.

**Never put `/admin/*` behind the same hostname policy as the public API without
checking.** The routes are mounted on the same server; the only thing separating
them from the world is this header.

## What a leaked token can do

Ordered by what it costs, not by how it reads.

| Reach | Routes |
|---|---|
| **Spends our money** | `POST /admin/grant-payment-key` — funds an existing key from our balance. It cannot CREATE a key and a grant cannot be withdrawn or forwarded to a developer (`is_grant`), so the loss is bounded by compute the attacker can burn. |
| **Breaks operations** | `DELETE /admin/workers/{worker_id}`, `DELETE /admin/grant-keys/{owner}/{nonce}` — remove records other things rely on. |
| **Widens what the coordinator concludes** | `POST /admin/binding-zones`, `POST /admin/hos-impl-code-hashes` — see below; both are lists whose growth relaxes a check. |
| **Reads customer data** | `GET /admin/earnings`, `/admin/connector-calls`, `/admin/egress-audit`, `/admin/compile-logs/{job_id}`, `/admin/health/detailed`. Egress audit is every outbound attempt every guest made. |
| **Harmless to repeat** | `POST /admin/collateral/check`, `GET /admin/collateral/status`, `POST /admin/keystore-stats/refresh`, `POST /admin/connector-prices/refresh` — refreshes and reads, idempotent by construction. |

## The two allowlists

Both are live tables rather than environment variables, because both change when
a partner ships something and a coordinator restart is a worse thing to need
than a row. They move in **opposite safety directions**, which is the only thing
worth memorising about them.

### `/admin/binding-zones` — where a revoke webhook may point

Account-name suffixes the partner's binding webhook is allowed to name. The
effective list is the union of this table and the deploy-time
`BINDING_WEBHOOK_SUFFIXES`.

**Empty means no restriction.** Adding the first zone NARROWS what the endpoint
accepts — the safe direction to move in by accident.

```
GET    /admin/binding-zones
POST   /admin/binding-zones          {"suffix": "...", "note": "..."}
DELETE /admin/binding-zones/{suffix}
```

A suffix set in the environment cannot be removed here; if it is still listed
after a DELETE, it came from the deploy.

### `/admin/hos-impl-code-hashes` — implementations we recognize

Answers exactly one question, for `hos_lease` bindings: when the leased
account's `hos_agent_status` will not answer, is that the chain telling us
something about the account, or a condition we must not act on?

It authorizes nothing. The spend grant on the leased account does that, and the
contract enforces it whatever this table says.

**Empty means we conclude nothing** — the same behaviour as before the list
existed. Adding a hash is what makes a refusal from an account running something
else readable as evidence, so a row WIDENS what the coordinator is willing to
conclude. That is the less-safe direction, and it is why this is an admin action
rather than a config default.

An unrecognized implementation produces `CodeHashUnknown`, which **suspends**
the binding and never revokes it: the fault is reversible, so recognizing the
code later brings every affected lane back with nothing rebuilt. That is
deliberate. The refusal that triggers it also covers a contract that panicked
once, a partner mid-migration, and a response this build cannot parse — a
terminal fault there would stamp `revoked_at` on every live leased binding with
no way back.

```
GET    /admin/hos-impl-code-hashes
POST   /admin/hos-impl-code-hashes   {"from_account": "...", "note": "..."}
                                     {"code_hash": "...",    "note": "..."}
DELETE /admin/hos-impl-code-hashes/{code_hash}
```

**Do not read the hash off the leased account.** These accounts reference a
global contract BY ACCOUNT ID, which leaves their own `code_hash` at the
all-zeros sentinel — the code that identifies them is on the implementation
account, one view further on. Send `from_account` and the coordinator makes that
hop itself (`near_client::fetch_impl_code_hash`); the sentinel is rejected
explicitly, because listing it would make every codeless account "recognized".

And because the indirection is **mutable** — whoever owns the implementation
account can redeploy it with no event on the leased accounts at all — a hash
here is a fact about a moment, never a guarantee about a period. An upgrade on
their side stops matching, the affected bindings go `suspended`, and adding the
new hash restores them.

`DELETE` narrows: with nothing recognized the coordinator stops calling anything
foreign. Safe to do in a hurry.

#### When the list is consulted

By default, **only when the status view refuses**. That leaves one thing open:
the leased account serves `hos_agent_status` itself, so an account repointed at
a contract which answers plausibly is believed on its own say-so — the grant it
reports, the membership, the lease.

`HOS_REQUIRE_RECOGNIZED_IMPL=true` closes it: the answer then counts only if the
code that answered is code on this list. A deploy-time switch and not a row,
because it is a posture decision made once, while the list changes whenever a
partner ships — and turning enforcement on with an empty list from a single POST
would suspend every leased binding.

`true`, `yes`, `on` and `1` all turn it on, and their opposites turn it off;
anything else keeps the default and says so in the startup log, naming the
variable and quoting what was set. A switch that read `yes` as "off" would hand
a deploy the weaker behaviour with nothing anywhere to notice.

It cannot fail closed by accident. A mismatch has to be POSITIVE: an empty list,
an unreadable list, an account that states no code and an unreachable node all
mean "no basis to call anything foreign" and pass straight through. An RPC blip
therefore cannot suspend anything.

Cost, and where it lands. The list is consulted on the REFUSING path as soon as
the table is non-empty — that is what the table is for — and on the ANSWERING
path only under the switch. Either way the database is asked first and an empty
table returns before any RPC, so an unused list costs one local query.

With a list, the extra work depends on the account: one that deploys its code
inline states its hash in the view already fetched and costs nothing more; one
that names a global contract by account id — which is what these leased accounts
do — costs one further `view_account`. On the refusing path that lands exactly
when the chain is already slow, which is the trade the table buys. The
five-second observation cache absorbs bursts on the signing path; nothing caches
it on the status path.

Setting the flag with an empty list is inert, and startup says so once in the
log rather than letting a deploy believe it got enforcement it did not get.

## Adding an admin route

Two routers carry `admin_auth`, and new routes belong on one of them — the
wallet-state one exists only because those handlers need `WalletState`, and it
applies the same layer.

Before adding one, answer: does it spend, delete, relax a check, or expose
customer data? If yes, say so in the table above. A route whose reach is not
written down is a route nobody weighs when deciding who gets the token.
