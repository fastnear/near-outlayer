# Register Contract - Quick Start

**5-minute setup guide**

---

## Prerequisites

- NEAR CLI installed
- `dcap-qvl` CLI installed
- One worker deployed to Phala (to get RTMR3)

---

## Setup

### 1. Build & Deploy (2 minutes)

```bash
cd register-contract
./build.sh

near deploy register.outlayer.near \
  use-file res/local/register_contract.wasm \
  with-init-call new \
  json-args '{"owner_id":"outlayer.near","operator_account_id":"operator.outlayer.near"}' \
  prepaid-gas '100.0 Tgas' network-config mainnet sign-with-keychain send
```

### 2. Get RTMR3 (1 minute)

```bash
# From coordinator database (after first worker deployment)
psql $DATABASE_URL -c "
SELECT last_seen_rtmr3
FROM worker_auth_tokens
WHERE last_seen_rtmr3 IS NOT NULL
LIMIT 1;
" -t | tr -d '[:space:]'
```

### 3. Add RTMR3 (30 seconds)

```bash
near call register.outlayer.near add_approved_rtmr3 \
  '{"rtmr3":"<rtmr3-from-step-2>"}' \
  --accountId outlayer.near
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
near call register.outlayer.near update_collateral \
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
2. Worker calls `register.outlayer.near::register_worker_key()`
3. Contract verifies TDX quote
4. Contract adds key to `operator.outlayer.near`
5. Worker uses this key to sign transactions

---

## Verify Setup

```bash
# Check approved RTMR3
near view register.outlayer.near get_approved_rtmr3

# Check collateral exists
near view register.outlayer.near get_collateral | head -5

# Check operator account
near view register.outlayer.near get_operator_account
```

---

## Maintenance

**Weekly**: Update collateral
```bash
dcap-qvl fetch-collateral --quote <recent-quote> -o collateral.json
near call register.outlayer.near update_collateral "$(cat collateral.json | jq -c)" --accountId outlayer.near --gas 300000000000000
```

**On worker update**: Add new RTMR3
```bash
near call register.outlayer.near add_approved_rtmr3 '{"rtmr3":"<new-rtmr3>"}' --accountId outlayer.near
```

---

## Full Documentation

- **[README.md](./README.md)** - Complete API reference
- **[DEPLOYMENT.md](./DEPLOYMENT.md)** - Detailed deployment guide
- **[COLLATERAL_TECHNICAL.md](./COLLATERAL_TECHNICAL.md)** - Technical details on collateral
