# 🚀 NEAR OutLayer - Deployment Guide

Complete guide to run the updated system with all new features.

## ✅ What Was Added

### 1. Coordinator Enhancements
- ✅ Worker status tracking with heartbeat
- ✅ Enhanced execution history (data_id, tx_id, user, payment)
- ✅ Public API endpoints (no auth required)
- ✅ WASM cache metadata queries

### 2. Worker Updates
- ✅ Heartbeat task (every 30 sec)
- ✅ Enhanced complete_task with all metadata
- ✅ Better error handling and logging

### 3. Dashboard (New!)
- ✅ Next.js 15 + TypeScript + Tailwind CSS
- ✅ NEAR Wallet Selector integration
- ✅ 5 pages: Home, Workers, Executions, Stats, Playground, Settings

---

## 📋 Prerequisites

- Docker & Docker Compose (for PostgreSQL + Redis)
- Rust 1.85+ with cargo
- Node.js 18+ with npm
- NEAR CLI (optional, for contract deployment)

---

## 🔧 Step-by-Step Deployment

### 1. Start Infrastructure (PostgreSQL + Redis)

```bash
cd /Users/alice/projects/near-offshore/coordinator
docker-compose up -d

# Verify services are running
docker-compose ps
```

**Expected output:**
```
offchainvm-postgres   Up (healthy)
offchainvm-redis      Up (healthy)
```

### 2. Rebuild & Restart Coordinator

```bash
cd /Users/alice/projects/near-offshore/coordinator

# Clean and rebuild (with new migrations and endpoints)
cargo clean
env SQLX_OFFLINE=true cargo build --release

# OR regenerate sqlx-data.json if database is running:
cargo sqlx prepare --database-url "postgres://postgres:postgres@localhost/offchainvm"
cargo build --release

# Stop old coordinator container if running
docker-compose stop coordinator

# Run new coordinator binary directly (easier for testing)
./target/release/offchainvm-coordinator
```

**Or rebuild Docker image:**
```bash
docker-compose build coordinator
docker-compose up -d coordinator
```

### 3. Rebuild & Restart Worker

```bash
cd /Users/alice/projects/near-offshore/worker

# Rebuild worker
cargo clean
cargo build --release

# Stop any running worker
pkill -f offchainvm-worker

# Start worker
./target/release/offchainvm-worker
```

**Check logs:**
- Should see: `Heartbeat task started (every 30 seconds)`
- Should see heartbeats every 30s: `send_heartbeat()`

### 4. Start Keystore Worker (if using encrypted secrets)

```bash
cd /Users/alice/projects/near-offshore/keystore-worker

# If using Docker
docker-compose up -d

# OR run locally
cargo run
```

### 5. Start Dashboard

```bash
cd /Users/alice/projects/near-offshore/dashboard

# Install dependencies (first time only)
npm install

# Run development server
npm run dev
```

**Dashboard will be available at:** http://localhost:3000

---

## 🧪 Testing the New Features

### Test 1: Check Public API Endpoints

```bash
# Get workers status
curl http://localhost:8080/public/workers | jq

# Get execution history
curl http://localhost:8080/public/executions | jq

# Get statistics
curl http://localhost:8080/public/stats | jq

# Check WASM cache
curl "http://localhost:8080/public/wasm/info?repo_url=https://github.com/zavodil/ai-ark&commit_hash=main&build_target=wasm32-wasip2" | jq

# Get user earnings (replace with actual account)
curl http://localhost:8080/public/users/alice.testnet/earnings | jq
```

### Test 2: Dashboard Pages

Open browser to http://localhost:3000 and navigate:

1. **Home** → Should see landing page
2. **Workers** → Should list active workers (refresh every 10s)
3. **Executions** → Should list execution history
4. **Stats** → Should show system statistics
5. **Playground** → Connect wallet and submit execution
6. **Settings** → View your earnings (requires wallet connection)

### Test 3: Submit Execution from Playground

1. Go to http://localhost:3000/playground
2. Click "Connect Wallet"
3. Fill in:
   - Repo: `https://github.com/zavodil/ai-ark`
   - Commit: `main`
   - Build Target: `wasm32-wasip2`
   - Args: `{}`
4. Click "Execute"
5. Sign transaction in wallet
6. Check execution in **Executions** page

### Test 4: Verify Worker Heartbeat

```bash
# Check coordinator logs
docker-compose logs -f coordinator

# OR if running directly
# Should see POST /workers/heartbeat every 30 seconds

# Query database directly
docker exec -it offchainvm-postgres psql -U postgres -d offchainvm -c "SELECT * FROM worker_status;"
```

### Test 5: Verify Execution History

After executing from playground:

```bash
# Check database
docker exec -it offchainvm-postgres psql -U postgres -d offchainvm -c "SELECT request_id, data_id, worker_id, success, instructions_used, resolve_tx_id FROM execution_history ORDER BY created_at DESC LIMIT 5;"
```

---

## 🔍 Troubleshooting

### Coordinator won't start

```bash
# Check if migrations are applied
cd coordinator
sqlx migrate run --database-url "postgres://postgres:postgres@localhost/offchainvm"

# Rebuild with offline mode
cargo clean
env SQLX_OFFLINE=true cargo build --release
```

### Worker not sending heartbeats

Check worker logs for errors. Worker needs:
- Valid `API_BASE_URL` in `.env`
- Valid `API_AUTH_TOKEN` (matching hash in database)

```bash
# Add worker token to database
docker exec -it offchainvm-postgres psql -U postgres -d offchainvm

INSERT INTO worker_auth_tokens (token_hash, worker_name, is_active)
VALUES (
    'cbd8f6f0e3e8ec29d3d1f58a2c8c6d6e8d7f5a4b3c2d1e0f1a2b3c4d5e6f7a8b',
    'test-worker-1',
    true
);
```

### Dashboard can't connect to API

Check `.env.local` in dashboard:
```bash
cd dashboard
cat .env.local

# Should have:
NEXT_PUBLIC_COORDINATOR_API_URL=http://localhost:8080
```

### CORS errors in dashboard

Coordinator needs CORS middleware (TODO: add if needed):
```rust
// In coordinator/src/main.rs
use tower_http::cors::{CorsLayer, Any};

let app = Router::new()
    // ... routes ...
    .layer(
        CorsLayer::new()
            .allow_origin(Any)
            .allow_methods(Any)
            .allow_headers(Any)
    );
```

---

## 📊 Database Schema Changes

New tables/columns added:

```sql
-- Worker status tracking
CREATE TABLE worker_status (
    worker_id TEXT PRIMARY KEY,
    worker_name TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'offline',
    current_task_id BIGINT,
    last_heartbeat_at TIMESTAMP NOT NULL DEFAULT NOW(),
    ...
);

-- Enhanced execution history
ALTER TABLE execution_history
ADD COLUMN data_id TEXT,
ADD COLUMN resolve_tx_id TEXT,
ADD COLUMN user_account_id TEXT,
ADD COLUMN near_payment_yocto TEXT;

-- WASM cache metadata
ALTER TABLE wasm_cache
ADD COLUMN build_target TEXT DEFAULT 'wasm32-wasip1';
```

---

## 🔐 Security Checklist

- [x] Coordinator auth enabled for worker endpoints
- [x] Public endpoints don't require auth
- [x] Keystore validates bearer tokens
- [x] Dashboard uses NEAR Wallet Selector (no private keys stored)
- [ ] TODO: Add CORS configuration for production
- [ ] TODO: Add rate limiting for public endpoints
- [ ] TODO: Add HTTPS in production

---

## 📈 Monitoring

### Check System Health

```bash
# Coordinator health
curl http://localhost:8080/health

# Keystore health (if running)
curl http://localhost:8081/health

# Database connection
docker exec -it offchainvm-postgres psql -U postgres -d offchainvm -c "SELECT version();"

# Redis connection
docker exec -it offchainvm-redis redis-cli ping
```

### Monitor Logs

```bash
# Coordinator logs
docker-compose logs -f coordinator

# Worker logs (if running as service)
journalctl -u offchainvm-worker -f

# Dashboard logs
cd dashboard && npm run dev
```

---

## 🎯 Next Steps

1. **Deploy to Production:**
   - Setup domain and SSL certificates
   - Deploy coordinator to server/cloud
   - Deploy dashboard to Vercel/Netlify
   - Use production database (managed PostgreSQL)

2. **Monitoring & Observability:**
   - Add Prometheus metrics
   - Setup Grafana dashboards
   - Add error tracking (Sentry)

3. **Additional Features:**
   - Add real-time updates (WebSockets)
   - Add execution result caching
   - Add advanced filtering in dashboard
   - Add charts/graphs for stats page

---

## 📞 Support

If something doesn't work:

1. Check logs in coordinator/worker
2. Check database state: `docker exec -it offchainvm-postgres psql -U postgres -d offchainvm`
3. Verify API responses with curl
4. Check browser console for dashboard errors

---

## ✅ Quick Verification Checklist

After deployment, verify:

- [ ] PostgreSQL is running
- [ ] Redis is running
- [ ] Coordinator is running on :8080
- [ ] Worker is running and sending heartbeats
- [ ] Dashboard is running on :3000
- [ ] Can access http://localhost:3000
- [ ] Can see workers in dashboard
- [ ] Can see stats in dashboard
- [ ] Can connect wallet in playground
- [ ] Keystore is running on :8081 (if using secrets)

**All green? You're ready to go! 🎉**
