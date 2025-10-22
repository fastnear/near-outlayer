import type { Metadata } from 'next';
import { Inter } from 'next/font/google';
import './globals.css';
import { NearWalletProvider } from '@/contexts/NearWalletContext';
import Link from 'next/link';

const inter = Inter({ subsets: ['latin'] });

export const metadata: Metadata = {
  title: 'NEAR Offshore - Verifiable Off-Chain Computation',
  description: 'Dashboard for NEAR Offshore off-chain execution platform',
};

export default function RootLayout({
  children,
}: Readonly<{
  children: React.ReactNode;
}>) {
  return (
    <html lang="en">
      <body className={inter.className}>
        <NearWalletProvider>
          <div className="min-h-screen bg-gray-50">
            {/* Navigation */}
            <nav className="bg-white shadow-sm">
              <div className="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8">
                <div className="flex justify-between h-16">
                  <div className="flex items-center space-x-8">
                    <Link href="/" className="text-xl font-bold text-blue-600">
                      NEAR Offshore
                    </Link>
                    <div className="hidden sm:flex space-x-4">
                      <Link href="/workers" className="text-gray-700 hover:text-blue-600 px-3 py-2 rounded-md text-sm font-medium">
                        Workers
                      </Link>
                      <Link href="/executions" className="text-gray-700 hover:text-blue-600 px-3 py-2 rounded-md text-sm font-medium">
                        Executions
                      </Link>
                      <Link href="/secrets" className="text-gray-700 hover:text-blue-600 px-3 py-2 rounded-md text-sm font-medium">
                        🔐 Secrets
                      </Link>
                      <Link href="/stats" className="text-gray-700 hover:text-blue-600 px-3 py-2 rounded-md text-sm font-medium">
                        Stats
                      </Link>
                      <Link href="/playground" className="text-gray-700 hover:text-blue-600 px-3 py-2 rounded-md text-sm font-medium">
                        Playground
                      </Link>
                      <Link href="/settings" className="text-gray-700 hover:text-blue-600 px-3 py-2 rounded-md text-sm font-medium">
                        Settings
                      </Link>
                    </div>
                  </div>
                </div>
              </div>
            </nav>

            {/* Main Content */}
            <main className="max-w-7xl mx-auto py-6 sm:px-6 lg:px-8">
              {children}
            </main>

            {/* Footer */}
            <footer className="bg-white mt-12 border-t">
              <div className="max-w-7xl mx-auto py-6 px-4 sm:px-6 lg:px-8">
                <p className="text-center text-gray-500 text-sm">
                  © 2025 NEAR Offshore. Verifiable off-chain computation for NEAR Protocol.
                </p>
              </div>
            </footer>
          </div>
        </NearWalletProvider>
      </body>
    </html>
  );
}
