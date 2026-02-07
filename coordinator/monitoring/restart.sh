#!/bin/bash
# Restart monitoring stack after git pull
# Usage: ./restart.sh

set -e
cd "$(dirname "$0")"

echo "Rebuilding and restarting monitoring..."
docker compose up -d --build

echo "Done. Logs: docker compose logs -f"
