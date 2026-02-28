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
| Send NEAR to someone | Use `POST /wallet/v1/transfer` with `chain: "near"` |
| Send FT tokens (USDT, wNEAR) to someone | Use `POST /wallet/v1/call` with `ft_transfer` (see FT transfer section) |
| Send tokens cross-chain (gasless) | Use `POST /wallet/v1/intents/withdraw` — no gas tokens needed on destination chain |
| Delete the wallet | Use `POST /wallet/v1/delete` — deletes on-chain account, sends NEAR to beneficiary, revokes API keys |
| Swap tokens (e.g. wNEAR → USDT) | Use `POST /wallet/v1/intents/swap` — atomic swap via 1Click API, output tokens delivered to wallet |
| Deposit tokens into Intents balance | Use `POST /wallet/v1/intents/deposit` — for manual intents operations |
| Call a NEAR smart contract | Use `POST /wallet/v1/call` — requires NEAR balance for gas |
| Check your balance | Use `GET /wallet/v1/balance?chain=near` or `&token=usdt.tether-token.near` |
| Get your address on any chain | Use `GET /wallet/v1/address?chain=ethereum` |
| Ask user to fund your wallet | Generate a fund link (see below) or share your NEAR address |
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

**Important:** Persist the `api_key` to a file or session state immediately after registration. If you lose the key, recovery depends on the user having set a policy (see "Key Recovery" below). Without the key, wallet access is lost.

The `near_account_id` is the NEAR implicit account (hex public key). Cross-chain transfers (Ethereum, Bitcoin, Solana, etc.) are handled via NEAR Intents — no gas tokens needed on other chains.

## Step 2: Request Funding from User

NEAR balance is needed for on-chain operations (`/wallet/v1/call`, `/wallet/v1/intents/swap`, `/wallet/v1/transfer`). Cross-chain operations via Intents (`/wallet/v1/intents/withdraw`) are gasless — no NEAR balance required.

Ask the user to send at least 0.1 NEAR.

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
| **Transaction types** | Restrict to `transfer`, `call`, `intents_withdraw`, `intents_swap`, `intents_deposit`, `delete` or any combination |
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

## Key Recovery

If you lost your wallet API key (e.g. after a session reset), the user may be able to provide it — **but only if they previously set a policy**, which auto-saves the key to their browser.

**Message to user:**

> I lost access to your wallet API key. If you previously set a policy, the key is saved in your browser.
>
> Please go to: https://outlayer.fastnear.com/wallet/manage
>
> Find your wallet, click **show** next to the API Key, then copy and paste it here.
>
> The key looks like: `wk_15807d...e545` (starts with `wk_`)

After receiving the key, verify it works:

```bash
curl -s https://api.outlayer.fastnear.com/wallet/v1/balance?chain=near \
  -H "Authorization: Bearer <api_key>"
```

Then persist the key to storage so you don't lose it again.

**When recovery is NOT possible:**
- User never set a policy (key was never saved to browser)
- User cleared browser data or switched browsers/devices
- User manually removed the key from the manage page

In these cases, register a new wallet with `POST /register`. The funds in the old wallet are not lost — the user can still access it by adding a new API key via the on-chain policy's `authorized_key_hashes` field.

## Quick Reference

| Action | Method | Endpoint |
|--------|--------|----------|
| Register | POST | `https://api.outlayer.fastnear.com/register` |
| Get address | GET | `https://api.outlayer.fastnear.com/wallet/v1/address?chain={chain}` |
| Get balance | GET | `https://api.outlayer.fastnear.com/wallet/v1/balance?chain={chain}&token={token}` |
| Transfer NEAR | POST | `https://api.outlayer.fastnear.com/wallet/v1/transfer` |
| Call contract | POST | `https://api.outlayer.fastnear.com/wallet/v1/call` |
| Withdraw (cross-chain) | POST | `https://api.outlayer.fastnear.com/wallet/v1/intents/withdraw` |
| Dry-run (check first) | POST | `https://api.outlayer.fastnear.com/wallet/v1/intents/withdraw/dry-run` |
| Intents deposit | POST | `https://api.outlayer.fastnear.com/wallet/v1/intents/deposit` |
| Swap | POST | `https://api.outlayer.fastnear.com/wallet/v1/intents/swap` |
| Delete wallet | POST | `https://api.outlayer.fastnear.com/wallet/v1/delete` |
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
```bash
# Native NEAR balance
curl -s -H "Authorization: Bearer $API_KEY" \
  "https://api.outlayer.fastnear.com/wallet/v1/balance?chain=near"

# FT token balance (e.g. USDT)
curl -s -H "Authorization: Bearer $API_KEY" \
  "https://api.outlayer.fastnear.com/wallet/v1/balance?chain=near&token=usdt.tether-token.near" ```

The `chain` parameter defaults to `"near"` if omitted.

Response:
```json
{"balance": "1000000000000000000000000", "token": "near", "account_id": "36842e..."}
```

### Transfer NEAR

Send native NEAR to another account. Requires NEAR balance for both the amount and gas.

**Before calling:** check NEAR balance with `GET /wallet/v1/balance?chain=near` and verify it covers the transfer amount + gas (~0.001 NEAR).

```bash
curl -s -X POST -H "Content-Type: application/json" \
  -H "Authorization: Bearer $API_KEY" \
  -d '{"chain":"near","receiver_id":"bob.near","amount":"1000000000000000000000000"}' \
  "https://api.outlayer.fastnear.com/wallet/v1/transfer" ```

Response:
```json
{"request_id": "uuid", "status": "success", "tx_hash": "..."}
```

### Transfer FT tokens (USDT, wNEAR, etc.)

Use the generic contract call endpoint with `ft_transfer`. The receiver must already have storage registered on the token contract. Requires 1 yoctoNEAR deposit.

**Before calling:** check the token balance with `GET /wallet/v1/balance?chain=near&token={token_contract}` and verify it covers the transfer amount. Also check NEAR balance for gas.

```bash
curl -s -X POST -H "Content-Type: application/json" \
  -H "Authorization: Bearer $API_KEY" \
  -d '{"receiver_id":"usdt.tether-token.near","method_name":"ft_transfer","args":{"receiver_id":"bob.near","amount":"1000000"},"gas":"30000000000000","deposit":"1"}' \
  "https://api.outlayer.fastnear.com/wallet/v1/call" ```

Response:
```json
{"request_id": "uuid", "status": "success", "tx_hash": "..."}
```

**Note:** If the receiver doesn't have storage on the token contract, `ft_transfer` will fail. In that case, first call `storage_deposit` on the token contract for the receiver, or use `ft_transfer_call` instead.

### Call a contract

**Before calling:** check NEAR balance with `GET /wallet/v1/balance?chain=near` and verify it covers the `deposit` amount + gas (~0.005 NEAR). If the call involves FT tokens, also check the token balance.

```bash
curl -s -X POST -H "Content-Type: application/json" \
  -H "Authorization: Bearer $API_KEY" \
  -d '{"receiver_id":"wrap.near","method_name":"near_deposit","args":{},"deposit":"10000000000000000000000"}' \
  "https://api.outlayer.fastnear.com/wallet/v1/call" ```

Response:
```json
{"request_id": "uuid", "status": "success", "tx_hash": "...", "result": ...}
```

The `result` field contains the decoded return value of the contract call (if any). The `status` field is `"success"` or `"failed"`. If `"failed"`, the `result` field contains the error details.

### Withdraw (cross-chain via Intents)

Withdraw tokens from your Intents balance to any supported chain. Uses NEAR Intents `ft_withdraw` — gasless on the destination chain.

**Before calling:** check NEAR balance with `GET /wallet/v1/balance?chain=near` (needed for gas, ~0.005 NEAR). Tokens must already be in your Intents balance — if not, deposit them first with `/wallet/v1/intents/deposit`.

```bash
curl -s -X POST -H "Content-Type: application/json" \
  -H "Authorization: Bearer $API_KEY" \
  -d '{"to":"receiver.near","amount":"1000000000000000000000000","token":"wrap.near","chain":"near"}' \
  "https://api.outlayer.fastnear.com/wallet/v1/intents/withdraw" ```

**Important:** Tokens must be in your Intents balance first. If tokens are in your NEAR account, deposit them first with `/wallet/v1/intents/deposit`, then withdraw.

### Dry-run (check without executing)
```bash
curl -s -X POST -H "Content-Type: application/json" \
  -H "Authorization: Bearer $API_KEY" \
  -d '{"to":"receiver.near","amount":"1000000000000000000000000","token":"wrap.near","chain":"near"}' \
  "https://api.outlayer.fastnear.com/wallet/v1/intents/withdraw/dry-run" ```

### Deposit tokens into Intents balance

Deposits a FT from your wallet's NEAR account into `intents.near` via `ft_transfer_call`. Storage deposit on intents.near is handled automatically if needed.

This is used for manual intents operations and as a prerequisite for `/wallet/v1/intents/withdraw`. The `/wallet/v1/intents/swap` endpoint handles deposits internally — you do NOT need to call this before swapping.

**Before calling:** check the token balance with `GET /wallet/v1/balance?chain=near&token={token_contract}` and verify it covers the deposit amount. Also check NEAR balance for gas (~0.005 NEAR).

```bash
curl -s -X POST -H "Content-Type: application/json" \
  -H "Authorization: Bearer $API_KEY" \
  -d '{"token":"wrap.near","amount":"1000000000000000000000000"}' \
  "https://api.outlayer.fastnear.com/wallet/v1/intents/deposit" ```

Response:
```json
{"request_id": "uuid", "status": "success", "tx_hash": "..."}
```

### Swap tokens

Swap one token for another via 1Click API. Token identifiers use `nep141:` prefix (defuse asset format).

**Before calling:** check the input token balance with `GET /wallet/v1/balance?chain=near&token={token_contract}` (e.g. `&token=wrap.near` for wNEAR) and verify it covers `amount_in`. Also check NEAR balance for gas (~0.01 NEAR for storage deposits + transaction fees).

The swap handles everything internally:

1. Gets a quote from 1Click API
2. Auto-registers storage for the output token on your wallet (if needed)
3. Deposits input tokens into `intents.near`
4. Transfers tokens to the 1Click deposit address via `mt_transfer`
5. Polls until the swap settles
6. Output tokens are delivered directly to your wallet's NEAR account

```bash
curl -s -X POST -H "Content-Type: application/json" \
  -H "Authorization: Bearer $API_KEY" \
  -d '{"token_in":"nep141:wrap.near","token_out":"nep141:usdt.tether-token.near","amount_in":"1000000000000000000000000","min_amount_out":"3000000"}' \
  "https://api.outlayer.fastnear.com/wallet/v1/intents/swap" ```

Response:
```json
{"request_id": "uuid", "status": "success", "amount_out": "3150000", "intent_hash": "..."}
```

`min_amount_out` is optional — omit for a market order. If the quote is below `min_amount_out`, the swap is rejected before execution.

**Requires NEAR balance** for gas (~0.01 NEAR for storage deposits + transaction fees).

**No prerequisites** — swap handles intents deposit, storage registration, and token transfer internally.

### Delete wallet

Permanently delete the wallet's on-chain NEAR account using the native `DeleteAccount` action. All remaining native NEAR balance is automatically sent to the beneficiary. All API keys are revoked.

**WARNING:** Only native NEAR tokens are sent to the beneficiary (this is handled by NEAR's `DeleteAccount` action). FT tokens (USDT, wNEAR, etc.) and Intents balances are **lost permanently** because the account is deleted from the network. Withdraw or transfer those assets before deleting.

**Before calling:** transfer all FT tokens (via `/wallet/v1/call` with `ft_transfer`), withdraw Intents balances (via `/wallet/v1/intents/withdraw`), and move any other on-chain assets to another account.

```bash
curl -s -X POST -H "Content-Type: application/json" \
  -H "Authorization: Bearer $API_KEY" \
  -d '{"beneficiary":"receiver.near","chain":"near"}' \
  "https://api.outlayer.fastnear.com/wallet/v1/delete" ```

Response:
```json
{"request_id": "uuid", "status": "success", "tx_hash": "...", "beneficiary": "receiver.near"}
```

After deletion, the on-chain account no longer exists and all API keys are revoked. Subsequent requests will return `invalid_api_key`.

### Check request status
```bash
curl -s -H "Authorization: Bearer $API_KEY" \
  "https://api.outlayer.fastnear.com/wallet/v1/requests/{request_id}" ```

Response:
```json
{
  "request_id": "uuid",
  "wallet_id": "uuid",
  "request_type": "swap",
  "chain": "near",
  "request_data": {"token_in": "nep141:wrap.near", "token_out": "nep141:usdt.tether-token.near", ...},
  "status": "success",
  "result_data": {"amount_out": "3150000", "deposit_tx": "...", "mt_transfer_tx": "...", ...},
  "created_at": "2026-02-22T12:00:00Z",
  "updated_at": "2026-02-22T12:00:05Z"
}
```

### List available tokens
```bash
curl -s -H "Authorization: Bearer $API_KEY" \
  "https://api.outlayer.fastnear.com/wallet/v1/tokens" ```

### View audit log
```bash
curl -s -H "Authorization: Bearer $API_KEY" \
  "https://api.outlayer.fastnear.com/wallet/v1/audit?limit=50" ```

### Request funds from user

When the agent needs funding (native NEAR or FT tokens), generate a fund link and share it with the user. The user opens the link, connects their NEAR wallet, and signs the transfer.

**Fund link format:**
```
https://outlayer.fastnear.com/wallet/fund?to={near_account_id}&amount={amount}&token={token}&msg={message}
```

| Param | Required | Description |
|-------|----------|-------------|
| `to` | yes | Agent's NEAR account (the `near_account_id` from registration) |
| `amount` | yes | Human-readable amount (e.g. `1` for 1 NEAR, `10` for 10 USDT) |
| `token` | no | `near` (default) or FT contract ID (e.g. `usdt.tether-token.near`) |
| `msg` | no | Message to display to the user (URL-encoded) |

**Example — request 1 NEAR:**
```
https://outlayer.fastnear.com/wallet/fund?to={near_account_id}&amount=1&token=near&msg=Fund%20agent%20wallet
```

**Example — request 10 USDT:**
```
https://outlayer.fastnear.com/wallet/fund?to={near_account_id}&amount=10&token=usdt.tether-token.near&msg=Payment%20for%20task
```

The page automatically handles FT storage deposits — if the agent's account is not registered on the token contract, the storage deposit is included in the same transaction.

**Alternative:** You can also share your NEAR address directly and ask the user to transfer from any NEAR wallet. The fund link is a convenience feature for a better UX.

## Reading Transaction Statuses

Every mutating endpoint returns a `status` field. Here's how to interpret them:

| Status | Meaning | Action |
|--------|---------|--------|
| `success` | Operation completed successfully | Done — read result fields |
| `failed` | Operation failed | Check `result` field for error details |
| `processing` | Still in progress (async) | Poll `GET /wallet/v1/requests/{request_id}` |
| `pending_approval` | Requires multisig approval | Inform user, provide dashboard link |

### Response fields by endpoint

**`/wallet/v1/call`** — returns `{ request_id, status, tx_hash, result }`:
- `tx_hash`: NEAR transaction hash (viewable at `https://nearblocks.io/txns/{tx_hash}`)
- `result`: decoded return value from the contract, or error details if `status: "failed"`. The `result` field for failed transactions contains NEAR protocol error structure: `{"ActionError": {"kind": {"FunctionCallError": {"ExecutionError": "reason"}}}}`

**`/wallet/v1/transfer`** — returns `{ request_id, status, tx_hash }`:
- `tx_hash`: NEAR transaction hash

**`/wallet/v1/intents/swap`** — returns `{ request_id, status, amount_out, intent_hash }`:
- `amount_out`: actual amount of output tokens received (in smallest unit — e.g. 6 decimals for USDT)
- `intent_hash`: 1Click internal swap reference

**`/wallet/v1/intents/withdraw`** — returns `{ request_id, status }`:
- If `status: "pending_approval"`, also returns `approval_id`, `required`, `approved`
- Poll `GET /wallet/v1/requests/{request_id}` for final result

**`/wallet/v1/intents/deposit`** — returns `{ request_id, status, tx_hash }`:
- `tx_hash`: NEAR transaction hash of the `ft_transfer_call` to `intents.near`

### Common on-chain errors

| Error | Meaning | Fix |
|-------|---------|-----|
| `Smart contract panicked: The account ... is not registered` | Receiver doesn't have storage on the token contract | Call `storage_deposit` on the token contract for the receiver first |
| `Smart contract panicked: The amount should be a positive number` | Tried to transfer 0 tokens | Check the amount is > 0 |
| `NotEnoughBalance` | Insufficient NEAR for gas + deposit | Fund the wallet with more NEAR |
| `InvalidNonce` | Nonce conflict (concurrent transactions) | Retry — the wallet uses nonce locking, but back-to-back calls can still race |

## Automatic Storage Registration

Several endpoints handle NEP-141 storage registration automatically:

| Endpoint | What it auto-registers |
|----------|----------------------|
| `/wallet/v1/intents/swap` | Output token storage on your wallet (e.g. registers USDT before swap delivers it) |
| `/wallet/v1/intents/deposit` | Your wallet's storage on `intents.near` (needed for intents balance) |
| Fund link (dashboard) | Your wallet's storage on the token contract (included in the funding transaction) |

**NOT auto-registered:**
- `/wallet/v1/call` — generic contract call, no automatic storage handling. If you call `ft_transfer` and the receiver isn't registered, it will fail. Register storage manually first:

```bash
curl -s -X POST -H "Content-Type: application/json" \
  -H "Authorization: Bearer $API_KEY" \
  -d '{"receiver_id":"usdt.tether-token.near","method_name":"storage_deposit","args":{"account_id":"receiver.near","registration_only":true},"gas":"30000000000000","deposit":"1250000000000000000000"}' \
  "https://api.outlayer.fastnear.com/wallet/v1/call" ```

Storage deposit costs 0.00125 NEAR per token contract per account.

## Token Amounts Reference

| Token | Decimals | 1 unit in smallest | Example |
|-------|----------|-------------------|---------|
| NEAR | 24 | `1000000000000000000000000` | 0.1 NEAR = `100000000000000000000000` |
| wNEAR | 24 | `1000000000000000000000000` | same as NEAR |
| USDT | 6 | `1000000` | 1 USDT = `1000000`, 0.01 USDT = `10000` |
| USDC | 6 | `1000000` | same as USDT |

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

- **Always check balance before any operation.** Before calling `/swap`, `/transfer`, `/call`, or `/withdraw`, query `/wallet/v1/balance` to verify the wallet has sufficient funds. For swaps and calls, also check NEAR balance for gas. Failing to check balance first wastes gas on transactions that will fail.
- Always use `withdraw/dry-run` before real withdrawals to check policy and balance
- Store the API key as a secret — never log or expose it
- If a request returns `pending_approval`, inform the user and provide the dashboard link
- Poll `/requests/{id}` for async operation status (withdraw, swap)
- NEAR amounts are in yoctoNEAR (1 NEAR = 10^24 yoctoNEAR)
- Swap requires NEAR balance for gas (~0.01 NEAR). No need to pre-deposit to intents — swap handles it
- For FT transfers, check that the receiver has storage on the token contract before calling `ft_transfer`
