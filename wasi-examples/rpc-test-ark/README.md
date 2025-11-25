# RPC Test Ark

Test WASM component for OutLayer RPC host functions.

## Overview

This component tests the `near:rpc/api@0.1.0` host functions provided by OutLayer worker:

- `view` - Call view functions on smart contracts
- `view-account` - Get account information
- `view-access-key` - Get access key information
- `block` - Get block information
- `gas-price` - Get current gas price
- `send-tx` - Send signed transaction
- `raw` - Raw JSON-RPC calls
- `call` - Call contract method with explicit signer (WASM provides private key)
- `transfer` - Transfer NEAR tokens with explicit signer (WASM provides private key)

## Building

```bash
# Make sure wasm32-wasip2 target is installed
rustup target add wasm32-wasip2

# Build the component
cargo build --target wasm32-wasip2 --release
```

## Running Tests

### With OutLayer Worker (Full Integration)

1. Start the coordinator and worker with RPC proxy enabled:
   ```bash
   # In coordinator directory
   cargo run

   # In worker directory (with RPC_PROXY_ENABLED=true in .env)
   cargo run
   ```

2. Request execution through the contract or create a task directly.

### With wasi-test-runner (Standalone)

Note: wasi-test-runner doesn't have RPC proxy support by default.
You'll see errors like "Host functions require WIT binding" unless
you modify wasi-test-runner to include RPC host functions.

```bash
# This will fail without RPC proxy - useful for build verification only
cd ../wasi-test-runner
./target/release/wasi-test \
  --wasm ../rpc-test-ark/target/wasm32-wasip2/release/rpc-test-ark.wasm \
  --input '{"test":"all","account_id":"outlayer.testnet"}'
```

## Input Format

```json
{
  "test": "view_account",
  "account_id": "outlayer.testnet",
  "contract_id": "wrap.testnet",
  "method_name": "ft_metadata",
  "args_json": "{}"
}
```

### Test Types

| Test | Description |
|------|-------------|
| `view_account` | Get account info (balance, storage, etc.) |
| `block` | Get latest final block |
| `gas_price` | Get current gas price |
| `view_call` | Call a view function on contract |
| `raw` | Raw RPC call (tests `status` method) |
| `all` | Run all tests |

## Output Format

```json
{
  "success": true,
  "test": "all",
  "results": [
    {
      "name": "view_account",
      "success": true,
      "result": { ... }
    },
    {
      "name": "block",
      "success": true,
      "result": { "height": 123456, "has_header": true, "has_chunks": true }
    },
    {
      "name": "gas_price",
      "success": true,
      "result": { "gas_price": "100000000" }
    },
    {
      "name": "view_call(wrap.testnet.ft_metadata)",
      "success": true,
      "result": { "name": "Wrapped NEAR", "symbol": "wNEAR", ... }
    },
    {
      "name": "raw(status)",
      "success": true,
      "result": { "chain_id": "testnet", "version": { ... } }
    }
  ]
}
```

## How It Works

1. WASM component imports `near:rpc/api@0.1.0` interface via wit-bindgen
2. Component calls host functions (e.g., `near::rpc::api::view_account`)
3. OutLayer worker's `outlayer_rpc` module handles the request
4. Worker adds API key and sends HTTP request to NEAR RPC
5. Response is returned to WASM and validated

## Architecture

```
┌────────────────────────────────────┐
│  rpc-test-ark WASM Component       │
│                                    │
│  wit_bindgen::generate!()          │
│  near::rpc::api::view_account()    │
└──────────────┬─────────────────────┘
               │ component model import
               ▼
┌────────────────────────────────────┐
│  OutLayer Worker                   │
│  (host_functions.rs)               │
│                                    │
│  linker.instance("near:rpc@0.1.0") │
│  interface.func_wrap_async(...)    │
└──────────────┬─────────────────────┘
               │ HTTP request (with API key)
               ▼
┌────────────────────────────────────┐
│  NEAR RPC                          │
│  (Pagoda, Infura, etc.)            │
└────────────────────────────────────┘
```

## WIT Interface

The component imports `near:rpc/api@0.1.0` with these functions:

```wit
package near:rpc@0.1.0;

interface api {
    view: func(contract-id: string, method-name: string, args-json: string)
        -> tuple<string, string>;
    view-account: func(account-id: string) -> tuple<string, string>;
    view-access-key: func(account-id: string, public-key: string)
        -> tuple<string, string>;
    block: func(finality-or-block-id: string) -> tuple<string, string>;
    gas-price: func() -> tuple<string, string>;
    send-tx: func(signed-tx-base64: string, wait-until: string)
        -> tuple<string, string>;
    raw: func(method: string, params-json: string) -> tuple<string, string>;

    /// Call a contract method with explicit signer (WASM provides private key)
    /// CRITICAL: Worker NEVER signs with its own key. WASM MUST provide signer credentials.
    call: func(signer-id: string, signer-key: string, receiver-id: string,
               method-name: string, args-json: string, deposit-yocto: string, gas: string)
        -> tuple<string, string>;

    /// Transfer NEAR tokens with explicit signer (WASM provides private key)
    /// CRITICAL: Worker NEVER signs with its own key. WASM MUST provide signer credentials.
    transfer: func(signer-id: string, signer-key: string, receiver-id: string,
                   amount-yocto: string)
        -> tuple<string, string>;
}
```

Return format: `(result, error)` - if error is non-empty, the call failed.

## See Also

- [outlayer-rpc-guest](../outlayer-rpc-guest/) - Guest crate documentation
- [WASI_TUTORIAL.md](../WASI_TUTORIAL.md) - WASI development guide
- [worker/.env.example](../../worker/.env.example) - RPC proxy configuration
