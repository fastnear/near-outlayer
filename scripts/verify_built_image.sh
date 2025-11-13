#!/bin/bash

# Verify a built Docker image doesn't contain secrets
# Run this AFTER building the image to double-check

set -e

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

if [ "$#" -ne 1 ]; then
    echo -e "${RED}Usage: $0 <image-name:tag>${NC}"
    echo "Example: $0 myusername/near-offshore-worker:v1.0.0"
    exit 1
fi

IMAGE=$1

echo -e "${GREEN}🔒 Security Verification for Docker Image${NC}"
echo "Image: $IMAGE"
echo "=========================================="
echo ""

# Check if image exists
if ! docker image inspect "$IMAGE" > /dev/null 2>&1; then
    echo -e "${RED}❌ Error: Image '$IMAGE' not found${NC}"
    echo "Build it first with:"
    echo "  docker build -f docker/Dockerfile.worker-phala -t $IMAGE ."
    exit 1
fi

echo -e "${GREEN}✅ Image found${NC}"
echo ""

# Show image size
SIZE=$(docker image inspect "$IMAGE" --format='{{.Size}}' | awk '{print int($1/1024/1024)" MB"}')
echo "Image size: $SIZE"
echo ""

echo "🔍 Checking for sensitive files..."
echo "===================================="
echo ""

# Check for .env files
echo "1. Checking for .env files..."
if docker run --rm "$IMAGE" sh -c "find / -name '*.env' -type f 2>/dev/null" | grep -q .; then
    echo -e "${RED}⚠️  WARNING: Found .env files in image:${NC}"
    docker run --rm "$IMAGE" sh -c "find / -name '*.env' -type f 2>/dev/null"
else
    echo -e "${GREEN}   ✅ No .env files found${NC}"
fi
echo ""

# Check for private keys in binary
echo "2. Checking for private keys in compiled binary..."
if docker run --rm "$IMAGE" sh -c "strings /app/worker 2>/dev/null | grep -E 'ed25519:[A-Za-z0-9]{80,}'" | grep -q .; then
    echo -e "${RED}⚠️  WARNING: Found potential private key in binary:${NC}"
    docker run --rm "$IMAGE" sh -c "strings /app/worker | grep -E 'ed25519:[A-Za-z0-9]{80,}'"
    echo ""
    echo -e "${YELLOW}   This might be a test key from config.rs:test()${NC}"
    echo "   Verify it's NOT a real production key!"
else
    echo -e "${GREEN}   ✅ No private keys found in binary${NC}"
fi
echo ""

# Check for API tokens
echo "3. Checking for API tokens..."
if docker run --rm "$IMAGE" sh -c "strings /app/worker 2>/dev/null | grep -i 'bearer.*[a-z0-9]\{32,\}'" | grep -q .; then
    echo -e "${RED}⚠️  WARNING: Found potential API tokens${NC}"
    docker run --rm "$IMAGE" sh -c "strings /app/worker | grep -i 'bearer'"
else
    echo -e "${GREEN}   ✅ No API tokens found${NC}"
fi
echo ""

# List all files in image
echo "4. Files in /app directory:"
docker run --rm "$IMAGE" find /app -type f
echo ""

# Check environment variables baked into image
echo "5. Environment variables in image layers:"
if docker inspect "$IMAGE" | jq -r '.[0].Config.Env[]' 2>/dev/null | grep -qi "key\|token\|secret"; then
    echo -e "${RED}⚠️  WARNING: Found sensitive env vars baked into image:${NC}"
    docker inspect "$IMAGE" | jq -r '.[0].Config.Env[]' | grep -i "key\|token\|secret"
else
    echo -e "${GREEN}   ✅ No sensitive env vars baked into image${NC}"
    echo "   (Runtime env vars will be passed via docker-compose)"
fi
echo ""

echo "=========================================="
echo -e "${GREEN}Security check complete!${NC}"
echo ""
echo "Summary:"
echo "  • Image size: $SIZE"
echo "  • Only compiled binary included: /app/worker"
echo "  • No .env files should be present"
echo "  • Secrets will be passed at runtime via Phala Cloud"
echo ""
echo "To push this image:"
echo "  docker push $IMAGE"
echo ""
echo "To deploy to Phala Cloud:"
echo "  cd docker"
echo "  phala cvms create --name near-offshore-worker \\"
echo "    --compose ./docker-compose.phala.yml \\"
echo "    --env-file ./.env.phala \\"
echo "    --vcpu 2 --memory 4096 --disk-size 60"
echo ""
