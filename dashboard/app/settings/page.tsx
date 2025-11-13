'use client';

import { useEffect, useState } from 'react';
import { useNearWallet } from '@/contexts/NearWalletContext';
import { fetchUserEarnings, UserEarnings, createApiKey } from '@/lib/api';
import NetworkSwitcher from '@/components/NetworkSwitcher';
import WalletConnectionModal from '@/components/WalletConnectionModal';

export default function SettingsPage() {
  const { accountId, isConnected, disconnect, shouldReopenModal, clearReopenModal } = useNearWallet();
  const [earnings, setEarnings] = useState<UserEarnings | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [showWalletModal, setShowWalletModal] = useState(false);

  // API Key state
  const [apiKey, setApiKey] = useState<string | null>(null);
  const [generatingKey, setGeneratingKey] = useState(false);
  const [keyError, setKeyError] = useState<string | null>(null);
  const [showApiKey, setShowApiKey] = useState(false);

  // Auto-open modal if we switched networks
  useEffect(() => {
    if (shouldReopenModal && !isConnected) {
      setShowWalletModal(true);
      clearReopenModal();
    }
  }, [shouldReopenModal, isConnected, clearReopenModal]);

  useEffect(() => {
    if (isConnected && accountId) {
      loadEarnings();
      loadApiKey();
    }
  }, [isConnected, accountId]);

  const loadApiKey = () => {
    if (!accountId) return;
    const stored = localStorage.getItem(`outlayer-api-key-${accountId}`);
    if (stored) {
      setApiKey(stored);
    }
  };

  const handleGenerateApiKey = async () => {
    if (!accountId) return;

    setGeneratingKey(true);
    setKeyError(null);

    try {
      const response = await createApiKey({
        near_account_id: accountId,
        key_name: 'dashboard-key',
      });

      setApiKey(response.api_key);
      localStorage.setItem(`outlayer-api-key-${accountId}`, response.api_key);
      setShowApiKey(true);
    } catch (err) {
      setKeyError('Failed to generate API key');
      console.error(err);
    } finally {
      setGeneratingKey(false);
    }
  };

  const loadEarnings = async () => {
    if (!accountId) return;

    setLoading(true);
    setError(null);

    try {
      const data = await fetchUserEarnings(accountId);
      setEarnings(data);
    } catch (err) {
      setError('Failed to load earnings data');
      console.error(err);
    } finally {
      setLoading(false);
    }
  };

  const formatYoctoNEAR = (yocto: string) => {
    const near = parseFloat(yocto) / 1e24;
    return near.toFixed(6);
  };

  const formatInstructions = (instructions: number) => {
    if (instructions > 1e9) return (instructions / 1e9).toFixed(2) + 'B';
    if (instructions > 1e6) return (instructions / 1e6).toFixed(2) + 'M';
    if (instructions > 1e3) return (instructions / 1e3).toFixed(2) + 'K';
    return instructions.toString();
  };

  if (!isConnected) {
    return (
      <div className="max-w-4xl mx-auto">
        <div className="text-center py-12">
          <svg
            className="mx-auto h-12 w-12 text-gray-400"
            fill="none"
            viewBox="0 0 24 24"
            stroke="currentColor"
          >
            <path
              strokeLinecap="round"
              strokeLinejoin="round"
              strokeWidth={2}
              d="M12 15v2m-6 4h12a2 2 0 002-2v-6a2 2 0 00-2-2H6a2 2 0 00-2 2v6a2 2 0 002 2zm10-10V7a4 4 0 00-8 0v4h8z"
            />
          </svg>
          <h3 className="mt-2 text-sm font-medium text-gray-900">Not Connected</h3>
          <p className="mt-1 text-sm text-gray-500">
            Connect your NEAR wallet to view your settings and earnings
          </p>
          <div className="mt-6">
            <button
              onClick={() => setShowWalletModal(true)}
              className="inline-flex items-center px-6 py-3 border border-transparent shadow-sm text-base font-medium rounded-md text-white bg-gradient-to-r from-[#cc6600] to-[#d4a017] hover:from-[#b35900] hover:to-[#c49016] transition-all"
            >
              Connect Wallet
            </button>
          </div>
        </div>

        {/* Wallet Connection Modal */}
        <WalletConnectionModal
          isOpen={showWalletModal}
          onClose={() => setShowWalletModal(false)}
        />
      </div>
    );
  }

  return (
    <div className="max-w-4xl mx-auto">
      <div className="sm:flex sm:items-center sm:justify-between">
        <div className="sm:flex-auto">
          <h1 className="text-3xl font-bold text-gray-900">Settings</h1>
          <p className="mt-2 text-sm text-gray-700">Manage your account and view usage statistics</p>
        </div>
        <div className="mt-4 sm:mt-0">
          <button
            onClick={disconnect}
            className="inline-flex items-center px-4 py-2 border border-gray-300 shadow-sm text-sm font-medium rounded-md text-gray-700 bg-white hover:bg-gray-50"
          >
            Disconnect
          </button>
        </div>
      </div>

      {/* Account Information */}
      <div className="mt-8 bg-white shadow overflow-hidden sm:rounded-lg">
        <div className="px-4 py-5 sm:px-6">
          <h3 className="text-lg leading-6 font-medium text-gray-900">Account Information</h3>
          <p className="mt-1 max-w-2xl text-sm text-gray-500">Your NEAR wallet details</p>
        </div>
        <div className="border-t border-gray-200 px-4 py-5 sm:p-0">
          <dl className="sm:divide-y sm:divide-gray-200">
            <div className="py-4 sm:py-5 sm:grid sm:grid-cols-3 sm:gap-4 sm:px-6">
              <dt className="text-sm font-medium text-gray-500">Account ID</dt>
              <dd className="mt-1 text-sm text-gray-900 sm:mt-0 sm:col-span-2 font-mono">
                {accountId}
              </dd>
            </div>
            <div className="py-4 sm:py-5 sm:grid sm:grid-cols-3 sm:gap-4 sm:px-6">
              <dt className="text-sm font-medium text-gray-500">Network</dt>
              <dd className="mt-1 text-sm text-gray-900 sm:mt-0 sm:col-span-2">
                <NetworkSwitcher />
              </dd>
            </div>
          </dl>
        </div>
      </div>

      {/* API Key Section */}
      <div className="mt-8 bg-white shadow overflow-hidden sm:rounded-lg">
        <div className="px-4 py-5 sm:px-6">
          <h3 className="text-lg leading-6 font-medium text-gray-900">API Key</h3>
          <p className="mt-1 max-w-2xl text-sm text-gray-500">
            Generate an API key to access attestation data
          </p>
        </div>
        <div className="border-t border-gray-200 px-4 py-5 sm:px-6">
          {!apiKey ? (
            <div>
              <p className="text-sm text-gray-600 mb-4">
                You need an API key to view TEE attestations for your executions.
              </p>
              <button
                onClick={handleGenerateApiKey}
                disabled={generatingKey}
                className="inline-flex items-center px-4 py-2 border border-transparent text-sm font-medium rounded-md shadow-sm text-white bg-gradient-to-r from-[#cc6600] to-[#d4a017] hover:from-[#b35900] hover:to-[#c49016] disabled:opacity-50 disabled:cursor-not-allowed"
              >
                {generatingKey ? (
                  <>
                    <svg className="animate-spin -ml-1 mr-2 h-4 w-4 text-white" fill="none" viewBox="0 0 24 24">
                      <circle className="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" strokeWidth="4" />
                      <path className="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z" />
                    </svg>
                    Generating...
                  </>
                ) : (
                  'Generate API Key'
                )}
              </button>
              {keyError && (
                <p className="mt-2 text-sm text-red-600">{keyError}</p>
              )}
            </div>
          ) : (
            <div>
              <div className="flex items-center justify-between mb-2">
                <label className="text-sm font-medium text-gray-500">Your API Key</label>
                <button
                  onClick={() => setShowApiKey(!showApiKey)}
                  className="text-sm text-blue-600 hover:text-blue-800"
                >
                  {showApiKey ? 'Hide' : 'Show'}
                </button>
              </div>
              <div className="flex items-center gap-2">
                <input
                  type={showApiKey ? 'text' : 'password'}
                  value={apiKey}
                  readOnly
                  className="flex-1 px-3 py-2 border border-gray-300 rounded-md font-mono text-sm bg-gray-50"
                />
                <button
                  onClick={() => {
                    navigator.clipboard.writeText(apiKey);
                    alert('API key copied to clipboard!');
                  }}
                  className="px-3 py-2 bg-gray-100 hover:bg-gray-200 rounded-md text-sm font-medium"
                  title="Copy to clipboard"
                >
                  Copy
                </button>
              </div>
              <p className="mt-2 text-xs text-gray-500">
                Keep your API key secure. It's stored locally in your browser.
              </p>
            </div>
          )}
        </div>
      </div>

      {/* Usage Statistics */}
      {loading ? (
        <div className="mt-8 flex justify-center">
          <div className="animate-spin rounded-full h-8 w-8 border-b-2 border-blue-600"></div>
        </div>
      ) : error ? (
        <div className="mt-8 bg-red-50 border border-red-200 rounded-md p-4">
          <p className="text-red-800">{error}</p>
        </div>
      ) : earnings ? (
        <>
          <div className="mt-8 bg-white shadow overflow-hidden sm:rounded-lg">
            <div className="px-4 py-5 sm:px-6">
              <h3 className="text-lg leading-6 font-medium text-gray-900">Usage Statistics</h3>
              <p className="mt-1 max-w-2xl text-sm text-gray-500">
                Your off-chain execution history and spending
              </p>
            </div>
            <div className="border-t border-gray-200 px-4 py-5 sm:p-0">
              <dl className="sm:divide-y sm:divide-gray-200">
                <div className="py-4 sm:py-5 sm:grid sm:grid-cols-3 sm:gap-4 sm:px-6">
                  <dt className="text-sm font-medium text-gray-500">Total Executions</dt>
                  <dd className="mt-1 text-sm text-gray-900 sm:mt-0 sm:col-span-2">
                    {earnings.total_executions}
                  </dd>
                </div>
                <div className="py-4 sm:py-5 sm:grid sm:grid-cols-3 sm:gap-4 sm:px-6">
                  <dt className="text-sm font-medium text-gray-500">Successful Executions</dt>
                  <dd className="mt-1 text-sm text-gray-900 sm:mt-0 sm:col-span-2">
                    {earnings.successful_executions}{' '}
                    {earnings.total_executions > 0 && (
                      <span className="text-gray-500">
                        ({((earnings.successful_executions / earnings.total_executions) * 100).toFixed(1)}%)
                      </span>
                    )}
                  </dd>
                </div>
                <div className="py-4 sm:py-5 sm:grid sm:grid-cols-3 sm:gap-4 sm:px-6">
                  <dt className="text-sm font-medium text-gray-500">Total NEAR Spent</dt>
                  <dd className="mt-1 text-sm text-gray-900 sm:mt-0 sm:col-span-2">
                    <span className="text-lg font-semibold">
                      {formatYoctoNEAR(earnings.total_near_spent_yocto)} NEAR
                    </span>
                  </dd>
                </div>
                <div className="py-4 sm:py-5 sm:grid sm:grid-cols-3 sm:gap-4 sm:px-6">
                  <dt className="text-sm font-medium text-gray-500">Total Instructions Used</dt>
                  <dd className="mt-1 text-sm text-gray-900 sm:mt-0 sm:col-span-2">
                    {formatInstructions(earnings.total_instructions_used)}
                  </dd>
                </div>
                <div className="py-4 sm:py-5 sm:grid sm:grid-cols-3 sm:gap-4 sm:px-6">
                  <dt className="text-sm font-medium text-gray-500">Average Execution Time</dt>
                  <dd className="mt-1 text-sm text-gray-900 sm:mt-0 sm:col-span-2">
                    {earnings.average_execution_time_ms} ms
                  </dd>
                </div>
              </dl>
            </div>
          </div>

          {/* Cost Breakdown */}
          {earnings.total_executions > 0 && (
            <div className="mt-8 bg-blue-50 border border-blue-200 rounded-lg p-4">
              <div className="flex">
                <div className="flex-shrink-0">
                  <svg
                    className="h-5 w-5 text-blue-400"
                    fill="none"
                    viewBox="0 0 24 24"
                    stroke="currentColor"
                  >
                    <path
                      strokeLinecap="round"
                      strokeLinejoin="round"
                      strokeWidth={2}
                      d="M13 16h-1v-4h-1m1-4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z"
                    />
                  </svg>
                </div>
                <div className="ml-3">
                  <h3 className="text-sm font-medium text-blue-800">Average Cost Per Execution</h3>
                  <div className="mt-2 text-sm text-blue-700">
                    <p>
                      ~{(parseFloat(earnings.total_near_spent_yocto) / 1e24 / earnings.total_executions).toFixed(6)} NEAR per execution
                    </p>
                  </div>
                </div>
              </div>
            </div>
          )}
        </>
      ) : null}
    </div>
  );
}
