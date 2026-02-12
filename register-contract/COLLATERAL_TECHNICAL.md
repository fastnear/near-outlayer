# TDX Quote Collateral - Technical Details

**Purpose**: Technical reference for understanding and managing Intel TDX quote collateral

---

## What is Collateral?

**Collateral** = Reference data from Intel needed to verify TDX quotes

### Components

```json
{
  "version": 4,
  "tcbInfo": {
    "issueDate": "2025-11-01T00:00:00Z",
    "nextUpdate": "2025-12-01T00:00:00Z",
    "fmspc": "00906ED50000",
    "pceId": "0000",
    "tcbType": 0,
    "tcbEvaluationDataNumber": 16,
    "tcbLevels": [...]
  },
  "certificates": {
    "tcbInfoIssuerChain": "-----BEGIN CERTIFICATE-----\nMIIC...",
    "rootCaCertificate": "-----BEGIN CERTIFICATE-----\nMIIC...",
    "pckCertificateChain": "-----BEGIN CERTIFICATE-----\nMIIC..."
  },
  "qeIdentity": {...},
  "crlDistributionPoint": "https://..."
}
```

### Why Needed?

TDX quote verification requires:
1. **TCB Info**: List of trusted TCB levels (platform firmware versions)
2. **Certificates**: Intel's certificate chain to verify quote signature
3. **CRL**: Certificate Revocation List (check for revoked certs)
4. **QE Identity**: Quote Enclave identity

Without collateral, you cannot verify if quote signature is valid.

---

## Collateral Lifecycle

### 1. Intel Releases New TCB

Intel regularly updates TCB levels when:
- Security vulnerabilities discovered (e.g., CVE-2025-XXXX)
- Microcode updates available
- Platform firmware updated

**Typical frequency**: Monthly

### 2. TCB Info Updated

Intel updates `tcbInfo.issueDate` and adds new TCB levels:

```json
{
  "tcbLevels": [
    {
      "tcb": {
        "sgxtcbcomponents": [...],
        "pcesvn": 13  // ← New version
      },
      "tcbDate": "2025-11-01T00:00:00Z",
      "tcbStatus": "UpToDate"
    },
    {
      "tcb": {
        "sgxtcbcomponents": [...],
        "pcesvn": 12  // ← Old version
      },
      "tcbDate": "2025-10-01T00:00:00Z",
      "tcbStatus": "OutOfDate"  // ← Now outdated
    }
  ]
}
```

### 3. Workers Generate Quotes

Worker in TEE generates quote with:
- **TDX measurements**: MRTD + RTMR0-3 (5 measurements identifying the TEE environment)
- **TCB level**: Platform's current firmware version
- **Signature**: Intel signs quote with their private key

### 4. Verification with Collateral

```rust
verify::verify(&quote_bytes, &collateral, timestamp)
  ↓
  1. Verify Intel signature (using certificates from collateral)
  2. Extract TCB level from quote
  3. Check if TCB level is "UpToDate" in collateral
  4. Check quote is not expired (issueDate < timestamp < nextUpdate)
  5. Check certificates not revoked (using CRL)
  ↓
  Result: ✅ or ❌
```

### 5. Stale Collateral Problem

If collateral not updated:

```
Worker quote generated: 2025-11-10 (with TCB pcesvn=13)
Collateral last updated: 2025-10-01 (max TCB pcesvn=12)
  ↓
Verification fails: "TCB level not found in collateral"
  ↓
Worker registration fails ❌
```

---

## How to Get Collateral

### Method 1: Intel PCS API (Direct)

**Pros**: Official source, always latest
**Cons**: Complex, need to parse multiple endpoints

```bash
# 1. Get FMSPC from TDX quote (Family-Model-Stepping-Platform-CustomSKU)
# This requires parsing the quote first

# 2. Fetch TCB Info
curl "https://api.trustedservices.intel.com/sgx/certification/v4/tcb?fmspc=<FMSPC>" \
  -H "Ocp-Apim-Subscription-Key: <your-api-key>"

# 3. Fetch PCK certificates
curl "https://api.trustedservices.intel.com/sgx/certification/v4/pckcert?..." \
  -H "Ocp-Apim-Subscription-Key: <your-api-key>"

# 4. Fetch QE Identity
curl "https://api.trustedservices.intel.com/sgx/certification/v4/qe/identity" \
  -H "Ocp-Apim-Subscription-Key: <your-api-key>"

# 5. Combine into collateral JSON
# (Complex - see dcap-qvl source code)
```

**Note**: Requires Intel API key (free, register at https://api.portal.trustedservices.intel.com/)

### Method 2: dcap-qvl CLI (Recommended)

**Pros**: Simple, handles all complexity
**Cons**: Requires existing TDX quote

```bash
# Install
git clone https://github.com/Phala-Network/dcap-qvl
cd dcap-qvl/cli
cargo install --path .

# Fetch collateral from quote
dcap-qvl fetch-collateral \
  --quote 48656c6c6f... \  # Hex-encoded TDX quote
  --output collateral.json

# Output: collateral.json (ready to use)
```

**How it works**:
1. Parses quote to extract FMSPC, PCEID, etc.
2. Queries Intel PCS API for all required data
3. Combines into single JSON file

### Method 3: Phala PCCS Cache

**Pros**: Fast, cached by Phala
**Cons**: Depends on Phala infrastructure

```bash
# Phala runs Provisioning Certificate Caching Service (PCCS)
# Caches collateral from Intel for faster access

# Get collateral via Phala's dcap-qvl
dcap-qvl fetch-collateral \
  --pccs-url https://pccs.phala.network \
  --quote <quote>
```

### Method 4: From Coordinator Cache

**Pros**: Simplest if coordinator already has it
**Cons**: Coordinator must be running

```bash
# Coordinator caches collateral after first worker registration
curl https://api.outlayer.fastnear.com/tdx/collateral > collateral.json
```

---

## Collateral Update Process

### Automated Script

Create `/opt/outlayer/scripts/update_collateral.sh`:

```bash
#!/bin/bash
set -e

# Configuration
NEAR_ACCOUNT="outlayer.near"
REGISTER_CONTRACT="worker.outlayer.near"
COORDINATOR_URL="https://api.outlayer.fastnear.com"
LOG_FILE="/var/log/outlayer/collateral-update.log"

log() {
    echo "[$(date +'%Y-%m-%d %H:%M:%S')] $1" | tee -a "$LOG_FILE"
}

log "Starting collateral update..."

# Option 1: From coordinator cache (if available)
if COLLATERAL=$(curl -sf "$COORDINATOR_URL/tdx/collateral"); then
    log "✅ Fetched collateral from coordinator cache"
else
    log "⚠️  Coordinator cache failed, fetching from Intel..."

    # Option 2: From Intel via dcap-qvl
    # Get a recent quote from database
    RECENT_QUOTE=$(psql $DATABASE_URL -t -c "
        SELECT tdx_quote
        FROM task_attestations
        WHERE tdx_quote IS NOT NULL
        ORDER BY created_at DESC
        LIMIT 1;
    " | tr -d '[:space:]')

    if [ -z "$RECENT_QUOTE" ]; then
        log "❌ No recent quotes found in database"
        exit 1
    fi

    # Fetch collateral using dcap-qvl
    COLLATERAL=$(dcap-qvl fetch-collateral --quote "$RECENT_QUOTE" --json)

    if [ $? -ne 0 ]; then
        log "❌ Failed to fetch collateral from Intel"
        exit 1
    fi

    log "✅ Fetched collateral from Intel PCS"
fi

# Validate JSON format
if ! echo "$COLLATERAL" | jq . > /dev/null 2>&1; then
    log "❌ Invalid collateral JSON format"
    exit 1
fi

# Check TCB issue date (should be recent)
ISSUE_DATE=$(echo "$COLLATERAL" | jq -r '.tcbInfo.issueDate')
DAYS_OLD=$(( ($(date +%s) - $(date -d "$ISSUE_DATE" +%s)) / 86400 ))

if [ $DAYS_OLD -gt 60 ]; then
    log "⚠️  WARNING: Collateral is $DAYS_OLD days old (issue date: $ISSUE_DATE)"
fi

log "Collateral issue date: $ISSUE_DATE ($DAYS_OLD days old)"

# Update contract
log "Updating register contract..."

if near call "$REGISTER_CONTRACT" update_collateral \
    "$COLLATERAL" \
    --accountId "$NEAR_ACCOUNT" \
    --gas 300000000000000 >> "$LOG_FILE" 2>&1; then

    log "✅ Collateral updated successfully in contract"

    # Verify update
    CONTRACT_ISSUE_DATE=$(near view "$REGISTER_CONTRACT" get_collateral | \
        jq -r '.tcbInfo.issueDate')

    if [ "$CONTRACT_ISSUE_DATE" = "$ISSUE_DATE" ]; then
        log "✅ Verification passed: contract collateral matches"
    else
        log "⚠️  WARNING: Contract collateral mismatch"
    fi
else
    log "❌ Failed to update contract"
    exit 1
fi

log "Collateral update complete"
```

### Cron Job Setup

```bash
# Install cron job (as root)
sudo tee /etc/cron.d/outlayer-collateral-update <<EOF
# Update TDX collateral every Monday at 3 AM
0 3 * * 1 outlayer /opt/outlayer/scripts/update_collateral.sh

# Update on 1st and 15th of every month at 3 AM (more frequent)
0 3 1,15 * * outlayer /opt/outlayer/scripts/update_collateral.sh
EOF

# Make script executable
sudo chmod +x /opt/outlayer/scripts/update_collateral.sh

# Test run
sudo -u outlayer /opt/outlayer/scripts/update_collateral.sh
```

---

## Monitoring Collateral Health

### Check Collateral Age

```bash
# Get issue date from contract
ISSUE_DATE=$(near view worker.outlayer.near get_collateral | \
    jq -r '.tcbInfo.issueDate')

# Calculate age in days
DAYS_OLD=$(( ($(date +%s) - $(date -d "$ISSUE_DATE" +%s)) / 86400 ))

echo "Collateral age: $DAYS_OLD days (issued: $ISSUE_DATE)"

if [ $DAYS_OLD -gt 30 ]; then
    echo "⚠️  WARNING: Collateral is outdated (>30 days)"
elif [ $DAYS_OLD -gt 14 ]; then
    echo "⚠️  Collateral should be updated soon (>14 days)"
else
    echo "✅ Collateral is fresh"
fi
```

### Alert Setup

Using prometheus + alertmanager:

```yaml
# prometheus-rules.yaml
groups:
  - name: outlayer-collateral
    rules:
      - alert: CollateralOutdated
        expr: outlayer_collateral_age_days > 30
        for: 1h
        labels:
          severity: critical
        annotations:
          summary: "TDX collateral is outdated ({{ $value }} days old)"
          description: "Update collateral ASAP to prevent worker registration failures"

      - alert: CollateralExpiringSoon
        expr: outlayer_collateral_age_days > 14
        for: 6h
        labels:
          severity: warning
        annotations:
          summary: "TDX collateral needs update ({{ $value }} days old)"
```

---

## Troubleshooting

### Problem: "TCB level not found in collateral"

**Cause**: Worker's platform TCB is newer than collateral

**Solution**:
```bash
# 1. Update collateral immediately
./scripts/update_collateral.sh

# 2. Workers will automatically retry registration
```

### Problem: "Certificate chain verification failed"

**Cause**: Certificates in collateral are revoked or expired

**Solution**:
```bash
# 1. Fetch fresh collateral from Intel (not cache)
dcap-qvl fetch-collateral --quote <recent-quote> --output collateral.json

# 2. Update contract
near call worker.outlayer.near update_collateral \
    "$(cat collateral.json | jq -c)" \
    --accountId outlayer.near \
    --gas 300000000000000
```

### Problem: "Quote expired"

**Cause**: Quote timestamp outside collateral validity window

**Solution**:
```bash
# Check collateral validity window
near view worker.outlayer.near get_collateral | \
    jq '{issueDate: .tcbInfo.issueDate, nextUpdate: .tcbInfo.nextUpdate}'

# If nextUpdate < now, update collateral immediately
```

---

## Best Practices

1. **Automate updates**: Use cron job (weekly or bi-weekly)
2. **Monitor age**: Alert if > 14 days old
3. **Keep backup**: Save collateral JSON files with timestamps
4. **Test before prod**: Update testnet contract first
5. **Multiple sources**: Try coordinator cache, then dcap-qvl, then Intel API
6. **Log everything**: Keep audit trail of all updates

---

## Emergency Update Procedure

If workers are failing registration:

```bash
# 1. Check current collateral age
near view worker.outlayer.near get_collateral | jq '.tcbInfo.issueDate'

# 2. Get fresh collateral (fastest method)
curl https://api.outlayer.fastnear.com/tdx/collateral > collateral.json

# 3. Update immediately
near call worker.outlayer.near update_collateral \
    "$(cat collateral.json | jq -c)" \
    --accountId outlayer.near \
    --gas 300000000000000

# 4. Verify workers can register
# (They retry every 60 seconds automatically)
psql $DATABASE_URL -c "
SELECT worker_name, last_attestation_at
FROM worker_auth_tokens
WHERE last_attestation_at > NOW() - INTERVAL '5 minutes';
"
```

---

## References

- **Intel PCS API**: https://api.portal.trustedservices.intel.com/
- **DCAP Specification**: https://download.01.org/intel-sgx/sgx-dcap/
- **Phala dcap-qvl**: https://github.com/Phala-Network/dcap-qvl
- **TCB Recovery**: https://www.intel.com/content/www/us/en/developer/articles/technical/intel-sgx-tcb-recovery.html

---

**Last Updated**: 2025-11-12
**Version**: 1.0.0
