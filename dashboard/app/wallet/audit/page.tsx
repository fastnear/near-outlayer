'use client';

import { useState, useEffect, useCallback, useRef, Suspense } from 'react';
import { useSearchParams } from 'next/navigation';
import { useNearWallet } from '@/contexts/NearWalletContext';
import { getCoordinatorApiUrl } from '@/lib/api';
import Link from 'next/link';
import { getAllWalletKeys } from '@/lib/wallet-keys';

interface AuditEvent {
  type: string;
  request_id: string | null;
  status: string | null;
  details: Record<string, unknown>;
  at: string;
}

interface WalletEntry {
  pubkey: string;
  apiKey: string;
  walletId: string;
  label: string;
  events: AuditEvent[];
  error?: string;
  hasMore: boolean;
  page: number;
}

const EVENT_TYPE_COLORS: Record<string, string> = {
  withdraw: 'bg-orange-100 text-orange-800',
  withdraw_pending_approval: 'bg-yellow-100 text-yellow-800',
  withdraw_auto_executed: 'bg-green-100 text-green-800',
  deposit: 'bg-green-100 text-green-800',
  policy_change: 'bg-purple-100 text-purple-800',
  approval: 'bg-blue-100 text-blue-800',
  freeze: 'bg-red-100 text-red-800',
  unfreeze: 'bg-teal-100 text-teal-800',
};

const PAGE_SIZE = 50;

export default function WalletAuditPage() {
  return (
    <Suspense fallback={<div className="max-w-4xl mx-auto py-8 text-gray-400">Loading...</div>}>
      <WalletAuditContent />
    </Suspense>
  );
}

function WalletAuditContent() {
  const { network } = useNearWallet();
  const coordinatorUrl = getCoordinatorApiUrl(network);
  const searchParams = useSearchParams();

  const [wallets, setWallets] = useState<WalletEntry[]>([]);
  const [loading, setLoading] = useState(true);
  const [selectedWallet, setSelectedWallet] = useState<string>('all'); // 'all' or walletId
  const [manualKeyInput, setManualKeyInput] = useState('');
  const [noKeys, setNoKeys] = useState(false);
  const loadedRef = useRef(false);

  // Fetch wallet address + first page of events for a single key
  const fetchWalletData = useCallback(async (
    pubkey: string,
    apiKey: string,
    label: string | undefined,
    page: number,
  ): Promise<WalletEntry> => {
    const addrResp = await fetch(`${coordinatorUrl}/wallet/v1/address?chain=near`, {
      headers: { 'Authorization': `Bearer ${apiKey}` },
    });
    if (!addrResp.ok) {
      const err = await addrResp.json().catch(() => ({ error: addrResp.statusText }));
      throw new Error(err.error || err.message || `API error: ${addrResp.status}`);
    }
    const addrData = await addrResp.json();
    const walletId = addrData.wallet_id as string;

    const params = new URLSearchParams({
      limit: PAGE_SIZE.toString(),
      offset: (page * PAGE_SIZE).toString(),
    });
    const auditResp = await fetch(`${coordinatorUrl}/wallet/v1/audit?${params}`, {
      headers: { 'Authorization': `Bearer ${apiKey}` },
    });
    if (!auditResp.ok) {
      const err = await auditResp.json().catch(() => ({ error: auditResp.statusText }));
      throw new Error(err.error || err.message || `API error: ${auditResp.status}`);
    }
    const auditData = await auditResp.json();
    const events: AuditEvent[] = auditData.events || [];

    return {
      pubkey,
      apiKey,
      walletId,
      label: label || walletId.substring(0, 16),
      events,
      hasMore: events.length === PAGE_SIZE,
      page,
    };
  }, [coordinatorUrl]);

  // Initial load: gather all keys and fetch data
  useEffect(() => {
    if (loadedRef.current) return;
    loadedRef.current = true;

    const keyFromUrl = searchParams.get('key');
    const saved = getAllWalletKeys();

    // Collect unique keys
    const keyMap = new Map<string, { pubkey: string; apiKey: string; label?: string }>();

    if (keyFromUrl) {
      keyMap.set(keyFromUrl, { pubkey: '_url', apiKey: keyFromUrl, label: 'URL key' });
    }

    for (const [pubkey, stored] of Object.entries(saved)) {
      if (!keyMap.has(stored.apiKey)) {
        keyMap.set(stored.apiKey, { pubkey, apiKey: stored.apiKey, label: stored.label });
      }
    }

    if (keyMap.size === 0) {
      setNoKeys(true);
      setLoading(false);
      return;
    }

    const entries = Array.from(keyMap.values());

    // If only URL key, auto-select it
    if (keyFromUrl && entries.length === 1) {
      setSelectedWallet('_single');
    }

    Promise.allSettled(
      entries.map(e => fetchWalletData(e.pubkey, e.apiKey, e.label, 0))
    ).then(results => {
      const loaded: WalletEntry[] = [];
      for (let i = 0; i < results.length; i++) {
        const r = results[i];
        if (r.status === 'fulfilled') {
          loaded.push(r.value);
        } else {
          loaded.push({
            ...entries[i],
            walletId: '',
            label: entries[i].label || entries[i].pubkey.substring(0, 16),
            events: [],
            error: (r.reason as Error).message,
            hasMore: false,
            page: 0,
          });
        }
      }
      setWallets(loaded);
      setLoading(false);
    });
  }, [searchParams, fetchWalletData]);

  // Load next/prev page for a specific wallet
  const loadPage = useCallback(async (walletId: string, newPage: number) => {
    const wallet = wallets.find(w => w.walletId === walletId);
    if (!wallet) return;

    setWallets(prev => prev.map(w =>
      w.walletId === walletId ? { ...w, page: newPage, events: [] } : w
    ));
    setLoading(true);

    try {
      const updated = await fetchWalletData(wallet.pubkey, wallet.apiKey, wallet.label, newPage);
      setWallets(prev => prev.map(w => w.walletId === walletId ? updated : w));
    } catch (err) {
      setWallets(prev => prev.map(w =>
        w.walletId === walletId ? { ...w, error: (err as Error).message, events: [], page: newPage } : w
      ));
    } finally {
      setLoading(false);
    }
  }, [wallets, fetchWalletData]);

  const handleManualKeySubmit = useCallback(() => {
    const key = manualKeyInput.trim();
    if (!key) return;
    setManualKeyInput('');
    setLoading(true);
    setNoKeys(false);

    fetchWalletData('_manual', key, 'Manual key', 0).then(entry => {
      setWallets(prev => {
        if (prev.some(w => w.walletId === entry.walletId)) return prev;
        return [...prev, entry];
      });
      setSelectedWallet(entry.walletId);
    }).catch(err => {
      setWallets(prev => [...prev, {
        pubkey: '_manual',
        apiKey: key,
        walletId: `_err_${Date.now()}`,
        label: 'Manual key',
        events: [],
        error: (err as Error).message,
        hasMore: false,
        page: 0,
      }]);
    }).finally(() => setLoading(false));
  }, [manualKeyInput, fetchWalletData]);

  const formatDate = (dateStr: string) => new Date(dateStr).toLocaleString();

  const shortenId = (id: string) => {
    if (id.length <= 24) return id;
    return `${id.substring(0, 12)}...${id.substring(id.length - 8)}`;
  };

  // Compute visible events
  const multiWallet = wallets.length > 1;
  const visibleWallets = selectedWallet === 'all' || selectedWallet === '_single'
    ? wallets
    : wallets.filter(w => w.walletId === selectedWallet);

  // Merge events from visible wallets, annotated with wallet info
  const mergedEvents = visibleWallets
    .flatMap(w => w.events.map(e => ({ ...e, _walletId: w.walletId, _walletLabel: w.label })))
    .sort((a, b) => new Date(b.at).getTime() - new Date(a.at).getTime());

  // Pagination: only available when single wallet is selected
  const singleWallet = visibleWallets.length === 1 ? visibleWallets[0] : null;

  // Errors from visible wallets
  const errors = visibleWallets.filter(w => w.error).map(w => `${w.label}: ${w.error}`);

  // No keys at all - show manual input
  if (noKeys && wallets.length === 0) {
    return (
      <div className="max-w-6xl mx-auto">
        <h1 className="text-3xl font-bold text-gray-900 mb-6">Wallet Audit Log</h1>
        <div className="bg-white shadow rounded-lg p-8">
          <p className="text-gray-600 mb-4">
            No saved wallet keys found. Enter an API key to view the audit log.
          </p>
          <div className="flex gap-3">
            <input
              type="text"
              value={manualKeyInput}
              onChange={(e) => setManualKeyInput(e.target.value)}
              onKeyDown={(e) => e.key === 'Enter' && handleManualKeySubmit()}
              placeholder="wk_..."
              className="flex-1 px-4 py-2 border border-gray-300 rounded-lg focus:ring-2 focus:ring-[#cc6600] focus:border-transparent font-mono text-sm"
            />
            <button
              onClick={handleManualKeySubmit}
              disabled={!manualKeyInput.trim()}
              className="px-6 py-2 bg-gradient-to-r from-[#cc6600] to-[#d4a017] text-white rounded-lg font-medium hover:from-[#b35900] hover:to-[#c49016] disabled:opacity-50"
            >
              Load
            </button>
          </div>
        </div>
      </div>
    );
  }

  return (
    <div className="max-w-6xl mx-auto">
      <div className="flex items-center justify-between mb-6">
        <h1 className="text-3xl font-bold text-gray-900">Wallet Audit Log</h1>
        <div className="flex items-center space-x-3">
          <Link
            href="/wallet/approvals"
            className="text-sm text-[#cc6600] hover:text-[#b35900] font-medium"
          >
            Approvals
          </Link>
          <Link
            href="/wallet/manage"
            className="text-sm text-[#cc6600] hover:text-[#b35900] font-medium"
          >
            Manage
          </Link>
        </div>
      </div>

      {/* Wallet filter tabs */}
      {multiWallet && (
        <div className="flex items-center gap-2 mb-4 flex-wrap">
          <button
            onClick={() => setSelectedWallet('all')}
            className={`px-3 py-1.5 text-sm rounded-lg border transition-colors ${
              selectedWallet === 'all'
                ? 'bg-[#cc6600] text-white border-[#cc6600]'
                : 'bg-white text-gray-600 border-gray-300 hover:border-[#cc6600]'
            }`}
          >
            All wallets ({wallets.length})
          </button>
          {wallets.map(w => (
            <button
              key={w.walletId}
              onClick={() => setSelectedWallet(w.walletId)}
              className={`px-3 py-1.5 text-sm rounded-lg border transition-colors font-mono ${
                selectedWallet === w.walletId
                  ? 'bg-[#cc6600] text-white border-[#cc6600]'
                  : 'bg-white text-gray-600 border-gray-300 hover:border-[#cc6600]'
              }`}
            >
              {w.label}
              {w.error && <span className="ml-1 text-red-400">!</span>}
            </button>
          ))}
        </div>
      )}

      {errors.length > 0 && (
        <div className="mb-4 bg-red-50 border border-red-200 rounded-md p-3">
          {errors.map((e, i) => (
            <p key={i} className="text-sm text-red-800">{e}</p>
          ))}
        </div>
      )}

      {/* Events table */}
      {loading && mergedEvents.length === 0 ? (
        <div className="flex items-center justify-center py-12">
          <svg className="animate-spin h-8 w-8 text-[#cc6600]" fill="none" viewBox="0 0 24 24">
            <circle className="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" strokeWidth="4" />
            <path className="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4z" />
          </svg>
          <span className="ml-3 text-gray-500">Loading audit log...</span>
        </div>
      ) : mergedEvents.length === 0 ? (
        <div className="bg-white shadow rounded-lg p-8 text-center">
          <p className="text-gray-500">No audit events found.</p>
        </div>
      ) : (
        <>
          <div className="bg-white shadow rounded-lg overflow-hidden">
            <table className="min-w-full divide-y divide-gray-200">
              <thead className="bg-gray-50">
                <tr>
                  <th className="px-4 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider">
                    Time
                  </th>
                  {multiWallet && selectedWallet === 'all' && (
                    <th className="px-4 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider">
                      Wallet
                    </th>
                  )}
                  <th className="px-4 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider">
                    Type
                  </th>
                  <th className="px-4 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider">
                    Details
                  </th>
                  <th className="px-4 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider">
                    Request
                  </th>
                </tr>
              </thead>
              <tbody className="bg-white divide-y divide-gray-200">
                {mergedEvents.map((event, i) => (
                  <tr key={`${event._walletId}-${i}`} className="hover:bg-gray-50">
                    <td className="px-4 py-3 whitespace-nowrap text-xs text-gray-500">
                      {formatDate(event.at)}
                    </td>
                    {multiWallet && selectedWallet === 'all' && (
                      <td className="px-4 py-3 whitespace-nowrap text-xs text-gray-400 font-mono">
                        {event._walletLabel}
                      </td>
                    )}
                    <td className="px-4 py-3 whitespace-nowrap">
                      <span
                        className={`inline-flex items-center px-2 py-0.5 rounded-full text-xs font-medium ${
                          EVENT_TYPE_COLORS[event.type] || 'bg-gray-100 text-gray-800'
                        }`}
                      >
                        {event.type}
                      </span>
                    </td>
                    <td className="px-4 py-3 text-xs text-gray-600 max-w-md">
                      <pre className="whitespace-pre-wrap break-all">
                        {(JSON.stringify(event.details ?? {}, null, 2) || '{}').substring(0, 200)}
                        {(JSON.stringify(event.details ?? {}) || '{}').length > 200 ? '...' : ''}
                      </pre>
                    </td>
                    <td className="px-4 py-3 text-xs text-gray-500 font-mono">
                      {event.request_id ? shortenId(event.request_id) : '-'}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>

          {/* Pagination - only when single wallet is focused */}
          {singleWallet && (
            <div className="flex items-center justify-between mt-4">
              <button
                onClick={() => loadPage(singleWallet.walletId, Math.max(0, singleWallet.page - 1))}
                disabled={singleWallet.page === 0 || loading}
                className="px-4 py-2 text-sm border border-gray-300 rounded hover:bg-gray-50 disabled:opacity-50 disabled:cursor-not-allowed"
              >
                Previous
              </button>
              <span className="text-sm text-gray-500">Page {singleWallet.page + 1}</span>
              <button
                onClick={() => loadPage(singleWallet.walletId, singleWallet.page + 1)}
                disabled={!singleWallet.hasMore || loading}
                className="px-4 py-2 text-sm border border-gray-300 rounded hover:bg-gray-50 disabled:opacity-50 disabled:cursor-not-allowed"
              >
                Next
              </button>
            </div>
          )}
        </>
      )}
    </div>
  );
}
