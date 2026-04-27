# Sequential NEAR Intents Spike

This document records the current OutLayer integration spike for using NEAR
yield/resume mechanics to execute ordered batches of NEAR calls through the
sequential gate pattern from `/Users/mikepurvis/near/sequential`.

The product idea is that an OutLayer wallet account can sign intent-like NEAR
actions inside the TEE, submit them to a gate contract, have the gate resume and
dispatch them in a strict chain, and then return receipt evidence to the WASI
agent. The agent can inspect those results off-chain and decide whether another
batch should be scheduled.

The current throughline is deliberately coordinator-centered. WASI asks for a
sequence and later polls status; the coordinator owns policy checks, Intents
payload signing, delegate construction, gate submission, resume, idempotency, and
receipt evidence. A parallel research spike in
`/Users/mikepurvis/near/fn/near-outlayer` explored a more guest-driven shape. We
borrow its useful test harness and receipt-parsing lessons, but not its raw
delegate-signing surface.

## What This Achieves

- Adds the OutLayer wallet host surface needed for a WASI agent to request a
  sequential batch and later poll its status.
- Adds keystore support for signing NEP-366 delegate actions using the existing
  deterministic OutLayer wallet NEAR key.
- Keeps generic contract-call sequencing and NEAR Intents-specific sequencing
  separate at the request boundary.
- Makes the predecessor model explicit: the current gate dispatches calls with
  `predecessor_id == gate`, while signed NEAR Intents payloads carry the wallet
  authorization inside the `execute_intents` arguments.
- Establishes the first safe live-mainnet shape: fund the OutLayer implicit
  wallet normally, deposit wNEAR into `intents.near` normally, then use the gate
  only for signed `execute_intents` calls.
- Adds a local proof-contract fixture for the future direct-user lane, where a
  TEE-held FullAccess key on `mike.near` can prove signer, predecessor, public
  key, and 1 yoctoNEAR evidence on a `sequential.near` subaccount.

This does not make ordinary contracts see `mike.near` or the OutLayer wallet as
the predecessor. Normal contracts that authenticate with
`env::predecessor_account_id()` still see the gate and are out of scope for this
first path.

## Two Execution Lanes

The fresh signal from `/Users/mikepurvis/near/fn/near-outlayer` is that there
are two valid lanes, and they solve different problems.

### Gate Lane: Proxy-Safe Sequential Intents

This is the current live-mainnet throughline for this repo:

1. The OutLayer implicit wallet signs NEP-413 payloads for NEAR Intents.
2. The coordinator wraps each `intents.near.execute_intents` call in a NEP-366
   delegate.
3. The coordinator submits delegates to `gate.sequential.near`.
4. The gate dispatches chained receipts in order.
5. The TEE receives ordered receipt evidence and can decide whether future work
   is warranted.

This lane is strongest when the target contract validates signed payloads in
arguments and does not require `predecessor_id == user`. It gives proxy
predecessor semantics, explicit `proxy_predecessor: true` evidence, and strict
cross-receiver/block-monotonic ordering from the sequential gate.

### Direct Lane: True-Predecessor User Transactions

The `fn` spike pivoted to a separate Shape 1 lane: the user installs a
TEE-controlled access key on their own account, and the TEE signs a native NEAR
transaction attributed to that user. For a same-receiver batch, one transaction
can contain multiple FunctionCall actions and the target sees
`predecessor_id == user`.

This lane is best for predecessor-sensitive work: staking/rewards, direct
user-account operations, and same-receiver workflows where atomic transaction
ordering is sufficient. It should be coordinator-owned and policy-first if added
here later. It should not be exposed as raw `call`, `multi-call`, or
`sign-nep366-delegate` host functions in production WIT.

The first implemented direct-user mainnet proof is deliberately smaller than the
FullAccess proof registry: derive a scoped FunctionCall key for `mike.near`,
using `mike.sequential.near` as the seed namespace, have the human add that key
to `mike.near` for `count.mike.near::increase`, then let the coordinator sign
and broadcast exactly one top-level counter transaction as `mike.near`.
`mike.sequential.near` does not sign on-chain in this proof; it is the
control-plane/namespace context that makes the derived key auditable.

The first live Intents proof stays on the gate lane: OutLayer implicit wallet,
wNEAR deposited in `intents.near`, three signed `execute_intents` calls through
`gate.sequential.near`, and no direct `mike.near` access-key dependency.

#### Direct-User Proof Registry

For the future direct-user lane, `sequential.near` can create a subaccount such
as `mike.sequential.near` and deploy a small proof/registry contract there. The
registry is a control-plane contract, not the signer. It records that the
OutLayer TEE key expected for `mike.near` has actually been used as the signer
key on a top-level `mike.near` transaction.

The proof flow is:

1. The coordinator or keystore derives a direct-user Ed25519 public key with a
   label such as `direct-user-fa:mike.near`.
2. The expected key is registered on `mike.sequential.near` by the
   `sequential.near` owner or by a setup call from `mike.near`.
3. The human adds that public key as a FullAccess key on `mike.near`.
4. The TEE signs a top-level transaction from `mike.near` to
   `mike.sequential.near::prove_full_access`, attaching exactly 1 yoctoNEAR.
5. The registry records proof only when:
   - `signer_account_id == mike.near`
   - `predecessor_account_id == mike.near`
   - `signer_account_pk == registered OutLayer public key`
   - `attached_deposit == 1`

The 1 yoctoNEAR deposit makes the proof more useful because a normal
FunctionCall access key cannot attach deposit. The signer public key evidence
binds the proof to the OutLayer-derived key rather than merely proving that some
key on `mike.near` made the call.

The critical invariant remains: an on-chain contract cannot sign a transaction
as `mike.near`. A later call from `mike.sequential.near` to another contract
would have `predecessor_id == mike.sequential.near`, not `mike.near`. To make a
staking pool, rewards contract, or wallet target see `predecessor_id ==
mike.near`, the TEE must sign and broadcast a top-level transaction from
`mike.near` directly to that target.

Before any direct-user operation, the coordinator should query RPC and confirm
that the expected key is still present on `mike.near`. Revocation is deletion of
the access key from `mike.near`, optionally followed by
`mike.sequential.near::revoke_expected_key` so the registry evidence matches the
live access-key table.

A future gate variant may consult the registry as delegate provenance, but that
still would not make gate-dispatched receipts appear as `mike.near`. It only
strengthens the evidence that the delegate came from a proven TEE-controlled key.

## Wallet Execution Planner

This suggests an explicit planner layer: the agent should describe the desired
wallet workflow, and the coordinator should classify each step before policy
checks, signing, or broadcast. The planner is not a raw signing API. It is a
decision record that says which lane is safe and what evidence the TEE should
expect back.

The first planner vocabulary is:

- `gate_proxy`
  - Use for `intents.near.execute_intents` calls with signed Intents payloads.
  - Evidence: `proxy_predecessor: true`, `predecessor_model: "gate"`,
    `ordering_model: "gate_chained"`.
  - Policy category: normal wallet call policy against `intents.near`, plus
    deeper Intents-payload policy before NEP-413 signing.

- `direct_user`
  - Use for future user-installed TEE access-key flows where the target must see
    `predecessor_id == user`.
  - Evidence: `predecessor_model: "user"`,
    `ordering_model: "single_tx_atomic"`.
  - Policy category: direct-user capability, with the coordinator binding the
    signer from the authenticated execution context.

- `funding_setup`
  - Use for wrapping, storage setup, FT deposits into Intents, balance reads, and
    other prerequisite wallet plumbing.
  - Evidence: `predecessor_model: "wallet"` for transactions or `"view"` for
    reads, `ordering_model: "normal_tx_or_view"`.
  - Policy category: setup/balance/funding policy, not sequential gate policy.

- `reject`
  - Use for unsafe or ambiguous mixes, especially normal contracts that require
    user predecessor but are requested through the gate.
  - Evidence: include a user-visible `reason` before any signing happens.

The planner must reject a gate batch that mixes proxy-safe Intents calls with
predecessor-sensitive staking/rewards calls. The gate lane and the direct lane
can both exist in one larger workflow, but not inside one gate-routed batch.

This repo has a local test-only planner harness under
`tests/sequential-intents/planner-harness`. It mirrors the coordinator
classification behavior without changing production WIT internals.

It also has a coordinator handoff harness under
`tests/sequential-intents/coordinator-handoff`. That harness became the behavior
spec for the new private coordinator checkout at
`/Users/mikepurvis/other/outlayer-coordinator`, branch
`codex/gate-intents-coordinator`. The coordinator branch now implements the
gate-first `sequential-batch` and wallet workflow endpoints with auth,
Postgres-backed idempotency, policy-before-signing, keystore signing, relayer
broadcast, RPC polling, gate log parsing, and ordered receipt evidence.

## Main Flows

### 1. Wallet and Intents Funding

Funding does not go through the sequential gate.

The wallet setup uses existing OutLayer wallet routes:

1. Resolve the OutLayer deterministic implicit NEAR account.
2. Fund it with a small amount of NEAR for gas.
3. Wrap NEAR by calling `wrap.near.near_deposit`.
4. Deposit wNEAR into `intents.near` with the existing
   `/wallet/v1/intents/deposit` route.
5. Verify the wallet's Intents balance through
   `/wallet/v1/balance?token=wrap.near&source=intents`.

This matters because Intents deposits are based on FT transfer semantics. If the
gate called `ft_transfer_call`, the token contract would see the gate as the
sender, not the OutLayer wallet.

### 2. WASI Agent Requests a Sequential Batch

The worker WIT now exposes:

```wit
sequence-calls: func(gate-id: string, calls-json: string, idempotency-key: string) -> tuple<string, string>;
get-sequence-status: func(request-id: string) -> tuple<string, string>;
plan-wallet-workflow: func(workflow-json: string) -> tuple<string, string>;
execute-wallet-workflow: func(workflow-json: string, idempotency-key: string) -> tuple<string, string>;
get-wallet-workflow-status: func(request-id: string) -> tuple<string, string>;
```

The worker validates `calls-json` before proxying it to the coordinator. Each
batch must contain 1 to 3 calls. Each call must include:

- `receiver_id`
- `method_name`
- `gas`
- `deposit`
- exactly one payload mode:
  - `args_base64`
  - `args_json`
  - `near_intents`

For `near_intents`, the worker enforces:

- `receiver_id == "intents.near"`
- `method_name == "execute_intents"`
- `deposit == "0"`

The worker forwards valid requests to:

```text
POST /wallet/v1/sequential-batch
GET  /wallet/v1/sequential-batch/{request_id}
```

This production WIT intentionally does not expose a generic NEAR `call` host
function or raw `sign-nep366-delegate` function to WASI. Those are useful research
primitives, but the live Intents path keeps signing and broadcast decisions behind
the coordinator policy boundary.

### 3. WASI Agent Requests a Wallet Workflow

The high-level wallet UX is now a planner surface. WASI submits a workflow JSON
object with ordered `steps`, asks for a plan, executes it with an idempotency key,
and polls workflow status. The worker performs lightweight schema validation and
rejects raw signing-shaped payloads before forwarding anything to the
coordinator.

Supported v1 step kinds are:

- `intents.transfer`
- `intents.swap`
- `intents.execute_raw`
- `funding.wrap_near`
- `funding.intents_deposit`
- `funding.balance_check`
- `funding.storage_deposit`
- `near.function_call`

For `near.function_call`, the worker requires
`predecessor_requirement == "user_required"`, a `user_id`, one `receiver_id`, and
either a single FunctionCall shape or an `actions[]` array. This keeps direct-user
execution explicit and same-receiver atomic. Cross-receiver direct-user work is
represented as multiple workflow steps, not one multi-action transaction.

The worker forwards valid workflow requests to:

```text
POST /wallet/v1/workflows/plan
POST /wallet/v1/workflows/execute
GET  /wallet/v1/workflows/{request_id}
```

The coordinator should return planner evidence for every step: lane,
predecessor model, ordering model, policy type, setup requirements, and
user-visible rejection reasons.

### 4. Coordinator Sequential And Workflow Execution

The private coordinator source now exists in the adjacent checkout:

```text
/Users/mikepurvis/other/outlayer-coordinator
```

It is pushed to:

```text
git@github.com:fastnear/outlayer-coordinator.git
branch: codex/gate-intents-coordinator
```

That branch is the first production slice for the gate lane. It does not restore
the entire historical coordinator; it focuses on the wallet workflow endpoints
needed for the mainnet dust run.

The coordinator behavior is:

1. Authenticate the wallet request using the existing internal wallet auth path.
2. Apply wallet policy before signing anything.
3. For generic calls, use the supplied `args_json` or `args_base64` directly.
4. For `near_intents`, build and sign a NEP-413 payload for
   `intents.near.execute_intents`, then wrap it as the actual call args.
5. Ask the keystore for one NEP-366 signed delegate per call.
6. Submit each signed delegate to `gate.submit_intent`.
7. Poll only the outer receipt of each submit transaction to collect
   `intent_submitted` logs and intent IDs.
8. Call `gate.resume_batch_chained(intent_ids)` with the configured approver and
   the required gate fee.
9. Poll receipt outcomes until dispatches resolve.
10. Return ordered receipt evidence to the WASI agent.

For wallet workflows, the coordinator plans first, policy-checks every step
before signing, then routes:

- `gate_proxy` steps through the existing NEP-413 inside NEP-366 sequential-gate
  path
- `funding_setup` steps to `requires_funding_setup` for this gate-first service;
  funding still happens through the existing wallet routes before gate execution
- `direct_user` steps return setup/unsupported evidence for the gate-first
  milestone, never through the gate

After the first gate dust run succeeds, direct-user execution can be enabled as
a separate coordinator lane. That lane must query RPC to confirm the derived key
is present on the user account, require registry proof before non-proof
execution, and return evidence that `predecessor_model == "user"`.

The coordinator parses gate logs across every receipt outcome, accepting
both top-level RPC outcome JSON and `result`-wrapped shapes. The minimum events to
capture are:

- `intent_submitted`
- `batch_started`
- `intent_dispatched`
- `chain_continued`

The returned evidence preserves the caller's original call order, not the
order in which submit transactions happened to complete. The response should
include submit transaction hashes, intent IDs, the batch/resume transaction hash,
dispatch receipts, dispatch block heights, final status, and
`proxy_predecessor: true`.

The important yield/resume detail is that submit transactions must not wait for
`FINAL` on the yielded callback. Waiting for the full yielded DAG can outlive the
pending intent and race the resume path. The sequential repo's outer-receipt
polling behavior is the model implemented in the coordinator branch.

For pre-mainnet testing, submit completion order should be scrambled while
`resume_batch_chained(intent_ids)` uses the intended call order. The target state
must still show the intended non-commutative sequence.

### 5. Keystore Signs NEP-366 Delegates And Direct-User Transactions

The keystore now exposes:

```text
POST /wallet/sign-nep366-delegate
POST /wallet/direct-user/prepare-key
POST /wallet/direct-user/prepare-function-call-key
POST /wallet/direct-user/sign-proof
POST /wallet/direct-user/sign-function-calls
POST /wallet/direct-user/sign-scoped-function-calls
```

The endpoint derives the existing deterministic wallet NEAR key and signs a
single-FunctionCall `DelegateAction` using NEP-461 hashing:

```text
sha256(borsh(NEP_366_DISCRIMINANT) || borsh(DelegateAction))
```

It returns:

- `signed_delegate_base64`
- `delegate_hash`
- `sender_id`
- `public_key`
- `nonce`
- `max_block_height`

The endpoint rejects:

- a `sender_id` that does not match the derived implicit wallet account
- zero gas
- zero `max_block_height`
- expired delegates when `current_block_height` is supplied
- malformed account IDs, deposits, or call arguments
- ambiguous call arguments that provide both `args_json` and `args_base64`

The internal NEP-366 helper intentionally supports only one FunctionCall action
per delegate, matching the current sequential gate wire format.

The direct-user endpoints are coordinator-only. They derive a separate key from:

```text
wallet:{wallet_id}:near:direct-user-fa:{user_id}:v1
```

The default proof registry label is:

```text
outlayer:{wallet_id}:direct-user-fa:v1
```

`prepare-key` returns only the public key and label. `sign-proof` signs a
top-level transaction from `user_id` to the proof registry method
`prove_full_access` with exactly 1 yoctoNEAR attached. `sign-function-calls`
signs one top-level same-receiver FunctionCall transaction as `user_id`.

The scoped FunctionCall key path derives from:

```text
wallet:{wallet_id}:near:direct-user-fc:{user_id}:{registry_id}:{receiver_id}:{method_names_hash}:v1
```

Its default label is:

```text
outlayer:{wallet_id}:direct-user-fc:{registry_id}:{receiver_id}:v1
```

For the counter proof, the coordinator requires `user_id == "mike.near"`,
`registry_id == "mike.sequential.near"`, `receiver_id == "count.mike.near"`,
`method_names == ["increase"]`, and attached deposit `0`.

These endpoints do not broadcast. The coordinator remains responsible for
policy, idempotency, registry proof checks, access-key presence checks, broadcast,
and receipt evidence.

### 6. Gate Dispatches the Batch

The first live target is the deployed mainnet sequential gate:

```text
gate.sequential.near
```

Read-only mainnet preflight on April 24, 2026 confirmed:

- owner: `sequential.near`
- approver: `approver.sequential.near`
- whitelisted relayer: `relayer.sequential.near`
- code hash: `B6UXBwbuk6JYorjDTqJJyqq7yn9kc7wjcdW4ggzvbkXB`
- fee for a 1 to 3 call batch: `0.03 NEAR`
- no pending batch tail at the time of the check

The gate verifies the signed delegate, stores each pending intent under
yield/resume, and later dispatches the FunctionCalls in chained order when the
approver calls `resume_batch_chained`.

The target contract sees the gate as predecessor. For NEAR Intents this is
acceptable only for methods that validate signed intent payloads in args, such
as `execute_intents`.

## First Mainnet Dust Shape

The first live money run should be deliberately small:

- Wallet: existing OutLayer deterministic implicit NEAR account.
- Token: `nep141:wrap.near`.
- Gate: `gate.sequential.near`.
- Batch size: 3 calls.
- Receiver: `intents.near`.
- Method: `execute_intents`.
- Amount per intent transfer: `1000000000000000000` yocto-wNEAR.
- Total amount moved: `3000000000000000000` yocto-wNEAR.
- Inner call gas: `100000000000000`.
- Attached deposit: `0`.

The WASI agent should submit exactly one batch, poll
`get-sequence-status`, and return the evidence. Automatic follow-on batches are
kept disabled for the first live run.

## Three Product Aims

This repo can carry three increasingly ambitious targets without losing the main
throughline: OutLayer wallet control, policy-first signing, evidence returned to
the TEE, and sequential gate use only where the target contract accepts proxy
predecessor semantics.

### 1. Basic `wrap.near` Wallet Work

The first aim is a clean wallet plumbing loop around native NEAR and wNEAR:

1. Resolve the OutLayer implicit wallet account and public key.
2. Fund it with a small amount of NEAR for gas.
3. Call `wrap.near.near_deposit` through existing wallet call support.
4. Deposit a small wNEAR amount into `intents.near`.
5. Read both wallet-side and Intents-side balances.
6. Optionally withdraw or unwrap a dust amount as a cleanup path.

This is not a sequential-gate target by itself. It proves the boring-but-critical
pieces: key derivation, wallet policy, storage handling, wrapping, Intents
deposit, balance reads, idempotent request tracking, and user-visible errors.
Existing scripts such as `tests/wallet_intents_e2e.sh` and `tests/gasless_e2e.sh`
already point in this direction.

Acceptance evidence should include transaction hashes, before/after NEAR and
wNEAR balances, Intents balance deltas, and clear failure messages for missing
storage, insufficient gas, or insufficient token balance.

### 2. Sequential Intents Swap: wNEAR to Wrapped Bitcoin

The second aim is to move from signed transfer intents to a small signed swap:
wNEAR in `intents.near` to a wrapped Bitcoin asset supported by NEAR Intents.

The flow should be:

1. Use the basic `wrap.near` path to fund and deposit wNEAR into the wallet's
   Intents balance.
2. Discover the exact wrapped Bitcoin asset identifier from the current Intents
   token/quote surface rather than hard-coding it.
3. Query a quote for `nep141:wrap.near` to the wrapped Bitcoin asset.
4. Run `simulate_intents` or equivalent validation for the signed payload.
5. Submit a sequential batch of `intents.near.execute_intents` calls through
   `gate.sequential.near`.
6. Return ordered gate evidence and final Intents balance deltas.

Before spending mainnet funds, do a chain prevalence check for this route. The
useful question is not just whether a quote exists, but whether recent
`intents.near` activity shows real solver support for wNEAR to wrapped Bitcoin:
recent successful swaps, total routed amount, solver diversity, observed
slippage, and failure rate. That chain scan belongs in the coordinator or a
one-off research script, not in the WASI guest.

The first version can still run a single batch only. Automatic second-batch
scheduling stays off until we have reliable evidence and accounting for one
money-moving sequence.

### 3. Rewards and Staking-Pool Automation

The third aim is a more agentic wallet workflow: observe staking or rewards
state, withdraw available rewards, optionally restake or convert the proceeds,
and return a verifiable report to the user.

This path is different from the Intents swap path because most staking-pool
contracts authenticate with `predecessor_id`. The current sequential gate would
make those contracts see the gate, not the wallet, so staking-pool calls should
not be forced through the gate unless the target explicitly accepts a signed
payload or proxy model.

The practical shape is:

1. The TEE reads staking-pool/rewards state off-chain.
2. The coordinator policy-checks any requested claim, withdraw, unstake, or
   restake action.
3. The wallet signs normal NEAR calls when the target needs the wallet as
   predecessor.
4. If rewards become wNEAR or another Intents-supported asset, later conversion
   can use the Intents path.
5. Sequential gate batching is reserved for the parts that are proxy-safe, such
   as signed `execute_intents` calls after funds are already in Intents.

This is the natural follow-up to the true-predecessor discussion. It may require
a scoped wallet-call surface or a dedicated rewards endpoint rather than a generic
delegate-signing API exposed to WASI.

The direct lane from the `fn` spike is the right mental model for these
predecessor-sensitive calls. Attached-deposit operations such as a user-account
`wrap.near.near_deposit` need separate policy and key-scope handling before we
claim they fit the narrower FunctionCall access-key path.

## Predecessor and DelegateAction Notes

There are two different signing stories:

1. Current implemented path:
   The OutLayer implicit wallet signs NEP-366 delegates for the gate and signs
   NEP-413 Intents payloads for `intents.near`. The gate dispatches as a proxy.

2. Follow-up direct true-predecessor lane:
   If `mike.near` adds a full access key controlled by the OutLayer TEE, the TEE
   can sign and broadcast normal NEAR transactions from `mike.near`. That would
   make target contracts see `predecessor_id == mike.near`, but it is a separate
   path from the current gate-as-proxy design. A narrower FunctionCall-scoped key
   is preferable when the target receiver, methods, and attached-deposit needs are
   known in advance.

The current sequential gate cannot make a dispatched receipt appear to come from
`mike.near`; it can only verify that `mike.near` or the wallet signed the
delegate and then dispatch from the gate account.

Adding an OutLayer-controlled key to `mike.near` is therefore provenance and
future compatibility unless the gate verifies that the delegate public key is an
active access key on the claimed sender account. With the current gate, deleting
that access key would not by itself revoke already-derived off-chain signing
ability; the practical kill switch is gate-side relayer/approver control.

## What We Borrowed From `fn`

The `/Users/mikepurvis/near/fn/near-outlayer` spike is useful as a research lab.
Its newest lesson is the Shape 1 pivot: for same-receiver batches that need true
user attribution, a native multi-action NEAR transaction signed by a
TEE-controlled user access key is cleaner than routing through the gate. That
does not replace our live Intents path; it gives us a second lane for future
predecessor-sensitive workflows.

Earlier `fn` artifacts also build delegates inside WASI, ask raw host functions
for signatures, call the gate directly, observe state, and can trigger a second
batch. That shape is excellent for proving mechanics, but too much signing
authority leaks into guest-visible primitives for the first production path.

We are borrowing:

- a mock target that records both authoritative `predecessor` and explicit
  `sender_arg`
- a non-commutative `deposit -> submit_intent -> settle` ordering harness
- duplicate-submit, duplicate-settle, and multi-sender isolation tests for the
  mock target
- receipt-log parsing across top-level and `result`-wrapped RPC outcomes
- explicit timing/trust notes around the roughly 202-block yield window
- the two-batch observe-then-decide pattern as a later capability
- the direct true-predecessor lane as a future product option for
  predecessor-sensitive workflows
- the dashboard binding-wizard pattern for a later direct-user setup page:
  redirect-safe local progress, RPC access-key presence checks, and honest trust
  envelope copy

We are not borrowing:

- raw delegate signing in production WIT
- raw `call` or `multi-call` host functions in production WIT
- guest-driven gate orchestration for the mainnet dust run
- the `mike.near` access-key path as the first proof
- the `fn` dashboard's exact `get_binding` / `bind()` API

The direct-user wizard should be adapted after the gate dust proof succeeds. In
this repo it should live under the existing wallet area and use the proof
registry shape already explored here: coordinator-owned `prepare-key` /
`prove-key`, registry `get_key_status`, and `prove_full_access` with exactly 1
yoctoNEAR.

## Implemented In This Repo

- `worker/wit/deps/wallet.wit`
  - Added `sequence-calls`.
  - Added `get-sequence-status`.
  - Added `plan-wallet-workflow`.
  - Added `execute-wallet-workflow`.
  - Added `get-wallet-workflow-status`.
  - Documented raw call payloads and `near_intents` helper mode.

- `worker/src/outlayer_wallet/host_functions.rs`
  - Validates sequential call shape before contacting the coordinator.
  - Enforces batch size 1 to 3.
  - Enforces exactly one payload mode per call.
  - Proxies sequential batch create/status requests to the coordinator.
  - Validates workflow JSON and proxies plan/execute/status requests.
  - Rejects raw signing fields in workflow payloads.
  - Keeps existing wallet rate limits in place.

- `keystore-worker/src/api.rs`
  - Adds `POST /wallet/sign-nep366-delegate`.
  - Adds `POST /wallet/direct-user/prepare-key`.
  - Adds `POST /wallet/direct-user/sign-proof`.
  - Adds `POST /wallet/direct-user/sign-function-calls`.
  - Rejects ambiguous call arguments.
  - Rejects mismatched derived sender IDs.
  - Supports optional current block height validation for delegate expiry.
  - Derives direct-user keys separately from implicit wallet keys.
  - Signs 1 yocto proof transactions and same-receiver direct-user FunctionCall
    transactions without exposing private keys.

- `keystore-worker/src/nep366.rs`
  - Adds NEP-366/NEP-461 Borsh-compatible delegate structs.
  - Signs one FunctionCall delegate.
  - Verifies byte compatibility against `near-primitives` delegate types in
    tests.

- `tests/sequential-intents/mock-intents`
  - Adds a test-only mock target for proving ordered gate dispatch.
  - Records `predecessor`, `sender_arg`, action, sequence number, and block
    height.
  - Supports both gate-proxy and direct-predecessor test modes.
  - Fails if `submit_intent` runs before `deposit` or `settle` runs before
    `submit_intent`.
  - Covers duplicate submit, duplicate settle, independent multi-sender state,
    and explicit direct-vs-gate predecessor evidence.

- `tests/sequential-intents/planner-harness`
  - Adds a test-only wallet execution planner classifier.
  - Emits ordered evidence for lane, predecessor model, ordering model, receiver,
    method, policy type, and rejection reason.
  - Covers gate-safe Intents batches, funding/setup work, direct-user staking
    work, and unsafe mixed gate batches.
  - Covers the production workflow `kind` vocabulary for Intents, funding, and
    direct-user FunctionCall steps.

- `tests/sequential-intents/coordinator-handoff`
  - Adds an executable handoff model for the private coordinator gate-first
    milestone.
  - Covers `sequential-batch` and wallet workflow execution.
  - Adds endpoint-shaped handlers for workflow plan/execute/status and
    sequential batch create/status, including JSON error responses and 404s.
  - Covers idempotency persistence before signing, policy denial before NEP-413
    or NEP-366 signing, and direct-user setup evidence.
  - Accepts prebuilt `near_intents` calls and explicitly proxy-safe raw call
    args, while rejecting predecessor-sensitive calls routed through the gate.
  - Encodes the first mainnet dust workflow and read-only preflight checks:
    gate identity, owner, approver, relayer whitelist, pending count, 1-3 call
    fee, wallet gas balance, Intents wNEAR balance, and signed-payload
    simulations.
  - Parses `intent_submitted`, `batch_started`, `intent_dispatched`, and
    `chain_continued` logs from both top-level and `result`-wrapped RPC shapes.
  - Preserves original call order even when submit receipts are observed out of
    order.
  - Returns ordered evidence for submit hashes, intent IDs, resume hash,
    dispatch receipts, block heights, balance delta, final status, and
    `proxy_predecessor: true`.

- `tests/sequential-intents/direct-user-proof`
  - Adds a test-only `sequential.near` subaccount proof/registry contract for the
    future direct-user lane.
  - Registers expected OutLayer public keys for user accounts by owner or user
    setup calls.
  - Records proof evidence only when signer, predecessor, signer public key, and
    exactly 1 yoctoNEAR all match.
  - Covers wrong-key, zero-deposit, wrong-signer, forwarded-call, expired-key,
    and revoked-key failures.

- `/Users/mikepurvis/other/outlayer-coordinator`
  - Adds the gate-first Axum coordinator service in the private coordinator
    checkout.
  - Implements wallet auth for internal WASI headers and configured
    `Bearer wk_...` wallet API keys.
  - Adds the sequential batch and workflow endpoints.
  - Persists idempotency records before signing or broadcasting.
  - Calls keystore NEP-413 and NEP-366 signing paths, relayer submit/resume
    paths, and NEAR RPC polling.
  - Returns TEE-usable evidence with `proxy_predecessor`, predecessor and
    ordering models, submit hashes, intent IDs, resume hash, dispatch receipts,
    block heights, balance deltas when available, final status, and
    user-visible errors.

## Verified Locally

The following checks passed:

```text
(cd tests/sequential-intents/coordinator-handoff && cargo test)
(cd tests/sequential-intents/planner-harness && cargo test)
cargo test outlayer_wallet --lib
cargo test call_args --bin keystore-worker
cargo test nep366 --bin keystore-worker
cargo test direct_user --bin keystore-worker
(cd tests/sequential-intents/mock-intents && cargo test)
(cd tests/sequential-intents/mock-intents && cargo near build non-reproducible-wasm)
(cd tests/sequential-intents/direct-user-proof && cargo test --locked)
(cd tests/sequential-intents/direct-user-proof && cargo near build non-reproducible-wasm)
cargo check --bin offchainvm-worker
cargo check --bin keystore-worker
git diff --check
(cd /Users/mikepurvis/other/outlayer-coordinator && cargo fmt)
(cd /Users/mikepurvis/other/outlayer-coordinator && cargo clippy --all-targets -- -D warnings)
(cd /Users/mikepurvis/other/outlayer-coordinator && cargo test)
(cd /Users/mikepurvis/other/outlayer-coordinator && SQLX_OFFLINE=true cargo check)
(cd /Users/mikepurvis/other/outlayer-coordinator && git diff --check)
```

The Rust checks still emit existing dead-code warnings unrelated to this spike.

## Remaining Mainnet Readiness Work

The gate-first coordinator implementation is now available, reviewed, committed,
and pushed. The remaining work before the first dust broadcast is operational and
integration-facing:

- Provision the coordinator environment and run the Postgres migration.
- Configure wallet auth, keystore auth, relayer account, approver account, RPC
  URL, gate account, and fee settings.
- Wire the deployed worker/coordinator path to the new endpoints.
- Run the coordinator test suite against the real service dependencies in a
  non-broadcast dry-run mode where possible.
- Fund the OutLayer implicit wallet for gas, wrap NEAR, and deposit dust wNEAR
  into `intents.near` through existing wallet routes.
- Run the read-only mainnet preflight immediately before broadcast.
- Simulate every signed Intents payload before the gate submit.
- Execute exactly one 3-call dust workflow with automatic follow-on batches
  disabled.
- Retry the same idempotency key and confirm no second batch is created.

The direct-user coordinator endpoints remain a follow-up after the gate dust
proof succeeds for broad predecessor-sensitive workflows. A narrower counter
proof endpoint set is implemented now:

- `POST /wallet/v1/direct-user/function-call-key/prepare`.
- `GET /wallet/v1/direct-user/function-call-key/status`.
- `POST /wallet/v1/direct-user/function-call/execute`.
- `GET /wallet/v1/direct-user/function-call/{request_id}`.

The broader direct-user setup/proof endpoints remain follow-up:

- Add `POST /wallet/v1/direct-user/prepare-key`.
- Add `POST /wallet/v1/direct-user/prove-key`.
- Add `GET /wallet/v1/direct-user/key-status?user_id=...`.
- Call the keystore direct-user prepare/proof/sign endpoints for
  predecessor-sensitive workflow steps.
- Check direct-user access-key presence by RPC before signing or broadcasting.
- Require registry proof before non-proof direct-user execution.
- Adapt the `fn` dashboard's binding flow into a polished wallet-area
  direct-user setup wizard using this repo's registry and coordinator APIs.

No contract deployment, coordinator restart, Docker work, or mainnet broadcast
has been performed from this checkout.
