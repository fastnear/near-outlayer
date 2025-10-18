'use client';

import { useEffect, useState } from 'react';
import { fetchJobs, JobHistoryEntry } from '@/lib/api';
import { getTransactionUrl } from '@/lib/explorer';
import Link from 'next/link';

export default function JobsPage() {
  const [jobs, setJobs] = useState<JobHistoryEntry[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

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
                    <th className="px-3 py-3.5 text-left text-sm font-semibold text-gray-900">Job ID</th>
                    <th className="px-3 py-3.5 text-left text-sm font-semibold text-gray-900">Request ID</th>
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
                      <td colSpan={12} className="px-3 py-8 text-center text-sm text-gray-500">
                        No jobs found
                      </td>
                    </tr>
                  ) : (
                    jobs.map((job) => (
                      <tr key={job.id}>
                        <td className="whitespace-nowrap px-3 py-4 text-sm text-gray-500 font-mono">
                          {job.job_id || 'N/A'}
                        </td>
                        <td className="whitespace-nowrap px-3 py-4 text-sm text-gray-900 font-mono">
                          #{job.request_id}
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
                              job.success
                                ? 'bg-green-100 text-green-800'
                                : 'bg-red-100 text-red-800'
                            }`}
                          >
                            {job.success ? 'Done' : 'Failed'}
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
                              href={getTransactionUrl(job.transaction_hash)}
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
                        <td className="whitespace-nowrap px-3 py-4 text-sm text-gray-500">
                          {new Date(job.created_at).toLocaleString(undefined, {
                            year: 'numeric',
                            month: '2-digit',
                            day: '2-digit',
                            hour: '2-digit',
                            minute: '2-digit',
                            second: '2-digit',
                            hour12: false
                          })}
                        </td>
                      </tr>
                    ))
                  )}
                </tbody>
              </table>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}
