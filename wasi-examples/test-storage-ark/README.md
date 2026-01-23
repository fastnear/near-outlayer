# test-storage-ark

Test WASM for OutLayer persistent storage host functions.

This WASI Preview 2 component demonstrates and tests the `near:storage@0.1.0` host functions provided by the OutLayer worker for encrypted persistent storage.

## Features

- **Persistent Storage**: Data persists across executions
- **Encrypted**: All data is encrypted before storage using keystore TEE
- **Per-User Isolation**: Each user's data is isolated using different encryption keys
- **Worker-Private Storage**: Special storage only accessible by WASM, not users
- **Public Storage**: Unencrypted storage readable by other projects (e.g., oracle price feeds)
- **Conditional Writes**: Compare-and-swap, set-if-absent, atomic increment/decrement
- **Cross-Project Reads**: Read public data from other projects by UUID
- **HTTP Verification**: Verify public storage via coordinator HTTP API
- **Version Migration**: Read data from previous WASM versions

## Build

```bash
# Add WASI P2 target
rustup target add wasm32-wasip2

# Build
cargo build --target wasm32-wasip2 --release
```

Output: `target/wasm32-wasip2/release/test-storage-ark.wasm`

## Environment Variables

Set by OutLayer runtime:

| Variable | Example | Description |
|----------|---------|-------------|
| `OUTLAYER_PROJECT_UUID` | `p0000000000000001` | Project UUID for cross-project reads |
| `OUTLAYER_PROJECT_ID` | `owner/name` | Project identifier |
| `OUTLAYER_PROJECT_OWNER` | `alice.near` | Project owner account |
| `OUTLAYER_PROJECT_NAME` | `my-project` | Project name |

## Input Format

```json
{
    "command": "set",
    "key": "my-key",
    "value": "my-value",
    "prefix": "",           // for list command
    "expected": "",         // for set_if_equals
    "delta": 0,             // for increment/decrement
    "project": "",          // for get_public_cross (project_uuid like "p0000000000000001")
    "coordinator_url": ""   // for verify_public_http/test_public_storage
}
```

## Commands

### Basic Storage

| Command | Description | Required Fields |
|---------|-------------|-----------------|
| `set` | Store a key-value pair | `key`, `value` |
| `get` | Retrieve value by key | `key` |
| `delete` | Delete a key | `key` |
| `has` | Check if key exists | `key` |
| `list` | List all keys | `prefix` (optional) |
| `set_worker` | Store worker-private data (encrypted) | `key`, `value` |
| `get_worker` | Get worker-private data | `key` |
| `clear_all` | Clear all storage | - |

### Conditional Writes

| Command | Description | Required Fields |
|---------|-------------|-----------------|
| `set_if_absent` | Store only if key doesn't exist | `key`, `value` |
| `set_if_equals` | Compare-and-swap | `key`, `expected`, `value` |
| `increment` | Atomic increment (creates if missing) | `key`, `delta` |
| `decrement` | Atomic decrement (creates if missing) | `key`, `delta` |

### Public Storage (Cross-Project Readable)

| Command | Description | Required Fields |
|---------|-------------|-----------------|
| `set_public` | Store unencrypted data | `key`, `value` |
| `get_public_cross` | Read public data from another project | `key`, `project` (project_uuid) |
| `verify_public_http` | Verify public storage via HTTP API | `key`, `coordinator_url` |

### Tests

| Command | Description | Required Fields |
|---------|-------------|-----------------|
| `test_all` | Run all storage tests | - |
| `test_public_storage` | Run public storage tests | `coordinator_url` (optional) |

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

### Conditional Write: Set if absent

```json
{"command": "set_if_absent", "key": "unique-id", "value": "first-value"}
```

Response:
```json
{"success": true, "command": "set_if_absent", "inserted": true, "value": "Inserted 11 bytes at key 'unique-id'"}
```

### Atomic Counter

```json
{"command": "increment", "key": "visits", "delta": 1}
```

Response:
```json
{"success": true, "command": "increment", "numeric_value": 42, "value": "Key 'visits' incremented by 1, new value: 42"}
```

### Public Storage (Cross-Project)

```json
{"command": "set_public", "key": "oracle:ETH", "value": "{\"price\":\"3500.00\"}"}
```

Response:
```json
{"success": true, "command": "set_public", "value": "Stored 21 bytes as PUBLIC at key 'oracle:ETH'"}
```

Read from another project:
```json
{"command": "get_public_cross", "key": "oracle:ETH", "project": "p0000000000000001"}
```

Verify via HTTP:
```bash
# JSON format (default) - base64 encoded value
curl "http://localhost:8080/public/storage/get?project_uuid=p0000000000000001&key=oracle:ETH"
# {"exists":true,"value":"eyJwcmljZSI6IjM1MDAuMDAifQ=="}

# Raw format - returns raw bytes
curl "http://localhost:8080/public/storage/get?project_uuid=p0000000000000001&key=oracle:ETH&format=raw"
# {"price":"3500.00"}
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
  "value": "28/28 tests passed",
  "tests": {
    "total": 28,
    "passed": 28,
    "failed": 0,
    "results": [...]
  }
}
```

## Storage API (WIT)

The storage interface is defined in `near:storage@0.1.0`:

```wit
interface api {
    // Basic operations
    set: func(key: string, value: list<u8>) -> string;
    get: func(key: string) -> tuple<list<u8>, string>;
    has: func(key: string) -> bool;
    delete: func(key: string) -> bool;
    list-keys: func(prefix: string) -> tuple<string, string>;

    // Conditional writes
    set-if-absent: func(key: string, value: list<u8>) -> tuple<bool, string>;
    set-if-equals: func(key: string, expected: list<u8>, new-value: list<u8>) -> tuple<bool, list<u8>, string>;
    increment: func(key: string, delta: s64) -> tuple<s64, string>;
    decrement: func(key: string, delta: s64) -> tuple<s64, string>;

    // Worker storage (with public option for cross-project reads)
    set-worker: func(key: string, value: list<u8>, is-encrypted: option<bool>) -> string;
    get-worker: func(key: string, project-uuid: option<string>) -> tuple<list<u8>, string>;

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
// Database key: project_uuid:@worker:total_count = "100" (encrypted)

// Any other user reads:
storage::get_worker("total_count")  // → "100" (same data)
```

**Key point:** Worker storage is shared, but users cannot directly access it. Only WASM code can call `get_worker`/`set_worker`. Users interact with worker data only through WASM logic (e.g., calling a method that returns aggregated stats).

### Public Storage (cross-project readable)

Public storage is unencrypted worker storage that can be read by other projects. Use case: oracle price feeds, shared configuration.

```rust
// Store public data (is_encrypted = false)
storage::set_worker_with_options("oracle:ETH", price_json.as_bytes(), Some(false))

// Read from current project
storage::get_worker("oracle:ETH")

// Read from another project by UUID
storage::get_worker_from_project("oracle:ETH", Some("p0000000000000001"))
```

**External HTTP API:**
```bash
# JSON format (default)
curl "http://coordinator/public/storage/get?project_uuid=p0000000000000001&key=oracle:ETH"
# {"exists":true,"value":"<base64-encoded-value>"}

# Raw format - returns raw bytes
curl "http://coordinator/public/storage/get?...&format=raw"
```

**Key points:**
- `is_encrypted=false` makes data readable by other projects
- Other projects read via `project_uuid` (e.g., `p0000000000000001`)
- External clients read via HTTP endpoint (returns base64-encoded value)
- Encrypted (default) worker data is NOT accessible cross-project

### Version Migration

The `wasm_hash` is stored with each record but NOT included in the unique key. This means:
- New WASM versions automatically read data written by old versions
- Use `storage::get_by_version("key", "old_wasm_hash")` to explicitly read old version's data

## Use Cases

1. **User Preferences**: Store user settings that persist across executions
2. **Counters/State**: Maintain state between invocations (use `increment`/`decrement` for thread-safe counters)
3. **Caching**: Cache expensive computation results
4. **Session Data**: Store session-specific data
5. **Worker State**: Private data for WASM logic (not user-accessible)
6. **Oracle Price Feeds**: Public storage for sharing data across projects
7. **Distributed Locks**: Use `set_if_absent` for implementing locks
8. **Optimistic Updates**: Use `set_if_equals` for compare-and-swap operations
