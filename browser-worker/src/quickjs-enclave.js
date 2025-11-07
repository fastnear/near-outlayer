// browser/quickjs-enclave.ts
// Deterministic QuickJS sandbox for the browser using quickjs-emscripten.
//
// SECURITY MODEL: This provides an *execution sandbox* with deterministic compute,
// NOT a secure vault. The host page can inspect WASM memory. For sensitive operations,
// keep secrets in WebCrypto and let this sandbox compute *what to sign*, not perform
// signing with raw keys. See docs/browser-sec-architecture.md for the full model.
//
// - Single runtime per instance, new context per invocation (prevents cross-call leakage).
// - Deterministic prelude: overrides Math.random/Date.now, disables eval/timers.
// - Simple NEAR-like state shim: near.storageRead/Write + near.log (captured).
// - Hard budgets: time (epoch interrupt) + memory limit.
// - No FS, no std/os modules, no network. Pure in-memory bridge via JSON.
//
// Usage:
//   import { QuickJSEnclave } from "./quickjs-enclave";
//   const q = await QuickJSEnclave.create();
//   const out = await q.invoke({
//     source: "globalThis.increment = () => { let c = near.storageRead('count')||0; c++; near.storageWrite('count', c); return {count:c}; }",
//     func: "increment",
//     args: [],
//     priorState: {},             // any JSON
//     seed: "demo-seed",          // deterministic seed
//     policy: { timeMs: 200, memoryBytes: 32<<20 } // 32 MiB
//   });
//   console.log(out.state, out.result, out.diagnostics);
import { getQuickJS } from "quickjs-emscripten";
export class QuickJSEnclave {
    static async create(opts) {
        const mod = await getQuickJS();
        const rt = mod.newRuntime();
        // set a generous default; callers can still pass stricter policy per call.
        rt.setMemoryLimit(opts?.memoryBytes ?? (64 << 20)); // 64 MiB default
        return new QuickJSEnclave(mod, rt);
    }
    constructor(mod, rt) {
        this.mod = mod;
        this.runtime = rt;
    }
    dispose() {
        try {
            this.runtime.setInterruptHandler(() => false);
        }
        catch { }
        try {
            this.runtime.executePendingJobs?.();
        }
        catch { }
        this.runtime.dispose();
    }
    async invoke(inv) {
        const t0 = performance.now();
        const deadline = t0 + Math.max(1, inv.policy.timeMs | 0);
        // tighten runtime budgets for this call
        this.runtime.setMemoryLimit(Math.max(1 << 20, inv.policy.memoryBytes | 0)); // >= 1 MiB
        this.runtime.setInterruptHandler(() => performance.now() > deadline);
        const ctx = this.runtime.newContext();
        try {
            // Install deterministic prelude first
            const prelude = buildDeterministicPrelude(inv.seed ?? "");
            const preludeRes = ctx.evalCode(prelude);
            if (preludeRes.error) {
                const errStr = ctx.dump(preludeRes.error);
                preludeRes.error.dispose();
                throw new Error(`Prelude installation failed: ${errStr}`);
            }
            preludeRes.value.dispose();
            // Probe: verify deterministic prelude is installed correctly
            {
                const probe = `
          (function(){
            const rnd = Math.random();
            const now = Date.now();
            const hasTimers = typeof setTimeout !== 'undefined' || typeof setInterval !== 'undefined';
            JSON.stringify({ rnd, now, hasTimers });
          })()
        `;
                const probeRes = ctx.evalCode(probe);
                if (probeRes.error) {
                    probeRes.error.dispose();
                    throw new Error("Prelude probe failed");
                }
                const probeJson = JSON.parse(String(ctx.dump(probeRes.value)));
                probeRes.value.dispose();
                // rnd must be number in [0,1), now must be 0, hasTimers must be false
                if (typeof probeJson.rnd !== "number" || probeJson.rnd < 0 || probeJson.rnd >= 1) {
                    throw new Error("Deterministic Math.random not installed");
                }
                if (probeJson.now !== 0)
                    throw new Error("Deterministic Date.now not installed");
                if (probeJson.hasTimers)
                    throw new Error("Timers not disabled");
            }
            // Now execute the near shim + user code
            const script = buildUserCodeEnvelope(inv);
            const res = ctx.evalCode(script);
            // Check if execution resulted in an error
            if (res.error) {
                // Attempt to stringify any thrown value
                const errStr = ctx.dump(res.error);
                res.error.dispose();
                const timeMs = Math.max(0, performance.now() - t0);
                return {
                    state: inv.priorState,
                    result: null,
                    diagnostics: { logs: [], timeMs, interrupted: true },
                };
            }
            // Expect a JSON string; parse it in the host.
            const value = ctx.dump(res.value);
            res.value.dispose();
            const jsonText = typeof value === "string" ? value : JSON.stringify(value);
            const parsed = JSON.parse(jsonText);
            const timeMs = Math.max(0, performance.now() - t0);
            return {
                state: parsed.state,
                result: parsed.result,
                diagnostics: { logs: parsed.logs || [], timeMs, interrupted: timeMs > inv.policy.timeMs },
            };
        }
        finally {
            // Opportunistic cleanup
            try {
                this.runtime.executePendingJobs?.();
            }
            catch { }
            ctx.dispose();
        }
    }
}
// -------- helpers --------
function jsStringLiteral(s) {
    // Safely embed a string into JS source as a literal
    return JSON.stringify(s);
}
function jsJSONLiteral(v) {
    // Serialize JSON into a JS literal
    return JSON.stringify(v ?? null);
}
function buildDeterministicPrelude(seed) {
    // Deterministic prelude: LCG PRNG + frozen intrinsics + disabled eval/timers.
    return `
const __LOGS__ = [];
(function __harden__(seed) {
  // tiny deterministic LCG
  let s = 0n;
  for (let i = 0; i < seed.length; i++) s = (s * 131n + BigInt(seed.charCodeAt(i))) & 0xffffffffffffffffn;
  function rand() {
    s = (6364136223846793005n * s + 1442695040888963407n) & 0xffffffffffffffffn;
    // map to [0,1)
    const n = Number(s & 0x3fffffffffffn);
    return n / Number(0x400000000000n);
  }
  // freeze some dangerous surfaces (minimal, keep perf)
  const g = globalThis;
  g.Math.random = () => rand();
  g.Date = class extends Date { constructor(...a){ super(...a);} static now(){ return 0; } };
  g.eval = function(){ throw new Error("eval disabled"); };
  g.setTimeout = undefined;
  g.setInterval = undefined;
  try { Object.freeze(Object.prototype); } catch {}
  try { Object.freeze(Array.prototype); } catch {}
  try { Object.freeze(Function.prototype); } catch {}
  try { Object.freeze(g); } catch {}
})(${jsStringLiteral(seed)});
`;
}
function buildUserCodeEnvelope(inv) {
    // near shim + contract source + function resolution and call
    const stateJson = jsJSONLiteral(inv.priorState ?? {});
    const argsJson = jsJSONLiteral(inv.args ?? []);
    const funcName = inv.func || "main";
    const src = inv.source || "";
    const shim = `
const state = ${stateJson};
const near = Object.freeze({
  storageRead: (k) => state[k],
  storageWrite: (k, v) => { state[k] = v; },
  log: (...a) => { __LOGS__.push(a.map(x => typeof x === 'string' ? x : JSON.stringify(x)).join(' ')); }
});
globalThis.near = near;
`;
    const contract = `\n/* contract source */\n${src}\n`;
    const resolveAndCall = `
const __FN__ = ${jsStringLiteral(funcName)};
const __ARGS__ = ${argsJson};
let __callable__ = globalThis[__FN__];
if (typeof __callable__ !== 'function' && globalThis.default && typeof globalThis.default[__FN__] === 'function') {
  __callable__ = globalThis.default[__FN__];
}
let __RESULT__ = null;
let __OK__ = true;
try {
  if (typeof __callable__ !== 'function') throw new Error("function not found: " + __FN__);
  __RESULT__ = __callable__.apply(null, __ARGS__);
} catch (e) {
  __OK__ = false;
  __RESULT__ = { error: String(e && e.message || e) };
}
JSON.stringify({ ok: __OK__, state, result: __RESULT__, logs: __LOGS__ });
`;
    return `${shim}\n${contract}\n${resolveAndCall}\n`;
}
function buildDeterministicEnvelope(inv) {
    // Everything runs in module scope (no std/os). We:
    //  1) Harden globals + set deterministic RNG/clock
    //  2) Create near shim using the captured `state` object
    //  3) Evaluate the contract source
    //  4) Resolve the function and call with args
    //  5) Return a single JSON string with { ok, state, result, logs }
    const seed = inv.seed ?? "";
    const stateJson = jsJSONLiteral(inv.priorState ?? {});
    const argsJson = jsJSONLiteral(inv.args ?? []);
    const funcName = inv.func || "main";
    const src = inv.source || "";
    // Deterministic prelude: LCG PRNG + frozen intrinsics + disabled eval/timers.
    const prelude = `
const __LOGS__ = [];
(function __harden__(seed) {
  // tiny deterministic LCG
  let s = 0n;
  for (let i = 0; i < seed.length; i++) s = (s * 131n + BigInt(seed.charCodeAt(i))) & 0xffffffffffffffffn;
  function rand() {
    s = (6364136223846793005n * s + 1442695040888963407n) & 0xffffffffffffffffn;
    // map to [0,1)
    const n = Number(s & 0x3fffffffffffn);
    return n / Number(0x400000000000n);
  }
  // freeze some dangerous surfaces (minimal, keep perf)
  const g = globalThis;
  g.Math.random = () => rand();
  g.Date = class extends Date { constructor(...a){ super(...a);} static now(){ return 0; } };
  g.eval = function(){ throw new Error("eval disabled"); };
  g.setTimeout = undefined;
  g.setInterval = undefined;
  try { Object.freeze(Object.prototype); } catch {}
  try { Object.freeze(Array.prototype); } catch {}
  try { Object.freeze(Function.prototype); } catch {}
  try { Object.freeze(g); } catch {}
})(${jsStringLiteral(seed)});
`;
    const shim = `
const state = ${stateJson};
const near = Object.freeze({
  storageRead: (k) => state[k],
  storageWrite: (k, v) => { state[k] = v; },
  log: (...a) => { __LOGS__.push(a.map(x => typeof x === 'string' ? x : JSON.stringify(x)).join(' ')); }
});
globalThis.near = near;
`;
    const contract = `\n/* contract source */\n${src}\n`;
    const resolveAndCall = `
const __FN__ = ${jsStringLiteral(funcName)};
const __ARGS__ = ${argsJson};
let __callable__ = globalThis[__FN__];
if (typeof __callable__ !== 'function' && globalThis.default && typeof globalThis.default[__FN__] === 'function') {
  __callable__ = globalThis.default[__FN__];
}
let __RESULT__ = null;
let __OK__ = true;
try {
  if (typeof __callable__ !== 'function') throw new Error("function not found: " + __FN__);
  __RESULT__ = __callable__.apply(null, __ARGS__);
} catch (e) {
  __OK__ = false;
  __RESULT__ = { error: String(e && e.message || e) };
}
JSON.stringify({ ok: __OK__, state, result: __RESULT__, logs: __LOGS__ });
`;
    return `${prelude}\n${shim}\n${contract}\n${resolveAndCall}\n`;
}
//# sourceMappingURL=quickjs-enclave.js.map