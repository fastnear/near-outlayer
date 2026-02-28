'use client';

import { useState, useEffect, useCallback, Suspense } from 'react';
import { useSearchParams } from 'next/navigation';
import { useNearWallet } from '@/contexts/NearWalletContext';
import WalletConnectionModal from '@/components/WalletConnectionModal';
import { getCoordinatorApiUrl } from '@/lib/api';
import Link from 'next/link';
import { actionCreators } from '@near-js/transactions';
import { saveWalletKey, getAllWalletKeys, removeWalletKey } from '@/lib/wallet-keys';

interface WalletPolicy {
  wallet_pubkey: string;
  owner: string;
  frozen: boolean;
  updated_at: number;
}

export default function WalletManagePageWrapper() {
  return (
    <Suspense>
      <WalletManagePage />
    </Suspense>
  );
}

function WalletManagePage() {
  const {
    accountId,
    isConnected,
    network,
    contractId,
    viewMethod,
    signAndSendTransaction,
  } = useNearWallet();
  const coordinatorUrl = getCoordinatorApiUrl(network);
  const searchParams = useSearchParams();

  const [showWalletModal, setShowWalletModal] = useState(false);
  const [wallets, setWallets] = useState<WalletPolicy[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [success, setSuccess] = useState<string | null>(null);
  const [submitting, setSubmitting] = useState(false);

  // API key wallet (from ?key=wk_... query param)
  const [apiKeyWallet, setApiKeyWallet] = useState<{ wallet_id: string; address: string } | null>(null);

  // Saved API keys from localStorage
  const [savedKeys, setSavedKeys] = useState<Record<string, string>>({});
  const [showKeyInput, setShowKeyInput] = useState<string | null>(null);
  const [keyInput, setKeyInput] = useState('');
  const [revealedKeys, setRevealedKeys] = useState<Set<string>>(new Set());

  // Load saved keys on mount
  useEffect(() => {
    const all = getAllWalletKeys();
    const map: Record<string, string> = {};
    for (const [pk, entry] of Object.entries(all)) {
      map[pk] = entry.apiKey;
    }
    setSavedKeys(map);
  }, []);

  // Also save key from URL param if we know the wallet pubkey
  useEffect(() => {
    const apiKey = searchParams.get('key');
    if (apiKey && apiKeyWallet) {
      const pk = `ed25519:${apiKeyWallet.address}`;
      saveWalletKey(pk, apiKey);
      setSavedKeys((prev) => ({ ...prev, [pk]: apiKey }));
    }
  }, [apiKeyWallet, searchParams]);

  // Resolve API key from query param → wallet_id
  useEffect(() => {
    const apiKey = searchParams.get('key');
    if (!apiKey) return;

    (async () => {
      try {
        const resp = await fetch(`${coordinatorUrl}/wallet/v1/address?chain=near`, {
          headers: { 'Authorization': `Bearer ${apiKey}` },
        });
        if (!resp.ok) {
          setError(`Invalid API key: HTTP ${resp.status}`);
          return;
        }
        const data = await resp.json();
        setApiKeyWallet({ wallet_id: data.wallet_id, address: data.address });
      } catch (err) {
        setError(`Failed to resolve API key: ${(err as Error).message}`);
      }
    })();
  }, [searchParams, coordinatorUrl]);

  // Load wallet policies owned by this account
  const loadWallets = useCallback(async () => {
    if (!accountId) return;
    setLoading(true);
    setError(null);

    try {
      const result = await viewMethod({
        contractId,
        method: 'get_wallet_policies_by_owner',
        args: { owner: accountId },
      }).catch(() => []);

      setWallets((result as WalletPolicy[]) || []);
    } catch (err) {
      setError((err as Error).message);
    } finally {
      setLoading(false);
    }
  }, [accountId, contractId, viewMethod]);

  useEffect(() => {
    if (isConnected && accountId) {
      loadWallets();
    }
  }, [isConnected, accountId, loadWallets]);

  const handleFreeze = async (walletPubkey: string) => {
    if (!accountId) return;
    setError(null);
    setSubmitting(true);

    try {
      const action = actionCreators.functionCall(
        'freeze_wallet',
        { wallet_pubkey: walletPubkey },
        BigInt('30000000000000'),
        BigInt('0')
      );

      await signAndSendTransaction({
        receiverId: contractId,
        actions: [action],
      });

      setSuccess(`Wallet ${walletPubkey.substring(0, 20)}... frozen`);
      setTimeout(() => {
        setSuccess(null);
        loadWallets();
      }, 2000);
    } catch (err) {
      setError((err as Error).message);
    } finally {
      setSubmitting(false);
    }
  };

  const handleUnfreeze = async (walletPubkey: string) => {
    if (!accountId) return;
    setError(null);
    setSubmitting(true);

    try {
      const action = actionCreators.functionCall(
        'unfreeze_wallet',
        { wallet_pubkey: walletPubkey },
        BigInt('30000000000000'),
        BigInt('0')
      );

      await signAndSendTransaction({
        receiverId: contractId,
        actions: [action],
      });

      setSuccess(`Wallet ${walletPubkey.substring(0, 20)}... unfrozen`);
      setTimeout(() => {
        setSuccess(null);
        loadWallets();
      }, 2000);
    } catch (err) {
      setError((err as Error).message);
    } finally {
      setSubmitting(false);
    }
  };

  /** Get the API key for a wallet — from saved keys or URL param */
  const getWalletApiKey = (walletPubkey: string): string | null => {
    return savedKeys[walletPubkey] || searchParams.get('key') || null;
  };

  const formatTimestamp = (nanos: number) => {
    return new Date(nanos / 1_000_000).toLocaleString();
  };

  if (!isConnected) {
    return (
      <div className="max-w-4xl mx-auto">
        <h1 className="text-3xl font-bold text-gray-900 mb-6">Manage Wallets</h1>
        <div className="bg-white shadow rounded-lg p-8 text-center">
          <p className="text-gray-600 mb-4">Connect your NEAR wallet to manage wallet policies.</p>
          <button
            onClick={() => setShowWalletModal(true)}
            className="px-6 py-3 bg-gradient-to-r from-[#cc6600] to-[#d4a017] text-white rounded-lg font-medium hover:from-[#b35900] hover:to-[#c49016]"
          >
            Connect Wallet
          </button>
        </div>
        <WalletConnectionModal isOpen={showWalletModal} onClose={() => setShowWalletModal(false)} />
      </div>
    );
  }

  return (
    <div className="max-w-4xl mx-auto">
      <div className="flex items-center justify-between mb-6">
        <h1 className="text-3xl font-bold text-gray-900">Manage Wallets</h1>
        <div className="flex items-center space-x-3">
          <Link
            href="/wallet/approvals"
            className="text-sm text-[#cc6600] hover:text-[#b35900] font-medium"
          >
            Approvals
          </Link>
          <Link
            href="/wallet/audit"
            className="text-sm text-[#cc6600] hover:text-[#b35900] font-medium"
          >
            Audit Log
          </Link>
        </div>
      </div>

      {error && (
        <div className="mb-4 bg-red-50 border border-red-200 rounded-md p-3">
          <p className="text-sm text-red-800">{error}</p>
        </div>
      )}

      {success && (
        <div className="mb-4 bg-green-50 border border-green-200 rounded-md p-3">
          <p className="text-sm text-green-800">{success}</p>
        </div>
      )}

      {/* API key wallet (from ?key= param) — new wallet without policy yet */}
      {apiKeyWallet && !wallets.some((w) => w.wallet_pubkey === `ed25519:${apiKeyWallet.address}`) && (
        <div className="mb-4 bg-white shadow rounded-lg border-2 border-dashed border-[#cc6600]">
          <div className="px-4 py-4 sm:px-6">
            <div className="flex items-center justify-between">
              <div>
                <div className="flex items-center space-x-2">
                  <span className="text-sm font-medium text-gray-900">New Wallet</span>
                  <span className="inline-flex items-center px-2.5 py-0.5 rounded-full text-xs font-medium bg-yellow-100 text-yellow-800">
                    No Policy
                  </span>
                </div>
                <p className="mt-1 text-xs text-gray-500 font-mono break-all">
                  ed25519:{apiKeyWallet.address}
                </p>
                <p className="text-xs text-gray-400 mt-1">
                  NEAR address: {apiKeyWallet.address}
                </p>
              </div>
              <Link
                href={`/wallet?key=${searchParams.get('key')}`}
                className="px-3 py-1.5 text-sm bg-[#cc6600] text-white rounded hover:bg-[#b35900]"
              >
                Set Policy
              </Link>
            </div>
          </div>
        </div>
      )}

      {loading ? (
        <div className="flex items-center justify-center py-12">
          <svg className="animate-spin h-8 w-8 text-[#cc6600]" fill="none" viewBox="0 0 24 24">
            <circle className="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" strokeWidth="4" />
            <path className="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4z" />
          </svg>
          <span className="ml-3 text-gray-500">Loading wallets...</span>
        </div>
      ) : wallets.length === 0 && !apiKeyWallet ? (
        <div className="bg-white shadow rounded-lg p-8 text-center">
          <p className="text-gray-500">No wallet policies found for your account.</p>
          <p className="text-sm text-gray-400 mt-2">
            Wallet policies are created when an AI agent registers a wallet with your account as controller.
          </p>
        </div>
      ) : (
        <div className="space-y-4">
          {wallets.map((wallet) => {
            const walletKey = getWalletApiKey(wallet.wallet_pubkey);
            return (
            <div
              key={wallet.wallet_pubkey}
              className={`bg-white shadow rounded-lg border ${
                wallet.frozen ? 'border-blue-300 bg-blue-50' : 'border-gray-200'
              }`}
            >
              <div className="px-4 py-4 sm:px-6">
                <div className="flex items-center justify-between">
                  <div>
                    <div className="flex items-center space-x-2">
                      <span className="inline-flex items-center px-2 py-0.5 rounded text-xs font-medium bg-gray-100 text-gray-700">
                        {wallet.wallet_pubkey.startsWith('ed25519:') ? 'NEAR' : wallet.wallet_pubkey.split(':')[0]}
                      </span>
                      {wallet.frozen ? (
                        <span className="inline-flex items-center px-2.5 py-0.5 rounded-full text-xs font-medium bg-blue-100 text-blue-800">
                          FROZEN
                        </span>
                      ) : (
                        <span className="inline-flex items-center px-2.5 py-0.5 rounded-full text-xs font-medium bg-green-100 text-green-800">
                          Active
                        </span>
                      )}
                    </div>
                    <p className="mt-1 text-xs text-gray-500 font-mono break-all">
                      {wallet.wallet_pubkey.split(':').slice(1).join(':') || wallet.wallet_pubkey}
                    </p>
                    <p className="text-xs text-gray-400 mt-1">
                      Updated: {formatTimestamp(wallet.updated_at)}
                    </p>
                  </div>
                  <div className="flex items-center space-x-2">
                    {walletKey ? (
                      <Link
                        href={`/wallet?key=${walletKey}`}
                        className="px-3 py-1.5 text-sm border border-[#cc6600] text-[#cc6600] rounded hover:bg-orange-50"
                      >
                        Edit Policy
                      </Link>
                    ) : (
                      <span className="px-3 py-1.5 text-sm border border-gray-300 text-gray-400 rounded cursor-not-allowed" title="Save an API key first to edit policy">
                        Edit Policy
                      </span>
                    )}
                    {wallet.frozen ? (
                      <button
                        onClick={() => handleUnfreeze(wallet.wallet_pubkey)}
                        disabled={submitting}
                        className="px-3 py-1.5 text-sm bg-green-600 text-white rounded hover:bg-green-700 disabled:opacity-50"
                      >
                        Unfreeze
                      </button>
                    ) : (
                      <button
                        onClick={() => handleFreeze(wallet.wallet_pubkey)}
                        disabled={submitting}
                        className="px-3 py-1.5 text-sm bg-red-600 text-white rounded hover:bg-red-700 disabled:opacity-50"
                      >
                        Freeze
                      </button>
                    )}
                  </div>
                </div>

                {/* API Key (local browser storage) */}
                <div className="mt-3 pt-3 border-t border-gray-100">
                  <div className="mb-2">
                    <span className="text-xs font-semibold text-gray-700">API Key</span>
                  </div>

                  {/* Local saved key */}
                  {savedKeys[wallet.wallet_pubkey] ? (
                    <div className="flex items-center gap-2 mb-2">
                      <span className="text-xs text-gray-500">Local:</span>
                      <code className="text-xs font-mono bg-gray-100 px-2 py-0.5 rounded select-all">
                        {revealedKeys.has(wallet.wallet_pubkey)
                          ? savedKeys[wallet.wallet_pubkey]
                          : savedKeys[wallet.wallet_pubkey].substring(0, 6) + '...' + savedKeys[wallet.wallet_pubkey].slice(-4)}
                      </code>
                      <button
                        onClick={() => setRevealedKeys((prev) => {
                          const next = new Set(prev);
                          next.has(wallet.wallet_pubkey) ? next.delete(wallet.wallet_pubkey) : next.add(wallet.wallet_pubkey);
                          return next;
                        })}
                        className="text-xs text-gray-400 hover:text-gray-600"
                      >
                        {revealedKeys.has(wallet.wallet_pubkey) ? 'hide' : 'show'}
                      </button>
                      <button
                        onClick={() => { navigator.clipboard.writeText(savedKeys[wallet.wallet_pubkey]); setSuccess('API key copied'); setTimeout(() => setSuccess(null), 2000); }}
                        className="text-xs text-[#cc6600] hover:underline"
                      >
                        copy
                      </button>
                      <button
                        onClick={() => { removeWalletKey(wallet.wallet_pubkey); setSavedKeys((prev) => { const n = { ...prev }; delete n[wallet.wallet_pubkey]; return n; }); }}
                        className="text-xs text-red-400 hover:text-red-600"
                      >
                        remove
                      </button>
                    </div>
                  ) : showKeyInput === wallet.wallet_pubkey ? (
                    <div className="flex items-center gap-2 mb-2">
                      <input
                        type="text"
                        value={keyInput}
                        onChange={(e) => setKeyInput(e.target.value)}
                        onKeyDown={(e) => {
                          if (e.key === 'Enter' && keyInput.trim()) {
                            saveWalletKey(wallet.wallet_pubkey, keyInput.trim());
                            setSavedKeys((prev) => ({ ...prev, [wallet.wallet_pubkey]: keyInput.trim() }));
                            setKeyInput('');
                            setShowKeyInput(null);
                          }
                        }}
                        placeholder="wk_..."
                        className="flex-1 px-2 py-1 border border-gray-300 rounded text-xs font-mono"
                        autoFocus
                      />
                      <button
                        onClick={() => {
                          if (keyInput.trim()) {
                            saveWalletKey(wallet.wallet_pubkey, keyInput.trim());
                            setSavedKeys((prev) => ({ ...prev, [wallet.wallet_pubkey]: keyInput.trim() }));
                            setKeyInput('');
                            setShowKeyInput(null);
                          }
                        }}
                        className="text-xs text-[#cc6600] hover:underline"
                      >
                        save
                      </button>
                      <button onClick={() => { setShowKeyInput(null); setKeyInput(''); }} className="text-xs text-gray-400 hover:text-gray-600">
                        cancel
                      </button>
                    </div>
                  ) : (
                    <button
                      onClick={() => { setShowKeyInput(wallet.wallet_pubkey); setKeyInput(''); }}
                      className="text-xs text-[#cc6600] hover:underline mb-2"
                    >
                      + Save API key to browser
                    </button>
                  )}

                  <p className="text-xs text-gray-400 mt-2">
                    Key is stored in this browser only. To add/rotate keys, update <code>authorized_key_hashes</code> in the policy.
                  </p>
                </div>
              </div>
            </div>
            );
          })}
        </div>
      )}
    </div>
  );
}
