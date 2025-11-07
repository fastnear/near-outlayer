# QuickJS Browser Sandbox Integration

**Status**: ✅ Phase 3 Complete - Deterministic JavaScript Contract Execution

## Overview

Clean, deterministic QuickJS sandbox for browser-based JavaScript contract execution with proper guardrails and minimal surface area.

**Security Model**: This provides an *execution sandbox* with deterministic compute, NOT a secure vault. See [docs/browser-sec-architecture.md](../docs/browser-sec-architecture.md) for the complete security model, threat analysis, and best practices.

**Tier-Based Deployment**: This is **Tier 1** (convenience). For enhanced security, see [Soft Enclave](../docs/soft-enclave.md) (**Tier 2**, cross-origin isolation) or TEE backends (**Tier 3**, hardware attestation).

## Architecture

```
┌─────────────────────────────────────────────────┐
│  ContractSimulator (mode: 'quickjs-browser')    │
│                                                  │
│  ┌────────────────────────────────────────────┐ │
│  │  QuickJSEnclave (execution sandbox)        │ │
│  │  ├── Single runtime (shared)               │ │
│  │  └── Fresh context per invocation          │ │
│  │      ├── Deterministic prelude             │ │
│  │      │   ├── LCG PRNG (Math.random)        │ │
│  │      │   ├── Fixed clock (Date.now = 0)    │ │
│  │      │   ├── Disabled eval                 │ │
│  │      │   ├── Disabled timers               │ │
│  │      │   └── Frozen intrinsics             │ │
│  │      ├── NEAR storage shim                 │ │
│  │      │   ├── near.storageRead(k)           │ │
│  │      │   ├── near.storageWrite(k, v)       │ │
│  │      │   └── near.log(...args)             │ │
│  │      ├── Contract evaluation               │ │
│  │      └── Function invocation                │ │
│  └────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────┘
```

## Key Properties

### ✅ Determinism Guarantees

1. **Fresh context per call**: New QuickJS context for each invocation prevents cross-call contamination
2. **Seeded PRNG**: Linear congruential generator (LCG) with configurable seed
3. **Fixed clock**: `Date.now()` always returns 0
4. **Disabled non-determinism**: `eval`, `setTimeout`, `setInterval` all disabled
5. **Frozen intrinsics**: `Object.prototype`, `Array.prototype`, `Function.prototype` frozen

### 🛡️ Security Guardrails

1. **Memory limit**: Configurable per invocation (default: 32 MiB)
2. **Time budget**: Wall-clock interrupt handler (default: 200ms)
3. **No ambient capabilities**: No `std`, `os`, `fetch` - only `near` shim
4. **No filesystem**: Pure in-memory JSON bridge
5. **Minimal API surface**: Only 3 `near` methods exposed

### 📊 Resource Budgets

```typescript
interface EnclavePolicy {
  timeMs: number;       // Wall-clock budget (enforced via interrupt)
  memoryBytes: number;  // Heap limit (enforced by QuickJS)
}
```

## Files Created

```
browser-worker/
├── src/
│   └── quickjs-enclave.ts         # Core enclave (220 lines)
├── examples/
│   └── counter.js                 # Minimal counter contract
├── __tests__/
│   └── quickjs-enclave.test.js    # Comprehensive determinism tests
├── package.json                    # Updated with quickjs-emscripten
├── tsconfig.json                   # TypeScript config
└── QUICKJS_INTEGRATION.md         # This file
```

## Usage

### 1. Basic Execution

```javascript
import { ContractSimulator } from './src/contract-simulator.js';

const sim = new ContractSimulator({
  executionMode: 'quickjs-browser',
  verboseLogging: true
});

// Execute JavaScript contract
const result = await sim.execute(
  `globalThis.increment = function() {
    let c = near.storageRead("count") || 0;
    c = (c|0) + 1;
    near.storageWrite("count", c);
    return { count: c };
  }`,
  'increment',
  {},
  { seed: 'deterministic-seed-123' }
);

console.log(result.result);  // { count: 1 }
console.log(result.logs);    // Captured near.log() calls
```

### 2. Stateful Contracts

```javascript
// First call: 0 → 1
const r1 = await sim.execute(counterSource, 'increment', {});
console.log(r1.result);  // { count: 1 }

// Second call: 1 → 2 (state persists)
const r2 = await sim.execute(counterSource, 'increment', {});
console.log(r2.result);  // { count: 2 }
```

### 3. Custom Budgets

```javascript
const result = await sim.execute(
  contractSource,
  'heavyComputation',
  { iterations: 10000 },
  {
    seed: 'seed-123',
    timeMs: 1000,           // 1 second timeout
    memoryBytes: 64 << 20   // 64 MiB memory
  }
);
```

## API Reference

### `QuickJSEnclave`

```typescript
class QuickJSEnclave {
  static async create(opts?: { memoryBytes?: number }): Promise<QuickJSEnclave>;

  async invoke(inv: Invocation): Promise<InvocationResult>;
}

interface Invocation {
  source: string;       // JS contract source
  func: string;         // Function name to call
  args: Json[];         // Positional arguments
  priorState: Json;     // JSON state object
  seed: string;         // Deterministic seed
  policy: EnclavePolicy;
}

interface InvocationResult {
  state: Json;                  // New state
  result: Json;                 // Function return value
  diagnostics: {
    logs: string[];
    timeMs: number;
    interrupted: boolean;
  };
}
```

### NEAR Storage Shim

```typescript
globalThis.near = {
  storageRead: (k: string) => state[k],
  storageWrite: (k: string, v: any) => { state[k] = v; },
  log: (...args: any[]) => { /* captured */ }
};
```

## When to Use This vs Soft Enclave

| Criteria | Tier 1 (This) | Tier 2 (Soft Enclave) |
|----------|---------------|----------------------|
| **Transaction value** | < $100 | $100 - $10,000 |
| **Setup complexity** | Low (single origin) | Medium (two origins) |
| **SOP barrier** | ❌ | ✅ |
| **Encrypted RPC** | ❌ | ✅ |
| **Egress guard** | ❌ | ✅ |
| **Use case** | Convenience, prototyping | Enhanced security, production |

**Choose Tier 1 (this) when:**
- User expects single-page UX
- Transaction value is low
- Deterministic compute + WebCrypto custody is sufficient

**Choose Tier 2 (Soft Enclave) when:**
- Transaction value warrants extra security
- User accepts slight UX friction (iframe + cross-origin)
- Defense-in-depth against XSS and malicious scripts is required

See [docs/soft-enclave.md](../docs/soft-enclave.md) for Tier 2 architecture and setup.

## Testing

### Run Tests

```bash
cd browser-worker
npm install
npm test
```

### Test Coverage

```
✅ Determinism
  ✓ Replays identically with same seed/state/args
  ✓ getValue returns current state without modification
  ✓ Reset clears counter state
  ✓ Deterministic Math.random with same seed
  ✓ Date.now returns 0 (deterministic clock)
  ✓ eval is disabled
  ✓ Function not found returns error
  ✓ near.log captures multiple arguments
  ✓ Multiple invocations do not leak state

✅ Arguments
  ✓ Passes arguments correctly
  ✓ Handles complex object arguments
```

## Example Contracts

### Counter (examples/counter.js)

```javascript
globalThis.increment = function () {
  let c = near.storageRead("count") || 0;
  c = (c|0) + 1;
  near.storageWrite("count", c);
  near.log("count ->", c);
  return { count: c };
};

globalThis.getValue = function () {
  let c = near.storageRead("count") || 0;
  return { count: c };
};

globalThis.reset = function () {
  near.storageWrite("count", 0);
  near.log("count reset to 0");
  return { count: 0 };
};
```

## Performance

- **Cold start**: ~5-10ms (QuickJS initialization)
- **Warm execution**: ~0.1-1ms per function call
- **State persistence**: JSON serialization (no binary formats)
- **Context cleanup**: Automatic per invocation

## Integration with OutLayer

### ContractSimulator Modes

```javascript
const modes = [
  'direct',           // Direct WASM execution
  'linux',            // Linux kernel WASM
  'enclave',          // Frozen Realm (E2EE)
  'quickjs-browser'   // QuickJS (this integration)
];

// Switch mode
await sim.setExecutionMode('quickjs-browser');
```

### State Isolation

QuickJS state is stored separately from WASM contract state:

```javascript
// Internal storage
this._quickjsState = {}; // Isolated from nearState Map

// Methods
getQuickJSState()   // Returns current JS contract state
saveQuickJSState(state)  // Persists after execution
```

## Limitations (Demo Mode)

- **No host syscalls**: All I/O via JSON state
- **Single-threaded**: No async/await support
- **No networking**: HTTP requests disabled
- **No modules**: Single-file contracts only (for now)
- **State size**: Practical limit ~1-10 MB (JSON overhead)

## Production Upgrade Path (Optional)

Future enhancements (not in this PR):

1. **Bytecode cache**: `ctx.evalBytecode()` keyed by SHA-256(source)
2. **Context pool**: Reuse contexts for high throughput
3. **Module loader**: Multi-file contracts with explicit import map
4. **Structured logs**: Line/column mapping with sourceURL
5. **C syscall bridge**: Direct NEAR host calls (not file-based)

## Definition of Done ✅

- ✅ Deterministic runtime with interrupt + memory limits
- ✅ Hardened globals and disabled timers/eval
- ✅ near.storageRead/Write/log shim only
- ✅ Clean API: `invoke({ source, func, args, priorState, seed, policy })`
- ✅ Determinism tests (0→1→2 with state persistence)
- ✅ Integrated with ContractSimulator (`quickjs-browser` mode)
- ✅ Zero changes to existing WASM execution paths
- ✅ Comprehensive test suite (11 tests, all passing)

## References

- QuickJS: https://bellard.org/quickjs/
- quickjs-emscripten: https://github.com/justjake/quickjs-emscripten
- NEAR Protocol: https://near.org/
- LCG PRNG: https://en.wikipedia.org/wiki/Linear_congruential_generator

## License

MIT

---

**Maintainer**: OutLayer Team
**Date**: 2025-01-06
**Version**: Phase 3 - Browser Enclave
