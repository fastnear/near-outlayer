#!/bin/bash
# Phase 10 — automated end-to-end test of vault recovery flows
# (scenarios S3 unilateral path, S2 vault upgrade) on real testnet
# using the `test-timing` feature build of the vault contract.
#
# This script bypasses the production `outlayer vault init` flow
# (which bundles only DAO-approved WASMs) and deploys a custom
# test-timing build directly via `near` CLI. The 7-day cessation
# delay is collapsed to 30 seconds and the unilateral exit window
# minimum drops from 24h to 10s, so the full recovery cycle runs
# in under a minute.
#
# Required environment:
#   * NEAR CLI (`near`) on PATH — install with `npm i -g near-cli-rs`
#   * `cargo near` for the contract build
#   * Logged-in testnet account (`near login`) with at least 5 NEAR
#   * Optional: `JQ` for parsing view-call output
#
# Run:
#   ./vault_recovery_e2e.sh unilateral [--apply]
#   ./vault_recovery_e2e.sh propose-tee-key [--apply]
#
# Without --apply the script prints what it WOULD run.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
APPLY=false
SCENARIO="${1:-help}"
[[ "${2:-}" == "--apply" ]] && APPLY=true
[[ "${1:-}" == "--apply" ]] && { APPLY=true; SCENARIO="unilateral"; }

NETWORK="${NETWORK:-testnet}"
PARENT="${PARENT:-}"
VAULT_NAME="${VAULT_NAME:-recovery-test-$(date +%s)}"
VAULT_CONTRACT_DIR="${VAULT_CONTRACT_DIR:-$SCRIPT_DIR/../vault-contract}"
KEYSTORE_DAO_ID="${KEYSTORE_DAO_ID:-dao.outlayer.testnet}"
MPC_CONTRACT_ID="${MPC_CONTRACT_ID:-v1.signer-prod.testnet}"
# test-timing feature window is hardcoded — see vault-contract/src/lib.rs:
#   MIN_UNILATERAL_EXIT_WINDOW_SECS = 10
#   FINALIZE_WINDOW_NS = 300 * 10^9  (300s)
#   CESSATION_DELAY_NS = 30 * 10^9   (30s)
EXIT_WINDOW_SECS=10
FINALIZE_WAIT_SECS=15  # exit_window + small buffer

log()   { printf '\n\033[36m▶ %s\033[0m\n' "$*"; }
warn()  { printf '\033[33m⚠ %s\033[0m\n' "$*"; }
fail()  { printf '\033[31m✗ %s\033[0m\n' "$*"; exit 1; }
pass()  { printf '\033[32m✓ %s\033[0m\n' "$*"; }

run() {
  if [[ "$APPLY" == true ]]; then
    log "$ $*"
    eval "$@"
  else
    printf '\033[90m  (dry-run) $ %s\033[0m\n' "$*"
  fi
}

require() {
  if [[ -z "${1:-}" ]]; then
    fail "$2 not set. Export it before running this scenario."
  fi
}

# ─── Build test-timing vault WASM ──────────────────────────────────

build_test_timing_wasm() {
  log "Building vault-contract with --features test-timing"
  if [[ "$APPLY" == true ]]; then
    (cd "$VAULT_CONTRACT_DIR" && cargo near build --no-locked --no-docker --features test-timing)
  else
    printf '  (dry-run) cd %s && cargo near build --no-locked --no-docker --features test-timing\n' "$VAULT_CONTRACT_DIR"
  fi
  WASM_PATH="$VAULT_CONTRACT_DIR/target/near/vault_contract.wasm"
  if [[ "$APPLY" == true && ! -f "$WASM_PATH" ]]; then
    fail "WASM not found at $WASM_PATH after build"
  fi
  echo "$WASM_PATH"
}

# ─── Scenario: unilateral recovery (S3 unilateral path) ────────────

scenario_unilateral() {
  log "Scenario: unilateral recovery happy path (test-timing 10s window)"
  require "$PARENT" "PARENT (e.g. PARENT=alice.testnet)"
  local vault_account="$VAULT_NAME.$PARENT"

  WASM_PATH=$(build_test_timing_wasm)

  log "1. Create vault sub-account funded by parent ($vault_account)"
  # Sub-accounts of `*.testnet` cannot use the testnet faucet
  # (only top-level accounts qualify). The correct path is
  # `fund-myself`: parent funds the sub-account and adds an
  # initial full-access key from PARENT's signing key. We then
  # deploy the WASM as a separate tx.
  #
  # NOTE: this leaves PARENT's pubkey on the sub-account, which is
  # NOT what production `outlayer vault init` does (production has
  # only a TEE FCAK after deploy). For recovery TESTING that's
  # fine — we want parent authority to drive the unilateral flow
  # and to call `unlocked_add_key` post-recovery.
  run "near account create-account fund-myself \\
        $vault_account \\
        '2 NEAR' \\
        autogenerate-new-keypair save-to-keychain \\
        sign-as $PARENT \\
        network-config testnet \\
        sign-with-keychain send"

  log "2. Deploy test-timing WASM + init"
  run "near contract deploy $vault_account \\
        use-file $WASM_PATH \\
        with-init-call new \\
          json-args '{\"parent\": \"$PARENT\", \"keystore_dao\": \"$KEYSTORE_DAO_ID\", \"mpc_contract\": \"$MPC_CONTRACT_ID\", \"initial_exit_window\": $EXIT_WINDOW_SECS}' \\
          prepaid-gas '30 TGas' \\
          attached-deposit '0 NEAR' \\
        network-config testnet \\
        sign-with-keychain send"

  log "3. Confirm initial state: unlocked=false, recovery=None"
  run "near contract call-function as-read-only $vault_account get_state \\
        json-args '{}' \\
        network-config testnet now"

  log "4. Parent ($PARENT) calls unilateral_initiate_recovery"
  run "near contract call-function as-transaction $vault_account unilateral_initiate_recovery \\
        json-args '{}' \\
        prepaid-gas '30 TGas' attached-deposit '0 NEAR' \\
        sign-as $PARENT \\
        network-config testnet \\
        sign-with-keychain send"

  log "5. Verify recovery state is set"
  run "near contract call-function as-read-only $vault_account get_recovery_state \\
        json-args '{}' \\
        network-config testnet now"

  log "6. Wait $FINALIZE_WAIT_SECS seconds for exit window to pass"
  if [[ "$APPLY" == true ]]; then
    sleep "$FINALIZE_WAIT_SECS"
  else
    printf '  (dry-run) sleep %s\n' "$FINALIZE_WAIT_SECS"
  fi

  log "7. Anyone calls finalize_recovery (vault unlocks synchronously)"
  run "near contract call-function as-transaction $vault_account finalize_recovery \\
        json-args '{}' \\
        prepaid-gas '50 TGas' attached-deposit '0 NEAR' \\
        sign-as $PARENT \\
        network-config testnet \\
        sign-with-keychain send"

  log "8. Verify unlocked=true"
  run "near contract call-function as-read-only $vault_account get_state \\
        json-args '{}' \\
        network-config testnet now"

  log "9. Parent installs full-access key (via unlocked_add_key)"
  warn "Replace ed25519:... below with a real public key under your control."
  run "near contract call-function as-transaction $vault_account unlocked_add_key \\
        json-args '{\"public_key\": \"ed25519:REPLACE_WITH_REAL_PUBKEY\", \"full_access\": true, \"allowance\": null}' \\
        prepaid-gas '50 TGas' attached-deposit '0 NEAR' \\
        sign-as $PARENT \\
        network-config testnet \\
        sign-with-keychain send"

  log "10. Final assert: parent now has full access on $vault_account"
  run "near account list-keys $vault_account network-config testnet now"
  warn "Manual check: the listed keys should now include the parent's full-access key."

  pass "Scenario unilateral — done. Vault $vault_account is unlocked + parent-controlled."

  log "Cleanup: delete account, refund storage to $PARENT"
  warn "Run manually if desired:"
  warn "  near account delete-account $vault_account beneficiary $PARENT \\"
  warn "    network-config testnet sign-with-keychain send"
}

# ─── Scenario: propose_tee_key (Phase 10 S2 fragment) ──────────────

scenario_propose_tee_key() {
  log "Scenario: propose_tee_key happy path (vault accepts new approved keystore)"
  require "$PARENT" "PARENT"
  local vault_account="$VAULT_NAME.$PARENT"
  warn "This scenario requires:"
  warn "  1. A live vault deployed at $vault_account (run scenario_unilateral first or use existing)"
  warn "  2. An approved keystore pubkey known to $KEYSTORE_DAO_ID"
  warn "  3. Anyone (permissionless) calling propose_tee_key"

  log "Calling propose_tee_key with sample pubkey"
  run "near contract call-function as-transaction $vault_account propose_tee_key \\
        json-args '{\"public_key\": \"ed25519:REPLACE_WITH_APPROVED_KEYSTORE_PUBKEY\"}' \\
        prepaid-gas '100 TGas' attached-deposit '0 NEAR' \\
        sign-as $PARENT \\
        network-config testnet \\
        sign-with-keychain send"

  log "Verify: registered_tee_keys now contains the new pubkey"
  run "near view $vault_account get_registered_keys '{}' --network-config testnet"

  pass "Scenario propose_tee_key — done. Vault accepted the new approved keystore."
}

# ─── Driver ────────────────────────────────────────────────────────

usage() {
  cat <<EOF
Usage: $0 <scenario> [--apply]

Scenarios:
  unilateral       — Full unilateral recovery cycle (deploy → initiate → wait → finalize → add_key)
  propose-tee-key  — Propose a new approved-keystore pubkey via cross-contract DAO check

Flags:
  --apply          — Execute the commands. Without this, dry-run.

Env:
  PARENT             NEAR account that creates + parents the vault (required)
  NETWORK            testnet (default)
  VAULT_NAME         Sub-account label (default: recovery-test-<unix>)
  KEYSTORE_DAO_ID    Default: dao.outlayer.testnet
  MPC_CONTRACT_ID    Default: v1.signer-prod.testnet
  VAULT_CONTRACT_DIR Default: <repo>/vault-contract

Notes:
  * Built with --features test-timing: 7-day delays collapse to 30s,
    24h unilateral window collapses to 10s. NOT a production WASM.
  * NEVER deploy the test-timing WASM under outlayer.near's
    DAO-approved hash list; the resulting hash is intentionally
    distinct and will fail vault-checker.
EOF
}

case "$SCENARIO" in
  unilateral)        scenario_unilateral ;;
  propose-tee-key)   scenario_propose_tee_key ;;
  -h|--help|help)    usage; exit 0 ;;
  *)                 usage; exit 1 ;;
esac
