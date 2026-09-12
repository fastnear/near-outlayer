#!/usr/bin/env bash
#
# The adversarial rows of the secrets catalogue: what a hostile or careless
# caller can do to the one secret model, against a PUBLISHED test-secrets
# project. `03_project_model.sh` in the example's tests is the happy path and
# the delegation rows; this is everything that should be REFUSED, and how.
#
# Every refusal here is judged on three things at once: the verdict, the
# STATUS (a 4xx, or a finished call — never a 5xx) and the TIME (an answer, not
# a hang). A refusal that hangs is the row-11 defect of the audit (the worker
# never settled the HTTPS call); such a case is reported as a FINDING with the
# call id rather than as a pass.
#
#   N1  a condition nested 200 deep never sits on chain: the contract refuses it
#   N2  a condition nested 60 deep is stored, evaluated within the call's own
#       timeout, and still decides correctly (owner in, stranger out)
#   M1–M8  a hostile secrets_ref over HTTPS — an account that does not exist,
#       an empty one, 300 characters, unicode, a colon; a profile with a slash,
#       whitespace, 10 KB — is answered, never with a 5xx, never with the secret
#   B1–B4  a malformed secrets_ref SHAPE — no profile, a number for the
#       account, extra fields, a megabyte — is refused at the door, never a 5xx
#   R1  a non-owner's update_access changes nothing: the contract refuses and
#       the row is byte-identical
#   R2  the owner's own empty whitelist refuses the owner's own run; the
#       author's row is untouched, so the project still runs for a caller who
#       names nothing
#   Y1  async:true with a secrets_ref sees the same environment as sync, read
#       back through /calls/{id}
#   S1  a wallet the row does not name is refused, with and without
#       use_bound_identity — a binding moves the name a guest acts as, not
#       access
#   P1  header AND body on a connector: the body's row wins [AGENT_SECRET_MODE;
#       RUN_CONNECTOR_BODY=0 skips it on a coordinator too old to honour a body
#       secrets_ref on the connector path]
#   T1  a grant whose ValidUntil has passed refuses, and the message names the
#       instant (the naming needs a worker carrying access_denied_message; the
#       rows skip themselves on a contract or keystore without ValidUntil)
#   T2  the same grant moved to the future admits
#   T3  until_ns "abc" is refused by the contract; "0" is stored, and the
#       owner's own Or-branch still admits the owner
#   T4  the whole cycle on one row: granted until a future instant and admitted,
#       the instant moved into the past and refused, a later instant and admitted
#       again — with the stored instant read back to the nanosecond and the
#       ciphertext never moving
#
# Needs:
#   PARENT               the project owner, key in the keychain; the outlayer
#                        CLI's credentials must be this account
#   $PARENT/test-secrets published from wasi-examples/test-secrets-example
#                        with ./build.sh (the manifest build); the `author`
#                        profile is stored here if it is missing
#   OWNER_PAYMENT_KEY    (M*, B*, Y1) a payment key owned by PARENT:
#                        `outlayer keys create`, then `outlayer keys show <nonce>`
#   AGENT_PAYMENT_KEY / AGENT_ACCOUNT   (S1, P1) a custody wallet's key and
#                        implicit account — a wallet the rows here never name
#   AGENT_WK             (P1) that wallet's wk_, for `secrets set-for-agent`
#   RUN_CONNECTOR_BODY=0 (P1) skip it against an older coordinator; default 1
#   AGENT_SECRET_MODE    run|skip (tests/lib/agent_secret_mode.sh); P1 is the
#                        one case here that uses the agent-secret mode
#
# Money: ~10 on-chain runs at 0.1 NEAR attached (0.001 charged, the rest
# refunded), a handful of HTTPS calls on the keys above, two storage rows, one
# throwaway sub-account at 1 NEAR on the first run only.
#
# Run:
#   PARENT=you.testnet ./tests/secrets_security_e2e.sh            # dry run: the plan
#   PARENT=you.testnet OWNER_PAYMENT_KEY=… ./tests/secrets_security_e2e.sh --apply
#   ONLY=N1,B4 … --apply                                          # a subset

set -uo pipefail
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/lib/hos_common.sh"
source "$SCRIPT_DIR/lib/agent_secret_mode.sh"

PARENT="${PARENT:-}"
PROJECT="${SECRETS_PROJECT:-}"
CONNECTOR_PROJECT="${CONNECTOR_PROJECT:-connectors.outlayer.testnet/connector-probe}"
OWNER_PAYMENT_KEY="${OWNER_PAYMENT_KEY:-}"
AGENT_PAYMENT_KEY="${AGENT_PAYMENT_KEY:-}"
AGENT_ACCOUNT="${AGENT_ACCOUNT:-}"
AGENT_WK="${AGENT_WK:-}"
OUTLAYER_BIN="${OUTLAYER_BIN:-outlayer}"
RUN_CONNECTOR_BODY="${RUN_CONNECTOR_BODY:-1}"
DEPOSIT='0.1 NEAR'

MODE="${1:-}"
if [[ "$MODE" != "--apply" ]]; then
  sed -n '3,67p' "$0" >&2
  echo "  Pass --apply to run." >&2
  exit 0
fi
hos_require
command -v "$OUTLAYER_BIN" >/dev/null || { echo "✗ the outlayer CLI is not on PATH (OUTLAYER_BIN)" >&2; exit 1; }
PROJECT="${PROJECT:-$PARENT/test-secrets}"
source "$SCRIPT_DIR/lib/secrets_common.sh"

STRANGER="xpat.$PARENT"
ROW=sec

# `ONLY=N1,B4` runs a subset (the fixture always runs; every row restores what
# it changed, so any subset leaves the rows as it found them).
want() { [[ -z "${ONLY:-}" ]] || [[ ",$ONLY," == *",$1,"* ]]; }

# `update_access` signed by anyone, reported rather than fatal: the refusals
# here are the point. Waits for finality when the transaction lands.
TRY_OUT=""
try_update_access() { # try_update_access <signer> <project> <profile> <access-json>
  local signer=$1 project=$2 profile=$3 access=$4 flag=with-legacy-keychain before
  [[ "$signer" == "$PARENT" ]] && flag=with-keychain
  before=$(jq -r '.updated_at // 0' <<<"$(row_of "$project" "$profile")")
  # The arguments are CONCATENATED and sent as base64: a tree nested past what
  # jq or near-cli will parse (N1 is one) must still reach the contract, whose
  # own parser is the one under test. Profiles here are plain words.
  local args
  args=$(printf '{"accessor":%s,"profile":"%s","new_access":%s}' "$(accessor_json "$project")" "$profile" "$access" | base64 | tr -d '\n')
  TRY_OUT=$(near --quiet contract call-function as-transaction "$CONTRACT_ID" update_access \
    base64-args "$args" \
    prepaid-gas '30.0 Tgas' attached-deposit '0 NEAR' \
    sign-as "$signer" network-config "$NETWORK" "sign-$flag" send 2>&1) || return 1
  [[ "$signer" == "$PARENT" ]] && wait_row_after "$project" "$profile" "$before"
  return 0
}

# What a hang MEANS, per section. A reference the contract could never match is
# the coordinator door's to refuse (`well_formed_secrets_ref` → 400
# `invalid_secrets_ref`); a reference it could match, refused by the row's own
# condition, is the worker's to settle (`report_refusal` → `complete_https_call`,
# audit row 11). Both live in the working tree: against a coordinator or a worker
# without them, such a call waits for the timeout sweeper instead of answering.
HANG_FIX="the worker must settle the refused call (audit row 11: report_refusal \
answers complete_https_call) — deploy the worker"

# Whose key may read the call a row made. `/calls/{id}` is authenticated: it
# admits the payment key that owns the call, or the wallet's own credential.
# Polling it without one answers an error rather than a status, which would read
# as a call nobody settled.
POLL_KEY=""

# A refusal's status and time, judged together with its verdict. `answered
# <label>` passes when the door answered at all — 4xx, or a finished call — and
# fails on a 5xx or on silence. A call that came back `pending` is followed for
# a minute; one still pending then is the row-11 hang, reported as a finding.
answered() {
  local label=$1 cid status i
  case "$HTTP_CODE" in
    000) fail "$label: nothing answered within 90 s — $HANG_FIX"; return ;;
    5*)  fail "$label: HTTP $HTTP_CODE — $(head -c 200 <<<"$ANS")"; return ;;
  esac
  status=$(jq -r '.status // ""' <<<"$ANS" 2>/dev/null)
  cid=$(jq -r '.call_id // ""' <<<"$ANS" 2>/dev/null)
  if [[ "$HTTP_CODE" == 2* && -n "$cid" && "$status" != "completed" && "$status" != "failed" ]]; then
    local polled poll_http
    for i in $(seq 1 20); do
      sleep 3
      polled=$(curl -sS --max-time 20 -w '\nHTTP:%{http_code}' "$COORDINATOR_URL/calls/$cid" \
        ${POLL_KEY:+-H "X-Payment-Key: $POLL_KEY"} 2>/dev/null)
      poll_http=${polled##*HTTP:}
      if [[ "$poll_http" != 2* ]]; then
        skip "$label: the call's status could not be read (HTTP $poll_http) — this row measures settlement, and that needs the key owning the call"
        return
      fi
      status=$(jq -r '.status // ""' <<<"${polled%$'\n'HTTP:*}" 2>/dev/null)
      [[ "$status" == "completed" || "$status" == "failed" ]] && break
    done
    if [[ "$status" != "completed" && "$status" != "failed" ]]; then
      finding "$label: call $cid is still '$status' a minute later — a refused run nobody settled (audit row 11; the worker redeploy)"
      return
    fi
  fi
  pass "$label: answered HTTP $HTTP_CODE${status:+, status $status}${RUN_ERR:+ — $(head -c 120 <<<"$RUN_ERR")}"
}

deep() { # deep <n> — the owner's whitelist under n NOTs (even n: the same verdict)
  jq -nc --arg p "$PARENT" --argjson n "$1" \
    'reduce range($n) as $i ({Whitelist:{accounts:[$p]}}; {Not:{condition:.}})'
}

# ── fixture ──────────────────────────────────────────────────────────────────
log "Fixture: the project, the stranger, the rows"
PROJECT_VIEW=$(near_view "$CONTRACT_ID" get_project "$(jq -nc --arg p "$PROJECT" '{project_id:$p}')")
if [[ -z "$PROJECT_VIEW" || "$PROJECT_VIEW" == "null" || "$PROJECT_VIEW" == "ERR" ]]; then
  skip "$PROJECT is not deployed — ./build.sh, outlayer upload, outlayer deploy test-secrets (see the example's README)"
  verdict "secrets security"; exit $?
fi
note "project: $PROJECT"
make_account "$STRANGER" "$PARENT" '1 NEAR'

if [[ -z "$(jq -r '.encrypted_secrets // empty' <<<"$(row_of "$PROJECT" author)")" ]]; then
  store "$PROJECT" author "$(jq -nc --arg v "author-$(openssl rand -hex 6)" '{AUTHOR_SECRET:$v}')" allow-all
else
  note "the author profile is stored; left as it is"
fi
USER_CANARY="user-$(openssl rand -hex 6)"
store "$PROJECT" "$ROW" "$(jq -nc --arg v "$USER_CANARY" '{USER_SECRET:$v}')" "whitelist:$PARENT"
restore_row() { set_access "$PROJECT" "$ROW" "$(whitelist "$PARENT")"; }
trap 'restore_row >/dev/null 2>&1 || true' EXIT

# ── N nesting ────────────────────────────────────────────────────────────────
if want N1; then
log "N1 a condition nested 200 deep"
BEFORE=$(row_of "$PROJECT" "$ROW")
if try_update_access "$PARENT" "$PROJECT" "$ROW" "$(deep 200)"; then
  finding "N1 the contract STORED a 200-deep condition: $(head -c 100 <<<"$TRY_OUT")"
  run_as "$PARENT" "$PARENT/$ROW"
  [[ "$RUN_OK" != "absent" ]] \
    && pass "N1 and the keystore still answered (success=$RUN_OK): no hang" \
    || fail "N1 the run never completed"
  restore_row
else
  pass "N1 the contract refused to store it: $(grep -o 'Smart contract panicked[^"]\{0,90\}\|[Rr]ecursion[^"]\{0,60\}\|Error:[^"]\{0,80\}' <<<"$TRY_OUT" | head -1)"
  [[ "$(row_of "$PROJECT" "$ROW")" == "$BEFORE" ]] \
    && pass "N1 and the row is byte-identical" \
    || fail "N1 the row changed under a refused update"
fi

fi

if want N2; then
log "N2 a condition nested 60 deep still decides, in time"
set_access "$PROJECT" "$ROW" "$(deep 60)"
run_as "$PARENT" "$PARENT/$ROW"
[[ "$RUN_OK" == "true" && "$(field .user)" == "true" && "$(secret_value USER_SECRET)" == "$USER_CANARY" ]] \
  && pass "N2 the owner is admitted through 60 NOTs and reads the canary" \
  || fail "N2 owner: success=$RUN_OK user=$(field .user) err='$RUN_ERR'"
run_as "$STRANGER" "$PARENT/$ROW"
[[ "$RUN_OK" == "false" ]] \
  && pass "N2 the stranger is refused through the same tree: $(head -c 100 <<<"$RUN_ERR")" \
  || fail "N2 stranger: success=$RUN_OK user=$(field .user)"
restore_row

fi

# ── M/B hostile references over HTTPS ────────────────────────────────────────
if ! want M && ! want B && ! want Y1; then
  :
elif [[ -z "$OWNER_PAYMENT_KEY" ]]; then
  skip "M1–M8, B1–B4, Y1 need OWNER_PAYMENT_KEY (a payment key owned by $PARENT)"
else
  # M and B name rows the contract could never hold. EITHER deploy clears them:
  # the coordinator's door check stops such a reference before a job exists, and
  # the worker's refusal path settles the call if one is already running.
  HANG_FIX="either deploy clears this: the coordinator refuses the reference at the door \
(invalid_secrets_ref, 400), or the worker settles the refused call (audit row 11)"
  POLL_KEY="$OWNER_PAYMENT_KEY"
  if want M; then
  log "M a hostile secrets_ref is answered, never with a 5xx, never with the secret"
  m_row() { # m_row <id> <account_id> <profile> <what>
    https_post "$OWNER_PAYMENT_KEY" "$PROJECT" \
      "$(jq -nc --arg a "$2" --arg pr "$3" '{input:{message:"probe"}, secrets_ref:{account_id:$a, profile:$pr}}')"
    answered "$1 $4"
    [[ -z "$(secret_value USER_SECRET)" ]] \
      && pass "$1 and no secret reached the guest" \
      || fail "$1 USER_SECRET reached the guest through '$2'/'$3'"
  }
  m_row M1 "nobody-$(openssl rand -hex 4).testnet" "$ROW" "an account that does not exist"
  m_row M2 "" "$ROW" "an empty account"
  m_row M3 "$(head -c 300 /dev/zero | tr '\0' a).testnet" "$ROW" "a 300-character account"
  m_row M4 "ünïcödé.testnet" "$ROW" "a unicode account"
  m_row M5 "$PARENT:$ROW" "$ROW" "an account with a colon"
  m_row M6 "$PARENT" "$ROW/../author" "a profile with slashes"
  m_row M7 "$PARENT" "   " "a whitespace profile"
  m_row M8 "$PARENT" "$(head -c 10240 /dev/zero | tr '\0' p)" "a 10 KB profile"

  fi
  if want B; then
  log "B a malformed secrets_ref shape is refused at the door"
  b_row() { # b_row <id> <raw-body> <what>
    https_post "$OWNER_PAYMENT_KEY" "$PROJECT" "$2"
    case "$HTTP_CODE" in
      4*) pass "$1 $3 → HTTP $HTTP_CODE: $(head -c 100 <<<"$RUN_ERR")" ;;
      5*|000) fail "$1 $3 → HTTP $HTTP_CODE — $HANG_FIX" ;;
      *) note "$1 $3 was ACCEPTED (HTTP $HTTP_CODE) — the field was ignored rather than refused"; answered "$1 $3" ;;
    esac
  }
  b_row B1 '{"input":{"message":"probe"},"secrets_ref":{"account_id":"nobody.testnet"}}' "no profile"
  b_row B2 '{"input":{"message":"probe"},"secrets_ref":{"account_id":42,"profile":"p"}}' "a number for the account"
  b_row B3 "$(jq -nc --arg p "$PARENT" '{input:{message:"probe"}, secrets_ref:{account_id:$p, profile:"nope", extra:1}}')" "an extra field"
  # A megabyte does not fit in an argument (ARG_MAX), so the body goes through
  # a file — `@path`, which curl reads and https_post passes on as it is.
  BIG=$(mktemp -t secsec_big.XXXXXX)
  { printf '{"input":{"message":"probe"},"secrets_ref":{"account_id":"%s","profile":"' "$PARENT"; head -c 1048576 /dev/zero | tr '\0' q; printf '"}}'; } > "$BIG"
  b_row B4 "@$BIG" "a one-megabyte profile (a size the contract's 64 characters cannot hold)"
  rm -f "$BIG"

  fi
  if want Y1; then
  log "Y1 async: the same environment, read back through /calls/{id}"
  https_post "$OWNER_PAYMENT_KEY" "$PROJECT" \
    "$(jq -nc --arg a "$PARENT" --arg pr "$ROW" '{input:{message:"probe"}, secrets_ref:{account_id:$a, profile:$pr}, async:true}')"
  CID=$(jq -r '.call_id // ""' <<<"$ANS" 2>/dev/null)
  if [[ "$HTTP_CODE" != 2* || -z "$CID" ]]; then
    fail "Y1 the async call was not accepted (HTTP $HTTP_CODE): $(head -c 160 <<<"$ANS")"
  else
    STATUS=""; POLLED=""
    for i in $(seq 1 30); do
      sleep 3
      POLLED=$(curl -sS --max-time 20 "$COORDINATOR_URL/calls/$CID" -H "X-Payment-Key: $OWNER_PAYMENT_KEY" 2>/dev/null)
      STATUS=$(jq -r '.status // ""' <<<"$POLLED" 2>/dev/null)
      [[ "$STATUS" == "completed" || "$STATUS" == "failed" ]] && break
    done
    RUN_OUT=$(jq -c '.output | if type=="string" then fromjson else . end' <<<"$POLLED" 2>/dev/null)
    [[ "$STATUS" == "completed" && "$(field .user)" == "true" && "$(secret_value USER_SECRET)" == "$USER_CANARY" ]] \
      && pass "Y1 completed through the poll with the owner's canary in the environment" \
      || fail "Y1 status=$STATUS user=$(field .user) err='$(jq -r '.error // ""' <<<"$POLLED" 2>/dev/null)'"
  fi
  fi
fi

# ── R update_access ──────────────────────────────────────────────────────────
if want R1; then
log "R1 a non-owner's update_access"
BEFORE=$(row_of "$PROJECT" "$ROW")
if try_update_access "$STRANGER" "$PROJECT" "$ROW" "$(whitelist "$STRANGER")"; then
  fail "R1 $STRANGER's update_access on $PARENT's row was ACCEPTED"
else
  pass "R1 refused by the contract: $(grep -o 'Secrets not found\|Smart contract panicked[^"]\{0,60\}' <<<"$TRY_OUT" | head -1)"
fi
sleep 3
[[ "$(row_of "$PROJECT" "$ROW")" == "$BEFORE" ]] \
  && pass "R1 the row is byte-identical" \
  || fail "R1 the row changed under a stranger's update_access"

fi

if want R2; then
log "R2 the owner's empty whitelist"
set_access "$PROJECT" "$ROW" '{"Whitelist":{"accounts":[]}}'
run_as "$PARENT" "$PARENT/$ROW"
[[ "$RUN_OK" == "false" ]] && grep -qi "denied" <<<"$RUN_ERR" \
  && pass "R2 the owner's own run is refused: $(head -c 100 <<<"$RUN_ERR")" \
  || fail "R2 owner: success=$RUN_OK user=$(field .user) err='$RUN_ERR'"
run_as "$PARENT"
[[ "$RUN_OK" == "true" && "$(field .author)" == "true" ]] \
  && pass "R2 and a run naming nothing still gets the author's row — one row's condition touches one row" \
  || fail "R2 the author's row stopped admitting: success=$RUN_OK author=$(field .author) err='$RUN_ERR'"
restore_row

fi

# ── S a wallet the row does not name ─────────────────────────────────────────
if ! want S1; then
  :
elif [[ -z "$AGENT_PAYMENT_KEY" ]]; then
  skip "S1 needs AGENT_PAYMENT_KEY (a custody wallet's key that the row does not name)"
else
  HANG_FIX="the worker must settle the refused call (audit row 11: report_refusal \
answers complete_https_call) — deploy the worker"
  POLL_KEY="$AGENT_PAYMENT_KEY"
  log "S1 a wallet the row does not name, with and without use_bound_identity"
  https_post "$AGENT_PAYMENT_KEY" "$PROJECT" \
    "$(jq -nc --arg a "$PARENT" --arg pr "$ROW" '{input:{message:"probe"}, secrets_ref:{account_id:$a, profile:$pr}}')"
  if [[ "$RUN_OK" == "true" && "$(field .user)" == "true" ]]; then
    fail "S1 an unnamed wallet read the owner's row"
  else
    answered "S1 unnamed wallet"
  fi
  https_post "$AGENT_PAYMENT_KEY" "$PROJECT" \
    "$(jq -nc --arg a "$PARENT" --arg pr "$ROW" '{input:{message:"probe"}, secrets_ref:{account_id:$a, profile:$pr}, use_bound_identity:true}')"
  if [[ "$RUN_OK" == "true" && "$(field .user)" == "true" ]]; then
    fail "S1 use_bound_identity admitted a wallet the row does not name"
  else
    answered "S1 with use_bound_identity"
  fi
fi

# ── P header and body together on a connector ────────────────────────────────
if ! want P1 || ! agent_secret_mode P1; then
  :
elif [[ "$RUN_CONNECTOR_BODY" != "1" ]]; then
  skip "P1 (RUN_CONNECTOR_BODY=0): needs the coordinator that honours a body secrets_ref on the connector path"
elif [[ -z "$AGENT_WK" || -z "$AGENT_PAYMENT_KEY" || -z "$AGENT_ACCOUNT" ]]; then
  skip "P1 needs AGENT_WK, AGENT_PAYMENT_KEY and AGENT_ACCOUNT"
elif ! "$OUTLAYER_BIN" secrets set-for-agent --help >/dev/null 2>&1; then
  skip "P1 '$OUTLAYER_BIN secrets set-for-agent' is not available"
else
  log "P1 header AND body on $CONNECTOR_PROJECT: the body's row wins"
  HDR_TOKEN="from-header-$(openssl rand -hex 4)"
  BODY_TOKEN="from-body-$(openssl rand -hex 4)"
  if ! OUTLAYER_WALLET_KEY="$AGENT_WK" OUTLAYER_NETWORK="$NETWORK" "$OUTLAYER_BIN" secrets set-for-agent \
       "$(jq -nc --arg t "$HDR_TOKEN" '{PROBE_TOKEN:$t}')" --project "$CONNECTOR_PROJECT" >/dev/null 2>&1; then
    fail "P1 could not store the agent's own row with set-for-agent"
  else
    store "$CONNECTOR_PROJECT" both "$(jq -nc --arg t "$BODY_TOKEN" '{PROBE_TOKEN:$t}')" "whitelist:$PARENT,$AGENT_ACCOUNT"
    call_https "$AGENT_PAYMENT_KEY" "$CONNECTOR_PROJECT" "$PARENT/both" '{"operation":"secret"}' -H 'X-Use-Owner-Secret: 1'
    GOT=$(jq -r '.secrets[]? | select(.key=="PROBE_TOKEN") | .sha256_prefix // empty' <<<"$RUN_OUT" 2>/dev/null)
    WANT=$(printf '%s' "$BODY_TOKEN" | shasum -a 256 | cut -c1-8)
    OTHER=$(printf '%s' "$HDR_TOKEN" | shasum -a 256 | cut -c1-8)
    if [[ "$RUN_OK" == "true" && "$GOT" == "$WANT" ]]; then
      pass "P1 the guest saw the BODY's token, not the header's"
    elif [[ "$GOT" == "$OTHER" ]]; then
      fail "P1 the header's row overrode the body's — the coordinator dropped what the body named"
    else
      fail "P1 success=$RUN_OK token=$GOT err='$RUN_ERR'"
    fi
    set_access "$CONNECTOR_PROJECT" both "$(whitelist "$PARENT")"
  fi
fi

# ── T time limits ────────────────────────────────────────────────────────────
NOW_NS=$(( $(date +%s) * 1000000000 ))
grant_until() { # grant_until <until_ns> — the owner always; the stranger until the instant
  jq -nc --arg p "$PARENT" --arg s "$STRANGER" --arg u "$1" \
    '{Logic:{operator:"Or",conditions:[{Whitelist:{accounts:[$p]}},
      {Logic:{operator:"And",conditions:[{Whitelist:{accounts:[$s]}},{ValidUntil:{until_ns:$u}}]}}]}}'
}
if want T; then
log "T1 a grant whose time limit has passed"
if ! try_update_access "$PARENT" "$PROJECT" "$ROW" "$(grant_until 1)"; then
  skip "T1–T3: the contract refuses ValidUntil ($(grep -o 'unknown variant[^"]\{0,60\}\|Smart contract panicked[^"]\{0,60\}' <<<"$TRY_OUT" | head -1)) — deploy the contract that carries it"
else
  run_as "$STRANGER" "$PARENT/$ROW"
  if [[ "$RUN_OK" == "false" ]] && grep -qi "unknown variant\|ValidUntil\|parse" <<<"$RUN_ERR"; then
    skip "T1–T3: the keystore does not know ValidUntil yet ($(head -c 100 <<<"$RUN_ERR")) — deploy the keystore that carries it"
    restore_row
    verdict "secrets security"; exit $?
  elif [[ "$RUN_OK" == "false" ]] && grep -qi "denied" <<<"$RUN_ERR"; then
    # The VERDICT and the REASON are two claims, and only the first is the
    # product's behaviour. A lapsed grant must refuse; naming the instant it
    # lapsed at is the worker passing the keystore's own sentence through
    # (`access_denied_message`), which a worker built before that fix replaces
    # with a fixed string.
    pass "T1 the lapsed grant refuses the stranger: $(head -c 100 <<<"$RUN_ERR")"
    if grep -q "time limit passed at 1970-01-01T00:00:00Z" <<<"$RUN_ERR"; then
      pass "T1 and the message names the instant it lapsed at"
    else
      finding "T1 the refusal does not name the instant — this worker replaces the keystore's sentence with a fixed string; needs the access_denied_message fix deployed"
    fi
  else
    fail "T1 stranger: success=$RUN_OK user=$(field .user) err='$RUN_ERR' (expected a refusal)"
  fi
  run_as "$PARENT" "$PARENT/$ROW"
  [[ "$RUN_OK" == "true" && "$(field .user)" == "true" ]] \
    && pass "T1 the owner's own branch has no limit and still admits" \
    || fail "T1 owner: success=$RUN_OK user=$(field .user) err='$RUN_ERR'"

  log "T2 the same grant, one hour into the future"
  set_access "$PROJECT" "$ROW" "$(grant_until $(( NOW_NS + 3600 * 1000000000 )))"
  run_as "$STRANGER" "$PARENT/$ROW"
  [[ "$RUN_OK" == "true" && "$(field .user)" == "true" && "$(secret_value USER_SECRET)" == "$USER_CANARY" ]] \
    && pass "T2 admitted before the instant, and reads the canary" \
    || fail "T2 stranger: success=$RUN_OK user=$(field .user) err='$RUN_ERR'"

  log "T3 raw until_ns values"
  if try_update_access "$PARENT" "$PROJECT" "$ROW" "$(grant_until abc)"; then
    fail "T3 the contract stored until_ns \"abc\""
  else
    pass "T3 until_ns \"abc\" is refused by the contract"
  fi
  set_access "$PROJECT" "$ROW" "$(grant_until 0)"
  run_as "$PARENT" "$PARENT/$ROW"
  [[ "$RUN_OK" == "true" && "$(field .user)" == "true" ]] \
    && pass "T3 until_ns \"0\" is stored and the owner's branch admits the owner" \
    || fail "T3 owner under until_ns 0: success=$RUN_OK user=$(field .user) err='$RUN_ERR'"
  run_as "$STRANGER" "$PARENT/$ROW"
  [[ "$RUN_OK" == "false" ]] \
    && pass "T3 and the stranger's lapsed branch refuses" \
    || fail "T3 stranger under until_ns 0 was admitted"

  # ── T4 the whole cycle, on one row, with the value never re-stored ──────────
  #
  # What an owner actually does: grant until a date, watch it lapse, grant
  # again. Nobody waits an hour for the lapse — moving the instant into the past
  # is the same thing to the keystore, and it is the same `update_access` the
  # owner would use to shorten a grant.
  log "T4 granted, lapsed, granted again"
  BLOB_T4=$(jq -r '.encrypted_secrets' <<<"$(row_of "$PROJECT" "$ROW")")
  FUTURE=$(( NOW_NS + 3600 * 1000000000 ))
  set_access "$PROJECT" "$ROW" "$(grant_until "$FUTURE")"
  # Stored is stored: the instant must come back as it went in, to the
  # nanosecond. A `U64` that lost precision or a string that became a number
  # would still look like a date here and admit at the wrong moment.
  STORED_UNTIL=$(jq -r '.. | objects | select(has("ValidUntil")) | .ValidUntil.until_ns' \
    <<<"$(row_of "$PROJECT" "$ROW")" 2>/dev/null | head -1)
  [[ "$STORED_UNTIL" == "$FUTURE" ]] \
    && pass "T4 the chain stored the exact instant it was given ($FUTURE)" \
    || fail "T4 the chain stored until_ns '$STORED_UNTIL', expected '$FUTURE'"
  run_as "$STRANGER" "$PARENT/$ROW"
  [[ "$RUN_OK" == "true" && "$(field .user)" == "true" ]] \
    && pass "T4 granted until an hour from now: the stranger reads it" \
    || fail "T4 granted: success=$RUN_OK user=$(field .user) err='$RUN_ERR'"

  set_access "$PROJECT" "$ROW" "$(grant_until 1)"
  run_as "$STRANGER" "$PARENT/$ROW"
  [[ "$RUN_OK" == "false" ]] && grep -qi "denied" <<<"$RUN_ERR" \
    && pass "T4 the instant moved into the past: the same caller is refused" \
    || fail "T4 lapsed: success=$RUN_OK user=$(field .user) err='$RUN_ERR'"

  set_access "$PROJECT" "$ROW" "$(grant_until "$FUTURE")"
  run_as "$STRANGER" "$PARENT/$ROW"
  [[ "$RUN_OK" == "true" && "$(field .user)" == "true" && "$(secret_value USER_SECRET)" == "$USER_CANARY" ]] \
    && pass "T4 a later instant brings it back, and the canary is the same secret" \
    || fail "T4 re-dated: success=$RUN_OK user=$(field .user) err='$RUN_ERR'"
  [[ -n "$BLOB_T4" && "$(jq -r '.encrypted_secrets' <<<"$(row_of "$PROJECT" "$ROW")")" == "$BLOB_T4" ]] \
    && pass "T4 and the ciphertext never moved: only the date was ever edited" \
    || fail "T4 the ciphertext changed while only the date was edited"
  restore_row
fi
fi

verdict "secrets security"
