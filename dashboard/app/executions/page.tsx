'use client';

import { useEffect, useState } from 'react';
import { fetchExecutions, ExecutionHistoryEntry } from '@/lib/api';
import { getTransactionUrl } from '@/lib/explorer';
import Link from 'next/link';

export default function ExecutionsPage() {
  const [executions, setExecutions] = useState<ExecutionHistoryEntry[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    loadExecutions();
  }, []);

  const loadExecutions = async () => {
    try {
      const data = await fetchExecutions(50, 0);
      setExecutions(data);
      setError(null);
    } catch (err) {
      setError('Failed to load executions');
      console.error(err);
    } finally {
      setLoading(false);
    }
  };

  const formatYoctoNEAR = (yocto: string | null) => {
    if (!yocto) return 'N/A';
    const near = parseFloat(yocto) / 1e24;
    return near.toFixed(6) + ' NEAR';
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
          <h1 className="text-3xl font-bold text-gray-900">Execution History</h1>
          <p className="mt-2 text-sm text-gray-700">
            Browse all off-chain execution requests and their results
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
                    <th className="px-3 py-3.5 text-left text-sm font-semibold text-gray-900">Request ID</th>
                    <th className="px-3 py-3.5 text-left text-sm font-semibold text-gray-900">Status</th>
                    <th className="px-3 py-3.5 text-left text-sm font-semibold text-gray-900">GitHub Repo</th>
                    <th className="px-3 py-3.5 text-left text-sm font-semibold text-gray-900">User</th>
                    <th className="px-3 py-3.5 text-left text-sm font-semibold text-gray-900">Worker</th>
                    <th className="px-3 py-3.5 text-left text-sm font-semibold text-gray-900">Instructions</th>
                    <th className="px-3 py-3.5 text-left text-sm font-semibold text-gray-900">Time (ms)</th>
                    <th className="px-3 py-3.5 text-left text-sm font-semibold text-gray-900">Payment</th>
                    <th className="px-3 py-3.5 text-left text-sm font-semibold text-gray-900">TX</th>
                    <th className="px-3 py-3.5 text-left text-sm font-semibold text-gray-900">Created</th>
                  </tr>
                </thead>
                <tbody className="divide-y divide-gray-200 bg-white">
                  {executions.length === 0 ? (
                    <tr>
                      <td colSpan={10} className="px-3 py-8 text-center text-sm text-gray-500">
                        No executions found
                      </td>
                    </tr>
                  ) : (
                    executions.map((execution) => (
                      <tr key={execution.id}>
                        <td className="whitespace-nowrap px-3 py-4 text-sm text-gray-900 font-mono">
                          #{execution.request_id}
                        </td>
                        <td className="whitespace-nowrap px-3 py-4 text-sm">
                          <span
                            className={`inline-flex rounded-full px-2 text-xs font-semibold leading-5 ${
                              execution.success
                                ? 'bg-green-100 text-green-800'
                                : 'bg-red-100 text-red-800'
                            }`}
                          >
                            {execution.success ? 'Success' : 'Failed'}
                          </span>
                        </td>
                        <td className="whitespace-nowrap px-3 py-4 text-sm text-gray-500">
                          {execution.github_repo ? (
                            <a
                              href={`${execution.github_repo}/tree/${execution.github_commit}`}
                              target="_blank"
                              rel="noopener noreferrer"
                              className="text-blue-600 hover:text-blue-800 hover:underline"
                              title={`${execution.github_repo} @ ${execution.github_commit}`}
                            >
                              {execution.github_repo.replace(/^https?:\/\/(www\.)?github\.com\//, '')}
                            </a>
                          ) : (
                            '-'
                          )}
                        </td>
                        <td className="whitespace-nowrap px-3 py-4 text-sm text-gray-500 font-mono">
                          {execution.user_account_id
                            ? execution.user_account_id.substring(0, 12) + '...'
                            : 'N/A'}
                        </td>
                        <td className="whitespace-nowrap px-3 py-4 text-sm text-gray-500 font-mono">
                          {execution.worker_id.substring(0, 8)}
                        </td>
                        <td className="whitespace-nowrap px-3 py-4 text-sm text-gray-900">
                          {formatInstructions(execution.instructions_used)}
                        </td>
                        <td className="whitespace-nowrap px-3 py-4 text-sm text-gray-900">
                          {execution.execution_time_ms}
                        </td>
                        <td className="whitespace-nowrap px-3 py-4 text-sm text-gray-900">
                          {formatYoctoNEAR(execution.near_payment_yocto)}
                        </td>
                        <td className="whitespace-nowrap px-3 py-4 text-sm">
                          {execution.resolve_tx_id ? (
                            <a
                              href={getTransactionUrl(execution.resolve_tx_id)}
                              target="_blank"
                              rel="noopener noreferrer"
                              className="text-blue-600 hover:text-blue-800 hover:underline"
                              title={execution.resolve_tx_id}
                            >
                              {execution.resolve_tx_id.substring(0, 8)}...
                            </a>
                          ) : (
                            '-'
                          )}
                        </td>
                        <td className="whitespace-nowrap px-3 py-4 text-sm text-gray-500">
                          {new Date(execution.created_at).toLocaleString()}
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
