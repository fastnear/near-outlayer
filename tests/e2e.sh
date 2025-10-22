#!/bin/bash
# End-to-end test via NEAR contract

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

echo "🧪 End-to-End Test - NEAR Contract Flow"
echo "========================================"
echo ""

# Configuration
CONTRACT_ID="${CONTRACT_ID:-c5.offchainvm.testnet}"
CALLER_ACCOUNT="${CALLER_ACCOUNT:-offchainvm.testnet}"
PAYMENT="${PAYMENT:-0.1}"

echo "📝 Configuration:"
echo "  Contract: $CONTRACT_ID"
echo "  Caller: $CALLER_ACCOUNT"
echo "  Payment: $PAYMENT NEAR"
echo "  Repo: https://github.com/zavodil/random-ark"
echo "  Commit: main"
echo ""

# Check prerequisites
echo "🔍 Checking prerequisites..."

# Check if NEAR CLI is available
if ! command -v near &> /dev/null; then
    echo "❌ NEAR CLI not found. Install with: npm install -g near-cli"
    exit 1
fi
echo "✓ NEAR CLI available"

# Check if coordinator is running
if ! curl -s http://localhost:8080/health > /dev/null 2>&1; then
    echo "⚠️  Warning: Coordinator not responding at http://localhost:8080"
    echo "   Make sure coordinator is running: cd coordinator && cargo run"
fi

# Check if worker is configured
if [ ! -f "$PROJECT_ROOT/worker/.env" ]; then
    echo "⚠️  Warning: Worker .env not found"
    echo "   Copy from .env.example and configure: cd worker && cp .env.example .env"
fi

echo ""
echo "🚀 Sending execution request to contract..."
echo ""

near contract call-function as-transaction "$CONTRACT_ID" request_execution json-args \
'{
  "code_source": {
    "repo": "https://github.com/zavodil/random-ark",
    "commit": "main",
    "build_target": "wasm32-wasip1"
  },
  "input_data": "{\"min\": 100, \"max\": 5000}",
  "resource_limits": {
    "max_instructions": 10000000000,
    "max_memory_mb": 128,
    "max_execution_seconds": 60
  }
}' \
prepaid-gas '300.0 Tgas' \
attached-deposit "$PAYMENT NEAR" \
sign-as "$CALLER_ACCOUNT" \
network-config testnet \
sign-with-keychain \
send

echo ""
echo "✅ Transaction sent!"
echo ""
echo "📊 Next steps:"
echo "  1. Check worker logs for execution progress"
echo "  2. View result in NEAR Explorer"
echo "  3. Query contract state:"
echo "     near contract call-function as-read-only $CONTRACT_ID get_stats json-args '{}' network-config testnet now"
echo ""
