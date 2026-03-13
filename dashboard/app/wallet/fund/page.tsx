'use client';

import { Suspense, useState, useEffect, useCallback } from 'react';
import { useSearchParams } from 'next/navigation';
import { useNearWallet } from '@/contexts/NearWalletContext';
import WalletConnectionModal from '@/components/WalletConnectionModal';
import { actionCreators } from '@near-js/transactions';
import { getTransactionUrl } from '@/lib/explorer';

interface TokenMeta {
  symbol: string;
  decimals: number;
  icon: string | null;
}

/** Convert human-readable amount to yoctoNEAR string */
function nearToYocto(amount: string): string {
  const parsed = parseFloat(amount);
  if (isNaN(parsed) || parsed <= 0) throw new Error('Invalid amount');
  const yocto = BigInt(Math.round(parsed * 1e6)) * BigInt(1e18);
  return yocto.toString();
}

/** Convert human-readable amount to FT minimal units using decimals */
function toMinimalUnits(amount: string, decimals: number): string {
  const parts = amount.split('.');
  const whole = parts[0] || '0';
  const frac = (parts[1] || '').padEnd(decimals, '0').slice(0, decimals);
  const result = BigInt(whole) * BigInt(10 ** decimals) + BigInt(frac);
  return result.toString();
}

/** Format yoctoNEAR to human-readable */
function formatYocto(yocto: string): string {
  const near = parseFloat(yocto) / 1e24;
  return near.toFixed(4);
}

/** Format FT minimal units to human-readable */
function formatFtAmount(minimal: string, decimals: number): string {
  if (!minimal || minimal === '0') return '0';
  const val = BigInt(minimal);
  const divisor = BigInt(10 ** decimals);
  const whole = val / divisor;
  const remainder = val % divisor;
  const fracStr = remainder.toString().padStart(decimals, '0').replace(/0+$/, '');
  return fracStr ? `${whole}.${fracStr}` : whole.toString();
}

/** Truncate a hex account for display */
function truncateAccount(account: string): string {
  if (account.length <= 20) return account;
  return `${account.slice(0, 10)}...${account.slice(-8)}`;
}

export default function FundPage() {
  return (
    <Suspense fallback={<div className="flex justify-center py-20"><LoadingSpinner /></div>}>
      <FundContent />
    </Suspense>
  );
}

function FundContent() {
  const searchParams = useSearchParams();
  const { accountId, isConnected, signAndSendTransaction, viewMethod, rpcUrl, network } = useNearWallet();

  const to = searchParams.get('to');
  const amount = searchParams.get('amount');
  const tokenParam = searchParams.get('token') || 'near';
  const msg = searchParams.get('msg');
  const destParam = searchParams.get('dest'); // "intents" = deposit to intents balance
  const isNative = !tokenParam || tokenParam === 'near';

  const [showWalletModal, setShowWalletModal] = useState(false);
  const [tokenMeta, setTokenMeta] = useState<TokenMeta | null>(null);
  const [userBalance, setUserBalance] = useState<string | null>(null);
  const [needsStorage, setNeedsStorage] = useState(false);
  const [sending, setSending] = useState(false);
  const [txHash, setTxHash] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [copied, setCopied] = useState(false);
  // Intents deposit toggle — only for FT tokens
  const [depositToIntents, setDepositToIntents] = useState(destParam === 'intents' && !isNative);

  // Validate params
  if (!to || !amount) {
    return (
      <div className="max-w-lg mx-auto mt-12">
        <div className="bg-red-50 border border-red-200 rounded-lg p-6 text-center">
          <h2 className="text-lg font-semibold text-red-800 mb-2">Invalid Fund Link</h2>
          <p className="text-red-700 text-sm">
            Missing required parameters. The link should include <code className="bg-red-100 px-1 rounded">to</code> and <code className="bg-red-100 px-1 rounded">amount</code>.
          </p>
        </div>
      </div>
    );
  }

  const parsedAmount = parseFloat(amount);
  if (isNaN(parsedAmount) || parsedAmount <= 0) {
    return (
      <div className="max-w-lg mx-auto mt-12">
        <div className="bg-red-50 border border-red-200 rounded-lg p-6 text-center">
          <h2 className="text-lg font-semibold text-red-800 mb-2">Invalid Amount</h2>
          <p className="text-red-700 text-sm">Amount must be a positive number.</p>
        </div>
      </div>
    );
  }

  // Fetch token metadata for FT tokens
  // eslint-disable-next-line react-hooks/rules-of-hooks
  useEffect(() => {
    if (isNative) {
      setTokenMeta({ symbol: 'NEAR', decimals: 24, icon: null });
      return;
    }
    (async () => {
      try {
        const meta = await viewMethod({ contractId: tokenParam, method: 'ft_metadata', args: {} }) as TokenMeta;
        setTokenMeta(meta);
      } catch {
        setError(`Failed to fetch token metadata for ${tokenParam}`);
      }
    })();
  }, [isNative, tokenParam, viewMethod]);

  // Fetch user balance + storage check when connected
  // eslint-disable-next-line react-hooks/rules-of-hooks
  const fetchBalances = useCallback(async () => {
    if (!isConnected || !accountId) return;
    setError(null);

    try {
      if (isNative) {
        // Fetch native NEAR balance via RPC
        const res = await fetch(rpcUrl, {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({
            jsonrpc: '2.0', id: 'balance',
            method: 'query',
            params: { request_type: 'view_account', finality: 'final', account_id: accountId },
          }),
        });
        const data = await res.json();
        if (data.result?.amount) {
          setUserBalance(data.result.amount);
        } else {
          setUserBalance('0');
        }
      } else {
        // FT: check user balance
        const bal = await viewMethod({
          contractId: tokenParam,
          method: 'ft_balance_of',
          args: { account_id: accountId },
        }) as string;
        setUserBalance(bal || '0');

        // Check if receiver needs storage registration on token contract
        // When depositing to intents, the receiver is intents.near (not the agent)
        const storageTarget = depositToIntents ? 'intents.near' : to;
        const storage = await viewMethod({
          contractId: tokenParam,
          method: 'storage_balance_of',
          args: { account_id: storageTarget },
        });
        setNeedsStorage(!storage);
      }
    } catch (e: unknown) {
      const errMsg = e instanceof Error ? e.message : String(e);
      setError(`Failed to check balances: ${errMsg}`);
    }
  }, [isConnected, accountId, isNative, rpcUrl, tokenParam, to, viewMethod, depositToIntents]);

  // eslint-disable-next-line react-hooks/rules-of-hooks
  useEffect(() => {
    fetchBalances();
  }, [fetchBalances]);

  const symbol = tokenMeta?.symbol || (isNative ? 'NEAR' : tokenParam);
  const decimals = tokenMeta?.decimals ?? (isNative ? 24 : 0);

  // Check if user has enough balance
  const hasEnough = (() => {
    if (!userBalance || !decimals) return false;
    if (isNative) {
      try {
        const required = BigInt(nearToYocto(amount));
        const reserve = BigInt('50000000000000000000000'); // 0.05 NEAR reserve
        return BigInt(userBalance) >= required + reserve;
      } catch { return false; }
    } else {
      try {
        const required = BigInt(toMinimalUnits(amount, decimals));
        return BigInt(userBalance) >= required;
      } catch { return false; }
    }
  })();

  const handleSend = async () => {
    if (!to || !amount) return;
    setSending(true);
    setError(null);

    try {
      let result;

      if (isNative) {
        const yoctoAmount = nearToYocto(amount);
        result = await signAndSendTransaction({
          receiverId: to,
          actions: [actionCreators.transfer(BigInt(yoctoAmount))],
        });
      } else {
        const minimalUnits = toMinimalUnits(amount, decimals);
        const actions = [];

        if (depositToIntents) {
          // Deposit to agent's intents balance via ft_transfer_call to intents.near
          // msg = agent account ID → intents.near credits the agent
          if (needsStorage) {
            actions.push(
              actionCreators.functionCall(
                'storage_deposit',
                { account_id: 'intents.near', registration_only: true },
                BigInt('30000000000000'), // 30 TGas
                BigInt('1250000000000000000000'), // 0.00125 NEAR
              ),
            );
          }

          actions.push(
            actionCreators.functionCall(
              'ft_transfer_call',
              { receiver_id: 'intents.near', amount: minimalUnits, msg: to },
              BigInt('100000000000000'), // 100 TGas
              BigInt('1'), // 1 yoctoNEAR
            ),
          );
        } else {
          // Direct FT transfer to agent account
          if (needsStorage) {
            actions.push(
              actionCreators.functionCall(
                'storage_deposit',
                { account_id: to, registration_only: true },
                BigInt('30000000000000'), // 30 TGas
                BigInt('1250000000000000000000'), // 0.00125 NEAR
              ),
            );
          }

          actions.push(
            actionCreators.functionCall(
              'ft_transfer',
              { receiver_id: to, amount: minimalUnits, memo: null },
              BigInt('30000000000000'), // 30 TGas
              BigInt('1'), // 1 yoctoNEAR
            ),
          );
        }

        result = await signAndSendTransaction({
          receiverId: tokenParam,
          actions,
        });
      }

      // Extract tx hash from result
      const hash = result?.transaction_outcome?.id
        || result?.transaction?.hash
        || (typeof result === 'string' ? result : null);
      if (hash) {
        setTxHash(hash);
      } else {
        setTxHash('submitted');
      }
    } catch (e: unknown) {
      const errMsg = e instanceof Error ? e.message : String(e);
      if (!errMsg.includes('User rejected') && !errMsg.includes('cancelled')) {
        setError(errMsg);
      }
    } finally {
      setSending(false);
    }
  };

  const copyAddress = () => {
    navigator.clipboard.writeText(to);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };

  // Success state
  if (txHash) {
    return (
      <div className="max-w-lg mx-auto mt-12">
        <div className="bg-white shadow rounded-lg p-6 text-center">
          <div className="w-16 h-16 bg-green-100 rounded-full flex items-center justify-center mx-auto mb-4">
            <svg className="w-8 h-8 text-green-600" fill="none" viewBox="0 0 24 24" stroke="currentColor">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M5 13l4 4L19 7" />
            </svg>
          </div>
          <h2 className="text-xl font-semibold text-gray-900 mb-2">Transfer Complete</h2>
          <p className="text-gray-600 mb-4">
            Sent {amount} {symbol} {depositToIntents ? 'to agent\u2019s Intents balance' : 'to agent'}
          </p>
          {txHash !== 'submitted' && (
            <a
              href={getTransactionUrl(txHash, network)}
              target="_blank"
              rel="noopener noreferrer"
              className="text-[#cc6600] hover:text-[#b35900] text-sm font-medium underline"
            >
              View transaction on explorer
            </a>
          )}
        </div>
      </div>
    );
  }

  return (
    <div className="max-w-lg mx-auto mt-12">
      <div className="bg-white shadow rounded-lg p-6">
        {/* Header */}
        <div className="text-center mb-6">
          <div className="w-12 h-12 bg-orange-100 rounded-full flex items-center justify-center mx-auto mb-3">
            {tokenMeta?.icon ? (
              <img src={tokenMeta.icon} alt={symbol} className="w-8 h-8 rounded-full" />
            ) : (
              <svg className="w-6 h-6 text-[#cc6600]" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M12 8c-1.657 0-3 .895-3 2s1.343 2 3 2 3 .895 3 2-1.343 2-3 2m0-8c1.11 0 2.08.402 2.599 1M12 8V7m0 1v8m0 0v1m0-1c-1.11 0-2.08-.402-2.599-1M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
              </svg>
            )}
          </div>
          <h1 className="text-xl font-semibold text-gray-900">Agent Fund Request</h1>
          <p className="text-gray-500 text-sm mt-1">An AI agent is requesting a transfer</p>
        </div>

        {/* Amount display */}
        <div className="bg-gray-50 rounded-lg p-4 mb-4">
          <div className="text-center">
            <span className="text-3xl font-bold text-gray-900">{amount}</span>
            <span className="text-xl text-gray-600 ml-2">{symbol}</span>
          </div>
        </div>

        {/* Agent message */}
        {msg && (
          <div className="bg-blue-50 border border-blue-200 rounded-lg p-3 mb-4">
            <p className="text-blue-800 text-sm">{msg}</p>
          </div>
        )}

        {/* Recipient */}
        <div className="mb-4">
          <label className="block text-xs font-medium text-gray-500 uppercase tracking-wide mb-1">Recipient</label>
          <div className="flex items-center gap-2 bg-gray-50 rounded-lg px-3 py-2">
            <span className="font-mono text-sm text-gray-700 flex-1 truncate">{truncateAccount(to)}</span>
            <button
              onClick={copyAddress}
              className="text-gray-400 hover:text-gray-600 text-xs flex-shrink-0"
              title="Copy full address"
            >
              {copied ? 'Copied!' : 'Copy'}
            </button>
          </div>
        </div>

        {/* Intents deposit toggle — FT tokens only */}
        {!isNative && (
          <div className="mb-4">
            <label className="flex items-center justify-between bg-gray-50 rounded-lg px-3 py-2.5 cursor-pointer">
              <div>
                <span className="text-sm font-medium text-gray-700">Deposit to Intents balance</span>
                <p className="text-xs text-gray-500 mt-0.5">
                  {depositToIntents
                    ? 'Funds go to agent\u2019s trading balance (swaps, payments)'
                    : 'Funds go directly to agent\u2019s token account'}
                </p>
              </div>
              <button
                type="button"
                role="switch"
                aria-checked={depositToIntents}
                onClick={() => setDepositToIntents(!depositToIntents)}
                className={`relative inline-flex h-6 w-11 flex-shrink-0 rounded-full border-2 border-transparent transition-colors duration-200 ease-in-out focus:outline-none ${
                  depositToIntents ? 'bg-[#cc6600]' : 'bg-gray-300'
                }`}
              >
                <span
                  className={`pointer-events-none inline-block h-5 w-5 rounded-full bg-white shadow transform transition duration-200 ease-in-out ${
                    depositToIntents ? 'translate-x-5' : 'translate-x-0'
                  }`}
                />
              </button>
            </label>
          </div>
        )}

        {/* Storage deposit notice */}
        {!isNative && needsStorage && (
          <div className="bg-yellow-50 border border-yellow-200 rounded-lg p-3 mb-4">
            <p className="text-yellow-800 text-sm">
              The recipient is not registered on this token contract. A one-time storage deposit of 0.00125 NEAR will be included automatically.
            </p>
          </div>
        )}

        {/* Error */}
        {error && (
          <div className="bg-red-50 border border-red-200 rounded-lg p-3 mb-4">
            <p className="text-red-800 text-sm">{error}</p>
          </div>
        )}

        {/* Connect or Send */}
        {!isConnected ? (
          <button
            onClick={() => setShowWalletModal(true)}
            className="w-full px-4 py-3 bg-gradient-to-r from-[#cc6600] to-[#d4a017] text-white rounded-lg font-medium hover:from-[#b35900] hover:to-[#c49016] transition-colors shadow-sm hover:shadow-md"
          >
            Connect Wallet to Send
          </button>
        ) : (
          <div>
            {/* Balance info */}
            {userBalance !== null && (
              <div className="text-sm text-gray-500 mb-3">
                Your balance:{' '}
                <span className="font-mono font-medium text-gray-700">
                  {isNative
                    ? `${formatYocto(userBalance)} NEAR`
                    : `${formatFtAmount(userBalance, decimals)} ${symbol}`}
                </span>
                {!hasEnough && (
                  <span className="text-red-600 ml-2">
                    (insufficient{isNative ? ' — keep ~0.05 NEAR for fees' : ''})
                  </span>
                )}
              </div>
            )}

            <button
              onClick={handleSend}
              disabled={sending || !hasEnough || !tokenMeta}
              className="w-full px-4 py-3 bg-gradient-to-r from-[#cc6600] to-[#d4a017] text-white rounded-lg font-medium hover:from-[#b35900] hover:to-[#c49016] transition-colors shadow-sm hover:shadow-md disabled:opacity-50 disabled:cursor-not-allowed"
            >
              {sending ? (
                <span className="flex items-center justify-center gap-2">
                  <LoadingSpinner small />
                  Sending...
                </span>
              ) : (
                `Send ${amount} ${symbol}`
              )}
            </button>

            <p className="text-xs text-gray-400 text-center mt-2">
              Connected as {accountId}
            </p>
          </div>
        )}
      </div>

      <WalletConnectionModal isOpen={showWalletModal} onClose={() => setShowWalletModal(false)} />
    </div>
  );
}

function LoadingSpinner({ small }: { small?: boolean }) {
  const size = small ? 'h-4 w-4' : 'h-8 w-8';
  return (
    <svg className={`animate-spin ${size} text-[#cc6600]`} xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24">
      <circle className="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" strokeWidth="4" />
      <path className="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z" />
    </svg>
  );
}
