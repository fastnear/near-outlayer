#!/usr/bin/env bash
#
# Test script for register contract
#
# Tests worker key registration flow with simulated TDX attestation
#
# Usage:
#   ./scripts/test_register_contract.sh <register-contract-id> <operator-account-id>
#
# Example:
#   ./scripts/test_register_contract.sh register.outlayer.testnet worker.outlayer.testnet
#

set -euo pipefail

REGISTER_CONTRACT="${1:-register.outlayer.testnet}"
OPERATOR_ACCOUNT="${2:-worker.outlayer.testnet}"

# Colors
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
RED='\033[0;31m'
NC='\033[0m'

echo -e "${BLUE}========================================${NC}"
echo -e "${BLUE}Register Contract Test${NC}"
echo -e "${BLUE}========================================${NC}"
echo ""
echo -e "Register contract: ${GREEN}${REGISTER_CONTRACT}${NC}"
echo -e "Operator account:  ${GREEN}${OPERATOR_ACCOUNT}${NC}"
echo ""

# Step 1: Generate a test keypair
echo -e "${YELLOW}[1/5] Generating test worker keypair...${NC}"

# Generate ed25519 keypair using openssl
TEMP_KEY_FILE=$(mktemp)
openssl genpkey -algorithm ed25519 -out "$TEMP_KEY_FILE" 2>/dev/null

# Extract public key bytes (32 bytes)
PUBLIC_KEY_HEX=$(openssl pkey -in "$TEMP_KEY_FILE" -pubout -outform DER 2>/dev/null | tail -c 32 | xxd -p -c 32)

echo -e "   Public key (hex): ${GREEN}${PUBLIC_KEY_HEX}${NC}"

# Create NEAR-format ed25519 public key (with 0x00 prefix for ed25519 type)
PUBLIC_KEY_NEAR="ed25519:$(echo -n "$PUBLIC_KEY_HEX" | xxd -r -p | base58)"

echo -e "   Public key (NEAR): ${GREEN}${PUBLIC_KEY_NEAR}${NC}"
echo ""

# Step 2: Generate simulated TDX quote
echo -e "${YELLOW}[2/5] Generating simulated TDX quote...${NC}"

# For simulated mode, quote is SHA256(public_key_bytes)
# This matches the simulated attestation generation in worker code
SIMULATED_QUOTE=$(echo -n "$PUBLIC_KEY_HEX" | xxd -r -p | sha256sum | cut -d' ' -f1)

echo -e "   Simulated quote (hex): ${GREEN}${SIMULATED_QUOTE}${NC}"
echo ""

# Step 3: Check if key already registered
echo -e "${YELLOW}[3/5] Checking if key is already registered...${NC}"

ALREADY_REGISTERED=$(near view "$REGISTER_CONTRACT" get_worker_keys \
  "{\"account_id\":\"$OPERATOR_ACCOUNT\"}" \
  --networkId testnet 2>/dev/null | grep -c "$PUBLIC_KEY_NEAR" || echo "0")

if [ "$ALREADY_REGISTERED" -gt 0 ]; then
    echo -e "   ${GREEN}✓${NC} Key already registered for $OPERATOR_ACCOUNT"
    echo -e "   ${YELLOW}Skipping registration test${NC}"
    echo ""
else
    echo -e "   ${BLUE}Key not yet registered${NC}"
    echo ""

    # Step 4: Register worker key
    echo -e "${YELLOW}[4/5] Registering worker key with contract...${NC}"
    echo -e "   ${BLUE}Note: Using simulated attestation (no real TDX)${NC}"
    echo ""

    # Call register_worker_key method
    # This will fail if:
    # - RTMR3 not approved (for real TDX)
    # - Key already registered
    # - Invalid attestation format

    if near call "$REGISTER_CONTRACT" register_worker_key \
      "{
        \"public_key\": \"$PUBLIC_KEY_NEAR\",
        \"tdx_quote_hex\": \"$SIMULATED_QUOTE\",
        \"collateral_json\": null
      }" \
      --accountId "$OPERATOR_ACCOUNT" \
      --gas 300000000000000 \
      --networkId testnet; then

        echo ""
        echo -e "${GREEN}✅ Worker key registered successfully!${NC}"
    else
        echo ""
        echo -e "${RED}❌ Registration failed${NC}"
        echo -e "   Common causes:"
        echo -e "   - RTMR3 not approved in whitelist"
        echo -e "   - Key already registered"
        echo -e "   - Invalid quote format"
        exit 1
    fi
fi

echo ""

# Step 5: Verify registration
echo -e "${YELLOW}[5/5] Verifying key registration...${NC}"

REGISTERED_KEYS=$(near view "$REGISTER_CONTRACT" get_worker_keys \
  "{\"account_id\":\"$OPERATOR_ACCOUNT\"}" \
  --networkId testnet 2>/dev/null)

echo -e "   Registered keys for $OPERATOR_ACCOUNT:"
echo "$REGISTERED_KEYS" | sed 's/^/   /'

if echo "$REGISTERED_KEYS" | grep -q "$PUBLIC_KEY_NEAR"; then
    echo ""
    echo -e "${GREEN}✅ Key found in registered keys!${NC}"
else
    echo ""
    echo -e "${RED}❌ Key NOT found in registered keys${NC}"
    exit 1
fi

echo ""
echo -e "${BLUE}========================================${NC}"
echo -e "${GREEN}✅ All tests passed!${NC}"
echo -e "${BLUE}========================================${NC}"
echo ""

# Cleanup
rm -f "$TEMP_KEY_FILE"

# Print summary
echo -e "${YELLOW}Summary:${NC}"
echo -e "   Register contract: $REGISTER_CONTRACT"
echo -e "   Operator account:  $OPERATOR_ACCOUNT"
echo -e "   Test public key:   $PUBLIC_KEY_NEAR"
echo -e "   Status: ${GREEN}REGISTERED${NC}"
echo ""

echo -e "${YELLOW}Next steps:${NC}"
echo -e "   1. Deploy worker with REGISTER_CONTRACT_ID=$REGISTER_CONTRACT"
echo -e "   2. Worker will auto-generate and register its own key on startup"
echo -e "   3. Check worker logs for registration status"
echo ""
