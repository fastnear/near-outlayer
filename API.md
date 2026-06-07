# OutLayer API

The OutLayer HTTP API is served by the **coordinator** — the task queue and
gateway that fronts the TEE workers, contracts, and keystore. It exposes
verifiable off-chain computation (execute WASI modules) and the Agent Custody
wallet over plain HTTPS.

## Base URLs (Networks)

| Network | Base URL | Contract |
|---------|----------|----------|
| Mainnet | `https://api.outlayer.fastnear.com` | `outlayer.near` |
| Testnet | `https://testnet-api.outlayer.fastnear.com` | `outlayer.testnet` |

The paths below are identical on both networks — only the host differs. Pick the
base URL that matches the network your project / wallet is deployed on.

> **NEAR Intents is mainnet-only.** Testnet does not run the Intents solver
> network, so the intents-dependent Agent Custody endpoints are **not available
> on testnet**: every `/wallet/v1/intents/*` route, cross-chain gasless
> withdrawals, and all `/wallet/v1/confidential/*` routes. Test those against
> the **mainnet** API only. The rest of the wallet API (address, balance,
> transfer, `call`, `sign-message`, policy, approval) works on both networks.

- **Interactive reference (Scalar UI)**: `https://api.outlayer.fastnear.com/docs`
- **OpenAPI 3.1 spec**: `https://api.outlayer.fastnear.com/openapi.json` — source of truth at [out-layer/api-spec](https://github.com/out-layer/api-spec)
- **TypeScript SDK**: [`@outlayer/sdk`](https://www.npmjs.com/package/@outlayer/sdk) ([source](https://github.com/out-layer/sdk-js))

## Authentication

| Header | Used by | Meaning |
|--------|---------|---------|
| `X-Payment-Key: owner:nonce:secret` | Paid execution calls | Prepaid USD (stablecoin) balance |
| `Authorization: Bearer wk_...` | Trial calls + all wallet endpoints | Wallet API key (free trial quota for `/call`) |
| _(none)_ | `/register`, public read endpoints | No auth |

> Only `X-Payment-Key` (paid) or `Authorization: Bearer wk_...` (trial / wallet)
> are accepted for authenticated calls. There is no `X-API-Key` header.

## Execution API

| Method | Endpoint | Auth | Description |
|--------|----------|------|-------------|
| POST | `/call/{owner}/{project}` | `X-Payment-Key` or `Bearer wk_...` | Execute a WASI module (sync response) |
| GET | `/calls/{call_id}` | — | Poll an async execution by id |
| GET | `/trial/status` | `Bearer wk_...` | Check remaining free trial quota |

Optional execution headers: `X-Compute-Limit` (max compute budget in USD
micro-units), `X-Attached-Deposit` (payment forwarded to the project author,
read by the WASM via the `USD_PAYMENT` env var).

## Agent Custody Wallet API

Deterministic per-agent wallets derived inside the TEE via NEAR MPC.
All endpoints require `Authorization: Bearer wk_...`.

| Method | Endpoint | Description |
|--------|----------|-------------|
| POST | `/register` | Register a wallet, get API key + trial quota (no auth) |
| GET | `/wallet/v1/balance` | Balance (NEAR or FT), per chain |
| GET | `/wallet/v1/address` | Address for any supported chain |
| GET | `/wallet/v1/tokens` | List supported tokens |
| POST | `/wallet/v1/transfer` | Transfer NEAR / FT |
| POST | `/wallet/v1/call` | Call a NEAR smart contract |
| POST | `/wallet/v1/sign-message` | NEP-413 message signing (`format:"raw"` removed — use `/auth-sign`) |
| POST | `/wallet/v1/auth-sign` | OutLayer NEAR-key auth signature (`{purpose, seed, vault_id?}`) |
| POST | `/wallet/v1/intents/deposit` | Deposit FT into Intents balance · **mainnet only** |
| POST | `/wallet/v1/intents/withdraw` | Withdrawal — same-chain (native NEAR / NEP-141) or cross-chain (gasless); `/dry-run` available · **mainnet only** |
| POST | `/wallet/v1/intents/swap` | Swap tokens via Intents; `/swap/quote` for a quote · **mainnet only** |
| POST | `/wallet/v1/create-payment-key` | Upgrade trial → paid (USDC or NEAR deposit) |
| POST | `/wallet/v1/policy` · `/sign-policy` · `/encrypt-policy` | Policy engine (spend limits, allowlists) |
| GET/POST | `/wallet/v1/approval/*` · `/approve/*` · `/reject/*` · `/pending_approvals*` | Multisig approval flow |
| GET | `/wallet/v1/requests/{id}` | Request status |
| GET | `/wallet/v1/audit` | Audit log |
| POST | `/wallet/v1/delete` | Delete wallet |

### Confidential Intents (private balances)

**Mainnet only** — built on NEAR Intents, so unavailable on the testnet API.

| Method | Endpoint | Description |
|--------|----------|-------------|
| POST | `/wallet/v1/confidential/shield` | Shield funds into the private shard (legacy alias `/wallet/v1/confidential/deposit`, still works) |
| GET | `/wallet/v1/confidential/balance` | Private balance |
| POST | `/wallet/v1/confidential/transfer` | Private transfer |
| POST | `/wallet/v1/confidential/swap` | Private swap; `/swap/quote` for a quote |
| POST | `/wallet/v1/confidential/withdraw` | Withdraw (incl. native NEAR); `/dry-run` available |
| POST | `/wallet/v1/confidential/unshield` | Move back to public balance |

### Payment Checks (gasless agent-to-agent payments)

`/wallet/v1/payment-check/{create,batch-create,claim,reclaim,peek,status,list}`
— see [docs/PAYMENT_CHECKS.md](docs/PAYMENT_CHECKS.md).

## Public (read-only, no auth)

| Method | Endpoint | Description |
|--------|----------|-------------|
| GET | `/health` | Health check |
| GET | `/public/pricing` | Current pricing |
| GET | `/public/stats` · `/public/workers` | Network stats / live workers |
| GET | `/public/storage/get` · `/public/storage/batch` | Read public (unencrypted) project storage |
| GET | `/public/payment-keys/{owner}/{nonce}/balance` · `/usage` | Payment key balance / usage |
| GET | `/public/project-earnings/{owner}` · `/users/{account}/earnings` | Earnings (read) |
| GET | `/vrf/pubkey` | VRF public key (for on-chain verification) |
| GET | `/tdx/collateral` | TDX attestation collateral |

## Source availability

**The coordinator (this API) is currently closed source.** It is purely the
**coordination layer** — task queue, WASM cache, payment accounting, and the
HTTPS gateway that routes requests to the verifiable components. It holds no
authority that you have to trust: it cannot read TEE memory, cannot forge
attestations, and cannot tamper with execution results.

**Every component that requires verification is open source:**

- **Workers** — execute WASI modules inside Intel TDX TEEs ([worker/](worker/))
- **Contracts** — `outlayer.near` and friends ([contract/](contract/),
  [register-contract/](register-contract/),
  [keystore-dao-contract/](keystore-dao-contract/), [vault-contract/](vault-contract/))
- **Keystore worker** — secrets decryption inside the TEE ([keystore-worker/](keystore-worker/))
- **Libraries / SDKs** — [sdk/](sdk/) (Rust, on crates.io as
  [`outlayer`](https://crates.io/crates/outlayer)),
  [`@outlayer/sdk`](https://github.com/out-layer/sdk-js) (TypeScript),
  [shared-tee-helpers](https://github.com/out-layer/shared-tee-helpers)
  (TEE challenge-response auth)

Trust in OutLayer comes from TEE attestation of the open-source workers and
from the on-chain contracts — not from trusting the coordinator. The
coordinator being closed source does not widen the trust boundary: a malicious
coordinator can withhold or misroute a request, but it cannot produce a result
that passes attestation without the genuine open-source worker having computed
it. See [README.md](README.md#security-model) and
[WORKER_ATTESTATION.md](WORKER_ATTESTATION.md).
