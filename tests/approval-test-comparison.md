# Agent-custody approval e2e — before/after deploy comparison

Command (both runs):
`ONLY=T4,T7,T9,T10 PARENT=zavodil2.testnet APPROVER2=t1.zavodil3.testnet ./tests/unified_op_e2e.sh --apply`
(default-vault mode — no `MPC_PUBLIC_KEY`, sub-wallets run under the coordinator's default vault)

---

## BASELINE — OLD code (pre-deploy) · 2026-06-07

Coordinator: `https://testnet-api.outlayer.fastnear.com` (OLD code, before the unified-op deploy).

**Setup: all green** — customer-recovery built; `Default-vault mode` log line present; 6 sub-wallets created under the default vault (old coordinator accepted the no-vault bearer); 6/6 `store_wallet_policy` succeeded on-chain; no hangs, no setup errors.

**Result: PASS = 4, FAILED = 4**

| Test / assertion | Result | Detail |
|---|---|---|
| T4a non-approver reject IGNORED | ✓ PASS | status stayed `pending_approval` |
| T4a real-approver reject VETOES | ✗ FAIL | reject did NOT veto — status stayed `pending_approval` |
| T4b approver YES executes | ✗ FAIL | `/approve` → **HTTP 401 invalid_signature** (old keystore expects 3-part vote; suite sends 4-part `approve:{id}:{wallet_pubkey}:{request_hash}`) → not executed |
| T7a gated op w/o approvals → pending | ✓ PASS | stayed pending, no execution |
| T7b substituted-hash approval rejected | ✓ PASS | `/approve` HTTP 401 `invalid_signature`, no execution — ⚠ via format-mismatch, not hash-binding |
| T7c cross-wallet replay rejected | ✓ PASS | `/approve` HTTP 401 `invalid_signature`, no execution — ⚠ via format-mismatch |
| T9a multisig swap → pending_approval | ✗ FAIL | **HTTP 403 policy_denied** `Transaction type 'intents_swap' is not allowed by policy` — old code rejects Trusted swap under multisig (T9b/T9c never ran) |
| T10a multisig cross-chain → pending_approval | ✗ FAIL | **HTTP 403 policy_denied** `Transaction type 'intents_withdraw' is not allowed by policy` (T10b/T10c never ran) |

**Expected after deploy:** T4 → PASS (4-part vote accepted, approve executes, real-approver reject vetoes); T9 → PASS (returns `pending_approval` + approval_id + request_hash, then executes); T10 → PASS (same); T7 → stays PASS, but now rejecting via **hash-binding** rather than format-mismatch (if T7 fails after deploy, that's a real regression).

Raw log of the baseline run: `/tmp/uop_run.log`.

---

## AFTER DEPLOY — NEW code · 2026-06-07

Coordinator: `https://testnet-api.outlayer.fastnear.com` (testnet updated by the user). Setup: all green (same default-vault mode; 6/6 policies stored). Raw log: `/tmp/uop_after.log`.

**Result: PASS = 5, FAILED = 3** (was 4/4).

| Test / assertion | Baseline (old) | After deploy (new) | Verdict |
|---|---|---|---|
| T4b approver YES executes | ✗ 401 | ✓ `/approve` HTTP 200 → executed | **FIXED** |
| T7a gated → pending | ✓ | ✓ | OK |
| T7b substituted-hash rejected | ✓ (format-mismatch) | ✓ (HTTP 401 invalid_signature) | OK |
| T7c cross-wallet replay rejected | ✓ (format-mismatch) | ✓ (HTTP 401 invalid_signature) | OK |
| T4a real-approver reject VETOES | ✗ | ✗ still `pending_approval` after a valid reject | **STILL FAIL** |
| T9a multisig swap → pending_approval | ✗ 403 `type 'intents_swap' not allowed` | ✗ 403 `Capability for 'swap' is not enabled by policy` | **STILL FAIL** (error changed) |
| T10a multisig cross-chain → pending_approval | ✗ 403 `type 'intents_withdraw' not allowed` | ✗ 403 `Capability for 'cross_chain_withdraw' is not enabled by policy` | **STILL FAIL** (error changed) |

## Diagnosis (2026-06-07)

**T4a (reject veto) — behavior/test mismatch, NOT a deploy issue.** On the new code the reject vote is now ACCEPTED (4-part message verifies), but the request stays `pending_approval`. By design (REJECT VETO), a reject is a stored NO vote that BLOCKS a later approve from executing — it does NOT proactively cancel/reject the pending request. T4a asserts the status flips to reject/cancel/fail after a lone reject, which the code doesn't do. → Fix: either change T4a to assert the veto BLOCKS a subsequent approve (no execution), or implement "a decisive reject (threshold unreachable) cancels the pending." (T4b approve→execute works; the binding is fine.)

**T9/T10 (capability not enabled) — ROOT CAUSE FOUND + FIXED. Real bug, NOT a deploy skew.**
Live read-back proved it: after storing `{rules:{transaction_types:["swap"]},capabilities:{swap:{allowed:true}}}`, `GET /wallet/v1/policy` returned `{rules:{transaction_types:["swap"]},approval:null,usage:{}}` — `capabilities` ENTIRELY ABSENT. The coordinator's `encrypt_policy` handler (`handlers.rs:3814`) rebuilds a canonical policy from a FIXED field whitelist that never included `capabilities`, and `EncryptPolicyRequest` (`types.rs:272`) had no `capabilities` field → serde silently dropped the client's capabilities on deserialize, so the encrypted blob the keystore decrypts genuinely has none. Pre-existing since commit `6aa5f47` (2026-03-09); never updated when the capabilities feature landed. **This silently disabled EVERY capability gate (swap, cross_chain_withdraw, payment_check, raw_sign, confidential, sign_message) end-to-end** — the crate/unit tests missed it because they exercise `evaluate` directly, bypassing the HTTP encrypt path.
**Fix (coordinator, compiles + 194 tests):** `EncryptPolicyRequest` += `capabilities`; `encrypt_policy` json! += `"capabilities": req.capabilities`; `get_policy` extracts + echoes `capabilities` (dashboard display) + `PolicyResponse` += field. Docs/OpenAPI/SDK needed NO change — Phase 6 already documented `capabilities` in `EncryptPolicyRequest`/`PolicyResponse` (the docs were AHEAD of the code; the bug was code-only).

---

## AFTER CAPABILITIES FIX + RE-DEPLOY · 2026-06-07

Coordinator re-deployed with the fix. Re-ran `ONLY=T4,T7,T9,T10 --apply` (default-vault). **Result: passed 13 / FAILED 1.** Zero `403` / `policy_denied` / `Capability ... not enabled` in the log.

| Test | pre-fix (after deploy) | after capabilities fix | Verdict |
|---|---|---|---|
| T9 multisig swap | ✗ 403 capability not enabled | ✓ `pending_approval` + approval_id + request_hash; approve HTTP 200; leaves pending after threshold | **FIXED** |
| T10 multisig cross-chain | ✗ 403 capability not enabled | ✓ same | **FIXED** |
| T4b approve executes | ✓ | ✓ | OK |
| T7a/b/c negatives | ✓ | ✓ | OK |
| T4a real-approver reject vetoes | ✗ | ✗ (unchanged) | **STILL OPEN — design decision** |

The capabilities fix resolved T9/T10. The ONLY remaining failure is **T4a**: a reject from a real approver BLOCKS a later approve (verify_approvals vetoes on any real reject) but does NOT flip the pending request's status; the test asserts a status change. Since any single real reject already vetoes the whole request (it can never execute), cancelling it on reject is the cleaner behavior. **RESOLVED → chose (a): implemented "decisive reject cancels the pending"** (coordinator, compiles + 194 tests). The reject handler now checks whether the SIGNING KEY (proven by the NEP-413 sig) is one of the policy's configured approver pubkeys (read from the decrypted policy, same source as get_policy — best-effort status update; the keystore stays the security arbiter at sign time). Real approver → request marked `rejected`; non-approver / unpinned → ignored (status stays pending, keystore still vetoes). **DONE — re-deployed + re-ran:** `✓ T4a non-approver reject IGNORED (status=pending_approval)` and `✓ T4a real-approver reject → vetoed (status=rejected)`.

---

## FINAL — all green · 2026-06-07

After both coordinator fixes (capabilities forwarding + reject-cancel) were deployed, `ONLY=T4,T7,T9,T10 --apply` → **14 passed / 0 failed · EXIT 0 · "ALL UNIFIED-OP E2E CHECKS PASSED"**. Progression: baseline 4/4 → after-deploy 5/3 → after capabilities fix 13/1 → after reject-cancel fix **14/0**. T4/T7/T9/T10 all green on testnet. (T9c/T10c terminated `failed` downstream on testnet liquidity, not the multisig gate — control-flow assertion passes.)
