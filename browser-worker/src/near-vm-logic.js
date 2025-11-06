/**
 * NEARVMLogic - Browser-based NEAR Protocol VM Host Functions
 *
 * Implements NEAR's host function interface for executing wasm32-unknown-unknown
 * contracts in the browser. Provides gas metering, storage operations, context
 * access, and cryptographic primitives that match NEAR's actual runtime.
 *
 * Architecture:
 * - Maps NEAR's 256 registers for data passing between host and guest
 * - Tracks gas consumption (1 Tgas = 1ms execution time)
 * - Manages persistent state via global nearState Map
 * - Enforces resource limits (gas, memory, storage)
 *
 * Integration:
 * - Can be used standalone in browser
 * - Can be integrated with WASM REPL via Emscripten
 * - Can be extended with CapabilityVMLogic for restricted execution
 *
 * @author OutLayer Team
 * @version 1.0.0
 */

class NEARVMLogic {
  constructor(isViewCall = true, context = {}) {
    this.isViewCall = isViewCall;

    // Registers for data passing (NEAR uses 256 registers)
    this.registers = new Array(256).fill(null);

    // Gas tracking
    this.gasUsed = 0;
    this.gasLimit = context.gasLimit || 300000000000000; // 300 Tgas default

    // Execution context
    this.logs = [];
    this.returnData = null;
    this.panicked = false;
    this.panicMessage = null;

    // Method arguments (set by ContractSimulator)
    this.methodArgs = null;

    // Reference to global state (set by ContractSimulator)
    this.state = null;

    // WASM memory reference (set after instantiation)
    this.memory = null;

    // Context from VMContext (mimic NEAR's execution context)
    this.context = {
      current_account_id: context.current_account_id || 'simulator.near',
      signer_account_id: context.signer_account_id || 'alice.near',
      predecessor_account_id: context.predecessor_account_id || 'alice.near',
      block_index: context.block_index || Math.floor(Date.now() / 1000),
      block_timestamp: context.block_timestamp || Date.now() * 1000000, // nanoseconds
      epoch_height: context.epoch_height || 1,
      account_balance: context.account_balance || '1000000000000000000000000', // 1 NEAR
      account_locked_balance: context.account_locked_balance || '0',
      storage_usage: context.storage_usage || 1000,
      attached_deposit: context.attached_deposit || '0',
      prepaid_gas: this.gasLimit,
      random_seed: context.random_seed || new Uint8Array(32).fill(42), // Deterministic
      is_view: isViewCall,
      output_data_receivers: []
    };

    // Gas costs (from NEAR Protocol 1.22.0)
    this.gasCosts = {
      wasm_regular_op: 2207874,
      storage_write_base: 64196736000,
      storage_write_per_byte: 310382320,
      storage_read_base: 56356845750,
      storage_read_per_byte: 30952380,
      storage_has_key_base: 54039896625,
      storage_has_key_per_byte: 30790845,
      storage_remove_base: 53473030500,
      storage_remove_ret_per_byte: 30459880,
      register_base: 2207874,
      register_per_byte: 2207874,
      log_base: 3543313050,
      log_byte: 13198791,
      sha256_base: 4540970250,
      sha256_byte: 24117351,
      keccak256_base: 5879491275,
      keccak256_byte: 21471105,
      ripemd160_base: 853675086500,
      ripemd160_block: 680107040,
      ecrecover_base: 278821988457,
      promise_create_base: 120192000000,
      promise_create_per_byte: 120192000,
    };
  }

  // ============================================================================
  // GAS METERING
  // ============================================================================

  /**
   * Track gas consumption and enforce limits
   */
  useGas(amount) {
    this.gasUsed += amount;

    if (this.gasUsed > this.gasLimit) {
      throw new Error(`Gas limit exceeded: ${this.gasUsed} > ${this.gasLimit}`);
    }
  }

  /**
   * Get remaining gas
   */
  getRemainingGas() {
    return this.gasLimit - this.gasUsed;
  }

  // ============================================================================
  // MEMORY OPERATIONS
  // ============================================================================

  /**
   * Set reference to WASM linear memory
   */
  setMemory(memory) {
    this.memory = memory;
  }

  /**
   * Read UTF-8 string from WASM memory
   */
  readString(ptr, len) {
    if (!this.memory) {
      throw new Error('WASM memory not set');
    }

    const bytes = new Uint8Array(this.memory.buffer, ptr, len);
    return new TextDecoder().decode(bytes);
  }

  /**
   * Read bytes from WASM memory
   */
  readBytes(ptr, len) {
    if (!this.memory) {
      throw new Error('WASM memory not set');
    }

    return new Uint8Array(this.memory.buffer, ptr, len);
  }

  /**
   * Write bytes to WASM memory
   */
  writeBytes(ptr, bytes) {
    if (!this.memory) {
      throw new Error('WASM memory not set');
    }

    const target = new Uint8Array(this.memory.buffer, ptr, bytes.length);
    target.set(bytes);
  }

  /**
   * Write u64 to WASM memory (little endian)
   */
  writeU64(ptr, value) {
    if (!this.memory) {
      throw new Error('WASM memory not set');
    }

    const view = new DataView(this.memory.buffer);
    view.setBigUint64(ptr, BigInt(value), true);
  }

  /**
   * Write u128 to WASM memory (little endian)
   */
  writeU128(ptr, value) {
    if (!this.memory) {
      throw new Error('WASM memory not set');
    }

    // Split 128-bit value into two 64-bit parts
    const bigValue = BigInt(value);
    const low = bigValue & 0xFFFFFFFFFFFFFFFFn;
    const high = bigValue >> 64n;

    const view = new DataView(this.memory.buffer);
    view.setBigUint64(ptr, low, true);
    view.setBigUint64(ptr + 8, high, true);
  }

  // ============================================================================
  // REGISTER OPERATIONS (Tier 1 - Essential)
  // ============================================================================

  /**
   * Get length of data in register
   * Host: register_len(register_id: u64) -> u64
   */
  register_len(register_id) {
    this.useGas(this.gasCosts.register_base);

    const data = this.registers[register_id];
    return data ? BigInt(data.length) : 0xFFFFFFFFFFFFFFFFn; // u64::MAX if empty
  }

  /**
   * Read data from register into WASM memory
   * Host: read_register(register_id: u64, ptr: u64)
   */
  read_register(register_id, ptr) {
    const data = this.registers[register_id];

    if (!data) {
      throw new Error(`Register ${register_id} is empty`);
    }

    this.useGas(this.gasCosts.register_base + data.length * this.gasCosts.register_per_byte);
    this.writeBytes(ptr, data);
  }

  /**
   * Write data from WASM memory into register
   * Host: write_register(register_id: u64, data_len: u64, data_ptr: u64)
   */
  write_register(register_id, data_len, data_ptr) {
    const data = this.readBytes(data_ptr, Number(data_len));
    this.useGas(this.gasCosts.register_base + data.length * this.gasCosts.register_per_byte);
    this.registers[register_id] = data;
  }

  // ============================================================================
  // INPUT/OUTPUT (Tier 1 - Essential)
  // ============================================================================

  /**
   * Read method arguments into register
   * Host: input(register_id: u64)
   *
   * CRITICAL: This is how contracts receive their arguments!
   */
  input(register_id) {
    const argsBytes = this.methodArgs || new Uint8Array(0);

    this.useGas(this.gasCosts.register_base + argsBytes.length * this.gasCosts.register_per_byte);
    this.registers[register_id] = argsBytes;
  }

  /**
   * Return data from contract execution
   * Host: value_return(value_len: u64, value_ptr: u64)
   */
  value_return(value_len, value_ptr) {
    const value = this.readBytes(value_ptr, Number(value_len));
    this.useGas(this.gasCosts.register_base + value.length * this.gasCosts.register_per_byte);
    this.returnData = value;
  }

  // ============================================================================
  // LOGGING (Tier 1 - Essential)
  // ============================================================================

  /**
   * Log UTF-8 string
   * Host: log_utf8(len: u64, ptr: u64)
   */
  log_utf8(len, ptr) {
    const message = this.readString(ptr, Number(len));
    this.useGas(this.gasCosts.log_base + message.length * this.gasCosts.log_byte);

    this.logs.push(message);

    if (typeof console !== 'undefined') {
      console.log(`[NEAR LOG] ${message}`);
    }
  }

  /**
   * Log UTF-16 string
   * Host: log_utf16(len: u64, ptr: u64)
   */
  log_utf16(len, ptr) {
    const bytes = this.readBytes(ptr, Number(len) * 2);
    const message = new TextDecoder('utf-16le').decode(bytes);
    this.useGas(this.gasCosts.log_base + bytes.length * this.gasCosts.log_byte);

    this.logs.push(message);

    if (typeof console !== 'undefined') {
      console.log(`[NEAR LOG] ${message}`);
    }
  }

  // ============================================================================
  // PANIC (Tier 1 - Essential)
  // ============================================================================

  /**
   * Panic without message
   * Host: panic()
   */
  panic() {
    this.panicked = true;
    throw new Error('Contract panicked');
  }

  /**
   * Panic with UTF-8 message
   * Host: panic_utf8(len: u64, ptr: u64)
   */
  panic_utf8(len, ptr) {
    const message = this.readString(ptr, Number(len));
    this.panicked = true;
    this.panicMessage = message;
    throw new Error(`Contract panicked: ${message}`);
  }

  // ============================================================================
  // STORAGE OPERATIONS (Tier 1 - Essential)
  // ============================================================================

  /**
   * Write to storage
   * Host: storage_write(key_len: u64, key_ptr: u64, value_len: u64, value_ptr: u64, register_id: u64) -> u64
   * Returns: 1 if key existed (eviction), 0 if new key
   */
  storage_write(key_len, key_ptr, value_len, value_ptr, register_id) {
    if (this.isViewCall) {
      throw new Error('Cannot modify state in view call');
    }

    if (!this.state) {
      throw new Error('State not initialized');
    }

    const key = this.readString(key_ptr, Number(key_len));
    const value = this.readBytes(value_ptr, Number(value_len));

    // Calculate gas cost
    const storageCost = this.gasCosts.storage_write_base +
                       (key.length + value.length) * this.gasCosts.storage_write_per_byte;
    this.useGas(storageCost);

    // Check if key exists (for return value)
    const hadValue = this.state.has(key);

    // Store old value in register if requested
    if (hadValue && register_id !== 0xFFFFFFFFFFFFFFFFn) {
      const oldEntry = this.state.get(key);
      this.registers[Number(register_id)] = oldEntry.data;
    }

    // Write to state
    this.state.set(key, {
      data: value,
      timestamp: Date.now()
    });

    if (typeof console !== 'undefined') {
      console.log(`[STORAGE WRITE] ${key.substring(0, 30)}... (${value.length} bytes)`);
    }

    return hadValue ? 1n : 0n;
  }

  /**
   * Read from storage
   * Host: storage_read(key_len: u64, key_ptr: u64, register_id: u64) -> u64
   * Returns: 1 if found, 0 if not found
   */
  storage_read(key_len, key_ptr, register_id) {
    if (!this.state) {
      throw new Error('State not initialized');
    }

    const key = this.readString(key_ptr, Number(key_len));

    this.useGas(this.gasCosts.storage_read_base + key.length * this.gasCosts.storage_read_per_byte);

    const entry = this.state.get(key);

    if (!entry) {
      if (typeof console !== 'undefined') {
        console.log(`[STORAGE READ] ${key} -> NOT FOUND`);
      }
      return 0n;
    }

    // Store in register
    this.registers[Number(register_id)] = entry.data;

    // Additional gas for reading value
    this.useGas(entry.data.length * this.gasCosts.storage_read_per_byte);

    if (typeof console !== 'undefined') {
      console.log(`[STORAGE READ] ${key} -> ${entry.data.length} bytes`);
    }

    return 1n;
  }

  /**
   * Check if storage key exists
   * Host: storage_has_key(key_len: u64, key_ptr: u64) -> u64
   * Returns: 1 if exists, 0 if not
   */
  storage_has_key(key_len, key_ptr) {
    if (!this.state) {
      throw new Error('State not initialized');
    }

    const key = this.readString(key_ptr, Number(key_len));

    this.useGas(this.gasCosts.storage_has_key_base +
               key.length * this.gasCosts.storage_has_key_per_byte);

    return this.state.has(key) ? 1n : 0n;
  }

  /**
   * Remove from storage
   * Host: storage_remove(key_len: u64, key_ptr: u64, register_id: u64) -> u64
   * Returns: 1 if removed, 0 if didn't exist
   */
  storage_remove(key_len, key_ptr, register_id) {
    if (this.isViewCall) {
      throw new Error('Cannot modify state in view call');
    }

    if (!this.state) {
      throw new Error('State not initialized');
    }

    const key = this.readString(key_ptr, Number(key_len));

    this.useGas(this.gasCosts.storage_remove_base);

    if (this.state.has(key)) {
      const entry = this.state.get(key);

      // Store removed value in register if requested
      if (register_id !== 0xFFFFFFFFFFFFFFFFn) {
        this.registers[Number(register_id)] = entry.data;
        this.useGas(entry.data.length * this.gasCosts.storage_remove_ret_per_byte);
      }

      this.state.delete(key);

      if (typeof console !== 'undefined') {
        console.log(`[STORAGE REMOVE] ${key}`);
      }

      return 1n;
    }

    return 0n;
  }

  // ============================================================================
  // CONTEXT GETTERS (Tier 1 - Essential)
  // ============================================================================

  /**
   * Get current account ID
   * Host: current_account_id(register_id: u64)
   */
  current_account_id(register_id) {
    const accountId = new TextEncoder().encode(this.context.current_account_id);
    this.useGas(this.gasCosts.register_base + accountId.length * this.gasCosts.register_per_byte);
    this.registers[Number(register_id)] = accountId;
  }

  /**
   * Get signer account ID
   * Host: signer_account_id(register_id: u64)
   */
  signer_account_id(register_id) {
    const accountId = new TextEncoder().encode(this.context.signer_account_id);
    this.useGas(this.gasCosts.register_base + accountId.length * this.gasCosts.register_per_byte);
    this.registers[Number(register_id)] = accountId;
  }

  /**
   * Get predecessor account ID
   * Host: predecessor_account_id(register_id: u64)
   */
  predecessor_account_id(register_id) {
    const accountId = new TextEncoder().encode(this.context.predecessor_account_id);
    this.useGas(this.gasCosts.register_base + accountId.length * this.gasCosts.register_per_byte);
    this.registers[Number(register_id)] = accountId;
  }

  /**
   * Get block index
   * Host: block_index() -> u64
   */
  block_index() {
    this.useGas(this.gasCosts.register_base);
    return BigInt(this.context.block_index);
  }

  /**
   * Get block timestamp (nanoseconds)
   * Host: block_timestamp() -> u64
   */
  block_timestamp() {
    this.useGas(this.gasCosts.register_base);
    return BigInt(this.context.block_timestamp);
  }

  /**
   * Get epoch height
   * Host: epoch_height() -> u64
   */
  epoch_height() {
    this.useGas(this.gasCosts.register_base);
    return BigInt(this.context.epoch_height);
  }

  /**
   * Get account balance
   * Host: account_balance(balance_ptr: u64)
   */
  account_balance(balance_ptr) {
    this.useGas(this.gasCosts.register_base);
    this.writeU128(balance_ptr, this.context.account_balance);
  }

  /**
   * Get account locked balance
   * Host: account_locked_balance(balance_ptr: u64)
   */
  account_locked_balance(balance_ptr) {
    this.useGas(this.gasCosts.register_base);
    this.writeU128(balance_ptr, this.context.account_locked_balance);
  }

  /**
   * Get attached deposit
   * Host: attached_deposit(balance_ptr: u64)
   */
  attached_deposit(balance_ptr) {
    this.useGas(this.gasCosts.register_base);
    this.writeU128(balance_ptr, this.context.attached_deposit);
  }

  /**
   * Get prepaid gas
   * Host: prepaid_gas() -> u64
   */
  prepaid_gas() {
    this.useGas(this.gasCosts.register_base);
    return BigInt(this.context.prepaid_gas);
  }

  /**
   * Get used gas
   * Host: used_gas() -> u64
   */
  used_gas() {
    this.useGas(this.gasCosts.register_base);
    return BigInt(this.gasUsed);
  }

  /**
   * Get storage usage
   * Host: storage_usage() -> u64
   */
  storage_usage() {
    this.useGas(this.gasCosts.register_base);
    return BigInt(this.context.storage_usage);
  }

  // ============================================================================
  // CRYPTOGRAPHY (Tier 2 - Important)
  // ============================================================================

  /**
   * Compute SHA-256 hash
   * Host: sha256(value_len: u64, value_ptr: u64, register_id: u64)
   */
  async sha256(value_len, value_ptr, register_id) {
    const value = this.readBytes(value_ptr, Number(value_len));

    this.useGas(this.gasCosts.sha256_base + value.length * this.gasCosts.sha256_byte);

    // Use browser's SubtleCrypto API
    const hashBuffer = await crypto.subtle.digest('SHA-256', value);
    const hash = new Uint8Array(hashBuffer);

    this.registers[Number(register_id)] = hash;
  }

  /**
   * Compute Keccak-256 hash
   * Host: keccak256(value_len: u64, value_ptr: u64, register_id: u64)
   *
   * NOTE: Requires external library (e.g., keccak256 from js-sha3)
   */
  keccak256(value_len, value_ptr, register_id) {
    this.useGas(this.gasCosts.keccak256_base + Number(value_len) * this.gasCosts.keccak256_byte);

    if (typeof console !== 'undefined') {
      console.warn('[CRYPTO] keccak256: Not implemented (requires js-sha3 library)');
    }

    // Placeholder: return zeros
    this.registers[Number(register_id)] = new Uint8Array(32);
  }

  /**
   * Compute RIPEMD-160 hash
   * Host: ripemd160(value_len: u64, value_ptr: u64, register_id: u64)
   *
   * NOTE: Requires external library
   */
  ripemd160(value_len, value_ptr, register_id) {
    this.useGas(this.gasCosts.ripemd160_base);

    if (typeof console !== 'undefined') {
      console.warn('[CRYPTO] ripemd160: Not implemented (requires external library)');
    }

    // Placeholder: return zeros
    this.registers[Number(register_id)] = new Uint8Array(20);
  }

  // ============================================================================
  // PROMISES (Tier 3 - Advanced, Simplified)
  // ============================================================================

  /**
   * Create promise for cross-contract call
   * Host: promise_create(account_id_len: u64, account_id_ptr: u64,
   *                      method_name_len: u64, method_name_ptr: u64,
   *                      arguments_len: u64, arguments_ptr: u64,
   *                      amount_ptr: u64, gas: u64) -> u64
   * Returns: promise_id
   *
   * NOTE: Simplified implementation - doesn't actually execute cross-contract calls
   */
  promise_create(account_id_len, account_id_ptr, method_name_len, method_name_ptr,
                arguments_len, arguments_ptr, amount_ptr, gas) {
    if (this.isViewCall) {
      throw new Error('Cannot create promises in view call');
    }

    const accountId = this.readString(account_id_ptr, Number(account_id_len));
    const methodName = this.readString(method_name_ptr, Number(method_name_len));
    const args = this.readBytes(arguments_ptr, Number(arguments_len));

    this.useGas(this.gasCosts.promise_create_base +
               Number(arguments_len) * this.gasCosts.promise_create_per_byte);

    if (typeof console !== 'undefined') {
      console.log(`[PROMISE] Creating promise: ${accountId}::${methodName} (${args.length} bytes args)`);
    }

    // Simplified: just track the promise, don't execute
    const promiseId = this.context.output_data_receivers.length;
    this.context.output_data_receivers.push({
      receiver_id: accountId,
      method_name: methodName
    });

    return BigInt(promiseId);
  }

  // ============================================================================
  // ENVIRONMENT CREATION
  // ============================================================================

  /**
   * Create the full WASM import environment
   * This is what gets passed to WebAssembly.instantiate()
   */
  createEnvironment() {
    // Bind all methods to preserve 'this' context
    const env = {
      // Memory operations (handled by WASM module itself)
      memory: new WebAssembly.Memory({ initial: 256, maximum: 512 }), // Will be overridden

      // Registers
      register_len: this.register_len.bind(this),
      read_register: this.read_register.bind(this),
      write_register: this.write_register.bind(this),

      // Input/Output
      input: this.input.bind(this),
      value_return: this.value_return.bind(this),

      // Logging
      log_utf8: this.log_utf8.bind(this),
      log_utf16: this.log_utf16.bind(this),

      // Panic
      panic: this.panic.bind(this),
      panic_utf8: this.panic_utf8.bind(this),

      // Storage
      storage_write: this.storage_write.bind(this),
      storage_read: this.storage_read.bind(this),
      storage_has_key: this.storage_has_key.bind(this),
      storage_remove: this.storage_remove.bind(this),

      // Context
      current_account_id: this.current_account_id.bind(this),
      signer_account_id: this.signer_account_id.bind(this),
      predecessor_account_id: this.predecessor_account_id.bind(this),
      block_index: this.block_index.bind(this),
      block_timestamp: this.block_timestamp.bind(this),
      epoch_height: this.epoch_height.bind(this),
      account_balance: this.account_balance.bind(this),
      account_locked_balance: this.account_locked_balance.bind(this),
      attached_deposit: this.attached_deposit.bind(this),
      prepaid_gas: this.prepaid_gas.bind(this),
      used_gas: this.used_gas.bind(this),
      storage_usage: this.storage_usage.bind(this),

      // Cryptography
      sha256: this.sha256.bind(this),
      keccak256: this.keccak256.bind(this),
      ripemd160: this.ripemd160.bind(this),

      // Promises (simplified)
      promise_create: this.promise_create.bind(this),
    };

    return { env };
  }
}

// Export for use in browser or Node.js
if (typeof module !== 'undefined' && module.exports) {
  module.exports = NEARVMLogic;
}
if (typeof window !== 'undefined') {
  window.NEARVMLogic = NEARVMLogic;
}
