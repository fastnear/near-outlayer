#!/bin/bash
# Run all tests in sequence

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# Colors
GREEN='\033[0;32m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

echo ""
echo "🧪 Running All Tests"
echo "===================="
echo ""

# Test 1: Unit Tests
echo -e "${BLUE}═══════════════════════════════════════════════════════════${NC}"
echo -e "${BLUE}Test 1/4: Unit Tests${NC}"
echo -e "${BLUE}═══════════════════════════════════════════════════════════${NC}"
echo ""
"$SCRIPT_DIR/unit.sh"
echo ""

# Test 2: Compilation Tests
echo -e "${BLUE}═══════════════════════════════════════════════════════════${NC}"
echo -e "${BLUE}Test 2/4: Compilation Tests${NC}"
echo -e "${BLUE}═══════════════════════════════════════════════════════════${NC}"
echo ""

# Check if Docker is running
if docker info > /dev/null 2>&1; then
    "$SCRIPT_DIR/compilation.sh"
    echo ""
else
    echo "⚠️  Skipping compilation tests - Docker not running"
    echo "   Start Docker and try again"
    echo ""
fi

# Test 3: Integration Tests
echo -e "${BLUE}═══════════════════════════════════════════════════════════${NC}"
echo -e "${BLUE}Test 3/4: Integration Tests${NC}"
echo -e "${BLUE}═══════════════════════════════════════════════════════════${NC}"
echo ""

# Check if coordinator is running
if curl -s http://localhost:8080/health > /dev/null 2>&1; then
    "$SCRIPT_DIR/integration.sh"
    echo ""
else
    echo "⚠️  Skipping integration tests - Coordinator not running"
    echo "   Start with: cd coordinator && cargo run"
    echo ""
fi

# Test 4: Wallet Tests (Mode 1 — Agent)
echo -e "${BLUE}═══════════════════════════════════════════════════════════${NC}"
echo -e "${BLUE}Test 4/6: Wallet Mode 1 — Simple Agent${NC}"
echo -e "${BLUE}═══════════════════════════════════════════════════════════${NC}"
echo ""

if curl -s http://localhost:8080/health > /dev/null 2>&1; then
    "$SCRIPT_DIR/wallet_mode1_agent.sh"
    echo ""
else
    echo "⚠️  Skipping wallet agent tests - Coordinator not running"
    echo "   cd coordinator && cargo run"
    echo ""
fi

# Test 5: Wallet Tests (Mode 2 — Policy)
echo -e "${BLUE}═══════════════════════════════════════════════════════════${NC}"
echo -e "${BLUE}Test 5/6: Wallet Mode 2 — User with Policy${NC}"
echo -e "${BLUE}═══════════════════════════════════════════════════════════${NC}"
echo ""

if curl -s http://localhost:8080/health > /dev/null 2>&1; then
    "$SCRIPT_DIR/wallet_mode2_policy.sh"
    echo ""
else
    echo "⚠️  Skipping wallet policy tests - Coordinator not running"
    echo "   cd coordinator && cargo run"
    echo ""
fi

# Test 6: E2E Tests
echo -e "${BLUE}═══════════════════════════════════════════════════════════${NC}"
echo -e "${BLUE}Test 6/6: End-to-End Tests${NC}"
echo -e "${BLUE}═══════════════════════════════════════════════════════════${NC}"
echo ""
echo "⚠️  E2E tests require manual execution (requires testnet contract)"
echo "   Run manually: $SCRIPT_DIR/e2e.sh"
echo ""

echo "═══════════════════════════════════════════════════════════"
echo -e "${GREEN}✅ Test suite completed!${NC}"
echo "═══════════════════════════════════════════════════════════"
echo ""
