'use client';

import { useEffect, useState, useCallback } from 'react';
import { fetchJobs, JobHistoryEntry, AttestationResponse, fetchAttestation } from '@/lib/api';
import { getTransactionUrl } from '@/lib/explorer';
import { useNearWallet } from '@/contexts/NearWalletContext';
import AttestationView from '@/components/AttestationView';
import Link from 'next/link';

const STORAGE_KEY = 'executions-table-settings';

interface TableSettings {
  showNear: boolean;
  showHttps: boolean;
  visibleColumns: {
    id: boolean;
    type: boolean;
    status: boolean;
    worker: boolean;
    source: boolean;
    user: boolean;
    time: boolean;
    fuel: boolean;
    payment: boolean;
    tx: boolean;
    created: boolean;
    tee: boolean;
  };
}

const DEFAULT_SETTINGS: TableSettings = {
  showNear: true,
  showHttps: false,
  visibleColumns: {
    id: true,
    type: true,
    status: true,
    worker: false,
    source: true,
    user: true,
    time: true,
    fuel: false,
    payment: true,
    tx: true,
    created: true,
    tee: true,
  },
};

const COLUMN_LABELS: Record<keyof TableSettings['visibleColumns'], string> = {
  id: 'ID',
  type: 'Type',
  status: 'Status',
  worker: 'Worker',
  source: 'Source',
  user: 'User',
  time: 'Time',
  fuel: 'Fuel',
  payment: 'Payment',
  tx: 'TX',
  created: 'Created',
  tee: 'TEE',
};

export default function JobsPage() {
  const { network } = useNearWallet();
  const [jobs, setJobs] = useState<JobHistoryEntry[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [expandedJobId, setExpandedJobId] = useState<number | null>(null);
  const [attestationModal, setAttestationModal] = useState<{
    jobId: number;
    isHttpsCall: boolean;
    attestation: AttestationResponse | null;
    loading: boolean;
    error: string | null
  } | null>(null);
  const [showAttestationHelp, setShowAttestationHelp] = useState(false);
  const [showColumnSettings, setShowColumnSettings] = useState(false);

  // Load settings from localStorage
  const [settings, setSettings] = useState<TableSettings>(() => {
    if (typeof window === 'undefined') return DEFAULT_SETTINGS;
    try {
      const saved = localStorage.getItem(STORAGE_KEY);
      if (saved) {
        const parsed = JSON.parse(saved);
        // Merge with defaults to handle new columns
        return {
          ...DEFAULT_SETTINGS,
          ...parsed,
          visibleColumns: {
            ...DEFAULT_SETTINGS.visibleColumns,
            ...parsed.visibleColumns,
          },
        };
      }
    } catch {
      // Ignore parse errors
    }
    return DEFAULT_SETTINGS;
  });

  // Save settings to localStorage
  useEffect(() => {
    if (typeof window !== 'undefined') {
      localStorage.setItem(STORAGE_KEY, JSON.stringify(settings));
    }
  }, [settings]);

  // Compute source filter for API
  const getSourceFilter = useCallback((): 'near' | 'https' | undefined => {
    if (settings.showNear && settings.showHttps) return undefined; // Both = no filter
    if (settings.showNear) return 'near';
    if (settings.showHttps) return 'https';
    return 'near'; // Fallback - should not happen
  }, [settings.showNear, settings.showHttps]);

  const loadJobs = useCallback(async () => {
    setLoading(true);
    try {
      const source = getSourceFilter();
      const data = await fetchJobs(50, 0, undefined, source);
      setJobs(data);
      setError(null);
    } catch (err) {
      setError('Failed to load jobs');
      console.error(err);
    } finally {
      setLoading(false);
    }
  }, [getSourceFilter]);

  // Load jobs when filter changes
  useEffect(() => {
    loadJobs();
  }, [loadJobs]);

  // Toggle handlers that prevent both from being unchecked
  const toggleNear = () => {
    setSettings((prev) => {
      // If trying to uncheck NEAR and HTTPS is already unchecked, don't allow
      if (prev.showNear && !prev.showHttps) return prev;
      return { ...prev, showNear: !prev.showNear };
    });
  };

  const toggleHttps = () => {
    setSettings((prev) => {
      // If trying to uncheck HTTPS and NEAR is already unchecked, don't allow
      if (prev.showHttps && !prev.showNear) return prev;
      return { ...prev, showHttps: !prev.showHttps };
    });
  };

  const loadAttestation = async (job: JobHistoryEntry) => {
    if (!job.job_id) {
      return; // No job_id available
    }

    setAttestationModal({
      jobId: job.job_id,
      isHttpsCall: job.is_https_call,
      attestation: null,
      loading: true,
      error: null
    });

    try {
      // Always use job_id to fetch attestation (works for both NEAR and HTTPS calls)
      const data = await fetchAttestation(job.job_id);

      if (!data) {
        setAttestationModal({
          jobId: job.job_id,
          isHttpsCall: job.is_https_call,
          attestation: null,
          loading: false,
          error: 'No attestation found for this job'
        });
        return;
      }

      setAttestationModal({
        jobId: job.job_id,
        isHttpsCall: job.is_https_call,
        attestation: data,
        loading: false,
        error: null
      });
    } catch (err: unknown) {
      console.error('Failed to load attestation:', err);
      const errorMessage = err instanceof Error
        ? err.message
        : (typeof err === 'object' && err !== null && 'response' in err && typeof err.response === 'object' && err.response !== null && 'data' in err.response && typeof err.response.data === 'object' && err.response.data !== null && 'error' in err.response.data)
          ? String(err.response.data.error)
          : 'Failed to load attestation';
      setAttestationModal({
        jobId: job.job_id,
        isHttpsCall: job.is_https_call,
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

  // Format USD from stablecoin minimal units (6 decimals)
  const formatUsd = (amount: string | null) => {
    if (!amount) return 'N/A';
    const usd = parseFloat(amount) / 1e6;
    return '$' + usd.toFixed(4);
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

  const toggleColumn = (column: keyof TableSettings['visibleColumns']) => {
    setSettings((prev) => ({
      ...prev,
      visibleColumns: {
        ...prev.visibleColumns,
        [column]: !prev.visibleColumns[column],
      },
    }));
  };

  // Effective column visibility: hide TX when only HTTPS is shown (no transactions)
  const httpsOnly = settings.showHttps && !settings.showNear;
  const effectiveColumns = {
    ...settings.visibleColumns,
    tx: settings.visibleColumns.tx && !httpsOnly,
  };

  // Count visible columns for colspan
  const visibleColumnCount = Object.values(effectiveColumns).filter(Boolean).length;

  if (error && !loading) {
    return (
      <div className="bg-red-50 border border-red-200 rounded-md p-4">
        <p className="text-red-800">{error}</p>
      </div>
    );
  }

  return (
    <div>
      <div className="sm:flex sm:items-center sm:justify-between">
        <div className="sm:flex-auto">
          <h1 className="text-3xl font-bold text-gray-900">Job History</h1>
          <p className="mt-2 text-sm text-gray-700">
            Browse all compilation and execution jobs
          </p>
        </div>

        {/* Controls */}
        <div className="mt-4 sm:mt-0 flex items-center gap-4">
          {/* Source filter checkboxes */}
          <div className="flex items-center gap-4">
            <label className="flex items-center gap-2 cursor-pointer">
              <input
                type="checkbox"
                checked={settings.showNear}
                onChange={toggleNear}
                disabled={settings.showNear && !settings.showHttps}
                className="w-4 h-4 text-green-600 rounded border-gray-300 focus:ring-green-500 disabled:opacity-50"
              />
              <span className={`text-sm font-medium ${settings.showNear ? 'text-green-700' : 'text-gray-500'}`}>
                NEAR
              </span>
            </label>
            <label className="flex items-center gap-2 cursor-pointer">
              <input
                type="checkbox"
                checked={settings.showHttps}
                onChange={toggleHttps}
                disabled={settings.showHttps && !settings.showNear}
                className="w-4 h-4 text-orange-500 rounded border-gray-300 focus:ring-orange-500 disabled:opacity-50"
              />
              <span className={`text-sm font-medium ${settings.showHttps ? 'text-orange-600' : 'text-gray-500'}`}>
                HTTPS
              </span>
            </label>
          </div>

          {/* Column settings dropdown */}
          <div className="relative">
            <button
              onClick={() => setShowColumnSettings(!showColumnSettings)}
              className="px-3 py-1.5 text-sm font-medium text-gray-600 bg-gray-100 hover:bg-gray-200 rounded-lg flex items-center gap-1"
              title="Column settings"
            >
              <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M12 6V4m0 2a2 2 0 100 4m0-4a2 2 0 110 4m-6 8a2 2 0 100-4m0 4a2 2 0 110-4m0 4v2m0-6V4m6 6v10m6-2a2 2 0 100-4m0 4a2 2 0 110-4m0 4v2m0-6V4" />
              </svg>
              Columns
            </button>

            {showColumnSettings && (
              <>
                <div
                  className="fixed inset-0 z-10"
                  onClick={() => setShowColumnSettings(false)}
                />
                <div className="absolute right-0 mt-2 w-48 bg-white rounded-lg shadow-lg border border-gray-200 z-20 py-2">
                  {(Object.keys(COLUMN_LABELS) as Array<keyof TableSettings['visibleColumns']>).map((col) => (
                    <label
                      key={col}
                      className="flex items-center px-4 py-2 hover:bg-gray-50 cursor-pointer"
                    >
                      <input
                        type="checkbox"
                        checked={settings.visibleColumns[col]}
                        onChange={() => toggleColumn(col)}
                        className="w-4 h-4 text-blue-600 rounded border-gray-300 focus:ring-blue-500"
                      />
                      <span className="ml-2 text-sm text-gray-700">{COLUMN_LABELS[col]}</span>
                    </label>
                  ))}
                </div>
              </>
            )}
          </div>
        </div>
      </div>

      <div className="mt-8 flex flex-col">
        <div className="-my-2 -mx-4 overflow-x-auto sm:-mx-6 lg:-mx-8">
          <div className="inline-block min-w-full py-2 align-middle md:px-6 lg:px-8">
            <div className="overflow-hidden shadow ring-1 ring-black ring-opacity-5 md:rounded-lg">
              <table className="min-w-full divide-y divide-gray-300">
                <thead className="bg-gray-50">
                  <tr>
                    {effectiveColumns.id && (
                      <th className="px-3 py-3.5 text-left text-sm font-semibold text-gray-900">ID</th>
                    )}
                    {effectiveColumns.tee && (
                      <th className="px-3 py-3.5 text-left text-sm font-semibold text-gray-900">TEE</th>
                    )}
                    {effectiveColumns.type && (
                      <th className="px-3 py-3.5 text-left text-sm font-semibold text-gray-900">Type</th>
                    )}
                    {effectiveColumns.status && (
                      <th className="px-3 py-3.5 text-left text-sm font-semibold text-gray-900">Status</th>
                    )}
                    {effectiveColumns.worker && (
                      <th className="px-3 py-3.5 text-left text-sm font-semibold text-gray-900">Worker</th>
                    )}
                    {effectiveColumns.source && (
                      <th className="px-3 py-3.5 text-left text-sm font-semibold text-gray-900">Source</th>
                    )}
                    {effectiveColumns.user && (
                      <th className="px-3 py-3.5 text-left text-sm font-semibold text-gray-900">User</th>
                    )}
                    {effectiveColumns.time && (
                      <th className="px-3 py-3.5 text-left text-sm font-semibold text-gray-900">Time (ms)</th>
                    )}
                    {effectiveColumns.fuel && (
                      <th className="px-3 py-3.5 text-left text-sm font-semibold text-gray-900" title="Instructions">Fuel</th>
                    )}
                    {effectiveColumns.payment && (
                      <th className="px-3 py-3.5 text-left text-sm font-semibold text-gray-900" title="In NEAR tokens">Payment</th>
                    )}
                    {effectiveColumns.tx && (
                      <th className="px-3 py-3.5 text-left text-sm font-semibold text-gray-900">TX</th>
                    )}
                    {effectiveColumns.created && (
                      <th className="px-3 py-3.5 text-left text-sm font-semibold text-gray-900">Created</th>
                    )}
                  </tr>
                </thead>
                <tbody className="divide-y divide-gray-200 bg-white">
                  {loading ? (
                    <tr>
                      <td colSpan={visibleColumnCount} className="px-3 py-8 text-center">
                        <div className="flex justify-center">
                          <div className="animate-spin rounded-full h-8 w-8 border-b-2 border-blue-600"></div>
                        </div>
                      </td>
                    </tr>
                  ) : jobs.length === 0 ? (
                    <tr>
                      <td colSpan={visibleColumnCount} className="px-3 py-8 text-center text-sm text-gray-500">
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
                            {effectiveColumns.id && (
                              <td className="whitespace-nowrap px-3 py-4 text-sm font-mono text-gray-900">
                                #{job.id}
                              </td>
                            )}
                            {effectiveColumns.tee && (
                              <td className="whitespace-nowrap px-3 py-4 text-sm">
                                {job.job_id ? (
                                  <button
                                    onClick={() => loadAttestation(job)}
                                    className="text-gray-400 hover:text-blue-600 cursor-pointer transition-colors"
                                    title="View attestation report"
                                  >
                                    <svg className="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                                      <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M9 12l2 2 4-4m5.618-4.016A11.955 11.955 0 0112 2.944a11.955 11.955 0 01-8.618 3.04A12.02 12.02 0 003 9c0 5.591 3.824 10.29 9 11.622 5.176-1.332 9-6.03 9-11.622 0-1.042-.133-2.052-.382-3.016z" />
                                    </svg>
                                  </button>
                                ) : (
                                  <span className="text-gray-200">
                                    <svg className="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                                      <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M9 12l2 2 4-4m5.618-4.016A11.955 11.955 0 0112 2.944a11.955 11.955 0 01-8.618 3.04A12.02 12.02 0 003 9c0 5.591 3.824 10.29 9 11.622 5.176-1.332 9-6.03 9-11.622 0-1.042-.133-2.052-.382-3.016z" />
                                    </svg>
                                  </span>
                                )}
                              </td>
                            )}
                            {effectiveColumns.type && (
                              <td className="whitespace-nowrap px-3 py-4 text-sm">
                                <div className="flex items-center gap-1">
                                  <span
                                    className={`inline-flex rounded-full px-2 text-xs font-semibold leading-5 ${
                                      job.job_type === 'compile'
                                        ? 'bg-blue-100 text-blue-800'
                                        : 'bg-purple-100 text-purple-800'
                                    }`}
                                  >
                                    {job.job_type || 'N/A'}
                                  </span>
                                  <span
                                    className={`inline-flex rounded-full px-2 text-xs font-semibold leading-5 ${
                                      job.is_https_call
                                        ? 'bg-orange-100 text-orange-800'
                                        : 'bg-green-100 text-green-800'
                                    }`}
                                    title={job.is_https_call
                                      ? `HTTPS API call (call_id: ${job.call_id})`
                                      : `NEAR blockchain call (tx: ${job.transaction_hash || 'N/A'})`}
                                  >
                                    {job.is_https_call ? 'HTTPS' : 'NEAR'}
                                  </span>
                                </div>
                              </td>
                            )}
                            {effectiveColumns.status && (
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
                            )}
                            {effectiveColumns.worker && (
                              <td className="px-3 py-4 text-sm text-gray-500 font-mono">
                                <div className="max-w-[100px] truncate" title={job.worker_id || 'N/A'}>
                                  {job.worker_id ? job.worker_id.split('-').pop() : 'N/A'}
                                </div>
                              </td>
                            )}
                            {effectiveColumns.source && (
                              <td className="px-3 py-4 text-sm text-gray-500">
                                {job.project_id ? (
                                  <span
                                    className="max-w-[120px] truncate block"
                                    title={job.project_id}
                                  >
                                    {job.project_id.split('/').pop() || job.project_id}
                                  </span>
                                ) : job.github_repo ? (
                                  <a
                                    href={`${job.github_repo}/tree/${job.github_commit}`}
                                    target="_blank"
                                    rel="noopener noreferrer"
                                    className="text-blue-600 hover:text-blue-800 hover:underline max-w-[120px] truncate block"
                                    title={`${job.github_repo} @ ${job.github_commit}`}
                                  >
                                    {job.github_repo.replace(/^https?:\/\/(www\.)?github\.com\//, '')}
                                  </a>
                                ) : (
                                  '-'
                                )}
                              </td>
                            )}
                            {effectiveColumns.user && (
                              <td className="whitespace-nowrap px-3 py-4 text-sm text-gray-500 font-mono">
                                {job.user_account_id
                                  ? job.user_account_id.substring(0, 12) + '...'
                                  : 'N/A'}
                              </td>
                            )}
                            {effectiveColumns.time && (
                              <td className="whitespace-nowrap px-3 py-4 text-sm text-gray-900">
                                {job.compile_time_ms && job.execution_time_ms
                                  ? `${job.compile_time_ms}ms + ${job.execution_time_ms}ms`
                                  : job.compile_time_ms
                                  ? `${job.compile_time_ms}ms`
                                  : job.execution_time_ms
                                  ? `${job.execution_time_ms}ms`
                                  : 'N/A'}
                              </td>
                            )}
                            {effectiveColumns.fuel && (
                              <td className="whitespace-nowrap px-3 py-4 text-sm text-gray-900">
                                {job.job_type === 'compile' ? '-' : formatInstructions(job.instructions_used)}
                              </td>
                            )}
                            {effectiveColumns.payment && (
                              <td className="whitespace-nowrap px-3 py-4 text-sm text-gray-900">
                                {job.is_https_call
                                  ? formatUsd(job.compute_cost_usd)
                                  : formatYoctoNEAR(getDisplayPayment(job))}
                              </td>
                            )}
                            {effectiveColumns.tx && (
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
                            )}
                            {effectiveColumns.created && (
                              <td
                                className="whitespace-nowrap px-3 py-4 text-sm text-gray-500"
                                title={new Date(job.created_at).toLocaleString()}
                              >
                                {formatTimestamp(job.created_at)}
                              </td>
                            )}
                          </tr>
                          {/* Error details row - only shown when expanded */}
                          {isExpanded && hasErrorDetails && (
                            <tr key={`${job.id}-details`}>
                              <td colSpan={visibleColumnCount} className="px-3 py-4 bg-gray-50">
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
          onClick={() => {
            setAttestationModal(null);
            setShowAttestationHelp(false);
          }}
        >
          <div
            className="bg-white rounded-lg shadow-xl max-w-4xl w-full max-h-[90vh] overflow-y-auto"
            onClick={(e) => e.stopPropagation()}
          >
            <div className="p-6">
              <div className="flex justify-between items-center mb-4">
                <div className="flex items-center gap-3">
                  <h2 className="text-2xl font-bold text-gray-900">
                    TEE Attestation - {attestationModal.isHttpsCall ? 'HTTPS' : 'NEAR'} Job #{attestationModal.jobId}
                  </h2>
                  <span
                    className={`inline-flex rounded-full px-2 text-xs font-semibold leading-5 ${
                      attestationModal.isHttpsCall
                        ? 'bg-orange-100 text-orange-800'
                        : 'bg-green-100 text-green-800'
                    }`}
                  >
                    {attestationModal.isHttpsCall ? 'HTTPS' : 'NEAR'}
                  </span>
                  {!attestationModal.isHttpsCall && (
                    <Link
                      href={`/attestation/${attestationModal.jobId}?network=${network}`}
                      target="_blank"
                      className="px-3 py-1 bg-green-100 hover:bg-green-200 text-green-700 text-sm font-medium rounded"
                      title="Open in new tab"
                    >
                      Direct Link
                    </Link>
                  )}
                </div>
                <button
                  onClick={() => {
                    setAttestationModal(null);
                    setShowAttestationHelp(false);
                  }}
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

              {attestationModal.attestation && (
                <AttestationView
                  attestation={attestationModal.attestation}
                  network={network}
                  showHelp={showAttestationHelp}
                  onToggleHelp={() => setShowAttestationHelp(!showAttestationHelp)}
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
