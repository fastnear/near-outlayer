#!/usr/bin/env bash
#
# Build and push NEAR OutLayer Keystore Docker image for Phala Cloud
#
# Usage:
#   ./scripts/build_and_push_keystore.sh <dockerhub-username> <version>
#
# Example:
#   ./scripts/build_and_push_keystore.sh zavodil v1.0.0
#

set -euo pipefail

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Check arguments
if [ $# -lt 2 ]; then
    echo -e "${RED}Error: Missing arguments${NC}"
    echo "Usage: $0 <dockerhub-username> <version>"
    echo "Example: $0 zavodil v1.0.0"
    exit 1
fi

DOCKERHUB_USER="$1"
VERSION="$2"
IMAGE_NAME="near-outlayer-keystore"
FULL_IMAGE="${DOCKERHUB_USER}/${IMAGE_NAME}:${VERSION}"
LATEST_IMAGE="${DOCKERHUB_USER}/${IMAGE_NAME}:latest"

echo -e "${BLUE}========================================${NC}"
echo -e "${BLUE}Building Keystore Docker Image${NC}"
echo -e "${BLUE}========================================${NC}"
echo ""
echo -e "  Image: ${GREEN}${FULL_IMAGE}${NC}"
echo -e "  Latest: ${GREEN}${LATEST_IMAGE}${NC}"
echo ""

# Step 1: Pre-build security check
echo -e "${YELLOW}[1/6] Running pre-build security check...${NC}"
./scripts/inspect_docker_context.sh docker/Dockerfile.keystore-phala keystore-worker
echo ""

# Step 2: Build Docker image for AMD64 (Phala Cloud)
echo -e "${YELLOW}[2/6] Building Docker image for linux/amd64...${NC}"
echo "Using docker buildx with cache for faster builds"

# Enable BuildKit cache
export DOCKER_BUILDKIT=1

docker buildx build \
    --platform linux/amd64 \
    -f docker/Dockerfile.keystore-phala \
    -t "${FULL_IMAGE}" \
    -t "${LATEST_IMAGE}" \
    --cache-from type=registry,ref="${DOCKERHUB_USER}/${IMAGE_NAME}:buildcache" \
    --cache-to type=registry,ref="${DOCKERHUB_USER}/${IMAGE_NAME}:buildcache",mode=max \
    --load \
    .

echo ""
echo "💡 Tip: Subsequent builds will be faster thanks to cache"

echo -e "${GREEN}✓ Build completed${NC}"
echo ""

# Step 3: Test image startup
echo -e "${YELLOW}[3/6] Testing image startup...${NC}"
echo "Starting keystore container for 5 seconds..."

# Create minimal test config
TEST_ENV_FILE=$(mktemp)
cat > "$TEST_ENV_FILE" <<EOF
SERVER_HOST=0.0.0.0
SERVER_PORT=8081
NEAR_NETWORK=testnet
NEAR_RPC_URL=https://rpc.testnet.near.org
OFFCHAINVM_CONTRACT_ID=outlayer.testnet
ALLOWED_WORKER_TOKEN_HASHES=test_hash_12345
TEE_MODE=none
KEYSTORE_MASTER_SECRET=a1b2c3d4e5f6a7b8c9d0e1f2a3b4c5d6e7f8a9b0c1d2e3f4a5b6c7d8e9f0a1b2
EOF

# Start container
CONTAINER_ID=$(docker run -d --env-file "$TEST_ENV_FILE" "${FULL_IMAGE}")

# Wait and check logs
sleep 5
LOGS=$(docker logs "$CONTAINER_ID" 2>&1 || true)

# Stop container
docker stop "$CONTAINER_ID" >/dev/null 2>&1 || true
docker rm "$CONTAINER_ID" >/dev/null 2>&1 || true
rm "$TEST_ENV_FILE"

# Check for startup success
if echo "$LOGS" | grep -q "Keystore worker API server started"; then
    echo -e "${GREEN}✓ Keystore started successfully${NC}"
    echo ""
    echo "Sample logs:"
    echo "$LOGS" | tail -10
else
    echo -e "${RED}✗ Keystore failed to start${NC}"
    echo ""
    echo "Error logs:"
    echo "$LOGS"
    exit 1
fi
echo ""

# Step 4: Verify built image
echo -e "${YELLOW}[4/6] Verifying built image security...${NC}"
./scripts/verify_built_image.sh "${FULL_IMAGE}"
echo ""

# Step 5: Push to Docker Hub
echo -e "${YELLOW}[5/6] Pushing to Docker Hub...${NC}"
echo "Pushing ${FULL_IMAGE}..."
docker push "${FULL_IMAGE}"

echo "Pushing ${LATEST_IMAGE}..."
docker push "${LATEST_IMAGE}"

echo -e "${GREEN}✓ Push completed${NC}"
echo ""

# Step 6: Show next steps
echo -e "${GREEN}========================================${NC}"
echo -e "${GREEN}Build Complete!${NC}"
echo -e "${GREEN}========================================${NC}"
echo ""
echo -e "Docker image: ${GREEN}${FULL_IMAGE}${NC}"
echo -e "Latest tag: ${GREEN}${LATEST_IMAGE}${NC}"
echo ""
echo -e "${BLUE}Next steps for Phala Cloud deployment:${NC}"
echo ""
echo "1. Update docker/.env.keystore-phala with your configuration:"
echo "   cp docker/.env.keystore-phala.example docker/.env.keystore-phala"
echo "   nano docker/.env.keystore-phala"
echo ""
echo "2. Generate and configure authentication tokens:"
echo "   # Generate token"
echo "   TOKEN=\$(openssl rand -hex 32)"
echo "   echo \"Worker token: \$TOKEN\""
echo ""
echo "   # Hash token for keystore config"
echo "   HASH=\$(echo -n \"\$TOKEN\" | sha256sum | cut -d' ' -f1)"
echo "   echo \"Token hash: \$HASH\""
echo ""
echo "   # Add hash to .env.keystore-phala:"
echo "   ALLOWED_WORKER_TOKEN_HASHES=\$HASH"
echo ""
echo "   # Give original token to workers (in .env.phala):"
echo "   KEYSTORE_AUTH_TOKEN=\$TOKEN"
echo ""
echo "3. Generate master secret:"
echo "   MASTER=\$(openssl rand -hex 32)"
echo "   echo \"KEYSTORE_MASTER_SECRET=\$MASTER\" >> docker/.env.keystore-phala"
echo ""
echo "4. Upload to Phala Cloud:"
echo "   - Navigate to: https://dstack.phala.network"
echo "   - Create new deployment"
echo "   - Upload docker-compose.keystore-phala.yml"
echo "   - Configure environment variables from .env.keystore-phala"
echo "   - Deploy and get your keystore URL (e.g., https://keystore-abc123.phala.cloud)"
echo ""
echo "5. Update worker configuration with keystore URL:"
echo "   KEYSTORE_BASE_URL=https://keystore-abc123.phala.cloud"
echo "   KEYSTORE_AUTH_TOKEN=<same token from step 2>"
echo ""
echo -e "${YELLOW}⚠️  Security reminders:${NC}"
echo "  - Keep KEYSTORE_MASTER_SECRET secret and backed up"
echo "  - Use different tokens for each environment (dev/staging/prod)"
echo "  - Never commit .env.keystore-phala to git"
echo "  - Only workers and coordinator should access keystore URL"
echo ""
