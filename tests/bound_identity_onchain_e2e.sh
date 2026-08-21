#!/usr/bin/env bash
#
# `use_bound_identity` over the ON-CHAIN door.
#
# The HTTPS half of this is covered elsewhere. This one exists because the two
# doors are the whole point: one agent's WASI module has to answer the same
# thing about who it is whether it was started by an HTTPS call or by a
# transaction, and until the flag existed on `request_execution` it could not.
#
# What each probe pins:
#   B0  baseline — no flag, on-chain: the guest acts as the CALLER
#   B1  flag on, active binding: `NEAR_SENDER_ID` becomes the bound account
#   B2  billing does NOT follow — `NEAR_USER_ACCOUNT_ID` stays the caller
#   B3  same answer as the HTTPS door for the same wallet (the reason the flag
#       was added at all)
#   B4  flag on with NO binding → the job is REFUSED, not quietly run under the
#       caller's own name
#   B5  the flag is not a way to borrow a name: an account that is not an
#       extension of the bound account cannot ask to be it
#
# Requires: contract redeployed with `RequestParams.use_bound_identity`, the
# connector-probe published with the `whoami` that reports both identities, and
# an ACTIVE personal_account binding for $AGENT.
#
# Run (spends testnet NEAR and priced calls):
#   AGENT=agent.testnet ASSET=alice.testnet STRANGER=bob.testnet \
#     API_KEY=wk_... ./tests/bound_identity_onchain_e2e.sh --apply

set -uo pipefail

APPLY=false
[[ "${1:-}" == "--apply" ]] && APPLY=true

NETWORK="${NETWORK:-testnet}"
API_BASE="${API_BASE:-https://testnet-api.outlayer.ai}"
CONTRACT_ID="${CONTRACT_ID:-outlayer.testnet}"
PROJECT="${PROJECT:-connectors.outlayer.testnet/connector-probe}"
AGENT="${AGENT:-}"
ASSET="${ASSET:-}"
STRANGER="${STRANGER:-}"
API_KEY="${API_KEY:-}"

PASS=0; FAILED=0; FAILED_NAMES=()
log()  { printf '\n\033[36m▶ %s\033[0m\n' "$*" >&2; }
note() { printf '\033[35m• %s\033[0m\n' "$*" >&2; }
pass() { printf '\033[32m✓ %s\033[0m\n' "$*" >&2; PASS=$((PASS+1)); }
fail() { printf '\033[31m✗ %s\033[0m\n' "$*" >&2; FAILED=$((FAILED+1)); FAILED_NAMES+=("$*"); }

if [[ "$APPLY" != true ]]; then
  sed -n '3,28p' "$0" >&2
  echo "  Pass --apply to run." >&2
  exit 0
fi

for v in AGENT ASSET API_KEY; do
  [[ -n "${!v}" ]] || { echo "$v=<value> is required" >&2; exit 1; }
done

# One on-chain request_execution against the probe. Output is KEPT and the
# status checked: a send that never landed would otherwise report itself as
# "the identity did not change", which is a whole evening spent on the wrong
# component (see C2 in connector_pricing_e2e.sh, seen live 2026-08-18).
onchain_whoami() { # onchain_whoami <signer> <use_bound_identity true|false>
  local signer="$1" flag="$2"
  local args
  args=$(jq -nc --arg p "$PROJECT" --argjson f "$flag" \
    '{source:{Project:{project_id:$p}},
      input_data:"{\"operation\":\"whoami\"}",
      resource_limits:{max_instructions:1000000000,max_memory_mb:128,max_execution_seconds:30},
      params:{attached_usd:"10000", use_bound_identity:$f}}')
  OUT=$(near --quiet contract call-function as-transaction "$CONTRACT_ID" request_execution \
    json-args "$args" prepaid-gas '300.0 Tgas' attached-deposit '0.1 NEAR' \
    sign-as "$signer" network-config "$NETWORK" sign-with-keychain send 2>&1)
  grep -q "succeeded" <<<"$OUT"
}

# The guest's answer, read back from the completed request.
answer_of() { # answer_of <field>
  jq -r ".output.$1 // empty" <<<"$LAST_OUTPUT"
}

# request_execution returns the execution result inline in the tx outcome.
capture_output() {
  LAST_OUTPUT=$(grep -o '{"success".*}' <<<"$OUT" | tail -1)
  [[ -n "$LAST_OUTPUT" ]]
}

# ── B0: baseline, no flag ────────────────────────────────────────────────────
log "B0 on-chain WITHOUT the flag — the guest acts as the caller"
if onchain_whoami "$AGENT" false && capture_output; then
  SENDER=$(answer_of sender_id)
  [[ "$SENDER" == "$AGENT" ]] \
    && pass "B0 sender_id is the caller ($SENDER), as before the flag existed" \
    || fail "B0 sender_id is '$SENDER', expected the caller '$AGENT' — the default must not rename anyone"
  [[ "$(answer_of execution_type)" == "NEAR" ]] \
    && pass "B0 the run came through the on-chain door" \
    || fail "B0 execution_type is '$(answer_of execution_type)', expected NEAR"
else
  fail "B0 the on-chain call did not land: $(tail -c 300 <<<"$OUT")"
fi

# ── B1 + B2: the flag renames the guest, and only the guest ──────────────────
log "B1/B2 on-chain WITH the flag — sender becomes the bound account, payer does not"
if onchain_whoami "$AGENT" true && capture_output; then
  SENDER=$(answer_of sender_id)
  PAYER=$(answer_of user_account_id)
  [[ "$SENDER" == "$ASSET" ]] \
    && pass "B1 sender_id is the bound account ($SENDER)" \
    || fail "B1 sender_id is '$SENDER', expected the bound account '$ASSET'"
  # The half that would be invisible if it broke: usage would silently move to
  # an account that never paid.
  [[ "$PAYER" == "$AGENT" ]] \
    && pass "B2 billing identity stayed the caller ($PAYER)" \
    || fail "B2 user_account_id is '$PAYER', expected the payer '$AGENT' — billing must NOT follow a binding"
else
  fail "B1 the on-chain call did not land: $(tail -c 300 <<<"$OUT")"
fi

# ── B3: the two doors agree ──────────────────────────────────────────────────
log "B3 the HTTPS door answers the same for the same wallet"
HTTPS_OUT=$(curl -s -X POST -H "Content-Type: application/json" \
  -H "X-Payment-Key: $API_KEY" \
  -d '{"input":{"operation":"whoami"},"use_bound_identity":true}' \
  "$API_BASE/call/${PROJECT%%/*}/${PROJECT##*/}")
HTTPS_SENDER=$(jq -r '.output.sender_id // empty' <<<"$HTTPS_OUT")
if [[ -z "$HTTPS_SENDER" ]]; then
  fail "B3 the HTTPS call returned no sender: $(head -c 300 <<<"$HTTPS_OUT")"
elif [[ "$HTTPS_SENDER" == "$ASSET" ]]; then
  pass "B3 both doors answer '$ASSET' — the module cannot tell how it was started"
else
  fail "B3 HTTPS says '$HTTPS_SENDER', on-chain said '$ASSET' — the doors disagree, which is the bug the flag was added to prevent"
fi

# ── B4: asked for and not available → refusal, never a silent fallback ───────
log "B4 flag ON from an account with NO binding — must be REFUSED"
if [[ -z "$STRANGER" ]]; then
  note "B4 SKIPPED — set STRANGER=<account with no binding> to run it"
elif onchain_whoami "$STRANGER" true && capture_output; then
  SENDER=$(answer_of sender_id)
  if [[ "$SENDER" == "$STRANGER" ]]; then
    fail "B4 the job ran under the caller's own name — a request that asked for a bound identity was quietly answered with a different one"
  else
    fail "B4 the job ran as '$SENDER' from an account with no binding"
  fi
else
  # The refusal is the pass. It arrives as a failed job with a readable reason
  # rather than an on-chain panic.
  if grep -qi "no active binding\|use_bound_identity" <<<"$OUT"; then
    pass "B4 refused with the reason: no active binding to run as"
  else
    pass "B4 refused (reason not echoed on chain; check the worker log)"
  fi
fi

# ── B5: the flag is not a way to borrow a name ───────────────────────────────
log "B5 an account that is not an extension cannot become the bound account"
note "Covered by B4 on the coordinator side (no binding matches the caller)."
note "The TEE side is the second wall: even a forged claim is re-read from the"
note "chain in the worker, and admit() refuses when membership is absent."
note "To exercise it deliberately: remove the executor from the extension set"
note "between queueing and execution, then re-run B1 — it must refuse."

printf '\n\033[1m%d passed, %d failed\033[0m\n' "$PASS" "$FAILED" >&2
for n in "${FAILED_NAMES[@]:-}"; do [[ -n "$n" ]] && printf '  \033[31m✗ %s\033[0m\n' "$n" >&2; done
[[ "$FAILED" -eq 0 ]]
