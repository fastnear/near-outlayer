# OutLayer Monitoring

Health collector + Grafana dashboards + Telegram alerts for OutLayer infrastructure.

## Architecture

```
                    ┌──────────────────────────────────────────┐
                    │           Docker Compose                  │
                    │                                           │
                    │  ┌────────────┐   ┌──────────────────┐   │
                    │  │ collector  │──▶│  collector-db     │   │
                    │  │ (Rust)     │   │  (PostgreSQL)     │   │
                    │  └─────┬──────┘   └────────┬─────────┘   │
                    │        │                   │              │
                    │        │ Telegram    ┌─────▼──────────┐   │
                    │        │ alerts      │  Grafana       │   │
                    │        │             │  :9848          │   │
                    │        │             └────────────────┘   │
                    └────────┼─────────────────────────────────┘
                             │
              ┌──────────────▼──┐     ┌──────────────────────┐
              │ mainnet         │     │ testnet              │
              │ coordinator     │     │ coordinator          │
              │ /health/detailed│     │ /health/detailed     │
              └─────────────────┘     └──────────────────────┘
```

The **collector** polls `/health/detailed` from each coordinator every 30 seconds, stores results in its own PostgreSQL database, and sends Telegram alerts when status changes. **Grafana** reads from the same database to show dashboards.

## What `/health/detailed` checks

| Check | OK | Warning / Degraded | Critical / Unhealthy |
|-------|----|--------------------|----------------------|
| **Database** | `SELECT 1` responds | — | Connection failed (unhealthy) |
| **Redis** | `PING` responds | — | Connection failed (unhealthy) |
| **Keystore** | `GET /health` responds 200 | — | Unreachable or non-200 (degraded) |
| **Workers** | All heartbeats < 2 min | Any heartbeat > 2 min (degraded) | 0 active workers (unhealthy) |
| **Event monitor** | All workers < 100 blocks behind, updated < 5 min | Stale update or > 100 blocks behind (degraded) | — |
| **TEE attestation** | All attestations < 1 hour | Any attestation > 1 hour (degraded) | — |

## Quick start

```bash
cd coordinator/monitoring
docker compose up -d
```

This starts 3 containers:
- **collector-db** — PostgreSQL for health history
- **collector** — Rust service polling coordinators
- **grafana** — Dashboard at http://127.0.0.1:9848 (admin / change-me)

## Configuration

All config via environment variables in `docker-compose.yml`:

| Variable | Default | Description |
|----------|---------|-------------|
| `COLLECTOR_TARGETS` | `mainnet=https://api.outlayer.fastnear.com` | Comma-separated `label=url` pairs |
| `COLLECTOR_POLL_INTERVAL` | `30` | Seconds between polls |
| `COLLECTOR_RETENTION_DAYS` | `90` | How long to keep data |
| `TELEGRAM_BOT_TOKEN` | — | Telegram bot token (optional) |
| `TELEGRAM_CHAT_ID` | — | Telegram chat ID (optional) |
| `GRAFANA_ADMIN_PASSWORD` | `change-me` | Grafana admin password |

### Adding testnet

Edit `COLLECTOR_TARGETS` in docker-compose.yml:

```yaml
COLLECTOR_TARGETS: "mainnet=https://api.outlayer.fastnear.com,testnet=https://testnet-api.outlayer.fastnear.com"
```

### Telegram alerts

1. Create bot via [@BotFather](https://t.me/BotFather), get token
2. Create group, add bot, get chat ID from `https://api.telegram.org/bot<TOKEN>/getUpdates`
3. Set env vars and restart:

```bash
TELEGRAM_BOT_TOKEN=123456:ABC... TELEGRAM_CHAT_ID=-100... docker compose up -d collector
```

Alerts fire when coordinator status changes (healthy -> degraded, degraded -> unhealthy, etc).

## Grafana dashboard

The provisioned dashboard "OutLayer Health" includes:

- **Coordinator Status** — current status with color coding
- **Health Timeline** — status history over time
- **Service Latencies** — DB, Redis, Keystore response times
- **Active Workers** — count over time
- **Worker Status Timeline** — each worker shown individually
- **Worker Heartbeat Age** — with 120s threshold line
- **Block Lag** — event monitor blocks behind with 100 threshold
- **Chain Tip Block** — NEAR finalized block tracking
- **TEE Attestation Age** — with 1 hour threshold

Use the `$network` dropdown to filter by mainnet/testnet.

## Exposing via nginx

To expose Grafana publicly:

```nginx
server {
    server_name status.outlayer.ai;

    location / {
        proxy_pass http://127.0.0.1:9848;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
        proxy_http_version 1.1;
        proxy_set_header Upgrade $http_upgrade;
        proxy_set_header Connection "upgrade";
    }
}
```

```bash
sudo certbot --nginx -d grafana.outlayer.fastnear.com
sudo nginx -t && sudo systemctl reload nginx
```

## Zombie workers

Workers register in `worker_status` table via heartbeat. If a worker crashes, its record stays with the last known status.

The `/health/detailed` endpoint only includes workers with `last_heartbeat_at` within the last **24 hours**. Workers with heartbeat > 2 minutes but < 24 hours are **stale** and trigger degraded status.

### Manual cleanup

```bash
# List workers
curl -s https://api.outlayer.fastnear.com/health/detailed \
  | jq '.checks.workers.details[] | {worker_id, worker_name, status, last_heartbeat_secs_ago}'

# Delete a specific worker
curl -X DELETE \
  -H "Authorization: Bearer $ADMIN_TOKEN" \
  https://api.outlayer.fastnear.com/admin/workers/<worker_id>
```

### Bulk cleanup via SQL

```sql
DELETE FROM worker_status
WHERE last_heartbeat_at < NOW() - INTERVAL '1 hour';
```
