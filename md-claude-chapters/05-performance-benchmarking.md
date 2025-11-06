# Chapter 5: Performance Benchmarking - Measurement Framework

**Phase**: Documentation Framework (Awaiting Production Benchmarking)
**Status**: Methodology Complete, Real Data TBD

---

## Overview

This chapter establishes a comprehensive benchmarking framework for NEAR OutLayer, defining methodology, metrics, and tooling for measuring performance across execution modes. While production benchmarking awaits Linux mode completion, this framework provides projected targets and measurement approaches.

### Why Benchmark?

1. **Quantify overhead**: Understand cost of each abstraction layer
2. **Identify bottlenecks**: Find optimization opportunities
3. **Validate scaling**: Ensure system handles production load
4. **Guide architecture**: Make data-driven decisions (direct vs Linux mode)

### Key Performance Dimensions

- **Latency**: Request-to-response time (ms)
- **Throughput**: Requests per second (rps)
- **Memory**: RAM usage patterns (MB)
- **Overhead**: Performance impact vs baseline (multiplier)

---

## Success Criteria

### Target Performance Matrix

| Metric | Target | Acceptable | Notes |
|--------|--------|-----------|--------|
| **Direct WASM execution** | < 10ms | < 50ms | Contract instantiation + call |
| **Linux mode overhead** | 2-3x direct | 5x direct | Includes syscall translation |
| **RPC proxy latency** | < 20ms | < 100ms | Coordinator throttle check |
| **Throttle check time** | < 1ms | < 5ms | Token bucket operation |
| **Memory per contract** | < 5MB | < 20MB | Loaded WASM instance |
| **Concurrent requests** | 100+ rps | 50+ rps | Per coordinator instance |
| **Secrets decryption** | < 100ms | < 200ms | Keystore roundtrip |

### Comparison Baseline

**Raw WebAssembly** (browser API, no OutLayer):
```
Instantiation:  ~3-5ms (first time)
Instantiation:  ~0.5ms (cached module)
Function call:  ~0.01ms (simple operation)
```

**NEAR RPC** (direct, no proxy):
```
Testnet:  ~100-300ms (network latency)
Mainnet:  ~50-150ms (depending on location)
```

---

## Test Scenarios

### Scenario 1: Simple Counter Contract

**Purpose**: Measure baseline WASM execution performance

**Contract**: Increment/decrement counter (minimal state)

**Test cases**:
1. Single execution (cold start)
2. Single execution (warm - module cached)
3. Sequential executions (100 calls)
4. Parallel executions (10 concurrent)

**Expected results**:
```
Cold start (direct):    ~5-10ms
Warm execution:         ~0.5-1ms
Throughput:            ~1000 ops/sec
```

### Scenario 2: Complex NFT Minting

**Purpose**: Test performance with larger state operations

**Contract**: Mint NFT with metadata storage

**Test cases**:
1. Mint single NFT
2. Batch mint 10 NFTs
3. Query NFT metadata (read-heavy)

**Expected results**:
```
Single mint:           ~20-30ms
Batch mint (10):       ~150-200ms
Metadata query:        ~5-10ms
```

### Scenario 3: Encrypted Secrets Access

**Purpose**: Measure secrets decryption overhead

**Flow**: Request → Decrypt secrets → Inject env vars → Execute

**Test cases**:
1. Execution with secrets (full flow)
2. Execution without secrets (baseline)
3. Large secrets payload (10+ keys)

**Expected overhead**:
```
Secrets decryption:    ~50-100ms (network + crypto)
Env var injection:     ~1-2ms
Total overhead:        ~60-120ms
```

### Scenario 4: RPC Throttling Burst

**Purpose**: Validate rate limiting under load

**Setup**: Anonymous client (5 rps limit, 10 burst)

**Test**: Send 50 requests in 1 second

**Expected behavior**:
```
Initial burst (10):    Succeed immediately
Requests 11-50:        Throttled, auto-retry
Total time:            ~10 seconds (5 rps sustained)
429 rate:              ~80% (40/50 requests)
Final success rate:    100% (with retries)
```

### Scenario 5: Linux Mode vs Direct Mode

**Purpose**: Quantify Linux execution overhead

**Test cases**:
- Same contract in both modes
- Multiple complexity levels (simple, medium, complex)
- Instruction count accuracy comparison

**Expected overhead**:
```
Simple contract:       2x slower
Medium contract:       2.5x slower
Complex contract:      3x slower
(Production mode; demo mode simulates ~10x)
```

---

## Methodology

### Benchmarking Approach

**Execution pattern**:
1. **Warm-up**: Run 10 iterations to populate caches
2. **Measurement**: Run 100 iterations for statistical significance
3. **Cooldown**: Wait 5 seconds between test suites
4. **Repetition**: Repeat entire suite 3 times, average results

### Benchmark Helper Class

```javascript
class Benchmark {
  constructor(name) {
    this.name = name;
    this.samples = [];
  }

  async run(fn, iterations = 100) {
    // Warm-up
    for (let i = 0; i < 10; i++) {
      await fn();
    }

    // Measurement
    for (let i = 0; i < iterations; i++) {
      const start = performance.now();
      await fn();
      const duration = performance.now() - start;
      this.samples.push(duration);
    }

    return this.getStats();
  }

  getStats() {
    const sorted = this.samples.slice().sort((a, b) => a - b);
    const sum = sorted.reduce((a, b) => a + b, 0);

    return {
      mean: sum / sorted.length,
      median: sorted[Math.floor(sorted.length / 2)],
      p95: sorted[Math.floor(sorted.length * 0.95)],
      p99: sorted[Math.floor(sorted.length * 0.99)],
      min: sorted[0],
      max: sorted[sorted.length - 1],
      samples: sorted.length,
    };
  }

  report() {
    const stats = this.getStats();
    console.log(`\n${this.name}:`);
    console.log(`  Mean:   ${stats.mean.toFixed(2)}ms`);
    console.log(`  Median: ${stats.median.toFixed(2)}ms`);
    console.log(`  P95:    ${stats.p95.toFixed(2)}ms`);
    console.log(`  P99:    ${stats.p99.toFixed(2)}ms`);
    console.log(`  Min:    ${stats.min.toFixed(2)}ms`);
    console.log(`  Max:    ${stats.max.toFixed(2)}ms`);
  }
}
```

**Usage**:
```javascript
const bench = new Benchmark('Contract Execution');
await bench.run(async () => {
  await simulator.execute('counter.wasm', 'increment', {});
}, 100);
bench.report();
```

---

## Direct WASM Execution Benchmarks

### Test: Contract Instantiation

**Measures**: Overhead of ContractSimulator.execute() vs raw WebAssembly

```javascript
async function benchmarkContractInstantiation() {
  const simulator = new ContractSimulator({
    verboseLogging: false,
    executionMode: 'direct',
  });

  const bench = new Benchmark('Contract Instantiation (Direct Mode)');

  await bench.run(async () => {
    await simulator.execute(
      'test-contracts/counter/res/counter.wasm',
      'increment',
      {}
    );
  });

  bench.report();
}

// Expected results:
// Mean:   ~5-10ms
// P95:    ~15ms
// Overhead vs baseline: ~2-3x (baseline ~3-5ms)
```

### Test: Sequential Executions (Cache Effectiveness)

```javascript
async function benchmarkSequentialExecutions() {
  const simulator = new ContractSimulator();

  // First call (cold)
  const coldStart = performance.now();
  await simulator.execute('counter.wasm', 'increment', {});
  const coldTime = performance.now() - coldStart;

  console.log(`Cold start: ${coldTime.toFixed(2)}ms`);

  // Warm calls
  const bench = new Benchmark('Sequential Executions (Warm)');

  await bench.run(async () => {
    await simulator.execute('counter.wasm', 'increment', {});
  }, 1000);

  bench.report();
}

// Expected results:
// Cold start:        ~5-10ms
// Warm mean:         ~0.5-1ms (cached module)
// Improvement:       ~10x faster
```

### Test: Parallel Executions (Concurrency)

```javascript
async function benchmarkParallelExecutions() {
  const simulator = new ContractSimulator();
  await simulator.execute('counter.wasm', 'increment', {});  // Warm up

  const concurrency = 10;
  const iterations = 100;
  const start = performance.now();

  for (let batch = 0; batch < iterations / concurrency; batch++) {
    const promises = [];
    for (let i = 0; i < concurrency; i++) {
      promises.push(
        simulator.execute('counter.wasm', 'increment', {})
      );
    }
    await Promise.all(promises);
  }

  const duration = performance.now() - start;
  const throughput = iterations / (duration / 1000);

  console.log(`\nParallel Executions (${concurrency} concurrent):`);
  console.log(`  Total time:  ${duration.toFixed(2)}ms`);
  console.log(`  Throughput:  ${throughput.toFixed(0)} ops/sec`);
  console.log(`  Avg latency: ${(duration / iterations).toFixed(2)}ms`);
}

// Expected results:
// Throughput:  ~500-1000 ops/sec (depends on CPU cores)
// Avg latency: ~5-10ms (parallel overhead)
```

---

## Linux Mode Execution Benchmarks

### Test: Demo Mode Performance

**Current state**: Simulated Linux with artificial delays

```javascript
async function benchmarkLinuxDemoMode() {
  const simulator = new ContractSimulator({
    executionMode: 'linux',
  });

  await simulator.setExecutionMode('linux');

  const bench = new Benchmark('Linux Demo Mode Execution');

  await bench.run(async () => {
    await simulator.execute('counter.wasm', 'increment', {});
  }, 50);  // Fewer iterations (slower)

  bench.report();
}

// Expected results (simulated):
// Mean:    ~100-150ms (includes simulated 100ms delay)
// Overhead: ~10x vs direct mode (demo simulation)
```

### Test: Production Mode Performance (Future)

**When production Linux is ready**:

```javascript
async function benchmarkLinuxProductionMode() {
  const simulator = new ContractSimulator({
    executionMode: 'linux',
    demoMode: false,  // Real kernel
  });

  // Wait for kernel boot
  await simulator.setExecutionMode('linux');

  // Cold start test
  const coldStart = performance.now();
  await simulator.execute('counter.wasm', 'increment', {});
  const coldTime = performance.now() - coldStart;

  console.log(`Linux cold start: ${coldTime.toFixed(2)}ms`);

  // Warm executions
  const bench = new Benchmark('Linux Production Mode (Warm)');

  await bench.run(async () => {
    await simulator.execute('counter.wasm', 'increment', {});
  });

  bench.report();

  // Linux statistics
  const stats = simulator.getLinuxStats();
  console.log('\nLinux Statistics:', stats);
}

// Expected results (production):
// Cold start:      ~50-100ms (task worker creation)
// Warm mean:       ~2-5ms (2-3x overhead vs direct)
// Syscall overhead: ~0.1ms per NEAR function call
```

### Test: Instruction Counting Accuracy

**Compare reported instruction counts between modes**:

```javascript
async function benchmarkInstructionCounting() {
  const simulator = new ContractSimulator();

  // Direct mode
  await simulator.setExecutionMode('direct');
  const directResult = await simulator.execute('counter.wasm', 'increment', {});
  const directInstructions = directResult.gasUsed;

  // Linux mode
  await simulator.setExecutionMode('linux');
  const linuxResult = await simulator.execute('counter.wasm', 'increment', {});
  const linuxInstructions = linuxResult.gasUsed;

  console.log('\nInstruction Counting Comparison:');
  console.log(`  Direct mode: ${directInstructions.toLocaleString()} instructions`);
  console.log(`  Linux mode:  ${linuxInstructions.toLocaleString()} instructions`);
  console.log(`  Difference:  ${Math.abs(directInstructions - linuxInstructions)} (${((Math.abs(directInstructions - linuxInstructions) / directInstructions) * 100).toFixed(2)}%)`);
}

// Expected results:
// Direct mode:  ~1,000,000 instructions (wasmi fuel)
// Linux mode:   ~1,050,000 instructions (includes syscall overhead)
// Difference:   ~5% (syscall translation adds instructions)
```

---

## RPC Throttling Performance

### Test: Throttle Middleware Overhead

**Measures**: Latency added by token bucket check

```javascript
async function benchmarkThrottleOverhead() {
  // Test 1: Direct RPC (no coordinator)
  const directBench = new Benchmark('Direct NEAR RPC');
  await directBench.run(async () => {
    await fetch('https://rpc.testnet.near.org', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({
        jsonrpc: '2.0',
        id: 'test',
        method: 'status',
        params: [],
      }),
    });
  }, 50);

  // Test 2: Proxied RPC (with throttling)
  const rpcClient = new RPCClient({
    coordinatorUrl: 'http://localhost:8080',
  });

  const proxiedBench = new Benchmark('Proxied RPC (with throttle)');
  await proxiedBench.run(async () => {
    await rpcClient.getStatus();
  }, 50);

  // Report
  directBench.report();
  proxiedBench.report();

  const directMean = directBench.getStats().mean;
  const proxiedMean = proxiedBench.getStats().mean;
  const overhead = proxiedMean - directMean;

  console.log(`\nThrottle Overhead: ${overhead.toFixed(2)}ms (~${((overhead / directMean) * 100).toFixed(1)}%)`);
}

// Expected results:
// Direct RPC:      ~100-300ms (network latency)
// Proxied RPC:     ~105-310ms
// Overhead:        ~5-10ms (~3-5% of total time)
```

### Test: Rate Limit Enforcement

**Verify 5 rps limit for anonymous users**:

```javascript
async function benchmarkRateLimitEnforcement() {
  const rpcClient = new RPCClient({
    coordinatorUrl: 'http://localhost:8080',
    // No API key = anonymous (5 rps)
  });

  const requestCount = 20;
  const start = performance.now();

  // Fire 20 requests rapidly
  const promises = [];
  for (let i = 0; i < requestCount; i++) {
    promises.push(
      rpcClient.getStatus()
        .then(() => ({ success: true, attempt: i }))
        .catch((err) => ({ success: false, attempt: i, error: err.message }))
    );
  }

  const results = await Promise.all(promises);
  const duration = performance.now() - start;

  const successful = results.filter(r => r.success).length;
  const failed = results.filter(r => !r.success).length;

  console.log('\nRate Limit Enforcement:');
  console.log(`  Total requests:   ${requestCount}`);
  console.log(`  Successful:       ${successful}`);
  console.log(`  Rate limited:     ${failed}`);
  console.log(`  Total time:       ${duration.toFixed(0)}ms`);
  console.log(`  Expected time:    ~${(requestCount / 5) * 1000}ms (at 5 rps)`);
  console.log(`  Effective rate:   ${(requestCount / (duration / 1000)).toFixed(1)} rps`);
}

// Expected results (with auto-retry):
// Total requests:   20
// Successful:       20 (all eventually succeed)
// Initial failures: ~10-15 (burst capacity: 10)
// Total time:       ~4000ms (20 / 5 rps)
// Effective rate:   5 rps (enforced limit)
```

### Test: API Key Tier Performance

**Compare anonymous (5 rps) vs keyed (20 rps)**:

```javascript
async function benchmarkAPIKeyTiers() {
  const anonClient = new RPCClient({
    coordinatorUrl: 'http://localhost:8080',
  });

  const keyedClient = new RPCClient({
    coordinatorUrl: 'http://localhost:8080',
    apiKey: 'test-api-key-123',
  });

  const requestCount = 50;

  // Test anonymous
  console.log('Testing anonymous tier (5 rps)...');
  const anonStart = performance.now();
  for (let i = 0; i < requestCount; i++) {
    await anonClient.getStatus();
  }
  const anonDuration = performance.now() - anonStart;

  // Test keyed
  console.log('Testing keyed tier (20 rps)...');
  const keyedStart = performance.now();
  for (let i = 0; i < requestCount; i++) {
    await keyedClient.getStatus();
  }
  const keyedDuration = performance.now() - keyedStart;

  console.log('\nAPI Key Tier Comparison:');
  console.log(`  Anonymous (5 rps):   ${anonDuration.toFixed(0)}ms`);
  console.log(`  Keyed (20 rps):      ${keyedDuration.toFixed(0)}ms`);
  console.log(`  Speedup:             ${(anonDuration / keyedDuration).toFixed(1)}x`);
}

// Expected results:
// Anonymous:   ~10,000ms (50 / 5 rps)
// Keyed:       ~2,500ms (50 / 20 rps)
// Speedup:     4x (matches rate limit ratio)
```

---

## Memory Profiling

### Test: Heap Usage Over Time

**Monitor memory growth during executions**:

```javascript
async function profileMemoryUsage() {
  if (!performance.memory) {
    console.warn('performance.memory not available (Chrome only)');
    return;
  }

  const simulator = new ContractSimulator();
  const samples = [];

  // Baseline
  samples.push({
    iteration: 0,
    heapUsed: performance.memory.usedJSHeapSize,
    heapTotal: performance.memory.totalJSHeapSize,
  });

  // Execute 100 times
  for (let i = 1; i <= 100; i++) {
    await simulator.execute('counter.wasm', 'increment', {});

    if (i % 10 === 0) {
      samples.push({
        iteration: i,
        heapUsed: performance.memory.usedJSHeapSize,
        heapTotal: performance.memory.totalJSHeapSize,
      });
    }
  }

  // Report
  console.log('\nMemory Usage Profile:');
  console.log('Iteration | Heap Used | Heap Total');
  console.log('----------|-----------|------------');

  samples.forEach(s => {
    console.log(
      `${String(s.iteration).padStart(9)} | ` +
      `${(s.heapUsed / 1024 / 1024).toFixed(2).padStart(9)} MB | ` +
      `${(s.heapTotal / 1024 / 1024).toFixed(2).padStart(10)} MB`
    );
  });

  const baseline = samples[0].heapUsed;
  const final = samples[samples.length - 1].heapUsed;
  const growth = final - baseline;

  console.log(`\nMemory Growth: ${(growth / 1024 / 1024).toFixed(2)} MB (${((growth / baseline) * 100).toFixed(1)}%)`);
}

// Expected results:
// Baseline:     ~10-20 MB (page + libraries)
// After 100:    ~12-25 MB
// Growth:       ~2-5 MB (cached modules + state)
// Growth rate:  Sublinear (cache stabilizes)
```

### Test: Memory Leak Detection

**Long-running session test**:

```javascript
async function detectMemoryLeaks() {
  if (!performance.memory) {
    console.warn('performance.memory not available');
    return;
  }

  const simulator = new ContractSimulator();

  // Force garbage collection (if available)
  if (global.gc) {
    global.gc();
  }

  const baseline = performance.memory.usedJSHeapSize;

  // Run 1000 executions
  for (let i = 0; i < 1000; i++) {
    await simulator.execute('counter.wasm', 'increment', {});

    // Periodic GC
    if (i % 100 === 0 && global.gc) {
      global.gc();
    }
  }

  // Final GC
  if (global.gc) {
    global.gc();
  }

  const final = performance.memory.usedJSHeapSize;
  const leaked = final - baseline;

  console.log('\nMemory Leak Detection:');
  console.log(`  Baseline:  ${(baseline / 1024 / 1024).toFixed(2)} MB`);
  console.log(`  Final:     ${(final / 1024 / 1024).toFixed(2)} MB`);
  console.log(`  Leaked:    ${(leaked / 1024 / 1024).toFixed(2)} MB`);
  console.log(`  Per exec:  ${(leaked / 1000).toFixed(0)} bytes`);

  if (leaked / baseline > 0.1) {
    console.warn('⚠️  Potential memory leak detected (>10% growth)');
  } else {
    console.log('✓ No significant memory leak detected');
  }
}

// Expected results:
// Leaked:     < 5 MB (acceptable for 1000 executions)
// Per exec:   < 5 KB (minimal retention)
// Status:     No leak
```

---

## Stress Testing

### Test: Sustained Load

**Run at capacity for extended period**:

```javascript
async function stressTestSustainedLoad() {
  const simulator = new ContractSimulator();
  const duration = 60000;  // 1 minute
  const targetRPS = 100;

  const startTime = Date.now();
  let executions = 0;
  let errors = 0;

  console.log(`Starting stress test: ${targetRPS} rps for ${duration / 1000}s...`);

  const interval = setInterval(async () => {
    try {
      await simulator.execute('counter.wasm', 'increment', {});
      executions++;
    } catch (error) {
      errors++;
    }
  }, 1000 / targetRPS);

  // Wait for duration
  await new Promise(resolve => setTimeout(resolve, duration));
  clearInterval(interval);

  const elapsed = Date.now() - startTime;
  const actualRPS = executions / (elapsed / 1000);

  console.log('\nStress Test Results:');
  console.log(`  Duration:     ${elapsed}ms`);
  console.log(`  Executions:   ${executions}`);
  console.log(`  Errors:       ${errors}`);
  console.log(`  Target RPS:   ${targetRPS}`);
  console.log(`  Actual RPS:   ${actualRPS.toFixed(1)}`);
  console.log(`  Success rate: ${((executions / (executions + errors)) * 100).toFixed(1)}%`);
}

// Expected results:
// Executions:   ~6000 (60s × 100 rps)
// Errors:       < 1%
// Actual RPS:   ~95-100 (close to target)
// Success rate: > 99%
```

### Test: Burst Capacity

**Massive burst, measure recovery**:

```javascript
async function stressTestBurstCapacity() {
  const rpcClient = new RPCClient({
    coordinatorUrl: 'http://localhost:8080',
  });

  const burstSize = 200;

  console.log(`Sending burst of ${burstSize} requests...`);

  const start = performance.now();

  const promises = Array(burstSize)
    .fill()
    .map((_, i) =>
      rpcClient.getStatus()
        .then(() => ({ index: i, success: true }))
        .catch(err => ({ index: i, success: false, error: err.message }))
    );

  const results = await Promise.all(promises);
  const duration = performance.now() - start;

  const successful = results.filter(r => r.success).length;
  const failed = results.filter(r => !r.success).length;

  console.log('\nBurst Test Results:');
  console.log(`  Burst size:        ${burstSize}`);
  console.log(`  Successful:        ${successful}`);
  console.log(`  Failed:            ${failed}`);
  console.log(`  Duration:          ${duration.toFixed(0)}ms`);
  console.log(`  Effective RPS:     ${(burstSize / (duration / 1000)).toFixed(1)}`);
  console.log(`  Success rate:      ${((successful / burstSize) * 100).toFixed(1)}%`);
}

// Expected results (anonymous, 5 rps, auto-retry):
// Burst size:      200
// Successful:      200 (all eventually succeed)
// Duration:        ~40,000ms (200 / 5 rps)
// Effective RPS:   5 (throttled to limit)
// Success rate:    100% (retry logic works)
```

---

## Performance Comparison Matrix

### Complete Summary Table

| Metric | Direct Mode | Linux Mode (Demo) | Linux Mode (Prod) | Target |
|--------|------------|-------------------|-------------------|--------|
| **Latency** |
| Cold start | 5-10ms | 100-150ms | 50-100ms | < 50ms |
| Warm execution | 0.5-1ms | 100-120ms | 2-5ms | < 10ms |
| With secrets | 60-120ms | 160-220ms | 110-150ms | < 200ms |
| **Throughput** |
| Single thread | 500-1000 ops/s | 8-10 ops/s | 200-500 ops/s | > 100 ops/s |
| 10 concurrent | 1000-2000 ops/s | 80-100 ops/s | 500-1000 ops/s | > 500 ops/s |
| **Memory** |
| Instance size | 2-5 MB | 10-15 MB | 25-35 MB | < 50 MB |
| Growth (1000 exec) | 2-5 MB | 5-10 MB | 5-10 MB | < 20 MB |
| **Overhead** |
| vs baseline WASM | 2-3x | 100x | 4-6x | < 5x |
| RPC throttle | +5-10ms | +5-10ms | +5-10ms | < 20ms |
| Secrets decrypt | +60-100ms | +60-100ms | +60-100ms | < 150ms |

### Mode Selection Guide

**Direct Mode**:
- ✅ Fastest execution (2-3x overhead vs raw WASM)
- ✅ Lowest memory usage (~2-5 MB)
- ✅ Best for simple contracts
- ❌ Limited syscall support (only NEAR host functions)
- **Use when**: Speed is critical, contract needs only NEAR APIs

**Linux Mode (Production)**:
- ✅ Full POSIX syscall support
- ✅ Real instruction counting (fuel metering)
- ✅ Multi-process execution (fork, pipes, workers)
- ⚠️ 4-6x overhead vs direct (acceptable for complex workloads)
- ⚠️ Higher memory usage (~30 MB kernel + workers)
- **Use when**: Need full OS capabilities, complex computation

**Linux Mode (Demo)**:
- ⚠️ Simulated performance (not representative)
- ✅ Good for architecture testing
- ❌ ~100x slower due to artificial delays
- **Use only for development**

---

## Benchmarking Tools

### Browser Tools

**Chrome DevTools Performance Tab**:
1. Open DevTools (F12)
2. Performance tab
3. Click Record
4. Run benchmarks
5. Stop recording
6. Analyze flame graph

**Performance API**:
```javascript
// High-resolution timing
const start = performance.now();
await operation();
const duration = performance.now() - start;

// Memory (Chrome only)
console.log(performance.memory.usedJSHeapSize);

// Mark and measure
performance.mark('start');
await operation();
performance.mark('end');
performance.measure('operation', 'start', 'end');
```

### Command-Line Tools

**Artillery (load testing)**:
```yaml
# artillery.yml
config:
  target: "http://localhost:8080"
  phases:
    - duration: 60
      arrivalRate: 10

scenarios:
  - name: "RPC Throttling"
    flow:
      - post:
          url: "/near-rpc"
          json:
            jsonrpc: "2.0"
            id: "test"
            method: "status"
            params: []
```

Run:
```bash
npm install -g artillery
artillery run artillery.yml
```

**k6 (performance testing)**:
```javascript
// k6-script.js
import http from 'k6/http';
import { check } from 'k6';

export const options = {
  vus: 10,
  duration: '30s',
};

export default function () {
  const res = http.post('http://localhost:8080/near-rpc', JSON.stringify({
    jsonrpc: '2.0',
    id: 'test',
    method: 'status',
    params: [],
  }), {
    headers: { 'Content-Type': 'application/json' },
  });

  check(res, {
    'status is 200': (r) => r.status === 200,
    'response time < 500ms': (r) => r.timings.duration < 500,
  });
}
```

Run:
```bash
k6 run k6-script.js
```

---

## Running Benchmarks

### Prerequisites

1. **Start coordinator**:
```bash
cd coordinator
cargo run --release
```

2. **Start keystore** (if testing secrets):
```bash
cd keystore-worker
python app.py
```

3. **Serve browser-worker**:
```bash
cd browser-worker
python -m http.server 8000
```

4. **Open test page**:
```
http://localhost:8000/test.html
```

### Manual Benchmarks (Browser Console)

```javascript
// Load benchmark helper
const bench = new Benchmark('My Test');

// Run benchmark
await bench.run(async () => {
  await simulator.execute('counter.wasm', 'increment', {});
}, 100);

// View results
bench.report();
```

### Automated Benchmarks (Node.js)

```bash
# Run full suite
node browser-worker/benchmarks/suite.js

# Run specific benchmark
node browser-worker/benchmarks/direct-mode.js

# Run with profiling
node --expose-gc browser-worker/benchmarks/memory-leak.js
```

---

## Key Takeaways

1. **Methodology is established**: Warm-up, measurement, statistical analysis (p50/p95/p99)
2. **Metrics are defined**: Latency, throughput, memory, overhead targets
3. **Tools are documented**: Browser APIs, CLI tools (artillery, k6), custom suite
4. **Baselines are projected**: Expected performance for direct vs Linux modes

### Next Steps (When Production Ready)

1. Implement production Linux mode (disable demo mode)
2. Run full benchmark suite
3. Compare results with projections in this chapter
4. Optimize bottlenecks identified
5. Update this document with real measurements

### Competitive Context

**OutLayer vs Alternatives**:
- **Direct mode**: 2-3x overhead → competitive with other WASM runtimes
- **Linux mode**: 4-6x overhead → still faster than full VM solutions (10-20x)
- **RPC throttling**: <10ms overhead → negligible impact on user experience

**Status**: Framework complete, awaiting production benchmarking

---

**Related Documentation**:
- [Chapter 2: Linux/WASM Integration](02-linux-wasm-integration.md) - Understanding execution modes
- [Chapter 3: Multi-Layer Roadmap](03-multi-layer-roadmap.md) - When to optimize each layer
- Full reference: `browser-worker/docs/PERFORMANCE_BENCHMARKING.md`
