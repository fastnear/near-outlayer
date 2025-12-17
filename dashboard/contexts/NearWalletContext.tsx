'use client';

import React, { createContext, useContext, useState, useEffect, ReactNode } from 'react';
import { setupWalletSelector } from '@near-wallet-selector/core';
import { setupMyNearWallet } from '@near-wallet-selector/my-near-wallet';
import { setupMeteorWallet } from '@near-wallet-selector/meteor-wallet';
import { setupHereWallet } from '@near-wallet-selector/here-wallet';
import { setupIntearWallet } from '@near-wallet-selector/intear-wallet';
import { setupModal } from '@near-wallet-selector/modal-ui';
import '@near-wallet-selector/modal-ui/styles.css';

export type NetworkType = 'testnet' | 'mainnet';

interface SignMessageParams {
  message: string;
  recipient: string;
  nonce: string;
}

interface SignedMessage {
  signature: string;
  publicKey: string;
  accountId: string;
}

interface NearWalletContextType {
  accountId: string | null;
  isConnected: boolean;
  isWalletReady: boolean;
  network: NetworkType;
  contractId: string;
  rpcUrl: string;
  shouldReopenModal: boolean;
  clearReopenModal: () => void;
  connect: () => void;
  disconnect: () => void;
  switchNetwork: (network: NetworkType) => void;
  signAndSendTransaction: (params: any) => Promise<any>;
  signMessage: (params: SignMessageParams) => Promise<SignedMessage | null>;
  viewMethod: (params: { contractId: string; method: string; args?: Record<string, unknown> }) => Promise<unknown>;
}

const NearWalletContext = createContext<NearWalletContextType | undefined>(undefined);

const getNetworkConfig = (network: NetworkType) => ({
  contractId: network === 'testnet'
    ? process.env.NEXT_PUBLIC_TESTNET_CONTRACT_ID || 'outlayer.testnet'
    : process.env.NEXT_PUBLIC_MAINNET_CONTRACT_ID || 'outlayer.near',
  rpcUrl: network === 'testnet'
    ? process.env.NEXT_PUBLIC_TESTNET_RPC_URL || 'https://rpc.testnet.near.org'
    : process.env.NEXT_PUBLIC_MAINNET_RPC_URL || 'https://rpc.mainnet.near.org',
});

export function NearWalletProvider({ children }: { children: ReactNode }) {
  // Read network from localStorage or use default
  const getInitialNetwork = (): NetworkType => {
    if (typeof window !== 'undefined') {
      const stored = localStorage.getItem('near-wallet-selector:selectedNetworkId');
      if (stored === 'testnet' || stored === 'mainnet') {
        return stored;
      }
    }
    return (process.env.NEXT_PUBLIC_DEFAULT_NETWORK || 'testnet') as NetworkType;
  };

  const [network] = useState<NetworkType>(getInitialNetwork);
  const [accountId, setAccountId] = useState<string | null>(null);
  const [selector, setSelector] = useState<any>(null);
  const [modal, setModal] = useState<any>(null);
  const [isWalletReady, setIsWalletReady] = useState(false);
  const [shouldReopenModal, setShouldReopenModal] = useState(false);

  const config = getNetworkConfig(network);

  // Check if we should reopen modal after page reload
  useEffect(() => {
    if (typeof window !== 'undefined') {
      const reopenFlag = localStorage.getItem('near-wallet-selector:reopenModal');
      if (reopenFlag === 'true') {
        setShouldReopenModal(true);
      }
    }
  }, []);

  const clearReopenModal = () => {
    setShouldReopenModal(false);
    if (typeof window !== 'undefined') {
      localStorage.removeItem('near-wallet-selector:reopenModal');
    }
  };

  useEffect(() => {
    // Mark as not ready when starting to setup
    setIsWalletReady(false);
    setSelector(null);
    setModal(null);

    setupWalletSelector({
      network,
      modules: [
        setupMyNearWallet(),
        setupMeteorWallet(),
        setupHereWallet(),
        setupIntearWallet(),
      ],
    }).then(async (_selector) => {
      // Setup modal WITHOUT contractId to avoid function call access key creation
      const _modal = setupModal(_selector, {
        contractId: '', // Empty string means no contract-specific access key
      });

      // Subscribe to account changes to auto-update UI
      const subscription = _selector.store.observable
        .subscribe((state: { accounts: Array<{ accountId: string }> }) => {
          const accounts = state.accounts;
          if (accounts.length > 0) {
            setAccountId(accounts[0].accountId);
          } else {
            setAccountId(null);
          }
        });

      // Check if wallet is already connected
      if (_selector.isSignedIn()) {
        const accounts = await _selector.store.getState().accounts;
        if (accounts.length > 0) {
          setAccountId(accounts[0].accountId);
        }
      }

      // Set selector and modal AFTER everything is configured
      setSelector(_selector);
      setModal(_modal);

      // Small delay to ensure modal is fully ready before allowing connections
      setTimeout(() => {
        setIsWalletReady(true);
      }, 100);

      // Cleanup subscription on unmount
      return () => subscription.unsubscribe();
    });
  }, [network]);

  const connect = () => {
    if (modal) {
      modal.show();
    }
  };

  const disconnect = async () => {
    if (selector) {
      const wallet = await selector.wallet();
      await wallet.signOut();
      setAccountId(null);
    }
  };

  const switchNetwork = async (newNetwork: NetworkType) => {
    // Store selected network in localStorage
    localStorage.setItem('near-wallet-selector:selectedNetworkId', newNetwork);
    // Set flag to reopen modal after reload
    localStorage.setItem('near-wallet-selector:reopenModal', 'true');

    // Disconnect current wallet before switching
    if (selector && accountId) {
      const wallet = await selector.wallet();
      await wallet.signOut();
      setAccountId(null);
    }

    // Reload page to reinitialize wallet selector with new network
    window.location.reload();
  };

  const signAndSendTransaction = async (params: any) => {
    if (!selector) throw new Error('Wallet not initialized');
    const wallet = await selector.wallet();
    return await wallet.signAndSendTransaction(params);
  };

  const signMessage = async (params: SignMessageParams): Promise<SignedMessage | null> => {
    if (!selector || !accountId) throw new Error('Wallet not connected');

    const wallet = await selector.wallet();

    // Check if wallet supports signMessage (NEP-413)
    if (!wallet.signMessage) {
      throw new Error('Current wallet does not support message signing. Please use a wallet that supports NEP-413 (e.g., MyNearWallet, Meteor, Here Wallet)');
    }

    try {
      const result = await wallet.signMessage({
        message: params.message,
        recipient: params.recipient,
        nonce: Buffer.from(params.nonce, 'base64'),
      });

      if (!result) {
        return null;
      }

      // Handle signature format - it can be Uint8Array or base64 string depending on wallet
      let signatureBase64: string;
      if (result.signature instanceof Uint8Array) {
        signatureBase64 = Buffer.from(result.signature).toString('base64');
      } else if (typeof result.signature === 'string') {
        // Already a string - assume it's base64
        signatureBase64 = result.signature;
      } else {
        // Array-like object
        signatureBase64 = Buffer.from(result.signature as ArrayLike<number>).toString('base64');
      }

      console.log('signMessage result:', {
        signatureType: typeof result.signature,
        signatureIsUint8Array: result.signature instanceof Uint8Array,
        signatureLength: result.signature?.length,
        signatureBase64Length: signatureBase64.length,
        publicKey: result.publicKey,
      });

      return {
        signature: signatureBase64,
        publicKey: result.publicKey,
        accountId: result.accountId,
      };
    } catch (error) {
      console.error('Error signing message:', error);
      throw error;
    }
  };

  const viewMethod = async (params: { contractId: string; method: string; args?: Record<string, unknown> }) => {
    if (!selector) throw new Error('Wallet not initialized');

    // Use selector's network to make view call via RPC
    const response = await fetch(config.rpcUrl, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({
        jsonrpc: '2.0',
        id: 'dontcare',
        method: 'query',
        params: {
          request_type: 'call_function',
          finality: 'final',
          account_id: params.contractId,
          method_name: params.method,
          args_base64: btoa(JSON.stringify(params.args || {})),
        },
      }),
    });

    const data = await response.json();

    if (data.error) {
      throw new Error(data.error.message || 'View method call failed');
    }

    const resultBytes = data.result?.result;
    if (!resultBytes || resultBytes.length === 0) {
      return null;
    }

    const resultStr = new TextDecoder().decode(new Uint8Array(resultBytes));
    return JSON.parse(resultStr);
  };

  return (
    <NearWalletContext.Provider
      value={{
        accountId,
        isConnected: !!accountId,
        isWalletReady,
        network,
        contractId: config.contractId,
        rpcUrl: config.rpcUrl,
        shouldReopenModal,
        clearReopenModal,
        connect,
        disconnect,
        switchNetwork,
        signAndSendTransaction,
        signMessage,
        viewMethod,
      }}
    >
      {children}
    </NearWalletContext.Provider>
  );
}

export function useNearWallet() {
  const context = useContext(NearWalletContext);
  if (context === undefined) {
    throw new Error('useNearWallet must be used within a NearWalletProvider');
  }
  return context;
}
