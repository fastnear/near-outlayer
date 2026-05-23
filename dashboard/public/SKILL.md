---
name: Agent Custody Wallet Setup
description: Register a gasless multi-chain custody wallet for an AI agent. Supports NEAR, Ethereum, Bitcoin, Solana, and other chains via NEAR Intents protocol. Use when an agent needs to send/receive tokens, swap cross-chain, or call smart contracts.
metadata:
  api:
    base_url: https://api.outlayer.fastnear.com
    version: v1
    auth: Bearer token
---

# OutLayer Agent Custody Wallet

Full skill with swap workflows, token reference, and cross-chain patterns: [skills/outlayer-skills/agent-custody/SKILL.md](https://skills.outlayer.ai/agent-custody/SKILL.md)

> **⚠️ Only send whitelisted Intents assets — anything else is lost permanently.**
> Deposits and withdrawals only work for assets listed in `GET /wallet/v1/tokens`,
> on the exact chain a deposit address was issued for. Sending an unsupported
> token, the wrong token, a token on the wrong chain, an NFT, or an unlisted
> native gas coin to a deposit address is **unrecoverable**. The wallet is
> NEAR-native: cross-chain value moves via NEAR Intents + the 1Click solver, not
> via native per-chain addresses. `GET /wallet/v1/address` returns the NEAR
> address only (`chain=near`); native ETH/SOL/BTC addresses are not issued.

## Quick Start

```bash
# 1. Register (no auth)
curl -s -X POST https://api.outlayer.fastnear.com/register
# → {"api_key": "wk_...", "near_account_id": "...", "handoff_url": "..."}

# 2. Check balance
curl -s -H "Authorization: Bearer $API_KEY" \
  "https://api.outlayer.fastnear.com/wallet/v1/balance?chain=near"

# 3. Swap tokens (wNEAR → USDT)
curl -s -X POST -H "Content-Type: application/json" \
  -H "Authorization: Bearer $API_KEY" \
  -d '{"token_in":"nep141:wrap.near","token_out":"nep141:usdt.tether-token.near","amount_in":"1000000000000000000000000"}' \
  "https://api.outlayer.fastnear.com/wallet/v1/intents/swap"

# 4. Withdraw NATIVE NEAR (chain=near default): unwraps your wNEAR → native NEAR,
#    gasless, recipient needs NO wrap.near storage. amount in yoctoNEAR (24 decimals).
#    Use token:"nep141:wrap.near" instead to deliver wNEAR (recipient needs storage).
curl -s -X POST -H "Content-Type: application/json" \
  -H "Authorization: Bearer $API_KEY" \
  -d '{"to":"receiver.near","amount":"1000000000000000000000000","token":"near","chain":"near"}' \
  "https://api.outlayer.fastnear.com/wallet/v1/intents/withdraw"
```

## Quick Reference

| Action | Method | Endpoint |
|--------|--------|----------|
| Register | POST | `/register` |
| Execute WASI (trial) | POST | `/call/{owner}/{project}` |
| Trial status | GET | `/trial/status` |
| Create payment key | POST | `/wallet/v1/create-payment-key` |
| Get address | GET | `/wallet/v1/address?chain=near` (NEAR only) |
| Cross-chain deposit | POST | `/wallet/v1/deposit-intent` (1Click bridge address) |
| Get balance | GET | `/wallet/v1/balance?chain={chain}&token={token}` |
| Get intents balance | GET | `/wallet/v1/balance?token={token}&source=intents` |
| Transfer NEAR | POST | `/wallet/v1/transfer` |
| Call contract | POST | `/wallet/v1/call` |
| Swap tokens | POST | `/wallet/v1/intents/swap` |
| Swap quote | POST | `/wallet/v1/intents/swap/quote` |
| Intents deposit | POST | `/wallet/v1/intents/deposit` |
| Withdraw (native NEAR / wNEAR / cross-chain) | POST | `/wallet/v1/intents/withdraw` |
| Dry-run withdrawal | POST | `/wallet/v1/intents/withdraw/dry-run` |
| List tokens | GET | `/wallet/v1/tokens` |
| Request status | GET | `/wallet/v1/requests/{request_id}` |
| Audit log | GET | `/wallet/v1/audit?limit=50` |
| Create payment check | POST | `/wallet/v1/payment-check/create` |
| Batch create checks | POST | `/wallet/v1/payment-check/batch-create` |
| Claim payment check | POST | `/wallet/v1/payment-check/claim` |
| Check status | GET | `/wallet/v1/payment-check/status?check_id={id}` |
| List checks | GET | `/wallet/v1/payment-check/list` |
| Reclaim check | POST | `/wallet/v1/payment-check/reclaim` |
| Peek check balance | POST | `/wallet/v1/payment-check/peek` |
| Delete wallet | POST | `/wallet/v1/delete` |

All endpoints except `/register` require `Authorization: Bearer <api_key>` header.
Base URL: `https://api.outlayer.fastnear.com`

## Fund Link

Ask the user to fund your wallet via a link:
```
https://outlayer.fastnear.com/wallet/fund?to={near_account_id}&amount={amount}&token={token}&msg={message}&dest=intents
```

- `dest=intents` — deposit directly to Intents balance (for swaps, payment checks, cross-chain). FT tokens only.
- Without `dest` — direct transfer to wallet account.
- The page has a toggle so the user can switch between modes.
