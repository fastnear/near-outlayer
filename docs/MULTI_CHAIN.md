# Multi-Chain Support for Agent Custody Wallets

Agent Custody allows AI agents to hold and manage funds via TEE-secured wallets with configurable spending policies. **NEAR and all EVM chains are supported**; Solana is not yet implemented. The keystore generates keys for EVM chains (secp256k1) and Solana (ed25519). This document describes the shipped EVM signing model and how to enable a remaining chain.

## Current Status

| Component | NEAR | EVM (eth/polygon/base/arbitrum/optimism/bsc/avalanche) | Solana |
|-----------|------|---------------------|--------|
| Key generation (keystore) | ed25519 | secp256k1 (one shared address across all EVM chains) | ed25519 |
| Transaction signing (keystore) | ed25519 | ECDSA secp256k1 (keccak256 + sign; off-chain EIP-712 / EIP-191 / raw-tx hash) | ed25519 |
| Derivation seed | `wallet:{id}:near` | `wallet:{id}:ethereum` (shared by every EVM chain) | `wallet:{id}:solana` |
| Coordinator handlers | withdraw, call, transfer, swap, deposit | `evm/sign-typed-data`, `evm/sign-message`, `evm/sign-transaction` (signing only — no build/broadcast) | not implemented |
| Dashboard UI | full support | address display | not implemented |
| Policy evaluation (keystore) | all rules | `evm_sign` capability (default-DENY under a policy; set `allowed:true`) + `raw_tx` sub-flag (default-OFF); shared policy | all rules (shared policy) |

## Architecture: Shared Policy

Policy is **one per `wallet_id`**, shared across all chains. Spending limits, rate limits, time restrictions, and approval thresholds apply to all operations regardless of chain. Address restrictions (`addresses.list`) can contain addresses of any format.

Policy is stored on-chain keyed by `wallet_pubkey` (the NEAR ed25519 key). This key serves as the wallet's "anchor". Other chain keys are linked through the same `wallet_id` in the coordinator database.

## EVM Signing (shipped)

EVM signing is **live**. The model is deliberately narrow: **the client builds and broadcasts; the keystore only hashes (keccak256) and signs.** The keystore and coordinator never assemble an EVM transaction, never pick a nonce or gas, and never broadcast. Cross-chain value movement still rides `/wallet/v1/deposit-intent` + `/wallet/v1/intents/withdraw` (1Click + NEAR signatures, no native EVM tx).

### Supported chains

`ethereum`, `polygon`, `base`, `arbitrum`, `optimism`, `bsc`, `avalanche` — plus the 1Click-style aliases `eth`, `pol`, `matic`, `arb`, `op`, `avax`. **All EVM chains share ONE derived secp256k1 address** (a single EOA, seed `wallet:{id}:ethereum`). `GET /wallet/v1/address` serves any of these and returns that one `0x` address. Solana stays gated (ed25519 key derivable, no signing path yet); account delete stays NEAR-only.

### Endpoints

| Endpoint | Standard | What the keystore does |
|----------|----------|------------------------|
| `POST /wallet/v1/evm/sign-typed-data` | EIP-712 v4 | Computes the digest from the full `eth_signTypedData_v4` object server-side, then signs |
| `POST /wallet/v1/evm/sign-message` | EIP-191 `personal_sign` | Computes `keccak256("\x19Ethereum Signed Message:\n" + len + msg)`, then signs |
| `POST /wallet/v1/evm/sign-transaction` | raw tx | The **client** serializes the unsigned tx; the keystore keccak256-hashes and signs it. No assembly, nonce/gas selection, or broadcast. For an EIP-1559 (type-2) tx the `yParity` needed to assemble the final tx is `v - 27`. |

All three return a 65-byte `0x` signature `r‖s‖v`, with `v ∈ {27, 28}` and low-s (EIP-2) normalization. The EIP-712 encoder is **hand-rolled** (no `alloy`/`ethers`) so it adds no dependency to the enclave's attestation surface; it is pinned against viem-generated reference vectors (`keystore-worker/src/eip712_vectors.json`).

### Policy capability: `evm_sign`

`evm_sign` is **default-DENY under a policy** — like every other fund-moving capability, a policy must explicitly set `capabilities.evm_sign.allowed = true` to permit EIP-712 / EIP-191 signing (a wallet with **no policy** is unrestricted). `sign_message` is the only default-allow capability. A `raw_tx` sub-flag is **default-OFF** and separately gates the raw-transaction endpoint. `requires_approval` is **not supported** for `evm_sign` (a policy that sets it fails closed rather than silently ignoring the owner's intent).

**Caveat — `evm_sign` is fund-moving authority, not a read-only grant.** An EIP-712 signature can itself move funds: EIP-3009 `transferWithAuthorization` ≈ a transfer, EIP-2612 `Permit` ≈ an approve. So `evm_sign` grants full authority over whatever float sits on the wallet's EVM address — bounded to what has been bridged there. The `raw_tx` flag is a kill-switch for arbitrary raw transactions, **not** a containment boundary for typed-data drains. The NEAR-intents balance is never exposed by any EVM signing path.

### Remaining EVM work

- **Dashboard chain selector for signing flows** — address display exists; per-chain signing UX is not built.
- **Persisting derived EVM addresses** (`wallet_chain_addresses`) is optional — the address is deterministic from the seed and the shared EOA is identical across chains, so the table is only a convenience cache.

## Checklist: Adding Solana (not yet implemented)

Solana is the one remaining chain. The keystore derives the ed25519 key but there is no signing path, so `validate_chain()` and `is_evm_chain()` both reject it today.

1. Add a Solana signing handler in the keystore (ed25519 over the message/tx digest) and stop rejecting `"solana"` in `validate_chain()`
2. Keystore already generates ed25519 keys for Solana (base58 format)
3. Follow the EVM model: the **client** builds and broadcasts the Solana transaction; the keystore only signs the supplied digest. Do not build/broadcast in the coordinator.
4. **Important**: Solana ed25519 and NEAR ed25519 are different keys (different seeds: `wallet:{id}:solana` vs `wallet:{id}:near`)

## Key Files

| File | What it does / what to change |
|------|----------------|
| `coordinator/src/wallet/handlers.rs` | Shipped: `validate_chain()` + `is_evm_chain()` admit all EVM chains; `evm_sign_typed_data` / `evm_sign_message` / `evm_sign_transaction` handlers forward to the keystore (no build/broadcast). Add Solana here. |
| `keystore-worker/src/crypto.rs` | Shipped: `derive_secp256k1_keypair()`, `derive_eth_address()`, `sign_secp256k1_prehash()` (returns 65-byte `r‖s‖v`, low-s, `v ∈ {27,28}`) |
| `keystore-worker/src/eip712.rs` | Shipped: hand-rolled EIP-712 v4 + EIP-191 digest computation (no `alloy`/`ethers`); pinned to `eip712_vectors.json` |
| `keystore-worker/src/api.rs` | Shipped: `/wallet/evm/{sign-typed-data,sign-message,sign-transaction}` handlers; `evm_sign` capability + `raw_tx` sub-flag gating via `shared_tee_helpers::wallet_policy::evm_sign_decision` |
| `dashboard/app/wallet/manage/page.tsx` | Multi-chain address display (per-chain signing UX still TODO) |
| `contract/src/wallet.rs` | No changes needed — policy is keyed by `wallet_pubkey` (NEAR key) |
