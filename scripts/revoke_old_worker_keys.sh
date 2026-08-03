#!/bin/bash
# Report — and optionally retire — the access keys of workers that are no longer running.
#
# Why this matters: a worker key is written to disk inside its CVM
# (`~/.near-credentials/worker-keypair.json`) and survives restarts, unlike the keystore's
# in-memory key. Every key on `worker.outlayer.*` may call `resolve_execution` and
# `submit_execution_output_and_resolve` on the main contract — which authorizes by
# `predecessor == operator_id` and never re-checks the registry. So a key left behind by a
# decommissioned worker can still submit execution results as long as a copy of its key file
# exists anywhere.
#
# How "live" is decided — no heuristics: the coordinator records each worker's public key when
# the worker opens its TEE session (`worker_status.worker_public_key`), and the worker uses the
# SAME key it registered on-chain (worker/src/main.rs: `tee_secret_key = secret_key.clone()`).
# Every live key is then confirmed against the chain with `view_access_key`.
#
# Full key forms come from the registration history: `register_worker_key` carries the public
# key in its arguments, and the indexer returns those arguments. This matters for ml-dsa-65
# keys — the chain only exposes them as `ml-dsa-65-hash:...`, which `PublicKey` cannot parse,
# so `remove_worker_keys` would be unable to name them without the history.
#
# Usage:
#   ./scripts/revoke_old_worker_keys.sh <testnet|mainnet> [--expect-live N] [--send]
#
#   --expect-live N   optional; abort unless the coordinator reports exactly N worker keys.
#                     The coordinator PRUNES worker_status rows, so a running worker can drop
#                     out of the KEEP set and land in the revoke list. Without the flag the
#                     report states how many workers it keeps — compare it with your fleet.
#
# Defaults (override in scripts/.env):
#   WORKER_CLEANUP_SSH=root@138.201.58.122      # host running the coordinator + its postgres
#   WORKER_CLEANUP_SSH_KEY=~/.ssh/id_ed25519
#   NEARBLOCKS_API_KEY=                          # optional; raises the indexer rate limit
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
cd "$SCRIPT_DIR/.."
[ -f "$SCRIPT_DIR/.env" ] && set -a && . "$SCRIPT_DIR/.env" && set +a || true

SSH_TARGET="${WORKER_CLEANUP_SSH:-root@138.201.58.122}"
SSH_KEY="${WORKER_CLEANUP_SSH_KEY:-$HOME/.ssh/id_ed25519}"

NETWORK="${1:-}"
shift || true
case "$NETWORK" in
  testnet) REGISTER="worker.outlayer.testnet"; OWNER="owner.outlayer.testnet"; INIT="init-worker.outlayer.testnet"
           RPC="https://rpc.testnet.fastnear.com"; IDX="https://api-testnet.nearblocks.io"
           PG="offchainvm-postgres-testnet"; DB="offchainvm" ;;
  mainnet) REGISTER="worker.outlayer.near"; OWNER="owner.outlayer.near"; INIT="init-worker.outlayer.near"
           RPC="https://rpc.mainnet.fastnear.com"; IDX="https://api.nearblocks.io"
           PG="offchainvm-postgres-mainnet"; DB="outlayer" ;;
  *) echo "Usage: $0 <testnet|mainnet> [--send]" >&2; exit 1 ;;
esac

SEND=false
EXPECT_LIVE=""
while [ $# -gt 0 ]; do
  case "$1" in
    --send) SEND=true; shift ;;
    --expect-live)
      [ $# -ge 2 ] || { echo "--expect-live needs a number" >&2; exit 1; }
      EXPECT_LIVE="$2"; shift 2 ;;
    *) echo "Unknown argument: $1" >&2; exit 1 ;;
  esac
done

# Every worker the coordinator currently knows about — deliberately NOT filtered by heartbeat
# age. A worker that is merely quiet (coordinator restart, network blip) is still running with
# its key file on disk; revoking it would break it until someone notices and restarts it. The
# heartbeat age is reported per key instead, so a genuinely dead row is visible before you sign.
LIVE=$(ssh -o BatchMode=yes "$SSH_TARGET" -i "$SSH_KEY" \
  "docker exec $PG psql -U postgres -d $DB -t -A -F'|' -c \
   \"SELECT worker_public_key, worker_id, round(extract(epoch from now()-last_heartbeat_at)/60) FROM worker_status WHERE worker_public_key IS NOT NULL AND worker_public_key <> '';\"") \
  || { echo "Could not read workers from $PG on $SSH_TARGET" >&2; exit 1; }

# An empty answer is the one failure that would sweep the whole fleet into the revoke list.
if [ -z "$(printf '%s' "$LIVE" | tr -d '[:space:]')" ]; then
  echo "The coordinator reports NO workers with a public key — refusing to build a plan." >&2
  echo "(Every installed key would look retired. Check $PG on $SSH_TARGET.)" >&2
  exit 1
fi

PLAN="$(mktemp -t worker_revoke.XXXXXX)"
trap 'rm -f "$PLAN"' EXIT

export REGISTER OWNER INIT RPC IDX NETWORK LIVE EXPECT_LIVE
python3 - <<'PY' > "$PLAN"
import json, os, sys, time, urllib.error, urllib.request

REGISTER, OWNER, INIT = os.environ['REGISTER'], os.environ['OWNER'], os.environ['INIT']
RPC, IDX, NETWORK = os.environ['RPC'], os.environ['IDX'], os.environ['NETWORK']
API_KEY = os.environ.get('NEARBLOCKS_API_KEY', '')
EXPECT_LIVE = os.environ.get('EXPECT_LIVE', '').strip()
live_rows = []
for line in os.environ['LIVE'].splitlines():
    line = line.strip()
    if not line:
        continue
    parts = line.split('|')
    live_rows.append((parts[0], parts[1] if len(parts) > 1 else '?',
                      parts[2] if len(parts) > 2 else '?'))
live_keys = [r[0] for r in live_rows]
if EXPECT_LIVE:
    # The coordinator deletes worker_status rows (stale-instance cleanup, superseded-version
    # pruning), so a RUNNING worker can silently vanish from the KEEP set. State the expected
    # fleet size and the run stops if reality disagrees.
    if len(live_keys) != int(EXPECT_LIVE):
        raise SystemExit(f"--expect-live {EXPECT_LIVE} but the coordinator reports "
                         f"{len(live_keys)} worker key(s). Refusing to build a plan.")
w = sys.stderr.write
BATCH = 16  # keys per remove_worker_keys call — the contract has no cap, this bounds gas


def rpc(params):
    body = json.dumps({"jsonrpc": "2.0", "id": 1, "method": "query", "params": params}).encode()
    return json.load(urllib.request.urlopen(
        urllib.request.Request(RPC, body, {'Content-Type': 'application/json'}), timeout=30))


_ak_cache = {}


def access_key(pk):
    """The full access key as the chain sees it, or None. Handles ml-dsa: the node resolves the
    full key to its stored hash form itself, which no local string comparison can do."""
    if pk not in _ak_cache:
        r = rpc({"request_type": "view_access_key", "finality": "final",
                 "account_id": REGISTER, "public_key": pk})
        res = r.get('result', {})
        _ak_cache[pk] = res if 'nonce' in res else None
    return _ak_cache[pk]


def key_on_account(pk):
    ak = access_key(pk)
    return ak['nonce'] if ak else None


def indexer(path):
    headers = {'Accept': 'application/json', 'User-Agent': 'outlayer-ops/1.0'}
    if API_KEY:
        headers['Authorization'] = f'Bearer {API_KEY}'
    req = urllib.request.Request(IDX + path, headers=headers)
    # NearBlocks counts requests per minute-window (~10/min on the free tier; an API key raises
    # the ceiling but not the window). Once rejected, only a full minute clears it — a shorter
    # wait just burns another rejection.
    for attempt in range(8):
        try:
            return json.load(urllib.request.urlopen(req, timeout=40))
        except urllib.error.HTTPError as e:
            if e.code in (429, 403) and attempt < 7:
                wait = 60
                w(f"  indexer {e.code} (rate limit), waiting {wait}s…\n")
                time.sleep(wait)
                continue
            raise SystemExit(f"indexer error {e.code} on {path}: set NEARBLOCKS_API_KEY in "
                             f"scripts/.env to avoid the 60s waits")
    raise SystemExit("indexer unreachable")


# 1. Every key ever registered, in FULL form, from the registration arguments.
registered = {}   # public_key -> block_timestamp
page = 1
while True:
    d = indexer(f"/v1/account/{INIT}/txns?page={page}&per_page=50&order=desc")
    txns = d.get('txns', [])
    if not txns:
        break
    for t in txns:
        # Provenance matters: this endpoint is receipt-based and returns INBOUND rows from any
        # sender, including failed ones. Without these three checks anybody could put a key of
        # their choosing into the revoke list by sending a failing `register_worker_key` to the
        # init account — including this account's own full-access keys.
        if t.get('receiver_account_id') != REGISTER:
            continue
        if t.get('predecessor_account_id') != INIT:
            continue
        status = (t.get('outcomes') or {}).get('status')
        if status is not True:
            continue
        for a in (t.get('actions') or []):
            if a.get('method') != 'register_worker_key':
                continue
            try:
                args = json.loads(a.get('args') or '{}')
            except json.JSONDecodeError:
                continue
            pk = args.get('public_key')
            if pk:
                registered.setdefault(pk, t.get('block_timestamp'))
    w(f"  history page {page}: {len(registered)} key(s) so far\n")
    if len(txns) < 50:
        break
    page += 1
    if page > 20:                                   # sanity bound
        w("!! stopped paging the indexer at 1000 transactions\n")
        break

w(f"\nregistration history: {len(registered)} distinct key(s) ever registered by {INIT}\n")

# 2. Which of them are still installed, and which are live.
live_set = set(live_keys)
for pk in live_keys:
    if key_on_account(pk) is None:
        raise SystemExit(f"Live worker key is NOT on {REGISTER}: {pk[:40]}…\n"
                         "Refusing to build a plan — the coordinator and the chain disagree.")
    registered.setdefault(pk, None)

# The permission profile of the LIVE keys defines what a worker key looks like on this account.
# Anything that does not match it is not ours to delete — this is what structurally prevents a
# full-access key from ever entering the plan, whatever the indexer claims.
live_receivers = set()
for pk in live_keys:
    perm = access_key(pk)['permission']
    if not isinstance(perm, dict):
        raise SystemExit(f"Live worker key {pk[:34]}… is NOT a FunctionCall key — refusing to "
                         f"reason about this account.")
    live_receivers.add(perm['FunctionCall']['receiver_id'])

installed, stale, foreign = [], [], []
for pk in registered:
    ak = access_key(pk)
    if ak is None:
        continue                                    # already deleted earlier
    installed.append(pk)
    if pk in live_set:
        continue
    perm = ak['permission']
    if not isinstance(perm, dict) or perm['FunctionCall']['receiver_id'] not in live_receivers:
        kind = 'FullAccess' if not isinstance(perm, dict) else \
            f"FunctionCall->{perm['FunctionCall']['receiver_id']}"
        foreign.append((pk, kind))
        continue
    stale.append((pk, ak['nonce'] % 1_000_000, registered[pk]))

onchain_total = len([k for k in rpc({"request_type": "view_access_key_list", "finality": "final",
                                     "account_id": REGISTER})['result']['keys']
                     if isinstance(k['access_key']['permission'], dict)])

w(f"on {REGISTER}: {onchain_total} FunctionCall key(s); recovered from history: {len(installed)}\n")
if onchain_total > len(installed):
    w(f"!! {onchain_total - len(installed)} key(s) on the account are NOT in the registration "
      f"history — they were not installed by register_worker_key. Inspect them manually; this "
      f"script will not touch them.\n")

w(f"\nKEEP — known to the coordinator ({len(live_keys)}):\n")
quiet = []
for pk, wid, age in sorted(live_rows, key=lambda r: r[1]):
    try:
        age_i = int(float(age))
    except ValueError:
        age_i = -1
    if age_i > 15:
        quiet.append((wid, age_i))
    w(f"    {pk[:34]}…  tx={key_on_account(pk) % 1_000_000:<4} {wid[:44]}  "
      f"heartbeat {age_i if age_i >= 0 else '?'}m ago\n")
if quiet:
    w(f"!! {len(quiet)} kept worker(s) have not sent a heartbeat recently — they are kept "
      f"anyway; confirm they are still meant to run: {quiet}\n")

def ts(v):
    if not v:
        return "unknown"
    import datetime
    return datetime.datetime.fromtimestamp(int(v) / 1e9, datetime.timezone.utc).strftime('%Y-%m-%d')

if foreign:
    w(f"\n!! {len(foreign)} key(s) came out of the history but do NOT look like worker keys "
      f"(expected FunctionCall -> {sorted(live_receivers)}). NOT revoked:\n")
    for pk, kind in foreign:
        w(f"    {pk[:34]}…  {kind}\n")

w(f"\nSTALE — revoke ({len(stale)}):\n")
for pk, used, when in sorted(stale, key=lambda s: -s[1]):
    w(f"    {pk[:34]}…  tx={used:<4} registered={ts(when)}\n")

w(f"\n=> remove {len(stale)} key(s) in {-(-len(stale) // BATCH)} call(s), signed by {OWNER}\n")
if stale:
    w(f"\n   CHECK BEFORE SIGNING: this keeps {len(live_keys)} worker(s). If MORE than "
      f"{len(live_keys)} workers are running right now, some are missing from the coordinator's\n"
      f"   table (it prunes rows) and their keys are in the list above — stop and re-run with "
      f"--expect-live <real count>.\n")
if not stale:
    print("echo 'Nothing to retire — every installed key belongs to a live worker.'")
for n in range(0, len(stale), BATCH):
    args = json.dumps({"public_keys": [pk for pk, _, _ in stale[n:n + BATCH]]})
    print(f"near contract call-function as-transaction {REGISTER} remove_worker_keys "
          f"json-args '{args}' prepaid-gas '300.0 Tgas' attached-deposit '0 NEAR' "
          f"sign-as {OWNER} network-config {NETWORK} sign-with-legacy-keychain send")
PY

VERIFY="near account list-keys $REGISTER network-config $NETWORK now"

if [ "$SEND" = true ]; then
  read -r -p "Send these transactions as $OWNER? Type 'yes': " CONFIRM
  [ "$CONFIRM" = "yes" ] || { echo "Aborted."; exit 1; }
  bash -e "$PLAN"
  echo
  echo "Verify: $VERIFY"
else
  echo "--- run these where the $OWNER key lives ---"
  echo
  cat "$PLAN"
  echo
  echo "Then verify: $VERIFY"
fi
