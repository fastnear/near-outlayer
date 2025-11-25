'use client';

import { UserSecret, isRepoAccessor, isWasmHashAccessor } from './types';
import { formatAccessCondition } from './utils';

interface SecretCardProps {
  secret: UserSecret;
  onEdit: () => void;
  onDelete: () => void;
}

export function SecretCard({ secret, onEdit, onDelete }: SecretCardProps) {
  // Validate accessor exists
  if (!secret.accessor) {
    return (
      <div className="border border-red-200 rounded-lg p-4 bg-red-50">
        <p className="text-sm text-red-700">⚠️ Invalid secret: accessor is missing</p>
      </div>
    );
  }

  return (
    <div className="border border-gray-200 rounded-lg p-4 hover:border-blue-300 hover:shadow-sm transition-all">
      <div className="flex items-start justify-between">
        <div className="flex-1 min-w-0">
          {/* Header */}
          <div className="flex items-center space-x-2 mb-2">
            {isRepoAccessor(secret.accessor) && secret.accessor.Repo && (
              <>
                <span className="inline-flex items-center px-2 py-0.5 rounded text-xs font-medium bg-gray-100 text-gray-700">
                  GitHub
                </span>
                <h3 className="text-sm font-semibold text-gray-900 truncate">
                  {secret.accessor.Repo.repo}
                </h3>
                {secret.accessor.Repo.branch && (
                  <span className="inline-flex items-center px-2 py-0.5 rounded text-xs font-medium bg-purple-100 text-purple-800">
                    {secret.accessor.Repo.branch}
                  </span>
                )}
              </>
            )}
            {isWasmHashAccessor(secret.accessor) && secret.accessor.WasmHash?.hash && (
              <>
                <span className="inline-flex items-center px-2 py-0.5 rounded text-xs font-medium bg-green-100 text-green-700">
                  WASM Hash
                </span>
                <h3 className="text-sm font-mono text-gray-900 truncate" title={secret.accessor.WasmHash.hash}>
                  {secret.accessor.WasmHash.hash.substring(0, 8)}...{secret.accessor.WasmHash.hash.substring(secret.accessor.WasmHash.hash.length - 8)}
                </h3>
              </>
            )}
          </div>

          {/* Profile */}
          <div className="mb-2">
            <span className="inline-flex items-center px-2.5 py-0.5 rounded-full text-xs font-medium bg-blue-100 text-blue-800">
              📦 {secret.profile}
            </span>
          </div>

          {/* Access Condition */}
          <div className="text-xs text-gray-600 mb-2">
            <span className="font-medium">Access:</span> {formatAccessCondition(secret.access)}
          </div>

          {/* Metadata */}
          <div className="flex items-center space-x-4 text-xs text-gray-500">
            <span>
              Created: {new Date(secret.created_at / 1000000).toLocaleDateString()}
            </span>
            <span>
              Storage: {(Number(secret.storage_deposit) / 1e24).toFixed(4)} NEAR
            </span>
          </div>
        </div>

        {/* Actions */}
        <div className="flex items-center space-x-2 ml-4">
          <button
            onClick={onEdit}
            className="inline-flex items-center px-3 py-1.5 border border-orange-300 text-xs font-medium rounded text-orange-700 bg-orange-50 hover:bg-orange-100 transition-colors"
            title="Replace encrypted secrets (you cannot view existing values)"
          >
            🔄 Replace
          </button>
          <button
            onClick={onDelete}
            className="inline-flex items-center px-3 py-1.5 border border-red-300 text-xs font-medium rounded text-red-700 bg-red-50 hover:bg-red-100 transition-colors"
            title="Delete secrets and refund storage deposit"
          >
            🗑️
          </button>
        </div>
      </div>
    </div>
  );
}
