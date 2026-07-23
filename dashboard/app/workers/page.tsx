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
                    <th className="px-3 py-3.5 text-left text-sm font-semibold text-gray-900">Instance</th>
                    <th className="px-3 py-3.5 text-left text-sm font-semibold text-gray-900">Status</th>
                    <th className="px-3 py-3.5 text-left text-sm font-semibold text-gray-900">Completed</th>
                    <th className="px-3 py-3.5 text-left text-sm font-semibold text-gray-900">Failed</th>
                    <th className="px-3 py-3.5 text-left text-sm font-semibold text-gray-900">First seen</th>
                    <th className="px-3 py-3.5 text-left text-sm font-semibold text-gray-900">Last Heartbeat</th>
                  </tr>
                </thead>
                <tbody className="divide-y divide-gray-200 bg-white">
                  {workers.length === 0 ? (
                    <tr>
                      <td colSpan={7} className="px-3 py-8 text-center text-sm text-gray-500">
                        No workers found
                      </td>
                    </tr>
                  ) : (
                    workers.map((worker) => {
                      // Extract app_id from worker_id (format: network-type-app_id).
                      const parts = worker.worker_id.split('-');
                      const network = parts[0];
                      const workerType = parts[1];
                      const appId = parts.length >= 3 ? parts.slice(2).join('-') : null;
                      const hasAppId = appId && /^[a-f0-9]{40}$/i.test(appId);
                      // Routing convention: Phala-hosted workers carry "phala" in their name/id;
                      // our self-hosted TDX workers do not. So "phala" -> Phala's explorer, else
                      // our own attestation portal (workers.outlayer.ai/app/<app_id>).
                      const isPhala =
                        /phala/i.test(worker.worker_id) || /phala/i.test(worker.worker_name);
                      let attestationUrl: string | null = null;
                      if (hasAppId) {
                        attestationUrl = isPhala
                          ? `https://trust.phala.com/app/${appId}?selected=app-code`
                          : `https://workers.outlayer.ai/app/${appId}`;
                      } else if (workerType === 'keystore' && (network === 'testnet' || network === 'mainnet')) {
                        // Keystore rows are synthesized by the coordinator without an app_id suffix
                        // (one keystore per network). The portal resolves the current keystore for
                        // the network and redirects to its attestation page.
                        attestationUrl = `https://workers.outlayer.ai/${network}-keystore`;
                      }

                      return (
                      // Several instances share a worker_id (it encodes the version, not the
                      // machine), so the attested instance key is part of the row key.
                      <tr key={`${worker.worker_id}:${worker.instance ?? ''}`}>
                        <td className="whitespace-nowrap px-3 py-4 text-sm text-gray-900 font-mono">
                          {attestationUrl ? (
                            <a
                              href={attestationUrl}
                              target="_blank"
                              rel="noopener noreferrer"
                              className="text-blue-600 hover:text-blue-800 hover:underline"
                              title={isPhala ? 'Verify TEE attestation on Phala Trust' : 'Verify TEE attestation on OutLayer'}
                            >
                              {worker.worker_id}
                            </a>
                          ) : (
                            worker.worker_id
                          )}
                        </td>
                        <td className="whitespace-nowrap px-3 py-4 text-sm text-gray-500 font-mono">
                          {worker.instance ? (
                            <span title="Prefix of this worker's attested public key (registered on-chain)">
                              {worker.instance}
                            </span>
                          ) : (
                            <span className="text-gray-400">—</span>
                          )}
                        </td>
                        <td className="whitespace-nowrap px-3 py-4 text-sm">
                          <span
                            className={
                              worker.status === 'online' || worker.status === 'busy'
                                ? 'text-green-700'
                                : 'text-gray-500'
                            }
                          >
                            {worker.status}
                          </span>
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
