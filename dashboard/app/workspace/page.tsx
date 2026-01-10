'use client';

import { useState, useEffect, useCallback } from 'react';
import Link from 'next/link';
import { useNearWallet } from '@/contexts/NearWalletContext';
import WalletConnectionModal from '@/components/WalletConnectionModal';
import NetworkSwitcher from '@/components/NetworkSwitcher';
import { getCoordinatorApiUrl, fetchUserEarnings, UserEarnings } from '@/lib/api';

// Types for data
interface ProjectView {
  project_id: string;
  name: string;
  owner: string;
  created_at: number;
  updated_at: number;
  uuid: string;
}

interface UserSecret {
  accessor: Record<string, unknown>;
  profile: string;
  created_at: number;
}

interface EarningsBalance {
  project_owner: string;
  balance: string;
  total_earned: string;
}

// Format USD from minimal units
function formatUsd(minimalUnits: string, decimals: number = 6): string {
  const num = BigInt(minimalUnits || '0');
  const divisor = BigInt(10 ** decimals);
  const whole = num / divisor;
  const fraction = num % divisor;
  const fractionStr = fraction.toString().padStart(decimals, '0').slice(0, 2);
  return `$${whole}.${fractionStr}`;
}

export default function WorkspacePage() {
  const {
    accountId,
    isConnected,
    disconnect,
    contractId,
    viewMethod,
    network,
    stablecoin,
    shouldReopenModal,
    clearReopenModal,
  } = useNearWallet();
  const coordinatorUrl = getCoordinatorApiUrl(network);

  const [showWalletModal, setShowWalletModal] = useState(false);
  const [loading, setLoading] = useState(false);

  // Data state
  const [projectCount, setProjectCount] = useState(0);
  const [secretsCount, setSecretsCount] = useState(0);
  const [paymentKeysCount, setPaymentKeysCount] = useState(0);
  const [earningsBalance, setEarningsBalance] = useState<EarningsBalance | null>(null);
  const [usageStats, setUsageStats] = useState<UserEarnings | null>(null);

  // Auto-open modal if we switched networks
  useEffect(() => {
    if (shouldReopenModal && !isConnected) {
      setShowWalletModal(true);
      clearReopenModal();
    }
  }, [shouldReopenModal, isConnected, clearReopenModal]);

  // Load all data
  const loadData = useCallback(async () => {
    if (!accountId) return;

    setLoading(true);
    try {
      // Load projects count
      const projects = await viewMethod({
        contractId,
        method: 'list_user_projects',
        args: { account_id: accountId },
      });
      setProjectCount(Array.isArray(projects) ? projects.length : 0);

      // Load secrets (all secrets including payment keys)
      const secrets = await viewMethod({
        contractId,
        method: 'list_user_secrets',
        args: { account_id: accountId },
      }) as UserSecret[];

      // Count secrets vs payment keys
      let secretsOnly = 0;
      let paymentKeys = 0;
      (secrets || []).forEach((s) => {
        if (s.accessor && typeof s.accessor === 'object' && 'System' in s.accessor) {
          const system = (s.accessor as { System: unknown }).System;
          if (system === 'PaymentKey') {
            paymentKeys++;
            return;
          }
        }
        secretsOnly++;
      });
      setSecretsCount(secretsOnly);
      setPaymentKeysCount(paymentKeys);

      // Load earnings balance
      try {
        const response = await fetch(`${coordinatorUrl}/public/project-earnings/${accountId}`);
        if (response.ok) {
          const data = await response.json();
          setEarningsBalance(data);
        }
      } catch {
        // Ignore - earnings might not exist
      }

      // Load usage stats
      try {
        const stats = await fetchUserEarnings(accountId);
        setUsageStats(stats);
      } catch {
        // Ignore - stats might not exist
      }
    } catch (err) {
      console.error('Failed to load workspace data:', err);
    } finally {
      setLoading(false);
    }
  }, [accountId, contractId, viewMethod, coordinatorUrl]);

  useEffect(() => {
    if (isConnected && accountId) {
      loadData();
    }
  }, [isConnected, accountId, loadData]);

  if (!isConnected) {
    return (
      <div className="max-w-4xl mx-auto">
        <div className="text-center py-12">
          <svg className="mx-auto h-16 w-16 text-gray-400" fill="none" viewBox="0 0 24 24" stroke="currentColor">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={1.5} d="M5.121 17.804A13.937 13.937 0 0112 16c2.5 0 4.847.655 6.879 1.804M15 10a3 3 0 11-6 0 3 3 0 016 0zm6 2a9 9 0 11-18 0 9 9 0 0118 0z" />
          </svg>
          <h2 className="mt-4 text-2xl font-bold text-gray-900">My Workspace</h2>
          <p className="mt-2 text-gray-600">Connect your NEAR wallet to access your workspace</p>
          <div className="mt-6">
            <button
              onClick={() => setShowWalletModal(true)}
              className="inline-flex items-center px-6 py-3 border border-transparent text-base font-medium rounded-md text-white bg-gradient-to-r from-[#cc6600] to-[#d4a017] hover:from-[#b35900] hover:to-[#c49016] shadow-sm"
            >
              Connect Wallet
            </button>
          </div>
        </div>
        <WalletConnectionModal isOpen={showWalletModal} onClose={() => setShowWalletModal(false)} />
      </div>
    );
  }

  return (
    <div className="max-w-5xl mx-auto">
      {/* Header with Account Info */}
      <div className="bg-white shadow rounded-lg p-6 mb-6">
        <div className="flex flex-col sm:flex-row sm:items-center sm:justify-between gap-4">
          <div>
            <h1 className="text-2xl font-bold text-gray-900">My Workspace</h1>
            <p className="text-sm text-gray-500 font-mono mt-1">{accountId}</p>
          </div>
          <div className="flex items-center gap-3">
            <NetworkSwitcher />
            <button
              onClick={disconnect}
              className="px-3 py-1.5 text-sm border border-gray-300 rounded-md text-gray-700 hover:bg-gray-50"
            >
              Disconnect
            </button>
            <Link
              href="/settings"
              className="px-3 py-1.5 text-sm border border-gray-300 rounded-md text-gray-700 hover:bg-gray-50"
            >
              Settings
            </Link>
          </div>
        </div>
      </div>

      {loading ? (
        <div className="flex items-center justify-center py-12">
          <svg className="animate-spin h-8 w-8 text-[#cc6600]" fill="none" viewBox="0 0 24 24">
            <circle className="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" strokeWidth="4" />
            <path className="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4z" />
          </svg>
        </div>
      ) : (
        <>
          {/* Main 4-Block Grid */}
          <div className="grid grid-cols-1 md:grid-cols-2 gap-6 mb-6">
            {/* Projects Block */}
            <div className="bg-white shadow rounded-lg p-6 border border-gray-200">
              <div className="flex items-start justify-between">
                <div className="flex items-center gap-3">
                  <div className="w-12 h-12 bg-blue-100 rounded-lg flex items-center justify-center">
                    <svg className="h-6 w-6 text-blue-600" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                      <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M3 7v10a2 2 0 002 2h14a2 2 0 002-2V9a2 2 0 00-2-2h-6l-2-2H5a2 2 0 00-2 2z" />
                    </svg>
                  </div>
                  <div>
                    <h3 className="text-lg font-semibold text-gray-900">Projects</h3>
                    <p className="text-sm text-gray-500">Your WASM applications</p>
                  </div>
                </div>
              </div>
              <div className="mt-4">
                <p className="text-3xl font-bold text-gray-900">{projectCount}</p>
                <p className="text-sm text-gray-500">total projects</p>
              </div>
              <div className="mt-4 pt-4 border-t border-gray-100">
                <Link
                  href="/projects"
                  className="inline-flex items-center text-sm font-medium text-blue-600 hover:text-blue-800"
                >
                  Manage Projects
                  <svg className="ml-1 h-4 w-4" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                    <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M9 5l7 7-7 7" />
                  </svg>
                </Link>
              </div>
            </div>

            {/* Secrets Block */}
            <div className="bg-white shadow rounded-lg p-6 border border-gray-200">
              <div className="flex items-start justify-between">
                <div className="flex items-center gap-3">
                  <div className="w-12 h-12 bg-green-100 rounded-lg flex items-center justify-center">
                    <svg className="h-6 w-6 text-green-600" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                      <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M12 15v2m-6 4h12a2 2 0 002-2v-6a2 2 0 00-2-2H6a2 2 0 00-2 2v6a2 2 0 002 2zm10-10V7a4 4 0 00-8 0v4h8z" />
                    </svg>
                  </div>
                  <div>
                    <h3 className="text-lg font-semibold text-gray-900">Secrets</h3>
                    <p className="text-sm text-gray-500">Encrypted environment variables</p>
                  </div>
                </div>
              </div>
              <div className="mt-4">
                <p className="text-3xl font-bold text-gray-900">{secretsCount}</p>
                <p className="text-sm text-gray-500">stored secrets</p>
              </div>
              <div className="mt-4 pt-4 border-t border-gray-100">
                <Link
                  href="/secrets"
                  className="inline-flex items-center text-sm font-medium text-green-600 hover:text-green-800"
                >
                  Manage Secrets
                  <svg className="ml-1 h-4 w-4" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                    <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M9 5l7 7-7 7" />
                  </svg>
                </Link>
              </div>
            </div>

            {/* Payment Keys Block */}
            <div className="bg-white shadow rounded-lg p-6 border border-gray-200">
              <div className="flex items-start justify-between">
                <div className="flex items-center gap-3">
                  <div className="w-12 h-12 bg-[#cc6600]/10 rounded-lg flex items-center justify-center">
                    <svg className="h-6 w-6 text-[#cc6600]" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                      <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M15 7a2 2 0 012 2m4 0a6 6 0 01-7.743 5.743L11 17H9v2H7v2H4a1 1 0 01-1-1v-2.586a1 1 0 01.293-.707l5.964-5.964A6 6 0 1121 9z" />
                    </svg>
                  </div>
                  <div>
                    <h3 className="text-lg font-semibold text-gray-900">Payment Keys</h3>
                    <p className="text-sm text-gray-500">For HTTPS API calls</p>
                  </div>
                </div>
              </div>
              <div className="mt-4">
                <p className="text-3xl font-bold text-gray-900">{paymentKeysCount}</p>
                <p className="text-sm text-gray-500">active keys</p>
              </div>
              <div className="mt-4 pt-4 border-t border-gray-100">
                <Link
                  href="/payment-keys"
                  className="inline-flex items-center text-sm font-medium text-[#cc6600] hover:text-[#b35900]"
                >
                  Manage Payment Keys
                  <svg className="ml-1 h-4 w-4" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                    <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M9 5l7 7-7 7" />
                  </svg>
                </Link>
              </div>
            </div>

            {/* Earnings Block */}
            <div className="bg-white shadow rounded-lg p-6 border border-gray-200">
              <div className="flex items-start justify-between">
                <div className="flex items-center gap-3">
                  <div className="w-12 h-12 bg-emerald-100 rounded-lg flex items-center justify-center">
                    <svg className="h-6 w-6 text-emerald-600" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                      <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M12 8c-1.657 0-3 .895-3 2s1.343 2 3 2 3 .895 3 2-1.343 2-3 2m0-8c1.11 0 2.08.402 2.599 1M12 8V7m0 1v8m0 0v1m0-1c-1.11 0-2.08-.402-2.599-1M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
                    </svg>
                  </div>
                  <div>
                    <h3 className="text-lg font-semibold text-gray-900">Earnings</h3>
                    <p className="text-sm text-gray-500">From project usage</p>
                  </div>
                </div>
              </div>
              <div className="mt-4">
                {earningsBalance ? (
                  <>
                    <p className="text-3xl font-bold text-emerald-600">
                      {formatUsd(earningsBalance.balance, stablecoin.decimals)}
                    </p>
                    <p className="text-sm text-gray-500">
                      available ({formatUsd(earningsBalance.total_earned, stablecoin.decimals)} total earned)
                    </p>
                  </>
                ) : (
                  <>
                    <p className="text-3xl font-bold text-gray-900">$0.00</p>
                    <p className="text-sm text-gray-500">no earnings yet</p>
                  </>
                )}
              </div>
              <div className="mt-4 pt-4 border-t border-gray-100">
                <Link
                  href="/earnings"
                  className="inline-flex items-center text-sm font-medium text-emerald-600 hover:text-emerald-800"
                >
                  View Earnings
                  <svg className="ml-1 h-4 w-4" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                    <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M9 5l7 7-7 7" />
                  </svg>
                </Link>
              </div>
            </div>
          </div>

          {/* Usage Stats Summary */}
          {usageStats && usageStats.total_executions > 0 && (
            <div className="bg-white shadow rounded-lg p-6 border border-gray-200">
              <h3 className="text-lg font-semibold text-gray-900 mb-4">Usage Summary</h3>
              <div className="grid grid-cols-2 md:grid-cols-4 gap-4">
                <div className="text-center p-3 bg-gray-50 rounded-lg">
                  <p className="text-2xl font-bold text-gray-900">{usageStats.total_executions}</p>
                  <p className="text-xs text-gray-500">Total Executions</p>
                </div>
                <div className="text-center p-3 bg-gray-50 rounded-lg">
                  <p className="text-2xl font-bold text-green-600">
                    {usageStats.total_executions > 0
                      ? ((usageStats.successful_executions / usageStats.total_executions) * 100).toFixed(0)
                      : 0}%
                  </p>
                  <p className="text-xs text-gray-500">Success Rate</p>
                </div>
                <div className="text-center p-3 bg-gray-50 rounded-lg">
                  <p className="text-2xl font-bold text-gray-900">
                    {(parseFloat(usageStats.total_near_spent_yocto) / 1e24).toFixed(4)}
                  </p>
                  <p className="text-xs text-gray-500">NEAR Spent</p>
                </div>
                <div className="text-center p-3 bg-gray-50 rounded-lg">
                  <p className="text-2xl font-bold text-gray-900">{usageStats.average_execution_time_ms}ms</p>
                  <p className="text-xs text-gray-500">Avg Execution</p>
                </div>
              </div>
            </div>
          )}

          {/* Quick Actions */}
          <div className="mt-6 bg-blue-50 border border-blue-200 rounded-lg p-4">
            <h3 className="text-sm font-semibold text-blue-900 mb-2">Quick Actions</h3>
            <div className="flex flex-wrap gap-3">
              <Link
                href="/projects"
                className="inline-flex items-center px-3 py-1.5 bg-white border border-blue-200 rounded-md text-sm text-blue-700 hover:bg-blue-50"
              >
                <svg className="h-4 w-4 mr-1.5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                  <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M12 4v16m8-8H4" />
                </svg>
                New Project
              </Link>
              <Link
                href="/secrets"
                className="inline-flex items-center px-3 py-1.5 bg-white border border-blue-200 rounded-md text-sm text-blue-700 hover:bg-blue-50"
              >
                <svg className="h-4 w-4 mr-1.5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                  <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M12 4v16m8-8H4" />
                </svg>
                Add Secret
              </Link>
              <Link
                href="/payment-keys"
                className="inline-flex items-center px-3 py-1.5 bg-white border border-blue-200 rounded-md text-sm text-blue-700 hover:bg-blue-50"
              >
                <svg className="h-4 w-4 mr-1.5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                  <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M12 4v16m8-8H4" />
                </svg>
                Create Payment Key
              </Link>
              <Link
                href="/playground"
                className="inline-flex items-center px-3 py-1.5 bg-white border border-blue-200 rounded-md text-sm text-blue-700 hover:bg-blue-50"
              >
                <svg className="h-4 w-4 mr-1.5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                  <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M14.752 11.168l-3.197-2.132A1 1 0 0010 9.87v4.263a1 1 0 001.555.832l3.197-2.132a1 1 0 000-1.664z" />
                  <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
                </svg>
                Try Playground
              </Link>
            </div>
          </div>
        </>
      )}

      <WalletConnectionModal isOpen={showWalletModal} onClose={() => setShowWalletModal(false)} />
    </div>
  );
}
