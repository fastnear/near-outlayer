# Chapter 3: Multi-Layer Architecture Roadmap (Phases 3-6)

**Status**: Strategic Planning
**Timeline**: 3-6 months total
**Goal**: Complete 4-layer virtualization stack

---

## Overview

This chapter outlines the strategic roadmap from our current state (Phase 2: Linux/WASM integration complete) to a full 4-layer virtualization architecture that positions NEAR OutLayer as the **only blockchain platform with JavaScript contracts, full POSIX support, and deterministic execution guarantees**.

### The 4-Layer Vision

```
L1: Host Wasm Runtime (Browser/Wasmtime)
└─ L2: Guest OS (linux-wasm kernel + POSIX userland)
   └─ L3: Guest Runtime (QuickJS JavaScript engine)
      └─ L4: Guest Code (User JS in Frozen Realm)
```

**What this enables**:
1. **JavaScript contracts** - No Rust/WASM compilation required
2. **Full POSIX environment** - Fork, pipes, filesystem, syscalls
3. **Deterministic execution** - Frozen Realms eliminate non-determinism
4. **Verifiable computation** - Same inputs → same outputs (auditable)
5. **Dynamic code execution** - Load strategies from IPFS, execute immediately

---

## Current State (Phase 2 Complete)

### What We Have

**L1: Host Runtime** (Complete)
- Browser WebAssembly support
- ContractSimulator orchestration layer
- wasmi/wasmtime execution engines

**L2: Linux/WASM** (Demo Mode Complete)
- LinuxExecutor class (439 lines)
- Demo mode simulation (fast iteration)
- Production mode architecture documented
- NEAR syscall mapping defined (400-499)

**L3: QuickJS** (Not Started)
- No JavaScript runtime layer
- No JS contract execution

**L4: Frozen Realms** (Not Started)
- No deterministic execution
- No SES integration

### Gap Analysis

| Layer | Status | Dependencies | Blocking |
|-------|--------|--------------|----------|
| L1 | Complete | None | - |
| L2 | Demo only | vmlinux.wasm (~24 MB) | SharedArrayBuffer support |
| L3 | Missing | L2 production + QuickJS WASI build | - |
| L4 | Missing | L3 + SES shim | - |

---

## Phase 3: QuickJS Integration (2-3 Weeks)

### Goal

Add L3 JavaScript runtime layer, enabling JavaScript contract execution within linux-wasm environment.

### QuickJS Background

**What is QuickJS?**
- Small, embeddable JavaScript engine by Fabrice Bellard
- ~210 KB binary (vs ~20 MB for V8)
- <300 μs startup time (vs ~50ms for V8)
- ES2020 support
- No JIT compiler (interprets bytecode)

**Why QuickJS?**
- Tiny footprint (fits in linux-wasm initramfs)
- Instant startup (critical for serverless)
- Deterministic (no JIT optimization non-determinism)
- Production-proven (Shopify Functions, millions of executions/day)
- Slower execution (~10-100x vs V8 for CPU-bound code)

### Task Breakdown

#### Task 3.1: QuickJS WASI Build (3 days)

**Objective**: Obtain QuickJS compiled to WASM with WASI support

**Options**:

1. **second-state/quickjs-wasi** (Recommended)
   - Pre-built, production-ready
   - Used by WasmEdge runtime
   - ~1.5-2 MB binary
   - Full ES2020 support

2. **Shopify's Javy**
   - QuickJS + Wizer snapshotting
   - Startup <1ms
   - Pre-compiles JS to bytecode at build time
   - Used in production at scale

3. **Build from source**
   - Clone quickjs + WASI SDK
   - Compile with wasm32-wasi target
   - More control, slower

**Implementation**:

```bash
# Option 1: Download pre-built
wget https://github.com/second-state/quickjs-wasi/releases/download/v0.5.0/quickjs.wasm
cp quickjs.wasm browser-worker/linux-runtime/bin/qjs.wasm

# Option 2: Build from source
git clone https://github.com/second-state/quickjs-wasi
cd quickjs-wasi
make
cp qjs.wasm ../near-outlayer/browser-worker/linux-runtime/bin/
```

**Deliverable**: `browser-worker/linux-runtime/bin/qjs.wasm` (~1.5-2 MB)

#### Task 3.2: QuickJS Executor Class (4 days)

**Objective**: Create JavaScript contract execution layer

**File**: `browser-worker/src/quickjs-executor.js`

```javascript
class QuickJSExecutor {
  constructor(linuxExecutor, options = {}) {
    this.linuxExecutor = linuxExecutor;  // L2 dependency
    this.options = {
      qjsPath: options.qjsPath || '/linux-runtime/bin/qjs.wasm',
      maxMemory: options.maxMemory || 50 * 1024 * 1024,  // 50 MB
      timeout: options.timeout || 30000,  // 30 seconds
    };

    this.stats = {
      totalExecutions: 0,
      totalInstructions: 0,
      averageTime: 0,
    };
  }

  // Execute JavaScript contract
  async executeContract(jsCode, methodName, args, nearState) {
    this.stats.totalExecutions++;

    // Ensure Linux kernel booted
    if (!this.linuxExecutor.kernelReady) {
      await this.linuxExecutor.initialize();
    }

    // Prepare JavaScript wrapper
    const wrappedCode = this.wrapContract(jsCode, methodName, args);

    // Execute QuickJS in linux-wasm
    const result = await this.linuxExecutor.executeProgram(
      await this.loadQuickJS(),
      ['--eval', wrappedCode],
      {
        NEAR_STATE_PATH: '/near/state',
        NEAR_METHOD: methodName,
        NEAR_ARGS: JSON.stringify(args),
      }
    );

    // Parse result
    let contractResult;
    try {
      contractResult = JSON.parse(result.stdout);
    } catch (error) {
      throw new Error(`Contract execution failed: ${error.message}\nStdout: ${result.stdout}\nStderr: ${result.stderr}`);
    }

    this.stats.totalInstructions += result.stats.instructions;

    return {
      result: contractResult,
      gasUsed: result.stats.instructions,
      logs: result.stderr.split('\n').filter(l => l),
      executionTime: result.stats.timeMs,
    };
  }

  // Wrap user contract with NEAR environment
  wrapContract(jsCode, methodName, args) {
    return `
      // NEAR host functions (will be replaced with Frozen Realm in Phase 4)
      const near = {
        storageRead: (key) => {
          // Call NEAR syscall 400 via C bridge
          return nearSyscall(400, key);
        },
        storageWrite: (key, value) => {
          return nearSyscall(401, key, value);
        },
        log: (msg) => {
          console.log('[NEAR]', msg);
        },
      };

      // User contract code
      ${jsCode}

      // Execute method
      const result = ${methodName}(${JSON.stringify(args)});

      // Output result (must be JSON)
      console.log(JSON.stringify(result));
    `;
  }

  async loadQuickJS() {
    // Load QuickJS WASM binary
    const response = await fetch(this.options.qjsPath);
    return new Uint8Array(await response.arrayBuffer());
  }
}
```

**Integration with ContractSimulator**:

```javascript
class ContractSimulator {
  constructor(options = {}) {
    this.options = {
      executionMode: options.executionMode || 'direct',  // 'direct' | 'linux' | 'quickjs'
      // ...
    };

    // L3: QuickJS executor (depends on L2)
    this.quickjsExecutor = null;
    if (this.options.executionMode === 'quickjs') {
      this.linuxExecutor = new LinuxExecutor({ demoMode: true });
      this.quickjsExecutor = new QuickJSExecutor(this.linuxExecutor);
    }
  }

  async execute(source, methodName, args, context) {
    if (this.options.executionMode === 'quickjs') {
      return await this.executeQuickJS(source, methodName, args, context);
    }
    // ... existing direct/linux paths
  }

  async executeQuickJS(jsCode, methodName, args, context) {
    this.stats.quickjsExecutions++;

    return await this.quickjsExecutor.executeContract(
      jsCode,
      methodName,
      args,
      nearState
    );
  }
}
```

**Deliverable**: QuickJS executor class with NEAR integration

#### Task 3.3: NEAR Syscall Bridge (3 days)

**Objective**: Connect QuickJS (L3) to NEAR host functions via Linux syscalls (L2)

**Challenge**: QuickJS runs as POSIX process in linux-wasm. It needs to call NEAR functions (storage, logs, promises).

**Solution**: C bridge library that QuickJS can link against

**File**: `browser-worker/linux-runtime/lib/near-bridge.c`

```c
// NEAR syscall bridge for QuickJS
#include <syscall.h>
#include <stdint.h>
#include <string.h>

// Storage operations
int near_storage_read(const char* key, size_t key_len, uint8_t* value, size_t value_len) {
    return syscall(400, key, key_len, value, value_len);
}

int near_storage_write(const char* key, size_t key_len, const uint8_t* value, size_t value_len) {
    return syscall(401, key, key_len, value, value_len);
}

int near_storage_remove(const char* key, size_t key_len) {
    return syscall(402, key, key_len);
}

// Logging
void near_log(const char* msg) {
    syscall(404, msg, strlen(msg));
}

// Account information
int near_current_account_id(char* buf, size_t buf_len) {
    return syscall(410, buf, buf_len);
}

// ... other NEAR functions
```

**QuickJS bindings**:

```c
// Expose NEAR functions to JavaScript
#include <quickjs.h>

static JSValue js_near_storage_read(JSContext *ctx, JSValueConst this_val, int argc, JSValueConst *argv) {
    const char *key = JS_ToCString(ctx, argv[0]);
    uint8_t value[1024];
    int result = near_storage_read(key, strlen(key), value, sizeof(value));

    if (result > 0) {
        return JS_NewArrayBuffer(ctx, value, result, NULL, NULL, FALSE);
    }
    return JS_NULL;
}

// Register functions
static const JSCFunctionListEntry js_near_funcs[] = {
    JS_CFUNC_DEF("storageRead", 1, js_near_storage_read),
    JS_CFUNC_DEF("storageWrite", 2, js_near_storage_write),
    // ...
};

void js_init_near_module(JSContext *ctx) {
    JSValue near = JS_NewObject(ctx);
    JS_SetPropertyFunctionList(ctx, near, js_near_funcs, countof(js_near_funcs));
    JS_SetPropertyStr(ctx, JS_GetGlobalObject(ctx), "near", near);
}
```

**Deliverable**: C bridge library compiled into QuickJS

#### Task 3.4: Test Suite (2 days)

**Objective**: Verify JavaScript contracts work end-to-end

**Test 1: Simple Counter**

```javascript
// counter.js
let count = 0;

export function increment() {
  // Read current count from NEAR storage
  const stored = near.storageRead('count');
  count = stored ? parseInt(stored) : 0;

  // Increment
  count++;

  // Write back
  near.storageWrite('count', count.toString());

  near.log(`Counter incremented to ${count}`);

  return { count };
}

export function getCount() {
  const stored = near.storageRead('count');
  return { count: stored ? parseInt(stored) : 0 };
}
```

**Test 2: Cross-Contract Call**

```javascript
// defi-strategy.js
export async function executeStrategy(poolId, amount) {
  near.log(`Executing strategy on pool ${poolId}`);

  // Query pool state
  const poolData = await near.promiseCreate(
    'pool.near',
    'get_pool_info',
    { pool_id: poolId },
    0,
    50_000_000_000_000
  );

  // Calculate optimal swap
  const swapAmount = calculateOptimalSwap(poolData, amount);

  // Execute swap
  await near.promiseCreate(
    'pool.near',
    'swap',
    { amount: swapAmount },
    0,
    100_000_000_000_000
  );

  return { swapAmount };
}
```

**Test execution**:

```javascript
// In test.html
async function testQuickJSContract() {
    log('\n⚡ Testing JavaScript contract execution...', 'info');

    // Load contract source
    const response = await fetch('test-contracts/counter.js');
    const jsCode = await response.text();

    // Switch to QuickJS mode
    await simulator.setExecutionMode('quickjs');

    // Execute
    const result = await simulator.execute(jsCode, 'increment', {});

    log(`✓ Result: ${JSON.stringify(result.result)}`, 'success');
    log(`  Gas used: ${result.gasUsed.toLocaleString()}`, 'info');
    log(`  Execution time: ${result.executionTime}ms`, 'info');
}
```

**Deliverable**: 5+ test contracts with automated test suite

#### Task 3.5: Documentation (1 day)

**Files to create**:
- `browser-worker/docs/QUICKJS_CONTRACTS.md` - How to write JS contracts
- `browser-worker/examples/` - Example contracts (counter, NFT, DeFi)
- Update test.html with QuickJS demo section

**Deliverable**: Complete documentation for JavaScript contract developers

### Phase 3 Requirements

- QuickJS runs as POSIX process in linux-wasm
- JavaScript contracts can call NEAR host functions
- Storage operations work (read/write/remove)
- Cross-contract calls work (promise_create/then)
- Test suite passes (5+ contracts)
- Execution time <100ms for simple contracts
- Documentation complete

**Timeline**: 2-3 weeks
**Output**: Working JavaScript contracts on OutLayer

---

## Phase 4: Frozen Realms (SES) Integration (2-3 Weeks)

### Goal

Add L4 deterministic execution layer using Secure ECMAScript (SES) and Frozen Realms pattern.

### SES Background

**What is SES?**
- Subset of JavaScript for secure, deterministic execution
- Agoric's production runtime (blockchain + smart contracts)
- TC39 proposal (Stage 2)
- Key primitives: `lockdown()`, `harden()`, `Compartment`

**Why SES?**
- Eliminates non-determinism (no Date.now, Math.random)
- Prevents prototype poisoning
- Isolates contracts (separate Compartments)
- Enables time-travel debugging
- Required for verifiable computation

### Task Breakdown

#### Task 4.1: SES Shim Build (3 days)

**Objective**: Bundle 'ses' npm package for QuickJS environment

```bash
# Install SES
npm install ses

# Bundle for QuickJS (no module system)
esbuild node_modules/ses/dist/ses.cjs \
  --bundle \
  --format=iife \
  --global-name=SES \
  --outfile=browser-worker/linux-runtime/lib/ses.js

# Minify
terser browser-worker/linux-runtime/lib/ses.js \
  --compress \
  --mangle \
  --output browser-worker/linux-runtime/lib/ses.min.js
```

**Test in QuickJS**:

```bash
# Load SES in QuickJS
qjs --eval "
  load('lib/ses.min.js');
  lockdown();  // Freeze JavaScript intrinsics
  console.log('SES loaded, intrinsics frozen');
"
```

**Deliverable**: `ses.min.js` working in QuickJS

#### Task 4.2: NearFrozenRealm Class (5 days)

**Objective**: Create deterministic execution environment for NEAR contracts

**File**: `browser-worker/src/near-frozen-realm.js`

```javascript
class NearFrozenRealm {
  constructor(nearState, options = {}) {
    this.nearState = nearState;
    this.options = {
      allowMathRandom: options.allowMathRandom || false,
      allowDateNow: options.allowDateNow || false,
      allowFetch: options.allowFetch || false,
    };

    // Load SES (must be loaded in QuickJS context)
    this.initializeSES();

    // Create isolated compartment
    this.compartment = this.createCompartment();
  }

  initializeSES() {
    // Load SES shim
    load('/linux-runtime/lib/ses.min.js');

    // Lock down JavaScript environment
    lockdown({
      errorTaming: 'safe',
      overrideTaming: 'moderate',
      stackFiltering: 'verbose',
    });

    // All intrinsics are now frozen:
    // - Array.prototype
    // - Object.prototype
    // - Function.prototype
    // - etc.
  }

  createCompartment() {
    // Create isolated execution context
    return new Compartment({
      // Endowments: controlled globals
      console: harden({
        log: (...args) => this.nearLog(...args),
        error: (...args) => this.nearLog('[ERROR]', ...args),
      }),

      // NEAR host functions (all hardened)
      near: harden({
        storageRead: (key) => this.nearState.get(key),
        storageWrite: (key, value) => this.nearState.set(key, value),
        storageRemove: (key) => this.nearState.delete(key),
        log: (msg) => this.nearLog(msg),
        currentAccountId: () => 'current.near',
        signerAccountId: () => 'signer.near',
        predecessorAccountId: () => 'predecessor.near',
        blockTimestamp: () => this.getBlockTimestamp(),
        blockHeight: () => this.getBlockHeight(),
        // No Math.random, no Date.now, no fetch
      }),
    });
  }

  // Execute contract in frozen realm
  async execute(contractCode, methodName, args) {
    // Evaluate code in isolated compartment
    const contractModule = this.compartment.evaluate(`
      ${contractCode}

      // Return exports
      ({ ${methodName} });
    `);

    // Call method
    const method = contractModule[methodName];
    if (!method) {
      throw new Error(`Method ${methodName} not found`);
    }

    const result = await method(args);
    return result;
  }

  // Controlled time (from NEAR blockchain)
  getBlockTimestamp() {
    // Use NEAR block timestamp, not Date.now()
    return nearBlockchainTimestamp;
  }

  getBlockHeight() {
    return nearBlockchainHeight;
  }

  nearLog(...args) {
    const msg = args.join(' ');
    near.log(msg);  // Call L2 syscall
  }
}
```

**Integration with QuickJSExecutor**:

```javascript
class QuickJSExecutor {
  async executeContract(jsCode, methodName, args, nearState) {
    // Wrap in Frozen Realm
    const frozenRealmCode = `
      // Load SES
      load('/linux-runtime/lib/ses.min.js');

      // Create frozen realm
      const realm = new NearFrozenRealm(nearState, {
        allowMathRandom: false,  // Deterministic only
        allowDateNow: false,
        allowFetch: false,
      });

      // Execute contract
      const result = await realm.execute(\`${jsCode}\`, '${methodName}', ${JSON.stringify(args)});

      // Output
      console.log(JSON.stringify(result));
    `;

    return await this.linuxExecutor.executeProgram(
      await this.loadQuickJS(),
      ['--eval', frozenRealmCode],
      { NEAR_STATE_PATH: '/near/state' }
    );
  }
}
```

**Deliverable**: NearFrozenRealm class with full SES integration

#### Task 4.3: Determinism Validation (3 days)

**Objective**: Prove same inputs → same outputs

**Test**: Run contract 100 times, verify identical results

```javascript
async function testDeterminism() {
  const contractCode = `
    export function calculate(input) {
      let sum = 0;
      for (let i = 0; i < 1000; i++) {
        sum += i * input;
      }
      return { sum };
    }
  `;

  const results = [];

  for (let i = 0; i < 100; i++) {
    const result = await quickjsExecutor.executeContract(
      contractCode,
      'calculate',
      { input: 42 },
      nearState
    );
    results.push(result.result.sum);
  }

  // Verify all results are identical
  const allSame = results.every(r => r === results[0]);
  console.assert(allSame, 'Determinism test failed');
  console.log(`✓ Determinism test passed: 100/100 runs identical`);
}
```

**Test non-deterministic APIs are blocked**:

```javascript
async function testNonDeterministicBlocked() {
  const tests = [
    { name: 'Math.random()', code: 'export function test() { return Math.random(); }' },
    { name: 'Date.now()', code: 'export function test() { return Date.now(); }' },
    { name: 'fetch()', code: 'export function test() { return fetch("http://example.com"); }' },
  ];

  for (const test of tests) {
    try {
      await quickjsExecutor.executeContract(test.code, 'test', {}, nearState);
      console.error(`✗ ${test.name} should be blocked but wasn't`);
    } catch (error) {
      console.log(`✓ ${test.name} correctly blocked: ${error.message}`);
    }
  }
}
```

**Deliverable**: 100% determinism rate, all non-deterministic APIs blocked

#### Task 4.4: Security Audit (3 days)

**Objective**: Verify Frozen Realm isolation

**Test 1: Prototype poisoning prevention**

```javascript
// Malicious contract tries to poison Array.prototype
const maliciousCode = `
  export function attack() {
    Array.prototype.evil = true;
    return { success: true };
  }
`;

// Execute attack
await quickjsExecutor.executeContract(maliciousCode, 'attack', {}, nearState);

// Execute victim contract
const victimCode = `
  export function checkCompromise() {
    return { compromised: Array.prototype.evil !== undefined };
  }
`;

const result = await quickjsExecutor.executeContract(victimCode, 'checkCompromise', {}, nearState);

console.assert(!result.result.compromised, 'Prototype poisoning succeeded (FAILURE)');
console.log('✓ Prototype poisoning blocked');
```

**Test 2: Compartment isolation**

```javascript
// Contract A sets global
const contractA = `
  globalThis.secretData = 'password123';
  export function setSecret() { return {}; }
`;

// Contract B tries to read
const contractB = `
  export function stealSecret() {
    return { stolen: globalThis.secretData };
  }
`;

await quickjsExecutor.executeContract(contractA, 'setSecret', {}, nearState);
const result = await quickjsExecutor.executeContract(contractB, 'stealSecret', {}, nearState);

console.assert(result.result.stolen === undefined, 'Compartment isolation failed');
console.log('✓ Compartment isolation working');
```

**Deliverable**: Security audit report, all tests passing

#### Task 4.5: Documentation (2 days)

**Files to create**:
- `browser-worker/docs/FROZEN_REALMS.md` - SES architecture, determinism guarantees
- `browser-worker/docs/WRITING_DETERMINISTIC_CONTRACTS.md` - Best practices
- Update examples with deterministic patterns

**Deliverable**: Complete deterministic contract developer guide

### Phase 4 Requirements

- SES integrated with QuickJS
- Frozen Realms isolate contracts
- 100% determinism (100 runs → identical results)
- Math.random, Date.now, fetch blocked
- Prototype poisoning prevented
- Compartment isolation verified
- Security audit passed
- Documentation complete

**Timeline**: 2-3 weeks
**Output**: Deterministic, auditable JavaScript contracts

---

## Phase 5: Production Linux Kernel (3-4 Weeks)

### Goal

Transition from demo mode to real vmlinux.wasm kernel with full POSIX support.

### Task Breakdown

#### Task 5.1: Build vmlinux.wasm (5 days)
#### Task 5.2: NEAR Syscall Kernel Patch (5 days)
#### Task 5.3: Worker Integration (4 days)
#### Task 5.4: Performance Optimization (4 days)
#### Task 5.5: CDN Distribution (3 days)

*(Detailed breakdown in Chapter 2: Linux/WASM Integration)*

### Phase 5 Requirements

- Production kernel running
- 2-3x overhead vs direct WASM (target met)
- SharedArrayBuffer working (with COOP/COEP)
- All test contracts pass
- Performance benchmarks documented

**Timeline**: 3-4 weeks
**Output**: Production-grade Linux execution

---

## Phase 6: Daring Applications MVP (4-6 Weeks)

### Goal

Ship one flagship application demonstrating OutLayer's unique capabilities.

### Three Options

**Option A: AI Trading Agent** (6 weeks)
- TensorFlow.js integration
- Deterministic ML inference
- Verifiable trading logic
- Risk management
- Testnet deployment

**Option B: Plugin System** (5 weeks)
- Plugin SDK
- DEX integration
- Plugin marketplace
- Security audit
- Mainnet launch

**Option C: Stateful Edge Computing** (6 weeks)
- Multi-process POSIX workflows
- Database integration
- Complex pipelines
- Edge deployment
- Performance benchmarks

*(Detailed breakdown in Chapter 7: Daring Applications)*

### Phase 6 Requirements

- One production application on mainnet
- Developer onboarding guide
- Case study published
- Community adoption metrics

**Timeline**: 4-6 weeks
**Output**: Production application + ecosystem growth

---

## Cumulative Timeline

| Phase | Duration | Dependencies | Status |
|-------|----------|--------------|--------|
| Phase 1: RPC Throttling | 1 day | None | Complete |
| Phase 2: Linux/WASM | 5 days | Phase 1 | Complete |
| Phase 3: QuickJS | 2-3 weeks | Phase 2 | Planned |
| Phase 4: Frozen Realms | 2-3 weeks | Phase 3 | Planned |
| Phase 5: Production Kernel | 3-4 weeks | Phase 2 | Planned |
| Phase 6: Daring App | 4-6 weeks | Phase 3-5 | Planned |

**Total**: 3-6 months for complete 4-layer stack

**Phases 3-4 can run in parallel with Phase 5** (QuickJS + SES don't require production kernel)

---

## Strategic Positioning

### Competitive Advantages

**vs Ethereum**:
- JavaScript contracts (15M developers vs 500K Rust)
- No compilation step (instant deployment)
- Full POSIX (Ethereum: EVM only)
- Deterministic execution (Ethereum: gas-based non-determinism)

**vs Solana**:
- JavaScript (Solana: Rust only)
- Browser execution (Solana: validators only)
- Stateful workflows (Solana: stateless transactions)

**vs Cosmos**:
- Single-chain integration (Cosmos: IBC complexity)
- Browser-native (Cosmos: server infrastructure)

**vs NEAR direct contracts**:
- JavaScript vs Rust (lower barrier)
- Dynamic code execution (load from IPFS)
- Verifiable computation (Frozen Realms)
- Slower execution (~10-100x) - acceptable for use cases where dynamism > speed

### Market Positioning

**"NEAR OutLayer: The Programmable Offshore Zone"**

**Tagline**: Move computation off-chain, keep security on-chain

**Value Propositions**:
1. **For JavaScript Developers**: Write contracts in JavaScript, deploy instantly
2. **For AI/ML Applications**: Verifiable, deterministic inference on-chain
3. **For DeFi Protocols**: Safe, auditable plugin systems
4. **For Edge Computing**: Full POSIX FaaS with millisecond startup

---

## Risk Mitigation

### Technical Risks

**Risk 1: Performance**
- **Impact**: QuickJS 10-100x slower than native WASM
- **Mitigation**: Use for logic-heavy, not computation-heavy contracts
- **Fallback**: Hybrid mode (critical path in Rust, business logic in JS)

**Risk 2: SharedArrayBuffer Support**
- **Impact**: Safari, mobile browsers lack COOP/COEP support
- **Mitigation**: Demo mode works everywhere, production mode for power users
- **Fallback**: Server-side execution for unsupported browsers

**Risk 3: Bundle Size**
- **Impact**: vmlinux.wasm ~24 MB initial download
- **Mitigation**: CDN, lazy loading, persistent cache
- **Fallback**: Demo mode (no download) for quick testing

### Go/No-Go Decision Points

**After Phase 3** (QuickJS):
- Go if: JavaScript contracts execute <100ms
- No-go if: Performance unacceptable, pivot to WASM-only

**After Phase 4** (Frozen Realms):
- Go if: 100% determinism, security audit passed
- No-go if: Determinism broken, security issues

**After Phase 5** (Production Kernel):
- Go if: 2-5x overhead vs direct WASM
- No-go if: >10x overhead, unacceptable for production

---

## Success Metrics

### Phase 3 (QuickJS)
- [ ] JavaScript contracts execute successfully
- [ ] <100ms execution time for simple contracts
- [ ] NEAR host functions working
- [ ] 5+ test contracts passing

### Phase 4 (Frozen Realms)
- [ ] 100% determinism (100 runs identical)
- [ ] All non-deterministic APIs blocked
- [ ] Security audit passed
- [ ] Prototype poisoning prevented

### Phase 5 (Production Kernel)
- [ ] 2-5x overhead vs direct WASM
- [ ] Boot time <30s first load, <1s cached
- [ ] All Phase 3-4 tests pass in production mode
- [ ] Performance benchmarks documented

### Phase 6 (Daring App)
- [ ] One production app on mainnet
- [ ] 10+ users/week
- [ ] Case study published
- [ ] Developer adoption metrics

---

## Related Documentation

- **Current state**: [Chapter 2: Linux/WASM Integration](02-linux-wasm-integration.md)
- **Deep dive**: [Chapter 6: 4-Layer Architecture](04-layer-architecture.md)
- **Applications**: [Chapter 7: Daring Applications](07-daring-applications.md)
- **IIFE patterns**: [Chapter 4: IIFE Bundling](04-iife-bundling.md)
- **Benchmarking**: [Chapter 5: Performance Benchmarking](05-performance-benchmarking.md)
