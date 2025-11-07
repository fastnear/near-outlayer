/**
 * QuickJS Browser Mode Adapter
 *
 * Wires the existing quickjs-enclave.ts into contract-simulator as mode: "quickjs-browser".
 * This adapter maps the simulator's canonical execute shape to the enclave's invoke interface.
 *
 * Architecture:
 * - One QuickJS runtime (shared across calls)
 * - Fresh context per invocation (no state leakage)
 * - Deterministic compute (seeded PRNG, Date.now=0, no eval/timers)
 * - Budget enforcement (time + memory per call)
 *
 * SECURITY BOUNDARY:
 * No private key bytes cross into QuickJS.
 * QuickJS computes what to sign; the host performs signing via WebCrypto.
 *
 * From docs/browser-sec-architecture.md: Tier 1 (convenience) security.
 */
import { QuickJSEnclave } from "../quickjs-enclave.js";
export default function createQuickJSBrowserMode(opts = {}) {
    const memoryBytes = opts.memoryBytes ?? (32 << 20);
    const defaultTimeMs = opts.defaultTimeMs ?? 200;
    const defaultSeed = opts.defaultSeed ?? "outlayer-policy-seed";
    let enclave = null;
    return {
        name: "quickjs-browser",
        async init() {
            enclave = await QuickJSEnclave.create({ memoryBytes });
        },
        async execute(input) {
            if (!enclave) {
                enclave = await QuickJSEnclave.create({ memoryBytes });
            }
            const source = input.contractSource;
            const func = input.functionName;
            const args = Array.isArray(input.args) ? input.args : [];
            const priorState = input.priorState ?? {};
            const seed = input.seed ?? defaultSeed;
            const timeMs = input.policy?.timeMs ?? defaultTimeMs;
            const memBytes = input.policy?.memoryBytes ?? memoryBytes;
            const out = await enclave.invoke({
                source,
                func,
                args,
                priorState,
                seed,
                policy: { timeMs, memoryBytes: memBytes },
            });
            return {
                state: out.state,
                result: out.result,
                logs: out.diagnostics.logs,
                timeMs: out.diagnostics.timeMs,
                interrupted: out.diagnostics.interrupted,
                mode: "quickjs-browser",
            };
        },
        async close() {
            if (enclave) {
                enclave.dispose();
                enclave = null;
            }
        },
    };
}
//# sourceMappingURL=quickjs-browser.js.map