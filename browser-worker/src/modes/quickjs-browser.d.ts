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
export type QuickJSBrowserModeOptions = {
    memoryBytes?: number;
    defaultTimeMs?: number;
    defaultSeed?: string;
};
export type ExecutionInput = {
    contractSource: string;
    functionName: string;
    args?: unknown[];
    priorState?: unknown;
    seed?: string;
    policy?: {
        timeMs?: number;
        memoryBytes?: number;
    };
};
export type ExecutionOutput = {
    state: unknown;
    result: unknown;
    logs: string[];
    timeMs: number;
    interrupted: boolean;
    mode: "quickjs-browser";
};
export interface SimulatorMode {
    name: string;
    init(): Promise<void>;
    execute(input: ExecutionInput): Promise<ExecutionOutput>;
    close(): Promise<void>;
}
export default function createQuickJSBrowserMode(opts?: QuickJSBrowserModeOptions): SimulatorMode;
//# sourceMappingURL=quickjs-browser.d.ts.map