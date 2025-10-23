#!/bin/bash

# NEAR OutLayer - Stop All Services

echo "🛑 Stopping NEAR OutLayer services..."
echo ""

# Stop Coordinator
if [ -f "/tmp/coordinator.pid" ]; then
    PID=$(cat /tmp/coordinator.pid)
    if ps -p $PID > /dev/null 2>&1; then
        echo "Stopping Coordinator (PID: $PID)..."
        kill $PID
        rm /tmp/coordinator.pid
    fi
fi

# Stop Worker
if [ -f "/tmp/worker.pid" ]; then
    PID=$(cat /tmp/worker.pid)
    if ps -p $PID > /dev/null 2>&1; then
        echo "Stopping Worker (PID: $PID)..."
        kill $PID
        rm /tmp/worker.pid
    fi
fi

# Stop Dashboard
if [ -f "/tmp/dashboard.pid" ]; then
    PID=$(cat /tmp/dashboard.pid)
    if ps -p $PID > /dev/null 2>&1; then
        echo "Stopping Dashboard (PID: $PID)..."
        kill $PID
        rm /tmp/dashboard.pid
    fi
fi

# Stop Docker services
echo "Stopping Docker services..."
cd "$(dirname "$0")/coordinator"
docker-compose stop coordinator 2>/dev/null || true

cd "$(dirname "$0")/keystore-worker"
docker-compose stop 2>/dev/null || true

echo ""
echo "✓ All services stopped"
echo ""
echo "Note: PostgreSQL and Redis are still running"
echo "To stop them: cd coordinator && docker-compose stop postgres redis"
