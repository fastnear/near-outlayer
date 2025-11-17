#!/bin/bash
set -e

# Fetch Intel Collateral for TDX Quote Verification
#
# This script fetches the Intel collateral data needed to verify TDX quotes.
# Collateral includes:
# - TCB Info (Trusted Computing Base information)
# - QE Identity (Quoting Enclave identity)
# - CRL (Certificate Revocation List)
#
# The collateral should be updated weekly or when Intel releases TCB updates.

echo "📥 Fetching Intel collateral for TDX quote verification..."

# Option 1: Phala API (recommended - includes all 9 fields with CRL)
echo ""
echo "Using Phala API (recommended)..."
echo ""

# Create a dummy TDX quote request to get collateral
# Phala API will return collateral even if quote verification fails
DUMMY_QUOTE=$(printf "00%.0s" {1..10000})

# Call Phala verification API
RESPONSE=$(curl -s 'https://cloud-api.phala.network/api/v1/attestations/verify' \
  -X POST \
  -H 'Content-Type: application/json' \
  -d "{\"quote\":\"$DUMMY_QUOTE\",\"verify_report\":false}")

# Extract collateral from response
COLLATERAL=$(echo "$RESPONSE" | jq -r '.collateral')

if [ "$COLLATERAL" = "null" ] || [ -z "$COLLATERAL" ]; then
    echo "❌ Failed to fetch collateral from Phala API"
    echo ""
    echo "Response:"
    echo "$RESPONSE"
    exit 1
fi

# Validate collateral has all 9 required fields
echo "$COLLATERAL" | jq -e '.tcb_info' > /dev/null || { echo "❌ Missing tcb_info"; exit 1; }
echo "$COLLATERAL" | jq -e '.tcb_info_issuer_chain' > /dev/null || { echo "❌ Missing tcb_info_issuer_chain"; exit 1; }
echo "$COLLATERAL" | jq -e '.tcb_info_signature' > /dev/null || { echo "❌ Missing tcb_info_signature"; exit 1; }
echo "$COLLATERAL" | jq -e '.qe_identity' > /dev/null || { echo "❌ Missing qe_identity"; exit 1; }
echo "$COLLATERAL" | jq -e '.qe_identity_issuer_chain' > /dev/null || { echo "❌ Missing qe_identity_issuer_chain"; exit 1; }
echo "$COLLATERAL" | jq -e '.qe_identity_signature' > /dev/null || { echo "❌ Missing qe_identity_signature"; exit 1; }
echo "$COLLATERAL" | jq -e '.pck_crl_issuer_chain' > /dev/null || { echo "❌ Missing pck_crl_issuer_chain"; exit 1; }
echo "$COLLATERAL" | jq -e '.root_ca_crl' > /dev/null || { echo "❌ Missing root_ca_crl"; exit 1; }
echo "$COLLATERAL" | jq -e '.pck_crl' > /dev/null || { echo "❌ Missing pck_crl"; exit 1; }

echo "✅ Fetched complete collateral with all 9 fields"
echo ""

# Pretty print collateral
echo "$COLLATERAL" | jq .

echo ""
echo "📋 Collateral Summary:"
echo "   tcb_info: $(echo "$COLLATERAL" | jq -r '.tcb_info' | wc -c) bytes"
echo "   qe_identity: $(echo "$COLLATERAL" | jq -r '.qe_identity' | wc -c) bytes"
echo "   pck_crl_issuer_chain: $(echo "$COLLATERAL" | jq -r '.pck_crl_issuer_chain' | wc -c) bytes"
echo "   root_ca_crl: $(echo "$COLLATERAL" | jq -r '.root_ca_crl' | wc -c) bytes"
echo "   pck_crl: $(echo "$COLLATERAL" | jq -r '.pck_crl' | wc -c) bytes"

echo ""
echo "💾 Usage:"
echo ""
echo "# Save to file:"
echo "./scripts/fetch_intel_collateral.sh > collateral.json"
echo ""
echo "# Update register contract:"
echo "near call register.outlayer.testnet update_collateral \\"
echo "  \"{\\\"collateral\\\": \$(cat collateral.json | jq -c .)}\" \\"
echo "  --accountId outlayer.testnet \\"
echo "  --gas 100000000000000"
echo ""

# Option 2: Intel PCS API (alternative - requires manual CRL assembly)
# echo ""
# echo "Alternative: Intel PCS API"
# echo ""
# echo "1. Get TCB Info:"
# echo "   curl https://api.trustedservices.intel.com/sgx/certification/v4/tcb?fmspc=YOUR_FMSPC"
# echo ""
# echo "2. Get QE Identity:"
# echo "   curl https://api.trustedservices.intel.com/sgx/certification/v4/qe/identity"
# echo ""
# echo "3. Get CRL:"
# echo "   curl https://api.trustedservices.intel.com/sgx/certification/v4/pckcrl?ca=processor"
# echo ""
# echo "Note: Using Phala API is easier as it returns complete collateral"
