# WASI Test Runner

Universal test tool to validate WASM modules for NEAR OutLayer compatibility.

## What It Tests

✅ Binary format correctness (WASI P1 or P2)
✅ Fuel metering (instruction counting)
✅ Input/output handling (stdin → stdout)
✅ Resource limits (memory, instructions)
✅ JSON validation
✅ Output size limits
✅ Compatibility with wasmtime 28+
✅ NEAR RPC host functions (view calls and transactions)

## Installation

```bash
cd wasi-examples/wasi-test-runner
cargo build --release
```

## Usage

### Basic Test

```bash
./target/release/wasi-test --wasm path/to/your-app.wasm --input '{"test":"data"}'
```

### Test with Input File

```bash
./target/release/wasi-test --wasm your-app.wasm --input-file input.json
```

### Test with Custom Limits

```bash
./target/release/wasi-test \
  --wasm your-app.wasm \
  --input '{"value":42}' \
  --max-instructions 50000000 \
  --max-memory-mb 256
```

### Verbose Mode

```bash
./target/release/wasi-test --wasm your-app.wasm --input '{}' --verbose
```

### Test Specific Examples

```bash
# Test random-ark (WASI P1)
./target/release/wasi-test \
  --wasm ../random-ark/target/wasm32-wasip1/release/random-ark.wasm \
  --input '{"min":1,"max":100}'

# Test ai-ark (WASI P2)
./target/release/wasi-test \
  --wasm ../ai-ark/target/wasm32-wasip2/release/ai-ark.wasm \
  --input '{"prompt":"What is NEAR Protocol?"}'

# Test oracle-ark (WASI P2)
./target/release/wasi-test \
  --wasm ../oracle-ark/target/wasm32-wasip2/release/oracle-ark.wasm \
  --input '{"tokens":[{"token_id":"bitcoin","sources":[{"name":"coingecko","token_id":null}],"aggregation_method":"average","min_sources_num":1}],"max_price_deviation_percent":10.0}' \
  --max-instructions 50000000000
```

### Test with NEAR RPC (View Calls Only)

```bash
# Test rpc-test-ark with view calls
./target/release/wasi-test \
  --wasm ../rpc-test-ark/target/wasm32-wasip2/release/rpc-test-ark.wasm \
  --input '{"test":"view_account","account_id":"outlayer.testnet"}' \
  --rpc \
  --max-instructions 50000000000

# Test with custom RPC endpoint
./target/release/wasi-test \
  --wasm ../rpc-test-ark/target/wasm32-wasip2/release/rpc-test-ark.wasm \
  --input '{"test":"all","account_id":"outlayer.testnet"}' \
  --rpc \
  --rpc-url "https://rpc.testnet.near.org" \
  --max-instructions 50000000000
```

### Test with NEAR RPC (Transactions)

```bash
# Test botfather-ark - create accounts (requires credentials via env)
./target/release/wasi-test \
  --wasm ../botfather-ark/target/wasm32-wasip2/release/bot-father.wasm \
  --input '{"action":"create_accounts","prompt":"space theme","count":3,"deposit_per_account":"1000000000000000000000000"}' \
  --env OPENAI_API_KEY=sk-... \
  --env BOT_FATHER_MASTER_KEY=ed25519:... \
  --env NEAR_SENDER_ID=alice.testnet \
  --env NEAR_SENDER_PRIVATE_KEY=ed25519:... \
  --rpc \
  --rpc-allow-transactions \
  --max-instructions 50000000000

# Test botfather-ark - list accounts (no credentials needed)
./target/release/wasi-test \
  --wasm ../botfather-ark/target/wasm32-wasip2/release/bot-father.wasm \
  --input '{"action":"list_accounts","indices":[]}' \
  --env BOT_FATHER_MASTER_KEY=ed25519:... \
  --env NEAR_SENDER_ID=alice.testnet \
  --rpc \
  --max-instructions 50000000000

# Test botfather-ark - fund specific accounts
./target/release/wasi-test \
  --wasm ../botfather-ark/target/wasm32-wasip2/release/bot-father.wasm \
  --input '{"action":"fund_accounts","total_amount":"30000000000000000000000000","indices":[0,2]}' \
  --env BOT_FATHER_MASTER_KEY=ed25519:... \
  --env NEAR_SENDER_ID=alice.testnet \
  --env NEAR_SENDER_PRIVATE_KEY=ed25519:... \
  --rpc \
  --rpc-allow-transactions \
  --max-instructions 50000000000
```

## Example Output

### Successful Test

```
🔍 Testing WASM module: random-ark.wasm
📝 Input: {"min":1,"max":100}
⚙️  Max instructions: 10000000000
💾 Max memory: 128 MB

✓ Detected: WASI Preview 1 Module
✅ Execution successful!

📊 Results:
  - Fuel consumed: 456789 instructions
  - Output size: 24 bytes

📤 Output:
{"random_number":42}

✓ Output is valid JSON
✅ All checks passed! Module is compatible with NEAR OutLayer.
```

### Failed Test

```
🔍 Testing WASM module: broken-app.wasm
📝 Input: {}
⚙️  Max instructions: 10000000000
💾 Max memory: 128 MB

❌ Execution failed!

Error: Failed to find _start function. Make sure you're using [[bin]] format with fn main()

💡 Common issues:
  - Make sure you're using [[bin]] format, not [lib]
  - Check that you have fn main() as entry point
  - Verify you're reading from stdin and writing to stdout
  - Use correct build target (wasm32-wasip1 or wasm32-wasip2)

📚 See WASI_TUTORIAL.md for detailed guide
```

## What Gets Validated

### 1. Binary Format
- ✅ Valid WASI P1 module with `_start` entry point
- ✅ Valid WASI P2 component with component model
- ❌ Old library format with custom exports

### 2. Input Handling
- ✅ Reads from stdin
- ❌ Uses command-line arguments (not supported)

### 3. Output Handling
- ✅ Writes to stdout
- ✅ Flushes output buffer
- ⚠️  Warns if output > 900 bytes
- ⚠️  Warns if output is not JSON

### 4. Resource Metering
- ✅ Fuel consumption tracked
- ✅ Memory limits enforced
- ✅ Reports actual instruction count

### 5. Compatibility
- ✅ Works with wasmtime 28+
- ✅ Same runtime as NEAR OutLayer worker

## Testing Your Own Module

### Step 1: Build Your WASM

```bash
# WASI P1
cargo build --target wasm32-wasip1 --release

# WASI P2
cargo build --target wasm32-wasip2 --release
```

### Step 2: Prepare Test Input

Create `test-input.json`:
```json
{
  "name": "Alice",
  "value": 42
}
```

### Step 3: Run Test

```bash
cd wasi-examples/wasi-test-runner
cargo run --release -- \
  --wasm ../../your-project/target/wasm32-wasip1/release/your-app.wasm \
  --input-file test-input.json \
  --verbose
```

### Step 4: Check Results

If all checks pass, your module is ready for NEAR OutLayer! 🎉

## Common Issues & Solutions

### "Failed to find _start function"

**Problem**: Using library format instead of binary

**Solution**: Change Cargo.toml:
```toml
# ✅ Use this
[[bin]]
name = "my-app"
path = "src/main.rs"

# ❌ Not this
[lib]
crate-type = ["cdylib"]
```

### "Output is empty"

**Problem**: Not writing to stdout or not flushing

**Solution**:
```rust
print!("{}", output);
io::stdout().flush()?; // Don't forget this!
```

### "Not a valid WASI P1 module or P2 component"

**Problem**: Wrong build target

**Solution**:
```bash
# Use one of these targets
cargo build --target wasm32-wasip1 --release
cargo build --target wasm32-wasip2 --release

# ❌ Not these
cargo build --target wasm32-unknown-unknown --release
```

### "Output is 2000 bytes (limit is 900 bytes)"

**Problem**: Output too large for NEAR Protocol

**Solution**: Truncate before returning:
```rust
let mut output = generate_output();
if output.len() > 800 {
    output.truncate(800);
    output.push_str("...");
}
```

## Integration with CI/CD

### GitHub Actions Example

```yaml
name: Test WASM

on: [push, pull_request]

jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3

      - name: Install Rust
        uses: actions-rs/toolchain@v1
        with:
          toolchain: stable
          target: wasm32-wasip1

      - name: Build WASM
        run: cargo build --target wasm32-wasip1 --release

      - name: Install Test Runner
        run: |
          cd wasi-examples/wasi-test-runner
          cargo build --release

      - name: Test WASM
        run: |
          ./wasi-examples/wasi-test-runner/target/release/wasi-test \
            --wasm target/wasm32-wasip1/release/my-app.wasm \
            --input '{"test":"data"}'
```

## Command-Line Options

```
Options:
  -w, --wasm <WASM>
          Path to WASM file

  -i, --input <INPUT>
          Input JSON data (or use --input-file)

      --input-file <INPUT_FILE>
          Path to input JSON file

      --max-instructions <MAX_INSTRUCTIONS>
          Maximum instructions (fuel limit)
          [default: 10000000000]

      --max-memory-mb <MAX_MEMORY_MB>
          Maximum memory in MB
          [default: 128]

  -e, --env <ENV>
          Environment variables (format: KEY=value, can be specified multiple times)

      --rpc
          Enable NEAR RPC proxy (provides near:rpc/api host functions)

      --rpc-url <RPC_URL>
          NEAR RPC URL for proxy
          [default: https://rpc.testnet.near.org]

      --rpc-max-calls <RPC_MAX_CALLS>
          Maximum RPC calls per execution
          [default: 100]

      --rpc-allow-transactions
          Allow transaction methods (call, transfer)

  -v, --verbose
          Verbose output

  -h, --help
          Print help

  -V, --version
          Print version
```

## NEAR RPC Host Functions

When `--rpc` flag is enabled, the test runner provides `near:rpc/api` host functions that WASM modules can use to interact with NEAR blockchain.

### Available Functions

**View Functions** (read-only, no credentials required):
- `view(contract_id, method_name, args_json)` - Call view function on contract
- `view_account(account_id)` - Get account information (balance, storage, etc.)
- `view_access_key(account_id, public_key)` - Get access key information
- `block(finality_or_block_id)` - Get block information
- `gas_price()` - Get current gas price
- `raw(method, params_json)` - Raw JSON-RPC call

**Transaction Functions** (require `--rpc-allow-transactions` + credentials via `--env`):
- `call(signer_id, signer_key, receiver_id, method_name, args_json, deposit_yocto, gas)` - Call contract method with transaction
- `transfer(signer_id, signer_key, receiver_id, amount_yocto)` - Transfer NEAR tokens

### Security Model

**CRITICAL**: Worker (test runner) NEVER signs transactions with its own key. All transaction signer credentials MUST be provided by WASM via environment variables:

- `NEAR_SENDER_ID` - Account ID that will sign transactions
- `NEAR_SENDER_PRIVATE_KEY` - Private key in NEAR format (ed25519:base58...)

WASM code reads these from environment and explicitly passes to `call()` or `transfer()` functions. This is the same security model as production OutLayer workers.

### Environment Variables

Pass secrets and configuration to WASM using `--env` flag:

```bash
--env NEAR_SENDER_ID=alice.testnet \
--env NEAR_SENDER_PRIVATE_KEY=ed25519:... \
--env OPENAI_API_KEY=sk-... \
--env BOT_FATHER_MASTER_KEY=ed25519:...
```

WASM code accesses these via standard `std::env::var("KEY_NAME")`.

### Example: Testing WASM with RPC

```rust
// In your WASM code (Rust)
use std::env;

// Read credentials from environment (passed via --env)
let signer_id = env::var("NEAR_SENDER_ID").unwrap();
let signer_key = env::var("NEAR_SENDER_PRIVATE_KEY").unwrap();

// Call NEAR RPC host function
let (result, error) = near::rpc::api::view_account(&signer_id);

// For transactions, explicitly provide credentials
let (tx_hash, error) = near::rpc::api::transfer(
    &signer_id,           // From env
    &signer_key,          // From env
    "receiver.testnet",
    "1000000000000000000000000",  // 1 NEAR in yoctoNEAR
);
```

Test this WASM:
```bash
./target/release/wasi-test \
  --wasm your-app.wasm \
  --input '{"action":"transfer"}' \
  --env NEAR_SENDER_ID=alice.testnet \
  --env NEAR_SENDER_PRIVATE_KEY=ed25519:... \
  --rpc \
  --rpc-allow-transactions
```

## Requirements

- Rust 1.85+ (for building test runner)
- wasmtime 28+ (included as dependency)
- Valid WASI P1 or P2 WASM binary

## See Also

- [WASI_TUTORIAL.md](../WASI_TUTORIAL.md) - Complete WASI development guide
- [random-ark](../random-ark/) - Example WASI P1 module
- [ai-ark](../ai-ark/) - Example WASI P2 component
- [rpc-test-ark](../rpc-test-ark/) - Example with NEAR RPC host functions
- [botfather-ark](../botfather-ark/) - Example with RPC transactions and secrets
- [SECURITY_AUDIT_REPORT.md](../SECURITY_AUDIT_REPORT.md) - Security model for transaction signing

---

**Last updated**: 2025-11-24
**Compatible with**: wasmtime 28+, NEAR OutLayer MVP
