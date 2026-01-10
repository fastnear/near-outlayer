'use client';

import { useState, useEffect, useCallback } from 'react';
import { useNearWallet } from '@/contexts/NearWalletContext';
import WalletConnectionModal from '@/components/WalletConnectionModal';
import { getCoordinatorApiUrl } from '@/lib/api';

interface EarningsBalance {
  project_owner: string;
  balance: string;
  total_earned: string;
  updated_at: number | null;
}

interface EarningRecord {
  id: number;
  call_id: string;
  project_id: string;
  payer_owner: string;
  payer_nonce: number;
  attached_deposit: string;
  created_at: number;
}

interface EarningsHistory {
  project_owner: string;
  earnings: EarningRecord[];
  total_count: number;
}

// Format USD from minimal units (6 decimals)
function formatUsd(minimalUnits: string, decimals: number = 6): string {
  const num = BigInt(minimalUnits || '0');
  const divisor = BigInt(10 ** decimals);
  const whole = num / divisor;
  const fraction = num % divisor;
  const fractionStr = fraction.toString().padStart(decimals, '0').slice(0, 6);
  return `$${whole}.${fractionStr}`;
}

export default function EarningsPage() {
  const {
    accountId,
    isConnected,
    network,
    stablecoin,
    shouldReopenModal,
    clearReopenModal,
  } = useNearWallet();
  const coordinatorUrl = getCoordinatorApiUrl(network);

  const [balance, setBalance] = useState<EarningsBalance | null>(null);
  const [history, setHistory] = useState<EarningRecord[]>([]);
  const [totalCount, setTotalCount] = useState(0);
  const [loading, setLoading] = useState(false);
  const [showWalletModal, setShowWalletModal] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // Load earnings balance
  const loadBalance = useCallback(async () => {
    if (!accountId) return;

    try {
      const response = await fetch(
        `${coordinatorUrl}/public/project-earnings/${accountId}`
      );
      if (response.ok) {
        const data = await response.json();
        setBalance(data);
      }
    } catch (err) {
      console.error('Failed to load earnings balance:', err);
    }
  }, [accountId, coordinatorUrl]);

  // Load earnings history
  const loadHistory = useCallback(async () => {
    if (!accountId) return;

    setLoading(true);
    try {
      const response = await fetch(
        `${coordinatorUrl}/public/project-earnings/${accountId}/history?limit=50`
      );
      if (response.ok) {
        const data: EarningsHistory = await response.json();
        setHistory(data.earnings);
        setTotalCount(data.total_count);
      }
    } catch (err) {
      console.error('Failed to load earnings history:', err);
      setError('Failed to load earnings history');
    } finally {
      setLoading(false);
    }
  }, [accountId, coordinatorUrl]);

  // Auto-open modal if we switched networks
  useEffect(() => {
    if (shouldReopenModal && !isConnected) {
      setShowWalletModal(true);
      clearReopenModal();
    }
  }, [shouldReopenModal, isConnected, clearReopenModal]);

  // Load data when connected
  useEffect(() => {
    if (isConnected && accountId) {
      loadBalance();
      loadHistory();
    }
  }, [isConnected, accountId, loadBalance, loadHistory]);

  // Clear error after 5 seconds
  useEffect(() => {
    if (error) {
      const timer = setTimeout(() => setError(null), 5000);
      return () => clearTimeout(timer);
    }
  }, [error]);

  return (
    <div className="max-w-4xl mx-auto">
      {/* Header */}
      <div className="sm:flex sm:items-center sm:justify-between">
        <div>
          <h1 className="text-3xl font-bold text-gray-900">My Earnings</h1>
          <p className="mt-2 text-sm text-gray-700">
            Track earnings from HTTPS API calls to your projects
          </p>
        </div>
        {isConnected && (
          <div className="mt-4 sm:mt-0">
            <button
              onClick={() => { loadBalance(); loadHistory(); }}
              className="inline-flex items-center px-4 py-2 border border-gray-300 text-sm font-medium rounded-md text-gray-700 bg-white hover:bg-gray-50 shadow-sm"
            >
              <svg className="h-4 w-4 mr-2" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15" />
              </svg>
              Refresh
            </button>
          </div>
        )}
      </div>

      {/* Connect Wallet Button */}
      {!isConnected && (
        <div className="mt-8 flex justify-center">
          <button
            onClick={() => setShowWalletModal(true)}
            className="inline-flex items-center px-6 py-3 border border-transparent text-base font-medium rounded-md text-white bg-gradient-to-r from-[#cc6600] to-[#d4a017] hover:from-[#b35900] hover:to-[#c49016] shadow-sm hover:shadow-md transition-all"
          >
            Connect Wallet
          </button>
        </div>
      )}

      {/* Wallet Modal */}
      <WalletConnectionModal
        isOpen={showWalletModal}
        onClose={() => setShowWalletModal(false)}
      />

      {/* Error Display */}
      {error && (
        <div className="mt-4 bg-red-50 border border-red-200 rounded-md p-3">
          <p className="text-sm text-red-800">{error}</p>
        </div>
      )}

      {/* Balance Cards */}
      {isConnected && (
        <div className="mt-8 grid grid-cols-1 md:grid-cols-2 gap-4">
          {/* Available Balance Card */}
          <div className="bg-white shadow rounded-lg p-6 border border-gray-200">
            <div className="flex items-center">
              <div className="flex-shrink-0">
                <div className="w-12 h-12 bg-green-100 rounded-full flex items-center justify-center">
                  <svg className="h-6 w-6 text-green-600" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                    <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M12 8c-1.657 0-3 .895-3 2s1.343 2 3 2 3 .895 3 2-1.343 2-3 2m0-8c1.11 0 2.08.402 2.599 1M12 8V7m0 1v8m0 0v1m0-1c-1.11 0-2.08-.402-2.599-1M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
                  </svg>
                </div>
              </div>
              <div className="ml-4">
                <p className="text-sm font-medium text-gray-500">Available Balance</p>
                <p className="text-2xl font-bold text-green-600">
                  {balance ? formatUsd(balance.balance, stablecoin.decimals) : '$0.000000'}
                </p>
              </div>
            </div>
            <div className="mt-4">
              <button
                disabled
                className="w-full inline-flex justify-center items-center px-4 py-2 border border-gray-300 text-sm font-medium rounded-md text-gray-400 bg-gray-100 cursor-not-allowed"
                title="Withdrawal coming soon"
              >
                Withdraw (Coming Soon)
              </button>
            </div>
          </div>

          {/* Total Earned Card */}
          <div className="bg-white shadow rounded-lg p-6 border border-gray-200">
            <div className="flex items-center">
              <div className="flex-shrink-0">
                <div className="w-12 h-12 bg-[#cc6600]/10 rounded-full flex items-center justify-center">
                  <svg className="h-6 w-6 text-[#cc6600]" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                    <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M13 7h8m0 0v8m0-8l-8 8-4-4-6 6" />
                  </svg>
                </div>
              </div>
              <div className="ml-4">
                <p className="text-sm font-medium text-gray-500">Total Earned</p>
                <p className="text-2xl font-bold text-[#cc6600]">
                  {balance ? formatUsd(balance.total_earned, stablecoin.decimals) : '$0.000000'}
                </p>
              </div>
            </div>
            {balance?.updated_at && (
              <p className="mt-4 text-xs text-gray-500">
                Last updated: {new Date(balance.updated_at * 1000).toLocaleString()}
              </p>
            )}
          </div>
        </div>
      )}

      {/* Earnings History */}
      {isConnected && (
        <div className="mt-8 bg-white shadow rounded-lg border border-gray-200 overflow-hidden">
          <div className="px-6 py-4 border-b border-gray-200">
            <h2 className="text-lg font-semibold text-gray-900">
              Earnings History
              {totalCount > 0 && (
                <span className="text-sm font-normal text-gray-500 ml-2">
                  ({totalCount} total)
                </span>
              )}
            </h2>
          </div>

          {loading ? (
            <div className="flex items-center justify-center py-12">
              <svg className="animate-spin h-8 w-8 text-[#cc6600]" fill="none" viewBox="0 0 24 24">
                <circle className="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" strokeWidth="4" />
                <path className="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4z" />
              </svg>
              <span className="ml-3 text-gray-500">Loading history...</span>
            </div>
          ) : history.length === 0 ? (
            <div className="text-center py-12">
              <svg className="mx-auto h-12 w-12 text-gray-400" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M9 5H7a2 2 0 00-2 2v12a2 2 0 002 2h10a2 2 0 002-2V7a2 2 0 00-2-2h-2M9 5a2 2 0 002 2h2a2 2 0 002-2M9 5a2 2 0 012-2h2a2 2 0 012 2" />
              </svg>
              <h3 className="mt-2 text-sm font-medium text-gray-900">No earnings yet</h3>
              <p className="mt-1 text-sm text-gray-500">
                Earnings will appear here when users call your projects with attached deposits.
              </p>
            </div>
          ) : (
            <div className="overflow-x-auto">
              <table className="w-full text-sm">
                <thead className="bg-gray-50">
                  <tr className="text-left text-gray-500">
                    <th className="px-6 py-3 font-medium">Date</th>
                    <th className="px-6 py-3 font-medium">Project</th>
                    <th className="px-6 py-3 font-medium">Payer</th>
                    <th className="px-6 py-3 font-medium text-right">Amount</th>
                  </tr>
                </thead>
                <tbody className="divide-y divide-gray-200">
                  {history.map((record) => (
                    <tr key={record.id} className="hover:bg-gray-50">
                      <td className="px-6 py-4 text-gray-700">
                        {new Date(record.created_at * 1000).toLocaleString()}
                      </td>
                      <td className="px-6 py-4">
                        <span className="text-[#cc6600] font-mono text-xs">
                          {record.project_id.length > 25
                            ? record.project_id.slice(0, 25) + '...'
                            : record.project_id}
                        </span>
                      </td>
                      <td className="px-6 py-4">
                        <span className="text-gray-700">{record.payer_owner}</span>
                        <span className="text-gray-400 text-xs ml-1">#{record.payer_nonce}</span>
                      </td>
                      <td className="px-6 py-4 text-right">
                        <span className="text-green-600 font-semibold">
                          +{formatUsd(record.attached_deposit, stablecoin.decimals)}
                        </span>
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          )}
        </div>
      )}

      {/* Info Section */}
      <div className="mt-8 bg-blue-50 border border-blue-200 rounded-lg p-6">
        <h3 className="text-sm font-semibold text-blue-900 mb-3">
          About Earnings
        </h3>
        <ul className="text-sm text-blue-800 space-y-2 list-disc list-inside">
          <li>
            <strong>HTTPS API Deposits</strong>: When users call your projects via HTTPS API with X-Attached-Deposit header
          </li>
          <li>
            <strong>Stablecoin</strong>: Earnings are in {stablecoin.symbol} ({stablecoin.contract})
          </li>
          <li>
            <strong>Instant Credit</strong>: Deposits are credited immediately after successful execution
          </li>
          <li>
            <strong>Withdrawal</strong>: Coming soon - withdraw to your NEAR wallet
          </li>
        </ul>

        <div className="mt-4 p-3 bg-white rounded border border-blue-200">
          <h4 className="text-xs font-semibold text-blue-900 mb-2">Example API Call with Deposit</h4>
          <code className="text-xs text-blue-800 font-mono block whitespace-pre-wrap">
{`curl -X POST https://api.outlayer.io/call/${accountId || 'yourname.near'}/project
  -H "X-Payment-Key: owner:nonce:key"
  -H "X-Attached-Deposit: 10000"  # $0.01 in minimal units`}
          </code>
        </div>
      </div>
    </div>
  );
}
