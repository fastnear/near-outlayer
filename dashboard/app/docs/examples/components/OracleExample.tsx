import Link from 'next/link';
import { Prism as SyntaxHighlighter } from 'react-syntax-highlighter';
import { vscDarkPlus } from 'react-syntax-highlighter/dist/esm/styles/prism';
import { ExampleCard, KeyFeaturesSection, TechnicalDetailsSection, CodeExampleSection, LearnMoreSection } from './';

export function OracleExample() {
  const badges = (
    <>
      <span className="ml-3 text-sm bg-purple-100 text-purple-800 px-3 py-1 rounded">WASI P2</span>
      <span className="ml-2 text-sm bg-red-100 text-red-800 px-3 py-1 rounded">Advanced</span>
    </>
  );

  return (
    <ExampleCard
      id="oracle-ark"
      title="oracle-ark"
      badges={badges}
      githubUrl="https://github.com/zavodil/oracle-ark"
      playgroundId="multi-source-data-oracle"
    >
      <p className="text-gray-700 mb-4">
        Multi-source price oracle with aggregation and validation. Production-ready decentralized oracle for cryptocurrency and commodity prices.
      </p>

      <KeyFeaturesSection items={[
        'Multiple API sources: CoinGecko, CoinMarketCap, TwelveData',
        'Price aggregation methods: average, median, weighted',
        'Deviation validation and error handling',
        'Batch requests for multiple tokens (up to 10)',
        'API key management via encrypted secrets',
        'Comprehensive error reporting per source'
      ]} />

      <h4 className="font-semibold mt-4 mb-2">Supported Assets:</h4>
      <ul className="list-disc list-inside text-gray-700 space-y-1 mb-4 ml-4">
        <li><strong>Cryptocurrencies:</strong> Bitcoin, Ethereum, NEAR, SOL, etc.</li>
        <li><strong>Commodities:</strong> Gold (XAU/USD), Oil (BRENT/USD)</li>
        <li><strong>Forex:</strong> EUR/USD, GBP/USD, etc.</li>
      </ul>

      <CodeExampleSection
        title="Input Example (Multi-token):"
        code={`{
  "tokens": [
    {
      "token_id": "bitcoin",
      "sources": [
        {"name": "coingecko", "token_id": null},
        {"name": "coinmarketcap", "token_id": "BTC"}
      ],
      "aggregation_method": "median",
      "min_sources_num": 2
    }
  ],
  "max_price_deviation_percent": 5.0
}`}
      />

      <CodeExampleSection
        title="Output Example:"
        code={`{
  "tokens": [
    {
      "token": "bitcoin",
      "data": {
        "price": 110836.0,
        "timestamp": 1729447200,
        "sources": ["coingecko", "coinmarketcap"]
      },
      "message": null
    }
  ]
}`}
      />

      <h4 className="font-semibold mt-4 mb-2">How to Use:</h4>
      <SyntaxHighlighter language="bash" style={vscDarkPlus} customStyle={{ borderRadius: '0.5rem', fontSize: '0.875rem' }}>
{`# 1. Get API keys (optional but recommended):
# - CoinMarketCap: https://coinmarketcap.com/api/
# - TwelveData: https://twelvedata.com/apikey
# - CoinGecko: Free tier (no key needed) or Pro

# 2. Store API keys as encrypted secrets via Dashboard
# Open https://outlayer.fastnear.com/secrets:
# - Secrets JSON: {"CMC_API_KEY":"...", "TWELVE_DATA_KEY":"..."}

# 3. Clone and build
git clone https://github.com/zavodil/oracle-ark.git
cd oracle-ark
cargo build --target wasm32-wasip2 --release

# 4. Request price data
near call outlayer.testnet request_execution '{
  "source": {
    "GitHub": {
      "repo": "https://github.com/YOUR_USERNAME/oracle-ark",
      "commit": "main",
      "build_target": "wasm32-wasip2"
    }
  },
  "secrets_ref": {
    "profile": "production",
    "account_id": "your-account.testnet"
  },
  "resource_limits": {
    "max_instructions": 100000000,
    "max_memory_mb": 128,
    "max_execution_seconds": 60
  },
  "input_data": "{\\"tokens\\":[{\\"token_id\\":\\"bitcoin\\",\\"sources\\":[{\\"name\\":\\"coingecko\\"},{\\"name\\":\\"coinmarketcap\\",\\"token_id\\":\\"BTC\\"}],\\"aggregation_method\\":\\"median\\",\\"min_sources_num\\":2}],\\"max_price_deviation_percent\\":5.0}"
}' --accountId your-account.testnet --deposit 0.1 --gas 300000000000000

# Result will contain median price from multiple sources`}
      </SyntaxHighlighter>

      <TechnicalDetailsSection items={[
        <><strong>WASI Version:</strong> Preview 2 (component model)</>,
        <><strong>Language:</strong> Rust</>,
        <><strong>HTTP Client:</strong> <code>reqwest</code> with WASI sockets</>,
        <><strong>Secrets:</strong> Optional (API keys for higher rate limits)</>,
        <><strong>Network:</strong> Required (outbound HTTPS to multiple APIs)</>,
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
            🎮 <Link href="/playground#multi-source-data-oracle" className="text-[var(--primary-orange)] hover:underline">
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
