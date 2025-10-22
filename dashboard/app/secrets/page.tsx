'use client';

import { useState, useEffect, useCallback } from 'react';
import { useNearWallet } from '@/contexts/NearWalletContext';
import { actionCreators } from '@near-js/transactions';
import { SecretsForm } from './components/SecretsForm';
import { SecretsList } from './components/SecretsList';
import { UserSecret, FormData } from './components/types';

const COORDINATOR_URL = 'http://localhost:8080';

export default function SecretsPage() {
  const { accountId, isConnected, connect, signAndSendTransaction, contractId, network } = useNearWallet();

  // User's secrets list
  const [userSecrets, setUserSecrets] = useState<UserSecret[]>([]);
  const [loadingSecrets, setLoadingSecrets] = useState(false);

  // UI state
  const [error, setError] = useState<string | null>(null);
  const [success, setSuccess] = useState<string | null>(null);

  // Edit mode
  const [editingSecret, setEditingSecret] = useState<UserSecret | null>(null);

  const loadUserSecrets = useCallback(async () => {
    if (!accountId) return;

    setLoadingSecrets(true);

    try {
      const viewResult = await fetch(`https://rpc.${network}.near.org`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          jsonrpc: '2.0',
          id: 'dontcare',
          method: 'query',
          params: {
            request_type: 'call_function',
            finality: 'final',
            account_id: contractId,
            method_name: 'list_user_secrets',
            args_base64: btoa(JSON.stringify({ account_id: accountId })),
          },
        }),
      });

      const viewData = await viewResult.json();

      if (viewData.error) {
        throw new Error(viewData.error.message || 'Failed to load secrets');
      }

      const resultBytes = viewData.result?.result;
      if (resultBytes && resultBytes.length > 0) {
        const resultStr = new TextDecoder().decode(new Uint8Array(resultBytes));
        const secrets = JSON.parse(resultStr);
        setUserSecrets(secrets);
      } else {
        setUserSecrets([]);
      }
    } catch (err) {
      console.error('Failed to load user secrets:', err);
      setError(`Failed to load secrets: ${(err as Error).message}`);
      setUserSecrets([]);
    } finally {
      setLoadingSecrets(false);
    }
  }, [accountId, network, contractId]);

  // Load user secrets when connected
  useEffect(() => {
    if (isConnected && accountId) {
      loadUserSecrets();
    }
  }, [isConnected, accountId, loadUserSecrets]);

  // Clear messages after 5 seconds
  useEffect(() => {
    if (error || success) {
      const timer = setTimeout(() => {
        setError(null);
        setSuccess(null);
      }, 5000);
      return () => clearTimeout(timer);
    }
  }, [error, success]);

  const handleSubmitSecrets = async (formData: FormData, encryptedArray: number[]) => {
    try {
      // Convert encrypted array to base64 for contract
      const encryptedBase64 = Buffer.from(encryptedArray).toString('base64');

      // Prepare transaction
      const transactionArgs = {
        repo: formData.repo,
        branch: formData.branch,
        profile: formData.profile,
        encrypted_secrets_base64: encryptedBase64,
        access: formData.access,
      };

      // Create function call action
      const action = actionCreators.functionCall(
        'store_secrets',
        transactionArgs,
        BigInt('50000000000000'), // 50 TGas
        BigInt('10000000000000000000000') // 0.01 NEAR for storage
      );

      const response = await signAndSendTransaction({
        receiverId: contractId,
        actions: [action],
      });

      setSuccess(`Secrets ${editingSecret ? 'updated' : 'created'} successfully! Transaction: ${response?.transaction?.hash || 'completed'}`);
      setEditingSecret(null);

      // Reload secrets list
      setTimeout(() => {
        loadUserSecrets();
      }, 2000);
    } catch (err) {
      setError(`Failed to save secrets: ${(err as Error).message}`);
      console.error(err);
      throw err;
    }
  };

  const handleEditSecret = (secret: UserSecret) => {
    if (!confirm(`⚠️ Replace secrets for ${secret.repo}:${secret.profile}?\n\nNote: You cannot decrypt/view existing secrets - only workers can decrypt them.\nThis will completely replace the encrypted secrets with new ones.`)) {
      return;
    }
    setEditingSecret(secret);
    // Scroll to form
    window.scrollTo({ top: 0, behavior: 'smooth' });
  };

  const handleDeleteSecret = async (secret: UserSecret) => {
    if (!confirm(`Delete secrets for ${secret.repo}:${secret.profile}? Storage deposit will be refunded automatically.`)) {
      return;
    }

    try {
      const action = actionCreators.functionCall(
        'delete_secrets',
        {
          repo: secret.repo,
          branch: secret.branch,
          profile: secret.profile,
        },
        BigInt('30000000000000'), // 30 TGas
        BigInt('0') // No deposit needed - storage deposit will be refunded automatically
      );

      await signAndSendTransaction({
        receiverId: contractId,
        actions: [action],
      });

      setSuccess('Secrets deleted successfully! Storage deposit refunded.');
      setTimeout(() => loadUserSecrets(), 2000);
    } catch (err) {
      setError(`Failed to delete: ${(err as Error).message}`);
    }
  };

  return (
    <div className="max-w-4xl mx-auto">
      {/* Header */}
      <div className="sm:flex sm:items-center">
        <div className="sm:flex-auto">
          <h1 className="text-3xl font-bold text-gray-900">Secrets Management</h1>
          <p className="mt-2 text-sm text-gray-700">
            Create and manage encrypted secrets for your repositories
          </p>
        </div>
        {!isConnected && (
          <div className="mt-4 sm:mt-0">
            <button
              onClick={connect}
              className="inline-flex items-center px-4 py-2 border border-transparent text-sm font-medium rounded-md text-white bg-blue-600 hover:bg-blue-700"
            >
              Connect Wallet
            </button>
          </div>
        )}
      </div>

      {/* Error Display */}
      {error && (
        <div className="mt-4 bg-red-50 border border-red-200 rounded-md p-3">
          <p className="text-sm text-red-800">{error}</p>
        </div>
      )}

      {/* Success Display */}
      {success && (
        <div className="mt-4 bg-green-50 border border-green-200 rounded-md p-3">
          <p className="text-sm text-green-800">{success}</p>
        </div>
      )}

      {/* Secrets Form */}
      <div className="mt-8">
        <SecretsForm
          isConnected={isConnected}
          accountId={accountId}
          onSubmit={handleSubmitSecrets}
          coordinatorUrl={COORDINATOR_URL}
          initialData={
            editingSecret
              ? {
                  repo: editingSecret.repo,
                  branch: editingSecret.branch || '',
                  profile: editingSecret.profile,
                }
              : undefined
          }
        />
      </div>

      {/* User's Secrets List */}
      <SecretsList
        secrets={userSecrets}
        loading={loadingSecrets}
        isConnected={isConnected}
        onEdit={handleEditSecret}
        onDelete={handleDeleteSecret}
        onRefresh={loadUserSecrets}
      />

      {/* Info Section */}
      <div className="mt-8 bg-blue-50 border border-blue-200 rounded-lg p-6">
        <h3 className="text-sm font-semibold text-blue-900 mb-3">
          🔐 How Repo-Based Secrets Work
        </h3>
        <ol className="text-sm text-blue-800 space-y-2 list-decimal list-inside">
          <li>
            <strong>Create secrets</strong>: Secrets are encrypted with keystore&apos;s public key and stored in the contract
          </li>
          <li>
            <strong>Reference in execution</strong>: Use <code className="bg-blue-100 px-1 py-0.5 rounded text-xs font-mono">secrets_ref: {`{profile: "production", account_id: "you.near"}`}</code>
          </li>
          <li>
            <strong>Automatic decryption</strong>: Worker fetches secrets from contract and decrypts via keystore
          </li>
          <li>
            <strong>Access validation</strong>: Keystore validates access conditions (balance checks, whitelists, etc.)
          </li>
          <li>
            <strong>WASI injection</strong>: Decrypted secrets injected as environment variables into WASM
          </li>
          <li>
            <strong>Code access</strong>: Your WASM code uses <code className="bg-blue-100 px-1 py-0.5 rounded text-xs font-mono">std::env::var(&quot;API_KEY&quot;)</code>
          </li>
        </ol>

        <div className="mt-4 p-3 bg-white rounded border border-blue-200">
          <h4 className="text-xs font-semibold text-blue-900 mb-2">Example: Request Execution with Secrets</h4>
          <pre className="text-xs text-blue-800 overflow-x-auto">
{`near call offchainvm.testnet request_execution '{
  "code_source": {
    "repo": "https://github.com/alice/myproject",
    "commit": "main",
    "build_target": "wasm32-wasip1"
  },
  "secrets_ref": {
    "profile": "production",
    "account_id": "alice.near"
  },
  "resource_limits": { ... },
  "input_data": "{}"
}' --accountId alice.near --deposit 0.1`}
          </pre>
        </div>
      </div>
    </div>
  );
}
