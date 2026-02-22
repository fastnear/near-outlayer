'use client';

import { useState, useEffect, useCallback, Suspense } from 'react';
import { getCoordinatorApiUrl } from '@/lib/api';
import { useNearWallet } from '@/contexts/NearWalletContext';
import Link from 'next/link';
import { useParams, useRouter } from 'next/navigation';

interface ApprovalDetail {
  id: string;
  wallet_id: string;
  request_type: string;
  request_data: Record<string, unknown>;
  request_hash: string;
  required_approvals: number;
  status: string;
  expires_at: string;
  created_at: string;
  approvers: { approver_id: string; approver_role: string; signature: string; created_at: string }[];
}

export default function ApprovalDetailPage() {
  return (
    <Suspense fallback={<div className="max-w-4xl mx-auto py-8 text-gray-400">Loading...</div>}>
      <ApprovalDetailContent />
    </Suspense>
  );
}

function ApprovalDetailContent() {
  const params = useParams();
  const router = useRouter();
  const approvalId = params.id as string;

  const { network, contractId, signMessage, isConnected } = useNearWallet();
  const coordinatorUrl = getCoordinatorApiUrl(network);

  const [approval, setApproval] = useState<ApprovalDetail | null>(null);
  const [loading, setLoading] = useState(false);
  const [approving, setApproving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [success, setSuccess] = useState<string | null>(null);

  const loadApproval = useCallback(async () => {
    if (!approvalId) return;
    setLoading(true);
    setError(null);

    try {
      const resp = await fetch(
        `${coordinatorUrl}/wallet/v1/approval/${encodeURIComponent(approvalId)}`
      );
      if (!resp.ok) {
        const errorData = await resp.json().catch(() => ({}));
        throw new Error(errorData.message || `Failed to load approval: ${resp.status}`);
      }
      const data = await resp.json();
      setApproval(data);
    } catch (err) {
      setError((err as Error).message);
    } finally {
      setLoading(false);
    }
  }, [approvalId, coordinatorUrl]);

  useEffect(() => {
    loadApproval();
  }, [loadApproval]);

  const handleApprove = async () => {
    if (!approval) return;

    if (!isConnected) {
      setError('Connect your NEAR wallet to approve.');
      return;
    }

    setApproving(true);
    setError(null);

    try {
      // Generate 32-byte random nonce
      const nonceBytes = crypto.getRandomValues(new Uint8Array(32));
      const nonceBase64 = Buffer.from(nonceBytes).toString('base64');

      // Build message: "approve:{approval_id}:{request_hash}"
      const message = `approve:${approval.id}:${approval.request_hash}`;

      // Sign with NEAR wallet (NEP-413)
      const signed = await signMessage({
        message,
        recipient: contractId,
        nonce: nonceBase64,
      });

      if (!signed) {
        throw new Error('Signature cancelled');
      }

      const resp = await fetch(
        `${coordinatorUrl}/wallet/v1/approve/${approval.id}`,
        {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({
            signature: signed.signature,
            public_key: signed.publicKey,
            account_id: signed.accountId,
            nonce: nonceBase64,
          }),
        }
      );

      if (!resp.ok) {
        const errorData = await resp.json().catch(() => ({}));
        throw new Error(errorData.error || errorData.message || `Approval failed: ${resp.status}`);
      }

      const result = await resp.json();
      if (result.request_id) {
        setSuccess(`Threshold met! Operation executing (request: ${result.request_id}). Refresh to track status.`);
      } else {
        setSuccess(`Approved (${result.approved}/${result.required}). Waiting for more approvals.`);
      }

      setTimeout(() => loadApproval(), 2000);
    } catch (err) {
      setError((err as Error).message);
    } finally {
      setApproving(false);
    }
  };

  const formatDate = (dateStr: string) => new Date(dateStr).toLocaleString();
  const isExpired = approval ? new Date(approval.expires_at) < new Date() : false;
  const backUrl = '/wallet/approvals';

  return (
    <div className="max-w-4xl mx-auto">
      <div className="flex items-center space-x-3 mb-6">
        <Link href={backUrl} className="text-[#cc6600] hover:text-[#b35900]">
          &larr; Back to Approvals
        </Link>
      </div>

      <h1 className="text-3xl font-bold text-gray-900 mb-6">Approval Details</h1>

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

      {loading ? (
        <div className="flex items-center justify-center py-12">
          <svg className="animate-spin h-8 w-8 text-[#cc6600]" fill="none" viewBox="0 0 24 24">
            <circle className="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" strokeWidth="4" />
            <path className="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4z" />
          </svg>
          <span className="ml-3 text-gray-500">Loading...</span>
        </div>
      ) : !approval ? (
        <div className="bg-white shadow rounded-lg p-8 text-center">
          <p className="text-gray-500">Approval not found.</p>
        </div>
      ) : (
        <div className="space-y-6">
          {/* Status card */}
          <div className="bg-white shadow rounded-lg p-6 border border-gray-200">
            <div className="flex items-center justify-between mb-4">
              <div className="flex items-center space-x-3">
                <span className={`inline-flex items-center px-3 py-1 rounded-full text-sm font-medium ${
                  approval.status === 'pending' ? 'bg-yellow-100 text-yellow-800' :
                  approval.status === 'approved' ? 'bg-green-100 text-green-800' :
                  approval.status === 'expired' ? 'bg-gray-100 text-gray-800' :
                  'bg-red-100 text-red-800'
                }`}>
                  {approval.status.toUpperCase()}
                </span>
                <span className="text-sm text-gray-500">{approval.request_type}</span>
              </div>
              <span className="text-sm text-gray-400">
                {approval.approvers?.length || 0} / {approval.required_approvals} approved
              </span>
            </div>

            <div className="grid grid-cols-2 gap-4 text-sm">
              <div>
                <p className="text-gray-500">Wallet</p>
                <p className="font-mono text-gray-900 text-xs break-all">{approval.wallet_id}</p>
              </div>
              <div>
                <p className="text-gray-500">Request Hash</p>
                <p className="font-mono text-gray-900 text-xs break-all">{approval.request_hash}</p>
              </div>
              <div>
                <p className="text-gray-500">Created</p>
                <p className="text-gray-900">{formatDate(approval.created_at)}</p>
              </div>
              <div>
                <p className="text-gray-500">Expires</p>
                <p className={`${isExpired ? 'text-red-600' : 'text-gray-900'}`}>
                  {formatDate(approval.expires_at)}
                  {isExpired && ' (EXPIRED)'}
                </p>
              </div>
            </div>
          </div>

          {/* Request data */}
          <div className="bg-white shadow rounded-lg p-6 border border-gray-200">
            <h2 className="text-lg font-semibold text-gray-900 mb-3">Request Data</h2>
            <pre className="bg-gray-50 rounded p-4 text-sm text-gray-700 overflow-x-auto">
              {JSON.stringify(approval.request_data, null, 2)}
            </pre>
          </div>

          {/* Existing approvers */}
          {approval.approvers && approval.approvers.length > 0 && (
            <div className="bg-white shadow rounded-lg p-6 border border-gray-200">
              <h2 className="text-lg font-semibold text-gray-900 mb-3">Approvers</h2>
              <div className="space-y-2">
                {approval.approvers.map((a, i) => (
                  <div key={i} className="flex items-center justify-between bg-green-50 rounded p-3">
                    <div>
                      <p className="text-sm font-mono text-gray-800">{a.approver_id}</p>
                      <p className="text-xs text-gray-500">Role: {a.approver_role}</p>
                    </div>
                    <p className="text-xs text-gray-400">{formatDate(a.created_at)}</p>
                  </div>
                ))}
              </div>
            </div>
          )}

          {/* Approve button */}
          {approval.status === 'pending' && !isExpired && (
            <div className="flex justify-end space-x-3">
              <button
                onClick={() => router.push(backUrl)}
                className="px-6 py-3 border border-gray-300 text-gray-700 rounded-lg font-medium hover:bg-gray-50"
              >
                Back
              </button>
              <button
                onClick={handleApprove}
                disabled={approving || !isConnected}
                className="px-6 py-3 bg-gradient-to-r from-[#cc6600] to-[#d4a017] text-white rounded-lg font-medium hover:from-[#b35900] hover:to-[#c49016] disabled:opacity-50"
              >
                {approving ? 'Approving...' : 'Approve'}
              </button>
            </div>
          )}

          {!isConnected && approval.status === 'pending' && (
            <div className="bg-yellow-50 border border-yellow-200 rounded-md p-3">
              <p className="text-sm text-yellow-800">
                Connect your NEAR wallet to approve this request.
              </p>
            </div>
          )}
        </div>
      )}
    </div>
  );
}
