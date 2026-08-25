#!/usr/bin/env bash
#
# §2 of the HoS test plan — every endpoint on the partner's method list, run
# through the classes that apply to it: HAPPY / AUTHZ / INPUT / BYPASS / STATE
# / DoS.
#
# `binding_lifecycle_e2e.sh` already drives the happy path of PUT/GET/DELETE
# and the setup kit (L1–L8) and is not repeated here. What this adds is
# everything that is NOT the happy path, plus the endpoints that appeared with
# the fund lane: `/binding/balance`, `/binding/transfer`, `/binding/events`,
# and the regression that the own-wallet endpoints still mean the wallet.
#
# Most probes cost nothing on chain: a refusal is decided before anything is
# signed, and a PUT names an account without proving anything about it (that
# is the point of `pending`). The two that do spend are marked.
#
#   PARENT=you.testnet ./tests/hos_endpoints_e2e.sh --apply
#
# Needs the shared fixture (tests/hos_fixture.sh --apply).

source "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/lib/hos_common.sh"

HOS_STATE="${HOS_STATE:-$REPO_ROOT/tests/.hos_fixture.env}"
[[ "${1:-}" == "--apply" ]] || { sed -n '3,20p' "$0" >&2; echo "  Pass --apply to run." >&2; exit 0; }
hos_require
[[ -f "$HOS_STATE" ]] || { echo "✗ no fixture — run tests/hos_fixture.sh --apply first" >&2; exit 1; }
# shellcheck disable=SC1090
source "$HOS_STATE"

WL="${WL:-zavodil2.testnet}"
# The setup kit refuses any account that already runs code, and zavodil2 does.
# So the kit's happy path needs an account created for it — cheap, and deleted
# on the way out.
EMPTY="hos-empty-$(openssl rand -hex 3).$PARENT"
SEED_A="hos-ep-a-$(date +%s)-$$"
SEED_B="hos-ep-b-$(date +%s)-$$"
read -r WID_A EXEC_A < <(wallet_address "$SEED_A")
read -r WID_B EXEC_B < <(wallet_address "$SEED_B")
[[ -n "$WID_A" && -n "$WID_B" ]] || { echo "✗ could not mint the throwaway wallets" >&2; exit 1; }
note "throwaway wallets: A=$WID_A B=$WID_B"
log "Creating $EMPTY — an account with no code, for the setup kit"
create_subaccount "$EMPTY" 0.1 || { echo "✗ $EMPTY never appeared" >&2; exit 1; }

# The fixture wallet's policy is whatever the previously-run suite left on it,
# and E11 spends through it. Set our own: no suite may be judged against
# another suite's policy, or the failure lands on whichever ran second.
log "Setting this suite's own policy on the fixture wallet"
store_policy "$SEED" "$WALLET_ID" \
  "$(jq -nc --arg s "$ASSET" --arg w "$WL" --arg e "$EMPTY" \
     '{rules:{addresses:{mode:"whitelist",list:[$s,$w,$e]},limits:{per_transaction:{native:"1000000000000000000000000"}}}}')" \
  || warn "the suite policy could not be stored — E11's spend may be refused by an inherited rule"

cleanup() {
  local rc=$?
  api "$SEED_A" DELETE /wallet/v1/binding >/dev/null 2>&1 || true
  api "$SEED_B" DELETE /wallet/v1/binding >/dev/null 2>&1 || true
  account_exists "$EMPTY" && { note "cleaning up $EMPTY"; delete_account "$EMPTY"; }
  return $rc
}
trap cleanup EXIT

put() { api "$1" PUT /wallet/v1/binding "$2" >/dev/null; }

# ── E1 AUTHZ: every endpoint refuses an unauthenticated caller ──────────────
log "E1 no Authorization header anywhere on the binding surface"
for spec in \
  "PUT|/wallet/v1/binding|{\"asset_account_id\":\"$WL\",\"kind\":\"personal_account\"}" \
  "GET|/wallet/v1/binding|" \
  "DELETE|/wallet/v1/binding|" \
  "GET|/wallet/v1/binding/setup?kind=personal_account|" \
  "GET|/wallet/v1/binding/balance|" \
  "POST|/wallet/v1/binding/transfer|{\"to\":\"$WL\",\"amount\":\"1\"}"
do
  m=${spec%%|*}; rest=${spec#*|}; p=${rest%%|*}; b=${rest#*|}
  api - "$m" "$p" "$b" >/dev/null
  assert_status "E1 $m $p" 401
done

# ── E2 GET on a wallet that has no binding ─────────────────────────────────
log "E2 GET /binding on an unbound wallet"
api "$SEED_A" GET /wallet/v1/binding >/dev/null
assert_status "E2 unbound GET" 404

# ── E3 PUT input validation ────────────────────────────────────────────────
log "E3 PUT input validation"
IMPLICIT="0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
LONGNAME="$(python3 -c 'print("a"*300)').testnet"

put "$SEED_A" "$(jq -nc --arg a "$WL" '{asset_account_id:$a, kind:"leased"}')"
assert_denied "E3a unknown kind" && assert_msg "E3a names the accepted kinds" "hos_lease.*personal_account|kind must be"

put "$SEED_A" '{"kind":"personal_account"}'
assert_denied "E3b asset_account_id missing"

put "$SEED_A" "$(jq -nc --arg a "$IMPLICIT" '{asset_account_id:$a, kind:"personal_account"}')"
assert_denied "E3c a 64-hex implicit account" && assert_msg "E3c says a named account is required" "implicit|named account"

put "$SEED_A" "$(jq -nc --arg a "$EXEC_A" '{asset_account_id:$a, kind:"personal_account"}')"
assert_denied "E3d the wallet's OWN executor as the asset" \
  && assert_msg "E3d says which rule bit — an implicit executor is caught by the named-account rule first" "own executor|implicit"

put "$SEED_A" '{"asset_account_id":"Bad Name!.testnet","kind":"personal_account"}'
assert_denied "E3e an invalid account id"

put "$SEED_A" "$(jq -nc --arg a "$LONGNAME" '{asset_account_id:$a, kind:"personal_account"}')"
assert_denied "E3f a 300-character account id"

api_raw "$SEED_A" PUT /wallet/v1/binding 'this is not json at all'
assert_denied "E3g a body that is not JSON"

put "$SEED_A" "$(jq -nc --arg a "$WL" '{asset_account_id:$a, kind:"hos_lease", owner_account_id:"owner.testnet"}')"
assert_denied "E3h hos_lease without impl_version" && assert_msg "E3h names the field" "impl_version"

put "$SEED_A" "$(jq -nc --arg a "$WL" '{asset_account_id:$a, kind:"hos_lease", owner_account_id:"owner.testnet", impl_version:999}')"
assert_denied "E3i hos_lease with an unsupported impl_version (§3.2 version gate)" \
  && assert_msg "E3i names the supported versions and says it is terminal" "supported|terminal"

put "$SEED_A" "$(jq -nc --arg a "$WL" '{asset_account_id:$a, kind:"hos_lease", impl_version:6}')"
assert_denied "E3j hos_lease without owner_account_id" && assert_msg "E3j names the field" "owner_account_id"

put "$SEED_A" "$(jq -nc --arg a "$WL" '{asset_account_id:$a, kind:"personal_account", owner_account_id:"someone-else.testnet"}')"
assert_denied "E3k personal_account with an owner that is not the account" && assert_msg "E3k says the owner IS the account" "owner IS the account|must equal"

put "$SEED_A" "$(jq -nc --arg a "$WL" '{asset_account_id:$a, kind:"personal_account", impl_version:1}')"
assert_denied "E3l personal_account with impl_version — refused, not ignored" && assert_msg "E3l names the field" "impl_version"

# ── E4 the body cannot name a different wallet ─────────────────────────────
log "E4 PUT carrying wallet_id of ANOTHER wallet in the body"
put "$SEED_A" "$(jq -nc --arg a "$EMPTY" --arg w "$WID_B" '{asset_account_id:$a, kind:"personal_account", wallet_id:$w}')"
if [[ "$HTTP" == "200" ]]; then
  [[ "$(jq -r '.wallet_id' <<<"$BODY")" == "$WID_A" ]] \
    && pass "E4 the binding belongs to the AUTHENTICATED wallet, not the one the body named" \
    || fail "E4 the body's wallet_id was honoured — a caller could bind somebody else's wallet"
  api "$SEED_B" GET /wallet/v1/binding >/dev/null
  assert_status "E4 wallet B is still unbound" 404
else
  fail "E4 PUT failed ($HTTP): $(msg_of)"
fi

# ── E5 exclusivity: pending claims nothing, active is exclusive ────────────
log "E5 a second wallet naming the SAME account while A's row is only pending"
put "$SEED_B" "$(jq -nc --arg a "$EMPTY" '{asset_account_id:$a, kind:"personal_account"}')"
if [[ "$HTTP" == "200" ]]; then
  pass "E5 a pending row is anyone's claim to make and nobody's to keep — B was not blocked by A's"
else
  fail "E5 B was blocked ($HTTP) by a row that authorizes nothing: $(msg_of)"
fi
log "E5' the same wallet naming the fixture's ACTIVE account"
put "$SEED_B" "$(jq -nc --arg a "$ASSET" '{asset_account_id:$a, kind:"personal_account"}')"
assert_denied "E5' an account already operated by another wallet" \
  && assert_msg "E5' says which wallet must release it first" "another wallet|already bound|already operated"

# ── E6 STATE: DELETE then re-bind ──────────────────────────────────────────
log "E6 DELETE, then PUT the same account again"
api "$SEED_B" DELETE /wallet/v1/binding >/dev/null; H1=$HTTP
api "$SEED_B" DELETE /wallet/v1/binding >/dev/null; H2=$HTTP
[[ "$H1" =~ ^2 ]] && pass "E6 DELETE removed it (HTTP $H1)" || fail "E6 DELETE answered $H1"
[[ "$H2" =~ ^2 ]] && pass "E6 a second DELETE is still a success (HTTP $H2) — cleanup can be re-run" \
  || fail "E6 the second DELETE answered $H2 — a retried cleanup would fail on it"
put "$SEED_B" "$(jq -nc --arg a "$WL" '{asset_account_id:$a, kind:"personal_account"}')"
assert_status "E6 re-binding the same account after a DELETE" 200
api "$SEED_B" DELETE /wallet/v1/binding >/dev/null

# ── E7 the setup kit ───────────────────────────────────────────────────────
log "E7 the setup kit"
api "$SEED_A" GET "/wallet/v1/binding/setup?kind=personal_account" >/dev/null
if [[ "$HTTP" == "200" ]]; then
  assert_json "E7 one transaction" '.transactions | length' 1
  assert_json "E7 three actions" '.transactions[0].actions | length' 3
  assert_json "E7 the pinned artifact" '.code_hash' "$PINNED_HASH_B58"
  assert_json "E7 signer is the owner's own account" '.transactions[0].signer_id' "$EMPTY"
  assert_json "E7 receiver is the owner's own account" '.transactions[0].receiver_id' "$EMPTY"
else
  fail "E7 the kit failed for a bound, code-free account (HTTP $HTTP): $(msg_of)"
fi

api "$SEED_A" GET "/wallet/v1/binding/setup?kind=hos_lease" >/dev/null
assert_denied "E7' no kit for hos_lease" && assert_msg "E7' says the partner provisions those" "no setup kit|provisioned"

api "$SEED_A" GET "/wallet/v1/binding/setup?kind=nonsense" >/dev/null
assert_denied "E7'' an unknown kind"

api "$SEED_A" GET "/wallet/v1/binding/setup" >/dev/null
assert_denied "E7''' the kit with no kind at all"

api "$SEED_B" GET "/wallet/v1/binding/setup?kind=personal_account" >/dev/null
assert_denied "E7'''' the kit for a wallet with no binding"

log "E8 the kit refuses an account that ALREADY runs code (the 2FA/multisig victim)"
api "$SEED" GET "/wallet/v1/binding/setup?kind=personal_account" >/dev/null
assert_denied "E8 refused" \
  && assert_msg "E8 says the existing state would NOT be cleared" "already has a contract|would NOT be cleared|not be cleared"

# ── E9 the partner webhook ─────────────────────────────────────────────────
log "E9 POST /binding/events — the shared secret is the ONLY thing guarding it"
api - POST /wallet/v1/binding/events "$(jq -nc --arg a "$ASSET" '{asset_account_id:$a, event:"revoked"}')" >/dev/null
H_NONE=$HTTP; B_NONE=$BODY
api - POST /wallet/v1/binding/events "$(jq -nc --arg a "$ASSET" '{asset_account_id:$a, event:"revoked"}')" \
  -H 'x-binding-webhook-secret: definitely-not-the-secret' >/dev/null
H_WRONG=$HTTP
if [[ "$H_NONE" == "503" ]]; then
  finding "the testnet coordinator has no BINDING_WEBHOOK_SECRET configured: every partner lifecycle event answers 503 'binding events are not configured on this deployment'. Their revoke/rotation webhook is dead on this deployment until an operator sets it — and §6's 'revoke invalidates a cached allow inside the TTL' cannot be proved here."
  skip "E9 webhook HAPPY + zone list + cache invalidation — needs BINDING_WEBHOOK_SECRET on the testnet coordinator"
  [[ "$H_WRONG" =~ ^[45] ]] \
    && pass "E9 a presented secret is still refused while unconfigured (HTTP $H_WRONG) — unconfigured never means open" \
    || fail "E9 an unconfigured deployment ACCEPTED an event (HTTP $H_WRONG)"
else
  [[ "$H_NONE" =~ ^4 ]] && pass "E9 no secret → refused $H_NONE" || fail "E9 no secret → $H_NONE, expected a refusal: $(msg_of "$B_NONE")"
  [[ "$H_WRONG" =~ ^4 ]] && pass "E9 a wrong secret → refused $H_WRONG" || fail "E9 a wrong secret → $H_WRONG"
fi

# ── E10 /binding/balance ───────────────────────────────────────────────────
log "E10 GET /binding/balance"
api "$SEED" GET "/wallet/v1/binding/balance?chain=near" >/dev/null
if [[ "$HTTP" == "200" ]]; then
  CHAIN_ASSET=$(account_field "$ASSET" amount)
  REPORTED=$(jq -r '.balance // .near_balance // .amount // ""' <<<"$BODY")
  note "reported: $(head -c 200 <<<"$BODY")"
  if [[ -n "$REPORTED" && "${REPORTED:0:6}" == "${CHAIN_ASSET:0:6}" ]]; then
    pass "E10 the answer is the BOUND account's balance, not the executor's"
  else
    # Not a failure by itself — the field name may differ; the account named is
    # the assertion that matters.
    grep -q "$ASSET" <<<"$BODY" \
      && pass "E10 the answer names the bound account $ASSET" \
      || fail "E10 the answer names neither the bound account nor its chain balance: $(head -c 200 <<<"$BODY")"
  fi
else
  fail "E10 balance on the active binding failed (HTTP $HTTP): $(msg_of)"
fi

api "$SEED_A" GET "/wallet/v1/binding/balance?chain=near" >/dev/null
assert_denied "E10' balance on a PENDING binding" \
  && assert_msg "E10' says WHY, without answering about another account" "pending|not active"

api "$SEED_B" GET "/wallet/v1/binding/balance?chain=near" >/dev/null
assert_denied "E10'' balance on an unbound wallet — not the executor's balance silently"

api "$SEED" GET "/wallet/v1/binding/balance?chain=near&source=intents" >/dev/null
assert_denied "E10''' source=intents on the bound account" \
  && assert_msg "E10''' says where to go instead" "GET /wallet/v1/balance|executor"

api "$SEED" GET "/wallet/v1/binding/balance?chain=ethereum" >/dev/null
assert_denied "E10'''' an unsupported chain"

# ── E11 /binding/transfer ──────────────────────────────────────────────────
log "E11 POST /binding/transfer"
api "$SEED_B" POST /wallet/v1/binding/transfer "$(jq -nc --arg t "$WL" '{to:$t, amount:"1"}')" >/dev/null
assert_denied "E11 transfer from an unbound wallet"
api "$SEED_A" POST /wallet/v1/binding/transfer "$(jq -nc --arg t "$WL" '{to:$t, amount:"1"}')" >/dev/null
assert_denied "E11' transfer on a PENDING binding" && assert_msg "E11' says nothing can be spent yet" "pending|not active"
api "$SEED" POST /wallet/v1/binding/transfer '{"to":"Bad Name!","amount":"1"}' >/dev/null
assert_denied "E11'' an invalid recipient"
api "$SEED" POST /wallet/v1/binding/transfer "$(jq -nc --arg t "$WL" '{to:$t, amount:"0.5"}')" >/dev/null
assert_denied "E11''' an amount that is not in the smallest unit" && assert_msg "E11''' says which unit" "smallest unit|yoctoNEAR|decimal"

# SPENDS: 0.0005 NEAR of the BOUND account's money.
AMT="500000000000000000000"
BEFORE_ASSET=$(account_field "$ASSET" amount)
BEFORE_EXEC=$(account_field "$EXECUTOR" amount)
BEFORE_DEST=$(account_field "$WL" amount)
api "$SEED" POST /wallet/v1/binding/transfer "$(jq -nc --arg t "$WL" --arg a "$AMT" '{to:$t, amount:$a}')" >/dev/null
if [[ "$HTTP" == "200" ]]; then
  pass "E11'''' the builder produced an envelope its own pre-flight accepts (status $(jq -r '.status' <<<"$BODY"))"
  sleep 8
  AFTER_ASSET=$(account_field "$ASSET" amount)
  AFTER_EXEC=$(account_field "$EXECUTOR" amount)
  AFTER_DEST=$(account_field "$WL" amount)
  # The recipient is the only exact number available: the bound account also
  # receives the unspent-gas refund of the receipt it spawned, so its own delta
  # is the amount minus a few tens of microNEAR, and asserting equality there
  # would fail on ordinary chain behaviour rather than on a defect.
  if python3 -c "import sys; sys.exit(0 if int('$AFTER_DEST') - int('$BEFORE_DEST') == int('$AMT') else 1)"; then
    pass "E11'''' the recipient received exactly the amount asked for"
  else
    fail "E11'''' the recipient received $(python3 -c "print(int('$AFTER_DEST')-int('$BEFORE_DEST'))"), not $AMT"
  fi
  if python3 -c "import sys; sys.exit(0 if int('$BEFORE_ASSET') - int('$AFTER_ASSET') >= int('$AMT')*9//10 else 1)"; then
    pass "E11'''' the money left the BOUND account (net $(python3 -c "print(int('$BEFORE_ASSET')-int('$AFTER_ASSET'))") after its gas refund)"
  else
    fail "E11'''' the bound account did not pay: $BEFORE_ASSET -> $AFTER_ASSET"
  fi
  if python3 -c "import sys; sys.exit(0 if int('$BEFORE_EXEC') - int('$AFTER_EXEC') < int('$AMT') else 1)"; then
    pass "E11'''' the executor only paid gas — it is the signer, not the source"
  else
    fail "E11'''' the executor paid the transfer: $BEFORE_EXEC -> $AFTER_EXEC"
  fi
else
  fail "E11'''' the builder's own transfer was refused (HTTP $HTTP): $(msg_of)"
fi

# ── E12 /address ───────────────────────────────────────────────────────────
log "E12 GET /address"
api "$SEED" "GET" "/wallet/v1/address?chain=near" >/dev/null
assert_json "E12 names the bound account" '.asset_account_id' "$ASSET"
assert_json "E12 names the executor" '.executor_account_id' "$EXECUTOR"
jq -e 'has("gas_balance")' <<<"$BODY" >/dev/null \
  && pass "E12 carries gas_balance, so the funder learns it BEFORE a call fails" \
  || fail "E12 no gas_balance in the answer"
api "$SEED_B" GET "/wallet/v1/address?chain=near" >/dev/null
if [[ "$(jq -r '.asset_account_id // ""' <<<"$BODY")" == "" ]]; then
  pass "E12' an unbound wallet has no asset account — the executor IS its address"
else
  fail "E12' an unbound wallet reports an asset account: $(head -c 160 <<<"$BODY")"
fi
api "$SEED" GET "/wallet/v1/address?chain=dogecoin" >/dev/null
assert_denied "E12'' an unknown chain"

# ── E13 regression: the own-wallet endpoints still mean the wallet ─────────
log "E13 the default /balance did NOT move to the bound account"
api "$SEED" GET "/wallet/v1/balance?chain=near" >/dev/null
if [[ "$HTTP" == "200" ]]; then
  if grep -q "$ASSET" <<<"$BODY"; then
    fail "E13 /wallet/v1/balance answers about the BOUND account — the default moved, and every pre-binding client now reads a different account's money"
  else
    pass "E13 /wallet/v1/balance still answers about the wallet's own account"
  fi
else
  fail "E13 /balance failed on a bound wallet (HTTP $HTTP): $(msg_of)"
fi

# ── E14 DoS: how much body will the door buffer ────────────────────────────
#
# Asked on a wallet with no binding, so nothing but the size can refuse it.
log "E14 oversized PUT bodies"
SEED_BODY="hos-ep-body-$(date +%s)-$$"
read -r WID_BODY _ < <(wallet_address "$SEED_BODY")
for MB in 2 8; do
  BIG=$(mktemp -t hos_big.XXXXXX)
  python3 -c "import json,sys; sys.stdout.write(json.dumps({'asset_account_id':'nonexistent-e14-$MB.testnet','kind':'personal_account','pad':'x'*($MB*1000000)}))" > "$BIG"
  throttle
  H=$(curl -sS -o /dev/null -w '%{http_code}' -X PUT "$COORDINATOR_URL/wallet/v1/binding" \
    -H "$(AUTH_FOR "$SEED_BODY")" -H 'Content-Type: application/json' --data-binary "@$BIG" --max-time 120 2>/dev/null)
  rm -f "$BIG"
  note "${MB} MB → HTTP $H"
  if [[ "$MB" == "8" ]]; then
    [[ "$H" == "413" ]] \
      && pass "E14 an 8 MB body is refused as too large (413) rather than buffered" \
      || fail "E14 an 8 MB body answered $H — the door buffers whatever it is sent"
  else
    [[ "$H" =~ ^[24] ]] && note "E14 a 2 MB body is accepted and parsed (HTTP $H)"
  fi
done
api "$SEED_BODY" DELETE /wallet/v1/binding >/dev/null 2>&1 || true

verdict "§2 endpoints"
