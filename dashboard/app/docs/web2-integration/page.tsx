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

export default function Web2IntegrationPage() {
  return (
    <div className="prose prose-lg max-w-none">
      <h1 className="text-3xl font-bold text-gray-900 mb-6">
        Web2 Integration
      </h1>

      <p className="text-gray-700 mb-6 text-lg">
        Call OutLayer from your <strong>web apps, mobile apps, APIs, and backend services</strong> via HTTPS.
        Every execution runs in a TEE and produces cryptographic attestation —
        you get verifiable proofs of exactly what code ran with what inputs.
      </p>

      <div className="bg-gradient-to-r from-purple-50 to-blue-50 border-l-4 border-purple-500 p-4 mb-6">
        <p className="text-sm text-purple-900">
          <strong>Blockchain-grade security, Web2 simplicity:</strong> No blockchain knowledge required.
          Just HTTP calls with stablecoin payments, but with full verifiability via Intel TDX attestation.
        </p>
      </div>

      <div className="bg-blue-50 border-l-4 border-blue-400 p-4 mb-8">
        <p className="text-sm text-blue-800">
          <strong>For NEAR smart contracts</strong> see{' '}
          <Link href="/docs/near-integration" className="underline font-medium">NEAR Integration</Link> —
          use yield/resume mechanism with NEAR token payments.
        </p>
      </div>

      {/* Why Web2 Integration */}
      <section className="mb-12">
        <AnchorHeading id="why-web2">Why Web2 Integration?</AnchorHeading>

        <div className="grid grid-cols-1 md:grid-cols-2 gap-4 mb-6">
          <div className="border-2 border-green-200 rounded-lg p-5 bg-green-50">
            <div className="text-2xl mb-2">⚡</div>
            <h4 className="font-semibold text-green-900 mb-2">Sub-Second Response</h4>
            <p className="text-sm text-gray-700">
              Direct HTTPS calls with instant response. No blockchain finalization delays.
              Execution time depends only on your WASM code complexity.
            </p>
          </div>

          <div className="border-2 border-blue-200 rounded-lg p-5 bg-blue-50">
            <div className="text-2xl mb-2">💵</div>
            <h4 className="font-semibold text-blue-900 mb-2">USDC Payments</h4>
            <p className="text-sm text-gray-700">
              Pay with USDC via prepaid Payment Keys. No gas fees per API call.
              One tx to create/top-up a key, one to withdraw balance.
            </p>
          </div>

          <div className="border-2 border-purple-200 rounded-lg p-5 bg-purple-50">
            <div className="text-2xl mb-2">🔐</div>
            <h4 className="font-semibold text-purple-900 mb-2">TEE Attestation</h4>
            <p className="text-sm text-gray-700">
              Every execution produces cryptographic proof (Intel TDX attestation).
              Verify that exact code ran with exact inputs — no trust required.
            </p>
          </div>

          <div className="border-2 border-orange-200 rounded-lg p-5 bg-orange-50">
            <div className="text-2xl mb-2">💰</div>
            <h4 className="font-semibold text-orange-900 mb-2">Monetize Your API</h4>
            <p className="text-sm text-gray-700">
              Earn USD when users call your project. Set your own prices,
              receive payments directly. No middlemen, no revenue share.
            </p>
          </div>
        </div>
      </section>

      {/* Quick Start */}
      <section className="mb-12">
        <AnchorHeading id="quick-start">Quick Start</AnchorHeading>

        <div className="space-y-4 mb-6">
          <div className="flex items-start space-x-4 p-4 border rounded-lg">
            <div className="flex-shrink-0 w-8 h-8 bg-[var(--primary-orange)] text-white rounded-full flex items-center justify-center font-bold">1</div>
            <div>
              <h4 className="font-semibold text-gray-900">Create Project</h4>
              <p className="text-sm text-gray-600 mt-1">
                Go to <Link href="/projects" className="text-[var(--primary-orange)] hover:underline">/projects</Link> →
                &quot;New Project&quot; → Enter GitHub URL or WASM file.
              </p>
              <Link href="/docs/projects#creating-project" className="text-[var(--primary-orange)] text-xs hover:underline">
                Detailed guide →
              </Link>
            </div>
          </div>

          <div className="flex items-start space-x-4 p-4 border rounded-lg">
            <div className="flex-shrink-0 w-8 h-8 bg-[var(--primary-orange)] text-white rounded-full flex items-center justify-center font-bold">2</div>
            <div>
              <h4 className="font-semibold text-gray-900">Create Payment Key</h4>
              <p className="text-sm text-gray-600 mt-1">
                Go to <Link href="/payment-keys" className="text-[var(--primary-orange)] hover:underline">/payment-keys</Link> →
                Create key with USD balance (e.g., $10).
              </p>
              <Link href="/docs/payment-keys" className="text-[var(--primary-orange)] text-xs hover:underline">
                Payment Keys documentation →
              </Link>
            </div>
          </div>

          <div className="flex items-start space-x-4 p-4 border rounded-lg">
            <div className="flex-shrink-0 w-8 h-8 bg-[var(--primary-orange)] text-white rounded-full flex items-center justify-center font-bold">3</div>
            <div>
              <h4 className="font-semibold text-gray-900">Call Your Project</h4>
              <p className="text-sm text-gray-600 mt-1">
                Make HTTP POST request with your Payment Key:
              </p>
            </div>
          </div>
        </div>

        <SyntaxHighlighter language="bash" style={vscDarkPlus} className="rounded-lg mb-4">
          {`curl -X POST https://api.outlayer.fastnear.com/call/alice.near/my-project \\
  -H "X-Payment-Key: alice.near:1:your_secret_key" \\
  -H "Content-Type: application/json" \\
  -d '{"city": "Tokyo"}'`}
        </SyntaxHighlighter>

        <SyntaxHighlighter language="json" style={vscDarkPlus} className="rounded-lg mb-4">
          {`{
  "status": "success",
  "output": "Weather in Tokyo: 22°C, Partly Cloudy",
  "compute_cost": "15000",
  "job_id": "abc123..."
}`}
        </SyntaxHighlighter>
      </section>

      {/* TEE Attestation */}
      <section className="mb-12">
        <AnchorHeading id="tee-attestation">TEE Attestation: Verifiable Execution</AnchorHeading>

        <p className="text-gray-700 mb-4">
          Every OutLayer execution produces a <strong>cryptographic attestation</strong> from Intel TDX (Trusted Execution Environment).
          This proves:
        </p>

        <ul className="list-disc list-inside text-gray-700 space-y-2 mb-6">
          <li><strong>Code integrity</strong> — Exact WASM binary that ran (SHA256 hash)</li>
          <li><strong>Input integrity</strong> — Exact input data received (SHA256 hash)</li>
          <li><strong>Output integrity</strong> — Result was produced by that code with that input</li>
          <li><strong>Worker identity</strong> — Registered TEE worker with verified measurements</li>
        </ul>

        <div className="bg-purple-50 border-l-4 border-purple-400 p-4 mb-6">
          <p className="text-sm text-purple-800">
            <strong>Why this matters:</strong> Your users can independently verify that your API actually ran
            the advertised code. No &quot;trust us&quot; — cryptographic proof. View attestations at{' '}
            <Link href="/executions" className="underline">/executions</Link> → &quot;View Attestation&quot;.
          </p>
        </div>

        <SyntaxHighlighter language="javascript" style={vscDarkPlus} className="rounded-lg mb-4">
          {`// Response includes job_id for attestation lookup
const result = await fetch('https://api.outlayer.fastnear.com/call/alice.near/my-api', {
  method: 'POST',
  headers: { 'X-Payment-Key': 'alice.near:1:secret' },
  body: JSON.stringify({ query: 'data' })
});

const { job_id, output } = await result.json();

// Users can verify attestation at:
// https://outlayer.fastnear.com/attestation/{job_id}
// Or via API: GET /attestation/{job_id}`}
        </SyntaxHighlighter>
      </section>

      {/* Payments */}
      <section className="mb-12">
        <AnchorHeading id="payments">Payments & Monetization</AnchorHeading>

        <div className="grid grid-cols-1 md:grid-cols-2 gap-6 mb-6">
          <div className="border rounded-lg p-5">
            <h4 className="font-semibold text-gray-900 mb-3">For API Consumers</h4>
            <ul className="text-sm text-gray-700 space-y-2">
              <li><strong>1.</strong> Create Payment Key with USD balance</li>
              <li><strong>2.</strong> Include <code className="bg-gray-100 px-1 rounded">X-Payment-Key</code> header in requests</li>
              <li><strong>3.</strong> Compute costs deducted automatically</li>
              <li><strong>4.</strong> Optionally tip project owner via <code className="bg-gray-100 px-1 rounded">X-Attached-Deposit</code></li>
            </ul>
          </div>

          <div className="border rounded-lg p-5">
            <h4 className="font-semibold text-gray-900 mb-3">For API Providers</h4>
            <ul className="text-sm text-gray-700 space-y-2">
              <li><strong>1.</strong> Users pay you via <code className="bg-gray-100 px-1 rounded">X-Attached-Deposit</code> header</li>
              <li><strong>2.</strong> Your WASM reads <code className="bg-gray-100 px-1 rounded">USD_PAYMENT</code> env var</li>
              <li><strong>3.</strong> Earnings accumulate in your account</li>
              <li><strong>4.</strong> Withdraw anytime at <Link href="/earnings" className="text-[var(--primary-orange)] hover:underline">/earnings</Link></li>
            </ul>
          </div>
        </div>

        <SyntaxHighlighter language="rust" style={vscDarkPlus} className="rounded-lg mb-4">
          {`// In your WASM code - check if user paid
let payment: u64 = std::env::var("USD_PAYMENT")
    .unwrap_or_else(|_| "0".to_string())
    .parse()
    .unwrap_or(0);

if payment < 100_000 {  // Require $0.10 minimum
    eprintln!("Payment required: $0.10 minimum");
    std::process::exit(1);
}

// Process paid request...`}
        </SyntaxHighlighter>
      </section>

      {/* Environment Variables */}
      <section className="mb-12">
        <AnchorHeading id="env-vars">Environment Variables</AnchorHeading>

        <p className="text-gray-700 mb-4">
          Your WASM code receives context via environment variables:
        </p>

        <div className="overflow-x-auto mb-6">
          <table className="min-w-full divide-y divide-gray-200 text-sm">
            <thead className="bg-gray-50">
              <tr>
                <th className="px-3 py-2 text-left text-xs font-medium text-gray-500 uppercase">Variable</th>
                <th className="px-3 py-2 text-left text-xs font-medium text-gray-500 uppercase">Description</th>
              </tr>
            </thead>
            <tbody className="bg-white divide-y divide-gray-200">
              <tr className="bg-blue-50">
                <td className="px-3 py-2 font-mono">OUTLAYER_EXECUTION_TYPE</td>
                <td className="px-3 py-2">&quot;HTTPS&quot; (for Web2 calls)</td>
              </tr>
              <tr>
                <td className="px-3 py-2 font-mono">NEAR_SENDER_ID</td>
                <td className="px-3 py-2">Payment Key owner (e.g., &quot;alice.near&quot;)</td>
              </tr>
              <tr className="bg-yellow-50">
                <td className="px-3 py-2 font-mono">USD_PAYMENT</td>
                <td className="px-3 py-2">Amount from X-Attached-Deposit (micro-USD)</td>
              </tr>
              <tr>
                <td className="px-3 py-2 font-mono">OUTLAYER_CALL_ID</td>
                <td className="px-3 py-2">Unique execution ID for this call</td>
              </tr>
            </tbody>
          </table>
        </div>

        <p className="text-gray-700">
          Full list: <Link href="/docs/https-api#env-vars" className="text-[var(--primary-orange)] hover:underline">HTTPS API environment variables</Link>
        </p>
      </section>

      {/* API Reference */}
      <section className="mb-12">
        <AnchorHeading id="api-reference">API Reference</AnchorHeading>

        <div className="overflow-x-auto mb-6">
          <table className="min-w-full divide-y divide-gray-200 text-sm">
            <thead className="bg-gray-50">
              <tr>
                <th className="px-3 py-2 text-left text-xs font-medium text-gray-500 uppercase">Header</th>
                <th className="px-3 py-2 text-left text-xs font-medium text-gray-500 uppercase">Required</th>
                <th className="px-3 py-2 text-left text-xs font-medium text-gray-500 uppercase">Description</th>
              </tr>
            </thead>
            <tbody className="bg-white divide-y divide-gray-200">
              <tr>
                <td className="px-3 py-2 font-mono">X-Payment-Key</td>
                <td className="px-3 py-2 text-green-600">Yes</td>
                <td className="px-3 py-2">Format: <code>owner:nonce:secret</code></td>
              </tr>
              <tr>
                <td className="px-3 py-2 font-mono">X-Attached-Deposit</td>
                <td className="px-3 py-2 text-gray-400">No</td>
                <td className="px-3 py-2">USD micro-units to pay project owner (1M = $1)</td>
              </tr>
              <tr>
                <td className="px-3 py-2 font-mono">X-Compute-Limit</td>
                <td className="px-3 py-2 text-gray-400">No</td>
                <td className="px-3 py-2">Max compute budget in USD micro-units</td>
              </tr>
            </tbody>
          </table>
        </div>

        <p className="text-gray-700">
          Full API documentation: <Link href="/docs/https-api" className="text-[var(--primary-orange)] hover:underline">HTTPS API Reference →</Link>
        </p>
      </section>

      {/* Code Examples */}
      <section className="mb-12">
        <AnchorHeading id="code-examples">Code Examples</AnchorHeading>

        <AnchorHeading id="example-javascript" level={3}>JavaScript / TypeScript</AnchorHeading>
        <SyntaxHighlighter language="javascript" style={vscDarkPlus} className="rounded-lg mb-6">
          {`async function callOutLayer(project, input, options = {}) {
  const response = await fetch(\`https://api.outlayer.fastnear.com/call/\${project}\`, {
    method: 'POST',
    headers: {
      'X-Payment-Key': process.env.OUTLAYER_PAYMENT_KEY,
      'Content-Type': 'application/json',
      ...(options.payment && { 'X-Attached-Deposit': options.payment.toString() })
    },
    body: JSON.stringify({ input })
  });

  const result = await response.json();

  if (result.status === 'failed') {
    throw new Error(result.error);
  }

  return {
    output: result.output,
    cost: Number(result.compute_cost) / 1_000_000,  // in USD
    jobId: result.job_id  // for attestation verification
  };
}

// Usage
const weather = await callOutLayer('alice.near/weather-api', { city: 'Tokyo' });
console.log(weather.output);  // "22°C, Partly Cloudy"
console.log(\`Cost: $\${weather.cost}\`);  // "Cost: $0.015"`}
        </SyntaxHighlighter>

        <AnchorHeading id="example-python" level={3}>Python</AnchorHeading>
        <SyntaxHighlighter language="python" style={vscDarkPlus} className="rounded-lg mb-6">
          {`import requests
import os

def call_outlayer(project: str, input_data: dict, payment: int = 0) -> dict:
    headers = {"X-Payment-Key": os.environ["OUTLAYER_PAYMENT_KEY"]}
    if payment:
        headers["X-Attached-Deposit"] = str(payment)

    response = requests.post(
        f"https://api.outlayer.fastnear.com/call/{project}",
        headers=headers,
        json={"input": input_data},
    )
    response.raise_for_status()

    result = response.json()
    if result["status"] == "failed":
        raise Exception(f"Execution failed: {result['error']}")

    return {
        "output": result["output"],
        "cost": int(result["compute_cost"]) / 1_000_000,
        "job_id": result["job_id"]
    }

# Usage
weather = call_outlayer("alice.near/weather-api", {"city": "Tokyo"})
print(weather["output"])`}
        </SyntaxHighlighter>
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
              Encrypted API keys and credentials for your WASM code
            </p>
          </Link>

          <Link href="/docs/payment-keys" className="block border rounded-lg p-4 hover:border-[var(--primary-orange)] transition-colors">
            <h4 className="font-semibold text-gray-900 mb-2">Payment Keys</h4>
            <p className="text-sm text-gray-600">
              Prepaid USD keys for HTTPS API access, restrictions
            </p>
          </Link>

          <Link href="/docs/earnings" className="block border rounded-lg p-4 hover:border-[var(--primary-orange)] transition-colors">
            <h4 className="font-semibold text-gray-900 mb-2">Earnings</h4>
            <p className="text-sm text-gray-600">
              Monetize your project, receive USD payments from users
            </p>
          </Link>
        </div>
      </section>

      {/* Related Documentation */}
      <section className="bg-gray-50 rounded-lg p-6">
        <h3 className="text-lg font-semibold text-gray-900 mb-4">Related Documentation</h3>

        <div className="grid grid-cols-1 md:grid-cols-2 gap-6">
          <div>
            <h4 className="font-semibold text-gray-900 mb-2">API & Payments</h4>
            <ul className="space-y-1 text-sm">
              <li><Link href="/docs/https-api" className="text-[var(--primary-orange)] hover:underline">HTTPS API Reference</Link> — Full endpoint docs</li>
              <li><Link href="/docs/payment-keys" className="text-[var(--primary-orange)] hover:underline">Payment Keys</Link> — Create and manage keys</li>
              <li><Link href="/docs/earnings" className="text-[var(--primary-orange)] hover:underline">Earnings</Link> — Monetize your API</li>
            </ul>
          </div>

          <div>
            <h4 className="font-semibold text-gray-900 mb-2">Building Projects</h4>
            <ul className="space-y-1 text-sm">
              <li><Link href="/docs/wasi" className="text-[var(--primary-orange)] hover:underline">Building OutLayer App</Link> — Build your WASM</li>
              <li><Link href="/docs/projects" className="text-[var(--primary-orange)] hover:underline">Projects</Link> — Version management</li>
              <li><Link href="/docs/storage" className="text-[var(--primary-orange)] hover:underline">Storage</Link> — Persistent data</li>
              <li><Link href="/docs/secrets" className="text-[var(--primary-orange)] hover:underline">Secrets</Link> — API keys in WASM</li>
            </ul>
          </div>

          <div>
            <h4 className="font-semibold text-gray-900 mb-2">Verification</h4>
            <ul className="space-y-1 text-sm">
              <li><Link href="/docs/tee-attestation" className="text-[var(--primary-orange)] hover:underline">TEE Attestation</Link> — How verification works</li>
            </ul>
          </div>

          <div>
            <h4 className="font-semibold text-gray-900 mb-2">Examples</h4>
            <ul className="space-y-1 text-sm">
              <li><Link href="/docs/examples" className="text-[var(--primary-orange)] hover:underline">Example Projects</Link> — Working code</li>
              <li><Link href="/playground" className="text-[var(--primary-orange)] hover:underline">Playground</Link> — Test execution</li>
            </ul>
          </div>
        </div>
      </section>
    </div>
  );
}
