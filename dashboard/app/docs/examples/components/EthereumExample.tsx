import Link from 'next/link';
import { Prism as SyntaxHighlighter } from 'react-syntax-highlighter';
import { vscDarkPlus } from 'react-syntax-highlighter/dist/esm/styles/prism';
import { ExampleCard, UseCasesSection, KeyFeaturesSection, TechnicalDetailsSection, CodeExampleSection, LearnMoreSection } from './';

export function EthereumExample() {
  const badges = (
    <>
      <span className="ml-3 text-sm bg-purple-100 text-purple-800 px-3 py-1 rounded">WASI P2</span>
      <span className="ml-2 text-sm bg-red-100 text-red-800 px-3 py-1 rounded">Advanced</span>
    </>
  );

  return (
    <ExampleCard
      id="ethereum-api"
      title="Ethereum API Oracle"
      badges={badges}
      githubUrl="https://github.com/zavodil/oracle-ark"
      playgroundId="ethereum-api"
    >
      <p className="text-gray-700 mb-4">
        Query Ethereum blockchain data via JSON-RPC (balances, smart contracts, transactions). Similar to the previous oracle example but configured for Ethereum node providers like Alchemy. Bridge NEAR with Ethereum data for cross-chain applications.
      </p>

      <KeyFeaturesSection items={[
        'Ethereum JSON-RPC API integration (eth_getBalance, eth_call, etc.)',
        'Support for Alchemy, Infura, and other Ethereum node providers',
        'Custom request structure with JSON path extraction',
        'Cross-chain data oracle for NEAR ↔ Ethereum bridges',
        'Encrypted API key storage (Alchemy secrets profile)',
        'Production-ready on testnet and mainnet'
      ]} />

      <CodeExampleSection
        title="Input Example (Check ETH Balance):"
        code={`{
  "requests": [
    {
      "id": "alchemy",
      "sources": [
        {
          "name": "custom",
          "custom": {
            "url": "https://eth-mainnet.g.alchemy.com/v2",
            "method": "POST",
            "body": {
              "method": "eth_getBalance",
              "params": ["0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045", "latest"],
              "id": 1,
              "jsonrpc": "2.0"
            },
            "json_path": "result",
            "value_type": "string"
          }
        }
      ]
    }
  ],
  "max_price_deviation_percent": 10.0
}`}
      />

      <CodeExampleSection
        title="Output Example:"
        code={`{
  "results": [
    {
      "id": "alchemy",
      "value": "0x1bc16d674ec80000",
      "sources_used": 1
    }
  ]
}`}
      />

      <h4 className="font-semibold mt-4 mb-2">Quick Start (Testnet):</h4>
      <SyntaxHighlighter language="bash" style={vscDarkPlus} customStyle={{ borderRadius: '0.5rem', fontSize: '0.875rem' }}>
{`# Using pre-configured Alchemy secrets on testnet
near contract call-function as-transaction outlayer.testnet request_execution \\
  json-args '{
    "code_source": {
      "repo": "https://github.com/zavodil/oracle-ark",
      "commit": "main",
      "build_target": "wasm32-wasip2"
    },
    "secrets_ref": {
      "profile": "alchemy",
      "account_id": "zavodil2.testnet"
    },
    "resource_limits": {
      "max_instructions": 100000000,
      "max_memory_mb": 128,
      "max_execution_seconds": 60
    },
    "input_data": "{\\"requests\\":[{\\"id\\":\\"alchemy\\",\\"sources\\":[{\\"name\\":\\"custom\\",\\"custom\\":{\\"url\\":\\"https://eth-mainnet.g.alchemy.com/v2\\",\\"method\\":\\"POST\\",\\"body\\":{\\"method\\":\\"eth_getBalance\\",\\"params\\":[\\"0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045\\",\\"latest\\"],\\"id\\":1,\\"jsonrpc\\":\\"2.0\\"},\\"json_path\\":\\"result\\",\\"value_type\\":\\"string\\"}}]}],\\"max_price_deviation_percent\\":10.0}"
  }' \\
  prepaid-gas '300.0 Tgas' \\
  attached-deposit '0.1 NEAR' \\
  sign-as your-account.testnet \\
  network-config testnet \\
  sign-with-keychain send`}
      </SyntaxHighlighter>

      <UseCasesSection items={[
        <><strong>Cross-Chain Bridges:</strong> Verify Ethereum transactions on NEAR</>,
        <><strong>DeFi Integration:</strong> Monitor Ethereum token balances from NEAR contracts</>,
        <><strong>Multi-Chain Wallets:</strong> Display ETH balances in NEAR apps</>,
        <><strong>Smart Contract State:</strong> Read Ethereum contract data (ERC20, NFTs)</>,
        <><strong>Block Explorer:</strong> Query Ethereum transaction history</>
      ]} />

      <h4 className="font-semibold mt-4 mb-2">Notes:</h4>
      <ul className="list-disc list-inside text-gray-700 space-y-1">
        <li>Same codebase as <code className="bg-gray-100 px-2 py-1 rounded">oracle-ark</code>, different secrets configuration</li>
        <li>Requires Alchemy API key (free tier: 300M compute units/month)</li>
        <li>Supports any Ethereum JSON-RPC method (eth_call, eth_getTransactionReceipt, etc.)</li>
        <li>Pre-configured secrets available on testnet: <code className="bg-gray-100 px-2 py-1 rounded">zavodil2.testnet</code></li>
        <li>Mainnet secrets: <code className="bg-gray-100 px-2 py-1 rounded">zavodil.near</code></li>
      </ul>

      <TechnicalDetailsSection items={[
        <><strong>WASI Version:</strong> Preview 2 (component model)</>,
        <><strong>Language:</strong> Rust</>,
        <><strong>HTTP Client:</strong> <code>reqwest</code> with WASI sockets</>,
        <><strong>Secrets:</strong> Required (<code>ALCHEMY_API_KEY</code>)</>,
        <><strong>Network:</strong> Required (outbound HTTPS to Alchemy/Infura)</>,
        <><strong>Build:</strong> <code>cargo component build --release</code></>,
        <><strong>Size:</strong> ~3.5MB compiled WASM</>
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
            🎮 <Link href="/playground#ethereum-api" className="text-[var(--primary-orange)] hover:underline">
              Try in Playground
            </Link>
          </li>
          <li>
            💻 <a href="https://github.com/zavodil/oracle-ark" target="_blank" rel="noopener noreferrer" className="text-[var(--primary-orange)] hover:underline">
              View Source Code
            </a>
          </li>
        </ul>
      </LearnMoreSection>
    </ExampleCard>
  );
}
