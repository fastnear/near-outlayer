/**
 * Frozen Realm Unit Tests
 *
 * Tests the L4 sandbox isolation and primordial freezing
 */

import { describe, test, expect, beforeAll } from '@jest/globals';

let FrozenRealm;

beforeAll(async () => {
  const module = await import('../src/frozen-realm.js');
  FrozenRealm = module.FrozenRealm;
});

describe('FrozenRealm: L4 Sandbox', () => {

  test('should create instance with default options', () => {
    const realm = new FrozenRealm();
    expect(realm).toBeInstanceOf(FrozenRealm);
    expect(realm.primordialsFrozen).toBe(false);
  });

  test('should freeze primordials on first execution', async () => {
    const realm = new FrozenRealm({ verbose: false });

    const code = 'return 42;';
    const result = await realm.execute(code);

    expect(result).toBe(42);
    expect(realm.primordialsFrozen).toBe(true);
  });

  test('should isolate code from outer scope (no closure access)', async () => {
    const realm = new FrozenRealm({ verbose: false });

    // Variable in test scope
    const outerVariable = 'I am outside the realm';

    // Code tries to access outer variable (should fail)
    const code = `
      try {
        return outerVariable; // ReferenceError: outerVariable is not defined
      } catch (e) {
        return 'ISOLATED:' + e.message;
      }
    `;

    const result = await realm.execute(code);

    expect(result).toContain('ISOLATED');
    expect(result).toContain('not defined');
  });

  test('should only access explicitly injected capabilities', async () => {
    const realm = new FrozenRealm({ verbose: false });

    const capabilities = {
      greeting: 'Hello',
      add: (a, b) => a + b
    };

    const code = `
      return greeting + ' World! 2+2=' + add(2, 2);
    `;

    const result = await realm.execute(code, capabilities);

    expect(result).toBe('Hello World! 2+2=4');
  });

  test('should reject dangerous capabilities', async () => {
    const realm = new FrozenRealm({ verbose: false });

    const dangerousCapabilities = {
      fetch: global.fetch, // Should be rejected
      eval: eval // Should be rejected
    };

    await expect(async () => {
      await realm.execute('return 42;', dangerousCapabilities);
    }).rejects.toThrow(/Dangerous capability/);
  });

  test('should timeout long-running code', async () => {
    const realm = new FrozenRealm({ verbose: false, executionTimeout: 100 });

    const infiniteLoopCode = `
      while(true) {} // Infinite loop
    `;

    await expect(async () => {
      await realm.execute(infiniteLoopCode);
    }).rejects.toThrow(/timeout/i);
  }, 10000); // Test timeout: 10s

  test('should handle async code', async () => {
    const realm = new FrozenRealm({ verbose: false });

    const asyncCapability = {
      delay: (ms) => new Promise(resolve => setTimeout(resolve, ms)),
      getValue: async () => 'async-value'
    };

    const code = `
      return (async function() {
        await delay(10);
        const value = await getValue();
        return 'Got: ' + value;
      })();
    `;

    const result = await realm.execute(code, asyncCapability);

    expect(result).toBe('Got: async-value');
  });

  test('should track execution statistics', async () => {
    const realm = new FrozenRealm({ verbose: false });

    await realm.execute('return 1;');
    await realm.execute('return 2;');
    await realm.execute('return 3;');

    const stats = realm.getStats();

    expect(stats.totalExecutions).toBe(3);
    expect(stats.totalFreezes).toBe(1); // Only frozen once
    expect(stats.primordialsFrozen).toBe(true);
    expect(stats.avgExecutionTime).toBeGreaterThan(0);
  });

  test('should prevent prototype pollution', async () => {
    const realm = new FrozenRealm({ verbose: false });

    const code = `
      try {
        Array.prototype.malicious = function() { return 'hacked'; };
        return 'FAILED: Should have thrown error';
      } catch (e) {
        return 'PROTECTED: ' + e.message;
      }
    `;

    const result = await realm.execute(code);

    expect(result).toContain('PROTECTED');
    expect(result).toMatch(/not extensible|read.only|Cannot/i);
  });

  test('should create safe logger', () => {
    const realm = new FrozenRealm({ verbose: true });

    const logger = realm.createSafeLogger('TestRealm');

    expect(typeof logger).toBe('function');

    // Should not throw
    logger('Test message');
  });

  test('should handle errors in guest code', async () => {
    const realm = new FrozenRealm({ verbose: false });

    const errorCode = `
      throw new Error('Intentional error from L4');
    `;

    await expect(async () => {
      await realm.execute(errorCode);
    }).rejects.toThrow('Intentional error from L4');
  });

  test('should handle syntax errors', async () => {
    const realm = new FrozenRealm({ verbose: false });

    const syntaxErrorCode = `
      return {{{{{ invalid syntax
    `;

    await expect(async () => {
      await realm.execute(syntaxErrorCode);
    }).rejects.toThrow(/Syntax error/i);
  });

  test('should reset statistics', async () => {
    const realm = new FrozenRealm({ verbose: false });

    await realm.execute('return 1;');
    await realm.execute('return 2;');

    let stats = realm.getStats();
    expect(stats.totalExecutions).toBe(2);

    realm.resetStats();

    stats = realm.getStats();
    expect(stats.totalExecutions).toBe(0);
    expect(stats.avgExecutionTime).toBe(0);
    // primordialsFrozen stays true (can't unfreeze)
    expect(stats.primordialsFrozen).toBe(true);
  });
});
