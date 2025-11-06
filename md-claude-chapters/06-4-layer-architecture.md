# Chapter 6: 4-Layer Virtualization Architecture - Deep Dive

**Document Type**: Technical Deep Dive
**Audience**: Architects, Security Engineers, Core Contributors
**Prerequisites**: Chapters 1-3

---

## Executive Summary

This chapter presents a comprehensive analysis of the 4-layer nested virtualization architecture that positions NEAR OutLayer as the **only blockchain platform** combining:

1. **JavaScript contracts** (vs Rust/Solidity)
2. **Full POSIX environment** (vs EVM/eBPF)
3. **Deterministic execution** (vs non-reproducible runs)
4. **Native WebAssembly speed** (vs x86 emulation)

This architecture enables "daring" applications impossible on other chains: verifiable AI agents, safe plugin systems, and stateful edge computing.

---

## The Four Layers Defined

```
┌─────────────────────────────────────────────────────────────┐
│ L1: Host Wasm Runtime                                       │
│     Browser (V8/SpiderMonkey) or Wasmtime/WasmEdge        │
│     - Native code execution                                 │
│     - Wasm VM sandbox (capability-based security)          │
│     - Host APIs: WASI, Web APIs, custom imports           │
└────────────────────────┬────────────────────────────────────┘
                         │ Wasm instantiation
                         ↓
┌─────────────────────────────────────────────────────────────┐
│ L2: Guest OS (linux-wasm)                                   │
│     Linux kernel 6.4.16 + BusyBox userland (NOMMU)        │
│     - Native Wasm execution (not x86 emulation!)           │
│     - Full POSIX syscall API                               │
│     - Multi-process via vfork/exec                         │
│     - Virtual filesystem, signals, pipes                   │
└────────────────────────┬────────────────────────────────────┘
                         │ POSIX process spawn
                         ↓
┌─────────────────────────────────────────────────────────────┐
│ L3: Guest Runtime (QuickJS)                                 │
│     JavaScript engine (~2 MB binary)                        │
│     - Runs as /bin/qjs in L2 userland                      │
│     - ES2020 support, no JIT                               │
│     - <300μs startup time                                   │
│     - NEAR host functions via C bridge → L2 syscalls       │
└────────────────────────┬────────────────────────────────────┘
                         │ SES Compartment
                         ↓
┌─────────────────────────────────────────────────────────────┐
│ L4: Guest Code (Frozen Realm)                               │
│     User JavaScript in hardened environment                 │
│     - Immutable globals (lockdown() freezes intrinsics)    │
│     - No Date.now(), Math.random(), fetch()                │
│     - Only injected NEAR capabilities                       │
│     - Deterministic: same input → same output always       │
└─────────────────────────────────────────────────────────────┘
```

---

## Layer 1: Host Wasm Runtime

### Role: Root Security Boundary

The L1 runtime is the **only hard security boundary** in the entire stack. It provides:

1. **Process isolation**: Separate WASM instances cannot interfere
2. **Memory safety**: WASM linear memory prevents out-of-bounds access
3. **Capability security**: No syscalls except via host functions
4. **Resource limiting**: CPU, memory, and time constraints

### Implementation Options

**Browser Runtimes**:
- **Chrome/Edge** (V8 Wasm): Production-ready, excellent performance
- **Firefox** (SpiderMonkey): Full SharedArrayBuffer support
- **Safari** (JavaScriptCore): Limited SharedArrayBuffer (requires COOP/COEP)

**Server Runtimes**:
- **Wasmtime**: Fast, standards-compliant, excellent WASI support
- **WasmEdge**: Edge-optimized, used by Shopify, Second State
- **Wasmer**: Universal runtime with multiple backends

### OutLayer Integration

**File**: `browser-worker/src/contract-simulator.js`

```javascript
// L1 Runtime abstraction
class WasmRuntime {
  async instantiate(wasmBytes, imports) {
    // Browser: WebAssembly.instantiate
    if (typeof WebAssembly !== 'undefined') {
      const { instance } = await WebAssembly.instantiate(wasmBytes, imports);
      return instance;
    }

    // Node.js: require('@wasmtime/wasmtime')
    // (Future server-side execution)
    throw new Error('No WASM runtime available');
  }
}
```

### Security Properties

**What L1 Protects Against**:
- Guest to host escapes (WASM sandbox enforced by CPU)
- Tenant to tenant interference (separate instances)
- Resource exhaustion (fuel metering, memory limits)
- Arbitrary syscalls (only whitelisted host functions)

**What L1 Does NOT Protect Against**:
- Logic bugs in L2-L4 code (application-level vulnerabilities)
- Non-determinism in L3 (unless L4 Frozen Realm used)
- Side-channel attacks (timing, spectre-like)

**Key Insight**: L1 is hardware-enforced (CPU MMU + WASM spec). Compromising L1 requires exploiting browser/wasmtime itself.

---

## Layer 2: Guest OS (linux-wasm)

### Role: POSIX API Provider

L2 is **not a security boundary**—it's an **API compatibility layer** that exposes full POSIX to L3/L4.

### Architecture: Native WASM, Not Emulation

**Critical Distinction**:
- **x86 Emulation** (WebVM, v86): Interprets x86 instructions in JavaScript (~10-20x overhead)
- **linux-wasm**: Compiles Linux kernel from C to WASM (~1-2x overhead vs native)

**How it works**:

1. **LLVM patches**: Recognize `wasm32` as target architecture
2. **Linux kernel patches**: 1,500+ lines modifying `arch/wasm/`
3. **musl libc patches**: WASM-specific syscall interface
4. **BusyBox patches**: Statically link against musl-wasm

**Result**: A single `vmlinux.wasm` file (~24 MB) that boots Linux natively in WASM.

### NOMMU Architecture

**What is NOMMU?**

MMU (Memory Management Unit) provides virtual memory, which enables:
- Per-process address spaces (memory isolation)
- Copy-on-write fork()
- mmap() for shared memory IPC

WASM has no MMU. Therefore, linux-wasm is **NOMMU Linux** (CONFIG_MMU=n).

**Implications**:

| Feature | With MMU | Without MMU (linux-wasm) |
|---------|----------|--------------------------|
| **fork()** | Yes, COW semantics | No, use vfork() instead |
| **Process isolation** | Hardware-enforced | **Shared memory space** |
| **mmap() IPC** | Yes | Restricted |
| **Stack overflows** | Caught by hardware | **Can corrupt kernel** |
| **Security** | Strong | **Weak (L1 is boundary)** |

**Key Insight**: All L2 processes (kernel + all userland) share the **same WASM linear memory**. A buffer overflow in one process can theoretically read/write another's memory.

**Why this is acceptable**:
- L1 WASM sandbox still protects host
- L2 provides **API richness**, not **security isolation**
- Real security boundary is L1, not L2

### NEAR Syscall Integration

L2 exposes NEAR operations as Linux syscalls (400-499 range).

**Kernel patch** (`arch/wasm/syscalls.c`):

```c
// NEAR storage read syscall
SYSCALL_DEFINE4(near_storage_read,
    const char __user *, key,
    size_t, key_len,
    char __user *, value,
    size_t, value_len)
{
    // Call L1 host function via import
    return wasm_import_near_storage_read(key, key_len, value, value_len);
}

// Register syscall
#define __NR_near_storage_read 400
```

**L1 host function** (provided by ContractSimulator):

```javascript
// In WASM imports
const imports = {
  env: {
    wasm_import_near_storage_read: (keyPtr, keyLen, valuePtr, valueLen) => {
      const key = readWasmString(memory, keyPtr, keyLen);
      const value = nearState.get(key);

      if (value) {
        writeWasmBytes(memory, valuePtr, value, valueLen);
        return value.length;
      }
      return -1;  // Not found
    },
  },
};
```

**L3 QuickJS usage**:

```c
// In QuickJS C bridge
int near_storage_read(const char* key, uint8_t* value, size_t value_len) {
    return syscall(400, key, strlen(key), value, value_len);
}
```

**Execution path**: L4 JS → L3 C bridge → L2 syscall → L1 host function → NEAR state

### Performance Characteristics

**Boot time**:
- First boot: ~30 seconds (kernel init + BusyBox mount)
- Subsequent boots: <1 second (cached compiled code)
- Demo mode: ~500ms (simulated, no actual kernel)

**Syscall overhead**:
- Native Linux: ~50-100 nanoseconds
- linux-wasm: ~100-200 microseconds (~1000x slower)
- Still acceptable for OutLayer use cases (logic-heavy, not syscall-heavy)

**Memory**:
- Kernel + BusyBox: ~10-15 MB
- Per-process overhead: ~100 KB (no copy-on-write)
- Total for full stack: ~30-40 MB

---

## Layer 3: Guest Runtime (QuickJS)

### Role: JavaScript Execution Engine

L3 provides JavaScript execution within the L2 POSIX environment.

### Why QuickJS?

**Comparison matrix**:

| Engine | Size | Startup | JIT | Deterministic | WASM-ready |
|--------|------|---------|-----|---------------|------------|
| V8 | ~20 MB | ~50ms | Yes | No | Difficult |
| JavaScriptCore | ~15 MB | ~30ms | Yes | No | Difficult |
| SpiderMonkey | ~25 MB | ~40ms | Yes | No | Difficult |
| **QuickJS** | **~2 MB** | **<1ms** | **No** | **Yes** | **✅ Yes** |

**Why "No JIT" is a feature**:
- JIT introduces non-determinism (optimization timing, inline caching)
- JIT adds security attack surface (RWX memory)
- JIT increases binary size significantly
- Interpretation is deterministic and predictable

**Production validation**:
- **Shopify Functions**: Millions of QuickJS executions/day
- **Javy**: Shopify's QuickJS WASM runtime (open source)
- **WasmEdge**: Uses quickjs-wasi for JS support

### Architecture

**QuickJS runs as POSIX binary** in L2:

```bash
# In linux-wasm userland
/bin/qjs --eval "console.log('Hello from L3')"
```

**Compilation to WASM**:

```bash
# Build QuickJS for wasm32-wasi
git clone https://github.com/second-state/quickjs-wasi
cd quickjs-wasi
make

# Output: qjs.wasm (~1.5-2 MB)
# Install to linux-wasm initramfs
cp qjs.wasm /initramfs/bin/qjs.wasm
```

**Integration with OutLayer**:

```javascript
// L2: LinuxExecutor spawns QuickJS
class LinuxExecutor {
  async executeJavaScript(jsCode, args) {
    // Load QuickJS WASM
    const qjsWasm = await fetch('/linux-runtime/bin/qjs.wasm').then(r => r.arrayBuffer());

    // Execute as L2 process
    const result = await this.executeProgram(
      qjsWasm,
      ['--eval', jsCode],
      { NEAR_STATE: '/near/state' }
    );

    return result;
  }
}
```

### NEAR Host Function Bridge

**Challenge**: QuickJS (L3) needs to call NEAR functions (L1).

**Solution**: C bridge library compiled into QuickJS binary.

**File**: `linux-runtime/lib/near-bridge.c`

```c
#include <quickjs.h>
#include <syscall.h>

// NEAR storage read wrapper
static JSValue js_near_storage_read(JSContext *ctx, JSValueConst this_val,
                                     int argc, JSValueConst *argv) {
    // Parse key from JS
    const char *key = JS_ToCString(ctx, argv[0]);
    size_t key_len = strlen(key);

    // Buffer for value
    uint8_t value[1024];

    // Call L2 syscall 400 (which calls L1 host function)
    int result = syscall(400, key, key_len, value, sizeof(value));

    JS_FreeCString(ctx, key);

    if (result > 0) {
        // Return value as ArrayBuffer
        return JS_NewArrayBufferCopy(ctx, value, result);
    }

    return JS_NULL;
}

// Register with QuickJS
static const JSCFunctionListEntry js_near_funcs[] = {
    JS_CFUNC_DEF("storageRead", 1, js_near_storage_read),
    JS_CFUNC_DEF("storageWrite", 2, js_near_storage_write),
    JS_CFUNC_DEF("log", 1, js_near_log),
    // ... other NEAR functions
};

// Initialize NEAR module
JSModuleDef *js_init_module_near(JSContext *ctx, const char *module_name) {
    JSModuleDef *m = JS_NewCModule(ctx, module_name, js_init_near);
    if (!m) return NULL;

    JS_AddModuleExportList(ctx, m, js_near_funcs, countof(js_near_funcs));
    return m;
}
```

**Usage in JavaScript**:

```javascript
// L4: User contract code
import * as near from 'near';

export function myContract(args) {
  const count = near.storageRead('count');
  near.storageWrite('count', count + 1);
  near.log(`Count: ${count + 1}`);
  return { count: count + 1 };
}
```

### Performance Characteristics

**Execution speed**:
- Simple operations: ~10-100x slower than native WASM
- Complex operations: ~5-20x slower (less overhead per operation)
- Acceptable for: Business logic, strategy calculation, rule engines
- Not acceptable for: Cryptographic operations, heavy math, tight loops

**Memory usage**:
- JSRuntime: ~500 KB baseline
- JSContext: ~100 KB per contract
- Garbage collection: Reference counting (deterministic, no GC pauses)

**Gas metering**:
- Track bytecode operations (not WASM instructions)
- QuickJS provides `JS_SetInterruptHandler()` for fuel metering
- Counts: function calls, loops, array operations

---

## Layer 4: Guest Code (Frozen Realm)

### Role: Deterministic Execution Environment

L4 is the **determinism layer** that makes OutLayer suitable for verifiable computation.

### The Non-Determinism Problem

**Standard JavaScript is non-deterministic**:

```javascript
// Different result every time
const random = Math.random();

// Different result every time
const now = Date.now();

// Non-deterministic timing
setTimeout(() => callback(), 1000);

// Network = external state
const data = await fetch('https://api.example.com/data');
```

**Why this breaks blockchain**:
- Validators must agree on execution result
- Same transaction → different results = consensus failure
- Replay attacks possible if results change

### SES (Secure ECMAScript) Solution

**SES** = Subset of JavaScript designed for secure, deterministic execution.

**Three primitives**:

1. **`lockdown()`**: Freezes all JavaScript intrinsics
2. **`harden()`**: Deep-freezes objects
3. **`Compartment`**: Isolated execution context

**Implementation**:

```javascript
// Load SES shim
import 'ses';

// Step 1: Freeze the world
lockdown({
  errorTaming: 'safe',        // Sanitize error messages
  overrideTaming: 'moderate', // Allow some overrides
  stackFiltering: 'verbose',  // Preserve stack traces
});

// After lockdown():
// - Array.prototype is frozen
// - Object.prototype is frozen
// - Function.prototype is frozen
// - Math is frozen
// - Date is frozen
// - All primordial objects frozen

// Step 2: Create isolated compartment
const compartment = new Compartment({
  // Endowments: ONLY these globals are available
  console: harden({
    log: (...args) => near.log(args.join(' ')),
  }),

  near: harden({
    storageRead: (key) => nearState.get(key),
    storageWrite: (key, value) => nearState.set(key, value),
    blockTimestamp: () => nearBlockTimestamp,  // From L1, not Date.now()
    // NO Math.random, NO Date.now, NO fetch
  }),
});

// Step 3: Execute user code in compartment
const contractModule = compartment.evaluate(`
  ${userContractCode}

  // Export methods
  ({ increment, getCount });
`);

// Step 4: Call method
const result = contractModule.increment({ amount: 1 });
```

### Determinism Guarantees

**What is eliminated**:

| Non-Deterministic API | Replacement | Rationale |
|----------------------|-------------|-----------|
| `Date.now()` | `near.blockTimestamp()` | Time from NEAR blockchain (same for all validators) |
| `Math.random()` | `near.randomSeed()` | Verifiable randomness from blockchain |
| `fetch()` | Removed | Network state is external, non-deterministic |
| `setTimeout()` | Removed | Timing is non-deterministic |
| `crypto.getRandomValues()` | `near.randomBytes()` | Deterministic random from chain |

**What is enforced**:

```javascript
// After lockdown() + Compartment:

// ✅ Allowed
const sum = [1, 2, 3].reduce((a, b) => a + b, 0);
const obj = { a: 1, b: 2 };
near.storageWrite('data', JSON.stringify(obj));

// ❌ Blocked (throws ReferenceError)
const now = Date.now();           // Date.now is not in endowments
const random = Math.random();     // Math.random is not in endowments
const data = await fetch(url);    // fetch is not in endowments

// ❌ Blocked (throws TypeError)
Array.prototype.evil = true;      // Array.prototype is frozen
Object.prototype.hack = () => {}; // Object.prototype is frozen
```

### Compartment Isolation

**Problem**: Multiple contracts in same runtime must not interfere.

**Solution**: Separate Compartment per contract.

```javascript
// Contract A
const compartmentA = new Compartment({
  near: harden({
    storageRead: (key) => stateA.get(key),
    storageWrite: (key, value) => stateA.set(key, value),
  }),
});

const contractA = compartmentA.evaluate(`
  globalThis.secret = 'password';  // Only visible in Compartment A
  export function getSecret() {
    return { secret: globalThis.secret };
  }
`);

// Contract B (different Compartment)
const compartmentB = new Compartment({
  near: harden({
    storageRead: (key) => stateB.get(key),
    storageWrite: (key, value) => stateB.set(key, value),
  }),
});

const contractB = compartmentB.evaluate(`
  export function stealSecret() {
    // This throws ReferenceError: globalThis.secret not defined
    return { stolen: globalThis.secret };
  }
`);
```

**Result**: Contract B cannot access Contract A's `globalThis` because they're in separate Compartments.

### Security Properties

**What L4 Prevents**:
- **Prototype poisoning**: `Array.prototype.hack = ...` throws TypeError
- **Global namespace pollution**: Each Compartment has isolated `globalThis`
- **Non-determinism**: No Date.now/Math.random/fetch
- **Side-channel timing**: No setTimeout/setInterval

**What L4 Does NOT Prevent**:
- **Logic bugs**: User code can still have vulnerabilities
- **Reentrancy**: Cross-contract calls can still reenter
- **Resource exhaustion**: Still need gas metering at L3

**Defense in Depth**:
- L1: Hardware memory safety
- L2: API abstraction (not security)
- L3: Gas metering, timeout enforcement
- L4: Determinism, prototype immutability

---

## The I/O Trombone Problem

### Challenge

Every I/O operation traverses **all four layers**, creating high latency.

**Example**: Contract reads NEAR storage

```
L4 (JS):        near.storageRead('key')
                  ↓ Compartment boundary
L3 (QuickJS):  js_near_storage_read() C function
                  ↓ syscall(400, ...)
L2 (linux-wasm): kernel syscall handler
                  ↓ wasm_import_near_storage_read()
L1 (Browser):   nearState.get('key')
                  ↓ JavaScript Map lookup
                  ↑ Return value
L2:             Copy to WASM linear memory
                  ↑
L3:             Copy to QuickJS heap
                  ↑
L4:             Return to user code
```

**Latency breakdown** (projected):
- L4→L3 boundary: ~0.1ms (C function call)
- L3→L2 boundary: ~0.5ms (syscall overhead)
- L2→L1 boundary: ~0.2ms (WASM import call)
- L1 operation: ~0.01ms (Map lookup)
- Return path: ~0.8ms (copy + unwind)
- **Total: ~1.6ms per storage operation**

Compare to direct WASM:
- L1 only: ~0.01ms (Map lookup)
- **Overhead: ~160x for simple operations**

### Mitigation Strategies

#### Strategy 1: Bypass for Performance-Critical Operations

**Idea**: Provide "fast path" directly from L4 to L1.

```javascript
// In Compartment endowments
const compartment = new Compartment({
  near: harden({
    // Slow path: L4→L3→L2→L1
    storageReadSlow: (key) => { /* full stack */ },

    // Fast path: L4→L1 direct (bypasses L2+L3)
    storageReadFast: (key) => nearState.get(key),
  }),
});

// User chooses:
const value1 = near.storageReadSlow('key');  // Correct, slow
const value2 = near.storageReadFast('key');  // Fast, but breaks "pure L2" model
```

**Trade-off**: Breaks pure layering, but pragmatic for production.

#### Strategy 2: Batching

**Idea**: Amortize overhead across multiple operations.

```javascript
// Instead of:
for (let i = 0; i < 100; i++) {
  const value = near.storageRead(`key${i}`);  // 100 round trips
  process(value);
}

// Do:
const keys = Array.from({ length: 100 }, (_, i) => `key${i}`);
const values = near.storageBatchRead(keys);  // 1 round trip
for (const value of values) {
  process(value);
}
```

#### Strategy 3: Caching at L3

**Idea**: Cache frequently accessed state in QuickJS heap.

```javascript
// L3: QuickJS maintains cache
const storageCache = new Map();

function near_storage_read_cached(key) {
  if (storageCache.has(key)) {
    return storageCache.get(key);  // No L2→L1 round trip
  }

  const value = syscall(400, key);  // Full round trip
  storageCache.set(key, value);
  return value;
}
```

**Trade-off**: Stale reads possible, cache invalidation complexity.

---

## Performance Projections

### Baseline: Direct WASM (L1 Only)

```
Cold start:        ~5-10ms
Warm execution:    ~0.5-1ms
Throughput:        500-1000 ops/sec
Memory:            2-5 MB per contract
```

### Full Stack: L1→L2→L3→L4

**Best case** (simple logic, minimal I/O):
```
Cold start:        ~50-100ms (kernel boot + QuickJS init)
Warm execution:    ~10-50ms (L3 interpretation + L2 syscalls)
Throughput:        20-100 ops/sec
Memory:            30-40 MB (kernel + QuickJS + contract)
Overhead:          ~10-50x vs direct WASM
```

**Worst case** (I/O-heavy workload):
```
Execution:         ~100-500ms (many storage operations)
Throughput:        2-10 ops/sec
Overhead:          ~100-500x vs direct WASM
```

**Acceptable use cases**:
- ✅ AI agents (model inference: compute-heavy, not I/O-heavy)
- ✅ DeFi strategies (complex logic: 1-10 storage operations)
- ✅ Plugin systems (rare execution: amortized overhead)
- ❌ High-frequency trading (needs <1ms latency)
- ❌ Token transfers (direct WASM is better)

---

## Competitive Analysis

### vs Ethereum (EVM)

| Dimension | Ethereum | OutLayer 4-Layer |
|-----------|----------|------------------|
| **Language** | Solidity | JavaScript |
| **Execution** | EVM bytecode | WASM → Linux → QuickJS |
| **Determinism** | Yes (gas-based) | Yes (Frozen Realms) |
| **POSIX** | No | Full (fork, pipes, etc.) |
| **Multi-process** | No | Yes (vfork/exec) |
| **Performance** | Fast (~10ms) | Slow (~50-100ms) |
| **Developer UX** | Steep learning curve | JavaScript (familiar) |
| **Use Cases** | DeFi, NFTs | AI agents, plugins, edge compute |

**Verdict**: Ethereum wins on speed, OutLayer wins on capabilities and developer UX.

### vs Solana (eBPF)

| Dimension | Solana | OutLayer 4-Layer |
|-----------|--------|------------------|
| **Language** | Rust | JavaScript |
| **Execution** | eBPF VM | WASM → Linux → QuickJS |
| **Determinism** | Yes | Yes |
| **POSIX** | No | Full |
| **Parallel** | Yes (Sealevel) | Limited (NOMMU) |
| **Performance** | Very fast (~1ms) | Slow (~50-100ms) |
| **State** | Account model | Verifiable off-chain |
| **Use Cases** | High-throughput DeFi | Complex logic, AI |

**Verdict**: Solana wins on speed and parallelism, OutLayer wins on expressiveness.

### vs NEAR Direct Contracts (near-sdk-rs)

| Dimension | NEAR Direct | OutLayer 4-Layer |
|-----------|-------------|------------------|
| **Language** | Rust | JavaScript |
| **Compilation** | Required (~30s) | None (instant deploy) |
| **Execution** | WASM (~1ms) | QuickJS (~50-100ms) |
| **Determinism** | Manual | Automatic (Frozen Realms) |
| **Dynamic Code** | No | Yes (load from IPFS) |
| **POSIX** | No | Full |
| **Verifiable Compute** | No | Yes |
| **Use Cases** | All | AI, plugins, edge |

**Verdict**: NEAR Direct wins on speed, OutLayer wins on dynamism and verifiability.

---

## Security Model

### Trust Boundaries

```
Untrusted ──────────────────────────────────────────────
              L4: User JavaScript Code
              ↓ (Frozen Realm boundary)
              L3: QuickJS Engine
              ↓ (Process boundary - weak)
              L2: linux-wasm Kernel
              ↓ (WASM sandbox - STRONG)
Trusted ─────────────────────────────────────────────────
              L1: Host Runtime
              ↓ (OS boundary)
              Host Operating System
```

**Only L1 is a hard security boundary.**

### Attack Scenarios

#### Scenario 1: Malicious L4 Contract

**Attack**: User contract tries to steal funds.

```javascript
// Malicious contract
export function attack() {
  // Try to access another user's storage
  const victimBalance = near.storageRead('victim.near/balance');

  // Try to transfer to attacker
  near.promiseCreate('token.near', 'transfer', {
    receiver: 'attacker.near',
    amount: victimBalance,
  });
}
```

**Defense**:
- L4 Compartment: No access to other contracts' storage
- L1 Host: NEAR protocol validates cross-contract calls
- Result: Attack fails, no funds stolen

#### Scenario 2: L3 QuickJS Exploit

**Attack**: Buffer overflow in QuickJS C code.

```javascript
// Trigger overflow
const bigArray = new Array(0x7FFFFFFF);
bigArray.fill('A'.repeat(1000));  // Try to exhaust memory
```

**Defense**:
- L3 Gas metering: Kills execution before overflow
- L2 Resource limits: Memory cap enforced by kernel
- L1 WASM: Even if L2 fails, L1 sandbox contains damage
- Result: Execution aborted, no host compromise

#### Scenario 3: L2 Kernel Exploit

**Attack**: Stack overflow in linux-wasm kernel.

```c
// Malicious syscall triggers kernel bug
syscall(999, huge_buffer, 0x7FFFFFFF);
```

**Defense**:
- L2 NOMMU: Could corrupt L2 kernel memory
- L1 WASM: Kernel runs in L1 WASM sandbox, cannot escape
- Result: L2 crashes, but L1 host unaffected

**Key Insight**: L2 is not a security boundary. L1 WASM sandbox is the only hard boundary.

---

## Future Enhancements

### WebAssembly Component Model

**Current**: Monolithic WASM modules with manual ABI.

**Future**: Composable components with language-agnostic interfaces.

**Impact on OutLayer**:

```wit
// WIT (WebAssembly Interface Types) definition
interface near {
  storage-read: func(key: string) -> option<list<u8>>;
  storage-write: func(key: string, value: list<u8>);
}

world outlayer {
  import near;  // L4 JS directly imports L1 NEAR interface
}
```

**Benefits**:
- Eliminate I/O trombone (L4 to L1 direct call)
- Type safety across layers
- Better performance (no L2 to L3 overhead for NEAR calls)

### Hardware TEE Integration

**Current**: Trust in L1 runtime (browser/wasmtime).

**Future**: Hardware-verified execution.

**Options**:
1. **Intel SGX**: Enclave-based execution, attestation
2. **AMD SEV**: VM-level encryption
3. **ARM TrustZone**: Mobile TEE

**Integration**:

```
L1: TEE-enabled Wasmtime
  ├─ SGX Enclave wraps L2-L4 execution
  ├─ Attestation report proves "correct code running"
  └─ Remote attestation via NEAR contract
```

**Benefits**:
- Trust worker execution (not just L1 sandbox)
- Confidential compute (encrypted state)
- Verifiable execution (attestation proves integrity)

---

## Conclusion

The 4-layer architecture is **bleeding-edge** and positions NEAR OutLayer as the only blockchain platform with:

1. **JavaScript contracts** - 15M developers, instant deployment
2. **Full POSIX** - Fork, pipes, syscalls (impossible on other chains)
3. **Deterministic execution** - Frozen Realms guarantee reproducibility
4. **Verifiable computation** - Attestation + determinism = auditability

**Trade-off**: ~10-100x slower than direct WASM

**Acceptable for**: AI agents, plugin systems, complex DeFi logic, edge computing

**Not acceptable for**: High-frequency operations, token transfers, simple state updates

**Strategic value**: Opens entirely new application categories impossible on Ethereum, Solana, or current NEAR.

---

## Related Documentation

- **Roadmap**: [Chapter 3: Multi-Layer Roadmap](03-multi-layer-roadmap.md)
- **Applications**: [Chapter 7: Daring Applications](07-daring-applications.md)
- **Implementation**: [Chapter 2: Linux/WASM Integration](02-linux-wasm-integration.md)
