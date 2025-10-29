'use client';

import React, { createContext, useContext, useState, useEffect, ReactNode } from 'react';
import { setupWalletSelector } from '@near-wallet-selector/core';
import { setupMyNearWallet } from '@near-wallet-selector/my-near-wallet';
import { setupModal } from '@near-wallet-selector/modal-ui';
import '@near-wallet-selector/modal-ui/styles.css';

export type NetworkType = 'testnet' | 'mainnet';

interface NearWalletContextType {
  accountId: string | null;
  isConnected: boolean;
  network: NetworkType;
  contractId: string;
  rpcUrl: string;
  connect: () => void;
  disconnect: () => void;
  switchNetwork: (network: NetworkType) => void;
  signAndSendTransaction: (params: any) => Promise<any>;
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
  const defaultNetwork = (process.env.NEXT_PUBLIC_DEFAULT_NETWORK || 'testnet') as NetworkType;
  const [network, setNetwork] = useState<NetworkType>(defaultNetwork);
  const [accountId, setAccountId] = useState<string | null>(null);
  const [selector, setSelector] = useState<any>(null);
  const [modal, setModal] = useState<any>(null);

  const config = getNetworkConfig(network);

  useEffect(() => {
    setupWalletSelector({
      network,
      modules: [setupMyNearWallet()],
    }).then(async (_selector) => {
      setSelector(_selector);

      const _modal = setupModal(_selector, {
        contractId: config.contractId,
      });
      setModal(_modal);

      // Check if wallet is already connected
      if (_selector.isSignedIn()) {
        const accounts = await _selector.store.getState().accounts;
        if (accounts.length > 0) {
          setAccountId(accounts[0].accountId);
        }
      }
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
    // Disconnect current wallet before switching
    if (selector) {
      const wallet = await selector.wallet();
      await wallet.signOut();
      setAccountId(null);
    }
    setNetwork(newNetwork);
  };

  const signAndSendTransaction = async (params: any) => {
    if (!selector) throw new Error('Wallet not initialized');
    const wallet = await selector.wallet();
    return await wallet.signAndSendTransaction(params);
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
        network,
        contractId: config.contractId,
        rpcUrl: config.rpcUrl,
        connect,
        disconnect,
        switchNetwork,
        signAndSendTransaction,
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
