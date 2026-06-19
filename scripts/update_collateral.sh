#!/bin/bash
# Update a register-contract collateral SLOT (Phase-1 multi-FMSPC support).
#
# The register-contract (worker.outlayer.<net>) now holds up to MAX_COLLATERALS collaterals,
# one per platform/FMSPC. register_worker_key reads the FMSPC out of the worker's quote and
# verifies ONLY the slot whose collateral matches it (a single dcap-qvl verify, regardless of
# how many slots are cached — so adding slots never raises the per-registration gas).
# Slots:   0 = Phala         (FMSPC 20a06f000000)
#          1 = self-hosted TDX (FMSPC B0C06F000000)
# Updating one slot does NOT delete the others (mixed fleet coexists).
#
# The collateral is the Intel DCAP verification material (TCB info, QE identity, PCK CRLs/certs)
# keyed by the platform's FMSPC. It is **platform-specific, NOT network-specific** — the SAME file
# is used for testnet AND mainnet (it certifies the TDX hardware/TCB, not the NEAR network); just
# cache it on each network's contract. It IS time-sensitive: regenerate when Intel's TCB/CRLs
# update or registration fails with a TCB/collateral error (e.g. status != UpToDate).
#
# Generate the collateral JSON per platform FIRST:
#   - Phala:       from the Phala PCCS (as before).
#   - self-hosted: ON THE NODE, as the `outlayer` user, with the patched dcap-qvl (v0.3.12 — dumps
#     QuoteCollateralV3 + accepts the self-signed local PCCS). `--hex` is a FLAG; the arg is a quote
#     FILE (not the hex string). A reusable platform quote is saved at /home/outlayer/platform-quote.hex
#     (any TDX quote from THIS platform works — the collateral is per-FMSPC, not per-quote):
#       ssh root@173.237.9.76
#       su - outlayer -c 'cd /tmp && PCCS_URL=https://localhost:8081 \
#         /home/outlayer/dcap-qvl/cli/target/release/dcap-qvl verify --hex /home/outlayer/platform-quote.hex'
#       # prints "Quote verified" (status UpToDate) + writes /tmp/our_collateral.json
#       # then from your laptop:  scp root@173.237.9.76:/tmp/our_collateral.json ./our_collateral.json
#     (If platform-quote.hex is lost, extract any worker's quote from its app_cert ext OID
#      1.3.6.1.4.1.62397.1.8; the FMSPC must be B0C06F000000.)
#
# Usage: ./scripts/update_collateral.sh <collateral.json> <index> [network] [contract]
#   ./scripts/update_collateral.sh our_collateral.json 1 testnet        # self-hosted TDX -> slot 1, testnet
#   ./scripts/update_collateral.sh our_collateral.json 1 mainnet        # SAME file -> slot 1, mainnet
#   ./scripts/update_collateral.sh phala_collateral.json 0 testnet      # Phala -> slot 0
set -euo pipefail

COLLATERAL_FILE="${1:?usage: update_collateral.sh <collateral.json> <index: 0=Phala | 1=self-hosted> [network] [contract]}"
INDEX="${2:?need slot index: 0=Phala (20a06f000000), 1=self-hosted TDX (B0C06F000000)}"
NETWORK="${3:-testnet}"
if [ "$NETWORK" = "mainnet" ]; then SUFFIX="near"; else SUFFIX="testnet"; fi
CONTRACT="${4:-worker.outlayer.$SUFFIX}"
OWNER="owner.outlayer.$SUFFIX"

[ -f "$COLLATERAL_FILE" ] || { echo "collateral file not found: $COLLATERAL_FILE" >&2; exit 1; }

# Contract signature: update_collateral(collateral: String, index: u32)
# -> `collateral` is the QuoteCollateralV3 JSON passed AS A STRING; `index` is the slot.
ARGS=$(jq -nc --arg c "$(jq -c . "$COLLATERAL_FILE")" --argjson i "$INDEX" '{collateral: $c, index: $i}')

echo "update_collateral on $CONTRACT  slot=$INDEX  signer=$OWNER  network=$NETWORK"
near contract call-function as-transaction "$CONTRACT" update_collateral \
  json-args "$ARGS" \
  prepaid-gas '300.0 Tgas' \
  attached-deposit '0 NEAR' \
  sign-as "$OWNER" \
  network-config "$NETWORK" \
  sign-with-legacy-keychain \
  send
