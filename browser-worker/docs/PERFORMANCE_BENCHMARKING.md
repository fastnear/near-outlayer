# Performance Benchmarking Guide - NEAR OutLayer

**Author**: OutLayer Team
**Date**: 2025-11-05
**Status**: Documentation for Future Benchmarking

---

## Table of Contents

1. [Overview](#overview)
2. [Benchmarking Objectives](#benchmarking-objectives)
3. [Test Scenarios](#test-scenarios)
4. [Methodology](#methodology)
5. [Baseline Measurements](#baseline-measurements)
6. [Direct WASM Execution](#direct-wasm-execution)
7. [Linux Mode Execution](#linux-mode-execution)
8. [RPC Throttling Performance](#rpc-throttling-performance)
9. [Memory Profiling](#memory-profiling)
10. [Network Performance](#network-performance)
11. [Stress Testing](#stress-testing)
12. [Comparison Matrix](#comparison-matrix)
13. [Benchmark Tools](#benchmark-tools)
14. [Running Benchmarks](#running-benchmarks)

---

## Overview

This guide provides a framework for benchmarking NEAR OutLayer's performance across different execution modes and components. While full benchmarking hasn't been performed yet (demo mode is active), this document establishes the methodology for future performance testing.

### Key Performance Metrics

1. **Latency**: Time from request to response
2. **Throughput**: Requests per second
3. **Memory**: RAM usage patterns
4. **CPU**: Processing overhead
5. **Network**: Bandwidth and request counts

### Components to Benchmark

- **ContractSimulator** (direct mode)
- **LinuxExecutor** (Linux mode)
- **RPCClient** (throttling and retry logic)
- **Coordinator API** (proxy overhead)

---

## Benchmarking Objectives

### Primary Goals

1. **Quantify overhead**: Measure performance impact of each layer
2. **Identify bottlenecks**: Find slow operations
3. **Validate scaling**: Ensure system handles load
4. **Guide optimization**: Prioritize improvements

### Success Criteria

| Metric | Target | Acceptable | Notes |
|--------|--------|-----------|--------|
| Direct WASM execution | < 10ms | < 50ms | Contract instantiation + call |
| Linux mode overhead | 2-3x direct | 5x direct | Includes syscall translation |
| RPC proxy latency | < 20ms | < 100ms | Added delay from coordinator |
| Throttle check time | < 1ms | < 5ms | Token bucket check |
| Memory per contract | < 5MB | < 20MB | Loaded WASM instance |
| Concurrent requests | 100+ rps | 50+ rps | Per coordinator instance |

---

## Test Scenarios

### Scenario 1: Simple Counter Contract

**Purpose**: Measure baseline WASM execution performance

**Contract**: Increment/decrement counter (minimal state changes)

**Test cases**:
- Single execution (cold start)
- Single execution (warm - cached)
- Sequential executions (100 calls)
- Parallel executions (10 concurrent)

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
- Mint single NFT
- Batch mint 10 NFTs
- Query NFT metadata (read-heavy)

**Expected results**:
```
Single mint:           ~20-30ms
Batch mint (10):       ~150-200ms
Metadata query:        ~5-10ms
```

### Scenario 3: Encrypted Secrets Access

**Purpose**: Measure secrets decryption overhead

**Flow**: Request execution → Decrypt secrets → Inject env vars → Execute

**Test cases**:
- Execution with secrets (full flow)
- Execution without secrets (baseline)
- Large secrets payload (10+ keys)

**Expected overhead**:
```
Secrets decryption:    ~50-100ms (network + crypto)
Env var injection:     ~1-2ms
Total overhead:        ~60-120ms
```

### Scenario 4: RPC Throttling Burst

**Purpose**: Validate rate limiting under load

**Setup**: Anonymous client (5 rps limit)

**Test cases**:
- Send 50 requests in 1 second
- Measure 429 responses
- Track retry delays
- Calculate effective throughput

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
- Measure instruction count accuracy

**Expected overhead**:
```
Simple contract:       2x slower
Medium contract:       2.5x slower
Complex contract:      3x slower
(All in production mode; demo mode simulates ~10x)
```

---

## Methodology

### Test Environment

**Hardware requirements**:
- CPU: 4+ cores (modern x86_64 or ARM64)
- RAM: 8+ GB
- Network: Stable connection (for RPC tests)

**Software stack**:
- Browser: Chrome/Firefox latest
- Node.js: v18+
- Coordinator: Rust release build
- Database: PostgreSQL 14, Redis 7

### Test Execution

**General approach**:

1. **Warm-up**: Run 10 iterations to populate caches
2. **Measurement**: Run 100 iterations for statistical significance
3. **Cooldown**: Wait 5 seconds between test suites
4. **Repetition**: Repeat entire suite 3 times, average results

**Data collection**:
```javascript
// Benchmark helper
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

---

## Baseline Measurements

### Browser Baseline (No OutLayer)

**Purpose**: Establish raw WebAssembly performance floor

**Test**: Load and execute WASM directly via browser API

```javascript
async function baselineWasmExecution() {
  // Load WASM
  const response = await fetch('counter.wasm');
  const buffer = await response.arrayBuffer();

  // Instantiate
  const startInstantiate = performance.now();
  const module = await WebAssembly.instantiate(buffer, {});
  const instantiateTime = performance.now() - startInstantiate;

  // Execute
  const startCall = performance.now();
  const result = module.instance.exports.increment();
  const callTime = performance.now() - startCall;

  return {
    instantiate: instantiateTime,
    call: callTime,
    total: instantiateTime + callTime,
  };
}

// Expected results:
// Instantiate: ~3-5ms (first time)
// Instantiate: ~0.5ms (cached module)
// Call:        ~0.01ms (simple function)
```

### NEAR RPC Baseline (Direct)

**Purpose**: Measure upstream RPC latency

**Test**: Call NEAR RPC without coordinator proxy

```javascript
async function baselineNearRPC() {
  const start = performance.now();

  const response = await fetch('https://rpc.testnet.near.org', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({
      jsonrpc: '2.0',
      id: 'dontcare',
      method: 'status',
      params: [],
    }),
  });

  const data = await response.json();
  const duration = performance.now() - start;

  return { duration, data };
}

// Expected results:
// Testnet RPC: ~100-300ms (network latency)
// Mainnet RPC: ~50-150ms (depending on location)
```

---

## Direct WASM Execution

### Test: Contract Instantiation

**Scenario**: Measure overhead of ContractSimulator.execute() vs raw WebAssembly

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

// Expected overhead vs baseline:
// OutLayer overhead: ~2-5ms (NEAR host function emulation)
// Total time:        ~5-10ms (baseline ~3-5ms + overhead)
```

### Test: Sequential Executions

**Scenario**: Measure performance of repeated calls (cache effectiveness)

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
// Warm execution:    ~0.5-1ms (cached module)
// Improvement:       ~10x faster
```

### Test: Parallel Executions

**Scenario**: Measure concurrency handling

```javascript
async function benchmarkParallelExecutions() {
  const simulator = new ContractSimulator();

  // Warm up
  await simulator.execute('counter.wasm', 'increment', {});

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

## Linux Mode Execution

### Test: Demo Mode Performance

**Scenario**: Current implementation (simulated Linux)

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

**Scenario**: Real Linux kernel execution (when implemented)

```javascript
async function benchmarkLinuxProductionMode() {
  const simulator = new ContractSimulator({
    executionMode: 'linux',
    demoMode: false,  // Real kernel
  });

  // Wait for kernel boot
  await simulator.setExecutionMode('linux');

  // Test 1: First execution (cold)
  const coldStart = performance.now();
  await simulator.execute('counter.wasm', 'increment', {});
  const coldTime = performance.now() - coldStart;

  console.log(`Linux cold start: ${coldTime.toFixed(2)}ms`);

  // Test 2: Warm executions
  const bench = new Benchmark('Linux Production Mode (Warm)');

  await bench.run(async () => {
    await simulator.execute('counter.wasm', 'increment', {});
  });

  bench.report();

  // Get Linux statistics
  const stats = simulator.getLinuxStats();
  console.log('\nLinux Statistics:', stats);
}

// Expected results (production mode, not yet measured):
// Cold start:      ~50-100ms (task worker creation)
// Warm execution:  ~2-5ms (2-3x overhead vs direct)
// Syscall overhead: ~0.1ms per NEAR function call
```

### Test: Instruction Counting Accuracy

**Scenario**: Compare reported instruction count between modes

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

**Scenario**: Measure latency added by token bucket check

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

**Scenario**: Verify 5 rps limit for anonymous users

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

**Scenario**: Compare anonymous (5 rps) vs keyed (20 rps)

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

**Scenario**: Monitor memory growth during executions

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

### Test: Memory Leaks

**Scenario**: Detect memory leaks in long-running sessions

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

## Network Performance

### Test: WASM Cache Hit Rate

**Scenario**: Measure filesystem cache effectiveness

```javascript
async function benchmarkWASMCacheHitRate() {
  const simulator = new ContractSimulator();

  // Test 1: First load (cache miss)
  const cacheMissBench = new Benchmark('WASM Load (Cache Miss)');
  await cacheMissBench.run(async () => {
    // Clear browser cache (requires manual step)
    await simulator.execute('counter.wasm', 'increment', {});
  }, 10);

  // Test 2: Subsequent loads (cache hit)
  const cacheHitBench = new Benchmark('WASM Load (Cache Hit)');
  await cacheHitBench.run(async () => {
    await simulator.execute('counter.wasm', 'increment', {});
  }, 100);

  cacheMissBench.report();
  cacheHitBench.report();

  const missTime = cacheMissBench.getStats().mean;
  const hitTime = cacheHitBench.getStats().mean;
  const speedup = missTime / hitTime;

  console.log(`\nCache Performance:`);
  console.log(`  Miss time: ${missTime.toFixed(2)}ms`);
  console.log(`  Hit time:  ${hitTime.toFixed(2)}ms`);
  console.log(`  Speedup:   ${speedup.toFixed(1)}x`);
}

// Expected results:
// Cache miss:  ~50-100ms (network fetch + parse)
// Cache hit:   ~5-10ms (module instantiation only)
// Speedup:     ~10x faster with cache
```

### Test: Secrets Decryption Latency

**Scenario**: Measure keystore roundtrip time

```javascript
async function benchmarkSecretsDecryption() {
  // Assume secrets configured in contract
  const simulator = new ContractSimulator();

  const bench = new Benchmark('Execution with Secrets Decryption');

  await bench.run(async () => {
    await simulator.execute(
      'contract-with-secrets.wasm',
      'useSecrets',
      {},
      {
        secretsRef: {
          profile: 'production',
          account_id: 'alice.testnet',
        },
      }
    );
  }, 20);

  bench.report();
}

// Expected results:
// Decryption overhead: ~50-100ms (keystore HTTP roundtrip)
// Total time:          ~70-130ms (includes contract execution)
```

---

## Stress Testing

### Test: Sustained Load

**Scenario**: Run at capacity for extended period

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

**Scenario**: Send massive burst, measure recovery

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

## Comparison Matrix

### Summary Table

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

### Interpretation

**Direct Mode**:
- ✅ Fastest execution (2-3x overhead vs raw WASM)
- ✅ Lowest memory usage
- ✅ Best for simple contracts
- ❌ Limited syscall support

**Linux Mode (Demo)**:
- ⚠️ Simulated performance (not representative)
- ✅ Good for testing architecture
- ❌ ~100x slower due to artificial delays
- 🎯 Use for development only

**Linux Mode (Production)**:
- ✅ Full syscall support
- ✅ Real instruction counting
- ⚠️ 4-6x overhead (acceptable for complex contracts)
- ⚠️ Higher memory usage (~30 MB)
- 🎯 Use when advanced features needed

---

## Benchmark Tools

### Browser Tools

**Chrome DevTools**:
```
1. Open DevTools (F12)
2. Performance tab
3. Click Record
4. Run benchmarks
5. Stop recording
6. Analyze flame graph
```

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

**Artillery** (load testing):
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

**k6** (performance testing):
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

### Custom Benchmark Suite

**File**: `browser-worker/benchmarks/suite.js`

```javascript
import { ContractSimulator } from '../src/contract-simulator.js';
import { RPCClient } from '../src/rpc-client.js';
import { Benchmark } from './benchmark-helper.js';

export async function runFullSuite() {
  console.log('='.repeat(60));
  console.log('NEAR OutLayer Performance Benchmark Suite');
  console.log('='.repeat(60));

  // Direct mode benchmarks
  await runDirectModeBenchmarks();

  // Linux mode benchmarks
  await runLinuxModeBenchmarks();

  // RPC throttling benchmarks
  await runThrottlingBenchmarks();

  // Memory profiling
  await runMemoryProfiling();

  console.log('\n' + '='.repeat(60));
  console.log('Benchmark suite complete!');
  console.log('='.repeat(60));
}

async function runDirectModeBenchmarks() {
  console.log('\n📦 Direct Mode Benchmarks');
  console.log('-'.repeat(60));

  // Add benchmark functions here
  await benchmarkContractInstantiation();
  await benchmarkSequentialExecutions();
  await benchmarkParallelExecutions();
}

// ... other benchmark categories

// Run if invoked directly
if (import.meta.url === `file://${process.argv[1]}`) {
  runFullSuite();
}
```

Run:
```bash
node browser-worker/benchmarks/suite.js
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

### Manual Benchmarks (Browser)

**Open console** and run:

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

### CI/CD Integration

**GitHub Actions** (`.github/workflows/benchmark.yml`):

```yaml
name: Performance Benchmarks

on:
  pull_request:
    branches: [main]

jobs:
  benchmark:
    runs-on: ubuntu-latest

    steps:
      - uses: actions/checkout@v3

      - name: Setup Node.js
        uses: actions/setup-node@v3
        with:
          node-version: 18

      - name: Install dependencies
        run: |
          cd browser-worker
          npm install

      - name: Run benchmarks
        run: |
          node browser-worker/benchmarks/suite.js > benchmark-results.txt

      - name: Upload results
        uses: actions/upload-artifact@v3
        with:
          name: benchmark-results
          path: benchmark-results.txt

      - name: Check regression
        run: |
          # Compare with baseline
          node scripts/compare-benchmarks.js \
            baseline-benchmarks.json \
            benchmark-results.txt
```

---

## Conclusion

This benchmarking guide provides a comprehensive framework for measuring NEAR OutLayer's performance across all components. Key takeaways:

1. **Methodology is established**: Warm-up, measurement, statistical analysis
2. **Metrics are defined**: Latency, throughput, memory, overhead
3. **Tools are documented**: Browser APIs, CLI tools, custom suite
4. **Baselines are projected**: Expected performance targets

**Next Steps** (when ready):
1. Implement production Linux mode (disable demo mode)
2. Run full benchmark suite
3. Compare results with projections in this document
4. Optimize bottlenecks identified
5. Update this document with real measurements

**Current Status**: Framework complete, awaiting production benchmarking.

---

**Document Version**: 1.0
**Last Updated**: 2025-11-05
**Maintained By**: OutLayer Team
