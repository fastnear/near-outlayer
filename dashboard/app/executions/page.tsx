'use client';

import { useEffect, useState } from 'react';
import { fetchJobs, JobHistoryEntry, AttestationResponse } from '@/lib/api';
import { getTransactionUrl } from '@/lib/explorer';
import { useNearWallet } from '@/contexts/NearWalletContext';

export default function JobsPage() {
  const { network } = useNearWallet();
  const [jobs, setJobs] = useState<JobHistoryEntry[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [expandedJobId, setExpandedJobId] = useState<number | null>(null);
  const [attestationModal, setAttestationModal] = useState<{ jobId: number; attestation: AttestationResponse | null; loading: boolean; error: string | null } | null>(null);

  useEffect(() => {
    loadJobs();
  }, []);

  const loadJobs = async () => {
    try {
      const data = await fetchJobs(50, 0);
      setJobs(data);
      setError(null);
    } catch (err) {
      setError('Failed to load jobs');
      console.error(err);
    } finally {
      setLoading(false);
    }
  };

  // Verify TDX quote by extracting RTMR3 and comparing with worker_measurement
  const verifyTdxQuote = (tdxQuoteBase64: string, expectedRtmr3: string): { valid: boolean; extractedRtmr3: string | null; error: string | null } => {
    try {
      // Decode base64 to bytes
      const binaryString = atob(tdxQuoteBase64);
      const bytes = new Uint8Array(binaryString.length);
      for (let i = 0; i < binaryString.length; i++) {
        bytes[i] = binaryString.charCodeAt(i);
      }

      // TDX Quote v4 structure: RTMR3 is at offset 256, 48 bytes
      const RTMR3_OFFSET = 256;
      const RTMR3_SIZE = 48;

      if (bytes.length < RTMR3_OFFSET + RTMR3_SIZE) {
        return { valid: false, extractedRtmr3: null, error: 'Quote too short' };
      }

      // Extract RTMR3 bytes
      const rtmr3Bytes = bytes.slice(RTMR3_OFFSET, RTMR3_OFFSET + RTMR3_SIZE);

      // Convert to hex string
      const extractedRtmr3 = Array.from(rtmr3Bytes)
        .map(b => b.toString(16).padStart(2, '0'))
        .join('');

      // Compare with expected value (case-insensitive)
      const valid = extractedRtmr3.toLowerCase() === expectedRtmr3.toLowerCase();

      return { valid, extractedRtmr3, error: null };
    } catch (err) {
      return { valid: false, extractedRtmr3: null, error: err instanceof Error ? err.message : 'Verification failed' };
    }
  };

  const loadAttestation = async (jobId: number) => {
    const requireApiKey = process.env.NEXT_PUBLIC_REQUIRE_ATTESTATION_API_KEY === 'true';
    const apiKey = process.env.NEXT_PUBLIC_COORDINATOR_API_KEY || '';

    if (requireApiKey && !apiKey) {
      setAttestationModal({
        jobId: jobId,
        attestation: null,
        loading: false,
        error: 'API key not configured. Please set NEXT_PUBLIC_COORDINATOR_API_KEY in .env'
      });
      return;
    }

    setAttestationModal({ jobId: jobId, attestation: null, loading: true, error: null });

    try {
      const { fetchAttestation } = await import('@/lib/api');
      const data = await fetchAttestation(jobId, apiKey || 'not-required');

      if (!data) {
        setAttestationModal({
          jobId: jobId,
          attestation: null,
          loading: false,
          error: 'No attestation found for this job'
        });
        return;
      }

      setAttestationModal({ jobId: jobId, attestation: data, loading: false, error: null });
    } catch (err: unknown) {
      console.error('Failed to load attestation:', err);
      const errorMessage = err instanceof Error
        ? err.message
        : (typeof err === 'object' && err !== null && 'response' in err && typeof err.response === 'object' && err.response !== null && 'data' in err.response && typeof err.response.data === 'object' && err.response.data !== null && 'error' in err.response.data)
          ? String(err.response.data.error)
          : 'Failed to load attestation';
      setAttestationModal({
        jobId: jobId,
        attestation: null,
        loading: false,
        error: errorMessage
      });
    }
  };

  const formatYoctoNEAR = (yocto: string | null) => {
    if (!yocto) return 'N/A';
    const near = parseFloat(yocto) / 1e24;
    return near.toFixed(6) + ' Ⓝ';
  };

  // Calculate payment for display: for execute jobs, subtract compile_cost if exists
  const getDisplayPayment = (job: JobHistoryEntry) => {
    if (job.job_type === 'execute' && job.actual_cost_yocto && job.compile_cost_yocto) {
      // Execute cost already includes compile cost, so subtract it
      const totalCost = parseFloat(job.actual_cost_yocto);
      const compileCost = parseFloat(job.compile_cost_yocto);
      const executeCost = totalCost - compileCost;
      return executeCost.toString();
    }
    // For compile jobs or execute without compile_cost, use as-is
    return job.compile_cost_yocto || job.actual_cost_yocto || job.near_payment_yocto;
  };

  const formatInstructions = (instructions: number | null) => {
    if (!instructions) return 'N/A';
    if (instructions > 1e9) return (instructions / 1e9).toFixed(2) + 'B';
    if (instructions > 1e6) return (instructions / 1e6).toFixed(2) + 'M';
    if (instructions > 1e3) return (instructions / 1e3).toFixed(2) + 'K';
    return instructions.toString();
  };

  // Remove trailing zeros from RTMR3 for display (cosmetic only)
  const formatRtmr3 = (rtmr3: string): string => {
    // Remove trailing zeros but keep at least some non-zero part
    return rtmr3.replace(/0+$/, '') || rtmr3;
  };

  // Get status badge color and text based on job status
  const getStatusDisplay = (status: string | null, success: boolean) => {
    const actualStatus = status || (success ? 'completed' : 'failed');

    switch (actualStatus) {
      case 'completed':
        return { color: 'bg-green-100 text-green-800', text: 'Done' };
      case 'access_denied':
        return { color: 'bg-yellow-100 text-yellow-800', text: 'Access Denied' };
      case 'compilation_failed':
        return { color: 'bg-orange-100 text-orange-800', text: 'Compilation Failed' };
      case 'execution_failed':
        return { color: 'bg-red-100 text-red-800', text: 'Execution Failed' };
      case 'insufficient_payment':
        return { color: 'bg-purple-100 text-purple-800', text: 'Insufficient Payment' };
      case 'custom':
        return { color: 'bg-blue-100 text-blue-800', text: 'Invalid Request' };
      case 'failed':
        return { color: 'bg-red-100 text-red-800', text: 'Infrastructure Error' };
      default:
        return { color: 'bg-gray-100 text-gray-800', text: actualStatus.replace('_', ' ') };
    }
  };

  // Format timestamp: show only time if today, otherwise full date
  const formatTimestamp = (timestamp: string) => {
    const date = new Date(timestamp);
    const now = new Date();
    const isToday = date.toDateString() === now.toDateString();

    if (isToday) {
      // Show only time for today
      return date.toLocaleTimeString(undefined, {
        hour: '2-digit',
        minute: '2-digit',
        second: '2-digit',
        hour12: false,
      });
    } else {
      // Show full date for other days
      return date.toLocaleString(undefined, {
        year: 'numeric',
        month: '2-digit',
        day: '2-digit',
        hour: '2-digit',
        minute: '2-digit',
        hour12: false,
      });
    }
  };

  if (loading) {
    return (
      <div className="flex justify-center items-center min-h-[400px]">
        <div className="animate-spin rounded-full h-12 w-12 border-b-2 border-blue-600"></div>
      </div>
    );
  }

  if (error) {
    return (
      <div className="bg-red-50 border border-red-200 rounded-md p-4">
        <p className="text-red-800">{error}</p>
      </div>
    );
  }

  return (
    <div>
      <div className="sm:flex sm:items-center">
        <div className="sm:flex-auto">
          <h1 className="text-3xl font-bold text-gray-900">Job History</h1>
          <p className="mt-2 text-sm text-gray-700">
            Browse all compilation and execution jobs
          </p>
        </div>
      </div>

      <div className="mt-8 flex flex-col">
        <div className="-my-2 -mx-4 overflow-x-auto sm:-mx-6 lg:-mx-8">
          <div className="inline-block min-w-full py-2 align-middle md:px-6 lg:px-8">
            <div className="overflow-hidden shadow ring-1 ring-black ring-opacity-5 md:rounded-lg">
              <table className="min-w-full divide-y divide-gray-300">
                <thead className="bg-gray-50">
                  <tr>
                    <th className="px-3 py-3.5 text-left text-sm font-semibold text-gray-900">ID</th>
                    <th className="px-3 py-3.5 text-left text-sm font-semibold text-gray-900">Type</th>
                    <th className="px-3 py-3.5 text-left text-sm font-semibold text-gray-900">Status</th>
                    <th className="px-3 py-3.5 text-left text-sm font-semibold text-gray-900">Worker</th>
                    <th className="px-3 py-3.5 text-left text-sm font-semibold text-gray-900">GitHub Repo</th>
                    <th className="px-3 py-3.5 text-left text-sm font-semibold text-gray-900">User</th>
                    <th className="px-3 py-3.5 text-left text-sm font-semibold text-gray-900">Time (ms)</th>
                    <th className="px-3 py-3.5 text-left text-sm font-semibold text-gray-900" title="Instructions">Fuel</th>
                    <th className="px-3 py-3.5 text-left text-sm font-semibold text-gray-900" title="In NEAR tokens">Payment</th>
                    <th className="px-3 py-3.5 text-left text-sm font-semibold text-gray-900">TX</th>
                    <th className="px-3 py-3.5 text-left text-sm font-semibold text-gray-900">Created</th>
                  </tr>
                </thead>
                <tbody className="divide-y divide-gray-200 bg-white">
                  {jobs.length === 0 ? (
                    <tr>
                      <td colSpan={11} className="px-3 py-8 text-center text-sm text-gray-500">
                        No jobs found
                      </td>
                    </tr>
                  ) : (
                    jobs.map((job) => {
                      const isExpanded = expandedJobId === job.id;
                      const hasErrorDetails = job.error_details && job.error_details.trim().length > 0;

                      return (
                        <>
                          <tr key={job.id}>
                            <td
                              className="whitespace-nowrap px-3 py-4 text-sm font-mono"
                              title={job.job_id ? `Job ID: ${job.job_id} - Click to view TEE attestation` : 'Click to view TEE attestation'}
                            >
                              <button
                                onClick={() => job.job_id && loadAttestation(job.job_id)}
                                className="text-blue-600 hover:text-blue-800 hover:underline"
                                disabled={!job.job_id}
                              >
                                #{job.id}
                              </button>
                            </td>
                            <td className="whitespace-nowrap px-3 py-4 text-sm">
                              <span
                                className={`inline-flex rounded-full px-2 text-xs font-semibold leading-5 ${
                                  job.job_type === 'compile'
                                    ? 'bg-blue-100 text-blue-800'
                                    : 'bg-purple-100 text-purple-800'
                                }`}
                              >
                                {job.job_type || 'N/A'}
                              </span>
                            </td>
                            <td className="whitespace-nowrap px-3 py-4 text-sm">
                              <span
                                className={`inline-flex rounded-full px-2 text-xs font-semibold leading-5 ${
                                  getStatusDisplay(job.status, job.success).color
                                } ${hasErrorDetails ? 'cursor-pointer hover:opacity-80' : ''}`}
                                onClick={() => hasErrorDetails && setExpandedJobId(isExpanded ? null : job.id)}
                                title={hasErrorDetails ? 'Click to show error details' : undefined}
                              >
                                {getStatusDisplay(job.status, job.success).text}
                                {hasErrorDetails && (
                                  <span className="ml-1">{isExpanded ? '▼' : '▶'}</span>
                                )}
                              </span>
                            </td>
                        <td className="px-3 py-4 text-sm text-gray-500 font-mono">
                          <div className="max-w-[100px] truncate" title={job.worker_id || 'N/A'}>
                            {job.worker_id || 'N/A'}
                          </div>
                        </td>
                        <td className="px-3 py-4 text-sm text-gray-500">
                          {job.github_repo ? (
                            <a
                              href={`${job.github_repo}/tree/${job.github_commit}`}
                              target="_blank"
                              rel="noopener noreferrer"
                              className="text-blue-600 hover:text-blue-800 hover:underline max-w-[100px] truncate block"
                              title={`${job.github_repo} @ ${job.github_commit}`}
                            >
                              {job.github_repo.replace(/^https?:\/\/(www\.)?github\.com\//, '')}
                            </a>
                          ) : (
                            '-'
                          )}
                        </td>
                        <td className="whitespace-nowrap px-3 py-4 text-sm text-gray-500 font-mono">
                          {job.user_account_id
                            ? job.user_account_id.substring(0, 12) + '...'
                            : 'N/A'}
                        </td>
                        <td className="whitespace-nowrap px-3 py-4 text-sm text-gray-900">
                          {job.compile_time_ms && job.execution_time_ms
                            ? `${job.compile_time_ms}ms + ${job.execution_time_ms}ms`
                            : job.compile_time_ms
                            ? `${job.compile_time_ms}ms`
                            : job.execution_time_ms
                            ? `${job.execution_time_ms}ms`
                            : 'N/A'}
                        </td>
                        <td className="whitespace-nowrap px-3 py-4 text-sm text-gray-900">
                          {formatInstructions(job.instructions_used)}
                        </td>
                        <td className="whitespace-nowrap px-3 py-4 text-sm text-gray-900">
                          {formatYoctoNEAR(getDisplayPayment(job))}
                        </td>
                        <td className="whitespace-nowrap px-3 py-4 text-sm">
                          {job.transaction_hash ? (
                            <a
                              href={getTransactionUrl(job.transaction_hash, network)}
                              target="_blank"
                              rel="noopener noreferrer"
                              className="text-blue-600 hover:text-blue-800 hover:underline"
                              title={job.transaction_hash}
                            >
                              {job.transaction_hash.substring(0, 8)}...
                            </a>
                          ) : (
                            '-'
                          )}
                        </td>
                            <td
                              className="whitespace-nowrap px-3 py-4 text-sm text-gray-500"
                              title={new Date(job.created_at).toLocaleString()}
                            >
                              {formatTimestamp(job.created_at)}
                            </td>
                          </tr>
                          {/* Error details row - only shown when expanded */}
                          {isExpanded && hasErrorDetails && (
                            <tr key={`${job.id}-details`}>
                              <td colSpan={11} className="px-3 py-4 bg-gray-50">
                                <div className="text-sm">
                                  <span className="font-semibold text-gray-700">Error Details:</span>
                                  <pre className="mt-2 p-3 bg-white border border-gray-200 rounded text-xs overflow-x-auto text-red-600">
                                    {job.error_details}
                                  </pre>
                                </div>
                              </td>
                            </tr>
                          )}
                        </>
                      );
                    })
                  )}
                </tbody>
              </table>
            </div>
          </div>
        </div>
      </div>

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
                <h2 className="text-2xl font-bold text-gray-900">
                  TEE Attestation - Job #{attestationModal.jobId}
                </h2>
                <button
                  onClick={() => setAttestationModal(null)}
                  className="text-gray-400 hover:text-gray-600"
                >
                  <svg className="w-6 h-6" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M6 18L18 6M6 6l12 12" />
                  </svg>
                </button>
              </div>

              {attestationModal.loading && (
                <div className="flex justify-center items-center py-12">
                  <div className="animate-spin rounded-full h-12 w-12 border-b-2 border-blue-600"></div>
                </div>
              )}

              {attestationModal.error && (
                <div className="bg-red-50 border border-red-200 rounded-md p-4 mb-4">
                  <p className="text-red-800">{attestationModal.error}</p>
                </div>
              )}

              {attestationModal.attestation && (() => {
                const verification = verifyTdxQuote(
                  attestationModal.attestation.tdx_quote,
                  attestationModal.attestation.worker_measurement
                );
                return (
                <div className="space-y-4">
                  {verification.valid ? (
                    <div className="bg-green-50 border border-green-200 rounded-md p-4">
                      <p className="text-green-800 font-semibold">
                        ✓ TDX Quote Verified: RTMR3 matches worker measurement
                      </p>
                      <p className="text-green-700 text-sm mt-1">
                        This execution was attested by Intel TDX TEE. The quote is cryptographically valid.
                      </p>
                    </div>
                  ) : (
                    <div className="bg-red-50 border border-red-200 rounded-md p-4">
                      <p className="text-red-800 font-semibold">
                        ⚠️ TDX Quote Verification Failed
                      </p>
                      <p className="text-red-700 text-sm mt-1">
                        {verification.error || 'RTMR3 mismatch'}
                      </p>
                      {verification.extractedRtmr3 && (
                        <details className="mt-2 text-xs">
                          <summary className="cursor-pointer text-red-600">Show details</summary>
                          <div className="mt-2 font-mono">
                            <div>Expected: {formatRtmr3(attestationModal.attestation.worker_measurement)}</div>
                            <div>Extracted: {formatRtmr3(verification.extractedRtmr3)}</div>
                          </div>
                        </details>
                      )}
                    </div>
                  )}

                  <div className="grid grid-cols-2 gap-4">
                    <div>
                      <label className="block text-sm font-semibold text-gray-700 mb-1">Task ID</label>
                      <div className="bg-gray-50 p-2 rounded border font-mono text-sm">
                        {attestationModal.attestation.task_id}
                      </div>
                    </div>
                    <div>
                      <label className="block text-sm font-semibold text-gray-700 mb-1">Task Type</label>
                      <div className="bg-gray-50 p-2 rounded border font-mono text-sm">
                        {attestationModal.attestation.task_type}
                      </div>
                    </div>
                  </div>

                  <div>
                    <label className="block text-sm font-semibold text-gray-700 mb-1">
                      Worker Measurement (RTMR3)
                    </label>
                    <div className="bg-gray-50 p-2 rounded border font-mono text-xs break-all">
                      {formatRtmr3(attestationModal.attestation.worker_measurement)}
                    </div>
                    <p className="text-xs text-gray-500 mt-1">
                      This is the cryptographic hash of the TEE environment (RTMR3 from TDX quote)
                    </p>
                  </div>

                  {attestationModal.attestation.repo_url && (
                    <div>
                      <label className="block text-sm font-semibold text-gray-700 mb-1">Source Code</label>
                      <div className="bg-gray-50 p-2 rounded border text-sm">
                        <a
                          href={`${attestationModal.attestation.repo_url}/tree/${attestationModal.attestation.commit_hash}`}
                          target="_blank"
                          rel="noopener noreferrer"
                          className="text-blue-600 hover:underline"
                        >
                          {attestationModal.attestation.repo_url} @ {attestationModal.attestation.commit_hash}
                        </a>
                      </div>
                    </div>
                  )}

                  <div className="grid grid-cols-2 gap-4">
                    {attestationModal.attestation.wasm_hash && (
                      <div>
                        <label className="block text-sm font-semibold text-gray-700 mb-1">WASM Hash</label>
                        <div className="bg-gray-50 p-2 rounded border font-mono text-xs break-all">
                          {attestationModal.attestation.wasm_hash}
                        </div>
                      </div>
                    )}
                    {attestationModal.attestation.input_hash && (
                      <div>
                        <label className="block text-sm font-semibold text-gray-700 mb-1">Input Hash</label>
                        <div className="bg-gray-50 p-2 rounded border font-mono text-xs break-all">
                          {attestationModal.attestation.input_hash}
                        </div>
                      </div>
                    )}
                  </div>

                  <div>
                    <label className="block text-sm font-semibold text-gray-700 mb-1">Output Hash</label>
                    <div className="bg-gray-50 p-2 rounded border font-mono text-xs break-all">
                      {attestationModal.attestation.output_hash}
                    </div>
                  </div>

                  {attestationModal.attestation.transaction_hash && (
                    <div>
                      <label className="block text-sm font-semibold text-gray-700 mb-1">NEAR Transaction</label>
                      <div className="bg-gray-50 p-2 rounded border">
                        <a
                          href={getTransactionUrl(attestationModal.attestation.transaction_hash, network)}
                          target="_blank"
                          rel="noopener noreferrer"
                          className="text-blue-600 hover:underline font-mono text-sm"
                        >
                          {attestationModal.attestation.transaction_hash}
                        </a>
                      </div>
                    </div>
                  )}

                  <div>
                    <div className="flex justify-between items-center mb-1">
                      <label className="block text-sm font-semibold text-gray-700">
                        TDX Quote (Raw, Base64)
                      </label>
                      <button
                        onClick={() => {
                          const result = verifyTdxQuote(
                            attestationModal.attestation!.tdx_quote,
                            attestationModal.attestation!.worker_measurement
                          );
                          alert(
                            result.valid
                              ? `✓ Quote Verified!\n\nExtracted RTMR3 from quote (offset 256, 48 bytes):\n${formatRtmr3(result.extractedRtmr3!)}\n\nMatches stored worker_measurement:\n${formatRtmr3(attestationModal.attestation!.worker_measurement)}\n\nThis proves the quote is authentic and was generated by the expected TEE environment.`
                              : `⚠️ Verification Failed!\n\n${result.error || 'RTMR3 mismatch'}\n\nExpected: ${formatRtmr3(attestationModal.attestation!.worker_measurement)}\nExtracted: ${result.extractedRtmr3 ? formatRtmr3(result.extractedRtmr3) : 'N/A'}`
                          );
                        }}
                        className="px-3 py-1 bg-blue-600 hover:bg-blue-700 text-white text-xs font-medium rounded"
                      >
                        Verify Quote
                      </button>
                    </div>
                    <textarea
                      readOnly
                      value={attestationModal.attestation.tdx_quote}
                      className="w-full h-32 bg-gray-50 p-2 rounded border font-mono text-xs"
                    />
                    <p className="text-xs text-gray-500 mt-1">
                      This is the full Intel TDX attestation quote. Click &quot;Verify Quote&quot; to extract and verify RTMR3.
                    </p>
                  </div>
                </div>
                );
              })()}
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
