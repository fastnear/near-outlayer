#!/usr/bin/env bash
# Shared by the secrets suites — `wasi-examples/test-secrets-example/tests/03_project_model.sh`
# and `tests/secrets_security_e2e.sh`: fixture accounts, the stored row as the
# chain reports it, `store` / `set_access` / `delete_row` that wait for finality,
# and one run on chain or over HTTPS judged the same way.
#
# Source AFTER hos_common.sh and after these are set:
#   PARENT        the row owner and on-chain signer (key in the keychain; the
#                 outlayer CLI's credentials must be this account)
#   PROJECT       the project every `run_as` executes
#   DEPOSIT       what `run_as` attaches (e.g. '0.1 NEAR')
# and, from hos_common.sh: CONTRACT_ID NETWORK RPC_URL COORDINATOR_URL.
#
# Every run sets RUN_OK (true / false / absent), RUN_ERR (the reason) and
# RUN_OUT (the module's own JSON answer).

# ── fixture helpers ──────────────────────────────────────────────────────────

account_is_on_chain() {
  local out
  out=$(curl -sS "$RPC_URL" -H 'content-type: application/json' -d "$(jq -nc --arg a "$1" \
        '{jsonrpc:"2.0",id:1,method:"query",
          params:{request_type:"view_account",finality:"final",account_id:$a}}')" 2>/dev/null)
  [[ -n "$(jq -r '.result.amount // empty' <<<"$out")" ]]
}

make_account() { # make_account <name> <parent> <amount>
  if account_is_on_chain "$1"; then note "$1 is already there"; return 0; fi
  local signer=with-keychain
  [[ "$2" != "$PARENT" ]] && signer=with-legacy-keychain
  near --quiet account create-account fund-myself "$1" "$3" \
    autogenerate-new-keypair save-to-legacy-keychain \
    sign-as "$2" network-config "$NETWORK" "sign-$signer" send >/dev/null 2>&1
  account_is_on_chain "$1" || { echo "✗ could not create $1" >&2; exit 1; }
  note "created $1 with $3"
}

accessor_json() { jq -nc --arg p "$1" '{Project:{project_id:$p}}'; }

# The stored row, as the chain reports it (ciphertext + condition), or empty.
row_of() { # row_of <project> <profile>
  near_view "$CONTRACT_ID" get_secrets "$(jq -nc --argjson a "$(accessor_json "$1")" --arg pr "$2" --arg o "$PARENT" \
    '{accessor:$a, profile:$pr, owner:$o}')"
}

# Wait until the row's `updated_at` moves past a value: `outlayer secrets set`
# and `near call` return once EXECUTED, `near_view` reads FINAL.
wait_row_after() { # wait_row_after <project> <profile> <previous updated_at>
  local after=$3 i
  for i in $(seq 1 15); do
    after=$(jq -r '.updated_at // 0' <<<"$(row_of "$1" "$2")")
    [[ "$after" != "$3" ]] && return 0
    sleep 2
  done
  return 1
}

# The outlayer CLI: OUTLAYER_BIN when a suite names one (a freshly built
# binary), else whatever is on PATH.
OUTLAYER_BIN="${OUTLAYER_BIN:-outlayer}"

store() { # store <project> <profile> <secrets-json> <access>   (the CLI signs as PARENT)
  local before out
  before=$(jq -r '.updated_at // 0' <<<"$(row_of "$1" "$2")")
  out=$(OUTLAYER_NETWORK="$NETWORK" "$OUTLAYER_BIN" secrets set "$3" --project "$1" --profile "$2" --access "$4" 2>&1) \
    || { echo "✗ could not store $1/$2: $(tail -1 <<<"$out" | head -c 200)" >&2; exit 1; }
  wait_row_after "$1" "$2" "$before" || { echo "✗ $1/$2 never became final" >&2; exit 1; }
  note "stored $1/$2 ($4)"
}

set_access() { # set_access <project> <profile> <access-json>
  local before
  before=$(jq -r '.updated_at // 0' <<<"$(row_of "$1" "$2")")
  near --quiet contract call-function as-transaction "$CONTRACT_ID" update_access \
    json-args "$(jq -nc --argjson a "$(accessor_json "$1")" --arg pr "$2" --argjson x "$3" \
      '{accessor:$a, profile:$pr, new_access:$x}')" \
    prepaid-gas '30.0 Tgas' attached-deposit '0 NEAR' \
    sign-as "$PARENT" network-config "$NETWORK" sign-with-keychain send >/dev/null 2>&1 \
    || { echo "✗ update_access failed for $1/$2" >&2; exit 1; }
  wait_row_after "$1" "$2" "$before" || { echo "✗ $1/$2 access change never became final" >&2; exit 1; }
  note "$1/$2 access → $(jq -c 'if type=="string" then . else keys[0] end' <<<"$3")"
}

delete_row() { # delete_row <project> <profile>
  near --quiet contract call-function as-transaction "$CONTRACT_ID" delete_secrets \
    json-args "$(jq -nc --argjson a "$(accessor_json "$1")" --arg pr "$2" '{accessor:$a, profile:$pr}')" \
    prepaid-gas '30.0 Tgas' attached-deposit '0 NEAR' \
    sign-as "$PARENT" network-config "$NETWORK" sign-with-keychain send >/dev/null 2>&1 \
    || { echo "✗ delete_secrets failed for $1/$2" >&2; exit 1; }
  local i
  for i in $(seq 1 15); do
    [[ -z "$(jq -r '.encrypted_secrets // empty' <<<"$(row_of "$1" "$2")")" ]] && { note "deleted $1/$2"; return 0; }
    sleep 2
  done
  echo "✗ $1/$2 still on chain after delete" >&2; exit 1
}

whitelist() { jq -nc '$ARGS.positional' --args "$@" | jq -c '{Whitelist:{accounts:.}}'; }

# ── one run, on chain ────────────────────────────────────────────────────────
#
# Sets RUN_OK / RUN_ERR from the completion event and RUN_OUT from the module's
# own answer. `run_as <signer> [owner/profile] [input-json]`; an empty second
# argument names no secret at all.
RUN_OK=""; RUN_ERR=""; RUN_OUT=""
run_as() {
  local signer=$1 ref=${2:-} input=${3:-'{"message":"probe"}'} out ev args signer_flag=with-legacy-keychain
  [[ "$signer" == "$PARENT" ]] && signer_flag=with-keychain
  args=$(jq -nc --arg p "$PROJECT" --arg i "$input" \
    '{source:{Project:{project_id:$p}}, input_data:$i,
      resource_limits:{max_instructions:1000000000,max_memory_mb:128,max_execution_seconds:30}}')
  if [[ -n "$ref" ]]; then
    args=$(jq -c --arg o "${ref%%/*}" --arg pr "${ref#*/}" '. + {secrets_ref:{profile:$pr, account_id:$o}}' <<<"$args")
  fi
  out=$(near contract call-function as-transaction "$CONTRACT_ID" request_execution \
    json-args "$args" prepaid-gas '300.0 Tgas' attached-deposit "$DEPOSIT" \
    sign-as "$signer" network-config "$NETWORK" "sign-$signer_flag" send 2>&1)
  ev=$(grep -o 'EVENT_JSON:.*execution_completed.*' <<<"$out" | sed 's/^EVENT_JSON://' | head -1)
  if [[ -z "$ev" ]]; then
    # No completion event at all: the transaction did not land, or the CLI
    # failed before sending. Neither is a verdict about the product.
    RUN_OK=absent; RUN_ERR=""; RUN_OUT=""
    note "no completion event from $signer: $(grep -iE 'error|fail|panick|reset|limit' <<<"$out" | head -2 | head -c 300)"
    return 0
  fi
  RUN_OK=$(jq -r '.data[0] | if has("success") then (.success|tostring) else "absent" end' <<<"$ev" 2>/dev/null)
  RUN_ERR=$(jq -r '.data[0].error_message // ""' <<<"$ev" 2>/dev/null)
  RUN_OUT=$(awk '/Function execution return value/{getline; print}' <<<"$out" \
    | jq -c 'select(. != null) | if type=="string" then fromjson else . end' 2>/dev/null)
}

# ── one run, over HTTPS ──────────────────────────────────────────────────────
#
# `call_https <payment-key> <project> [owner/profile] [input-json] [extra curl args...]`
call_https() {
  local key=$1 project=$2 ref=${3:-} input=${4:-'{"message":"probe"}'}; shift 4 2>/dev/null || shift $#
  local body
  body=$(jq -nc --argjson i "$input" '{input:$i}')
  if [[ -n "$ref" ]]; then
    body=$(jq -c --arg o "${ref%%/*}" --arg pr "${ref#*/}" '. + {secrets_ref:{profile:$pr, account_id:$o}}' <<<"$body")
  fi
  https_post "$key" "$project" "$body" "$@"
}

# `https_post <payment-key> <project> <body> [extra curl args...]` — the body
# sent verbatim (it may be malformed on purpose). Also leaves HTTP_CODE (`000`
# when nothing answered within 90 s) and ANS (the raw body) for a caller that
# judges the door rather than the run.
HTTP_CODE=""; ANS=""
https_post() {
  local key=$1 project=$2 body=$3; shift 3
  throttle
  local raw
  raw=$(curl -sS --max-time 90 -w '\nHTTP:%{http_code}' -X POST "$COORDINATOR_URL/call/$project" \
    -H "X-Payment-Key: $key" -H 'Content-Type: application/json' "$@" --data-binary "$body" 2>&1)
  HTTP_CODE=${raw##*HTTP:}; ANS=${raw%$'\n'HTTP:*}
  # The envelope is `{call_id, status, output, …}` on a run and `{error, …}` on a
  # refusal — there is no `success` field. `status: completed` is the run that
  # ran; anything else, or a non-2xx, is a refusal whose reason is in the body.
  RUN_OUT=$(jq -c '.output | if type=="string" then fromjson else . end' <<<"$ANS" 2>/dev/null)
  RUN_ERR=$(jq -r '.error // .message // .status // ""' <<<"$ANS" 2>/dev/null)
  if [[ "$HTTP_CODE" == 2* ]] && [[ "$(jq -r '.status // ""' <<<"$ANS" 2>/dev/null)" == "completed" ]]; then
    RUN_OK=true
  elif jq -e . <<<"$ANS" >/dev/null 2>&1; then
    RUN_OK=false
  else
    RUN_OK=absent
    note "no JSON answer (HTTP $HTTP_CODE): $(head -c 200 <<<"$ANS")"
  fi
}

field() { jq -r "$1 | if . == null then \"\" else tostring end" <<<"$RUN_OUT" 2>/dev/null; }
secret_value() { jq -r --arg k "$1" '.secrets[]? | select(.key==$k) | .value // empty' <<<"$RUN_OUT" 2>/dev/null; }

