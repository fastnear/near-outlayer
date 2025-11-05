'use client';

import Link from 'next/link';
import { usePathname } from 'next/navigation';
import { useState, useEffect } from 'react';

// Define page structure with subsections
const pageStructure = {
  '/docs/getting-started': [
    { id: 'what-is-outlayer', title: 'What is OutLayer?' },
    { id: 'how-yield-resume-works', title: 'How Yield/Resume Works' },
    { id: 'why-outlayer', title: 'Why OutLayer Makes This Easy' },
    { id: 'quick-start', title: 'Quick Start: 4 Steps' },
    { id: 'secrets', title: 'Need API Keys or Secrets?' },
    { id: 'payment', title: 'Payment & Pricing' },
  ],
  '/docs/contract-integration': [
    { id: 'request-execution', title: 'Method: request_execution' },
    { id: 'callback', title: 'What You Get Back (Callback)' },
    { id: 'pricing-payment', title: 'Pricing & Payment' },
    { id: 'performance', title: 'Performance Tips' },
  ],
  '/docs/dev-guide': [
    { id: 'problem', title: 'The Problem' },
    { id: 'step-1', title: 'Step 1: Write WASI Code' },
    { id: 'step-2', title: 'Step 2: Push to GitHub' },
    { id: 'step-3', title: 'Step 3: Test on Testnet' },
    { id: 'step-4', title: 'Step 4: Integrate in Contract' },
  ],
  '/docs/wasi': [
    { id: 'what-is-wasi', title: 'What is WASI?' },
    { id: 'supported-languages', title: 'Supported Languages' },
    { id: 'wasi-preview', title: 'WASI Preview 1 vs Preview 2' },
    { id: 'wasi-interface', title: 'WASI Interface' },
    { id: 'critical-requirements', title: 'Critical Requirements' },
    { id: 'working-examples', title: 'Working Examples' },
    { id: 'resource-considerations', title: 'Resource Considerations' },
    { id: 'testing-locally', title: 'Testing Locally' },
    { id: 'common-pitfalls', title: 'Common Pitfalls' },
    { id: 'next-steps', title: 'Next Steps' },
  ],
  '/docs/secrets': [
    { id: 'what-are-secrets', title: 'What are Secrets?' },
    { id: 'creating-secrets', title: 'Creating Secrets' },
    { id: 'access-control', title: 'Access Control' },
    { id: 'using-secrets', title: 'Using Secrets in Code' },
    { id: 'storage-costs', title: 'Storage Costs' },
    { id: 'security-model', title: 'Security Model' },
  ],
  '/docs/pricing': [
    { id: 'dynamic-pricing', title: 'Dynamic Pricing Model' },
    { id: 'cost-calculation', title: 'Cost Calculation' },
    { id: 'resource-limits', title: 'Resource Limits' },
    { id: 'refund-policy', title: 'Refund Policy' },
    { id: 'optimization-tips', title: 'Optimization Tips' },
  ],
  '/docs/architecture': [
    { id: 'system-components', title: 'System Components' },
    { id: 'execution-flow', title: 'Execution Flow' },
    { id: 'security-guarantees', title: 'Security Guarantees' },
    { id: 'scalability', title: 'Scalability' },
    { id: 'wasm-caching', title: 'WASM Caching Strategy' },
    { id: 'high-availability', title: 'High Availability' },
  ],
};

export default function DocsLayout({
  children,
}: {
  children: React.ReactNode;
}) {
  const pathname = usePathname();
  const [expandedPages, setExpandedPages] = useState<Record<string, boolean>>({
    [pathname]: true, // Auto-expand current page
  });

  // Auto-expand menu when pathname changes
  useEffect(() => {
    if (pageStructure[pathname as keyof typeof pageStructure]) {
      setExpandedPages(prev => ({ ...prev, [pathname]: true }));
    }
  }, [pathname]);

  const isActive = (path: string) => pathname === path;

  const toggleExpand = (path: string) => {
    setExpandedPages(prev => ({ ...prev, [path]: !prev[path] }));
  };

  const scrollToSection = (sectionId: string) => {
    const element = document.getElementById(sectionId);
    if (element) {
      element.scrollIntoView({ behavior: 'smooth', block: 'start' });
    }
  };

  return (
    <div className="max-w-7xl mx-auto">
      <h1 className="text-4xl font-bold mb-8 text-transparent bg-clip-text bg-gradient-to-r from-[var(--primary-orange)] to-[var(--primary-green)]">
        OutLayer Documentation
      </h1>

      <div className="grid grid-cols-1 lg:grid-cols-4 gap-8">
        {/* Sidebar Navigation */}
        <div className="lg:col-span-1">
          <div className="bg-white rounded-lg shadow-sm p-6 sticky top-4 max-h-[calc(100vh-120px)] overflow-y-auto">
            <h3 className="text-lg font-bold mb-4 text-gray-900">Contents</h3>
            <nav className="space-y-1">
              {/* Getting Started */}
              <div>
                <Link
                  href="/docs/getting-started"
                  className={`flex items-center justify-between w-full text-left px-3 py-2 rounded-md text-sm font-medium transition-colors ${
                    isActive('/docs/getting-started')
                      ? 'bg-[var(--primary-orange)] text-white'
                      : 'text-gray-700 hover:bg-gray-100'
                  }`}
                  onClick={(e) => {
                    if (isActive('/docs/getting-started')) {
                      e.preventDefault();
                      toggleExpand('/docs/getting-started');
                    }
                  }}
                >
                  <span>Getting Started</span>
                  {pageStructure['/docs/getting-started'] && (
                    <svg className={`w-4 h-4 transition-transform ${expandedPages['/docs/getting-started'] ? 'rotate-90' : ''}`} fill="none" stroke="currentColor" viewBox="0 0 24 24">
                      <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M9 5l7 7-7 7" />
                    </svg>
                  )}
                </Link>
                {expandedPages['/docs/getting-started'] && pageStructure['/docs/getting-started'] && (
                  <div className="ml-4 mt-1 space-y-1">
                    {pageStructure['/docs/getting-started'].map(section => (
                      <button
                        key={section.id}
                        onClick={() => scrollToSection(section.id)}
                        className="block w-full text-left px-3 py-1.5 text-xs text-gray-600 hover:text-[var(--primary-orange)] hover:bg-gray-50 rounded transition-colors cursor-pointer"
                      >
                        {section.title}
                      </button>
                    ))}
                  </div>
                )}
              </div>

              {/* Developer Guide */}
              <div>
                <Link
                  href="/docs/dev-guide"
                  className={`flex items-center justify-between w-full text-left px-3 py-2 rounded-md text-sm font-medium transition-colors ${
                    isActive('/docs/dev-guide')
                      ? 'bg-[var(--primary-orange)] text-white'
                      : 'text-gray-700 hover:bg-gray-100'
                  }`}
                  onClick={(e) => {
                    if (isActive('/docs/dev-guide')) {
                      e.preventDefault();
                      toggleExpand('/docs/dev-guide');
                    }
                  }}
                >
                  <span>Developer Guide</span>
                  {pageStructure['/docs/dev-guide'] && (
                    <svg className={`w-4 h-4 transition-transform ${expandedPages['/docs/dev-guide'] ? 'rotate-90' : ''}`} fill="none" stroke="currentColor" viewBox="0 0 24 24">
                      <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M9 5l7 7-7 7" />
                    </svg>
                  )}
                </Link>
                {expandedPages['/docs/dev-guide'] && pageStructure['/docs/dev-guide'] && (
                  <div className="ml-4 mt-1 space-y-1">
                    {pageStructure['/docs/dev-guide'].map(section => (
                      <button
                        key={section.id}
                        onClick={() => scrollToSection(section.id)}
                        className="block w-full text-left px-3 py-1.5 text-xs text-gray-600 hover:text-[var(--primary-orange)] hover:bg-gray-50 rounded transition-colors cursor-pointer"
                      >
                        {section.title}
                      </button>
                    ))}
                  </div>
                )}
              </div>

              {/* Contract Integration */}
              <div>
                <Link
                  href="/docs/contract-integration"
                  className={`flex items-center justify-between w-full text-left px-3 py-2 rounded-md text-sm font-medium transition-colors ${
                    isActive('/docs/contract-integration')
                      ? 'bg-[var(--primary-orange)] text-white'
                      : 'text-gray-700 hover:bg-gray-100'
                  }`}
                  onClick={(e) => {
                    if (isActive('/docs/contract-integration')) {
                      e.preventDefault();
                      toggleExpand('/docs/contract-integration');
                    }
                  }}
                >
                  <span>Contract Integration</span>
                  {pageStructure['/docs/contract-integration'] && (
                    <svg className={`w-4 h-4 transition-transform ${expandedPages['/docs/contract-integration'] ? 'rotate-90' : ''}`} fill="none" stroke="currentColor" viewBox="0 0 24 24">
                      <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M9 5l7 7-7 7" />
                    </svg>
                  )}
                </Link>
                {expandedPages['/docs/contract-integration'] && pageStructure['/docs/contract-integration'] && (
                  <div className="ml-4 mt-1 space-y-1">
                    {pageStructure['/docs/contract-integration'].map(section => (
                      <button
                        key={section.id}
                        onClick={() => scrollToSection(section.id)}
                        className="block w-full text-left px-3 py-1.5 text-xs text-gray-600 hover:text-[var(--primary-orange)] hover:bg-gray-50 rounded transition-colors cursor-pointer"
                      >
                        {section.title}
                      </button>
                    ))}
                  </div>
                )}
              </div>

              {/* Writing WASI Code */}
              <div>
                <Link
                  href="/docs/wasi"
                  className={`flex items-center justify-between w-full text-left px-3 py-2 rounded-md text-sm font-medium transition-colors ${
                    isActive('/docs/wasi')
                      ? 'bg-[var(--primary-orange)] text-white'
                      : 'text-gray-700 hover:bg-gray-100'
                  }`}
                  onClick={(e) => {
                    if (isActive('/docs/wasi')) {
                      e.preventDefault();
                      toggleExpand('/docs/wasi');
                    }
                  }}
                >
                  <span>Writing WASI Code</span>
                  {pageStructure['/docs/wasi'] && (
                    <svg className={`w-4 h-4 transition-transform ${expandedPages['/docs/wasi'] ? 'rotate-90' : ''}`} fill="none" stroke="currentColor" viewBox="0 0 24 24">
                      <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M9 5l7 7-7 7" />
                    </svg>
                  )}
                </Link>
                {expandedPages['/docs/wasi'] && pageStructure['/docs/wasi'] && (
                  <div className="ml-4 mt-1 space-y-1">
                    {pageStructure['/docs/wasi'].map(section => (
                      <button
                        key={section.id}
                        onClick={() => scrollToSection(section.id)}
                        className="block w-full text-left px-3 py-1.5 text-xs text-gray-600 hover:text-[var(--primary-orange)] hover:bg-gray-50 rounded transition-colors cursor-pointer"
                      >
                        {section.title}
                      </button>
                    ))}
                  </div>
                )}
              </div>
              {/* WASI Examples - no subsections */}
              <Link
                href="/docs/examples"
                className={`block w-full text-left px-3 py-2 rounded-md text-sm font-medium transition-colors ${
                  isActive('/docs/examples')
                    ? 'bg-[var(--primary-orange)] text-white'
                    : 'text-gray-700 hover:bg-gray-100'
                }`}
              >
                WASI Examples
              </Link>

              {/* Managing Secrets */}
              <div>
                <Link
                  href="/docs/secrets"
                  className={`flex items-center justify-between w-full text-left px-3 py-2 rounded-md text-sm font-medium transition-colors ${
                    isActive('/docs/secrets')
                      ? 'bg-[var(--primary-orange)] text-white'
                      : 'text-gray-700 hover:bg-gray-100'
                  }`}
                  onClick={(e) => {
                    if (isActive('/docs/secrets')) {
                      e.preventDefault();
                      toggleExpand('/docs/secrets');
                    }
                  }}
                >
                  <span>Managing Secrets</span>
                  {pageStructure['/docs/secrets'] && (
                    <svg className={`w-4 h-4 transition-transform ${expandedPages['/docs/secrets'] ? 'rotate-90' : ''}`} fill="none" stroke="currentColor" viewBox="0 0 24 24">
                      <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M9 5l7 7-7 7" />
                    </svg>
                  )}
                </Link>
                {expandedPages['/docs/secrets'] && pageStructure['/docs/secrets'] && (
                  <div className="ml-4 mt-1 space-y-1">
                    {pageStructure['/docs/secrets'].map(section => (
                      <button
                        key={section.id}
                        onClick={() => scrollToSection(section.id)}
                        className="block w-full text-left px-3 py-1.5 text-xs text-gray-600 hover:text-[var(--primary-orange)] hover:bg-gray-50 rounded transition-colors cursor-pointer"
                      >
                        {section.title}
                      </button>
                    ))}
                  </div>
                )}
              </div>

              {/* Pricing & Limits */}
              <div>
                <Link
                  href="/docs/pricing"
                  className={`flex items-center justify-between w-full text-left px-3 py-2 rounded-md text-sm font-medium transition-colors ${
                    isActive('/docs/pricing')
                      ? 'bg-[var(--primary-orange)] text-white'
                      : 'text-gray-700 hover:bg-gray-100'
                  }`}
                  onClick={(e) => {
                    if (isActive('/docs/pricing')) {
                      e.preventDefault();
                      toggleExpand('/docs/pricing');
                    }
                  }}
                >
                  <span>Pricing & Limits</span>
                  {pageStructure['/docs/pricing'] && (
                    <svg className={`w-4 h-4 transition-transform ${expandedPages['/docs/pricing'] ? 'rotate-90' : ''}`} fill="none" stroke="currentColor" viewBox="0 0 24 24">
                      <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M9 5l7 7-7 7" />
                    </svg>
                  )}
                </Link>
                {expandedPages['/docs/pricing'] && pageStructure['/docs/pricing'] && (
                  <div className="ml-4 mt-1 space-y-1">
                    {pageStructure['/docs/pricing'].map(section => (
                      <button
                        key={section.id}
                        onClick={() => scrollToSection(section.id)}
                        className="block w-full text-left px-3 py-1.5 text-xs text-gray-600 hover:text-[var(--primary-orange)] hover:bg-gray-50 rounded transition-colors cursor-pointer"
                      >
                        {section.title}
                      </button>
                    ))}
                  </div>
                )}
              </div>

              {/* Architecture */}
              <div>
                <Link
                  href="/docs/architecture"
                  className={`flex items-center justify-between w-full text-left px-3 py-2 rounded-md text-sm font-medium transition-colors ${
                    isActive('/docs/architecture')
                      ? 'bg-[var(--primary-orange)] text-white'
                      : 'text-gray-700 hover:bg-gray-100'
                  }`}
                  onClick={(e) => {
                    if (isActive('/docs/architecture')) {
                      e.preventDefault();
                      toggleExpand('/docs/architecture');
                    }
                  }}
                >
                  <span>Architecture</span>
                  {pageStructure['/docs/architecture'] && (
                    <svg className={`w-4 h-4 transition-transform ${expandedPages['/docs/architecture'] ? 'rotate-90' : ''}`} fill="none" stroke="currentColor" viewBox="0 0 24 24">
                      <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M9 5l7 7-7 7" />
                    </svg>
                  )}
                </Link>
                {expandedPages['/docs/architecture'] && pageStructure['/docs/architecture'] && (
                  <div className="ml-4 mt-1 space-y-1">
                    {pageStructure['/docs/architecture'].map(section => (
                      <button
                        key={section.id}
                        onClick={() => scrollToSection(section.id)}
                        className="block w-full text-left px-3 py-1.5 text-xs text-gray-600 hover:text-[var(--primary-orange)] hover:bg-gray-50 rounded transition-colors cursor-pointer"
                      >
                        {section.title}
                      </button>
                    ))}
                  </div>
                )}
              </div>
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
