/**
 * RPC Client - NEAR RPC calls via Coordinator throttle proxy
 *
 * Routes all NEAR RPC requests through the coordinator's /near-rpc endpoint
 * which provides:
 * - Token-bucket rate limiting (5 rps anon, 20 rps with API key)
 * - Automatic retry on 429 with backoff
 * - Request/response logging
 * - Centralized infrastructure protection
 *
 * This replaces direct calls to rpc.testnet.near.org or other RPC endpoints.
 *
 * @author OutLayer Team
 * @version 1.0.0
 */

class RPCClient {
  constructor(options = {}) {
    // Coordinator proxy URL (default: local development)
    this.coordinatorUrl = options.coordinatorUrl || 'http://localhost:8080';

    // API key for higher rate limits (optional)
    this.apiKey = options.apiKey || null;

    // Network (testnet, mainnet)
    this.network = options.network || 'testnet';

    // Request ID counter
    this.requestId = 0;

    // Statistics
    this.stats = {
      totalRequests: 0,
      successfulRequests: 0,
      failedRequests: 0,
      retriedRequests: 0,
      totalRetryDelay: 0,
    };

    // Verbose logging
    this.verbose = options.verbose || false;
  }

  /**
   * Send NEAR RPC request through coordinator proxy
   *
   * @param {string} method - RPC method (e.g., "query", "broadcast_tx_commit")
   * @param {Object} params - Method parameters
   * @param {Object} options - Request options (retries, timeout)
   * @returns {Promise<Object>} RPC response result
   */
  async call(method, params, options = {}) {
    const requestId = `rpc-${Date.now()}-${++this.requestId}`;

    const rpcRequest = {
      jsonrpc: '2.0',
      id: requestId,
      method,
      params,
    };

    this.stats.totalRequests++;

    if (this.verbose) {
      console.log(`[RPC] → ${method}`, params);
    }

    // Retry configuration
    const maxRetries = options.maxRetries !== undefined ? options.maxRetries : 3;
    const timeout = options.timeout || 30000; // 30 seconds default

    let lastError;

    for (let attempt = 0; attempt <= maxRetries; attempt++) {
      try {
        // Build request to coordinator proxy
        const headers = {
          'Content-Type': 'application/json',
        };

        // Add API key for higher rate limits
        if (this.apiKey) {
          headers['Authorization'] = `Bearer ${this.apiKey}`;
        }

        // Call coordinator /near-rpc endpoint
        const controller = new AbortController();
        const timeoutId = setTimeout(() => controller.abort(), timeout);

        const response = await fetch(`${this.coordinatorUrl}/near-rpc`, {
          method: 'POST',
          headers,
          body: JSON.stringify(rpcRequest),
          signal: controller.signal,
        });

        clearTimeout(timeoutId);

        // Handle rate limiting from coordinator
        if (response.status === 429) {
          const retryAfter = response.headers.get('Retry-After') || '5';
          const delayMs = parseInt(retryAfter) * 1000;

          this.stats.retriedRequests++;
          this.stats.totalRetryDelay += delayMs;

          if (attempt < maxRetries) {
            if (this.verbose) {
              console.warn(`[RPC] Rate limited, retrying in ${delayMs}ms (attempt ${attempt + 1}/${maxRetries})`);
            }

            await this.sleep(delayMs);
            continue; // Retry
          } else {
            throw new Error(`Rate limit exceeded after ${maxRetries} retries`);
          }
        }

        // Handle other HTTP errors
        if (!response.ok) {
          const errorText = await response.text();
          throw new Error(`HTTP ${response.status}: ${errorText}`);
        }

        // Parse JSON response
        const rpcResponse = await response.json();

        // Check for RPC error
        if (rpcResponse.error) {
          throw new Error(`RPC error: ${JSON.stringify(rpcResponse.error)}`);
        }

        // Success!
        this.stats.successfulRequests++;

        if (this.verbose) {
          console.log(`[RPC] ← ${method} success`);
        }

        return rpcResponse.result;

      } catch (error) {
        lastError = error;

        // Don't retry on certain errors
        if (error.name === 'AbortError') {
          // Timeout - don't retry
          this.stats.failedRequests++;
          throw new Error(`RPC request timed out after ${timeout}ms`);
        }

        if (error.message.includes('RPC error')) {
          // RPC-level error (e.g., invalid params) - don't retry
          this.stats.failedRequests++;
          throw error;
        }

        // Network error or other issue - retry with exponential backoff
        if (attempt < maxRetries) {
          const backoffMs = Math.min(1000 * Math.pow(2, attempt), 10000); // Max 10s

          if (this.verbose) {
            console.warn(`[RPC] Error, retrying in ${backoffMs}ms: ${error.message}`);
          }

          this.stats.retriedRequests++;
          this.stats.totalRetryDelay += backoffMs;

          await this.sleep(backoffMs);
          continue; // Retry
        }
      }
    }

    // All retries exhausted
    this.stats.failedRequests++;
    throw lastError;
  }

  /**
   * Helper: Sleep for specified milliseconds
   */
  sleep(ms) {
    return new Promise(resolve => setTimeout(resolve, ms));
  }

  // ============================================================================
  // COMMON NEAR RPC METHODS
  // ============================================================================

  /**
   * Query contract view method
   *
   * @param {string} contractId - Contract account ID
   * @param {string} methodName - View method name
   * @param {Object} args - Method arguments (will be JSON serialized)
   * @param {string} finality - Block finality ("optimistic" or "final")
   * @returns {Promise<any>} Deserialized result
   */
  async viewCall(contractId, methodName, args = {}, finality = 'final') {
    const argsBase64 = btoa(JSON.stringify(args));

    const result = await this.call('query', {
      request_type: 'call_function',
      finality,
      account_id: contractId,
      method_name: methodName,
      args_base64: argsBase64,
    });

    // Decode result
    const resultJson = atob(result.result.map(byte => String.fromCharCode(byte)).join(''));
    return JSON.parse(resultJson);
  }

  /**
   * Get account state
   *
   * @param {string} accountId - Account ID to query
   * @param {string} finality - Block finality
   * @returns {Promise<Object>} Account info
   */
  async viewAccount(accountId, finality = 'final') {
    return await this.call('query', {
      request_type: 'view_account',
      finality,
      account_id: accountId,
    });
  }

  /**
   * Get account access keys
   *
   * @param {string} accountId - Account ID
   * @param {string} finality - Block finality
   * @returns {Promise<Array>} Access keys
   */
  async viewAccessKeys(accountId, finality = 'final') {
    const result = await this.call('query', {
      request_type: 'view_access_key_list',
      finality,
      account_id: accountId,
    });

    return result.keys;
  }

  /**
   * Get block info
   *
   * @param {Object} blockReference - { block_id } or { finality }
   * @returns {Promise<Object>} Block data
   */
  async getBlock(blockReference = { finality: 'final' }) {
    return await this.call('block', blockReference);
  }

  /**
   * Get transaction status
   *
   * @param {string} txHash - Transaction hash
   * @param {string} accountId - Sender account ID
   * @returns {Promise<Object>} Transaction result
   */
  async getTxStatus(txHash, accountId) {
    return await this.call('tx', {
      tx_hash: txHash,
      sender_account_id: accountId,
    });
  }

  /**
   * Broadcast signed transaction
   *
   * @param {string} signedTxBase64 - Base64-encoded signed transaction
   * @returns {Promise<Object>} Transaction result
   */
  async sendTransaction(signedTxBase64) {
    return await this.call('broadcast_tx_commit', [signedTxBase64]);
  }

  /**
   * Get gas price
   *
   * @param {string|number} blockId - Block ID or null for latest
   * @returns {Promise<string>} Gas price in yoctoNEAR
   */
  async getGasPrice(blockId = null) {
    const result = await this.call('gas_price', [blockId]);
    return result.gas_price;
  }

  /**
   * Get network status
   *
   * @returns {Promise<Object>} Network info
   */
  async getStatus() {
    return await this.call('status', []);
  }

  // ============================================================================
  // STATISTICS
  // ============================================================================

  /**
   * Get client statistics
   *
   * @returns {Object} Stats object
   */
  getStats() {
    return {
      ...this.stats,
      successRate: this.stats.totalRequests > 0
        ? (this.stats.successfulRequests / this.stats.totalRequests * 100).toFixed(2) + '%'
        : 'N/A',
      avgRetryDelay: this.stats.retriedRequests > 0
        ? (this.stats.totalRetryDelay / this.stats.retriedRequests).toFixed(0) + 'ms'
        : 'N/A',
    };
  }

  /**
   * Reset statistics
   */
  resetStats() {
    this.stats = {
      totalRequests: 0,
      successfulRequests: 0,
      failedRequests: 0,
      retriedRequests: 0,
      totalRetryDelay: 0,
    };
  }
}

// Export for use in other modules
if (typeof module !== 'undefined' && module.exports) {
  module.exports = { RPCClient };
}

// Browser global
if (typeof window !== 'undefined') {
  window.RPCClient = RPCClient;
}
