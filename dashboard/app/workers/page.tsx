'use client';

import { useEffect, useState } from 'react';
import { fetchWorkers, WorkerInfo } from '@/lib/api';

export default function WorkersPage() {
  const [workers, setWorkers] = useState<WorkerInfo[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    loadWorkers();
    const interval = setInterval(loadWorkers, 10000); // Refresh every 10s
    return () => clearInterval(interval);
  }, []);

  const loadWorkers = async () => {
    try {
      const data = await fetchWorkers();
      setWorkers(data);
      setError(null);
    } catch (err) {
      setError('Failed to load workers');
      console.error(err);
    } finally {
      setLoading(false);
    }
  };

  const formatUptime = (seconds: number | null) => {
    if (!seconds) return 'N/A';
    const hours = Math.floor(seconds / 3600);
    const minutes = Math.floor((seconds % 3600) / 60);
    return `${hours}h ${minutes}m`;
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
          <h1 className="text-3xl font-bold text-gray-900">Workers</h1>
          <p className="mt-2 text-sm text-gray-700">
            Active workers executing off-chain computation tasks
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
                    <th className="px-3 py-3.5 text-left text-sm font-semibold text-gray-900">Worker ID</th>
                    <th className="px-3 py-3.5 text-left text-sm font-semibold text-gray-900">Completed</th>
                    <th className="px-3 py-3.5 text-left text-sm font-semibold text-gray-900">Failed</th>
                    <th className="px-3 py-3.5 text-left text-sm font-semibold text-gray-900">Uptime</th>
                    <th className="px-3 py-3.5 text-left text-sm font-semibold text-gray-900">Last Heartbeat</th>
                  </tr>
                </thead>
                <tbody className="divide-y divide-gray-200 bg-white">
                  {workers.length === 0 ? (
                    <tr>
                      <td colSpan={5} className="px-3 py-8 text-center text-sm text-gray-500">
                        No workers found
                      </td>
                    </tr>
                  ) : (
                    workers.map((worker) => {
                      // Extract app_id from worker_id (format: network-type-app_id)
                      const parts = worker.worker_id.split('-');
                      const appId = parts.length >= 3 ? parts.slice(2).join('-') : null;
                      const isPhalaAppId = appId && /^[a-f0-9]{40}$/i.test(appId);

                      return (
                      <tr key={worker.worker_id}>
                        <td className="whitespace-nowrap px-3 py-4 text-sm text-gray-900 font-mono">
                          {isPhalaAppId ? (
                            <a
                              href={`https://trust.phala.com/app/${appId}?selected=app-code`}
                              target="_blank"
                              rel="noopener noreferrer"
                              className="text-blue-600 hover:text-blue-800 hover:underline"
                              title="Verify TEE attestation on Phala Trust"
                            >
                              {worker.worker_id}
                            </a>
                          ) : (
                            worker.worker_id
                          )}
                        </td>
                        <td className="whitespace-nowrap px-3 py-4 text-sm text-gray-900">
                          {worker.total_tasks_completed}
                        </td>
                        <td className="whitespace-nowrap px-3 py-4 text-sm text-gray-900">
                          {worker.total_tasks_failed}
                        </td>
                        <td className="whitespace-nowrap px-3 py-4 text-sm text-gray-500">
                          {formatUptime(worker.uptime_seconds)}
                        </td>
                        <td className="whitespace-nowrap px-3 py-4 text-sm text-gray-500">
                          {new Date(worker.last_heartbeat_at).toLocaleString()}
                        </td>
                      </tr>
                    );})
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
