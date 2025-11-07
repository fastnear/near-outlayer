/**
 * QuickJS Enclave - Determinism Replay Test (50x)
 *
 * Critical property: Same inputs → identical outputs across many invocations.
 * This is the foundation of verifiable compute: any observer running the same
 * code with the same inputs must get byte-identical results.
 *
 * Pattern from docs/browser-sec-architecture.md section 6.
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

async function createEnclave() {
  return QuickJSEnclave.create({ memoryBytes: 32 << 20 });
}

describe('QuickJSEnclave - Determinism Replay (50x)', () => {
  let enclave;

  beforeAll(async () => {
    enclave = await createEnclave();
  });

  afterAll(() => {
    if (enclave) enclave.dispose();
  });

  test('50× replay: same seed/state/args → identical outputs', async () => {
    const inv = {
      source: counterSource,
      func: 'increment',
      args: [],
      priorState: {},
      seed: 'determinism-seed-50x',
      policy: { timeMs: 200, memoryBytes: 32 << 20 },
    };

    const outputs = [];
    for (let i = 0; i < 50; i++) {
      outputs.push(await enclave.invoke(inv));
    }

    // Every output must be identical to the first
    const canonical = outputs[0];
    expect(canonical.result).toEqual({ count: 1 });
    expect(canonical.state).toEqual({ count: 1 });

    for (let i = 1; i < outputs.length; i++) {
      expect(outputs[i].result).toEqual(canonical.result);
      expect(outputs[i].state).toEqual(canonical.state);
      expect(outputs[i].diagnostics.logs).toEqual(canonical.diagnostics.logs);
    }
  });

  test('state carry: 0 → 1 → 2 (deterministic sequencing)', async () => {
    const baseInv = {
      source: counterSource,
      func: 'increment',
      args: [],
      seed: 'state-carry-seed',
      policy: { timeMs: 200, memoryBytes: 32 << 20 },
    };

    // First call: 0 → 1
    const result1 = await enclave.invoke({
      ...baseInv,
      priorState: {},
    });
    expect(result1.result).toEqual({ count: 1 });
    expect(result1.state).toEqual({ count: 1 });

    // Second call: 1 → 2 (using state from first)
    const result2 = await enclave.invoke({
      ...baseInv,
      priorState: result1.state,
    });
    expect(result2.result).toEqual({ count: 2 });
    expect(result2.state).toEqual({ count: 2 });

    // Third call: 2 → 3 (using state from second)
    const result3 = await enclave.invoke({
      ...baseInv,
      priorState: result2.state,
    });
    expect(result3.result).toEqual({ count: 3 });
    expect(result3.state).toEqual({ count: 3 });
  });

  test('different seeds → different Math.random sequences (but still deterministic)', async () => {
    const randomSource = `
      globalThis.getRandom = function() {
        const r1 = Math.random();
        const r2 = Math.random();
        const r3 = Math.random();
        return { r1, r2, r3 };
      };
    `;

    const inv1 = {
      source: randomSource,
      func: 'getRandom',
      args: [],
      priorState: {},
      seed: 'seed-A',
      policy: { timeMs: 200, memoryBytes: 32 << 20 },
    };

    const inv2 = {
      ...inv1,
      seed: 'seed-B',
    };

    // Run each seed 10 times
    const outputsA = [];
    const outputsB = [];

    for (let i = 0; i < 10; i++) {
      outputsA.push(await enclave.invoke(inv1));
      outputsB.push(await enclave.invoke(inv2));
    }

    // Within same seed: all outputs identical
    for (let i = 1; i < 10; i++) {
      expect(outputsA[i].result).toEqual(outputsA[0].result);
      expect(outputsB[i].result).toEqual(outputsB[0].result);
    }

    // Across different seeds: outputs differ
    expect(outputsA[0].result).not.toEqual(outputsB[0].result);
  });

  test('Math.random produces valid [0,1) range', async () => {
    const source = `
      globalThis.testRange = function() {
        const samples = [];
        for (let i = 0; i < 1000; i++) {
          samples.push(Math.random());
        }
        return { samples };
      };
    `;

    const result = await enclave.invoke({
      source,
      func: 'testRange',
      args: [],
      priorState: {},
      seed: 'range-test',
      policy: { timeMs: 500, memoryBytes: 32 << 20 },
    });

    const { samples } = result.result;
    expect(samples.length).toBe(1000);

    // Every sample must be: 0 <= x < 1
    for (const x of samples) {
      expect(typeof x).toBe('number');
      expect(x).toBeGreaterThanOrEqual(0);
      expect(x).toBeLessThan(1);
    }

    // Verify determinism: same seed → same sequence
    const result2 = await enclave.invoke({
      source,
      func: 'testRange',
      args: [],
      priorState: {},
      seed: 'range-test',
      policy: { timeMs: 500, memoryBytes: 32 << 20 },
    });

    expect(result2.result.samples).toEqual(samples);
  });

  test('Date.now always returns 0', async () => {
    const source = `
      globalThis.getTimestamp = function() {
        return {
          now1: Date.now(),
          now2: Date.now(),
          now3: Date.now(),
        };
      };
    `;

    const result = await enclave.invoke({
      source,
      func: 'getTimestamp',
      args: [],
      priorState: {},
      seed: 'date-test',
      policy: { timeMs: 200, memoryBytes: 32 << 20 },
    });

    expect(result.result).toEqual({
      now1: 0,
      now2: 0,
      now3: 0,
    });
  });

  test('100× replay with complex state mutations', async () => {
    const complexSource = `
      globalThis.complexMutation = function(args, state) {
        const { operation, value } = args;

        // Initialize
        if (!state.counter) state.counter = 0;
        if (!state.history) state.history = [];

        // Mutate based on operation
        switch (operation) {
          case 'increment':
            state.counter += value;
            break;
          case 'decrement':
            state.counter -= value;
            break;
          case 'multiply':
            state.counter *= value;
            break;
        }

        // Record in history
        state.history.push({
          op: operation,
          val: value,
          result: state.counter,
          rand: Math.random()
        });

        return {
          current: state.counter,
          historyLength: state.history.length
        };
      };
    `;

    const inv = {
      source: complexSource,
      func: 'complexMutation',
      args: [{ operation: 'increment', value: 5 }],
      priorState: {},
      seed: 'complex-100x',
      policy: { timeMs: 200, memoryBytes: 32 << 20 },
    };

    const outputs = [];
    for (let i = 0; i < 100; i++) {
      outputs.push(await enclave.invoke(inv));
    }

    // All outputs must be identical
    const canonical = JSON.stringify(outputs[0]);
    for (let i = 1; i < outputs.length; i++) {
      expect(JSON.stringify(outputs[i])).toBe(canonical);
    }

    // Verify the result makes sense
    expect(outputs[0].result.current).toBe(5);
    expect(outputs[0].result.historyLength).toBe(1);
    expect(outputs[0].state.counter).toBe(5);
    expect(outputs[0].state.history.length).toBe(1);
  });
});
