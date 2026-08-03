#!/bin/bash
# Report — and optionally retire — everything belonging to OLD keystore versions.
#
# Run it as the LAST step of an upgrade: new keystore deployed and voted in, workers and
# coordinator repointed at it, everything verified. Then this drops the previous versions.
#
# How "old" is decided: every approved key is grouped by the IMAGE it registered under (the
# measurements recorded in its registration proposal). The image of the most recent
# registration is CURRENT; every other image is OLD. Grouping by image — not by key — is what
# makes this stable:
#   * a fleet of several instances of the current version keeps ALL of its keys;
#   * an instance that restarted and re-registered produces a new proposal with the SAME
#     measurements, so it stays in the current group instead of looking "newer";
#   * keys whose proposal is unreadable (they pre-date the current proposal format) are old by
#     construction — those proposals are the oldest ones on the contract.
#
# What it retires, in one run:
#   1. the OLD keys    — `propose_revoke_keystore_keys` (one proposal per <=16 keys, signed by
#                        zavodil.*). With a single DAO member the proposal executes in the same
#                        call; with more members the others just vote by id, with the same
#                        `vote {"proposal_id":N,"approve":true}` used for registrations
#   2. the OLD images  — allowlist rebuilt to the current image(s), so a retired image can
#                        never register again (owner-signed, one atomic call)
# Both belong here rather than at deploy time: while an old instance is still serving, removing
# its key or its image breaks it — its registration key is ephemeral, so it cannot re-register
# after a restart.
#
# A retired instance keeps answering from RAM for vaults it already loaded, but cannot derive a
# master for a NEW vault and cannot survive a restart. Shut it down afterwards.
#
# Usage:
#   ./scripts/revoke_old_keystore_keys.sh <testnet|mainnet> [--keep ed25519:KEY]... [--send]
#
#   --keep   force a key into the CURRENT group (a live instance whose proposal is unreadable,
#            or an instance you are deliberately keeping on an older image)
#   --send   execute here after a confirmation prompt; without it the script only prints the
#            report and ready-to-run commands (mainnet keys usually live on another machine)
set -euo pipefail

NETWORK="${1:-}"
shift || true
case "$NETWORK" in
  testnet) DAO="dao.outlayer.testnet"; SIGNER="zavodil.testnet"; OWNER="owner.outlayer.testnet"; RPC="https://rpc.testnet.fastnear.com" ;;
  mainnet) DAO="dao.outlayer.near";    SIGNER="zavodil.near";    OWNER="owner.outlayer.near";    RPC="https://rpc.mainnet.fastnear.com" ;;
  *) echo "Usage: $0 <testnet|mainnet> [--keep ed25519:KEY]... [--send]" >&2; exit 1 ;;
esac

KEEP=()
SEND=false
while [ $# -gt 0 ]; do
  case "$1" in
    --keep)
      [ $# -ge 2 ] || { echo "--keep needs a public key" >&2; exit 1; }
      KEEP+=("$2"); shift 2 ;;
    --send) SEND=true; shift ;;
    *) echo "Unknown argument: $1" >&2; exit 1 ;;
  esac
done
# MAX_REVOKE_BATCH in keystore-dao-contract: the contract rejects a larger ballot.
BATCH=16

PLAN="$(mktemp -t revoke_plan.XXXXXX)"
trap 'rm -f "$PLAN"' EXIT

export DAO RPC BATCH SIGNER OWNER NETWORK
KEEP_ARGS="${KEEP[*]:-}" python3 - <<'PY' > "$PLAN"
import base64, datetime, json, os, sys, urllib.request

RPC, DAO, BATCH = os.environ['RPC'], os.environ['DAO'], int(os.environ['BATCH'])
SIGNER, OWNER, NETWORK = os.environ['SIGNER'], os.environ['OWNER'], os.environ['NETWORK']
forced_keep = [k for k in os.environ.get('KEEP_ARGS', '').split() if k]
w = sys.stderr.write


def rpc(params):
    body = json.dumps({"jsonrpc": "2.0", "id": 1, "method": "query", "params": params}).encode()
    return json.load(urllib.request.urlopen(
        urllib.request.Request(RPC, body, {'Content-Type': 'application/json'})))


def call(method, args):
    return rpc({"request_type": "call_function", "finality": "final", "account_id": DAO,
                "method_name": method,
                "args_base64": base64.b64encode(json.dumps(args).encode()).decode()})


def view(method, args):
    """Decoded view result; any failure is fatal — silently skipping a read is how a live key
    ends up in the revoke list."""
    r = call(method, args)
    if 'result' not in r or 'result' not in r['result']:
        err = json.dumps(r)
        if 'MethodNotFound' in err and method == 'get_approved_keystores':
            raise SystemExit(
                f"{DAO} does not have `get_approved_keystores` — the wasm with "
                f"`propose_revoke_keystore_keys` is not deployed on {NETWORK} yet. Deploy "
                f"keystore-dao-contract/res/keystore_dao_contract.wasm, then call migrate().")
        raise SystemExit(f"RPC error on {method}({args}): {err[:400]}")
    return json.loads(bytes(r['result']['result']).decode())


def read_proposal(pid):
    """('ok', p) | ('legacy', None). Proposals older than the current format no longer
    deserialize — permanent and harmless. Any OTHER error aborts: a plan built from partial
    data is exactly how the wrong key gets revoked."""
    r = call('get_proposal', {"proposal_id": pid})
    if 'result' in r and 'result' in r['result']:
        return ('ok', json.loads(bytes(r['result']['result']).decode()))
    err = json.dumps(r)
    if 'Borsh' in err or 'deserialize' in err.lower():
        return ('legacy', None)
    raise SystemExit(f"RPC error reading proposal {pid}: {err[:400]}\n"
                     "Refusing to build a plan from partial data — re-run.")


approved, i = [], 0
while True:
    chunk = view('get_approved_keystores', {"from_index": i, "limit": 50})
    approved += chunk
    if len(chunk) < 50:
        break
    i += 50
cfg = view('get_config', {})

# Integrity check: the contract's set must match the FunctionCall keys actually installed on
# the account. A key on the account but NOT in the set cannot be revoked through the DAO at all
# (the contract only removes set members), so it is worth seeing before you trust the plan.
akl = rpc({"request_type": "view_access_key_list", "finality": "final", "account_id": DAO})
account_fc = {k['public_key'] for k in akl.get('result', {}).get('keys', [])
              if isinstance(k['access_key'].get('permission'), dict)}

# Key -> its (latest) executed registration proposal, which carries the image measurements.
by_key, legacy = {}, 0
for pid in range(0, cfg['next_proposal_id']):
    kind, p = read_proposal(pid)
    if kind == 'legacy':
        legacy += 1
    elif p and p.get('status') == 'Executed':
        prev = by_key.get(p['public_key'])
        if not prev or p['created_at'] > prev['created_at']:
            by_key[p['public_key']] = p

# Group approved keys by image; None = unknown image (unreadable proposal) => old.
groups = {}
for k in approved:
    p = by_key.get(k)
    img = json.dumps(p['measurements'], sort_keys=True) if p else None
    g = groups.setdefault(img, {"keys": [], "latest": -1})
    g["keys"].append(k)
    if p:
        g["latest"] = max(g["latest"], p['created_at'])

known = {img: g for img, g in groups.items() if img is not None}
if not known:
    raise SystemExit("No approved key has a readable registration proposal — cannot tell "
                     "versions apart. Re-run with explicit --keep key(s).")
current_img = max(known, key=lambda img: known[img]["latest"])

unknown_forced = [k for k in forced_keep if k not in approved]
if unknown_forced:
    raise SystemExit("--keep key(s) not in approved_keystores: " + ", ".join(unknown_forced))

keep_keys = set(groups[current_img]["keys"]) | set(forced_keep)
old_keys = [k for k in approved if k not in keep_keys]

keep_images = [json.loads(current_img)]
for k in forced_keep:
    p = by_key.get(k)
    if p and p['measurements'] not in keep_images:
        keep_images.append(p['measurements'])
forced_unknown_image = [k for k in forced_keep if k not in by_key]


def stamp(ts):
    return (datetime.datetime.fromtimestamp(ts / 1e9, datetime.timezone.utc)
            .strftime('%Y-%m-%d %H:%M')) if ts > 0 else 'unknown'


def show(img, g, label):
    m = json.loads(img) if img else None
    head = (f"mrtd={m['mrtd'][:8]}… rtmr3={m['rtmr3'][:8]}…" if m
            else "image unknown (proposal unreadable)")
    w(f"\n{label}  {head}   last registered {stamp(g['latest'])}   {len(g['keys'])} key(s)\n")
    for k in sorted(g['keys']):
        missing = "" if k in account_fc else "  (not on the account)"
        forced = "  [--keep]" if k in forced_keep and img != current_img else ""
        w(f"    {k}{missing}{forced}\n")


w(f"\nDAO {DAO}  —  {len(approved)} approved key(s) in {len(groups)} image group(s), "
  f"threshold {cfg['approval_threshold']} of {cfg['dao_members_count']} member(s)\n")
if legacy:
    w(f"note: {legacy} proposal(s) pre-date the current format and cannot be read\n")

show(current_img, groups[current_img], "CURRENT")
for img, g in sorted(known.items(), key=lambda kv: -kv[1]["latest"]):
    if img != current_img:
        show(img, g, "OLD    ")
if None in groups:
    show(None, groups[None], "OLD    ")

orphans = sorted(account_fc - set(approved))
if orphans:
    w(f"\n!! {len(orphans)} FunctionCall key(s) on the account are NOT in approved_keystores, "
      f"so a revoke proposal cannot remove them (owner must delete them directly):\n")
    for k in orphans:
        w(f"    {k}\n")

images_onchain = view('get_approved_measurements', {})
stale_images = [m for m in images_onchain if m not in keep_images]

w(f"\n=> revoke {len(old_keys)} key(s) via {-(-len(old_keys) // BATCH)} proposal(s); "
  f"drop {len(stale_images)} of {len(images_onchain)} approved image(s)\n")

for n in range(0, len(old_keys), BATCH):
    args = json.dumps({"public_keys": old_keys[n:n + BATCH]})
    print(f"near contract call-function as-transaction {DAO} propose_revoke_keystore_keys "
          f"json-args '{args}' prepaid-gas '300.0 Tgas' attached-deposit '0 NEAR' "
          f"sign-as {SIGNER} network-config {NETWORK} sign-with-legacy-keychain send")

if forced_unknown_image:
    w("\nImage cleanup SKIPPED: a --keep key has no readable proposal, so its image cannot be "
      "re-added and rebuilding the allowlist could strand it.\n")
elif stale_images:
    # clear_others=true on the first call replaces the whole list atomically — the kept images
    # go back in the same transaction, so there is never a moment without them.
    for idx, m in enumerate(keep_images):
        args = json.dumps({"measurements": m, "clear_others": idx == 0})
        print(f"near contract call-function as-transaction {DAO} add_approved_measurements "
              f"json-args '{args}' prepaid-gas '30.0 Tgas' attached-deposit '0 NEAR' "
              f"sign-as {OWNER} network-config {NETWORK} sign-with-legacy-keychain send")

if not old_keys and not stale_images:
    print("echo 'Nothing to retire — only the current version is approved.'")
PY

VERIFY="near contract call-function as-read-only $DAO get_approved_keystores json-args '{\"from_index\":0,\"limit\":50}' network-config $NETWORK now"

if [ "$SEND" = true ]; then
  read -r -p "Send these transactions as $SIGNER / $OWNER? Type 'yes': " CONFIRM
  [ "$CONFIRM" = "yes" ] || { echo "Aborted."; exit 1; }
  bash -e "$PLAN"
  echo
  echo "Verify: $VERIFY"
else
  echo "--- run these where the $SIGNER / $OWNER keys live ---"
  echo "(each proposal records the proposer's vote; with $(printf '%s' "$DAO") at threshold 1 it executes at once,"
  echo " otherwise other members vote with: vote {\"proposal_id\":N,\"approve\":true})"
  echo
  cat "$PLAN"
  echo
  echo "Then verify: $VERIFY"
fi
