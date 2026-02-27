'use client';

import { useState, useEffect, useRef, useCallback, Suspense } from 'react';
import { useSearchParams, useRouter } from 'next/navigation';
import { useNearWallet } from '@/contexts/NearWalletContext';
import WalletConnectionModal from '@/components/WalletConnectionModal';
import { getCoordinatorApiUrl } from '@/lib/api';
import Link from 'next/link';
import { findKeyForWallets, saveWalletKey } from '@/lib/wallet-keys';

interface PendingApproval {
  id: string;
  wallet_id: string;
  request_type: string;
  request_data: Record<string, unknown>;
  required_approvals: number;
  approved_count: number;
  request_hash: string;
  expires_at: string;
  created_at: string;
  // display helpers
  wallet_pubkey?: string;
}

/** Auto-refresh interval in ms (30 seconds) */
const REFRESH_INTERVAL = 30_000;

export default function WalletApprovalsPage() {
  return (
    <Suspense fallback={<div className="max-w-4xl mx-auto py-8 text-gray-400">Loading...</div>}>
      <WalletApprovalsContent />
    </Suspense>
  );
}

function WalletApprovalsContent() {
  const { accountId, isConnected, network, contractId, viewMethod, signMessage } = useNearWallet();
  const coordinatorUrl = getCoordinatorApiUrl(network);
  const searchParams = useSearchParams();
  const router = useRouter();

  const [showWalletModal, setShowWalletModal] = useState(false);
  const [approvals, setApprovals] = useState<PendingApproval[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [success, setSuccess] = useState<string | null>(null);
  const [approvingId, setApprovingId] = useState<string | null>(null);
  const [hasPolicies, setHasPolicies] = useState(false);
  const [nextRefreshIn, setNextRefreshIn] = useState<number | null>(null);
  // Cached wallet pubkeys (loaded once from contract, reused for polling)
  const walletPubkeysRef = useRef<string[]>([]);

  // API key for approve action
  const [apiKey, setApiKey] = useState<string>('');
  const [showApiKeyPrompt, setShowApiKeyPrompt] = useState(false);
  const [pendingApprovalId, setPendingApprovalId] = useState<string | null>(null);

  // Read API key from URL params on mount
  useEffect(() => {
    const keyFromUrl = searchParams.get('key');
    if (keyFromUrl) {
      setApiKey(keyFromUrl);
    }
  }, [searchParams]);

  // Try to load API key from localStorage when approvals load
  useEffect(() => {
    if (!apiKey && approvals.length > 0) {
      const walletPubkeys = approvals
        .map((a) => a.wallet_pubkey)
        .filter((pk): pk is string => !!pk);
      const savedKey = findKeyForWallets(walletPubkeys);
      if (savedKey) {
        setApiKey(savedKey);
      }
    }
  }, [approvals, apiKey]);

  // Fetch pending approvals for cached wallet pubkeys (coordinator only, no RPC)
  const fetchPendingApprovals = useCallback(async (pubkeys: string[], silent = false) => {
    const allApprovals: PendingApproval[] = [];
    for (const pubkey of pubkeys) {
      try {
        const resp = await fetch(
          `${coordinatorUrl}/wallet/v1/pending_approvals_by_pubkey?near_pubkey=${encodeURIComponent(pubkey)}`
        );
        if (!resp.ok) continue;
        const data = await resp.json();
        if (data.pending_approvals) {
          for (const pa of data.pending_approvals) {
            allApprovals.push({ ...pa, wallet_pubkey: pubkey });
          }
        }
      } catch {
        // skip individual wallet errors
      }
    }
    setApprovals(allApprovals);
  }, [coordinatorUrl]);

  // Initial load: get wallet pubkeys from contract (once), then fetch approvals
  const loadApprovals = useCallback(async () => {
    if (!accountId || !contractId) return;
    setLoading(true);
    setError(null);

    try {
      const wallets = await viewMethod({
        contractId,
        method: 'get_wallet_policies_by_owner',
        args: { owner: accountId },
      }).catch(() => []) as Array<{ wallet_pubkey: string }>;

      const pubkeys = wallets.map(w => w.wallet_pubkey);
      walletPubkeysRef.current = pubkeys;
      setHasPolicies(pubkeys.length > 0);

      if (pubkeys.length === 0) {
        setApprovals([]);
        return;
      }

      await fetchPendingApprovals(pubkeys);
    } catch (err) {
      setError((err as Error).message);
    } finally {
      setLoading(false);
    }
  }, [accountId, contractId, viewMethod, fetchPendingApprovals]);

  // Initial load when connected
  useEffect(() => {
    if (isConnected && accountId) {
      loadApprovals();
    }
  }, [isConnected, accountId, loadApprovals]);

  // Auto-refresh timer: poll coordinator for pending approvals (no contract RPC)
  useEffect(() => {
    if (!hasPolicies || !isConnected) {
      setNextRefreshIn(null);
      return;
    }

    let countdown = REFRESH_INTERVAL / 1000;
    setNextRefreshIn(countdown);

    const tick = setInterval(() => {
      countdown -= 1;
      if (countdown <= 0) {
        fetchPendingApprovals(walletPubkeysRef.current, true);
        countdown = REFRESH_INTERVAL / 1000;
      }
      setNextRefreshIn(countdown);
    }, 1000);

    return () => clearInterval(tick);
  }, [hasPolicies, isConnected, fetchPendingApprovals]);

  // Approve a pending request (requires NEAR wallet signature, not API key)
  const handleApprove = async (approvalId: string) => {
    const approval = approvals.find((a) => a.id === approvalId);
    if (!approval) return;

    setApprovingId(approvalId);
    setError(null);
    setSuccess(null);

    try {
      // Generate 32-byte random nonce
      const nonceBytes = crypto.getRandomValues(new Uint8Array(32));
      const nonceBase64 = Buffer.from(nonceBytes).toString('base64');

      // Build message: "approve:{approval_id}:{request_hash}"
      const message = `approve:${approvalId}:${approval.request_hash}`;

      // Sign with NEAR wallet (NEP-413)
      const signed = await signMessage({
        message,
        recipient: contractId,
        nonce: nonceBase64,
      });

      if (!signed) {
        throw new Error('Signature cancelled');
      }

      // Send signature to coordinator (no Bearer token needed)
      const resp = await fetch(`${coordinatorUrl}/wallet/v1/approve/${approvalId}`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          signature: signed.signature,
          public_key: signed.publicKey,
          account_id: signed.accountId,
          nonce: nonceBase64,
        }),
      });
      if (!resp.ok) {
        const err = await resp.json().catch(() => ({ error: resp.statusText }));
        throw new Error(err.error || err.message || `API error: ${resp.status}`);
      }
      const data = await resp.json();
      if (data.request_id) {
        // Threshold met — redirect to audit page
        const auditUrl = apiKey ? `/wallet/audit?key=${encodeURIComponent(apiKey)}` : '/wallet/audit';
        router.push(auditUrl);
        return;
      } else {
        setSuccess(`Approved (${data.approved}/${data.required}). Waiting for more approvals.`);
      }

      // Reload
      await loadApprovals();
    } catch (err) {
      setError((err as Error).message);
    } finally {
      setApprovingId(null);
    }
  };

  // Reject a pending request
  const handleReject = async (approvalId: string) => {
    if (!apiKey) {
      setPendingApprovalId(approvalId);
      setShowApiKeyPrompt(true);
      return;
    }

    if (!confirm('Reject this request? This cannot be undone.')) return;

    setApprovingId(approvalId);
    setError(null);
    setSuccess(null);

    try {
      const resp = await fetch(`${coordinatorUrl}/wallet/v1/reject/${approvalId}`, {
        method: 'POST',
        headers: {
          'Authorization': `Bearer ${apiKey}`,
          'Content-Type': 'application/json',
        },
        body: JSON.stringify({ approver_account: accountId }),
      });
      if (!resp.ok) {
        const err = await resp.json().catch(() => ({ error: resp.statusText }));
        throw new Error(err.error || err.message || `API error: ${resp.status}`);
      }
      setSuccess('Request rejected.');
      await loadApprovals();
    } catch (err) {
      setError((err as Error).message);
    } finally {
      setApprovingId(null);
    }
  };

  const handleApiKeySubmit = () => {
    setShowApiKeyPrompt(false);
    if (pendingApprovalId && apiKey) {
      // Save key to localStorage for the wallet
      const approval = approvals.find((a) => a.id === pendingApprovalId);
      if (approval?.wallet_pubkey) {
        saveWalletKey(approval.wallet_pubkey, apiKey);
      }
      handleReject(pendingApprovalId);
    }
    setPendingApprovalId(null);
  };

  const formatDate = (dateStr: string) => {
    return new Date(dateStr).toLocaleString();
  };

  const isExpired = (expiresAt: string) => {
    return new Date(expiresAt) < new Date();
  };

  // Not connected — show connect prompt
  if (!isConnected) {
    return (
      <div className="max-w-4xl mx-auto">
        <h1 className="text-3xl font-bold text-gray-900 mb-6">Wallet Approvals</h1>
        <div className="bg-white shadow rounded-lg p-8 text-center">
          <p className="text-gray-600 mb-4">
            Connect your NEAR wallet to view pending approvals for your AI wallets.
          </p>
          <button
            onClick={() => setShowWalletModal(true)}
            className="px-6 py-2 bg-gradient-to-r from-[#cc6600] to-[#d4a017] text-white rounded-lg font-medium hover:from-[#b35900] hover:to-[#c49016]"
          >
            Connect Wallet
          </button>
        </div>
        {showWalletModal && <WalletConnectionModal isOpen={showWalletModal} onClose={() => setShowWalletModal(false)} />}
      </div>
    );
  }

  return (
    <div className="max-w-4xl mx-auto">
      <div className="flex items-center justify-between mb-6">
        <h1 className="text-3xl font-bold text-gray-900">
          Wallet Approvals
          {approvals.length > 0 && (
            <span className="ml-2 inline-flex items-center justify-center px-2.5 py-0.5 rounded-full text-sm font-bold text-white bg-red-500">
              {approvals.length}
            </span>
          )}
        </h1>
        <div className="flex items-center space-x-3">
          <span className="text-xs text-gray-400 font-mono">
            {accountId}
          </span>
          {nextRefreshIn !== null && (
            <span className="text-xs text-gray-400">
              {nextRefreshIn}s
            </span>
          )}
          <button
            onClick={() => loadApprovals()}
            className="text-sm text-[#cc6600] hover:text-[#b35900] font-medium"
          >
            Refresh
          </button>
          <Link
            href="/wallet/manage"
            className="text-sm text-[#cc6600] hover:text-[#b35900] font-medium"
          >
            Manage
          </Link>
          <Link
            href="/wallet/audit"
            className="text-sm text-[#cc6600] hover:text-[#b35900] font-medium"
          >
            Audit
          </Link>
        </div>
      </div>

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

      {/* API key prompt dialog */}
      {showApiKeyPrompt && (
        <div className="mb-4 bg-blue-50 border border-blue-200 rounded-md p-4">
          <p className="text-sm text-blue-800 mb-2">
            Enter the wallet API key to reject this request.
            It will be saved in this browser for future use.
          </p>
          <div className="flex gap-3">
            <input
              type="text"
              value={apiKey}
              onChange={(e) => setApiKey(e.target.value)}
              onKeyDown={(e) => e.key === 'Enter' && handleApiKeySubmit()}
              placeholder="wk_..."
              className="flex-1 px-4 py-2 border border-gray-300 rounded-lg focus:ring-2 focus:ring-[#cc6600] focus:border-transparent font-mono text-sm"
              autoFocus
            />
            <button
              onClick={handleApiKeySubmit}
              disabled={!apiKey.trim()}
              className="px-4 py-2 bg-gradient-to-r from-[#cc6600] to-[#d4a017] text-white rounded-lg text-sm font-medium hover:from-[#b35900] hover:to-[#c49016] disabled:opacity-50"
            >
              Submit
            </button>
            <button
              onClick={() => { setShowApiKeyPrompt(false); setPendingApprovalId(null); }}
              className="px-4 py-2 text-gray-600 hover:text-gray-800 text-sm"
            >
              Cancel
            </button>
          </div>
        </div>
      )}

      {loading ? (
        <div className="flex items-center justify-center py-12">
          <svg className="animate-spin h-8 w-8 text-[#cc6600]" fill="none" viewBox="0 0 24 24">
            <circle className="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" strokeWidth="4" />
            <path className="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4z" />
          </svg>
          <span className="ml-3 text-gray-500">Loading approvals...</span>
        </div>
      ) : !hasPolicies ? (
        <div className="bg-white shadow rounded-lg p-8 text-center">
          <p className="text-gray-500">No wallet policies found for this account.</p>
          <p className="text-sm text-gray-400 mt-2">
            Go to <Link href="/wallet/manage" className="text-[#cc6600] hover:underline">Manage</Link> to set up policies for your AI wallets.
          </p>
        </div>
      ) : approvals.length === 0 ? (
        <div className="bg-white shadow rounded-lg p-8 text-center">
          <p className="text-gray-500">No pending approvals.</p>
          <p className="text-sm text-gray-400 mt-2">
            Approvals appear when a wallet operation requires multisig confirmation.
          </p>
        </div>
      ) : (
        <div className="space-y-4">
          {approvals.map((approval) => (
            <div
              key={approval.id}
              className={`bg-white shadow rounded-lg border ${
                isExpired(approval.expires_at)
                  ? 'border-gray-300 opacity-60'
                  : 'border-[#cc6600]'
              }`}
            >
              <div className="px-4 py-4 sm:px-6">
                <div className="flex items-center justify-between">
                  <div>
                    <div className="flex items-center space-x-2">
                      <span className="inline-flex items-center px-2.5 py-0.5 rounded-full text-xs font-medium bg-yellow-100 text-yellow-800">
                        {approval.request_type}
                      </span>
                      {isExpired(approval.expires_at) && (
                        <span className="inline-flex items-center px-2.5 py-0.5 rounded-full text-xs font-medium bg-gray-100 text-gray-800">
                          Expired
                        </span>
                      )}
                    </div>
                    {approval.wallet_pubkey && (
                      <p className="mt-1 text-xs text-gray-400 font-mono">
                        Wallet: {approval.wallet_pubkey.substring(0, 24)}...
                      </p>
                    )}
                  </div>
                  <div className="text-right">
                    <p className="text-sm text-gray-500">
                      {approval.approved_count} / {approval.required_approvals} approved
                    </p>
                    <p className="text-xs text-gray-400 mt-1">
                      Expires: {formatDate(approval.expires_at)}
                    </p>
                  </div>
                </div>

                {/* Request details */}
                <div className="mt-3 bg-gray-50 rounded p-3">
                  <pre className="text-xs text-gray-700 overflow-x-auto">
                    {JSON.stringify(approval.request_data, null, 2)}
                  </pre>
                </div>

                {/* Action buttons */}
                {!isExpired(approval.expires_at) && (
                  <div className="mt-4 flex items-center justify-between">
                    <p className="text-xs text-gray-400">
                      Created: {formatDate(approval.created_at)}
                    </p>
                    <div className="flex items-center space-x-2">
                      <button
                        onClick={() => handleReject(approval.id)}
                        disabled={approvingId === approval.id}
                        className="px-4 py-2 border border-red-300 text-red-600 text-sm rounded-lg font-medium hover:bg-red-50 disabled:opacity-50"
                      >
                        Reject
                      </button>
                      <button
                        onClick={() => handleApprove(approval.id)}
                        disabled={approvingId === approval.id}
                        className="px-4 py-2 bg-gradient-to-r from-[#cc6600] to-[#d4a017] text-white text-sm rounded-lg font-medium hover:from-[#b35900] hover:to-[#c49016] disabled:opacity-50"
                      >
                        {approvingId === approval.id ? 'Processing...' : 'Approve'}
                      </button>
                    </div>
                  </div>
                )}
              </div>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
