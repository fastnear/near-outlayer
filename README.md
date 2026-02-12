# NEAR OutLayer

**Verifiable off-chain computation for NEAR smart contracts using Intel TDX**

OutLayer lets smart contracts execute arbitrary code off-chain and receive verified results on-chain. Computation runs inside Intel TDX Trusted Execution Environments (TEEs) on Phala Cloud, ensuring that neither the operator nor any third party can tamper with execution or access secrets.

## Quick Links

- **Dashboard & Docs**: [outlayer.fastnear.com](https://outlayer.fastnear.com/dashboard)
- **HTTPS API**: `https://api.outlayer.fastnear.com/call/{owner}/{project}`
- **Contract (mainnet)**: `outlayer.near`
- **Production App**: [near.email](https://near.email) — blockchain-native email built on OutLayer

## How It Works

1. A NEAR smart contract (or HTTP client) submits a computation request
2. A TEE worker picks up the task, compiles/executes the WASI binary with resource limits
3. The verified result is returned on-chain (via yield/resume) or via HTTP response

## Project Structure

```
near-outlayer/
├── contract/              # Main NEAR contract (outlayer.near)
├── register-contract/     # TEE worker registration contract (5-measurement TDX verification)
├── keystore-dao-contract/ # DAO governance for keystore worker registration
├── coordinator/           # Task queue & API server (Rust + Axum, PostgreSQL + Redis)
├── worker/                # Execution workers (Rust + Tokio, wasmi runtime)
├── keystore-worker/       # Secrets decryption service (Rust, runs in TEE)
├── dashboard/             # Web UI + documentation (Next.js + React)
├── sdk/                   # OutLayer SDK for WASI apps (Rust, wasm32-wasip2)
├── wasi-examples/         # Example WASI projects
├── scripts/               # Deployment & utility scripts
├── docker/                # Docker configurations (Phala Cloud deployment)
├── tee-auth/              # TEE authentication utilities
└── tests/                 # Integration tests
```

## Two Integration Modes

### Blockchain (NEAR Smart Contracts)

Your contract calls `request_execution()` on `outlayer.near`. The result comes back via NEAR's yield/resume mechanism. Best for on-chain workflows that need verified computation.

```rust
// In your NEAR contract
#[ext_contract(ext_outlayer)]
trait OutLayer {
    fn request_execution(
        &mut self,
        execution_source: ExecutionSource,
        request_params: RequestParams,
    ) -> Promise;
}
```

### HTTPS API (Web2 Apps)

Call the API directly with a payment key. Best for web apps, bots, and services that need off-chain computation without a smart contract.

```bash
curl -X POST https://api.outlayer.fastnear.com/call/alice.near/my-project \
  -H "X-Payment-Key: alice.near:1:your_secret_key" \
  -H "Content-Type: application/json" \
  -d '{"prompt": "Hello"}'
```

## Security Model

- **Intel TDX**: Hardware-level memory encryption and isolation — the host operator cannot read TEE memory
- **5-Measurement Verification**: Workers are verified using all 5 TDX measurements (MRTD + RTMR0-3), preventing dev/debug images from passing attestation
- **Sigstore Certification**: Release binaries are cryptographically linked to source code via [Sigstore](https://www.sigstore.dev/)
- **Phala Trust Center**: Independently verify the exact image hash running in each TEE worker
- **No Compilation in TEE**: TEE workers with access to secrets only execute pre-compiled WASM, preventing supply chain attacks via malicious build scripts

## Development

### Prerequisites

- Rust 1.85+ (see `rust-toolchain.toml` in each component)
- Docker & Docker Compose
- Node.js 18+ (for dashboard)
- NEAR CLI
- `cargo-near` (for contract builds)
- `sqlx-cli` (for coordinator migrations)

### Build & Run

```bash
# Contract
cd contract && ./build.sh

# Coordinator (requires PostgreSQL + Redis)
cd coordinator && cargo run

# Worker
cd worker && cargo run

# Keystore Worker
cd keystore-worker && cargo run

# Dashboard
cd dashboard && npm install && npm run dev
```

See [QUICK_START.md](QUICK_START.md) for full setup instructions including database initialization and Docker services.

## Documentation

| Document | Description |
|----------|-------------|
| [PROJECT.md](PROJECT.md) | Complete technical specification |
| [QUICK_START.md](QUICK_START.md) | Setup and deployment guide |
| [WORKER_ATTESTATION.md](WORKER_ATTESTATION.md) | TEE attestation deep dive |
| [AUTHENTICATION.md](AUTHENTICATION.md) | Authentication configuration |
| [Onepager.md](Onepager.md) | Project overview one-pager |
| [contract/README.md](contract/README.md) | Contract API reference |
| [worker/README.md](worker/README.md) | Worker configuration |
| [wasi-examples/WASI_TUTORIAL.md](wasi-examples/WASI_TUTORIAL.md) | WASI development tutorial |
| [wasi-examples/BEST_PRACTICES_OUTLAYER_NEAR.md](wasi-examples/BEST_PRACTICES_OUTLAYER_NEAR.md) | Best practices guide |
| [dashboard/DOCS_INDEX.md](dashboard/DOCS_INDEX.md) | Dashboard documentation index |

## Default Ports

| Service | Port |
|---------|------|
| Dashboard | 3000 |
| Coordinator API | 8080 |
| Keystore Worker | 8081 |
| PostgreSQL | 5432 |
| Redis | 6379 |

## License

MIT
