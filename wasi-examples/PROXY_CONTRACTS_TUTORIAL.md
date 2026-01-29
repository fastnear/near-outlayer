# Proxy Contracts for OutLayer

How to build NEAR smart contracts that call OutLayer for off-chain computation.

## Overview

A proxy contract is a NEAR smart contract that:
1. Accepts user calls with attached NEAR
2. Forwards computation requests to OutLayer
3. Receives results via callback
4. Returns results to users or updates state

```
User → Your Contract → OutLayer Contract → Worker → Callback → Your Contract
```

## Basic Setup

### Dependencies

```toml
# Cargo.toml
[dependencies]
near-sdk = "5.9.0"
serde = { version = "1", features = ["derive"] }
serde_json = "1"

[lib]
crate-type = ["cdylib"]

[profile.release]
codegen-units = 1
opt-level = "z"
lto = true
```

### Toolchain

```toml
# rust-toolchain.toml
[toolchain]
channel = "1.85.0"
```

## OutLayer Contract Interface

Define the external contract interface:

```rust
use near_sdk::{ext_contract, AccountId, Gas, NearToken};

const OUTLAYER_CONTRACT_ID: &str = "outlayer.near"; // or "outlayer.testnet"

#[ext_contract(ext_outlayer)]
trait OutLayer {
    fn request_execution(
        &mut self,
        code_source: serde_json::Value,
        resource_limits: serde_json::Value,
        input_data: String,
        secrets_ref: Option<serde_json::Value>,
        response_format: String,
        payer_account_id: Option<AccountId>,
    );
}
```

### Parameters

| Parameter | Type | Description |
|-----------|------|-------------|
| `code_source` | JSON | GitHub URL or WASM hash |
| `resource_limits` | JSON | Memory, instructions, time limits |
| `input_data` | String | JSON input for your WASM |
| `secrets_ref` | Option<JSON> | Reference to encrypted secrets |
| `response_format` | String | `"String"` or `"Json"` |
| `payer_account_id` | Option<AccountId> | Who gets refund on failure |

## Simple Example: Coin Flip

A minimal proxy that calls OutLayer for random number generation.

```rust
use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};
use near_sdk::{env, near_bindgen, AccountId, Gas, NearToken, Promise, PromiseError};

const OUTLAYER_CONTRACT_ID: &str = "outlayer.near";
const MIN_DEPOSIT: NearToken = NearToken::from_millinear(10); // 0.01 NEAR
const CALLBACK_GAS: Gas = Gas::from_tgas(5);

#[ext_contract(ext_outlayer)]
trait OutLayer {
    fn request_execution(
        &mut self,
        code_source: serde_json::Value,
        resource_limits: serde_json::Value,
        input_data: String,
        secrets_ref: Option<serde_json::Value>,
        response_format: String,
        payer_account_id: Option<AccountId>,
    );
}

#[ext_contract(ext_self)]
trait SelfCallback {
    fn on_flip_result(&mut self, player: AccountId) -> String;
}

#[near_bindgen]
#[derive(BorshDeserialize, BorshSerialize, Default)]
pub struct CoinFlip {
    wins: u64,
    losses: u64,
}

#[near_bindgen]
impl CoinFlip {
    #[payable]
    pub fn flip(&mut self) -> Promise {
        let deposit = env::attached_deposit();
        assert!(deposit >= MIN_DEPOSIT, "Minimum deposit is 0.01 NEAR");

        let player = env::predecessor_account_id();

        // Code source - GitHub URL
        let code_source = serde_json::json!({
            "GitHub": {
                "url": "https://github.com/example/random-wasm",
                "hash": null
            }
        });

        // Resource limits
        let resource_limits = serde_json::json!({
            "max_memory_mb": 64,
            "max_instructions": 1_000_000_000u64,
            "max_execution_seconds": 30
        });

        // Input data
        let input_data = serde_json::json!({
            "action": "flip"
        }).to_string();

        // Calculate gas for OutLayer call
        let remaining_gas = env::prepaid_gas().saturating_sub(CALLBACK_GAS);

        ext_outlayer::ext(OUTLAYER_CONTRACT_ID.parse().unwrap())
            .with_attached_deposit(deposit)
            .with_static_gas(remaining_gas)
            .with_unused_gas_weight(1)
            .request_execution(
                code_source,
                resource_limits,
                input_data,
                None,           // no secrets
                "String".into(),
                Some(player.clone()), // refund to player on failure
            )
            .then(
                ext_self::ext(env::current_account_id())
                    .with_static_gas(CALLBACK_GAS)
                    .on_flip_result(player)
            )
    }

    #[private]
    pub fn on_flip_result(
        &mut self,
        player: AccountId,
        #[callback_result] result: Result<Option<String>, PromiseError>,
    ) -> String {
        match result {
            Ok(Some(output)) => {
                if output.contains("heads") {
                    self.wins += 1;
                    format!("{} won! Result: {}", player, output)
                } else {
                    self.losses += 1;
                    format!("{} lost! Result: {}", player, output)
                }
            }
            Ok(None) => {
                self.losses += 1;
                "No result returned".to_string()
            }
            Err(_) => {
                // Deposit refunded to payer_account_id
                "Execution failed".to_string()
            }
        }
    }

    pub fn stats(&self) -> (u64, u64) {
        (self.wins, self.losses)
    }
}
```

## Code Source Formats

### GitHub URL

```rust
let code_source = serde_json::json!({
    "GitHub": {
        "url": "https://github.com/owner/repo",
        "hash": null  // or specific commit hash
    }
});
```

### Project ID

```rust
let code_source = serde_json::json!({
    "Project": {
        "project_id": "alice.near/my-app"
    }
});
```

### WASM Hash (pre-uploaded)

```rust
let code_source = serde_json::json!({
    "WasmHash": {
        "hash": "abc123..."
    }
});
```

## Advanced: OutLayerResponse Wrapper

For JSON responses, OutLayer wraps results:

```rust
#[derive(Deserialize)]
struct OutLayerResponse<T> {
    success: bool,
    result: Option<T>,
    error: Option<String>,
}
```

### Parsing JSON Results

```rust
use serde::Deserialize;

#[derive(Deserialize)]
struct VoteResult {
    yes_votes: u64,
    no_votes: u64,
    passed: bool,
}

#[private]
pub fn on_tally_result(
    &mut self,
    proposal_id: u64,
    #[callback_result] result: Result<Option<String>, PromiseError>,
) {
    let Ok(Some(output)) = result else {
        env::log_str("Execution failed");
        return;
    };

    // Parse OutLayerResponse wrapper
    let response: OutLayerResponse<VoteResult> = match serde_json::from_str(&output) {
        Ok(r) => r,
        Err(e) => {
            env::log_str(&format!("Parse error: {}", e));
            return;
        }
    };

    if !response.success {
        env::log_str(&format!("WASM error: {:?}", response.error));
        return;
    }

    if let Some(vote_result) = response.result {
        // Update proposal state
        if let Some(proposal) = self.proposals.get_mut(&proposal_id) {
            proposal.yes_votes = vote_result.yes_votes;
            proposal.no_votes = vote_result.no_votes;
            proposal.passed = Some(vote_result.passed);
        }
    }
}
```

## Using Secrets

For WASM that needs encrypted secrets (API keys, private keys):

```rust
#[payable]
pub fn execute_with_secrets(&mut self) -> Promise {
    let player = env::predecessor_account_id();

    // Reference secrets owned by current contract
    let secrets_ref = serde_json::json!({
        "owner_id": env::current_account_id(),
        "names": ["API_KEY", "PRIVATE_KEY"]
    });

    ext_outlayer::ext(OUTLAYER_CONTRACT_ID.parse().unwrap())
        .with_attached_deposit(env::attached_deposit())
        .with_static_gas(env::prepaid_gas().saturating_sub(CALLBACK_GAS))
        .with_unused_gas_weight(1)
        .request_execution(
            code_source,
            resource_limits,
            input_data,
            Some(secrets_ref),  // <- secrets reference
            "Json".into(),
            Some(player),
        )
        .then(/* callback */)
}
```

The WASM code accesses secrets via env vars:

```rust
// In your WASM
let api_key = std::env::var("API_KEY").ok();
let private_key = std::env::var("PRIVATE_KEY").ok();
```

## Gas Management

### Constants

```rust
const CALLBACK_GAS: Gas = Gas::from_tgas(5);      // 5 TGas for callback
const OUTLAYER_BASE_GAS: Gas = Gas::from_tgas(10); // Minimum for OutLayer
```

### Gas Allocation Pattern

```rust
// Reserve gas for callback, give rest to OutLayer
let remaining_gas = env::prepaid_gas()
    .saturating_sub(CALLBACK_GAS)
    .saturating_sub(Gas::from_tgas(5)); // buffer for current call

ext_outlayer::ext(...)
    .with_static_gas(remaining_gas)
    .with_unused_gas_weight(1)  // OutLayer gets unused gas
    .request_execution(...)
```

### `with_unused_gas_weight(1)`

This is important - it tells NEAR to give any unused gas from the current call to the OutLayer call. OutLayer needs gas to:
1. Parse and validate request
2. Store job in queue
3. Return result via callback

## Deposit Requirements

OutLayer charges based on computation used. Minimum: **0.01 NEAR**.

```rust
const MIN_DEPOSIT: NearToken = NearToken::from_millinear(10);

#[payable]
pub fn my_method(&mut self) -> Promise {
    let deposit = env::attached_deposit();
    assert!(deposit >= MIN_DEPOSIT, "Minimum deposit is 0.01 NEAR");
    // ...
}
```

Unused deposit is refunded to `payer_account_id`.

## Multiple OutLayer Calls

For complex workflows with multiple OutLayer calls:

```rust
#[payable]
pub fn complex_workflow(&mut self) -> Promise {
    let deposit = env::attached_deposit();
    let half_deposit = NearToken::from_yoctonear(deposit.as_yoctonear() / 2);

    // First call
    let first_call = ext_outlayer::ext(OUTLAYER_CONTRACT_ID.parse().unwrap())
        .with_attached_deposit(half_deposit)
        .with_static_gas(Gas::from_tgas(50))
        .request_execution(/* step 1 params */);

    // Chain with callback that triggers second call
    first_call.then(
        ext_self::ext(env::current_account_id())
            .with_static_gas(Gas::from_tgas(100))
            .on_first_result()
    )
}

#[private]
pub fn on_first_result(
    &mut self,
    #[callback_result] result: Result<Option<String>, PromiseError>,
) -> Promise {
    // Process first result, then call OutLayer again
    let second_call = ext_outlayer::ext(OUTLAYER_CONTRACT_ID.parse().unwrap())
        .with_attached_deposit(NearToken::from_millinear(10))
        .with_static_gas(Gas::from_tgas(50))
        .request_execution(/* step 2 params */);

    second_call.then(
        ext_self::ext(env::current_account_id())
            .with_static_gas(Gas::from_tgas(5))
            .on_final_result()
    )
}
```

## Error Handling

### Callback Result Types

```rust
#[callback_result] result: Result<Option<String>, PromiseError>
```

| Result | Meaning |
|--------|---------|
| `Ok(Some(output))` | Success, WASM returned output |
| `Ok(None)` | Success but no output (unusual) |
| `Err(PromiseError)` | OutLayer call failed |

### Refunds on Failure

When `payer_account_id` is set, OutLayer refunds unused deposit on failure:

```rust
ext_outlayer::ext(...)
    .request_execution(
        // ...
        Some(env::predecessor_account_id()), // refund to caller
    )
```

## Build & Deploy

```bash
# Build
cargo near build

# Deploy
near deploy your-contract.testnet ./target/near/your_contract.wasm

# Initialize (if needed)
near call your-contract.testnet new '{}' --accountId your-contract.testnet
```

## Testing Locally

```bash
# Call your proxy contract
near call your-contract.testnet flip '{}' \
    --accountId alice.testnet \
    --deposit 0.1 \
    --gas 100000000000000
```

## Complete Example Structure

```
my-proxy/
├── Cargo.toml
├── rust-toolchain.toml
├── src/
│   └── lib.rs
└── build.sh
```

### build.sh

```bash
#!/bin/bash
set -e
cargo near build
```

## See Also

- [WASI Tutorial](./WASI_TUTORIAL.md) - Building WASM modules for OutLayer
- [WASM Environment Variables](./WASM_ENV_VARS.md) - Env vars available in WASM
- [random-ark](./random-ark/) - Simple coin flip example
- [private-dao-ark](./private-dao-ark/) - Complex DAO with voting

## Examples in This Repository

| Example | Description | Complexity |
|---------|-------------|------------|
| [random-ark](./random-ark/) | Coin flip with random number | Simple |
| [private-dao-ark](./private-dao-ark/) | DAO with encrypted voting | Advanced |
