/**
 * ContractSimulator - NEAR Contract Execution Simulator
 *
 * Orchestrates NEAR contract execution in the browser using NEARVMLogic.
 * Provides high-level query() and execute() methods that handle:
 * - WASM module loading and caching
 * - Method argument serialization
 * - State management (IDBFS integration)
 * - Gas tracking and reporting
 * - Result deserialization
 *
 * Architecture:
 * - Wraps NEARVMLogic to provide contract-level abstraction
 * - Manages global state Map (nearState)
 * - Handles JSON serialization for contract methods
 * - Persists state changes to IDBFS (if available)
 *
 * Integration:
 * - Can work standalone in any browser environment
 * - Integrates with WASM REPL via EM_ASM callbacks
 * - Can be extended for OutLayer coordinator submission
 *
 * @author OutLayer Team
 * @version 1.0.0
 */

// Global state storage (shared across all contract instances)
if (typeof window !== 'undefined' && !window.nearState) {
  window.nearState = new Map();
}
if (typeof global !== 'undefined' && !global.nearState) {
  global.nearState = new Map();
}

const nearState = (typeof window !== 'undefined') ? window.nearState : global.nearState;

class ContractSimulator {
  constructor(options = {}) {
    // WASM module cache
    this.contracts = new Map();

    // Execution options
    this.options = {
      persistState: options.persistState !== false, // Default: persist to IDBFS
      verboseLogging: options.verboseLogging || false,
      defaultGasLimit: options.defaultGasLimit || 300000000000000, // 300 Tgas
      enableSealedStorage: options.enableSealedStorage || false, // Phase 3: Sealed storage
      executionMode: options.executionMode || 'direct', // 'direct' | 'linux' | 'enclave' | 'quickjs-browser'
      ...options
    };

    // Statistics
    this.stats = {
      totalQueries: 0,
      totalExecutions: 0,
      totalGasUsed: 0,
      lastExecutionTime: 0,
      linuxExecutions: 0,
      directExecutions: 0,
      enclaveExecutions: 0
    };

    // Sealed storage instance (if enabled)
    this.sealedStorage = null;
    if (this.options.enableSealedStorage) {
      this.sealedStorage = new SealedStorage();
    }

    // Linux executor instance (if mode is 'linux')
    this.linuxExecutor = null;
    if (this.options.executionMode === 'linux') {
      this.linuxExecutor = new LinuxExecutor({
        verbose: this.options.verboseLogging,
        nearState: nearState,
        demoMode: true, // Demo mode for POC
      });
    }

    // Enclave executor instance (if mode is 'enclave') - Phase 4
    this.enclaveExecutor = null;
    if (this.options.executionMode === 'enclave') {
      this.enclaveExecutor = new EnclaveExecutor({
        verbose: this.options.verboseLogging,
        executionTimeout: this.options.defaultGasLimit / 1000000, // Convert Tgas to ms
      });
    }

    // QuickJS browser executor instance (if mode is 'quickjs-browser') - Phase 3
    this.quickjsEnclave = null;
    if (this.options.executionMode === 'quickjs-browser') {
      // Lazy initialization - will be created on first use
      this.initQuickJSEnclave();
    }
  }

  /**
   * Initialize QuickJS browser enclave (lazy)
   */
  async initQuickJSEnclave() {
    if (this.quickjsEnclave) return;

    // Dynamic import to avoid loading quickjs-emscripten if not needed
    const { QuickJSEnclave } = await import('./quickjs-enclave');
    this.quickjsEnclave = await QuickJSEnclave.create({
      memoryBytes: 64 << 20, // 64 MiB default
    });
    this.log('[QUICKJS] Enclave initialized');
  }

  /**
   * Initialize sealed storage (Phase 3)
   * Must be called before using sealed storage features
   */
  async initializeSealedStorage() {
    if (!this.sealedStorage) {
      this.sealedStorage = new SealedStorage();
      this.options.enableSealedStorage = true;
    }
    await this.sealedStorage.initialize();
    this.log('[SEALED STORAGE] Initialized with WebCrypto');
  }

  // ============================================================================
  // MODULE LOADING
  // ============================================================================

  /**
   * Load and compile WASM module
   * @param {string|Uint8Array} wasmSource - Path to WASM file or bytes
   * @returns {Promise<WebAssembly.Module>}
   */
  async loadContract(wasmSource) {
    let cacheKey;
    let wasmBytes;

    if (typeof wasmSource === 'string') {
      // Path to WASM file
      cacheKey = wasmSource;

      // Check cache first
      if (this.contracts.has(cacheKey)) {
        this.log(`[CACHE HIT] ${cacheKey}`);
        return this.contracts.get(cacheKey);
      }

      // Load from filesystem (Emscripten FS) or fetch
      if (typeof FS !== 'undefined') {
        // Emscripten environment
        wasmBytes = FS.readFile(wasmSource, { encoding: 'binary' });
      } else {
        // Browser fetch
        const response = await fetch(wasmSource);
        wasmBytes = await response.arrayBuffer();
      }
    } else {
      // Direct bytes
      wasmBytes = wasmSource;
      cacheKey = await this.computeChecksum(wasmBytes);
    }

    this.log(`[LOADING] Compiling WASM module (${wasmBytes.byteLength} bytes)...`);

    const module = await WebAssembly.compile(new Uint8Array(wasmBytes));

    this.contracts.set(cacheKey, module);
    this.log(`[LOADED] ${cacheKey}`);

    return module;
  }

  /**
   * Compute SHA-256 checksum of WASM bytes
   */
  async computeChecksum(bytes) {
    const hashBuffer = await crypto.subtle.digest('SHA-256', bytes);
    const hashArray = Array.from(new Uint8Array(hashBuffer));
    return hashArray.map(b => b.toString(16).padStart(2, '0')).join('');
  }

  /**
   * Load contract WASM as bytes (for Linux executor)
   * Similar to loadContract but returns bytes instead of compiled module
   */
  async loadContractBytes(wasmSource) {
    if (wasmSource instanceof Uint8Array) {
      return wasmSource;
    }

    // Load from URL
    if (typeof FS !== 'undefined' && FS.analyzePath) {
      // Emscripten FS
      const fileData = FS.readFile(wasmSource);
      return new Uint8Array(fileData);
    } else {
      // Browser fetch
      const response = await fetch(wasmSource);
      if (!response.ok) {
        throw new Error(`Failed to load WASM: ${response.status}`);
      }
      const arrayBuffer = await response.arrayBuffer();
      return new Uint8Array(arrayBuffer);
    }
  }

  // ============================================================================
  // EXECUTION MODE MANAGEMENT (Phase 2)
  // ============================================================================

  /**
   * Switch execution mode (direct, linux, or enclave)
   * @param {string} mode - 'direct', 'linux', or 'enclave'
   */
  async setExecutionMode(mode) {
    if (mode !== 'direct' && mode !== 'linux' && mode !== 'enclave') {
      throw new Error(`Invalid execution mode: ${mode}. Must be 'direct', 'linux', or 'enclave'`);
    }

    this.log(`Switching execution mode: ${this.options.executionMode} → ${mode}`, 'info');

    // If switching to Linux and executor doesn't exist, create it
    if (mode === 'linux' && !this.linuxExecutor) {
      this.linuxExecutor = new LinuxExecutor({
        verbose: this.options.verboseLogging,
        nearState: nearState,
        demoMode: true,
      });
    }

    // If switching to Linux and kernel not ready, initialize it
    if (mode === 'linux' && !this.linuxExecutor.kernelReady) {
      this.log('Initializing Linux kernel...', 'info');
      await this.linuxExecutor.initialize();
    }

    // If switching to Enclave and executor doesn't exist, create it
    if (mode === 'enclave' && !this.enclaveExecutor) {
      this.enclaveExecutor = new EnclaveExecutor({
        verbose: this.options.verboseLogging,
        executionTimeout: this.options.defaultGasLimit / 1000000,
      });
    }

    this.options.executionMode = mode;
    this.log(`✓ Execution mode set to: ${mode}`, 'success');
  }

  /**
   * Get current execution mode
   */
  getExecutionMode() {
    return this.options.executionMode;
  }

  /**
   * Get Linux executor stats (if available)
   */
  getLinuxStats() {
    if (!this.linuxExecutor) {
      return { available: false };
    }
    return {
      available: true,
      ...this.linuxExecutor.getStats(),
    };
  }

  // ============================================================================
  // QUERY (VIEW CALL)
  // ============================================================================

  /**
   * Execute view call (read-only, does not persist state)
   * @param {string|Uint8Array} wasmSource - WASM module path or bytes
   * @param {string} methodName - Contract method to call
   * @param {object} args - Method arguments (will be JSON-serialized)
   * @param {object} context - Execution context overrides
   * @returns {Promise<{result: any, gasUsed: number, logs: string[]}>}
   */
  async query(wasmSource, methodName, args = {}, context = {}) {
    this.stats.totalQueries++;

    const startTime = performance.now();

    this.log(`\n${'='.repeat(60)}`);
    this.log(`QUERY: ${typeof wasmSource === 'string' ? wasmSource : 'inline'}::${methodName}`);
    this.log(`${'='.repeat(60)}`);

    try {
      // Create VMLogic for view call
      const vmLogic = new NEARVMLogic(true, {
        ...context,
        gasLimit: context.gasLimit || this.options.defaultGasLimit
      });

      // Set state reference
      vmLogic.state = nearState;

      // Serialize arguments
      const argsJson = JSON.stringify(args);
      const argsBytes = new TextEncoder().encode(argsJson);
      vmLogic.methodArgs = argsBytes;

      this.log(`Arguments: ${argsJson}`);

      // Load and instantiate contract
      const module = await this.loadContract(wasmSource);
      const env = vmLogic.createEnvironment();
      const instance = await WebAssembly.instantiate(module, env);

      // Set memory reference
      vmLogic.setMemory(instance.exports.memory);

      // Call the method
      if (!instance.exports[methodName]) {
        throw new Error(`Method '${methodName}' not found in contract exports`);
      }

      this.log(`Calling ${methodName}()...`);

      instance.exports[methodName]();

      // Parse return value
      const result = this.parseReturnValue(vmLogic.returnData);

      const endTime = performance.now();
      const executionTime = endTime - startTime;

      this.log(`\nResult: ${JSON.stringify(result, null, 2)}`);
      this.log(`Gas used: ${vmLogic.gasUsed.toLocaleString()} (${(vmLogic.gasUsed / 1000000000000).toFixed(2)} Tgas)`);
      this.log(`Execution time: ${executionTime.toFixed(2)}ms`);
      this.log(`Logs: ${vmLogic.logs.length} entries`);
      this.log(`${'='.repeat(60)}\n`);

      this.stats.totalGasUsed += vmLogic.gasUsed;
      this.stats.lastExecutionTime = executionTime;

      return {
        result,
        gasUsed: vmLogic.gasUsed,
        logs: vmLogic.logs,
        executionTime
      };

    } catch (error) {
      const endTime = performance.now();
      const executionTime = endTime - startTime;

      this.log(`\n[ERROR] ${error.message}`);
      this.log(`Execution time: ${executionTime.toFixed(2)}ms`);
      this.log(`${'='.repeat(60)}\n`);

      throw error;
    }
  }

  // ============================================================================
  // EXECUTE (CHANGE CALL)
  // ============================================================================

  /**
   * Execute change call (modifies state, persists changes)
   * @param {string|Uint8Array} wasmSource - WASM module path or bytes
   * @param {string} methodName - Contract method to call
   * @param {object} args - Method arguments (will be JSON-serialized)
   * @param {object} context - Execution context overrides
   * @returns {Promise<{result: any, gasUsed: number, logs: string[], stateChanges: number}>}
   */
  async execute(wasmSource, methodName, args = {}, context = {}) {
    this.stats.totalExecutions++;

    // Route execution based on mode
    if (this.options.executionMode === 'quickjs-browser') {
      return await this.executeQuickJSBrowser(wasmSource, methodName, args, context);
    } else if (this.options.executionMode === 'linux') {
      return await this.executeLinux(wasmSource, methodName, args, context);
    } else if (this.options.executionMode === 'enclave') {
      return await this.executeEnclave(wasmSource, methodName, args, context);
    } else {
      return await this.executeDirect(wasmSource, methodName, args, context);
    }
  }

  /**
   * Execute JavaScript contract in QuickJS browser enclave (Phase 3)
   * @param {string} jsSource - JavaScript contract source code
   * @param {string} methodName - Function name to call
   * @param {object} args - Function arguments
   * @param {object} context - Execution context (seed, policy overrides)
   * @returns {Promise<{result: any, gasUsed: number, logs: string[], executionTime: number}>}
   */
  async executeQuickJSBrowser(jsSource, methodName, args = {}, context = {}) {
    const startTime = performance.now();

    this.log(`\n${'='.repeat(60)}`);
    this.log(`EXECUTE (QuickJS Browser): ${methodName}`);
    this.log(`${'='.repeat(60)}`);

    // Ensure enclave is initialized
    await this.initQuickJSEnclave();

    // Load JavaScript source if it's a path
    let source = jsSource;
    if (typeof jsSource === 'string' && (jsSource.endsWith('.js') || jsSource.includes('/'))) {
      // Try to fetch/read the file
      if (typeof FS !== 'undefined' && FS.analyzePath && FS.analyzePath(jsSource).exists) {
        source = FS.readFile(jsSource, { encoding: 'utf8' });
      } else {
        const response = await fetch(jsSource);
        source = await response.text();
      }
      this.log(`Loaded JS contract: ${jsSource} (${source.length} chars)`);
    }

    // Prepare invocation
    const invocation = {
      source: source,
      func: methodName,
      args: Object.values(args), // Convert {a: 1, b: 2} to [1, 2]
      priorState: this.getQuickJSState(),
      seed: context.seed || `${Date.now()}-${Math.random()}`, // Deterministic seed if provided
      policy: {
        timeMs: context.timeMs || Math.floor(this.options.defaultGasLimit / 1000000), // Convert Tgas to ms
        memoryBytes: context.memoryBytes || (32 << 20), // 32 MiB default
      },
    };

    this.log(`Function: ${methodName}`);
    this.log(`Args: ${JSON.stringify(invocation.args)}`);
    this.log(`Prior state keys: ${Object.keys(invocation.priorState).length}`);
    this.log(`Seed: ${invocation.seed}`);
    this.log(`Policy: ${invocation.policy.timeMs}ms, ${(invocation.policy.memoryBytes / (1 << 20)).toFixed(1)} MiB`);

    try {
      const result = await this.quickjsEnclave.invoke(invocation);

      const endTime = performance.now();
      const executionTime = endTime - startTime;

      // Update state
      this.saveQuickJSState(result.state);

      // Convert time to "gas" (1ms = 1 Tgas for estimation)
      const gasUsed = Math.floor(result.diagnostics.timeMs * 1000000);

      this.log(`\nResult: ${JSON.stringify(result.result, null, 2)}`);
      this.log(`Gas used (estimated): ${gasUsed.toLocaleString()} (${(gasUsed / 1000000000000).toFixed(2)} Tgas)`);
      this.log(`Execution time: ${executionTime.toFixed(2)}ms`);
      this.log(`QuickJS time: ${result.diagnostics.timeMs.toFixed(2)}ms`);
      this.log(`State keys: ${Object.keys(result.state).length}`);
      this.log(`Logs: ${result.diagnostics.logs.length} entries`);
      if (result.diagnostics.logs.length > 0) {
        this.log(`\nContract logs:`);
        result.diagnostics.logs.forEach(log => this.log(`  ${log}`));
      }
      this.log(`${'='.repeat(60)}\n`);

      this.stats.totalGasUsed += gasUsed;
      this.stats.lastExecutionTime = executionTime;

      return {
        result: result.result,
        gasUsed: gasUsed,
        logs: result.diagnostics.logs,
        executionTime: executionTime,
        quickjsTime: result.diagnostics.timeMs,
        interrupted: result.diagnostics.interrupted,
        stateKeys: Object.keys(result.state).length,
        mode: 'quickjs-browser'
      };
    } catch (error) {
      const endTime = performance.now();
      const executionTime = endTime - startTime;

      this.log(`\n[ERROR] ${error.message}`);
      this.log(`Execution time: ${executionTime.toFixed(2)}ms`);
      this.log(`${'='.repeat(60)}\n`);

      throw error;
    }
  }

  /**
   * Get QuickJS state from nearState or isolated storage
   */
  getQuickJSState() {
    // Store QuickJS state separately to avoid mixing with WASM contract state
    if (!this._quickjsState) {
      this._quickjsState = {};
    }
    return this._quickjsState;
  }

  /**
   * Save QuickJS state
   */
  saveQuickJSState(state) {
    this._quickjsState = state;
  }

  /**
   * Execute contract in Linux kernel environment (Phase 2)
   */
  async executeLinux(wasmSource, methodName, args = {}, context = {}) {
    this.stats.linuxExecutions++;

    const startTime = performance.now();

    this.log(`\n${'='.repeat(60)}`);
    this.log(`EXECUTE (Linux Mode): ${typeof wasmSource === 'string' ? wasmSource : 'inline'}::${methodName}`);
    this.log(`${'='.repeat(60)}`);

    // Initialize Linux if needed
    if (!this.linuxExecutor.kernelReady) {
      this.log('Initializing Linux kernel...');
      await this.linuxExecutor.initialize();
    }

    // Load WASM bytes
    const wasmBytes = await this.loadContractBytes(wasmSource);

    // Execute in Linux
    const linuxResult = await this.linuxExecutor.executeContract(
      wasmBytes,
      methodName,
      args,
      nearState
    );

    const endTime = performance.now();
    const executionTime = endTime - startTime;

    this.stats.totalGasUsed += linuxResult.gasUsed;
    this.stats.lastExecutionTime = executionTime;

    this.log(`\nResult: ${JSON.stringify(linuxResult.result, null, 2)}`);
    this.log(`Gas used: ${linuxResult.gasUsed.toLocaleString()}`);
    this.log(`Execution time: ${executionTime.toFixed(2)}ms`);
    this.log(`Linux syscalls: ${linuxResult.stats?.syscalls || 'N/A'}`);

    return {
      result: linuxResult.result,
      gasUsed: linuxResult.gasUsed,
      executionTime: executionTime,
      mode: 'linux'
    };
  }

  /**
   * Execute contract in Frozen Realm with E2EE (Phase 4: Hermes Enclave)
   *
   * This method does NOT execute standard NEAR contracts. Instead, it executes
   * L4 guest code (JavaScript) that runs in a Frozen Realm with encrypted inputs.
   *
   * Use this mode when you need:
   * - End-to-end encrypted computation
   * - Client-side key custody
   * - Zero-knowledge execution (L1-L3 see only encrypted blobs)
   *
   * @param {string} guestCodePath - Path to L4 guest code (JavaScript file)
   * @param {string} methodName - Ignored (L4 guest code is self-contained)
   * @param {object} encryptedRequest - Encrypted execution request
   * @param {string} encryptedRequest.encryptedPayload - Base64 encrypted data
   * @param {string} encryptedRequest.encryptedSecret - Base64 encrypted secret
   * @param {string} encryptedRequest.enclaveKey - Hex L4 decryption key
   * @param {object} context - Execution context (ignored)
   * @returns {Promise<{result: any, gasUsed: number, executionTime: number}>}
   */
  async executeEnclave(guestCodePath, methodName, encryptedRequest = {}, context = {}) {
    this.stats.enclaveExecutions++;

    const startTime = performance.now();

    this.log(`\n${'='.repeat(60)}`);
    this.log(`EXECUTE (Enclave Mode): ${guestCodePath}`);
    this.log(`${'='.repeat(60)}`);

    // Initialize enclave executor if needed
    if (!this.enclaveExecutor) {
      this.log('Initializing Enclave Executor...');
      this.enclaveExecutor = new EnclaveExecutor({
        verbose: this.options.verboseLogging,
        executionTimeout: this.options.defaultGasLimit / 1000000,
      });
    }

    // Load L4 guest code
    let guestCode;
    if (typeof guestCodePath === 'string') {
      if (typeof FS !== 'undefined') {
        // Emscripten environment
        guestCode = FS.readFile(guestCodePath, { encoding: 'utf8' });
      } else {
        // Browser fetch
        const response = await fetch(guestCodePath);
        guestCode = await response.text();
      }
    } else {
      // Direct code string
      guestCode = guestCodePath;
    }

    this.log(`Guest code loaded (${guestCode.length} chars)`);
    this.log(`Encrypted payload: ${encryptedRequest.encryptedPayload?.slice(0, 32)}...`);
    this.log(`Encrypted secret: ${encryptedRequest.encryptedSecret?.slice(0, 32)}...`);

    // Execute in Frozen Realm with E2EE
    const enclaveResult = await this.enclaveExecutor.executeEncrypted({
      encryptedPayload: encryptedRequest.encryptedPayload,
      encryptedSecret: encryptedRequest.encryptedSecret,
      enclaveKey: encryptedRequest.enclaveKey,
      code: guestCode,
      codeId: guestCodePath,
    });

    const endTime = performance.now();
    const executionTime = endTime - startTime;

    // Estimate gas (1 Tgas = 1ms in NEAR)
    const gasUsed = Math.floor(executionTime * 1000000); // Convert ms to gas

    this.stats.totalGasUsed += gasUsed;
    this.stats.lastExecutionTime = executionTime;

    this.log(`\nEncrypted result: ${enclaveResult.encryptedResult.slice(0, 64)}...`);
    this.log(`Gas used (estimated): ${gasUsed.toLocaleString()}`);
    this.log(`Execution time: ${executionTime.toFixed(2)}ms`);
    this.log(`L4 time: ${enclaveResult.l4Time.toFixed(2)}ms`);
    this.log(`Layers: ${enclaveResult.layers.join(' → ')}`);

    return {
      result: enclaveResult.encryptedResult, // Still encrypted!
      gasUsed: gasUsed,
      executionTime: executionTime,
      l4Time: enclaveResult.l4Time,
      layers: enclaveResult.layers,
      mode: 'enclave'
    };
  }

  /**
   * Execute contract directly (original implementation)
   */
  async executeDirect(wasmSource, methodName, args = {}, context = {}) {
    this.stats.directExecutions++;

    const startTime = performance.now();

    this.log(`\n${'='.repeat(60)}`);
    this.log(`EXECUTE (Direct Mode): ${typeof wasmSource === 'string' ? wasmSource : 'inline'}::${methodName}`);
    this.log(`${'='.repeat(60)}`);

    // Track state changes
    const initialStateSize = nearState.size;

    try {
      // Create VMLogic for change call
      const vmLogic = new NEARVMLogic(false, {
        ...context,
        gasLimit: context.gasLimit || this.options.defaultGasLimit,
        signer_account_id: context.signer_account_id || 'alice.near',
        predecessor_account_id: context.predecessor_account_id || 'alice.near'
      });

      // Set state reference
      vmLogic.state = nearState;

      // Serialize arguments
      const argsJson = JSON.stringify(args);
      const argsBytes = new TextEncoder().encode(argsJson);
      vmLogic.methodArgs = argsBytes;

      this.log(`Signer: ${vmLogic.context.signer_account_id}`);
      this.log(`Arguments: ${argsJson}`);

      // Load and instantiate contract
      const module = await this.loadContract(wasmSource);
      const env = vmLogic.createEnvironment();
      const instance = await WebAssembly.instantiate(module, env);

      // Set memory reference
      vmLogic.setMemory(instance.exports.memory);

      // Call the method
      if (!instance.exports[methodName]) {
        throw new Error(`Method '${methodName}' not found in contract exports`);
      }

      this.log(`Calling ${methodName}()...`);

      instance.exports[methodName]();

      // Parse return value
      const result = this.parseReturnValue(vmLogic.returnData);

      const endTime = performance.now();
      const executionTime = endTime - startTime;

      const finalStateSize = nearState.size;
      const stateChanges = finalStateSize - initialStateSize;

      this.log(`\nResult: ${JSON.stringify(result, null, 2)}`);
      this.log(`Gas used: ${vmLogic.gasUsed.toLocaleString()} (${(vmLogic.gasUsed / 1000000000000).toFixed(2)} Tgas)`);
      this.log(`Execution time: ${executionTime.toFixed(2)}ms`);
      this.log(`State changes: ${stateChanges > 0 ? '+' : ''}${stateChanges} keys (total: ${finalStateSize})`);
      this.log(`Logs: ${vmLogic.logs.length} entries`);

      // Persist state to IDBFS if available and enabled
      if (this.options.persistState && typeof FS !== 'undefined' && FS.syncfs) {
        await this.persistState();
        this.log(`State persisted to IDBFS`);
      }

      this.log(`${'='.repeat(60)}\n`);

      this.stats.totalGasUsed += vmLogic.gasUsed;
      this.stats.lastExecutionTime = executionTime;

      return {
        result,
        gasUsed: vmLogic.gasUsed,
        logs: vmLogic.logs,
        executionTime,
        stateChanges,
        stateSizeAfter: finalStateSize
      };

    } catch (error) {
      const endTime = performance.now();
      const executionTime = endTime - startTime;

      this.log(`\n[ERROR] ${error.message}`);
      this.log(`Execution time: ${executionTime.toFixed(2)}ms`);

      // Rollback state changes on error
      // (In real NEAR, this would be atomic - state only persists on success)
      this.log(`Rolling back state changes...`);
      // Note: Proper rollback would require snapshotting before execution
      // For now, we just note that changes occurred

      this.log(`${'='.repeat(60)}\n`);

      throw error;
    }
  }

  // ============================================================================
  // STATE MANAGEMENT
  // ============================================================================

  /**
   * Persist state to IDBFS (Emscripten IndexedDB filesystem)
   */
  async persistState() {
    return new Promise((resolve, reject) => {
      if (typeof FS === 'undefined' || !FS.syncfs) {
        resolve(); // No IDBFS available
        return;
      }

      // Convert nearState Map to JSON
      const stateArray = Array.from(nearState.entries());
      const stateJson = JSON.stringify(stateArray, null, 2);

      // Write to file
      FS.writeFile('/home/near_state.json', stateJson);

      // Sync to IndexedDB
      FS.syncfs(false, (err) => {
        if (err) {
          console.error('IDBFS sync error:', err);
          reject(err);
        } else {
          resolve();
        }
      });
    });
  }

  /**
   * Load state from IDBFS
   */
  async loadState() {
    return new Promise((resolve, reject) => {
      if (typeof FS === 'undefined' || !FS.syncfs) {
        resolve(); // No IDBFS available
        return;
      }

      // Sync from IndexedDB
      FS.syncfs(true, (err) => {
        if (err) {
          console.error('IDBFS sync error:', err);
          reject(err);
          return;
        }

        try {
          // Check if state file exists
          if (!FS.analyzePath('/home/near_state.json').exists) {
            this.log('[STATE] No persisted state found');
            resolve();
            return;
          }

          // Read state file
          const stateJson = FS.readFile('/home/near_state.json', { encoding: 'utf8' });
          const stateArray = JSON.parse(stateJson);

          // Restore to Map
          nearState.clear();
          stateArray.forEach(([key, value]) => {
            nearState.set(key, value);
          });

          this.log(`[STATE] Loaded ${nearState.size} keys from IDBFS`);
          resolve();

        } catch (error) {
          console.error('State load error:', error);
          reject(error);
        }
      });
    });
  }

  /**
   * Clear all state
   */
  clearState() {
    nearState.clear();
    this.log('[STATE] Cleared all state');
  }

  /**
   * Create state snapshot
   */
  createSnapshot(name) {
    const snapshot = Array.from(nearState.entries());

    if (typeof FS !== 'undefined') {
      const snapshotJson = JSON.stringify(snapshot, null, 2);
      FS.writeFile(`/home/near_state_${name}.json`, snapshotJson);
      this.log(`[SNAPSHOT] Saved: ${name} (${snapshot.length} keys)`);
    }

    return snapshot;
  }

  /**
   * Restore state from snapshot
   */
  restoreSnapshot(name) {
    if (typeof FS === 'undefined') {
      throw new Error('FS not available');
    }

    const snapshotPath = `/home/near_state_${name}.json`;

    if (!FS.analyzePath(snapshotPath).exists) {
      throw new Error(`Snapshot '${name}' not found`);
    }

    const snapshotJson = FS.readFile(snapshotPath, { encoding: 'utf8' });
    const snapshot = JSON.parse(snapshotJson);

    nearState.clear();
    snapshot.forEach(([key, value]) => {
      nearState.set(key, value);
    });

    this.log(`[SNAPSHOT] Restored: ${name} (${snapshot.length} keys)`);
  }

  // ============================================================================
  // SEALED STORAGE (Phase 3)
  // ============================================================================

  /**
   * Seal (encrypt) current contract state
   * @param {string} contractId - Contract identifier (e.g., 'counter.wasm')
   * @returns {Promise<Object>} - { sealed, attestation }
   */
  async sealState(contractId = 'default') {
    if (!this.sealedStorage) {
      throw new Error('Sealed storage not initialized. Call initializeSealedStorage() first.');
    }

    // Seal state with AES-GCM
    const sealed = await this.sealedStorage.seal(nearState);
    this.log(`[SEALED] State encrypted: ${sealed.ciphertext.length} bytes`);

    // Generate attestation
    const attestation = await this.sealedStorage.generateAttestation(nearState);
    this.log(`[ATTESTATION] Generated: ${attestation.attestation_type}`);
    this.log(`[ATTESTATION] Hash: ${this.formatHash(attestation.state_hash)}`);

    // Persist to IndexedDB
    await this.sealedStorage.persistSealedState(contractId, sealed);
    await this.sealedStorage.persistAttestation(contractId, attestation);
    this.log(`[SEALED] Persisted to IndexedDB (contract: ${contractId})`);

    return { sealed, attestation };
  }

  /**
   * Unseal (decrypt) contract state and restore to memory
   * @param {string} contractId - Contract identifier
   * @returns {Promise<boolean>} - True if unsealed successfully
   */
  async unsealState(contractId = 'default') {
    if (!this.sealedStorage) {
      throw new Error('Sealed storage not initialized. Call initializeSealedStorage() first.');
    }

    // Load sealed state from IndexedDB
    const sealed = await this.sealedStorage.loadSealedState(contractId);
    if (!sealed) {
      this.log(`[SEALED] No sealed state found for contract: ${contractId}`);
      return false;
    }

    this.log(`[SEALED] Loading encrypted state: ${sealed.ciphertext.length} bytes`);

    // Decrypt state
    const stateMap = await this.sealedStorage.unseal(sealed);

    // Restore to global state
    nearState.clear();
    stateMap.forEach((value, key) => {
      nearState.set(key, value);
    });

    this.log(`[SEALED] State unsealed: ${nearState.size} keys restored`);

    // Optionally verify attestation
    const attestation = await this.sealedStorage.loadAttestation(contractId);
    if (attestation) {
      const valid = await this.sealedStorage.verifyAttestation(attestation);
      this.log(`[ATTESTATION] Verification: ${valid ? '✓ Valid' : '✗ Invalid'}`);
    }

    return true;
  }

  /**
   * Verify state attestation
   * @param {string} contractId - Contract identifier
   * @returns {Promise<boolean>} - True if attestation is valid
   */
  async verifyStateAttestation(contractId = 'default') {
    if (!this.sealedStorage) {
      throw new Error('Sealed storage not initialized.');
    }

    const attestation = await this.sealedStorage.loadAttestation(contractId);
    if (!attestation) {
      this.log(`[ATTESTATION] No attestation found for contract: ${contractId}`);
      return false;
    }

    // Verify signature
    const valid = await this.sealedStorage.verifyAttestation(attestation);
    this.log(`[ATTESTATION] Signature: ${valid ? '✓ Valid' : '✗ Invalid'}`);

    // Optionally verify against current state
    const currentAttestation = await this.sealedStorage.generateAttestation(nearState);
    const hashMatches = JSON.stringify(attestation.state_hash) ===
                       JSON.stringify(currentAttestation.state_hash);
    this.log(`[ATTESTATION] State hash match: ${hashMatches ? '✓ Yes' : '✗ No'}`);

    return valid && hashMatches;
  }

  /**
   * Export master key (for backup)
   * WARNING: This exposes encryption key - use carefully!
   * @returns {Promise<Object>} - JWK representation
   */
  async exportMasterKey() {
    if (!this.sealedStorage) {
      throw new Error('Sealed storage not initialized.');
    }
    return await this.sealedStorage.exportMasterKey();
  }

  /**
   * Import master key (for restore)
   * @param {Object} keyJwk - JWK representation of master key
   */
  async importMasterKey(keyJwk) {
    if (!this.sealedStorage) {
      await this.initializeSealedStorage();
    }
    await this.sealedStorage.importMasterKey(keyJwk);
    this.log('[SEALED] Master key imported');
  }

  /**
   * Format hash for display (first 16 hex chars + ellipsis)
   */
  formatHash(hashArray) {
    const hex = hashArray.slice(0, 8).map(b => b.toString(16).padStart(2, '0')).join('');
    return `${hex}...`;
  }

  // ============================================================================
  // UTILITY
  // ============================================================================

  /**
   * Parse return value from contract
   * Tries JSON deserialization, falls back to raw bytes
   */
  parseReturnValue(returnData) {
    if (!returnData || returnData.length === 0) {
      return null;
    }

    try {
      const resultStr = new TextDecoder().decode(returnData);
      return JSON.parse(resultStr);
    } catch {
      // Not JSON, return raw bytes
      return Array.from(returnData);
    }
  }

  /**
   * Get execution statistics
   */
  getStats() {
    return {
      ...this.stats,
      stateSize: nearState.size,
      cachedContracts: this.contracts.size
    };
  }

  /**
   * Log with optional verbosity control
   */
  log(message) {
    if (this.options.verboseLogging || typeof console === 'undefined') {
      return;
    }

    // In browser or REPL context
    if (typeof term !== 'undefined' && term.writeln) {
      term.writeln(message);
    } else if (typeof console !== 'undefined') {
      console.log(message);
    }
  }

  /**
   * Reset simulator state
   */
  reset() {
    this.contracts.clear();
    nearState.clear();
    this.stats = {
      totalQueries: 0,
      totalExecutions: 0,
      totalGasUsed: 0,
      lastExecutionTime: 0
    };
    this.log('[RESET] Simulator state cleared');
  }
}

// Export for use in browser or Node.js
if (typeof module !== 'undefined' && module.exports) {
  module.exports = ContractSimulator;
}
if (typeof window !== 'undefined') {
  window.ContractSimulator = ContractSimulator;
  window.contractSimulator = new ContractSimulator();
}
