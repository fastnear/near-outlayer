'use client';

import { useNearWallet } from '@/contexts/NearWalletContext';
import { useState } from 'react';
import type { NetworkType } from '@/contexts/NearWalletContext';

// Network switcher component for Settings page - shows current network and allows switching
export default function NetworkSwitcher() {
  const { isConnected, network, switchNetwork, disconnect } = useNearWallet();
  const [showNetworkModal, setShowNetworkModal] = useState(false);
  const [pendingNetwork, setPendingNetwork] = useState<NetworkType | null>(null);

  const handleNetworkSwitch = (newNetwork: NetworkType) => {
    // Do nothing if clicking on current network
    if (newNetwork === network) {
      return;
    }

    if (isConnected) {
      // Show confirmation modal if connected
      setPendingNetwork(newNetwork);
      setShowNetworkModal(true);
    } else {
      // Switch directly if not connected
      switchNetwork(newNetwork);
    }
  };

  const confirmNetworkSwitch = async () => {
    if (pendingNetwork) {
      // Disconnect first to avoid "no wallet selected" error
      await disconnect();
      // Then switch network - wallet selector will reinitialize via useEffect
      switchNetwork(pendingNetwork);
      setShowNetworkModal(false);
      setPendingNetwork(null);
    }
  };

  const cancelNetworkSwitch = () => {
    setShowNetworkModal(false);
    setPendingNetwork(null);
  };

  return (
    <div className="flex items-center">
      {/* Network Switcher */}
      <div className="flex items-center bg-gray-100 rounded-lg p-1">
        <button
          onClick={() => handleNetworkSwitch('testnet')}
          disabled={!isConnected}
          className={`px-3 py-1 rounded-md text-sm font-medium transition-colors ${
            network === 'testnet'
              ? 'bg-white text-[#cc6600] shadow-sm'
              : 'text-gray-600 hover:text-gray-900'
          } ${!isConnected ? 'opacity-50 cursor-not-allowed' : 'cursor-pointer'}`}
        >
          Testnet
        </button>
        <button
          onClick={() => handleNetworkSwitch('mainnet')}
          disabled={!isConnected}
          className={`px-3 py-1 rounded-md text-sm font-medium transition-colors ${
            network === 'mainnet'
              ? 'bg-white text-[#5a8f3a] shadow-sm'
              : 'text-gray-600 hover:text-gray-900'
          } ${!isConnected ? 'opacity-50 cursor-not-allowed' : 'cursor-pointer'}`}
        >
          Mainnet
        </button>
      </div>

      {/* Network Switch Confirmation Modal */}
      {showNetworkModal && (
        <div className="fixed inset-0 bg-black bg-opacity-50 z-50 flex items-center justify-center p-4">
          <div className="bg-white rounded-lg shadow-xl max-w-md w-full p-6">
            <h3 className="text-lg font-semibold text-gray-900 mb-3">
              Switch Network?
            </h3>
            <p className="text-gray-600 mb-6">
              You will be disconnected from your current wallet and need to reconnect on{' '}
              <span className="font-semibold">{pendingNetwork}</span>.
            </p>
            <div className="flex space-x-3">
              <button
                onClick={confirmNetworkSwitch}
                className="flex-1 px-4 py-2 bg-gradient-to-r from-[#cc6600] to-[#d4a017] text-white rounded-lg font-medium hover:from-[#b35900] hover:to-[#c49016] transition-colors"
              >
                Switch Network
              </button>
              <button
                onClick={cancelNetworkSwitch}
                className="flex-1 px-4 py-2 bg-gray-200 text-gray-700 rounded-lg font-medium hover:bg-gray-300 transition-colors"
              >
                Cancel
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
