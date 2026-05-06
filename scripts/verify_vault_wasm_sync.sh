#!/bin/bash
# Verify all bundled copies of vault_contract.wasm are byte-identical.
#
# Why: the vault contract WASM is bundled in three places:
#   * vault-contract/res/vault_contract.wasm        (canonical source)
#   * outlayer-cli/res/vault_contract.wasm          (CLI bundles for `outlayer vault init`)
#   * dashboard/public/vault_contract.wasm          (dashboard fetches at runtime)
#
# All three MUST hash to the same `Base58CryptoHash` because the customer's
# atomic-deploy tx submits a `is_vault_code_approved(hash)` view-call against
# keystore-DAO before signing. A drift between the three copies surfaces as a
# misleading "code hash NOT approved" error rather than the real "the
# CLI/dashboard you have is stale" cause.
#
# Run from anywhere; uses absolute repo paths.
# Exits 0 if all match, 1 otherwise.

set -euo pipefail

NEAR_OFFSHORE="/Users/alice/projects/near-offshore"
OUTLAYER_CLI="/Users/alice/projects/outlayer-cli"

CANONICAL="$NEAR_OFFSHORE/vault-contract/res/vault_contract.wasm"
COPIES=(
  "$OUTLAYER_CLI/res/vault_contract.wasm"
  "$NEAR_OFFSHORE/dashboard/public/vault_contract.wasm"
)

if [[ ! -f "$CANONICAL" ]]; then
  echo "✗ canonical WASM missing: $CANONICAL" >&2
  echo "  build it first: cd $NEAR_OFFSHORE/vault-contract && ./build.sh" >&2
  exit 1
fi

CANONICAL_HASH=$(shasum -a 256 "$CANONICAL" | awk '{print $1}')
echo "canonical:  $CANONICAL_HASH  $CANONICAL"

mismatch=0
for copy in "${COPIES[@]}"; do
  if [[ ! -f "$copy" ]]; then
    echo "✗ copy missing: $copy" >&2
    mismatch=1
    continue
  fi
  hash=$(shasum -a 256 "$copy" | awk '{print $1}')
  if [[ "$hash" != "$CANONICAL_HASH" ]]; then
    echo "✗ MISMATCH:  $hash  $copy" >&2
    mismatch=1
  else
    echo "✓ matches:   $hash  $copy"
  fi
done

if [[ $mismatch -ne 0 ]]; then
  echo "" >&2
  echo "Vault WASM is OUT OF SYNC across bundled copies." >&2
  echo "Refresh the downstream copies from canonical:" >&2
  for copy in "${COPIES[@]}"; do
    echo "  cp \"$CANONICAL\" \"$copy\"" >&2
  done
  exit 1
fi

echo ""
echo "All vault WASM copies match canonical."
