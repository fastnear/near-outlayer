'use client';

import { Prism as SyntaxHighlighter } from 'react-syntax-highlighter';
import { vscDarkPlus } from 'react-syntax-highlighter/dist/esm/styles/prism';
import { AnchorHeading, useHashNavigation } from '../sections/utils';

export default function PaymentChecksPage() {
  useHashNavigation();

  return (
    <div className="max-w-5xl">
      <h1 className="text-4xl font-bold mb-3">Payment Checks</h1>
      <p className="text-gray-600 mb-8">
        Gasless agent-to-agent payments. Agent A locks tokens into a check and sends a single key to Agent B,
        who claims the funds &mdash; no gas, no on-chain account, no private key exchange.
        Supports partial claims, expiry, and reclaim.
      </p>

      {/* Key Value Props */}
      <section className="mb-10">
        <div className="grid grid-cols-1 md:grid-cols-2 gap-4 mb-6">
          <div className="border border-gray-200 rounded-lg p-5">
            <div className="text-2xl mb-2">&#9981;&#65039;</div>
            <h3 className="font-semibold text-gray-900 mb-1">Fully Gasless</h3>
            <p className="text-sm text-gray-600">
              Create, claim, and reclaim &mdash; all operations work without NEAR for gas.
              Uses the NEAR Intents solver relay with off-chain NEP-413 signatures.
              Neither sender nor receiver needs gas tokens.
            </p>
          </div>
          <div className="border border-gray-200 rounded-lg p-5">
            <div className="text-2xl mb-2">&#128273;</div>
            <h3 className="font-semibold text-gray-900 mb-1">Single Key Transfer</h3>
            <p className="text-sm text-gray-600">
              The entire check is represented by a single <code className="text-xs bg-gray-100 px-1 rounded">check_key</code> &mdash;
              a 64-character hex string. Send it over any channel (HTTP, message, QR code).
              Whoever has the key can claim the funds.
            </p>
          </div>
          <div className="border border-gray-200 rounded-lg p-5">
            <div className="text-2xl mb-2">&#128274;</div>
            <h3 className="font-semibold text-gray-900 mb-1">No Private Key Exposure</h3>
            <p className="text-sm text-gray-600">
              The sender&apos;s wallet key never leaves the TEE. Checks use ephemeral keys derived
              deterministically inside the enclave &mdash; the sender can always reclaim funds
              without storing anything locally.
            </p>
          </div>
          <div className="border border-gray-200 rounded-lg p-5">
            <div className="text-2xl mb-2">&#128200;</div>
            <h3 className="font-semibold text-gray-900 mb-1">Partial Claims &amp; Reclaim</h3>
            <p className="text-sm text-gray-600">
              A check can be claimed in parts (e.g., claim 30%, then claim more later).
              The sender can reclaim unclaimed funds at any time. Supports optional expiry.
            </p>
          </div>
        </div>
      </section>

      {/* How It Works */}
      <section id="how-it-works" className="mb-10 scroll-mt-4">
        <AnchorHeading id="how-it-works">How It Works</AnchorHeading>

        {/* Visual flow diagram */}
        <div className="bg-gray-50 rounded-xl border border-gray-200 p-6 mb-6 overflow-x-auto">
          <div className="min-w-[600px]">

            {/* Row 1: Sender and Receiver */}
            <div className="grid grid-cols-[1fr_1fr_1fr] gap-4 items-start">
              <div className="bg-white rounded-lg border-2 border-blue-400 p-3 text-center">
                <div className="text-lg mb-1">&#129302;</div>
                <div className="text-xs font-bold text-blue-700">Agent A (Sender)</div>
                <div className="text-xs text-gray-500 mt-1">Has wallet API key</div>
              </div>
              <div className="flex flex-col items-center justify-center pt-4">
                <div className="text-xs text-gray-500 font-medium">check_key</div>
                <div className="flex items-center gap-1">
                  <div className="h-px w-16 bg-gray-400"></div>
                  <svg width="12" height="12" className="text-gray-400"><path d="M0 6 L8 6 M4 2 L8 6 L4 10" stroke="currentColor" strokeWidth="2" fill="none"/></svg>
                </div>
                <div className="text-[10px] text-gray-400">any channel</div>
              </div>
              <div className="bg-white rounded-lg border-2 border-green-400 p-3 text-center">
                <div className="text-lg mb-1">&#129302;</div>
                <div className="text-xs font-bold text-green-700">Agent B (Receiver)</div>
                <div className="text-xs text-gray-500 mt-1">Has wallet API key</div>
              </div>
            </div>

            {/* Row 2: Arrows to TEE */}
            <div className="grid grid-cols-[1fr_1fr_1fr] gap-4 py-2">
              <div className="flex flex-col items-center">
                <svg width="24" height="28" className="text-blue-400"><path d="M12 0 L12 20 M6 14 L12 20 L18 14" stroke="currentColor" strokeWidth="2" fill="none"/></svg>
                <span className="text-[10px] text-blue-600 font-medium">create check</span>
              </div>
              <div />
              <div className="flex flex-col items-center">
                <svg width="24" height="28" className="text-green-400"><path d="M12 0 L12 20 M6 14 L12 20 L18 14" stroke="currentColor" strokeWidth="2" fill="none"/></svg>
                <span className="text-[10px] text-green-600 font-medium">claim check</span>
              </div>
            </div>

            {/* Row 3: TEE + Ephemeral */}
            <div className="relative border-2 border-green-500 rounded-xl p-4 bg-green-50/50">
              <span className="absolute -top-3 left-1/2 -translate-x-1/2 bg-green-500 text-white text-xs font-bold px-3 py-0.5 rounded-full whitespace-nowrap">TEE (Intel TDX)</span>
              <div className="grid grid-cols-3 gap-4 mt-2">
                <div className="bg-white rounded-lg border border-green-300 p-3 text-center">
                  <div className="text-xs font-semibold text-gray-800">Ephemeral Key</div>
                  <div className="text-xs text-gray-500 mt-1">Derived per check</div>
                  <div className="text-xs text-gray-500">HMAC from master secret</div>
                </div>
                <div className="bg-white rounded-lg border border-green-300 p-3 text-center">
                  <div className="text-xs font-semibold text-gray-800">NEP-413 Signing</div>
                  <div className="text-xs text-gray-500 mt-1">Off-chain intent signatures</div>
                  <div className="text-xs text-gray-500">No gas required</div>
                </div>
                <div className="bg-white rounded-lg border border-green-300 p-3 text-center">
                  <div className="text-xs font-semibold text-gray-800">Reclaim</div>
                  <div className="text-xs text-gray-500 mt-1">Re-derives ephemeral key</div>
                  <div className="text-xs text-gray-500">Sender never stores key</div>
                </div>
              </div>
            </div>

            {/* Row 4: Arrow to solver relay */}
            <div className="flex justify-center py-2">
              <div className="flex flex-col items-center">
                <svg width="24" height="28" className="text-purple-400"><path d="M12 0 L12 20 M6 14 L12 20 L18 14" stroke="currentColor" strokeWidth="2" fill="none"/></svg>
                <span className="text-[10px] text-purple-600 font-medium">publish_intent (gasless)</span>
              </div>
            </div>

            {/* Row 5: Solver Relay + Intents */}
            <div className="grid grid-cols-2 gap-4">
              <div className="bg-white rounded-lg border-2 border-purple-400 p-3 text-center">
                <div className="text-xs font-bold text-purple-700">Solver Relay</div>
                <div className="text-xs text-gray-500 mt-1">Executes signed transfer intents</div>
                <div className="text-xs text-gray-500">No gas from sender or receiver</div>
              </div>
              <div className="bg-white rounded-lg border-2 border-purple-400 p-3 text-center">
                <div className="text-xs font-bold text-purple-700">intents.near</div>
                <div className="text-xs text-gray-500 mt-1">Token balances (multi-token)</div>
                <div className="text-xs text-gray-500">Ephemeral accounts as escrow</div>
              </div>
            </div>

          </div>
        </div>

        <ol className="list-decimal list-inside text-gray-700 space-y-3 mb-4">
          <li>
            <strong>Create</strong> &mdash; Agent A calls <code className="text-xs bg-gray-100 px-1 rounded">POST /payment-check/create</code>.
            The TEE derives a unique ephemeral key, transfers tokens from the wallet to the ephemeral account via solver relay (gasless),
            and returns a <code className="text-xs bg-gray-100 px-1 rounded">check_key</code>.
          </li>
          <li>
            <strong>Share</strong> &mdash; Agent A sends the <code className="text-xs bg-gray-100 px-1 rounded">check_key</code> to Agent B
            over any channel (API call, message, QR code). The key is a 64-char hex string &mdash; compact and easy to transmit.
          </li>
          <li>
            <strong>Claim</strong> &mdash; Agent B calls <code className="text-xs bg-gray-100 px-1 rounded">POST /payment-check/claim</code> with the key.
            The coordinator signs a transfer intent from the ephemeral account to Agent B&apos;s wallet and submits it via solver relay. Agent B receives the tokens gaslessly.
          </li>
          <li>
            <strong>Reclaim</strong> (optional) &mdash; Agent A can reclaim any unclaimed funds at any time.
            The TEE re-derives the ephemeral key (no need to store it) and signs a transfer back to Agent A.
          </li>
        </ol>
      </section>

      {/* Transfer Mechanism */}
      <section id="mechanism" className="mb-10 scroll-mt-4">
        <AnchorHeading id="mechanism">Transfer Mechanism</AnchorHeading>

        <p className="text-gray-700 mb-4">
          All payment check operations use the <strong>NEAR Intents solver relay</strong> &mdash; a gasless off-chain transfer protocol.
          Instead of submitting on-chain transactions (which require NEAR for gas), the coordinator signs NEP-413 messages
          and submits them to the solver relay, which executes the transfer on <code className="text-xs bg-gray-100 px-1 rounded">intents.near</code>.
        </p>

        <div className="overflow-x-auto mb-6">
          <table className="min-w-full text-sm border border-gray-200">
            <thead className="bg-gray-50">
              <tr>
                <th className="px-4 py-2 text-left font-semibold border-b">Operation</th>
                <th className="px-4 py-2 text-left font-semibold border-b">From</th>
                <th className="px-4 py-2 text-left font-semibold border-b">To</th>
                <th className="px-4 py-2 text-left font-semibold border-b">Who Signs</th>
                <th className="px-4 py-2 text-left font-semibold border-b">Gas Required</th>
              </tr>
            </thead>
            <tbody>
              <tr className="border-b">
                <td className="px-4 py-2 font-medium">Create</td>
                <td className="px-4 py-2">Wallet</td>
                <td className="px-4 py-2">Ephemeral</td>
                <td className="px-4 py-2">Wallet key (TEE keystore)</td>
                <td className="px-4 py-2 text-green-600 font-medium">None</td>
              </tr>
              <tr className="border-b">
                <td className="px-4 py-2 font-medium">Claim</td>
                <td className="px-4 py-2">Ephemeral</td>
                <td className="px-4 py-2">Claimer wallet</td>
                <td className="px-4 py-2">Ephemeral key (from check_key)</td>
                <td className="px-4 py-2 text-green-600 font-medium">None</td>
              </tr>
              <tr className="border-b">
                <td className="px-4 py-2 font-medium">Reclaim</td>
                <td className="px-4 py-2">Ephemeral</td>
                <td className="px-4 py-2">Creator wallet</td>
                <td className="px-4 py-2">Ephemeral key (TEE re-derivation)</td>
                <td className="px-4 py-2 text-green-600 font-medium">None</td>
              </tr>
            </tbody>
          </table>
        </div>

        <div className="bg-blue-50 border-l-4 border-blue-500 p-4 mb-4">
          <p className="text-sm text-gray-700">
            <strong>Why ephemeral accounts?</strong> Each check gets its own ephemeral account on <code className="text-xs bg-gray-100 px-1 rounded">intents.near</code> &mdash;
            derived from the wallet&apos;s master secret + a monotonic counter. This gives each check an isolated balance
            that can only be moved by whoever holds the check_key (claim) or by the TEE re-deriving the key (reclaim).
            Think of it as a single-use escrow address.
          </p>
        </div>
      </section>

      {/* Key Derivation */}
      <section id="key-derivation" className="mb-10 scroll-mt-4">
        <AnchorHeading id="key-derivation">Key Derivation</AnchorHeading>

        <p className="text-gray-700 mb-4">
          Ephemeral keys are derived deterministically inside the TEE using HMAC-SHA256 from the master secret.
          The derivation path includes the wallet ID and a monotonic counter, ensuring each check gets a unique, non-reusable key.
        </p>

        <SyntaxHighlighter language="text" style={vscDarkPlus} className="rounded-lg text-sm mb-4">
{`Derivation hierarchy:
  wallet:{id}:near                        <- main wallet key
  wallet:{id}:near:check:{counter}        <- ephemeral key per check

The check_key IS the raw ed25519 private key of the ephemeral account.
The ephemeral account ID = hex(public_key) on intents.near.`}
        </SyntaxHighlighter>

        <p className="text-gray-700 mb-4">
          Because the derivation is deterministic, the sender never needs to store the ephemeral key.
          For <strong>reclaim</strong>, the TEE re-derives it from the same path. For <strong>claim</strong>,
          the receiver uses the check_key directly to sign the transfer intent.
        </p>
      </section>

      {/* Use Cases */}
      <section id="use-cases" className="mb-10 scroll-mt-4">
        <AnchorHeading id="use-cases">Use Cases</AnchorHeading>

        <div className="space-y-4">
          <div className="border border-gray-200 rounded-lg p-4">
            <h3 className="font-semibold text-gray-900 mb-2">Agent-to-Agent Payments</h3>
            <p className="text-sm text-gray-600">
              Agent A needs data from Agent B. A creates a check, sends the key as part of the API request.
              B validates the check (peek), performs the work, claims the payment. If B doesn&apos;t deliver, A reclaims.
            </p>
          </div>

          <div className="border border-gray-200 rounded-lg p-4">
            <h3 className="font-semibold text-gray-900 mb-2">Bounties &amp; Task Rewards</h3>
            <p className="text-sm text-gray-600">
              Create a check with an expiry. Share the check_key with whoever completes the task.
              Partial claims allow splitting rewards among multiple contributors.
              Unclaimed funds auto-expire and can be reclaimed.
            </p>
          </div>

          <div className="border border-gray-200 rounded-lg p-4">
            <h3 className="font-semibold text-gray-900 mb-2">Escrow-Style Payments</h3>
            <p className="text-sm text-gray-600">
              Lock funds in a check. Share the key only when conditions are met (off-chain verification, delivery confirmation).
              The check acts as a lightweight escrow without a smart contract.
            </p>
          </div>

          <div className="border border-gray-200 rounded-lg p-4">
            <h3 className="font-semibold text-gray-900 mb-2">Batch Payouts</h3>
            <p className="text-sm text-gray-600">
              Use <code className="text-xs bg-gray-100 px-1 rounded">batch-create</code> to generate up to 10 checks in a single call.
              Each check gets its own key and can be distributed independently.
            </p>
          </div>
        </div>
      </section>

      {/* Quick Start */}
      <section id="quick-start" className="mb-10 scroll-mt-4">
        <AnchorHeading id="quick-start">Quick Start</AnchorHeading>

        <p className="text-gray-700 mb-4">All examples use the OutLayer API at <code className="text-xs bg-gray-100 px-1 rounded">https://api.outlayer.fastnear.com</code>.</p>

        <h3 className="font-semibold text-gray-900 mb-3">1. Create a check</h3>
        <SyntaxHighlighter language="bash" style={vscDarkPlus} className="rounded-lg text-sm mb-4">
{`curl -X POST https://api.outlayer.fastnear.com/wallet/v1/payment-check/create \\
  -H "Authorization: Bearer wk_YOUR_API_KEY" \\
  -H "Content-Type: application/json" \\
  -d '{
    "token": "17208628f84f5d6ad33f0da3bbbeb27ffcb398eac501a31bd6ad2011e36133a1",
    "amount": "1000000",
    "memo": "Payment for data analysis",
    "expires_in": 3600
  }'`}
        </SyntaxHighlighter>

        <SyntaxHighlighter language="json" style={vscDarkPlus} className="rounded-lg text-sm mb-6">
{`{
  "check_id": "a1b2c3d4-...",
  "check_key": "7f3a9b2c...64 hex chars...",
  "token": "17208628f84f5d6ad33f0da3bbbeb27ffcb398eac501a31bd6ad2011e36133a1",
  "amount": "1000000",
  "memo": "Payment for data analysis",
  "created_at": "2026-03-13T10:00:00Z",
  "expires_at": "2026-03-13T11:00:00Z"
}`}
        </SyntaxHighlighter>

        <h3 className="font-semibold text-gray-900 mb-3">2. Share the check_key with the receiver</h3>
        <p className="text-gray-700 mb-6">
          Send <code className="text-xs bg-gray-100 px-1 rounded">check_key</code> to the other agent via any channel.
          The key is all they need to claim the funds.
        </p>

        <h3 className="font-semibold text-gray-900 mb-3">3. Receiver claims the check</h3>
        <SyntaxHighlighter language="bash" style={vscDarkPlus} className="rounded-lg text-sm mb-4">
{`# Full claim (all funds)
curl -X POST https://api.outlayer.fastnear.com/wallet/v1/payment-check/claim \\
  -H "Authorization: Bearer wk_RECEIVER_API_KEY" \\
  -H "Content-Type: application/json" \\
  -d '{"check_key": "7f3a9b2c...64 hex chars..."}'

# Partial claim (specific amount)
curl -X POST https://api.outlayer.fastnear.com/wallet/v1/payment-check/claim \\
  -H "Authorization: Bearer wk_RECEIVER_API_KEY" \\
  -H "Content-Type: application/json" \\
  -d '{"check_key": "7f3a9b2c...", "amount": "500000"}'`}
        </SyntaxHighlighter>

        <SyntaxHighlighter language="json" style={vscDarkPlus} className="rounded-lg text-sm mb-6">
{`{
  "token": "17208628f84f5d6ad33f0da3bbbeb27ffcb398eac501a31bd6ad2011e36133a1",
  "amount_claimed": "500000",
  "remaining": "500000",
  "memo": "Payment for data analysis",
  "claimed_at": "2026-03-13T10:05:00Z",
  "intent_hash": "Bx7k..."
}`}
        </SyntaxHighlighter>

        <h3 className="font-semibold text-gray-900 mb-3">4. Sender reclaims unclaimed funds (optional)</h3>
        <SyntaxHighlighter language="bash" style={vscDarkPlus} className="rounded-lg text-sm mb-4">
{`curl -X POST https://api.outlayer.fastnear.com/wallet/v1/payment-check/reclaim \\
  -H "Authorization: Bearer wk_SENDER_API_KEY" \\
  -H "Content-Type: application/json" \\
  -d '{"check_id": "a1b2c3d4-..."}'`}
        </SyntaxHighlighter>

        <SyntaxHighlighter language="json" style={vscDarkPlus} className="rounded-lg text-sm mb-6">
{`{
  "token": "17208628f84f5d6ad33f0da3bbbeb27ffcb398eac501a31bd6ad2011e36133a1",
  "amount_reclaimed": "500000",
  "remaining": "0",
  "reclaimed_at": "2026-03-13T10:10:00Z",
  "intent_hash": "Cx9m..."
}`}
        </SyntaxHighlighter>
      </section>

      {/* API Reference */}
      <section id="api-reference" className="mb-10 scroll-mt-4">
        <AnchorHeading id="api-reference">API Reference</AnchorHeading>

        <p className="text-gray-700 mb-4">
          Base URL: <code className="text-xs bg-gray-100 px-1 rounded">https://api.outlayer.fastnear.com/wallet/v1/payment-check</code>.
          All endpoints require wallet API key authentication via <code className="text-xs bg-gray-100 px-1 rounded">Authorization: Bearer wk_...</code> header.
        </p>

        {/* POST /create */}
        <div className="border border-gray-200 rounded-lg mb-4">
          <div className="bg-gray-50 px-4 py-2 border-b border-gray-200 flex items-center gap-2">
            <span className="text-xs font-bold text-white bg-green-600 px-2 py-0.5 rounded">POST</span>
            <code className="text-sm font-mono">/create</code>
          </div>
          <div className="p-4">
            <p className="text-sm text-gray-700 mb-3">Create a new payment check. Locks tokens from the sender&apos;s intents balance into an ephemeral account.</p>
            <p className="text-xs font-semibold text-gray-500 mb-1">Request body</p>
            <div className="overflow-x-auto mb-3">
              <table className="min-w-full text-xs border border-gray-200">
                <thead className="bg-gray-50">
                  <tr>
                    <th className="px-3 py-1.5 text-left font-semibold border-b">Field</th>
                    <th className="px-3 py-1.5 text-left font-semibold border-b">Type</th>
                    <th className="px-3 py-1.5 text-left font-semibold border-b">Required</th>
                    <th className="px-3 py-1.5 text-left font-semibold border-b">Description</th>
                  </tr>
                </thead>
                <tbody>
                  <tr className="border-b"><td className="px-3 py-1.5 font-mono">token</td><td className="px-3 py-1.5">string</td><td className="px-3 py-1.5">yes</td><td className="px-3 py-1.5">Token contract ID (e.g., USDC contract address)</td></tr>
                  <tr className="border-b"><td className="px-3 py-1.5 font-mono">amount</td><td className="px-3 py-1.5">string</td><td className="px-3 py-1.5">yes</td><td className="px-3 py-1.5">Amount in smallest units (e.g., &quot;1000000&quot; = 1 USDC)</td></tr>
                  <tr className="border-b"><td className="px-3 py-1.5 font-mono">memo</td><td className="px-3 py-1.5">string</td><td className="px-3 py-1.5">no</td><td className="px-3 py-1.5">Optional memo (max 256 chars), visible to receiver</td></tr>
                  <tr className="border-b"><td className="px-3 py-1.5 font-mono">expires_in</td><td className="px-3 py-1.5">number</td><td className="px-3 py-1.5">no</td><td className="px-3 py-1.5">Expiry in seconds from now (e.g., 3600 = 1 hour)</td></tr>
                </tbody>
              </table>
            </div>
            <p className="text-xs font-semibold text-gray-500 mb-1">Response</p>
            <p className="text-xs text-gray-600">Returns <code className="bg-gray-100 px-1 rounded">check_id</code>, <code className="bg-gray-100 px-1 rounded">check_key</code>, <code className="bg-gray-100 px-1 rounded">token</code>, <code className="bg-gray-100 px-1 rounded">amount</code>, <code className="bg-gray-100 px-1 rounded">memo</code>, <code className="bg-gray-100 px-1 rounded">created_at</code>, <code className="bg-gray-100 px-1 rounded">expires_at</code>.</p>
          </div>
        </div>

        {/* POST /batch-create */}
        <div className="border border-gray-200 rounded-lg mb-4">
          <div className="bg-gray-50 px-4 py-2 border-b border-gray-200 flex items-center gap-2">
            <span className="text-xs font-bold text-white bg-green-600 px-2 py-0.5 rounded">POST</span>
            <code className="text-sm font-mono">/batch-create</code>
          </div>
          <div className="p-4">
            <p className="text-sm text-gray-700 mb-3">Create multiple checks in a single call (max 10). Each check gets independent key and ephemeral account.</p>
            <p className="text-xs font-semibold text-gray-500 mb-1">Request body</p>
            <div className="overflow-x-auto mb-3">
              <table className="min-w-full text-xs border border-gray-200">
                <thead className="bg-gray-50">
                  <tr>
                    <th className="px-3 py-1.5 text-left font-semibold border-b">Field</th>
                    <th className="px-3 py-1.5 text-left font-semibold border-b">Type</th>
                    <th className="px-3 py-1.5 text-left font-semibold border-b">Description</th>
                  </tr>
                </thead>
                <tbody>
                  <tr className="border-b"><td className="px-3 py-1.5 font-mono">checks</td><td className="px-3 py-1.5">array</td><td className="px-3 py-1.5">Array of create requests (same fields as /create), max 10</td></tr>
                </tbody>
              </table>
            </div>
            <p className="text-xs font-semibold text-gray-500 mb-1">Response</p>
            <p className="text-xs text-gray-600">Returns <code className="bg-gray-100 px-1 rounded">{`{ checks: [...] }`}</code> &mdash; array of create responses.</p>
          </div>
        </div>

        {/* POST /claim */}
        <div className="border border-gray-200 rounded-lg mb-4">
          <div className="bg-gray-50 px-4 py-2 border-b border-gray-200 flex items-center gap-2">
            <span className="text-xs font-bold text-white bg-green-600 px-2 py-0.5 rounded">POST</span>
            <code className="text-sm font-mono">/claim</code>
          </div>
          <div className="p-4">
            <p className="text-sm text-gray-700 mb-3">Claim funds from a check. Transfers from ephemeral account to the caller&apos;s wallet. Supports partial claims.</p>
            <p className="text-xs font-semibold text-gray-500 mb-1">Request body</p>
            <div className="overflow-x-auto mb-3">
              <table className="min-w-full text-xs border border-gray-200">
                <thead className="bg-gray-50">
                  <tr>
                    <th className="px-3 py-1.5 text-left font-semibold border-b">Field</th>
                    <th className="px-3 py-1.5 text-left font-semibold border-b">Type</th>
                    <th className="px-3 py-1.5 text-left font-semibold border-b">Required</th>
                    <th className="px-3 py-1.5 text-left font-semibold border-b">Description</th>
                  </tr>
                </thead>
                <tbody>
                  <tr className="border-b"><td className="px-3 py-1.5 font-mono">check_key</td><td className="px-3 py-1.5">string</td><td className="px-3 py-1.5">yes</td><td className="px-3 py-1.5">The 64-char hex key received from the sender</td></tr>
                  <tr className="border-b"><td className="px-3 py-1.5 font-mono">amount</td><td className="px-3 py-1.5">string</td><td className="px-3 py-1.5">no</td><td className="px-3 py-1.5">Partial claim amount (omit for full claim)</td></tr>
                </tbody>
              </table>
            </div>
            <p className="text-xs font-semibold text-gray-500 mb-1">Response</p>
            <p className="text-xs text-gray-600">Returns <code className="bg-gray-100 px-1 rounded">token</code>, <code className="bg-gray-100 px-1 rounded">amount_claimed</code>, <code className="bg-gray-100 px-1 rounded">remaining</code>, <code className="bg-gray-100 px-1 rounded">memo</code>, <code className="bg-gray-100 px-1 rounded">claimed_at</code>, <code className="bg-gray-100 px-1 rounded">intent_hash</code>.</p>
          </div>
        </div>

        {/* POST /reclaim */}
        <div className="border border-gray-200 rounded-lg mb-4">
          <div className="bg-gray-50 px-4 py-2 border-b border-gray-200 flex items-center gap-2">
            <span className="text-xs font-bold text-white bg-green-600 px-2 py-0.5 rounded">POST</span>
            <code className="text-sm font-mono">/reclaim</code>
          </div>
          <div className="p-4">
            <p className="text-sm text-gray-700 mb-3">Reclaim unclaimed funds back to the sender&apos;s wallet. Only the original creator can reclaim. Supports partial reclaim.</p>
            <p className="text-xs font-semibold text-gray-500 mb-1">Request body</p>
            <div className="overflow-x-auto mb-3">
              <table className="min-w-full text-xs border border-gray-200">
                <thead className="bg-gray-50">
                  <tr>
                    <th className="px-3 py-1.5 text-left font-semibold border-b">Field</th>
                    <th className="px-3 py-1.5 text-left font-semibold border-b">Type</th>
                    <th className="px-3 py-1.5 text-left font-semibold border-b">Required</th>
                    <th className="px-3 py-1.5 text-left font-semibold border-b">Description</th>
                  </tr>
                </thead>
                <tbody>
                  <tr className="border-b"><td className="px-3 py-1.5 font-mono">check_id</td><td className="px-3 py-1.5">string</td><td className="px-3 py-1.5">yes</td><td className="px-3 py-1.5">The check ID returned from create</td></tr>
                  <tr className="border-b"><td className="px-3 py-1.5 font-mono">amount</td><td className="px-3 py-1.5">string</td><td className="px-3 py-1.5">no</td><td className="px-3 py-1.5">Partial reclaim amount (omit for full reclaim)</td></tr>
                </tbody>
              </table>
            </div>
            <p className="text-xs font-semibold text-gray-500 mb-1">Response</p>
            <p className="text-xs text-gray-600">Returns <code className="bg-gray-100 px-1 rounded">token</code>, <code className="bg-gray-100 px-1 rounded">amount_reclaimed</code>, <code className="bg-gray-100 px-1 rounded">remaining</code>, <code className="bg-gray-100 px-1 rounded">reclaimed_at</code>, <code className="bg-gray-100 px-1 rounded">intent_hash</code>.</p>
          </div>
        </div>

        {/* GET /status */}
        <div className="border border-gray-200 rounded-lg mb-4">
          <div className="bg-gray-50 px-4 py-2 border-b border-gray-200 flex items-center gap-2">
            <span className="text-xs font-bold text-white bg-blue-600 px-2 py-0.5 rounded">GET</span>
            <code className="text-sm font-mono">/status?check_id=...</code>
          </div>
          <div className="p-4">
            <p className="text-sm text-gray-700 mb-3">Get the current status of a check. Only the creator can query by check_id.</p>
            <p className="text-xs font-semibold text-gray-500 mb-1">Query parameters</p>
            <div className="overflow-x-auto mb-3">
              <table className="min-w-full text-xs border border-gray-200">
                <thead className="bg-gray-50">
                  <tr>
                    <th className="px-3 py-1.5 text-left font-semibold border-b">Param</th>
                    <th className="px-3 py-1.5 text-left font-semibold border-b">Description</th>
                  </tr>
                </thead>
                <tbody>
                  <tr className="border-b"><td className="px-3 py-1.5 font-mono">check_id</td><td className="px-3 py-1.5">The check ID to query</td></tr>
                </tbody>
              </table>
            </div>
            <p className="text-xs font-semibold text-gray-500 mb-1">Response</p>
            <p className="text-xs text-gray-600">Returns full check details: <code className="bg-gray-100 px-1 rounded">check_id</code>, <code className="bg-gray-100 px-1 rounded">token</code>, <code className="bg-gray-100 px-1 rounded">amount</code>, <code className="bg-gray-100 px-1 rounded">claimed_amount</code>, <code className="bg-gray-100 px-1 rounded">reclaimed_amount</code>, <code className="bg-gray-100 px-1 rounded">status</code>, <code className="bg-gray-100 px-1 rounded">memo</code>, <code className="bg-gray-100 px-1 rounded">created_at</code>, <code className="bg-gray-100 px-1 rounded">expires_at</code>, <code className="bg-gray-100 px-1 rounded">claimed_at</code>, <code className="bg-gray-100 px-1 rounded">claimed_by</code>.</p>
          </div>
        </div>

        {/* GET /list */}
        <div className="border border-gray-200 rounded-lg mb-4">
          <div className="bg-gray-50 px-4 py-2 border-b border-gray-200 flex items-center gap-2">
            <span className="text-xs font-bold text-white bg-blue-600 px-2 py-0.5 rounded">GET</span>
            <code className="text-sm font-mono">/list?status=...&amp;limit=...&amp;offset=...</code>
          </div>
          <div className="p-4">
            <p className="text-sm text-gray-700 mb-3">List all checks created by the authenticated wallet. Supports filtering and pagination.</p>
            <p className="text-xs font-semibold text-gray-500 mb-1">Query parameters</p>
            <div className="overflow-x-auto mb-3">
              <table className="min-w-full text-xs border border-gray-200">
                <thead className="bg-gray-50">
                  <tr>
                    <th className="px-3 py-1.5 text-left font-semibold border-b">Param</th>
                    <th className="px-3 py-1.5 text-left font-semibold border-b">Required</th>
                    <th className="px-3 py-1.5 text-left font-semibold border-b">Description</th>
                  </tr>
                </thead>
                <tbody>
                  <tr className="border-b"><td className="px-3 py-1.5 font-mono">status</td><td className="px-3 py-1.5">no</td><td className="px-3 py-1.5">Filter: unclaimed, claimed, reclaimed, partially_claimed</td></tr>
                  <tr className="border-b"><td className="px-3 py-1.5 font-mono">limit</td><td className="px-3 py-1.5">no</td><td className="px-3 py-1.5">Max results (default 50, max 100)</td></tr>
                  <tr className="border-b"><td className="px-3 py-1.5 font-mono">offset</td><td className="px-3 py-1.5">no</td><td className="px-3 py-1.5">Pagination offset (default 0)</td></tr>
                </tbody>
              </table>
            </div>
            <p className="text-xs font-semibold text-gray-500 mb-1">Response</p>
            <p className="text-xs text-gray-600">Returns <code className="bg-gray-100 px-1 rounded">{`{ checks: [...] }`}</code> &mdash; array of status responses.</p>
          </div>
        </div>

        {/* POST /peek */}
        <div className="border border-gray-200 rounded-lg mb-4">
          <div className="bg-gray-50 px-4 py-2 border-b border-gray-200 flex items-center gap-2">
            <span className="text-xs font-bold text-white bg-green-600 px-2 py-0.5 rounded">POST</span>
            <code className="text-sm font-mono">/peek</code>
          </div>
          <div className="p-4">
            <p className="text-sm text-gray-700 mb-3">Check a payment check&apos;s balance and status using the check_key. Use this to verify a check before claiming.</p>
            <p className="text-xs font-semibold text-gray-500 mb-1">Request body</p>
            <div className="overflow-x-auto mb-3">
              <table className="min-w-full text-xs border border-gray-200">
                <thead className="bg-gray-50">
                  <tr>
                    <th className="px-3 py-1.5 text-left font-semibold border-b">Field</th>
                    <th className="px-3 py-1.5 text-left font-semibold border-b">Type</th>
                    <th className="px-3 py-1.5 text-left font-semibold border-b">Description</th>
                  </tr>
                </thead>
                <tbody>
                  <tr className="border-b"><td className="px-3 py-1.5 font-mono">check_key</td><td className="px-3 py-1.5">string</td><td className="px-3 py-1.5">The 64-char hex check key</td></tr>
                </tbody>
              </table>
            </div>
            <p className="text-xs font-semibold text-gray-500 mb-1">Response</p>
            <p className="text-xs text-gray-600">Returns <code className="bg-gray-100 px-1 rounded">token</code>, <code className="bg-gray-100 px-1 rounded">balance</code> (on-chain), <code className="bg-gray-100 px-1 rounded">memo</code>, <code className="bg-gray-100 px-1 rounded">status</code>, <code className="bg-gray-100 px-1 rounded">expires_at</code>.</p>
          </div>
        </div>
      </section>

      {/* Check Lifecycle */}
      <section id="lifecycle" className="mb-10 scroll-mt-4">
        <AnchorHeading id="lifecycle">Check Lifecycle</AnchorHeading>

        <div className="bg-gray-50 rounded-xl border border-gray-200 p-6 mb-4">
          <SyntaxHighlighter language="text" style={vscDarkPlus} className="rounded-lg text-sm">
{`                    create
                      |
                      v
                  [unclaimed]
                   /      \\
          claim   /        \\  reclaim
                 v          v
       [partially_claimed] [partially_reclaimed]
              |       \\         /       |
       claim  |        \\       /        | reclaim
              v         v     v         v
          [claimed]   (mixed)    [reclaimed]

Statuses:
  unclaimed           - funds locked, waiting to be claimed
  partially_claimed   - some funds claimed, rest available
  partially_reclaimed - some funds reclaimed by sender
  claimed             - all funds claimed by receiver
  reclaimed           - all funds reclaimed by sender`}
          </SyntaxHighlighter>
        </div>

        <div className="bg-yellow-50 border-l-4 border-yellow-500 p-4 mb-4">
          <p className="text-sm text-gray-700">
            <strong>Expiry:</strong> If <code className="text-xs bg-gray-100 px-1 rounded">expires_in</code> is set, the check cannot be claimed after expiry.
            However, funds remain in the ephemeral account &mdash; the sender must explicitly reclaim them.
            Expiry prevents new claims but does not auto-return funds.
          </p>
        </div>
      </section>

      {/* Security Model */}
      <section id="security" className="mb-10 scroll-mt-4">
        <AnchorHeading id="security">Security Model</AnchorHeading>

        <div className="overflow-x-auto mb-4">
          <table className="min-w-full text-sm border border-gray-200">
            <thead className="bg-gray-50">
              <tr>
                <th className="px-4 py-2 text-left font-semibold border-b">Threat</th>
                <th className="px-4 py-2 text-left font-semibold border-b">Mitigation</th>
              </tr>
            </thead>
            <tbody>
              <tr className="border-b">
                <td className="px-4 py-2">check_key intercepted in transit</td>
                <td className="px-4 py-2">Use encrypted channels (HTTPS, E2E encrypted messaging). A leaked key lets anyone with a wallet API key claim the funds.</td>
              </tr>
              <tr className="border-b">
                <td className="px-4 py-2">Sender&apos;s wallet compromised</td>
                <td className="px-4 py-2">Wallet private key never leaves TEE. API key can be revoked. Policy engine limits exposure.</td>
              </tr>
              <tr className="border-b">
                <td className="px-4 py-2">Replay of claim/reclaim</td>
                <td className="px-4 py-2">Each intent has a unique nonce and 5-minute deadline. Solver relay rejects duplicates. DB tracks claimed amounts atomically.</td>
              </tr>
              <tr className="border-b">
                <td className="px-4 py-2">Ephemeral key derivation collision</td>
                <td className="px-4 py-2">Monotonic counter per wallet (DB enforced, atomic increment). Same wallet + counter = same key, but counter never reuses.</td>
              </tr>
              <tr className="border-b">
                <td className="px-4 py-2">Funds stuck in ephemeral account</td>
                <td className="px-4 py-2">Sender can always reclaim &mdash; TEE re-derives the ephemeral key from the same deterministic path. No key storage needed.</td>
              </tr>
            </tbody>
          </table>
        </div>

        <div className="bg-green-50 border-l-4 border-green-500 p-4">
          <p className="text-sm text-gray-700">
            <strong>Key insight:</strong> The check_key is the <em>only</em> secret. It never touches the blockchain, it never enters the TEE
            (for claim &mdash; the coordinator signs locally with it). The sender doesn&apos;t even need to store it &mdash;
            the TEE can re-derive it for reclaim. This makes the system self-healing: even if the sender loses all local state,
            their funds are recoverable through the TEE.
          </p>
        </div>
      </section>

      {/* Comparison */}
      <section id="comparison" className="mb-10 scroll-mt-4">
        <AnchorHeading id="comparison">Comparison with Alternatives</AnchorHeading>

        <div className="overflow-x-auto mb-4">
          <table className="min-w-full text-sm border border-gray-200">
            <thead className="bg-gray-50">
              <tr>
                <th className="px-4 py-2 text-left font-semibold border-b"></th>
                <th className="px-4 py-2 text-left font-semibold border-b">Payment Checks</th>
                <th className="px-4 py-2 text-left font-semibold border-b">Direct Transfer</th>
                <th className="px-4 py-2 text-left font-semibold border-b">Smart Contract Escrow</th>
              </tr>
            </thead>
            <tbody>
              <tr className="border-b">
                <td className="px-4 py-2 font-medium">Gas required</td>
                <td className="px-4 py-2 text-green-600">None</td>
                <td className="px-4 py-2 text-red-600">Sender pays</td>
                <td className="px-4 py-2 text-red-600">Both parties pay</td>
              </tr>
              <tr className="border-b">
                <td className="px-4 py-2 font-medium">Receiver needs account</td>
                <td className="px-4 py-2 text-green-600">Only a wallet API key</td>
                <td className="px-4 py-2 text-red-600">On-chain account + gas</td>
                <td className="px-4 py-2 text-red-600">On-chain account + gas</td>
              </tr>
              <tr className="border-b">
                <td className="px-4 py-2 font-medium">Partial payment</td>
                <td className="px-4 py-2 text-green-600">Built-in</td>
                <td className="px-4 py-2 text-red-600">N/A</td>
                <td className="px-4 py-2 text-yellow-600">Custom logic</td>
              </tr>
              <tr className="border-b">
                <td className="px-4 py-2 font-medium">Reclaim</td>
                <td className="px-4 py-2 text-green-600">Built-in, gasless</td>
                <td className="px-4 py-2 text-red-600">N/A (irreversible)</td>
                <td className="px-4 py-2 text-yellow-600">Custom logic + gas</td>
              </tr>
              <tr className="border-b">
                <td className="px-4 py-2 font-medium">On-chain footprint</td>
                <td className="px-4 py-2 text-green-600">Solver relay only</td>
                <td className="px-4 py-2 text-yellow-600">1 transaction</td>
                <td className="px-4 py-2 text-red-600">Contract deployment + calls</td>
              </tr>
              <tr className="border-b">
                <td className="px-4 py-2 font-medium">Setup complexity</td>
                <td className="px-4 py-2 text-green-600">One API call</td>
                <td className="px-4 py-2 text-green-600">One API call</td>
                <td className="px-4 py-2 text-red-600">Contract dev + deployment</td>
              </tr>
            </tbody>
          </table>
        </div>
      </section>

    </div>
  );
}
