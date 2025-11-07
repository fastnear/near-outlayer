'use client';

import Link from 'next/link';
import { Prism as SyntaxHighlighter } from 'react-syntax-highlighter';
import { vscDarkPlus } from 'react-syntax-highlighter/dist/esm/styles/prism';

export default function ExamplesPage() {
  return (
    <div className="prose max-w-none">
      <h2 className="text-3xl font-bold mb-6 text-[var(--primary-orange)]">WASI Examples</h2>

      <p className="text-gray-700 mb-4">
        Production-ready examples demonstrating different WASI patterns and use cases. All examples are open-source and fully documented.
      </p>

      <div className="mb-4 p-4 bg-gray-50 border-l-4 border-gray-400">
        <p className="text-sm text-gray-700">
          🚢 <strong>Why &ldquo;ark&rdquo;?</strong> We use <span className="font-mono font-semibold">*-ark</span> as a naming convention for WASI projects, inspired by <strong>NOAH Ark</strong> (where NOAH stands for <strong>N</strong>ear <strong>O</strong>ffchain <strong>A</strong>gent <strong>H</strong>ub). Each &ldquo;ark&rdquo; is a self-contained, autonomous container that safely executes off-chain computation and returns results back to the blockchain.
        </p>
      </div>

      <div className="mb-6 p-4 bg-blue-50 border-l-4 border-blue-400">
        <p className="text-sm text-gray-700">
          📚 <strong>New to WASI?</strong> Read the <Link href="/docs/wasi" className="text-[var(--primary-orange)] hover:underline font-semibold">Writing WASI Code</Link> guide first to understand WASI basics, supported languages, and critical requirements.
        </p>
      </div>

      <div className="space-y-8">
        {/* random-ark */}
        <div id="random-ark" className="border border-gray-200 rounded-lg p-6 hover:shadow-lg transition-shadow scroll-mt-4">
          <div className="flex items-start justify-between mb-3">
            <h3 className="text-2xl font-semibold">
              random-ark
              <span className="ml-3 text-sm bg-blue-100 text-blue-800 px-3 py-1 rounded">WASI P1</span>
              <span className="ml-2 text-sm bg-green-100 text-green-800 px-3 py-1 rounded">Beginner</span>
            </h3>
          </div>
          <a
            href="https://github.com/zavodil/random-ark"
            target="_blank"
            rel="noopener noreferrer"
            className="inline-flex items-center gap-2 px-4 py-2 mb-4 bg-gray-900 text-white rounded-lg hover:bg-gray-800 transition-colors font-medium"
          >
            <svg className="w-5 h-5" fill="currentColor" viewBox="0 0 24 24">
              <path fillRule="evenodd" d="M12 2C6.477 2 2 6.484 2 12.017c0 4.425 2.865 8.18 6.839 9.504.5.092.682-.217.682-.483 0-.237-.008-.868-.013-1.703-2.782.605-3.369-1.343-3.369-1.343-.454-1.158-1.11-1.466-1.11-1.466-.908-.62.069-.608.069-.608 1.003.07 1.531 1.032 1.531 1.032.892 1.53 2.341 1.088 2.91.832.092-.647.35-1.088.636-1.338-2.22-.253-4.555-1.113-4.555-4.951 0-1.093.39-1.988 1.029-2.688-.103-.253-.446-1.272.098-2.65 0 0 .84-.27 2.75 1.026A9.564 9.564 0 0112 6.844c.85.004 1.705.115 2.504.337 1.909-1.296 2.747-1.027 2.747-1.027.546 1.379.202 2.398.1 2.651.64.7 1.028 1.595 1.028 2.688 0 3.848-2.339 4.695-4.566 4.943.359.309.678.92.678 1.855 0 1.338-.012 2.419-.012 2.747 0 .268.18.58.688.482A10.019 10.019 0 0022 12.017C22 6.484 17.522 2 12 2z" clipRule="evenodd"/>
            </svg>
            <span>Source Code on GitHub</span>
          </a>
          <p className="text-gray-700 mb-4">
            Generate cryptographically secure random numbers using WASI random source. Perfect starter example for learning WASI basics.
          </p>

          <div className="mb-4 p-3 bg-blue-50 border-l-4 border-blue-400">
            <p className="text-sm text-gray-700">
              📖 <strong>Complete Tutorial:</strong> See the <Link href="/docs/dev-guide" className="text-[var(--primary-orange)] hover:underline font-semibold">Developer Guide</Link> for a detailed step-by-step walkthrough of building this example from scratch.
            </p>
          </div>

          <h4 className="font-semibold mt-4 mb-2">Key Features:</h4>
          <ul className="list-disc list-inside text-gray-700 space-y-1 mb-4">
            <li>Random number generation in specified range</li>
            <li>JSON input/output handling</li>
            <li>Small binary size (~111KB)</li>
            <li>Unit tests included</li>
            <li>Simple error handling</li>
          </ul>

          <h4 className="font-semibold mt-4 mb-2">Input Example:</h4>
          <SyntaxHighlighter language="json" style={vscDarkPlus} customStyle={{ borderRadius: '0.5rem', fontSize: '0.875rem' }}>
{`{
  "min": 1,
  "max": 100
}`}
          </SyntaxHighlighter>

          <h4 className="font-semibold mt-4 mb-2">Output Example:</h4>
          <SyntaxHighlighter language="json" style={vscDarkPlus} customStyle={{ borderRadius: '0.5rem', fontSize: '0.875rem' }}>
{`{
  "random_number": 42
}`}
          </SyntaxHighlighter>

          <h4 className="font-semibold mt-4 mb-2">How to Use:</h4>
          <SyntaxHighlighter language="bash" style={vscDarkPlus} customStyle={{ borderRadius: '0.5rem', fontSize: '0.875rem' }}>
{`# 1. Clone and build
git clone https://github.com/zavodil/random-ark.git
cd random-ark
cargo build --target wasm32-wasip1 --release

# 2. Test locally
echo '{"min":1,"max":100}' | wasmtime target/wasm32-wasip1/release/random-ark.wasm

# 3. Push to GitHub (or use existing repo)
git push origin main

# 4. Request execution on NEAR
near call outlayer.testnet request_execution '{
  "code_source": {
    "repo": "https://github.com/YOUR_USERNAME/random-ark",
    "commit": "main",
    "build_target": "wasm32-wasip1"
  },
  "resource_limits": {
    "max_instructions": 10000000,
    "max_memory_mb": 128,
    "max_execution_seconds": 60
  },
  "input_data": "{\\"min\\":1,\\"max\\":100}"
}' --accountId your-account.testnet --deposit 0.1 --gas 300000000000000

# 5. Check result in NEAR Explorer or via:
near tx-status TRANSACTION_HASH --accountId your-account.testnet`}
          </SyntaxHighlighter>
        </div>

        {/* echo-ark */}
        <div id="echo-ark" className="border border-gray-200 rounded-lg p-6 hover:shadow-lg transition-shadow scroll-mt-4">
          <div className="flex items-start justify-between mb-3">
            <h3 className="text-2xl font-semibold">
              echo-ark
              <span className="ml-3 text-sm bg-blue-100 text-blue-800 px-3 py-1 rounded">WASI P1</span>
              <span className="ml-2 text-sm bg-green-100 text-green-800 px-3 py-1 rounded">Beginner</span>
            </h3>
          </div>
          <a
            href="https://github.com/zavodil/echo-ark"
            target="_blank"
            rel="noopener noreferrer"
            className="inline-flex items-center gap-2 px-4 py-2 mb-4 bg-gray-900 text-white rounded-lg hover:bg-gray-800 transition-colors font-medium"
          >
            <svg className="w-5 h-5" fill="currentColor" viewBox="0 0 24 24">
              <path fillRule="evenodd" d="M12 2C6.477 2 2 6.484 2 12.017c0 4.425 2.865 8.18 6.839 9.504.5.092.682-.217.682-.483 0-.237-.008-.868-.013-1.703-2.782.605-3.369-1.343-3.369-1.343-.454-1.158-1.11-1.466-1.11-1.466-.908-.62.069-.608.069-.608 1.003.07 1.531 1.032 1.531 1.032.892 1.53 2.341 1.088 2.91.832.092-.647.35-1.088.636-1.338-2.22-.253-4.555-1.113-4.555-4.951 0-1.093.39-1.988 1.029-2.688-.103-.253-.446-1.272.098-2.65 0 0 .84-.27 2.75 1.026A9.564 9.564 0 0112 6.844c.85.004 1.705.115 2.504.337 1.909-1.296 2.747-1.027 2.747-1.027.546 1.379.202 2.398.1 2.651.64.7 1.028 1.595 1.028 2.688 0 3.848-2.339 4.695-4.566 4.943.359.309.678.92.678 1.855 0 1.338-.012 2.419-.012 2.747 0 .268.18.58.688.482A10.019 10.019 0 0022 12.017C22 6.484 17.522 2 12 2z" clipRule="evenodd"/>
            </svg>
            <span>Source Code on GitHub</span>
          </a>
          <p className="text-gray-700 mb-4">
            Echo messages with NEAR blockchain context information. Demonstrates how to access execution metadata via environment variables.
          </p>

          <h4 className="font-semibold mt-4 mb-2">Key Features:</h4>
          <ul className="list-disc list-inside text-gray-700 space-y-1 mb-4">
            <li>Access NEAR environment variables (sender, block height, timestamp)</li>
            <li>Simple input/output transformation</li>
            <li>Context injection demonstration</li>
            <li>Shows available NEAR metadata</li>
          </ul>

          <h4 className="font-semibold mt-4 mb-2">Available Environment Variables:</h4>
          <ul className="list-disc list-inside text-gray-700 space-y-1 mb-4 ml-4 text-sm">
            <li><code className="bg-gray-100 px-2 py-1 rounded">NEAR_SENDER_ID</code> - Account that requested execution</li>
            <li><code className="bg-gray-100 px-2 py-1 rounded">NEAR_BLOCK_HEIGHT</code> - Block height when request was made</li>
            <li><code className="bg-gray-100 px-2 py-1 rounded">NEAR_BLOCK_TIMESTAMP</code> - Block timestamp (nanoseconds)</li>
            <li><code className="bg-gray-100 px-2 py-1 rounded">NEAR_CONTRACT_ID</code> - OutLayer contract address</li>
            <li><code className="bg-gray-100 px-2 py-1 rounded">NEAR_REQUEST_ID</code> - Unique request ID</li>
          </ul>

          <h4 className="font-semibold mt-4 mb-2">Output Example:</h4>
          <SyntaxHighlighter language="json" style={vscDarkPlus} customStyle={{ borderRadius: '0.5rem', fontSize: '0.875rem' }}>
{`{
  "echo": "alice.near said \\"Hello!\\" at block 123456789"
}`}
          </SyntaxHighlighter>

          <h4 className="font-semibold mt-4 mb-2">How to Use:</h4>
          <SyntaxHighlighter language="bash" style={vscDarkPlus} customStyle={{ borderRadius: '0.5rem', fontSize: '0.875rem' }}>
{`# 1. Clone and build
git clone https://github.com/zavodil/echo-ark.git
cd echo-ark
cargo build --target wasm32-wasip1 --release

# 2. Request execution (OutLayer will inject NEAR env vars)
near call outlayer.testnet request_execution '{
  "code_source": {
    "repo": "https://github.com/YOUR_USERNAME/echo-ark",
    "commit": "main",
    "build_target": "wasm32-wasip1"
  },
  "resource_limits": {
    "max_instructions": 10000000,
    "max_memory_mb": 128,
    "max_execution_seconds": 60
  },
  "input_data": "{\\"message\\":\\"Hello!\\"}"
}' --accountId alice.testnet --deposit 0.1 --gas 300000000000000

# Worker automatically provides:
# - NEAR_SENDER_ID=alice.testnet
# - NEAR_BLOCK_HEIGHT=123456789
# - NEAR_BLOCK_TIMESTAMP=...
# - NEAR_CONTRACT_ID=outlayer.testnet
# - NEAR_REQUEST_ID=unique-id`}
          </SyntaxHighlighter>
        </div>

        {/* ai-ark */}
        <div id="ai-ark" className="border border-gray-200 rounded-lg p-6 hover:shadow-lg transition-shadow scroll-mt-4">
          <div className="flex items-start justify-between mb-3">
            <h3 className="text-2xl font-semibold">
              ai-ark
              <span className="ml-3 text-sm bg-purple-100 text-purple-800 px-3 py-1 rounded">WASI P2</span>
              <span className="ml-2 text-sm bg-yellow-100 text-yellow-800 px-3 py-1 rounded">Intermediate</span>
            </h3>
          </div>
          <a
            href="https://github.com/zavodil/ai-ark"
            target="_blank"
            rel="noopener noreferrer"
            className="inline-flex items-center gap-2 px-4 py-2 mb-4 bg-gray-900 text-white rounded-lg hover:bg-gray-800 transition-colors font-medium"
          >
            <svg className="w-5 h-5" fill="currentColor" viewBox="0 0 24 24">
              <path fillRule="evenodd" d="M12 2C6.477 2 2 6.484 2 12.017c0 4.425 2.865 8.18 6.839 9.504.5.092.682-.217.682-.483 0-.237-.008-.868-.013-1.703-2.782.605-3.369-1.343-3.369-1.343-.454-1.158-1.11-1.466-1.11-1.466-.908-.62.069-.608.069-.608 1.003.07 1.531 1.032 1.531 1.032.892 1.53 2.341 1.088 2.91.832.092-.647.35-1.088.636-1.338-2.22-.253-4.555-1.113-4.555-4.951 0-1.093.39-1.988 1.029-2.688-.103-.253-.446-1.272.098-2.65 0 0 .84-.27 2.75 1.026A9.564 9.564 0 0112 6.844c.85.004 1.705.115 2.504.337 1.909-1.296 2.747-1.027 2.747-1.027.546 1.379.202 2.398.1 2.651.64.7 1.028 1.595 1.028 2.688 0 3.848-2.339 4.695-4.566 4.943.359.309.678.92.678 1.855 0 1.338-.012 2.419-.012 2.747 0 .268.18.58.688.482A10.019 10.019 0 0022 12.017C22 6.484 17.522 2 12 2z" clipRule="evenodd"/>
            </svg>
            <span>Source Code on GitHub</span>
          </a>
          <p className="text-gray-700 mb-4">
            OpenAI API integration with HTTPS POST requests. Your first step into WASI Preview 2 capabilities and external API calls.
          </p>

          <h4 className="font-semibold mt-4 mb-2">Key Features:</h4>
          <ul className="list-disc list-inside text-gray-700 space-y-1 mb-4">
            <li>HTTPS client using <code className="bg-gray-100 px-2 py-1 rounded">wasi-http-client</code> crate</li>
            <li>External API integration pattern</li>
            <li>Component model example (WASI P2)</li>
            <li>Fuel metering demonstration</li>
            <li>JSON request/response handling</li>
          </ul>

          <h4 className="font-semibold mt-4 mb-2">Requirements:</h4>
          <ul className="list-disc list-inside text-gray-700 space-y-1 mb-4 ml-4">
            <li>OpenAI API key (stored as encrypted secret)</li>
            <li>wasmtime 28+ for local testing</li>
            <li>Network access during execution</li>
          </ul>

          <h4 className="font-semibold mt-4 mb-2">How to Use:</h4>
          <SyntaxHighlighter language="bash" style={vscDarkPlus} customStyle={{ borderRadius: '0.5rem', fontSize: '0.875rem' }}>
{`# 1. Get OpenAI API key from https://platform.openai.com/api-keys

# 2. Store API key as encrypted secret via Dashboard
# Open https://outlayer.fastnear.com/secrets and create:
# - Repo: github.com/YOUR_USERNAME/ai-ark
# - Branch: main
# - Profile: production
# - Secrets JSON: {"OPENAI_API_KEY":"sk-..."}

# 3. Clone and build
git clone https://github.com/zavodil/ai-ark.git
cd ai-ark
cargo build --target wasm32-wasip2 --release

# 4. Request execution with secrets
near call outlayer.testnet request_execution '{
  "code_source": {
    "repo": "https://github.com/YOUR_USERNAME/ai-ark",
    "commit": "main",
    "build_target": "wasm32-wasip2"
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
  "input_data": "{\\"prompt\\":\\"What is NEAR Protocol?\\"}"
}' --accountId your-account.testnet --deposit 0.1 --gas 300000000000000

# WASI worker will:
# - Decrypt secrets and inject OPENAI_API_KEY into environment
# - Execute WASM with your prompt
# - Return AI response`}
          </SyntaxHighlighter>
        </div>

        {/* weather-ark */}
        <div id="weather-ark" className="border border-gray-200 rounded-lg p-6 hover:shadow-lg transition-shadow scroll-mt-4">
          <div className="flex items-start justify-between mb-3">
            <h3 className="text-2xl font-semibold">
              weather-ark
              <span className="ml-3 text-sm bg-purple-100 text-purple-800 px-3 py-1 rounded">WASI P2</span>
              <span className="ml-2 text-sm bg-yellow-100 text-yellow-800 px-3 py-1 rounded">Intermediate</span>
            </h3>
          </div>
          <a
            href="https://github.com/zavodil/weather-ark"
            target="_blank"
            rel="noopener noreferrer"
            className="inline-flex items-center gap-2 px-4 py-2 mb-4 bg-gray-900 text-white rounded-lg hover:bg-gray-800 transition-colors font-medium"
          >
            <svg className="w-5 h-5" fill="currentColor" viewBox="0 0 24 24">
              <path fillRule="evenodd" d="M12 2C6.477 2 2 6.484 2 12.017c0 4.425 2.865 8.18 6.839 9.504.5.092.682-.217.682-.483 0-.237-.008-.868-.013-1.703-2.782.605-3.369-1.343-3.369-1.343-.454-1.158-1.11-1.466-1.11-1.466-.908-.62.069-.608.069-.608 1.003.07 1.531 1.032 1.531 1.032.892 1.53 2.341 1.088 2.91.832.092-.647.35-1.088.636-1.338-2.22-.253-4.555-1.113-4.555-4.951 0-1.093.39-1.988 1.029-2.688-.103-.253-.446-1.272.098-2.65 0 0 .84-.27 2.75 1.026A9.564 9.564 0 0112 6.844c.85.004 1.705.115 2.504.337 1.909-1.296 2.747-1.027 2.747-1.027.546 1.379.202 2.398.1 2.651.64.7 1.028 1.595 1.028 2.688 0 3.848-2.339 4.695-4.566 4.943.359.309.678.92.678 1.855 0 1.338-.012 2.419-.012 2.747 0 .268.18.58.688.482A10.019 10.019 0 0022 12.017C22 6.484 17.522 2 12 2z" clipRule="evenodd"/>
            </svg>
            <span>Source Code on GitHub</span>
          </a>
          <p className="text-gray-700 mb-4">
            Real-time weather data oracle via OpenWeather API. Specialized oracle example showing how to fetch and format data from a specific API. Ready to test on testnet with pre-configured secrets!
          </p>

          <h4 className="font-semibold mt-4 mb-2">Key Features:</h4>
          <ul className="list-disc list-inside text-gray-700 space-y-1 mb-4">
            <li>Real-time weather data for any city worldwide</li>
            <li>Support for metric (Celsius) and imperial (Fahrenheit) units</li>
            <li>Pre-configured secrets on testnet for instant testing</li>
            <li>HTTPS client via <code className="bg-gray-100 px-2 py-1 rounded">wasi-http-client</code></li>
            <li>OpenWeather API integration (free tier: 60 calls/min)</li>
            <li>Clean JSON input/output format</li>
          </ul>

          <h4 className="font-semibold mt-4 mb-2">Input Example:</h4>
          <SyntaxHighlighter language="json" style={vscDarkPlus} customStyle={{ borderRadius: '0.5rem', fontSize: '0.875rem' }}>
{`{
  "city": "London",
  "units": "metric"
}`}
          </SyntaxHighlighter>

          <h4 className="font-semibold mt-4 mb-2">Output Example:</h4>
          <SyntaxHighlighter language="json" style={vscDarkPlus} customStyle={{ borderRadius: '0.5rem', fontSize: '0.875rem' }}>
{`{
  "city": "London",
  "country": "GB",
  "temperature": 15.5,
  "temperature_unit": "C",
  "description": "overcast clouds",
  "humidity": 72,
  "wind_speed": 3.6
}`}
          </SyntaxHighlighter>

          <h4 className="font-semibold mt-4 mb-2">Quick Start (Testnet - No Setup Needed!):</h4>
          <SyntaxHighlighter language="bash" style={vscDarkPlus} customStyle={{ borderRadius: '0.5rem', fontSize: '0.875rem' }}>
{`# Test immediately with pre-configured secrets
near contract call-function as-transaction outlayer.testnet request_execution \\
  json-args '{
    "code_source": {
      "repo": "https://github.com/zavodil/weather-ark",
      "commit": "main",
      "build_target": "wasm32-wasip2"
    },
    "secrets_ref": {
      "repo": "github.com/zavodil/weather-ark",
      "profile": "default",
      "account_id": "zavodil2.testnet"
    },
    "resource_limits": {
      "max_instructions": 50000000000,
      "max_memory_mb": 128,
      "max_execution_seconds": 30
    },
    "response_format": "Json",
    "input_data": "{\\"city\\":\\"London\\",\\"units\\":\\"metric\\"}"
  }' \\
  prepaid-gas '100.0 Tgas' \\
  attached-deposit '0.1 NEAR' \\
  sign-as your-account.testnet \\
  network-config testnet \\
  sign-with-keychain \\
  send

# Try different cities:
# Tokyo: --input_data '{"city":"Tokyo","units":"metric"}'
# New York (Fahrenheit): --input_data '{"city":"New York","units":"imperial"}'
# Paris: --input_data '{"city":"Paris"}'`}
          </SyntaxHighlighter>

          <div className="mt-4 p-3 bg-blue-50 border-l-4 border-blue-400">
            <p className="text-sm text-gray-700 mb-2">
              <strong>Ready to Test!</strong> Pre-configured secrets are available on testnet:
            </p>
            <ul className="text-sm text-gray-700 space-y-1 ml-4">
              <li>Repo: <code className="bg-white px-2 py-1 rounded">github.com/zavodil/weather-ark</code></li>
              <li>Profile: <code className="bg-white px-2 py-1 rounded">default</code></li>
              <li>Owner: <code className="bg-white px-2 py-1 rounded">zavodil2.testnet</code></li>
            </ul>
          </div>

          <h4 className="font-semibold mt-4 mb-2">Production Deployment (Your Own Secrets):</h4>
          <SyntaxHighlighter language="bash" style={vscDarkPlus} customStyle={{ borderRadius: '0.5rem', fontSize: '0.875rem' }}>
{`# 1. Get OpenWeather API key from https://openweathermap.org/api
#    Free tier: 60 calls/min, 1M calls/month, no credit card

# 2. Store API key as encrypted secret
# Open https://outlayer.fastnear.com/secrets:
# - Repo: github.com/YOUR_USERNAME/your-repo
# - Profile: weather-production
# - Secrets JSON: {"OPENWEATHER_API_KEY":"your_key_here"}

# 3. Clone and build
git clone https://github.com/zavodil/weather-ark.git
cd weather-ark
cargo build --target wasm32-wasip2 --release

# 4. Request with your own secrets
near call outlayer.testnet request_execution '{
  "code_source": {
    "repo": "https://github.com/YOUR_USERNAME/your-repo",
    "commit": "main",
    "build_target": "wasm32-wasip2",
    "build_path": "wasi-examples/weather-ark"
  },
  "secrets_ref": {
    "repo": "github.com/YOUR_USERNAME/your-repo",
    "profile": "weather-production",
    "account_id": "your.testnet"
  },
  "input_data": "{\\"city\\":\\"Paris\\"}"
}' --accountId your.testnet --deposit 0.1`}
          </SyntaxHighlighter>

          <h4 className="font-semibold mt-4 mb-2">Use Cases:</h4>
          <ul className="list-disc list-inside text-gray-700 space-y-1 mb-4 ml-4">
            <li><strong>Weather Bot:</strong> On-chain smart contract providing weather data to users</li>
            <li><strong>Conditional Payments:</strong> Release funds based on weather conditions</li>
            <li><strong>Agricultural Contracts:</strong> Trigger actions based on local weather</li>
            <li><strong>Travel Planning:</strong> Check weather before booking</li>
          </ul>
        </div>

        {/* oracle-ark */}
        <div id="oracle-ark" className="border border-gray-200 rounded-lg p-6 hover:shadow-lg transition-shadow scroll-mt-4">
          <div className="flex items-start justify-between mb-3">
            <h3 className="text-2xl font-semibold">
              oracle-ark
              <span className="ml-3 text-sm bg-purple-100 text-purple-800 px-3 py-1 rounded">WASI P2</span>
              <span className="ml-2 text-sm bg-orange-100 text-orange-800 px-3 py-1 rounded">Advanced</span>
            </h3>
          </div>
          <a
            href="https://github.com/zavodil/oracle-ark"
            target="_blank"
            rel="noopener noreferrer"
            className="inline-flex items-center gap-2 px-4 py-2 mb-4 bg-gray-900 text-white rounded-lg hover:bg-gray-800 transition-colors font-medium"
          >
            <svg className="w-5 h-5" fill="currentColor" viewBox="0 0 24 24">
              <path fillRule="evenodd" d="M12 2C6.477 2 2 6.484 2 12.017c0 4.425 2.865 8.18 6.839 9.504.5.092.682-.217.682-.483 0-.237-.008-.868-.013-1.703-2.782.605-3.369-1.343-3.369-1.343-.454-1.158-1.11-1.466-1.11-1.466-.908-.62.069-.608.069-.608 1.003.07 1.531 1.032 1.531 1.032.892 1.53 2.341 1.088 2.91.832.092-.647.35-1.088.636-1.338-2.22-.253-4.555-1.113-4.555-4.951 0-1.093.39-1.988 1.029-2.688-.103-.253-.446-1.272.098-2.65 0 0 .84-.27 2.75 1.026A9.564 9.564 0 0112 6.844c.85.004 1.705.115 2.504.337 1.909-1.296 2.747-1.027 2.747-1.027.546 1.379.202 2.398.1 2.651.64.7 1.028 1.595 1.028 2.688 0 3.848-2.339 4.695-4.566 4.943.359.309.678.92.678 1.855 0 1.338-.012 2.419-.012 2.747 0 .268.18.58.688.482A10.019 10.019 0 0022 12.017C22 6.484 17.522 2 12 2z" clipRule="evenodd"/>
            </svg>
            <span>Source Code on GitHub</span>
          </a>
          <p className="text-gray-700 mb-4">
            Multi-source price oracle with aggregation and validation. Production-ready decentralized oracle for cryptocurrency and commodity prices.
          </p>

          <h4 className="font-semibold mt-4 mb-2">Key Features:</h4>
          <ul className="list-disc list-inside text-gray-700 space-y-1 mb-4">
            <li>Multiple API sources: CoinGecko, CoinMarketCap, TwelveData</li>
            <li>Price aggregation methods: average, median, weighted</li>
            <li>Deviation validation and error handling</li>
            <li>Batch requests for multiple tokens (up to 10)</li>
            <li>API key management via encrypted secrets</li>
            <li>Comprehensive error reporting per source</li>
          </ul>

          <h4 className="font-semibold mt-4 mb-2">Supported Assets:</h4>
          <ul className="list-disc list-inside text-gray-700 space-y-1 mb-4 ml-4">
            <li><strong>Cryptocurrencies:</strong> Bitcoin, Ethereum, NEAR, SOL, etc.</li>
            <li><strong>Commodities:</strong> Gold (XAU/USD), Oil (BRENT/USD)</li>
            <li><strong>Forex:</strong> EUR/USD, GBP/USD, etc.</li>
          </ul>

          <h4 className="font-semibold mt-4 mb-2">Input Example (Multi-token):</h4>
          <SyntaxHighlighter language="json" style={vscDarkPlus} customStyle={{ borderRadius: '0.5rem', fontSize: '0.875rem' }}>
{`{
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
          </SyntaxHighlighter>

          <h4 className="font-semibold mt-4 mb-2">Output Example:</h4>
          <SyntaxHighlighter language="json" style={vscDarkPlus} customStyle={{ borderRadius: '0.5rem', fontSize: '0.875rem' }}>
{`{
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
          </SyntaxHighlighter>

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
  "code_source": {
    "repo": "https://github.com/YOUR_USERNAME/oracle-ark",
    "commit": "main",
    "build_target": "wasm32-wasip2"
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
        </div>

        {/* intents-ark */}
        <div id="intents-ark" className="border border-gray-200 rounded-lg p-6 hover:shadow-lg transition-shadow scroll-mt-4">
          <div className="flex items-start justify-between mb-3">
            <h3 className="text-2xl font-semibold">
              intents-ark
              <span className="ml-3 text-sm bg-purple-100 text-purple-800 px-3 py-1 rounded">WASI P2</span>
              <span className="ml-2 text-sm bg-orange-100 text-orange-800 px-3 py-1 rounded">Advanced</span>
            </h3>
          </div>
          <a
            href="https://github.com/zavodil/intents-ark"
            target="_blank"
            rel="noopener noreferrer"
            className="inline-flex items-center gap-2 px-4 py-2 mb-4 bg-gray-900 text-white rounded-lg hover:bg-gray-800 transition-colors font-medium"
          >
            <svg className="w-5 h-5" fill="currentColor" viewBox="0 0 24 24">
              <path fillRule="evenodd" d="M12 2C6.477 2 2 6.484 2 12.017c0 4.425 2.865 8.18 6.839 9.504.5.092.682-.217.682-.483 0-.237-.008-.868-.013-1.703-2.782.605-3.369-1.343-3.369-1.343-.454-1.158-1.11-1.466-1.11-1.466-.908-.62.069-.608.069-.608 1.003.07 1.531 1.032 1.531 1.032.892 1.53 2.341 1.088 2.91.832.092-.647.35-1.088.636-1.338-2.22-.253-4.555-1.113-4.555-4.951 0-1.093.39-1.988 1.029-2.688-.103-.253-.446-1.272.098-2.65 0 0 .84-.27 2.75 1.026A9.564 9.564 0 0112 6.844c.85.004 1.705.115 2.504.337 1.909-1.296 2.747-1.027 2.747-1.027.546 1.379.202 2.398.1 2.651.64.7 1.028 1.595 1.028 2.688 0 3.848-2.339 4.695-4.566 4.943.359.309.678.92.678 1.855 0 1.338-.012 2.419-.012 2.747 0 .268.18.58.688.482A10.019 10.019 0 0022 12.017C22 6.484 17.522 2 12 2z" clipRule="evenodd"/>
            </svg>
            <span>Source Code on GitHub</span>
          </a>
          <p className="text-gray-700 mb-4">
            DEX token swaps via NEAR Intents protocol. User&apos;s FT transfer transaction pauses, WASI performs swap off-chain, then resolves by sending swapped tokens back - all within single transaction.
          </p>

          <h4 className="font-semibold mt-4 mb-2">Key Features:</h4>
          <ul className="list-disc list-inside text-gray-700 space-y-1 mb-4">
            <li>FT transfer pauses during off-chain swap execution</li>
            <li>NEAR Intents API integration (quote → publish → settle)</li>
            <li>NEP-413 message signing with ed25519</li>
            <li>Automatic token withdrawal to user upon completion</li>
            <li>Private key management via encrypted secrets</li>
            <li>Storage deposit handling for fungible tokens</li>
          </ul>

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
        </div>

        {/* captcha-ark */}
        <div id="captcha-ark" className="border border-gray-200 rounded-lg p-6 hover:shadow-lg transition-shadow scroll-mt-4">
          <div className="flex items-start justify-between mb-3">
            <h3 className="text-2xl font-semibold">
              captcha-ark
              <span className="ml-3 text-sm bg-purple-100 text-purple-800 px-3 py-1 rounded">WASI P2</span>
              <span className="ml-2 text-sm bg-green-100 text-green-800 px-3 py-1 rounded">Full Stack</span>
            </h3>
          </div>
          <a
            href="https://github.com/zavodil/captcha-ark"
            target="_blank"
            rel="noopener noreferrer"
            className="inline-flex items-center gap-2 px-4 py-2 mb-4 bg-gray-900 text-white rounded-lg hover:bg-gray-800 transition-colors font-medium"
          >
            <svg className="w-5 h-5" fill="currentColor" viewBox="0 0 24 24">
              <path fillRule="evenodd" d="M12 2C6.477 2 2 6.484 2 12.017c0 4.425 2.865 8.18 6.839 9.504.5.092.682-.217.682-.483 0-.237-.008-.868-.013-1.703-2.782.605-3.369-1.343-3.369-1.343-.454-1.158-1.11-1.466-1.11-1.466-.908-.62.069-.608.069-.608 1.003.07 1.531 1.032 1.531 1.032.892 1.53 2.341 1.088 2.91.832.092-.647.35-1.088.636-1.338-2.22-.253-4.555-1.113-4.555-4.951 0-1.093.39-1.988 1.029-2.688-.103-.253-.446-1.272.098-2.65 0 0 .84-.27 2.75 1.026A9.564 9.564 0 0112 6.844c.85.004 1.705.115 2.504.337 1.909-1.296 2.747-1.027 2.747-1.027.546 1.379.202 2.398.1 2.651.64.7 1.028 1.595 1.028 2.688 0 3.848-2.339 4.695-4.566 4.943.359.309.678.92.678 1.855 0 1.338-.012 2.419-.012 2.747 0 .268.18.58.688.482A10.019 10.019 0 0022 12.017C22 6.484 17.522 2 12 2z" clipRule="evenodd"/>
            </svg>
            <span>Source Code on GitHub</span>
          </a>
          <p className="text-gray-700 mb-4">
            Token sale with mandatory CAPTCHA verification. Transaction won&apos;t complete until user solves CAPTCHA. WASI worker receives session ID from backend and waits for verification signal. Example implementation demonstrating async human verification pattern.
          </p>

          <h4 className="font-semibold mt-4 mb-2">Key Features:</h4>
          <ul className="list-disc list-inside text-gray-700 space-y-1 mb-4">
            <li>Transaction blocking until CAPTCHA solved</li>
            <li>WASI worker requests CAPTCHA challenge from backend</li>
            <li>Backend sends CAPTCHA to user via WebSocket</li>
            <li>Worker waits for verification signal (long-polling)</li>
            <li>hCaptcha integration for human verification</li>
            <li>React frontend with NEAR Wallet Selector</li>
            <li>Node.js backend with Express + WebSocket server</li>
          </ul>

          <h4 className="font-semibold mt-4 mb-2">Architecture:</h4>
          <SyntaxHighlighter language="text" style={vscDarkPlus} customStyle={{ borderRadius: '0.5rem', fontSize: '0.75rem' }}>
{`User Browser → Token Sale Contract → OutLayer → WASI Worker
      ↑                                              ↓
      └────── Launchpad Backend (WebSocket) ←────────┘

Flow:
1. User clicks "Buy Tokens" with session_id
2. Contract calls OutLayer, transaction pauses
3. WASI worker → backend: "I received payment from session_id X.
   Verify this is a real user, not a bot. Send CAPTCHA and notify
   me when user completes it."
4. Backend sends CAPTCHA to user's browser via WebSocket
5. User solves CAPTCHA in modal
6. Backend verifies solution and marks session as verified
7. Worker receives "verified" signal, returns success to contract
8. Transaction resumes - user receives tokens`}
          </SyntaxHighlighter>

          <h4 className="font-semibold mt-4 mb-2">Components:</h4>
          <ul className="list-disc list-inside text-gray-700 space-y-1 mb-4 ml-4">
            <li><strong>WASI Worker:</strong> Rust WASM that verifies CAPTCHA (wasm32-wasip2)</li>
            <li><strong>Smart Contract:</strong> Token sale logic with OutLayer integration</li>
            <li><strong>Backend:</strong> Node.js Express server with WebSocket support</li>
            <li><strong>Frontend:</strong> React app with hCaptcha widget</li>
          </ul>

          <h4 className="font-semibold mt-4 mb-2">How to Use:</h4>
          <SyntaxHighlighter language="bash" style={vscDarkPlus} customStyle={{ borderRadius: '0.5rem', fontSize: '0.875rem' }}>
{`# 1. Get hCaptcha account (free at hcaptcha.com)
# - Create site and get Site Key + Secret Key

# 2. Clone repository
git clone https://github.com/zavodil/captcha-ark.git
cd captcha-ark

# 3. Setup backend
cd launchpad-backend
npm install
cat > .env <<EOF
PORT=3181
HCAPTCHA_SITE_KEY=your_site_key
HCAPTCHA_SECRET=your_secret_key
WORKER_API_KEY=$(openssl rand -hex 32)
EOF
npm start

# 4. Setup frontend
cd ../launchpad-app
npm install
cat > .env <<EOF
REACT_APP_CONTRACT_ID=tokensale.testnet
REACT_APP_NEAR_NETWORK=testnet
REACT_APP_HCAPTCHA_SITE_KEY=your_site_key
EOF
npm run build
# Deploy build/ to your web server

# 5. Build WASI worker
cd ../captcha-ark
cargo build --target wasm32-wasip2 --release
git push origin main

# 6. Deploy token sale contract
cd ../token-sale-contract
cargo near build
near deploy tokensale.testnet \\
  use-file res/token_sale_contract.wasm \\
  with-init-call new \\
  json-args '{"owner":"owner.testnet","total_supply":"10000","launchpad_url":"https://api.yourdomain.com"}' \\
  prepaid-gas '100.0 Tgas' \\
  attached-deposit '0 NEAR'

# 7. Users can now buy tokens - CAPTCHA required!
# Visit https://launchpad.yourdomain.com and click "Buy Tokens"`}
          </SyntaxHighlighter>

          <div className="mt-4 p-3 bg-blue-50 border-l-4 border-blue-400">
            <p className="text-sm text-gray-700 mb-2">
              <strong>Production Setup:</strong> Requires SSL certificates for both frontend and backend domains. See <a href="https://github.com/zavodil/captcha-ark/blob/main/CONFIGURATION.md" target="_blank" rel="noopener noreferrer" className="text-[var(--primary-orange)] hover:underline">CONFIGURATION.md</a> for complete deployment guide.
            </p>
          </div>

          <div className="mt-4 p-3 bg-yellow-50 border-l-4 border-yellow-400">
            <p className="text-sm text-gray-700">
              <strong>⚠️ Security Note:</strong> Backend must implement worker authentication to prevent spam attacks. Unauthorized requests to create CAPTCHA challenges should be rejected. See README for implementation details.
            </p>
          </div>
        </div>
      </div>

      <div className="mt-8 p-6 bg-gray-50 border border-gray-200 rounded-lg">
        <h3 className="text-xl font-semibold mb-4">Ready to Build?</h3>
        <ul className="list-disc list-inside space-y-2 text-gray-700">
          <li>Read the <a href="https://github.com/fastnear/near-outlayer/blob/main/wasi-examples/WASI_TUTORIAL.md" target="_blank" rel="noopener noreferrer" className="text-[var(--primary-orange)] hover:underline">complete WASI tutorial</a></li>
          <li>Clone examples: <code className="bg-gray-100 px-2 py-1 rounded">git clone https://github.com/zavodil/near-offshore.git</code></li>
          <li>Start with <strong>random-ark</strong> or <strong>echo-ark</strong> for simple use cases</li>
          <li>Try <strong>weather-ark</strong> for instant testing (pre-configured secrets on testnet!)</li>
          <li>Use <strong>ai-ark</strong> or <strong>oracle-ark</strong> for HTTPS-based applications</li>
          <li>Study <strong>captcha-ark</strong> for full-stack deployment</li>
          <li>Explore <strong>intents-ark</strong> for advanced DeFi integration</li>
        </ul>
      </div>
    </div>
  );
}
