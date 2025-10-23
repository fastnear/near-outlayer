#!/bin/bash

# NEAR OutLayer - Quick Start Script
# This script helps you restart all services with new features

set -e

echo "🚀 NEAR OutLayer Quick Start"
echo "=============================="
echo ""

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Get project root
PROJECT_ROOT="$(cd "$(dirname "$0")" && pwd)"

echo "📍 Project root: $PROJECT_ROOT"
echo ""

# Step 1: Start infrastructure
echo -e "${GREEN}[1/5]${NC} Starting PostgreSQL and Redis..."
cd "$PROJECT_ROOT/coordinator"
docker-compose up -d postgres redis
sleep 3
docker-compose ps

echo ""
echo -e "${GREEN}✓${NC} Infrastructure started"
echo ""

# Step 2: Rebuild Coordinator
echo -e "${GREEN}[2/5]${NC} Rebuilding Coordinator..."
cd "$PROJECT_ROOT/coordinator"

# Stop old coordinator if running
docker-compose stop coordinator 2>/dev/null || true

# Rebuild
echo "   Building Coordinator (this may take a while)..."
env SQLX_OFFLINE=true cargo build --release

if [ $? -eq 0 ]; then
    echo -e "${GREEN}✓${NC} Coordinator built successfully"
else
    echo -e "${RED}✗${NC} Coordinator build failed"
    exit 1
fi

# Start coordinator in background
echo "   Starting Coordinator on :8080..."
./target/release/offchainvm-coordinator > /tmp/coordinator.log 2>&1 &
COORDINATOR_PID=$!
echo $COORDINATOR_PID > /tmp/coordinator.pid
sleep 2

if ps -p $COORDINATOR_PID > /dev/null; then
    echo -e "${GREEN}✓${NC} Coordinator started (PID: $COORDINATOR_PID)"
else
    echo -e "${RED}✗${NC} Coordinator failed to start. Check /tmp/coordinator.log"
    exit 1
fi

echo ""

# Step 3: Rebuild Worker
echo -e "${GREEN}[3/5]${NC} Rebuilding Worker..."
cd "$PROJECT_ROOT/worker"

# Stop old worker if running
pkill -f offchainvm-worker 2>/dev/null || true

# Rebuild
echo "   Building Worker..."
cargo build --release

if [ $? -eq 0 ]; then
    echo -e "${GREEN}✓${NC} Worker built successfully"
else
    echo -e "${RED}✗${NC} Worker build failed"
    exit 1
fi

# Start worker in background
echo "   Starting Worker..."
./target/release/offchainvm-worker > /tmp/worker.log 2>&1 &
WORKER_PID=$!
echo $WORKER_PID > /tmp/worker.pid
sleep 2

if ps -p $WORKER_PID > /dev/null; then
    echo -e "${GREEN}✓${NC} Worker started (PID: $WORKER_PID)"
else
    echo -e "${RED}✗${NC} Worker failed to start. Check /tmp/worker.log"
    exit 1
fi

echo ""

# Step 4: Start Keystore (optional)
echo -e "${GREEN}[4/5]${NC} Starting Keystore Worker..."
cd "$PROJECT_ROOT/keystore-worker"

if [ -f "docker-compose.yml" ]; then
    docker-compose up -d
    echo -e "${GREEN}✓${NC} Keystore started on :8081"
else
    echo -e "${YELLOW}⚠${NC}  Keystore docker-compose.yml not found, skipping..."
fi

echo ""

# Step 5: Start Dashboard
echo -e "${GREEN}[5/5]${NC} Starting Dashboard..."
cd "$PROJECT_ROOT/dashboard"

# Check if node_modules exists
if [ ! -d "node_modules" ]; then
    echo "   Installing dependencies..."
    npm install
fi

echo "   Starting Next.js dev server..."
npm run dev > /tmp/dashboard.log 2>&1 &
DASHBOARD_PID=$!
echo $DASHBOARD_PID > /tmp/dashboard.pid

echo -e "${GREEN}✓${NC} Dashboard starting (PID: $DASHBOARD_PID)"
echo ""

# Wait a bit for services to start
echo "⏳ Waiting for services to initialize..."
sleep 5

# Test endpoints
echo ""
echo "🧪 Testing services..."
echo ""

# Test Coordinator
if curl -s http://localhost:8080/health > /dev/null; then
    echo -e "${GREEN}✓${NC} Coordinator: http://localhost:8080 - OK"
else
    echo -e "${RED}✗${NC} Coordinator: http://localhost:8080 - FAILED"
fi

# Test Dashboard (may take longer to start)
if curl -s http://localhost:3000 > /dev/null 2>&1; then
    echo -e "${GREEN}✓${NC} Dashboard: http://localhost:3000 - OK"
else
    echo -e "${YELLOW}⚠${NC}  Dashboard: http://localhost:3000 - Starting... (check in 30s)"
fi

# Test Public API
if curl -s http://localhost:8080/public/stats > /dev/null; then
    echo -e "${GREEN}✓${NC} Public API: http://localhost:8080/public/stats - OK"
else
    echo -e "${YELLOW}⚠${NC}  Public API: http://localhost:8080/public/stats - Check logs"
fi

echo ""
echo "=============================="
echo "🎉 All services started!"
echo "=============================="
echo ""
echo "📊 Dashboards:"
echo "   Web UI:        http://localhost:3000"
echo "   Coordinator:   http://localhost:8080"
echo "   Keystore:      http://localhost:8081"
echo ""
echo "📝 Logs:"
echo "   Coordinator:   tail -f /tmp/coordinator.log"
echo "   Worker:        tail -f /tmp/worker.log"
echo "   Dashboard:     tail -f /tmp/dashboard.log"
echo ""
echo "🛑 Stop services:"
echo "   ./stop-services.sh"
echo ""
echo "📚 Documentation:"
echo "   DEPLOYMENT_GUIDE.md"
echo "   COMPLETED_FEATURES.md"
echo ""
