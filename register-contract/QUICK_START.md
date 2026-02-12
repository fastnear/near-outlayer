# Register Contract - Quick Start

**5-minute setup guide**

---

## Prerequisites

- NEAR CLI installed
- `dcap-qvl` CLI installed
- One worker deployed to Phala (to get TDX measurements)

---

## Setup

### 1. Build & Deploy (2 minutes)

```bash
cd register-contract
./build.sh

near deploy worker.outlayer.near \
  use-file res/local/register_contract.wasm \
  with-init-call new \
  json-args '{"owner_id":"outlayer.near","operator_account_id":"operator.outlayer.near"}' \
  prepaid-gas '100.0 Tgas' network-config mainnet sign-with-keychain send
```

### 2. Get TDX Measurements (1 minute)

Use `scripts/deploy_phala.sh` which extracts all 5 measurements from the Phala attestation automatically, or get them manually:

```bash
# From Phala attestation (extracts MRTD + RTMR0-3)
phala cvms attestation --json <cvm-name> | jq '{
  mrtd: .mrtd,
  rtmr0: .rtmr0,
  rtmr1: .rtmr1,
  rtmr2: .rtmr2,
  rtmr3: .rtmr3
}'
```

### 3. Add Approved Measurements (30 seconds)

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

### 4. Get Collateral (1 minute)

```bash
# Get recent quote
psql $DATABASE_URL -c "
SELECT tdx_quote
FROM task_attestations
WHERE tdx_quote IS NOT NULL
ORDER BY created_at DESC
LIMIT 1;
" -t | tr -d '[:space:]' > quote.hex

# Fetch collateral
dcap-qvl fetch-collateral --quote $(cat quote.hex) --output collateral.json
```

### 5. Update Collateral (30 seconds)

```bash
near call worker.outlayer.near update_collateral \
  "$(cat collateral.json | jq -c)" \
  --accountId outlayer.near \
  --gas 300000000000000
```

### 6. Create Gas Accounts (1 minute)

```bash
# One per worker
near create-account worker1.outlayer.near --masterAccount outlayer.near --initialBalance 5
near create-account worker2.outlayer.near --masterAccount outlayer.near --initialBalance 5

# Generate keys
near generate-key worker1.outlayer.near
near generate-key worker2.outlayer.near
```

---

## Done! ✅

Workers can now register their keys on startup.

**What happens when worker starts**:
1. Worker generates keypair IN TEE
2. Worker calls `worker.outlayer.near::register_worker_key()`
3. Contract verifies TDX quote
4. Contract adds key to `operator.outlayer.near`
5. Worker uses this key to sign transactions

---

## Verify Setup

```bash
# Check approved measurements
near view worker.outlayer.near get_approved_measurements

# Check collateral exists
near view worker.outlayer.near get_collateral | head -5

# Check operator account
near view worker.outlayer.near get_operator_account
```

---

## Maintenance

**Weekly**: Update collateral
```bash
dcap-qvl fetch-collateral --quote <recent-quote> -o collateral.json
near call worker.outlayer.near update_collateral "$(cat collateral.json | jq -c)" --accountId outlayer.near --gas 300000000000000
```

**On worker update**: Add new measurements (use `"clear_others": true` to replace old ones)
```bash
near call worker.outlayer.near add_approved_measurements '{"measurements":{...}, "clear_others": true}' --accountId outlayer.near
```

---

## Full Documentation

- **[README.md](./README.md)** - Complete API reference
- **[DEPLOYMENT.md](./DEPLOYMENT.md)** - Detailed deployment guide
- **[COLLATERAL_TECHNICAL.md](./COLLATERAL_TECHNICAL.md)** - Technical details on collateral
