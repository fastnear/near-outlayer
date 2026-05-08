'use client';

import { useEffect, useState } from 'react';

import { listVaults, type VaultListEntry } from '@/lib/vault';
import type { NetworkType } from '@/lib/api';

/**
 * "Encryption master" picker for /secrets, /wallet, payment-key forms.
 *
 * Default state: collapsed, OutLayer master is implied. A small link
 * "Use a custom MPC vault" expands the picker. Most customers never
 * need a vault — surfacing the picker by default added noise and made
 * users think a choice was required.
 *
 * Expanded state:
 *   - if the customer has at least one vault → dropdown
 *     (Default + their vaults), plus a short note telling them where
 *     the keys come from for the current selection.
 *   - if the customer has no vaults → inline pointer to /vault to
 *     create one, plus a "cancel" link to re-collapse.
 *
 * The toggle stays expanded once a non-null `value` is selected, so a
 * user who opted into a vault sees the picker on every render and can
 * unselect or switch.
 *
 * Caller contract:
 *   * `network` + `owner` drive the GET /customer/list-vaults call.
 *   * `value` is the currently-selected vault id (or `null` for the
 *     OutLayer master). Caller persists this as part of its form
 *     state.
 *   * `onChange(value)` fires every time the user picks a different
 *     scope.
 *   * `disabled` mirrors the parent form's busy state.
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
  // Expanded if the user explicitly opened the picker OR if a vault is
  // already selected (so they can see / change their choice).
  const [expanded, setExpanded] = useState<boolean>(value !== null);

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

  // Auto-expand if a vault selection arrives from the parent form
  // (e.g. /secrets in update mode reads vault binding from chain).
  useEffect(() => {
    if (value !== null) setExpanded(true);
  }, [value]);

  if (!expanded) {
    return (
      <div className="mb-3">
        <button
          type="button"
          onClick={() => setExpanded(true)}
          disabled={disabled}
          className="text-xs text-gray-600 underline hover:text-gray-900 disabled:text-gray-400"
        >
          Use a custom MPC vault?
        </button>
        <span className="text-xs text-gray-500 ml-2">
          (default: OutLayer master key)
        </span>
      </div>
    );
  }

  // Expanded — but the customer has no vaults yet.
  if (vaults !== null && vaults.length === 0 && !error && owner) {
    return (
      <div className="mb-3 border border-gray-200 rounded p-3 bg-gray-50">
        <div className="text-sm text-gray-800 mb-1">
          {label ?? 'Encryption master'}
        </div>
        <p className="text-xs text-gray-700 mb-2">
          No MPC vaults registered for <code>{owner}</code>. An MPC vault
          binds an on-chain CKD-issuer contract to your account so the
          per-customer master is reproducible by you directly via NEAR
          MPC.
        </p>
        <a
          href="/vault"
          className="text-sm text-blue-600 hover:underline mr-3"
        >
          Create an MPC vault →
        </a>
        <button
          type="button"
          onClick={() => setExpanded(false)}
          className="text-xs text-gray-500 hover:underline"
        >
          cancel, use OutLayer master
        </button>
      </div>
    );
  }

  return (
    <div className="mb-3">
      <label className="block text-sm font-medium mb-1">
        {label ?? 'Encryption master'}
      </label>
      <select
        value={value ?? ''}
        onChange={(e) => onChange(e.target.value || null)}
        disabled={disabled || vaults === null}
        className="w-full rounded border border-gray-300 bg-white text-gray-900 px-3 py-2 text-sm"
      >
        <option value="">OutLayer master key (default)</option>
        {(vaults ?? []).map((v) => (
          <option key={v.vault_id} value={v.vault_id}>
            {v.vault_id}
            {/* Pubkey suffix as a per-vault identifier — full pubkey is
                too long for a dropdown row. */}
            {v.near_pubkey ? ` — ${v.near_pubkey.replace(/^ed25519:/, '').slice(0, 12)}…` : ''}
          </option>
        ))}
      </select>
      {value && (
        <p className="text-xs text-gray-600 mt-1">
          Keys derive from your MPC vault <code>{value}</code>. You can later
          take this vault over yourself (own TEE / runtime) instead of going
          through OutLayer &mdash; same MPC derivation, same keys.
        </p>
      )}
      {!value && (
        <p className="text-xs text-gray-600 mt-1">
          OutLayer master: shared keystore key, no on-chain footprint, zero
          setup. Choose an MPC vault above to bind keys to a contract you
          control.{' '}
          <button
            type="button"
            onClick={() => setExpanded(false)}
            className="underline hover:text-gray-900"
          >
            hide this
          </button>
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
