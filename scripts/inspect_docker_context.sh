#!/bin/bash

# Show what files will be included in Docker build context
# This helps verify no secrets are accidentally included
#
# Usage:
#   ./scripts/inspect_docker_context.sh [dockerfile] [source_dir]
#
# Examples:
#   ./scripts/inspect_docker_context.sh docker/Dockerfile.worker-phala worker
#   ./scripts/inspect_docker_context.sh docker/Dockerfile.keystore-phala keystore-worker

DOCKERFILE="${1:-docker/Dockerfile.worker-phala}"
SOURCE_DIR="${2:-worker}"

echo "Files that will be sent to Docker build context:"
echo "================================================"
echo "Dockerfile: $DOCKERFILE"
echo "Source dir: $SOURCE_DIR"
echo ""

cd "$(dirname "$0")/.."

# Show .dockerignore if exists
if [ -f ".dockerignore" ]; then
    echo "✅ .dockerignore exists:"
    cat .dockerignore
    echo ""
else
    echo "⚠️  WARNING: No .dockerignore file found!"
    echo "Creating one now..."
    cat > .dockerignore << 'EOF'
# Don't include these in Docker build
.env
.env.*
!.env.example
*.env
*.env.*
!*.env.example

# Git
.git/
.gitignore

# IDE
.idea/
.vscode/

# Build artifacts
target/
node_modules/
.next/
dist/

# Logs
*.log
logs/

# Database
*.db
*.sqlite

# Temp files
tmp/
/tmp/

# SQLx offline data
.sqlx/

# Docs (not needed in image)
*.md
!README.md

# Scripts (not needed in image)
scripts/
EOF
    echo "✅ Created .dockerignore"
    cat .dockerignore
    echo ""
fi

echo ""
echo "Checking for sensitive files in $SOURCE_DIR directory:"
echo "================================================"

# Check for .env files
echo ""
echo "🔍 Looking for .env files..."
if [ -d "$SOURCE_DIR" ]; then
    find "$SOURCE_DIR/" -name "*.env*" -type f 2>/dev/null | while read file; do
        if [[ "$file" == *".example"* ]]; then
            echo "  ✅ $file (example file - OK)"
        else
            echo "  ⚠️  $file (SECRETS! Should be in .dockerignore)"
        fi
    done
else
    echo "  ⚠️  Directory $SOURCE_DIR not found"
fi

# Check for private keys
echo ""
echo "🔍 Looking for private key patterns..."
if [ -d "$SOURCE_DIR/src" ]; then
    grep -r "ed25519:" "$SOURCE_DIR/src/" 2>/dev/null | grep -v "example" | grep -v "test" | grep -v "TODO" || echo "  ✅ No hardcoded private keys found"
else
    echo "  ℹ️  No src/ directory in $SOURCE_DIR"
fi

# Check for API tokens
echo ""
echo "🔍 Looking for API tokens..."
if [ -d "$SOURCE_DIR/src" ]; then
    grep -r "api.*token\|auth.*token" "$SOURCE_DIR/src/" 2>/dev/null | grep -v "API_AUTH_TOKEN" | grep -v "//" || echo "  ✅ No hardcoded tokens found"
else
    echo "  ℹ️  No src/ directory in $SOURCE_DIR"
fi

# Show what will actually be copied
echo ""
echo "Files that will be copied to Docker image:"
echo "=========================================="
echo ""
echo "From $DOCKERFILE:"
if [ -f "$DOCKERFILE" ]; then
    grep "^COPY" "$DOCKERFILE"
else
    echo "  ⚠️  Dockerfile not found: $DOCKERFILE"
fi

echo ""
echo ""
echo "Summary of $SOURCE_DIR/src/ files:"
if [ -d "$SOURCE_DIR/src" ]; then
    find "$SOURCE_DIR/src" -type f -name "*.rs" | head -20
    echo ""
    echo "Total Rust files: $(find "$SOURCE_DIR/src" -type f -name "*.rs" | wc -l)"
else
    echo "  ℹ️  No src/ directory in $SOURCE_DIR"
fi
echo ""
