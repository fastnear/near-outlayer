// Payment Key types

export interface PaymentKeyData {
  // From contract secret (profile is the nonce)
  nonce: number;
  created_at: number;
  updated_at: number;
  storage_deposit: string;
  // Parsed from encrypted_secrets after creation
  // These are only available if the key was just created in this session
  project_ids: string[]; // empty = any project allowed
  max_per_call: string; // max USD per single call (in minimal units)
  initial_balance: string; // initial balance (in minimal units)
  // From coordinator
  spent?: string;
  reserved?: string;
}

export interface PaymentKeyBalance {
  owner: string;
  nonce: number;
  initial_balance: string;
  spent: string;
  reserved: string;
  available: string;
  last_used_at: string | null;
}

export interface PaymentKeyUsage {
  id: string;
  call_id: string;
  job_id: number | null;
  project_id: string;
  compute_cost: string;
  attached_deposit: string;
  status: 'pending' | 'completed' | 'failed';
  created_at: string;
}

export interface CreatePaymentKeyForm {
  projectIds: string[]; // empty = any project
  maxPerCall: string; // USD amount (e.g., "10.00")
  initialDeposit: string; // USD amount (e.g., "100.00")
}

// Payment Key secret format (stored encrypted)
export interface PaymentKeySecret {
  key: string; // 32-byte random key as hex (64 chars)
  project_ids: string[];
  max_per_call: string; // in minimal units
  initial_balance: string; // in minimal units
}

// Creation flow steps
export type CreationStep =
  | 'form' // User filling form
  | 'generating' // Generating random key
  | 'storing' // Transaction 1: store_secrets
  | 'topping_up' // Transaction 2: ft_transfer_call
  | 'complete' // Both transactions done
  | 'error'; // Error occurred

export interface CreationState {
  step: CreationStep;
  error?: string;
  // Temporary storage during creation
  generatedKey?: string;
  nonce?: number;
}

// Format USD amount for display
export function formatUsd(minimalUnits: string, decimals: number): string {
  const value = BigInt(minimalUnits);
  const divisor = BigInt(10 ** decimals);
  const whole = value / divisor;
  const fraction = value % divisor;
  const fractionStr = fraction.toString().padStart(decimals, '0');
  // Show 6 decimal places for precision (minimal unit is 0.000001 USD)
  return `$${whole}.${fractionStr.slice(0, 6)}`;
}

// Parse USD input to minimal units
export function parseUsdToMinimalUnits(usd: string, decimals: number): string {
  const [whole, frac = ''] = usd.replace('$', '').split('.');
  const wholeNum = BigInt(whole || '0');
  const fracPadded = frac.padEnd(decimals, '0').slice(0, decimals);
  const fracNum = BigInt(fracPadded);
  const result = wholeNum * BigInt(10 ** decimals) + fracNum;
  return result.toString();
}
