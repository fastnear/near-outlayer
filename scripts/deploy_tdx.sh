#!/usr/bin/env bash
#
# Mac-side orchestrator: deploy a self-hosted TDX worker END-TO-END from your laptop,
# mirroring scripts/deploy_phala.sh. Unlike Phala (cloud API reachable from anywhere), the
# CVM is created on the node (vmm is loopback-only there), so this SSHes to the node to
# deploy/restart, and signs the owner measurement-approval LOCALLY — the owner key stays on
# your Mac (never on the worker node, which could otherwise approve a rogue measurement).
#
# Usage:
#   ./scripts/deploy_tdx.sh worker <testnet|mainnet> [instance-name] --version <ver> --node <ssh> [opts]
#
# Required:
#   --version <ver>     worker release, e.g. 0.1.35
#   --node <ssh>        SSH target for the TDX node (lands as root/sudoer), e.g. root@173.237.9.76
# Optional:
#   [instance-name]     CVM VM label (default outlayer-worker-<net>-<ver>-1)
#   --digest <sha256>   worker image digest override (else resolved + attested locally via gh)
#   --remote-user <u>   run node commands as this user (default: outlayer; '' = don't su)
#   --remote-dir <d>    node clone of self-hosted-tdx (default: /home/outlayer/self-hosted-tdx)
#   --dry-run           show what would happen; no deploy, no tx
#
# Measurements are approved ONCE per (network, version): the measured compose-name is stable
# (outlayer-worker-<net>-<ver>), so re-runs / extra instances of the same version hit the
# idempotent is_measurements_approved check and skip.
#
# Examples:
#   ./scripts/deploy_tdx.sh worker testnet --version 0.1.35 --node root@173.237.9.76
#   ./scripts/deploy_tdx.sh worker testnet worker-b --version 0.1.35 --node root@173.237.9.76
set -euo pipefail

DEPLOY_VERSION=""; NODE=""; DIGEST=""; DRY_RUN=false
REMOTE_USER="outlayer"; REMOTE_DIR="/home/outlayer/self-hosted-tdx"
POS=()
while [[ $# -gt 0 ]]; do case "$1" in
  --version)     DEPLOY_VERSION="${2:?}"; shift 2;;
  --node)        NODE="${2:?}"; shift 2;;
  --digest)      DIGEST="${2:?}"; shift 2;;
  --remote-user) REMOTE_USER="${2-}"; shift 2;;
  --remote-dir)  REMOTE_DIR="${2:?}"; shift 2;;
  --dry-run|--info) DRY_RUN=true; shift;;
  *) POS+=("$1"); shift;;
esac; done
COMPONENT="${POS[0]:-}"; NETWORK="${POS[1]:-testnet}"; INSTANCE_NAME="${POS[2]:-}"

[ "$COMPONENT" = worker ] || { echo "Only 'worker' is supported on TDX (keystore migrates last)." >&2; exit 1; }
case "$NETWORK" in testnet|mainnet) ;; *) echo "network must be testnet|mainnet (got '$NETWORK')" >&2; exit 1;; esac
[ -n "$DEPLOY_VERSION" ] || { echo "--version <ver> required (e.g. --version 0.1.35)" >&2; exit 1; }
[ -n "$NODE" ] || { echo "--node <ssh-target> required (e.g. --node root@173.237.9.76)" >&2; exit 1; }

VER="${DEPLOY_VERSION#v}"
SUFFIX=$([ "$NETWORK" = mainnet ] && echo near || echo testnet)
REGISTER="worker.outlayer.$SUFFIX"
OWNER="owner.outlayer.$SUFFIX"
NAME="${INSTANCE_NAME:-outlayer-worker-${NETWORK}-${VER}-1}"

# Run a command on the node as REMOTE_USER inside REMOTE_DIR. base64 avoids all quoting hell.
node_run() {
  local cmd b64
  cmd="cd '$REMOTE_DIR' && $*"
  b64=$(printf '%s' "$cmd" | base64 | tr -d '\n')
  if [ -n "$REMOTE_USER" ]; then
    ssh "$NODE" "su - '$REMOTE_USER' -c 'echo $b64 | base64 -d | bash'"
  else
    ssh "$NODE" "echo $b64 | base64 -d | bash"
  fi
}

echo "== deploy_tdx: worker $NETWORK  vm-label=$NAME  version=v$VER  node=$NODE =="

# [1/5] resolve + attest the worker digest locally (the node has no gh)
if [ -z "$DIGEST" ]; then
  echo "[1/5] Resolve worker digest for v$VER (gh, local)..."
  DIGEST=$(gh release view "v$VER" --repo fastnear/near-outlayer --json body -q '.body' 2>/dev/null \
    | grep -iE '\| *worker *\|' | grep -oE 'sha256:[a-f0-9]{64}' | head -1) || true
  [ -n "$DIGEST" ] || { echo "  Could not resolve digest via gh — pass --digest sha256:..." >&2; exit 1; }
fi
echo "  digest: $DIGEST"
if command -v gh >/dev/null 2>&1; then
  if gh attestation verify "oci://docker.io/outlayer/near-outlayer-worker@$DIGEST" -R fastnear/near-outlayer >/dev/null 2>&1; then
    echo "  Sigstore attestation: verified"
  else
    echo "  Sigstore attestation: NOT verified (continuing — verify manually before trusting)"
  fi
fi

if $DRY_RUN; then
  echo "(dry-run) would: deploy CVM '$NAME' on $NODE -> read measurements -> approve on $REGISTER (signer $OWNER) -> restart -> verify"
  exit 0
fi

# [2/5] deploy the CVM on the node (node-side deploy_tdx.sh derives the stable COMPOSE_NAME)
echo "[2/5] Deploy CVM on node (reads node env: $REMOTE_DIR/worker/.env.${NETWORK}-worker-tdx — NOT your local copy)..."
node_run "WORKER_DIGEST=$DIGEST ./deploy_tdx.sh worker $NETWORK $NAME --version $VER"

# [3/5] read the 5 TEE measurements from the worker's logs
echo "[3/5] Read TEE measurements from worker logs (waiting for boot)..."
LOGS=""; MRTD=""
for i in $(seq 1 20); do
  LOGS=$(node_run "NAME=$NAME TAIL=600 ./worker-ctl.sh logs" 2>/dev/null || true)
  MRTD=$(printf '%s' "$LOGS" | grep -oE 'MRTD:[[:space:]]+[a-f0-9]{96}' | tail -1 | grep -oE '[a-f0-9]{96}' || true)
  [ -n "$MRTD" ] && break
  sleep 12
done
[ -n "$MRTD" ] || { echo "  Could not read measurements after ~4min. Check: NAME=$NAME worker-ctl.sh follow" >&2; exit 1; }
rd(){ printf '%s' "$LOGS" | grep -oE "$1:[[:space:]]+[a-f0-9]{96}" | tail -1 | grep -oE '[a-f0-9]{96}'; }
RTMR0=$(rd RTMR0); RTMR1=$(rd RTMR1); RTMR2=$(rd RTMR2); RTMR3=$(rd RTMR3)
MEAS=$(printf '{"mrtd":"%s","rtmr0":"%s","rtmr1":"%s","rtmr2":"%s","rtmr3":"%s"}' "$MRTD" "$RTMR0" "$RTMR1" "$RTMR2" "$RTMR3")
echo "  mrtd=$MRTD"
echo "  rtmr3=$RTMR3"

# [4/5] approve measurements on the register-contract (idempotent; owner key local)
echo "[4/5] Approve measurements on $REGISTER (signer $OWNER)..."
APPROVED=$(near contract call-function as-read-only "$REGISTER" is_measurements_approved \
  json-args "{\"measurements\":$MEAS}" network-config "$NETWORK" now 2>/dev/null | tr -d '[:space:]' || true)
if printf '%s' "$APPROVED" | grep -qi true; then
  echo "  already approved — skipping (approve-once per net+version)"
else
  near contract call-function as-transaction "$REGISTER" add_approved_measurements \
    json-args "{\"measurements\":$MEAS,\"clear_others\":false}" \
    prepaid-gas '100.0 Tgas' attached-deposit '0 NEAR' \
    sign-as "$OWNER" network-config "$NETWORK" sign-with-legacy-keychain send
  echo "  measurements approved"
fi

# [5/5] restart the worker -> it re-attempts registration
echo "[5/5] Restart worker to trigger registration..."
node_run "NAME=$NAME ./worker-ctl.sh restart" >/dev/null || true
echo "  watching for registration..."
for i in $(seq 1 20); do
  L=$(node_run "NAME=$NAME TAIL=200 ./worker-ctl.sh logs" 2>/dev/null || true)
  if printf '%s' "$L" | grep -q "Worker key registered successfully"; then
    echo "  REGISTERED:"; printf '%s' "$L" | grep -E "registered successfully|Transaction:" | tail -2
    echo "Done. Manage: ssh $NODE -> su - $REMOTE_USER -> cd $REMOTE_DIR -> NAME=$NAME ./worker-ctl.sh follow"
    exit 0
  fi
  if printf '%s' "$L" | grep -qiE "Exceeded the prepaid gas|did not verify|No cached collateral"; then
    echo "  registration error:"; printf '%s' "$L" | grep -iE "exceeded|verif|collateral" | tail -3 >&2; exit 1
  fi
  sleep 12
done
echo "  No decisive result yet — check: NAME=$NAME worker-ctl.sh follow (on the node)"
