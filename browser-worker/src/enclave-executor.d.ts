/**
 * TypeScript definitions for enclave-executor.js
 * Plain vanilla types - here to help, not hinder!
 */

import { FrozenRealm, FrozenRealmStats } from './frozen-realm';
import { CryptoUtils, CryptoStats } from './crypto-utils';

export interface EnclaveExecutorOptions {
  /** Enable verbose logging */
  verbose?: boolean;
  /** Enable L2 (linux-wasm) layer (future) */
  useLinux?: boolean;
  /** Enable L3 (QuickJS) layer (future) */
  useQuickJS?: boolean;
  /** Execution timeout in milliseconds */
  executionTimeout?: number;
  /** Crypto options to pass through */
  cryptoOptions?: Record<string, any>;
}

export interface EnclaveExecutionRequest {
  /** Base64 encrypted payload */
  encryptedPayload: string;
  /** Base64 encrypted secret */
  encryptedSecret: string;
  /** Hex-encoded L4 decryption key */
  enclaveKey: string;
  /** L4 guest code to execute */
  code: string;
  /** Optional identifier for debugging */
  codeId?: string;
}

export interface EnclaveExecutionResult {
  /** Encrypted result (base64) */
  encryptedResult: string;
  /** Total execution time in milliseconds */
  executionTime: number;
  /** L4 execution time in milliseconds */
  l4Time: number;
  /** Layers traversed */
  layers: string[];
}

export interface PlaintextExecutionRequest {
  /** Plaintext payload */
  payload: string;
  /** Plaintext secret */
  secret: string;
  /** L4 guest code to execute */
  code: string;
  /** Optional identifier */
  codeId?: string;
}

export interface PlaintextExecutionResult {
  /** Plaintext result */
  result: any;
  /** Execution time in milliseconds */
  executionTime: number;
  /** Layers used */
  layers: string[];
}

export interface L4Capabilities {
  /** Safe logging function */
  log: (message: string) => void;
  /** Encrypted payload (opaque to L1-L3) */
  encryptedPayload: string;
  /** Encrypted secret (opaque to L1-L3) */
  encryptedSecret: string;
  /** Enclave decryption key */
  enclaveKey: string;
  /** Crypto capability */
  crypto: {
    decrypt: (encrypted: string, key: string) => Promise<string>;
    encrypt: (data: string, key: string) => Promise<string>;
    hash: (data: string) => Promise<string>;
  };
  /** Deterministic utilities */
  utils: {
    parseJSON: typeof JSON.parse;
    stringifyJSON: typeof JSON.stringify;
  };
}

export interface EnclaveExecutorStats {
  totalExecutions: number;
  encryptedExecutions: number;
  avgExecutionTime: number;
  avgDecryptionTime: number;
  frozenRealm?: FrozenRealmStats;
  crypto?: CryptoStats;
}

export class EnclaveExecutor {
  constructor(options?: EnclaveExecutorOptions);

  /** Execute with E2EE ferry pattern (L1→L4) */
  executeEncrypted(request: EnclaveExecutionRequest): Promise<EnclaveExecutionResult>;

  /** Execute with plaintext (for comparison/benchmarking) */
  executePlaintext(request: PlaintextExecutionRequest): Promise<PlaintextExecutionResult>;

  /** Create L4 capabilities (internal, exposed for testing) */
  createL4Capabilities(request: EnclaveExecutionRequest): L4Capabilities;

  /** Get execution statistics */
  getStats(): EnclaveExecutorStats;

  /** Reset statistics */
  resetStats(): void;
}
