/**
 * TypeScript definitions for frozen-realm.js
 * Plain vanilla types - here to help, not hinder!
 */

export interface FrozenRealmOptions {
  /** Enable verbose logging */
  verbose?: boolean;
  /** Allow specific dangerous globals (debugging only) */
  allowedGlobals?: string[];
  /** Timeout for realm execution in milliseconds */
  executionTimeout?: number;
}

export interface FrozenRealmStats {
  totalExecutions: number;
  totalFreezes: number;
  avgExecutionTime: number;
  primordialsFrozen: boolean;
}

export class FrozenRealm {
  constructor(options?: FrozenRealmOptions);

  /** Freeze all JavaScript primordials (irreversible!) */
  freezePrimordials(): void;

  /** Execute code in isolated sandbox */
  execute<T = any>(
    untrustedCode: string,
    capabilities?: Record<string, any>,
    codeId?: string
  ): Promise<T>;

  /** Create safe logger for guest code */
  createSafeLogger(prefix?: string): (message: string) => void;

  /** Get execution statistics */
  getStats(): FrozenRealmStats;

  /** Reset statistics */
  resetStats(): void;
}

/** Helper function - create and execute in one step */
export function executeInFrozenRealm<T = any>(
  code: string,
  capabilities?: Record<string, any>,
  options?: FrozenRealmOptions
): Promise<T>;
