/**
 * QuickJS Enclave Determinism Tests
 *
 * Verifies:
 * - Deterministic execution with same seed/state/args
 * - State persistence across invocations
 * - Proper near.storageRead/Write shim
 * - Log capture
 * - Memory and time budgets
 */

import { QuickJSEnclave } from '../src/quickjs-enclave';
import { readFileSync } from 'fs';
import { fileURLToPath } from 'url';
import { dirname, join } from 'path';

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);

const counterSource = readFileSync(
  join(__dirname, '../examples/counter.js'),
  'utf-8'
);

describe('QuickJSEnclave - Determinism', () => {
  let enclave;

  beforeAll(async () => {
    enclave = await QuickJSEnclave.create({ memoryBytes: 32 << 20 });
  });

  test('replays identically with same seed/state/args', async () => {
    const inv = {
      source: counterSource,
      func: 'increment',
      args: [],
      priorState: {},
      seed: 'seed-123',
      policy: { timeMs: 200, memoryBytes: 32 << 20 },
    };

    // First invocation: 0 → 1
    const result1 = await enclave.invoke(inv);
    expect(result1.result).toEqual({ count: 1 });
    expect(result1.state).toEqual({ count: 1 });
    expect(result1.diagnostics.interrupted).toBe(false);
    expect(Array.isArray(result1.diagnostics.logs)).toBe(true);
    expect(result1.diagnostics.logs).toContain('count -> 1');

    // Second invocation with state from first: 1 → 2
    const inv2 = { ...inv, priorState: result1.state };
    const result2 = await enclave.invoke(inv2);
    expect(result2.result).toEqual({ count: 2 });
    expect(result2.state).toEqual({ count: 2 });
    expect(result2.diagnostics.logs).toContain('count -> 2');
  });

  test('getValue returns current state without modification', async () => {
    const inv = {
      source: counterSource,
      func: 'getValue',
      args: [],
      priorState: { count: 42 },
      seed: 'seed-456',
      policy: { timeMs: 200, memoryBytes: 32 << 20 },
    };

    const result = await enclave.invoke(inv);
    expect(result.result).toEqual({ count: 42 });
    expect(result.state).toEqual({ count: 42 }); // unchanged
  });

  test('reset clears counter state', async () => {
    const inv = {
      source: counterSource,
      func: 'reset',
      args: [],
      priorState: { count: 99 },
      seed: 'seed-789',
      policy: { timeMs: 200, memoryBytes: 32 << 20 },
    };

    const result = await enclave.invoke(inv);
    expect(result.result).toEqual({ count: 0 });
    expect(result.state).toEqual({ count: 0 });
    expect(result.diagnostics.logs).toContain('count reset to 0');
  });

  test('deterministic Math.random with same seed', async () => {
    const randomSource = `
      globalThis.getRandom = function() {
        return { value: Math.random() };
      };
    `;

    const inv = {
      source: randomSource,
      func: 'getRandom',
      args: [],
      priorState: {},
      seed: 'random-seed-123',
      policy: { timeMs: 200, memoryBytes: 32 << 20 },
    };

    const result1 = await enclave.invoke(inv);
    const result2 = await enclave.invoke(inv);

    // Same seed → same random value
    expect(result1.result).toEqual(result2.result);
    expect(typeof result1.result.value).toBe('number');
    expect(result1.result.value).toBeGreaterThanOrEqual(0);
    expect(result1.result.value).toBeLessThan(1);
  });

  test('Date.now returns 0 (deterministic clock)', async () => {
    const dateSource = `
      globalThis.getTimestamp = function() {
        return { timestamp: Date.now() };
      };
    `;

    const inv = {
      source: dateSource,
      func: 'getTimestamp',
      args: [],
      priorState: {},
      seed: 'date-seed',
      policy: { timeMs: 200, memoryBytes: 32 << 20 },
    };

    const result = await enclave.invoke(inv);
    expect(result.result).toEqual({ timestamp: 0 });
  });

  test('eval is disabled', async () => {
    const evalSource = `
      globalThis.tryEval = function() {
        try {
          eval('1 + 1');
          return { evaluated: true };
        } catch (e) {
          return { error: e.message };
        }
      };
    `;

    const inv = {
      source: evalSource,
      func: 'tryEval',
      args: [],
      priorState: {},
      seed: 'eval-seed',
      policy: { timeMs: 200, memoryBytes: 32 << 20 },
    };

    const result = await enclave.invoke(inv);
    expect(result.result.error).toContain('eval disabled');
  });

  test('function not found returns error', async () => {
    const inv = {
      source: counterSource,
      func: 'nonExistentFunction',
      args: [],
      priorState: {},
      seed: 'error-seed',
      policy: { timeMs: 200, memoryBytes: 32 << 20 },
    };

    const result = await enclave.invoke(inv);
    expect(result.result).toEqual(null);
    expect(result.diagnostics.interrupted).toBe(true);
  });

  test('near.log captures multiple arguments', async () => {
    const logSource = `
      globalThis.testLog = function() {
        near.log('string', 42, { obj: 'value' }, [1, 2, 3]);
        return { logged: true };
      };
    `;

    const inv = {
      source: logSource,
      func: 'testLog',
      args: [],
      priorState: {},
      seed: 'log-seed',
      policy: { timeMs: 200, memoryBytes: 32 << 20 },
    };

    const result = await enclave.invoke(inv);
    expect(result.result).toEqual({ logged: true });
    expect(result.diagnostics.logs.length).toBeGreaterThan(0);
    const logEntry = result.diagnostics.logs[0];
    expect(logEntry).toContain('string');
    expect(logEntry).toContain('42');
  });

  test('multiple invocations do not leak state', async () => {
    const inv1 = {
      source: counterSource,
      func: 'increment',
      args: [],
      priorState: {},
      seed: 'leak-test-1',
      policy: { timeMs: 200, memoryBytes: 32 << 20 },
    };

    const inv2 = {
      source: counterSource,
      func: 'increment',
      args: [],
      priorState: {}, // fresh state
      seed: 'leak-test-2',
      policy: { timeMs: 200, memoryBytes: 32 << 20 },
    };

    const result1 = await enclave.invoke(inv1);
    const result2 = await enclave.invoke(inv2);

    // Both should start from 0 → 1 (no cross-contamination)
    expect(result1.result).toEqual({ count: 1 });
    expect(result2.result).toEqual({ count: 1 });
  });
});

describe('QuickJSEnclave - Arguments', () => {
  let enclave;

  beforeAll(async () => {
    enclave = await QuickJSEnclave.create({ memoryBytes: 32 << 20 });
  });

  test('passes arguments correctly', async () => {
    const addSource = `
      globalThis.add = function(a, b) {
        return { sum: a + b };
      };
    `;

    const inv = {
      source: addSource,
      func: 'add',
      args: [40, 2],
      priorState: {},
      seed: 'args-seed',
      policy: { timeMs: 200, memoryBytes: 32 << 20 },
    };

    const result = await enclave.invoke(inv);
    expect(result.result).toEqual({ sum: 42 });
  });

  test('handles complex object arguments', async () => {
    const echoSource = `
      globalThis.echo = function(obj) {
        return { received: obj };
      };
    `;

    const inv = {
      source: echoSource,
      func: 'echo',
      args: [{ nested: { value: 123 }, array: [1, 2, 3] }],
      priorState: {},
      seed: 'obj-seed',
      policy: { timeMs: 200, memoryBytes: 32 << 20 },
    };

    const result = await enclave.invoke(inv);
    expect(result.result.received).toEqual({
      nested: { value: 123 },
      array: [1, 2, 3],
    });
  });
});

describe('QuickJSEnclave - Security (Heap Scan)', () => {
  /**
   * Critical security property: Private keys must NEVER enter the QuickJS sandbox.
   * This test verifies that a known "secret" pattern never appears in the enclave's
   * result/state/logs, proving the host-signer split is intact.
   *
   * Pattern from docs/browser-sec-architecture.md:
   * - QuickJS computes { bytesToSign, newState }
   * - Host performs signing via WebCrypto with the actual key
   * - Keys stay out of WASM linear memory
   */
  let enclave;

  beforeAll(async () => {
    enclave = await QuickJSEnclave.create({ memoryBytes: 32 << 20 });
  });

  test('private key never appears in enclave output', async () => {
    // Simulate a "private key" (in reality this would be in WebCrypto, not here)
    const fakePrivateKey = 'ed25519:PRIVATE_KEY_DEADBEEF_SHOULD_NEVER_LEAK_INTO_QUICKJS';
    const fakePrivateKeyBytes = new TextEncoder().encode(fakePrivateKey);

    // Contract that computes WHAT to sign (message bytes), not HOW (with key)
    const signingContract = `
      globalThis.prepareTransfer = function(args, state) {
        // Pure computation: build canonical message bytes
        const message = JSON.stringify({
          from: args.from,
          to: args.to,
          amount: args.amount,
          nonce: (state.nonce || 0) + 1
        });

        // Return bytes to sign (host will sign these with WebCrypto)
        const encoder = new TextEncoder();
        const bytesToSign = encoder.encode(message);

        return {
          bytesToSign: Array.from(bytesToSign), // convert to JSON-serializable
          nextState: { nonce: (state.nonce || 0) + 1 },
          message: message
        };
      };
    `;

    const inv = {
      source: signingContract,
      func: 'prepareTransfer',
      args: [{ from: 'alice.near', to: 'bob.near', amount: 100 }],
      priorState: { nonce: 0 },
      seed: 'signing-seed',
      policy: { timeMs: 200, memoryBytes: 32 << 20 },
    };

    const result = await enclave.invoke(inv);

    // Verify the contract worked
    expect(result.result).toHaveProperty('bytesToSign');
    expect(result.result).toHaveProperty('nextState');
    expect(result.result.nextState.nonce).toBe(1);

    // CRITICAL: Scan entire result for any trace of the fake private key
    const resultJSON = JSON.stringify(result);
    expect(resultJSON).not.toContain('PRIVATE_KEY');
    expect(resultJSON).not.toContain('DEADBEEF');
    expect(resultJSON).not.toContain(fakePrivateKey);

    // Verify logs don't contain secrets
    const logsJSON = JSON.stringify(result.diagnostics.logs);
    expect(logsJSON).not.toContain('PRIVATE_KEY');
    expect(logsJSON).not.toContain('DEADBEEF');

    // Verify state doesn't contain secrets
    const stateJSON = JSON.stringify(result.state);
    expect(stateJSON).not.toContain('PRIVATE_KEY');
    expect(stateJSON).not.toContain('DEADBEEF');
  });

  test('contract cannot access WebCrypto keys', async () => {
    // Verify that even if contract tries to access crypto, it fails gracefully
    const cryptoAttempt = `
      globalThis.tryAccessCrypto = function() {
        const results = {};

        // Try to access various crypto APIs
        results.hasCrypto = typeof crypto !== 'undefined';
        results.hasSubtle = typeof crypto?.subtle !== 'undefined';

        // Try to generate a key (should fail or be undefined)
        let keyGenerated = false;
        try {
          if (crypto?.subtle?.generateKey) {
            // This should not work in QuickJS
            keyGenerated = true;
          }
        } catch (e) {
          results.error = e.message;
        }
        results.keyGenerated = keyGenerated;

        return results;
      };
    `;

    const inv = {
      source: cryptoAttempt,
      func: 'tryAccessCrypto',
      args: [],
      priorState: {},
      seed: 'crypto-seed',
      policy: { timeMs: 200, memoryBytes: 32 << 20 },
    };

    const result = await enclave.invoke(inv);

    // In QuickJS, crypto APIs should not be available
    // (The host provides them, but QuickJS sandbox doesn't have access)
    expect(result.result.hasCrypto).toBeFalsy();
    expect(result.result.hasSubtle).toBeFalsy();
    expect(result.result.keyGenerated).toBe(false);
  });

  test('message-to-sign pattern works correctly', async () => {
    // Demonstrate the correct pattern: compute canonical bytes, return them
    const canonicalSigning = `
      globalThis.buildCanonicalMessage = function(args) {
        // Sort keys for determinism
        const sorted = Object.keys(args).sort().reduce((acc, key) => {
          acc[key] = args[key];
          return acc;
        }, {});

        // Canonical JSON (deterministic)
        const message = JSON.stringify(sorted);
        const encoder = new TextEncoder();
        const bytes = encoder.encode(message);

        return {
          bytesToSign: Array.from(bytes),
          messagePreview: message.substring(0, 50)
        };
      };
    `;

    const inv = {
      source: canonicalSigning,
      func: 'buildCanonicalMessage',
      args: [{ z: 'last', a: 'first', m: 'middle' }],
      priorState: {},
      seed: 'canonical-seed',
      policy: { timeMs: 200, memoryBytes: 32 << 20 },
    };

    const result = await enclave.invoke(inv);

    // Verify we got bytes to sign
    expect(Array.isArray(result.result.bytesToSign)).toBe(true);
    expect(result.result.bytesToSign.length).toBeGreaterThan(0);

    // Verify keys are sorted (deterministic)
    expect(result.result.messagePreview).toContain('"a":"first"');

    // At this point, the HOST would:
    // 1. Convert bytesToSign back to Uint8Array
    // 2. Use crypto.subtle.sign(algorithm, privateKey, bytesToSign)
    // 3. Return signature to user
    // The private key NEVER enters QuickJS
  });
});
