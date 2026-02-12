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

export default function PaymentKeysPage() {
  return (
    <div className="prose prose-lg max-w-none">
      <h1 className="text-3xl font-bold text-gray-900 mb-6">Payment Keys</h1>

      <p className="text-gray-700 mb-8 text-lg">
        Payment Keys enable HTTPS API access to OutLayer projects without NEAR transactions.
        Users prepay in USD stablecoins and spend from their balance with simple HTTP requests.
      </p>

      {/* What are Payment Keys */}
      <section className="mb-12">
        <AnchorHeading id="what-are-payment-keys">What are Payment Keys?</AnchorHeading>

        <p className="text-gray-700 mb-4">
          A Payment Key is a secret token linked to your NEAR account with a prepaid USD balance.
          It allows you to call OutLayer projects via{' '}
          <Link href="/docs/https-api" className="text-[var(--primary-orange)] hover:underline">HTTPS API</Link>
          {' '}without signing NEAR transactions.
        </p>

        <div className="bg-blue-50 border-l-4 border-blue-400 p-4 mb-6">
          <p className="text-sm text-blue-800">
            <strong>Key benefits:</strong>
          </p>
          <ul className="list-disc list-inside text-sm text-blue-800 mt-2 space-y-1">
            <li>No blockchain transactions per API call (only key setup/withdrawal)</li>
            <li>Sub-second response times</li>
            <li>Easy integration with existing backends</li>
            <li>USD stablecoin payments (USDT/USDC)</li>
          </ul>
        </div>

        <AnchorHeading id="key-format" level={3}>Key Format</AnchorHeading>

        <p className="text-gray-700 mb-4">
          Payment Keys are passed in the <code>X-Payment-Key</code> HTTP header:
        </p>

        <SyntaxHighlighter language="text" style={vscDarkPlus} className="rounded-lg mb-4">
          {`X-Payment-Key: {owner}:{nonce}:{secret}

Example: X-Payment-Key: alice.near:1:K7xR2mN9pQs5vW3yZ8bF...`}
        </SyntaxHighlighter>

        <div className="overflow-x-auto mb-6">
          <table className="min-w-full divide-y divide-gray-200">
            <thead className="bg-gray-50">
              <tr>
                <th className="px-4 py-3 text-left text-xs font-medium text-gray-500 uppercase">Part</th>
                <th className="px-4 py-3 text-left text-xs font-medium text-gray-500 uppercase">Description</th>
                <th className="px-4 py-3 text-left text-xs font-medium text-gray-500 uppercase">Example</th>
              </tr>
            </thead>
            <tbody className="bg-white divide-y divide-gray-200">
              <tr>
                <td className="px-4 py-3 text-sm font-mono">owner</td>
                <td className="px-4 py-3 text-sm text-gray-600">Your NEAR account ID</td>
                <td className="px-4 py-3 text-sm font-mono">alice.near</td>
              </tr>
              <tr>
                <td className="px-4 py-3 text-sm font-mono">nonce</td>
                <td className="px-4 py-3 text-sm text-gray-600">Key number (1, 2...)</td>
                <td className="px-4 py-3 text-sm font-mono">1</td>
              </tr>
              <tr>
                <td className="px-4 py-3 text-sm font-mono">secret</td>
                <td className="px-4 py-3 text-sm text-gray-600">32-byte random token (Base64)</td>
                <td className="px-4 py-3 text-sm font-mono">K7xR2mN9pQs5vW3yZ8bF...</td>
              </tr>
            </tbody>
          </table>
        </div>

        <p className="text-gray-700 mb-4">
          The <strong>secret</strong> is a 32-byte cryptographically random value encoded in Base64 (44 characters).
          It&apos;s generated client-side and shown only once during creation.
        </p>
      </section>

      {/* Creating Payment Keys */}
      <section className="mb-12">
        <AnchorHeading id="creating-keys">Creating Payment Keys</AnchorHeading>

        <p className="text-gray-700 mb-4">
          Creating a Payment Key requires <strong>two transactions</strong>:
        </p>

        <ol className="list-decimal list-inside text-gray-700 space-y-3 mb-6">
          <li>
            <strong>Create the key</strong> - Stores encrypted key data on-chain via <code>store_secrets</code>
          </li>
          <li>
            <strong>Initial deposit</strong> - Transfers stablecoins via <code>ft_transfer_call</code> to fund the key
          </li>
        </ol>

        <AnchorHeading id="via-dashboard" level={3}>Via Dashboard</AnchorHeading>

        <ol className="list-decimal list-inside text-gray-700 space-y-2 mb-6">
          <li>Go to <Link href="/payment-keys" className="text-[var(--primary-orange)] hover:underline">/payment-keys</Link></li>
          <li>Click <strong>&quot;Create Payment Key&quot;</strong></li>
          <li>Configure restrictions (see below)</li>
          <li>Enter initial deposit amount (minimum $1)</li>
          <li>Sign both transactions in your wallet</li>
          <li><strong>Copy the key immediately</strong> - it&apos;s shown only once!</li>
        </ol>

        <div className="bg-red-50 border-l-4 border-red-400 p-4 mb-6">
          <p className="text-sm text-red-800">
            <strong>Critical:</strong> The secret key is displayed only once after creation.
            Copy it immediately and store securely (e.g., password manager, environment variable).
            If lost, you must create a new key.
          </p>
        </div>

        <AnchorHeading id="via-cli" level={3}>Via CLI</AnchorHeading>

        <SyntaxHighlighter language="bash" style={vscDarkPlus} className="rounded-lg mb-4">
          {`# Step 1: Generate random key (32 bytes in Base64)
SECRET_KEY=$(openssl rand -base64 32)
echo "Your secret key: $SECRET_KEY"

# Step 2: Create key on contract (initial_balance = 0)
near call outlayer.near store_secrets '{
  "accessor": {"System": "PaymentKey"},
  "profile": "1",
  "encrypted_data": "<encrypted JSON with key>"
}' --accountId alice.near --depositYocto 1

# Step 3: Top up with stablecoins
near call usdt.tether-token.near ft_transfer_call '{
  "receiver_id": "outlayer.near",
  "amount": "10000000",
  "msg": "{\\"action\\": \\"top_up_payment_key\\", \\"nonce\\": 1}"
}' --accountId alice.near --depositYocto 1`}
        </SyntaxHighlighter>
      </section>

      {/* Key Restrictions */}
      <section className="mb-12">
        <AnchorHeading id="restrictions">Key Restrictions</AnchorHeading>

        <p className="text-gray-700 mb-4">
          Payment Keys can be restricted to limit their capabilities for security:
        </p>

        <AnchorHeading id="project-restrictions" level={3}>Project Restrictions</AnchorHeading>

        <div className="overflow-x-auto mb-6">
          <table className="min-w-full divide-y divide-gray-200">
            <thead className="bg-gray-50">
              <tr>
                <th className="px-4 py-3 text-left text-xs font-medium text-gray-500 uppercase">Option</th>
                <th className="px-4 py-3 text-left text-xs font-medium text-gray-500 uppercase">Description</th>
                <th className="px-4 py-3 text-left text-xs font-medium text-gray-500 uppercase">Use Case</th>
              </tr>
            </thead>
            <tbody className="bg-white divide-y divide-gray-200">
              <tr>
                <td className="px-4 py-3 text-sm font-semibold">Any project</td>
                <td className="px-4 py-3 text-sm text-gray-600">
                  Key works with all OutLayer projects
                </td>
                <td className="px-4 py-3 text-sm text-gray-600">
                  General-purpose API access
                </td>
              </tr>
              <tr>
                <td className="px-4 py-3 text-sm font-semibold">Specific projects</td>
                <td className="px-4 py-3 text-sm text-gray-600">
                  Key only works with selected projects (e.g., <code>alice.near/my-app</code>)
                </td>
                <td className="px-4 py-3 text-sm text-gray-600">
                  Production keys for specific services
                </td>
              </tr>
            </tbody>
          </table>
        </div>

        <SyntaxHighlighter language="json" style={vscDarkPlus} className="rounded-lg mb-4">
          {`// Key restricted to specific projects
{
  "key": "K7xR2mN9pQs5vW3yZ8bF...",
  "initial_balance": "10000000",
  "project_ids": ["alice.near/weather-api", "alice.near/ai-assistant"],
  "max_per_call": "1000000"
}`}
        </SyntaxHighlighter>

        <AnchorHeading id="spending-limits" level={3}>Spending Limits</AnchorHeading>

        <div className="border rounded-lg p-4 mb-6">
          <h4 className="font-semibold text-gray-900 mb-2">max_per_call</h4>
          <p className="text-sm text-gray-600 mb-2">
            Maximum amount that can be spent in a single API call (compute + attached deposit combined).
          </p>
          <p className="text-sm text-gray-600">
            <strong>Example:</strong> <code>max_per_call: &quot;1000000&quot;</code> limits each call to $1.00 maximum.
          </p>
        </div>

        <div className="bg-yellow-50 border-l-4 border-yellow-400 p-4 mb-6">
          <p className="text-sm text-yellow-800">
            <strong>Security tip:</strong> For production keys, always set both project restrictions
            and spending limits to minimize damage if a key is compromised.
          </p>
        </div>
      </section>

      {/* Balance Management */}
      <section className="mb-12">
        <AnchorHeading id="balance">Balance Management</AnchorHeading>

        <AnchorHeading id="checking-balance" level={3}>Checking Balance</AnchorHeading>

        <p className="text-gray-700 mb-4">
          View your key balance on the{' '}
          <Link href="/payment-keys" className="text-[var(--primary-orange)] hover:underline">Payment Keys</Link>
          {' '}dashboard page. The balance shows:
        </p>

        <ul className="list-disc list-inside text-gray-700 space-y-2 mb-6">
          <li><strong>Initial balance</strong> - Total amount deposited</li>
          <li><strong>Spent</strong> - Amount already used</li>
          <li><strong>Available</strong> - Remaining balance (initial - spent)</li>
        </ul>

        <AnchorHeading id="top-up" level={3}>Topping Up Balance</AnchorHeading>

        <p className="text-gray-700 mb-4">
          Add more funds to an existing key:
        </p>

        <ol className="list-decimal list-inside text-gray-700 space-y-2 mb-6">
          <li>Go to <Link href="/payment-keys" className="text-[var(--primary-orange)] hover:underline">/payment-keys</Link></li>
          <li>Find your key and click <strong>&quot;Top Up&quot;</strong></li>
          <li>Enter amount (minimum $1)</li>
          <li>Sign the <code>ft_transfer_call</code> transaction</li>
        </ol>

        <SyntaxHighlighter language="bash" style={vscDarkPlus} className="rounded-lg mb-4">
          {`# Top up via CLI (add $10 USDT to key with nonce 0)
near call usdt.tether-token.near ft_transfer_call '{
  "receiver_id": "outlayer.near",
  "amount": "10000000",
  "msg": "{\\"action\\": \\"top_up_payment_key\\", \\"nonce\\": 1}"
}' --accountId alice.near --depositYocto 1`}
        </SyntaxHighlighter>

        <AnchorHeading id="balance-protection" level={3}>Balance Protection</AnchorHeading>

        <p className="text-gray-700 mb-4">
          OutLayer uses a <strong>reserved balance</strong> mechanism to prevent overdraft attacks:
        </p>

        <ol className="list-decimal list-inside text-gray-700 space-y-2 mb-6">
          <li>When a call starts, the estimated cost is <strong>reserved</strong></li>
          <li>Call is rejected if <code>available - reserved &lt; estimated_cost</code></li>
          <li>After completion, actual cost is charged and reservation is released</li>
        </ol>

        <p className="text-gray-700 mb-4">
          This prevents an attacker from spending $90 with a $1 balance by launching many parallel requests.
        </p>
      </section>

      {/* Data Storage */}
      <section className="mb-12">
        <AnchorHeading id="storage">How Keys are Stored</AnchorHeading>

        <p className="text-gray-700 mb-4">
          Payment Key data is stored as an encrypted secret on the OutLayer contract:
        </p>

        <SyntaxHighlighter language="json" style={vscDarkPlus} className="rounded-lg mb-4">
          {`// Encrypted data structure (decrypted only in TEE)
{
  "key": "K7xR2mN9pQs5vW3yZ8bF...",      // 32-byte secret
  "initial_balance": "10000000",          // Total deposited (micro-units)
  "project_ids": ["alice.near/my-app"],   // Allowed projects ([] = any)
  "max_per_call": "1000000"               // Spending limit per call
}`}
        </SyntaxHighlighter>

        <div className="grid grid-cols-1 md:grid-cols-2 gap-4 mb-6">
          <div className="border rounded-lg p-4">
            <h4 className="font-semibold text-gray-900 mb-2">On-Chain (Contract)</h4>
            <ul className="list-disc list-inside text-sm text-gray-600 space-y-1">
              <li>Encrypted key data</li>
              <li>Owner (NEAR account)</li>
              <li>Nonce (key number)</li>
            </ul>
          </div>
          <div className="border rounded-lg p-4">
            <h4 className="font-semibold text-gray-900 mb-2">Off-Chain (Coordinator)</h4>
            <ul className="list-disc list-inside text-sm text-gray-600 space-y-1">
              <li>Spent amount (running total)</li>
              <li>Reserved amount (in-flight)</li>
              <li>Usage history (audit log)</li>
            </ul>
          </div>
        </div>

        <div className="bg-blue-50 border-l-4 border-blue-400 p-4 mb-6">
          <p className="text-sm text-blue-800">
            <strong>Security:</strong> The secret key is encrypted with TEE keys and can only be
            decrypted inside the Trusted Execution Environment during API validation. The coordinator
            never sees the plaintext key - it only validates hashes.
          </p>
        </div>
      </section>

      {/* Rate Limits */}
      <section className="mb-12">
        <AnchorHeading id="rate-limits">Rate Limits</AnchorHeading>

        <p className="text-gray-700 mb-4">
          To prevent abuse, Payment Keys have rate limits:
        </p>

        <div className="overflow-x-auto mb-6">
          <table className="min-w-full divide-y divide-gray-200">
            <thead className="bg-gray-50">
              <tr>
                <th className="px-4 py-3 text-left text-xs font-medium text-gray-500 uppercase">Limit</th>
                <th className="px-4 py-3 text-left text-xs font-medium text-gray-500 uppercase">Value</th>
                <th className="px-4 py-3 text-left text-xs font-medium text-gray-500 uppercase">Scope</th>
              </tr>
            </thead>
            <tbody className="bg-white divide-y divide-gray-200">
              <tr>
                <td className="px-4 py-3 text-sm">Requests per minute (IP)</td>
                <td className="px-4 py-3 text-sm font-mono">100</td>
                <td className="px-4 py-3 text-sm text-gray-600">Before key validation</td>
              </tr>
              <tr>
                <td className="px-4 py-3 text-sm">Requests per minute (Key)</td>
                <td className="px-4 py-3 text-sm font-mono">1000</td>
                <td className="px-4 py-3 text-sm text-gray-600">After key validation</td>
              </tr>
              <tr>
                <td className="px-4 py-3 text-sm">Concurrent jobs per key</td>
                <td className="px-4 py-3 text-sm font-mono">10</td>
                <td className="px-4 py-3 text-sm text-gray-600">Simultaneous executions</td>
              </tr>
              <tr>
                <td className="px-4 py-3 text-sm">Minimum compute limit</td>
                <td className="px-4 py-3 text-sm font-mono">$0.001</td>
                <td className="px-4 py-3 text-sm text-gray-600">Per request</td>
              </tr>
            </tbody>
          </table>
        </div>

        <p className="text-gray-700 mb-4">
          Rate limit headers are included in responses:
        </p>

        <SyntaxHighlighter language="http" style={vscDarkPlus} className="rounded-lg mb-4">
          {`X-RateLimit-Limit: 1000
X-RateLimit-Remaining: 950
X-RateLimit-Reset: 1704067260`}
        </SyntaxHighlighter>

        <p className="text-gray-700">
          When limits are exceeded, you&apos;ll receive HTTP <code>429 Too Many Requests</code>.
        </p>
      </section>

      {/* Security Best Practices */}
      <section className="mb-12">
        <AnchorHeading id="security">Security Best Practices</AnchorHeading>

        <div className="space-y-4">
          <div className="border-l-4 border-green-400 pl-4">
            <h4 className="font-semibold text-gray-900">1. Restrict to specific projects</h4>
            <p className="text-sm text-gray-600">
              Production keys should only work with the projects they need. Avoid &quot;any project&quot; keys.
            </p>
          </div>

          <div className="border-l-4 border-green-400 pl-4">
            <h4 className="font-semibold text-gray-900">2. Set spending limits</h4>
            <p className="text-sm text-gray-600">
              Always set <code>max_per_call</code> to limit damage from a compromised key.
            </p>
          </div>

          <div className="border-l-4 border-green-400 pl-4">
            <h4 className="font-semibold text-gray-900">3. Use environment variables</h4>
            <p className="text-sm text-gray-600">
              Never hardcode keys in source code. Use environment variables or secret managers.
            </p>
          </div>

          <div className="border-l-4 border-green-400 pl-4">
            <h4 className="font-semibold text-gray-900">4. Rotate keys periodically</h4>
            <p className="text-sm text-gray-600">
              Create new keys and revoke old ones regularly, especially after team changes.
            </p>
          </div>

          <div className="border-l-4 border-green-400 pl-4">
            <h4 className="font-semibold text-gray-900">5. Monitor usage</h4>
            <p className="text-sm text-gray-600">
              Regularly check the <Link href="/payment-keys" className="text-[var(--primary-orange)] hover:underline">Payment Keys</Link>
              {' '}dashboard for unexpected spending patterns.
            </p>
          </div>
        </div>
      </section>

      {/* Related Documentation */}
      <section className="bg-gray-50 rounded-lg p-6">
        <h3 className="text-lg font-semibold text-gray-900 mb-4">Related Documentation</h3>

        <ul className="space-y-2 text-sm">
          <li>
            <Link href="/docs/https-api" className="text-[var(--primary-orange)] hover:underline">HTTPS API</Link>
            {' '}- Full API reference for using Payment Keys
          </li>
          <li>
            <Link href="/docs/earnings" className="text-[var(--primary-orange)] hover:underline">Earnings</Link>
            {' '}- How project authors earn from API calls
          </li>
          <li>
            <Link href="/docs/web2-integration" className="text-[var(--primary-orange)] hover:underline">Web2 Integration</Link>
            {' '}- HTTPS API integration guide
          </li>
          <li>
            <Link href="/docs/pricing" className="text-[var(--primary-orange)] hover:underline">Pricing</Link>
            {' '}- Cost calculation for HTTPS calls
          </li>
        </ul>
      </section>
    </div>
  );
}
