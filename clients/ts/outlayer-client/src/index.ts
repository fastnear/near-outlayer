/**
 * NEAR OutLayer TypeScript Client
 *
 * Phase 1 Hardening: Full support for NEAR-signed authentication and idempotency.
 *
 * ## Features
 * - Bearer token auth (dev/testing)
 * - NEAR-signed ed25519 auth (production)
 * - Idempotency-Key generation
 * - Type-safe request/response handling
 * - Automatic retry with exponential backoff
 *
 * ## Usage
 *
 * ```typescript
 * // Bearer auth (development)
 * const client = new OutLayerClient({
 *   baseUrl: 'http://localhost:8080',
 *   auth: { type: 'bearer', token: 'dev-token' }
 * });
 *
 * // NEAR-signed auth (production)
 * const client = new OutLayerClient({
 *   baseUrl: 'https://outlayer.near.org',
 *   auth: {
 *     type: 'near',
 *     accountId: 'worker.near',
 *     privateKey: 'ed25519:...'
 *   }
 * });
 *
 * // Make requests
 * const result = await client.claimJobs({
 *   requestId: 123,
 *   dataId: 'abc...',
 *   wasmChecksum: 'def...',
 *   idempotencyKey: 'uuid-v4'  // Optional
 * });
 * ```
 */

import { KeyPair } from 'near-api-js';
import * as crypto from 'crypto';
import { sha256 } from 'js-sha256';
import bs58 from 'bs58';

// ============================================================================
// Types
// ============================================================================

export interface OutLayerClientConfig {
  baseUrl: string;
  auth: BearerAuth | NearAuth;
  timeout?: number;  // Request timeout in ms (default: 30000)
  retries?: number;  // Max retry attempts (default: 3)
}

export interface BearerAuth {
  type: 'bearer';
  token: string;
}

export interface NearAuth {
  type: 'near';
  accountId: string;
  privateKey: string;  // ed25519:base58_encoded
}

export interface ClaimJobsRequest {
  requestId: number;
  dataId: string;
  wasmChecksum?: string;
  idempotencyKey?: string;  // Optional UUID for deduplication
}

export interface ClaimJobsResponse {
  jobs: JobType[];
}

export type JobType = 'compile' | 'execute';

export interface UploadWasmRequest {
  requestId: number;
  dataId: string;
  wasmBytes: Uint8Array;
  checksum: string;
  idempotencyKey?: string;
}

export interface UploadWasmResponse {
  success: boolean;
  checksum: string;
}

export interface SubmitResultRequest {
  requestId: number;
  success: boolean;
  output?: any;
  error?: string;
  resourcesUsed: ResourceMetrics;
  compilationNote?: string;
  idempotencyKey?: string;
}

export interface ResourceMetrics {
  instructions: number;
  timeMs: number;
  compileTimeMs?: number;
}

export interface SubmitResultResponse {
  success: boolean;
}

// ============================================================================
// Client Implementation
// ============================================================================

export class OutLayerClient {
  private config: OutLayerClientConfig;
  private keyPair?: KeyPair;

  constructor(config: OutLayerClientConfig) {
    this.config = {
      timeout: 30000,
      retries: 3,
      ...config,
    };

    // Parse NEAR key pair if using NEAR auth
    if (config.auth.type === 'near') {
      this.keyPair = KeyPair.fromString(config.auth.privateKey);
    }
  }

  // =========================================================================
  // Job Claim API
  // =========================================================================

  /**
   * Claim jobs for a request
   *
   * Returns list of jobs to execute (e.g., ['compile', 'execute'] or ['execute'] if cached)
   */
  async claimJobs(request: ClaimJobsRequest): Promise<ClaimJobsResponse> {
    const params = new URLSearchParams({
      request_id: request.requestId.toString(),
      data_id: request.dataId,
    });

    if (request.wasmChecksum) {
      params.append('wasm_checksum', request.wasmChecksum);
    }

    return this.request<ClaimJobsResponse>(
      'POST',
      `/jobs/claim?${params.toString()}`,
      undefined,
      request.idempotencyKey
    );
  }

  // =========================================================================
  // WASM Upload API
  // =========================================================================

  /**
   * Upload compiled WASM to coordinator cache
   */
  async uploadWasm(request: UploadWasmRequest): Promise<UploadWasmResponse> {
    const params = new URLSearchParams({
      request_id: request.requestId.toString(),
      data_id: request.dataId,
      checksum: request.checksum,
    });

    return this.request<UploadWasmResponse>(
      'POST',
      `/wasm/upload?${params.toString()}`,
      request.wasmBytes,
      request.idempotencyKey
    );
  }

  // =========================================================================
  // Result Submission API
  // =========================================================================

  /**
   * Submit execution result to coordinator
   */
  async submitResult(request: SubmitResultRequest): Promise<SubmitResultResponse> {
    return this.request<SubmitResultResponse>(
      'POST',
      '/results/submit',
      request,
      request.idempotencyKey
    );
  }

  // =========================================================================
  // Core Request Handler
  // =========================================================================

  private async request<T>(
    method: string,
    path: string,
    body?: any,
    idempotencyKey?: string
  ): Promise<T> {
    const url = `${this.config.baseUrl}${path}`;

    // Prepare body
    let bodyBytes: Uint8Array;
    let contentType: string;

    if (body instanceof Uint8Array) {
      bodyBytes = body;
      contentType = 'application/octet-stream';
    } else if (body !== undefined) {
      const bodyStr = JSON.stringify(body);
      bodyBytes = new TextEncoder().encode(bodyStr);
      contentType = 'application/json';
    } else {
      bodyBytes = new Uint8Array(0);
      contentType = 'application/json';
    }

    // Build headers
    const headers: Record<string, string> = {
      'Content-Type': contentType,
    };

    // Add authentication headers
    if (this.config.auth.type === 'bearer') {
      headers['Authorization'] = `Bearer ${this.config.auth.token}`;
    } else if (this.config.auth.type === 'near') {
      const nearHeaders = this.generateNearAuthHeaders(method, path, bodyBytes);
      Object.assign(headers, nearHeaders);
    }

    // Add idempotency key if provided
    if (idempotencyKey) {
      headers['Idempotency-Key'] = idempotencyKey;
    }

    // Make request with retries
    let lastError: Error | undefined;

    for (let attempt = 0; attempt <= (this.config.retries || 3); attempt++) {
      try {
        const response = await fetch(url, {
          method,
          headers,
          body: bodyBytes.length > 0 ? bodyBytes : undefined,
          signal: AbortSignal.timeout(this.config.timeout || 30000),
        });

        if (!response.ok) {
          const errorText = await response.text();
          throw new Error(`HTTP ${response.status}: ${errorText}`);
        }

        // Check if this was a cached idempotent response
        const isCached = response.headers.get('X-Idempotency-Replay') === 'true';
        if (isCached) {
          console.log(`[OutLayer] Received cached response for idempotency key: ${idempotencyKey}`);
        }

        return await response.json() as T;
      } catch (error) {
        lastError = error as Error;

        // Don't retry on 4xx errors (client errors)
        if (error instanceof Error && error.message.includes('HTTP 4')) {
          throw error;
        }

        // Exponential backoff
        if (attempt < (this.config.retries || 3)) {
          const backoffMs = Math.pow(2, attempt) * 1000;
          console.log(`[OutLayer] Request failed, retrying in ${backoffMs}ms...`);
          await new Promise(resolve => setTimeout(resolve, backoffMs));
        }
      }
    }

    throw lastError || new Error('Request failed after all retries');
  }

  // =========================================================================
  // NEAR-Signed Authentication
  // =========================================================================

  /**
   * Generate NEAR-signed authentication headers
   *
   * Protocol: Sign `method|path|body_sha256|timestamp` with ed25519 key
   */
  private generateNearAuthHeaders(
    method: string,
    path: string,
    body: Uint8Array
  ): Record<string, string> {
    if (this.config.auth.type !== 'near' || !this.keyPair) {
      throw new Error('NEAR auth not configured');
    }

    // Compute body hash
    const bodyHash = sha256(body);

    // Get current timestamp
    const timestamp = Math.floor(Date.now() / 1000);

    // Construct message: method|path|body_hash|timestamp
    const message = `${method}|${path}|${bodyHash}|${timestamp}`;

    // Sign with ed25519 private key
    const messageBytes = new TextEncoder().encode(message);
    const signature = this.keyPair.sign(messageBytes);

    // Encode signature as base58
    const signatureB58 = bs58.encode(signature.signature);

    return {
      'X-Near-Account': this.config.auth.accountId,
      'X-Near-Signature': signatureB58,
      'X-Near-Timestamp': timestamp.toString(),
    };
  }
}

// ============================================================================
// Utility Functions
// ============================================================================

/**
 * Generate UUID v4 for idempotency keys
 */
export function generateIdempotencyKey(): string {
  return crypto.randomUUID();
}

/**
 * Compute SHA-256 hash of bytes
 */
export function computeChecksum(data: Uint8Array): string {
  return sha256(data);
}

/**
 * Create OutLayer client from environment variables
 *
 * Expects:
 * - OUTLAYER_BASE_URL (default: http://localhost:8080)
 * - OUTLAYER_AUTH_TYPE (bearer or near)
 * - OUTLAYER_AUTH_TOKEN (if bearer)
 * - OUTLAYER_ACCOUNT_ID (if near)
 * - OUTLAYER_PRIVATE_KEY (if near)
 */
export function createClientFromEnv(): OutLayerClient {
  const baseUrl = process.env.OUTLAYER_BASE_URL || 'http://localhost:8080';
  const authType = process.env.OUTLAYER_AUTH_TYPE || 'bearer';

  if (authType === 'bearer') {
    const token = process.env.OUTLAYER_AUTH_TOKEN;
    if (!token) {
      throw new Error('OUTLAYER_AUTH_TOKEN not set');
    }

    return new OutLayerClient({
      baseUrl,
      auth: { type: 'bearer', token },
    });
  } else if (authType === 'near') {
    const accountId = process.env.OUTLAYER_ACCOUNT_ID;
    const privateKey = process.env.OUTLAYER_PRIVATE_KEY;

    if (!accountId || !privateKey) {
      throw new Error('OUTLAYER_ACCOUNT_ID and OUTLAYER_PRIVATE_KEY must be set');
    }

    return new OutLayerClient({
      baseUrl,
      auth: { type: 'near', accountId, privateKey },
    });
  } else {
    throw new Error(`Unknown auth type: ${authType}`);
  }
}

// ============================================================================
// Export Everything
// ============================================================================

export default OutLayerClient;
