# Payment Checks

Gasless agent-to-agent payments via ephemeral intents accounts. Agent A locks tokens into a check and sends a single key to Agent B, who claims the funds — no gas, no on-chain account, no private key exchange. Supports partial claims, expiry, and reclaim.

## Why Payment Checks

| Problem | Payment Checks Solution |
|---------|------------------------|
| Direct transfers require gas (NEAR) from both sides | Fully gasless — uses solver relay with off-chain NEP-413 signatures |
| Receiver needs an on-chain account with gas | Receiver only needs a wallet API key |
| Direct transfers are irreversible | Sender can reclaim unclaimed funds at any time |
| No native partial payment support | Built-in partial claim and partial reclaim |
| Escrow requires smart contract development | Check acts as lightweight escrow — one API call |

## How It Works

```
Agent A (Sender)              Agent B (Receiver)
     |                              |
     |  1. POST /create             |
     |  ────────────────►           |
     |  ← check_key                 |
     |                              |
     |  2. Send check_key           |
     |  ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─►|
     |     (any channel)            |
     |                              |
     |                 3. POST /peek |
     |                 (verify)      |
     |                              |
     |                4. POST /claim |
     |                ◄─────────────|
     |                 funds move    |
     |                              |
     |  5. POST /reclaim (optional) |
     |  (get unclaimed funds back)  |
```

### Step-by-step

1. **Create** — Agent A calls `POST /wallet/v1/payment-check/create`. The TEE derives a unique ephemeral key, transfers tokens from the wallet to the ephemeral account via solver relay (gasless), and returns a `check_key`.

2. **Share** — Agent A sends the `check_key` to Agent B over any channel (HTTP, message, QR code). The key is a 64-char hex string.

3. **Peek** (optional) — Agent B calls `POST /wallet/v1/payment-check/peek` with the key to verify the check's balance, memo, and expiry before claiming.

4. **Claim** — Agent B calls `POST /wallet/v1/payment-check/claim` with the key. The coordinator signs a transfer intent from the ephemeral account to Agent B's wallet via solver relay. Supports partial claims.

5. **Reclaim** (optional) — Agent A can reclaim any unclaimed funds at any time via `POST /wallet/v1/payment-check/reclaim`. The TEE re-derives the ephemeral key (no need to store it).

## Transfer Mechanism

All operations use the **NEAR Intents solver relay** — a gasless off-chain transfer protocol. Instead of on-chain transactions (which require NEAR for gas), the coordinator signs NEP-413 messages and submits them to the solver relay.

| Operation | From → To | Who Signs | Gas |
|-----------|-----------|-----------|-----|
| Create | Wallet → Ephemeral | Wallet key (TEE keystore) | None |
| Claim | Ephemeral → Claimer | Ephemeral key (from check_key) | None |
| Reclaim | Ephemeral → Creator | Ephemeral key (TEE re-derivation) | None |

### Ephemeral Accounts

Each check gets its own ephemeral account on `intents.near`, derived from the wallet's master secret + a monotonic counter. This gives each check an isolated balance that can only be moved by whoever holds the `check_key` (claim) or by the TEE re-deriving the key (reclaim).

```
Key derivation hierarchy:
  wallet:{id}:near                        ← main wallet key
  wallet:{id}:near:check:{counter}        ← ephemeral key per check

The check_key IS the raw ed25519 private key of the ephemeral account.
The ephemeral account ID = hex(public_key) on intents.near.
```

## API Reference

Base URL: `https://api.outlayer.fastnear.com/wallet/v1/payment-check`

All endpoints require `Authorization: Bearer wk_...` header.

### POST /create

Create a new payment check.

**Request:**
```json
{
  "token": "17208628f84f5d6ad33f0da3bbbeb27ffcb398eac501a31bd6ad2011e36133a1",
  "amount": "1000000",
  "memo": "Payment for data analysis",
  "expires_in": 3600
}
```

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `token` | string | yes | Token contract ID (e.g., USDC contract) |
| `amount` | string | yes | Amount in smallest units (e.g., "1000000" = 1 USDC) |
| `memo` | string | no | Optional memo (max 256 chars), visible to receiver |
| `expires_in` | number | no | Expiry in seconds from now |

**Response:**
```json
{
  "check_id": "a1b2c3d4-...",
  "check_key": "7f3a9b2c...64 hex chars...",
  "token": "17208628f84f5d6ad33f0da3bbbeb27ffcb398eac501a31bd6ad2011e36133a1",
  "amount": "1000000",
  "memo": "Payment for data analysis",
  "created_at": "2026-03-13T10:00:00Z",
  "expires_at": "2026-03-13T11:00:00Z"
}
```

### POST /batch-create

Create multiple checks in one call (max 10).

**Request:**
```json
{
  "checks": [
    {"token": "170...a1", "amount": "1000000", "memo": "Task 1"},
    {"token": "170...a1", "amount": "2000000", "memo": "Task 2"}
  ]
}
```

**Response:**
```json
{
  "checks": [
    {"check_id": "...", "check_key": "...", "token": "...", "amount": "1000000", ...},
    {"check_id": "...", "check_key": "...", "token": "...", "amount": "2000000", ...}
  ]
}
```

### POST /claim

Claim funds from a check. Supports partial claims.

**Request:**
```json
{
  "check_key": "7f3a9b2c...64 hex chars...",
  "amount": "500000"
}
```

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `check_key` | string | yes | The 64-char hex key from the sender |
| `amount` | string | no | Partial claim amount (omit for full claim) |

**Response:**
```json
{
  "token": "17208628f84f5d6ad33f0da3bbbeb27ffcb398eac501a31bd6ad2011e36133a1",
  "amount_claimed": "500000",
  "remaining": "500000",
  "memo": "Payment for data analysis",
  "claimed_at": "2026-03-13T10:05:00Z",
  "intent_hash": "Bx7k..."
}
```

### POST /reclaim

Reclaim unclaimed funds. Only the original creator can reclaim.

**Request:**
```json
{
  "check_id": "a1b2c3d4-...",
  "amount": "500000"
}
```

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `check_id` | string | yes | The check ID from create response |
| `amount` | string | no | Partial reclaim amount (omit for full reclaim) |

**Response:**
```json
{
  "token": "17208628f84f5d6ad33f0da3bbbeb27ffcb398eac501a31bd6ad2011e36133a1",
  "amount_reclaimed": "500000",
  "remaining": "0",
  "reclaimed_at": "2026-03-13T10:10:00Z",
  "intent_hash": "Cx9m..."
}
```

### GET /status?check_id=...

Get current status of a check. Only the creator can query.

**Response:**
```json
{
  "check_id": "a1b2c3d4-...",
  "token": "17208628f84f5d6ad33f0da3bbbeb27ffcb398eac501a31bd6ad2011e36133a1",
  "amount": "1000000",
  "claimed_amount": "500000",
  "reclaimed_amount": "500000",
  "status": "reclaimed",
  "memo": "Payment for data analysis",
  "created_at": "2026-03-13T10:00:00Z",
  "expires_at": "2026-03-13T11:00:00Z",
  "claimed_at": "2026-03-13T10:05:00Z",
  "claimed_by": "a2b3b5b5c72c..."
}
```

### GET /list?status=...&limit=50&offset=0

List all checks created by the authenticated wallet.

| Param | Required | Description |
|-------|----------|-------------|
| `status` | no | Filter: unclaimed, claimed, reclaimed, partially_claimed |
| `limit` | no | Max results (default 50, max 100) |
| `offset` | no | Pagination offset |

### POST /peek

Check a payment check's balance using the `check_key`. Use to verify before claiming.

**Request:**
```json
{
  "check_key": "7f3a9b2c...64 hex chars..."
}
```

**Response:**
```json
{
  "token": "17208628f84f5d6ad33f0da3bbbeb27ffcb398eac501a31bd6ad2011e36133a1",
  "balance": "1000000",
  "memo": "Payment for data analysis",
  "status": "unclaimed",
  "expires_at": "2026-03-13T11:00:00Z"
}
```

## Check Lifecycle

```
                create
                  |
                  v
              [unclaimed]
               /      \
      claim   /        \  reclaim
             v          v
   [partially_claimed] [partially_reclaimed]
          |       \         /       |
   claim  |        \       /        | reclaim
          v         v     v         v
      [claimed]   (mixed)    [reclaimed]
```

**Statuses:**
- `unclaimed` — funds locked, waiting to be claimed
- `partially_claimed` — some funds claimed, rest available
- `partially_reclaimed` — some funds reclaimed by sender
- `claimed` — all funds claimed by receiver
- `reclaimed` — all funds reclaimed by sender

**Expiry:** If `expires_in` is set, the check cannot be claimed after expiry. Funds remain in the ephemeral account — the sender must explicitly reclaim them. Expiry prevents new claims but does not auto-return funds.

## Security

| Threat | Mitigation |
|--------|------------|
| check_key intercepted in transit | Use encrypted channels (HTTPS, E2E messaging). Leaked key + any wallet API key = funds claimed. |
| Sender's wallet compromised | Wallet private key never leaves TEE. API key can be revoked. Policy engine limits exposure. |
| Replay of claim/reclaim | Each intent has unique nonce + 5-minute deadline. Solver relay rejects duplicates. DB tracks amounts atomically. |
| Ephemeral key collision | Monotonic counter per wallet (DB enforced). Same wallet + counter = same key, counter never reuses. |
| Funds stuck in ephemeral account | Sender can always reclaim — TEE re-derives ephemeral key from same deterministic path. |

**Key insight:** The `check_key` is the only secret. It never touches the blockchain, never enters the TEE for claim (coordinator signs locally with it). The sender doesn't need to store it — the TEE re-derives it for reclaim. Even if the sender loses all local state, funds are recoverable through the TEE.

## Comparison

|  | Payment Checks | Direct Transfer | Smart Contract Escrow |
|--|----------------|-----------------|----------------------|
| Gas required | None | Sender pays | Both parties pay |
| Receiver needs account | Only wallet API key | On-chain account + gas | On-chain account + gas |
| Partial payment | Built-in | N/A | Custom logic |
| Reclaim | Built-in, gasless | N/A (irreversible) | Custom logic + gas |
| On-chain footprint | Solver relay only | 1 transaction | Contract deployment + calls |
| Setup complexity | One API call | One API call | Contract dev + deployment |

## Use Cases

- **Agent-to-agent payments** — Agent A creates a check, sends key as part of API request. Agent B verifies (peek), performs work, claims payment. If B doesn't deliver, A reclaims.
- **Bounties & task rewards** — Create check with expiry. Share key with task completer. Partial claims allow splitting among contributors.
- **Escrow-style payments** — Lock funds in a check. Share key only when conditions are met. Lightweight escrow without a smart contract.
- **Batch payouts** — Use `batch-create` to generate up to 10 checks in a single call, each with independent key.
