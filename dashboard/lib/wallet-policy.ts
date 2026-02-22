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
}

export const DEFAULT_POLICY: PolicyForm = {
  daily_limit: '',
  hourly_limit: '',
  monthly_limit: '',
  per_transaction_limit: '',
  allowed_tokens: '*',
  address_mode: 'none',
  addresses: '',
  transaction_types: 'withdraw,call',
  allowed_hours_start: '',
  allowed_hours_end: '',
  allowed_days: '',
  max_per_hour: '',
  webhook_url: '',
  additional_key_hashes: '',
};

// ============================================================================
// Build policy rules from form state (everything except approval)
// ============================================================================

export function buildPolicyRules(
  form: PolicyForm,
  apiKeyHash?: string,
): Record<string, unknown> {
  const rules: Record<string, unknown> = {};

  const limits: Record<string, unknown> = {};
  if (form.per_transaction_limit) limits.per_transaction = { '*': form.per_transaction_limit };
  if (form.daily_limit) limits.daily = { '*': form.daily_limit };
  if (form.hourly_limit) limits.hourly = { '*': form.hourly_limit };
  if (form.monthly_limit) limits.monthly = { '*': form.monthly_limit };
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

  const policy: Record<string, unknown> = {};
  if (Object.keys(rules).length > 0) policy.rules = rules;
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
  data: { rules?: any; approval?: any; authorized_key_hashes?: string[]; webhook_url?: string },
  currentApiKeyHash?: string,
): ParsedPolicy {
  const rules = data.rules || {};
  const limits = rules.limits || {};
  const addr = rules.addresses || {};
  const tr = rules.time_restrictions || {};

  const form: PolicyForm = {
    per_transaction_limit: limits.per_transaction?.['*'] || '',
    daily_limit: limits.daily?.['*'] || '',
    hourly_limit: limits.hourly?.['*'] || '',
    monthly_limit: limits.monthly?.['*'] || '',
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
    headers: { 'Content-Type': 'application/json' },
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
