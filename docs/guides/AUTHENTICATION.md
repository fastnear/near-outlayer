# Authentication Configuration

## Overview

The Coordinator API supports two authentication modes:

1. **Development Mode** (`REQUIRE_AUTH=false`) - No authentication required
2. **Production Mode** (`REQUIRE_AUTH=true`) - Bearer token authentication required for protected endpoints

**Important:** Public endpoints (`/public/*` and `/health`) are **always accessible without authentication**, even when `REQUIRE_AUTH=true`.

---

## Authentication Modes

### Development Mode (Default)

In development mode, all endpoints are accessible without authentication:

```env
# coordinator/.env
REQUIRE_AUTH=false
```

**Use case:** Local development, testing

**Security:** ⚠️ Not suitable for production

### Production Mode

In production mode, protected endpoints require valid Bearer tokens:

```env
# coordinator/.env
REQUIRE_AUTH=true
```

**Use case:** Production deployment, public-facing servers

**Security:** ✅ Recommended for production

---

## Endpoint Types

### Public Endpoints (No Auth Required)

These endpoints are **always accessible** without authentication:

- `GET /health` - Health check
- `GET /public/workers` - List workers
- `GET /public/executions` - List execution history
- `GET /public/stats` - System statistics
- `GET /public/wasm/info` - Check WASM cache
- `GET /public/users/:account/earnings` - User statistics

**Dashboard uses only public endpoints**, so it works regardless of auth mode.

### Protected Endpoints (Auth Required in Production)

These endpoints require Bearer token authentication when `REQUIRE_AUTH=true`:

- `GET /tasks/poll` - Poll for tasks (workers)
- `POST /tasks/complete` - Complete task (workers)
- `POST /tasks/fail` - Fail task (workers)
- `POST /tasks/create` - Create task (event monitor)
- `POST /wasm/upload` - Upload WASM (workers)
- `GET /wasm/:checksum` - Download WASM (workers)
- `POST /locks/acquire` - Acquire distributed lock (workers)
- `DELETE /locks/release/:key` - Release lock (workers)
- `POST /workers/heartbeat` - Worker heartbeat

---

## Enabling Authentication (Production Setup)

### Step 1: Generate Token Hash

Generate a SHA256 hash of your secret token:

```bash
# Choose a strong random token
TOKEN="my-secret-worker-token-$(openssl rand -hex 16)"
echo "Your token: $TOKEN"

# Generate SHA256 hash
HASH=$(echo -n "$TOKEN" | shasum -a 256 | awk '{print $1}')
echo "Token hash: $HASH"
```

**Example output:**
```
Your token: my-secret-worker-token-abc123def456
Token hash: cbd8f6f0e3e8ec29d3d1f58a2c8c6d6e8d7f5a4b3c2d1e0f1a2b3c4d5e6f7a8b
```

**⚠️ Important:** Save the **token** (not the hash) securely - you'll need it for worker configuration.

### Step 2: Add Token Hash to Database

Connect to PostgreSQL and insert the token hash:

```bash
# Connect to database
docker exec -it offchainvm-postgres psql -U postgres -d offchainvm
```

```sql
-- Insert worker token
INSERT INTO worker_auth_tokens (token_hash, worker_name, is_active)
VALUES (
    'cbd8f6f0e3e8ec29d3d1f58a2c8c6d6e8d7f5a4b3c2d1e0f1a2b3c4d5e6f7a8b',  -- Token hash
    'production-worker-1',  -- Worker identifier
    true  -- Active
);

-- Verify
SELECT token_hash, worker_name, is_active, created_at FROM worker_auth_tokens;
```

**Output:**
```
                              token_hash                              |    worker_name      | is_active |         created_at
----------------------------------------------------------------------+---------------------+-----------+----------------------------
 cbd8f6f0e3e8ec29d3d1f58a2c8c6d6e8d7f5a4b3c2d1e0f1a2b3c4d5e6f7a8b | production-worker-1 | t         | 2025-10-15 19:30:45.123456
```

### Step 3: Enable Auth in Coordinator

Edit `coordinator/.env`:

```env
# Enable authentication
REQUIRE_AUTH=true
```

Restart coordinator:

```bash
docker-compose restart coordinator
```

### Step 4: Configure Workers

Edit `worker/.env` and add the **original token** (not the hash):

```env
# Coordinator API Configuration
API_BASE_URL=http://localhost:8080
API_AUTH_TOKEN=my-secret-worker-token-abc123def456  # ← Original token
```

Restart worker:

```bash
cd worker
cargo run
```

### Step 5: Configure Keystore Worker (Optional)

If using encrypted secrets, configure keystore worker:

Edit `keystore-worker/.env`:

```env
# Generate separate token for keystore
# Use the same process as Step 1
KEYSTORE_AUTH_TOKEN=my-secret-keystore-token-xyz789
```

Add keystore token hash to database:

```sql
INSERT INTO worker_auth_tokens (token_hash, worker_name, is_active)
VALUES (
    'sha256_hash_of_keystore_token',
    'keystore-worker',
    true
);
```

---

## Testing Authentication

### Test Public Endpoint (Should Work)

```bash
# No authentication required
curl http://localhost:8080/public/workers
```

**Expected:** Returns worker list (JSON)

### Test Protected Endpoint Without Token (Should Fail)

```bash
curl http://localhost:8080/tasks/poll
```

**Expected (when REQUIRE_AUTH=true):**
```
Unauthorized
```

### Test Protected Endpoint With Token (Should Work)

```bash
curl -H "Authorization: Bearer my-secret-worker-token-abc123def456" \
     http://localhost:8080/tasks/poll?timeout=5
```

**Expected:** Returns task or empty response after timeout

---

## Managing Tokens

### List All Tokens

```sql
SELECT
    id,
    LEFT(token_hash, 12) || '...' as token_hash_preview,
    worker_name,
    is_active,
    last_used_at,
    created_at
FROM worker_auth_tokens
ORDER BY created_at DESC;
```

### Disable Token

```sql
-- Disable token without deleting
UPDATE worker_auth_tokens
SET is_active = false
WHERE worker_name = 'production-worker-1';
```

### Re-enable Token

```sql
UPDATE worker_auth_tokens
SET is_active = true
WHERE worker_name = 'production-worker-1';
```

### Delete Token

```sql
-- Permanently delete token
DELETE FROM worker_auth_tokens
WHERE worker_name = 'production-worker-1';
```

### Rotate Token

```bash
# 1. Generate new token and hash
NEW_TOKEN="my-new-secret-token-$(openssl rand -hex 16)"
NEW_HASH=$(echo -n "$NEW_TOKEN" | shasum -a 256 | awk '{print $1}')

# 2. Update database
docker exec -it offchainvm-postgres psql -U postgres -d offchainvm -c "
UPDATE worker_auth_tokens
SET token_hash = '$NEW_HASH'
WHERE worker_name = 'production-worker-1';
"

# 3. Update worker/.env with NEW_TOKEN
# 4. Restart worker
```

---

## Security Best Practices

### 1. Use Strong Random Tokens

```bash
# Generate cryptographically secure random token
openssl rand -base64 32
```

### 2. Never Commit Tokens to Git

Add to `.gitignore`:
```gitignore
.env
.env.local
*.secret
```

### 3. Use Different Tokens Per Worker

Each worker should have a unique token for auditing and revocation:

```sql
-- Worker 1
INSERT INTO worker_auth_tokens (token_hash, worker_name, is_active)
VALUES ('hash1', 'worker-1', true);

-- Worker 2
INSERT INTO worker_auth_tokens (token_hash, worker_name, is_active)
VALUES ('hash2', 'worker-2', true);
```

### 4. Rotate Tokens Regularly

Rotate tokens every 30-90 days in production.

### 5. Monitor Token Usage

Check `last_used_at` to detect unused or compromised tokens:

```sql
SELECT
    worker_name,
    last_used_at,
    AGE(NOW(), last_used_at) as inactive_duration
FROM worker_auth_tokens
WHERE is_active = true
ORDER BY last_used_at DESC NULLS LAST;
```

### 6. Use HTTPS in Production

Always use HTTPS when `REQUIRE_AUTH=true`:

```env
# Worker configuration
API_BASE_URL=https://coordinator.example.com  # ← HTTPS
```

### 7. Restrict Network Access

Use firewall rules to restrict coordinator access:

```bash
# Example: Allow only specific IPs
iptables -A INPUT -p tcp --dport 8080 -s 10.0.1.0/24 -j ACCEPT
iptables -A INPUT -p tcp --dport 8080 -j DROP
```

---

## Troubleshooting

### Worker: "Unauthorized" Error

**Cause:** Token mismatch or auth enabled without token

**Solution:**
1. Verify token in `worker/.env` matches hash in database
2. Check token is active: `SELECT is_active FROM worker_auth_tokens WHERE worker_name = 'your-worker';`
3. Verify coordinator has `REQUIRE_AUTH=true`

### Worker: "Failed to authenticate"

**Cause:** Token hash not found in database

**Solution:**
```sql
-- Generate and insert hash
INSERT INTO worker_auth_tokens (token_hash, worker_name, is_active)
VALUES ('your_token_hash', 'worker-name', true);
```

### Dashboard: Can't Load Data

**Cause:** This should NOT happen - public endpoints don't require auth

**Solution:**
1. Check CORS is enabled (see coordinator logs)
2. Verify dashboard `.env.local` has correct `NEXT_PUBLIC_COORDINATOR_API_URL`
3. Check browser console for errors

### Logs Show "Auth disabled (dev mode)"

**Cause:** `REQUIRE_AUTH=false` in `coordinator/.env`

**Solution:**
```bash
# Edit coordinator/.env
REQUIRE_AUTH=true

# Restart coordinator
docker-compose restart coordinator
```

---

## Migration from Dev to Production

### Quick Checklist

- [ ] Generate strong random tokens (≥32 characters)
- [ ] Insert token hashes into database
- [ ] Set `REQUIRE_AUTH=true` in `coordinator/.env`
- [ ] Update all worker `.env` files with tokens
- [ ] Test protected endpoints with tokens
- [ ] Enable HTTPS (recommended)
- [ ] Set up firewall rules (recommended)
- [ ] Enable monitoring and alerting
- [ ] Document token rotation procedure

---

## Additional Resources

- [PORTS_CONFIGURATION.md](PORTS_CONFIGURATION.md) - Port configuration guide
- [QUICK_START.md](QUICK_START.md) - Quick start guide
- [coordinator/README.md](coordinator/README.md) - Coordinator API documentation
- [worker/README.md](worker/README.md) - Worker configuration guide
