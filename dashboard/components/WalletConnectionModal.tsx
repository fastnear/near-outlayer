'use client';

import { useNearWallet } from '@/contexts/NearWalletContext';
import { useState, useEffect } from 'react';
import type { NetworkType } from '@/contexts/NearWalletContext';

interface WalletConnectionModalProps {
  isOpen: boolean;
  onClose: () => void;
}

export default function WalletConnectionModal({ isOpen, onClose }: WalletConnectionModalProps) {
  const { network, switchNetwork, connect, isConnected, isWalletReady } = useNearWallet();
  const [pendingNetwork, setPendingNetwork] = useState<NetworkType>(network);

  // Sync pendingNetwork with actual network when modal opens
  useEffect(() => {
    if (isOpen) {
      setPendingNetwork(network);
    }
  }, [isOpen, network]);

  // Auto-close modal when wallet gets connected
  useEffect(() => {
    if (isConnected && isOpen) {
      onClose();
    }
  }, [isConnected, isOpen, onClose]);

  if (!isOpen) return null;

  const handleNetworkChange = async (newNetwork: NetworkType) => {
    if (newNetwork === network) {
      setPendingNetwork(newNetwork);
      return;
    }

    // Switch network immediately - wallet selector will reinitialize via useEffect
    setPendingNetwork(newNetwork);
    switchNetwork(newNetwork);
  };

  const handleConnect = () => {
    if (!isWalletReady) {
      // Wallet selector is still reinitializing, don't connect yet
      return;
    }
    connect();
  };

  const handleDisconnect = () => {
    // Just close modal, disconnect button is in Settings
    onClose();
  };

  return (
    <div className="fixed inset-0 bg-black bg-opacity-50 z-50 flex items-center justify-center p-4">
      <div className="bg-white rounded-lg shadow-xl max-w-md w-full p-6">
        <div className="flex items-center justify-between mb-4">
          <h3 className="text-lg font-semibold text-gray-900">
            {isConnected ? 'Wallet Connected' : 'Connect Wallet'}
          </h3>
          <button
            onClick={onClose}
            className="text-gray-400 hover:text-gray-600 transition-colors"
          >
            <svg className="w-6 h-6" fill="none" viewBox="0 0 24 24" stroke="currentColor">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M6 18L18 6M6 6l12 12" />
            </svg>
          </button>
        </div>

        {!isConnected ? (
          <>
            <p className="text-gray-600 mb-4">
              Select network and login with NEAR
            </p>

            {/* Network Selector */}
            <div className="mb-6">
              <label className="block text-sm font-medium text-gray-700 mb-2">
                Network
              </label>
              <div className="flex items-center bg-gray-100 rounded-lg p-1">
                <button
                  onClick={() => handleNetworkChange('testnet')}
                  className={`flex-1 px-4 py-2 rounded-md text-sm font-medium transition-colors ${
                    pendingNetwork === 'testnet'
                      ? 'bg-white text-[#cc6600] shadow-sm'
                      : 'text-gray-600 hover:text-gray-900'
                  }`}
                >
                  Testnet
                </button>
                <button
                  onClick={() => handleNetworkChange('mainnet')}
                  className={`flex-1 px-4 py-2 rounded-md text-sm font-medium transition-colors ${
                    pendingNetwork === 'mainnet'
                      ? 'bg-white text-[#5a8f3a] shadow-sm'
                      : 'text-gray-600 hover:text-gray-900'
                  }`}
                >
                  Mainnet
                </button>
              </div>
            </div>

            {/* Connect Button */}
            <button
              onClick={handleConnect}
              disabled={!isWalletReady || pendingNetwork !== network}
              className="w-full px-4 py-3 bg-gradient-to-r from-[#cc6600] to-[#d4a017] text-white rounded-lg font-medium hover:from-[#b35900] hover:to-[#c49016] transition-colors shadow-sm hover:shadow-md disabled:opacity-50 disabled:cursor-not-allowed"
            >
              {!isWalletReady || pendingNetwork !== network ? 'Switching network...' : `Connect to ${pendingNetwork === 'testnet' ? 'Testnet' : 'Mainnet'}`}
            </button>

            {(!isWalletReady || pendingNetwork !== network) && (
              <p className="mt-3 text-xs text-gray-500 text-center">
                Please wait while we switch to {pendingNetwork}...
              </p>
            )}
          </>
        ) : (
          <>
            <p className="text-gray-600 mb-6">
              Your wallet is already connected. Go to Settings to disconnect or switch network.
            </p>
            <button
              onClick={handleDisconnect}
              className="w-full px-4 py-3 bg-gray-200 text-gray-700 rounded-lg font-medium hover:bg-gray-300 transition-colors"
            >
              Close
            </button>
          </>
        )}
      </div>
    </div>
  );
}
