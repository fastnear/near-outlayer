/**
 * Example: OutLayer Worker Implementation
 *
 * This demonstrates how to use the OutLayer TypeScript client to build a worker
 * that processes execution requests.
 */

import OutLayerClient, { generateIdempotencyKey, computeChecksum } from '../src/index';
import { readFile } from 'fs/promises';
import { execSync } from 'child_process';

// ============================================================================
// Configuration
// ============================================================================

const client = new OutLayerClient({
  baseUrl: process.env.COORDINATOR_URL || 'http://localhost:8080',
  auth: {
    type: 'near',
    accountId: process.env.WORKER_ACCOUNT_ID || 'worker.testnet',
    privateKey: process.env.WORKER_PRIVATE_KEY || 'ed25519:...'
  },
  timeout: 60000,  // 60 seconds
  retries: 3
});

// ============================================================================
// Main Worker Loop
// ============================================================================

async function main() {
  console.log('🚀 OutLayer Worker started');
  console.log(`   Account: ${process.env.WORKER_ACCOUNT_ID}`);
  console.log(`   Coordinator: ${process.env.COORDINATOR_URL}`);

  // Poll for tasks (in production, use Redis BRPOP or similar)
  while (true) {
    try {
      const task = await pollForTask();

      if (task) {
        await processTask(task);
      } else {
        // No tasks available, wait before polling again
        await sleep(5000);
      }
    } catch (error) {
      console.error('❌ Worker error:', error);
      await sleep(10000); // Back off on error
    }
  }
}

// ============================================================================
// Task Processing
// ============================================================================

interface Task {
  requestId: number;
  dataId: string;
  repo: string;
  commit: string;
  buildPath?: string;
  buildTarget?: string;
}

async function processTask(task: Task) {
  console.log(`\n📦 Processing request ${task.requestId}`);
  console.log(`   Repo: ${task.repo}`);
  console.log(`   Commit: ${task.commit}`);

  const startTime = Date.now();

  try {
    // 1. Claim jobs with idempotency
    const claimKey = generateIdempotencyKey();
    const { jobs } = await client.claimJobs({
      requestId: task.requestId,
      dataId: task.dataId,
      idempotencyKey: claimKey
    });

    if (jobs.length === 0) {
      console.log('⚠️  No jobs to process (already claimed by another worker)');
      return;
    }

    console.log(`✅ Claimed jobs: ${jobs.join(', ')}`);

    // 2. Compile if needed
    let wasmChecksum: string | undefined;

    if (jobs.includes('compile')) {
      console.log('🔨 Compiling WASM...');
      const compileStart = Date.now();

      const wasmBytes = await compileWasm(task);
      wasmChecksum = computeChecksum(wasmBytes);

      const compileTime = Date.now() - compileStart;
      console.log(`✅ Compilation complete (${compileTime}ms, checksum: ${wasmChecksum.slice(0, 8)}...)`);

      // Upload to cache with idempotency
      const uploadKey = generateIdempotencyKey();
      await client.uploadWasm({
        requestId: task.requestId,
        dataId: task.dataId,
        wasmBytes,
        checksum: wasmChecksum,
        idempotencyKey: uploadKey
      });

      console.log('✅ WASM uploaded to cache');
    }

    // 3. Execute
    if (jobs.includes('execute')) {
      console.log('⚡ Executing WASM...');
      const execStart = Date.now();

      const result = await executeWasm(task);

      const execTime = Date.now() - execStart;
      console.log(`✅ Execution complete (${execTime}ms, ${result.resourcesUsed.instructions} instructions)`);

      // Submit result with idempotency
      const submitKey = generateIdempotencyKey();
      await client.submitResult({
        requestId: task.requestId,
        success: result.success,
        output: result.output,
        error: result.error,
        resourcesUsed: result.resourcesUsed,
        compilationNote: wasmChecksum ? `WASM: ${wasmChecksum.slice(0, 16)}` : undefined,
        idempotencyKey: submitKey
      });

      console.log('✅ Result submitted');
    }

    const totalTime = Date.now() - startTime;
    console.log(`🎉 Request ${task.requestId} completed in ${totalTime}ms`);

  } catch (error) {
    console.error(`❌ Task failed:`, error);

    // Submit error result
    try {
      await client.submitResult({
        requestId: task.requestId,
        success: false,
        error: error instanceof Error ? error.message : 'Unknown error',
        resourcesUsed: {
          instructions: 0,
          timeMs: Date.now() - startTime
        }
      });
    } catch (submitError) {
      console.error('Failed to submit error result:', submitError);
    }
  }
}

// ============================================================================
// Compilation (Simplified)
// ============================================================================

async function compileWasm(task: Task): Promise<Uint8Array> {
  // In production, this would:
  // 1. Clone repo: git clone {task.repo} --depth 1 --branch {task.commit}
  // 2. cd to build_path
  // 3. Run: cargo build --release --target {task.buildTarget}
  // 4. Read: target/{buildTarget}/release/*.wasm

  // For this example, just read a dummy file
  const wasmPath = './dummy.wasm';
  const wasmBytes = await readFile(wasmPath);

  return new Uint8Array(wasmBytes);
}

// ============================================================================
// Execution (Simplified)
// ============================================================================

interface ExecutionResult {
  success: boolean;
  output?: any;
  error?: string;
  resourcesUsed: {
    instructions: number;
    timeMs: number;
    compileTimeMs?: number;
  };
}

async function executeWasm(task: Task): Promise<ExecutionResult> {
  // In production, this would:
  // 1. Load WASM from cache
  // 2. Instantiate with wasmtime/wasmer
  // 3. Execute with fuel metering
  // 4. Capture output and metrics

  // For this example, return dummy result
  return {
    success: true,
    output: { result: 42 },
    resourcesUsed: {
      instructions: 1_000_000,
      timeMs: 150
    }
  };
}

// ============================================================================
// Task Polling (Simplified)
// ============================================================================

async function pollForTask(): Promise<Task | null> {
  // In production, this would:
  // 1. Redis BRPOP on task queue (blocking poll)
  // 2. Parse task JSON
  // 3. Return task

  // For this example, return null (no tasks)
  return null;
}

// ============================================================================
// Utilities
// ============================================================================

function sleep(ms: number): Promise<void> {
  return new Promise(resolve => setTimeout(resolve, ms));
}

// ============================================================================
// Run Worker
// ============================================================================

if (require.main === module) {
  main().catch(error => {
    console.error('Fatal error:', error);
    process.exit(1);
  });
}

export { processTask, compileWasm, executeWasm };
