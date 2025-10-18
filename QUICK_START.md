# NEAR Offshore - Quick Start

## TL;DR

```bash
# 1. Copy .env files
cp coordinator/.env.example coordinator/.env
cp worker/.env.example worker/.env
cp keystore-worker/.env.example keystore-worker/.env
cp dashboard/.env.example dashboard/.env.local

# 2. Edit configuration (optional)
# - coordinator/.env - internal port (PORT=8080)
# - .env (root) - external Docker ports (COORDINATOR_EXTERNAL_PORT=8080)
# - dashboard/.env.local - dashboard port (PORT=3000)

# 3. Start Docker services
docker-compose up -d

# 4. Initialize database (first time only)
cd coordinator
docker exec offchainvm-postgres psql -U postgres -c "DROP DATABASE IF EXISTS offchainvm;"
docker exec offchainvm-postgres psql -U postgres -c "CREATE DATABASE offchainvm;"
sqlx migrate run --database-url postgres://postgres:postgres@localhost/offchainvm
DATABASE_URL=postgres://postgres:postgres@localhost/offchainvm cargo sqlx prepare
docker-compose restart coordinator

# 5. Start worker (separate terminal)
cd worker
cargo run

# 6. Start dashboard (separate terminal)
cd dashboard
npm install
npm run dev
```

Dashboard available at http://localhost:3000

## Database Setup (Development Only)

**First time setup or after schema changes:**

```bash
cd coordinator

# Recreate database
docker exec offchainvm-postgres psql -U postgres -c "DROP DATABASE IF EXISTS offchainvm;"
docker exec offchainvm-postgres psql -U postgres -c "CREATE DATABASE offchainvm;"

# Apply migrations
sqlx migrate run --database-url postgres://postgres:postgres@localhost/offchainvm

# Generate sqlx offline cache (required for Docker build)
DATABASE_URL=postgres://postgres:postgres@localhost/offchainvm cargo sqlx prepare

# Rebuild coordinator
docker-compose build coordinator
docker-compose restart coordinator
```

**⚠️ Production:** Use proper migrations workflow - never drop database!

## Default Ports

| Service | Port | Configuration |
|---------|------|---------------|
| Dashboard | 3000 | `dashboard/.env.local` → `PORT=3000` |
| Coordinator API | 8080 | `coordinator/.env` → `PORT=8080` |
| Keystore Worker | 8081 | `keystore-worker/.env` → `SERVER_PORT=8081` |
| PostgreSQL | 5432 | docker-compose.yml |
| Redis | 6379 | docker-compose.yml |

## Change Docker External Ports

Edit **root `.env`**:
```env
COORDINATOR_EXTERNAL_PORT=9090
POSTGRES_EXTERNAL_PORT=15432
REDIS_EXTERNAL_PORT=16379
```

Restart services:
```bash
docker-compose up -d
```

## Change Dashboard Port

```bash
# Option 1: Specify in command
cd dashboard
PORT=4000 npm run dev

# Option 2: Export variable
export PORT=4000
cd dashboard
npm run dev

# Option 3: Create .env file (read by cross-env)
cd dashboard
echo "PORT=4000" > .env
npm run dev
```

## Network Configuration (Testnet/Mainnet)

Dashboard supports switching between testnet and mainnet.

Configure contracts in `dashboard/.env.local`:
```env
# Testnet
NEXT_PUBLIC_TESTNET_CONTRACT_ID=c2.offchainvm.testnet
NEXT_PUBLIC_TESTNET_RPC_URL=https://rpc.testnet.fastnear.com?apiKey=YOUR_API_KEY

# Mainnet
NEXT_PUBLIC_MAINNET_CONTRACT_ID=offchainvm.near
NEXT_PUBLIC_MAINNET_RPC_URL=https://rpc.mainnet.near.org

# Default network
NEXT_PUBLIC_DEFAULT_NETWORK=testnet
```

Network switcher available on **Playground** page.

## Documentation

- [AUTHENTICATION.md](AUTHENTICATION.md) - Authentication setup (production mode)
- [PORTS_CONFIGURATION.md](PORTS_CONFIGURATION.md) - Detailed port configuration
- [CLAUDE.md](CLAUDE.md) - Complete project documentation
- [contract/README.md](contract/README.md) - Contract API
- [worker/README.md](worker/README.md) - Worker configuration
