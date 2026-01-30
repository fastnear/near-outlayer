# OutLayer Smart Contract

NEAR smart contract for off-chain WASM execution using yield/resume mechanism.

- Testnet: `outlayer.testnet`
- Mainnet: `outlayer.near`

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
near call outlayer.testnet request_execution '{
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
  "input_data": "{\"key\": \"value\"}"  
}' --accountId user.testnet --deposit 0.01
```

**With encrypted secrets (e.g., API keys):**
```bash
# 1. Get keystore public key
near view outlayer.testnet get_keystore_pubkey

# 2. Encrypt your secrets with the public key (use keystore encryption library)
# encrypted_data = encrypt_for_keystore(pubkey, "OPENAI_API_KEY=sk-...")

# 3. Call with encrypted secrets
near call outlayer.testnet request_execution '{
  "code_source": {...},
  "resource_limits": {...},
  "input_data": "{...}",
  "secrets_ref": {
    "profile": "default",
    "account_id": "dev.testnet"
  }
}' --accountId user.testnet --deposit 0.1
```

#### `cancel_stale_execution`
Cancel execution request after timeout (10 minutes).

```bash
near call outlayer.testnet cancel_stale_execution '{
  "request_id": 123
}' --accountId user.testnet
```

### Operator Functions

#### `resolve_execution`
Resolve execution with results (called by worker).

**Small output (<1024 bytes):**
```bash
near call outlayer.testnet resolve_execution '{
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

#### `submit_execution_output_and_resolve`
Optimized single-transaction method for large outputs (>1024 bytes). Used automatically by worker.

```bash
near call outlayer.testnet submit_execution_output_and_resolve '{
  "request_id": 0,
  "output": {"Text": "Very long output..."},
  "success": true,
  "error": null,
  "resources_used": {
    "instructions": 1000000,
    "time_ms": 100,
    "compile_time_ms": null
  },
  "compilation_note": null
}' --accountId operator.testnet
```

**Note**: Worker automatically chooses between `resolve_execution` (small output) and `submit_execution_output_and_resolve` (large output) based on payload size.

### Admin Functions

#### `set_operator`
Change operator account.

```bash
near call outlayer.testnet set_operator '{
  "new_operator_id": "new-operator.testnet"
}' --accountId owner.testnet
```

#### `set_pricing`
Update pricing parameters.

```bash
near call outlayer.testnet set_pricing '{
  "base_fee": "10000000000000000000000",
  "per_instruction_fee": "1000000000000000",
  "per_mb_fee": "100000000000000000000",
  "per_second_fee": "1000000000000000000000"
}' --accountId owner.testnet
```

#### `set_paused`
Pause/unpause contract.

```bash
near call outlayer.testnet set_paused '{
  "paused": true
}' --accountId owner.testnet
```

### View Functions

#### `get_request`
Get execution request by ID.

```bash
near view outlayer.testnet get_request '{
  "request_id": 123
}'
```

#### `get_stats`
Get contract statistics.

```bash
near view outlayer.testnet get_stats '{}'
```

#### `get_pricing`
Get current pricing.

```bash
near view outlayer.testnet get_pricing '{}'
```

#### `get_config`
Get contract configuration.

```bash
near view outlayer.testnet get_config '{}'
```

### Secrets Management Functions

#### `store_secrets`
Store encrypted secrets for a repository.

**Important:** Always estimate storage cost first using `estimate_storage_cost` to attach the correct deposit.

```bash
# 1. Estimate storage cost
near view outlayer.testnet estimate_storage_cost '{
  "repo": "github.com/alice/project",
  "branch": "main",
  "profile": "default",
  "owner": "alice.testnet",
  "encrypted_secrets_base64": "YWJjZGVm...",
  "access": "AllowAll"
}'
# Output: "1500000000000000000000" (0.0015 NEAR)

# 2. Store secrets with exact deposit
near call outlayer.testnet store_secrets '{
  "repo": "github.com/alice/project",
  "branch": "main",
  "profile": "default",
  "encrypted_secrets_base64": "YWJjZGVm...",
  "access": "AllowAll"
}' --accountId alice.testnet --deposit 0.0015
```

#### `estimate_storage_cost`
Estimate the storage cost before storing secrets. Returns exact cost in yoctoNEAR.

```bash
near view outlayer.testnet estimate_storage_cost '{
  "repo": "github.com/alice/project",
  "branch": null,
  "profile": "production",
  "owner": "alice.testnet",
  "encrypted_secrets_base64": "YWJjZGVm...",
  "access": {"Whitelist": {"accounts": ["alice.testnet", "bob.testnet"]}}
}'
```

**Pricing factors:**
- Base overhead: 40 bytes (LookupMap entry)
- Key size: repo + branch + profile + owner (with Borsh length prefixes)
- Value size: encrypted_secrets + access condition + timestamps
- Index overhead: 64 bytes (for new secrets)
- Storage price: 0.00001 NEAR per byte

**Note:** Complex access conditions (e.g., Whitelist with many accounts) cost more than simple ones (e.g., AllowAll).

#### `get_secrets`
Retrieve secrets for a repository (called by keystore worker).

```bash
near view outlayer.testnet get_secrets '{
  "repo": "github.com/alice/project",
  "branch": "main",
  "profile": "default",
  "owner": "alice.testnet"
}'
```

#### `delete_secrets`
Delete secrets and get storage deposit refund.

```bash
near call outlayer.testnet delete_secrets '{
  "repo": "github.com/alice/project",
  "branch": "main",
  "profile": "default"
}' --accountId alice.testnet
```

#### `list_user_secrets`
List all secrets stored by an account.

```bash
near view outlayer.testnet list_user_secrets '{
  "account_id": "alice.testnet"
}'
```

## Events

### `execution_requested`
Emitted when user requests execution.

```json
{
  "standard": "near-outlayer",
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
  "standard": "near-outlayer",
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
./build.sh
```

### Deploy with init

```bash
near contract deploy outlayer.testnet use-file res/local/outlayer_contract.wasm with-init-call new json-args '{"owner_id":"owner.outlayer.testnet","operator_id":"worker.outlayer.testnet"}' prepaid-gas '100.0 Tgas' attached-deposit '0 NEAR' network-config testnet sign-with-keychain send
```

### Set event standard

```
near contract call-function as-transaction dev.outlayer.testnet set_event_metadata json-args '{"standard":"near-outlayer-dev"}' prepaid-gas '100.0 Tgas' attached-deposit '0 NEAR' sign-as owner.outlayer.testnet network-config testnet sign-with-keychain send
```


### Set operator account

```
near contract call-function as-transaction dev.outlayer.testnet set_operator json-args '{"new_operator_id":"dev.outlayer.testnet"}' prepaid-gas '100.0 Tgas' attached-deposit '0 NEAR' sign-as owner.outlayer.testnet network-config testnet sign-with-keychain send
```

### Set testnet USDC

near contract call-function as-transaction dev.outlayer.testnet set_payment_token_contract json-args '{"token_contract":"usdc.fakes.testnet"}' prepaid-gas '100.0 Tgas' attached-deposit '0 NEAR' sign-as owner.outlayer.testnet network-config testnet sign-with-keychain send

# register storage
near contract call-function as-transaction usdc.fakes.testnet storage_deposit json-args '{"account_id": "dev.outlayer.testnet"}' prepaid-gas '100.0 Tgas' attached-deposit '0.1 NEAR' sign-as dev.outlayer.testnet network-config testnet sign-with-keychain send

### Deploy without init

```bash
near contract deploy dev.outlayer.testnet use-file res/local/outlayer_contract.wasm without-init-call network-config testnet sign-with-keychain send
```

```bash
near contract deploy outlayer.testnet use-file res/local/outlayer_contract.wasm without-init-call network-config testnet sign-with-keychain send
```

# Mainnet
```
near contract deploy outlayer.near use-file res/local/outlayer_contract.wasm without-init-call network-config mainnet sign-with-keychain send

near contract call-function as-transaction outlayer.near new json-args '{"owner_id":"owner.outlayer.near","operator_id":"worker.outlayer.near"}' prepaid-gas '100.0 Tgas' attached-deposit '0 NEAR' sign-as outlayer.near network-config mainnet sign-with-keychain send

near contract call-function as-transaction outlayer.near set_payment_token_contract json-args '{"token_contract":"17208628f84f5d6ad33f0da3bbbeb27ffcb398eac501a31bd6ad2011e36133a1"}' prepaid-gas '100.0 Tgas' attached-deposit '0 NEAR' sign-as owner.outlayer.near network-config mainnet sign-with-keychain send

near contract call-function as-transaction 17208628f84f5d6ad33f0da3bbbeb27ffcb398eac501a31bd6ad2011e36133a1 storage_deposit json-args '{"account_id": "outlayer.near"}' prepaid-gas '100.0 Tgas' attached-deposit '0.1 NEAR' sign-as outlayer.near network-config mainnet sign-with-keychain send
```

### Test

```bash
cargo test
```

## License

MIT
