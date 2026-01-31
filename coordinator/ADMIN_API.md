# Admin API

Admin endpoints require `Authorization: Bearer $ADMIN_BEARER_TOKEN` header.

Set token via environment variable:
```bash
export ADMIN_BEARER_TOKEN="your-secret-token"
```

## Grant Payment Keys

Grant keys are payment keys funded by admin that:
- Cannot use `X-Attached-Deposit` (no earnings transfer to developers)
- Compute usage is charged normally (only for gas/compute)
- Cannot be withdrawn
- Balance can only be used by the key owner (who controls the secret)

### Security Model

**Admin cannot create new keys** - only grant balance to EXISTING keys:
1. User creates a payment key via `store_secrets()` (they control the secret)
2. Admin grants balance to the key via `POST /admin/grant-payment-key`
3. Admin does NOT know the secret key - cannot use the key themselves
4. Key is marked `is_grant=true` - cannot withdraw or use X-Attached-Deposit

This prevents admin from impersonating users or accessing their secrets.

### Grant Balance to Existing Key

```bash
curl -X POST http://localhost:8080/admin/grant-payment-key \
  -H "Authorization: Bearer $ADMIN_BEARER_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "owner": "zavodil2.testnet",
    "nonce": 1,
    "amount": "10000000"
  }'
```

**Requirements:**
- Key must already exist (user created via `store_secrets`)
- For new grants: key must have zero balance (not yet funded by user)
- For top-ups: key must already be a grant key (`is_grant=true`)
- Cannot grant to user-funded keys (balance > 0 and not a grant)

**Parameters:**
| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `owner` | string | yes | NEAR account ID that owns the key |
| `nonce` | number | yes | Payment key nonce |
| `amount` | string | yes | Amount to add in minimal units (1000000 = $1.00) |
| `note` | string | no | Admin note for reference |

**Response:**
```json
{
  "owner": "zavodil2.testnet",
  "nonce": 1,
  "initial_balance": "10000000",
  "is_grant": true
}
```

Note: `initial_balance` is the new total balance (existing + added amount).

### List Grant Keys

```bash
curl http://localhost:8080/admin/grant-keys \
  -H "Authorization: Bearer $ADMIN_BEARER_TOKEN"
```

**Response:**
```json
[
  {
    "owner": "developer.near",
    "nonce": 0,
    "initial_balance": "10000000",
    "spent": "2500000",
    "reserved": "0",
    "available": "7500000",
    "project_ids": ["alice.near/my-app"],
    "max_per_call": "1000000",
    "created_at": "2025-01-21 10:30:00"
  }
]
```

### Delete Grant Key

Use when a grant is finished and the key is no longer needed.

```bash
curl -X DELETE http://localhost:8080/admin/grant-keys/developer.near/0 \
  -H "Authorization: Bearer $ADMIN_BEARER_TOKEN"
```

**Response:** `204 No Content` on success.

## Using Grant Key (for developers)

Developer uses the grant key like a normal payment key:

```bash
curl -X POST http://localhost:8080/call/alice.near/my-app \
  -H "X-Payment-Key: developer.near:0:your-secret-key" \
  -H "X-Compute-Limit: 100000" \
  -H "Content-Type: application/json" \
  -d '{"input": {"foo": "bar"}}'
```

**Note:** `X-Attached-Deposit` header is forbidden for grant keys (returns 403 error).

### Check Balance

Developer can check their balance (requires the secret key):

```bash
curl http://localhost:8080/payment-keys/balance \
  -H "X-Payment-Key: developer.near:0:your-secret-key"
```

**Response:**
```json
{
  "owner": "developer.near",
  "nonce": 0,
  "initial_balance": "10000000",
  "spent": "2500000",
  "reserved": "0",
  "available": "7500000",
  "last_used_at": "2025-01-21T12:00:00Z",
  "is_grant": true
}
```

## Other Admin Endpoints

### Get Compile Logs

```bash
curl http://localhost:8080/admin/compile-logs/123 \
  -H "Authorization: Bearer $ADMIN_BEARER_TOKEN"
```

Returns raw stderr/stdout from compilation (for debugging failed builds).
