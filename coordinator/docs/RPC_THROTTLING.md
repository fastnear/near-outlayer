# RPC Throttling - Configuration and Usage Guide

**Version**: 1.0.0
**Last Updated**: November 5, 2025
**Status**: Phase 1 Complete

---

## Overview

The coordinator implements intelligent rate limiting for all NEAR RPC requests and external API calls. This protects infrastructure from burst traffic, prevents 429 cascades, and provides a fair experience for all users.

### Key Features

- **Token-bucket algorithm**: Smooth rate limiting with burst capacity
- **Concurrency control**: Limits simultaneous in-flight requests
- **Per-route bucketing**: Different limits for different endpoints
- **API-key aware**: Higher limits for authenticated users (20 rps vs 5 rps)
- **Automatic retry**: Handles upstream 429s with exponential backoff
- **Real-time metrics**: Monitor throttle state via `/throttle/metrics` endpoint

---

## Architecture

```
Browser/Worker
  ↓ POST /near-rpc (JSON-RPC request)
Coordinator Throttle Middleware
  ├─ Check token bucket (5 or 20 tokens/second)
  ├─ Check concurrency (4 or 8 in-flight)
  ├─ Allow → Proxy to NEAR RPC → Return response
  └─ Deny → 429 with Retry-After header
```

### Token Bucket Algorithm

Each bucket has:
- **RPS (Requests Per Second)**: Token refill rate
- **Burst**: Maximum token capacity (allows short bursts)
- **Concurrent**: Maximum simultaneous in-flight requests

Example with 5 rps, burst 10, concurrent 4:
- Starts with 10 tokens
- Refills at 5 tokens/second
- Can have at most 4 requests executing simultaneously
- After burst, sustained rate is 5 req/s

---

## Rate Limit Profiles

### Anonymous Users (Default)

No API key required. Suitable for free-tier usage.

```rust
RateLimitProfile {
    rps: 5,           // 5 requests per second sustained
    burst: 10,        // 10 requests in initial burst
    concurrent: 4,    // 4 simultaneous in-flight requests
}
```

**Use case**: Public browser clients, development, testing

### API-Key Users

Provide `Authorization: Bearer YOUR_API_KEY` header.

```rust
RateLimitProfile {
    rps: 20,          // 20 requests per second sustained
    burst: 40,        // 40 requests in initial burst
    concurrent: 8,    // 8 simultaneous in-flight requests
}
```

**Use case**: Production workers, high-volume applications

---

## Endpoints

### 1. NEAR RPC Proxy

**Endpoint**: `POST /near-rpc`

Proxies NEAR JSON-RPC requests with throttling.

**Request**:
```json
{
  "jsonrpc": "2.0",
  "id": "dontcare",
  "method": "query",
  "params": {
    "request_type": "view_account",
    "finality": "final",
    "account_id": "example.near"
  }
}
```

**Response** (success):
```json
{
  "jsonrpc": "2.0",
  "id": "dontcare",
  "result": {
    "amount": "100000000000000000000000000",
    "locked": "0",
    ...
  }
}
```

**Response** (rate limited):
```http
HTTP/1.1 429 Too Many Requests
Retry-After: 5
X-RateLimit-Limit: 5

Rate limit exceeded: Rate limit exceeded (5rps)
```

**JavaScript Example**:
```javascript
// Using RPCClient (recommended)
const client = new RPCClient({
  coordinatorUrl: 'http://localhost:8080',
  apiKey: 'your-api-key',  // Optional
});

const status = await client.getStatus();

// Direct fetch
const response = await fetch('http://localhost:8080/near-rpc', {
  method: 'POST',
  headers: {
    'Content-Type': 'application/json',
    'Authorization': 'Bearer YOUR_API_KEY',  // Optional
  },
  body: JSON.stringify({
    jsonrpc: '2.0',
    id: '1',
    method: 'status',
    params: [],
  }),
});
```

### 2. External API Proxy

**Endpoint**: `POST /external/{service}`

Proxies external third-party API calls (OpenAI, Anthropic, etc.) with same throttling.

**Allowed services**:
- `openai` - OpenAI API
- `anthropic` - Anthropic API
- `coingecko` - CoinGecko API
- `etherscan` - Etherscan API

**Request**:
```json
{
  "method": "POST",
  "url": "https://api.openai.com/v1/completions",
  "headers": {
    "Authorization": "Bearer sk-..."
  },
  "body": {
    "model": "gpt-4",
    "prompt": "Hello"
  }
}
```

**Response**:
```json
{
  "status": 200,
  "headers": {
    "content-type": "application/json",
    ...
  },
  "body": {
    "id": "cmpl-...",
    "choices": [...]
  }
}
```

### 3. Throttle Metrics

**Endpoint**: `GET /throttle/metrics`

Returns current state of all rate limit buckets.

**Response**:
```json
{
  "/near-rpc:anon": [5, 10, 2],   // [rps, burst, in-flight]
  "/near-rpc:keyed": [20, 40, 1],
  "/external/openai:anon": [5, 10, 0]
}
```

**JavaScript Example**:
```javascript
const metrics = await fetch('http://localhost:8080/throttle/metrics')
  .then(r => r.json());

console.log('Anonymous NEAR RPC:', metrics['/near-rpc:anon']);
// Output: [5, 10, 2] = 5 rps, 10 burst, 2 in-flight
```

---

## Configuration

### Environment Variables

Add to `coordinator/.env`:

```bash
# Throttle profiles (optional - defaults shown)
THROTTLE_ANON_RPS=5
THROTTLE_ANON_BURST=10
THROTTLE_ANON_CONCURRENT=4

THROTTLE_KEYED_RPS=20
THROTTLE_KEYED_BURST=40
THROTTLE_KEYED_CONCURRENT=8

# Upstream NEAR RPC
NEAR_RPC_URL=https://rpc.testnet.near.org
```

### Customizing Profiles (Code)

Edit `coordinator/src/main.rs`:

```rust
// Initialize throttle manager
let anon_profile = RateLimitProfile {
    rps: 10,        // Increase to 10 rps for anon users
    burst: 20,
    concurrent: 8,
};

let keyed_profile = RateLimitProfile {
    rps: 50,        // Increase to 50 rps for API-key users
    burst: 100,
    concurrent: 16,
};

let throttle_manager = Arc::new(ThrottleManager::new(anon_profile, keyed_profile));
```

---

## Client Integration

### Browser (JavaScript)

Use the provided `RPCClient` class:

```javascript
// 1. Load RPC client
<script src="src/rpc-client.js"></script>

// 2. Initialize
const rpc = new RPCClient({
  coordinatorUrl: 'http://localhost:8080',
  network: 'testnet',
  verbose: true,
  apiKey: 'your-key',  // Optional
});

// 3. Make calls (auto-retry on 429)
const status = await rpc.getStatus();
const account = await rpc.viewAccount('example.near');
const result = await rpc.viewCall('counter.near', 'get_count', {});

// 4. Check statistics
const stats = rpc.getStats();
console.log(`Success rate: ${stats.successRate}`);
console.log(`Avg retry delay: ${stats.avgRetryDelay}`);
```

### Rust Worker

Update `worker/src/api_client.rs`:

```rust
pub async fn call_near_rpc(
    &self,
    method: &str,
    params: serde_json::Value,
) -> Result<serde_json::Value> {
    let request = serde_json::json!({
        "jsonrpc": "2.0",
        "id": "dontcare",
        "method": method,
        "params": params,
    });

    let response = self.client
        .post(format!("{}/near-rpc", self.base_url))
        .header("Authorization", format!("Bearer {}", self.api_key))
        .json(&request)
        .send()
        .await?;

    // Handle 429
    if response.status() == 429 {
        let retry_after = response
            .headers()
            .get("retry-after")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(5);

        tokio::time::sleep(Duration::from_secs(retry_after)).await;

        // Retry once
        return self.call_near_rpc(method, params).await;
    }

    let rpc_response: serde_json::Value = response.json().await?;

    Ok(rpc_response["result"].clone())
}
```

---

## Monitoring and Tuning

### Real-Time Monitoring

```javascript
// Browser dashboard
setInterval(async () => {
  const metrics = await fetch('http://localhost:8080/throttle/metrics')
    .then(r => r.json());

  Object.entries(metrics).forEach(([bucket, [rps, burst, inFlight]]) => {
    console.log(`${bucket}: ${inFlight}/${burst} tokens, ${rps} rps`);
  });
}, 5000);  // Every 5 seconds
```

### Key Metrics to Watch

1. **In-flight count approaching concurrent limit**: Increase concurrent capacity
2. **High retry rate**: Increase RPS or burst capacity
3. **Long average retry delay**: Tune burst capacity or add more coordinator instances

### Tuning Guidelines

**Scenario: Dashboard with 100 active users**

Each user makes ~2 requests/second → 200 total rps needed

**Solution**:
- Increase `keyed_profile.rps` to 200+
- Increase `keyed_profile.concurrent` to 50+
- Provide API keys to dashboard

**Scenario: Worker hitting limits during execution**

Worker makes 10 RPC calls per job, jobs every 5 seconds

**Solution**:
- Ensure worker uses API key (20 rps)
- Batch RPC calls where possible
- Increase `keyed_profile.rps` if still limiting

---

## Troubleshooting

### Problem: "Rate limit exceeded" errors

**Check**:
1. Are you using an API key? (5 rps vs 20 rps)
2. Is your burst traffic sustainable? (Can't exceed RPS long-term)
3. Are multiple clients sharing the same bucket?

**Solution**:
- Add `Authorization: Bearer YOUR_KEY` header
- Implement request batching
- Increase throttle profiles in config

### Problem: Requests timing out

**Check**:
1. Is concurrency limit reached? (Too many in-flight)
2. Is upstream NEAR RPC slow?

**Solution**:
- Increase `concurrent` limit
- Add timeout to client requests
- Check coordinator logs for upstream errors

### Problem: Inconsistent rate limiting

**Possible causes**:
- Multiple coordinator instances (each has separate buckets)
- Clock skew between client and server
- Burst traffic exceeding capacity

**Solution**:
- Use shared Redis for distributed rate limiting (future enhancement)
- Sync clocks via NTP
- Increase burst capacity

---

## Testing

### Manual Testing

```bash
# 1. Start coordinator
cd coordinator
cargo run

# 2. Open test page
open browser-worker/test.html

# 3. Test throttling
Click: 📡 Initialize RPC Client
Click: 💥 Burst 10 Calls (Test Throttle)
Click: 📊 Show RPC Statistics
```

### Automated Testing

```bash
# Test rate limiting with curl
for i in {1..20}; do
  curl -X POST http://localhost:8080/near-rpc \
    -H "Content-Type: application/json" \
    -d '{"jsonrpc":"2.0","id":"'$i'","method":"status","params":[]}' \
    &
done

# Expected: First 10 succeed quickly, next 10 delayed or 429
```

---

## Future Enhancements

1. **Distributed rate limiting**: Shared Redis buckets across coordinator instances
2. **Per-user buckets**: Separate limits for each account_id
3. **Dynamic pricing**: Adjust RPS based on payment tier
4. **Circuit breaker**: Auto-disable proxying if upstream consistently fails
5. **Request prioritization**: Queue management with priority levels

---

## Summary

The RPC throttling system provides:
- ✅ Infrastructure protection from burst traffic
- ✅ Fair resource allocation between users
- ✅ Automatic retry with intelligent backoff
- ✅ Real-time monitoring and metrics
- ✅ Simple client integration (drop-in replacement for direct RPC calls)

For support or feature requests, file an issue at: https://github.com/near/outlayer/issues
