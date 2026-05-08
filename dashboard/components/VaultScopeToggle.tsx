'use client';

import { useEffect, useState } from 'react';

import { listVaults, type VaultListEntry } from '@/lib/vault';
import type { NetworkType } from '@/lib/api';

/**
 * Reusable "Use my vault" toggle for wallet / secrets / payment-key
 * forms.
 *
 * Phase 7 audit F2: until this shipped, dashboard pages always
 * passed `vault_id: null` and customers could only opt INTO vault
 * scope through the CLI. With this control, the customer picks
 * "Default OutLayer master" or one of their verified vaults from a
 * dropdown; the form passes the chosen `vault_id` (or `null`)
 * through to its existing API call.
 *
 * Caller contract:
 *   * `network` + `owner` drive the GET /customer/list-vaults call.
 *   * `value` is the currently-selected vault id (or `null` for
 *     default master). Caller persists this as part of its form
 *     state.
 *   * `onChange(value)` fires every time the user picks a different
 *     scope.
 *   * `disabled` mirrors the parent form's busy state.
 *
 * The component fetches the vault list once on mount + whenever
 * `owner`/`network` changes. Empty list ⇒ only the "Default
 * OutLayer master" option is shown — keeps the UI honest about
 * whether the customer has any vaults at all.
 */
export function VaultScopeToggle({
  network,
  owner,
  value,
  onChange,
  disabled,
  label,
}: {
  network: NetworkType;
  owner: string | null;
  value: string | null;
  onChange: (vaultId: string | null) => void;
  disabled?: boolean;
  label?: string;
}) {
  const [vaults, setVaults] = useState<VaultListEntry[] | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!owner) {
      setVaults([]);
      return;
    }
    let cancelled = false;
    setError(null);
    listVaults(network, owner)
      .then((v) => {
        if (!cancelled) setVaults(v);
      })
      .catch((e) => {
        if (!cancelled) {
          setVaults([]);
          setError((e as Error).message);
        }
      });
    return () => {
      cancelled = true;
    };
  }, [network, owner]);

  return (
    <div className="mb-3">
      <label className="block text-sm font-medium mb-1">
        {label ?? 'Master key'}
      </label>
      <select
        value={value ?? ''}
        onChange={(e) => onChange(e.target.value || null)}
        disabled={disabled || vaults === null}
        className="w-full rounded border border-gray-300 bg-white text-gray-900 px-3 py-2 text-sm"
      >
        <option value="">
          Default OutLayer master (no recovery)
        </option>
        {(vaults ?? []).map((v) => (
          <option key={v.vault_id} value={v.vault_id}>
            {v.vault_id}
            {/* Strip the `ed25519:` prefix for the dropdown subtitle
                — full pubkey is too long. Customers can find the
                full key on the wallet detail page. */}
            {v.near_pubkey ? ` — ${v.near_pubkey.replace(/^ed25519:/, '').slice(0, 12)}…` : ''}
          </option>
        ))}
      </select>
      {value && (
        <p className="text-xs text-gray-600 mt-1">
          Keys derived from your vault <code>{value}</code>. Recoverable through
          DAO cessation or your own unilateral exit window.
        </p>
      )}
      {!value && vaults && vaults.length > 0 && (
        <p className="text-xs text-gray-600 mt-1">
          Default master: simpler, no on-chain footprint, NOT recoverable if
          OutLayer ceases.
        </p>
      )}
      {vaults !== null && vaults.length === 0 && !error && owner && (
        <p className="text-xs text-gray-500 mt-1">
          No vaults registered for <code>{owner}</code>. Create one on the{' '}
          <a href="/vault" className="text-blue-600 hover:underline">Vaults</a> page.
        </p>
      )}
      {error && (
        <p className="text-xs text-red-600 mt-1">
          Could not load vaults: {error}
        </p>
      )}
    </div>
  );
}
