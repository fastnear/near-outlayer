#!/bin/bash

# Security check for Coordinator API before going public
# Run this to verify coordinator is properly secured

set -e

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

echo -e "${GREEN}🔒 Coordinator Security Check${NC}"
echo "========================================"
echo ""

if [ "$#" -lt 1 ]; then
    echo "Usage: $0 <coordinator-url> [auth-token]"
    echo ""
    echo "Examples:"
    echo "  $0 http://localhost:8080"
    echo "  $0 https://coordinator.your-domain.com your-token-here"
    exit 1
fi

COORDINATOR_URL=$1
AUTH_TOKEN=${2:-""}

echo "Testing coordinator: $COORDINATOR_URL"
echo ""

# Test 1: Health check (should work without auth)
echo "Test 1: Health check (public endpoint)"
echo "========================================"
HTTP_CODE=$(curl -s -o /dev/null -w "%{http_code}" "$COORDINATOR_URL/health")

if [ "$HTTP_CODE" = "200" ]; then
    echo -e "${GREEN}✅ PASS${NC} - Health check accessible (HTTP $HTTP_CODE)"
else
    echo -e "${RED}❌ FAIL${NC} - Health check failed (HTTP $HTTP_CODE)"
    echo "   Coordinator may not be running or not accessible"
fi
echo ""

# Test 2: Public API (should work without auth)
echo "Test 2: Public API access (no auth required)"
echo "========================================"
HTTP_CODE=$(curl -s -o /dev/null -w "%{http_code}" "$COORDINATOR_URL/public/stats")

if [ "$HTTP_CODE" = "200" ]; then
    echo -e "${GREEN}✅ PASS${NC} - Public API accessible (HTTP $HTTP_CODE)"
    curl -s "$COORDINATOR_URL/public/stats" | head -3
else
    echo -e "${YELLOW}⚠️  WARNING${NC} - Public API returned HTTP $HTTP_CODE"
    echo "   This might be expected if public endpoints are disabled"
fi
echo ""

# Test 3: Protected endpoint WITHOUT auth (should return 401)
echo "Test 3: Protected endpoint without auth (should reject)"
echo "========================================"
HTTP_CODE=$(curl -s -o /dev/null -w "%{http_code}" "$COORDINATOR_URL/executions/poll")

if [ "$HTTP_CODE" = "401" ]; then
    echo -e "${GREEN}✅ PASS${NC} - Protected endpoint requires auth (HTTP 401)"
    echo "   This is GOOD - authentication is enforced!"
elif [ "$HTTP_CODE" = "200" ]; then
    echo -e "${RED}❌ FAIL${NC} - Protected endpoint accessible without auth!"
    echo "   This is DANGEROUS! Set REQUIRE_AUTH=true in coordinator/.env"
else
    echo -e "${YELLOW}⚠️  WARNING${NC} - Unexpected HTTP code: $HTTP_CODE"
fi
echo ""

# Test 4: Protected endpoint WITH auth (if token provided)
if [ -n "$AUTH_TOKEN" ]; then
    echo "Test 4: Protected endpoint with auth token"
    echo "========================================"
    HTTP_CODE=$(curl -s -o /dev/null -w "%{http_code}" \
        -H "Authorization: Bearer $AUTH_TOKEN" \
        "$COORDINATOR_URL/executions/poll")

    if [ "$HTTP_CODE" = "200" ]; then
        echo -e "${GREEN}✅ PASS${NC} - Auth token accepted (HTTP 200)"
        echo "   Token is valid and active"
    elif [ "$HTTP_CODE" = "401" ]; then
        echo -e "${RED}❌ FAIL${NC} - Auth token rejected (HTTP 401)"
        echo "   Token is invalid or inactive"
        echo "   Check token in database:"
        echo "   SELECT * FROM worker_auth_tokens WHERE is_active = true;"
    else
        echo -e "${YELLOW}⚠️  WARNING${NC} - Unexpected HTTP code: $HTTP_CODE"
    fi
    echo ""
else
    echo "Test 4: SKIPPED (no auth token provided)"
    echo "   Provide token as 2nd argument to test auth"
    echo ""
fi

# Test 5: HTTPS check
echo "Test 5: HTTPS/SSL check"
echo "========================================"
if [[ "$COORDINATOR_URL" == https://* ]]; then
    echo -e "${GREEN}✅ PASS${NC} - Using HTTPS (secure)"

    # Check SSL certificate validity
    HOST=$(echo "$COORDINATOR_URL" | sed -E 's|https://([^/]+).*|\1|')
    if command -v openssl &> /dev/null; then
        echo ""
        echo "SSL Certificate info:"
        echo "---------------------"
        echo | openssl s_client -connect "$HOST:443" -servername "$HOST" 2>/dev/null | \
            openssl x509 -noout -dates 2>/dev/null || echo "   Could not verify SSL cert"
    fi
elif [[ "$COORDINATOR_URL" == http://* ]]; then
    if [[ "$COORDINATOR_URL" == http://localhost* ]] || [[ "$COORDINATOR_URL" == http://127.0.0.1* ]]; then
        echo -e "${YELLOW}⚠️  WARNING${NC} - Using HTTP (localhost only)"
        echo "   This is OK for local development"
        echo "   For production, use HTTPS!"
    else
        echo -e "${RED}❌ FAIL${NC} - Using HTTP for external access!"
        echo "   This is INSECURE - bearer tokens will be sent in plaintext"
        echo "   Use HTTPS for production!"
    fi
fi
echo ""

# Summary
echo "========================================"
echo -e "${GREEN}Security Check Summary${NC}"
echo "========================================"
echo ""

ISSUES=0

# Check HTTPS for non-localhost
if [[ "$COORDINATOR_URL" == http://* ]] && [[ "$COORDINATOR_URL" != http://localhost* ]] && [[ "$COORDINATOR_URL" != http://127.0.0.1* ]]; then
    echo -e "${RED}❌ Critical: Not using HTTPS for external access${NC}"
    ISSUES=$((ISSUES + 1))
fi

# Check if auth is working
if [ "$HTTP_CODE" != "401" ] && [ "$HTTP_CODE" != "200" ]; then
    echo -e "${YELLOW}⚠️  Warning: Could not verify auth is enabled${NC}"
fi

if [ $ISSUES -eq 0 ]; then
    echo -e "${GREEN}✅ No critical security issues found!${NC}"
    echo ""
    echo "Recommendations for production:"
    echo "  1. Ensure REQUIRE_AUTH=true in coordinator/.env"
    echo "  2. Use HTTPS with valid SSL certificate"
    echo "  3. Use strong random tokens (32+ bytes)"
    echo "  4. Enable rate limiting"
    echo "  5. Monitor logs for suspicious activity"
    echo ""
    echo "See COORDINATOR_PUBLIC_ACCESS.md for details"
else
    echo -e "${RED}❌ Found $ISSUES critical security issue(s)!${NC}"
    echo ""
    echo "Fix before deploying to production!"
fi
echo ""
