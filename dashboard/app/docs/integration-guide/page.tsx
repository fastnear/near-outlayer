'use client';

import Link from 'next/link';
import { Prism as SyntaxHighlighter } from 'react-syntax-highlighter';
import { vscDarkPlus } from 'react-syntax-highlighter/dist/esm/styles/prism';

function AnchorHeading({ id, children, level = 2 }: { id: string; children: React.ReactNode; level?: 2 | 3 | 4 }) {
  const sizeClass = level === 2 ? 'text-2xl' : level === 3 ? 'text-xl' : 'text-lg';
  const className = `${sizeClass} font-bold text-gray-900 mb-4 scroll-mt-4 group`;
  const anchor = (
    <a href={`#${id}`} className="ml-2 text-gray-400 hover:text-[var(--primary-orange)] opacity-0 group-hover:opacity-100 transition-opacity">
      #
    </a>
  );

  if (level === 3) return <h3 id={id} className={className}>{children}{anchor}</h3>;
  if (level === 4) return <h4 id={id} className={className}>{children}{anchor}</h4>;
  return <h2 id={id} className={className}>{children}{anchor}</h2>;
}

export default function IntegrationGuidePage() {
  return (
    <div className="prose prose-lg max-w-none">
      <h1 className="text-3xl font-bold text-gray-900 mb-6">
        Project Integration Guide
      </h1>

      <p className="text-gray-700 mb-8 text-lg">
        This guide provides a high-level overview of deploying a project on OutLayer. For detailed
        documentation on each topic, follow the links to dedicated pages.
      </p>

      {/* Overview */}
      <section className="mb-12">
        <AnchorHeading id="overview">Overview</AnchorHeading>

        <p className="text-gray-700 mb-4">
          An OutLayer <strong>Project</strong> is a container for your WASM code with version management,
          persistent storage, secrets, and monetization capabilities. Projects can be accessed via
          NEAR transactions or HTTPS API.
        </p>

        <div className="bg-blue-50 border-l-4 border-blue-400 p-4 mb-6">
          <p className="text-sm text-blue-800">
            <strong>Key concept:</strong> Your WASM code works identically regardless of how it&apos;s called.
            The only difference is how input arrives and how payment is handled.
          </p>
        </div>
      </section>

      {/* Project Capabilities */}
      <section className="mb-12">
        <AnchorHeading id="capabilities">Project Capabilities</AnchorHeading>

        <div className="grid grid-cols-1 md:grid-cols-2 gap-4 mb-6">
          <Link href="/docs/projects" className="block border rounded-lg p-4 hover:border-[var(--primary-orange)] transition-colors">
            <h4 className="font-semibold text-gray-900 mb-2">Projects & Storage</h4>
            <p className="text-sm text-gray-600">
              Version management, persistent encrypted storage, project IDs
            </p>
          </Link>

          <Link href="/docs/secrets" className="block border rounded-lg p-4 hover:border-[var(--primary-orange)] transition-colors">
            <h4 className="font-semibold text-gray-900 mb-2">Secrets</h4>
            <p className="text-sm text-gray-600">
              Encrypted API keys and credentials, access control, CKD
            </p>
          </Link>

          <Link href="/docs/payment-keys" className="block border rounded-lg p-4 hover:border-[var(--primary-orange)] transition-colors">
            <h4 className="font-semibold text-gray-900 mb-2">Payment Keys</h4>
            <p className="text-sm text-gray-600">
              Prepaid USD keys for HTTPS API access
            </p>
          </Link>

          <Link href="/docs/earnings" className="block border rounded-lg p-4 hover:border-[var(--primary-orange)] transition-colors">
            <h4 className="font-semibold text-gray-900 mb-2">Earnings</h4>
            <p className="text-sm text-gray-600">
              Monetize your project, receive USD payments from users
            </p>
          </Link>

          <Link href="/docs/https-api" className="block border rounded-lg p-4 hover:border-[var(--primary-orange)] transition-colors">
            <h4 className="font-semibold text-gray-900 mb-2">HTTPS API</h4>
            <p className="text-sm text-gray-600">
              Call projects via HTTP requests without blockchain transactions
            </p>
          </Link>

          <Link href="/docs/tee-attestation" className="block border rounded-lg p-4 hover:border-[var(--primary-orange)] transition-colors">
            <h4 className="font-semibold text-gray-900 mb-2">TEE Attestation</h4>
            <p className="text-sm text-gray-600">
              Cryptographic proof of correct execution
            </p>
          </Link>
        </div>
      </section>

      {/* Two Ways to Access */}
      <section className="mb-12">
        <AnchorHeading id="two-ways">Two Ways to Access Projects</AnchorHeading>

        <div className="grid grid-cols-1 md:grid-cols-2 gap-6 mb-6">
          <div className="border border-blue-200 rounded-lg p-5 bg-blue-50">
            <h4 className="font-semibold text-blue-900 mb-3">NEAR Transactions</h4>
            <ul className="text-sm text-gray-700 space-y-2">
              <li>Smart contracts call <code>request_execution()</code></li>
              <li>Results via callback to your contract</li>
              <li>Payment in NEAR tokens</li>
              <li>~2-3 second latency (blockchain finalization)</li>
            </ul>
            <div className="mt-4">
              <Link href="/docs/contract-integration" className="text-[var(--primary-orange)] text-sm hover:underline">
                Contract Integration docs &rarr;
              </Link>
            </div>
          </div>

          <div className="border border-green-200 rounded-lg p-5 bg-green-50">
            <h4 className="font-semibold text-green-900 mb-3">HTTPS API</h4>
            <ul className="text-sm text-gray-700 space-y-2">
              <li>HTTP POST to <code>/call/owner/project</code></li>
              <li>Results in HTTP response body</li>
              <li>Payment in USD stablecoins via Payment Keys</li>
              <li>Instant response (sync mode)</li>
            </ul>
            <div className="mt-4">
              <Link href="/docs/https-api" className="text-[var(--primary-orange)] text-sm hover:underline">
                HTTPS API docs &rarr;
              </Link>
            </div>
          </div>
        </div>

        <SyntaxHighlighter language="bash" style={vscDarkPlus} className="rounded-lg mb-4">
          {`# HTTPS API example
curl -X POST https://api.outlayer.io/call/alice.near/weather-api \\
  -H "X-Payment-Key: bob.near:0:SECRET_KEY" \\
  -H "X-Attached-Deposit: 100000" \\
  -d '{"input": {"city": "Tokyo"}}'`}
        </SyntaxHighlighter>
      </section>

      {/* Quick Setup Steps */}
      <section className="mb-12">
        <AnchorHeading id="quick-setup">Quick Setup Steps</AnchorHeading>

        <div className="space-y-4">
          <div className="flex items-start space-x-4 p-4 border rounded-lg">
            <div className="flex-shrink-0 w-8 h-8 bg-[var(--primary-orange)] text-white rounded-full flex items-center justify-center font-bold">1</div>
            <div>
              <h4 className="font-semibold text-gray-900">Create Your Project</h4>
              <p className="text-sm text-gray-600 mt-1">
                Go to <Link href="/projects" className="text-[var(--primary-orange)] hover:underline">/projects</Link>,
                click &quot;New Project&quot;, select GitHub or WASM URL source.
              </p>
              <Link href="/docs/projects#creating-project" className="text-[var(--primary-orange)] text-xs hover:underline">
                Detailed guide &rarr;
              </Link>
            </div>
          </div>

          <div className="flex items-start space-x-4 p-4 border rounded-lg">
            <div className="flex-shrink-0 w-8 h-8 bg-[var(--primary-orange)] text-white rounded-full flex items-center justify-center font-bold">2</div>
            <div>
              <h4 className="font-semibold text-gray-900">Add Secrets (if needed)</h4>
              <p className="text-sm text-gray-600 mt-1">
                Store API keys and credentials via <Link href="/secrets" className="text-[var(--primary-orange)] hover:underline">/secrets</Link>.
                Access them as environment variables in your WASM code.
              </p>
              <Link href="/docs/secrets" className="text-[var(--primary-orange)] text-xs hover:underline">
                Secrets documentation &rarr;
              </Link>
            </div>
          </div>

          <div className="flex items-start space-x-4 p-4 border rounded-lg">
            <div className="flex-shrink-0 w-8 h-8 bg-[var(--primary-orange)] text-white rounded-full flex items-center justify-center font-bold">3</div>
            <div>
              <h4 className="font-semibold text-gray-900">Create Payment Key (for HTTPS)</h4>
              <p className="text-sm text-gray-600 mt-1">
                Go to <Link href="/payment-keys" className="text-[var(--primary-orange)] hover:underline">/payment-keys</Link>,
                create a key with USD balance to enable HTTPS API access.
              </p>
              <Link href="/docs/payment-keys" className="text-[var(--primary-orange)] text-xs hover:underline">
                Payment Keys documentation &rarr;
              </Link>
            </div>
          </div>

          <div className="flex items-start space-x-4 p-4 border rounded-lg">
            <div className="flex-shrink-0 w-8 h-8 bg-[var(--primary-orange)] text-white rounded-full flex items-center justify-center font-bold">4</div>
            <div>
              <h4 className="font-semibold text-gray-900">Call Your Project</h4>
              <p className="text-sm text-gray-600 mt-1">
                Use HTTPS API with your Payment Key, or integrate into a NEAR smart contract.
              </p>
              <div className="flex space-x-4 mt-1">
                <Link href="/docs/https-api" className="text-[var(--primary-orange)] text-xs hover:underline">
                  HTTPS API &rarr;
                </Link>
                <Link href="/docs/contract-integration" className="text-[var(--primary-orange)] text-xs hover:underline">
                  Contract Integration &rarr;
                </Link>
              </div>
            </div>
          </div>

          <div className="flex items-start space-x-4 p-4 border rounded-lg">
            <div className="flex-shrink-0 w-8 h-8 bg-[var(--primary-green)] text-white rounded-full flex items-center justify-center font-bold">$</div>
            <div>
              <h4 className="font-semibold text-gray-900">Monetize (optional)</h4>
              <p className="text-sm text-gray-600 mt-1">
                Earn USD when users call your project with <code>X-Attached-Deposit</code>.
                Check payment in your WASM via <code>USD_PAYMENT</code> env var.
              </p>
              <Link href="/docs/earnings" className="text-[var(--primary-orange)] text-xs hover:underline">
                Earnings documentation &rarr;
              </Link>
            </div>
          </div>
        </div>
      </section>

      {/* Environment Variables */}
      <section className="mb-12">
        <AnchorHeading id="env-vars">Key Environment Variables</AnchorHeading>

        <p className="text-gray-700 mb-4">
          Your WASM code receives context via environment variables. Key differences between execution modes:
        </p>

        <div className="overflow-x-auto mb-6">
          <table className="min-w-full divide-y divide-gray-200 text-sm">
            <thead className="bg-gray-50">
              <tr>
                <th className="px-3 py-2 text-left text-xs font-medium text-gray-500 uppercase">Variable</th>
                <th className="px-3 py-2 text-left text-xs font-medium text-gray-500 uppercase">NEAR</th>
                <th className="px-3 py-2 text-left text-xs font-medium text-gray-500 uppercase">HTTPS</th>
              </tr>
            </thead>
            <tbody className="bg-white divide-y divide-gray-200">
              <tr className="bg-blue-50">
                <td className="px-3 py-2 font-mono">OUTLAYER_EXECUTION_TYPE</td>
                <td className="px-3 py-2">&quot;NEAR&quot;</td>
                <td className="px-3 py-2">&quot;HTTPS&quot;</td>
              </tr>
              <tr>
                <td className="px-3 py-2 font-mono">NEAR_SENDER_ID</td>
                <td className="px-3 py-2">Transaction signer</td>
                <td className="px-3 py-2">Payment Key owner</td>
              </tr>
              <tr className="bg-yellow-50">
                <td className="px-3 py-2 font-mono">USD_PAYMENT</td>
                <td className="px-3 py-2">&quot;0&quot;</td>
                <td className="px-3 py-2">X-Attached-Deposit</td>
              </tr>
              <tr>
                <td className="px-3 py-2 font-mono">NEAR_PAYMENT_YOCTO</td>
                <td className="px-3 py-2">Attached NEAR</td>
                <td className="px-3 py-2">&quot;0&quot;</td>
              </tr>
            </tbody>
          </table>
        </div>

        <p className="text-gray-700 mb-4">
          Full environment variables reference:
        </p>
        <ul className="list-disc list-inside text-gray-700 space-y-1">
          <li><Link href="/docs/https-api#env-vars" className="text-[var(--primary-orange)] hover:underline">HTTPS API environment variables</Link></li>
          <li><Link href="/docs/wasi" className="text-[var(--primary-orange)] hover:underline">WASI development guide</Link></li>
        </ul>
      </section>

      {/* Documentation Links */}
      <section className="bg-gray-50 rounded-lg p-6">
        <h3 className="text-lg font-semibold text-gray-900 mb-4">Detailed Documentation</h3>

        <div className="grid grid-cols-1 md:grid-cols-2 gap-6">
          <div>
            <h4 className="font-semibold text-gray-900 mb-2">Core Concepts</h4>
            <ul className="space-y-1 text-sm">
              <li><Link href="/docs/projects" className="text-[var(--primary-orange)] hover:underline">Projects & Storage</Link></li>
              <li><Link href="/docs/secrets" className="text-[var(--primary-orange)] hover:underline">Secrets</Link></li>
              <li><Link href="/docs/wasi" className="text-[var(--primary-orange)] hover:underline">Writing WASI Code</Link></li>
              <li><Link href="/docs/pricing" className="text-[var(--primary-orange)] hover:underline">Pricing & Limits</Link></li>
            </ul>
          </div>

          <div>
            <h4 className="font-semibold text-gray-900 mb-2">Access Methods</h4>
            <ul className="space-y-1 text-sm">
              <li><Link href="/docs/https-api" className="text-[var(--primary-orange)] hover:underline">HTTPS API</Link> - Full API reference</li>
              <li><Link href="/docs/payment-keys" className="text-[var(--primary-orange)] hover:underline">Payment Keys</Link> - Authentication for HTTPS</li>
              <li><Link href="/docs/earnings" className="text-[var(--primary-orange)] hover:underline">Earnings</Link> - Monetization</li>
              <li><Link href="/docs/contract-integration" className="text-[var(--primary-orange)] hover:underline">Contract Integration</Link> - NEAR transactions</li>
            </ul>
          </div>

          <div>
            <h4 className="font-semibold text-gray-900 mb-2">Dashboard Pages</h4>
            <ul className="space-y-1 text-sm">
              <li><Link href="/projects" className="text-[var(--primary-orange)] hover:underline">/projects</Link> - Manage projects</li>
              <li><Link href="/secrets" className="text-[var(--primary-orange)] hover:underline">/secrets</Link> - Store secrets</li>
              <li><Link href="/payment-keys" className="text-[var(--primary-orange)] hover:underline">/payment-keys</Link> - Payment Keys</li>
              <li><Link href="/earnings" className="text-[var(--primary-orange)] hover:underline">/earnings</Link> - View earnings</li>
              <li><Link href="/executions" className="text-[var(--primary-orange)] hover:underline">/executions</Link> - Execution history</li>
            </ul>
          </div>

          <div>
            <h4 className="font-semibold text-gray-900 mb-2">Advanced</h4>
            <ul className="space-y-1 text-sm">
              <li><Link href="/docs/tee-attestation" className="text-[var(--primary-orange)] hover:underline">TEE Attestation</Link></li>
              <li><Link href="/docs/architecture" className="text-[var(--primary-orange)] hover:underline">Architecture</Link></li>
              <li><Link href="/docs/examples" className="text-[var(--primary-orange)] hover:underline">Example Projects</Link></li>
            </ul>
          </div>
        </div>
      </section>
    </div>
  );
}
