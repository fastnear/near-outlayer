# Admin API

Admin endpoints require `Authorization: Bearer $ADMIN_BEARER_TOKEN` header.

Set token via environment variable:
```bash
export ADMIN_BEARER_TOKEN="your-secret-token"
```

## Grant Keys

Grant keys are non-withdrawable payment keys for developers:
- Compute usage charged normally
- Cannot use `X-Attached-Deposit` (no earnings transfer)
- Created by admin only, not synced from contract

### Create Grant Key

```bash
curl -X POST http://localhost:8080/admin/grant-keys \
  -H "Authorization: Bearer $ADMIN_BEARER_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "owner": "zavodil2.testnet",
    "initial_balance": "10000000",
    "project_ids": ["zavodil2.testnet/test-storage"],
    "max_per_call": "1000000"
  }'
```

### Try it
```
curl -X POST http://localhost:8080/call/zavodil2.testnet/test-storage \
            -H "Content-Type: application/json" \
            -H "X-Payment-Key: zavodil2.testnet:0:7f8f254f6ea2c728cd287974e38636c0ab660d7a7b62dadd3a79e77652b75558" \
            -d '{
      "input": {"command":"test_all"},
      "resource_limits": {
        "max_instructions": 500000000,
        "max_execution_seconds": 30
      }
    }'
```

**Parameters:**
| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `owner` | string | yes | NEAR account ID that will use this key |
| `initial_balance` | string | yes | Balance in minimal units (1000000 = $1.00) |
| `project_ids` | string[] | no | Allowed projects (empty = all allowed) |
| `max_per_call` | string | no | Max per API call (null = unlimited) |

**Response:**
```json
{
  "owner": "developer.near",
  "nonce": 0,
  "key": "a1b2c3d4e5f6...",
  "initial_balance": "10000000",
  "project_ids": ["alice.near/my-app"],
  "max_per_call": "1000000"
}
```

**Important:** Save the `key` value! It's only shown once.

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

```bash
curl -X DELETE http://localhost:8080/admin/grant-keys/developer.near/0 \
  -H "Authorization: Bearer $ADMIN_BEARER_TOKEN"
```

**Response:** `204 No Content` on success.

## Using Grant Key (for developers)

Developer uses the grant key like a normal payment key:

```bash
curl -X POST http://localhost:8080/call/alice.near/my-app \
  -H "X-Payment-Key: developer.near:0:a1b2c3d4e5f6..." \
  -H "X-Compute-Limit: 100000" \
  -H "Content-Type: application/json" \
  -d '{"input": {"foo": "bar"}}'
```

**Note:** `X-Attached-Deposit` header is forbidden for grant keys (returns 403 error).

## Other Admin Endpoints

### Get Compile Logs

```bash
curl http://localhost:8080/admin/compile-logs/123 \
  -H "Authorization: Bearer $ADMIN_BEARER_TOKEN"
```

Returns raw stderr/stdout from compilation (for debugging failed builds).
