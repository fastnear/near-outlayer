#!/bin/bash
set -e

# Local worker runner - for development and testing
#
# Usage:
#   ./scripts/run_worker_local.sh [env-file-path]
#
# Examples:
#   ./scripts/run_worker_local.sh                                                    # Uses worker/.env
#   ./scripts/run_worker_local.sh docker/.env.testnet-worker-phala                  # Relative path
#   ./scripts/run_worker_local.sh /Users/alice/projects/near-offshore/docker/.env  # Absolute path

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Navigate to project root
cd "$(dirname "$0")/.."

# Determine env file to use
if [ $# -eq 0 ]; then
    # No argument - use worker/.env
    ENV_FILE="worker/.env"
    echo -e "${YELLOW}No env file specified, using default: worker/.env${NC}"
elif [[ "$1" = /* ]]; then
    # Absolute path provided
    ENV_FILE="$1"
    echo -e "${YELLOW}Using env file (absolute): $ENV_FILE${NC}"
else
    # Relative path provided
    ENV_FILE="$1"
    echo -e "${YELLOW}Using env file (relative): $ENV_FILE${NC}"
fi

# Check if env file exists
if [ ! -f "$ENV_FILE" ]; then
    echo -e "${RED}Error: Env file not found: $ENV_FILE${NC}"
    echo ""
    echo "Available env files:"
    find . -name "*.env*" -o -name ".env*" | grep -v node_modules | sort
    exit 1
fi

echo -e "${GREEN}✅ Found env file: $ENV_FILE${NC}"
echo ""

# Check if worker is compiled
if [ ! -f "target/release/offchainvm-worker" ]; then
    echo -e "${YELLOW}Worker binary not found, building...${NC}"
    cd worker
    env SQLX_OFFLINE=true cargo build --release
    cd ..
    echo -e "${GREEN}✅ Worker built successfully${NC}"
    echo ""
fi

# Show key environment variables (without sensitive data)
echo -e "${YELLOW}Configuration:${NC}"
echo "  ENV_FILE: $ENV_FILE"

# Extract and show non-sensitive config
if command -v grep &> /dev/null; then
    echo "  API_BASE_URL: $(grep '^API_BASE_URL=' "$ENV_FILE" | cut -d'=' -f2- || echo 'not set')"
    echo "  OFFCHAINVM_CONTRACT_ID: $(grep '^OFFCHAINVM_CONTRACT_ID=' "$ENV_FILE" | cut -d'=' -f2- || echo 'not set')"
    echo "  OPERATOR_ACCOUNT_ID: $(grep '^OPERATOR_ACCOUNT_ID=' "$ENV_FILE" | cut -d'=' -f2- || echo 'not set')"
    echo "  USE_TEE_REGISTRATION: $(grep '^USE_TEE_REGISTRATION=' "$ENV_FILE" | cut -d'=' -f2- || echo 'not set')"
    echo "  TEE_MODE: $(grep '^TEE_MODE=' "$ENV_FILE" | cut -d'=' -f2- || echo 'not set')"
    echo "  COMPILATION_ENABLED: $(grep '^COMPILATION_ENABLED=' "$ENV_FILE" | cut -d'=' -f2- || echo 'not set')"
    echo "  EXECUTION_ENABLED: $(grep '^EXECUTION_ENABLED=' "$ENV_FILE" | cut -d'=' -f2- || echo 'not set')"
fi
echo ""

# Load env file
echo -e "${GREEN}🚀 Starting worker...${NC}"
echo ""

# Export all variables from env file
set -a
source "$ENV_FILE"
set +a

# Run worker
cd worker
cargo run --release

# Note: If worker exits, script will exit with same code
