# Next Steps: Mainnet Dust Run Readiness

This repo is past the pure research/spec stage for sequential NEAR Intents. The
gate-first coordinator branch now exists and has been pushed, but we should not
broadcast mainnet funds until it is configured against real services and dry-run
end to end.

The first live proof should stay deliberately narrow: the existing OutLayer
implicit wallet, dust wNEAR already deposited into `intents.near`, three signed
`intents.near.execute_intents` calls through `gate.sequential.near`, and no
automatic second batch.

There is also now an even smaller true-predecessor proof path: a scoped
FunctionCall key on `mike.near` for `count.mike.near::increase`. This path does
not use the sequential gate and is suitable for a first direct-user smoke test.
Read-only mainnet recheck on April 27, 2026: local `near` CLI is `4.0.13`, and
`count.mike.near.get_count({})` returned `4`.

## 0. Direct-User Counter Proof

Implemented keystore endpoints:

- `POST /wallet/direct-user/prepare-function-call-key`
- `POST /wallet/direct-user/sign-scoped-function-calls`

Implemented coordinator endpoints:

- `POST /wallet/v1/direct-user/function-call-key/prepare`
- `GET /wallet/v1/direct-user/function-call-key/status`
- `POST /wallet/v1/direct-user/function-call/execute`
- `GET /wallet/v1/direct-user/function-call/{request_id}`

Mainnet runbook:

1. Confirm or create `mike.sequential.near` under `sequential.near`; no contract
   deploy is required for this minimal proof.
2. Call the prepare endpoint with `user_id=mike.near`,
   `registry_id=mike.sequential.near`, `receiver_id=count.mike.near`, and
   `method_names=["increase"]`.
3. Add the returned public key to `mike.near`:
   `near add-key mike.near <PUBLIC_KEY> --contractId count.mike.near --methodNames increase --allowance 0.05 --networkId mainnet`.
4. Call the status endpoint and require `installed: true`.
5. Execute one idempotent counter call. The coordinator reads `get_count`, signs
   and broadcasts `increase` as `mike.near`, waits for the final transaction, and
   verifies `after == before + 1`.
6. Retry the same idempotency key and confirm no second increment happens.
7. Optionally remove the key with
   `near delete-key mike.near <PUBLIC_KEY> --networkId mainnet`.

Acceptance:

- Transaction signer is `mike.near`.
- Installed key permission is `FunctionCall`, receiver `count.mike.near`,
  method exactly `increase`, finite allowance, and zero attached deposit.
- Response includes public key, key label, tx hash, before/after count,
  `predecessor_model: "user"`, and final status.

## 1. Wire And Dry-Run The Coordinator Branch

Coordinator checkout:

```text
/Users/mikepurvis/other/outlayer-coordinator
remote: git@github.com:fastnear/outlayer-coordinator.git
branch: codex/gate-intents-coordinator
```

Implemented sequential endpoints:

- `POST /wallet/v1/sequential-batch`
- `GET /wallet/v1/sequential-batch/{request_id}`

Implemented workflow endpoints:

- `POST /wallet/v1/workflows/plan`
- `POST /wallet/v1/workflows/execute`
- `GET /wallet/v1/workflows/{request_id}`

The branch already implements the gate-first behavior: auth, Postgres-backed
idempotency, planner evidence, policy-before-signing, NEP-413 payload signing,
NEP-366 delegate signing, async gate submit, outer-receipt polling, resume,
dispatch polling, wrapped/top-level RPC outcome parsing, and ordered evidence.

Broad direct-user coordinator endpoints remain a follow-up after the first
counter proof and gate dust proof, not a blocker for the first Intents run:

- `POST /wallet/v1/direct-user/prepare-key`
- `POST /wallet/v1/direct-user/prove-key`
- `GET /wallet/v1/direct-user/key-status?user_id=...`

Before mainnet broadcast, the remaining coordinator work is:

- Run its Postgres migration in the coordinator environment.
- Configure wallet auth, keystore auth, relayer account, approver account, NEAR
  RPC URL, gate account, and fee settings.
- Point the deployed worker/coordinator path at the new endpoint handlers.
- Run `cargo test` and `SQLX_OFFLINE=true cargo check` in the coordinator
  checkout after environment-specific config is added.
- Exercise workflow plan/execute/status against mocked or dry-run dependencies.
- Confirm policy denial happens before keystore signing in the deployed stack.
- Confirm idempotency returns the same signed payloads and evidence on retry.

## 2. Reconfirm Idempotency Is Boring

The coordinator branch persists the batch record before signing or broadcasting.
Dry-run it against the real database and service wiring.

Persist at least:

- request id and idempotency key
- generated NEP-413 nonces and signed payloads
- generated NEP-366 delegate nonces and signed delegates
- submit transaction hashes
- parsed intent IDs
- resume/batch transaction hash
- dispatch receipt outcomes
- dispatch block heights
- final status and user-visible error, if any

Retrying the same idempotency key must return the same batch record. It must not
create new signatures, new intent IDs, or a second money-moving batch.

## 3. Reconfirm Evidence The TEE Can Reason About

The sequential batch response must keep the predecessor and ordering model
obvious.

Include:

- `proxy_predecessor: true`
- `predecessor_model: "gate"`
- `ordering_model: "gate_chained"`
- submit transaction hashes
- ordered intent IDs
- batch/resume transaction hash
- ordered dispatch outcomes
- dispatch block heights
- final status
- balance deltas where available

The response order must follow the original `calls[]` order, not submit
completion order.

## 4. Prove Ordering Before Mainnet Money

Use the local mock-intents harness and the coordinator branch tests to prove
coordinator behavior before the dust run.

Acceptance:

- Submit timing can be scrambled.
- `resume_batch_chained(intent_ids)` preserves the intended call order.
- The mock target final state reflects `deposit -> submit_intent -> settle`.
- Dispatch block heights are strictly increasing for gate-routed calls.
- Coordinator errors propagate to the WASI guest as user-visible errors.

Local fixtures already in this repo:

- `tests/sequential-intents/coordinator-handoff`
- `tests/sequential-intents/mock-intents`
- `tests/sequential-intents/planner-harness`
- `tests/sequential-intents/direct-user-proof`

The coordinator handoff fixture is the newest one. It models the private
coordinator's gate-first behavior locally for both `sequential-batch` and wallet
workflow requests: plan first, persist idempotency before signing, policy-check
before NEP-413 or NEP-366 signing, parse gate logs from top-level and
`result`-wrapped RPC shapes, preserve original call order when submit receipts
arrive out of order, and return ordered evidence.

The local handoff includes endpoint-shaped handlers for the private coordinator port:
workflow plan/execute/status and sequential batch create/status, with JSON error
responses, user-visible 404s, idempotency-key enforcement for execution, and
direct-user steps surfaced as `requires_direct_user_setup`.

It also encodes the first mainnet dust workflow and read-only preflight gates:
gate identity/config, relayer whitelist, pending state, fee tier, wallet gas
balance, Intents wNEAR balance, signed-payload simulations, expected
`proxy_predecessor` evidence, ordered dispatch block heights, and the exact
`3000000000000000000` yocto-wNEAR delta.

## 5. Mainnet Read-Only Preflight

Run this immediately before any broadcast.

Check:

- `gate.sequential.near` code hash and owner
- approver account
- relayer whitelist
- pending list and batch tail
- fee tier for a 1 to 3 call batch
- OutLayer implicit wallet NEAR balance for gas
- wallet wNEAR balance
- `intents.near` wNEAR balance for the wallet
- each signed Intents payload via `simulate_intents` or equivalent

Abort if the gate has unexpected pending state, the relayer/approver config has
changed, the fee tier is different, or any simulation fails.

## 6. First Mainnet Dust Run

Use the smallest useful live proof.

Parameters:

- gate: `gate.sequential.near`
- receiver: `intents.near`
- method: `execute_intents`
- calls: 3
- token: `nep141:wrap.near`
- amount per call: `1000000000000000000` yocto-wNEAR
- total amount: `3000000000000000000` yocto-wNEAR
- attached deposit per call: `0`
- gas per call: `100000000000000`
- automatic second batch: disabled

Acceptance:

- response includes submit tx hashes, ordered intent IDs, batch tx hash,
  dispatch outcomes, block heights, `proxy_predecessor: true`, and final status
- dispatch block heights are strictly increasing
- wallet Intents wNEAR balance decreases by exactly
  `3000000000000000000`
- no retry creates a second batch

Flow:

1. Use `execute-wallet-workflow` with exactly three `intents.transfer` steps.
2. Confirm the coordinator returns a plan with all three steps as `gate_proxy`.
3. Confirm policy checks pass before any signing.
4. Sign three NEP-413 Intents payloads and three NEP-366 delegates.
5. Async-submit three `gate.submit_intent` calls.
6. Poll only outer submit receipts and collect ordered intent IDs.
7. Call `resume_batch_chained(intent_ids)` with the configured approver and fee.
8. Poll dispatch receipts and return evidence to the TEE.
9. Retry the same idempotency key and confirm no new batch is created.

## 7. After The Dust Transfer Works

Only after the dust transfer proves the full evidence loop:

- Run a chain prevalence check for wNEAR to wrapped Bitcoin on NEAR Intents.
- Confirm recent successful swaps, solver support, amounts, slippage, and
  failure rate.
- Try a tiny wNEAR to wrapped Bitcoin swap with the same one-batch discipline.
- Keep staking/rewards automation on the future direct-user lane, not the gate
  lane, unless a target explicitly accepts proxy-safe signed payloads.
- Adapt the `fn` dashboard's binding wizard into this repo's wallet area for the
  direct-user lane. Borrow redirect-safe progress persistence, RPC access-key
  presence checks, and trust-envelope framing, but target this repo's
  `prepare-key` / `prove-key` / `get_key_status` / `prove_full_access` flow
  instead of the spike's `get_binding` / `bind()` API.

## 8. Direct-User Proof Lane

This is a separate follow-up from the first Intents dust run. It proves the
TEE-held FullAccess key shape before any predecessor-sensitive money movement.

Local fixture:

- `tests/sequential-intents/direct-user-proof`

Local keystore support:

- `POST /wallet/direct-user/prepare-key`
- `POST /wallet/direct-user/sign-proof`
- `POST /wallet/direct-user/sign-function-calls`

Dry proof sequence:

- Create a `sequential.near` subaccount such as `mike.sequential.near`.
- Deploy the proof/registry contract there.
- Derive the OutLayer direct-user Ed25519 public key for `mike.near`, using a
  stable label such as `direct-user-fa:mike.near`.
- Register that expected key on the proof contract.
- Add that public key as a FullAccess key on `mike.near`.
- Have the TEE sign a top-level transaction from `mike.near` to
  `mike.sequential.near::prove_full_access` with exactly 1 yoctoNEAR attached.
- Confirm the proof record shows signer, predecessor, signer public key,
  challenge, and block height.
- Before any later direct-user call, query RPC to confirm the key is still active
  on `mike.near`.
- For predecessor-sensitive actions, route through `execute-wallet-workflow` as
  `near.function_call` with `predecessor_requirement: "user_required"`.

Acceptance:

- The proof contract records signer and predecessor as `mike.near`.
- The recorded public key is the OutLayer-derived key.
- A forwarded call from `mike.sequential.near` does not satisfy true-predecessor
  semantics.
- Deleting the key from `mike.near` is treated as revocation even if the registry
  has not yet been updated.

## Current Boundary

No deployment, Docker work, coordinator restart, or mainnet broadcast should be
done from this repo pass. Human operators handle coordinator checkout,
deployment, credentials, and live execution.
