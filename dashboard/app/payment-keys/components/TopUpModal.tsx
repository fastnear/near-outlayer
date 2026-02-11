'use client';

import { useState } from 'react';
import { actionCreators } from '@near-js/transactions';
import { StablecoinConfig } from '@/contexts/NearWalletContext';
import { parseUsdToMinimalUnits } from './types';

interface TopUpModalProps {
  accountId: string;
  nonce: number;
  contractId: string;
  stablecoin: StablecoinConfig;
  signAndSendTransaction: (params: unknown) => Promise<unknown>;
  onComplete: () => void;
  onCancel: () => void;
}

export function TopUpModal({
  accountId,
  nonce,
  contractId,
  stablecoin,
  signAndSendTransaction,
  onComplete,
  onCancel,
}: TopUpModalProps) {
  const [amount, setAmount] = useState<string>('10');
  const [isSubmitting, setIsSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const handleTopUp = async () => {
    if (isSubmitting) return;

    try {
      setError(null);
      setIsSubmitting(true);

      const amountNum = parseFloat(amount);
      if (isNaN(amountNum) || amountNum < 0.01) {
        throw new Error('Minimum top-up is $0.01');
      }

      const depositAmount = parseUsdToMinimalUnits(amount, stablecoin.decimals);

      // Build ft_transfer_call args
      const ftTransferArgs = {
        receiver_id: contractId,
        amount: depositAmount,
        msg: JSON.stringify({
          action: 'top_up_payment_key',
          nonce: nonce,
        }),
      };

      const action = actionCreators.functionCall(
        'ft_transfer_call',
        ftTransferArgs,
        BigInt('100000000000000'), // 100 TGas
        BigInt('1') // 1 yoctoNEAR
      );

      // ft_transfer_call to stablecoin contract
      await signAndSendTransaction({
        receiverId: stablecoin.contract,
        actions: [action],
      });

      onComplete();
    } catch (err) {
      console.error('Failed to top up:', err);
      setError((err as Error).message);
    } finally {
      setIsSubmitting(false);
    }
  };

  return (
    <div className="fixed inset-0 bg-black bg-opacity-50 flex items-center justify-center z-50 p-4">
      <div className="bg-white rounded-lg shadow-xl p-6 max-w-md w-full">
        <h2 className="text-xl font-bold text-gray-900 mb-4">Top Up Payment Key #{nonce}</h2>

        {error && (
          <div className="mb-4 p-3 bg-red-50 border border-red-200 rounded-lg">
            <p className="text-red-800 text-sm">{error}</p>
          </div>
        )}

        <div className="mb-4">
          <label className="block text-sm font-medium text-gray-700 mb-1">
            Amount ({stablecoin.symbol})
          </label>
          <input
            type="text"
            value={amount}
            onChange={(e) => setAmount(e.target.value)}
            placeholder="10.00"
            className="w-full border border-gray-300 rounded-lg px-3 py-2 text-gray-900 placeholder-gray-400 focus:ring-2 focus:ring-[#cc6600] focus:border-transparent"
            disabled={isSubmitting}
          />
          <p className="text-xs text-gray-500 mt-1">
            Minimum $0.01
          </p>
        </div>

        {/* Info */}
        <div className="mb-4 p-3 bg-blue-50 border border-blue-200 rounded-lg">
          <p className="text-sm text-blue-800">
            This will transfer {amount} {stablecoin.symbol} from your wallet to top up the payment key balance.
          </p>
        </div>

        {/* Actions */}
        <div className="flex gap-3">
          <button
            onClick={onCancel}
            className="flex-1 bg-gray-100 hover:bg-gray-200 text-gray-700 px-4 py-2 rounded-lg font-medium transition-colors"
            disabled={isSubmitting}
          >
            Cancel
          </button>
          <button
            onClick={handleTopUp}
            className="flex-1 bg-gradient-to-r from-[#cc6600] to-[#d4a017] hover:from-[#b35900] hover:to-[#c49016] text-white px-4 py-2 rounded-lg font-medium disabled:opacity-50 transition-colors"
            disabled={isSubmitting}
          >
            {isSubmitting ? 'Processing...' : 'Top Up'}
          </button>
        </div>
      </div>
    </div>
  );
}
