'use client';

import { useState, useEffect, useCallback } from 'react';
import { useNearWallet } from '@/contexts/NearWalletContext';
import WalletConnectionModal from '@/components/WalletConnectionModal';
import { getCoordinatorApiUrl } from '@/lib/api';
import { actionCreators } from '@near-js/transactions';

// HTTPS earnings balance (from coordinator)
interface HttpsEarningsBalance {
  project_owner: string;
  balance: string;
  total_earned: string;
  updated_at: number | null;
}

// Unified earning record from earnings_history table
interface EarningRecord {
  id: number;
  project_id: string;
  attached_usd: string;
  refund_usd: string;
  amount: string; // Net amount (attached - refund)
  source: 'blockchain' | 'https';
  // Blockchain-specific
  tx_hash?: string;
  caller?: string;
  request_id?: number;
  // HTTPS-specific
  call_id?: string;
  payment_key_owner?: string;
  payment_key_nonce?: number;
  created_at: number;
}

interface EarningsHistoryResponse {
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
    contractId,
    viewMethod,
    signAndSendTransaction,
    shouldReopenModal,
    clearReopenModal,
  } = useNearWallet();
  const coordinatorUrl = getCoordinatorApiUrl(network);

  // Blockchain earnings (from contract)
  const [blockchainBalance, setBlockchainBalance] = useState<string>('0');
  // HTTPS earnings (from coordinator)
  const [httpsBalance, setHttpsBalance] = useState<HttpsEarningsBalance | null>(null);
  // Unified history
  const [history, setHistory] = useState<EarningRecord[]>([]);
  const [totalCount, setTotalCount] = useState(0);
  const [sourceFilter, setSourceFilter] = useState<'all' | 'blockchain' | 'https'>('all');

  const [loading, setLoading] = useState(false);
  const [showWalletModal, setShowWalletModal] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [success, setSuccess] = useState<string | null>(null);
  const [withdrawing, setWithdrawing] = useState(false);

  // Load blockchain earnings from contract
  const loadBlockchainBalance = useCallback(async () => {
    if (!accountId) return;

    try {
      const balance = await viewMethod({
        contractId,
        method: 'get_developer_earnings',
        args: { account_id: accountId },
      });
      // Contract returns U128 as string
      setBlockchainBalance(typeof balance === 'string' ? balance : (balance as { toString: () => string })?.toString() || '0');
    } catch (err) {
      console.error('Failed to load blockchain earnings:', err);
      setBlockchainBalance('0');
    }
  }, [accountId, contractId, viewMethod]);

  // Load HTTPS earnings from coordinator
  const loadHttpsBalance = useCallback(async () => {
    if (!accountId) return;

    try {
      const response = await fetch(
        `${coordinatorUrl}/public/project-earnings/${accountId}`
      );
      if (response.ok) {
        const data = await response.json();
        setHttpsBalance(data);
      }
    } catch (err) {
      console.error('Failed to load HTTPS earnings balance:', err);
    }
  }, [accountId, coordinatorUrl]);

  // Load unified earnings history
  const loadHistory = useCallback(async () => {
    if (!accountId) return;

    setLoading(true);
    try {
      const sourceParam = sourceFilter !== 'all' ? `&source=${sourceFilter}` : '';
      const response = await fetch(
        `${coordinatorUrl}/public/project-earnings/${accountId}/history?limit=50${sourceParam}`
      );
      if (response.ok) {
        const data: EarningsHistoryResponse = await response.json();
        setHistory(data.earnings);
        setTotalCount(data.total_count);
      }
    } catch (err) {
      console.error('Failed to load earnings history:', err);
      setError('Failed to load earnings history');
    } finally {
      setLoading(false);
    }
  }, [accountId, coordinatorUrl, sourceFilter]);

  // Withdraw blockchain earnings
  const handleWithdraw = async () => {
    if (!accountId || BigInt(blockchainBalance || '0') <= BigInt(0)) return;

    setWithdrawing(true);
    setError(null);
    setSuccess(null);

    try {
      const action = actionCreators.functionCall(
        'withdraw_developer_earnings',
        {},
        BigInt('50000000000000'), // 50 TGas
        BigInt('1') // 1 yoctoNEAR required
      );

      await signAndSendTransaction({
        receiverId: contractId,
        actions: [action],
      });

      setSuccess(`Successfully withdrew ${formatUsd(blockchainBalance, stablecoin.decimals)} to your wallet!`);
      // Reload balances after withdrawal
      setTimeout(() => {
        loadBlockchainBalance();
        loadHistory();
      }, 2000);
    } catch (err) {
      setError((err as Error).message || 'Withdrawal failed');
    } finally {
      setWithdrawing(false);
    }
  };

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
      loadBlockchainBalance();
      loadHttpsBalance();
      loadHistory();
    }
  }, [isConnected, accountId, loadBlockchainBalance, loadHttpsBalance, loadHistory]);

  // Reload history when filter changes
  useEffect(() => {
    if (isConnected && accountId) {
      loadHistory();
    }
  }, [sourceFilter]); // eslint-disable-line react-hooks/exhaustive-deps

  // Clear messages after 5 seconds
  useEffect(() => {
    if (error || success) {
      const timer = setTimeout(() => {
        setError(null);
        setSuccess(null);
      }, 5000);
      return () => clearTimeout(timer);
    }
  }, [error, success]);

  return (
    <div className="max-w-4xl mx-auto">
      {/* Header */}
      <div className="sm:flex sm:items-center sm:justify-between">
        <div>
          <h1 className="text-3xl font-bold text-gray-900">My Earnings</h1>
          <p className="mt-2 text-sm text-gray-700">
            Track earnings from blockchain calls and HTTPS API calls to your projects
          </p>
        </div>
        {isConnected && (
          <div className="mt-4 sm:mt-0">
            <button
              onClick={() => { loadBlockchainBalance(); loadHttpsBalance(); loadHistory(); }}
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

      {/* Error/Success Display */}
      {error && (
        <div className="mt-4 bg-red-50 border border-red-200 rounded-md p-3">
          <p className="text-sm text-red-800">{error}</p>
        </div>
      )}
      {success && (
        <div className="mt-4 bg-green-50 border border-green-200 rounded-md p-3">
          <p className="text-sm text-green-800">{success}</p>
        </div>
      )}

      {/* Balance Cards - Two Columns */}
      {isConnected && (
        <div className="mt-8 grid grid-cols-1 md:grid-cols-2 gap-4">
          {/* Blockchain Earnings Card */}
          <div className="bg-gradient-to-br from-purple-50 to-indigo-50 shadow rounded-lg p-6 border border-purple-200">
            <div className="flex items-center mb-4">
              <div className="flex-shrink-0">
                <div className="w-12 h-12 bg-purple-100 rounded-full flex items-center justify-center">
                  <svg className="h-6 w-6 text-purple-600" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                    <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M19.428 15.428a2 2 0 00-1.022-.547l-2.387-.477a6 6 0 00-3.86.517l-.318.158a6 6 0 01-3.86.517L6.05 15.21a2 2 0 00-1.806.547M8 4h8l-1 1v5.172a2 2 0 00.586 1.414l5 5c1.26 1.26.367 3.414-1.415 3.414H4.828c-1.782 0-2.674-2.154-1.414-3.414l5-5A2 2 0 009 10.172V5L8 4z" />
                  </svg>
                </div>
              </div>
              <div className="ml-4">
                <p className="text-sm font-medium text-purple-700">Blockchain Earnings</p>
                <p className="text-xs text-purple-500">From smart contract calls</p>
              </div>
            </div>
            <p className="text-3xl font-bold text-purple-600 mb-4">
              {formatUsd(blockchainBalance, stablecoin.decimals)}
            </p>
            <button
              onClick={handleWithdraw}
              disabled={withdrawing || BigInt(blockchainBalance || '0') <= BigInt(0)}
              className="w-full inline-flex justify-center items-center px-4 py-2 border border-transparent text-sm font-medium rounded-md text-white bg-purple-600 hover:bg-purple-700 disabled:bg-gray-400 disabled:cursor-not-allowed shadow-sm"
            >
              {withdrawing ? (
                <>
                  <svg className="animate-spin -ml-1 mr-2 h-4 w-4 text-white" fill="none" viewBox="0 0 24 24">
                    <circle className="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" strokeWidth="4" />
                    <path className="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4z" />
                  </svg>
                  Withdrawing...
                </>
              ) : (
                <>Withdraw to Wallet</>
              )}
            </button>
            <p className="mt-2 text-xs text-purple-500 text-center">
              Stored in OutLayer contract
            </p>
          </div>

          {/* HTTPS Earnings Card */}
          <div className="bg-gradient-to-br from-green-50 to-emerald-50 shadow rounded-lg p-6 border border-green-200">
            <div className="flex items-center mb-4">
              <div className="flex-shrink-0">
                <div className="w-12 h-12 bg-green-100 rounded-full flex items-center justify-center">
                  <svg className="h-6 w-6 text-green-600" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                    <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M21 12a9 9 0 01-9 9m9-9a9 9 0 00-9-9m9 9H3m9 9a9 9 0 01-9-9m9 9c1.657 0 3-4.03 3-9s-1.343-9-3-9m0 18c-1.657 0-3-4.03-3-9s1.343-9 3-9m-9 9a9 9 0 019-9" />
                  </svg>
                </div>
              </div>
              <div className="ml-4">
                <p className="text-sm font-medium text-green-700">HTTPS API Earnings</p>
                <p className="text-xs text-green-500">From payment key calls</p>
              </div>
            </div>
            <p className="text-3xl font-bold text-green-600 mb-4">
              {httpsBalance ? formatUsd(httpsBalance.balance, stablecoin.decimals) : '$0.000000'}
            </p>
            <button
              disabled
              className="w-full inline-flex justify-center items-center px-4 py-2 border border-gray-300 text-sm font-medium rounded-md text-gray-400 bg-gray-100 cursor-not-allowed"
              title="Withdrawal coming soon"
            >
              Withdraw (Coming Soon)
            </button>
            <p className="mt-2 text-xs text-green-500 text-center">
              Stored in coordinator database
            </p>
          </div>
        </div>
      )}

      {/* Total Summary */}
      {isConnected && (
        <div className="mt-4 bg-white shadow rounded-lg p-4 border border-gray-200">
          <div className="flex items-center justify-between">
            <span className="text-sm font-medium text-gray-600">Total Available</span>
            <span className="text-xl font-bold text-gray-900">
              {formatUsd(
                (BigInt(blockchainBalance || '0') + BigInt(httpsBalance?.balance || '0')).toString(),
                stablecoin.decimals
              )}
            </span>
          </div>
        </div>
      )}

      {/* Earnings History */}
      {isConnected && (
        <div className="mt-8 bg-white shadow rounded-lg border border-gray-200 overflow-hidden">
          <div className="px-6 py-4 border-b border-gray-200 flex flex-col sm:flex-row sm:items-center sm:justify-between gap-3">
            <h2 className="text-lg font-semibold text-gray-900">
              Earnings History
              {totalCount > 0 && (
                <span className="text-sm font-normal text-gray-500 ml-2">
                  ({totalCount} total)
                </span>
              )}
            </h2>
            {/* Source Filter */}
            <div className="flex gap-2">
              <button
                onClick={() => setSourceFilter('all')}
                className={`px-3 py-1 text-xs font-medium rounded-full ${
                  sourceFilter === 'all'
                    ? 'bg-gray-800 text-white'
                    : 'bg-gray-100 text-gray-600 hover:bg-gray-200'
                }`}
              >
                All
              </button>
              <button
                onClick={() => setSourceFilter('blockchain')}
                className={`px-3 py-1 text-xs font-medium rounded-full ${
                  sourceFilter === 'blockchain'
                    ? 'bg-purple-600 text-white'
                    : 'bg-purple-100 text-purple-600 hover:bg-purple-200'
                }`}
              >
                Blockchain
              </button>
              <button
                onClick={() => setSourceFilter('https')}
                className={`px-3 py-1 text-xs font-medium rounded-full ${
                  sourceFilter === 'https'
                    ? 'bg-green-600 text-white'
                    : 'bg-green-100 text-green-600 hover:bg-green-200'
                }`}
              >
                HTTPS
              </button>
            </div>
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
                    <th className="px-6 py-3 font-medium">Source</th>
                    <th className="px-6 py-3 font-medium">Project</th>
                    <th className="px-6 py-3 font-medium">Details</th>
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
                        {record.source === 'blockchain' ? (
                          <span className="inline-flex items-center px-2 py-0.5 rounded text-xs font-medium bg-purple-100 text-purple-800">
                            Blockchain
                          </span>
                        ) : (
                          <span className="inline-flex items-center px-2 py-0.5 rounded text-xs font-medium bg-green-100 text-green-800">
                            HTTPS
                          </span>
                        )}
                      </td>
                      <td className="px-6 py-4">
                        <span className="text-[#cc6600] font-mono text-xs">
                          {record.project_id.length > 20
                            ? record.project_id.slice(0, 20) + '...'
                            : record.project_id}
                        </span>
                      </td>
                      <td className="px-6 py-4 text-xs text-gray-500">
                        {record.source === 'blockchain' ? (
                          record.caller ? (
                            <span>by {record.caller.length > 15 ? record.caller.slice(0, 15) + '...' : record.caller}</span>
                          ) : '-'
                        ) : (
                          record.payment_key_owner ? (
                            <span>{record.payment_key_owner}#{record.payment_key_nonce}</span>
                          ) : '-'
                        )}
                      </td>
                      <td className="px-6 py-4 text-right">
                        <div>
                          <span className="text-green-600 font-semibold">
                            +{formatUsd(record.amount, stablecoin.decimals)}
                          </span>
                          {BigInt(record.refund_usd || '0') > BigInt(0) && (
                            <span className="text-xs text-gray-400 ml-1">
                              (refund: {formatUsd(record.refund_usd, stablecoin.decimals)})
                            </span>
                          )}
                        </div>
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
        <div className="grid grid-cols-1 md:grid-cols-2 gap-4 text-sm text-blue-800">
          <div>
            <h4 className="font-medium text-purple-700 mb-1">Blockchain Earnings</h4>
            <ul className="space-y-1 list-disc list-inside text-xs">
              <li>From request_execution with attached_usd</li>
              <li>Stored in OutLayer smart contract</li>
              <li>Withdraw directly to your wallet</li>
            </ul>
          </div>
          <div>
            <h4 className="font-medium text-green-700 mb-1">HTTPS API Earnings</h4>
            <ul className="space-y-1 list-disc list-inside text-xs">
              <li>From payment key calls with X-Attached-Deposit</li>
              <li>Stored in coordinator database</li>
              <li>Withdrawal coming soon</li>
            </ul>
          </div>
        </div>
      </div>
    </div>
  );
}
