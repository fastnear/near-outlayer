/**
 * Verification of a task attestation, performed entirely in the visitor's browser.
 *
 * Three independent layers, each answering a different question:
 *
 *   1. Authenticity — is this a genuine Intel TDX quote? The signature chain is checked against the
 *      Intel root certificate embedded in the verifier, using Intel-issued collateral (TCB info, QE
 *      identity, CRLs). This is the layer that makes the other two mean anything: without it, the
 *      values below are just numbers someone wrote down.
 *   2. Identity — do the measurements taken from the verified quote appear in the on-chain list of
 *      approved worker builds? Read straight from the register-contract over public NEAR RPC.
 *   3. Binding — does the quote commit to THIS execution? The TEE hashes the task parameters into
 *      the quote's report_data before Intel signs it, so recomputing that hash from the published
 *      fields and matching it proves the signature covers this exact input, output and code.
 *
 * The verifier is `dcap-qvl` compiled to WebAssembly — the same crate and version used to verify
 * quotes on chain, so a browser verdict cannot drift from the contract's. Nothing here trusts the
 * coordinator: the quote, the collateral and the approved-measurement list are all inputs that fail
 * the checks if they are wrong.
 */

import { getCoordinatorApiUrl } from './api';
import type { NetworkType } from './near-rpc';

/** Measurements decoded from a verified quote. Lowercase hex. */
export interface Measurements {
  mrtd: string;
  rtmr0: string;
  rtmr1: string;
  rtmr2: string;
  rtmr3: string;
}

export interface AuthenticityResult {
  ok: boolean;
  /** Intel's TCB status verbatim. Only `UpToDate` is fully good — see `tcbStatusIsCurrent`. */
  status?: string;
  advisoryIds: string[];
  error?: string;
}

export interface IdentityResult {
  ok: boolean;
  contract: string;
  error?: string;
}

export interface BindingResult {
  ok: boolean;
  expectedTaskHash: string;
  quoteTaskHash?: string;
  /** report_data bytes 32..64 must be zero — the layout reserves them. */
  reservedBytesZero?: boolean;
  error?: string;
}

export interface CollateralInfo {
  fmspc: string;
  validFrom: string;
  validUntil: string;
  /** False when no published collateral covers the execution time; see the note in the UI. */
  coversExecutionTime: boolean;
}

export interface AttestationVerification {
  authenticity: AuthenticityResult;
  identity: IdentityResult;
  binding: BindingResult;
  measurements?: Measurements;
  collateral?: CollateralInfo;
}

/** Everything the verification needs from an attestation record. */
export interface AttestationInput {
  tdx_quote: string; // base64
  task_type: string;
  task_id: number;
  timestamp: number; // unix seconds — also the instant we verify "as of"
  output_hash: string;
  repo_url?: string;
  commit_hash?: string;
  build_target?: string;
  wasm_hash?: string;
  input_hash?: string;
  block_height?: number;
  caller_account_id?: string;
  project_id?: string;
  secrets_ref?: string;
  attached_usd?: string;
}

/** The register-contract: it holds the approved worker measurements for its network. */
export function registerContractId(network: NetworkType): string {
  return network === 'mainnet' ? 'worker.outlayer.near' : 'worker.outlayer.testnet';
}

/** fastnear, never near.org. */
function rpcUrl(network: NetworkType): string {
  return network === 'mainnet'
    ? process.env.NEXT_PUBLIC_MAINNET_RPC_URL || 'https://rpc.mainnet.fastnear.com'
    : process.env.NEXT_PUBLIC_TESTNET_RPC_URL || 'https://rpc.testnet.fastnear.com';
}

/**
 * Only `UpToDate` means the platform is fully current. `verify()` also succeeds for `OutOfDate` and
 * `ConfigurationNeeded` — the signature is genuine in those cases, but Intel is flagging the
 * platform — so the status is surfaced instead of being collapsed into the pass/fail bit.
 */
export function tcbStatusIsCurrent(status?: string): boolean {
  return status === 'UpToDate';
}

let wasmReady: Promise<typeof import('./attestation-wasm/verifier')> | null = null;

/**
 * Load the WebAssembly verifier once, on demand.
 *
 * Served from `public/` and loaded by explicit URL rather than through bundler asset handling: it
 * keeps the ~660 KB module out of the initial page load and off the critical path for every visitor
 * who never opens an attestation.
 */
async function loadVerifier() {
  if (!wasmReady) {
    wasmReady = (async () => {
      const mod = await import('./attestation-wasm/verifier');
      await mod.default({ module_or_path: '/attestation-verifier.wasm' });
      return mod;
    })();
  }
  return wasmReady;
}

function base64ToBytes(base64: string): Uint8Array {
  const binary = atob(base64);
  const bytes = new Uint8Array(binary.length);
  for (let i = 0; i < binary.length; i++) bytes[i] = binary.charCodeAt(i);
  return bytes;
}

/**
 * Cut-off after which attestations commit to the extended (V1) field set.
 * `0` means the cut-off is not configured, in which case every attestation is read as pre-V1.
 */
const V1_CUTOFF = parseInt(process.env.NEXT_PUBLIC_OUTLAYER_ATTESTATION_V1 || '0', 10);

export function isV1Attestation(timestamp: number): boolean {
  return V1_CUTOFF > 0 && timestamp >= V1_CUTOFF;
}

/**
 * Recompute the commitment the TEE hashed into report_data before the quote was signed.
 *
 * Field order and encoding are part of the attestation format: strings as UTF-8, `task_id` and
 * `timestamp` as little-endian i64, `block_height` as little-endian u64, absent optional fields
 * contributing nothing. Attestations from before the V1 cut-off commit to the first block of
 * fields only, so the same function has to reproduce both shapes to check either.
 */
export async function calculateTaskHash(att: AttestationInput): Promise<string> {
  const encoder = new TextEncoder();
  const parts: Uint8Array[] = [];

  const int64 = (value: number, signed: boolean) => {
    const buffer = new ArrayBuffer(8);
    const view = new DataView(buffer);
    if (signed) view.setBigInt64(0, BigInt(value), true);
    else view.setBigUint64(0, BigInt(value), true);
    return new Uint8Array(buffer);
  };

  parts.push(encoder.encode(att.task_type));
  parts.push(int64(att.task_id, true));
  if (att.repo_url) parts.push(encoder.encode(att.repo_url));
  if (att.commit_hash) parts.push(encoder.encode(att.commit_hash));
  if (att.build_target) parts.push(encoder.encode(att.build_target));
  if (att.wasm_hash) parts.push(encoder.encode(att.wasm_hash));
  if (att.input_hash) parts.push(encoder.encode(att.input_hash));
  parts.push(encoder.encode(att.output_hash));
  if (att.block_height) parts.push(int64(att.block_height, false));

  if (isV1Attestation(att.timestamp)) {
    if (att.caller_account_id) parts.push(encoder.encode(att.caller_account_id));
    if (att.project_id) parts.push(encoder.encode(att.project_id));
    if (att.secrets_ref) parts.push(encoder.encode(att.secrets_ref));
    parts.push(int64(att.timestamp, true));
    if (att.attached_usd) parts.push(encoder.encode(att.attached_usd));
  }

  const total = parts.reduce((sum, part) => sum + part.length, 0);
  const combined = new Uint8Array(total);
  let offset = 0;
  for (const part of parts) {
    combined.set(part, offset);
    offset += part.length;
  }

  const digest = await crypto.subtle.digest('SHA-256', combined);
  return Array.from(new Uint8Array(digest))
    .map((b) => b.toString(16).padStart(2, '0'))
    .join('');
}

/**
 * The collateral that was valid when this execution ran.
 *
 * Collateral is time-bounded: Intel publishes TCB info with a validity window, and a quote is
 * verified as of the moment it was produced, so a past execution needs the collateral that was
 * current back then. Every published version is archived and served here.
 */
async function fetchCollateral(
  fmspc: string,
  atUnixSeconds: number,
  network: NetworkType,
): Promise<{ body: string; info: CollateralInfo }> {
  const url = `${getCoordinatorApiUrl(network)}/public/collateral?fmspc=${encodeURIComponent(
    fmspc,
  )}&at=${atUnixSeconds}`;

  const response = await fetch(url);
  if (!response.ok) {
    throw new Error(
      `collateral for platform ${fmspc} is unavailable (HTTP ${response.status})`,
    );
  }
  const data = await response.json();
  return {
    body: data.collateral,
    info: {
      fmspc: data.fmspc,
      validFrom: data.valid_from,
      validUntil: data.valid_until,
      coversExecutionTime: data.covers_requested_time,
    },
  };
}

/** Ask the register-contract whether these measurements are an approved worker build. */
async function measurementsApproved(
  measurements: Measurements,
  network: NetworkType,
): Promise<boolean> {
  const contract = registerContractId(network);
  const args = btoa(JSON.stringify({ measurements }));

  const response = await fetch(rpcUrl(network), {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({
      jsonrpc: '2.0',
      id: 'attestation-verify',
      method: 'query',
      params: {
        request_type: 'call_function',
        finality: 'final',
        account_id: contract,
        method_name: 'is_measurements_approved',
        args_base64: args,
      },
    }),
  });

  const json = await response.json();
  if (json.error) throw new Error(`RPC error: ${JSON.stringify(json.error).slice(0, 200)}`);
  if (json.result?.error) throw new Error(String(json.result.error).slice(0, 200));

  const decoded = new TextDecoder().decode(new Uint8Array(json.result.result));
  return JSON.parse(decoded) === true;
}

/**
 * Run all three layers. Never throws: a failure in one layer is reported in that layer and the
 * others still run where they can, so the page can show exactly how far verification got.
 */
export async function verifyAttestation(
  att: AttestationInput,
  network: NetworkType,
): Promise<AttestationVerification> {
  const expectedTaskHash = await calculateTaskHash(att);

  const result: AttestationVerification = {
    authenticity: { ok: false, advisoryIds: [] },
    identity: { ok: false, contract: registerContractId(network) },
    binding: { ok: false, expectedTaskHash },
  };

  let quote: Uint8Array;
  try {
    quote = base64ToBytes(att.tdx_quote);
  } catch {
    result.authenticity.error = 'attestation does not contain a decodable quote';
    return result;
  }

  let verifier: Awaited<ReturnType<typeof loadVerifier>>;
  try {
    verifier = await loadVerifier();
  } catch (e) {
    result.authenticity.error = `verifier failed to load: ${e instanceof Error ? e.message : e}`;
    return result;
  }

  // The quote states its own platform; that is what selects the collateral.
  const parsed = verifier.parse_quote(quote) as {
    ok: boolean;
    error?: string;
    fmspc?: string;
  };
  if (!parsed.ok || !parsed.fmspc) {
    result.authenticity.error = parsed.error ?? 'quote could not be decoded';
    return result;
  }

  let collateral: { body: string; info: CollateralInfo };
  try {
    collateral = await fetchCollateral(parsed.fmspc, att.timestamp, network);
    result.collateral = collateral.info;
  } catch (e) {
    result.authenticity.error = e instanceof Error ? e.message : String(e);
    return result;
  }

  // Layer 1 — verify as of the moment the attestation was produced, not the wall clock.
  const verified = verifier.verify_quote(quote, collateral.body, att.timestamp) as {
    ok: boolean;
    error?: string;
    status?: string;
    advisory_ids?: string[];
    measurements?: Measurements;
    report_data_prefix?: string;
  };

  result.authenticity = {
    ok: verified.ok,
    status: verified.status,
    advisoryIds: verified.advisory_ids ?? [],
    error: verified.error,
  };

  if (!verified.ok) return result;
  result.measurements = verified.measurements;

  // Layer 3 — the commitment inside the now-verified quote must match the published fields.
  result.binding = {
    ok: verified.report_data_prefix === expectedTaskHash,
    expectedTaskHash,
    quoteTaskHash: verified.report_data_prefix,
  };

  // Layer 2 — the verified measurements must be an approved build.
  if (verified.measurements) {
    try {
      result.identity.ok = await measurementsApproved(verified.measurements, network);
      if (!result.identity.ok) {
        result.identity.error = 'measurements are not in the approved list';
      }
    } catch (e) {
      result.identity.error = e instanceof Error ? e.message : String(e);
    }
  }

  return result;
}
