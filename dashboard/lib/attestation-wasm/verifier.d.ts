/* tslint:disable */
/* eslint-disable */

export function js_verify(raw_quote: any, quote_collateral: any, now: bigint): any;

export function js_verify_with_root_ca(raw_quote: any, quote_collateral: any, root_ca_der: any, now: bigint): any;

/**
 * Decode a quote without verifying it.
 *
 * Used for one thing only: learning the FMSPC so the caller can fetch the collateral that matches
 * this platform. Everything it returns is unverified by definition — display nothing from here as
 * established fact.
 */
export function parse_quote(quote: Uint8Array): any;

/**
 * Verify a quote against Intel-issued collateral, as of `now_secs`.
 *
 * `now_secs` must be the moment the attestation was produced, not the wall clock: collateral is
 * only valid inside its own window, so a past execution has to be judged against the collateral
 * that was current when it ran.
 */
export function verify_quote(quote: Uint8Array, collateral_json: string, now_secs: number): any;

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
    readonly memory: WebAssembly.Memory;
    readonly parse_quote: (a: number, b: number) => any;
    readonly verify_quote: (a: number, b: number, c: number, d: number, e: number) => any;
    readonly js_verify: (a: any, b: any, c: bigint) => [number, number, number];
    readonly js_verify_with_root_ca: (a: any, b: any, c: any, d: bigint) => [number, number, number];
    readonly __wbindgen_malloc: (a: number, b: number) => number;
    readonly __wbindgen_realloc: (a: number, b: number, c: number, d: number) => number;
    readonly __wbindgen_exn_store: (a: number) => void;
    readonly __externref_table_alloc: () => number;
    readonly __wbindgen_externrefs: WebAssembly.Table;
    readonly __externref_table_dealloc: (a: number) => void;
    readonly __wbindgen_start: () => void;
}

export type SyncInitInput = BufferSource | WebAssembly.Module;

/**
 * Instantiates the given `module`, which can either be bytes or
 * a precompiled `WebAssembly.Module`.
 *
 * @param {{ module: SyncInitInput }} module - Passing `SyncInitInput` directly is deprecated.
 *
 * @returns {InitOutput}
 */
export function initSync(module: { module: SyncInitInput } | SyncInitInput): InitOutput;

/**
 * If `module_or_path` is {RequestInfo} or {URL}, makes a request and
 * for everything else, calls `WebAssembly.instantiate` directly.
 *
 * @param {{ module_or_path: InitInput | Promise<InitInput> }} module_or_path - Passing `InitInput` directly is deprecated.
 *
 * @returns {Promise<InitOutput>}
 */
export default function __wbg_init (module_or_path?: { module_or_path: InitInput | Promise<InitInput> } | InitInput | Promise<InitInput>): Promise<InitOutput>;
