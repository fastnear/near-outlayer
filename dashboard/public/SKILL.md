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

Full skill with swap workflows, token reference, and cross-chain patterns: [skills/outlayer-skills/agent-custody/SKILL.md](https://github.com/nickolay-near/near-offshore/tree/main/skills/outlayer-skills/agent-custody/SKILL.md)

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
```

## Quick Reference

| Action | Method | Endpoint |
|--------|--------|----------|
| Register | POST | `/register` |
| Execute WASI (trial) | POST | `/call/{owner}/{project}` |
| Trial status | GET | `/trial/status` |
| Create payment key | POST | `/wallet/v1/create-payment-key` |
| Get address | GET | `/wallet/v1/address?chain={chain}` |
| Get balance | GET | `/wallet/v1/balance?chain={chain}&token={token}` |
| Get intents balance | GET | `/wallet/v1/balance?token={token}&source=intents` |
| Transfer NEAR | POST | `/wallet/v1/transfer` |
| Call contract | POST | `/wallet/v1/call` |
| Swap tokens | POST | `/wallet/v1/intents/swap` |
| Swap quote | POST | `/wallet/v1/intents/swap/quote` |
| Intents deposit | POST | `/wallet/v1/intents/deposit` |
| Withdraw (cross-chain) | POST | `/wallet/v1/intents/withdraw` |
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
