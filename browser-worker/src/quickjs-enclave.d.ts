export type Json = unknown;
export interface EnclavePolicy {
    timeMs: number;
    memoryBytes: number;
}
export interface Invocation {
    source: string;
    func: string;
    args: Json[];
    priorState: Json;
    seed: string;
    policy: EnclavePolicy;
}
export interface InvocationResult {
    state: Json;
    result: Json;
    diagnostics: {
        logs: string[];
        timeMs: number;
        interrupted: boolean;
    };
}
export declare class QuickJSEnclave {
    private mod;
    private runtime;
    static create(opts?: {
        memoryBytes?: number;
    }): Promise<QuickJSEnclave>;
    private constructor();
    dispose(): void;
    invoke(inv: Invocation): Promise<InvocationResult>;
}
//# sourceMappingURL=quickjs-enclave.d.ts.map