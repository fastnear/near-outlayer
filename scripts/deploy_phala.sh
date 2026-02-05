#!/usr/bin/env bash
#
# Automated deployment to Phala Cloud with RTMR3 whitelisting and DAO voting
#
# Usage:
#   ./scripts/deploy_phala.sh keystore [testnet|mainnet] [instance-name]
#   ./scripts/deploy_phala.sh worker [testnet|mainnet] [instance-name]
#
# Examples:
#   ./scripts/deploy_phala.sh keystore testnet            # creates outlayer-testnet-keystore
#   ./scripts/deploy_phala.sh keystore testnet keystore2  # creates outlayer-testnet-keystore2
#   ./scripts/deploy_phala.sh worker testnet              # creates outlayer-testnet-worker
#   ./scripts/deploy_phala.sh worker testnet worker2      # creates outlayer-testnet-worker2
#

set -euo pipefail

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
NC='\033[0m'

# Check arguments
if [ "$#" -lt 1 ] || [ "$#" -gt 3 ]; then
    echo -e "${RED}Usage: $0 <keystore|worker> [testnet|mainnet] [instance-name]${NC}"
    exit 1
fi

COMPONENT="$1"
NETWORK="${2:-testnet}"
INSTANCE_NAME="${3:-}"

# Set account suffix based on network
if [ "$NETWORK" = "mainnet" ]; then
    ACCOUNT_SUFFIX="near"
else
    ACCOUNT_SUFFIX="testnet"
fi

# Validate component
if [[ "$COMPONENT" != "keystore" && "$COMPONENT" != "worker" ]]; then
    echo -e "${RED}Error: Component must be 'keystore' or 'worker'${NC}"
    exit 1
fi

# Configuration based on component and network
case "$COMPONENT" in
    keystore)
        KEYSTORE_SUFFIX="${INSTANCE_NAME:-keystore}"
        CVM_NAME="outlayer-${NETWORK}-${KEYSTORE_SUFFIX}"
        COMPOSE_FILE="docker-compose.keystore-phala.yml"
        ENV_FILE=".env.${NETWORK}-keystore-phala"
        DAO_CONTRACT="dao.outlayer.${ACCOUNT_SUFFIX}"
        SIGNER_ACCOUNT="owner.outlayer.${ACCOUNT_SUFFIX}"
        VOTER_ACCOUNT="zavodil.${ACCOUNT_SUFFIX}"
        BUILD_SCRIPT="./scripts/build_and_push_keystore_tee.sh"
        BUILD_ARGS="zavodil latest"
        ;;
    worker)
        WORKER_SUFFIX="${INSTANCE_NAME:-worker}"
        CVM_NAME="outlayer-${NETWORK}-${WORKER_SUFFIX}"
        COMPOSE_FILE="docker-compose.phala.yml"
        ENV_FILE=".env.${NETWORK}-worker-phala"
        DAO_CONTRACT="worker.outlayer.${ACCOUNT_SUFFIX}"
        SIGNER_ACCOUNT="owner.outlayer.${ACCOUNT_SUFFIX}"
        VOTER_ACCOUNT="zavodil.${ACCOUNT_SUFFIX}"
        BUILD_SCRIPT="./scripts/build_and_push_phala.sh"
        BUILD_ARGS="zavodil latest worker"
        ;;
esac


cd "$(dirname "$0")/.."

echo -e "${CYAN}========================================${NC}"
echo -e "${CYAN}🚀 Phala Deployment: ${COMPONENT} (${NETWORK})${NC}"
echo -e "${CYAN}========================================${NC}"
echo ""

# Step 1: Build and push Docker image
echo -e "${YELLOW}[1/7] Building and pushing Docker image...${NC}"
$BUILD_SCRIPT $BUILD_ARGS

# Step 2: Clear phala config (required for new deployments)
echo -e "${YELLOW}[2/7] Clearing Phala config...${NC}"
PHALA_CONFIG="docker/.phala/config"
if [ -f "$PHALA_CONFIG" ]; then
    echo "{}" > "$PHALA_CONFIG"
    echo -e "${GREEN}✓ Cleared $PHALA_CONFIG${NC}"
else
    mkdir -p docker/.phala
    echo "{}" > "$PHALA_CONFIG"
    echo -e "${GREEN}✓ Created empty $PHALA_CONFIG${NC}"
fi

# Step 3: Check if CVM exists and delete if needed
echo -e "${YELLOW}[3/7] Checking existing CVM...${NC}"
if phala cvms get "$CVM_NAME" --json 2>/dev/null | jq -e '.success' > /dev/null 2>&1; then
    echo -e "${YELLOW}Existing CVM found, deleting...${NC}"
    phala cvms delete "$CVM_NAME" --yes 2>/dev/null || true
    echo -e "${GREEN}✓ Old CVM deleted${NC}"
    sleep 5
else
    echo -e "${GREEN}✓ No existing CVM found${NC}"
fi

# Step 4: Deploy to Phala
echo -e "${YELLOW}[4/7] Deploying to Phala Cloud...${NC}"
cd docker

phala deploy \
    --name "$CVM_NAME" \
    --compose "$COMPOSE_FILE" \
    -e "$ENV_FILE" \
    --vcpu 2 \
    --memory 2G \
    --disk-size 1G \
    --kms-id phala-prod10

cd ..
echo -e "${GREEN}✓ Deployment initiated${NC}"

# Step 5: Wait for CVM to be running and get RTMR3
echo -e "${YELLOW}[5/7] Waiting for CVM to start and getting RTMR3...${NC}"

MAX_ATTEMPTS=60
ATTEMPT=0
RTMR3=""

while [ $ATTEMPT -lt $MAX_ATTEMPTS ]; do
    ATTEMPT=$((ATTEMPT + 1))
    echo -n "."

    # Check if CVM is running
    STATUS=$(phala cvms get "$CVM_NAME" --json 2>/dev/null | jq -r '.status // "unknown"' 2>/dev/null || echo "unknown")

    if [ "$STATUS" = "running" ]; then
        echo ""
        echo -e "${GREEN}✓ CVM is running${NC}"

        # Get attestation with RTMR3
        sleep 10  # Wait for attestation to be ready
        RTMR3=$(phala cvms attestation "$CVM_NAME" --json 2>/dev/null | jq -r '.tcb_info.rtmr3 // empty' 2>/dev/null || echo "")

        if [ -n "$RTMR3" ]; then
            echo -e "${GREEN}✓ Got RTMR3: ${RTMR3}${NC}"
            break
        fi
    fi

    sleep 5
done

if [ -z "$RTMR3" ]; then
    echo ""
    echo -e "${RED}Error: Failed to get RTMR3 after $MAX_ATTEMPTS attempts${NC}"
    exit 1
fi

# Step 6: Add RTMR3 to DAO (if not already approved)
echo -e "${YELLOW}[6/7] Checking if RTMR3 is already approved...${NC}"

RTMR3_APPROVED=$(near contract call-function as-read-only "$DAO_CONTRACT" is_rtmr3_approved \
    json-args "{\"rtmr3\": \"$RTMR3\"}" \
    network-config "$NETWORK" now 2>/dev/null | grep -o 'true\|false' | head -1 || echo "false")

if [ "$RTMR3_APPROVED" = "true" ]; then
    echo -e "${GREEN}✓ RTMR3 already approved, skipping add_approved_rtmr3${NC}"
else
    echo -e "${YELLOW}RTMR3 not approved, adding to DAO contract...${NC}"

    # If this is an additional instance (has INSTANCE_NAME), don't clear others
    if [ -n "$INSTANCE_NAME" ]; then
        CLEAR_OTHERS="false"
    else
        CLEAR_OTHERS="true"
    fi

    near contract call-function as-transaction "$DAO_CONTRACT" add_approved_rtmr3 \
        json-args "{\"rtmr3\": \"$RTMR3\", \"clear_others\": $CLEAR_OTHERS}" \
        prepaid-gas '30.0 Tgas' \
        attached-deposit '0 NEAR' \
        sign-as "$SIGNER_ACCOUNT" \
        network-config "$NETWORK" \
        sign-with-keychain send

    echo -e "${GREEN}✓ RTMR3 added to DAO${NC}"
fi

# Step 7: Restart CVM (and wait for proposal if keystore)
echo -e "${YELLOW}[7/7] Restarting CVM...${NC}"

phala cvms restart "$CVM_NAME"

# For keystore: wait for proposal and vote
if [ "$COMPONENT" = "keystore" ]; then
    echo "Waiting for CVM to restart and create proposal..."
    sleep 30

    # Get proposal ID from logs
    MAX_LOG_ATTEMPTS=30
    LOG_ATTEMPT=0
    PROPOSAL_ID=""

    while [ $LOG_ATTEMPT -lt $MAX_LOG_ATTEMPTS ]; do
        LOG_ATTEMPT=$((LOG_ATTEMPT + 1))
        echo -n "."

        # Get logs and decode base64, search for proposal ID
        PROPOSAL_ID=$(phala cvms logs "$CVM_NAME" --tail 200 2>/dev/null | \
            jq -r '.message // empty' 2>/dev/null | \
            base64 -d 2>/dev/null | \
            grep -oP 'Proposal ID: \K\d+' 2>/dev/null | \
            tail -1 || echo "")

        if [ -n "$PROPOSAL_ID" ]; then
            echo ""
            echo -e "${GREEN}✓ Found Proposal ID: ${PROPOSAL_ID}${NC}"
            break
        fi

        sleep 5
    done

    if [ -z "$PROPOSAL_ID" ]; then
        echo ""
        echo -e "${RED}Error: Could not find proposal ID in logs${NC}"
        echo -e "${YELLOW}Check logs manually: phala cvms logs $CVM_NAME${NC}"
        exit 1
    fi

    # Vote on proposal
    echo -e "${YELLOW}Voting on proposal #${PROPOSAL_ID}...${NC}"

    near contract call-function as-transaction "$DAO_CONTRACT" vote \
        json-args "{\"proposal_id\": $PROPOSAL_ID, \"approve\": true}" \
        prepaid-gas '100.0 Tgas' \
        attached-deposit '0 NEAR' \
        sign-as "$VOTER_ACCOUNT" \
        network-config "$NETWORK" \
        sign-with-keychain send

    echo -e "${GREEN}✓ Voted on proposal #${PROPOSAL_ID}${NC}"
else
    echo -e "${GREEN}✓ CVM restarted (no proposal needed for worker)${NC}"
fi

# Done
echo ""
echo -e "${CYAN}========================================${NC}"
echo -e "${CYAN}✅ Deployment Complete!${NC}"
echo -e "${CYAN}========================================${NC}"
echo ""
echo -e "CVM Name: ${GREEN}${CVM_NAME}${NC}"
echo -e "RTMR3:    ${GREEN}${RTMR3}${NC}"
if [ "$COMPONENT" = "keystore" ]; then
    echo -e "Proposal: ${GREEN}${PROPOSAL_ID}${NC}"
fi
echo ""
echo "Monitor logs:"
echo "  phala cvms logs $CVM_NAME --follow"
echo ""
echo "Get CVM details:"
echo "  phala cvms get $CVM_NAME"
echo ""
