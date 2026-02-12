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

export default function HttpsApiPage() {
  return (
    <div className="prose prose-lg max-w-none">
      <h1 className="text-3xl font-bold text-gray-900 mb-6">HTTPS API</h1>

      <p className="text-gray-700 mb-8 text-lg">
        Call OutLayer projects via simple HTTP requests without NEAR transactions.
        Authentication uses{' '}
        <Link href="/docs/payment-keys" className="text-[var(--primary-orange)] hover:underline">Payment Keys</Link>
        {' '}with prepaid USD stablecoin balance.
      </p>

      {/* Overview */}
      <section className="mb-12">
        <AnchorHeading id="overview">Overview</AnchorHeading>

        <div className="grid grid-cols-1 md:grid-cols-2 gap-4 mb-6">
          <div className="border border-blue-200 rounded-lg p-4 bg-blue-50">
            <h4 className="font-semibold text-blue-900 mb-2">NEAR Transactions</h4>
            <ul className="text-sm text-gray-700 space-y-1">
              <li>Payment: NEAR tokens</li>
              <li>Latency: ~2-3 seconds</li>
              <li>Response: callback to contract</li>
            </ul>
          </div>
          <div className="border border-green-200 rounded-lg p-4 bg-green-50">
            <h4 className="font-semibold text-green-900 mb-2">HTTPS API</h4>
            <ul className="text-sm text-gray-700 space-y-1">
              <li>Payment: USD stablecoins</li>
              <li>Latency: instant (sync mode)</li>
              <li>Response: HTTP response body</li>
            </ul>
          </div>
        </div>

        <SyntaxHighlighter language="bash" style={vscDarkPlus} className="rounded-lg mb-4">
          {`# Simple example
curl -X POST https://api.outlayer.fastnear.com/call/alice.near/my-assistant \\
  -H "X-Payment-Key: bob.near:1:K7xR2mN9pQs5vW3yZ8bF..." \\
  -H "Content-Type: application/json" \\
  -d '{"input": {"prompt": "Hello!"}}'`}
        </SyntaxHighlighter>
      </section>

      {/* Request Format */}
      <section className="mb-12">
        <AnchorHeading id="request-format">Request Format</AnchorHeading>

        <AnchorHeading id="endpoint" level={3}>Endpoint</AnchorHeading>

        <SyntaxHighlighter language="text" style={vscDarkPlus} className="rounded-lg mb-4">
          {`POST https://api.outlayer.fastnear.com/call/{project_owner}/{project_name}`}
        </SyntaxHighlighter>

        <p className="text-gray-700 mb-4">
          Example: <code>POST https://api.outlayer.fastnear.com/call/alice.near/weather-api</code>
        </p>

        <AnchorHeading id="headers" level={3}>Request Headers</AnchorHeading>

        <div className="overflow-x-auto mb-6">
          <table className="min-w-full divide-y divide-gray-200">
            <thead className="bg-gray-50">
              <tr>
                <th className="px-4 py-3 text-left text-xs font-medium text-gray-500 uppercase">Header</th>
                <th className="px-4 py-3 text-left text-xs font-medium text-gray-500 uppercase">Required</th>
                <th className="px-4 py-3 text-left text-xs font-medium text-gray-500 uppercase">Default</th>
                <th className="px-4 py-3 text-left text-xs font-medium text-gray-500 uppercase">Description</th>
              </tr>
            </thead>
            <tbody className="bg-white divide-y divide-gray-200">
              <tr className="bg-green-50">
                <td className="px-4 py-3 text-sm font-mono">X-Payment-Key</td>
                <td className="px-4 py-3 text-sm text-green-600 font-semibold">Yes</td>
                <td className="px-4 py-3 text-sm text-gray-400">-</td>
                <td className="px-4 py-3 text-sm text-gray-600">
                  Format: <code>owner:nonce:secret</code><br/>
                  Authentication for the API call
                </td>
              </tr>
              <tr>
                <td className="px-4 py-3 text-sm font-mono">X-Compute-Limit</td>
                <td className="px-4 py-3 text-sm text-gray-500">No</td>
                <td className="px-4 py-3 text-sm font-mono">10000</td>
                <td className="px-4 py-3 text-sm text-gray-600">
                  Max compute budget in USD micro-units<br/>
                  10000 = $0.01, 100000 = $0.10
                </td>
              </tr>
              <tr className="bg-yellow-50">
                <td className="px-4 py-3 text-sm font-mono">X-Attached-Deposit</td>
                <td className="px-4 py-3 text-sm text-gray-500">No</td>
                <td className="px-4 py-3 text-sm font-mono">0</td>
                <td className="px-4 py-3 text-sm text-gray-600">
                  <strong>Payment to project author</strong> in USD micro-units<br/>
                  1000000 = $1.00. Goes to author&apos;s earnings.
                </td>
              </tr>
              <tr>
                <td className="px-4 py-3 text-sm font-mono">Content-Type</td>
                <td className="px-4 py-3 text-sm text-gray-500">No</td>
                <td className="px-4 py-3 text-sm font-mono">application/json</td>
                <td className="px-4 py-3 text-sm text-gray-600">Request body format</td>
              </tr>
            </tbody>
          </table>
        </div>

        <div className="bg-yellow-50 border-l-4 border-yellow-400 p-4 mb-6">
          <p className="text-sm text-yellow-800">
            <strong>X-Attached-Deposit vs X-Compute-Limit:</strong>
          </p>
          <ul className="list-disc list-inside text-sm text-yellow-800 mt-2 space-y-1">
            <li><code>X-Compute-Limit</code> - pays for infrastructure (OutLayer), refunded if unused</li>
            <li><code>X-Attached-Deposit</code> - pays the project author, charged immediately</li>
          </ul>
        </div>

        <AnchorHeading id="body" level={3}>Request Body</AnchorHeading>

        <SyntaxHighlighter language="json" style={vscDarkPlus} className="rounded-lg mb-4">
          {`{
  "input": {                    // Required - passed to WASM as stdin
    "prompt": "Hello, AI!",
    "temperature": 0.7
  },
  "resource_limits": {          // Optional - override project defaults
    "max_instructions": 10000000000,
    "max_memory_mb": 128,
    "max_execution_seconds": 60
  },
  "secrets_ref": {              // Optional - keystore secrets
    "profile": "default",
    "account_id": "alice.near"
  },
  "async": false,               // Optional - sync (default) or async mode
  "version_key": "user/repo@a1b2c3" // Optional - pin to specific version
}`}
        </SyntaxHighlighter>

        <div className="overflow-x-auto mb-6">
          <table className="min-w-full divide-y divide-gray-200">
            <thead className="bg-gray-50">
              <tr>
                <th className="px-4 py-3 text-left text-xs font-medium text-gray-500 uppercase">Field</th>
                <th className="px-4 py-3 text-left text-xs font-medium text-gray-500 uppercase">Type</th>
                <th className="px-4 py-3 text-left text-xs font-medium text-gray-500 uppercase">Required</th>
                <th className="px-4 py-3 text-left text-xs font-medium text-gray-500 uppercase">Description</th>
              </tr>
            </thead>
            <tbody className="bg-white divide-y divide-gray-200">
              <tr>
                <td className="px-4 py-3 text-sm font-mono">input</td>
                <td className="px-4 py-3 text-sm">object</td>
                <td className="px-4 py-3 text-sm text-green-600">Yes</td>
                <td className="px-4 py-3 text-sm text-gray-600">Passed to WASM code as JSON via stdin</td>
              </tr>
              <tr>
                <td className="px-4 py-3 text-sm font-mono">resource_limits</td>
                <td className="px-4 py-3 text-sm">object</td>
                <td className="px-4 py-3 text-sm text-gray-500">No</td>
                <td className="px-4 py-3 text-sm text-gray-600">Override project&apos;s default limits</td>
              </tr>
              <tr className="bg-purple-50">
                <td className="px-4 py-3 text-sm font-mono">secrets_ref</td>
                <td className="px-4 py-3 text-sm">object</td>
                <td className="px-4 py-3 text-sm text-gray-500">No</td>
                <td className="px-4 py-3 text-sm text-gray-600">
                  Reference to{' '}
                  <Link href="/docs/secrets" className="text-[var(--primary-orange)] hover:underline">keystore secrets</Link>.
                  Contains <code>profile</code> (string) and <code>account_id</code> (string).
                  Decrypted secrets are injected as environment variables into WASM.
                </td>
              </tr>
              <tr>
                <td className="px-4 py-3 text-sm font-mono">async</td>
                <td className="px-4 py-3 text-sm">boolean</td>
                <td className="px-4 py-3 text-sm text-gray-500">No</td>
                <td className="px-4 py-3 text-sm text-gray-600">false = wait for result, true = return call_id immediately</td>
              </tr>
              <tr>
                <td className="px-4 py-3 text-sm font-mono">version_key</td>
                <td className="px-4 py-3 text-sm">string</td>
                <td className="px-4 py-3 text-sm text-gray-500">No</td>
                <td className="px-4 py-3 text-sm text-gray-600">
                  Pin to a specific project version. Format: WASM hash or <code>repo@commit</code>.
                  If omitted, uses the project&apos;s active version.
                </td>
              </tr>
            </tbody>
          </table>
        </div>

        <AnchorHeading id="resource-limits" level={3}>Resource Limits</AnchorHeading>

        <div className="overflow-x-auto mb-6">
          <table className="min-w-full divide-y divide-gray-200">
            <thead className="bg-gray-50">
              <tr>
                <th className="px-4 py-3 text-left text-xs font-medium text-gray-500 uppercase">Limit</th>
                <th className="px-4 py-3 text-left text-xs font-medium text-gray-500 uppercase">Default</th>
                <th className="px-4 py-3 text-left text-xs font-medium text-gray-500 uppercase">Maximum</th>
                <th className="px-4 py-3 text-left text-xs font-medium text-gray-500 uppercase">Description</th>
              </tr>
            </thead>
            <tbody className="bg-white divide-y divide-gray-200">
              <tr>
                <td className="px-4 py-3 text-sm font-mono">max_instructions</td>
                <td className="px-4 py-3 text-sm">1 billion</td>
                <td className="px-4 py-3 text-sm">500 billion</td>
                <td className="px-4 py-3 text-sm text-gray-600">WASM instructions to execute</td>
              </tr>
              <tr>
                <td className="px-4 py-3 text-sm font-mono">max_memory_mb</td>
                <td className="px-4 py-3 text-sm">128 MB</td>
                <td className="px-4 py-3 text-sm">512 MB</td>
                <td className="px-4 py-3 text-sm text-gray-600">Memory available to WASM</td>
              </tr>
              <tr>
                <td className="px-4 py-3 text-sm font-mono">max_execution_seconds</td>
                <td className="px-4 py-3 text-sm">60 sec</td>
                <td className="px-4 py-3 text-sm">180 sec</td>
                <td className="px-4 py-3 text-sm text-gray-600">Wall-clock timeout</td>
              </tr>
            </tbody>
          </table>
        </div>
      </section>

      {/* Response Format */}
      <section className="mb-12">
        <AnchorHeading id="response-format">Response Format</AnchorHeading>

        <AnchorHeading id="sync-response" level={3}>Synchronous Response (default)</AnchorHeading>

        <p className="text-gray-700 mb-4">
          When <code>async: false</code> (default), the request waits until execution completes:
        </p>

        <SyntaxHighlighter language="json" style={vscDarkPlus} className="rounded-lg mb-4">
          {`// Success
{
  "call_id": "550e8400-e29b-41d4-a716-446655440000",
  "status": "completed",
  "output": "Here's the weather forecast for...",
  "compute_cost": "45000",
  "job_id": 12345,
  "attestation_url": "https://outlayer.fastnear.com/attestations/12345"
}

// Failure
{
  "call_id": "550e8400-e29b-41d4-a716-446655440000",
  "status": "failed",
  "error": "Execution timeout after 60 seconds",
  "compute_cost": "100000",
  "job_id": 12345
}`}
        </SyntaxHighlighter>

        <AnchorHeading id="async-response" level={3}>Asynchronous Response</AnchorHeading>

        <p className="text-gray-700 mb-4">
          When <code>async: true</code>, the request returns immediately with a <code>call_id</code> for polling:
        </p>

        <SyntaxHighlighter language="json" style={vscDarkPlus} className="rounded-lg mb-4">
          {`// Initial response
{
  "call_id": "550e8400-e29b-41d4-a716-446655440000",
  "status": "pending",
  "poll_url": "https://api.outlayer.fastnear.com/calls/550e8400-e29b-41d4-a716-446655440000"
}`}
        </SyntaxHighlighter>

        <p className="text-gray-700 mb-4">
          Poll the result using <code>GET /calls/{'{call_id}'}</code>:
        </p>

        <SyntaxHighlighter language="bash" style={vscDarkPlus} className="rounded-lg mb-4">
          {`curl -H "X-Payment-Key: bob.near:1:..." \\
  https://api.outlayer.fastnear.com/calls/550e8400-e29b-41d4-a716-446655440000`}
        </SyntaxHighlighter>

        <AnchorHeading id="response-fields" level={3}>Response Fields</AnchorHeading>

        <div className="overflow-x-auto mb-6">
          <table className="min-w-full divide-y divide-gray-200">
            <thead className="bg-gray-50">
              <tr>
                <th className="px-4 py-3 text-left text-xs font-medium text-gray-500 uppercase">Field</th>
                <th className="px-4 py-3 text-left text-xs font-medium text-gray-500 uppercase">Type</th>
                <th className="px-4 py-3 text-left text-xs font-medium text-gray-500 uppercase">Description</th>
              </tr>
            </thead>
            <tbody className="bg-white divide-y divide-gray-200">
              <tr>
                <td className="px-4 py-3 text-sm font-mono">call_id</td>
                <td className="px-4 py-3 text-sm">UUID</td>
                <td className="px-4 py-3 text-sm text-gray-600">Unique identifier for this API call</td>
              </tr>
              <tr>
                <td className="px-4 py-3 text-sm font-mono">status</td>
                <td className="px-4 py-3 text-sm">string</td>
                <td className="px-4 py-3 text-sm text-gray-600">&quot;pending&quot;, &quot;completed&quot;, or &quot;failed&quot;</td>
              </tr>
              <tr>
                <td className="px-4 py-3 text-sm font-mono">output</td>
                <td className="px-4 py-3 text-sm">any</td>
                <td className="px-4 py-3 text-sm text-gray-600">WASM stdout (only when completed)</td>
              </tr>
              <tr>
                <td className="px-4 py-3 text-sm font-mono">error</td>
                <td className="px-4 py-3 text-sm">string</td>
                <td className="px-4 py-3 text-sm text-gray-600">Error message (only when failed)</td>
              </tr>
              <tr className="bg-yellow-50">
                <td className="px-4 py-3 text-sm font-mono">compute_cost</td>
                <td className="px-4 py-3 text-sm">string</td>
                <td className="px-4 py-3 text-sm text-gray-600">
                  <strong>Actual USD cost</strong> in micro-units (e.g., &quot;45000&quot; = $0.045).
                  Charged even on failure.
                </td>
              </tr>
              <tr>
                <td className="px-4 py-3 text-sm font-mono">job_id</td>
                <td className="px-4 py-3 text-sm">number</td>
                <td className="px-4 py-3 text-sm text-gray-600">Internal job ID for attestation lookup</td>
              </tr>
              <tr>
                <td className="px-4 py-3 text-sm font-mono">attestation_url</td>
                <td className="px-4 py-3 text-sm">string</td>
                <td className="px-4 py-3 text-sm text-gray-600">Link to TEE attestation (completed only)</td>
              </tr>
            </tbody>
          </table>
        </div>
      </section>

      {/* Environment Variables */}
      <section className="mb-12">
        <AnchorHeading id="env-vars">Environment Variables in WASM</AnchorHeading>

        <p className="text-gray-700 mb-4">
          Your WASM code can detect the execution context and access payment information via environment variables.
          Values differ between NEAR transactions and HTTPS API calls:
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
              <tr className="bg-blue-50">
                <td className="px-3 py-2 font-mono">NEAR_NETWORK_ID</td>
                <td className="px-3 py-2">&quot;testnet&quot; or &quot;mainnet&quot;</td>
                <td className="px-3 py-2">&quot;testnet&quot; or &quot;mainnet&quot;</td>
              </tr>
              <tr>
                <td className="px-3 py-2 font-mono">NEAR_SENDER_ID</td>
                <td className="px-3 py-2">Transaction signer</td>
                <td className="px-3 py-2">Payment Key owner</td>
              </tr>
              <tr className="bg-yellow-50">
                <td className="px-3 py-2 font-mono">USD_PAYMENT</td>
                <td className="px-3 py-2">&quot;0&quot;</td>
                <td className="px-3 py-2">X-Attached-Deposit value</td>
              </tr>
              <tr>
                <td className="px-3 py-2 font-mono">NEAR_PAYMENT_YOCTO</td>
                <td className="px-3 py-2">Attached NEAR</td>
                <td className="px-3 py-2">&quot;0&quot;</td>
              </tr>
              <tr>
                <td className="px-3 py-2 font-mono">OUTLAYER_CALL_ID</td>
                <td className="px-3 py-2">&quot;&quot;</td>
                <td className="px-3 py-2">call_id UUID</td>
              </tr>
              <tr>
                <td className="px-3 py-2 font-mono">NEAR_TRANSACTION_HASH</td>
                <td className="px-3 py-2">Transaction hash</td>
                <td className="px-3 py-2">&quot;&quot;</td>
              </tr>
              <tr>
                <td className="px-3 py-2 font-mono">NEAR_BLOCK_HEIGHT</td>
                <td className="px-3 py-2">Block number</td>
                <td className="px-3 py-2">&quot;&quot;</td>
              </tr>
              <tr>
                <td className="px-3 py-2 font-mono">NEAR_BLOCK_TIMESTAMP</td>
                <td className="px-3 py-2">Block timestamp</td>
                <td className="px-3 py-2">&quot;&quot;</td>
              </tr>
              <tr>
                <td className="px-3 py-2 font-mono">OUTLAYER_PROJECT_ID</td>
                <td className="px-3 py-2" colSpan={2}>owner/name (same for both)</td>
              </tr>
              <tr>
                <td className="px-3 py-2 font-mono">OUTLAYER_PROJECT_OWNER</td>
                <td className="px-3 py-2" colSpan={2}>Project owner account (e.g., &quot;alice.near&quot;)</td>
              </tr>
              <tr>
                <td className="px-3 py-2 font-mono">OUTLAYER_PROJECT_NAME</td>
                <td className="px-3 py-2" colSpan={2}>Project name (may contain &quot;/&quot;)</td>
              </tr>
            </tbody>
          </table>
        </div>

        <AnchorHeading id="detecting-mode" level={3}>Detecting Execution Mode</AnchorHeading>

        <SyntaxHighlighter language="rust" style={vscDarkPlus} className="rounded-lg mb-4">
          {`let execution_type = std::env::var("OUTLAYER_EXECUTION_TYPE")
    .unwrap_or_else(|_| "NEAR".to_string());

match execution_type.as_str() {
    "HTTPS" => {
        // HTTPS API call
        let usd_payment: u64 = std::env::var("USD_PAYMENT")
            .unwrap_or_else(|_| "0".to_string())
            .parse()
            .unwrap_or(0);

        let call_id = std::env::var("OUTLAYER_CALL_ID").unwrap_or_default();
        let cost = usd_payment as f64 / 1_000_000.0;
        println!("HTTPS call {}: paid {} USD", call_id, cost);
    }
    "NEAR" => {
        // NEAR transaction
        let near_payment: u128 = std::env::var("NEAR_PAYMENT_YOCTO")
            .unwrap_or_else(|_| "0".to_string())
            .parse()
            .unwrap_or(0);

        let tx_hash = std::env::var("NEAR_TRANSACTION_HASH").unwrap_or_default();
        println!("NEAR tx {}: paid {} yoctoNEAR", tx_hash, near_payment);
    }
    _ => {}
}`}
        </SyntaxHighlighter>

        <AnchorHeading id="checking-payment" level={3}>Checking Payment (USD)</AnchorHeading>

        <SyntaxHighlighter language="rust" style={vscDarkPlus} className="rounded-lg mb-4">
          {`// USD_PAYMENT is in micro-units: 1000000 = $1.00
let usd_payment: u64 = std::env::var("USD_PAYMENT")
    .unwrap_or_else(|_| "0".to_string())
    .parse()
    .unwrap_or(0);

// Require $0.10 minimum for premium features
const MIN_PREMIUM_USD: u64 = 100_000; // $0.10

if usd_payment >= MIN_PREMIUM_USD {
    // Premium feature
    expensive_ai_analysis();
} else if usd_payment > 0 {
    // Basic paid feature
    basic_analysis();
} else {
    // Free tier
    simple_response();
}`}
        </SyntaxHighlighter>
      </section>

      {/* Secrets */}
      <section className="mb-12">
        <AnchorHeading id="secrets">Using Secrets</AnchorHeading>

        <p className="text-gray-700 mb-4">
          If your WASM code needs API keys or other sensitive data, use{' '}
          <Link href="/docs/secrets" className="text-[var(--primary-orange)] hover:underline">Keystore Secrets</Link>.
          Secrets are encrypted and stored on-chain, then decrypted inside TEE at execution time
          and injected as environment variables.
        </p>

        <p className="text-gray-700 mb-4">
          Pass <code>secrets_ref</code> in the request body to specify which secret profile to use:
        </p>

        <SyntaxHighlighter language="bash" style={vscDarkPlus} className="rounded-lg mb-4">
          {`curl -X POST https://api.outlayer.fastnear.com/call/alice.near/weather-api \\
  -H "X-Payment-Key: bob.near:1:K7xR2mN9pQs5vW3yZ8bF..." \\
  -H "Content-Type: application/json" \\
  -d '{
    "input": {"city": "Tokyo"},
    "secrets_ref": {
      "profile": "default",
      "account_id": "alice.near"
    }
  }'`}
        </SyntaxHighlighter>

        <div className="overflow-x-auto mb-6">
          <table className="min-w-full divide-y divide-gray-200">
            <thead className="bg-gray-50">
              <tr>
                <th className="px-4 py-3 text-left text-xs font-medium text-gray-500 uppercase">Field</th>
                <th className="px-4 py-3 text-left text-xs font-medium text-gray-500 uppercase">Type</th>
                <th className="px-4 py-3 text-left text-xs font-medium text-gray-500 uppercase">Description</th>
              </tr>
            </thead>
            <tbody className="bg-white divide-y divide-gray-200">
              <tr>
                <td className="px-4 py-3 text-sm font-mono">profile</td>
                <td className="px-4 py-3 text-sm">string</td>
                <td className="px-4 py-3 text-sm text-gray-600">
                  Secret profile name (e.g., &quot;default&quot;, &quot;production&quot;).
                  Configured in the{' '}
                  <Link href="/docs/secrets" className="text-[var(--primary-orange)] hover:underline">Secrets</Link> page.
                </td>
              </tr>
              <tr>
                <td className="px-4 py-3 text-sm font-mono">account_id</td>
                <td className="px-4 py-3 text-sm">string</td>
                <td className="px-4 py-3 text-sm text-gray-600">NEAR account that owns the secrets (the one who encrypted them)</td>
              </tr>
            </tbody>
          </table>
        </div>

        <AnchorHeading id="secrets-access" level={3}>Accessing Secrets in WASM</AnchorHeading>

        <p className="text-gray-700 mb-4">
          Decrypted secrets are available as regular environment variables:
        </p>

        <SyntaxHighlighter language="rust" style={vscDarkPlus} className="rounded-lg mb-4">
          {`// Secrets are injected as env vars by the TEE worker
let api_key = std::env::var("OPENAI_API_KEY")
    .expect("OPENAI_API_KEY secret not set");

let db_url = std::env::var("DATABASE_URL")
    .expect("DATABASE_URL secret not set");`}
        </SyntaxHighlighter>

        <div className="bg-blue-50 border-l-4 border-blue-400 p-4 mb-6">
          <p className="text-sm text-blue-800">
            <strong>Works the same as NEAR transactions.</strong>{' '}
            The <code>secrets_ref</code> field uses the same keystore system as the{' '}
            <code>secrets_ref</code> parameter in <code>request_execution</code> contract calls.
            If your WASM already uses secrets via NEAR transactions, just pass the same profile in HTTPS calls.
          </p>
        </div>
      </section>

      {/* Error Codes */}
      <section className="mb-12">
        <AnchorHeading id="errors">Error Codes</AnchorHeading>

        <div className="overflow-x-auto mb-6">
          <table className="min-w-full divide-y divide-gray-200">
            <thead className="bg-gray-50">
              <tr>
                <th className="px-4 py-3 text-left text-xs font-medium text-gray-500 uppercase">Code</th>
                <th className="px-4 py-3 text-left text-xs font-medium text-gray-500 uppercase">Status</th>
                <th className="px-4 py-3 text-left text-xs font-medium text-gray-500 uppercase">Description</th>
              </tr>
            </thead>
            <tbody className="bg-white divide-y divide-gray-200">
              <tr>
                <td className="px-4 py-3 text-sm font-mono">400</td>
                <td className="px-4 py-3 text-sm">Bad Request</td>
                <td className="px-4 py-3 text-sm text-gray-600">Invalid request body or headers</td>
              </tr>
              <tr>
                <td className="px-4 py-3 text-sm font-mono">401</td>
                <td className="px-4 py-3 text-sm">Unauthorized</td>
                <td className="px-4 py-3 text-sm text-gray-600">Invalid or missing Payment Key</td>
              </tr>
              <tr>
                <td className="px-4 py-3 text-sm font-mono">402</td>
                <td className="px-4 py-3 text-sm">Payment Required</td>
                <td className="px-4 py-3 text-sm text-gray-600">Insufficient balance on Payment Key</td>
              </tr>
              <tr>
                <td className="px-4 py-3 text-sm font-mono">403</td>
                <td className="px-4 py-3 text-sm">Forbidden</td>
                <td className="px-4 py-3 text-sm text-gray-600">Project not allowed for this key</td>
              </tr>
              <tr>
                <td className="px-4 py-3 text-sm font-mono">404</td>
                <td className="px-4 py-3 text-sm">Not Found</td>
                <td className="px-4 py-3 text-sm text-gray-600">Project does not exist</td>
              </tr>
              <tr>
                <td className="px-4 py-3 text-sm font-mono">429</td>
                <td className="px-4 py-3 text-sm">Too Many Requests</td>
                <td className="px-4 py-3 text-sm text-gray-600">Rate limit exceeded (IP or key)</td>
              </tr>
              <tr>
                <td className="px-4 py-3 text-sm font-mono">500</td>
                <td className="px-4 py-3 text-sm">Internal Error</td>
                <td className="px-4 py-3 text-sm text-gray-600">Server error during execution</td>
              </tr>
              <tr>
                <td className="px-4 py-3 text-sm font-mono">504</td>
                <td className="px-4 py-3 text-sm">Gateway Timeout</td>
                <td className="px-4 py-3 text-sm text-gray-600">Execution timeout (300s max)</td>
              </tr>
            </tbody>
          </table>
        </div>
      </section>

      {/* Examples */}
      <section className="mb-12">
        <AnchorHeading id="examples">Code Examples</AnchorHeading>

        <AnchorHeading id="example-curl" level={3}>cURL</AnchorHeading>

        <SyntaxHighlighter language="bash" style={vscDarkPlus} className="rounded-lg mb-4">
          {`# Basic call
curl -X POST https://api.outlayer.fastnear.com/call/alice.near/weather-api \\
  -H "X-Payment-Key: bob.near:1:K7xR2mN9pQs5vW3yZ8bF..." \\
  -H "Content-Type: application/json" \\
  -d '{"input": {"city": "Tokyo"}}'

# With compute limit and author payment
curl -X POST https://api.outlayer.fastnear.com/call/alice.near/premium-api \\
  -H "X-Payment-Key: bob.near:1:K7xR2mN9pQs5vW3yZ8bF..." \\
  -H "X-Compute-Limit: 500000" \\
  -H "X-Attached-Deposit: 100000" \\
  -H "Content-Type: application/json" \\
  -d '{"input": {"query": "complex analysis"}}'

# Async mode
curl -X POST https://api.outlayer.fastnear.com/call/alice.near/long-running \\
  -H "X-Payment-Key: bob.near:1:K7xR2mN9pQs5vW3yZ8bF..." \\
  -H "Content-Type: application/json" \\
  -d '{"input": {...}, "async": true}'`}
        </SyntaxHighlighter>

        <AnchorHeading id="example-javascript" level={3}>JavaScript / TypeScript</AnchorHeading>

        <SyntaxHighlighter language="typescript" style={vscDarkPlus} className="rounded-lg mb-4">
          {`async function callOutLayer(projectId: string, input: object) {
  const response = await fetch(\`https://api.outlayer.fastnear.com/call/\${projectId}\`, {
    method: 'POST',
    headers: {
      'X-Payment-Key': process.env.OUTLAYER_PAYMENT_KEY!,
      'X-Compute-Limit': '100000', // $0.10 max
      'X-Attached-Deposit': '50000', // $0.05 to author
      'Content-Type': 'application/json',
    },
    body: JSON.stringify({ input }),
  });

  if (!response.ok) {
    throw new Error(\`OutLayer error: \${response.status}\`);
  }

  const result = await response.json();

  if (result.status === 'failed') {
    throw new Error(\`Execution failed: \${result.error}\`);
  }

  const costUsd = Number(result.compute_cost) / 1_000_000;
  console.log('Cost:', costUsd, 'USD');
  return result.output;
}

// Usage
const weather = await callOutLayer('alice.near/weather-api', { city: 'Tokyo' });`}
        </SyntaxHighlighter>

        <AnchorHeading id="example-python" level={3}>Python</AnchorHeading>

        <SyntaxHighlighter language="python" style={vscDarkPlus} className="rounded-lg mb-4">
          {`import os
import requests

def call_outlayer(project_id: str, input_data: dict) -> dict:
    """Call an OutLayer project via HTTPS API."""
    response = requests.post(
        f"https://api.outlayer.fastnear.com/call/{project_id}",
        headers={
            "X-Payment-Key": os.environ["OUTLAYER_PAYMENT_KEY"],
            "X-Compute-Limit": "100000",  # $0.10 max
            "X-Attached-Deposit": "50000",  # $0.05 to author
            "Content-Type": "application/json",
        },
        json={"input": input_data},
    )
    response.raise_for_status()

    result = response.json()
    if result["status"] == "failed":
        raise Exception(f"Execution failed: {result['error']}")

    cost = int(result['compute_cost']) / 1_000_000
    print(f"Cost: {cost} USD")
    return result["output"]

# Usage
weather = call_outlayer("alice.near/weather-api", {"city": "Tokyo"})`}
        </SyntaxHighlighter>
      </section>

      {/* Pricing */}
      <section className="mb-12">
        <AnchorHeading id="pricing">Pricing</AnchorHeading>

        <p className="text-gray-700 mb-4">
          HTTPS API calls are charged in USD stablecoins based on actual resource consumption:
        </p>

        <div className="bg-gray-50 rounded-lg p-4 mb-6 font-mono text-sm">
          compute_cost = base_fee_usd<br/>
          &nbsp;&nbsp;+ (instructions / 1M) × per_million_instructions_usd<br/>
          &nbsp;&nbsp;+ execution_time_ms × per_ms_usd<br/>
          &nbsp;&nbsp;+ compile_time_ms × per_compile_ms_usd
        </div>

        <p className="text-gray-700 mb-4">
          See <Link href="/docs/pricing" className="text-[var(--primary-orange)] hover:underline">Pricing & Limits</Link>
          {' '}for current rates. Key points:
        </p>

        <ul className="list-disc list-inside text-gray-700 space-y-2 mb-6">
          <li><strong>compute_cost</strong> - charged based on actual resources used</li>
          <li><strong>X-Attached-Deposit</strong> - charged immediately, goes to project author</li>
          <li><strong>Minimum</strong>: X-Compute-Limit must be at least $0.001 (1000)</li>
          <li><strong>Failures</strong>: you still pay for resources used before the failure</li>
        </ul>
      </section>

      {/* Related Documentation */}
      <section className="bg-gray-50 rounded-lg p-6">
        <h3 className="text-lg font-semibold text-gray-900 mb-4">Related Documentation</h3>

        <ul className="space-y-2 text-sm">
          <li>
            <Link href="/docs/payment-keys" className="text-[var(--primary-orange)] hover:underline">Payment Keys</Link>
            {' '}- Creating and managing Payment Keys
          </li>
          <li>
            <Link href="/docs/earnings" className="text-[var(--primary-orange)] hover:underline">Earnings</Link>
            {' '}- How project authors earn from X-Attached-Deposit
          </li>
          <li>
            <Link href="/docs/secrets" className="text-[var(--primary-orange)] hover:underline">Secrets (Keystore)</Link>
            {' '}- Encrypting and managing secrets for WASM execution
          </li>
          <li>
            <Link href="/docs/web2-integration" className="text-[var(--primary-orange)] hover:underline">Web2 Integration</Link>
            {' '}- Complete HTTPS integration guide
          </li>
          <li>
            <Link href="/docs/tee-attestation" className="text-[var(--primary-orange)] hover:underline">TEE Attestation</Link>
            {' '}- Verifying execution attestations
          </li>
        </ul>
      </section>
    </div>
  );
}
