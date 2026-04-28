'use client';

import { useNearWallet } from '@/contexts/NearWalletContext';
import { isTestnetWorkersEnabled } from '@/lib/api';

interface Props {
  variant?: 'banner' | 'block';
}

export default function TestnetDisabledNotice({ variant = 'block' }: Props) {
  const { network } = useNearWallet();

  if (network !== 'testnet' || isTestnetWorkersEnabled()) {
    return null;
  }

  if (variant === 'banner') {
    return (
      <div className="bg-yellow-50 border-b border-yellow-200 text-yellow-900 text-sm">
        <div className="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8 py-2 flex items-start gap-2">
          <svg className="w-4 h-4 mt-0.5 flex-shrink-0" fill="currentColor" viewBox="0 0 20 20">
            <path fillRule="evenodd" d="M8.485 2.495c.673-1.167 2.357-1.167 3.03 0l6.28 10.875c.673 1.167-.17 2.625-1.516 2.625H3.72c-1.347 0-2.189-1.458-1.515-2.625L8.485 2.495zM10 6a1 1 0 011 1v3a1 1 0 11-2 0V7a1 1 0 011-1zm0 8a1 1 0 100-2 1 1 0 000 2z" clipRule="evenodd" />
          </svg>
          <span>
            Testnet workers are currently <span className="font-semibold">offline</span> due to low usage.
            Please switch to <span className="font-semibold">mainnet</span>. If testing on NEAR testnet
            is critical for you, please contact the developers.
          </span>
        </div>
      </div>
    );
  }

  return (
    <div className="bg-yellow-50 border border-yellow-200 rounded-lg p-6">
      <div className="flex items-start gap-3">
        <svg className="w-6 h-6 text-yellow-600 flex-shrink-0 mt-0.5" fill="currentColor" viewBox="0 0 20 20">
          <path fillRule="evenodd" d="M8.485 2.495c.673-1.167 2.357-1.167 3.03 0l6.28 10.875c.673 1.167-.17 2.625-1.516 2.625H3.72c-1.347 0-2.189-1.458-1.515-2.625L8.485 2.495zM10 6a1 1 0 011 1v3a1 1 0 11-2 0V7a1 1 0 011-1zm0 8a1 1 0 100-2 1 1 0 000 2z" clipRule="evenodd" />
        </svg>
        <div className="text-sm text-yellow-900 space-y-2">
          <p className="font-semibold text-base">Testnet infrastructure is offline</p>
          <p>
            OutLayer testnet workers are currently disabled due to low usage. We recommend
            using <span className="font-semibold">mainnet</span> workers instead.
          </p>
          <p>
            If testing on NEAR testnet is critical for you, please contact the developers.
          </p>
        </div>
      </div>
    </div>
  );
}
