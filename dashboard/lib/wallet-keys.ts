/**
 * Browser-local wallet API key storage.
 *
 * Keys are stored ONLY in the browser's localStorage.
 * The server never stores plaintext API keys — only SHA256 hashes.
 * Users should back up their keys independently.
 */

const STORAGE_KEY = 'outlayer_wallet_keys';

interface StoredKey {
  apiKey: string;
  savedAt: string;
  label?: string;
}

type KeyStore = Record<string, StoredKey>; // walletPubkey → StoredKey

function load(): KeyStore {
  if (typeof window === 'undefined') return {};
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    return raw ? JSON.parse(raw) : {};
  } catch {
    return {};
  }
}

function save(store: KeyStore) {
  if (typeof window === 'undefined') return;
  localStorage.setItem(STORAGE_KEY, JSON.stringify(store));
}

/** Save an API key for a wallet pubkey (e.g. "ed25519:abc...") */
export function saveWalletKey(walletPubkey: string, apiKey: string, label?: string) {
  const store = load();
  store[walletPubkey] = { apiKey, savedAt: new Date().toISOString(), label };
  save(store);
}

/** Get saved API key for a wallet pubkey. Returns null if not found. */
export function getWalletKey(walletPubkey: string): string | null {
  const store = load();
  return store[walletPubkey]?.apiKey ?? null;
}

/** Get all saved wallet keys */
export function getAllWalletKeys(): Record<string, StoredKey> {
  return load();
}

/** Remove a saved key */
export function removeWalletKey(walletPubkey: string) {
  const store = load();
  delete store[walletPubkey];
  save(store);
}

/** Find API key by matching any of the given wallet pubkeys */
export function findKeyForWallets(walletPubkeys: string[]): string | null {
  const store = load();
  for (const pk of walletPubkeys) {
    if (store[pk]?.apiKey) return store[pk].apiKey;
  }
  return null;
}

/** Compute SHA256 hex hash of an API key string */
export async function computeKeyHash(key: string): Promise<string> {
  const buf = await crypto.subtle.digest('SHA-256', new TextEncoder().encode(key));
  return Array.from(new Uint8Array(buf))
    .map((b) => b.toString(16).padStart(2, '0'))
    .join('');
}

/** Validate wallet API key format. Returns error message or null if valid. */
export function validateWalletKeyFormat(key: string): string | null {
  if (!key.startsWith('wk_')) return 'Key must start with "wk_"';
  if (key.length !== 67) return `Key must be 67 characters (wk_ + 64 hex), got ${key.length}`;
  if (!/^wk_[0-9a-f]{64}$/.test(key)) return 'Key must be wk_ followed by 64 lowercase hex characters';
  return null;
}

/** Generate a random wallet API key */
export function generateWalletKey(): string {
  const bytes = new Uint8Array(32);
  crypto.getRandomValues(bytes);
  return 'wk_' + Array.from(bytes).map((b) => b.toString(16).padStart(2, '0')).join('');
}
