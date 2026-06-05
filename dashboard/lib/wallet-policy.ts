import { actionCreators } from '@near-js/transactions';

// ============================================================================
// Types
// ============================================================================

export interface PolicyForm {
  daily_limit: string;
  hourly_limit: string;
  monthly_limit: string;
  per_transaction_limit: string;
  allowed_tokens: string;
  address_mode: 'whitelist' | 'blacklist' | 'none';
  addresses: string;
  transaction_types: string;
  allowed_hours_start: string;
  allowed_hours_end: string;
  allowed_days: string;
  max_per_hour: string;
  webhook_url: string;
  /** Additional authorized API key hashes (one per line, hex SHA256) */
  additional_key_hashes: string;
  // ── Capabilities (all default-DENY unless enabled; see PolicyFormFields warnings) ──
  /** raw_sign: sign arbitrary bytes on any enabled chain — bypasses the structured policy. */
  raw_sign_enabled: boolean;
  /** Comma-separated chain allowlist for raw_sign; empty = ALL chains (incl. near). */
  raw_sign_chains: string;
  raw_sign_requires_approval: boolean;
  /** confidential: confidential-intents flows. */
  confidential_enabled: boolean;
  confidential_requires_approval: boolean;
  /** payment_check: claimable-link escrow — funds reach an arbitrary holder (whitelist-bypass). */
  payment_check_enabled: boolean;
  payment_check_requires_approval: boolean;
  /** swap: 1Click swap — Trusted (coordinator-supplied quote/route, unbound to policy). */
  swap_enabled: boolean;
  swap_requires_approval: boolean;
  /** sign_message: comma-separated NEP-413 recipient allowlist (default-DENY; never fund-moving). */
  sign_message_allowed_recipients: string;
}

export const DEFAULT_POLICY: PolicyForm = {
  daily_limit: '',
  hourly_limit: '',
  monthly_limit: '',
  per_transaction_limit: '',
  allowed_tokens: '*',
  address_mode: 'none',
  addresses: '',
  // cross_chain_withdraw is intentionally NOT a default — it is the riskiest exit and
  // must be opted in explicitly. intents_deposit folds into `call` (no separate type).
  transaction_types: 'transfer,call,delete,intents_withdraw,intents_swap',
  allowed_hours_start: '',
  allowed_hours_end: '',
  allowed_days: '',
  max_per_hour: '',
  webhook_url: '',
  additional_key_hashes: '',
  raw_sign_enabled: false,
  raw_sign_chains: '',
  raw_sign_requires_approval: false,
  confidential_enabled: false,
  confidential_requires_approval: false,
  payment_check_enabled: false,
  payment_check_requires_approval: false,
  swap_enabled: false,
  swap_requires_approval: false,
  sign_message_allowed_recipients: '',
};

// ============================================================================
// NEAR ↔ yoctoNEAR conversion helpers
// ============================================================================

/** Convert human-readable NEAR (e.g. "10.5") to yoctoNEAR string */
export function nearToYocto(near: string): string {
  if (!near || near === '0') return '0';
  const parts = near.split('.');
  const whole = parts[0] || '0';
  const frac = (parts[1] || '').padEnd(24, '0').slice(0, 24);
  const raw = whole + frac;
  return raw.replace(/^0+/, '') || '0';
}

/** Convert yoctoNEAR string to human-readable NEAR (e.g. "10.5") */
export function yoctoToNear(yocto: string): string {
  if (!yocto || yocto === '0') return '';
  const padded = yocto.padStart(25, '0');
  const whole = padded.slice(0, -24).replace(/^0+/, '') || '0';
  const frac = padded.slice(-24).replace(/0+$/, '');
  return frac ? `${whole}.${frac}` : whole;
}

// ============================================================================
// Build policy rules from form state (everything except approval)
// ============================================================================

export function buildPolicyRules(
  form: PolicyForm,
  apiKeyHash?: string,
): Record<string, unknown> {
  const rules: Record<string, unknown> = {};

  const limits: Record<string, unknown> = {};
  if (form.per_transaction_limit) limits.per_transaction = { native: nearToYocto(form.per_transaction_limit) };
  if (form.daily_limit) limits.daily = { native: nearToYocto(form.daily_limit) };
  if (form.hourly_limit) limits.hourly = { native: nearToYocto(form.hourly_limit) };
  if (form.monthly_limit) limits.monthly = { native: nearToYocto(form.monthly_limit) };
  if (Object.keys(limits).length > 0) rules.limits = limits;

  if (form.address_mode !== 'none' && form.addresses.trim()) {
    rules.addresses = {
      mode: form.address_mode,
      list: form.addresses.split(',').map((a) => a.trim()).filter(Boolean),
    };
  }

  if (form.transaction_types.trim()) {
    rules.transaction_types = form.transaction_types.split(',').map((t) => t.trim()).filter(Boolean);
  }

  if (form.allowed_tokens && form.allowed_tokens !== '*') {
    rules.allowed_tokens = form.allowed_tokens.split(',').map((t) => t.trim()).filter(Boolean);
  }

  if (form.allowed_hours_start || form.allowed_days) {
    const tr: Record<string, unknown> = { timezone: 'UTC' };
    if (form.allowed_hours_start && form.allowed_hours_end) {
      tr.allowed_hours = [parseInt(form.allowed_hours_start, 10), parseInt(form.allowed_hours_end, 10)];
    }
    if (form.allowed_days) {
      tr.allowed_days = form.allowed_days.split(',').map((d) => parseInt(d.trim(), 10)).filter((d) => !isNaN(d));
    }
    rules.time_restrictions = tr;
  }

  if (form.max_per_hour) {
    rules.rate_limit = { max_per_hour: parseInt(form.max_per_hour, 10) };
  }

  // Capabilities — each non-Built primitive is default-DENY; emit a capability only when
  // the owner opts in (absence = denied by the keystore). sign_message defaults on but its
  // recipient allowlist is default-DENY, so emit it only when recipients are listed.
  const capabilities: Record<string, unknown> = {};
  if (form.raw_sign_enabled) {
    const rs: Record<string, unknown> = { allowed: true, requires_approval: form.raw_sign_requires_approval };
    const chains = form.raw_sign_chains.split(',').map((c) => c.trim()).filter(Boolean);
    if (chains.length > 0) rs.chains = chains;
    capabilities.raw_sign = rs;
  }
  // Trusted capabilities (confidential/payment_check/swap) do NOT emit requires_approval:
  // the post-approval artifact can't be bound to the approved op, so the keystore rejects
  // Trusted+multisig — setting it would permanently brick the op. Only raw_sign (HashPinned)
  // supports requires_approval.
  if (form.confidential_enabled) {
    capabilities.confidential = { allowed: true };
  }
  if (form.payment_check_enabled) {
    capabilities.payment_check = { allowed: true };
  }
  if (form.swap_enabled) {
    capabilities.swap = { allowed: true };
  }
  const smRecipients = form.sign_message_allowed_recipients.split(',').map((r) => r.trim()).filter(Boolean);
  if (smRecipients.length > 0) {
    capabilities.sign_message = { allowed: true, allowed_recipients: smRecipients };
  }

  const policy: Record<string, unknown> = {};
  if (Object.keys(rules).length > 0) policy.rules = rules;
  if (Object.keys(capabilities).length > 0) policy.capabilities = capabilities;
  if (form.webhook_url) policy.webhook_url = form.webhook_url;

  // Merge current API key hash + any additional hashes from form
  const keyHashes: string[] = [];
  if (apiKeyHash) keyHashes.push(apiKeyHash);
  if (form.additional_key_hashes.trim()) {
    form.additional_key_hashes.split('\n').map((h) => h.trim()).filter(Boolean).forEach((h) => {
      if (!keyHashes.includes(h)) keyHashes.push(h);
    });
  }
  if (keyHashes.length > 0) policy.authorized_key_hashes = keyHashes;

  return policy;
}

// ============================================================================
// Parse policy API response back to form fields
// ============================================================================

/* eslint-disable @typescript-eslint/no-explicit-any */

export interface ParsedPolicy {
  form: PolicyForm;
  /** Approval section (page-specific, returned as-is for the caller to handle) */
  approval: {
    above_usd: string;
    required: string;
    approvers: string; // "account_id, role" lines
  } | null;
  /** The full policy JSON for the editor */
  fullJson: Record<string, unknown>;
}

/**
 * Parse the coordinator's GET /wallet/v1/policy response into form fields.
 * `currentApiKeyHash` is excluded from additional_key_hashes (it's auto-included).
 */
export function parsePolicyResponse(
  data: { rules?: any; approval?: any; capabilities?: any; authorized_key_hashes?: string[]; webhook_url?: string },
  currentApiKeyHash?: string,
): ParsedPolicy {
  const rules = data.rules || {};
  const limits = rules.limits || {};
  const addr = rules.addresses || {};
  const tr = rules.time_restrictions || {};
  const caps = data.capabilities || {};

  const form: PolicyForm = {
    per_transaction_limit: yoctoToNear(limits.per_transaction?.native || limits.per_transaction?.['*'] || ''),
    daily_limit: yoctoToNear(limits.daily?.native || limits.daily?.['*'] || ''),
    hourly_limit: yoctoToNear(limits.hourly?.native || limits.hourly?.['*'] || ''),
    monthly_limit: yoctoToNear(limits.monthly?.native || limits.monthly?.['*'] || ''),
    address_mode: addr.mode || 'none',
    addresses: (addr.list || []).join(', '),
    transaction_types: (rules.transaction_types || []).join(','),
    allowed_tokens: (rules.allowed_tokens || []).join(',') || '*',
    allowed_hours_start: tr.allowed_hours?.[0]?.toString() || '',
    allowed_hours_end: tr.allowed_hours?.[1]?.toString() || '',
    allowed_days: (tr.allowed_days || []).join(','),
    max_per_hour: rules.rate_limit?.max_per_hour?.toString() || '',
    webhook_url: data.webhook_url || '',
    additional_key_hashes: (data.authorized_key_hashes || [])
      .filter((h) => h !== currentApiKeyHash)
      .join('\n'),
    raw_sign_enabled: caps.raw_sign?.allowed === true,
    raw_sign_chains: (caps.raw_sign?.chains || []).join(', '),
    raw_sign_requires_approval: caps.raw_sign?.requires_approval === true,
    confidential_enabled: caps.confidential?.allowed === true,
    confidential_requires_approval: caps.confidential?.requires_approval === true,
    payment_check_enabled: caps.payment_check?.allowed === true,
    payment_check_requires_approval: caps.payment_check?.requires_approval === true,
    swap_enabled: caps.swap?.allowed === true,
    swap_requires_approval: caps.swap?.requires_approval === true,
    sign_message_allowed_recipients: (caps.sign_message?.allowed_recipients || []).join(', '),
  };

  let approval: ParsedPolicy['approval'] = null;
  if (data.approval) {
    const ap = data.approval;
    const approverLines = (ap.approvers || [])
      .map((a: any) => `${a.id}, ${a.role || 'signer'}`)
      .join('\n');
    approval = {
      above_usd: ap.above_usd?.toString() || '0',
      required: ap.threshold?.required?.toString() || '1',
      approvers: approverLines,
    };
  }

  // Reconstruct the full JSON (rules + approval + key hashes + webhook)
  const fullJson: Record<string, unknown> = {};
  if (data.rules) fullJson.rules = data.rules;
  if (data.approval) fullJson.approval = data.approval;
  if (data.capabilities) fullJson.capabilities = data.capabilities;
  if (data.webhook_url) fullJson.webhook_url = data.webhook_url;
  if (data.authorized_key_hashes?.length) fullJson.authorized_key_hashes = data.authorized_key_hashes;

  return { form, approval, fullJson };
}

/* eslint-enable @typescript-eslint/no-explicit-any */

// ============================================================================
// Submit policy: encrypt → sign → store on-chain → invalidate cache
// ============================================================================

export interface SubmitPolicyParams {
  coordinatorUrl: string;
  apiKey: string;
  walletId: string;
  policyJsonText: string;
  contractId: string;
  viewMethod: (params: { contractId: string; method: string; args?: Record<string, unknown> }) => Promise<unknown>;
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  signAndSendTransaction: (params: any) => Promise<any>;
}

export interface SubmitPolicyResult {
  walletPubkey: string;
}

export async function submitPolicy(params: SubmitPolicyParams): Promise<SubmitPolicyResult> {
  const { coordinatorUrl, apiKey, walletId, policyJsonText, contractId, viewMethod, signAndSendTransaction } = params;

  // Parse policy JSON
  let policyData: Record<string, unknown>;
  try {
    policyData = JSON.parse(policyJsonText);
  } catch {
    throw new Error('Invalid JSON in policy editor');
  }

  // Step 1: Encrypt policy via coordinator
  const encryptResp = await fetch(`${coordinatorUrl}/wallet/v1/encrypt-policy`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json', 'Authorization': `Bearer ${apiKey}` },
    body: JSON.stringify({ wallet_id: walletId, ...policyData }),
  });

  if (!encryptResp.ok) {
    const errorData = await encryptResp.json().catch(() => ({}));
    throw new Error(errorData.message || `Policy encryption failed (HTTP ${encryptResp.status})`);
  }

  const encrypted = await encryptResp.json();

  // Step 2: Sign encrypted policy with agent's ed25519 key
  const signResp = await fetch(`${coordinatorUrl}/wallet/v1/sign-policy`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json', 'Authorization': `Bearer ${apiKey}` },
    body: JSON.stringify({ encrypted_data: encrypted.encrypted_base64 }),
  });

  if (!signResp.ok) {
    const signErr = await signResp.json().catch(() => ({}));
    throw new Error(signErr.message || `Policy signing failed (HTTP ${signResp.status})`);
  }

  const signed = await signResp.json();
  const walletPubkey = `ed25519:${signed.public_key_hex}`;

  // Step 3: Estimate storage cost + store on-chain
  let storageCost = '100000000000000000000000'; // default 0.1 NEAR
  try {
    const cost = await viewMethod({
      contractId,
      method: 'estimate_wallet_policy_cost',
      args: { wallet_pubkey: walletPubkey, encrypted_data: encrypted.encrypted_base64 },
    });
    if (cost) storageCost = cost as string;
  } catch (e) {
    console.warn('estimate_wallet_policy_cost failed, using default:', e);
  }

  const action = actionCreators.functionCall(
    'store_wallet_policy',
    {
      wallet_pubkey: walletPubkey,
      encrypted_data: encrypted.encrypted_base64,
      wallet_signature: signed.signature_hex,
    },
    BigInt('100000000000000'),
    BigInt(storageCost),
  );

  await signAndSendTransaction({ receiverId: contractId, actions: [action] });

  // Step 4: Invalidate coordinator cache + save plaintext policy to DB
  // (so dashboard can read it immediately without waiting for worker sync)
  await fetch(`${coordinatorUrl}/wallet/v1/invalidate-cache`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ wallet_id: walletId, policy_json: policyData }),
  }).catch(() => {});

  return { walletPubkey };
}
