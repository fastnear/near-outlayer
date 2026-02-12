'use client';

import { useState, useEffect } from 'react';
import { actionCreators } from '@near-js/transactions';
import { ChaCha20Poly1305 } from '@stablelib/chacha20poly1305';
import { randomBytes } from '@stablelib/random';
import { StablecoinConfig } from '@/contexts/NearWalletContext';
import { CreationState, parseUsdToMinimalUnits, PaymentKeySecret } from './types';

interface CreateKeyFormProps {
  accountId: string;
  contractId: string;
  stablecoin: StablecoinConfig;
  nextNonce: number;
  coordinatorUrl: string;
  signAndSendTransaction: (params: unknown) => Promise<unknown>;
  onComplete: (generatedKey: string, nonce: number) => void;
  onError: (error: string) => void;
  onCancel: () => void;
  creationState: CreationState;
  setCreationState: (state: CreationState) => void;
}

const STORAGE_KEY_PREFIX = 'payment_key_creation_';

export function CreateKeyForm({
  accountId,
  contractId,
  stablecoin,
  nextNonce,
  coordinatorUrl,
  signAndSendTransaction,
  onComplete,
  onError,
  onCancel,
  creationState,
  setCreationState,
}: CreateKeyFormProps) {
  const [projectIds, setProjectIds] = useState<string>('');
  const [maxPerCall, setMaxPerCall] = useState<string>('');
  const [initialDeposit, setInitialDeposit] = useState<string>('2');
  const [isCreating, setIsCreating] = useState(false);

  // Pre-fetched pubkey to avoid async delay when user clicks Create
  const [prefetchedPubkey, setPrefetchedPubkey] = useState<string | null>(null);
  const [pubkeyError, setPubkeyError] = useState<string | null>(null);

  // Pre-fetch pubkey when form opens to avoid popup blocking
  // Browser blocks popups if there's async work between click and popup open
  useEffect(() => {
    const fetchPubkey = async () => {
      try {
        // Accessor formats differ:
        // - Coordinator API (internally tagged): { type: "System", PaymentKey: {} }
        const accessorForCoordinator = { type: 'System', PaymentKey: {} };

        const response = await fetch(`${coordinatorUrl}/secrets/pubkey`, {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({
            accessor: accessorForCoordinator,
            owner: accountId,
            profile: nextNonce.toString(),
            secrets_json: '{}', // Dummy, pubkey is the same for all
          }),
        });

        if (!response.ok) {
          const errorText = await response.text();
          throw new Error(`Failed to get encryption key: ${errorText}`);
        }

        const { pubkey } = await response.json();
        setPrefetchedPubkey(pubkey);
        setPubkeyError(null);
      } catch (err) {
        console.error('Failed to pre-fetch pubkey:', err);
        setPubkeyError((err as Error).message);
      }
    };

    fetchPubkey();
  }, [accountId, nextNonce, coordinatorUrl]);

  // Generate random 32-byte key as hex (64 characters, alphanumeric only)
  const generateKey = (): string => {
    const bytes = new Uint8Array(32);
    crypto.getRandomValues(bytes);
    return Array.from(bytes).map(b => b.toString(16).padStart(2, '0')).join('');
  };

  // Save creation state to localStorage (in case of page reload)
  // step: 'store_secrets' | 'topup' - which transaction is in progress
  // Also save depositAmount for TopUp continuation after first tx
  const saveCreationState = (
    key: string,
    nonce: number,
    step: 'store_secrets' | 'topup',
    depositAmount?: string
  ) => {
    localStorage.setItem(`${STORAGE_KEY_PREFIX}${accountId}`, JSON.stringify({
      key,
      nonce,
      step,
      depositAmount: depositAmount || initialDeposit,
      timestamp: Date.now(),
    }));
  };

  // Clear creation state from localStorage
  const clearCreationState = () => {
    localStorage.removeItem(`${STORAGE_KEY_PREFIX}${accountId}`);
  };

  // Get project IDs array
  const getProjectIdsArray = (): string[] => {
    if (!projectIds.trim()) return [];
    return projectIds.split(',').map(s => s.trim()).filter(s => s.length > 0);
  };

  // Handle form submit
  const handleCreate = async () => {
    if (isCreating) return;

    try {
      setIsCreating(true);
      const depositNum = parseFloat(initialDeposit);

      if (isNaN(depositNum) || depositNum < 0.01) {
        throw new Error('Minimum deposit is $0.01');
      }

      // Step 1: Generate key
      setCreationState({ step: 'generating' });
      const generatedKey = generateKey();
      const nonce = nextNonce;

      // Log the full API key to console for debugging (in case popup fails)
      const fullApiKey = `${accountId}:${nonce}:${generatedKey}`;
      console.log('=== GENERATED PAYMENT KEY ===');
      console.log('Full API key:', fullApiKey);
      console.log('Copy this before approving transactions!');
      console.log('=============================');

      // Save to localStorage in case of page reload (will be updated before each transaction)
      // Initial save - will update step before each signAndSendTransaction
      saveCreationState(generatedKey, nonce, 'store_secrets');

      // Check if pubkey was pre-fetched
      if (!prefetchedPubkey) {
        if (pubkeyError) {
          throw new Error(`Cannot encrypt: ${pubkeyError}`);
        }
        throw new Error('Encryption key not ready. Please wait a moment and try again.');
      }

      // Prepare Payment Key secret data (initially with balance 0)
      const secretData: PaymentKeySecret = {
        key: generatedKey,
        project_ids: getProjectIdsArray(),
        max_per_call: maxPerCall ? parseUsdToMinimalUnits(maxPerCall, stablecoin.decimals) : '0',
        initial_balance: '0', // Will be updated via TopUp
      };

      const secretJson = JSON.stringify(secretData);

      // Encrypt secret for contract using pre-fetched pubkey
      // This avoids async delay that would cause popup to be blocked
      setCreationState({ step: 'storing', generatedKey, nonce });

      // NEAR Contract uses externally tagged unit variant: { System: "PaymentKey" }
      const accessorForContract = { System: 'PaymentKey' };

      // Encrypt the secret data with ChaCha20-Poly1305 (sync operation)
      const encryptedArray = encryptWithPubkey(prefetchedPubkey, secretJson);
      const encryptedBase64 = btoa(String.fromCharCode(...Array.from(encryptedArray)));

      // Step 2: Transaction 1 - store_secrets
      // Use fixed storage deposit (0.1 NEAR) - excess is refunded automatically
      // We skip estimate_storage_cost viewMethod call to avoid async delay
      // that would cause browser to block the wallet popup
      const storageCostBigInt = BigInt('100000000000000000000000'); // 0.1 NEAR

      // Build store_secrets args
      const storeSecretsArgs = {
        accessor: accessorForContract,
        profile: nonce.toString(),
        encrypted_secrets_base64: encryptedBase64,
        access: 'AllowAll',
      };

      // Execute store_secrets transaction
      const storeAction = actionCreators.functionCall(
        'store_secrets',
        storeSecretsArgs,
        BigInt('100000000000000'), // 100 TGas
        storageCostBigInt
      );

      // callbackUrl - redirect back to this page after wallet approval
      const callbackUrl = window.location.href.split('?')[0]; // Remove any existing params

      await signAndSendTransaction({
        receiverId: contractId,
        actions: [storeAction],
        callbackUrl, // Return to this page after approval
      });

      // First transaction succeeded!
      // DON'T try to do second transaction automatically - browser will block the popup
      // because "user gesture" is already consumed by the first transaction.
      // Instead, show the key and let user click "Continue: Add Balance" button.

      // For popup wallets: code reaches here after popup closes
      // For redirect wallets: page reloads, this code never runs, useEffect handles it

      console.log('First transaction (store_secrets) completed. Key is now stored on contract.');
      console.log('User needs to click "Continue: Add Balance" for the second transaction.');

      // Don't clear localStorage - we need it for the pending TopUp flow
      // The page.tsx useEffect will detect this and show "Continue: Add Balance" button
      onComplete(generatedKey, nonce);
    } catch (err) {
      console.error('Failed to create payment key:', err);
      clearCreationState();
      onError(`Failed to create payment key: ${(err as Error).message}`);
    } finally {
      setIsCreating(false);
    }
  };

  // Convert hex string to bytes (same as SecretsForm)
  const hexToBytes = (hex: string): Uint8Array => {
    const bytes = new Uint8Array(hex.length / 2);
    for (let i = 0; i < hex.length; i += 2) {
      bytes[i / 2] = parseInt(hex.substring(i, i + 2), 16);
    }
    return bytes;
  };

  // Encrypt with ChaCha20-Poly1305 (same as SecretsForm)
  const encryptWithPubkey = (pubkeyHex: string, plaintext: string): Uint8Array => {
    const keyMaterial = hexToBytes(pubkeyHex);
    const plaintextBytes = new TextEncoder().encode(plaintext);
    const cipher = new ChaCha20Poly1305(keyMaterial);
    const nonce = randomBytes(12);
    const ciphertextWithTag = cipher.seal(nonce, plaintextBytes);
    const encrypted = new Uint8Array(12 + ciphertextWithTag.length);
    encrypted.set(nonce, 0);
    encrypted.set(ciphertextWithTag, 12);
    return encrypted;
  };

  const isSubmitting = creationState.step !== 'form' && creationState.step !== 'error';

  return (
    <div className="fixed inset-0 bg-black bg-opacity-50 flex items-center justify-center z-50 p-4">
      <div className="bg-white rounded-lg shadow-xl p-6 max-w-lg w-full">
        <h2 className="text-xl font-bold text-gray-900 mb-4">Create Payment Key</h2>

        {/* Progress indicator */}
        {isSubmitting && (
          <div className="mb-4 p-3 bg-blue-50 border border-blue-200 rounded-lg">
            <div className="flex items-center gap-2">
              <div className="animate-spin rounded-full h-4 w-4 border-b-2 border-[#cc6600]"></div>
              <span className="text-blue-700">
                {creationState.step === 'generating' && 'Generating secure key...'}
                {creationState.step === 'storing' && 'Transaction 1/2: Storing encrypted key...'}
                {creationState.step === 'topping_up' && 'Transaction 2/2: Adding initial balance...'}
              </span>
            </div>
          </div>
        )}

        {/* Info */}
        <div className="mb-4 p-3 bg-yellow-50 border border-yellow-200 rounded-lg">
          <p className="text-sm text-yellow-800">
            Creating a Payment Key requires 2 transactions:
          </p>
          <ol className="text-sm text-yellow-700 mt-2 ml-4 list-decimal">
            <li>Store encrypted key on contract (NEAR storage deposit)</li>
            <li>Top up balance with {stablecoin.symbol} (initial deposit)</li>
          </ol>
        </div>

        {/* Form */}
        <div className="space-y-4">
          {/* Project restrictions */}
          <div>
            <label className="block text-sm font-medium text-gray-700 mb-1">
              Allowed Projects (optional)
            </label>
            <input
              type="text"
              value={projectIds}
              onChange={(e) => setProjectIds(e.target.value)}
              placeholder="owner.near/project1, owner.near/project2"
              className="w-full border border-gray-300 rounded-lg px-3 py-2 text-gray-900 placeholder-gray-400 focus:ring-2 focus:ring-[#cc6600] focus:border-transparent"
              disabled={isSubmitting}
            />
            <p className="text-xs text-gray-500 mt-1">
              Leave empty to allow all projects. Comma-separated list.
            </p>
          </div>

          {/* Max per call */}
          <div>
            <label className="block text-sm font-medium text-gray-700 mb-1">
              Max per Call ({stablecoin.symbol}, optional)
            </label>
            <input
              type="text"
              value={maxPerCall}
              onChange={(e) => setMaxPerCall(e.target.value)}
              placeholder="100.00"
              className="w-full border border-gray-300 rounded-lg px-3 py-2 text-gray-900 placeholder-gray-400 focus:ring-2 focus:ring-[#cc6600] focus:border-transparent"
              disabled={isSubmitting}
            />
            <p className="text-xs text-gray-500 mt-1">
              Maximum spend per single API call. Leave empty for no limit.
            </p>
          </div>

          {/* Initial deposit */}
          <div>
            <label className="block text-sm font-medium text-gray-700 mb-1">
              Initial Deposit ({stablecoin.symbol}) *
            </label>
            <input
              type="text"
              value={initialDeposit}
              onChange={(e) => setInitialDeposit(e.target.value)}
              placeholder="2.00"
              className="w-full border border-gray-300 rounded-lg px-3 py-2 text-gray-900 placeholder-gray-400 focus:ring-2 focus:ring-[#cc6600] focus:border-transparent"
              disabled={isSubmitting}
            />
            <p className="text-xs text-gray-500 mt-1">
              Minimum $0.01. This will be your starting balance.
            </p>
          </div>
        </div>

        {/* Pubkey loading/error indicator */}
        {!prefetchedPubkey && !pubkeyError && (
          <div className="flex items-center gap-2 text-sm text-gray-500">
            <div className="animate-spin rounded-full h-4 w-4 border-b-2 border-gray-400"></div>
            Preparing encryption...
          </div>
        )}
        {pubkeyError && (
          <div className="text-sm text-red-600">
            Failed to load encryption key: {pubkeyError}
          </div>
        )}

        {/* Actions */}
        <div className="flex gap-3 mt-6">
          <button
            onClick={onCancel}
            className="flex-1 bg-gray-100 hover:bg-gray-200 text-gray-700 px-4 py-2 rounded-lg font-medium transition-colors"
            disabled={isSubmitting}
          >
            Cancel
          </button>
          <button
            onClick={handleCreate}
            className="flex-1 bg-gradient-to-r from-[#cc6600] to-[#d4a017] hover:from-[#b35900] hover:to-[#c49016] text-white px-4 py-2 rounded-lg font-medium disabled:opacity-50 transition-colors"
            disabled={isSubmitting || !prefetchedPubkey}
          >
            {isSubmitting ? 'Creating...' : !prefetchedPubkey ? 'Preparing...' : 'Create Key'}
          </button>
        </div>

        {/* Nonce info */}
        <p className="text-xs text-gray-500 mt-4 text-center">
          This will be key #{nextNonce} for your account
        </p>
      </div>
    </div>
  );
}
