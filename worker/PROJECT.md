# Worker Project Notes

## NEAR RPC Calls - Use Existing Patterns

**DO NOT** write raw HTTP JSON-RPC requests like this:
```rust
// WRONG - don't do this
let rpc_request = serde_json::json!({
    "jsonrpc": "2.0",
    "id": "dontcare",
    "method": "query",
    "params": {
        "request_type": "call_function",
        "finality": "final",
        "account_id": contract_id,
        "method_name": "get_request",
        "args_base64": args_base64
    }
});
let response = http_client.post(&rpc_url).json(&rpc_request).send().await?;
```

**USE** the `near-jsonrpc-client` crate with proper types:

```rust
use near_jsonrpc_client::{methods, JsonRpcClient};
use near_primitives::types::{AccountId, BlockReference, Finality};
use near_primitives::views::QueryRequest;

// Create client once
let rpc_client = JsonRpcClient::connect(&near_rpc_url);

// Make view call
let request = methods::query::RpcQueryRequest {
    block_reference: BlockReference::Finality(Finality::Final),
    request: QueryRequest::CallFunction {
        account_id: contract_id.clone(),
        method_name: "get_request".to_string(),
        args: serde_json::json!({ "request_id": request_id })
            .to_string()
            .into_bytes()
            .into(),
    },
};

let response = rpc_client.call(request).await?;

// Extract result
let result_bytes = match response.kind {
    near_jsonrpc_primitives::types::query::QueryResponseKind::CallResult(call_result) => {
        call_result.result
    }
    _ => anyhow::bail!("Unexpected response kind"),
};

let data: MyType = serde_json::from_slice(&result_bytes)?;
```

## Existing Examples in Codebase

- `near_client.rs:796-810` - `fetch_project()` view call
- `near_client.rs:828-850` - `fetch_project_version()` view call
- `registration.rs:176-184` - `ViewAccessKey` query
- `event_monitor.rs` - `fetch_input_data_from_contract()` view call

## Key Imports

```rust
// For RPC client
use near_jsonrpc_client::{methods, JsonRpcClient};

// For query types
use near_primitives::types::{AccountId, BlockReference, Finality};
use near_primitives::views::QueryRequest;

// For parsing responses
use near_jsonrpc_primitives::types::query::QueryResponseKind;
```

## Common Patterns

### View Call (read-only)
```rust
QueryRequest::CallFunction {
    account_id,
    method_name: "method_name".to_string(),
    args: json_args.to_string().into_bytes().into(),
}
```

### View Access Key
```rust
QueryRequest::ViewAccessKey {
    account_id,
    public_key,
}
```

### View Account
```rust
QueryRequest::ViewAccount {
    account_id,
}
```
