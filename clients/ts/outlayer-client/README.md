# NEAR OutLayer TypeScript Client

Type-safe, production-ready client for NEAR OutLayer with full support for NEAR-signed authentication and idempotency.

## Features

✅ **NEAR-Signed Auth** - ed25519 signature-based authentication (production)
✅ **Bearer Token Auth** - Simple token-based auth (development)
✅ **Idempotency** - Automatic request deduplication with `Idempotency-Key`
✅ **Type Safety** - Full TypeScript types for all requests/responses
✅ **Auto Retry** - Exponential backoff on transient failures
✅ **Timeout Control** - Configurable request timeouts

## Installation

```bash
npm install @near-outlayer/client
```

## Quick Start

### Bearer Auth (Development)

```typescript
import OutLayerClient from '@near-outlayer/client';

const client = new OutLayerClient({
  baseUrl: 'http://localhost:8080',
  auth: {
    type: 'bearer',
    token: 'your-dev-token'
  }
});

// Claim jobs for a request
const { jobs } = await client.claimJobs({
  requestId: 123,
  dataId: 'abc...',
  wasmChecksum: 'def...'
});

console.log('Jobs to execute:', jobs); // ['compile', 'execute'] or ['execute']
```

### NEAR-Signed Auth (Production)

```typescript
import OutLayerClient from '@near-outlayer/client';

const client = new OutLayerClient({
  baseUrl: 'https://outlayer.near.org',
  auth: {
    type: 'near',
    accountId: 'worker.near',
    privateKey: 'ed25519:5JueXZhEEVqGVT5powZ9pzMEuL4oHW9Ye4RdYNOH6yqHNXAYmKfEYEKvpzhRzKdqUPAR2T4cUAdNBNrG9M4uaYb'
  }
});

// All requests are automatically signed with ed25519
const result = await client.submitResult({
  requestId: 123,
  success: true,
  output: { result: 42 },
  resourcesUsed: {
    instructions: 1_000_000,
    timeMs: 150
  }
});
```

### Environment-Based Config

```typescript
import { createClientFromEnv } from '@near-outlayer/client';

// Reads from env vars:
// - OUTLAYER_BASE_URL
// - OUTLAYER_AUTH_TYPE (bearer or near)
// - OUTLAYER_AUTH_TOKEN (if bearer)
// - OUTLAYER_ACCOUNT_ID (if near)
// - OUTLAYER_PRIVATE_KEY (if near)

const client = createClientFromEnv();
```

## API Reference

### Constructor

```typescript
new OutLayerClient(config: OutLayerClientConfig)
```

**Config Options:**

```typescript
interface OutLayerClientConfig {
  baseUrl: string;           // Coordinator URL
  auth: BearerAuth | NearAuth;
  timeout?: number;          // Request timeout in ms (default: 30000)
  retries?: number;          // Max retry attempts (default: 3)
}
```

### Authentication Types

**Bearer Auth:**
```typescript
interface BearerAuth {
  type: 'bearer';
  token: string;
}
```

**NEAR Auth:**
```typescript
interface NearAuth {
  type: 'near';
  accountId: string;
  privateKey: string;  // ed25519:base58_encoded
}
```

### Methods

#### `claimJobs(request)`

Claim jobs for an execution request.

```typescript
await client.claimJobs({
  requestId: 123,
  dataId: 'abc...',
  wasmChecksum?: 'def...',     // Optional: Skip if WASM exists
  idempotencyKey?: 'uuid-v4'   // Optional: Prevent duplicate claims
});

// Returns:
{
  jobs: ['compile', 'execute']  // or ['execute'] if WASM cached
}
```

#### `uploadWasm(request)`

Upload compiled WASM to coordinator cache.

```typescript
const wasmBytes = await fs.readFile('target/wasm32-wasip1/release/my_module.wasm');

await client.uploadWasm({
  requestId: 123,
  dataId: 'abc...',
  wasmBytes: new Uint8Array(wasmBytes),
  checksum: computeChecksum(new Uint8Array(wasmBytes)),
  idempotencyKey?: 'uuid-v4'
});

// Returns:
{
  success: true,
  checksum: 'sha256...'
}
```

#### `submitResult(request)`

Submit execution result to coordinator.

```typescript
await client.submitResult({
  requestId: 123,
  success: true,
  output: { result: 42 },
  error?: 'Execution failed: ...',
  resourcesUsed: {
    instructions: 1_000_000,
    timeMs: 150,
    compileTimeMs?: 3000
  },
  compilationNote?: 'Cached WASM from 2025-01-10 14:30 UTC',
  idempotencyKey?: 'uuid-v4'
});

// Returns:
{
  success: true
}
```

## Idempotency

Use idempotency keys to prevent duplicate request processing:

```typescript
import { generateIdempotencyKey } from '@near-outlayer/client';

const key = generateIdempotencyKey(); // UUID v4

// First request: Processed normally
await client.claimJobs({ requestId: 123, dataId: 'abc', idempotencyKey: key });

// Duplicate request (within 10 minutes): Returns cached response
await client.claimJobs({ requestId: 123, dataId: 'abc', idempotencyKey: key });
// Response header: X-Idempotency-Replay: true
```

**When to use:**
- Network retries (client resends same request)
- Job claim atomicity (multiple workers)
- Payment deduplication (user clicks "submit" multiple times)

**TTL:** 10 minutes (configurable on server)

## NEAR-Signed Authentication

The client automatically signs requests using ed25519 when `auth.type === 'near'`.

**Signature Protocol:**

1. Compute body hash: `sha256(request_body)`
2. Create message: `method|path|body_hash|timestamp`
3. Sign with ed25519 private key
4. Encode signature as base58

**Headers sent:**
```
X-Near-Account: worker.near
X-Near-Signature: 5JueXZhEEVqGVT5pow...
X-Near-Timestamp: 1704931200
```

**Server verification:**
- Validates timestamp (±5 minute window) - prevents replay attacks
- Verifies signature against registered public key
- Checks body hash integrity

## Error Handling

```typescript
try {
  const result = await client.claimJobs({ ... });
} catch (error) {
  if (error.message.includes('HTTP 401')) {
    console.error('Authentication failed - check credentials');
  } else if (error.message.includes('HTTP 409')) {
    console.error('Job already claimed by another worker');
  } else if (error.message.includes('HTTP 429')) {
    console.error('Rate limited - slow down');
  } else {
    console.error('Request failed:', error);
  }
}
```

**Automatic Retries:**
- 5xx errors: Retried with exponential backoff (1s, 2s, 4s)
- 4xx errors: NOT retried (client errors)
- Network errors: Retried
- Timeouts: Retried

## Utility Functions

### `generateIdempotencyKey()`

Generate UUID v4 for idempotency keys.

```typescript
import { generateIdempotencyKey } from '@near-outlayer/client';

const key = generateIdempotencyKey();
// '550e8400-e29b-41d4-a716-446655440000'
```

### `computeChecksum(data)`

Compute SHA-256 hash of bytes.

```typescript
import { computeChecksum } from '@near-outlayer/client';

const wasmBytes = new Uint8Array([ ... ]);
const checksum = computeChecksum(wasmBytes);
// 'a7ffc6f8bf1ed76651c14756a061d662f580ff4de43b49fa82d80a4b80f8434a'
```

## TypeScript Support

Full type definitions included:

```typescript
import type {
  OutLayerClientConfig,
  BearerAuth,
  NearAuth,
  ClaimJobsRequest,
  ClaimJobsResponse,
  UploadWasmRequest,
  SubmitResultRequest,
  ResourceMetrics,
  JobType
} from '@near-outlayer/client';
```

## Development

```bash
# Install dependencies
npm install

# Build
npm run build

# Watch mode
npm run watch

# Run tests
npm test
```

## Examples

### Worker Implementation

```typescript
import OutLayerClient from '@near-outlayer/client';
import { readFile } from 'fs/promises';

const client = new OutLayerClient({
  baseUrl: process.env.COORDINATOR_URL || 'http://localhost:8080',
  auth: {
    type: 'near',
    accountId: process.env.WORKER_ACCOUNT_ID!,
    privateKey: process.env.WORKER_PRIVATE_KEY!
  }
});

async function processRequest(requestId: number, dataId: string) {
  // 1. Claim jobs
  const { jobs } = await client.claimJobs({ requestId, dataId });

  if (jobs.length === 0) {
    console.log('No jobs to process (already claimed)');
    return;
  }

  // 2. Compile if needed
  if (jobs.includes('compile')) {
    const wasmBytes = await compileWasm(requestId, dataId);
    const checksum = computeChecksum(wasmBytes);

    await client.uploadWasm({
      requestId,
      dataId,
      wasmBytes,
      checksum
    });
  }

  // 3. Execute
  if (jobs.includes('execute')) {
    const result = await executeWasm(requestId, dataId);

    await client.submitResult({
      requestId,
      success: result.success,
      output: result.output,
      error: result.error,
      resourcesUsed: result.metrics
    });
  }
}
```

### Custom Timeout & Retries

```typescript
const client = new OutLayerClient({
  baseUrl: 'http://localhost:8080',
  auth: { type: 'bearer', token: 'dev-token' },
  timeout: 60000,  // 60 seconds
  retries: 5       // 5 retry attempts
});
```

## License

MIT

## Support

- **Docs**: https://docs.near-outlayer.org
- **Issues**: https://github.com/near/outlayer/issues
- **Discord**: https://discord.gg/near
