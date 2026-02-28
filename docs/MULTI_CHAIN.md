# Multi-Chain Support for Agent Custody Wallets

Agent Custody allows AI agents to hold and manage funds via TEE-secured wallets with configurable spending policies. Currently only NEAR is supported. The keystore already generates keys for EVM chains (secp256k1) and Solana (ed25519). This document describes how to enable a new chain.

## Current Status

| Component | NEAR | EVM (ETH/Base/Arb) | Solana |
|-----------|------|---------------------|--------|
| Key generation (keystore) | ed25519 | secp256k1 | ed25519 |
| Transaction signing (keystore) | ed25519 | ECDSA secp256k1 | ed25519 |
| Derivation seed | `wallet:{id}:near` | `wallet:{id}:ethereum` | `wallet:{id}:solana` |
| Coordinator handlers | withdraw, call, transfer, swap, deposit | not implemented | not implemented |
| Dashboard UI | full support | not implemented | not implemented |
| Policy evaluation (keystore) | all rules | all rules (shared policy) | all rules (shared policy) |

## Architecture: Shared Policy

Policy is **one per `wallet_id`**, shared across all chains. Spending limits, rate limits, time restrictions, and approval thresholds apply to all operations regardless of chain. Address restrictions (`addresses.list`) can contain addresses of any format.

Policy is stored on-chain keyed by `wallet_pubkey` (the NEAR ed25519 key). This key serves as the wallet's "anchor". Other chain keys are linked through the same `wallet_id` in the coordinator database.

## Checklist: Adding an EVM Chain (Ethereum/Base/Arbitrum)

### 1. Database Migration (coordinator)

```sql
-- migrations/YYYYMMDD_wallet_chain_addresses.sql
CREATE TABLE wallet_chain_addresses (
    wallet_id TEXT NOT NULL REFERENCES wallet_accounts(wallet_id),
    chain TEXT NOT NULL,         -- "ethereum", "base", "solana"
    pubkey TEXT NOT NULL UNIQUE, -- "secp256k1:<hex33>" or "ed25519:<hex32>"
    address TEXT NOT NULL,       -- 0x1234... or base58...
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (wallet_id, chain)
);
```

### 2. Coordinator: validate_chain()

File: `coordinator/src/wallet/handlers.rs`, function `validate_chain()`

```rust
// Add chain to the match:
"ethereum" | "base" | "arbitrum" => Ok(()),
```

### 3. Coordinator: get_address handler

Persist the derived address in `wallet_chain_addresses` on first request:

```rust
"ethereum" | "base" | "arbitrum" => {
    // Keystore already returns a secp256k1 address
    sqlx::query(
        "INSERT INTO wallet_chain_addresses (wallet_id, chain, pubkey, address)
         VALUES ($1, $2, $3, $4) ON CONFLICT DO NOTHING"
    )
    .bind(&wallet_id).bind(&chain)
    .bind(&derive_resp.public_key).bind(&derive_resp.address)
    .execute(&db).await?;
}
```

### 4. Coordinator: withdraw/transfer for EVM

New handlers (or extend existing ones) for EVM operations:

1. Build the EVM transaction (RLP-encode: nonce, gasPrice, gasLimit, to, value, data)
2. Sign via keystore: `POST /internal/wallet-sign-transaction` with `chain: "ethereum"`
3. Broadcast via RPC node (Infura/Alchemy/self-hosted)
4. Record the result in `wallet_requests`

### 5. Dashboard: display multi-chain addresses

In `manage/page.tsx` — show addresses for all chains per wallet:

```typescript
// Chain badge already supports:
// ed25519: → NEAR
// secp256k1: → EVM (need to add chain name from wallet_chain_addresses)
```

### 6. Dashboard: chain selector

Add chain selection for withdraw/transfer operations. The policy form remains shared across all chains.

## Checklist: Adding Solana

1. Add `"solana" => Ok(())` to `validate_chain()`
2. Keystore already generates ed25519 keys for Solana (base58 format)
3. Coordinator: build Solana Transaction, sign via keystore, broadcast via Solana RPC
4. **Important**: Solana ed25519 and NEAR ed25519 are different keys (different seeds: `wallet:{id}:solana` vs `wallet:{id}:near`)

## Key Files

| File | What to change |
|------|----------------|
| `coordinator/src/wallet/handlers.rs` | `validate_chain()`, withdraw/transfer handlers |
| `coordinator/migrations/` | New migration for `wallet_chain_addresses` |
| `keystore-worker/src/crypto.rs` | Ready: `derive_secp256k1_keypair()`, `derive_eth_address()`, `sign_secp256k1()` |
| `keystore-worker/src/api.rs` | Ready: derive and sign handlers for EVM/Solana |
| `dashboard/app/wallet/manage/page.tsx` | Multi-chain address display |
| `contract/src/wallet.rs` | No changes needed — policy is keyed by `wallet_pubkey` (NEAR key) |
