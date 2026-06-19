'use client';

import React, { createContext, useContext, useState, useEffect, useRef, useCallback, ReactNode } from 'react';
import { NearConnector } from '@hot-labs/near-connect';
import { extractTxHashFromError, waitForTransactionOutcome } from '@/lib/near-rpc';

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

export interface StablecoinConfig {
  contract: string;
  decimals: number;
  symbol: string;
}

interface NearWalletContextType {
  accountId: string | null;
  isConnected: boolean;
  isWalletReady: boolean;
  network: NetworkType;
  contractId: string;
  rpcUrl: string;
  stablecoin: StablecoinConfig;
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
  stablecoin: {
    contract: network === 'testnet'
      ? process.env.NEXT_PUBLIC_TESTNET_STABLECOIN_CONTRACT || 'usdc.fakes.testnet'
      : process.env.NEXT_PUBLIC_MAINNET_STABLECOIN_CONTRACT || '17208628f84f5d6ad33f0da3bbbeb27ffcb398eac501a31bd6ad2011e36133a1',
    decimals: network === 'testnet'
      ? parseInt(process.env.NEXT_PUBLIC_TESTNET_STABLECOIN_DECIMALS || '6', 10)
      : parseInt(process.env.NEXT_PUBLIC_MAINNET_STABLECOIN_DECIMALS || '6', 10),
    symbol: network === 'testnet'
      ? process.env.NEXT_PUBLIC_TESTNET_STABLECOIN_SYMBOL || 'USDC'
      : process.env.NEXT_PUBLIC_MAINNET_STABLECOIN_SYMBOL || 'USDC',
  },
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

  const [network, setNetwork] = useState<NetworkType>(getInitialNetwork);
  const [accountId, setAccountId] = useState<string | null>(null);
  const [isWalletReady, setIsWalletReady] = useState(false);
  const [shouldReopenModal, setShouldReopenModal] = useState(false);

  const connectorRef = useRef<NearConnector | null>(null);
  const config = getNetworkConfig(network);

  // Check if we should reopen modal after page reload (legacy from
  // wallet-selector days; near-connect's switchNetwork doesn't reload
  // anymore but the flag is still respected by the modal component).
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

  // Patch every sandboxed wallet iframe so it gets the `bluetooth`
  // feature-policy permission — without this, Ledger Nano X over BLE
  // fails silently inside the wallet sandbox. NearConnector creates
  // the iframes dynamically when the picker opens, so a one-shot
  // querySelectorAll isn't enough; we keep a MutationObserver running
  // for the lifetime of the page. Same fix shipped by the trezu
  // reference impl (nt-fe/stores/near-store.ts:45-66).
  useEffect(() => {
    if (typeof window === 'undefined') return;
    const ensureBluetoothAllow = (iframe: HTMLIFrameElement) => {
      if (iframe.dataset.bluetoothPatched === '1') return;
      const cur = iframe.getAttribute('allow') || '';
      if (cur.includes('bluetooth')) {
        iframe.dataset.bluetoothPatched = '1';
        return;
      }
      const next = cur ? `${cur}; bluetooth *` : 'bluetooth *';
      iframe.setAttribute('allow', next);
      iframe.dataset.bluetoothPatched = '1';
    };
    document.querySelectorAll('iframe').forEach(ensureBluetoothAllow);
    const observer = new MutationObserver((mutations) => {
      for (const m of mutations) {
        m.addedNodes.forEach((node) => {
          if (node instanceof HTMLIFrameElement) {
            ensureBluetoothAllow(node);
          } else if (node instanceof Element) {
            node.querySelectorAll('iframe').forEach(ensureBluetoothAllow);
          }
        });
      }
    });
    observer.observe(document.body, { childList: true, subtree: true });
    return () => observer.disconnect();
  }, []);

  // Initialize connector and restore session on every network change.
  useEffect(() => {
    setIsWalletReady(false);
    setAccountId(null);

    let cancelled = false;
    let connector: NearConnector | null = null;

    const handleSignIn = ({ accounts }: { accounts: Array<{ accountId: string }> }) => {
      if (accounts.length > 0) {
        setAccountId(accounts[0].accountId);
      }
    };

    const handleSignOut = () => {
      setAccountId(null);
    };

    (async () => {
      // The upstream manifest is missing two things we need to patch
      // before passing it to NearConnector:
      //
      // (1) `features.testnet: true` on wallets that DO support testnet
      //     (MyNearWallet, HOT, Meteor, OKX, …). Without this flag the
      //     connector hides them when `network === 'testnet'`.
      //
      // (2) Per-wallet `permissions.allowsOpen` for testnet domain
      //     variants. Example: MyNearWallet's manifest only allows
      //     `https://app.mynearwallet.com`, but its sandbox tries to
      //     `open` `https://testnet.mynearwallet.com` on testnet — the
      //     connector then rejects with a fatal-looking "Permission
      //     denied" thrown from `assertPermissions`. We extend
      //     `allowsOpen` per known wallet so the testnet flow works.
      //
      // All patches live inline and don't depend on upstream changes.
      const ALLOWS_OPEN_TESTNET_PATCHES: Record<string, string[]> = {
        mynearwallet: ['https://testnet.mynearwallet.com'],
      };
      let manifestObj: { wallets: any[]; version: string } | undefined;
      try {
        const res = await fetch(
          'https://raw.githubusercontent.com/hot-dao/near-selector/refs/heads/main/repository/manifest.json',
        );
        const m = await res.json();
        if (Array.isArray(m?.wallets)) {
          for (const w of m.wallets) {
            if (!w.features) w.features = {};
            w.features.testnet = true;

            const extra = ALLOWS_OPEN_TESTNET_PATCHES[w.id];
            if (extra) {
              if (!w.permissions) w.permissions = {};
              const existing = Array.isArray(w.permissions.allowsOpen)
                ? w.permissions.allowsOpen
                : [];
              w.permissions.allowsOpen = Array.from(new Set([...existing, ...extra]));
            }
          }
          manifestObj = m;
        }
      } catch {
        // Fall back to letting NearConnector load its defaults.
      }

      if (cancelled) return;

      connector = new NearConnector({
        network: network,
        autoConnect: false,
        // We previously excluded `meteor-wallet` and `trezu-wallet`
        // because their bundled @near-js predates the
        // UseGlobalContract action and the vault deploy tx would
        // fail with "Invalid action type". Re-enabling them now —
        // most wallet operations (login, signMessage, regular
        // signAndSendTransaction) work fine, and the explicit
        // pre-flight check in `signAndSendTransaction` below catches
        // UseGlobalContract attempts with a clear error pointing the
        // user at MyNearWallet / HOT / Intear instead of the cryptic
        // wallet-side failure.
        ...(manifestObj ? { manifest: manifestObj } : {}),
      });

      connectorRef.current = connector;

      connector.on('wallet:signIn', handleSignIn as any);
      // Some wallets emit `wallet:signInAndSignMessage` when the user
      // signs in and signs a message in a single round-trip; the
      // payload shape is the same as `wallet:signIn`. Without this
      // listener we'd miss the accountId update for those flows
      // (caught by trezu's reference impl in nt-fe/stores/near-store.ts).
      connector.on('wallet:signInAndSignMessage', handleSignIn as any);
      connector.on('wallet:signOut', handleSignOut);

      // Try to restore an existing session — if the user previously
      // connected on this network, near-connect surfaces the account
      // immediately without re-prompting.
      try {
        const { accounts } = await connector.getConnectedWallet();
        if (!cancelled && accounts.length > 0) {
          setAccountId(accounts[0].accountId);
        }
      } catch {
        // No existing session — that's fine.
      }
      if (!cancelled) {
        setIsWalletReady(true);
      }
    })();

    return () => {
      cancelled = true;
      if (connector) {
        connector.off('wallet:signIn', handleSignIn as any);
        connector.off('wallet:signInAndSignMessage', handleSignIn as any);
        connector.off('wallet:signOut', handleSignOut);
      }
    };
  }, [network]);

  const connect = useCallback(() => {
    if (!connectorRef.current) return;
    connectorRef.current.connect().catch(() => {
      // User rejected or wallet error — no action needed.
    });
  }, []);

  const disconnect = useCallback(async () => {
    if (!connectorRef.current) return;
    try {
      await connectorRef.current.disconnect();
    } catch {
      // Already disconnected.
    }
    setAccountId(null);
  }, []);

  const switchNetwork = useCallback(async (newNetwork: NetworkType) => {
    // Persist user choice and (legacy) ask the modal to reopen on
    // next render. With near-connect we no longer have to reload the
    // page; the useEffect above re-instantiates the connector.
    localStorage.setItem('near-wallet-selector:selectedNetworkId', newNetwork);
    localStorage.setItem('near-wallet-selector:reopenModal', 'true');

    if (connectorRef.current && accountId) {
      try {
        await connectorRef.current.disconnect();
      } catch {
        // Already disconnected.
      }
      setAccountId(null);
    }

    setNetwork(newNetwork);
  }, [accountId]);

  const signAndSendTransaction = useCallback(async (params: any) => {
    const connector = connectorRef.current;
    if (!connector) throw new Error('Wallet not initialized');
    const wallet = await connector.wallet();

    // Several wallets ship an @near-js old enough to predate the
    // UseGlobalContract action (NEP-591). Failure modes observed:
    //   - Meteor / Trezu throw a cryptic "Invalid action type" from
    //     their selector-action converter.
    //   - MyNearWallet ships near-api-js@0.45.1 (years pre-NEP-591),
    //     deserialises the action discriminant inside a Promise its
    //     UI doesn't surface — the sign page just hangs.
    // Pre-empt with an explicit message rather than letting the user
    // wait at a frozen wallet UI. Recommendation depends on network:
    // HOT's sandbox doesn't surface in the testnet picker today, so
    // there we point only at Intear.
    const usesGlobalContract = Array.isArray(params?.actions)
      && params.actions.some((a: any) => a?.useGlobalContract != null);
    if (usesGlobalContract) {
      const walletId = wallet?.manifest?.id ?? '';
      const INCOMPATIBLE = new Set(['meteor-wallet', 'trezu-wallet', 'mynearwallet']);
      if (INCOMPATIBLE.has(walletId)) {
        const recommend =
          network === 'testnet'
            ? 'Reconnect with Intear and retry.'
            : 'Reconnect with HOT or Intear and retry.';
        throw new Error(
          `${wallet.manifest.name} cannot sign vault deploys yet — its bundled `
          + `@near-js predates the UseGlobalContract action (NEP-591). `
          + recommend,
        );
      }
    }

    try {
      return await wallet.signAndSendTransaction(params);
    } catch (err) {
      // The wallet may report a false failure when its bundled near-api-js
      // broadcasts the tx, the RPC times out, and the fallback tx-status
      // poll hits a load-balanced node that hasn't synced the tx yet:
      //   "[-32000] Server error: Transaction <hash> doesn't exist".
      // The tx is on chain — recover its real outcome from our own RPC
      // before surfacing the error to the user.
      const hash = extractTxHashFromError(err);
      if (hash && accountId) {
        const outcome = await waitForTransactionOutcome(hash, accountId, config.rpcUrl);
        if (outcome) return outcome;
      }
      throw err;
    }
  }, [accountId, network, config.rpcUrl]);

  const signMessage = useCallback(async (params: SignMessageParams): Promise<SignedMessage | null> => {
    const connector = connectorRef.current;
    if (!connector || !accountId) throw new Error('Wallet not connected');

    try {
      const wallet = await connector.wallet();

      const result = await wallet.signMessage({
        message: params.message,
        recipient: params.recipient,
        nonce: Buffer.from(params.nonce, 'base64'),
        network: network,
        signerId: accountId,
      });

      if (!result) {
        return null;
      }

      // near-connect returns `signature` already as a string (base64
      // for NEP-413). No more shape-detection wallet-selector forced
      // on us.
      return {
        signature: result.signature,
        publicKey: result.publicKey,
        accountId: result.accountId,
      };
    } catch (error) {
      console.error('Error signing message:', error);
      throw error;
    }
  }, [accountId, network]);

  const viewMethod = useCallback(async (params: { contractId: string; method: string; args?: Record<string, unknown> }) => {
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
  }, [config.rpcUrl]);

  return (
    <NearWalletContext.Provider
      value={{
        accountId,
        isConnected: !!accountId,
        isWalletReady,
        network,
        contractId: config.contractId,
        rpcUrl: config.rpcUrl,
        stablecoin: config.stablecoin,
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
