# Register Contract - Deployment & Maintenance Guide

**Date**: 2025-11-12
**Status**: Production ready
**Contract**: `worker.outlayer.near`
**Operator**: `operator.outlayer.near`

---

## Overview

Register contract manages worker TEE key registration with cryptographic proof:
1. Worker generates keypair **inside TEE** (private key never leaves)
2. Worker generates TDX quote with public key embedded
3. Register contract verifies TDX quote (Intel signature)
4. Register contract adds public key to operator account

**Result**: Cryptographic proof that operator keys were generated in approved TEE

---

## Initial Deployment

### Step 1: Build contract

```bash
cd register-contract
./build.sh
```

**Expected output**:
```
✅ Build complete: res/local/register_contract.wasm
-rw-r--r-- 1 user staff 450K Nov 12 14:00 res/local/register_contract.wasm
```

**Note**: Contract size ~400-500 KB due to dcap-qvl library

### Step 2: Deploy contract

```bash
near deploy worker.outlayer.near \
  use-file res/local/register_contract.wasm \
  with-init-call new \
  json-args '{
    "owner_id": "outlayer.near",
    "operator_account_id": "operator.outlayer.near"
  }' \
  prepaid-gas '100.0 Tgas' \
  attached-deposit '0 NEAR' \
  network-config mainnet \
  sign-with-keychain \
  send
```

**Verify deployment**:
```bash
near view worker.outlayer.near get_operator_account
# Expected: "operator.outlayer.near"
```

### Step 3: Get initial collateral

Collateral is Intel's reference data (certificates, TCB info) needed for TDX quote verification.

**Option A: Using Phala dcap-qvl CLI** (Recommended)

```bash
# Install dcap-qvl CLI
git clone https://github.com/Phala-Network/dcap-qvl
cd dcap-qvl/cli
cargo install --path .

# Fetch collateral for a sample quote
# (You'll need a TDX quote from any Phala worker)
dcap-qvl fetch-collateral --quote <quote-hex> --output collateral.json
```

**Option B: Using Intel PCS API**

```bash
# Get collateral from Intel's API
# This requires FMSPC and other identifiers from a TDX quote
curl "https://api.trustedservices.intel.com/sgx/certification/v4/tcb?fmspc=<FMSPC>" \
  -o tcb_info.json

# Combine into collateral format (complex, see dcap-qvl documentation)
```

**Option C: From coordinator** (After first worker deployment)

```bash
# After first worker runs, get collateral from coordinator cache
curl https://api.outlayer.fastnear.com/tdx/collateral > collateral.json
```

### Step 4: Update collateral in contract

```bash
near call worker.outlayer.near update_collateral \
  "$(cat collateral.json | jq -c)" \
  --accountId outlayer.near \
  --gas 300000000000000
```

**Verify**:
```bash
near view worker.outlayer.near get_collateral | jq .
```

### Step 5: Get production worker TDX measurements

Deploy one test worker to Phala Cloud and get its measurements. Use `scripts/deploy_phala.sh` (recommended) or extract manually:

```bash
# From Phala attestation (extracts all 5 measurements: MRTD + RTMR0-3)
phala cvms attestation --json test-worker-1 | jq '{
  mrtd: .mrtd,
  rtmr0: .rtmr0,
  rtmr1: .rtmr1,
  rtmr2: .rtmr2,
  rtmr3: .rtmr3
}'
```

### Step 6: Add approved measurements

All 5 measurements must match for a worker to register. This prevents dev/debug images (e.g., with SSH access) from passing verification.

```bash
near call worker.outlayer.near add_approved_measurements '{
  "measurements": {
    "mrtd": "<96-hex>",
    "rtmr0": "<96-hex>",
    "rtmr1": "<96-hex>",
    "rtmr2": "<96-hex>",
    "rtmr3": "<96-hex>"
  },
  "clear_others": true
}' --accountId outlayer.near
```

**Verify**:
```bash
near view worker.outlayer.near get_approved_measurements
```

---

## Collateral Update Schedule

### Why Update Collateral?

Intel periodically releases new TCB (Trusted Computing Base) versions:
- Security patches
- Microcode updates
- Certificate rotations

**Stale collateral → Workers cannot register (verification fails)**

### Update Frequency

- **Recommended**: Every 2 weeks
- **Minimum**: Monthly
- **Critical**: When Intel releases security advisory

### How to Update

#### Option 1: Manual update (Simple)

```bash
# 1. Fetch new collateral from Intel or Phala
dcap-qvl fetch-collateral --quote <recent-quote> --output collateral.json

# 2. Update contract
near call worker.outlayer.near update_collateral \
  "$(cat collateral.json | jq -c)" \
  --accountId outlayer.near \
  --gas 300000000000000

# 3. Verify
near view worker.outlayer.near get_collateral | head -20
```

#### Option 2: Automated update (Production)

Create cron job:

```bash
# /etc/cron.d/update-collateral
0 3 * * 1 /opt/outlayer/scripts/update_collateral.sh >> /var/log/outlayer/collateral-update.log 2>&1
```

**Script** (`scripts/update_collateral.sh`):

```bash
#!/bin/bash
set -e

LOG_PREFIX="[Collateral Update]"
NEAR_ACCOUNT="outlayer.near"
CONTRACT="worker.outlayer.near"

echo "$LOG_PREFIX Starting collateral update..."

# Get latest collateral from coordinator cache
COLLATERAL=$(curl -s https://api.outlayer.fastnear.com/tdx/collateral)

if [ -z "$COLLATERAL" ]; then
    echo "$LOG_PREFIX ERROR: Failed to fetch collateral"
    exit 1
fi

# Update contract
near call "$CONTRACT" update_collateral \
    "$COLLATERAL" \
    --accountId "$NEAR_ACCOUNT" \
    --gas 300000000000000

echo "$LOG_PREFIX ✅ Collateral updated successfully"
```

### Monitoring Collateral Freshness

```bash
# Check TCB issue date in collateral
near view worker.outlayer.near get_collateral | \
  jq -r '.tcbInfo.issueDate'

# Should be within last 30 days
```

---

## Measurements Management

### When Measurements Change

**TDX measurements change when**:
- Worker Docker image updated (new code)
- Phala configuration changed
- Dependencies updated (Rust version, libraries)

**Process**:

1. **Update worker code**
2. **Build new Docker image**
3. **Deploy test worker** to Phala (use `scripts/deploy_phala.sh`)
4. **Get new measurements** from Phala attestation:
   ```bash
   phala cvms attestation --json <cvm-name> | jq '{
     mrtd: .mrtd, rtmr0: .rtmr0, rtmr1: .rtmr1,
     rtmr2: .rtmr2, rtmr3: .rtmr3
   }'
   ```
5. **Add to approved list** (with `clear_others: true` to replace old set):
   ```bash
   near call worker.outlayer.near add_approved_measurements '{
     "measurements": {"mrtd":"...","rtmr0":"...","rtmr1":"...","rtmr2":"...","rtmr3":"..."},
     "clear_others": true
   }' --accountId outlayer.near
   ```
6. **Rolling update**: Deploy remaining workers (they auto-register with new measurements)

### Remove Old Measurements

If using `"clear_others": true` during step 5, old measurements are removed automatically.
Otherwise, after all workers migrated to new version:

```bash
near call worker.outlayer.near remove_approved_measurements '{
  "measurements": {"mrtd":"<old>","rtmr0":"<old>","rtmr1":"<old>","rtmr2":"<old>","rtmr3":"<old>"}
}' --accountId outlayer.near
```

---

## Worker Deployment Flow

### New Worker Setup

**1. Create gas account** (one-time per worker):

```bash
# Create sub-account for worker gas
near create-account worker1.outlayer.near \
  --masterAccount outlayer.near \
  --initialBalance 5

# Generate key for gas account
near generate-key worker1.outlayer.near
# Saves to ~/.near-credentials/mainnet/worker1.outlayer.near.json
```

**2. Worker `.env` configuration**:

```bash
# Worker configuration
WORKER_ID=worker1
API_BASE_URL=https://api.outlayer.fastnear.com
API_AUTH_TOKEN=<coordinator-token>

# TEE configuration
TEE_MODE=tdx

# NEAR configuration
NEAR_RPC_URL=https://rpc.mainnet.near.org
OFFCHAINVM_CONTRACT_ID=outlayer.near
OPERATOR_ACCOUNT_ID=operator.outlayer.near

# Registration (register-contract is deployed at OPERATOR_ACCOUNT_ID)
GAS_ACCOUNT_ID=worker1.outlayer.near
GAS_ACCOUNT_PRIVATE_KEY=ed25519:5J... # From ~/.near-credentials

# Keystore
KEYSTORE_BASE_URL=http://host.docker.internal:8081
```

**3. Deploy worker** to Phala Cloud:

```bash
# Worker will automatically:
# 1. Generate keypair inside TEE
# 2. Generate TDX quote with public key
# 3. Call worker.outlayer.near::register_worker_key
# 4. On success: Key added to operator.outlayer.near
# 5. Worker starts processing tasks
```

**4. Verify registration**:

```bash
# Check operator has new key
near view-access-keys operator.outlayer.near

# Should see new key with FunctionCall permission to outlayer.near::resolve_execution
```

---

## Troubleshooting

### Problem: Worker registration fails with "Measurements not approved"

**Cause**: Worker's TDX measurements not in approved list

**Solution**:

```bash
# 1. Get worker's measurements from Phala attestation
phala cvms attestation --json <cvm-name> | jq '{mrtd,rtmr0,rtmr1,rtmr2,rtmr3}'

# 2. Add to approved list
near call worker.outlayer.near add_approved_measurements '{
  "measurements": {"mrtd":"...","rtmr0":"...","rtmr1":"...","rtmr2":"...","rtmr3":"..."}
}' --accountId outlayer.near

# 3. Worker will retry registration automatically (60 second interval)
```

### Problem: "TDX quote verification failed"

**Cause**: Collateral outdated or corrupted

**Solution**:

```bash
# 1. Fetch fresh collateral
dcap-qvl fetch-collateral --quote <recent-quote> -o collateral.json

# 2. Update contract
near call worker.outlayer.near update_collateral \
  "$(cat collateral.json | jq -c)" \
  --accountId outlayer.near \
  --gas 300000000000000

# 3. Workers will retry automatically
```

### Problem: "Out of gas" during registration

**Cause**: Gas account (worker1.outlayer.near) balance too low

**Solution**:

```bash
# Check balance
near view-account worker1.outlayer.near

# Top up if needed
near send outlayer.near worker1.outlayer.near 5
```

### Problem: Worker generates key but registration fails

**Logs to check**:

```bash
# Worker logs (Phala Cloud)
docker logs <worker-container> | grep -A10 "register_worker_key"

# Coordinator logs (if using coordinator as proxy)
journalctl -u outlayer-coordinator | grep tdx

# NEAR transaction logs
near tx-status <transaction-hash> --accountId outlayer.near
```

---

## Security Checklist

- [ ] Register contract deployed with correct `operator_account_id`
- [ ] Only `outlayer.near` can manage approved measurements list
- [ ] Collateral updated within last 30 days
- [ ] Production worker measurements (all 5) added to approved list
- [ ] Gas accounts have sufficient balance (>= 2 NEAR)
- [ ] Gas account keys stored securely (encrypted .env)
- [ ] Old measurements removed after migration complete

---

## Monitoring

### Daily checks

```bash
# 1. Check collateral freshness
near view worker.outlayer.near get_collateral | jq '.tcbInfo.issueDate'
# Should be < 30 days old

# 2. Check approved measurements count
near view worker.outlayer.near get_approved_measurements | jq '. | length'
# Should be 1-3 (current + maybe 1 old during migration)

# 3. Check worker registrations (last 24h)
psql $DATABASE_URL -c "
SELECT
    COUNT(DISTINCT worker_name) as registered_workers,
    COUNT(*) as total_attestations
FROM worker_auth_tokens
WHERE last_attestation_at > NOW() - INTERVAL '24 hours';
"
```

### Alerts

Set up monitoring for:
- Collateral age > 30 days → Update collateral
- Worker registration failures → Check logs
- Gas account balance < 1 NEAR → Top up

---

## Maintenance Schedule

| Frequency | Task | Command |
|-----------|------|---------|
| **Weekly** | Update collateral | `near call worker.outlayer.near update_collateral ...` |
| **Monthly** | Check gas balances | `near view-account worker*.outlayer.near` |
| **On worker update** | Add new measurements | `near call worker.outlayer.near add_approved_measurements ...` |
| **After migration** | Remove old measurements | `near call worker.outlayer.near remove_approved_measurements ...` |
| **Quarterly** | Review access keys | `near view-access-keys operator.outlayer.near` |

---

## References

- **Intel DCAP**: https://github.com/intel/SGXDataCenterAttestationPrimitives
- **Phala dcap-qvl**: https://github.com/Phala-Network/dcap-qvl
- **NEAR Access Keys**: https://docs.near.org/concepts/basics/accounts/access-keys
- **TDX Specification**: https://www.intel.com/content/www/us/en/developer/tools/trust-domain-extensions/overview.html

---

**Last Updated**: 2025-11-12
**Version**: 1.0.0
**Status**: ✅ Production Ready
