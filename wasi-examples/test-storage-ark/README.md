# test-storage-ark

Test WASM for OutLayer persistent storage host functions.

This WASI Preview 2 component demonstrates and tests the `near:rpc/storage@0.1.0` host functions provided by the OutLayer worker for encrypted persistent storage.

## Features

- **Persistent Storage**: Data persists across executions
- **Encrypted**: All data is encrypted before storage using keystore TEE
- **Per-User Isolation**: Each user's data is isolated using different encryption keys
- **Worker-Private Storage**: Special storage only accessible by WASM, not users
- **Version Migration**: Read data from previous WASM versions

## Build

```bash
# Add WASI P2 target
rustup target add wasm32-wasip2

# Build
cargo build --target wasm32-wasip2 --release
```

Output: `target/wasm32-wasip2/release/test-storage-ark.wasm`

## Input Format

```json
{
    "command": "set",
    "key": "my-key",
    "value": "my-value"
}
```

## Commands

| Command | Description | Required Fields |
|---------|-------------|-----------------|
| `set` | Store a key-value pair | `key`, `value` |
| `get` | Retrieve value by key | `key` |
| `delete` | Delete a key | `key` |
| `has` | Check if key exists | `key` |
| `list` | List all keys | `prefix` (optional) |
| `set_worker` | Store worker-private data | `key`, `value` |
| `get_worker` | Get worker-private data | `key` |
| `clear_all` | Clear all storage | - |
| `test_all` | Run all storage tests | - |

## Examples

### Store a value

```json
{"command": "set", "key": "counter", "value": "42"}
```

Response:
```json
{"success": true, "command": "set", "value": "Stored 2 bytes at key 'counter'"}
```

### Get a value

```json
{"command": "get", "key": "counter"}
```

Response:
```json
{"success": true, "command": "get", "value": "42", "exists": true}
```

### List keys with prefix

```json
{"command": "list", "prefix": "user:"}
```

Response:
```json
{"success": true, "command": "list", "keys": ["user:alice", "user:bob"]}
```

### Run all tests

```json
{"command": "test_all"}
```

Response:
```json
{
  "success": true,
  "command": "test_all",
  "value": "13/13 tests passed",
  "tests": {
    "total": 13,
    "passed": 13,
    "failed": 0,
    "results": [...]
  }
}
```

## Storage API (WIT)

The storage interface is defined in `wit/world.wit`:

```wit
interface storage {
    // Basic operations
    set: func(key: string, value: list<u8>) -> string;
    get: func(key: string) -> tuple<list<u8>, string>;
    has: func(key: string) -> bool;
    delete: func(key: string) -> bool;
    list-keys: func(prefix: string) -> tuple<string, string>;

    // Worker-private storage
    set-worker: func(key: string, value: list<u8>) -> string;
    get-worker: func(key: string) -> tuple<list<u8>, string>;

    // Version migration
    get-by-version: func(key: string, wasm-hash: string) -> tuple<list<u8>, string>;

    // Cleanup
    clear-all: func() -> string;
    clear-version: func(wasm-hash: string) -> string;
}
```

## Architecture

```
WASM (this module)
    │ WIT host function calls
    ▼
OutLayer Worker (host_functions.rs)
    │ calls StorageClient
    ▼
StorageClient → Keystore (encrypt/decrypt)
    │ encrypted data
    ▼
Coordinator API (/storage/*)
    │
    ▼
PostgreSQL (storage_data table)
```

## Security Notes

- All data is encrypted using keystore TEE before storage
- Encryption key is derived from: `storage:{project_uuid}:{account_id}`
- Worker-private storage uses `@worker` as account_id
- Data is isolated per user - each user can only read/write their own data
- User isolation is automatic at protocol level: `alice.near` cannot access `bob.near`'s data

## Storage Key Structure

Understanding how storage keys work is essential for using OutLayer storage correctly.

### User Storage (isolated per account)

When a user calls `storage::set("balance", "100")`, the actual database key includes the account ID:

```
// alice.near calls execution:
storage::set("balance", "100")
// Database key: project_uuid:alice.near:balance = "100"

// bob.near calls execution:
storage::set("balance", "200")
// Database key: project_uuid:bob.near:balance = "200"

// alice.near reads:
storage::get("balance")  // → "100" (her data)

// bob.near reads:
storage::get("balance")  // → "200" (his data)
```

**Key points:**
- WASM code CANNOT read another user's data
- There is no function like `storage::get_for_account("bob.near", "balance")`
- User data is only accessible when that user triggers the execution

### Worker Storage (shared across all users)

When WASM calls `storage::set_worker("key", value)`, the account is replaced with `@worker`:

```
// Any user calls execution:
storage::set_worker("total_count", "100")
// Database key: project_uuid:@worker:total_count = "100"

// Any other user reads:
storage::get_worker("total_count")  // → "100" (same data)
```

**Key point:** Worker storage is shared, but users cannot directly access it. Only WASM code can call `get_worker`/`set_worker`. Users interact with worker data only through WASM logic (e.g., calling a method that returns aggregated stats).

### Version Migration

The `wasm_hash` is stored with each record but NOT included in the unique key. This means:
- New WASM versions automatically read data written by old versions
- Use `storage::get_by_version("key", "old_wasm_hash")` to explicitly read old version's data

## Use Cases

1. **User Preferences**: Store user settings that persist across executions
2. **Counters/State**: Maintain state between invocations
3. **Caching**: Cache expensive computation results
4. **Session Data**: Store session-specific data
5. **Worker State**: Private data for WASM logic (not user-accessible)
