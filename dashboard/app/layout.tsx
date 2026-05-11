'use client';

import { Inter } from 'next/font/google';
import './globals.css';
import { NearWalletProvider } from '@/contexts/NearWalletContext';
import Link from 'next/link';
import { useState, useEffect, useCallback, useRef } from 'react';
import { useNearWallet } from '@/contexts/NearWalletContext';
import { getCoordinatorApiUrl } from '@/lib/api';
import TestnetDisabledNotice from '@/components/TestnetDisabledNotice';

const inter = Inter({ subsets: ['latin'] });

// Poll once per minute globally — single leader across all tabs +
// all pages of the dashboard. Followers (other tabs, other browser
// windows) receive the result via BroadcastChannel without hitting
// the coordinator themselves. The /wallet/approvals page listens on
// the same channel and uses these broadcasts as its data source.
const POLL_INTERVAL_MS = 60_000;
const LOCK_NAME = 'outlayer-approvals-poller';
const CHANNEL_NAME = 'outlayer-approvals-results';

function PendingApprovalsBadge() {
  const { accountId, isConnected, network, contractId, viewMethod } = useNearWallet();
  const coordinatorUrl = getCoordinatorApiUrl(network);
  const [count, setCount] = useState(0);
  // Stable ref for viewMethod to avoid re-triggering useEffect on every render
  const viewMethodRef = useRef(viewMethod);
  viewMethodRef.current = viewMethod;

  const fetchAndBroadcast = useCallback(
    async (channel: BroadcastChannel | null) => {
      if (!accountId || !contractId) return;
      try {
        const wallets = await viewMethodRef.current({
          contractId,
          method: 'get_wallet_policies_by_owner',
          args: { owner: accountId },
        }).catch(() => []) as Array<{ wallet_pubkey: string }>;

        const allApprovals: Array<{ wallet_pubkey: string } & Record<string, unknown>> = [];
        for (const w of wallets) {
          try {
            const resp = await fetch(
              `${coordinatorUrl}/wallet/v1/pending_approvals_by_pubkey?near_pubkey=${encodeURIComponent(w.wallet_pubkey)}`,
            );
            if (!resp.ok) continue;
            const data = await resp.json();
            if (data.pending_approvals) {
              for (const pa of data.pending_approvals) {
                allApprovals.push({ ...pa, wallet_pubkey: w.wallet_pubkey });
              }
            }
          } catch { /* skip */ }
        }
        setCount(allApprovals.length);
        // Broadcast full approvals payload so the /wallet/approvals
        // page (when open in another tab) consumes the same fetch.
        channel?.postMessage({ type: 'approvals-update', approvals: allApprovals });
      } catch { /* skip */ }
    },
    [accountId, contractId, coordinatorUrl],
  );

  useEffect(() => {
    if (!isConnected || !accountId) { setCount(0); return; }

    const channel =
      typeof BroadcastChannel !== 'undefined'
        ? new BroadcastChannel(CHANNEL_NAME)
        : null;

    // Followers update their badge count when ANY tab broadcasts.
    if (channel) {
      channel.onmessage = (event) => {
        if (event.data?.type === 'approvals-update' && Array.isArray(event.data.approvals)) {
          setCount(event.data.approvals.length);
        }
      };
    }

    let cancelled = false;
    let pollIntervalId: ReturnType<typeof setInterval> | null = null;
    let releaseLock: (() => void) | null = null;

    const startLeaderPolling = () => {
      if (cancelled) return;
      // Immediate fetch when becoming leader, then once per minute.
      fetchAndBroadcast(channel);
      pollIntervalId = setInterval(() => fetchAndBroadcast(channel), POLL_INTERVAL_MS);
    };

    const locks = typeof navigator !== 'undefined' ? navigator.locks : undefined;
    if (locks && typeof locks.request === 'function') {
      locks.request(
        LOCK_NAME,
        { mode: 'exclusive' },
        () =>
          new Promise<void>((release) => {
            if (cancelled) { release(); return; }
            releaseLock = release;
            startLeaderPolling();
          }),
      );
    } else {
      // No Web Locks API — every tab polls independently. Acceptable
      // fallback; modern browsers all support locks.
      startLeaderPolling();
    }

    return () => {
      cancelled = true;
      if (pollIntervalId) clearInterval(pollIntervalId);
      if (releaseLock) releaseLock();
      channel?.close();
    };
  }, [isConnected, accountId, fetchAndBroadcast]);

  if (count <= 0) return null;
  return (
    <span className="ml-1.5 inline-flex items-center justify-center w-5 h-5 text-xs font-bold text-white bg-red-500 rounded-full">
      {count}
    </span>
  );
}

export default function RootLayout({
  children,
}: Readonly<{
  children: React.ReactNode;
}>) {
  const [mobileMenuOpen, setMobileMenuOpen] = useState(false);

  return (
    <html lang="en">
      <head>
        <title>OutLayer. Verifiable off-chain computation for NEAR Blockchain</title>
        <link rel="icon" href="/favicon.ico" sizes="any" />
        <link rel="icon" href="/icon.svg" type="image/svg+xml" />
        <link rel="apple-touch-icon" href="/apple-touch-icon.png" />
      </head>
      <body className={inter.className}>
        <NearWalletProvider>
          <div className="min-h-screen bg-gray-50">
            <TestnetDisabledNotice variant="banner" />
            {/* Navigation */}
            <nav className="relative z-50 bg-white shadow-sm">
              <div className="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8">
                <div className="flex justify-between h-16">
                  <div className="flex items-center space-x-8">
                    <Link href="/" className="text-xl font-bold text-transparent bg-clip-text bg-gradient-to-r from-[#cc6600] to-[#d4a017]">
                      NEAR OutLayer
                    </Link>
                    {/* Desktop menu - left side */}
                    <div className="hidden md:flex space-x-4">
                      <Link href="/docs" className="text-gray-700 hover:text-[#cc6600] px-3 py-2 rounded-md text-sm font-medium transition-colors">
                        Docs
                      </Link>
                      <Link href="/executions" className="text-gray-700 hover:text-[#cc6600] px-3 py-2 rounded-md text-sm font-medium transition-colors">
                        Executions
                      </Link>
                      <Link href="/stats" className="text-gray-700 hover:text-[#cc6600] px-3 py-2 rounded-md text-sm font-medium transition-colors">
                        Stats
                      </Link>
                      <Link href="/playground" className="text-gray-700 hover:text-[#cc6600] px-3 py-2 rounded-md text-sm font-medium transition-colors">
                        Playground
                      </Link>
                    </div>
                  </div>

                  {/* Desktop menu - right side with dropdown */}
                  <div className="hidden md:flex items-center">
                    <div className="relative group">
                      <Link
                        href="/workspace"
                        className="text-gray-700 hover:text-[#cc6600] px-3 py-2 rounded-md text-sm font-medium transition-colors border border-gray-200 hover:border-[#cc6600] rounded-lg inline-flex items-center gap-1"
                      >
                        My Workspace
                        <PendingApprovalsBadge />
                        <svg className="w-4 h-4 transition-transform group-hover:rotate-180" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                          <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M19 9l-7 7-7-7" />
                        </svg>
                      </Link>
                      {/* Dropdown menu */}
                      <div className="absolute right-0 mt-1 w-48 bg-white rounded-lg shadow-lg border border-gray-200 opacity-0 invisible group-hover:opacity-100 group-hover:visible transition-all duration-150 z-50">
                        <div className="py-1">
                          <Link href="/workspace" className="block px-4 py-2 text-sm text-gray-700 hover:bg-gray-50 hover:text-[#cc6600]">
                            Overview
                          </Link>
                          <Link href="/projects" className="block px-4 py-2 text-sm text-gray-700 hover:bg-gray-50 hover:text-[#cc6600]">
                            Projects
                          </Link>
                          <Link href="/secrets" className="block px-4 py-2 text-sm text-gray-700 hover:bg-gray-50 hover:text-[#cc6600]">
                            Secrets
                          </Link>
                          <Link href="/payment-keys" className="block px-4 py-2 text-sm text-gray-700 hover:bg-gray-50 hover:text-[#cc6600]">
                            Payment Keys
                          </Link>
                          <Link href="/vault" className="block px-4 py-2 text-sm text-gray-700 hover:bg-gray-50 hover:text-[#cc6600]">
                            Vaults
                          </Link>
                          <Link href="/earnings" className="block px-4 py-2 text-sm text-gray-700 hover:bg-gray-50 hover:text-[#cc6600]">
                            Earnings
                          </Link>
                          <div className="border-t border-gray-100 my-1"></div>
                          <Link href="/wallet/manage" className="block px-4 py-2 text-sm text-gray-700 hover:bg-gray-50 hover:text-[#cc6600]">
                            Wallet Management
                          </Link>
                          <Link href="/wallet/approvals" className="flex items-center px-4 py-2 text-sm text-gray-700 hover:bg-gray-50 hover:text-[#cc6600]">
                            Wallet Approvals
                            <PendingApprovalsBadge />
                          </Link>
                          <div className="border-t border-gray-100 my-1"></div>
                          <Link href="/settings" className="block px-4 py-2 text-sm text-gray-700 hover:bg-gray-50 hover:text-[#cc6600]">
                            Settings
                          </Link>
                        </div>
                      </div>
                    </div>
                  </div>

                  {/* Mobile menu button */}
                  <div className="md:hidden flex items-center">
                    <button
                      onClick={() => setMobileMenuOpen(!mobileMenuOpen)}
                      className="text-gray-700 hover:text-[#cc6600] p-2"
                    >
                      <svg className="h-6 w-6" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                        {mobileMenuOpen ? (
                          <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M6 18L18 6M6 6l12 12" />
                        ) : (
                          <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M4 6h16M4 12h16M4 18h16" />
                        )}
                      </svg>
                    </button>
                  </div>
                </div>

                {/* Mobile menu */}
                {mobileMenuOpen && (
                  <div className="md:hidden pb-4">
                    <div className="flex flex-col space-y-2">
                      <Link
                        href="/docs"
                        onClick={() => setMobileMenuOpen(false)}
                        className="text-gray-700 hover:text-[#cc6600] px-3 py-2 rounded-md text-sm font-medium transition-colors"
                      >
                        Docs
                      </Link>
                      <Link
                        href="/executions"
                        onClick={() => setMobileMenuOpen(false)}
                        className="text-gray-700 hover:text-[#cc6600] px-3 py-2 rounded-md text-sm font-medium transition-colors"
                      >
                        Executions
                      </Link>
                      <Link
                        href="/stats"
                        onClick={() => setMobileMenuOpen(false)}
                        className="text-gray-700 hover:text-[#cc6600] px-3 py-2 rounded-md text-sm font-medium transition-colors"
                      >
                        Stats
                      </Link>
                      <Link
                        href="/playground"
                        onClick={() => setMobileMenuOpen(false)}
                        className="text-gray-700 hover:text-[#cc6600] px-3 py-2 rounded-md text-sm font-medium transition-colors"
                      >
                        Playground
                      </Link>
                      <Link
                        href="/workspace"
                        onClick={() => setMobileMenuOpen(false)}
                        className="text-gray-700 hover:text-[#cc6600] px-3 py-2 rounded-md text-sm font-medium transition-colors"
                      >
                        My Workspace
                      </Link>
                    </div>
                  </div>
                )}
              </div>
            </nav>

            {/* Main Content */}
            <main className="max-w-7xl mx-auto py-6 px-4 sm:px-6 lg:px-8">
              {children}
            </main>

            {/* Footer */}
            <footer className="bg-white mt-12 border-t">
              <div className="max-w-7xl mx-auto py-6 px-4 sm:px-6 lg:px-8">
                <p className="text-center text-gray-500 text-sm">
                  © 2025 NEAR OutLayer. Verifiable off-chain computation for NEAR Protocol.
                </p>
              </div>
            </footer>
          </div>
        </NearWalletProvider>
      </body>
    </html>
  );
}
