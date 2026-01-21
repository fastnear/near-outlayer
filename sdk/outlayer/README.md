# OutLayer SDK

Rust SDK for building WASM applications on [OutLayer](https://outlayer.fastnear.com) - verifiable off-chain computation for NEAR.

[![Crates.io](https://img.shields.io/crates/v/outlayer.svg)](https://crates.io/crates/outlayer)
[![Documentation](https://docs.rs/outlayer/badge.svg)](https://docs.rs/outlayer)

## Installation

```toml
[dependencies]
outlayer = "0.1"
```

**Requirements:** WASI Preview 2 (`wasm32-wasip2` target)

```bash
rustup target add wasm32-wasip2
cargo build --target wasm32-wasip2 --release
```

## Quick Start

```rust
use outlayer::{env, storage};

fn main() {
    // Get caller info
    let signer = env::signer_account_id().unwrap_or_default();

    // Read input
    let input = env::input_string().unwrap_or_default();

    // Use persistent storage
    let count = storage::increment("visits", 1).unwrap();

    // Output result
    env::output_json(&serde_json::json!({
        "signer": signer,
        "visits": count,
        "input": input
    })).unwrap();
}
```

## Features

### Environment (`outlayer::env`)

Access execution context and I/O:

```rust
use outlayer::env;

// Get NEAR account info
let signer = env::signer_account_id();           // User who signed tx (alice.near)
let predecessor = env::predecessor_account_id(); // Contract that called OutLayer
let tx_hash = env::transaction_hash();

// Input/Output
let input: MyRequest = env::input_json()?.unwrap();
env::output_json(&response)?;

// Environment variables (including secrets)
let api_key = env::var("OPENAI_API_KEY");
```

**Available environment variables:**
- `NEAR_SENDER_ID` - Account that signed the transaction
- `NEAR_PREDECESSOR_ID` - Contract that called OutLayer
- `NEAR_TRANSACTION_HASH` - Transaction hash
- `USD_PAYMENT` - Attached USD payment (micro-units)
- Custom secrets stored via dashboard

### Storage (`outlayer::storage`)

Encrypted persistent key-value storage:

```rust
use outlayer::storage;

// Basic operations
storage::set("key", b"value")?;
let data = storage::get("key")?;
let exists = storage::has("key");
storage::delete("key");
let keys = storage::list_keys("prefix:")?;

// Convenience methods
storage::set_string("name", "Alice")?;
storage::set_json("config", &my_struct)?;
let config: Config = storage::get_json("config")?.unwrap();

// Atomic operations (concurrent-safe)
storage::increment("counter", 1)?;
storage::decrement("stock", 1)?;
storage::set_if_absent("init", b"done")?;
storage::set_if_equals("balance", &old, &new)?;

// Worker-private storage (shared across all users)
storage::set_worker("global_state", b"data")?;
let state = storage::get_worker("global_state")?;

// Public storage (readable by other projects)
storage::set_worker_with_options("oracle:ETH", &price, Some(false))?;
let price = storage::get_worker_from_project("oracle:ETH", Some("p0000000000000001"))?;
```

**Storage isolation:**
- User storage: Isolated per caller (`alice.near` can't read `bob.near`'s data)
- Worker storage: Shared across all users, only accessible from WASM
- Public storage: Cross-project readable (for oracles, shared configs)

### Version Migration

```rust
// Read data from previous WASM version
let old_data = storage::get_by_version("key", "abc123...")?;

// Clean up old version's data after migration
storage::clear_version("abc123...")?;
```

## Example Project

```toml
# Cargo.toml
[package]
name = "my-outlayer-app"
version = "0.1.0"
edition = "2021"

[[bin]]
name = "my-outlayer-app"
path = "src/main.rs"

[dependencies]
outlayer = "0.1"
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"

[profile.release]
opt-level = "s"
lto = true
strip = true
```

```rust
// src/main.rs
use outlayer::{env, storage};
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
struct Request {
    action: String,
}

#[derive(Serialize)]
struct Response {
    success: bool,
    message: String,
}

fn main() {
    let result = run();
    let response = match result {
        Ok(msg) => Response { success: true, message: msg },
        Err(e) => Response { success: false, message: e.to_string() },
    };
    env::output_json(&response).unwrap();
}

fn run() -> Result<String, Box<dyn std::error::Error>> {
    let signer = env::signer_account_id()
        .ok_or("No signer")?;

    let request: Request = env::input_json()?
        .ok_or("No input")?;

    match request.action.as_str() {
        "increment" => {
            let count = storage::increment(&format!("count:{}", signer), 1)?;
            Ok(format!("Count: {}", count))
        }
        "get" => {
            let count = storage::get_json::<i64>(&format!("count:{}", signer))?
                .unwrap_or(0);
            Ok(format!("Count: {}", count))
        }
        _ => Err("Unknown action".into())
    }
}
```

Build and test:

```bash
cargo build --target wasm32-wasip2 --release
echo '{"action":"increment"}' | wasmtime target/wasm32-wasip2/release/my-outlayer-app.wasm
```

## Documentation

- [OutLayer Docs](https://outlayer.fastnear.com/docs) - Full documentation
- [Storage Guide](https://outlayer.fastnear.com/docs/storage) - Persistent storage
- [WASI Tutorial](https://github.com/fastnear/near-outlayer/blob/main/wasi-examples/WASI_TUTORIAL.md) - Building WASM apps
- [Examples](https://github.com/fastnear/near-outlayer/tree/main/wasi-examples) - Working examples

## License

MIT OR Apache-2.0
