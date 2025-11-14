'use client';

import { useState, useEffect } from 'react';
import { ChaCha20Poly1305 } from '@stablelib/chacha20poly1305';
import { randomBytes } from '@stablelib/random';
import { AccessConditionBuilder } from './AccessConditionBuilder';
import { AccessCondition, FormData } from './types';
import { convertAccessToContractFormat } from './utils';

interface SecretsFormProps {
  isConnected: boolean;
  accountId: string | null;
  onSubmit: (formData: FormData, encryptedSecrets: number[]) => Promise<void>;
  coordinatorUrl: string;
  initialData?: {
    repo: string;
    branch: string;
    profile: string;
  };
}

interface SecretToGenerate {
  id: string;
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

export function SecretsForm({ isConnected, accountId, onSubmit, coordinatorUrl, initialData }: SecretsFormProps) {
  const [repo, setRepo] = useState('');
  const [branch, setBranch] = useState('');
  const [profile, setProfile] = useState('default');
  const [plaintextSecrets, setPlaintextSecrets] = useState('{\n  "API_KEY": "your-api-key"\n}');
  const [accessCondition, setAccessCondition] = useState<AccessCondition>({ type: 'AllowAll' });
  const [encrypting, setEncrypting] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [secretsToGenerate, setSecretsToGenerate] = useState<SecretToGenerate[]>([]);
  const [generatedKeys, setGeneratedKeys] = useState<string[]>([]);

  // Load initial data if provided (for edit mode)
  useEffect(() => {
    if (initialData) {
      setRepo(initialData.repo);
      setBranch(initialData.branch || '');
      setProfile(initialData.profile);
      setPlaintextSecrets('{\n  "API_KEY": "your-new-api-key"\n}');
    }
  }, [initialData]);

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

  const handleEncryptAndSubmit = async () => {
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

    if (!profile.trim()) {
      setError('Profile is required');
      return;
    }

    // Validate that at least one type of secrets is provided
    const hasManualSecrets = plaintextSecrets.trim() !== '' && plaintextSecrets.trim() !== '{\n  "API_KEY": "your-api-key"\n}';
    const validSecretsToGenerate = secretsToGenerate.filter(s => s.name.trim() !== '');
    const hasGeneratedSecrets = validSecretsToGenerate.length > 0;

    if (!hasManualSecrets && !hasGeneratedSecrets) {
      setError('Please provide either manual secrets or secrets to generate');
      return;
    }

    // Validate JSON format if manual secrets provided
    if (hasManualSecrets) {
      try {
        const parsed = JSON.parse(plaintextSecrets);
        if (typeof parsed !== 'object' || Array.isArray(parsed)) {
          throw new Error('Secrets must be a JSON object');
        }
      } catch (err) {
        setError(`Invalid JSON format: ${(err as Error).message}`);
        return;
      }
    }

    // Check for duplicate names in generated secrets
    if (hasGeneratedSecrets) {
      const names = validSecretsToGenerate.map(s => s.name.trim());
      const uniqueNames = new Set(names);
      if (names.length !== uniqueNames.size) {
        setError('Duplicate secret names are not allowed in generated secrets');
        return;
      }
    }

    setEncrypting(true);

    try {
      // If we have both manual and generated, OR only generated, use /secrets/add_generated_secret
      if (hasGeneratedSecrets) {
        // Step 1: Encrypt manual secrets if provided
        let encryptedSecretsBase64: string | null = null;

        if (hasManualSecrets) {
          // Get public key and encrypt
          const pubkeyResp = await fetch(`${coordinatorUrl}/secrets/pubkey`, {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({
              repo: repo.trim(),
              owner: accountId,
              branch: branch.trim() || null,
              secrets_json: plaintextSecrets,
            }),
          });

          if (!pubkeyResp.ok) {
            const errorText = await pubkeyResp.text();
            try {
              const errorJson = JSON.parse(errorText);
              throw new Error(errorJson.error || errorText);
            } catch {
              throw new Error(errorText);
            }
          }

          const pubkeyData = await pubkeyResp.json();
          const pubkeyHex = pubkeyData.pubkey;

          // Encrypt with ChaCha20-Poly1305
          const keyMaterial = hexToBytes(pubkeyHex);
          const encoder = new TextEncoder();
          const plaintextBytes = encoder.encode(plaintextSecrets);
          const cipher = new ChaCha20Poly1305(keyMaterial);
          const nonce = randomBytes(12);
          const ciphertextWithTag = cipher.seal(nonce, plaintextBytes);
          const encrypted = new Uint8Array(12 + ciphertextWithTag.length);
          encrypted.set(nonce, 0);
          encrypted.set(ciphertextWithTag, 12);

          // Convert to base64
          encryptedSecretsBase64 = btoa(String.fromCharCode(...Array.from(encrypted)));
        }

        // Step 2: Call /secrets/add_generated_secret
        const response = await fetch(`${coordinatorUrl}/secrets/add_generated_secret`, {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({
            repo: repo.trim(),
            owner: accountId,
            branch: branch.trim() || null,
            encrypted_secrets_base64: encryptedSecretsBase64,
            new_secrets: validSecretsToGenerate.map(s => ({
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

        // Use all_keys from keystore response (authoritative source)
        const allKeys: string[] = data.all_keys || [];

        setGeneratedKeys(allKeys);

        console.log('🔑 ENCRYPTED SECRETS:', {
          total_keys: allKeys.length,
          all_keys: allKeys,
          encrypted_data_length: data.encrypted_data_base64.length,
        });

        // Convert base64 to array and submit to contract
        const encryptedArray = Array.from(atob(data.encrypted_data_base64), c => c.charCodeAt(0));
        const contractAccess = convertAccessToContractFormat(accessCondition);
        const formData: FormData = {
          repo: repo.trim(),
          branch: branch.trim() || null,
          profile: profile.trim(),
          access: contractAccess,
        };

        await onSubmit(formData, encryptedArray);

        // Clear form on success
        setRepo('');
        setBranch('');
        setProfile('default');
        setPlaintextSecrets('{\n  "API_KEY": "your-api-key"\n}');
        setAccessCondition({ type: 'AllowAll' });
        setSecretsToGenerate([]);
        setError(null);
      } else {
        // Only manual secrets - use original flow
        const pubkeyResp = await fetch(`${coordinatorUrl}/secrets/pubkey`, {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({
            repo: repo.trim(),
            owner: accountId,
            branch: branch.trim() || null,
            secrets_json: plaintextSecrets,
          }),
        });

        if (!pubkeyResp.ok) {
          const errorText = await pubkeyResp.text();
          try {
            const errorJson = JSON.parse(errorText);
            throw new Error(errorJson.error || errorText);
          } catch {
            throw new Error(errorText);
          }
        }

        const pubkeyData = await pubkeyResp.json();
        const pubkeyHex = pubkeyData.pubkey;
        const repoNormalized = pubkeyData.repo_normalized;

        const keyMaterial = hexToBytes(pubkeyHex);
        const encoder = new TextEncoder();
        const plaintextBytes = encoder.encode(plaintextSecrets);
        const cipher = new ChaCha20Poly1305(keyMaterial);
        const nonce = randomBytes(12);
        const ciphertextWithTag = cipher.seal(nonce, plaintextBytes);
        const encrypted = new Uint8Array(12 + ciphertextWithTag.length);
        encrypted.set(nonce, 0);
        encrypted.set(ciphertextWithTag, 12);
        const encryptedArray = Array.from(encrypted);

        const contractAccess = convertAccessToContractFormat(accessCondition);
        const formData: FormData = {
          repo: repoNormalized,
          branch: branch.trim() || null,
          profile: profile.trim(),
          access: contractAccess,
        };

        await onSubmit(formData, encryptedArray);

        // Clear form on success
        setRepo('');
        setBranch('');
        setProfile('default');
        setPlaintextSecrets('{\n  "API_KEY": "your-api-key"\n}');
        setAccessCondition({ type: 'AllowAll' });
        setError(null);
      }
    } catch (err) {
      console.error('Encryption error:', err);
      setError(`Failed to encrypt secrets: ${(err as Error).message}`);
    } finally {
      setEncrypting(false);
    }
  };

  const hexToBytes = (hex: string): Uint8Array => {
    const bytes = new Uint8Array(hex.length / 2);
    for (let i = 0; i < hex.length; i += 2) {
      bytes[i / 2] = parseInt(hex.substring(i, i + 2), 16);
    }
    return bytes;
  };

  return (
    <div className="bg-white shadow sm:rounded-lg">
      <div className="px-4 py-5 sm:p-6">
        <h2 className="text-lg font-medium text-gray-900 mb-4">
          {initialData ? 'Update Secrets' : 'Create New Secrets'}
        </h2>

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
            disabled={encrypting}
          />
          <p className="mt-1 text-xs text-gray-500">
            Examples: alice/project, https://github.com/alice/project
          </p>
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
            disabled={encrypting}
          />
        </div>

        {/* Profile */}
        <div className="mb-4">
          <label className="block text-sm font-medium text-gray-700 mb-2">
            Profile *
          </label>
          <input
            type="text"
            value={profile}
            onChange={(e) => setProfile(e.target.value)}
            placeholder="default, prod, staging, etc."
            className="w-full px-3 py-2 border border-gray-300 rounded-md shadow-sm focus:outline-none focus:ring-blue-500 focus:border-blue-500"
            disabled={encrypting}
          />
          <p className="mt-1 text-xs text-gray-500">
            Profile name for organizing multiple secret sets per repo
          </p>
        </div>

        {/* Plaintext Secrets (JSON) */}
        <div className="mb-4">
          <label className="block text-sm font-medium text-gray-700 mb-2">
            Secrets (JSON format, optional)
          </label>
          <textarea
            value={plaintextSecrets}
            onChange={(e) => setPlaintextSecrets(e.target.value)}
            rows={8}
            className="w-full px-3 py-2 border border-gray-300 rounded-md shadow-sm font-mono text-sm focus:outline-none focus:ring-blue-500 focus:border-blue-500"
            placeholder='{\n  "OPENAI_KEY": "sk-...",\n  "DATABASE_URL": "postgres://..."\n}'
            disabled={encrypting}
          />
          <p className="mt-1 text-xs text-gray-500">
            JSON object with key-value pairs. Keys will be available as environment variables in WASM execution.
          </p>
          <p className="mt-1 text-xs text-amber-600">
            ⚠️ Cannot use <code className="bg-amber-100 px-1 rounded">PROTECTED_*</code> prefix (reserved for auto-generated secrets)
          </p>
        </div>

        {/* Secrets to Generate */}
        <div className="mb-4">
          <label className="block text-sm font-medium text-gray-700 mb-2">
            Secrets to Generate (optional)
          </label>
          <p className="text-xs text-gray-600 mb-2">
            Auto-generate cryptographically secure secrets without seeing their values. Perfect for MASTER_KEY, API tokens, passwords, and ED25519 keys.
          </p>
          <p className="text-xs text-green-600 mb-2">
            ✅ Must start with <code className="bg-green-100 px-1 rounded">PROTECTED_*</code> prefix (proves secret was generated in TEE)
          </p>

          {secretsToGenerate.length > 0 && (
            <div className="space-y-2 mb-2">
              {secretsToGenerate.map((secret) => (
                <div key={secret.id} className="flex gap-2">
                  <input
                    type="text"
                    value={secret.name}
                    onChange={(e) => updateSecretRow(secret.id, 'name', e.target.value)}
                    placeholder="e.g., PROTECTED_MASTER_KEY"
                    className="flex-1 px-3 py-2 border border-gray-300 rounded-md shadow-sm focus:outline-none focus:ring-blue-500 focus:border-blue-500"
                    disabled={encrypting}
                  />
                  <select
                    value={secret.generationType}
                    onChange={(e) => updateSecretRow(secret.id, 'generationType', e.target.value)}
                    className="w-64 px-3 py-2 border border-gray-300 rounded-md shadow-sm focus:outline-none focus:ring-blue-500 focus:border-blue-500"
                    disabled={encrypting}
                  >
                    {GENERATION_TYPES.map((type) => (
                      <option key={type.value} value={type.value}>
                        {type.label}
                      </option>
                    ))}
                  </select>
                  <button
                    onClick={() => removeSecretRow(secret.id)}
                    disabled={encrypting}
                    className="px-3 py-2 border border-red-300 text-red-700 rounded-md hover:bg-red-50 disabled:opacity-50"
                  >
                    ✕
                  </button>
                </div>
              ))}
            </div>
          )}

          <button
            onClick={addSecretRow}
            disabled={encrypting}
            className="px-3 py-1 text-sm border border-gray-300 text-gray-700 rounded-md hover:bg-gray-50 disabled:opacity-50"
          >
            + Add Generated Secret
          </button>
        </div>

        {/* All Encrypted Keys Display */}
        {generatedKeys.length > 0 && (
          <div className="mb-4 bg-green-50 border border-green-200 rounded-lg p-4">
            <h3 className="text-sm font-medium text-green-900 mb-2">
              ✅ Encrypted Keys ({generatedKeys.length} total)
            </h3>
            <ul className="text-xs text-green-800 space-y-1 list-disc list-inside">
              {generatedKeys.map((key) => (
                <li key={key}>{key}</li>
              ))}
            </ul>
            <p className="mt-2 text-xs text-green-700">
              All secrets are now encrypted and stored. You cannot retrieve their values later.
            </p>
          </div>
        )}

        {/* Access Condition */}
        <div className="mb-6">
          <label className="block text-sm font-medium text-gray-700 mb-2">
            Access Control *
          </label>
          <AccessConditionBuilder condition={accessCondition} onChange={setAccessCondition} />
        </div>

        {/* Error Display */}
        {error && (
          <div className="mb-4 bg-red-50 border border-red-200 rounded-lg p-4">
            <p className="text-sm text-red-800">{error}</p>
          </div>
        )}

        {/* Submit Button */}
        <div className="flex items-center justify-between">
          <button
            onClick={handleEncryptAndSubmit}
            disabled={!isConnected || encrypting}
            className="inline-flex items-center px-4 py-2 border border-transparent text-sm font-medium rounded-md shadow-sm text-white bg-blue-600 hover:bg-blue-700 focus:outline-none focus:ring-2 focus:ring-offset-2 focus:ring-blue-500 disabled:bg-gray-300 disabled:cursor-not-allowed"
          >
            {encrypting ? '🔄 Encrypting & Storing...' : initialData ? '💾 Update Secrets' : '🔐 Encrypt & Store Secrets'}
          </button>

          {!isConnected && (
            <p className="text-sm text-red-600">
              Please connect your wallet to create secrets
            </p>
          )}
        </div>

        {/* Info Box */}
        <div className="mt-6 bg-blue-50 border border-blue-200 rounded-lg p-4">
          <h3 className="text-sm font-medium text-blue-900 mb-2">💡 How it works</h3>
          <ul className="text-xs text-blue-800 space-y-1 list-disc list-inside">
            <li><strong>Manual secrets</strong>: Encrypted client-side with ChaCha20-Poly1305 AEAD</li>
            <li><strong>Generated secrets</strong>: Keystore generates inside TEE (you never see values)</li>
            <li><strong>Combined flow</strong>: Manual secrets encrypted → sent with generation specs → keystore decrypts, generates, merges, re-encrypts</li>
            <li>Uses keystore&apos;s public key (coordinator never sees plaintext)</li>
            <li>Encrypted data is stored on NEAR contract</li>
            <li>Only verified TEE workers can decrypt secrets during execution</li>
            <li>You can provide manual secrets, generated secrets, or both</li>
            <li>Storage costs ~0.01 NEAR per secret set</li>
          </ul>
        </div>

      </div>
    </div>
  );
}
