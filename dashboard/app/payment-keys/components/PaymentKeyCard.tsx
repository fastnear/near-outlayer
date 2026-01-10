'use client';

import { useState } from 'react';
import { StablecoinConfig } from '@/contexts/NearWalletContext';
import { PaymentKeyData, PaymentKeyBalance, PaymentKeyUsage, formatUsd } from './types';
import { fetchAttestation, AttestationResponse } from '@/lib/api';
import AttestationView from '@/components/AttestationView';
import type { NetworkType } from '@/lib/api';

interface PaymentKeyCardProps {
  paymentKey: PaymentKeyData;
  balance?: PaymentKeyBalance;
  stablecoin: StablecoinConfig;
  onTopUp: () => void;
  onDelete: () => void;
  coordinatorUrl: string;
  accountId: string;
  network: NetworkType;
}

export function PaymentKeyCard({
  paymentKey,
  balance,
  stablecoin,
  onTopUp,
  onDelete,
  coordinatorUrl,
  accountId,
  network,
}: PaymentKeyCardProps) {
  const [showUsage, setShowUsage] = useState(false);
  const [usage, setUsage] = useState<PaymentKeyUsage[]>([]);
  const [loadingUsage, setLoadingUsage] = useState(false);
  const [usageOffset, setUsageOffset] = useState(0);
  const [usageTotal, setUsageTotal] = useState(0);
  const usageLimit = 20;
  const [attestationModal, setAttestationModal] = useState<{
    jobId: number;
    attestation: AttestationResponse | null;
    loading: boolean;
    error: string | null;
  } | null>(null);

  const loadUsage = async (offset: number = 0) => {
    if (loadingUsage) return;

    setLoadingUsage(true);
    try {
      const response = await fetch(
        `${coordinatorUrl}/public/payment-keys/${accountId}/${paymentKey.nonce}/usage?offset=${offset}&limit=${usageLimit}`
      );
      if (response.ok) {
        const data = await response.json();
        setUsage(data.usage || []);
        setUsageTotal(data.total || 0);
        setUsageOffset(offset);
      }
    } catch (err) {
      console.error('Failed to load usage:', err);
    } finally {
      setLoadingUsage(false);
    }
  };

  const loadAttestation = async (jobId: number | null) => {
    if (!jobId) {
      return; // No job_id available
    }

    setAttestationModal({ jobId, attestation: null, loading: true, error: null });

    try {
      const apiKey = process.env.NEXT_PUBLIC_COORDINATOR_API_KEY || 'not-required';
      const data = await fetchAttestation(jobId, apiKey);

      if (!data) {
        setAttestationModal({
          jobId,
          attestation: null,
          loading: false,
          error: 'No attestation found for this job'
        });
        return;
      }

      setAttestationModal({ jobId, attestation: data, loading: false, error: null });
    } catch (err) {
      console.error('Failed to load attestation:', err);
      setAttestationModal({
        jobId,
        attestation: null,
        loading: false,
        error: err instanceof Error ? err.message : 'Failed to load attestation'
      });
    }
  };

  const toggleUsage = () => {
    if (!showUsage && usage.length === 0) {
      loadUsage(0);
    }
    setShowUsage(!showUsage);
  };

  const handlePrevPage = () => {
    const newOffset = Math.max(0, usageOffset - usageLimit);
    loadUsage(newOffset);
  };

  const handleNextPage = () => {
    const newOffset = usageOffset + usageLimit;
    if (newOffset < usageTotal) {
      loadUsage(newOffset);
    }
  };

  const formatDate = (timestamp: number | string) => {
    const date = typeof timestamp === 'number'
      ? new Date(timestamp / 1_000_000) // nanoseconds to ms
      : new Date(timestamp);
    return date.toLocaleDateString() + ' ' + date.toLocaleTimeString();
  };

  // Calculate available balance
  const initialBalance = balance?.initial_balance || '0';
  const spent = balance?.spent || '0';
  const reserved = balance?.reserved || '0';
  const available = balance?.available || '0';

  return (
    <div className="bg-white shadow rounded-lg overflow-hidden">
      {/* Header */}
      <div className="px-4 py-4 sm:px-6">
        <div className="flex justify-between items-start">
          <div className="flex items-center space-x-3">
            <div className="flex-shrink-0">
              <svg className="h-8 w-8 text-[#cc6600]" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M15 7a2 2 0 012 2m4 0a6 6 0 01-7.743 5.743L11 17H9v2H7v2H4a1 1 0 01-1-1v-2.586a1 1 0 01.293-.707l5.964-5.964A6 6 0 1121 9z" />
              </svg>
            </div>
            <div>
              <h3 className="text-lg font-medium text-gray-900">Key #{paymentKey.nonce}</h3>
              <p className="text-sm text-gray-500">
                Created: {formatDate(paymentKey.created_at)}
              </p>
            </div>
          </div>
          <div className="flex gap-2">
            <button
              onClick={onTopUp}
              className="inline-flex items-center px-3 py-1.5 border border-transparent text-sm font-medium rounded-md text-white bg-[#cc6600] hover:bg-[#b35900] transition-colors"
            >
              Top Up
            </button>
            <button
              onClick={onDelete}
              className="inline-flex items-center px-3 py-1.5 border border-red-300 text-sm font-medium rounded-md text-red-700 bg-white hover:bg-red-50 transition-colors"
              title="Delete Payment Key. WARNING: Remaining balance will be lost! Refunds are not yet implemented."
            >
              <svg className="h-4 w-4" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16" />
              </svg>
            </button>
          </div>
        </div>

        {/* Balance section */}
        <div className="mt-4 grid grid-cols-2 md:grid-cols-4 gap-3">
          <div className="bg-gray-50 rounded-lg p-3 border border-gray-200">
            <p className="text-xs text-gray-500">Initial Balance</p>
            <p className="font-mono text-sm text-gray-900 font-medium">
              {formatUsd(initialBalance, stablecoin.decimals)}
            </p>
          </div>
          <div className="bg-gray-50 rounded-lg p-3 border border-gray-200">
            <p className="text-xs text-gray-500">Spent</p>
            <p className="font-mono text-sm text-red-600 font-medium">
              -{formatUsd(spent, stablecoin.decimals)}
            </p>
          </div>
          <div className="bg-gray-50 rounded-lg p-3 border border-gray-200">
            <p className="text-xs text-gray-500">Reserved</p>
            <p className="font-mono text-sm text-yellow-600 font-medium">
              {formatUsd(reserved, stablecoin.decimals)}
            </p>
          </div>
          <div className="bg-green-50 rounded-lg p-3 border border-green-200">
            <p className="text-xs text-gray-500">Available</p>
            <p className="font-mono text-sm text-green-700 font-bold">
              {formatUsd(available, stablecoin.decimals)}
            </p>
          </div>
        </div>

        {/* Last used */}
        {balance?.last_used_at && (
          <p className="text-xs text-gray-500 mt-3">
            Last used: {new Date(balance.last_used_at).toLocaleString()}
          </p>
        )}

        {/* Usage toggle */}
        <button
          onClick={toggleUsage}
          className="mt-3 text-sm text-[#cc6600] hover:text-[#b35900] font-medium"
        >
          {showUsage ? 'Hide' : 'Show'} Usage History
        </button>
      </div>

      {/* Usage table */}
      {showUsage && (
        <div className="border-t border-gray-200 px-4 py-4 sm:px-6 bg-gray-50">
          {loadingUsage ? (
            <div className="flex items-center justify-center py-4">
              <svg className="animate-spin h-5 w-5 text-gray-400" fill="none" viewBox="0 0 24 24">
                <circle className="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" strokeWidth="4" />
                <path className="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4z" />
              </svg>
              <span className="ml-2 text-sm text-gray-500">Loading usage...</span>
            </div>
          ) : usage.length === 0 ? (
            <p className="text-gray-500 text-sm py-2">No usage history yet.</p>
          ) : (
            <div className="overflow-x-auto">
              <table className="w-full text-sm">
                <thead>
                  <tr className="text-left text-gray-500 border-b border-gray-200">
                    <th className="pb-2 font-medium">Date</th>
                    <th className="pb-2 font-medium">Project</th>
                    <th className="pb-2 font-medium">Compute</th>
                    <th className="pb-2 font-medium">Deposit</th>
                    <th className="pb-2 font-medium">Status</th>
                    <th className="pb-2 font-medium">TEE</th>
                  </tr>
                </thead>
                <tbody>
                  {usage.map((u) => (
                    <tr key={u.id} className="border-b border-gray-100">
                      <td className="py-2 text-gray-700">
                        {new Date(u.created_at).toLocaleString()}
                      </td>
                      <td className="py-2 text-gray-700 font-mono text-xs">
                        {u.project_id}
                      </td>
                      <td className="py-2 text-gray-700">
                        {formatUsd(u.compute_cost, stablecoin.decimals)}
                      </td>
                      <td className="py-2 text-gray-700">
                        {formatUsd(u.attached_deposit, stablecoin.decimals)}
                      </td>
                      <td className="py-2">
                        <span
                          className={`inline-flex px-2 py-0.5 rounded text-xs font-medium ${
                            u.status === 'completed'
                              ? 'bg-green-100 text-green-800'
                              : u.status === 'failed'
                              ? 'bg-red-100 text-red-800'
                              : 'bg-yellow-100 text-yellow-800'
                          }`}
                        >
                          {u.status}
                        </span>
                      </td>
                      <td className="py-2">
                        {u.job_id ? (
                          <button
                            onClick={() => loadAttestation(u.job_id)}
                            className="text-[#cc6600] hover:text-[#b35900] text-xs font-medium"
                            title={`View TEE attestation for job #${u.job_id}`}
                          >
                            View
                          </button>
                        ) : (
                          <span className="text-gray-400 text-xs">-</span>
                        )}
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
              {/* Pagination controls */}
              {usageTotal > usageLimit && (
                <div className="flex items-center justify-between mt-4 pt-3 border-t border-gray-200">
                  <span className="text-xs text-gray-500">
                    Showing {usageOffset + 1}-{Math.min(usageOffset + usageLimit, usageTotal)} of {usageTotal}
                  </span>
                  <div className="flex gap-2">
                    <button
                      onClick={handlePrevPage}
                      disabled={usageOffset === 0 || loadingUsage}
                      className={`px-3 py-1 text-sm rounded border ${
                        usageOffset === 0 || loadingUsage
                          ? 'bg-gray-100 text-gray-400 border-gray-200 cursor-not-allowed'
                          : 'bg-white text-gray-700 border-gray-300 hover:bg-gray-50'
                      }`}
                    >
                      Previous
                    </button>
                    <button
                      onClick={handleNextPage}
                      disabled={usageOffset + usageLimit >= usageTotal || loadingUsage}
                      className={`px-3 py-1 text-sm rounded border ${
                        usageOffset + usageLimit >= usageTotal || loadingUsage
                          ? 'bg-gray-100 text-gray-400 border-gray-200 cursor-not-allowed'
                          : 'bg-white text-gray-700 border-gray-300 hover:bg-gray-50'
                      }`}
                    >
                      Next
                    </button>
                  </div>
                </div>
              )}
            </div>
          )}
        </div>
      )}

      {/* Attestation Modal */}
      {attestationModal && (
        <div
          className="fixed inset-0 bg-black bg-opacity-50 flex items-center justify-center z-50 p-4"
          onClick={() => setAttestationModal(null)}
        >
          <div
            className="bg-white rounded-lg shadow-xl max-w-4xl w-full max-h-[90vh] overflow-y-auto"
            onClick={(e) => e.stopPropagation()}
          >
            <div className="p-6">
              <div className="flex justify-between items-center mb-4">
                <div className="flex items-center gap-3">
                  <h2 className="text-2xl font-bold text-gray-900">
                    TEE Attestation - HTTPS Call
                  </h2>
                  <span className="inline-flex rounded-full px-2 text-xs font-semibold leading-5 bg-orange-100 text-orange-800">
                    HTTPS
                  </span>
                </div>
                <button
                  onClick={() => setAttestationModal(null)}
                  className="text-gray-400 hover:text-gray-600"
                >
                  <svg className="w-6 h-6" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M6 18L18 6M6 6l12 12" />
                  </svg>
                </button>
              </div>

              <p className="text-sm text-gray-500 mb-4 font-mono">
                Job ID: #{attestationModal.jobId}
              </p>

              {attestationModal.loading && (
                <div className="flex justify-center items-center py-12">
                  <div className="animate-spin rounded-full h-12 w-12 border-b-2 border-[#cc6600]"></div>
                </div>
              )}

              {attestationModal.error && (
                <div className="bg-red-50 border border-red-200 rounded-md p-4 mb-4">
                  <p className="text-red-800">{attestationModal.error}</p>
                </div>
              )}

              {attestationModal.attestation && (
                <AttestationView
                  attestation={attestationModal.attestation}
                  network={network}
                  showHelp={false}
                  onToggleHelp={() => {}}
                  isModal={true}
                />
              )}
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
