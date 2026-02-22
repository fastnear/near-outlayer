import { useState, useEffect } from 'react';
import { computeKeyHash } from '@/lib/wallet-keys';

/** Computes SHA256 hex hash of the given key string. Returns '' while computing or if key is null. */
export function useApiKeyHash(key: string | null): string {
  const [hash, setHash] = useState('');

  useEffect(() => {
    if (!key) { setHash(''); return; }
    computeKeyHash(key).then(setHash).catch(() => setHash(''));
  }, [key]);

  return hash;
}
