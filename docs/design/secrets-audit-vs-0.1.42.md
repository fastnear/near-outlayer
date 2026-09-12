# Secrets: every entry point, v0.1.42 → working tree

Mainnet runs `v0.1.42` (tag `52dcd6a`, 2026-07-24) for worker, keystore and
contract. The coordinator has no tags; its mainnet commit is read off the box
(`docker inspect`) and filled in below. What follows is not a diff read — the
secret-related diff is ~7.5k lines across six files — but a trace of each place
a secret can enter or leave a run, old against new, with what covers it.

Baseline facts read from the tag: the worker had no manifest code at all
(`connector_manifest.rs` did not exist; zero mentions of `author_secrets`); the
contract's secret methods were `store_secrets`, `delete_secrets`,
`update_access`; the keystore's `decrypt_handler` went condition → seed with no
agent rule; the `AccessCondition` variant list is today's minus the appended
`ValidUntil` (row 8).

| # | entry point | v0.1.42 | working tree | changed by | covered by |
|---|---|---|---|---|---|
| 1 | request → `secrets_ref` capture | HTTPS: the wrapper body's `secrets_ref` honoured for every project; on chain: named in `request_execution`, forwarded by the event monitor | same, plus two things: on the connector path the coordinator BUILDS the reference from the payment key's owner when the body names none and `X-Use-Owner-Secret` is set (`agent_secrets_ref` in `call.rs`), and a reference the contract could never hold a row for — an `account_id` that is not a NEAR account id, a `profile` empty or over 64 characters — is refused AT THE DOOR with 400 `invalid_secrets_ref` (`well_formed_secrets_ref`), so no call row, no job and no keystore round trip is made for it. The bounds are the contract's own, never narrower: any profile text it would store passes | `44a5bf7` connectors as ordinary projects; header path with the agent-secret commits (`f2df2b3`, `d388a45`) | coordinator `a_connector_honours_the_body_reference_and_builds_one_only_when_absent`, `a_reference_that_can_name_no_row_is_refused_at_the_door`, `agent_secrets_ref` tests; live: `secrets_security_e2e.sh` M1–M8/B1–B4 (a hostile or malformed reference is answered, never with a 5xx, never with the secret) and P1; `connector_pricing_e2e.sh` C6 (fenced, `AGENT_SECRET_MODE`) |
| 2 | the caller the keystore judges | `caller = user_account_id.unwrap_or(&secrets_ref.account_id)` (`main.rs:1915` at the tag) — an absent sender was judged AS the owner | `proven_sender`: an absent sender refuses, for caller and author secrets alike | working tree | `proven_sender_tests`; unreachable in practice — contract `sender_id: AccountId` (`lib.rs:204`), `event_monitor.rs:1164`, `call.rs:3325`, `tasks.rs:587` all fill it |
| 3 | keystore `decrypt_handler` order | condition evaluated against `req.user_account_id` (`api.rs:1610` at the tag) → seed `project:{id}:{owner}` (`:1716`) | `enforce_agent_secret` first (an implicit-account PROFILE is readable only by that account, and only if it stored it), then the same condition, then the same seed | `f2df2b3`, `d388a45` | keystore `agent_secret_tests`; live: `connector_pricing_e2e.sh` C12 (fenced); catalogue U11 |
| 4 | env merge and reserved names | `merge_env_vars` inserted `NEAR_SENDER_ID` = payment-key owner / tx sender (`main.rs:939/976/1012` at the tag) | every `SYSTEM_ENV_VARS` name stripped from the merged secrets before the worker writes its own; `NEAR_SENDER_ID` = bound account or payer, `NEAR_USER_ACCOUNT_ID` always the payer | HoS stage 1 (2026-08-18) | worker `every_injected_variable_is_declared_a_system_name`, `no_secret_can_occupy_a_system_variable_on_either_path`, `a_bound_sender_renames_the_guest_but_not_the_payer`; live: `hos_identity_e2e.sh`; catalogue D11 |
| 5 | author secrets from the manifest | absent | `author_secrets_for_run` + `author_secrets_ref` + `merge_secret_maps`: the row the manifest names, decrypted into every run of any project carrying an `outlayer.manifest` section; condition judged against the real caller; collision with a caller key refuses; absent sender refuses | `6a461ef`, `d4ebca1` | worker `connector_manifest::tests` (`author_secrets_are_resolved_against_the_publishing_account`, `a_name_defined_on_both_sides_is_refused_not_resolved`); live: `03_project_model.sh` A1/A2/A3/A7/A10 (present for all, missing profile refuses, the admission gate, the collision, egress with a manifest), W5 on connector-probe (`CONNECTOR_TEST_LOG.md`); A4–A6/A9/A11 uncovered (below) |
| 6 | agent secrets | absent | contract `store_agent_secret` / `delete_agent_secret` (owner = implicit account of the signing key; signed message rebuilt in the keystore), coordinator `/wallet/v1/agent-secret/{pubkey,store,prepare,delete}`, keystore signing endpoint | `f2df2b3`, `d388a45`, `c2878db` | contract tests; coordinator tests; live: `connector_pricing_e2e.sh` C6/C12/C18/C19, `unified_vault_e2e.sh` V6 (all fenced) |
| 7 | contract `store_secrets` / `update_access` / `delete_secrets` | present (`secrets.rs:41/242/327` at the tag); `update_access` keyed on `(accessor, profile, caller)`, blob untouched | same, plus `canonical_profile` (payment-key nonces) and `assert_profile_is_unambiguous` on the agent door | `f2df2b3` and later | contract tests; live: `03_project_model.sh` A3/U6/U3/D1/D4/D5/D6 (every access change there is an `update_access`), `secrets_security_e2e.sh` R1/R2 (a non-owner's `update_access` changes nothing; an owner's empty whitelist refuses the owner) |
| 8 | `AccessCondition` variants | `Logic, Not, AllowAll, Whitelist, AccountPattern, NearBalance, FtBalance, NftOwned, DaoMember` | the same nine plus `ValidUntil { until_ns }`, APPENDED as the tenth (Borsh index 9 — `SecretProfile.access` is stored as Borsh of the enum directly, so the index is the storage layout and every row already on chain decodes by position); evaluated in the keystore as `now < until` against the host clock (a clock before the epoch answers `u64::MAX`, so it denies), a refusal names the lapsed limit, and that sentence now REACHES the caller: the worker used to replace any keystore 401 containing "Access denied" with a fixed string, which made a lapsed grant read exactly like never having been granted (`access_denied_message`, worker `keystore_client.rs`) | working tree (contract `types.rs`, keystore `types.rs`, worker `keystore_client.rs`, CLI `whitelist:acc@<date>` with a bare number refused rather than read as epoch seconds, dashboard "Until a date" + grant rows) | contract `borsh_positions_are_the_layout_rows_were_written_in`, keystore `a_time_limit_admits_before_and_denies_from_its_instant` + composition tests, worker `access_denied_tests`, CLI `deadlines_read_as_utc_instants`, dashboard `secrets-grants.test.mjs` (the UTC converters and the grant tree); `secret_access_conditions_e2e.sh` A1–A5/S1–S2 for the older variants; live: `secrets_security_e2e.sh` T1–T3 once the contract and keystore carrying the variant are deployed |
| 9 | what gates a run besides secrets | nothing manifest-driven | `may_run` (operation — checked before any secret is decrypted, so a call that will not run costs no keystore round trip), `resolve_network_policy` (allowlist, fail-closed for connectors, opt-in otherwise; `connector_id` outside the namespace flips it), `sub_key_connector_id` | `44a5bf7` onward | `connector_manifest::tests`; live: connector-probe `forbidden_fetch`, subkey-probe; catalogue A10 |
| 10 | coordinator HTTPS path (mainnet baseline) | **commit TBD** — read from the box | body `secrets_ref` honoured everywhere; header convenience on the connector path | — | to be filled when the baseline commit is known |
| 11 | **settling an HTTPS call refused at the secrets stage** | the refusal branch submits the failure to the CONTRACT (for a request that has no on-chain request) and completes the job; it never calls `complete_https_call`, and the coordinator settles an HTTPS call only through that endpoint — so the call sits `pending` until the timeout sweeper marks it `Timeout` and bills the compute limit for a run that never started | one path, `report_refusal` → `settle_refusal`, answers the contract (`submit_execution_result`) or the HTTPS caller (`complete_https_call`) with the real reason, then completes the job; used by every post-claim refusal: the three secrets refusals, both binding-verification arms (`refuse_claimed_jobs`), the connector operation check (`may_run`, no longer a `bail!`), both wasm-download failures | working tree (found live 2026-09-12, row D4 of the matrix; the same gap is in v0.1.42, so mainnet has it today; the binding and download sites also never answered the CONTRACT before) | worker `refusal_settlement_tests` (both doors answered; every site routed); live, REPRODUCED from seven angles 2026-09-12 on the current testnet worker: `03_project_model.sh` D4 and `secrets_security_e2e.sh` S1 (a row that exists, refused by its condition) plus M2–M5, M8 and B4 (a reference no row can match) — every one of them a 90-second silence ending in the caller's own timeout, never an answer. S1 needs the worker fix; the M/B rows are additionally prevented from reaching the worker at all by the door check in row 1, so either deploy clears them |

## Trust assumptions stated rather than discovered

- The keystore judges a secret's condition against the caller the **worker**
  sends. Both run in TEEs; the worker's value is the payment key's owner or the
  transaction's signer, never a claim from the body.
- `ValidUntil` reads the keystore host's clock — the same machine trust the rest of custody rests on. A clock it cannot read denies.
- The coordinator is trusted (decision 2026-08-13); nothing here defends against
  it, only against callers.

## Live coverage

Two suites carry the catalogue of the plan (`~/.claude/plans/iridescent-singing-clover.md`,
"Test catalogue"): `wasi-examples/test-secrets-example/tests/03_project_model.sh`
(A1 A2 A3 A7 A10 U1 U2 U3 U6 D1 D4 D5 D6 C1) and `tests/secrets_security_e2e.sh`
(the adversarial rows: deep nesting, malformed references, a non-owner's
`update_access`, header-and-body precedence, async, a stranger's bound wallet,
time limits). Rows that wait on a deploy say so in their own output:

- A connector call carrying a body `secrets_ref` over HTTPS (03 C1, security C2)
  — after the coordinator redeploy.
- An HTTPS run refused by a secret's condition, settled with the reason (row 11;
  03 D4, security rows judged by `status`) — after the worker redeploy; on the
  current testnet worker such a call hangs until the sweeper.
- `ValidUntil` live (security T1–T3) — after the contract and keystore redeploy.

## Not covered by any live test

- A4 (`DaoMember` admission gate: needs a sputnik DAO fixture), A5/A6 (manifest
  `owner` field: needs a second publishing account and a rebuilt artefact),
  A9 (two versions, one with a manifest), A11 (oversized or malformed manifest
  section: needs hand-built artefacts), D2/D3/D7 (a bound wallet on chain and a
  binding revoked: the bearer-token machinery of `bound_identity_onchain_e2e.sh`),
  K2 (keystore unreachable: operator toggle), K4 (deposit accounting at 2 000
  entries), W1 (a wallet storing its own row through `/wallet/v1/call`), G1–G3
  (Gmail: the test mailbox).
- The unknown-sender refusal — unreachable from any door by construction; unit
  only (`proven_sender_tests`).
- Unit-only by nature: reserved names at store time (A8, contract tests), the
  appended variant's layout (K3).

## Residual

Personal secrets stored with `AllowAll` before the interface default changes
stay readable by whoever names them. Handled by the dashboard notice, not by
code.
