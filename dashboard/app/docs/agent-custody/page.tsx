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
            <h3 className="font-semibold text-gray-900 mb-1">Cross-Chain Value</h3>
            <p className="text-sm text-gray-600">One NEAR-native wallet that deposits from and withdraws to NEAR, Ethereum, Bitcoin, Solana, and other chains via the <strong>NEAR Intents</strong> protocol. No bridges, no wrapping. EVM signing (EIP-712 / EIP-191 / raw tx) is now supported; native Solana signing is still planned.</p>
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

        <div className="bg-blue-50 border-l-4 border-blue-500 p-4 mb-6">
          <p className="text-sm text-gray-700">
            <strong>Optional: sovereign vaults.</strong> Custody wallets are derived from a shared OutLayer master by default
            &mdash; convenient and recovery-free. If your application&rsquo;s value-at-risk justifies an extra setup step,
            you can deploy a per-customer{' '}
            <Link href="/docs/vaults" className="text-[#cc6600] hover:underline">
              sovereign vault
            </Link>
            : the wallet&rsquo;s keys are then derived from a master that&rsquo;s recoverable by you through DAO cessation
            or your own unilateral exit, even if OutLayer ceases. The agent code does not change &mdash; the API key
            fully determines which master is used.
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
                <td className="px-4 py-2 font-mono">Intents balance</td>
                <td className="px-4 py-2">Cross-chain value, held on <code className="bg-gray-100 px-1 rounded">intents.near</code></td>
                <td className="px-4 py-2 font-mono text-xs">USDC, USDT, ETH, SOL...</td>
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
          The wallet is <strong>NEAR-native</strong>: its identity and signing key live on NEAR, and its cross-chain value is custodied
          on <code className="bg-gray-100 px-1 rounded">intents.near</code>. You move value across chains the way you would with a
          centralized exchange &mdash; deposit in, operate, withdraw out to an external address &mdash; all via the
          <strong> NEAR Intents protocol</strong> and the 1Click solver network. No bridges, and no gas tokens needed on the destination chain.
        </p>
        <p className="text-gray-700 mb-4">
          The wallet signs EVM payloads directly. <code className="bg-gray-100 px-1 rounded">GET /wallet/v1/address</code> returns the NEAR address
          and a single shared secp256k1 <code className="bg-gray-100 px-1 rounded">0x</code> address (the same across all EVM chains), and the wallet signs
          EIP-712 typed data, EIP-191 messages, and raw EVM transactions via <code className="bg-gray-100 px-1 rounded">/wallet/v1/evm/*</code> &mdash; you build
          and broadcast the transaction; the TEE only keccak-hashes and signs. Native Solana signing is still a planned extension. Cross-chain deposits and
          withdrawals via NEAR Intents do not need any of this.
        </p>

        <div className="overflow-x-auto mb-4">
          <table className="min-w-full text-sm border border-gray-200">
            <thead className="bg-gray-50">
              <tr>
                <th className="px-4 py-2 text-left font-semibold border-b">Chain</th>
                <th className="px-4 py-2 text-left font-semibold border-b">Deposit</th>
                <th className="px-4 py-2 text-left font-semibold border-b">Withdraw</th>
                <th className="px-4 py-2 text-left font-semibold border-b">Native address / contract calls</th>
              </tr>
            </thead>
            <tbody>
              <tr className="border-b">
                <td className="px-4 py-2 font-semibold">NEAR</td>
                <td className="px-4 py-2 text-green-600">&#10003; direct to Intents balance</td>
                <td className="px-4 py-2 text-green-600">&#10003; via Intents (gasless)</td>
                <td className="px-4 py-2 text-green-600">&#10003; NEAR address + direct contract calls</td>
              </tr>
              <tr className="border-b">
                <td className="px-4 py-2 font-semibold">Ethereum</td>
                <td className="px-4 py-2 text-green-600">&#10003; cross-chain (1Click)</td>
                <td className="px-4 py-2 text-green-600">&#10003; via Intents (gasless)</td>
                <td className="px-4 py-2 text-gray-400">&mdash; (planned)</td>
              </tr>
              <tr className="border-b">
                <td className="px-4 py-2 font-semibold">Bitcoin</td>
                <td className="px-4 py-2 text-green-600">&#10003; cross-chain (1Click)</td>
                <td className="px-4 py-2 text-green-600">&#10003; via Intents (gasless)</td>
                <td className="px-4 py-2 text-gray-400">&mdash; (planned)</td>
              </tr>
              <tr className="border-b">
                <td className="px-4 py-2 font-semibold">Solana</td>
                <td className="px-4 py-2 text-green-600">&#10003; cross-chain (1Click)</td>
                <td className="px-4 py-2 text-green-600">&#10003; via Intents (gasless)</td>
                <td className="px-4 py-2 text-gray-400">&mdash; (planned)</td>
              </tr>
              <tr>
                <td className="px-4 py-2 font-semibold">Base, Arbitrum, BSC, Polygon, Optimism, Avalanche</td>
                <td className="px-4 py-2 text-green-600">&#10003; cross-chain (1Click)</td>
                <td className="px-4 py-2 text-green-600">&#10003; via Intents (gasless)</td>
                <td className="px-4 py-2 text-gray-400">&mdash; (planned)</td>
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

        <div className="bg-red-50 border-l-4 border-red-500 p-4 mb-4">
          <p className="text-sm text-gray-800 font-semibold mb-1">⚠️ Only send whitelisted Intents assets &mdash; anything else is lost permanently</p>
          <p className="text-sm text-gray-700">
            Deposits only work for assets in the NEAR Intents / 1Click token catalog (<code className="bg-red-100 px-1 rounded">GET /wallet/v1/tokens</code>),
            sent on the exact chain the deposit address was issued for. If you send an unsupported token, the wrong token, a token on the wrong chain,
            an NFT, or a native gas coin that is not a listed asset, <strong>it cannot be credited or recovered</strong>. Deposit addresses from
            <code className="bg-red-100 px-1 rounded">/wallet/v1/intents/deposit/cross-chain</code> (legacy alias <code className="bg-red-100 px-1 rounded">/wallet/v1/deposit-intent</code>) are per-request and expire (30 min) &mdash; never reuse an old one or send after expiry.
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
                <td className="px-4 py-2">Restrict allowed operations: <code className="bg-gray-100 px-1 rounded">transfer</code>, <code className="bg-gray-100 px-1 rounded">call</code> (incl. deposits to Intents), <code className="bg-gray-100 px-1 rounded">withdraw</code>, <code className="bg-gray-100 px-1 rounded">swap</code>, <code className="bg-gray-100 px-1 rounded">cross_chain_withdraw</code> (separate, default-deny), <code className="bg-gray-100 px-1 rounded">delete</code></td>
                <td className="px-4 py-2 font-mono text-xs">call, swap only</td>
              </tr>
              <tr className="border-b">
                <td className="px-4 py-2 font-semibold">Capabilities</td>
                <td className="px-4 py-2">Opt-in gates for powerful primitives, all <strong>default-deny</strong> under a policy except <code className="bg-gray-100 px-1 rounded">sign_message</code> (default-allow): <code className="bg-gray-100 px-1 rounded">raw_sign</code> (+ per-chain allowlist), <code className="bg-gray-100 px-1 rounded">swap</code>, <code className="bg-gray-100 px-1 rounded">cross_chain_withdraw</code>, <code className="bg-gray-100 px-1 rounded">confidential</code>, <code className="bg-gray-100 px-1 rounded">payment_check</code>, <code className="bg-gray-100 px-1 rounded">sign_message</code> (+ recipient allowlist), <code className="bg-gray-100 px-1 rounded">evm_sign</code> (EVM EIP-712/EIP-191/raw-tx; <strong>default-DENY</strong> — set <code className="bg-gray-100 px-1 rounded">allowed:true</code> to permit, with a <code className="bg-gray-100 px-1 rounded">raw_tx</code> sub-flag default-OFF). A wallet with <strong>no policy</strong> is unrestricted. Each may also set <code className="bg-gray-100 px-1 rounded">requires_approval</code></td>
                <td className="px-4 py-2 font-mono text-xs">swap: allowed</td>
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

        <div className="rounded-lg border border-amber-300 bg-amber-50 p-4 mb-4">
          <p className="text-sm font-semibold text-amber-900 mb-1">Velocity limits are best-effort under concurrency</p>
          <p className="text-sm text-amber-900">
            Per-transaction limits, whitelists, time windows, capabilities, freeze, and the multisig
            trigger are enforced <strong>exactly</strong> inside the TEE on every signature.
            The <strong>cumulative</strong> limits &mdash; daily / hourly / monthly spend and the
            hourly transaction-count (<code>rate_limit</code>) &mdash; are checked against a usage
            counter the API gateway supplies; the TEE keeps no state of its own. If you fire several
            requests <strong>concurrently</strong>, they can each read the same pre-spend counter and
            all pass, so the cumulative caps may be exceeded by the in-flight batch (e.g. a
            &ldquo;60&nbsp;tx/hour&rdquo; cap could admit a few extra under a burst). Single-threaded
            agents (one request at a time) are unaffected.
          </p>
          <p className="text-sm text-amber-900 mt-2">
            If exact cumulative enforcement matters to you, either <strong>serialize</strong> your
            agent&rsquo;s spending requests (don&rsquo;t issue the next until the previous returns),
            or leave a <strong>safety margin</strong> in the limit to absorb the maximum number of
            requests you might have in flight at once. For hard stops use the per-transaction limit,
            multisig, or freeze &mdash; those are exact.
          </p>
        </div>

        <h3 className="text-lg font-semibold mt-6 mb-2">Example Policy</h3>
        <SyntaxHighlighter language="json" style={vscDarkPlus} customStyle={{ borderRadius: '0.5rem', fontSize: '0.875rem' }}>
{`{
  "rules": {
    "transaction_types": ["transfer", "call", "withdraw", "swap", "delete"],
    "allowed_tokens": ["*"],
    "addresses": {
      "mode": "whitelist",
      "list": ["bob.near", "dex.near"]
    },
    "limits": {
      "per_transaction": { "native": "10000000000000000000000000" },
      "daily": { "*": "100000000000000000000000000" },
      "hourly": { "*": "50000000000000000000000000" }
    },
    "rate_limit": { "max_per_hour": 60 },
    "time_restrictions": {
      "timezone": "UTC",
      "allowed_hours": [9, 17],
      "allowed_days": [1, 2, 3, 4, 5]
    }
  },
  "approval": {
    "threshold": { "required": 2 },
    "approvers": [
      { "id": "alice.near", "role": "admin",  "pubkey": "ed25519:<base58>" },
      { "id": "bob.near",   "role": "signer", "pubkey": "ed25519:<base58>" },
      { "id": "carol.near", "role": "signer", "pubkey": "ed25519:<base58>" }
    ]
  },
  "capabilities": {
    "raw_sign":     { "allowed": false, "chains": ["ethereum", "solana"], "requires_approval": true },
    "confidential": { "allowed": false },
    "sign_message": { "allowed": true,  "allowed_recipients": [] },
    "swap":         { "allowed": false },
    "cross_chain_withdraw": { "allowed": false },
    "payment_check": { "allowed": false },
    "evm_sign":     { "allowed": true,  "raw_tx": false }
  }
}`}
        </SyntaxHighlighter>
      </section>

      {/* Multisig */}
      <section id="multisig" className="mb-10 scroll-mt-4">
        <AnchorHeading id="multisig">Multisig Approval</AnchorHeading>

        <p className="text-gray-700 mb-4">
          On a wallet with an approval threshold, fund-moving operations go into a pending state.
          Designated approvers sign the request using their NEAR wallet (NEP-413 signature). Once the required number of signatures is collected, the transaction executes automatically.
        </p>

        <div className="bg-blue-50 border-l-4 border-blue-400 p-4 mb-4">
          <p className="text-sm text-gray-700">
            <strong>Multisig also covers NEAR Intents operations.</strong> On a multisig wallet, a
            swap &mdash; or any NEAR Intents <em>Trusted</em> operation (<code className="bg-blue-100 px-1 rounded">swap</code>,
            <code className="bg-blue-100 px-1 rounded">confidential</code>, <code className="bg-blue-100 px-1 rounded">cross_chain_withdraw</code>) &mdash;
            executes only after the required approvers confirm it. Approval controls <em>whether</em> the
            operation happens: the TEE verifies the approver signatures and pins the recipient. It does
            <strong>not</strong> itself re-check the token or amount &mdash; for these Trusted operations the
            gateway builds the 1Click artifact (quote &rarr; deposit address) at execution and the TEE signs it,
            trusting the gateway to have built it from the approved operation. The off-chain destination (the
            1Click deposit address) is the same way: provided by the gateway at execution and
            <strong>not independently verifiable</strong> by the TEE. So a compromised gateway could substitute
            the token, amount, or routing after approval &mdash; the on-chain guarantees are the recipient pin
            and the approver signatures, not the value terms (a documented tradeoff).
            Claimable links (<code className="bg-blue-100 px-1 rounded">payment_check</code>) are the exception &mdash;
            they are gated by their capability and the per-transaction amount cap rather than by multisig.
          </p>
        </div>

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

        <h3 className="text-lg font-semibold mt-4 mb-2">2. Get address</h3>
        <SyntaxHighlighter language="bash" style={vscDarkPlus} customStyle={{ borderRadius: '0.5rem', fontSize: '0.875rem' }}>
{`# NEAR address (wallet identity; chain=near only — native ETH/SOL
# addresses are not issued, see Multi-Chain Support above)
curl -s -H "Authorization: Bearer $API_KEY" \\
  "https://api.outlayer.fastnear.com/wallet/v1/address?chain=near"

# To fund from another chain, request a cross-chain deposit address
# (via 1Click / NEAR Intents). Pass a defuse \`source_asset\` from
# GET /wallet/v1/tokens — the source chain is derived from its prefix
# (e.g. \`eth-…\` → Ethereum).
# ⚠️ Only send the exact whitelisted token on the exact chain below —
#    any other asset sent to this address is lost permanently.
# (Legacy alias /wallet/v1/deposit-intent still works.)
curl -s -X POST -H "Content-Type: application/json" \\
  -H "Authorization: Bearer $API_KEY" \\
  -d '{"source_asset":"nep141:eth-0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48.omft.near","amount":"10000000"}' \\
  "https://api.outlayer.fastnear.com/wallet/v1/intents/deposit/cross-chain"

# Returned \`deposit_address\` format depends on the source chain:
# NEAR — 64-char hex implicit account; EVM — 0x + 40 hex;
# Solana — base58; Bitcoin — bc1…/1…/3….
# For NEAR-source funds prefer POST /wallet/v1/intents/deposit
# (one direct ft_transfer_call, ~3s, no solver hop).`}
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
  -d '{"to":"bob.near","amount":"1000000000000000000000000"}' \\
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

        <p className="text-gray-700 mt-2">
          If the transaction is broadcast but its execution <em>reverts on-chain</em> (contract panic,
          out of gas), the response is <strong>HTTP 422</strong> with
          <code className="bg-gray-100 px-1 rounded">error: onchain_tx_failed</code>, the decoded
          execution error, and the real <code className="bg-gray-100 px-1 rounded">tx_hash</code> &mdash;
          the transaction <em>is</em> on chain, so do <strong>not</strong> retry it. Same contract on
          <code className="bg-gray-100 px-1 rounded">/transfer</code>,
          <code className="bg-gray-100 px-1 rounded">/delete</code>,
          <code className="bg-gray-100 px-1 rounded">/storage-deposit</code> and
          <code className="bg-gray-100 px-1 rounded">/intents/deposit</code>.
          For operations that go through <strong>multisig approval</strong>, execution happens in the
          background after the threshold is met &mdash; a revert there surfaces as
          <code className="bg-gray-100 px-1 rounded">status: "failed"</code> via
          <code className="bg-gray-100 px-1 rounded">GET /wallet/v1/requests/&#123;id&#125;</code> and the
          <code className="bg-gray-100 px-1 rounded">request_completed</code> webhook, not as a
          synchronous 422.
        </p>

        <h3 className="text-lg font-semibold mt-4 mb-2">7. Withdraw (gasless cross-chain via Intents)</h3>
        <SyntaxHighlighter language="bash" style={vscDarkPlus} customStyle={{ borderRadius: '0.5rem', fontSize: '0.875rem' }}>
{`# Tokens must be in Intents balance first (use /intents/deposit or /swap).
# Only whitelisted Intents assets can be withdrawn (see GET /wallet/v1/tokens).

# Withdraw NATIVE NEAR (default for chain=near): intents.near unwraps your wNEAR
# and sends native NEAR. Gasless, and the recipient needs NO wrap.near storage.
# amount is in yoctoNEAR (24 decimals) — 1 NEAR = 1000000000000000000000000.
curl -s -X POST -H "Content-Type: application/json" \\
  -H "Authorization: Bearer $API_KEY" \\
  -d '{"to":"receiver.near","amount":"1000000000000000000000000","token":"near","chain":"near"}' \\
  "https://api.outlayer.fastnear.com/wallet/v1/intents/withdraw"

# Withdraw wNEAR (NEP-141) instead — explicit opt-in; recipient must be
# storage-registered on wrap.near (POST /wallet/v1/storage-deposit):
curl -s -X POST -H "Content-Type: application/json" \\
  -H "Authorization: Bearer $API_KEY" \\
  -d '{"to":"receiver.near","amount":"1000000000000000000000000","token":"nep141:wrap.near","chain":"near"}' \\
  "https://api.outlayer.fastnear.com/wallet/v1/intents/withdraw"

# Withdraw USDT (NEP-141) to a NEAR account (recipient must have usdt storage):
curl -s -X POST -H "Content-Type: application/json" \\
  -H "Authorization: Bearer $API_KEY" \\
  -d '{"to":"receiver.near","amount":"1000000","token":"usdt.tether-token.near","chain":"near"}' \\
  "https://api.outlayer.fastnear.com/wallet/v1/intents/withdraw"

# Withdraw cross-chain to an external address (1Click bridges + delivers native):
curl -s -X POST -H "Content-Type: application/json" \\
  -H "Authorization: Bearer $API_KEY" \\
  -d '{"to":"0xRecipient...","amount":"1000000","token":"usdt.tether-token.near","chain":"ethereum"}' \\
  "https://api.outlayer.fastnear.com/wallet/v1/intents/withdraw"`}
        </SyntaxHighlighter>

        <h3 className="text-lg font-semibold mt-4 mb-2">8. Transfer inside Intents (to another account)</h3>
        <p className="text-gray-700 mb-2">
          Move a token from your Intents balance to <strong>another account&apos;s</strong> Intents balance, gasless, staying <strong>inside</strong> <code className="bg-gray-100 px-1 rounded">intents.near</code> &mdash; the recipient is credited there, nothing lands on the public chain. This is <strong>not</strong> a withdrawal: use it when the recipient also holds an Intents balance (e.g. another OutLayer custody wallet); use <code className="bg-gray-100 px-1 rounded">/intents/withdraw</code> to deliver to a plain on-chain account instead. NEAR-only (no <code className="bg-gray-100 px-1 rounded">chain</code> field); <code className="bg-gray-100 px-1 rounded">token</code> is required (to send NEAR, transfer <code className="bg-gray-100 px-1 rounded">nep141:wrap.near</code>). The recipient need not exist on-chain.
        </p>
        <SyntaxHighlighter language="bash" style={vscDarkPlus} customStyle={{ borderRadius: '0.5rem', fontSize: '0.875rem' }}>
{`# Transfer 1 USDT to another account's Intents balance (gasless, stays inside Intents):
curl -s -X POST -H "Content-Type: application/json" \\
  -H "Authorization: Bearer $API_KEY" \\
  -d '{"to":"partner.near","amount":"1000000","token":"nep141:usdt.tether-token.near"}' \\
  "https://api.outlayer.fastnear.com/wallet/v1/intents/transfer"`}
        </SyntaxHighlighter>

        <h3 className="text-lg font-semibold mt-4 mb-2">9. Delete wallet (irreversible)</h3>
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

        <h3 className="text-lg font-semibold mt-4 mb-2">10. Configure policy (optional)</h3>
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
                <td className="px-4 py-2">Sign EVM typed data (EIP-712)</td>
                <td className="px-4 py-2 font-mono">POST</td>
                <td className="px-4 py-2 font-mono text-xs">/wallet/v1/evm/sign-typed-data</td>
              </tr>
              <tr className="border-b">
                <td className="px-4 py-2">Sign EVM message (EIP-191)</td>
                <td className="px-4 py-2 font-mono">POST</td>
                <td className="px-4 py-2 font-mono text-xs">/wallet/v1/evm/sign-message</td>
              </tr>
              <tr className="border-b">
                <td className="px-4 py-2">Sign raw EVM transaction <span className="text-gray-400">(needs <code className="bg-gray-100 px-1 rounded">evm_sign.raw_tx</code>; client serializes &amp; broadcasts)</span></td>
                <td className="px-4 py-2 font-mono">POST</td>
                <td className="px-4 py-2 font-mono text-xs">/wallet/v1/evm/sign-transaction</td>
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
                <td className="px-4 py-2">Transfer inside Intents (to another account&apos;s intents balance)</td>
                <td className="px-4 py-2 font-mono">POST</td>
                <td className="px-4 py-2 font-mono text-xs">/wallet/v1/intents/transfer</td>
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
              <tr className="border-b">
                <td className="px-4 py-2">Audit log</td>
                <td className="px-4 py-2 font-mono">GET</td>
                <td className="px-4 py-2 font-mono text-xs">/wallet/v1/audit</td>
              </tr>
              <tr className="border-b bg-blue-50/30">
                <td className="px-4 py-2">Register (deterministic)</td>
                <td className="px-4 py-2 font-mono">POST</td>
                <td className="px-4 py-2 font-mono text-xs">/register (with NEAR sig body)</td>
              </tr>
              <tr className="border-b bg-blue-50/30">
                <td className="px-4 py-2">Register delegate key</td>
                <td className="px-4 py-2 font-mono">PUT</td>
                <td className="px-4 py-2 font-mono text-xs">/wallet/v1/api-key</td>
              </tr>
              <tr className="bg-blue-50/30">
                <td className="px-4 py-2">Revoke delegate key</td>
                <td className="px-4 py-2 font-mono">DELETE</td>
                <td className="px-4 py-2 font-mono text-xs">/wallet/v1/api-key/&#123;key_hash&#125;</td>
              </tr>
            </tbody>
          </table>
        </div>

        <p className="text-gray-700 mb-3">
          Base URL: <code className="bg-gray-100 px-1 rounded">https://api.outlayer.fastnear.com</code> (mainnet)
          {' · '}
          <code className="bg-gray-100 px-1 rounded">https://testnet-api.outlayer.fastnear.com</code> (testnet)
        </p>

        <div className="bg-yellow-50 border-l-4 border-yellow-400 p-4 text-sm text-gray-700">
          <strong>NEAR Intents is mainnet-only.</strong> There are no testnet Intents solvers, so on
          testnet the coordinator returns <strong>HTTP 503</strong> for every intents-dependent
          endpoint — namely{' '}
          <code className="bg-gray-100 px-1 rounded">/wallet/v1/intents/*</code>{' '}
          (deposit, withdraw, swap, cross-chain deposit, payment-check, and their quote / dry-run
          variants), cross-chain gasless withdrawals, and all{' '}
          <code className="bg-gray-100 px-1 rounded">/wallet/v1/confidential/*</code> routes.
          Test these against the <strong>mainnet</strong> API only. Account, address,
          balance, transfer, contract <code className="bg-gray-100 px-1 rounded">call</code>,
          message signing, policy, approval, and delete endpoints work on both networks.
        </div>
      </section>

      {/* Confidential Intents */}
      <section id="confidential-intents" className="mb-10 scroll-mt-4">
        <AnchorHeading id="confidential-intents">Confidential Intents</AnchorHeading>

        <p className="text-gray-700 mb-4">
          Move balances between your <strong>public</strong> intents shard, a{' '}
          <strong>confidential</strong> shielded-pool shard, and external chains — same
          TEE-mediated signing, a different shard. The{' '}
          <code className="bg-gray-100 px-1 rounded">/wallet/v1/confidential/*</code> routes mirror{' '}
          <code className="bg-gray-100 px-1 rounded">/wallet/v1/intents/*</code>. Like all Intents
          flows they are <strong>mainnet-only</strong> — they return{' '}
          <code className="bg-gray-100 px-1 rounded">HTTP 503</code> on testnet (no testnet solvers)
          or wherever the deployment has not enabled confidential intents. The confidential shard is the{' '}
          <code className="bg-gray-100 px-1 rounded">intents.far</code> contract on a private NEAR
          shard with no public RPC: balances are <strong>real on-chain state</strong> there, just
          not publicly readable. Full integration guide:{' '}
          <a className="text-blue-600 underline" href="https://github.com/out-layer/coordinator/blob/main/docs/CONFIDENTIAL_INTENTS.md">
            CONFIDENTIAL_INTENTS.md
          </a>.
        </p>

        <ul className="list-disc list-inside text-gray-700 space-y-1 mb-4">
          <li><code className="bg-gray-100 px-1 rounded">POST /confidential/shield</code> — SHIELD: public intents → confidential (legacy alias <code className="bg-gray-100 px-1 rounded">/confidential/deposit</code>, still works)</li>
          <li><code className="bg-gray-100 px-1 rounded">POST /confidential/unshield</code> — confidential → public intents</li>
          <li><code className="bg-gray-100 px-1 rounded">POST /confidential/withdraw</code> — confidential → external chain (or <code className="bg-gray-100 px-1 rounded">chain=&quot;near&quot;</code> for native NEAR delivery via <code className="bg-gray-100 px-1 rounded">native_withdraw</code>)</li>
          <li><code className="bg-gray-100 px-1 rounded">POST /confidential/transfer</code> — private confidential → confidential transfer</li>
          <li><code className="bg-gray-100 px-1 rounded">POST /confidential/swap</code> (+ <code className="bg-gray-100 px-1 rounded">/swap/quote</code>, <code className="bg-gray-100 px-1 rounded">/withdraw/dry-run</code>) &mdash; on a multisig wallet, like a public swap, approval controls <em>whether</em> it runs (the TEE does not itself re-check the swap&apos;s token/amount against the artifact &mdash; that relies on gateway trust)</li>
          <li><code className="bg-gray-100 px-1 rounded">POST /confidential/deposit/cross-chain</code> — cross-chain deposit (via 1Click / NEAR Intents; legacy alias <code className="bg-gray-100 px-1 rounded">/confidential/deposit-intent</code>, still works)</li>
          <li><code className="bg-gray-100 px-1 rounded">GET /confidential/balance</code> — read confidential balances</li>
        </ul>

        <p className="text-gray-700 mb-4">
          Action routes are asynchronous — they return{' '}
          <code className="bg-gray-100 px-1 rounded">request_id</code> with status{' '}
          <code className="bg-gray-100 px-1 rounded">pending_deposit</code>; poll{' '}
          <code className="bg-gray-100 px-1 rounded">GET /wallet/v1/requests/&#123;id&#125;</code> until terminal.
        </p>

        <div className="bg-amber-50 border-l-4 border-amber-500 p-4 mb-4">
          <p className="text-sm text-gray-700">
            <strong>Privacy is nuanced.</strong> Confidential balances are <strong>real on-chain
            state on a private shard</strong> (<code className="bg-amber-100 px-1 rounded">intents.far</code>)
            with no public RPC — chain-graph bots cannot read them. <strong>SHIELD/UNSHIELD link
            your wallet on the public chain</strong> (entry/exit reveal). Cross-chain
            deposit/withdraw keep your NEAR wallet off the public chain — only the external-chain
            sender/receiver is public, on that chain. <strong>Never hidden:</strong> the shard
            itself is an auditable smart contract — the Defuse/1Click solver layer (sees plaintext
            intents), the partner mapping, your source-chain identity, and the shard operator /
            auditors / law enforcement with a warrant can all see confidential state.{' '}
            <strong>For unlinkability</strong>, fund via cross-chain deposit and exit via
            cross-chain withdraw rather than SHIELD/UNSHIELD — only one confidential identity per
            wallet, so multi-op unlinkability is not achievable today.
          </p>
        </div>

        <SyntaxHighlighter language="bash" style={vscDarkPlus} customStyle={{ borderRadius: '0.5rem', fontSize: '0.875rem' }}>
{`# SHIELD 0.01 wNEAR into the confidential shard
# (legacy alias /wallet/v1/confidential/deposit still works)
curl -s -X POST -H "Content-Type: application/json" \\
  -H "Authorization: Bearer $API_KEY" \\
  -d '{"token":"nep141:wrap.near","amount":"10000000000000000000000"}' \\
  "https://api.outlayer.fastnear.com/wallet/v1/confidential/shield"

# Read confidential balances
curl -s -H "Authorization: Bearer $API_KEY" \\
  "https://api.outlayer.fastnear.com/wallet/v1/confidential/balance"`}
        </SyntaxHighlighter>
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

      {/* Deterministic Wallets */}
      <section id="deterministic-wallets" className="mb-10 scroll-mt-4">
        <AnchorHeading id="deterministic-wallets">Deterministic Wallets</AnchorHeading>

        <p className="text-gray-700 mb-4">
          For servers, bots, and agents with a NEAR account: create wallets that require <strong>zero per-user key storage</strong>.
          The wallet ID is derived from <code className="bg-gray-100 px-1 rounded">(account_id, seed, vault_or_none)</code> &mdash; same inputs always produce the same wallet, and different vault scopes legitimately mint independent sub-wallets under the same seed.
          Auth uses NEAR ed25519 signatures on every request instead of stored API keys.
          Seed format: <code className="bg-gray-100 px-1 rounded">[a-zA-Z0-9._-]</code>, 1-256 chars.
        </p>

        <div className="bg-blue-50 border-l-4 border-blue-500 p-4 mb-6">
          <p className="text-sm text-gray-700">
            <strong>Zero stored secrets.</strong> The coordinator stores no auth credentials for deterministic wallets. Key revocation = remove the key from your NEAR account.
            Access is revoked within 60 seconds (cache TTL). No coordinator action needed.
          </p>
        </div>

        <div className="bg-yellow-50 border-l-4 border-yellow-500 p-4 mb-6">
          <p className="text-sm text-gray-700">
            <strong>NEAR signature auth</strong> is for integrators who have their own NEAR account key (bots, servers).
            Custody wallets (<code className="bg-gray-100 px-1 rounded">wk_</code> API key) can also create sub-agents &mdash; just pass your Bearer header to <code className="bg-gray-100 px-1 rounded">PUT /wallet/v1/api-key</code> with <code className="bg-gray-100 px-1 rounded">seed</code> and <code className="bg-gray-100 px-1 rounded">key_hash</code> in body, no NEAR signatures needed.
          </p>
        </div>

        <div className="grid grid-cols-1 md:grid-cols-3 gap-4 mb-6">
          <div className="border border-gray-200 rounded-lg p-4">
            <h4 className="font-semibold text-gray-900 mb-1">Telegram / Discord Bot</h4>
            <p className="text-xs text-gray-600">One NEAR key in env. <code className="bg-gray-100 px-0.5 rounded text-xs">seed = SHA256(user_id)</code>. Creates wallets for thousands of users. Zero per-user DB.</p>
          </div>
          <div className="border border-gray-200 rounded-lg p-4">
            <h4 className="font-semibold text-gray-900 mb-1">Web App (OAuth)</h4>
            <p className="text-xs text-gray-600">Server has one NEAR key. <code className="bg-gray-100 px-0.5 rounded text-xs">seed = SHA256(provider:user_id)</code>. Google login &rarr; instant wallet.</p>
          </div>
          <div className="border border-gray-200 rounded-lg p-4">
            <h4 className="font-semibold text-gray-900 mb-1">Sub-agents</h4>
            <p className="text-xs text-gray-600">Parent derives <code className="bg-gray-100 px-0.5 rounded text-xs">wk_</code> keys from NEAR key + seed. Sub-agent uses simple Bearer token &mdash; no crypto.</p>
          </div>
        </div>

        <h3 className="text-lg font-semibold mt-6 mb-2">Two auth methods</h3>
        <div className="overflow-x-auto mb-4">
          <table className="min-w-full text-sm border border-gray-200">
            <thead className="bg-gray-50">
              <tr>
                <th className="px-4 py-2 text-left font-semibold border-b">Header</th>
                <th className="px-4 py-2 text-left font-semibold border-b">For</th>
                <th className="px-4 py-2 text-left font-semibold border-b">Stored secrets</th>
              </tr>
            </thead>
            <tbody>
              <tr className="border-b">
                <td className="px-4 py-2 font-mono text-xs">Bearer wk_...</td>
                <td className="px-4 py-2">Random wallets, sub-agents</td>
                <td className="px-4 py-2">API key hash in DB</td>
              </tr>
              <tr>
                <td className="px-4 py-2 font-mono text-xs">Bearer near:&lt;base64url&gt;</td>
                <td className="px-4 py-2">Deterministic wallets</td>
                <td className="px-4 py-2">Nothing &mdash; verified via NEAR RPC</td>
              </tr>
            </tbody>
          </table>
        </div>

        <h3 className="text-lg font-semibold mt-6 mb-2">Signature format</h3>
        <p className="text-gray-700 mb-2">
          All signatures are <strong>raw ed25519</strong> &mdash; sign the message string bytes directly with your NEAR key, then base58-encode the 64-byte result.
          This is <strong>NOT</strong> NEP-413 (the <code className="bg-gray-100 px-1 rounded">/sign-message</code> endpoint returns NEP-413 signatures, which are a different format and won&apos;t work here).
        </p>
        <div className="overflow-x-auto mb-4">
          <table className="min-w-full text-sm border border-gray-200">
            <thead className="bg-gray-50">
              <tr>
                <th className="px-4 py-2 text-left font-semibold border-b">Field</th>
                <th className="px-4 py-2 text-left font-semibold border-b">Format</th>
                <th className="px-4 py-2 text-left font-semibold border-b">Example</th>
              </tr>
            </thead>
            <tbody>
              <tr className="border-b">
                <td className="px-4 py-2 font-mono text-xs">pubkey</td>
                <td className="px-4 py-2">With <code className="bg-gray-100 px-1 rounded">ed25519:</code> prefix</td>
                <td className="px-4 py-2 font-mono text-xs">ed25519:6E8sCc...</td>
              </tr>
              <tr>
                <td className="px-4 py-2 font-mono text-xs">signature</td>
                <td className="px-4 py-2">Base58, <strong>no</strong> prefix</td>
                <td className="px-4 py-2 font-mono text-xs">4dJh2r...</td>
              </tr>
            </tbody>
          </table>
        </div>

        <h3 className="text-lg font-semibold mt-6 mb-2">Register a deterministic wallet</h3>
        <SyntaxHighlighter language="bash" style={vscDarkPlus} customStyle={{ borderRadius: '0.5rem', fontSize: '0.875rem' }}>
{`# Sign "register:<seed>:<timestamp>" with your NEAR ed25519 key (raw, not NEP-413)
curl -s -X POST -H "Content-Type: application/json" \\
  -d '{
    "account_id": "my-bot.near",
    "seed": "user-42",
    "pubkey": "ed25519:<base58_pubkey>",
    "message": "register:user-42:1712000000",
    "signature": "<base58_signature>"
  }' \\
  "https://api.outlayer.fastnear.com/register"

# Timestamp window: ±5 minutes for registration
# Response: { "wallet_id": "...", "near_account_id": "..." }
# No api_key — use Bearer near:... for all requests`}
        </SyntaxHighlighter>

        <h3 className="text-lg font-semibold mt-6 mb-2">Bearer near: token format</h3>
        <p className="text-gray-700 mb-2">
          Base64url-encode a JSON object. The signed message format depends on whether you include a vault scope (±30s window):
        </p>
        <ul className="list-disc pl-6 text-gray-700 mb-2 text-sm space-y-1">
          <li>No vault: <code className="bg-gray-100 px-1 rounded">auth:&lt;seed&gt;:&lt;timestamp&gt;</code></li>
          <li>With vault: <code className="bg-gray-100 px-1 rounded">auth:&lt;seed&gt;:&lt;timestamp&gt;:&lt;vault_id&gt;</code> — vault_id MUST be inside the signed message (not only in JSON), otherwise verify fails with 401.</li>
        </ul>
        <SyntaxHighlighter language="json" style={vscDarkPlus} customStyle={{ borderRadius: '0.5rem', fontSize: '0.875rem' }}>
{`{
  "account_id": "my-bot.near",
  "seed": "user-42",
  "pubkey": "ed25519:<base58>",
  "timestamp": 1712000000,
  "signature": "<base58>",
  "vault_id": "vault.my-bot.near"   // optional; include in signed message too
}`}
        </SyntaxHighlighter>

        <div className="bg-blue-50 border-l-4 border-blue-400 p-4 mb-4 mt-3">
          <p className="text-sm text-gray-700">
            <strong>Key lives in the TEE?</strong> The <code className="bg-blue-100 px-1 rounded">signature</code> above assumes you hold the NEAR ed25519 key locally. For a wallet whose key lives in OutLayer custody (the TEE), you don&apos;t have the private key &mdash; instead call <code className="bg-blue-100 px-1 rounded">POST /wallet/v1/auth-sign</code> with <code className="bg-blue-100 px-1 rounded">{`{ "purpose": "bearer", "seed": "<seed>" }`}</code> and it returns the <code className="bg-blue-100 px-1 rounded">auth:&lt;seed&gt;:&lt;timestamp&gt;</code> message and signature (signed inside the TEE with a fresh server timestamp) to drop into the token.
          </p>
        </div>

        <h3 className="text-lg font-semibold mt-6 mb-2">Delegate keys for sub-agents</h3>
        <p className="text-gray-700 mb-2">
          Register a <code className="bg-gray-100 px-1 rounded">wk_</code> key hash so a sub-agent can use simple Bearer auth.
          Works from both custody wallets (<code className="bg-gray-100 px-1 rounded">Bearer wk_...</code>) and external NEAR accounts (signature in body).
        </p>
        <SyntaxHighlighter language="bash" style={vscDarkPlus} customStyle={{ borderRadius: '0.5rem', fontSize: '0.875rem' }}>
{`# From a custody wallet — just pass your Bearer token, no NEAR signatures needed
# key_hash = SHA256("wk_" + derived_key_hex)
curl -s -X PUT -H "Authorization: Bearer $API_KEY" -H "Content-Type: application/json" \\
  -d '{"seed": "sub-task", "key_hash": "<sha256_hex_of_wk_key>"}' \\
  "https://api.outlayer.fastnear.com/wallet/v1/api-key"
# Response: { "wallet_id": "...", "near_account_id": "..." }

# Sub-agent: simple Bearer token, no crypto
curl -H "Authorization: Bearer wk_derived_key_here" \\
  "https://api.outlayer.fastnear.com/wallet/v1/balance?chain=near"

# Revoke: DELETE /wallet/v1/api-key/{key_hash}
# Returns 409 if last active key for the wallet`}
        </SyntaxHighlighter>

        <h3 className="text-lg font-semibold mt-6 mb-2">Key rotation</h3>
        <p className="text-gray-700 mb-4">
          No endpoint needed. Add a new key to your NEAR account, start signing with it, remove the old key.
          Old key access is revoked within 60 seconds (cache TTL). Wallet identity is tied to <code className="bg-gray-100 px-1 rounded">(account_id, seed, vault_or_none)</code>, not to which key signs.
        </p>
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
curl -s https://skills.outlayer.ai/agent-custody/SKILL.md`}
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
