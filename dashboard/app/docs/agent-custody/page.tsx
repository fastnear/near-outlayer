'use client';

import Link from 'next/link';
import { Prism as SyntaxHighlighter } from 'react-syntax-highlighter';
import { vscDarkPlus } from 'react-syntax-highlighter/dist/esm/styles/prism';
import { AnchorHeading, useHashNavigation } from '../sections/utils';

export default function AgentCustodyPage() {
  useHashNavigation();

  return (
    <div className="max-w-5xl">
      <h1 className="text-4xl font-bold mb-3">Agent Custody</h1>
      <p className="text-gray-600 mb-8">
        Institutional-grade custody for AI agents. Give your agent a multi-chain wallet with full policy controls &mdash;
        the agent can send and receive tokens, but it physically cannot lose or leak the private key because it never has one.
        You stay in control of how your agent spends crypto: set spending limits, whitelists, require manual confirmations for large transactions, or freeze the wallet instantly.
      </p>

      {/* Key Value Props */}
      <section className="mb-10">
        <div className="grid grid-cols-1 md:grid-cols-2 gap-4 mb-6">
          <div className="border border-gray-200 rounded-lg p-5">
            <div className="text-2xl mb-2">&#128274;</div>
            <h3 className="font-semibold text-gray-900 mb-1">No Private Key Exposure</h3>
            <p className="text-sm text-gray-600">The agent never sees or stores a private key. It authenticates with an API key. All transaction signing happens inside a TEE (Trusted Execution Environment) &mdash; even if the agent is fully compromised, the key stays safe.</p>
          </div>
          <div className="border border-gray-200 rounded-lg p-5">
            <div className="text-2xl mb-2">&#127760;</div>
            <h3 className="font-semibold text-gray-900 mb-1">Multi-Chain Addresses</h3>
            <p className="text-sm text-gray-600">One wallet &mdash; addresses on NEAR, Ethereum, Bitcoin, Solana, and other chains. Cross-chain transfers happen via the <strong>NEAR Intents</strong> protocol. No bridges, no wrapping.</p>
          </div>
          <div className="border border-gray-200 rounded-lg p-5">
            <div className="text-2xl mb-2">&#128220;</div>
            <h3 className="font-semibold text-gray-900 mb-1">Policy Engine</h3>
            <p className="text-sm text-gray-600">Per-transaction limits, daily/hourly/monthly caps, address whitelists, time restrictions, rate limits, and multisig approval. All policy checks happen inside the TEE &mdash; the agent cannot bypass or tamper with them.</p>
          </div>
          <div className="border border-gray-200 rounded-lg p-5">
            <div className="text-2xl mb-2">&#9981;&#65039;</div>
            <h3 className="font-semibold text-gray-900 mb-1">Gasless Transactions</h3>
            <p className="text-sm text-gray-600">Swaps and cross-chain transfers work without the agent holding gas tokens on the destination chain. The NEAR Intents protocol handles gas abstraction.</p>
          </div>
        </div>

        <div className="bg-green-50 border-l-4 border-green-500 p-4 mb-6">
          <p className="text-sm text-gray-700">
            <strong>Security foundation:</strong> The keystore that holds wallet keys runs inside Intel TDX enclaves (TEE).
            Its master secret is derived from the <strong>NEAR Protocol MPC network</strong> &mdash; the same distributed key infrastructure
            that secures billions of dollars across the NEAR ecosystem.
            All critical operations &mdash; key derivation, transaction signing, and policy evaluation &mdash; happen exclusively inside the TEE.
            No single party, not even the infrastructure operator, can extract keys or bypass policy rules.
          </p>
        </div>
      </section>

      {/* How It Works */}
      <section id="how-it-works" className="mb-10 scroll-mt-4">
        <AnchorHeading id="how-it-works">How It Works</AnchorHeading>

        {/* Canvas-style visual architecture diagram */}
        <div className="bg-gray-50 rounded-xl border border-gray-200 p-6 mb-6 overflow-x-auto">
          <div className="min-w-[700px]">

            {/* Row 1: Agent and Wallet Owner on the sides */}
            <div className="grid grid-cols-[1fr_2fr_1fr] gap-0 items-start">
              {/* Agent */}
              <div className="bg-white rounded-lg border-2 border-blue-400 p-3 text-center">
                <div className="text-lg mb-1">&#129302;</div>
                <div className="text-xs font-bold text-blue-700">AI Agent</div>
                <div className="text-xs text-gray-500 mt-1">Has API key only</div>
              </div>

              {/* Spacer for the middle */}
              <div />

              {/* Wallet Owner */}
              <div className="bg-white rounded-lg border-2 border-orange-400 p-3 text-center">
                <div className="text-lg mb-1">&#128100;</div>
                <div className="text-xs font-bold text-orange-700">Wallet Owner</div>
                <div className="text-xs text-gray-500 mt-1">Controls policy</div>
              </div>
            </div>

            {/* Row 2: Arrows down from Agent and Owner */}
            <div className="grid grid-cols-[1fr_2fr_1fr] gap-0">
              <div className="flex flex-col items-center py-1">
                <svg width="24" height="32" className="text-blue-400"><path d="M12 0 L12 24 M6 18 L12 24 L18 18" stroke="currentColor" strokeWidth="2" fill="none"/></svg>
                <span className="text-[10px] text-blue-600 font-medium">transfer, swap, call</span>
              </div>
              <div />
              <div className="flex flex-col items-center py-1">
                <svg width="24" height="32" className="text-orange-400"><path d="M12 0 L12 24 M6 18 L12 24 L18 18" stroke="currentColor" strokeWidth="2" fill="none"/></svg>
                <span className="text-[10px] text-orange-600 font-medium">set policy, freeze</span>
              </div>
            </div>

            {/* Row 3: TEE box in the center spanning full width */}
            <div className="relative border-2 border-green-500 rounded-xl p-4 bg-green-50/50">
              <span className="absolute -top-3 left-1/2 -translate-x-1/2 bg-green-500 text-white text-xs font-bold px-3 py-0.5 rounded-full whitespace-nowrap">TEE (Intel TDX) &mdash; all security here</span>
              <div className="grid grid-cols-3 gap-4 mt-2">
                <div className="bg-white rounded-lg border border-green-300 p-3 text-center">
                  <div className="text-lg mb-1">&#128273;</div>
                  <div className="text-xs font-semibold text-gray-800">Key Derivation</div>
                  <div className="text-xs text-gray-500 mt-1">MPC master secret + HMAC</div>
                  <div className="text-xs text-gray-500">Private keys never leave</div>
                </div>
                <div className="bg-white rounded-lg border border-green-300 p-3 text-center">
                  <div className="text-lg mb-1">&#9997;&#65039;</div>
                  <div className="text-xs font-semibold text-gray-800">Transaction Signing</div>
                  <div className="text-xs text-gray-500 mt-1">Signs transfer, intents_withdraw, intents_swap, call, delete</div>
                  <div className="text-xs text-gray-500">inside secure enclave</div>
                </div>
                <div className="bg-white rounded-lg border border-green-300 p-3 text-center">
                  <div className="text-lg mb-1">&#128737;</div>
                  <div className="text-xs font-semibold text-gray-800">Policy Evaluation</div>
                  <div className="text-xs text-gray-500 mt-1">Decrypts policy from chain</div>
                  <div className="text-xs text-gray-500">Enforces all limits &amp; rules</div>
                </div>
              </div>
            </div>

            {/* Row 4: Arrows down + up from TEE to blockchain */}
            <div className="flex justify-center py-1">
              <div className="flex flex-col items-center">
                <div className="flex items-center gap-2">
                  <svg width="24" height="32" className="text-purple-400"><path d="M12 0 L12 24 M6 18 L12 24 L18 18" stroke="currentColor" strokeWidth="2" fill="none"/></svg>
                  <span className="text-[10px] text-purple-600 font-medium leading-tight">submit signed tx<br/>read encrypted policy</span>
                  <svg width="24" height="32" className="text-purple-400"><path d="M12 24 L12 0 M6 6 L12 0 L18 6" stroke="currentColor" strokeWidth="2" fill="none"/></svg>
                </div>
              </div>
            </div>

            {/* Row 5: NEAR Blockchain + NEAR Intents */}
            <div className="grid grid-cols-2 gap-4">
              <div className="bg-white rounded-lg border-2 border-purple-400 p-3 text-center">
                <div className="text-lg mb-1">&#9939;&#65039;</div>
                <div className="text-xs font-bold text-purple-700">NEAR Blockchain</div>
                <div className="text-xs text-gray-500 mt-1">Encrypted policy storage</div>
                <div className="text-xs text-gray-500">Freeze / unfreeze on-chain</div>
              </div>
              <div className="bg-white rounded-lg border-2 border-purple-400 p-3 text-center">
                <div className="text-lg mb-1">&#128256;</div>
                <div className="text-xs font-bold text-purple-700">NEAR Intents</div>
                <div className="text-xs text-gray-500 mt-1">Gasless cross-chain transfers</div>
                <div className="text-xs text-gray-500">NEAR, ETH, BTC, SOL</div>
              </div>
            </div>

          </div>
        </div>

        <p className="text-gray-700 mb-4">
          The API gateway is a stateless proxy &mdash; it authenticates the API key and forwards requests to the TEE.
          Everything security-critical (keys, signing, policy checks) happens exclusively inside the Trusted Execution Environment.
        </p>

        <ol className="list-decimal list-inside text-gray-700 space-y-2 mt-4">
          <li><strong>Register</strong> &mdash; one API call creates a wallet and returns an API key. The TEE derives the private key from the MPC master secret and returns the public key. No blockchain transaction needed.</li>
          <li><strong>Operate</strong> &mdash; the agent uses the API key to transfer NEAR, swap tokens, call smart contracts, or withdraw cross-chain. The TEE checks the policy, signs the transaction, and broadcasts it to the NEAR network.</li>
          <li><strong>Control</strong> &mdash; the wallet owner configures policies for the agent via the dashboard: spending limits, address whitelists, transaction confirmations, multisig, or instant freeze.</li>
        </ol>
      </section>

      {/* Agent ID */}
      <section id="agent-id" className="mb-10 scroll-mt-4">
        <AnchorHeading id="agent-id">Agent ID</AnchorHeading>

        <p className="text-gray-700 mb-4">
          Every wallet is identified by its <strong>Agent ID</strong> &mdash; a public key derived deterministically inside the TEE from the MPC master secret.
          The agent receives an API key for authentication, while the Agent ID serves as the on-chain identity.
        </p>

        <div className="overflow-x-auto mb-4">
          <table className="min-w-full text-sm border border-gray-200">
            <thead className="bg-gray-50">
              <tr>
                <th className="px-4 py-2 text-left font-semibold border-b">Concept</th>
                <th className="px-4 py-2 text-left font-semibold border-b">Description</th>
                <th className="px-4 py-2 text-left font-semibold border-b">Example</th>
              </tr>
            </thead>
            <tbody>
              <tr className="border-b">
                <td className="px-4 py-2 font-mono">Agent ID</td>
                <td className="px-4 py-2">Public key (wallet identity)</td>
                <td className="px-4 py-2 font-mono text-xs">ed25519:9a65d26b...</td>
              </tr>
              <tr className="border-b">
                <td className="px-4 py-2 font-mono">API Key</td>
                <td className="px-4 py-2">Secret token for API auth</td>
                <td className="px-4 py-2 font-mono text-xs">wk_15807dbda492...</td>
              </tr>
              <tr className="border-b">
                <td className="px-4 py-2 font-mono">NEAR address</td>
                <td className="px-4 py-2">Derived implicit account</td>
                <td className="px-4 py-2 font-mono text-xs">36842e2f73d0b7...</td>
              </tr>
              <tr className="border-b">
                <td className="px-4 py-2 font-mono">ETH address</td>
                <td className="px-4 py-2">Derived Ethereum address</td>
                <td className="px-4 py-2 font-mono text-xs">0x7f3a...</td>
              </tr>
            </tbody>
          </table>
        </div>

        <p className="text-gray-700">
          The agent uses the <code className="bg-gray-100 px-1 rounded">API Key</code> to authenticate all requests.
          The private key behind the Agent ID <strong>never leaves the TEE</strong> &mdash; transactions are signed inside the secure enclave and the agent never has access to it.
        </p>
      </section>

      {/* Multi-Chain */}
      <section id="multi-chain" className="mb-10 scroll-mt-4">
        <AnchorHeading id="multi-chain">Multi-Chain Support</AnchorHeading>

        <p className="text-gray-700 mb-4">
          A single wallet provides addresses on multiple blockchains. All cross-chain transfers go through the <strong>NEAR Intents protocol</strong> &mdash;
          no bridges, no gas tokens needed on the destination chain.
        </p>

        <div className="overflow-x-auto mb-4">
          <table className="min-w-full text-sm border border-gray-200">
            <thead className="bg-gray-50">
              <tr>
                <th className="px-4 py-2 text-left font-semibold border-b">Chain</th>
                <th className="px-4 py-2 text-left font-semibold border-b">Address Type</th>
                <th className="px-4 py-2 text-left font-semibold border-b">Withdraw</th>
                <th className="px-4 py-2 text-left font-semibold border-b">Contract Calls</th>
              </tr>
            </thead>
            <tbody>
              <tr className="border-b">
                <td className="px-4 py-2 font-semibold">NEAR</td>
                <td className="px-4 py-2">Ed25519 implicit account</td>
                <td className="px-4 py-2 text-green-600">&#10003; via Intents (gasless)</td>
                <td className="px-4 py-2 text-green-600">&#10003; direct (requires NEAR for gas)</td>
              </tr>
              <tr className="border-b">
                <td className="px-4 py-2 font-semibold">Ethereum</td>
                <td className="px-4 py-2">secp256k1 address</td>
                <td className="px-4 py-2 text-green-600">&#10003; via Intents (gasless)</td>
                <td className="px-4 py-2 text-gray-400">&mdash;</td>
              </tr>
              <tr className="border-b">
                <td className="px-4 py-2 font-semibold">Bitcoin</td>
                <td className="px-4 py-2">Derived address</td>
                <td className="px-4 py-2 text-green-600">&#10003; via Intents (gasless)</td>
                <td className="px-4 py-2 text-gray-400">&mdash;</td>
              </tr>
              <tr className="border-b">
                <td className="px-4 py-2 font-semibold">Solana</td>
                <td className="px-4 py-2">Ed25519 address</td>
                <td className="px-4 py-2 text-green-600">&#10003; via Intents (gasless)</td>
                <td className="px-4 py-2 text-gray-400">&mdash;</td>
              </tr>
              <tr>
                <td className="px-4 py-2 font-semibold">Others</td>
                <td className="px-4 py-2">Chain-specific</td>
                <td className="px-4 py-2 text-green-600">&#10003; via Intents (gasless)</td>
                <td className="px-4 py-2 text-gray-400">&mdash;</td>
              </tr>
            </tbody>
          </table>
        </div>

        <div className="bg-blue-50 border-l-4 border-blue-400 p-4 mb-4">
          <p className="text-sm text-gray-700">
            <strong>Gasless cross-chain:</strong> When the agent calls <code className="bg-blue-100 px-1 rounded">POST /wallet/v1/intents/withdraw</code> to send tokens to Ethereum or Solana,
            it does not need ETH or SOL for gas. The NEAR Intents protocol handles execution and fee settlement natively.
          </p>
        </div>
      </section>

      {/* Policy Engine */}
      <section id="policy-engine" className="mb-10 scroll-mt-4">
        <AnchorHeading id="policy-engine">Policy Engine</AnchorHeading>

        <p className="text-gray-700 mb-4">
          The wallet owner controls the agent&apos;s permissions through a policy &mdash; a set of rules that are encrypted and stored on-chain.
          The policy is decrypted and evaluated inside the TEE on every transaction. The agent cannot bypass or modify it.
        </p>

        <div className="overflow-x-auto mb-4">
          <table className="min-w-full text-sm border border-gray-200">
            <thead className="bg-gray-50">
              <tr>
                <th className="px-4 py-2 text-left font-semibold border-b">Rule</th>
                <th className="px-4 py-2 text-left font-semibold border-b">Description</th>
                <th className="px-4 py-2 text-left font-semibold border-b">Example</th>
              </tr>
            </thead>
            <tbody>
              <tr className="border-b">
                <td className="px-4 py-2 font-semibold">Per-transaction limit</td>
                <td className="px-4 py-2">Max amount per single transaction, per token</td>
                <td className="px-4 py-2 font-mono text-xs">native: 10 NEAR, USDT: $1,000</td>
              </tr>
              <tr className="border-b">
                <td className="px-4 py-2 font-semibold">Hourly limit</td>
                <td className="px-4 py-2">Total spending cap per hour</td>
                <td className="px-4 py-2 font-mono text-xs">*: 50 NEAR/hour</td>
              </tr>
              <tr className="border-b">
                <td className="px-4 py-2 font-semibold">Daily limit</td>
                <td className="px-4 py-2">Total spending cap per day</td>
                <td className="px-4 py-2 font-mono text-xs">*: 500 NEAR/day</td>
              </tr>
              <tr className="border-b">
                <td className="px-4 py-2 font-semibold">Monthly limit</td>
                <td className="px-4 py-2">Total spending cap per month</td>
                <td className="px-4 py-2 font-mono text-xs">*: 5,000 NEAR/month</td>
              </tr>
              <tr className="border-b">
                <td className="px-4 py-2 font-semibold">Address whitelist</td>
                <td className="px-4 py-2">Only allow transfers to approved addresses</td>
                <td className="px-4 py-2 font-mono text-xs">bob.near, dex.near</td>
              </tr>
              <tr className="border-b">
                <td className="px-4 py-2 font-semibold">Address blacklist</td>
                <td className="px-4 py-2">Block transfers to specific addresses</td>
                <td className="px-4 py-2 font-mono text-xs">scammer.near</td>
              </tr>
              <tr className="border-b">
                <td className="px-4 py-2 font-semibold">Time restrictions</td>
                <td className="px-4 py-2">Allow operations only during business hours</td>
                <td className="px-4 py-2 font-mono text-xs">Mon-Fri 9:00-17:00 UTC</td>
              </tr>
              <tr className="border-b">
                <td className="px-4 py-2 font-semibold">Rate limit</td>
                <td className="px-4 py-2">Max transactions per hour</td>
                <td className="px-4 py-2 font-mono text-xs">60 tx/hour</td>
              </tr>
              <tr className="border-b">
                <td className="px-4 py-2 font-semibold">Multisig approval</td>
                <td className="px-4 py-2">Require human approval above a threshold</td>
                <td className="px-4 py-2 font-mono text-xs">2-of-3 above $1,000</td>
              </tr>
              <tr className="border-b">
                <td className="px-4 py-2 font-semibold">Transaction types</td>
                <td className="px-4 py-2">Restrict allowed operations (transfer, call, intents_withdraw, intents_swap, intents_deposit, delete)</td>
                <td className="px-4 py-2 font-mono text-xs">call, swap only</td>
              </tr>
              <tr>
                <td className="px-4 py-2 font-semibold">Emergency freeze</td>
                <td className="px-4 py-2">Instantly halt all operations</td>
                <td className="px-4 py-2 font-mono text-xs">One-click from dashboard</td>
              </tr>
            </tbody>
          </table>
        </div>

        <p className="text-gray-700 mb-4">
          Policies are stored <strong>encrypted on-chain</strong> (on the NEAR blockchain). Only the TEE can decrypt and evaluate them.
          This means neither the API gateway operator nor the agent can see or tamper with the raw policy rules.
        </p>

        <h3 className="text-lg font-semibold mt-6 mb-2">Example Policy</h3>
        <SyntaxHighlighter language="json" style={vscDarkPlus} customStyle={{ borderRadius: '0.5rem', fontSize: '0.875rem' }}>
{`{
  "rules": {
    "limits": {
      "per_transaction": { "native": "10000000000000000000000000" },
      "daily": { "*": "100000000000000000000000000" },
      "hourly": { "*": "50000000000000000000000000" }
    },
    "transaction_types": ["transfer", "call", "intents_withdraw", "intents_swap", "intents_deposit", "delete"],
    "addresses": {
      "mode": "whitelist",
      "list": ["bob.near", "dex.near"]
    },
    "rate_limit": { "max_per_hour": 60 },
    "time_restrictions": {
      "timezone": "UTC",
      "allowed_hours": [9, 17],
      "allowed_days": [1, 2, 3, 4, 5]
    }
  },
  "approval": {
    "threshold": { "required": 2, "of": 3 },
    "above_usd": 1000,
    "approvers": [
      { "id": "ed25519:pubkey1", "role": "admin" },
      { "id": "ed25519:pubkey2", "role": "signer" },
      { "id": "ed25519:pubkey3", "role": "signer" }
    ]
  }
}`}
        </SyntaxHighlighter>
      </section>

      {/* Multisig */}
      <section id="multisig" className="mb-10 scroll-mt-4">
        <AnchorHeading id="multisig">Multisig Approval</AnchorHeading>

        <p className="text-gray-700 mb-4">
          When a transaction exceeds the approval threshold (e.g. above $1,000), it goes into a pending state.
          Designated approvers sign the request using their NEAR wallet (NEP-413 signature). Once the required number of signatures is collected, the transaction executes automatically.
        </p>

        <SyntaxHighlighter language="text" style={vscDarkPlus} customStyle={{ borderRadius: '0.5rem', fontSize: '0.875rem' }}>
{`Agent: POST /wallet/v1/intents/withdraw { amount: "$5,000" }
  -> Response: { status: "pending_approval", required: 2, approved: 0 }

Approver 1: Signs approval via NEAR wallet (dashboard)
  -> { approved: 1, required: 2 }

Approver 2: Signs approval via NEAR wallet (dashboard)
  -> Threshold met -> auto-execute -> { status: "success", tx_hash: "..." }`}
        </SyntaxHighlighter>

        <div className="overflow-x-auto mt-4 mb-4">
          <table className="min-w-full text-sm border border-gray-200">
            <thead className="bg-gray-50">
              <tr>
                <th className="px-4 py-2 text-left font-semibold border-b">Role</th>
                <th className="px-4 py-2 text-left font-semibold border-b">Approve transactions</th>
                <th className="px-4 py-2 text-left font-semibold border-b">Modify policy</th>
                <th className="px-4 py-2 text-left font-semibold border-b">Freeze wallet</th>
              </tr>
            </thead>
            <tbody>
              <tr className="border-b">
                <td className="px-4 py-2 font-semibold">Admin</td>
                <td className="px-4 py-2 text-green-600">&#10003;</td>
                <td className="px-4 py-2 text-green-600">&#10003; (quorum)</td>
                <td className="px-4 py-2 text-green-600">&#10003;</td>
              </tr>
              <tr>
                <td className="px-4 py-2 font-semibold">Signer</td>
                <td className="px-4 py-2 text-green-600">&#10003;</td>
                <td className="px-4 py-2 text-red-500">&#10007;</td>
                <td className="px-4 py-2 text-red-500">&#10007;</td>
              </tr>
            </tbody>
          </table>
        </div>
      </section>

      {/* Quick Start */}
      <section id="quick-start" className="mb-10 scroll-mt-4">
        <AnchorHeading id="quick-start">Quick Start</AnchorHeading>

        <h3 className="text-lg font-semibold mt-2 mb-2">1. Register a wallet</h3>
        <SyntaxHighlighter language="bash" style={vscDarkPlus} customStyle={{ borderRadius: '0.5rem', fontSize: '0.875rem' }}>
{`curl -s -X POST https://api.outlayer.fastnear.com/register

# Response:
# {
#   "api_key": "wk_15807dbda492...",
#   "near_account_id": "36842e2f73d0...",
#   "handoff_url": "https://outlayer.fastnear.com/wallet?key=wk_..."
# }`}
        </SyntaxHighlighter>
        <p className="text-sm text-gray-600 mt-1 mb-4">The <code className="bg-gray-100 px-1 rounded">api_key</code> is shown only once. Store it securely.</p>

        <h3 className="text-lg font-semibold mt-4 mb-2">2. Get addresses</h3>
        <SyntaxHighlighter language="bash" style={vscDarkPlus} customStyle={{ borderRadius: '0.5rem', fontSize: '0.875rem' }}>
{`# NEAR address
curl -s -H "Authorization: Bearer $API_KEY" \\
  "https://api.outlayer.fastnear.com/wallet/v1/address?chain=near"

# Ethereum address
curl -s -H "Authorization: Bearer $API_KEY" \\
  "https://api.outlayer.fastnear.com/wallet/v1/address?chain=ethereum"`}
        </SyntaxHighlighter>

        <h3 className="text-lg font-semibold mt-4 mb-2">3. Check balance</h3>
        <SyntaxHighlighter language="bash" style={vscDarkPlus} customStyle={{ borderRadius: '0.5rem', fontSize: '0.875rem' }}>
{`# Native NEAR balance
curl -s -H "Authorization: Bearer $API_KEY" \\
  "https://api.outlayer.fastnear.com/wallet/v1/balance?chain=near"

# FT token balance (e.g. USDT)
curl -s -H "Authorization: Bearer $API_KEY" \\
  "https://api.outlayer.fastnear.com/wallet/v1/balance?chain=near&token=usdt.tether-token.near"`}
        </SyntaxHighlighter>

        <h3 className="text-lg font-semibold mt-4 mb-2">4. Transfer NEAR</h3>
        <SyntaxHighlighter language="bash" style={vscDarkPlus} customStyle={{ borderRadius: '0.5rem', fontSize: '0.875rem' }}>
{`curl -s -X POST -H "Content-Type: application/json" \\
  -H "Authorization: Bearer $API_KEY" \\
  -d '{"receiver_id":"bob.near","amount":"1000000000000000000000000"}' \\
  "https://api.outlayer.fastnear.com/wallet/v1/transfer"`}
        </SyntaxHighlighter>

        <h3 className="text-lg font-semibold mt-4 mb-2">5. Swap tokens</h3>
        <SyntaxHighlighter language="bash" style={vscDarkPlus} customStyle={{ borderRadius: '0.5rem', fontSize: '0.875rem' }}>
{`# Swap wNEAR -> USDT (handles deposit, storage, and settlement automatically)
curl -s -X POST -H "Content-Type: application/json" \\
  -H "Authorization: Bearer $API_KEY" \\
  -d '{"token_in":"nep141:wrap.near","token_out":"nep141:usdt.tether-token.near","amount_in":"1000000000000000000000000"}' \\
  "https://api.outlayer.fastnear.com/wallet/v1/intents/swap"`}
        </SyntaxHighlighter>

        <h3 className="text-lg font-semibold mt-4 mb-2">6. Call a NEAR contract</h3>
        <SyntaxHighlighter language="bash" style={vscDarkPlus} customStyle={{ borderRadius: '0.5rem', fontSize: '0.875rem' }}>
{`curl -s -X POST -H "Content-Type: application/json" \\
  -H "Authorization: Bearer $API_KEY" \\
  -d '{"receiver_id":"wrap.near","method_name":"near_deposit","args":{},"deposit":"10000000000000000000000"}' \\
  "https://api.outlayer.fastnear.com/wallet/v1/call"`}
        </SyntaxHighlighter>

        <h3 className="text-lg font-semibold mt-4 mb-2">7. Withdraw (gasless cross-chain via Intents)</h3>
        <SyntaxHighlighter language="bash" style={vscDarkPlus} customStyle={{ borderRadius: '0.5rem', fontSize: '0.875rem' }}>
{`# Tokens must be in Intents balance first (use /intents/deposit or /swap)
curl -s -X POST -H "Content-Type: application/json" \\
  -H "Authorization: Bearer $API_KEY" \\
  -d '{"to":"receiver.near","amount":"1000000","token":"usdt.tether-token.near","chain":"near"}' \\
  "https://api.outlayer.fastnear.com/wallet/v1/intents/withdraw"`}
        </SyntaxHighlighter>

        <h3 className="text-lg font-semibold mt-4 mb-2">8. Delete wallet (irreversible)</h3>
        <p className="text-gray-700 mb-2">Delete the on-chain account, send all NEAR to a beneficiary, and revoke all API keys:</p>
        <SyntaxHighlighter language="bash" style={vscDarkPlus} customStyle={{ borderRadius: '0.5rem', fontSize: '0.875rem' }}>
{`curl -s -X POST -H "Content-Type: application/json" \\
  -H "Authorization: Bearer $API_KEY" \\
  -d '{"beneficiary":"receiver.near","chain":"near"}' \\
  "https://api.outlayer.fastnear.com/wallet/v1/delete"`}
        </SyntaxHighlighter>
        <div className="bg-red-50 border-l-4 border-red-500 p-4 mt-2 mb-4">
          <p className="text-sm text-gray-700">
            <strong>Warning:</strong> Only native NEAR tokens are sent to the beneficiary. FT tokens (USDT, wNEAR, etc.) and Intents balances
            are <strong>lost permanently</strong> because the account is deleted from the network. Withdraw or transfer those assets <strong>before</strong> deleting.
          </p>
        </div>

        <h3 className="text-lg font-semibold mt-4 mb-2">9. Configure policy (optional)</h3>
        <p className="text-gray-700 mb-2">Share the handoff URL with the wallet owner so they can set spending limits, whitelists, and multisig rules from the dashboard:</p>
        <SyntaxHighlighter language="text" style={vscDarkPlus} customStyle={{ borderRadius: '0.5rem', fontSize: '0.875rem' }}>
{`https://outlayer.fastnear.com/wallet?key=wk_...`}
        </SyntaxHighlighter>
      </section>

      {/* API Reference */}
      <section id="api-reference" className="mb-10 scroll-mt-4">
        <AnchorHeading id="api-reference">API Reference</AnchorHeading>

        <p className="text-gray-700 mb-4">All endpoints except <code className="bg-gray-100 px-1 rounded">/register</code> require the <code className="bg-gray-100 px-1 rounded">Authorization: Bearer &lt;api_key&gt;</code> header.</p>

        <div className="overflow-x-auto mb-4">
          <table className="min-w-full text-sm border border-gray-200">
            <thead className="bg-gray-50">
              <tr>
                <th className="px-4 py-2 text-left font-semibold border-b">Action</th>
                <th className="px-4 py-2 text-left font-semibold border-b">Method</th>
                <th className="px-4 py-2 text-left font-semibold border-b">Endpoint</th>
              </tr>
            </thead>
            <tbody>
              <tr className="border-b">
                <td className="px-4 py-2">Register wallet</td>
                <td className="px-4 py-2 font-mono">POST</td>
                <td className="px-4 py-2 font-mono text-xs">/register</td>
              </tr>
              <tr className="border-b">
                <td className="px-4 py-2">Get address</td>
                <td className="px-4 py-2 font-mono">GET</td>
                <td className="px-4 py-2 font-mono text-xs">/wallet/v1/address?chain=&#123;chain&#125;</td>
              </tr>
              <tr className="border-b">
                <td className="px-4 py-2">Check balance</td>
                <td className="px-4 py-2 font-mono">GET</td>
                <td className="px-4 py-2 font-mono text-xs">/wallet/v1/balance?chain=near&amp;token=&#123;token&#125;</td>
              </tr>
              <tr className="border-b">
                <td className="px-4 py-2">Transfer NEAR</td>
                <td className="px-4 py-2 font-mono">POST</td>
                <td className="px-4 py-2 font-mono text-xs">/wallet/v1/transfer</td>
              </tr>
              <tr className="border-b">
                <td className="px-4 py-2">Call NEAR contract</td>
                <td className="px-4 py-2 font-mono">POST</td>
                <td className="px-4 py-2 font-mono text-xs">/wallet/v1/call</td>
              </tr>
              <tr className="border-b">
                <td className="px-4 py-2">Swap tokens</td>
                <td className="px-4 py-2 font-mono">POST</td>
                <td className="px-4 py-2 font-mono text-xs">/wallet/v1/intents/swap</td>
              </tr>
              <tr className="border-b">
                <td className="px-4 py-2">Deposit to Intents</td>
                <td className="px-4 py-2 font-mono">POST</td>
                <td className="px-4 py-2 font-mono text-xs">/wallet/v1/intents/deposit</td>
              </tr>
              <tr className="border-b">
                <td className="px-4 py-2">Withdraw (cross-chain)</td>
                <td className="px-4 py-2 font-mono">POST</td>
                <td className="px-4 py-2 font-mono text-xs">/wallet/v1/intents/withdraw</td>
              </tr>
              <tr className="border-b">
                <td className="px-4 py-2">Dry-run withdraw</td>
                <td className="px-4 py-2 font-mono">POST</td>
                <td className="px-4 py-2 font-mono text-xs">/wallet/v1/intents/withdraw/dry-run</td>
              </tr>
              <tr className="border-b">
                <td className="px-4 py-2">Delete wallet</td>
                <td className="px-4 py-2 font-mono">POST</td>
                <td className="px-4 py-2 font-mono text-xs">/wallet/v1/delete</td>
              </tr>
              <tr className="border-b">
                <td className="px-4 py-2">Request status</td>
                <td className="px-4 py-2 font-mono">GET</td>
                <td className="px-4 py-2 font-mono text-xs">/wallet/v1/requests/&#123;request_id&#125;</td>
              </tr>
              <tr className="border-b">
                <td className="px-4 py-2">List requests</td>
                <td className="px-4 py-2 font-mono">GET</td>
                <td className="px-4 py-2 font-mono text-xs">/wallet/v1/requests</td>
              </tr>
              <tr className="border-b">
                <td className="px-4 py-2">List tokens</td>
                <td className="px-4 py-2 font-mono">GET</td>
                <td className="px-4 py-2 font-mono text-xs">/wallet/v1/tokens</td>
              </tr>
              <tr className="border-b">
                <td className="px-4 py-2">View policy</td>
                <td className="px-4 py-2 font-mono">GET</td>
                <td className="px-4 py-2 font-mono text-xs">/wallet/v1/policy</td>
              </tr>
              <tr>
                <td className="px-4 py-2">Audit log</td>
                <td className="px-4 py-2 font-mono">GET</td>
                <td className="px-4 py-2 font-mono text-xs">/wallet/v1/audit</td>
              </tr>
            </tbody>
          </table>
        </div>

        <p className="text-gray-700">
          Base URL: <code className="bg-gray-100 px-1 rounded">https://api.outlayer.fastnear.com</code>
        </p>
      </section>

      {/* Delete Wallet */}
      <section id="delete-wallet" className="mb-10 scroll-mt-4">
        <AnchorHeading id="delete-wallet">Delete Wallet</AnchorHeading>

        <p className="text-gray-700 mb-4">
          Permanently delete the wallet&apos;s on-chain NEAR account using the native <code className="bg-gray-100 px-1 rounded">DeleteAccount</code> action.
          All remaining native NEAR balance is automatically sent to the beneficiary. All API keys are revoked.
        </p>

        <div className="bg-red-50 border-l-4 border-red-500 p-4 mb-4">
          <p className="text-sm text-gray-700">
            <strong>Warning:</strong> Only native NEAR tokens are sent to the beneficiary (handled by NEAR&apos;s <code className="bg-red-100 px-1 rounded">DeleteAccount</code>).
            FT tokens (USDT, wNEAR, etc.) and Intents balances are <strong>lost permanently</strong> because the account is deleted from the network.
            Withdraw or transfer those assets <strong>before</strong> deleting the wallet.
          </p>
        </div>

        <h3 className="text-lg font-semibold mt-4 mb-2">Before deleting</h3>
        <ol className="list-decimal list-inside text-gray-700 space-y-2 mb-4">
          <li>Transfer all FT tokens via <code className="bg-gray-100 px-1 rounded">POST /wallet/v1/call</code> with <code className="bg-gray-100 px-1 rounded">ft_transfer</code></li>
          <li>Withdraw Intents balances via <code className="bg-gray-100 px-1 rounded">POST /wallet/v1/intents/withdraw</code></li>
          <li>Move any other on-chain assets to another account</li>
        </ol>

        <h3 className="text-lg font-semibold mt-4 mb-2">Request</h3>
        <SyntaxHighlighter language="bash" style={vscDarkPlus} customStyle={{ borderRadius: '0.5rem', fontSize: '0.875rem' }}>
{`curl -s -X POST -H "Content-Type: application/json" \\
  -H "Authorization: Bearer $API_KEY" \\
  -d '{"beneficiary":"receiver.near","chain":"near"}' \\
  "https://api.outlayer.fastnear.com/wallet/v1/delete"`}
        </SyntaxHighlighter>

        <h3 className="text-lg font-semibold mt-4 mb-2">Response</h3>
        <SyntaxHighlighter language="json" style={vscDarkPlus} customStyle={{ borderRadius: '0.5rem', fontSize: '0.875rem' }}>
{`{
  "request_id": "uuid",
  "status": "success",
  "tx_hash": "...",
  "beneficiary": "receiver.near"
}`}
        </SyntaxHighlighter>

        <p className="text-gray-700 mt-4">
          After deletion, the on-chain account no longer exists and all API keys are revoked. Subsequent API requests will return <code className="bg-gray-100 px-1 rounded">invalid_api_key</code>.
        </p>
      </section>

      {/* Sign Message (NEP-413) */}
      <section id="sign-message" className="mb-10 scroll-mt-4">
        <AnchorHeading id="sign-message">Sign Message (NEP-413)</AnchorHeading>

        <p className="text-gray-700 mb-4">
          Sign an arbitrary message using the wallet&apos;s NEAR private key following the <strong>NEP-413</strong> standard.
          Use this to authenticate your agent to external services that verify NEAR signatures &mdash; no on-chain transaction needed.
        </p>

        <h3 className="text-lg font-semibold mt-4 mb-2">Request</h3>
        <SyntaxHighlighter language="bash" style={vscDarkPlus} customStyle={{ borderRadius: '0.5rem', fontSize: '0.875rem' }}>
{`curl -s -X POST -H "Content-Type: application/json" \\
  -H "Authorization: Bearer $API_KEY" \\
  -d '{"message":"Login to example.com","recipient":"example.com"}' \\
  "https://api.outlayer.fastnear.com/wallet/v1/sign-message"`}
        </SyntaxHighlighter>

        <h3 className="text-lg font-semibold mt-4 mb-2">Response</h3>
        <SyntaxHighlighter language="json" style={vscDarkPlus} customStyle={{ borderRadius: '0.5rem', fontSize: '0.875rem' }}>
{`{
  "account_id": "aabbccdd11223344...",
  "public_key": "ed25519:...",
  "signature": "ed25519:...",
  "nonce": "base64-encoded-32-bytes"
}`}
        </SyntaxHighlighter>

        <h3 className="text-lg font-semibold mt-4 mb-2">Parameters</h3>
        <div className="overflow-x-auto mb-4">
          <table className="min-w-full text-sm">
            <thead><tr className="border-b"><th className="text-left py-2 px-3">Field</th><th className="text-left py-2 px-3">Required</th><th className="text-left py-2 px-3">Description</th></tr></thead>
            <tbody>
              <tr className="border-b"><td className="py-2 px-3"><code className="bg-gray-100 px-1 rounded">message</code></td><td className="py-2 px-3">Yes</td><td className="py-2 px-3">Text to sign (max 10,000 bytes)</td></tr>
              <tr className="border-b"><td className="py-2 px-3"><code className="bg-gray-100 px-1 rounded">recipient</code></td><td className="py-2 px-3">Yes</td><td className="py-2 px-3">Service that will verify the signature (1&ndash;128 chars)</td></tr>
              <tr className="border-b"><td className="py-2 px-3"><code className="bg-gray-100 px-1 rounded">nonce</code></td><td className="py-2 px-3">No</td><td className="py-2 px-3">Base64-encoded 32 bytes. Auto-generated if omitted</td></tr>
            </tbody>
          </table>
        </div>

        <div className="bg-blue-50 border-l-4 border-blue-400 p-4 mb-4">
          <p className="text-sm text-gray-700">
            <strong>Verification:</strong> The signature follows NEP-413. For implicit accounts, the verifier can confirm
            identity without RPC: <code className="bg-blue-100 px-1 rounded">account_id == hex(public_key_bytes)</code>.
          </p>
        </div>
      </section>

      {/* Security Model */}
      <section id="security" className="mb-10 scroll-mt-4">
        <AnchorHeading id="security">Security Model</AnchorHeading>

        <div className="space-y-4">
          <div className="border-l-4 border-gray-300 pl-4">
            <h3 className="font-semibold text-gray-900">MPC master secret</h3>
            <p className="text-sm text-gray-700">
              The keystore TEE obtains its master secret from the <strong>NEAR Protocol MPC network</strong> via a DAO-governed approval process.
              Individual wallet keys are then derived inside the TEE using <code className="bg-gray-100 px-1 rounded">HMAC-SHA256(master_secret, &quot;wallet:&#123;id&#125;:&#123;chain&#125;&quot;)</code>.
              The same wallet ID always produces the same addresses across chains.
            </p>
          </div>

          <div className="border-l-4 border-gray-300 pl-4">
            <h3 className="font-semibold text-gray-900">TEE attestation</h3>
            <p className="text-sm text-gray-700">
              Key derivation, transaction signing, and policy evaluation all happen inside Intel TDX enclaves.
              TEE attestation proves that the code running inside the enclave has not been tampered with.
              No one &mdash; not even the infrastructure operator &mdash; can extract keys or bypass policy checks.
            </p>
          </div>

          <div className="border-l-4 border-gray-300 pl-4">
            <h3 className="font-semibold text-gray-900">Policy on-chain</h3>
            <p className="text-sm text-gray-700">
              Policies are stored encrypted on the NEAR blockchain. Only the TEE can decrypt them. The controller (wallet owner) can freeze the wallet
              instantly via an on-chain transaction &mdash; no API gateway involvement needed.
            </p>
          </div>

          <div className="border-l-4 border-gray-300 pl-4">
            <h3 className="font-semibold text-gray-900">Agent compromise recovery</h3>
            <p className="text-sm text-gray-700">
              If the agent&apos;s API key is compromised: (1) freeze the wallet from the dashboard (instant), (2) revoke the API key,
              (3) create a new API key, (4) transfer funds to a new wallet if needed. The private key itself is never exposed &mdash; there is nothing to rotate.
            </p>
          </div>
        </div>
      </section>

      {/* AI Agent Skill */}
      <section id="ai-skill" className="mb-10 scroll-mt-4">
        <AnchorHeading id="ai-skill">AI Agent Skill File</AnchorHeading>

        <p className="text-gray-700 mb-4">
          For AI agents that support skill files (Claude MCP, OpenAI functions, etc.), OutLayer provides a ready-to-use
          skill definition that teaches the agent how to register, fund, and operate the wallet.
        </p>

        <SyntaxHighlighter language="bash" style={vscDarkPlus} customStyle={{ borderRadius: '0.5rem', fontSize: '0.875rem' }}>
{`# Fetch the skill file
curl -s https://outlayer.fastnear.com/SKILL.md`}
        </SyntaxHighlighter>

        <p className="text-gray-700 mt-2 mb-4">
          The skill file contains step-by-step instructions, API reference, error handling, and guidelines for the AI agent.
          Point your agent framework to this URL and the agent will know how to set up and use its wallet automatically.
        </p>

        <div className="bg-gray-50 border border-gray-200 rounded-lg p-4">
          <h4 className="font-semibold text-gray-900 mb-2">What the skill file covers:</h4>
          <ul className="list-disc list-inside text-sm text-gray-700 space-y-1">
            <li>Wallet registration (one POST call, no blockchain needed)</li>
            <li>Getting addresses and balances on any supported chain</li>
            <li>Native NEAR transfers, FT transfers, token swaps</li>
            <li>Cross-chain withdrawals via Intents (gasless)</li>
            <li>Intents deposits for manual balance management</li>
            <li>Contract calls with arbitrary function arguments</li>
            <li>Balance checks before every operation (required)</li>
            <li>Status polling, error handling, and automatic storage registration</li>
            <li>Fund link generation for requesting NEAR from the user</li>
            <li>Guiding the user to configure spending policies</li>
          </ul>
        </div>
      </section>

      {/* CLI Integration */}
      <section id="cli-integration" className="mb-10 scroll-mt-4">
        <AnchorHeading id="cli-integration">OutLayer CLI Integration</AnchorHeading>

        <p className="text-gray-700 mb-4">
          Agents with custody wallets can use the <strong>OutLayer CLI</strong> directly &mdash; no NEAR private key needed.
          Login with the wallet API key and all commands route signing through the coordinator&apos;s wallet API transparently.
        </p>

        <SyntaxHighlighter language="bash" style={vscDarkPlus} customStyle={{ borderRadius: '0.5rem', fontSize: '0.875rem' }}>
{`# Login with wallet key (instead of NEAR private key)
outlayer login --wallet-key wk_15807dbda492636df5280629d7617c3ea80f915ba960389b621e420ca275e545

# All commands work transparently
outlayer deploy my-agent
outlayer keys create
outlayer run alice.near/my-agent '{"test": true}'
outlayer secrets set '{"API_KEY":"sk-..."}' --project alice.near/my-agent
outlayer earnings`}
        </SyntaxHighlighter>

        <div className="bg-blue-50 border-l-4 border-blue-400 p-4 mt-4 mb-4">
          <p className="text-sm text-gray-700">
            <strong>How it works:</strong> When logged in with <code className="bg-blue-100 px-1 rounded">--wallet-key</code>, the CLI stores the auth type as <code className="bg-blue-100 px-1 rounded">wallet_key</code>.
            All contract operations are routed through <code className="bg-blue-100 px-1 rounded">POST /wallet/v1/call</code> instead of local transaction signing.
            NEP-413 signatures (used by <code className="bg-blue-100 px-1 rounded">secrets update</code>) go through <code className="bg-blue-100 px-1 rounded">POST /wallet/v1/sign-message</code>.
          </p>
        </div>

        <h4 className="font-semibold text-gray-900 mb-2">Supported commands</h4>
        <div className="overflow-x-auto mb-4">
          <table className="min-w-full text-sm border border-gray-200">
            <thead className="bg-gray-50">
              <tr>
                <th className="px-4 py-2 text-left font-semibold border-b">Command</th>
                <th className="px-4 py-2 text-left font-semibold border-b">Status</th>
              </tr>
            </thead>
            <tbody>
              <tr className="border-b"><td className="px-4 py-2 font-mono">deploy, run, keys, secrets, earnings, versions</td><td className="px-4 py-2 text-green-700">Supported</td></tr>
              <tr><td className="px-4 py-2 font-mono">upload (FastFS)</td><td className="px-4 py-2 text-yellow-700">Not yet &mdash; requires raw Borsh args</td></tr>
            </tbody>
          </table>
        </div>
      </section>

      {/* Comparison */}
      <section id="comparison" className="mb-10 scroll-mt-4">
        <AnchorHeading id="comparison">Comparison with Traditional Custody</AnchorHeading>

        <div className="overflow-x-auto mb-4">
          <table className="min-w-full text-sm border border-gray-200">
            <thead className="bg-gray-50">
              <tr>
                <th className="px-4 py-2 text-left font-semibold border-b">Feature</th>
                <th className="px-4 py-2 text-left font-semibold border-b">OutLayer Agent Custody</th>
                <th className="px-4 py-2 text-left font-semibold border-b">Traditional (Fireblocks etc.)</th>
              </tr>
            </thead>
            <tbody>
              <tr className="border-b">
                <td className="px-4 py-2 font-semibold">Setup</td>
                <td className="px-4 py-2">One API call, instant</td>
                <td className="px-4 py-2">Enterprise onboarding, days-weeks</td>
              </tr>
              <tr className="border-b">
                <td className="px-4 py-2 font-semibold">Key management</td>
                <td className="px-4 py-2">MPC + TEE, no key exposure</td>
                <td className="px-4 py-2">MPC, HSM</td>
              </tr>
              <tr className="border-b">
                <td className="px-4 py-2 font-semibold">Policy engine</td>
                <td className="px-4 py-2">On-chain encrypted, TEE-enforced</td>
                <td className="px-4 py-2">Server-side, proprietary</td>
              </tr>
              <tr className="border-b">
                <td className="px-4 py-2 font-semibold">Cross-chain</td>
                <td className="px-4 py-2">NEAR Intents (gasless)</td>
                <td className="px-4 py-2">Per-chain integration</td>
              </tr>
              <tr className="border-b">
                <td className="px-4 py-2 font-semibold">Gas tokens</td>
                <td className="px-4 py-2">Not needed for transfers</td>
                <td className="px-4 py-2">Required per chain</td>
              </tr>
              <tr className="border-b">
                <td className="px-4 py-2 font-semibold">AI agent ready</td>
                <td className="px-4 py-2">Skill file, REST API, WASI host functions</td>
                <td className="px-4 py-2">SDK integration</td>
              </tr>
              <tr>
                <td className="px-4 py-2 font-semibold">Pricing</td>
                <td className="px-4 py-2">Pay per transaction</td>
                <td className="px-4 py-2">Monthly subscription + per-tx</td>
              </tr>
            </tbody>
          </table>
        </div>
      </section>

      {/* Dashboard */}
      <section id="dashboard" className="mb-10 scroll-mt-4">
        <AnchorHeading id="dashboard">Dashboard</AnchorHeading>

        <p className="text-gray-700 mb-4">
          The wallet owner manages everything through the OutLayer dashboard. The agent only sees the API.
        </p>

        <div className="overflow-x-auto mb-4">
          <table className="min-w-full text-sm border border-gray-200">
            <thead className="bg-gray-50">
              <tr>
                <th className="px-4 py-2 text-left font-semibold border-b">Page</th>
                <th className="px-4 py-2 text-left font-semibold border-b">What it does</th>
              </tr>
            </thead>
            <tbody>
              <tr className="border-b">
                <td className="px-4 py-2 font-mono">/wallet/manage</td>
                <td className="px-4 py-2">Create and edit policies, manage approvers, freeze/unfreeze wallet</td>
              </tr>
              <tr className="border-b">
                <td className="px-4 py-2 font-mono">/wallet/approvals</td>
                <td className="px-4 py-2">View and sign pending multisig approvals</td>
              </tr>
              <tr>
                <td className="px-4 py-2 font-mono">/wallet/audit</td>
                <td className="px-4 py-2">Full history of all transactions, policy changes, approvals, freezes</td>
              </tr>
            </tbody>
          </table>
        </div>
      </section>

    </div>
  );
}
