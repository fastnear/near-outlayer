---
name: Agent Custody Wallet Setup
description: Register a gasless multi-chain custody wallet for an AI agent. Supports NEAR, Ethereum, Bitcoin, Solana, and other chains via NEAR Intents protocol. Use when an agent needs to send/receive tokens or call smart contracts.
metadata:
  api:
    base_url: https://api.outlayer.fastnear.com
    version: v1
    auth: Bearer token
---

# OutLayer Wallet Setup

You are an AI agent that needs a custody wallet for multi-chain operations (transfers, contract calls, DeFi). The wallet uses NEAR Intents protocol for gasless cross-chain transactions — the agent doesn't need native gas tokens on destination chains. Follow these steps to register, fund, and configure your wallet.

## When to Use This Skill

| You need... | Action |
|-------------|--------|
| A crypto wallet for your agent | Register via `POST /register` |
| Send tokens cross-chain (gasless) | Use `POST /wallet/v1/withdraw` — no gas tokens needed on destination chain |
| Call a NEAR smart contract | Use `POST /wallet/v1/call` — requires NEAR balance for gas |
| Get your address on any chain | Use `GET /wallet/v1/address?chain=ethereum` |
| Let the user set spending limits | Share the `handoff_url` from registration |

## Configuration

- **API Base URL**: `https://api.outlayer.fastnear.com`
- **Dashboard**: `https://outlayer.fastnear.com`
- **Network**: mainnet (for testnet use `COORDINATOR_URL` env var)

## Step 1: Register Wallet

Call the registration endpoint. No auth required.

```bash
curl -s -X POST https://api.outlayer.fastnear.com/register ```

Response:
```json
{
  "api_key": "wk_15807dbda492636df5280629d7617c3ea80f915ba960389b621e420ca275e545",
  "near_account_id": "36842e2f73d0b7b2f2af6e0d94a7a997398c2c09d9cf09ca3fa23b5426fccf88",
  "handoff_url": "https://outlayer.fastnear.com/wallet?key=wk_..."
}
```

**Save `api_key` securely** — it is shown only once. All subsequent requests require it.

The `near_account_id` is the NEAR implicit account (hex public key). Cross-chain transfers (Ethereum, Bitcoin, Solana, etc.) are handled via NEAR Intents — no gas tokens needed on other chains.

## Step 2: Request Funding from User (Optional)

Funding is only needed for direct NEAR contract calls (`/wallet/v1/call`). Cross-chain operations via Intents (`/wallet/v1/withdraw`) are gasless — no NEAR balance required. If the agent needs to call NEAR contracts, ask the user to send at least 0.1 NEAR.

**Message to user:**

> Your wallet address is `{address}`. Please send at least 0.1 NEAR to fund it:
>
> ```
> near send YOUR_ACCOUNT.near {address} 0.1 --networkId mainnet
> ```
>
> Or transfer from any NEAR wallet to: `{address}`

## Step 3: Request Policy from User

A policy defines spending limits, address whitelists, and multisig rules for the wallet. Without a policy, the wallet operates without restrictions.

**Available policy types:**

| Policy | Description |
|--------|-------------|
| **Spending limits** | Cap per-transaction, hourly, daily, or monthly amounts (in USD) |
| **Address whitelist/blacklist** | Restrict which addresses the agent can send to |
| **Allowed tokens** | Limit which tokens the agent can transfer (default: all) |
| **Transaction types** | Restrict to `withdraw` only, `call` only, or both |
| **Time restrictions** | Allow operations only during certain hours/days (UTC) |
| **Rate limit** | Max transactions per hour |
| **Multisig approval** | Require human approval for transactions above a USD threshold |
| **Authorized API keys** | Control which API keys can operate the wallet |
| **Webhook** | Notify an external URL on every transaction |

**Message to user:**

> Please configure a security policy for your wallet. Open the dashboard:
>
> https://outlayer.fastnear.com/wallet?key={api_key}
>
> You can set spending limits, address whitelists, multisig approval thresholds, and more.

## Quick Reference

| Action | Method | Endpoint |
|--------|--------|----------|
| Register | POST | `https://api.outlayer.fastnear.com/register` |
| Get address | GET | `https://api.outlayer.fastnear.com/wallet/v1/address?chain={chain}` |
| Call contract | POST | `https://api.outlayer.fastnear.com/wallet/v1/call` |
| Withdraw / transfer | POST | `https://api.outlayer.fastnear.com/wallet/v1/withdraw` |
| Dry-run (check first) | POST | `https://api.outlayer.fastnear.com/wallet/v1/withdraw/dry-run` |
| Request status | GET | `https://api.outlayer.fastnear.com/wallet/v1/requests/{request_id}` |
| List tokens | GET | `https://api.outlayer.fastnear.com/wallet/v1/tokens` |
| Audit log | GET | `https://api.outlayer.fastnear.com/wallet/v1/audit?limit=50` |

All endpoints except `/register` require `Authorization: Bearer <api_key>` header.

## After Setup

Once funded and (optionally) policy-configured, you can use these endpoints:

### Get address (for other chains)
```bash
curl -s -H "Authorization: Bearer $API_KEY" \
  "https://api.outlayer.fastnear.com/wallet/v1/address?chain=ethereum" ```
Supported chains: `near`, `ethereum`, `solana`, `bitcoin`, etc.

### Check balance
Query NEAR RPC directly with the implicit address.

### Call a contract
```bash
curl -s -X POST -H "Content-Type: application/json" \
  -H "Authorization: Bearer $API_KEY" \
  -d '{"receiver_id":"wrap.near","method_name":"near_deposit","args":{},"deposit":"10000000000000000000000"}' \
  "https://api.outlayer.fastnear.com/wallet/v1/call" ```

### Withdraw / transfer
```bash
curl -s -X POST -H "Content-Type: application/json" \
  -H "Authorization: Bearer $API_KEY" \
  -d '{"to":"receiver.near","amount":"1000000000000000000000000","token":"wrap.near","chain":"near"}' \
  "https://api.outlayer.fastnear.com/wallet/v1/withdraw" ```

### Dry-run (check without executing)
```bash
curl -s -X POST -H "Content-Type: application/json" \
  -H "Authorization: Bearer $API_KEY" \
  -d '{"to":"receiver.near","amount":"1000000000000000000000000","token":"wrap.near","chain":"near"}' \
  "https://api.outlayer.fastnear.com/wallet/v1/withdraw/dry-run" ```

### Check request status
```bash
curl -s -H "Authorization: Bearer $API_KEY" \
  "https://api.outlayer.fastnear.com/wallet/v1/requests/{request_id}" ```

### List available tokens
```bash
curl -s -H "Authorization: Bearer $API_KEY" \
  "https://api.outlayer.fastnear.com/wallet/v1/tokens" ```

### View audit log
```bash
curl -s -H "Authorization: Bearer $API_KEY" \
  "https://api.outlayer.fastnear.com/wallet/v1/audit?limit=50" ```

## Error Handling

| Error | Meaning |
|-------|---------|
| `missing_auth` | No `Authorization: Bearer` header |
| `invalid_api_key` | Key revoked or not found |
| `policy_denied` | Operation blocked by policy rules |
| `wallet_frozen` | Wallet frozen by controller |
| `insufficient_balance` | Not enough funds |
| `pending_approval` | Needs multisig approval (not an error) |

## Guidelines

- Always use `withdraw/dry-run` before real withdrawals to check policy and balance
- Store the API key as a secret — never log or expose it
- If a request returns `pending_approval`, inform the user and provide the dashboard link
- Poll `/requests/{id}` for async operation status (withdraw, call)
- NEAR amounts are in yoctoNEAR (1 NEAR = 10^24 yoctoNEAR)
