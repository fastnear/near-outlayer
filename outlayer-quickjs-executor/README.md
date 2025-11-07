# QuickJS Executor for OutLayer

Demo-mode QuickJS executor that runs JavaScript contracts with deterministic NEAR storage shim.

## Overview

This crate provides JavaScript contract execution for OutLayer without requiring:
- C compilation toolchain
- Linux kernel
- Custom syscall bridge

Everything works via a preopened `/work` directory with file-based I/O.

## Architecture

```
┌─────────────────────────────────────────────────┐
│  QuickJsExecutor                                │
│  ┌───────────────────────────────────────────┐  │
│  │  Temp Directory (/work)                   │  │
│  │  ├── loader.mjs      (execution runtime)  │  │
│  │  ├── contract.js     (your JS contract)   │  │
│  │  ├── state.json      (persistent state)   │  │
│  │  ├── args.json       (function + args)    │  │
│  │  └── out.json        (execution result)   │  │
│  └───────────────────────────────────────────┘  │
│                                                  │
│  ┌───────────────────────────────────────────┐  │
│  │  QuickJS WASM (qjs.wasm)                  │  │
│  │  - Evaluates contract.js                  │  │
│  │  - Calls requested function               │  │
│  │  - Uses near.storageRead/Write shim       │  │
│  └───────────────────────────────────────────┘  │
└─────────────────────────────────────────────────┘
```

## NEAR Storage Shim

The loader provides a deterministic `near` global object:

```javascript
globalThis.near = {
  storageRead: (k) => state[k],
  storageWrite: (k, v) => { state[k] = v; },
  log: (...args) => stderr.puts(args.join(' ') + '\n'),
};
```

State is persisted to `/work/state.json` between invocations.

## Usage

### 1. Obtain QuickJS WASM Binary

Download from [second-state/quickjs-wasi](https://github.com/second-state/quickjs-wasi) or build locally:

```bash
# Option A: Download prebuilt
curl -LO https://github.com/second-state/quickjs-wasi/releases/download/v0.5.0-alpha/quickjs.wasm

# Option B: Build from source
git clone https://github.com/second-state/quickjs-wasi
cd quickjs-wasi
cargo build --release --target wasm32-wasi
```

### 2. Set Environment Variable

```bash
export QJS_WASM=/absolute/path/to/quickjs.wasm
```

### 3. Write JavaScript Contract

```javascript
// counter.js
globalThis.increment = function () {
  let count = near.storageRead("count") || 0;
  count = (count|0) + 1;
  near.storageWrite("count", count);
  return { count: count };
};

globalThis.getValue = function () {
  let count = near.storageRead("count") || 0;
  return { count: count };
};
```

### 4. Execute

```rust
use outlayer_quickjs_executor::{QuickJsConfig, QuickJsExecutor, Invocation};
use std::time::Duration;

let qjs_wasm = std::fs::read(std::env::var("QJS_WASM")?)?;
let contract_src = std::fs::read_to_string("counter.js")?;

let config = QuickJsConfig {
    max_wall_time: Duration::from_secs(10),
    max_fuel: 100_000_000,
};

let executor = QuickJsExecutor::new(&qjs_wasm, config)?;

// First call: increment from 0 → 1
let inv1 = Invocation {
    contract_source: &contract_src,
    function: "increment",
    args: serde_json::json!([]),
    prior_state_json: b"{}",
};
let result1 = executor.execute(&inv1)?;
println!("Result: {}", result1.result); // {"count": 1}

// Second call: increment from 1 → 2
let inv2 = Invocation {
    contract_source: &contract_src,
    function: "increment",
    args: serde_json::json!([]),
    prior_state_json: &result1.new_state_json,
};
let result2 = executor.execute(&inv2)?;
println!("Result: {}", result2.result); // {"count": 2}
```

## Running Tests

```bash
# Set QuickJS WASM path
export QJS_WASM=/path/to/quickjs.wasm

# Run all tests
cargo test -p outlayer-quickjs-executor -- --nocapture

# Expected output:
# test counter_persists_state_across_invocations ... ok
# test add_is_pure_function_no_state_change ... ok
```

## API Reference

### `QuickJsConfig`

```rust
pub struct QuickJsConfig {
    /// Maximum wall-clock time for execution
    pub max_wall_time: Duration,
    /// Fuel budget (instruction metering)
    pub max_fuel: u64,
}
```

### `Invocation<'a>`

```rust
pub struct Invocation<'a> {
    /// JavaScript contract source code
    pub contract_source: &'a str,
    /// Function to call (e.g., "increment")
    pub function: &'a str,
    /// JSON arguments array
    pub args: serde_json::Value,
    /// Prior state JSON bytes (pass b"{}" for fresh state)
    pub prior_state_json: &'a [u8],
}
```

### `InvocationResult`

```rust
pub struct InvocationResult {
    /// New state JSON bytes (pass to next invocation)
    pub new_state_json: Vec<u8>,
    /// Function return value as JSON
    pub result: serde_json::Value,
    /// Captured log lines (currently unused)
    pub logs: Vec<String>,
}
```

### `QuickJsExecutor`

```rust
impl QuickJsExecutor {
    /// Create executor from QuickJS WASM bytes
    pub fn new(quickjs_wasm: &[u8], cfg: QuickJsConfig) -> Result<Self>;

    /// Execute a contract function (deterministic)
    pub fn execute(&self, inv: &Invocation) -> Result<InvocationResult>;
}
```

## Limitations (Demo Mode)

- **No host syscalls**: All I/O via files
- **Single-threaded**: No async/await support
- **No networking**: HTTP requests not available
- **State size**: Limited by JSON serialization overhead

## Production Upgrade Path

To convert to production mode:

1. **Add C syscall bridge** (`src/bridge.c`):
   ```c
   int near_storage_write(const char* key, const uint8_t* value) {
       return syscall(400, key, value);
   }
   ```

2. **Compile bridge to WASM**:
   ```bash
   wasi-sdk-clang bridge.c -o bridge.wasm
   ```

3. **Link with QuickJS**: Combine bridge + QuickJS into single module

4. **Replace file shim**: Loader calls C bridge → host syscalls

## Performance

- Cold start: ~5-10ms (includes WASM compilation)
- Execution: ~0.1-1ms per function call (depends on complexity)
- State persistence: JSON serialization (deterministic, no binary formats)

## Integration with OutLayer

### ContractSimulator Integration

Add to `browser-worker/src/contract-simulator.js`:

```javascript
async executeQuickJS(jsSource, methodName, args = {}, context = {}) {
  const config = {
    max_wall_time: Duration::from_millis(10000),
    max_fuel: 100_000_000,
  };

  const executor = QuickJsExecutor.new(this.quickjsWasm, config);

  const invocation = {
    contract_source: jsSource,
    function: methodName,
    args: args,
    prior_state_json: this.loadPriorState(),
  };

  const result = executor.execute(invocation);
  this.savePriorState(result.new_state_json);

  return {
    result: result.result,
    gasUsed: result.fuel_consumed || 0,
    mode: 'quickjs'
  };
}
```

## License

MIT

## Credits

- QuickJS WASM runtime: [second-state/quickjs-wasi](https://github.com/second-state/quickjs-wasi)
- Wasmtime engine: [bytecodealliance/wasmtime](https://github.com/bytecodealliance/wasmtime)
- NEAR OutLayer Team
