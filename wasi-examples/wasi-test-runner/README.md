# WASI Test Runner

Universal test tool to validate WASM modules for NEAR Offshore compatibility.

## What It Tests

✅ Binary format correctness (WASI P1 or P2)
✅ Fuel metering (instruction counting)
✅ Input/output handling (stdin → stdout)
✅ Resource limits (memory, instructions)
✅ JSON validation
✅ Output size limits
✅ Compatibility with wasmtime 28+

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
✅ All checks passed! Module is compatible with NEAR Offshore.
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
- ✅ Same runtime as NEAR Offshore worker

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

If all checks pass, your module is ready for NEAR Offshore! 🎉

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

  -v, --verbose
          Verbose output

  -h, --help
          Print help

  -V, --version
          Print version
```

## Requirements

- Rust 1.85+ (for building test runner)
- wasmtime 28+ (included as dependency)
- Valid WASI P1 or P2 WASM binary

## See Also

- [WASI_TUTORIAL.md](../WASI_TUTORIAL.md) - Complete WASI development guide
- [random-ark](../random-ark/) - Example WASI P1 module
- [ai-ark](../ai-ark/) - Example WASI P2 component

---

**Last updated**: 2025-10-15
**Compatible with**: wasmtime 28+, NEAR Offshore MVP
