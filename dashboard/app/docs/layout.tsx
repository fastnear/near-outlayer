'use client';

import Link from 'next/link';
import { usePathname } from 'next/navigation';
import { useState, useEffect } from 'react';

// Define page structure with subsections
const pageStructure = {
  '/docs/getting-started': [
    { id: 'what-is-outlayer', title: 'Overview' },    
    { id: 'two-modes', title: 'Integration Modes' },
    { id: 'quick-start', title: 'Quick Start' },
    { id: 'secrets', title: 'Features' },
  ],
  '/docs/near-integration': [
    { id: 'request-execution', title: 'Method: request_execution' },
    { id: 'callback', title: 'Callback Response' },
    { id: 'example', title: 'Example Contract Call' },
    { id: 'pricing-payment', title: 'Pricing & Payment' },
    { id: 'performance', title: 'Performance Tips' },
    { id: 'fastfs-workflow', title: 'FastFS Workflow' },
    { id: 'tee-attestation', title: 'TEE Attestation' },
  ],
  '/docs/web2-integration': [
    { id: 'why-web2', title: 'Why Web2 Integration?' },
    { id: 'quick-start', title: 'Quick Start' },
    { id: 'tee-attestation', title: 'TEE Attestation' },
    { id: 'payments', title: 'Payments & Monetization' },
    { id: 'env-vars', title: 'Environment Variables' },
    { id: 'api-reference', title: 'API Reference' },
    { id: 'code-examples', title: 'Code Examples' },
    { id: 'capabilities', title: 'Project Capabilities' },
  ],
  '/docs/https-api': [
    { id: 'overview', title: 'Overview' },
    { id: 'request-format', title: 'Request Format' },
    { id: 'headers', title: 'Request Headers' },
    { id: 'body', title: 'Request Body' },
    { id: 'response-format', title: 'Response Format' },
    { id: 'env-vars', title: 'Environment Variables' },
    { id: 'errors', title: 'Error Codes' },
    { id: 'examples', title: 'Code Examples' },
  ],
  '/docs/payment-keys': [
    { id: 'what-are-payment-keys', title: 'What are Payment Keys?' },
    { id: 'key-format', title: 'Key Format' },
    { id: 'creating-keys', title: 'Creating Payment Keys' },
    { id: 'restrictions', title: 'Key Restrictions' },
    { id: 'balance', title: 'Balance Management' },
    { id: 'rate-limits', title: 'Rate Limits' },
    { id: 'security', title: 'Security Best Practices' },
  ],
  '/docs/earnings': [
    { id: 'how-it-works', title: 'How Earnings Work' },
    { id: 'checking-payment', title: 'Checking Payment in WASM' },
    { id: 'viewing-earnings', title: 'Viewing Your Earnings' },
    { id: 'withdrawing', title: 'Withdrawing Earnings' },
    { id: 'pricing-strategies', title: 'Pricing Strategies' },
    { id: 'best-practices', title: 'Best Practices' },
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
    { id: 'host-functions', title: 'Host Functions (Advanced)' },
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
    { id: 'secrets-binding', title: 'Secrets Binding Types' },
    { id: 'project-binding', title: 'Project Binding' },
    { id: 'access-control', title: 'Access Control' },
    { id: 'using-secrets', title: 'Using Secrets in Code' },
    { id: 'storage-costs', title: 'Storage Costs' },
    { id: 'security-model', title: 'Security Model' },
    { id: 'confidential-key-derivation', title: 'Confidential Key Derivation (CKD)' },
    { id: 'dao-governance', title: 'DAO Governance & Keystore' },
    { id: 'ckd-faq', title: 'CKD & MPC FAQ' },
  ],
  '/docs/projects': [
    { id: 'what-are-projects', title: 'What are Projects?' },
    { id: 'project-id', title: 'Project ID Format' },
    { id: 'creating-project', title: 'Creating a Project' },
    { id: 'wasm-metadata', title: 'How Project Binding Works' },
    { id: 'project-env-vars', title: 'Project Environment Variables' },
    { id: 'storage-security', title: 'Storage Security Model' },
    { id: 'managing-versions', title: 'Managing Versions' },
    { id: 'persistent-storage', title: 'Persistent Storage' },
    { id: 'project-secrets', title: 'Project Secrets' },
    { id: 'use-cases', title: 'Use Cases' },
    { id: 'best-practices', title: 'Best Practices' },
  ],
  '/docs/storage': [
    { id: 'overview', title: 'Overview' },
    { id: 'quick-start', title: 'Quick Start' },
    { id: 'api', title: 'Storage API' },
    { id: 'methods', title: 'Methods Reference' },
    { id: 'atomic-operations', title: 'Atomic Operations' },
    { id: 'user-isolation', title: 'User Data Isolation' },
    { id: 'worker-storage', title: 'Worker Storage' },
    { id: 'public-storage', title: 'Public Storage' },
    { id: 'public-http-api', title: 'External HTTP API' },
    { id: 'security', title: 'Security' },
    { id: 'use-cases', title: 'Use Cases' },
    { id: 'best-practices', title: 'Best Practices' },
  ],
  '/docs/sdk': [
    { id: 'installation', title: 'Installation' },
    { id: 'when-to-use', title: 'When Do You Need the SDK?' },
    { id: 'env-module', title: 'Environment Module' },
    { id: 'storage-module', title: 'Storage Module' },
    { id: 'examples', title: 'Examples Using the SDK' },
    { id: 'minimal-project', title: 'Minimal Project Template' },
    { id: 'api-reference', title: 'API Reference' },
    { id: 'storage-types', title: 'Storage Types Overview' },
  ],
  '/docs/pricing': [
    { id: 'dynamic-pricing', title: 'Dynamic Pricing Model' },
    { id: 'cost-calculation', title: 'Cost Calculation' },
    { id: 'resource-limits', title: 'Resource Limits' },
    { id: 'refund-policy', title: 'Refund Policy' },
    { id: 'optimization-tips', title: 'Optimization Tips' },
  ],
  '/docs/examples': [
    { id: 'random-ark', title: 'Random Number' },
    { id: 'echo-ark', title: 'Echo' },
    { id: 'ai-ark', title: 'AI Integration' },
    { id: 'weather-ark', title: 'Weather Oracle' },
    { id: 'oracle-ark', title: 'Price Oracle' },
    { id: 'ethereum-api', title: 'Ethereum API' },
    { id: 'botfather-ark', title: 'Bot Father' },
    { id: 'intents-ark', title: 'NEAR Intents swap' },
    { id: 'private-dao-ark', title: 'Private DAO Voting' },
    { id: 'captcha-ark', title: '2FA Verification' },
    { id: 'near-email', title: 'NEAR Email' },
  ],
  '/docs/tee-attestation': [
    { id: 'what-is-tee', title: 'What is TEE Attestation?' },
    { id: 'worker-registration', title: 'Worker Registration' },
    { id: 'execution-attestation', title: 'Execution Attestation' },
    { id: 'verification-process', title: 'Verification Process' },
    { id: 'security-guarantees', title: 'Security Guarantees' },
    { id: 'dashboard-verification', title: 'Dashboard Verification' },
  ],
  '/docs/trust-verification': [
    { id: 'overview', title: 'Trust Architecture' },
    { id: 'phala-trust-center', title: 'Phala Trust Center' },
    { id: 'sigstore', title: 'GitHub Releases & Sigstore' },
    { id: 'measurements', title: '5-Measurement TDX Verification' },
    { id: 'registration-flow', title: 'Worker Registration Flow' },
    { id: 'ephemeral-keys', title: 'Ephemeral Keys & Blockchain Trail' },
    { id: 'ckd', title: 'Deterministic Keystore Secrets (CKD)' },
    { id: 'operator-limits', title: 'What Operator Cannot Do' },
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

  // Auto-expand only current page when pathname changes
  useEffect(() => {
    if (pageStructure[pathname as keyof typeof pageStructure]) {
      setExpandedPages({ [pathname]: true }); // Only current page
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
      // Update URL with hash
      window.history.pushState(null, '', `#${sectionId}`);
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
          <div className="bg-white rounded-lg shadow-sm p-2">            
            <nav className="space-y-0.5">
              {/* Getting Started */}
              <div>
                <Link
                  href="/docs/getting-started"
                  className={`flex items-center justify-between w-full text-left px-3 py-1.5 rounded-md text-sm font-medium transition-colors ${
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
                  <div className="ml-4 mt-1 space-y-0.5">
                    {pageStructure['/docs/getting-started'].map(section => (
                      <button
                        key={section.id}
                        onClick={() => scrollToSection(section.id)}
                        className="block w-full text-left px-3 py-1 text-xs text-gray-600 hover:text-[var(--primary-orange)] hover:bg-gray-50 rounded transition-colors cursor-pointer"
                      >
                        {section.title}
                      </button>
                    ))}
                  </div>
                )}
              </div>

              {/* Example Projects */}
              <div>
                <Link
                  href="/docs/examples"
                  className={`flex items-center justify-between w-full text-left px-3 py-1.5 rounded-md text-sm font-medium transition-colors ${
                    isActive('/docs/examples')
                      ? 'bg-[var(--primary-orange)] text-white'
                      : 'text-gray-700 hover:bg-gray-100'
                  }`}
                  onClick={(e) => {
                    if (isActive('/docs/examples')) {
                      e.preventDefault();
                      toggleExpand('/docs/examples');
                    }
                  }}
                >
                  <span>Example Projects</span>
                  {pageStructure['/docs/examples'] && (
                    <svg className={`w-4 h-4 transition-transform ${expandedPages['/docs/examples'] ? 'rotate-90' : ''}`} fill="none" stroke="currentColor" viewBox="0 0 24 24">
                      <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M9 5l7 7-7 7" />
                    </svg>
                  )}
                </Link>
                {expandedPages['/docs/examples'] && pageStructure['/docs/examples'] && (
                  <div className="ml-4 mt-1 space-y-0.5">
                    {pageStructure['/docs/examples'].map(section => (
                      <button
                        key={section.id}
                        onClick={() => scrollToSection(section.id)}
                        className="block w-full text-left px-3 py-1 text-xs text-gray-600 hover:text-[var(--primary-orange)] hover:bg-gray-50 rounded transition-colors cursor-pointer"
                      >
                        {section.title}
                      </button>
                    ))}
                  </div>
                )}
              </div>

              {/* Guides Section */}
              <div className="pt-3 pb-1">
                <span className="px-3 text-xs font-semibold text-gray-500 uppercase tracking-wider">Guides</span>
              </div>

              {/* NEAR Integration */}
              <div>
                <Link
                  href="/docs/near-integration"
                  className={`flex items-center justify-between w-full text-left px-3 py-1.5 rounded-md text-sm font-medium transition-colors ${
                    isActive('/docs/near-integration')
                      ? 'bg-[var(--primary-orange)] text-white'
                      : 'text-gray-700 hover:bg-gray-100'
                  }`}
                  onClick={(e) => {
                    if (isActive('/docs/near-integration')) {
                      e.preventDefault();
                      toggleExpand('/docs/near-integration');
                    }
                  }}
                >
                  <span>NEAR Integration</span>
                  {pageStructure['/docs/near-integration'] && (
                    <svg className={`w-4 h-4 transition-transform ${expandedPages['/docs/near-integration'] ? 'rotate-90' : ''}`} fill="none" stroke="currentColor" viewBox="0 0 24 24">
                      <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M9 5l7 7-7 7" />
                    </svg>
                  )}
                </Link>
                {expandedPages['/docs/near-integration'] && pageStructure['/docs/near-integration'] && (
                  <div className="ml-4 mt-1 space-y-0.5">
                    {pageStructure['/docs/near-integration'].map(section => (
                      <button
                        key={section.id}
                        onClick={() => scrollToSection(section.id)}
                        className="block w-full text-left px-3 py-1 text-xs text-gray-600 hover:text-[var(--primary-orange)] hover:bg-gray-50 rounded transition-colors cursor-pointer"
                      >
                        {section.title}
                      </button>
                    ))}
                  </div>
                )}
              </div>

              {/* Web2 Integration */}
              <div>
                <Link
                  href="/docs/web2-integration"
                  className={`flex items-center justify-between w-full text-left px-3 py-1.5 rounded-md text-sm font-medium transition-colors ${
                    isActive('/docs/web2-integration')
                      ? 'bg-[var(--primary-orange)] text-white'
                      : 'text-gray-700 hover:bg-gray-100'
                  }`}
                  onClick={(e) => {
                    if (isActive('/docs/web2-integration')) {
                      e.preventDefault();
                      toggleExpand('/docs/web2-integration');
                    }
                  }}
                >
                  <span>Web2 Integration</span>
                  {pageStructure['/docs/web2-integration'] && (
                    <svg className={`w-4 h-4 transition-transform ${expandedPages['/docs/web2-integration'] ? 'rotate-90' : ''}`} fill="none" stroke="currentColor" viewBox="0 0 24 24">
                      <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M9 5l7 7-7 7" />
                    </svg>
                  )}
                </Link>
                {expandedPages['/docs/web2-integration'] && pageStructure['/docs/web2-integration'] && (
                  <div className="ml-4 mt-1 space-y-0.5">
                    {pageStructure['/docs/web2-integration'].map(section => (
                      <button
                        key={section.id}
                        onClick={() => scrollToSection(section.id)}
                        className="block w-full text-left px-3 py-1 text-xs text-gray-600 hover:text-[var(--primary-orange)] hover:bg-gray-50 rounded transition-colors cursor-pointer"
                      >
                        {section.title}
                      </button>
                    ))}
                  </div>
                )}
              </div>

              {/* Building OutLayer App */}
              <div>
                <Link
                  href="/docs/wasi"
                  className={`flex items-center justify-between w-full text-left px-3 py-1.5 rounded-md text-sm font-medium transition-colors ${
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
                  <span>Building OutLayer App</span>
                  {pageStructure['/docs/wasi'] && (
                    <svg className={`w-4 h-4 transition-transform ${expandedPages['/docs/wasi'] ? 'rotate-90' : ''}`} fill="none" stroke="currentColor" viewBox="0 0 24 24">
                      <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M9 5l7 7-7 7" />
                    </svg>
                  )}
                </Link>
                {expandedPages['/docs/wasi'] && pageStructure['/docs/wasi'] && (
                  <div className="ml-4 mt-1 space-y-0.5">
                    {pageStructure['/docs/wasi'].map(section => (
                      <button
                        key={section.id}
                        onClick={() => scrollToSection(section.id)}
                        className="block w-full text-left px-3 py-1 text-xs text-gray-600 hover:text-[var(--primary-orange)] hover:bg-gray-50 rounded transition-colors cursor-pointer"
                      >
                        {section.title}
                      </button>
                    ))}
                  </div>
                )}
              </div>

              {/* Tutorial: First App */}
              <div>
                <Link
                  href="/docs/dev-guide"
                  className={`flex items-center justify-between w-full text-left px-3 py-1.5 rounded-md text-sm font-medium transition-colors ${
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
                  <span>Tutorial: First App</span>
                  {pageStructure['/docs/dev-guide'] && (
                    <svg className={`w-4 h-4 transition-transform ${expandedPages['/docs/dev-guide'] ? 'rotate-90' : ''}`} fill="none" stroke="currentColor" viewBox="0 0 24 24">
                      <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M9 5l7 7-7 7" />
                    </svg>
                  )}
                </Link>
                {expandedPages['/docs/dev-guide'] && pageStructure['/docs/dev-guide'] && (
                  <div className="ml-4 mt-1 space-y-0.5">
                    {pageStructure['/docs/dev-guide'].map(section => (
                      <button
                        key={section.id}
                        onClick={() => scrollToSection(section.id)}
                        className="block w-full text-left px-3 py-1 text-xs text-gray-600 hover:text-[var(--primary-orange)] hover:bg-gray-50 rounded transition-colors cursor-pointer"
                      >
                        {section.title}
                      </button>
                    ))}
                  </div>
                )}
              </div>

              {/* OutLayer Features Section */}
              <div className="pt-3 pb-1">
                <span className="px-3 text-xs font-semibold text-gray-500 uppercase tracking-wider">OutLayer Features</span>
              </div>

              {/* Secrets */}
              <div>
                <Link
                  href="/docs/secrets"
                  className={`flex items-center justify-between w-full text-left px-3 py-1.5 rounded-md text-sm font-medium transition-colors ${
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
                  <span>Secrets</span>
                  {pageStructure['/docs/secrets'] && (
                    <svg className={`w-4 h-4 transition-transform ${expandedPages['/docs/secrets'] ? 'rotate-90' : ''}`} fill="none" stroke="currentColor" viewBox="0 0 24 24">
                      <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M9 5l7 7-7 7" />
                    </svg>
                  )}
                </Link>
                {expandedPages['/docs/secrets'] && pageStructure['/docs/secrets'] && (
                  <div className="ml-4 mt-1 space-y-0.5">
                    {pageStructure['/docs/secrets'].map(section => (
                      <button
                        key={section.id}
                        onClick={() => scrollToSection(section.id)}
                        className="block w-full text-left px-3 py-1 text-xs text-gray-600 hover:text-[var(--primary-orange)] hover:bg-gray-50 rounded transition-colors cursor-pointer"
                      >
                        {section.title}
                      </button>
                    ))}
                  </div>
                )}
              </div>

              {/* Projects */}
              <div>
                <Link
                  href="/docs/projects"
                  className={`flex items-center justify-between w-full text-left px-3 py-1.5 rounded-md text-sm font-medium transition-colors ${
                    isActive('/docs/projects')
                      ? 'bg-[var(--primary-orange)] text-white'
                      : 'text-gray-700 hover:bg-gray-100'
                  }`}
                  onClick={(e) => {
                    if (isActive('/docs/projects')) {
                      e.preventDefault();
                      toggleExpand('/docs/projects');
                    }
                  }}
                >
                  <span>Projects</span>
                  {pageStructure['/docs/projects'] && (
                    <svg className={`w-4 h-4 transition-transform ${expandedPages['/docs/projects'] ? 'rotate-90' : ''}`} fill="none" stroke="currentColor" viewBox="0 0 24 24">
                      <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M9 5l7 7-7 7" />
                    </svg>
                  )}
                </Link>
                {expandedPages['/docs/projects'] && pageStructure['/docs/projects'] && (
                  <div className="ml-4 mt-1 space-y-0.5">
                    {pageStructure['/docs/projects'].map(section => (
                      <button
                        key={section.id}
                        onClick={() => scrollToSection(section.id)}
                        className="block w-full text-left px-3 py-1 text-xs text-gray-600 hover:text-[var(--primary-orange)] hover:bg-gray-50 rounded transition-colors cursor-pointer"
                      >
                        {section.title}
                      </button>
                    ))}
                  </div>
                )}
              </div>

              {/* Storage */}
              <div>
                <Link
                  href="/docs/storage"
                  className={`flex items-center justify-between w-full text-left px-3 py-1.5 rounded-md text-sm font-medium transition-colors ${
                    isActive('/docs/storage')
                      ? 'bg-[var(--primary-orange)] text-white'
                      : 'text-gray-700 hover:bg-gray-100'
                  }`}
                  onClick={(e) => {
                    if (isActive('/docs/storage')) {
                      e.preventDefault();
                      toggleExpand('/docs/storage');
                    }
                  }}
                >
                  <span>Storage</span>
                  {pageStructure['/docs/storage'] && (
                    <svg className={`w-4 h-4 transition-transform ${expandedPages['/docs/storage'] ? 'rotate-90' : ''}`} fill="none" stroke="currentColor" viewBox="0 0 24 24">
                      <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M9 5l7 7-7 7" />
                    </svg>
                  )}
                </Link>
                {expandedPages['/docs/storage'] && pageStructure['/docs/storage'] && (
                  <div className="ml-4 mt-1 space-y-0.5">
                    {pageStructure['/docs/storage'].map(section => (
                      <button
                        key={section.id}
                        onClick={() => scrollToSection(section.id)}
                        className="block w-full text-left px-3 py-1 text-xs text-gray-600 hover:text-[var(--primary-orange)] hover:bg-gray-50 rounded transition-colors cursor-pointer"
                      >
                        {section.title}
                      </button>
                    ))}
                  </div>
                )}
              </div>

              {/* SDK */}
              <div>
                <Link
                  href="/docs/sdk"
                  className={`flex items-center justify-between w-full text-left px-3 py-1.5 rounded-md text-sm font-medium transition-colors ${
                    isActive('/docs/sdk')
                      ? 'bg-[var(--primary-orange)] text-white'
                      : 'text-gray-700 hover:bg-gray-100'
                  }`}
                  onClick={(e) => {
                    if (isActive('/docs/sdk')) {
                      e.preventDefault();
                      toggleExpand('/docs/sdk');
                    }
                  }}
                >
                  <span>SDK</span>
                  {pageStructure['/docs/sdk'] && (
                    <svg className={`w-4 h-4 transition-transform ${expandedPages['/docs/sdk'] ? 'rotate-90' : ''}`} fill="none" stroke="currentColor" viewBox="0 0 24 24">
                      <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M9 5l7 7-7 7" />
                    </svg>
                  )}
                </Link>
                {expandedPages['/docs/sdk'] && pageStructure['/docs/sdk'] && (
                  <div className="ml-4 mt-1 space-y-0.5">
                    {pageStructure['/docs/sdk'].map(section => (
                      <button
                        key={section.id}
                        onClick={() => scrollToSection(section.id)}
                        className="block w-full text-left px-3 py-1 text-xs text-gray-600 hover:text-[var(--primary-orange)] hover:bg-gray-50 rounded transition-colors cursor-pointer"
                      >
                        {section.title}
                      </button>
                    ))}
                  </div>
                )}
              </div>

              {/* HTTPS API */}
              <div>
                <Link
                  href="/docs/https-api"
                  className={`flex items-center justify-between w-full text-left px-3 py-1.5 rounded-md text-sm font-medium transition-colors ${
                    isActive('/docs/https-api')
                      ? 'bg-[var(--primary-orange)] text-white'
                      : 'text-gray-700 hover:bg-gray-100'
                  }`}
                  onClick={(e) => {
                    if (isActive('/docs/https-api')) {
                      e.preventDefault();
                      toggleExpand('/docs/https-api');
                    }
                  }}
                >
                  <span>HTTPS API</span>
                  {pageStructure['/docs/https-api'] && (
                    <svg className={`w-4 h-4 transition-transform ${expandedPages['/docs/https-api'] ? 'rotate-90' : ''}`} fill="none" stroke="currentColor" viewBox="0 0 24 24">
                      <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M9 5l7 7-7 7" />
                    </svg>
                  )}
                </Link>
                {expandedPages['/docs/https-api'] && pageStructure['/docs/https-api'] && (
                  <div className="ml-4 mt-1 space-y-0.5">
                    {pageStructure['/docs/https-api'].map(section => (
                      <button
                        key={section.id}
                        onClick={() => scrollToSection(section.id)}
                        className="block w-full text-left px-3 py-1 text-xs text-gray-600 hover:text-[var(--primary-orange)] hover:bg-gray-50 rounded transition-colors cursor-pointer"
                      >
                        {section.title}
                      </button>
                    ))}
                  </div>
                )}
              </div>

              {/* Payment Keys */}
              <div>
                <Link
                  href="/docs/payment-keys"
                  className={`flex items-center justify-between w-full text-left px-3 py-1.5 rounded-md text-sm font-medium transition-colors ${
                    isActive('/docs/payment-keys')
                      ? 'bg-[var(--primary-orange)] text-white'
                      : 'text-gray-700 hover:bg-gray-100'
                  }`}
                  onClick={(e) => {
                    if (isActive('/docs/payment-keys')) {
                      e.preventDefault();
                      toggleExpand('/docs/payment-keys');
                    }
                  }}
                >
                  <span>Payment Keys</span>
                  {pageStructure['/docs/payment-keys'] && (
                    <svg className={`w-4 h-4 transition-transform ${expandedPages['/docs/payment-keys'] ? 'rotate-90' : ''}`} fill="none" stroke="currentColor" viewBox="0 0 24 24">
                      <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M9 5l7 7-7 7" />
                    </svg>
                  )}
                </Link>
                {expandedPages['/docs/payment-keys'] && pageStructure['/docs/payment-keys'] && (
                  <div className="ml-4 mt-1 space-y-0.5">
                    {pageStructure['/docs/payment-keys'].map(section => (
                      <button
                        key={section.id}
                        onClick={() => scrollToSection(section.id)}
                        className="block w-full text-left px-3 py-1 text-xs text-gray-600 hover:text-[var(--primary-orange)] hover:bg-gray-50 rounded transition-colors cursor-pointer"
                      >
                        {section.title}
                      </button>
                    ))}
                  </div>
                )}
              </div>

              {/* Earnings */}
              <div>
                <Link
                  href="/docs/earnings"
                  className={`flex items-center justify-between w-full text-left px-3 py-1.5 rounded-md text-sm font-medium transition-colors ${
                    isActive('/docs/earnings')
                      ? 'bg-[var(--primary-orange)] text-white'
                      : 'text-gray-700 hover:bg-gray-100'
                  }`}
                  onClick={(e) => {
                    if (isActive('/docs/earnings')) {
                      e.preventDefault();
                      toggleExpand('/docs/earnings');
                    }
                  }}
                >
                  <span>Earnings</span>
                  {pageStructure['/docs/earnings'] && (
                    <svg className={`w-4 h-4 transition-transform ${expandedPages['/docs/earnings'] ? 'rotate-90' : ''}`} fill="none" stroke="currentColor" viewBox="0 0 24 24">
                      <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M9 5l7 7-7 7" />
                    </svg>
                  )}
                </Link>
                {expandedPages['/docs/earnings'] && pageStructure['/docs/earnings'] && (
                  <div className="ml-4 mt-1 space-y-0.5">
                    {pageStructure['/docs/earnings'].map(section => (
                      <button
                        key={section.id}
                        onClick={() => scrollToSection(section.id)}
                        className="block w-full text-left px-3 py-1 text-xs text-gray-600 hover:text-[var(--primary-orange)] hover:bg-gray-50 rounded transition-colors cursor-pointer"
                      >
                        {section.title}
                      </button>
                    ))}
                  </div>
                )}
              </div>

              {/* TEE Attestation */}
              <div>
                <Link
                  href="/docs/tee-attestation"
                  className={`flex items-center justify-between w-full text-left px-3 py-1.5 rounded-md text-sm font-medium transition-colors ${
                    isActive('/docs/tee-attestation')
                      ? 'bg-[var(--primary-orange)] text-white'
                      : 'text-gray-700 hover:bg-gray-100'
                  }`}
                  onClick={(e) => {
                    if (isActive('/docs/tee-attestation')) {
                      e.preventDefault();
                      toggleExpand('/docs/tee-attestation');
                    }
                  }}
                >
                  <span>TEE Attestation</span>
                  {pageStructure['/docs/tee-attestation'] && (
                    <svg className={`w-4 h-4 transition-transform ${expandedPages['/docs/tee-attestation'] ? 'rotate-90' : ''}`} fill="none" stroke="currentColor" viewBox="0 0 24 24">
                      <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M9 5l7 7-7 7" />
                    </svg>
                  )}
                </Link>
                {expandedPages['/docs/tee-attestation'] && pageStructure['/docs/tee-attestation'] && (
                  <div className="ml-4 mt-1 space-y-0.5">
                    {pageStructure['/docs/tee-attestation'].map(section => (
                      <button
                        key={section.id}
                        onClick={() => scrollToSection(section.id)}
                        className="block w-full text-left px-3 py-1 text-xs text-gray-600 hover:text-[var(--primary-orange)] hover:bg-gray-50 rounded transition-colors cursor-pointer"
                      >
                        {section.title}
                      </button>
                    ))}
                  </div>
                )}
              </div>

              {/* Trust & Verification */}
              <div>
                <Link
                  href="/docs/trust-verification"
                  className={`flex items-center justify-between w-full text-left px-3 py-1.5 rounded-md text-sm font-medium transition-colors ${
                    isActive('/docs/trust-verification')
                      ? 'bg-[var(--primary-orange)] text-white'
                      : 'text-gray-700 hover:bg-gray-100'
                  }`}
                  onClick={(e) => {
                    if (isActive('/docs/trust-verification')) {
                      e.preventDefault();
                      toggleExpand('/docs/trust-verification');
                    }
                  }}
                >
                  <span>Trust & Verification</span>
                  {pageStructure['/docs/trust-verification'] && (
                    <svg className={`w-4 h-4 transition-transform ${expandedPages['/docs/trust-verification'] ? 'rotate-90' : ''}`} fill="none" stroke="currentColor" viewBox="0 0 24 24">
                      <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M9 5l7 7-7 7" />
                    </svg>
                  )}
                </Link>
                {expandedPages['/docs/trust-verification'] && pageStructure['/docs/trust-verification'] && (
                  <div className="ml-4 mt-1 space-y-0.5">
                    {pageStructure['/docs/trust-verification'].map(section => (
                      <button
                        key={section.id}
                        onClick={() => scrollToSection(section.id)}
                        className="block w-full text-left px-3 py-1 text-xs text-gray-600 hover:text-[var(--primary-orange)] hover:bg-gray-50 rounded transition-colors cursor-pointer"
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
                  className={`flex items-center justify-between w-full text-left px-3 py-1.5 rounded-md text-sm font-medium transition-colors ${
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
                  <div className="ml-4 mt-1 space-y-0.5">
                    {pageStructure['/docs/pricing'].map(section => (
                      <button
                        key={section.id}
                        onClick={() => scrollToSection(section.id)}
                        className="block w-full text-left px-3 py-1 text-xs text-gray-600 hover:text-[var(--primary-orange)] hover:bg-gray-50 rounded transition-colors cursor-pointer"
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
