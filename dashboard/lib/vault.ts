/**
 * Per-customer sovereign vault — dashboard client (Phase 7).
 *
 * Mirrors the surface of `outlayer-cli/src/commands/vault.rs`:
 *   - WASM hash lookup against keystore-DAO `is_vault_code_approved`
 *   - TEE pubkey fetch via `coordinator /customer/derive-tee-key`
 *   - 5-action atomic deploy via wallet-selector `signAndSendTransaction`
 *   - sign-verification + customer/register
 *   - vault.get_state + access-key audit for `verify`
 *
 * Plan reference: partitioned-dreaming-patterson.md lines 681-722.
 */

import { actionCreators, GlobalContractIdentifier } from '@near-js/transactions';
import { PublicKey } from '@near-js/crypto';

import { getCoordinatorApiUrl, type NetworkType } from './api';

// ─── Base58 (NEAR `Base58CryptoHash` form) ───────────────────────────────
//
// Standard Bitcoin alphabet — identical to `bs58.encode/decode` output.
// Decoder is the only call site that matters now: the operator-supplied
// vault code hash arrives as base58 from env and we need 32 raw bytes
// for the `UseGlobalContract` action.
const BASE58_ALPHABET =
  '123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz';

function bs58Decode(s: string): Uint8Array {
  const map = new Map<string, number>();
  for (let i = 0; i < BASE58_ALPHABET.length; i++) map.set(BASE58_ALPHABET[i], i);
  let zeros = 0;
  while (zeros < s.length && s[zeros] === BASE58_ALPHABET[0]) zeros++;
  const bytes: number[] = [];
  for (let i = zeros; i < s.length; i++) {
    const v = map.get(s[i]);
    if (v === undefined) throw new Error(`bs58Decode: invalid char '${s[i]}'`);
    let carry = v;
    for (let j = 0; j < bytes.length; j++) {
      carry += bytes[j] * 58;
      bytes[j] = carry & 0xff;
      carry >>= 8;
    }
    while (carry > 0) {
      bytes.push(carry & 0xff);
      carry >>= 8;
    }
  }
  const out = new Uint8Array(zeros + bytes.length);
  for (let i = bytes.length - 1, k = zeros; i >= 0; i--, k++) out[k] = bytes[i];
  return out;
}

// ─── Network-scoped contract addresses ────────────────────────────────────

export interface VaultNetworkConfig {
  /** keystore-DAO contract id (`is_vault_verified`, `is_vault_code_approved`). */
  keystoreDaoId: string;
  /** MPC signer contract id — burned into the vault contract at deploy. */
  mpcContractId: string;
}

export function getVaultNetworkConfig(network: NetworkType): VaultNetworkConfig {
  if (network === 'mainnet') {
    return {
      // Production deploy uses `dao.outlayer.near` (docker
      // .env.mainnet-keystore-phala). The keystore worker's
      // KEYSTORE_DAO_CONTRACT is the canonical source.
      keystoreDaoId: process.env.NEXT_PUBLIC_MAINNET_KEYSTORE_DAO_ID || 'dao.outlayer.near',
      mpcContractId: process.env.NEXT_PUBLIC_MAINNET_MPC_CONTRACT_ID || 'v1.signer',
    };
  }
  return {
    keystoreDaoId: process.env.NEXT_PUBLIC_TESTNET_KEYSTORE_DAO_ID || 'dao.outlayer.testnet',
    mpcContractId: process.env.NEXT_PUBLIC_TESTNET_MPC_CONTRACT_ID || 'v1.signer-prod.testnet',
  };
}

// ─── Wire types ───────────────────────────────────────────────────────────

export interface VaultState {
  parent: string;
  keystore_dao: string;
  mpc_contract: string;
  registered_tee_keys: string[];
  recovery: RecoveryState | null;
  unlocked: boolean;
  unilateral_exit_window_secs: number;
}

export interface RecoveryState {
  // u64 nanoseconds. The contract serializes these as JSON numbers
  // (no `U64` wrapper), so values >2^53 lose precision through
  // `JSON.parse`. We keep `number | string` here to allow callers to
  // detect a string and preserve precision via `BigInt` for any
  // future arithmetic; for display via `Date(.../1e6)` either form
  // is fine because Date's resolution is milliseconds.
  initiated_at: number | string;
  finalize_after: number | string;
  finalize_before: number | string;
  trigger: 'Cessation' | 'Unilateral';
}

/** Convert a u64-as-(number|string) ns timestamp to a JS `Date`. */
export function nsToDate(ns: number | string): Date {
  const big = typeof ns === 'string' ? BigInt(ns) : BigInt(Math.floor(ns));
  // Drop ns→ms (1e6) inside BigInt to avoid Number's 53-bit limit.
  const ms = Number(big / BigInt(1_000_000));
  return new Date(ms);
}

export interface AccessKeyEntry {
  public_key: string;
  permission:
    | { kind: 'FullAccess' }
    | {
        kind: 'FunctionCall';
        allowance: string | null; // null = unlimited
        receiver_id: string;
        method_names: string[];
      };
}

// ─── Sizing constants ─────────────────────────────────────────────────────

/**
 * Initial NEAR transferred to the vault account at deploy.
 *
 * With NEP-591 `UseGlobalContract` the WASM bytes (~150 KB) live in
 * the global registry, not on this account, so storage stake collapses
 * from ~1.5 NEAR to whatever the contract STATE plus access keys take.
 * Measured on-chain after a fresh deploy: `storage_usage = 391 bytes`
 * (three `AccountId`s, flags, empty `registered_tee_keys` Vec) →
 * ~0.004 NEAR storage stake.
 *
 * Outbound `vault.request_master → mpc.request_app_private_key` costs
 * ~0.001 NEAR/call (gas burn; the deposit is 1 yocto). The master is
 * cached in keystore-worker enclave memory after the first call, so a
 * vault typically only triggers MPC a handful of times in its
 * lifetime (initial customer use + occasional keystore restarts).
 *
 * 0.1 NEAR ≈ 0.004 storage + ~100 MPC-calls headroom with 10× safety
 * margin on storage growth (registered_tee_keys, recovery state).
 * High-frequency derivers can top up; the parent account budget check
 * is enforced before deploy so the user can't get stuck mid-flow.
 *
 * Must match `outlayer-cli/src/commands/vault.rs::VAULT_INITIAL_NEAR`.
 */
export const VAULT_INITIAL_YOCTO = BigInt('100000000000000000000000'); // 0.1 NEAR

/** Conservative parent-balance check: initial transfer + 0.1 NEAR gas headroom. */
export const VAULT_PARENT_BUDGET_YOCTO =
  VAULT_INITIAL_YOCTO + BigInt('100000000000000000000000');

/** Gas for the inline `new()` call (30 TGas — pure constructor logic). */
export const VAULT_NEW_GAS = BigInt('30000000000000');

/** Gas for vault recovery / set-exit-window / unlocked-add-key (100 TGas). */
export const VAULT_CALL_GAS = BigInt('100000000000000');

// ─── Window parsing ───────────────────────────────────────────────────────

/**
 * Parse '24h' / '7d' / '30d' into seconds. Same shape as the CLI's
 * `parse_exit_window`. Bounds (24h..30d) are enforced contract-side.
 */
export function parseExitWindow(input: string): number {
  const trimmed = input.trim();
  if (!trimmed) {
    throw new Error("exit window cannot be empty (use '24h', '7d', or '30d')");
  }
  const m = trimmed.match(/^(\d+)([hdHD])$/);
  if (!m) {
    throw new Error(
      `invalid exit window '${input}'; use '24h', '7d', '30d' or similar`,
    );
  }
  const n = parseInt(m[1], 10);
  const unit = m[2].toLowerCase();
  const secs = unit === 'h' ? n * 3600 : n * 86_400;
  if (secs < 86_400 || secs > 30 * 86_400) {
    throw new Error(
      `exit window must be between 24h (86400s) and 30d (2592000s); got ${secs}s`,
    );
  }
  return secs;
}

export function formatSeconds(secs: number): string {
  if (secs % 86_400 === 0 && secs >= 86_400) {
    return `${secs / 86_400}d (${secs}s)`;
  }
  if (secs % 3600 === 0 && secs >= 3600) {
    return `${secs / 3600}h (${secs}s)`;
  }
  return `${secs}s`;
}

// ─── Vault global-contract code hash (operator-managed) ──────────────────

/**
 * Resolve the vault WASM hash to deploy against by reading
 * `keystore-DAO.list_approved_vault_versions()` and picking the most
 * recently approved non-deprecated entry. Authoritative source =
 * on-chain DAO state. No env-var, no config bake-in: when the DAO
 * approves a new version (and optionally deprecates the old one),
 * every dashboard user picks it up on their next deploy.
 *
 * The hash names a blob already published via `near contract
 * deploy-as-global ... as-global-hash` AND approved by DAO — both
 * conditions are gated by `approve_vault_version` on the DAO side.
 *
 * Throws if the DAO has no non-deprecated approved version — the
 * dashboard surfaces this with a banner pointing at DAO governance.
 */
type VaultVersionInfo = {
  label: string;
  deprecated: boolean;
  approved_at: string; // u64 nanoseconds serialized as a JSON string by near-sdk
  audit_url?: string | null;
};

export async function getVaultCodeHash(
  viewMethod: ViewMethodFn,
  network: NetworkType,
): Promise<{ hashB58: string; hashBytes: Uint8Array; label: string }> {
  const cfg = getVaultNetworkConfig(network);

  const versions = (await viewMethod({
    contractId: cfg.keystoreDaoId,
    method: 'list_approved_vault_versions',
    args: {},
  })) as Array<[string, VaultVersionInfo]>;

  let best: { hash: string; info: VaultVersionInfo; ts: bigint } | null = null;
  for (const [hash, info] of versions) {
    if (info.deprecated) continue;
    const ts = BigInt(info.approved_at);
    if (best === null || ts > best.ts) {
      best = { hash, info, ts };
    }
  }
  if (!best) {
    throw new Error(
      `${cfg.keystoreDaoId} has no non-deprecated approved vault code hash. `
        + `Operator must publish a vault WASM as a global contract and approve the `
        + `hash via \`approve_vault_version\` before any vault can be deployed.`,
    );
  }
  const hashBytes = bs58Decode(best.hash);
  if (hashBytes.length !== 32) {
    throw new Error(
      `DAO returned vault code hash '${best.hash}' that decoded to ${hashBytes.length} bytes; expected 32.`,
    );
  }
  // Log the resolved hash + label so the operator (and customer in
  // browser devtools) can confirm which version is about to be
  // deployed. Helps catch "DAO whitelisted but I'm pointing at the
  // wrong network's DAO" mistakes without ceremony.
  // ns → ms via integer division on bigint, then Number for Date.
  // Safe through year 2554 (Date max ≈ 8.64e15 ms).
  const approvedAtMs = Number(best.ts / BigInt(1_000_000));
  // eslint-disable-next-line no-console
  console.info(
    `[vault] Using vault code hash ${best.hash} (label="${best.info.label}", `
      + `approved at ${new Date(approvedAtMs).toISOString()}) `
      + `from ${cfg.keystoreDaoId}`,
  );
  return { hashB58: best.hash, hashBytes, label: best.info.label };
}

// ─── View calls (RPC) ─────────────────────────────────────────────────────

interface ViewMethodFn {
  (params: {
    contractId: string;
    method: string;
    args?: Record<string, unknown>;
  }): Promise<unknown>;
}

export async function isVaultVerified(
  viewMethod: ViewMethodFn,
  network: NetworkType,
  vaultId: string,
): Promise<boolean> {
  const cfg = getVaultNetworkConfig(network);
  const result = await viewMethod({
    contractId: cfg.keystoreDaoId,
    method: 'is_vault_verified',
    args: { vault_id: vaultId },
  });
  return result === true;
}

export async function isVaultCodeApproved(
  viewMethod: ViewMethodFn,
  network: NetworkType,
  hashB58: string,
): Promise<boolean> {
  const cfg = getVaultNetworkConfig(network);
  // Contract method signature is
  // `is_vault_code_approved(hash: Base58CryptoHash)`. The arg key MUST
  // be `hash` — `code_hash` is rejected.
  const result = await viewMethod({
    contractId: cfg.keystoreDaoId,
    method: 'is_vault_code_approved',
    args: { hash: hashB58 },
  });
  return result === true;
}

export async function getVaultState(
  viewMethod: ViewMethodFn,
  vaultId: string,
): Promise<VaultState> {
  const result = (await viewMethod({
    contractId: vaultId,
    method: 'get_state',
    args: {},
  })) as VaultState;
  return result;
}

/** Wraps a generic NEAR JSON-RPC view_account/view_access_key_list call. */
export async function viewAccountInfo(
  rpcUrl: string,
  accountId: string,
): Promise<{ exists: boolean; codeHash: string; amountYocto: string }> {
  const resp = await fetch(rpcUrl, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({
      jsonrpc: '2.0',
      id: 'dontcare',
      method: 'query',
      params: {
        request_type: 'view_account',
        finality: 'final',
        account_id: accountId,
      },
    }),
  });
  const data = await resp.json();
  if (data.error) {
    const msg = (data.error.message || '') + ' ' + JSON.stringify(data.error.data || '');
    if (msg.includes('UnknownAccount') || msg.includes('does not exist')) {
      return { exists: false, codeHash: '', amountYocto: '0' };
    }
    throw new Error(`view_account('${accountId}') failed: ${msg}`);
  }
  // NEP-591: accounts deployed via UseGlobalContract leave `code_hash`
  // at the all-zeros sentinel and store the real WASM hash in
  // `global_contract_hash`. Surface either as the same opaque string
  // so the DAO whitelist check works for both deploy shapes.
  const inline: string = data.result.code_hash;
  const global: string | undefined = data.result.global_contract_hash;
  const NO_CODE_LOCAL = '11111111111111111111111111111111';
  const codeHash = inline && inline !== NO_CODE_LOCAL ? inline : (global || inline || '');
  return {
    exists: true,
    codeHash,
    amountYocto: data.result.amount,
  };
}

export async function viewAccessKeyList(
  rpcUrl: string,
  accountId: string,
): Promise<AccessKeyEntry[]> {
  const resp = await fetch(rpcUrl, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({
      jsonrpc: '2.0',
      id: 'dontcare',
      method: 'query',
      params: {
        request_type: 'view_access_key_list',
        finality: 'final',
        account_id: accountId,
      },
    }),
  });
  const data = await resp.json();
  if (data.error) {
    throw new Error(`view_access_key_list('${accountId}') failed: ${data.error.message}`);
  }
  type RawKey = {
    public_key: string;
    access_key: {
      permission:
        | 'FullAccess'
        | { FunctionCall: { allowance: string | null; receiver_id: string; method_names: string[] } };
    };
  };
  const keys = (data.result.keys || []) as RawKey[];
  return keys.map((k): AccessKeyEntry => {
    if (k.access_key.permission === 'FullAccess') {
      return { public_key: k.public_key, permission: { kind: 'FullAccess' } };
    }
    const fc = k.access_key.permission.FunctionCall;
    return {
      public_key: k.public_key,
      permission: {
        kind: 'FunctionCall',
        allowance: fc.allowance,
        receiver_id: fc.receiver_id,
        method_names: fc.method_names,
      },
    };
  });
}

// ─── Coordinator endpoints ────────────────────────────────────────────────

export async function deriveVaultTeeKey(
  network: NetworkType,
  vaultAccountId: string,
): Promise<string> {
  const url = `${getCoordinatorApiUrl(network)}/customer/derive-tee-key`;
  const resp = await fetch(url, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ vault_account_id: vaultAccountId }),
  });
  if (!resp.ok) {
    throw new Error(`derive-tee-key failed (${resp.status}): ${await resp.text()}`);
  }
  const data = (await resp.json()) as { public_key: string };
  return data.public_key;
}

export async function signVaultVerification(
  network: NetworkType,
  vaultAccountId: string,
): Promise<{ tx_hash: string | null; already_verified: boolean }> {
  const url = `${getCoordinatorApiUrl(network)}/customer/sign-verification`;
  const resp = await fetch(url, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ vault_account_id: vaultAccountId }),
  });
  if (!resp.ok) {
    throw new Error(`sign-verification failed (${resp.status}): ${await resp.text()}`);
  }
  return resp.json();
}

/**
 * `GET /customer/list-vaults?owner=<account>` — populate the
 * "Use my vault" dropdown in wallet/secrets forms (Phase 7 F2).
 *
 * Returns vaults that completed `/customer/register` for this owner
 * on the current coordinator. Returns `[]` (not error) for unknown
 * owners or coordinators with no registered vaults.
 */
export interface VaultListEntry {
  vault_id: string;
  /**
   * Custody NEAR public key (`ed25519:<base58>`) derived from this
   * vault's per-customer master. The implicit account id is
   * `hex(pubkey)` — the dashboard can render either.
   */
  near_pubkey: string | null;
}

export async function listVaults(
  network: NetworkType,
  owner: string,
): Promise<VaultListEntry[]> {
  const url = `${getCoordinatorApiUrl(network)}/customer/list-vaults?owner=${encodeURIComponent(owner)}`;
  const resp = await fetch(url);
  if (!resp.ok) {
    throw new Error(`list-vaults failed (${resp.status}): ${await resp.text()}`);
  }
  const data = (await resp.json()) as { vaults: VaultListEntry[] };
  return data.vaults || [];
}

// ─── Atomic deploy action builder ─────────────────────────────────────────

/**
 * Build the 5-action vault-deploy actions list. Caller passes this
 * (along with `receiverId = vaultAccountId`) to wallet-selector's
 * `signAndSendTransaction`. All-or-nothing: if any action panics the
 * sub-account state is rolled back.
 */
export function buildVaultDeployActions(args: {
  parent: string;
  vaultAccountId: string; // `${name}.${parent}`
  keystoreDaoId: string;
  mpcContractId: string;
  exitWindowSecs: number;
  /** Raw 32-byte SHA256 of the canonical vault WASM. The bytes must
   *  already be deployed on-chain as a Global Contract by hash (see
   *  `near contract deploy-as-global use-file ... as-global-hash`). */
  wasmCodeHash: Uint8Array;
  teePublicKey: string; // 'ed25519:...'
}) {
  const newArgs = JSON.stringify({
    parent: args.parent,
    keystore_dao: args.keystoreDaoId,
    mpc_contract: args.mpcContractId,
    initial_exit_window: args.exitWindowSecs,
  });

  return [
    actionCreators.createAccount(),
    actionCreators.transfer(VAULT_INITIAL_YOCTO),
    // UseGlobalContract instead of inline DeployContract: the same
    // ~150 KB WASM is referenced by hash from the chain's global
    // contract storage instead of being shipped in this tx. Tx
    // payload drops from ~200 KB to ~hundreds of bytes — fits inside
    // MyNearWallet's URL limit. Pre-condition: the WASM with this
    // hash must already be deployed via DeployGlobalContract.
    actionCreators.useGlobalContract(
      new GlobalContractIdentifier({ CodeHash: args.wasmCodeHash }),
    ),
    actionCreators.functionCall(
      'new',
      new TextEncoder().encode(newArgs),
      VAULT_NEW_GAS,
      BigInt(0),
    ),
    actionCreators.addKey(
      PublicKey.fromString(args.teePublicKey),
      // The TEE function-call key signs `vault.request_master(...)` —
      // a self-call into the vault's MPC-CKD proxy method. Direct
      // `mpc.request_app_private_key` calls are blocked by FC-key
      // deposit rules (MPC asserts 1 yocto, FC keys can only attach
      // 0); the proxy supplies the yocto from the vault's balance.
      actionCreators.functionCallAccessKey(args.vaultAccountId, ['request_master']),
    ),
  ];
}

// ─── verify helper (defense-in-depth) ─────────────────────────────────────

export interface VerifyReport {
  vaultId: string;
  isVerified: boolean;
  exists: boolean;
  codeHashApproved: boolean | null; // null = no contract
  state: VaultState | null;
  accessKeys: AccessKeyEntry[];
  warnings: string[];
  /** Final user-facing safety verdict — false ⇒ "do not deposit". */
  safe: boolean;
  /**
   * Vault account balance in yoctoNEAR. Surfaced because gas for
   * outbound `vault.request_master → mpc.request_app_private_key` is
   * paid from the vault account (it owns the FCAK that signs that
   * call). When the balance falls below the gas reserve, the next
   * MPC-CKD round-trip stalls and the keystore can no longer refresh
   * the customer's master in enclave memory. The dashboard renders a
   * top-up prompt below this threshold.
   */
  amountYocto: string;
}

/**
 * Below this balance the dashboard surfaces a top-up warning. ~50
 * MPC calls of headroom — enough that we don't cry-wolf during normal
 * operation but loud enough that the customer notices before stalls.
 */
export const VAULT_LOW_BALANCE_YOCTO = BigInt('50000000000000000000000'); // 0.05 NEAR

/** Minimum top-up the dashboard suggests when balance is low. */
export const VAULT_TOPUP_SUGGESTED_YOCTO = BigInt('100000000000000000000000'); // 0.1 NEAR

export async function verifyVault(
  viewMethod: ViewMethodFn,
  rpcUrl: string,
  network: NetworkType,
  vaultId: string,
): Promise<VerifyReport> {
  const warnings: string[] = [];
  const cfg = getVaultNetworkConfig(network);

  const info = await viewAccountInfo(rpcUrl, vaultId);
  if (!info.exists) {
    return {
      vaultId,
      isVerified: false,
      exists: false,
      codeHashApproved: null,
      state: null,
      accessKeys: [],
      warnings: [`Account ${vaultId} does not exist on ${network}`],
      safe: false,
      amountYocto: '0',
    };
  }

  const isVerified = await isVaultVerified(viewMethod, network, vaultId);
  if (!isVerified) {
    warnings.push('vault is NOT in keystore-dao.verified_vaults');
  }

  const NO_CODE = '11111111111111111111111111111111';
  let codeHashApproved: boolean | null = null;
  if (info.codeHash === NO_CODE) {
    warnings.push(`no contract deployed at ${vaultId}`);
    codeHashApproved = null;
  } else {
    codeHashApproved = await isVaultCodeApproved(viewMethod, network, info.codeHash);
    if (!codeHashApproved) {
      warnings.push(`vault code hash ${info.codeHash} is NOT in keystore-DAO approved set`);
    }
  }

  let state: VaultState | null = null;
  try {
    state = await getVaultState(viewMethod, vaultId);
  } catch (e) {
    warnings.push(`get_state failed: ${(e as Error).message}`);
  }

  if (state) {
    if (state.keystore_dao !== cfg.keystoreDaoId) {
      warnings.push(`vault.keystore_dao = ${state.keystore_dao} ≠ network ${cfg.keystoreDaoId}`);
    }
    if (state.mpc_contract !== cfg.mpcContractId) {
      warnings.push(`vault.mpc_contract = ${state.mpc_contract} ≠ network ${cfg.mpcContractId}`);
    }
    if (state.unlocked) {
      warnings.push('vault is UNLOCKED — parent has post-recovery key authority');
    }
    if (state.registered_tee_keys.length === 0) {
      warnings.push('vault has no registered TEE keys');
    }
    if (state.recovery) {
      warnings.push(`recovery in progress (${state.recovery.trigger})`);
    }
  }

  const accessKeys = await viewAccessKeyList(rpcUrl, vaultId);
  let badKeys = 0;
  for (const k of accessKeys) {
    if (k.permission.kind === 'FullAccess') {
      badKeys++;
      warnings.push(`vault has a FULL-ACCESS key ${k.public_key} — must not exist`);
    } else {
      // The TEE function-call key signs `vault.request_master(...)`
      // — a self-call into the vault's MPC-CKD proxy. Direct
      // `mpc.request_app_private_key` calls are blocked by FC-key
      // deposit rules, so the receiver must be the vault itself
      // and the only allowed method is `request_master`.
      const scopeOk =
        k.permission.receiver_id === vaultId &&
        k.permission.method_names.length === 1 &&
        k.permission.method_names[0] === 'request_master';
      const unlockedSelfCall =
        state?.unlocked === true && k.permission.receiver_id === vaultId;
      if (!scopeOk && !unlockedSelfCall) {
        badKeys++;
        warnings.push(
          `access key ${k.public_key} unexpected scope: receiver=${k.permission.receiver_id}, methods=${JSON.stringify(k.permission.method_names)}`,
        );
      }
      if (scopeOk && k.permission.allowance !== null) {
        warnings.push(
          `TEE access key ${k.public_key} has limited allowance ${k.permission.allowance} (expected unlimited)`,
        );
      }
    }
  }

  // Cross-check registered_tee_keys ⊆ access keys.
  if (state) {
    const onChain = new Set(accessKeys.map((k) => k.public_key));
    for (const k of state.registered_tee_keys) {
      if (!onChain.has(k)) {
        warnings.push(`registered TEE key ${k} not present on access-key list`);
      }
    }
  }

  const safe =
    isVerified &&
    badKeys === 0 &&
    !!state &&
    !state.unlocked &&
    codeHashApproved === true;

  if (BigInt(info.amountYocto) < VAULT_LOW_BALANCE_YOCTO) {
    warnings.push(
      `vault balance is ${(Number(info.amountYocto) / 1e24).toFixed(4)} NEAR — `
        + `top up so the keystore can keep refreshing your master via MPC`,
    );
  }

  return {
    vaultId,
    isVerified,
    exists: true,
    codeHashApproved,
    state,
    accessKeys,
    warnings,
    safe,
    amountYocto: info.amountYocto,
  };
}
