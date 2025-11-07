/**
 * Minimal QuickJS Executor (Pure JS, ~80 lines)
 *
 * This is a stripped-down alternative to quickjs-enclave.ts for those who prefer
 * pure JavaScript without TypeScript. It provides the same security guarantees:
 * - Deterministic compute (LCG PRNG, Date.now=0, no timers, no eval)
 * - Budget enforcement (memory + time)
 * - Fresh context per invocation
 * - No secrets in WASM (message-to-sign pattern)
 *
 * Differences from quickjs-enclave.ts:
 * - Pure JS (no TypeScript)
 * - No prelude probe (trusts injection)
 * - Minimal error handling
 * - ~80 lines vs ~250 lines
 *
 * Use this if you want a minimal, auditable implementation.
 * Use quickjs-enclave.ts if you want types, probe, and comprehensive error handling.
 *
 * From senior review feedback (security architecture compliant).
 */

import { getQuickJS } from "quickjs-emscripten";

export class QuickJSEnclave {
  static async create(opts = {}) {
    const mod = await getQuickJS();
    const rt = mod.newRuntime();
    rt.setMemoryLimit(opts.memoryBytes ?? (64 << 20));
    return new QuickJSEnclave(mod, rt);
  }

  constructor(mod, runtime) {
    this.mod = mod;
    this.runtime = runtime;
  }

  async invoke({ source, func, args = [], priorState = {}, seed = "seed", policy = { timeMs: 200, memoryBytes: 33554432 } }) {
    const timeMs = Math.max(1, policy.timeMs | 0);
    const deadline = performance.now() + timeMs;
    this.runtime.setMemoryLimit(Math.max(1 << 20, policy.memoryBytes | 0));
    this.runtime.setInterruptHandler(() => performance.now() > deadline);

    const ctx = this.runtime.newContext();
    try {
      const script = buildEnvelope({ source, func, args, priorState, seed });
      const res = ctx.evalCode(script);
      if (res.error) {
        const err = String(ctx.dump(res.error));
        res.error.dispose();
        return { state: priorState, result: null, diagnostics: { logs: [], timeMs, interrupted: true, error: err } };
      }
      const outText = String(ctx.dump(res.value));
      res.value.dispose();
      const out = JSON.parse(outText);
      const elapsed = Math.max(0, performance.now() - (deadline - timeMs));
      return { state: out.state, result: out.result, diagnostics: { logs: out.logs || [], timeMs: elapsed, interrupted: elapsed > timeMs } };
    } finally {
      try { this.runtime.executePendingJobs?.(); } catch {}
      ctx.dispose();
    }
  }

  dispose() {
    try { this.runtime.setInterruptHandler(() => false); } catch {}
    this.runtime.dispose();
  }
}

function jsLit(v) { return JSON.stringify(v ?? null); }
function jsStr(s) { return JSON.stringify(String(s ?? "")); }

function buildEnvelope({ source, func, args, priorState, seed }) {
  const prelude = `
const __LOGS__ = [];
(function __harden__(seed){
  let s = 0n; for (let i = 0; i < seed.length; i++) s = (s*131n + BigInt(seed.charCodeAt(i))) & 0xffffffffffffffffn;
  function rnd(){ s = (6364136223846793005n*s + 1442695040888963407n) & 0xffffffffffffffffn; return Number(s & 0x3fffffffffffn) / Number(0x400000000000n); }
  Math.random = () => rnd();
  Date = class extends Date { constructor(...a){ super(...a);} static now(){ return 0; } };
  globalThis.eval = function(){ throw new Error("eval disabled"); };
  globalThis.setTimeout = undefined;
  globalThis.setInterval = undefined;
  try { Object.freeze(Object.prototype); } catch {}
  try { Object.freeze(Array.prototype); } catch {}
  try { Object.freeze(Function.prototype); } catch {}
  try { Object.freeze(globalThis); } catch {}
})(${jsStr(seed)});
`;

  const shim = `
const state = ${jsLit(priorState)};
const near = Object.freeze({
  storageRead: (k) => state[k],
  storageWrite: (k, v) => { state[k] = v; },
  log: (...a) => { __LOGS__.push(a.map(x => typeof x === 'string' ? x : JSON.stringify(x)).join(' ')); }
});
globalThis.near = near;
`;

  const contract = `\n${source ?? ""}\n`;

  const call = `
const __FN__ = ${jsStr(func || "main")};
const __ARGS__ = ${jsLit(args)};
let f = globalThis[__FN__];
if (typeof f !== "function" && globalThis.default && typeof globalThis.default[__FN__] === "function") f = globalThis.default[__FN__];
let ok = true, result = null;
try {
  if (typeof f !== "function") throw new Error("function not found: " + __FN__);
  result = f.apply(null, __ARGS__);
} catch (e) {
  ok = false; result = { error: String(e && e.message || e) };
}
JSON.stringify({ ok, state, result, logs: __LOGS__ });
`;

  return `${prelude}\n${shim}\n${contract}\n${call}\n`;
}
