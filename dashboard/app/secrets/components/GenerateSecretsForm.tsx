'use client';

import { useState } from 'react';

interface GenerateSecretsFormProps {
  isConnected: boolean;
  accountId: string | null;
  coordinatorUrl: string;
  onGenerated: (generatedKeys: string[]) => void;
  existingEncryptedData?: string; // Base64 encoded encrypted secrets (optional)
}

interface SecretToGenerate {
  id: string; // Temporary ID for UI
  name: string;
  generationType: string;
}

const GENERATION_TYPES = [
  { value: 'hex32', label: 'Hex 32 bytes (64 chars)' },
  { value: 'hex16', label: 'Hex 16 bytes (32 chars)' },
  { value: 'hex64', label: 'Hex 64 bytes (128 chars)' },
  { value: 'ed25519', label: 'ED25519 Private Key (NEAR format)' },
  { value: 'ed25519_seed', label: 'ED25519 Seed (32 bytes)' },
  { value: 'password', label: 'Password (32 chars)' },
  { value: 'password:16', label: 'Password (16 chars)' },
  { value: 'password:64', label: 'Password (64 chars)' },
  { value: 'password:128', label: 'Password (128 chars)' },
];

export function GenerateSecretsForm({
  isConnected,
  accountId,
  coordinatorUrl,
  onGenerated,
  existingEncryptedData,
}: GenerateSecretsFormProps) {
  const [repo, setRepo] = useState('');
  const [branch, setBranch] = useState('');
  const [secretsToGenerate, setSecretsToGenerate] = useState<SecretToGenerate[]>([
    { id: '1', name: 'MASTER_KEY', generationType: 'hex32' }
  ]);
  const [generating, setGenerating] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [generatedKeys, setGeneratedKeys] = useState<string[]>([]);

  const addSecretRow = () => {
    const newId = String(Date.now());
    setSecretsToGenerate([
      ...secretsToGenerate,
      { id: newId, name: '', generationType: 'hex32' }
    ]);
  };

  const removeSecretRow = (id: string) => {
    setSecretsToGenerate(secretsToGenerate.filter(s => s.id !== id));
  };

  const updateSecretRow = (id: string, field: 'name' | 'generationType', value: string) => {
    setSecretsToGenerate(
      secretsToGenerate.map(s =>
        s.id === id ? { ...s, [field]: value } : s
      )
    );
  };

  const handleGenerateAndEncrypt = async () => {
    setError(null);
    setGeneratedKeys([]);

    if (!isConnected || !accountId) {
      setError('Please connect your wallet first');
      return;
    }

    if (!repo.trim()) {
      setError('Repository is required');
      return;
    }

    // Validate secret names
    const validSecrets = secretsToGenerate.filter(s => s.name.trim() !== '');
    if (validSecrets.length === 0) {
      setError('At least one secret name is required');
      return;
    }

    // Check for duplicate names
    const names = validSecrets.map(s => s.name.trim());
    const uniqueNames = new Set(names);
    if (names.length !== uniqueNames.size) {
      setError('Duplicate secret names are not allowed');
      return;
    }

    setGenerating(true);

    try {
      // Call coordinator to add generated secrets
      const response = await fetch(`${coordinatorUrl}/secrets/add_generated_secret`, {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
        },
        body: JSON.stringify({
          repo: repo.trim(),
          owner: accountId,
          branch: branch.trim() || null,
          encrypted_secrets_base64: existingEncryptedData || null,
          new_secrets: validSecrets.map(s => ({
            name: s.name.trim(),
            generation_type: s.generationType,
          })),
        }),
      });

      if (!response.ok) {
        const errorText = await response.text();
        try {
          const errorJson = JSON.parse(errorText);
          throw new Error(errorJson.error || errorText);
        } catch {
          throw new Error(errorText);
        }
      }

      const data = await response.json();

      // Store generated keys for display
      setGeneratedKeys(data.generated_keys || []);

      console.log('🔑 GENERATED SECRETS:', {
        generated_keys: data.generated_keys,
        encrypted_data_length: data.encrypted_data_base64.length,
      });

      // Pass back to parent
      onGenerated(data.generated_keys);

      // Clear form
      setRepo('');
      setBranch('');
      setSecretsToGenerate([{ id: '1', name: 'MASTER_KEY', generationType: 'hex32' }]);
      setError(null);
    } catch (err) {
      console.error('Generation error:', err);
      setError(`Failed to generate secrets: ${(err as Error).message}`);
    } finally {
      setGenerating(false);
    }
  };

  return (
    <div className="bg-white shadow sm:rounded-lg">
      <div className="px-4 py-5 sm:p-6">
        <h2 className="text-lg font-medium text-gray-900 mb-4">
          🔑 Generate Secrets
        </h2>

        <p className="text-sm text-gray-600 mb-4">
          Auto-generate cryptographically secure secrets without seeing their values.
          Perfect for MASTER_KEY, API tokens, passwords, and ED25519 keys.
        </p>

        {/* Repository */}
        <div className="mb-4">
          <label className="block text-sm font-medium text-gray-700 mb-2">
            GitHub Repository *
          </label>
          <input
            type="text"
            value={repo}
            onChange={(e) => setRepo(e.target.value)}
            placeholder="owner/repo or https://github.com/owner/repo"
            className="w-full px-3 py-2 border border-gray-300 rounded-md shadow-sm focus:outline-none focus:ring-blue-500 focus:border-blue-500"
            disabled={generating}
          />
        </div>

        {/* Branch (optional) */}
        <div className="mb-4">
          <label className="block text-sm font-medium text-gray-700 mb-2">
            Branch (optional)
          </label>
          <input
            type="text"
            value={branch}
            onChange={(e) => setBranch(e.target.value)}
            placeholder="main, develop, etc. (leave empty for all branches)"
            className="w-full px-3 py-2 border border-gray-300 rounded-md shadow-sm focus:outline-none focus:ring-blue-500 focus:border-blue-500"
            disabled={generating}
          />
        </div>

        {/* Secrets to Generate */}
        <div className="mb-4">
          <label className="block text-sm font-medium text-gray-700 mb-2">
            Secrets to Generate *
          </label>

          <div className="space-y-2">
            {secretsToGenerate.map((secret) => (
              <div key={secret.id} className="flex gap-2">
                <input
                  type="text"
                  value={secret.name}
                  onChange={(e) => updateSecretRow(secret.id, 'name', e.target.value)}
                  placeholder="Secret name (e.g., MASTER_KEY)"
                  className="flex-1 px-3 py-2 border border-gray-300 rounded-md shadow-sm focus:outline-none focus:ring-blue-500 focus:border-blue-500"
                  disabled={generating}
                />
                <select
                  value={secret.generationType}
                  onChange={(e) => updateSecretRow(secret.id, 'generationType', e.target.value)}
                  className="w-64 px-3 py-2 border border-gray-300 rounded-md shadow-sm focus:outline-none focus:ring-blue-500 focus:border-blue-500"
                  disabled={generating}
                >
                  {GENERATION_TYPES.map((type) => (
                    <option key={type.value} value={type.value}>
                      {type.label}
                    </option>
                  ))}
                </select>
                {secretsToGenerate.length > 1 && (
                  <button
                    onClick={() => removeSecretRow(secret.id)}
                    disabled={generating}
                    className="px-3 py-2 border border-red-300 text-red-700 rounded-md hover:bg-red-50 disabled:opacity-50"
                  >
                    ✕
                  </button>
                )}
              </div>
            ))}
          </div>

          <button
            onClick={addSecretRow}
            disabled={generating}
            className="mt-2 px-3 py-1 text-sm border border-gray-300 text-gray-700 rounded-md hover:bg-gray-50 disabled:opacity-50"
          >
            + Add Another Secret
          </button>
        </div>

        {/* Generated Keys Display */}
        {generatedKeys.length > 0 && (
          <div className="mb-4 bg-green-50 border border-green-200 rounded-lg p-4">
            <h3 className="text-sm font-medium text-green-900 mb-2">
              ✅ Generated Keys (never shown again)
            </h3>
            <ul className="text-xs text-green-800 space-y-1 list-disc list-inside">
              {generatedKeys.map((key) => (
                <li key={key}>{key}</li>
              ))}
            </ul>
            <p className="mt-2 text-xs text-green-700">
              These secrets are now encrypted and stored. You cannot retrieve their values later.
            </p>
          </div>
        )}

        {/* Error Display */}
        {error && (
          <div className="mb-4 bg-red-50 border border-red-200 rounded-lg p-4">
            <p className="text-sm text-red-800">{error}</p>
          </div>
        )}

        {/* Submit Button */}
        <div className="flex items-center justify-between">
          <button
            onClick={handleGenerateAndEncrypt}
            disabled={!isConnected || generating}
            className="inline-flex items-center px-4 py-2 border border-transparent text-sm font-medium rounded-md shadow-sm text-white bg-green-600 hover:bg-green-700 focus:outline-none focus:ring-2 focus:ring-offset-2 focus:ring-green-500 disabled:bg-gray-300 disabled:cursor-not-allowed"
          >
            {generating ? '🔄 Generating...' : '🔑 Generate & Encrypt Secrets'}
          </button>

          {!isConnected && (
            <p className="text-sm text-red-600">
              Please connect your wallet to generate secrets
            </p>
          )}
        </div>

        {/* Info Box */}
        <div className="mt-6 bg-blue-50 border border-blue-200 rounded-lg p-4">
          <h3 className="text-sm font-medium text-blue-900 mb-2">💡 How it works</h3>
          <ul className="text-xs text-blue-800 space-y-1 list-disc list-inside">
            <li>Keystore generates secrets inside TEE (you never see the values)</li>
            <li>Generated secrets are immediately encrypted with ChaCha20-Poly1305</li>
            <li>You only see the list of key names (for verification)</li>
            <li>Secrets can be added incrementally to existing encrypted data</li>
            <li>Perfect for HKDF seeds, API keys, passwords, and ED25519 keys</li>
          </ul>
        </div>

      </div>
    </div>
  );
}
