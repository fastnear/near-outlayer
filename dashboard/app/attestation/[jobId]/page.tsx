'use client';

import { useEffect, useState } from 'react';
import { useParams, useSearchParams } from 'next/navigation';
import { AttestationResponse } from '@/lib/api';
import { useNearWallet } from '@/contexts/NearWalletContext';
import AttestationView from '@/components/AttestationView';
import Link from 'next/link';

export default function AttestationPage() {
  const params = useParams();
  const searchParams = useSearchParams();
  const jobId = params?.jobId as string;
  const { network } = useNearWallet();

  // Get network from URL query parameter
  const urlNetwork = searchParams?.get('network') as 'mainnet' | 'testnet' | null;
  const isNetworkMismatch = urlNetwork && urlNetwork !== network;
  const displayNetwork = urlNetwork || network;

  const [attestation, setAttestation] = useState<AttestationResponse | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [showHelp, setShowHelp] = useState(false);

  useEffect(() => {
    if (jobId) {
      loadAttestation(parseInt(jobId));
    }
  }, [jobId]);

  const loadAttestation = async (id: number) => {
    const requireApiKey = process.env.NEXT_PUBLIC_REQUIRE_ATTESTATION_API_KEY === 'true';
    const apiKey = process.env.NEXT_PUBLIC_COORDINATOR_API_KEY || '';

    if (requireApiKey && !apiKey) {
      setError('API key not configured');
      setLoading(false);
      return;
    }

    try {
      const { fetchAttestation } = await import('@/lib/api');
      const data = await fetchAttestation(id, apiKey || 'not-required');

      if (!data) {
        setError('No attestation found for this job');
        setLoading(false);
        return;
      }

      setAttestation(data);
      setError(null);
    } catch (err: unknown) {
      console.error('Failed to load attestation:', err);
      const errorMessage = err instanceof Error ? err.message : 'Failed to load attestation';
      setError(errorMessage);
    } finally {
      setLoading(false);
    }
  };

  if (loading) {
    return (
      <div className="flex justify-center items-center min-h-[600px]">
        <div className="text-center">
          <div className="animate-spin rounded-full h-12 w-12 border-b-2 border-blue-600 mx-auto mb-4"></div>
          <p className="text-gray-600">Loading attestation #{jobId}...</p>
        </div>
      </div>
    );
  }

  if (error) {
    return (
      <div className="max-w-4xl mx-auto mt-8">
        <div className="bg-red-50 border border-red-200 rounded-lg p-6">
          <h2 className="text-xl font-bold text-red-800 mb-2">Error</h2>
          <p className="text-red-700">{error}</p>
          <Link
            href="/executions"
            className="inline-block mt-4 px-4 py-2 bg-red-600 text-white rounded hover:bg-red-700"
          >
            Back to Executions
          </Link>
        </div>
      </div>
    );
  }

  if (!attestation) {
    return (
      <div className="max-w-4xl mx-auto mt-8">
        <div className="bg-yellow-50 border border-yellow-200 rounded-lg p-6">
          <h2 className="text-xl font-bold text-yellow-800 mb-2">Not Found</h2>
          <p className="text-yellow-700">No attestation data for job #{jobId}</p>
          <Link
            href="/executions"
            className="inline-block mt-4 px-4 py-2 bg-yellow-600 text-white rounded hover:bg-yellow-700"
          >
            Back to Executions
          </Link>
        </div>
      </div>
    );
  }

  return (
    <div className="max-w-5xl mx-auto">
      {/* Network Mismatch Warning */}
      {isNetworkMismatch && (
        <div className="mb-6 bg-amber-50 border border-amber-200 rounded-lg p-4">
          <div className="flex items-start">
            <span className="text-amber-600 text-xl mr-3">⚠️</span>
            <div>
              <h3 className="text-amber-900 font-semibold mb-1">Network Mismatch</h3>
              <p className="text-amber-800 text-sm">
                This attestation is from <strong>{urlNetwork}</strong>, but your wallet is connected to <strong>{network}</strong>.
                <br />
                You&apos;re viewing the attestation in read-only mode. Any transactions will be sent to {network}.
              </p>
            </div>
          </div>
        </div>
      )}

      {/* Header */}
      <div className="bg-white rounded-lg shadow-sm border border-gray-200 p-6 mb-6">
        <div className="flex justify-between items-start">
          <div>
            <h1 className="text-3xl font-bold text-gray-900 mb-2">
              TEE Attestation
            </h1>
            <p className="text-gray-600">
              Job #{attestation.task_id} • {attestation.task_type === 'compile' ? 'Compilation' : 'Execution'}
              {urlNetwork && <span className="ml-2 text-sm bg-gray-100 px-2 py-1 rounded">({urlNetwork})</span>}
            </p>
          </div>
          <Link
            href="/executions"
            className="px-4 py-2 bg-gray-100 hover:bg-gray-200 text-gray-700 font-medium rounded"
          >
            All Executions
          </Link>
        </div>
      </div>

      {/* Attestation Component */}
      <div className="bg-white rounded-lg shadow-sm border border-gray-200 p-6">
        <AttestationView
          attestation={attestation}
          network={displayNetwork}
          showHelp={showHelp}
          onToggleHelp={() => setShowHelp(!showHelp)}
          isModal={false}
        />
      </div>

      {/* Share Section */}
      <div className="mt-6 bg-gray-50 rounded-lg p-4 text-center">
        <p className="text-sm text-gray-600 mb-3">Share this verifiable attestation:</p>
        <div className="flex justify-center gap-3">
          <button
            onClick={() => {
              // Include network parameter in the URL
              const baseUrl = window.location.origin + window.location.pathname;
              const shareUrl = urlNetwork
                ? baseUrl + `?network=${urlNetwork}`
                : baseUrl + `?network=${network}`;
              navigator.clipboard.writeText(shareUrl);
              alert('Link copied to clipboard!');
            }}
            className="px-4 py-2 bg-blue-600 hover:bg-blue-700 text-white font-medium rounded"
          >
            📋 Copy Link
          </button>
        </div>
      </div>
    </div>
  );
}