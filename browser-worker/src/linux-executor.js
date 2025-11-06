/**
 * LinuxExecutor - Linux Kernel WASM Runtime Integration
 *
 * Provides a proof-of-concept integration between NEAR contract execution
 * and a full Linux kernel running in WebAssembly. This demonstrates OutLayer's
 * ability to handle complex WASM workloads beyond simple contracts.
 *
 * Architecture:
 * - Main thread (this file) orchestrates kernel boot and task management
 * - Worker threads run individual "CPU" contexts (one per task)
 * - Linux kernel (vmlinux.wasm) handles syscalls and process management
 * - NEAR state mapped to virtual filesystem (/near/state/)
 * - NEAR host functions exposed as custom syscalls (400-499 range)
 *
 * Integration with OutLayer:
 * - ContractSimulator.execute() can route to LinuxExecutor
 * - NEAR storage operations mapped to Linux VFS
 * - Gas metering via instruction counting (wasmi/wasmtime fuel)
 * - Results returned through standard NEAR receipt format
 *
 * @author OutLayer Team
 * @version 1.0.0 - Proof of Concept
 */

class LinuxExecutor {
  constructor(options = {}) {
    this.options = {
      // Path to vmlinux.wasm (Linux kernel compiled to WASM)
      kernelPath: options.kernelPath || '/linux-runtime/vmlinux.wasm',

      // Path to initrd.cpio (initial ramdisk with busybox, etc.)
      initrdPath: options.initrdPath || '/linux-runtime/initramfs.cpio',

      // Memory configuration
      memoryPages: options.memoryPages || 30,  // 30 pages = ~2 MB initial
      maxMemoryPages: options.maxMemoryPages || 0x10000,  // ~1 GB max

      // Enable verbose logging
      verbose: options.verbose || false,

      // NEAR state integration
      nearState: options.nearState || null,  // Reference to global nearState Map

      // Execution mode demo (without actual kernel for POC)
      demoMode: options.demoMode !== false,  // Default to demo mode
    };

    // Kernel state
    this.kernelReady = false;
    this.mainWorker = null;
    this.taskWorkers = new Map();  // task_id → Worker instance

    // Shared memory for synchronization (if supported)
    this.sharedMemory = null;
    this.locks = null;

    // Statistics
    this.stats = {
      bootTime: 0,
      totalTasks: 0,
      totalSyscalls: 0,
      totalInstructions: 0,
    };

    this.log('LinuxExecutor initialized', 'info');
  }

  /**
   * Initialize Linux kernel runtime
   *
   * Steps:
   * 1. Create SharedArrayBuffer for memory (if supported)
   * 2. Load vmlinux.wasm (kernel binary)
   * 3. Load initrd.cpio (filesystem)
   * 4. Create main worker thread
   * 5. Boot kernel
   * 6. Wait for init process
   *
   * @returns {Promise<boolean>} Success
   */
  async initialize() {
    if (this.kernelReady) {
      this.log('Linux kernel already initialized', 'warn');
      return true;
    }

    this.log('🐧 Initializing Linux/WASM runtime...', 'info');

    const startTime = Date.now();

    try {
      // Demo mode: Simulate initialization without actual kernel
      if (this.options.demoMode) {
        this.log('Running in DEMO mode (no actual kernel loaded)', 'warn');
        this.log('In production: would load vmlinux.wasm (~24 MB)', 'info');
        this.log('In production: would load initramfs.cpio (~5 MB)', 'info');

        await this.sleep(500); // Simulate boot time

        this.kernelReady = true;
        this.stats.bootTime = Date.now() - startTime;

        this.log(`✓ Linux kernel initialized (demo) in ${this.stats.bootTime}ms`, 'success');
        return true;
      }

      // Production mode: Actual kernel loading
      // 1. Create shared memory
      if (typeof SharedArrayBuffer !== 'undefined') {
        this.sharedMemory = new WebAssembly.Memory({
          initial: this.options.memoryPages,
          maximum: this.options.maxMemoryPages,
          shared: true,
        });

        // Create synchronization primitives
        this.locks = {
          _memory: new Int32Array(new SharedArrayBuffer(4 * 32)), // 32 locks
          serialize: 0,
          boot: 1,
          syscall: 2,
        };

        this.log('✓ Shared memory created', 'info');
      } else {
        this.log('⚠️  SharedArrayBuffer not available (COOP/COEP headers required)', 'warn');
        // Fall back to non-shared memory
        this.sharedMemory = new WebAssembly.Memory({
          initial: this.options.memoryPages,
          maximum: this.options.maxMemoryPages,
          shared: false,
        });
      }

      // 2. Load kernel WASM
      this.log('Loading vmlinux.wasm...', 'info');
      const kernelResponse = await fetch(this.options.kernelPath);
      if (!kernelResponse.ok) {
        throw new Error(`Failed to load kernel: ${kernelResponse.status}`);
      }

      const kernelBytes = await kernelResponse.arrayBuffer();
      this.log(`✓ Kernel loaded (${(kernelBytes.byteLength / 1024 / 1024).toFixed(1)} MB)`, 'info');

      // 3. Load initrd
      this.log('Loading initramfs.cpio...', 'info');
      const initrdResponse = await fetch(this.options.initrdPath);
      if (!initrdResponse.ok) {
        throw new Error(`Failed to load initrd: ${initrdResponse.status}`);
      }

      const initrdBytes = await initrdResponse.arrayBuffer();
      this.log(`✓ Initrd loaded (${(initrdBytes.byteLength / 1024 / 1024).toFixed(1)} MB)`, 'info');

      // 4. Create main worker
      this.log('Creating main kernel worker...', 'info');
      this.mainWorker = new Worker('/linux-runtime/linux-worker.js');

      // Send initialization message
      this.mainWorker.postMessage({
        type: 'init',
        memory: this.sharedMemory,
        vmlinux: kernelBytes,
        initrd: initrdBytes,
        locks: this.locks,
      });

      // Wait for boot complete
      await new Promise((resolve, reject) => {
        const timeout = setTimeout(() => {
          reject(new Error('Kernel boot timeout (30s)'));
        }, 30000);

        this.mainWorker.addEventListener('message', (event) => {
          if (event.data.type === 'boot_complete') {
            clearTimeout(timeout);
            resolve();
          } else if (event.data.type === 'error') {
            clearTimeout(timeout);
            reject(new Error(event.data.message));
          }
        });
      });

      this.kernelReady = true;
      this.stats.bootTime = Date.now() - startTime;

      this.log(`✓ Linux kernel booted successfully in ${this.stats.bootTime}ms`, 'success');
      return true;

    } catch (error) {
      this.log(`✗ Failed to initialize Linux kernel: ${error.message}`, 'error');
      throw error;
    }
  }

  /**
   * Execute WASM program in Linux environment
   *
   * Steps:
   * 1. Load WASM binary into kernel memory
   * 2. Create new task (process) for execution
   * 3. Map NEAR state to /near/state/ virtual filesystem
   * 4. Execute program with args
   * 5. Capture stdout/stderr
   * 6. Return exit code + output
   *
   * @param {Uint8Array} wasmBytes - Compiled WASM binary
   * @param {string[]} args - Command-line arguments
   * @param {Object} env - Environment variables
   * @returns {Promise<Object>} { stdout, stderr, exitCode, stats }
   */
  async executeProgram(wasmBytes, args = [], env = {}) {
    if (!this.kernelReady) {
      throw new Error('Linux kernel not initialized. Call initialize() first.');
    }

    this.log(`Executing WASM program (${wasmBytes.length} bytes)...`, 'info');

    // Demo mode: Simulate execution
    if (this.options.demoMode) {
      this.log('[DEMO] Simulating Linux execution...', 'info');

      await this.sleep(100); // Simulate execution time

      const demoOutput = {
        stdout: `Demo Linux execution\nArgs: ${args.join(' ')}\nEnv vars: ${Object.keys(env).length}\nWASM size: ${wasmBytes.length} bytes\n`,
        stderr: '',
        exitCode: 0,
        stats: {
          instructions: 1000000,  // Simulated
          timeMs: 100,
          syscalls: 42,
        },
      };

      this.stats.totalTasks++;
      this.stats.totalInstructions += demoOutput.stats.instructions;

      this.log('[DEMO] Execution complete (simulated)', 'success');
      return demoOutput;
    }

    // Production mode: Actual execution
    const taskId = this.stats.totalTasks++;

    return new Promise((resolve, reject) => {
      const timeout = setTimeout(() => {
        reject(new Error('Program execution timeout (60s)'));
      }, 60000);

      // Create task-specific worker
      const taskWorker = new Worker('/linux-runtime/linux-worker.js');
      this.taskWorkers.set(taskId, taskWorker);

      // Send execution request
      taskWorker.postMessage({
        type: 'execute',
        taskId,
        wasmBytes,
        args,
        env,
        nearState: this.serializeNearState(),
      });

      // Handle result
      taskWorker.addEventListener('message', (event) => {
        if (event.data.type === 'execution_complete') {
          clearTimeout(timeout);

          const result = {
            stdout: event.data.stdout,
            stderr: event.data.stderr,
            exitCode: event.data.exitCode,
            stats: {
              instructions: event.data.instructions,
              timeMs: event.data.timeMs,
              syscalls: event.data.syscalls,
            },
          };

          // Update statistics
          this.stats.totalInstructions += result.stats.instructions;
          this.stats.totalSyscalls += result.stats.syscalls;

          // Cleanup worker
          taskWorker.terminate();
          this.taskWorkers.delete(taskId);

          this.log(`✓ Execution complete: exit code ${result.exitCode}`, 'success');
          resolve(result);

        } else if (event.data.type === 'error') {
          clearTimeout(timeout);
          taskWorker.terminate();
          this.taskWorkers.delete(taskId);
          reject(new Error(event.data.message));
        }
      });
    });
  }

  /**
   * Execute NEAR contract method in Linux environment
   *
   * This is the integration point with ContractSimulator.
   * Maps NEAR contract execution to Linux process execution.
   *
   * @param {Uint8Array} wasmBytes - Contract WASM
   * @param {string} methodName - Contract method
   * @param {Object} args - Method arguments (JSON)
   * @param {Map} nearState - NEAR state (will be mapped to VFS)
   * @returns {Promise<Object>} { result, gasUsed }
   */
  async executeContract(wasmBytes, methodName, args, nearState) {
    this.log(`Executing NEAR contract method: ${methodName}`, 'info');

    // Set NEAR state reference
    this.options.nearState = nearState;

    // Prepare environment variables for contract
    const env = {
      NEAR_METHOD: methodName,
      NEAR_ARGS: JSON.stringify(args),
      NEAR_STATE_PATH: '/near/state',
    };

    // Execute in Linux
    const result = await this.executeProgram(wasmBytes, [methodName], env);

    // Parse result from stdout
    let contractResult;
    try {
      // Contract should output JSON result to stdout
      contractResult = JSON.parse(result.stdout.trim());
    } catch (error) {
      this.log(`⚠️  Failed to parse contract result: ${error.message}`, 'warn');
      contractResult = { error: 'Invalid contract output', stdout: result.stdout };
    }

    return {
      result: contractResult,
      gasUsed: result.stats.instructions,  // Use instruction count as gas
      logs: result.stderr ? result.stderr.split('\n') : [],
      exitCode: result.exitCode,
    };
  }

  /**
   * Serialize NEAR state to pass to worker
   * (In production, would map to virtual filesystem)
   */
  serializeNearState() {
    if (!this.options.nearState) return {};

    const serialized = {};
    this.options.nearState.forEach((value, key) => {
      serialized[key] = Array.from(value);
    });
    return serialized;
  }

  /**
   * Shutdown Linux kernel and cleanup workers
   */
  async shutdown() {
    this.log('Shutting down Linux kernel...', 'info');

    // Terminate all task workers
    this.taskWorkers.forEach((worker, taskId) => {
      worker.terminate();
      this.log(`Terminated task worker ${taskId}`, 'info');
    });
    this.taskWorkers.clear();

    // Terminate main worker
    if (this.mainWorker) {
      this.mainWorker.terminate();
      this.mainWorker = null;
    }

    this.kernelReady = false;
    this.log('✓ Linux kernel shut down', 'success');
  }

  /**
   * Get executor statistics
   */
  getStats() {
    return {
      ...this.stats,
      kernelReady: this.kernelReady,
      activeTasks: this.taskWorkers.size,
      demoMode: this.options.demoMode,
    };
  }

  /**
   * Helper: sleep for specified milliseconds
   */
  sleep(ms) {
    return new Promise(resolve => setTimeout(resolve, ms));
  }

  /**
   * Helper: logging
   */
  log(message, level = 'info') {
    if (!this.options.verbose && level === 'info') return;

    const prefix = '[LinuxExecutor]';
    const timestamp = new Date().toISOString().split('T')[1].split('.')[0];

    switch (level) {
      case 'error':
        console.error(`${timestamp} ${prefix} ❌`, message);
        break;
      case 'warn':
        console.warn(`${timestamp} ${prefix} ⚠️ `, message);
        break;
      case 'success':
        console.log(`${timestamp} ${prefix} ✓`, message);
        break;
      default:
        console.log(`${timestamp} ${prefix}`, message);
    }
  }
}

// Export for use in other modules
if (typeof module !== 'undefined' && module.exports) {
  module.exports = { LinuxExecutor };
}

// Browser global
if (typeof window !== 'undefined') {
  window.LinuxExecutor = LinuxExecutor;
}
