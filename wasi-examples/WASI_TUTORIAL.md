# WASI Development Tutorial for NEAR OutLayer

This guide explains how to create WASM modules that work with NEAR OutLayer platform.

## Table of Contents

1. [Overview](#overview)
2. [WASI Preview 1 vs Preview 2](#wasi-preview-1-vs-preview-2)
3. [Quick Start: WASI P1](#quick-start-wasi-p1)
4. [Quick Start: WASI P2](#quick-start-wasi-p2)
5. [Input/Output Format](#inputoutput-format)
6. [Important Requirements](#important-requirements)
7. [Testing Your Module](#testing-your-module)
8. [Common Pitfalls](#common-pitfalls)
9. [Examples](#examples)

## Overview

NEAR OutLayer executes WASM modules off-chain using wasmtime runtime. Your code runs in a sandboxed environment with:
- **Stdin** for input data (JSON)
- **Stdout** for output data (JSON)
- **WASI** for system interfaces (random, time, etc.)
- **Fuel metering** for instruction counting
- **Resource limits** (memory, time, instructions)

## WASI Preview 1 vs Preview 2

### WASI Preview 1 (P1)
- **Target**: `wasm32-wasip1` or `wasm32-wasi`
- **Format**: Binary with `main()` function
- **Use case**: Simple computations, random numbers, basic I/O
- **Features**: Core WASI functions (random, stdio, environment)
- **Size**: Smaller binaries (~100-200KB)
- **Example**: [random-ark](./random-ark/)

### WASI Preview 2 (P2)
- **Target**: `wasm32-wasip2`
- **Format**: Component model with typed interfaces
- **Use case**: HTTP requests, complex I/O, modern features
- **Features**: HTTP client, advanced filesystem, sockets
- **Size**: Larger binaries (~500KB-1MB)
- **Example**: [ai-ark](./ai-ark/)

### Which to Choose?

| Feature | WASI P1 | WASI P2 |
|---------|---------|---------|
| HTTP requests | ❌ | ✅ |
| JSON processing | ✅ | ✅ |
| Random numbers | ✅ | ✅ |
| File I/O | ⚠️ Limited | ✅ Full |
| Binary size | 🟢 Small | 🟡 Larger |
| Compilation speed | 🟢 Fast | 🟡 Slower |
| Stability | 🟢 Stable | 🟡 Newer |

**Rule of thumb**: Use P1 unless you need HTTP or advanced I/O.

## Quick Start: WASI P1

### 1. Create Binary Project

```bash
cargo new my-wasi-app
cd my-wasi-app
```

### 2. Configure Cargo.toml

```toml
[package]
name = "my-wasi-app"
version = "0.1.0"
edition = "2021"

# IMPORTANT: Must be a binary, not a library
[[bin]]
name = "my-wasi-app"
path = "src/main.rs"

[dependencies]
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"

[profile.release]
opt-level = "z"  # Optimize for size
lto = true       # Link-time optimization
strip = true     # Strip debug symbols
```

### 3. Write Code (src/main.rs)

```rust
use serde::{Deserialize, Serialize};
use std::io::{self, Read, Write};

#[derive(Deserialize)]
struct Input {
    name: String,
}

#[derive(Serialize)]
struct Output {
    greeting: String,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Read input from stdin
    let mut input_string = String::new();
    io::stdin().read_to_string(&mut input_string)?;

    // Parse JSON input
    let input: Input = serde_json::from_str(&input_string)?;

    // Process
    let output = Output {
        greeting: format!("Hello, {}!", input.name),
    };

    // Write JSON output to stdout
    let json = serde_json::to_string(&output)?;
    print!("{}", json);
    io::stdout().flush()?;

    Ok(())
}
```

### 4. Build

```bash
# Add target
rustup target add wasm32-wasip1

# Build
cargo build --target wasm32-wasip1 --release

# Output: target/wasm32-wasip1/release/my-wasi-app.wasm
```

### 5. Test Locally

```bash
# Test with wasmtime
echo '{"name":"World"}' | wasmtime target/wasm32-wasip1/release/my-wasi-app.wasm

# Expected: {"greeting":"Hello, World!"}
```

## Quick Start: WASI P2

### 1. Create Component Project

```bash
cargo new my-http-app
cd my-http-app
```

### 2. Configure Cargo.toml

```toml
[package]
name = "my-http-app"
version = "0.1.0"
edition = "2021"

[[bin]]
name = "my-http-app"
path = "src/main.rs"

[dependencies]
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
wasi-http-client = "0.2"  # For HTTP requests

[profile.release]
opt-level = "z"
lto = true
strip = true
```

### 3. Write Code (src/main.rs)

```rust
use serde::{Deserialize, Serialize};
use std::io::{self, Read, Write};
use wasi_http_client::{Client, Request, Method};

#[derive(Deserialize)]
struct Input {
    url: String,
}

#[derive(Serialize)]
struct Output {
    status: u16,
    body: String,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Read input
    let mut input_string = String::new();
    io::stdin().read_to_string(&mut input_string)?;
    let input: Input = serde_json::from_str(&input_string)?;

    // Make HTTP request
    let client = Client::new();
    let request = Request::new(Method::Get, &input.url);
    let response = client.send(request)?;

    // Process response
    let output = Output {
        status: response.status(),
        body: String::from_utf8_lossy(response.body()).to_string(),
    };

    // Write output
    let json = serde_json::to_string(&output)?;
    print!("{}", json);
    io::stdout().flush()?;

    Ok(())
}
```

### 4. Build

```bash
# Add target
rustup target add wasm32-wasip2

# Build
cargo build --target wasm32-wasip2 --release

# Output: target/wasm32-wasip2/release/my-http-app.wasm
```

### 5. Test Locally

```bash
# Test with wasmtime
echo '{"url":"https://api.example.com/data"}' | wasmtime target/wasm32-wasip2/release/my-http-app.wasm
```

## Input/Output Format

### CRITICAL REQUIREMENTS

1. **Input**: Always read from `stdin` (not command-line arguments)
2. **Output**: Always write to `stdout` (not stderr)
3. **Format**: JSON only (UTF-8 encoded)
4. **Size limit**: Output must be ≤900 bytes (NEAR Protocol limit)
5. **No buffering**: Call `stdout().flush()` after writing

### Example Pattern

```rust
use std::io::{self, Read, Write};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // ✅ CORRECT: Read from stdin
    let mut input = String::new();
    io::stdin().read_to_string(&mut input)?;

    // Process...
    let output = process(&input)?;

    // ✅ CORRECT: Write to stdout and flush
    print!("{}", output);
    io::stdout().flush()?;

    Ok(())
}

// ❌ WRONG: Reading from args
fn main() {
    let args: Vec<String> = std::env::args().collect(); // Won't work!
}

// ❌ WRONG: Writing to stderr
fn main() {
    eprintln!("result"); // Won't be captured!
}
```

## Important Requirements

### 1. Binary Format (Not Library)

```toml
# ✅ CORRECT
[[bin]]
name = "my-app"
path = "src/main.rs"

# ❌ WRONG
[lib]
crate-type = ["cdylib"]
```

### 2. Entry Point

```rust
// ✅ CORRECT: main() function
fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Your code
    Ok(())
}

// ❌ WRONG: Custom exports
#[no_mangle]
pub extern "C" fn execute() { } // Old pattern, don't use
```

### 3. Error Handling

```rust
// ✅ CORRECT: Return errors from main
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let data = serde_json::from_str(&input)?; // Propagates error
    Ok(())
}

// ❌ WRONG: Panics crash the worker
fn main() {
    let data = serde_json::from_str(&input).unwrap(); // Don't use unwrap()!
}
```

### 4. Output Size

```rust
// ✅ CORRECT: Truncate large outputs
let mut output = generate_large_output();
if output.len() > 800 {
    output.truncate(800);
    output.push_str("...");
}
print!("{}", output);
```

### 5. Dependencies

```rust
// ✅ Safe dependencies
serde, serde_json          // JSON processing
rand                       // Random numbers (P1 & P2)
wasi-http-client          // HTTP requests (P2 only)

// ⚠️ Avoid these
tokio                      // Async runtime (not needed in WASM)
reqwest                    // Use wasi-http-client instead
std::thread               // Threading not supported
```

## Testing Your Module

### Option 1: Quick Test with wasmtime

```bash
# Install wasmtime
curl https://wasmtime.dev/install.sh -sSf | bash

# Test your WASM
echo '{"test":"data"}' | wasmtime your-app.wasm
```

### Option 2: Use Universal Test Runner

See [WASI_TEST_RUNNER.md](./WASI_TEST_RUNNER.md) for a comprehensive test tool that validates:
- ✅ Binary format correctness
- ✅ Fuel metering
- ✅ Input/output handling
- ✅ Resource limits
- ✅ Compatibility with NEAR OutLayer

## Common Pitfalls

### 1. "entry symbol not defined: _initialize"

**Problem**: Using `[lib]` with `crate-type = ["cdylib"]`

**Solution**: Use `[[bin]]` format (see Quick Start)

### 2. Empty output

**Problem**: Forgot to flush stdout

**Solution**:
```rust
print!("{}", output);
io::stdout().flush()?; // ← Add this!
```

### 3. "Failed to instantiate WASM module"

**Problem**: Wrong target or missing `main()`

**Solution**:
- Use `wasm32-wasip1` or `wasm32-wasip2` target
- Ensure you have `fn main()` function

### 4. Output truncated in NEAR explorer

**Problem**: Output > 900 bytes

**Solution**: Truncate before returning:
```rust
if output.len() > 800 {
    output.truncate(800);
}
```

### 5. "use of unstable library feature" when building

**Problem**: Test dependencies in WASM build

**Solution**: Use optional dependencies with features (see ai-ark example)

### 6. HTTP requests fail

**Problem**: Using WASI P1 instead of P2

**Solution**: Use `wasm32-wasip2` target and `wasi-http-client` crate

## Examples

### Complete Working Examples

1. **[random-ark](./random-ark/)** - WASI P1
   - Random number generation
   - JSON input/output
   - ~111KB binary

2. **[ai-ark](./ai-ark/)** - WASI P2
   - HTTP POST requests
   - OpenAI API integration
   - Component model

### Minimal Example (Copy-Paste Ready)

```rust
use serde::{Deserialize, Serialize};
use std::io::{self, Read, Write};

#[derive(Deserialize)]
struct Input {
    value: i32,
}

#[derive(Serialize)]
struct Output {
    result: i32,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut input = String::new();
    io::stdin().read_to_string(&mut input)?;

    let data: Input = serde_json::from_str(&input)?;

    let output = Output {
        result: data.value * 2,
    };

    print!("{}", serde_json::to_string(&output)?);
    io::stdout().flush()?;

    Ok(())
}
```

**Cargo.toml**:
```toml
[package]
name = "example"
version = "0.1.0"
edition = "2021"

[[bin]]
name = "example"
path = "src/main.rs"

[dependencies]
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"

[profile.release]
opt-level = "z"
lto = true
strip = true
```

**Build & Test**:
```bash
cargo build --target wasm32-wasip1 --release
echo '{"value":21}' | wasmtime target/wasm32-wasip1/release/example.wasm
# Output: {"result":42}
```

## Deployment to NEAR OutLayer

1. **Push code to GitHub**
2. **Call contract**:

```bash
near call outlayer.testnet request_execution '{
  "code_source": {
    "repo": "https://github.com/username/repo",
    "commit": "main",
    "build_target": "wasm32-wasip1"
  },
  "resource_limits": {
    "max_instructions": 10000000,
    "max_memory_mb": 128,
    "max_execution_seconds": 60
  },
  "input_data": "{\"value\":21}"
}' --accountId your.testnet --deposit 0.1
```

3. **Check result** in NEAR Explorer

## Need Help?

- Check [examples](./random-ark/) for working code
- Use [test runner](./WASI_TEST_RUNNER.md) to validate your module
- Review [common pitfalls](#common-pitfalls) section
- Test locally with wasmtime before deploying

---

**Last updated**: 2025-10-15
**Compatible with**: wasmtime 28+, NEAR OutLayer MVP
