'use client';

import { useEffect, useMemo, useState } from 'react';

import { isVaultVerified, listVaults, type VaultListEntry } from '@/lib/vault';
import type { NetworkType } from '@/lib/api';
import { useNearWallet } from '@/contexts/NearWalletContext';

/**
 * "Encryption master" picker for /secrets, /wallet, payment-key forms.
 *
 * Default state: collapsed, OutLayer master is implied. A small link
 * "Use a custom MPC vault" expands the picker.
 *
 * Expanded state: a TEXT INPUT for the vault id is the primary
 * control — for secret forms a customer often has a verified vault
 * with NO wallet minted under it yet (and that's the common case for
 * secret-only customers), so the coordinator's `/customer/list-vaults`
 * endpoint (which joins on `wallet_accounts`) wouldn't surface it. A
 * secondary "← Choose from registered" link expands the dropdown of
 * already-minted vaults for customers who DO have wallets.
 *
 * The typed value is validated against
 * `keystore-dao.is_vault_verified(<vault>) == true` (debounced ~400ms
 * after the last keystroke). The form only accepts a vault id that
 * the DAO has approved.
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
  const { viewMethod } = useNearWallet();

  const [vaults, setVaults] = useState<VaultListEntry[] | null>(null);
  const [listError, setListError] = useState<string | null>(null);
  const [showList, setShowList] = useState<boolean>(false);
  // Expanded if the user explicitly opened the picker OR if a vault is
  // already selected (so they can see / change their choice).
  const [expanded, setExpanded] = useState<boolean>(value !== null);
  // What the user is typing — separate from `value` because we only
  // promote `text → value` after the verified-on-chain check passes.
  const [text, setText] = useState<string>(value ?? '');
  const [verifyState, setVerifyState] = useState<
    | { kind: 'idle' }
    | { kind: 'checking' }
    | { kind: 'verified' }
    | { kind: 'not_verified'; reason: string }
  >({ kind: 'idle' });

  // ─── Load the registered list lazily ────────────────────────────────
  //
  // We fetch the list once, but only render it when the user clicks
  // "Choose from registered". Fetching upfront keeps the dropdown
  // instant to render when toggled, and the response is small.
  useEffect(() => {
    if (!owner) {
      setVaults([]);
      return;
    }
    let cancelled = false;
    setListError(null);
    listVaults(network, owner)
      .then((v) => {
        if (!cancelled) setVaults(v);
      })
      .catch((e) => {
        if (!cancelled) {
          setVaults([]);
          setListError((e as Error).message);
        }
      });
    return () => {
      cancelled = true;
    };
  }, [network, owner]);

  // ─── Auto-expand when parent supplies a non-null value ──────────────
  useEffect(() => {
    if (value !== null) setExpanded(true);
    // Keep the text box in sync with the canonical value when the
    // parent form pushes one in (e.g. /secrets in update mode reading
    // an existing on-chain binding).
    if (value !== null && value !== text) setText(value);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [value]);

  // ─── Debounced validation of the typed value ────────────────────────
  //
  // Run `is_vault_verified` 400ms after the last keystroke. We don't
  // call `onChange` until the check passes — that way the parent
  // form's `value` only ever holds DAO-verified ids. If the user
  // clears the field, we propagate `null` immediately (OutLayer
  // master).
  useEffect(() => {
    const trimmed = text.trim();
    if (trimmed === '') {
      setVerifyState({ kind: 'idle' });
      if (value !== null) onChange(null);
      return;
    }
    if (trimmed === value) {
      // Already accepted upstream — nothing to re-check.
      setVerifyState({ kind: 'verified' });
      return;
    }
    setVerifyState({ kind: 'checking' });
    let cancelled = false;
    const t = setTimeout(async () => {
      try {
        const ok = await isVaultVerified(viewMethod, network, trimmed);
        if (cancelled) return;
        if (ok) {
          setVerifyState({ kind: 'verified' });
          onChange(trimmed);
        } else {
          setVerifyState({
            kind: 'not_verified',
            reason: `keystore-dao.is_vault_verified('${trimmed}') === false. Either the account isn't a vault, or it hasn't been approved by the DAO yet.`,
          });
          if (value !== null) onChange(null);
        }
      } catch (e) {
        if (cancelled) return;
        setVerifyState({
          kind: 'not_verified',
          reason: `view-call failed: ${(e as Error).message}`,
        });
        if (value !== null) onChange(null);
      }
    }, 400);
    return () => {
      cancelled = true;
      clearTimeout(t);
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [text, network]);

  const verifyIcon = useMemo(() => {
    switch (verifyState.kind) {
      case 'checking':
        return <span className="text-xs text-gray-500">checking…</span>;
      case 'verified':
        return <span className="text-xs text-green-700">✓ verified on DAO</span>;
      case 'not_verified':
        return (
          <span className="text-xs text-red-600" title={verifyState.reason}>
            ✗ not verified
          </span>
        );
      default:
        return null;
    }
  }, [verifyState]);

  // ─── Collapsed: small inline link ───────────────────────────────────
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

  // ─── Expanded: primary text input + secondary list link ─────────────
  return (
    <div className="mb-3">
      <label className="block text-sm font-medium mb-1">
        {label ?? 'Encryption master'}
      </label>
      <div className="flex gap-2 items-center">
        <input
          type="text"
          value={text}
          onChange={(e) => setText(e.target.value)}
          disabled={disabled}
          placeholder="vault.alice.testnet (any DAO-verified vault)"
          className="flex-1 rounded border border-gray-300 bg-white text-gray-900 px-3 py-2 text-sm font-mono"
        />
        <div className="min-w-[8rem] text-right">{verifyIcon}</div>
      </div>

      {verifyState.kind === 'not_verified' && (
        <p className="text-xs text-red-600 mt-1">{verifyState.reason}</p>
      )}

      {/* Secondary path: dropdown for customers who already have wallets. */}
      {!showList && vaults !== null && vaults.length > 0 && (
        <button
          type="button"
          onClick={() => setShowList(true)}
          className="text-xs text-blue-600 underline hover:text-blue-800 mt-2"
        >
          ← Choose from registered ({vaults.length})
        </button>
      )}

      {showList && (
        <div className="mt-2 border border-gray-200 rounded p-2 bg-gray-50">
          <div className="flex items-center justify-between mb-1">
            <span className="text-xs text-gray-700">
              Vaults with at least one minted wallet (from coordinator):
            </span>
            <button
              type="button"
              onClick={() => setShowList(false)}
              className="text-xs text-gray-500 underline hover:text-gray-700"
            >
              hide list
            </button>
          </div>
          <select
            value={(vaults ?? []).some((v) => v.vault_id === text) ? text : ''}
            onChange={(e) => {
              if (e.target.value) setText(e.target.value);
            }}
            disabled={disabled}
            className="w-full rounded border border-gray-300 bg-white text-gray-900 px-2 py-1 text-sm font-mono"
          >
            <option value="">— pick one —</option>
            {(vaults ?? []).map((v) => (
              <option key={v.vault_id} value={v.vault_id}>
                {v.vault_id}
                {' '}
                ({v.wallet_count} {v.wallet_count === 1 ? 'wallet' : 'wallets'})
              </option>
            ))}
          </select>
          {listError && (
            <p className="text-xs text-red-600 mt-1">
              Could not load vaults: {listError}
            </p>
          )}
        </div>
      )}

      <p className="text-xs text-gray-600 mt-2">
        {verifyState.kind === 'verified' ? (
          <>
            Keys derive from your MPC vault <code>{value}</code>. You can
            later take this vault over yourself (own TEE / runtime) instead
            of going through OutLayer &mdash; same MPC derivation, same
            keys.
          </>
        ) : (
          <>
            Type any DAO-verified vault id (sub-account of <code>{owner ?? '<your account>'}</code>).
            A vault appears in the list above only after you mint at least
            one wallet under it via <code>/wallet</code>, so secret-only
            customers usually need to type it in here directly.{' '}
            <a href="/vault" className="underline text-blue-600 hover:text-blue-800">
              Create a vault →
            </a>
            <button
              type="button"
              onClick={() => {
                setExpanded(false);
                setText('');
              }}
              className="underline hover:text-gray-900 ml-2"
            >
              cancel
            </button>
          </>
        )}
      </p>
    </div>
  );
}
