# OffchainVM Smart Contract

NEAR smart contract for off-chain WASM execution using yield/resume mechanism.

## Features

- **Yield/Resume Mechanism**: Uses `promise_yield_create` to pause execution
- **Off-chain Computation**: Execute arbitrary WASM code off-chain
- **Resource Limits**: Configurable limits for instructions, memory, and time
- **Dynamic Pricing**: Cost calculated based on actual resource usage
- **Stale Request Cancellation**: Users can cancel requests after timeout
- **Admin Controls**: Owner can manage operators, pricing, and pause contract
- **Secret Management**: Encrypted secrets support via keystore worker integration

## Contract API

### User Functions

#### `request_execution`
Request off-chain execution of WASM code.

**Basic execution (no secrets):**
```bash
near call offchainvm.testnet request_execution '{
  "code_source": {
    "repo": "https://github.com/user/project",
    "commit": "abc123",
    "build_target": "wasm32-wasi"
  },
  "resource_limits": {
    "max_instructions": 1000000000,
    "max_memory_mb": 128,
    "max_execution_seconds": 60
  },
  "input_data": "{\"key\": \"value\"}",
  "encrypted_secrets": null
}' --accountId user.testnet --deposit 0.1
```

**With encrypted secrets (e.g., API keys):**
```bash
# 1. Get keystore public key
near view offchainvm.testnet get_keystore_pubkey

# 2. Encrypt your secrets with the public key (use keystore encryption library)
# encrypted_data = encrypt_for_keystore(pubkey, "OPENAI_API_KEY=sk-...")

# 3. Call with encrypted secrets
near call offchainvm.testnet request_execution '{
  "code_source": {...},
  "resource_limits": {...},
  "input_data": "{...}",
  "encrypted_secrets": [1, 2, 3, ...]
}' --accountId user.testnet --deposit 0.1
```

#### `cancel_stale_execution`
Cancel execution request after timeout (10 minutes).

```bash
near call offchainvm.testnet cancel_stale_execution '{
  "request_id": 123
}' --accountId user.testnet
```

### Operator Functions

#### `resolve_execution`
Resolve execution with results (called by worker).

**Small output (<1024 bytes):**
```bash
near call offchainvm.testnet resolve_execution '{
  "request_id": 0,
  "response": {
    "success": true,
    "output": {"Text": "Hello, NEAR!"},
    "error": null,
    "resources_used": {
      "instructions": 1000000,
      "time_ms": 100
    }
  }
}' --accountId operator.testnet
```

**Large output (>1024 bytes) - 2-call flow:**

1. First, submit the large output:
```bash
near call offchainvm.testnet submit_execution_output '{
  "request_id": 0,
  "output": {"Text": "Very long output..."}
}' --accountId operator.testnet
```

2. Then, resolve with metadata only:
```bash
near call offchainvm.testnet resolve_execution '{
  "request_id": 0,
  "response": {
    "success": true,
    "output": null,
    "error": null,
    "resources_used": {
      "instructions": 1000000,
      "time_ms": 100
    }
  }
}' --accountId operator.testnet
```

### Admin Functions

#### `set_operator`
Change operator account.

```bash
near call offchainvm.testnet set_operator '{
  "new_operator_id": "new-operator.testnet"
}' --accountId owner.testnet
```

#### `set_pricing`
Update pricing parameters.

```bash
near call offchainvm.testnet set_pricing '{
  "base_fee": "10000000000000000000000",
  "per_instruction_fee": "1000000000000000",
  "per_mb_fee": "100000000000000000000",
  "per_second_fee": "1000000000000000000000"
}' --accountId owner.testnet
```

#### `set_paused`
Pause/unpause contract.

```bash
near call offchainvm.testnet set_paused '{
  "paused": true
}' --accountId owner.testnet
```

### View Functions

#### `get_request`
Get execution request by ID.

```bash
near view offchainvm.testnet get_request '{
  "request_id": 123
}'
```

#### `get_stats`
Get contract statistics.

```bash
near view offchainvm.testnet get_stats '{}'
```

#### `get_pricing`
Get current pricing.

```bash
near view offchainvm.testnet get_pricing '{}'
```

#### `get_config`
Get contract configuration.

```bash
near view offchainvm.testnet get_config '{}'
```

## Events

### `execution_requested`
Emitted when user requests execution.

```json
{
  "standard": "near-offshore",
  "version": "1.0.0",
  "event": "execution_requested",
  "data": [{
    "request_data": "{...}",
    "data_id": [0,1,2,...],
    "timestamp": 1234567890
  }]
}
```

### `execution_completed`
Emitted when execution is completed.

```json
{
  "standard": "near-offshore",
  "version": "1.0.0",
  "event": "execution_completed",
  "data": [{
    "sender_id": "user.testnet",
    "code_source": {...},
    "resources_used": {...},
    "success": true,
    "timestamp": 1234567890
  }]
}
```

## Build & Deploy

### Build

```bash
cargo build --release --target wasm32-unknown-unknown
```

Or with cargo-near:

```bash
cargo near build --release
```

### Deploy

```bash
near deploy offchainvm.testnet \
  --wasmFile target/wasm32-unknown-unknown/release/offchainvm_contract.wasm \
  --initFunction new \
  --initArgs '{"owner_id": "owner.testnet", "operator_id": "operator.testnet"}'
```

### Test

```bash
cargo test
```

Integration tests:

```bash
cargo test --test integration_tests
```

## Pricing

Default pricing (can be updated by owner):

- **Base fee**: 0.01 NEAR per request
- **Per instruction**: 0.000001 NEAR per 1M instructions
- **Per MB memory**: 0.0001 NEAR per MB
- **Per second**: 0.001 NEAR per second

**Example cost calculation:**
- 10M instructions + 64MB memory + 5 seconds = 0.01 + 0.00001 + 0.0064 + 0.005 = **0.02141 NEAR**

## Development

### Project Structure

```
contract/
├── src/
│   ├── lib.rs           # Main contract structure
│   ├── execution.rs     # Execution logic with yield/resume
│   ├── events.rs        # Event emissions
│   ├── views.rs         # Read-only functions
│   └── admin.rs         # Admin functions
├── tests/
│   └── integration_tests.rs
└── Cargo.toml
```

### Key Components

- **promise_yield_create**: Pauses contract execution
- **promise_yield_resume**: Resumes with worker's result
- **DATA_ID_REGISTER**: Register 37 for data_id
- **MIN_RESPONSE_GAS**: 50 Tgas for callback

## License

MIT
