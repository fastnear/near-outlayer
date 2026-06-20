#!/usr/bin/env bash
#
# Mac-side orchestrator: deploy a self-hosted TDX worker END-TO-END from your laptop,
# mirroring scripts/deploy_phala.sh. Unlike Phala (cloud API reachable from anywhere), the
# CVM is created on the node (vmm is loopback-only there), so this SSHes to the node to
# deploy/restart, and signs the owner measurement-approval LOCALLY — the owner key stays on
# your Mac (never on the worker node, which could otherwise approve a rogue measurement).
#
# Usage:
#   ./scripts/deploy_tdx.sh worker   <testnet|mainnet> [instance-name] --version <ver> --node <ssh> [opts]
#   ./scripts/deploy_tdx.sh keystore <testnet|mainnet> [instance-name] --version <ver> --node <ssh> [opts]
#
# Required:
#   --version <ver>     release, e.g. 0.1.35
#   --node <ssh>        SSH target for the TDX node (lands as root/sudoer), e.g. root@173.237.9.76
# Optional:
#   [instance-name]     CVM VM label (default outlayer-<component>-<net>-<ver>-1)
#   --digest <sha256>   image digest override (else resolved + attested locally via gh)
#   --remote-user <u>   run node commands as this user (default: outlayer; '' = don't su)
#   --remote-dir <d>    node clone of self-hosted-tdx (default: /home/outlayer/self-hosted-tdx)
#   --gateway-url <url> keystore only: per-VM dstack-gateway URL (e.g.
#                       https://gateway.dstack.outlayer.ai:9202). Set -> keystore deploys
#                       gateway-enabled and gets a public HTTPS endpoint
#                       https://<keystore-app-id>-8081.<domain>. Unset -> loopback-only keystore.
#   --dry-run           show what would happen; no deploy, no tx
#
# Measurements are approved ONCE per (network, version): the measured compose-name is stable
# (outlayer-<component>-<net>-<ver>), so re-runs / extra instances of the same version hit the
# idempotent is_measurements_approved check and skip.
#
# worker:   deploy -> read measurements -> owner-approve on the register-contract -> restart
#           -> verify registration. Owner key stays on the Mac.
# keystore: deploy (self-submits its DAO registration) -> read measurements -> owner-approve on
#           the DAO contract -> wait for the keystore's "Proposal ID: N" -> vote as zavodil
#           (auto on testnet, printed for manual run on mainnet) -> poll /health until ready.
#           Owner + zavodil keys stay on the Mac.
#
# Examples:
#   ./scripts/deploy_tdx.sh worker   testnet --version 0.1.35 --node root@173.237.9.76
#   ./scripts/deploy_tdx.sh worker   testnet worker-b --version 0.1.35 --node root@173.237.9.76
#   ./scripts/deploy_tdx.sh keystore testnet --version 0.1.35 --node root@173.237.9.76
set -euo pipefail

DEPLOY_VERSION=""; NODE=""; DIGEST=""; DRY_RUN=false; GATEWAY_URL=""
REMOTE_USER="outlayer"; REMOTE_DIR="/home/outlayer/self-hosted-tdx"
POS=()
while [[ $# -gt 0 ]]; do case "$1" in
  --version)     DEPLOY_VERSION="${2:?}"; shift 2;;
  --node)        NODE="${2:?}"; shift 2;;
  --digest)      DIGEST="${2:?}"; shift 2;;
  --remote-user) REMOTE_USER="${2-}"; shift 2;;
  --remote-dir)  REMOTE_DIR="${2:?}"; shift 2;;
  # keystore only: per-VM dstack-gateway URL. Set -> keystore deploys gateway-enabled (public HTTPS
  # https://<keystore-app-id>-8081.<domain>). Forwarded to the node as GATEWAY_URL=...
  --gateway-url) GATEWAY_URL="${2:?}"; shift 2;;
  --dry-run|--info) DRY_RUN=true; shift;;
  *) POS+=("$1"); shift;;
esac; done
COMPONENT="${POS[0]:-}"; NETWORK="${POS[1]:-testnet}"; INSTANCE_NAME="${POS[2]:-}"

case "$COMPONENT" in worker|keystore) ;; *) echo "component must be worker|keystore (got '$COMPONENT')" >&2; exit 1;; esac
case "$NETWORK" in testnet|mainnet) ;; *) echo "network must be testnet|mainnet (got '$NETWORK')" >&2; exit 1;; esac
[ -n "$DEPLOY_VERSION" ] || { echo "--version <ver> required (e.g. --version 0.1.35)" >&2; exit 1; }
[ -n "$NODE" ] || { echo "--node <ssh-target> required (e.g. --node root@173.237.9.76)" >&2; exit 1; }

VER="${DEPLOY_VERSION#v}"
SUFFIX=$([ "$NETWORK" = mainnet ] && echo near || echo testnet)
OWNER="owner.outlayer.$SUFFIX"
# Per-component config:
#   IMAGE   = Docker Hub image (also the GitHub-release digest row label, lowercased).
#   NAME    = per-instance CVM VM label (lsvm / worker-ctl.sh).
#   worker:   measurements + registration live on the register-contract worker.outlayer.<net>.
#   keystore: governance lives on the DAO dao.outlayer.<net>; the keystore container is
#             dstack-keystore-1 (worker-ctl.sh defaults to dstack-worker-1, so we pass CONTAINER).
case "$COMPONENT" in
  worker)
    IMAGE="outlayer/near-outlayer-worker"
    REGISTER="worker.outlayer.$SUFFIX"
    NAME="${INSTANCE_NAME:-outlayer-worker-${NETWORK}-${VER}-1}"
    ;;
  keystore)
    IMAGE="outlayer/near-outlayer-keystore"
    DAO="dao.outlayer.$SUFFIX"
    VOTER="zavodil.$SUFFIX"
    KS_CONTAINER="dstack-keystore-1"
    NAME="${INSTANCE_NAME:-outlayer-keystore-${NETWORK}-${VER}-1}"
    ;;
esac

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

echo "== deploy_tdx: $COMPONENT $NETWORK  vm-label=$NAME  version=v$VER  node=$NODE =="

# [1/N] resolve + attest the image digest locally (the node has no gh). The GitHub release
# body lists one row per component: `| worker | sha256:... |`, `| keystore | sha256:... |`.
if [ -z "$DIGEST" ]; then
  echo "[1] Resolve $COMPONENT digest for v$VER (gh, local)..."
  DIGEST=$(gh release view "v$VER" --repo fastnear/near-outlayer --json body -q '.body' 2>/dev/null \
    | grep -iE "\\| *$COMPONENT *\\|" | grep -oE 'sha256:[a-f0-9]{64}' | head -1) || true
  [ -n "$DIGEST" ] || { echo "  Could not resolve digest via gh — pass --digest sha256:..." >&2; exit 1; }
fi
echo "  digest: $DIGEST"
if command -v gh >/dev/null 2>&1; then
  if gh attestation verify "oci://docker.io/$IMAGE@$DIGEST" -R fastnear/near-outlayer >/dev/null 2>&1; then
    echo "  Sigstore attestation: verified"
  else
    echo "  Sigstore attestation: NOT verified (continuing — verify manually before trusting)"
  fi
fi

if $DRY_RUN; then
  if [ "$COMPONENT" = worker ]; then
    echo "(dry-run) would: deploy CVM '$NAME' on $NODE -> read measurements -> approve on $REGISTER (signer $OWNER) -> restart -> verify"
  else
    echo "(dry-run) would: deploy CVM '$NAME' on $NODE${GATEWAY_URL:+ (gateway-url=$GATEWAY_URL)} -> read measurements -> approve on $DAO (signer $OWNER) -> wait for proposal -> vote (signer $VOTER) -> poll /health"
  fi
  exit 0
fi

if [ "$COMPONENT" = worker ]; then
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
exit 0
fi  # end worker

# ============================== keystore flow ==============================
# Unlike the worker (owner-approve -> restart -> register), the keystore self-submits its DAO
# registration on boot (logs "Proposal ID: N"). Governance = owner-approve measurements on the
# DAO + a DAO member (zavodil) vote. Both signed LOCALLY (keys stay on the Mac).

# Logs come from the dstack-keystore-1 container (worker-ctl.sh defaults to dstack-worker-1).
ks_logs() { node_run "NAME=$NAME CONTAINER=$KS_CONTAINER TAIL=${1:-600} ./worker-ctl.sh logs"; }

# [2/6] deploy the CVM on the node (node-side deploy_tdx.sh derives the stable COMPOSE_NAME).
# GATEWAY_URL (optional) flows Mac --gateway-url -> node env -> 40-deploy-keystore.sh: when set, the
# keystore deploys gateway-enabled and gets a public HTTPS endpoint.
echo "[2/6] Deploy CVM on node (reads node env: $REMOTE_DIR/keystore/.env.${NETWORK}-keystore-tdx — NOT your local copy)..."
if [ -n "$GATEWAY_URL" ]; then
  echo "  gateway mode: GATEWAY_URL=$GATEWAY_URL"
else
  echo "  WARNING: no --gateway-url given -> deploying a PLAIN (loopback-only) keystore."
  echo "           It registers + serves on the node, but has NO public endpoint: the gateway URL"
  echo "           https://<app-id>-8081.<gateway-domain> will NOT route to it (404)."
  echo "           For a public TEE-terminated endpoint, pass: --gateway-url https://gateway.<domain>:9202"
fi
# Capture the node deploy output (tee'd so the operator still sees it) to recover the gateway-mode
# KEYSTORE_BASE_URL=... line that 40-deploy-keystore.sh prints (app-id is computed node-side from
# the final app-compose). Empty in plain mode.
DEPLOY_LOG=$(node_run "GATEWAY_URL='$GATEWAY_URL' WORKER_DIGEST=$DIGEST ./deploy_tdx.sh keystore $NETWORK $NAME --version $VER" 2>&1 | tee /dev/stderr) || true
KEYSTORE_BASE_URL=$(printf '%s' "$DEPLOY_LOG" | grep -oE 'KEYSTORE_BASE_URL=https://[^[:space:]]+' | tail -1 | sed 's/^KEYSTORE_BASE_URL=//' || true)

# [3/6] read the 5 TEE measurements from the keystore's logs.
# The keystore logs them as a single DEBUG line: "TDX Measurements: MRTD=<hex>, RTMR0=<hex>, ..."
# (comma-separated KEY=hex — NOT the worker's "MRTD:  <hex>" colon form). RUST_LOG includes
# keystore_worker=debug so the line is emitted.
#
# B1: that line is printed ONCE per boot (tdx_attestation.rs call_phala_dstack_socket, debug). The
# old TAIL=600 window was too small — on a chatty boot the line scrolls past it and the grep finds
# nothing -> a false abort. Fix: (a) capture a MUCH larger window (KS_MEAS_TAIL=8000), and (b) force
# a clean restart FIRST so the once-per-boot line is freshly near the tail of the captured window
# (it's the same kind of restart the flow already does before grepping the Proposal ID). Robust under
# set -euo pipefail: worker-ctl.sh restart is best-effort (|| true), then we poll.
KS_MEAS_TAIL="${KS_MEAS_TAIL:-8000}"
echo "[3/6] Restart keystore so it re-emits the once-per-boot 'TDX Measurements:' line, then read it..."
node_run "NAME=$NAME ./worker-ctl.sh restart" >/dev/null 2>&1 || true
echo "  reading TEE measurements from keystore logs (window=$KS_MEAS_TAIL lines; waiting for boot)..."
LOGS=""; MRTD=""
for i in $(seq 1 20); do
  LOGS=$(ks_logs "$KS_MEAS_TAIL" 2>/dev/null || true)
  MRTD=$(printf '%s' "$LOGS" | grep -oE 'MRTD=[a-f0-9]{96}' | tail -1 | grep -oE '[a-f0-9]{96}' || true)
  [ -n "$MRTD" ] && break
  sleep 12
done
[ -n "$MRTD" ] || { echo "  Could not read measurements after ~4min. Check: NAME=$NAME CONTAINER=$KS_CONTAINER worker-ctl.sh follow" >&2; exit 1; }
rd(){ printf '%s' "$LOGS" | grep -oE "$1=[a-f0-9]{96}" | tail -1 | grep -oE '[a-f0-9]{96}'; }
RTMR0=$(rd RTMR0); RTMR1=$(rd RTMR1); RTMR2=$(rd RTMR2); RTMR3=$(rd RTMR3)
MEAS=$(printf '{"mrtd":"%s","rtmr0":"%s","rtmr1":"%s","rtmr2":"%s","rtmr3":"%s"}' "$MRTD" "$RTMR0" "$RTMR1" "$RTMR2" "$RTMR3")
echo "  mrtd=$MRTD"
echo "  rtmr3=$RTMR3"

# [4/6] approve measurements on the DAO contract (idempotent; owner key local)
echo "[4/6] Approve measurements on $DAO (signer $OWNER)..."
# M1: parse the bool strictly (the old `grep -qi true` over full near-cli-rs output is fragile — it
# matches "true" anywhere in the noisy output). Mirror the stricter phala form: pull the FIRST
# true|false token and compare exactly.
APPROVED=$(near contract call-function as-read-only "$DAO" is_measurements_approved \
  json-args "{\"measurements\":$MEAS}" network-config "$NETWORK" now 2>/dev/null \
  | grep -oiE 'true|false' | head -1 || true)
if [ "$APPROVED" = true ]; then
  echo "  already approved — skipping (approve-once per net+version)"
else
  near contract call-function as-transaction "$DAO" add_approved_measurements \
    json-args "{\"measurements\":$MEAS,\"clear_others\":false}" \
    prepaid-gas '100.0 Tgas' attached-deposit '0 NEAR' \
    sign-as "$OWNER" network-config "$NETWORK" sign-with-legacy-keychain send
  echo "  measurements approved"
fi

# [5/6] wait for the keystore's self-submitted proposal, then vote as zavodil.
# Measurements are approved now, so if the keystore had already submitted (and the DAO rejected
# for not-approved) it will resubmit on the next boot. Restart once to force a clean submit
# against the now-approved list, then grep for the Proposal ID.
echo "[5/6] Restart keystore so it submits against the approved measurements..."
node_run "NAME=$NAME ./worker-ctl.sh restart" >/dev/null || true
echo "  waiting for the keystore to submit its DAO proposal (Proposal ID: N)..."
PROPOSAL_ID=""
for i in $(seq 1 25); do
  L=$(ks_logs 300 2>/dev/null || true)
  # main.rs logs "Proposal ID: {}" on a successful submit; tee_registration.rs logs the same.
  PROPOSAL_ID=$(printf '%s' "$L" | grep -oE 'Proposal ID: [0-9]+' | grep -oE '[0-9]+' | tail -1 || true)
  [ -n "$PROPOSAL_ID" ] && break
  # Do NOT early-exit on "measurements not approved" / "Registration rejected": that line is EXPECTED
  # from the keystore's FIRST boot (before [4/6] approved) and persists in the log buffer, so bailing
  # on it is a false negative — the keystore retries (container `restart: on-failure`) and submits a
  # proposal once the measurements are approved. The ~5min timeout below catches a genuine failure.
  sleep 12
done
[ -n "$PROPOSAL_ID" ] || { echo "  No 'Proposal ID' in logs after ~5min. Check: NAME=$NAME CONTAINER=$KS_CONTAINER worker-ctl.sh follow" >&2; exit 1; }
echo "  proposal id: $PROPOSAL_ID"

# Vote: auto on testnet, manual on mainnet (mirror deploy_phala.sh step 7 — a human turns the
# mainnet key). Threshold with the single member zavodil is 1, so one approve passes it.
VOTE_ARGS="{\"proposal_id\":$PROPOSAL_ID,\"approve\":true}"
if [ "$NETWORK" = mainnet ]; then
  echo "  MAINNET — vote manually to approve proposal #$PROPOSAL_ID:"
  echo ""
  echo "    near contract call-function as-transaction $DAO vote \\"
  echo "      json-args '$VOTE_ARGS' \\"
  echo "      prepaid-gas '100.0 Tgas' attached-deposit '0 NEAR' \\"
  echo "      sign-as $VOTER network-config $NETWORK sign-with-legacy-keychain send"
  echo ""
  echo "  After the vote lands, the keystore pulls its MPC-CKD master and becomes ready."
  echo "  Check: NAME=$NAME CONTAINER=$KS_CONTAINER worker-ctl.sh follow (on the node)"
  if [ -n "$KEYSTORE_BASE_URL" ]; then
    echo ""
    echo "  Once READY, the public endpoint (via dstack-gateway) is:"
    echo "    KEYSTORE_BASE_URL=$KEYSTORE_BASE_URL"
  fi
  exit 0
fi
echo "  Voting on proposal #$PROPOSAL_ID as $VOTER..."
near contract call-function as-transaction "$DAO" vote \
  json-args "$VOTE_ARGS" \
  prepaid-gas '100.0 Tgas' attached-deposit '0 NEAR' \
  sign-as "$VOTER" network-config "$NETWORK" sign-with-legacy-keychain send
echo "  voted"

# [6/6] poll the keystore until it is READY. /health returns 200 unconditionally (it does NOT
# reflect is_ready), so 200 only proves the server is UP. The authoritative readiness signal is
# the log line "✅ TEE registration complete! Keystore is now ready to serve requests" (main.rs).
# We confirm liveness via the loopback KS port AND readiness via that log line.
echo "[6/6] Wait for the keystore to finish TEE registration + become ready..."
for i in $(seq 1 25); do
  # Loopback liveness probe through the node (the KS port is 127.0.0.1-only on the node). The
  # keystore port -> guest 8081 is read from the live qemu cmdline, same as worker-ctl's agent_port.
  HEALTH=$(node_run "set -e; u=\$(NAME=$NAME ./worker-ctl.sh uuid); \
    p=\$(for pid in \$(pgrep -f qemu-system-x86_64); do c=\$(tr '\\0' ' ' < /proc/\$pid/cmdline 2>/dev/null) || continue; \
      [[ \"\$c\" == *\"\$u\"* ]] || continue; echo \"\$c\" | grep -oE '127\\.0\\.0\\.1:[0-9]+-:8081' | sed -E 's/.*:([0-9]+)-:8081/\\1/' | head -1; break; done); \
    [ -n \"\$p\" ] && curl -fs --max-time 5 \"http://127.0.0.1:\$p/health\" || true" 2>/dev/null || true)
  L=$(ks_logs 300 2>/dev/null || true)
  if printf '%s' "$L" | grep -q "Keystore is now ready to serve requests"; then
    echo "  READY: keystore completed TEE registration + pulled its MPC-CKD master."
    [ -n "$HEALTH" ] && echo "  /health (loopback): $HEALTH"
    echo "Done. Manage: ssh $NODE -> su - $REMOTE_USER -> cd $REMOTE_DIR -> NAME=$NAME CONTAINER=$KS_CONTAINER ./worker-ctl.sh follow"
    if [ -n "$KEYSTORE_BASE_URL" ]; then
      echo ""
      echo "  PUBLIC ENDPOINT (via dstack-gateway, TLS terminates in the TEE) — wire this into"
      echo "  workers/coordinator as the keystore base URL:"
      echo "    KEYSTORE_BASE_URL=$KEYSTORE_BASE_URL"
    else
      echo "NOTE: this exposes only a LOOPBACK port on the node. Public ingress (TLS-in-TEE gateway)"
      echo "needs gateway mode — re-run with --gateway-url <https://gateway.<domain>:9202> to get a"
      echo "public https://<keystore-app-id>-8081.<domain> endpoint."
    fi
    exit 0
  fi
  # Do NOT early-exit on "TEE registration failed" / "remain in not-ready": those lines are EXPECTED
  # from the keystore's pre-approval boot attempts and persist in the log buffer, so bailing on them is
  # a false negative — the keystore retries (container `restart: on-failure`) and, once the vote
  # executes, pulls its MPC-CKD master and logs "ready to serve requests". Wait for THAT (the timeout
  # below catches a genuine never-ready).
  sleep 12
done
echo "  Voted on #$PROPOSAL_ID but not READY yet — check: NAME=$NAME CONTAINER=$KS_CONTAINER worker-ctl.sh follow (on the node)"
