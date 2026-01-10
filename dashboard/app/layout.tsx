'use client';

import { Inter } from 'next/font/google';
import './globals.css';
import { NearWalletProvider } from '@/contexts/NearWalletContext';
import Link from 'next/link';
import { useState } from 'react';

const inter = Inter({ subsets: ['latin'] });

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
                          <Link href="/earnings" className="block px-4 py-2 text-sm text-gray-700 hover:bg-gray-50 hover:text-[#cc6600]">
                            Earnings
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
