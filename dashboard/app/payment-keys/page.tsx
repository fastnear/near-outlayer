'use client';

import { useState, useEffect, useCallback } from 'react';
import { actionCreators } from '@near-js/transactions';
import { useNearWallet } from '@/contexts/NearWalletContext';
import WalletConnectionModal from '@/components/WalletConnectionModal';
import { getCoordinatorApiUrl } from '@/lib/api';
import { PaymentKeyData, PaymentKeyBalance, CreationState } from './components/types';
import { CreateKeyForm } from './components/CreateKeyForm';
import { PaymentKeyCard } from './components/PaymentKeyCard';
import { TopUpModal } from './components/TopUpModal';

interface UserSecret {
  accessor: { System?: { PaymentKey?: Record<string, never> } } | Record<string, unknown>;
  profile: string;
  created_at: number;
  updated_at: number;
  storage_deposit: string;
  access: unknown;
}

export default function PaymentKeysPage() {
  const {
    accountId,
    isConnected,
    signAndSendTransaction,
    contractId,
    viewMethod,
    network,
    stablecoin,
    shouldReopenModal,
    clearReopenModal,
  } = useNearWallet();
  const coordinatorUrl = getCoordinatorApiUrl(network);

  // Payment keys list
  const [paymentKeys, setPaymentKeys] = useState<PaymentKeyData[]>([]);
  const [balances, setBalances] = useState<Map<number, PaymentKeyBalance>>(new Map());
  const [loading, setLoading] = useState(false);
  const [showWalletModal, setShowWalletModal] = useState(false);

  // UI state
  const [error, setError] = useState<string | null>(null);
  const [success, setSuccess] = useState<string | null>(null);

  // Creation flow
  const [showCreateForm, setShowCreateForm] = useState(false);
  const [creationState, setCreationState] = useState<CreationState>({ step: 'form' });

  // TopUp modal
  const [topUpKey, setTopUpKey] = useState<PaymentKeyData | null>(null);

  // Load payment keys from contract
  const loadPaymentKeys = useCallback(async () => {
    if (!accountId) return;

    setLoading(true);
    try {
      const secrets = await viewMethod({
        contractId,
        method: 'list_user_secrets',
        args: { account_id: accountId },
      }) as UserSecret[];

      // Filter to only Payment Keys (System accessor)
      // Contract returns { System: 'PaymentKey' } - System is a string, not object
      const paymentKeySecrets = (secrets || []).filter(
        (s) => {
          if (!s.accessor || typeof s.accessor !== 'object') return false;
          if (!('System' in s.accessor)) return false;
          const system = (s.accessor as { System: unknown }).System;
          // System is a unit variant string 'PaymentKey', not an object
          return system === 'PaymentKey';
        }
      );

      // Convert to PaymentKeyData
      const keys: PaymentKeyData[] = paymentKeySecrets.map((s) => ({
        nonce: parseInt(s.profile, 10),
        created_at: s.created_at,
        updated_at: s.updated_at,
        storage_deposit: s.storage_deposit,
        project_ids: [], // Not available without decryption
        max_per_call: '0',
        initial_balance: '0',
      }));

      // Sort by nonce
      keys.sort((a, b) => a.nonce - b.nonce);
      setPaymentKeys(keys);

      // Load balances from coordinator
      await loadBalances(keys);
    } catch (err) {
      console.error('Failed to load payment keys:', err);
      setError(`Failed to load payment keys: ${(err as Error).message}`);
    } finally {
      setLoading(false);
    }
  }, [accountId, contractId, viewMethod]);

  // Load balances from coordinator
  const loadBalances = useCallback(async (keys: PaymentKeyData[]) => {
    if (!accountId || keys.length === 0) return;

    const newBalances = new Map<number, PaymentKeyBalance>();

    for (const key of keys) {
      try {
        const response = await fetch(
          `${coordinatorUrl}/public/payment-keys/${accountId}/${key.nonce}/balance`
        );
        if (response.ok) {
          const balance = await response.json();
          newBalances.set(key.nonce, balance);
        }
      } catch (err) {
        console.warn(`Failed to load balance for nonce ${key.nonce}:`, err);
      }
    }

    setBalances(newBalances);
  }, [accountId, coordinatorUrl]);

  // Auto-open modal if we switched networks
  useEffect(() => {
    if (shouldReopenModal && !isConnected) {
      setShowWalletModal(true);
      clearReopenModal();
    }
  }, [shouldReopenModal, isConnected, clearReopenModal]);

  // Load payment keys when connected
  useEffect(() => {
    if (isConnected && accountId) {
      loadPaymentKeys();
    }
  }, [isConnected, accountId, loadPaymentKeys]);

  // State for pending TopUp after first transaction
  const [pendingTopUp, setPendingTopUp] = useState<{
    nonce: number;
    depositAmount: string;
    generatedKey: string;
  } | null>(null);

  // Restore Payment Key creation state after wallet redirect
  // Wallet-selector redirects with ?transactionHashes=xxx after successful tx
  useEffect(() => {
    if (!accountId) return;

    const storageKey = `payment_key_creation_${accountId}`;
    const savedState = localStorage.getItem(storageKey);

    if (!savedState) return;

    try {
      const { key, nonce, step, depositAmount, timestamp } = JSON.parse(savedState);
      // Only restore if saved within the last 10 minutes
      const tenMinutesAgo = Date.now() - 10 * 60 * 1000;
      if (timestamp <= tenMinutesAgo || !key || typeof nonce !== 'number') {
        localStorage.removeItem(storageKey);
        return;
      }

      // Check URL for transaction result (wallet-selector adds ?transactionHashes=xxx on success)
      const urlParams = new URLSearchParams(window.location.search);
      const transactionHashes = urlParams.get('transactionHashes');
      const errorCode = urlParams.get('errorCode');

      // Clean URL params after reading
      if (transactionHashes || errorCode) {
        window.history.replaceState({}, '', window.location.pathname);
      }

      // Handle transaction error
      if (errorCode) {
        localStorage.removeItem(storageKey);
        setError('Transaction was rejected or failed.');
        return;
      }

      // No transaction result yet - don't restore (user might be in the middle of approval)
      if (!transactionHashes) {
        return;
      }

      // Transaction succeeded - check which step we were on
      if (step === 'store_secrets') {
        // First transaction done, need to do TopUp
        // Show key and set up pending TopUp
        setCreationState({ step: 'complete', generatedKey: key, nonce });
        setPendingTopUp({ nonce, depositAmount: depositAmount || '2', generatedKey: key });
        setSuccess('Key stored! Click "Continue TopUp" to add initial balance.');
        // Don't remove localStorage yet - we'll need it if user refreshes before TopUp

      } else if (step === 'topup') {
        // Second transaction done - all complete!
        setCreationState({ step: 'complete', generatedKey: key, nonce });
        setSuccess('Payment Key created with initial balance! Copy the key now.');
        localStorage.removeItem(storageKey);
        // Reload keys to show the new one
        loadPaymentKeys();

      } else {
        // Unknown step, just show the key
        setCreationState({ step: 'complete', generatedKey: key, nonce });
        localStorage.removeItem(storageKey);
      }

    } catch {
      localStorage.removeItem(storageKey);
    }
  }, [accountId, loadPaymentKeys]);

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

  // Get next available nonce (starting from 1, not 0)
  const getNextNonce = useCallback((): number => {
    if (paymentKeys.length === 0) return 1;
    const maxNonce = Math.max(...paymentKeys.map((k) => k.nonce));
    return maxNonce + 1;
  }, [paymentKeys]);

  // Handle creation complete (first transaction done, TopUp still needed)
  const handleCreationComplete = useCallback((generatedKey: string, nonce: number) => {
    setCreationState({ step: 'complete', generatedKey, nonce });
    setShowCreateForm(false);
    loadPaymentKeys();

    // Check if we need to do TopUp (read deposit amount from localStorage)
    const storageKey = `payment_key_creation_${accountId}`;
    const savedState = localStorage.getItem(storageKey);
    let depositAmount = '2'; // default
    if (savedState) {
      try {
        const parsed = JSON.parse(savedState);
        depositAmount = parsed.depositAmount || '2';
      } catch {
        // ignore
      }
    }

    // Set up pending TopUp for the second transaction
    // User will need to click "Continue: Add Balance" button
    setPendingTopUp({ nonce, depositAmount, generatedKey });
    setSuccess('Key created! Now click "Continue: Add Balance" to top up.');
  }, [loadPaymentKeys, accountId]);

  // Handle creation error
  const handleCreationError = useCallback((error: string) => {
    setCreationState({ step: 'error', error });
    setError(error);
  }, []);

  // Handle TopUp complete
  const handleTopUpComplete = useCallback(() => {
    setTopUpKey(null);
    setPendingTopUp(null);
    loadPaymentKeys();
    setSuccess('Balance topped up successfully!');
  }, [loadPaymentKeys]);

  // Handle continuing TopUp after first transaction (store_secrets) completed
  const handleContinueTopUp = useCallback(async () => {
    if (!pendingTopUp || !accountId) return;

    const { nonce, depositAmount, generatedKey } = pendingTopUp;
    const storageKey = `payment_key_creation_${accountId}`;

    try {
      // Update localStorage step to 'topup' before transaction
      localStorage.setItem(storageKey, JSON.stringify({
        key: generatedKey,
        nonce,
        step: 'topup',
        depositAmount,
        timestamp: Date.now(),
      }));

      const depositAmountMinimal = (parseFloat(depositAmount) * Math.pow(10, stablecoin.decimals)).toString();

      const ftTransferArgs = {
        receiver_id: contractId,
        amount: depositAmountMinimal,
        msg: JSON.stringify({
          action: 'top_up_payment_key',
          nonce: nonce,
        }),
      };

      const ftAction = actionCreators.functionCall(
        'ft_transfer_call',
        ftTransferArgs,
        BigInt('100000000000000'), // 100 TGas
        BigInt('1') // 1 yoctoNEAR
      );

      const callbackUrl = window.location.href.split('?')[0];

      await signAndSendTransaction({
        receiverId: stablecoin.contract,
        actions: [ftAction],
        callbackUrl,
      });

      // If we get here (non-redirect wallet), transaction succeeded
      localStorage.removeItem(storageKey);
      setPendingTopUp(null);
      setSuccess('Payment Key created with initial balance! Copy the key now.');
      loadPaymentKeys();

    } catch (err) {
      console.error('Failed to top up:', err);
      setError(`Failed to add balance: ${(err as Error).message}`);
    }
  }, [pendingTopUp, accountId, stablecoin, contractId, signAndSendTransaction, loadPaymentKeys]);

  // Handle delete key
  const handleDeleteKey = useCallback(async (key: PaymentKeyData) => {
    try {
      // Use delete_payment_key with yield/resume:
      // 1. Contract emits DeletePaymentKey event
      // 2. Worker receives event, deletes from coordinator PostgreSQL
      // 3. Worker resumes on contract
      // 4. Contract callback deletes the secret
      const action = actionCreators.functionCall(
        'delete_payment_key',
        {
          nonce: key.nonce,
        },
        BigInt('100000000000000'), // 100 TGas (needs gas for yield/resume)
        BigInt('1') // 1 yoctoNEAR required for security
      );

      await signAndSendTransaction({
        receiverId: contractId,
        actions: [action],
      });

      setSuccess('Payment Key deleted. Storage deposit refunded.');
      loadPaymentKeys();
    } catch (err) {
      console.error('Failed to delete key:', err);
      setError(`Failed to delete: ${(err as Error).message}`);
    }
  }, [contractId, signAndSendTransaction, loadPaymentKeys]);

  return (
    <div className="max-w-4xl mx-auto">
      {/* Header */}
      <div className="sm:flex sm:items-center sm:justify-between">
        <div>
          <h1 className="text-3xl font-bold text-gray-900">Payment Keys</h1>
          <p className="mt-2 text-sm text-gray-700">
            Manage API keys for HTTPS calls to OutLayer projects
          </p>
        </div>
        {isConnected && (
          <div className="mt-4 sm:mt-0">
            <button
              onClick={() => {
                setShowCreateForm(true);
                setCreationState({ step: 'form' });
              }}
              className="inline-flex items-center px-4 py-2 border border-transparent text-sm font-medium rounded-md text-white bg-gradient-to-r from-[#cc6600] to-[#d4a017] hover:from-[#b35900] hover:to-[#c49016] shadow-sm"
            >
              <svg className="h-5 w-5 mr-2" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M12 4v16m8-8H4" />
              </svg>
              New Key
            </button>
          </div>
        )}
      </div>

      {/* Connect Wallet Button */}
      {!isConnected && (
        <div className="mt-8 flex justify-center">
          <button
            onClick={() => setShowWalletModal(true)}
            className="inline-flex items-center px-6 py-3 border border-transparent text-base font-medium rounded-md text-white bg-gradient-to-r from-[#cc6600] to-[#d4a017] hover:from-[#b35900] hover:to-[#c49016] shadow-sm hover:shadow-md transition-all"
          >
            Connect Wallet
          </button>
        </div>
      )}

      {/* Wallet Modal */}
      <WalletConnectionModal
        isOpen={showWalletModal}
        onClose={() => setShowWalletModal(false)}
      />

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

      {/* Show generated key after creation */}
      {creationState.step === 'complete' && creationState.generatedKey && (
        <div className="mt-6 bg-yellow-50 border border-yellow-200 rounded-lg p-4">
          <h3 className="font-semibold text-yellow-800 mb-2">Your Payment Key (copy now!)</h3>
          <p className="text-sm text-yellow-700 mb-3">
            This key will NOT be shown again. Store it securely.
          </p>
          <div className="flex gap-2">
            <code className="flex-1 bg-white border border-yellow-300 p-3 rounded font-mono text-sm break-all text-gray-900">
              {accountId}:{creationState.nonce}:{creationState.generatedKey}
            </code>
            <button
              onClick={() => {
                navigator.clipboard.writeText(
                  `${accountId}:${creationState.nonce}:${creationState.generatedKey}`
                );
                setSuccess('Copied to clipboard!');
              }}
              className="bg-[#cc6600] hover:bg-[#b35900] text-white px-4 py-2 rounded"
            >
              Copy
            </button>
          </div>
          <p className="text-xs text-yellow-700 mt-2">
            Format: owner:nonce:key — Use this in X-Payment-Key header
          </p>

          {/* Continue TopUp button - shown when first transaction completed but TopUp pending */}
          {pendingTopUp && pendingTopUp.nonce === creationState.nonce && (
            <div className="mt-4 pt-4 border-t border-yellow-300">
              <p className="text-sm text-yellow-800 mb-2">
                Step 2: Add initial balance of ${pendingTopUp.depositAmount} {stablecoin.symbol}
              </p>
              <button
                onClick={handleContinueTopUp}
                className="w-full bg-gradient-to-r from-[#cc6600] to-[#d4a017] hover:from-[#b35900] hover:to-[#c49016] text-white px-4 py-2 rounded font-medium"
              >
                Continue: Add Balance
              </button>
            </div>
          )}
        </div>
      )}

      {/* Create form modal */}
      {showCreateForm && (
        <CreateKeyForm
          accountId={accountId!}
          contractId={contractId}
          stablecoin={stablecoin}
          nextNonce={getNextNonce()}
          coordinatorUrl={coordinatorUrl}
          signAndSendTransaction={signAndSendTransaction}
          onComplete={handleCreationComplete}
          onError={handleCreationError}
          onCancel={() => setShowCreateForm(false)}
          creationState={creationState}
          setCreationState={setCreationState}
        />
      )}

      {/* TopUp modal */}
      {topUpKey && (
        <TopUpModal
          accountId={accountId!}
          nonce={topUpKey.nonce}
          contractId={contractId}
          stablecoin={stablecoin}
          signAndSendTransaction={signAndSendTransaction}
          onComplete={handleTopUpComplete}
          onCancel={() => setTopUpKey(null)}
        />
      )}

      {/* Payment keys list */}
      {isConnected && (
        <div className="mt-8">
          {loading ? (
            <div className="flex items-center justify-center py-12">
              <svg className="animate-spin h-8 w-8 text-[#cc6600]" fill="none" viewBox="0 0 24 24">
                <circle className="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" strokeWidth="4" />
                <path className="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4z" />
              </svg>
              <span className="ml-3 text-gray-500">Loading payment keys...</span>
            </div>
          ) : paymentKeys.length === 0 ? (
            <div className="text-center py-12">
              <svg className="mx-auto h-12 w-12 text-gray-400" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M15 7a2 2 0 012 2m4 0a6 6 0 01-7.743 5.743L11 17H9v2H7v2H4a1 1 0 01-1-1v-2.586a1 1 0 01.293-.707l5.964-5.964A6 6 0 1121 9z" />
              </svg>
              <h3 className="mt-2 text-sm font-medium text-gray-900">No payment keys</h3>
              <p className="mt-1 text-sm text-gray-500">Get started by creating a new payment key.</p>
              <div className="mt-6">
                <button
                  onClick={() => {
                    setShowCreateForm(true);
                    setCreationState({ step: 'form' });
                  }}
                  className="inline-flex items-center px-4 py-2 border border-transparent text-sm font-medium rounded-md text-white bg-[#cc6600] hover:bg-[#b35900]"
                >
                  <svg className="h-5 w-5 mr-2" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                    <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M12 4v16m8-8H4" />
                  </svg>
                  Create Key
                </button>
              </div>
            </div>
          ) : (
            <div className="space-y-4">
              {paymentKeys.map((key) => (
                <PaymentKeyCard
                  key={key.nonce}
                  paymentKey={key}
                  balance={balances.get(key.nonce)}
                  stablecoin={stablecoin}
                  onTopUp={() => setTopUpKey(key)}
                  onDelete={() => handleDeleteKey(key)}
                  coordinatorUrl={coordinatorUrl}
                  accountId={accountId!}
                  network={network}
                />
              ))}
            </div>
          )}

          {/* Refresh Button */}
          {paymentKeys.length > 0 && (
            <div className="mt-4 flex justify-center">
              <button
                onClick={loadPaymentKeys}
                className="text-sm text-gray-500 hover:text-[#cc6600] flex items-center"
              >
                <svg className="h-4 w-4 mr-1" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                  <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15" />
                </svg>
                Refresh
              </button>
            </div>
          )}
        </div>
      )}

      {/* Info Section */}
      <div className="mt-8 bg-blue-50 border border-blue-200 rounded-lg p-6">
        <h3 className="text-sm font-semibold text-blue-900 mb-3">
          About Payment Keys
        </h3>
        <ul className="text-sm text-blue-800 space-y-2 list-disc list-inside">
          <li>
            <strong>HTTPS API Access</strong>: Call OutLayer projects without NEAR transactions
          </li>
          <li>
            <strong>Prepaid Balance</strong>: Top up with {stablecoin.symbol}, pay per compute
          </li>
          <li>
            <strong>Project Restrictions</strong>: Optionally limit key to specific projects
          </li>
          <li>
            <strong>Secure Storage</strong>: Keys are stored encrypted on-chain
          </li>
        </ul>

        <div className="mt-4 p-3 bg-white rounded border border-blue-200">
          <h4 className="text-xs font-semibold text-blue-900 mb-2">Header Format</h4>
          <code className="text-xs text-blue-800 font-mono">
            X-Payment-Key: {accountId || 'yourname.near'}:1:your-secret-key
          </code>
        </div>
      </div>
    </div>
  );
}
