#!/usr/bin/env bash
# Force the keystore vault/session counters in Grafana to update NOW.
#
# The coordinator reads `/admin/loaded-vaults` from the keystore at most once an hour: the numbers
# move when a vault is first touched or a worker re-handshakes — events, not a continuous signal —
# so a faster poll would only rewrite the same pair, which is the noise we just removed from the
# certificate panel. This is the escape hatch for "I just did something and want to see it":
#
#   register a vault → ./scripts/refresh_keystore_stats.sh testnet → Grafana within ~30s
#
# Chain of custody for the number:
#   keystore /admin/loaded-vaults  →  coordinator /health/detailed  →  collector (30s poll)  →  DB
# This script only clears the coordinator's cache; the rest happens on its own.
#
# Config comes from scripts/.env (see scripts/.env.example):
#   SMOKE_API_URL_<NET>        the coordinator to poke
#   ADMIN_BEARER_TOKEN_<NET>   admin token (the same one monitoring/cleanup-workers.sh uses)

set -uo pipefail

NETWORK="${1:-}"
if [ "$NETWORK" != "testnet" ] && [ "$NETWORK" != "mainnet" ]; then
  echo "usage: $0 <testnet|mainnet>" >&2
  exit 2
fi

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
[ -f "$HERE/.env" ] && { set -a; . "$HERE/.env"; set +a; }

UP="$(echo "$NETWORK" | tr '[:lower:]' '[:upper:]')"
var() { eval "printf '%s' \"\${${1}_${UP}:-}\""; }

API_URL="$(var SMOKE_API_URL)"
ADMIN_TOKEN="$(var ADMIN_BEARER_TOKEN)"

[ -n "$API_URL" ] || { echo "SMOKE_API_URL_$UP is not set in scripts/.env" >&2; exit 1; }
[ -n "$ADMIN_TOKEN" ] || { echo "ADMIN_BEARER_TOKEN_$UP is not set in scripts/.env" >&2; exit 1; }

code=$(curl -sS --max-time 20 -o /tmp/refresh_ks.$$ -w '%{http_code}' \
  -X POST -H "Authorization: Bearer $ADMIN_TOKEN" \
  "$API_URL/admin/keystore-stats/refresh") || code=000
body=$(cat /tmp/refresh_ks.$$ 2>/dev/null); rm -f /tmp/refresh_ks.$$

if [ "$code" != "200" ]; then
  echo "refresh failed (HTTP $code): $body" >&2
  exit 1
fi
echo "cache cleared on $API_URL"

# Read the fresh numbers back, which is also what re-populates the cache. Without this the
# re-read would not happen until the next health poll, so the operator would be told "done"
# while the value they came to check was still the old one.
code=$(curl -sS --max-time 30 -o /tmp/refresh_ks.$$ -w '%{http_code}' \
  -H "Authorization: Bearer $ADMIN_TOKEN" \
  "$API_URL/admin/health/detailed") || code=000
body=$(cat /tmp/refresh_ks.$$ 2>/dev/null); rm -f /tmp/refresh_ks.$$

if [ "$code" != "200" ]; then
  echo "could not read back /admin/health/detailed (HTTP $code): $body" >&2
  exit 1
fi

printf '%s' "$body" | python3 -c '
import json, sys
d = json.load(sys.stdin)
v = d.get("checks", {}).get("keystore_vaults")
if not v:
    print("keystore_vaults: absent — the keystore did not answer, or KEYSTORE_BASE_URL is unset.")
    print("                 checks.keystore says:", d.get("checks", {}).get("keystore", {}).get("status"))
    sys.exit(1)
print(f"loaded vault masters : {v[\"vaults\"]}")
print(f"live TEE sessions    : {v[\"tee_sessions\"]}")
print(f"read                 : {v[\"age_seconds\"]}s ago")
'
echo
echo "Grafana picks this up on the collector's next poll (~30s)."
