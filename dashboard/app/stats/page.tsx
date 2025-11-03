'use client';

import { useEffect, useState } from 'react';
import { fetchStats, ExecutionStats, fetchPopularRepos, PopularRepo, fetchPricing, PricingConfig } from '@/lib/api';

export default function StatsPage() {
  const [stats, setStats] = useState<ExecutionStats | null>(null);
  const [repos, setRepos] = useState<PopularRepo[]>([]);
  const [pricing, setPricing] = useState<PricingConfig | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    loadData();
    const interval = setInterval(loadData, 30000); // Refresh every 30s
    return () => clearInterval(interval);
  }, []);

  const loadData = async () => {
    try {
      const [statsData, reposData, pricingData] = await Promise.all([
        fetchStats(),
        fetchPopularRepos(),
        fetchPricing(),
      ]);
      setStats(statsData);
      setRepos(reposData);
      setPricing(pricingData);
      setError(null);
    } catch (err) {
      setError('Failed to load statistics');
      console.error(err);
    } finally {
      setLoading(false);
    }
  };

  const formatYoctoNEAR = (yocto: string) => {
    const near = parseFloat(yocto) / 1e24;
    // Use adaptive precision for very small values
    if (near === 0) return '0 NEAR';
    if (near < 0.000001) return near.toFixed(12) + ' NEAR';
    if (near < 0.001) return near.toFixed(9) + ' NEAR';
    return near.toFixed(6) + ' NEAR';
  };

  const formatInstructions = (instructions: number) => {
    if (instructions > 1e12) return (instructions / 1e12).toFixed(2) + 'T';
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

  if (error || !stats) {
    return (
      <div className="bg-red-50 border border-red-200 rounded-md p-4">
        <p className="text-red-800">{error}</p>
      </div>
    );
  }

  // Platform success rate = requests handled without infrastructure errors
  // We're not at fault if user has wrong repo/secrets - only count infrastructure failures
  const platformSuccesses = stats.total_executions - stats.failed_executions;
  const successRate = stats.total_executions > 0
    ? ((platformSuccesses / stats.total_executions) * 100).toFixed(1)
    : '0';

  return (
    <div>
      <div className="sm:flex sm:items-center">
        <div className="sm:flex-auto">
          <h1 className="text-3xl font-bold text-gray-900">System Statistics</h1>
          <p className="mt-2 text-sm text-gray-700">
            Real-time metrics from the OutLayer platform
          </p>
        </div>
      </div>

      <div className="mt-8 grid grid-cols-1 gap-5 sm:grid-cols-2 lg:grid-cols-4">
        {/* Total Executions */}
        <div className="bg-white overflow-hidden shadow rounded-lg">
          <div className="p-5">
            <div className="flex items-center">
              <div className="flex-shrink-0">
                <svg
                  className="h-6 w-6 text-gray-400"
                  fill="none"
                  viewBox="0 0 24 24"
                  stroke="currentColor"
                >
                  <path
                    strokeLinecap="round"
                    strokeLinejoin="round"
                    strokeWidth={2}
                    d="M9 12h6m-6 4h6m2 5H7a2 2 0 01-2-2V5a2 2 0 012-2h5.586a1 1 0 01.707.293l5.414 5.414a1 1 0 01.293.707V19a2 2 0 01-2 2z"
                  />
                </svg>
              </div>
              <div className="ml-5 w-0 flex-1">
                <dl>
                  <dt className="text-sm font-medium text-gray-500 truncate">Total Executions</dt>
                  <dd className="flex items-baseline">
                    <div className="text-2xl font-semibold text-gray-900">
                      {stats.total_executions.toLocaleString()}
                    </div>
                  </dd>
                </dl>
              </div>
            </div>
          </div>
        </div>

        {/* Success Rate */}
        <div className="bg-white overflow-hidden shadow rounded-lg">
          <div className="p-5">
            <div className="flex items-center">
              <div className="flex-shrink-0">
                <svg
                  className="h-6 w-6 text-green-400"
                  fill="none"
                  viewBox="0 0 24 24"
                  stroke="currentColor"
                >
                  <path
                    strokeLinecap="round"
                    strokeLinejoin="round"
                    strokeWidth={2}
                    d="M9 12l2 2 4-4m6 2a9 9 0 11-18 0 9 9 0 0118 0z"
                  />
                </svg>
              </div>
              <div className="ml-5 w-0 flex-1">
                <dl>
                  <dt className="text-sm font-medium text-gray-500 truncate">Platform Success Rate</dt>
                  <dd className="flex items-baseline">
                    <div className="text-2xl font-semibold text-gray-900">{successRate}%</div>
                    <div className="ml-2 text-sm text-gray-500">
                      {platformSuccesses}/{stats.total_executions}
                    </div>
                  </dd>
                </dl>
              </div>
            </div>
          </div>
        </div>

        {/* Active Workers - Clickable Link */}
        <a
          href="/workers"
          className="bg-white overflow-hidden shadow rounded-lg hover:shadow-md transition-shadow cursor-pointer"
        >
          <div className="p-5">
            <div className="flex items-center">
              <div className="flex-shrink-0">
                <svg
                  className="h-6 w-6 text-blue-400"
                  fill="none"
                  viewBox="0 0 24 24"
                  stroke="currentColor"
                >
                  <path
                    strokeLinecap="round"
                    strokeLinejoin="round"
                    strokeWidth={2}
                    d="M9 3v2m6-2v2M9 19v2m6-2v2M5 9H3m2 6H3m18-6h-2m2 6h-2M7 19h10a2 2 0 002-2V7a2 2 0 00-2-2H7a2 2 0 00-2 2v10a2 2 0 002 2zM9 9h6v6H9V9z"
                  />
                </svg>
              </div>
              <div className="ml-5 w-0 flex-1">
                <dl>
                  <dt className="text-sm font-medium text-gray-500 truncate">Active Workers</dt>
                  <dd className="flex items-baseline">
                    <div className="text-2xl font-semibold text-blue-600 hover:text-blue-800">
                      {stats.active_workers}
                    </div>
                  </dd>
                </dl>
              </div>
              <div className="ml-2">
                <svg className="h-5 w-5 text-gray-400" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                  <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M9 5l7 7-7 7" />
                </svg>
              </div>
            </div>
          </div>
        </a>

        {/* Unique Users */}
        <div className="bg-white overflow-hidden shadow rounded-lg">
          <div className="p-5">
            <div className="flex items-center">
              <div className="flex-shrink-0">
                <svg
                  className="h-6 w-6 text-purple-400"
                  fill="none"
                  viewBox="0 0 24 24"
                  stroke="currentColor"
                >
                  <path
                    strokeLinecap="round"
                    strokeLinejoin="round"
                    strokeWidth={2}
                    d="M17 20h5v-2a3 3 0 00-5.356-1.857M17 20H7m10 0v-2c0-.656-.126-1.283-.356-1.857M7 20H2v-2a3 3 0 015.356-1.857M7 20v-2c0-.656.126-1.283.356-1.857m0 0a5.002 5.002 0 019.288 0M15 7a3 3 0 11-6 0 3 3 0 016 0zm6 3a2 2 0 11-4 0 2 2 0 014 0zM7 10a2 2 0 11-4 0 2 2 0 014 0z"
                  />
                </svg>
              </div>
              <div className="ml-5 w-0 flex-1">
                <dl>
                  <dt className="text-sm font-medium text-gray-500 truncate">Unique Users</dt>
                  <dd className="flex items-baseline">
                    <div className="text-2xl font-semibold text-gray-900">{stats.unique_users}</div>
                  </dd>
                </dl>
              </div>
            </div>
          </div>
        </div>
      </div>

      {/* Additional Stats */}
      <div className="mt-8 grid grid-cols-1 gap-5 sm:grid-cols-2 lg:grid-cols-3">
        <div className="bg-white overflow-hidden shadow rounded-lg">
          <div className="px-4 py-5 sm:p-6">
            <dt className="text-sm font-medium text-gray-500 truncate">Total Instructions Executed</dt>
            <dd className="mt-1 text-3xl font-semibold text-gray-900">
              {formatInstructions(stats.total_instructions_used)}
            </dd>
          </div>
        </div>

        <div className="bg-white overflow-hidden shadow rounded-lg">
          <div className="px-4 py-5 sm:p-6">
            <dt className="text-sm font-medium text-gray-500 truncate">Avg Execution Time</dt>
            <dd className="mt-1 text-3xl font-semibold text-gray-900">
              {stats.average_execution_time_ms} ms
            </dd>
          </div>
        </div>

        <div className="bg-white overflow-hidden shadow rounded-lg">
          <div className="px-4 py-5 sm:p-6">
            <dt className="text-sm font-medium text-gray-500 truncate">Total NEAR Paid</dt>
            <dd className="mt-1 text-3xl font-semibold text-gray-900">
              {formatYoctoNEAR(stats.total_near_paid_yocto)}
            </dd>
          </div>
        </div>
      </div>

      {/* Execution Results Breakdown */}
      <div className="mt-8">
        <h2 className="text-2xl font-bold text-gray-900 mb-4">Execution Results Breakdown</h2>
        <div className="grid grid-cols-1 gap-4 sm:grid-cols-2 lg:grid-cols-4">
          {/* Successful */}
          <div className="bg-green-50 border border-green-200 rounded-lg p-4">
            <div className="flex items-center justify-between">
              <div>
                <p className="text-sm font-medium text-green-800">Successful</p>
                <p className="mt-1 text-2xl font-semibold text-green-900">{stats.successful_executions}</p>
              </div>
              <svg className="h-8 w-8 text-green-400" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M9 12l2 2 4-4m6 2a9 9 0 11-18 0 9 9 0 0118 0z" />
              </svg>
            </div>
          </div>

          {/* Infrastructure Errors (Platform Responsibility) */}
          <div className="bg-red-50 border border-red-200 rounded-lg p-4">
            <div className="flex items-center justify-between">
              <div>
                <p className="text-sm font-medium text-red-800">Infrastructure Errors</p>
                <p className="mt-1 text-2xl font-semibold text-red-900">{stats.failed_executions}</p>
              </div>
              <svg className="h-8 w-8 text-red-400" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-3L13.732 4c-.77-1.333-2.694-1.333-3.464 0L3.34 16c-.77 1.333.192 3 1.732 3z" />
              </svg>
            </div>
          </div>

          {/* Access Denied (User Error) */}
          <div className="bg-yellow-50 border border-yellow-200 rounded-lg p-4">
            <div className="flex items-center justify-between">
              <div>
                <p className="text-sm font-medium text-yellow-800">Access Denied</p>
                <p className="mt-1 text-2xl font-semibold text-yellow-900">{stats.access_denied_executions}</p>
              </div>
              <svg className="h-8 w-8 text-yellow-400" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M12 15v2m-6 4h12a2 2 0 002-2v-6a2 2 0 00-2-2H6a2 2 0 00-2 2v6a2 2 0 002 2zm10-10V7a4 4 0 00-8 0v4h8z" />
              </svg>
            </div>
          </div>

          {/* Compilation Failed (User Error) */}
          <div className="bg-orange-50 border border-orange-200 rounded-lg p-4">
            <div className="flex items-center justify-between">
              <div>
                <p className="text-sm font-medium text-orange-800">Compilation Failed</p>
                <p className="mt-1 text-2xl font-semibold text-orange-900">{stats.compilation_failed_executions}</p>
              </div>
              <svg className="h-8 w-8 text-orange-400" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M10 20l4-16m4 4l4 4-4 4M6 16l-4-4 4-4" />
              </svg>
            </div>
          </div>

          {/* Execution Failed (User Error) */}
          <div className="bg-purple-50 border border-purple-200 rounded-lg p-4">
            <div className="flex items-center justify-between">
              <div>
                <p className="text-sm font-medium text-purple-800">Execution Failed</p>
                <p className="mt-1 text-2xl font-semibold text-purple-900">{stats.execution_failed_executions}</p>
              </div>
              <svg className="h-8 w-8 text-purple-400" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M6 18L18 6M6 6l12 12" />
              </svg>
            </div>
          </div>

          {/* Insufficient Payment (User Error) */}
          <div className="bg-pink-50 border border-pink-200 rounded-lg p-4">
            <div className="flex items-center justify-between">
              <div>
                <p className="text-sm font-medium text-pink-800">Insufficient Payment</p>
                <p className="mt-1 text-2xl font-semibold text-pink-900">{stats.insufficient_payment_executions}</p>
              </div>
              <svg className="h-8 w-8 text-pink-400" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M12 8c-1.657 0-3 .895-3 2s1.343 2 3 2 3 .895 3 2-1.343 2-3 2m0-8c1.11 0 2.08.402 2.599 1M12 8V7m0 1v8m0 0v1m0-1c-1.11 0-2.08-.402-2.599-1" />
              </svg>
            </div>
          </div>

          {/* Custom Errors (User Error) */}
          <div className="bg-gray-50 border border-gray-200 rounded-lg p-4">
            <div className="flex items-center justify-between">
              <div>
                <p className="text-sm font-medium text-gray-800">Custom Errors</p>
                <p className="mt-1 text-2xl font-semibold text-gray-900">{stats.custom_executions}</p>
              </div>
              <svg className="h-8 w-8 text-gray-400" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M7 8h10M7 12h4m1 8l-4-4H5a2 2 0 01-2-2V6a2 2 0 012-2h14a2 2 0 012 2v8a2 2 0 01-2 2h-3l-4 4z" />
              </svg>
            </div>
          </div>
        </div>

        <div className="mt-4 text-sm text-gray-600">
          <p><strong>Platform Success Rate:</strong> Excludes user errors (access denied, compilation failed, execution failed, insufficient payment, custom). Only infrastructure errors count as platform failures.</p>
        </div>
      </div>

      {/* Popular Repositories */}
      {repos.filter(repo => repo.successful_executions > 0).length > 0 && (
        <div className="mt-8">
          <h2 className="text-2xl font-bold text-gray-900 mb-4">Popular Repositories</h2>
          <div className="bg-white shadow overflow-hidden sm:rounded-lg">
            <table className="min-w-full divide-y divide-gray-200">
              <thead className="bg-gray-50">
                <tr>
                  <th className="px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider">
                    Repository
                  </th>
                  <th className="px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider">
                    Executions
                  </th>
                  <th className="px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider">
                    Success Rate
                  </th>
                  <th className="px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider">
                    Last Commit
                  </th>
                </tr>
              </thead>
              <tbody className="bg-white divide-y divide-gray-200">
                {repos
                  .filter(repo => repo.successful_executions > 0)
                  .map((repo, idx) => {
                    // Actual executions = total - compilation failures (compilation failures don't count as executions for repo stats)
                    const actualExecutions = repo.total_executions - repo.compilation_failed_executions;
                    // Platform success rate = requests handled without infrastructure errors
                    const platformSuccesses = actualExecutions - repo.failed_executions;
                    const successRate = actualExecutions > 0
                      ? ((platformSuccesses / actualExecutions) * 100).toFixed(1)
                      : '0';
                    return (
                      <tr key={idx}>
                        <td className="px-6 py-4 whitespace-nowrap">
                          <a
                            href={repo.github_repo}
                            target="_blank"
                            rel="noopener noreferrer"
                            className="text-blue-600 hover:text-blue-800 hover:underline"
                          >
                            {repo.github_repo.replace(/^https?:\/\/(www\.)?github\.com\//, '')}
                          </a>
                        </td>
                        <td className="px-6 py-4 whitespace-nowrap text-sm text-gray-900">
                          {actualExecutions}
                        </td>
                        <td className="px-6 py-4 whitespace-nowrap">
                          <span className={`inline-flex rounded-full px-2 text-xs font-semibold leading-5 ${
                            parseFloat(successRate) > 90 ? 'bg-green-100 text-green-800' : 'bg-yellow-100 text-yellow-800'
                          }`}>
                            {successRate}%
                          </span>
                        </td>
                        <td className="px-6 py-4 whitespace-nowrap text-sm text-gray-500 font-mono">
                          {repo.last_commit ? repo.last_commit.substring(0, 8) : 'N/A'}
                        </td>
                      </tr>
                    );
                  })}
              </tbody>
            </table>
          </div>
        </div>
      )}

      {/* Pricing Configuration */}
      {pricing && (
        <div className="mt-8">
          <h2 className="text-2xl font-bold text-gray-900 mb-4">Pricing & Limits</h2>
          <div className="bg-white shadow overflow-hidden sm:rounded-lg">
            <div className="px-4 py-5 sm:p-6">
              <h3 className="text-lg font-medium text-gray-900 mb-4">Pricing Rates</h3>
              <dl className="grid grid-cols-1 gap-x-4 gap-y-6 sm:grid-cols-2">
                <div className="sm:col-span-1">
                  <dt className="text-sm font-medium text-gray-500">Base Fee</dt>
                  <dd className="mt-1 text-sm text-gray-900">{formatYoctoNEAR(pricing.base_fee)}</dd>
                </div>
                <div className="sm:col-span-1">
                  <dt className="text-sm font-medium text-gray-500">Per Million Instructions</dt>
                  <dd className="mt-1 text-sm text-gray-900">{formatYoctoNEAR(pricing.per_instruction_fee)}</dd>
                </div>
                <div className="sm:col-span-1">
                  <dt className="text-sm font-medium text-gray-500">Per Millisecond (Execution)</dt>
                  <dd className="mt-1 text-sm text-gray-900">{formatYoctoNEAR(pricing.per_ms_fee)}</dd>
                </div>
                <div className="sm:col-span-1">
                  <dt className="text-sm font-medium text-gray-500">Per Millisecond (Compilation)</dt>
                  <dd className="mt-1 text-sm text-gray-900">{formatYoctoNEAR(pricing.per_compile_ms_fee)}</dd>
                </div>
              </dl>

              <h3 className="text-lg font-medium text-gray-900 mt-6 mb-4">Resource Limits</h3>
              <dl className="grid grid-cols-1 gap-x-4 gap-y-6 sm:grid-cols-3">
                <div className="sm:col-span-1">
                  <dt className="text-sm font-medium text-gray-500">Max Instructions</dt>
                  <dd className="mt-1 text-sm text-gray-900">{formatInstructions(pricing.max_instructions)}</dd>
                  <dd className="mt-1 text-xs text-gray-500">Hard cap on WASM instructions per execution</dd>
                </div>
                <div className="sm:col-span-1">
                  <dt className="text-sm font-medium text-gray-500">Max Execution Time</dt>
                  <dd className="mt-1 text-sm text-gray-900">{pricing.max_execution_seconds} seconds</dd>
                  <dd className="mt-1 text-xs text-gray-500">Hard cap on execution duration</dd>
                </div>
                <div className="sm:col-span-1">
                  <dt className="text-sm font-medium text-gray-500">Max Compilation Time</dt>
                  <dd className="mt-1 text-sm text-gray-900">{pricing.max_compilation_seconds} seconds</dd>
                  <dd className="mt-1 text-xs text-gray-500">Hard cap on compilation duration</dd>
                </div>
              </dl>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
