import Link from 'next/link';
import { Prism as SyntaxHighlighter } from 'react-syntax-highlighter';
import { vscDarkPlus } from 'react-syntax-highlighter/dist/esm/styles/prism';
import { ExampleCard, KeyFeaturesSection, TechnicalDetailsSection, LearnMoreSection } from './';

export function IntentsExample() {
  const badges = (
    <>
      <span className="ml-3 text-sm bg-purple-100 text-purple-800 px-3 py-1 rounded">WASI P2</span>
      <span className="ml-2 text-sm bg-red-100 text-red-800 px-3 py-1 rounded">Advanced</span>
    </>
  );

  return (
    <ExampleCard
      id="intents-ark"
      title="intents-ark"
      badges={badges}
      githubUrl="https://github.com/zavodil/intents-ark"
      playgroundId="near-intents-swap"
    >
      <p className="text-gray-700 mb-4">
        DEX token swaps via NEAR Intents protocol. User&apos;s FT transfer transaction pauses, WASI performs swap off-chain, then resolves by sending swapped tokens back - all within single transaction.
      </p>

      <KeyFeaturesSection items={[
        'FT transfer pauses during off-chain swap execution',
        'NEAR Intents API integration (quote → publish → settle)',
        'NEP-413 message signing with ed25519',
        'Automatic token withdrawal to user upon completion',
        'Private key management via encrypted secrets',
        'Storage deposit handling for fungible tokens'
      ]} />

      <h4 className="font-semibold mt-4 mb-2">Transaction Flow:</h4>
      <ol className="list-decimal list-inside text-gray-700 space-y-2 mb-4 ml-4">
        <li>User calls <code className="bg-gray-100 px-2 py-1 rounded">ft_transfer_call</code> to swap contract</li>
        <li>Contract receives tokens and calls OutLayer</li>
        <li><strong>Transaction pauses</strong> - contract enters yield state</li>
        <li>WASI worker performs swap via NEAR Intents API</li>
        <li>Worker withdraws swapped tokens to user</li>
        <li><strong>Transaction resumes</strong> - contract completes with callback</li>
      </ol>

      <h4 className="font-semibold mt-4 mb-2">How to Use:</h4>
      <SyntaxHighlighter language="bash" style={vscDarkPlus} customStyle={{ borderRadius: '0.5rem', fontSize: '0.875rem' }}>
{`# 1. Create operator account on NEAR mainnet
near create-account operator.near --useFaucet

# 2. Store operator private key as encrypted secret
# Open https://outlayer.fastnear.com/secrets:
# - Secrets JSON: {"OPERATOR_PRIVATE_KEY":"ed25519:..."}

# 3. Clone and deploy swap contract
git clone https://github.com/zavodil/intents-ark.git
cd intents-ark/contract
cargo near build
near deploy swap-contract.near res/swap_contract.wasm

# 4. Initialize contract
near call swap-contract.near new '{
  "operator_account_id": "operator.near"
}' --accountId swap-contract.near

# 5. User swaps 1 WNEAR for USDC
near call wrap.near ft_transfer_call '{
  "receiver_id": "swap-contract.near",
  "amount": "1000000000000000000000000",
  "msg": "{\\"Swap\\":{\\"token_out\\":\\"usdc.token\\",\\"min_amount_out\\":\\"900000\\"}}"
}' --accountId user.near --depositYocto 1 --gas 300000000000000

# Transaction will:
# - Pause after receiving WNEAR
# - Execute swap off-chain via NEAR Intents
# - Resume and send swapped USDC to user`}
      </SyntaxHighlighter>

      <div className="mt-4 p-3 bg-yellow-50 border-l-4 border-yellow-400">
        <p className="text-sm text-gray-700">
          <strong>Note:</strong> NEAR Intents works on mainnet only. For testnet testing, use mock swap implementation.
        </p>
      </div>

      <TechnicalDetailsSection items={[
        <><strong>WASI Version:</strong> Preview 2 (component model)</>,
        <><strong>Language:</strong> Rust</>,
        <><strong>HTTP Client:</strong> <code>reqwest</code> for NEAR Intents API</>,
        <><strong>Secrets:</strong> Required (<code>OPERATOR_PRIVATE_KEY</code>)</>,
        <><strong>Network:</strong> Required (NEAR Intents API, mainnet only)</>,
        <><strong>Signing:</strong> NEP-413 message signing with ed25519</>,
        <><strong>Build:</strong> <code>cargo component build --release</code></>,
        <><strong>Size:</strong> ~3.8MB compiled WASM</>
      ]} />

      <LearnMoreSection>
        <ul className="text-sm text-gray-700 space-y-1">
          <li>
            📖 <Link href="/docs/wasi#wasi-preview-2" className="text-[var(--primary-orange)] hover:underline">
              WASI Preview 2 Documentation
            </Link>
          </li>
          <li>
            🔐 <Link href="/docs/secrets" className="text-[var(--primary-orange)] hover:underline">
              Secrets Management Guide
            </Link>
          </li>
          <li>
            🎮 <Link href="/playground#near-intents-swap" className="text-[var(--primary-orange)] hover:underline">
              Try in Playground (Mainnet only)
            </Link>
          </li>
          <li>
            💻 <a href="https://github.com/zavodil/intents-ark" target="_blank" rel="noopener noreferrer" className="text-[var(--primary-orange)] hover:underline">
              View Source Code
            </a>
          </li>
          <li>
            🌐 <a href="https://intents.near.org" target="_blank" rel="noopener noreferrer" className="text-[var(--primary-orange)] hover:underline">
              NEAR Intents Protocol
            </a>
          </li>
        </ul>
      </LearnMoreSection>
    </ExampleCard>
  );
}
