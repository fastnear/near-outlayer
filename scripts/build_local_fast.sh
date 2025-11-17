#!/usr/bin/env bash
#
# Fast local build for development (no push, uses local cache only)
#
# Usage:
#   ./scripts/build_local_fast.sh [keystore|worker]
#
# Examples:
#   ./scripts/build_local_fast.sh keystore
#   ./scripts/build_local_fast.sh worker
#

set -euo pipefail

TARGET="${1:-worker}"

# Colors
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

echo -e "${BLUE}Fast local build for ${TARGET}${NC}"
echo ""

# Enable BuildKit
export DOCKER_BUILDKIT=1

if [ "$TARGET" = "keystore" ]; then
    echo -e "${YELLOW}Building keystore (AMD64)...${NC}"
    docker buildx build \
        --platform linux/amd64 \
        -f docker/Dockerfile.keystore-phala \
        -t local/near-outlayer-keystore:dev \
        --load \
        .

    echo ""
    echo -e "${GREEN}✅ Keystore built: local/near-outlayer-keystore:dev${NC}"
    echo "Test with: docker run --rm local/near-outlayer-keystore:dev"

elif [ "$TARGET" = "worker" ]; then
    echo -e "${YELLOW}Building worker (AMD64)...${NC}"
    docker buildx build \
        --platform linux/amd64 \
        -f docker/Dockerfile.worker-phala \
        -t local/near-outlayer-worker:dev \
        --load \
        .

    echo ""
    echo -e "${GREEN}✅ Worker built: local/near-outlayer-worker:dev${NC}"
    echo "Test with: docker run --rm local/near-outlayer-worker:dev"

else
    echo "Unknown target: $TARGET"
    echo "Use: keystore or worker"
    exit 1
fi

echo ""
echo "💡 This is a local-only build (not pushed to Docker Hub)"
echo "   For production builds, use:"
echo "   - ./scripts/build_and_push_keystore.sh"
echo "   - ./scripts/build_and_push_phala.sh"
