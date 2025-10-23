'use client';

import { useState, useEffect } from 'react';
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

export function SecretsForm({ isConnected, accountId, onSubmit, coordinatorUrl, initialData }: SecretsFormProps) {
  const [repo, setRepo] = useState('');
  const [branch, setBranch] = useState('');
  const [profile, setProfile] = useState('default');
  const [plaintextSecrets, setPlaintextSecrets] = useState('{\n  "API_KEY": "your-api-key"\n}');
  const [accessCondition, setAccessCondition] = useState<AccessCondition>({ type: 'AllowAll' });
  const [encrypting, setEncrypting] = useState(false);

  // Load initial data if provided (for edit mode)
  useEffect(() => {
    if (initialData) {
      setRepo(initialData.repo);
      setBranch(initialData.branch || '');
      setProfile(initialData.profile);
      setPlaintextSecrets('{\n  "API_KEY": "your-new-api-key"\n}');
    }
  }, [initialData]);

  const handleEncryptAndSubmit = async () => {
    if (!isConnected || !accountId) {
      alert('Please connect your wallet first');
      return;
    }

    if (!repo.trim()) {
      alert('Repository is required');
      return;
    }

    if (!profile.trim()) {
      alert('Profile is required');
      return;
    }

    // Validate JSON
    try {
      const parsed = JSON.parse(plaintextSecrets);
      if (typeof parsed !== 'object' || Array.isArray(parsed)) {
        throw new Error('Secrets must be a JSON object');
      }
    } catch (err) {
      alert(`Invalid JSON format: ${(err as Error).message}`);
      return;
    }

    setEncrypting(true);

    try {
      // Get public key from coordinator (which proxies to keystore)
      const params = new URLSearchParams({
        repo: repo.trim(),
        owner: accountId,
      });
      if (branch.trim()) {
        params.set('branch', branch.trim());
      }

      const pubkeyResp = await fetch(`${coordinatorUrl}/secrets/pubkey?${params}`);
      if (!pubkeyResp.ok) {
        const errorText = await pubkeyResp.text();
        throw new Error(`Failed to get public key: ${errorText}`);
      }
      const pubkeyData = await pubkeyResp.json();
      const pubkeyHex = pubkeyData.pubkey;
      const repoNormalized = pubkeyData.repo_normalized; // Get normalized repo from coordinator

      // DEBUG: Log encryption parameters
      console.log('🔐 ENCRYPTION DEBUG:', {
        repo_input: repo.trim(),
        repo_normalized: repoNormalized,
        owner: accountId,
        branch: branch.trim() || null,
        pubkey_hex: pubkeyHex,
        plaintext_length: plaintextSecrets.length
      });

      // Encrypt secrets (simple XOR encryption - same as encrypt_secrets.py)
      const keyMaterial = hexToBytes(pubkeyHex);
      const encoder = new TextEncoder();
      const plaintextBytes = encoder.encode(plaintextSecrets);

      // Derive symmetric key (SHA-256 of pubkey + salt)
      const hashInput = new Uint8Array([...keyMaterial, ...encoder.encode('keystore-encryption-v1')]);
      const derivedKeyBuffer = await crypto.subtle.digest('SHA-256', hashInput);
      const derivedKey = new Uint8Array(derivedKeyBuffer);

      // XOR encryption
      const encrypted = new Uint8Array(plaintextBytes.length);
      for (let i = 0; i < plaintextBytes.length; i++) {
        encrypted[i] = plaintextBytes[i] ^ derivedKey[i % derivedKey.length];
      }

      const encryptedArray = Array.from(encrypted);

      // Convert access condition to contract format
      const contractAccess = convertAccessToContractFormat(accessCondition);

      const formData: FormData = {
        repo: repoNormalized, // Use normalized repo from coordinator
        branch: branch.trim() || null,
        profile: profile.trim(),
        access: contractAccess,
      };

      // DEBUG: Log what we're storing in contract
      console.log('📝 STORING IN CONTRACT:', {
        ...formData,
        encrypted_array_length: encryptedArray.length
      });

      await onSubmit(formData, encryptedArray);

      // Clear form on success
      setRepo('');
      setBranch('');
      setProfile('default');
      setPlaintextSecrets('{\n  "API_KEY": "your-api-key"\n}');
      setAccessCondition({ type: 'AllowAll' });
    } catch (err) {
      console.error('Encryption error:', err);
      alert(`Failed to encrypt secrets: ${(err as Error).message}`);
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
            Secrets (JSON format) *
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
        </div>

        {/* Access Condition */}
        <div className="mb-6">
          <label className="block text-sm font-medium text-gray-700 mb-2">
            Access Control *
          </label>
          <AccessConditionBuilder onChange={setAccessCondition} />
        </div>

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
            <li>Secrets are encrypted client-side using keystore&apos;s public key</li>
            <li>Encrypted data is stored on NEAR contract</li>
            <li>Only verified TEE workers can decrypt secrets during execution</li>
            <li>You can update secrets anytime (will overwrite existing)</li>
            <li>Storage costs ~0.01 NEAR per secret set</li>
          </ul>
        </div>
      </div>
    </div>
  );
}
