# Vault contract — global-contract deploy

The vault WASM is deployed once on-chain as a [Global Contract by hash](https://docs.near.org/protocol/global-contracts) and every customer's `vault.X` account references it via `Action::UseGlobalContract`. This drops the per-vault deploy tx from ~200 KB (DeployContract carrying the inline WASM) to under 1 KB (UseGlobalContract carrying just the 32-byte SHA256), which is what makes browser-wallet flows like MyNearWallet's URL-redirect signing tractable for vault init.

## One-time deploy (per network)

Run **once** per network. Anyone with enough NEAR (~1.5 NEAR for storage) can deploy; the resulting global identifier is the WASM's SHA-256 — same as the hash already approved on the keystore-DAO, so no extra DAO vote is required.

```bash
near contract deploy-as-global use-file \
  res/vault_contract.wasm \
  as-global-hash \
  outlayer.testnet \
  network-config testnet sign-with-keychain send
```

Replace `outlayer.testnet` with whichever account is paying storage. On mainnet:

```bash
near contract deploy-as-global use-file \
  res/vault_contract.wasm \
  as-global-hash \
  outlayer.near \
  network-config mainnet sign-with-keychain send
```

The deploy tx is small (just the WASM bytes + storage staking) — it does not hit any wallet URL limit. Use a CLI signer for it.

After landing, the global identifier is the same `Base58CryptoHash` value that's already approved via `keystore-dao.approve_vault_version`. Confirm on-chain availability:

```bash
HASH=GiQkqctRW3oGWbSDk5V4MPqZrWw3zf8GDaU75a78pgbD   # whatever shasum -a 256 res/vault_contract.wasm | base58 says
near contract call-function as-read-only ... # not applicable

# Easier: raw RPC
curl -sS -X POST https://rpc.testnet.fastnear.com -H 'Content-Type: application/json' -d "{
  \"jsonrpc\":\"2.0\",\"id\":\"q\",\"method\":\"query\",
  \"params\":{\"request_type\":\"view_global_contract_code\",
              \"finality\":\"final\",
              \"code_hash\":\"$HASH\"}}"
```

Should return `{ "result": { "code_base64": "...", "block_height": ... }, ... }`.

## What the vault deploy tx looks like now

`buildVaultDeployActions` in `dashboard/lib/vault.ts` and the equivalent in `outlayer-cli/src/near.rs` build the same five-action atomic deploy:

1. `CreateAccount` — the new sub-account for the vault.
2. `Transfer` — initial NEAR balance (storage + small reserve for the vault's outbound MPC `request_master` calls).
3. **`UseGlobalContract { CodeHash: <32 bytes> }`** — references the WASM already on chain. **Nothing about the WASM bytes ships in this tx.**
4. `FunctionCall { method: "new", args: { parent, keystore_dao, mpc_contract, initial_exit_window } }` — runs the contract's constructor.
5. `AddKey` — the TEE-derived function-call key, scoped to `(receiver=<vault>, methods=["request_master"])`.

Both contract bytecode and wallet UX benefit:
- Tx body is bounded by step 4's `args` JSON (~200 bytes) and step 5's pubkey + permission, so the whole atomic deploy fits comfortably inside MyNearWallet's URL.
- All vaults share one canonical WASM in global storage instead of duplicating ~150 KB per customer.

## Updating the WASM

Bumping the vault contract is the same flow as before, with one extra step:

1. Edit `vault-contract/src/lib.rs`. Rebuild via `bash build.sh`.
2. Sync the three bundled copies (`scripts/verify_vault_wasm_sync.sh` will tell you which paths drifted) and rebuild `outlayer-cli` so the bundled hash refreshes.
3. **Approve the new hash via DAO multisig** — `keystore-dao.approve_vault_version`.
4. **Deploy the new WASM as a global contract** with the same `near contract deploy-as-global use-file ... as-global-hash` command. Each global hash costs storage independently; the previous version stays referenceable for vaults that were already deployed against it.
5. Customers calling `outlayer vault init` (or the dashboard's Create Vault) from then on will use the new hash automatically — the bundled WASM in dashboard/CLI is what determines which `code_hash` the `UseGlobalContract` action references.

Old vaults keep working — they were deployed via `UseGlobalContract { CodeHash: <old_hash> }` and that reference is permanent. They use the old contract's behaviour until the parent does an unlock + redeploy from scratch (or the vault-checker bans them off the verified set).

## Why this matters

Without global contracts, the vault contract's ~150 KB WASM has to be base64-encoded into the wallet's redirect URL. That blew past the 32 KB URL ceiling MyNearWallet enforces, so customers couldn't sign vault deploys through a browser wallet at all. With UseGlobalContract:

| | Old (DeployContract inline) | New (UseGlobalContract) |
|---|---|---|
| Tx payload size | ~200 KB | < 1 KB |
| Browser wallet redirect | ❌ Fails URL limit | ✅ Fits comfortably |
| Per-vault on-chain WASM bytes | 150 KB × N | 0 (single global copy) |
| Storage cost per vault | Pays full WASM | Just account storage |
| WASM upgrade flow | None — each vault baked-in forever | Add new global hash, new vaults pick it up |

## Caveats

- The deployer of a global contract by hash pays storage permanently; there's no method to delete a global contract. Pick the canonical operator account.
- Anyone can deploy a WASM as a global contract; the hash is what identifies it, not who paid. So an attacker could front-run the operator and deploy the same WASM. That's harmless: hash-addressed deploys are content-addressed.
- The `as-global-account-id` mode is mutable (the deployer account can replace the code). We use `as-global-hash` precisely so the on-chain code is immutable — vaults reference an unchangeable artefact.
