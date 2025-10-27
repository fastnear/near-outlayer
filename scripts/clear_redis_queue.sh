#!/bin/bash

# Usage: ./clear_redis_queue.sh [testnet|mainnet]
# Clears Redis task queue for the specified network
# Default: mainnet

set -e

NETWORK=${1:-mainnet}

if [ "$NETWORK" != "testnet" ] && [ "$NETWORK" != "mainnet" ]; then
    echo "Error: Invalid network. Use 'testnet' or 'mainnet'"
    exit 1
fi

# Determine script directory
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

# Load environment variables
# For testnet use .env.testnet, for mainnet use .env
if [ "$NETWORK" = "testnet" ]; then
    ENV_FILE="$PROJECT_ROOT/coordinator/.env.testnet"
else
    ENV_FILE="$PROJECT_ROOT/coordinator/.env"
fi

if [ ! -f "$ENV_FILE" ]; then
    echo "Error: Environment file not found: $ENV_FILE"
    exit 1
fi

# Extract Redis password from .env file
REDIS_PASSWORD=$(grep "^REDIS_PASSWORD=" "$ENV_FILE" | cut -d'=' -f2- | tr -d '"' | tr -d "'")
REDIS_TASK_QUEUE=$(grep "^REDIS_TASK_QUEUE=" "$ENV_FILE" | cut -d'=' -f2- | tr -d '"' | tr -d "'")

# Use defaults if not set
REDIS_PASSWORD=${REDIS_PASSWORD:-redis123}
REDIS_TASK_QUEUE=${REDIS_TASK_QUEUE:-offchainvm:tasks:queue}

# Determine container name based on network
if [ "$NETWORK" = "testnet" ]; then
    REDIS_CONTAINER="offchainvm-redis-testnet"
else
    REDIS_CONTAINER="offchainvm-redis-mainnet"
fi

# Check if container is running
if ! docker ps --format '{{.Names}}' | grep -q "^${REDIS_CONTAINER}$"; then
    echo "Error: Redis container '$REDIS_CONTAINER' is not running"
    exit 1
fi

echo "═══════════════════════════════════════════════════════════"
echo "Clearing Redis Task Queue"
echo "═══════════════════════════════════════════════════════════"
echo "Network: $NETWORK"
echo "Container: $REDIS_CONTAINER"
echo "Queue: $REDIS_TASK_QUEUE"
echo ""

# Get queue length before clearing
QUEUE_LENGTH=$(docker exec -i "$REDIS_CONTAINER" redis-cli -a "$REDIS_PASSWORD" --no-auth-warning LLEN "$REDIS_TASK_QUEUE" 2>/dev/null || echo "0")

echo "Current queue length: $QUEUE_LENGTH tasks"
echo ""

if [ "$QUEUE_LENGTH" = "0" ]; then
    echo "✅ Queue is already empty, nothing to clear"
    exit 0
fi

# Confirm before clearing
read -p "Are you sure you want to clear $QUEUE_LENGTH tasks? (yes/no): " CONFIRM

if [ "$CONFIRM" != "yes" ]; then
    echo "Cancelled"
    exit 0
fi

# Clear the queue
docker exec -i "$REDIS_CONTAINER" redis-cli -a "$REDIS_PASSWORD" --no-auth-warning DEL "$REDIS_TASK_QUEUE" > /dev/null

echo ""
echo "✅ Queue cleared successfully!"
echo ""
echo "Note: This only clears the Redis queue. Tasks already in PostgreSQL"
echo "will not be affected. Workers will skip tasks that already have jobs"
echo "in the database."
