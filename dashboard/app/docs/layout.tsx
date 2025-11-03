'use client';

import Link from 'next/link';
import { usePathname } from 'next/navigation';

export default function DocsLayout({
  children,
}: {
  children: React.ReactNode;
}) {
  const pathname = usePathname();

  const isActive = (path: string) => pathname === path;

  return (
    <div className="max-w-7xl mx-auto">
      <h1 className="text-4xl font-bold mb-8 text-transparent bg-clip-text bg-gradient-to-r from-[var(--primary-orange)] to-[var(--primary-green)]">
        OutLayer Documentation
      </h1>

      <div className="grid grid-cols-1 lg:grid-cols-4 gap-8">
        {/* Sidebar Navigation */}
        <div className="lg:col-span-1">
          <div className="bg-white rounded-lg shadow-sm p-6 sticky top-4">
            <h3 className="text-lg font-bold mb-4 text-gray-900">Contents</h3>
            <nav className="space-y-2">
              <Link
                href="/docs/getting-started"
                className={`block w-full text-left px-3 py-2 rounded-md text-sm font-medium transition-colors ${
                  isActive('/docs/getting-started')
                    ? 'bg-[var(--primary-orange)] text-white'
                    : 'text-gray-700 hover:bg-gray-100'
                }`}
              >
                Getting Started
              </Link>
              <Link
                href="/docs/dev-guide"
                className={`block w-full text-left px-3 py-2 rounded-md text-sm font-medium transition-colors ${
                  isActive('/docs/dev-guide')
                    ? 'bg-[var(--primary-orange)] text-white'
                    : 'text-gray-700 hover:bg-gray-100'
                }`}
              >
                Developer Guide
              </Link>
              <Link
                href="/docs/contract-integration"
                className={`block w-full text-left px-3 py-2 rounded-md text-sm font-medium transition-colors ${
                  isActive('/docs/contract-integration')
                    ? 'bg-[var(--primary-orange)] text-white'
                    : 'text-gray-700 hover:bg-gray-100'
                }`}
              >
                Contract Integration
              </Link>
              <Link
                href="/docs/wasi"
                className={`block w-full text-left px-3 py-2 rounded-md text-sm font-medium transition-colors ${
                  isActive('/docs/wasi')
                    ? 'bg-[var(--primary-orange)] text-white'
                    : 'text-gray-700 hover:bg-gray-100'
                }`}
              >
                Writing WASI Code
              </Link>
              <Link
                href="/docs/secrets"
                className={`block w-full text-left px-3 py-2 rounded-md text-sm font-medium transition-colors ${
                  isActive('/docs/secrets')
                    ? 'bg-[var(--primary-orange)] text-white'
                    : 'text-gray-700 hover:bg-gray-100'
                }`}
              >
                Managing Secrets
              </Link>
              <Link
                href="/docs/pricing"
                className={`block w-full text-left px-3 py-2 rounded-md text-sm font-medium transition-colors ${
                  isActive('/docs/pricing')
                    ? 'bg-[var(--primary-orange)] text-white'
                    : 'text-gray-700 hover:bg-gray-100'
                }`}
              >
                Pricing & Limits
              </Link>
              <Link
                href="/docs/architecture"
                className={`block w-full text-left px-3 py-2 rounded-md text-sm font-medium transition-colors ${
                  isActive('/docs/architecture')
                    ? 'bg-[var(--primary-orange)] text-white'
                    : 'text-gray-700 hover:bg-gray-100'
                }`}
              >
                Architecture
              </Link>
            </nav>
          </div>
        </div>

        {/* Main Content */}
        <div className="lg:col-span-3">
          <div className="bg-white rounded-lg shadow-sm p-8">
            {children}
          </div>
        </div>
      </div>
    </div>
  );
}
