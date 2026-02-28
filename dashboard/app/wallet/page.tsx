'use client';

import { useState, useEffect, useCallback, Suspense } from 'react';
import { useSearchParams, useRouter } from 'next/navigation';
import { useNearWallet } from '@/contexts/NearWalletContext';
import WalletConnectionModal from '@/components/WalletConnectionModal';
import { getCoordinatorApiUrl } from '@/lib/api';
import Link from 'next/link';
import { saveWalletKey, computeKeyHash } from '@/lib/wallet-keys';
import { submitPolicy, parsePolicyResponse } from '@/lib/wallet-policy';
import { useApiKeyHash } from '@/hooks/useApiKeyHash';
import { usePolicyForm } from '@/hooks/usePolicyForm';
import { PolicyFormFields } from '@/components/wallet/PolicyFormFields';
import { PolicyJsonEditor } from '@/components/wallet/PolicyJsonEditor';

interface WalletInfo {
  wallet_id: string;
  address: string;
  chain: string;
}

export default function WalletHandoffPage() {
  return (
    <Suspense fallback={<div className="max-w-4xl mx-auto py-8 text-gray-400">Loading...</div>}>
      <WalletHandoffContent />
    </Suspense>
  );
}

function WalletHandoffContent() {
  const searchParams = useSearchParams();
  const router = useRouter();
  const apiKey = searchParams.get('key');

  const {
    accountId,
    isConnected,
    network,
    contractId,
    viewMethod,
    signAndSendTransaction,
  } = useNearWallet();
  const coordinatorUrl = getCoordinatorApiUrl(network);

  const [showWalletModal, setShowWalletModal] = useState(false);
  const [walletInfo, setWalletInfo] = useState<WalletInfo | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [success, setSuccess] = useState<string | null>(null);
  const [submitting, setSubmitting] = useState(false);
  const [existingPolicy, setExistingPolicy] = useState<boolean | null>(null);

  // Owner mode: connect wallet or enter manually
  const [ownerMode, setOwnerMode] = useState<'wallet' | 'manual'>('wallet');
  const [manualOwner, setManualOwner] = useState('');

  // The effective owner account
  const effectiveOwner = ownerMode === 'wallet' ? accountId : (manualOwner.trim() || null);
  const ownerReady = ownerMode === 'wallet' ? isConnected : !!manualOwner.trim();

  // SHA256 hash of current API key (for authorized_key_hashes in policy)
  const apiKeyHash = useApiKeyHash(apiKey);

  // Build knownKeyHashes map from the handoff API key
  const [knownKeyHashes, setKnownKeyHashes] = useState<Map<string, string>>(new Map());

  useEffect(() => {
    if (apiKey && apiKeyHash) {
      setKnownKeyHashes(new Map([[apiKeyHash, 'current handoff key']]));
    }
  }, [apiKey, apiKeyHash]);

  const handleSaveKey = useCallback((newKey: string) => {
    if (walletInfo) {
      const pk = `ed25519:${walletInfo.address}`;
      saveWalletKey(pk, newKey, 'generated key');
    }
  }, [walletInfo]);

  // Approval
  const [requireApproval, setRequireApproval] = useState(true);
  const [approvalRequired, setApprovalRequired] = useState('1');
  const [additionalApprovers, setAdditionalApprovers] = useState('');
  // Which types require approval (unchecked = excluded_types)
  const allTxTypes = ['transfer', 'call', 'delete', 'intents_withdraw', 'intents_swap', 'intents_deposit'] as const;
  const [approvalTypes, setApprovalTypes] = useState<Set<string>>(new Set(['transfer', 'call', 'delete', 'intents_withdraw']));

  // Policy form with augmentPolicy that adds owner-based approval
  const {
    policyForm,
    setPolicyForm,
    policyJsonText,
    setPolicyJsonText,
    jsonEdited,
    setJsonEdited,
    resetJson,
  } = usePolicyForm({
    apiKeyHash,
    augmentPolicy: useCallback((base: Record<string, unknown>) => {
      if (!requireApproval) return base;
      const approvers: Array<{ id: string; role: string }> = [{ id: effectiveOwner || '', role: 'admin' }];
      if (additionalApprovers.trim()) {
        additionalApprovers.split('\n').filter((l) => l.trim()).forEach((line) => {
          const [id, role] = line.split(',').map((s) => s.trim());
          if (id) approvers.push({ id, role: role || 'signer' });
        });
      }
      const excluded_types = allTxTypes.filter((t) => !approvalTypes.has(t));
      return {
        ...base,
        approval: {
          threshold: { required: parseInt(approvalRequired, 10) || 1 },
          ...(excluded_types.length > 0 ? { excluded_types } : {}),
          approvers,
        },
      };
    }, [requireApproval, approvalRequired, additionalApprovers, effectiveOwner, approvalTypes]),
  });

  // Fetch wallet info using the API key
  const loadWalletInfo = useCallback(async () => {
    if (!apiKey) return;
    setLoading(true);
    setError(null);

    try {
      const resp = await fetch(`${coordinatorUrl}/wallet/v1/address?chain=near`, {
        headers: { 'Authorization': `Bearer ${apiKey}` },
      });

      if (!resp.ok) {
        const data = await resp.json().catch(() => ({}));
        throw new Error(data.message || `Failed to fetch wallet info (HTTP ${resp.status})`);
      }

      const data = await resp.json();
      setWalletInfo({
        wallet_id: data.wallet_id,
        address: data.address,
        chain: 'near',
      });
    } catch (err) {
      setError((err as Error).message);
    } finally {
      setLoading(false);
    }
  }, [apiKey, coordinatorUrl]);

  // Check if policy already exists on-chain, and if so load it into the form
  const checkExistingPolicy = useCallback(async () => {
    if (!walletInfo || !apiKey) return;

    try {
      const walletPubkey = `ed25519:${walletInfo.address}`;
      const result = await viewMethod({
        contractId,
        method: 'get_wallet_policy',
        args: { wallet_pubkey: walletPubkey },
      }).catch(() => null);

      const exists = result !== null;
      setExistingPolicy(exists);

      // Load current policy from coordinator and pre-fill the form
      if (exists) {
        try {
          const resp = await fetch(`${coordinatorUrl}/wallet/v1/policy`, {
            headers: { 'Authorization': `Bearer ${apiKey}` },
          });
          if (resp.ok) {
            const data = await resp.json();
            const parsed = parsePolicyResponse(data, apiKeyHash || undefined);
            setPolicyForm(parsed.form);

            if (parsed.approval) {
              setRequireApproval(true);
              setApprovalRequired(parsed.approval.required);
              setAdditionalApprovers(
                // Remove owner since it's auto-added — match by account_id, not role
                parsed.approval.approvers
                  .split('\n')
                  .filter((line) => {
                    const id = line.split(',').map((s) => s.trim())[0] || '';
                    return id !== effectiveOwner;
                  })
                  .join('\n')
              );
              // Restore approvalTypes from excluded_types
              const excluded = (data.approval?.excluded_types || []) as string[];
              setApprovalTypes(new Set(allTxTypes.filter((t) => !excluded.includes(t))));
            }
          }
        } catch {
          // Failed to load — form stays default
        }
      }
    } catch {
      setExistingPolicy(false);
    }
  }, [walletInfo, contractId, viewMethod, apiKey, coordinatorUrl, apiKeyHash, setPolicyForm, setRequireApproval, setApprovalRequired, setAdditionalApprovers]);

  useEffect(() => {
    loadWalletInfo();
  }, [loadWalletInfo]);

  useEffect(() => {
    if (walletInfo) {
      checkExistingPolicy();
    }
  }, [walletInfo, checkExistingPolicy]);

  const handleSubmitPolicy = async () => {
    if (!effectiveOwner || !walletInfo) return;

    // Manual mode requires connected wallet to sign the transaction
    if (ownerMode === 'manual' && !isConnected) {
      setError('Connect your NEAR wallet to sign the transaction. The manual account will be set as owner.');
      return;
    }

    setError(null);
    setSubmitting(true);

    try {
      const { walletPubkey } = await submitPolicy({
        coordinatorUrl,
        apiKey: apiKey!,
        walletId: walletInfo.wallet_id,
        policyJsonText,
        contractId,
        viewMethod,
        signAndSendTransaction,
      });

      // Save API key to browser localStorage for approvals
      saveWalletKey(walletPubkey, apiKey!);

      setSuccess('Policy stored on-chain! Redirecting to wallet management...');
      setExistingPolicy(true);
      setTimeout(() => router.push('/wallet/manage'), 2000);
    } catch (err) {
      setError((err as Error).message);
    } finally {
      setSubmitting(false);
    }
  };

  // No API key provided
  if (!apiKey) {
    return (
      <div className="max-w-2xl mx-auto py-12">
        <h1 className="text-3xl font-bold text-gray-900 mb-4">Wallet Handoff</h1>
        <div className="bg-white shadow rounded-lg p-8 text-center">
          <p className="text-gray-600 mb-4">
            This page is used to take control of an AI agent wallet.
          </p>
          <p className="text-sm text-gray-500">
            Your agent should have given you a handoff URL like:<br />
            <code className="text-xs bg-gray-100 px-2 py-1 rounded mt-1 inline-block">
              /wallet?key=wk_...
            </code>
          </p>
          <div className="mt-6">
            <Link
              href="/wallet/manage"
              className="text-[#cc6600] hover:text-[#b35900] font-medium"
            >
              Or manage existing wallets &rarr;
            </Link>
          </div>
        </div>
      </div>
    );
  }

  return (
    <div className="max-w-3xl mx-auto py-8">
      <h1 className="text-3xl font-bold text-gray-900 mb-2">Wallet Handoff</h1>
      <p className="text-gray-500 mb-6">
        Take control of your AI agent&apos;s wallet by setting a spending policy.
      </p>

      {error && (
        <div className="mb-4 bg-red-50 border border-red-200 rounded-md p-3">
          <p className="text-sm text-red-800">{error}</p>
        </div>
      )}

      {success && (
        <div className="mb-4 bg-green-50 border border-green-200 rounded-md p-3">
          <p className="text-sm text-green-800">{success}</p>
        </div>
      )}

      {/* Step 1: Wallet Info */}
      <div className="bg-white shadow rounded-lg p-6 mb-6">
        <h2 className="text-lg font-semibold text-gray-900 mb-3">
          <span className="inline-flex items-center justify-center w-6 h-6 rounded-full bg-[#cc6600] text-white text-xs mr-2">1</span>
          Wallet Info
        </h2>

        {loading ? (
          <div className="flex items-center py-4">
            <svg className="animate-spin h-5 w-5 text-[#cc6600] mr-3" fill="none" viewBox="0 0 24 24">
              <circle className="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" strokeWidth="4" />
              <path className="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4z" />
            </svg>
            <span className="text-gray-500">Loading wallet...</span>
          </div>
        ) : walletInfo ? (
          <div className="space-y-2">
            <div>
              <span className="text-xs text-gray-500 uppercase">Wallet ID</span>
              <p className="text-sm font-mono text-gray-900 break-all">{walletInfo.wallet_id}</p>
            </div>
            <div>
              <span className="text-xs text-gray-500 uppercase">NEAR Address (implicit)</span>
              <p className="text-sm font-mono text-gray-900 break-all">{walletInfo.address}</p>
            </div>
            {existingPolicy === true && (
              <div className="mt-2 bg-blue-50 border border-blue-200 rounded p-2">
                <p className="text-sm text-blue-800">This wallet already has a policy on-chain. Submitting a new one will replace it.</p>
              </div>
            )}
          </div>
        ) : null}
      </div>

      {/* Step 2: Policy Owner */}
      {walletInfo && (
        <div className="bg-white shadow rounded-lg p-6 mb-6">
          <h2 className="text-lg font-semibold text-gray-900 mb-3">
            <span className="inline-flex items-center justify-center w-6 h-6 rounded-full bg-[#cc6600] text-white text-xs mr-2">2</span>
            Policy Owner
          </h2>

          <p className="text-sm text-gray-500 mb-4">
            The owner can freeze the wallet, update the policy, and approve transactions.
          </p>

          <div className="flex space-x-3 mb-4">
            <button
              type="button"
              onClick={() => setOwnerMode('wallet')}
              className={`px-4 py-2 text-sm rounded-lg border ${
                ownerMode === 'wallet'
                  ? 'bg-[#cc6600] text-white border-[#cc6600]'
                  : 'bg-white text-gray-700 border-gray-300 hover:bg-gray-50'
              }`}
            >
              Connect Wallet
            </button>
            <button
              type="button"
              onClick={() => setOwnerMode('manual')}
              className={`px-4 py-2 text-sm rounded-lg border ${
                ownerMode === 'manual'
                  ? 'bg-[#cc6600] text-white border-[#cc6600]'
                  : 'bg-white text-gray-700 border-gray-300 hover:bg-gray-50'
              }`}
            >
              Enter Account ID
            </button>
          </div>

          {ownerMode === 'wallet' ? (
            isConnected ? (
              <p className="text-sm text-green-700">
                Connected as <span className="font-mono font-medium">{accountId}</span>
              </p>
            ) : (
              <div>
                <button
                  onClick={() => setShowWalletModal(true)}
                  className="px-4 py-2 bg-gradient-to-r from-[#cc6600] to-[#d4a017] text-white rounded-lg font-medium hover:from-[#b35900] hover:to-[#c49016]"
                >
                  Connect Wallet
                </button>
              </div>
            )
          ) : (
            <div>
              <input
                type="text"
                value={manualOwner}
                onChange={(e) => setManualOwner(e.target.value)}
                placeholder="e.g. alice.near"
                className="w-full border border-gray-300 rounded px-3 py-2 text-sm font-mono"
              />
              <p className="text-xs text-gray-400 mt-1">
                This NEAR account will be the policy owner. You still need to connect a wallet to sign the transaction.
              </p>
              {manualOwner.trim() && !isConnected && (
                <div className="mt-3">
                  <button
                    onClick={() => setShowWalletModal(true)}
                    className="px-4 py-2 text-sm border border-[#cc6600] text-[#cc6600] rounded-lg hover:bg-orange-50"
                  >
                    Connect wallet to sign transaction
                  </button>
                </div>
              )}
            </div>
          )}
        </div>
      )}

      {/* Step 3: Set Policy */}
      {walletInfo && ownerReady && (
        <div className="bg-white shadow rounded-lg p-6 mb-6">
          <h2 className="text-lg font-semibold text-gray-900 mb-6">
            <span className="inline-flex items-center justify-center w-6 h-6 rounded-full bg-[#cc6600] text-white text-xs mr-2">3</span>
            Set Spending Policy
          </h2>

          <div className="space-y-6">
            {/* Transaction Approval */}
            <div>
              <h3 className="text-sm font-semibold text-gray-800 mb-3">Transaction Approval</h3>
              <div
                className={`border-2 rounded-lg p-4 cursor-pointer transition-colors ${
                  requireApproval
                    ? 'border-[#cc6600] bg-orange-50'
                    : 'border-gray-200 hover:border-gray-300'
                }`}
                onClick={() => setRequireApproval(!requireApproval)}
              >
                <div className="flex items-start">
                  <input
                    type="checkbox"
                    checked={requireApproval}
                    onChange={(e) => setRequireApproval(e.target.checked)}
                    className="mt-1 mr-3 h-4 w-4 accent-[#cc6600]"
                  />
                  <div className="flex-1">
                    <p className="font-medium text-gray-900">Require personal approval</p>
                    <p className="text-sm text-gray-500 mt-1">
                      Transactions will need approval before being executed. You can approve or reject from the dashboard.
                    </p>
                  </div>
                </div>

                {requireApproval && (
                  <div className="mt-4 ml-7 space-y-3" onClick={(e) => e.stopPropagation()}>
                    <div className="grid grid-cols-2 gap-3">
                      <div>
                        <label className="block text-xs font-medium text-gray-600 mb-1">Required Approvals</label>
                        <input
                          type="number"
                          min="1"
                          value={approvalRequired}
                          onChange={(e) => setApprovalRequired(e.target.value)}
                          className="w-full border border-gray-300 rounded px-3 py-2 text-sm"
                        />
                      </div>
                      <div>
                        <label className="block text-xs font-medium text-gray-600 mb-1">Primary Approver</label>
                        <div className="px-3 py-2 text-sm bg-gray-50 border border-gray-200 rounded font-mono truncate">
                          {effectiveOwner} (admin)
                        </div>
                      </div>
                    </div>

                    <div>
                      <label className="block text-xs font-medium text-gray-600 mb-1">
                        Additional Approvers (one per line: account_id, role)
                      </label>
                      <textarea
                        value={additionalApprovers}
                        onChange={(e) => setAdditionalApprovers(e.target.value)}
                        placeholder={"alice.near, signer\nbob.near, signer"}
                        rows={3}
                        className="w-full border border-gray-300 rounded px-3 py-2 text-sm font-mono"
                      />
                      <p className="text-xs text-gray-400 mt-1">Roles: admin (can update policy), signer (can only approve)</p>
                    </div>

                    <div>
                      <label className="block text-xs font-medium text-gray-600 mb-2">Require approval for:</label>
                      {(() => {
                        const txTypeLabels: Record<string, string> = {
                          transfer: 'Transfer (send native currency)',
                          call: 'Contract call',
                          delete: 'Delete wallet',
                          intents_withdraw: 'Send cross-chain',
                          intents_swap: 'Swap',
                          intents_deposit: 'Deposit to Intents',
                        };
                        const directTypes = ['transfer', 'call', 'delete'] as const;
                        const intentsTypes = ['intents_withdraw', 'intents_swap', 'intents_deposit'] as const;
                        const renderCheckbox = (txType: string) => (
                          <label key={txType} className="flex items-center gap-1.5 text-sm cursor-pointer">
                            <input
                              type="checkbox"
                              checked={approvalTypes.has(txType)}
                              onChange={() => {
                                setApprovalTypes((prev) => {
                                  const next = new Set(prev);
                                  next.has(txType) ? next.delete(txType) : next.add(txType);
                                  return next;
                                });
                              }}
                              className="rounded border-gray-300"
                            />
                            <span>{txTypeLabels[txType] || txType}</span>
                          </label>
                        );
                        return (
                          <div className="space-y-3">
                            <div>
                              <span className="text-xs text-gray-400">Direct on-chain operations:</span>
                              <div className="flex flex-col gap-1 mt-1">
                                {directTypes.map(renderCheckbox)}
                              </div>
                            </div>
                            <div>
                              <span className="text-xs text-gray-400">NEAR Intents (use expiring quotes):</span>
                              <div className="flex flex-col gap-1 mt-1">
                                {intentsTypes.map(renderCheckbox)}
                              </div>
                            </div>
                          </div>
                        );
                      })()}
                      <p className="text-xs text-gray-400 mt-1">Unchecked types execute immediately without approval.</p>
                      {(approvalTypes.has('intents_swap') || approvalTypes.has('intents_deposit')) && (
                        <p className="text-xs text-red-600 font-medium mt-1 animate-pulse">
                          Warning: Intents operations (deposit, swap) use expiring quotes — approval delays may cause transaction failures.
                        </p>
                      )}
                    </div>
                  </div>
                )}
              </div>
            </div>

            {/* Shared policy form fields */}
            <PolicyFormFields policyForm={policyForm} onChange={setPolicyForm} apiKeyHash={apiKeyHash} knownKeyHashes={knownKeyHashes} onSaveKey={handleSaveKey} />
          </div>

          {/* Policy JSON Editor */}
          <PolicyJsonEditor
            policyJsonText={policyJsonText}
            onChangeText={(text) => { setPolicyJsonText(text); setJsonEdited(true); }}
            jsonEdited={jsonEdited}
            onReset={resetJson}
          />

          <div className="mt-4 pt-4 border-t flex items-center justify-between">
            <p className="text-xs text-gray-400">
              Policy will be encrypted in TEE and stored on-chain with <span className="font-mono">{effectiveOwner}</span> as owner.
            </p>
            <button
              onClick={handleSubmitPolicy}
              disabled={submitting || !isConnected}
              className="px-5 py-2 bg-gradient-to-r from-[#cc6600] to-[#d4a017] text-white rounded-lg font-medium hover:from-[#b35900] hover:to-[#c49016] disabled:opacity-50"
            >
              {submitting ? 'Encrypting & Storing...' : 'Store Policy On-Chain'}
            </button>
          </div>
        </div>
      )}

      {/* After success — next steps */}
      {success && (
        <div className="bg-white shadow rounded-lg p-6">
          <h2 className="text-lg font-semibold text-gray-900 mb-3">Next Steps</h2>
          <ul className="space-y-2 text-sm text-gray-700">
            <li>
              <Link href={`/wallet/approvals?key=${apiKey}`} className="text-[#cc6600] hover:text-[#b35900] font-medium">
                Approvals
              </Link>
              {' '}&mdash; review and approve pending transactions
            </li>
            <li>
              <Link href={`/wallet/manage?key=${apiKey}`} className="text-[#cc6600] hover:text-[#b35900] font-medium">
                Manage Wallets
              </Link>
              {' '}&mdash; edit policy, freeze wallet
            </li>
            <li>
              <Link href={`/wallet/audit?key=${apiKey}`} className="text-[#cc6600] hover:text-[#b35900] font-medium">
                Audit Log
              </Link>
              {' '}&mdash; view transaction history
            </li>
          </ul>
        </div>
      )}

      <WalletConnectionModal isOpen={showWalletModal} onClose={() => setShowWalletModal(false)} />
    </div>
  );
}
