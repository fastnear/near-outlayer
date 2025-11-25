'use client';

import Link from 'next/link';
import { useEffect } from 'react';
import { Prism as SyntaxHighlighter } from 'react-syntax-highlighter';
import { vscDarkPlus } from 'react-syntax-highlighter/dist/esm/styles/prism';

// Anchor heading component with clickable link
function AnchorHeading({ id, children, badges }: { id: string; children: React.ReactNode; badges?: React.ReactNode }) {
  const handleClick = (e: React.MouseEvent) => {
    e.preventDefault();
    const element = document.getElementById(id);
    if (element) {
      element.scrollIntoView({ behavior: 'smooth', block: 'start' });
      window.history.pushState(null, '', `#${id}`);
    }
  };

  return (
    <h3 className="text-2xl font-semibold group relative">
      <a href={`#${id}`} onClick={handleClick} className="hover:text-[var(--primary-orange)] transition-colors">
        {children}
        {badges}
        <span className="absolute -left-6 opacity-0 group-hover:opacity-100 transition-opacity text-gray-400">#</span>
      </a>
    </h3>
  );
}

// Hook to handle hash navigation on page load
function useHashNavigation() {
  useEffect(() => {
    if (typeof window === 'undefined') return;

    const hash = window.location.hash.slice(1);
    if (hash) {
      setTimeout(() => {
        const element = document.getElementById(hash);
        if (element) {
          element.scrollIntoView({ behavior: 'smooth', block: 'start' });
        }
      }, 100);
    }
  }, []);
}

export default function ExamplesPage() {
  useHashNavigation();
  return (
    <div className="prose max-w-none">
      <h2 className="text-3xl font-bold mb-6 text-[var(--primary-orange)]">Example Projects</h2>

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
          <AnchorHeading
            id="random-ark"
            badges={
              <>
                <span className="ml-3 text-sm bg-blue-100 text-blue-800 px-3 py-1 rounded">WASI P1</span>
                <span className="ml-2 text-sm bg-green-100 text-green-800 px-3 py-1 rounded">Beginner</span>
              </>
            }
          >
            random-ark
          </AnchorHeading>
          <div className="flex flex-wrap gap-3 mt-4 mb-4">
            <a
              href="https://github.com/zavodil/random-ark"
              target="_blank"
              rel="noopener noreferrer"
              className="inline-flex items-center gap-2 px-4 py-2 bg-gray-900 text-white rounded-lg hover:bg-gray-800 transition-colors font-medium"
            >
              <svg className="w-5 h-5" fill="currentColor" viewBox="0 0 24 24">
                <path fillRule="evenodd" d="M12 2C6.477 2 2 6.484 2 12.017c0 4.425 2.865 8.18 6.839 9.504.5.092.682-.217.682-.483 0-.237-.008-.868-.013-1.703-2.782.605-3.369-1.343-3.369-1.343-.454-1.158-1.11-1.466-1.11-1.466-.908-.62.069-.608.069-.608 1.003.07 1.531 1.032 1.531 1.032.892 1.53 2.341 1.088 2.91.832.092-.647.35-1.088.636-1.338-2.22-.253-4.555-1.113-4.555-4.951 0-1.093.39-1.988 1.029-2.688-.103-.253-.446-1.272.098-2.65 0 0 .84-.27 2.75 1.026A9.564 9.564 0 0112 6.844c.85.004 1.705.115 2.504.337 1.909-1.296 2.747-1.027 2.747-1.027.546 1.379.202 2.398.1 2.651.64.7 1.028 1.595 1.028 2.688 0 3.848-2.339 4.695-4.566 4.943.359.309.678.92.678 1.855 0 1.338-.012 2.419-.012 2.747 0 .268.18.58.688.482A10.019 10.019 0 0022 12.017C22 6.484 17.522 2 12 2z" clipRule="evenodd"/>
              </svg>
              <span>Source Code on GitHub</span>
            </a>
            <Link
              href="/playground#random-number-generator"
              className="inline-flex items-center gap-2 px-4 py-2 bg-[var(--primary-orange)] text-white rounded-lg hover:bg-orange-600 transition-colors font-medium"
            >
              <svg className="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M14.752 11.168l-3.197-2.132A1 1 0 0010 9.87v4.263a1 1 0 001.555.832l3.197-2.132a1 1 0 000-1.664z" />
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
              </svg>
              <span>Try in Playground</span>
            </Link>
          </div>
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
          <AnchorHeading
            id="echo-ark"
            badges={
              <>
                <span className="ml-3 text-sm bg-blue-100 text-blue-800 px-3 py-1 rounded">WASI P1</span>
                <span className="ml-2 text-sm bg-green-100 text-green-800 px-3 py-1 rounded">Beginner</span>
              </>
            }
          >
            echo-ark
          </AnchorHeading>
          <div className="flex flex-wrap gap-3 mt-4 mb-4">
            <a
              href="https://github.com/zavodil/echo-ark"
              target="_blank"
              rel="noopener noreferrer"
              className="inline-flex items-center gap-2 px-4 py-2 bg-gray-900 text-white rounded-lg hover:bg-gray-800 transition-colors font-medium"
            >
              <svg className="w-5 h-5" fill="currentColor" viewBox="0 0 24 24">
                <path fillRule="evenodd" d="M12 2C6.477 2 2 6.484 2 12.017c0 4.425 2.865 8.18 6.839 9.504.5.092.682-.217.682-.483 0-.237-.008-.868-.013-1.703-2.782.605-3.369-1.343-3.369-1.343-.454-1.158-1.11-1.466-1.11-1.466-.908-.62.069-.608.069-.608 1.003.07 1.531 1.032 1.531 1.032.892 1.53 2.341 1.088 2.91.832.092-.647.35-1.088.636-1.338-2.22-.253-4.555-1.113-4.555-4.951 0-1.093.39-1.988 1.029-2.688-.103-.253-.446-1.272.098-2.65 0 0 .84-.27 2.75 1.026A9.564 9.564 0 0112 6.844c.85.004 1.705.115 2.504.337 1.909-1.296 2.747-1.027 2.747-1.027.546 1.379.202 2.398.1 2.651.64.7 1.028 1.595 1.028 2.688 0 3.848-2.339 4.695-4.566 4.943.359.309.678.92.678 1.855 0 1.338-.012 2.419-.012 2.747 0 .268.18.58.688.482A10.019 10.019 0 0022 12.017C22 6.484 17.522 2 12 2z" clipRule="evenodd"/>
              </svg>
              <span>Source Code on GitHub</span>
            </a>
            <Link
              href="/playground#echo-generator"
              className="inline-flex items-center gap-2 px-4 py-2 bg-[var(--primary-orange)] text-white rounded-lg hover:bg-orange-600 transition-colors font-medium"
            >
              <svg className="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M14.752 11.168l-3.197-2.132A1 1 0 0010 9.87v4.263a1 1 0 001.555.832l3.197-2.132a1 1 0 000-1.664z" />
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
              </svg>
              <span>Try in Playground</span>
            </Link>
          </div>
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
          <AnchorHeading
            id="ai-ark"
            badges={
              <>
                <span className="ml-3 text-sm bg-purple-100 text-purple-800 px-3 py-1 rounded">WASI P2</span>
                <span className="ml-2 text-sm bg-yellow-100 text-yellow-800 px-3 py-1 rounded">Intermediate</span>
              </>
            }
          >
            ai-ark
          </AnchorHeading>
          <div className="flex flex-wrap gap-3 mt-4 mb-4">
            <a
              href="https://github.com/zavodil/ai-ark"
              target="_blank"
              rel="noopener noreferrer"
              className="inline-flex items-center gap-2 px-4 py-2 bg-gray-900 text-white rounded-lg hover:bg-gray-800 transition-colors font-medium"
            >
              <svg className="w-5 h-5" fill="currentColor" viewBox="0 0 24 24">
                <path fillRule="evenodd" d="M12 2C6.477 2 2 6.484 2 12.017c0 4.425 2.865 8.18 6.839 9.504.5.092.682-.217.682-.483 0-.237-.008-.868-.013-1.703-2.782.605-3.369-1.343-3.369-1.343-.454-1.158-1.11-1.466-1.11-1.466-.908-.62.069-.608.069-.608 1.003.07 1.531 1.032 1.531 1.032.892 1.53 2.341 1.088 2.91.832.092-.647.35-1.088.636-1.338-2.22-.253-4.555-1.113-4.555-4.951 0-1.093.39-1.988 1.029-2.688-.103-.253-.446-1.272.098-2.65 0 0 .84-.27 2.75 1.026A9.564 9.564 0 0112 6.844c.85.004 1.705.115 2.504.337 1.909-1.296 2.747-1.027 2.747-1.027.546 1.379.202 2.398.1 2.651.64.7 1.028 1.595 1.028 2.688 0 3.848-2.339 4.695-4.566 4.943.359.309.678.92.678 1.855 0 1.338-.012 2.419-.012 2.747 0 .268.18.58.688.482A10.019 10.019 0 0022 12.017C22 6.484 17.522 2 12 2z" clipRule="evenodd"/>
              </svg>
              <span>Source Code on GitHub</span>
            </a>
            <Link
              href="/playground#ai-completions"
              className="inline-flex items-center gap-2 px-4 py-2 bg-[var(--primary-orange)] text-white rounded-lg hover:bg-orange-600 transition-colors font-medium"
            >
              <svg className="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M14.752 11.168l-3.197-2.132A1 1 0 0010 9.87v4.263a1 1 0 001.555.832l3.197-2.132a1 1 0 000-1.664z" />
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
              </svg>
              <span>Try in Playground</span>
            </Link>
          </div>
          <p className="text-gray-700 mb-4">
            OpenAI API integration with HTTPS POST requests. Your first step into WASI Preview 2 capabilities and external API calls. You can store a custom <code className="bg-gray-100 px-2 py-1 rounded">SYSTEM_PROMPT</code> in encrypted secrets to control AI behavior - it will be automatically injected but hidden from end users.
          </p>

          <h4 className="font-semibold mt-4 mb-2">Key Features:</h4>
          <ul className="list-disc list-inside text-gray-700 space-y-1 mb-4">
            <li>HTTPS client using <code className="bg-gray-100 px-2 py-1 rounded">wasi-http-client</code> crate</li>
            <li><strong>Custom system prompts via <code className="bg-gray-100 px-2 py-1 rounded">SYSTEM_PROMPT</code> secret</strong> - control AI behavior invisibly</li>
            <li>External API integration pattern</li>
            <li>Component model example (WASI P2)</li>
            <li>Fuel metering demonstration</li>
            <li>JSON request/response handling</li>
            <li>Conversation history support</li>
          </ul>

          <h4 className="font-semibold mt-4 mb-2">Requirements:</h4>
          <ul className="list-disc list-inside text-gray-700 space-y-1 mb-4 ml-4">
            <li><code className="bg-gray-100 px-2 py-1 rounded">OPENAI_API_KEY</code> - Required (stored as encrypted secret)</li>
            <li><code className="bg-gray-100 px-2 py-1 rounded">SYSTEM_PROMPT</code> - Optional (control AI behavior invisibly)</li>
            <li>wasmtime 28+ for local testing</li>
            <li>Network access during execution</li>
          </ul>

          <h4 className="font-semibold mt-4 mb-2">How to Use:</h4>
          <SyntaxHighlighter language="bash" style={vscDarkPlus} customStyle={{ borderRadius: '0.5rem', fontSize: '0.875rem' }}>
{`# 1. Get OpenAI API key from https://platform.openai.com/api-keys

# 2. Store secrets via Dashboard (with optional SYSTEM_PROMPT)
# Open https://outlayer.fastnear.com/secrets and create:
# - Repo: github.com/YOUR_USERNAME/ai-ark
# - Branch: main
# - Profile: production
# - Secrets JSON:
#   {
#     "OPENAI_API_KEY": "sk-...",
#     "SYSTEM_PROMPT": "Only start sentences with O. Omit extra words."
#   }
# Note: SYSTEM_PROMPT is optional but powerful - it controls AI behavior
# while staying hidden from end users who only provide the prompt

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
# - Decrypt secrets and inject OPENAI_API_KEY + SYSTEM_PROMPT into environment
# - SYSTEM_PROMPT is automatically added to conversation (hidden from user)
# - Execute WASM with user's prompt
# - Return AI response (following system prompt rules)`}
          </SyntaxHighlighter>
        </div>

        {/* weather-ark */}
        <div id="weather-ark" className="border border-gray-200 rounded-lg p-6 hover:shadow-lg transition-shadow scroll-mt-4">
          <AnchorHeading
            id="weather-ark"
            badges={
              <>
                <span className="ml-3 text-sm bg-purple-100 text-purple-800 px-3 py-1 rounded">WASI P2</span>
                <span className="ml-2 text-sm bg-yellow-100 text-yellow-800 px-3 py-1 rounded">Intermediate</span>
              </>
            }
          >
            weather-ark
          </AnchorHeading>
          <div className="flex flex-wrap gap-3 mt-4 mb-4">
            <a
              href="https://github.com/zavodil/weather-ark"
              target="_blank"
              rel="noopener noreferrer"
              className="inline-flex items-center gap-2 px-4 py-2 bg-gray-900 text-white rounded-lg hover:bg-gray-800 transition-colors font-medium"
            >
              <svg className="w-5 h-5" fill="currentColor" viewBox="0 0 24 24">
                <path fillRule="evenodd" d="M12 2C6.477 2 2 6.484 2 12.017c0 4.425 2.865 8.18 6.839 9.504.5.092.682-.217.682-.483 0-.237-.008-.868-.013-1.703-2.782.605-3.369-1.343-3.369-1.343-.454-1.158-1.11-1.466-1.11-1.466-.908-.62.069-.608.069-.608 1.003.07 1.531 1.032 1.531 1.032.892 1.53 2.341 1.088 2.91.832.092-.647.35-1.088.636-1.338-2.22-.253-4.555-1.113-4.555-4.951 0-1.093.39-1.988 1.029-2.688-.103-.253-.446-1.272.098-2.65 0 0 .84-.27 2.75 1.026A9.564 9.564 0 0112 6.844c.85.004 1.705.115 2.504.337 1.909-1.296 2.747-1.027 2.747-1.027.546 1.379.202 2.398.1 2.651.64.7 1.028 1.595 1.028 2.688 0 3.848-2.339 4.695-4.566 4.943.359.309.678.92.678 1.855 0 1.338-.012 2.419-.012 2.747 0 .268.18.58.688.482A10.019 10.019 0 0022 12.017C22 6.484 17.522 2 12 2z" clipRule="evenodd"/>
              </svg>
              <span>Source Code on GitHub</span>
            </a>
            <Link
              href="/playground#weather-data-oracle"
              className="inline-flex items-center gap-2 px-4 py-2 bg-[var(--primary-orange)] text-white rounded-lg hover:bg-orange-600 transition-colors font-medium"
            >
              <svg className="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M14.752 11.168l-3.197-2.132A1 1 0 0010 9.87v4.263a1 1 0 001.555.832l3.197-2.132a1 1 0 000-1.664z" />
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
              </svg>
              <span>Try in Playground</span>
            </Link>
          </div>
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

        {/* botfather-ark */}
        <div id="botfather-ark" className="border border-gray-200 rounded-lg p-6 hover:shadow-lg transition-shadow scroll-mt-4">
          <AnchorHeading
            id="botfather-ark"
            badges={
              <>
                <span className="ml-3 text-sm bg-purple-100 text-purple-800 px-3 py-1 rounded">WASI P2</span>
                <span className="ml-2 text-sm bg-yellow-100 text-yellow-800 px-3 py-1 rounded">Intermediate</span>
              </>
            }
          >
            botfather-ark
          </AnchorHeading>
          <div className="flex flex-wrap gap-3 mt-4 mb-4">
            <a
              href="https://github.com/zavodil/botfather-ark"
              target="_blank"
              rel="noopener noreferrer"
              className="inline-flex items-center gap-2 px-4 py-2 bg-gray-900 text-white rounded-lg hover:bg-gray-800 transition-colors font-medium"
            >
              <svg className="w-5 h-5" fill="currentColor" viewBox="0 0 24 24">
                <path fillRule="evenodd" d="M12 2C6.477 2 2 6.484 2 12.017c0 4.425 2.865 8.18 6.839 9.504.5.092.682-.217.682-.483 0-.237-.008-.868-.013-1.703-2.782.605-3.369-1.343-3.369-1.343-.454-1.158-1.11-1.466-1.11-1.466-.908-.62.069-.608.069-.608 1.003.07 1.531 1.032 1.531 1.032.892 1.53 2.341 1.088 2.91.832.092-.647.35-1.088.636-1.338-2.22-.253-4.555-1.113-4.555-4.951 0-1.093.39-1.988 1.029-2.688-.103-.253-.446-1.272.098-2.65 0 0 .84-.27 2.75 1.026A9.564 9.564 0 0112 6.844c.85.004 1.705.115 2.504.337 1.909-1.296 2.747-1.027 2.747-1.027.546 1.379.202 2.398.1 2.651.64.7 1.028 1.595 1.028 2.688 0 3.848-2.339 4.695-4.566 4.943.359.309.678.92.678 1.855 0 1.338-.012 2.419-.012 2.747 0 .268.18.58.688.482A10.019 10.019 0 0022 12.017C22 6.484 17.522 2 12 2z" clipRule="evenodd"/>
              </svg>
              <span>Source Code on GitHub</span>
            </a>
          </div>
          <p className="text-gray-700 mb-4">
            Account factory pattern for NEAR - create and manage multiple NEAR accounts with AI-generated names using hierarchical key derivation. Demonstrates advanced host function usage with <code className="bg-gray-100 px-2 py-1 rounded">call()</code> and <code className="bg-gray-100 px-2 py-1 rounded">transfer()</code>, providing access to private NEAR RPC endpoints (powered by FastNEAR).
          </p>

          <h4 className="font-semibold mt-4 mb-2">Use Cases:</h4>
          <ul className="list-disc list-inside text-gray-700 space-y-1 mb-4">
            <li><strong>Account Factory:</strong> Generate multiple NEAR accounts programmatically</li>
            <li><strong>Batch Operations:</strong> Execute contract calls on multiple accounts simultaneously (e.g., buy tokens, delegate to staking pool)</li>
            <li><strong>Onboarding Platform:</strong> Create named accounts for users via Telegram bots or web apps</li>
            <li><strong>Sub-Account Management:</strong> Manage hierarchical account structures with deterministic key derivation</li>
          </ul>

          <h4 className="font-semibold mt-4 mb-2">Key Features:</h4>
          <ul className="list-disc list-inside text-gray-700 space-y-1 mb-4">            
            <li>AI-powered account name generation (GPT integration)</li>
            <li>Deterministic key derivation from master seed using SHA-256</li>
            <li>Account discovery via Fastnear API</li>
            <li>Batch contract calls with template variables (<code className="bg-gray-100 px-2 py-1 rounded">{`{{account_id}}`}</code>)</li>
            <li>Fund distribution across multiple accounts</li>
            <li>Uses <code className="bg-gray-100 px-2 py-1 rounded">near:rpc/api</code> host functions: <code className="bg-gray-100 px-2 py-1 rounded">call()</code>, <code className="bg-gray-100 px-2 py-1 rounded">transfer()</code></li>
          </ul>

          <h4 className="font-semibold mt-4 mb-2">Actions:</h4>
          <div className="space-y-3">
            <div>
              <p className="text-sm font-medium text-gray-800 mb-1">1. Create Accounts:</p>
              <SyntaxHighlighter language="json" style={vscDarkPlus} customStyle={{ borderRadius: '0.5rem', fontSize: '0.875rem' }}>
{`{
  "action": "create_accounts",
  "prompt": "space exploration theme",
  "count": 3,
  "deposit_per_account": "1000000000000000000000000"
}`}
              </SyntaxHighlighter>
              <p className="text-sm text-gray-600 mt-1">Creates accounts like <code className="bg-gray-100 px-2 py-1 rounded">mars-rover.testnet</code>, <code className="bg-gray-100 px-2 py-1 rounded">moon-base.testnet</code></p>
            </div>

            <div>
              <p className="text-sm font-medium text-gray-800 mb-1">2. Fund Accounts:</p>
              <SyntaxHighlighter language="json" style={vscDarkPlus} customStyle={{ borderRadius: '0.5rem', fontSize: '0.875rem' }}>
{`{
  "action": "fund_accounts",
  "total_amount": "30000000000000000000000000",
  "indices": []
}`}
              </SyntaxHighlighter>
              <p className="text-sm text-gray-600 mt-1">Empty <code className="bg-gray-100 px-2 py-1 rounded">indices</code> = fund all accounts equally (30 NEAR ÷ 3 accounts = 10 NEAR each)</p>
            </div>

            <div>
              <p className="text-sm font-medium text-gray-800 mb-1">3. Batch Contract Calls:</p>
              <SyntaxHighlighter language="json" style={vscDarkPlus} customStyle={{ borderRadius: '0.5rem', fontSize: '0.875rem' }}>
{`{
  "action": "batch_call",
  "contract_id": "token.near",
  "method_name": "transfer",
  "args": {"receiver_id": "{{account_id}}", "amount": "1000"},
  "deposit": "1",
  "gas": "30000000000000",
  "indices": [0, 2]
}`}
              </SyntaxHighlighter>
              <p className="text-sm text-gray-600 mt-1">Execute on accounts at indices 0 and 2. Use <code className="bg-gray-100 px-2 py-1 rounded">{`{{account_id}}`}</code> placeholder for dynamic account ID</p>
            </div>

            <div>
              <p className="text-sm font-medium text-gray-800 mb-1">4. List Accounts:</p>
              <SyntaxHighlighter language="json" style={vscDarkPlus} customStyle={{ borderRadius: '0.5rem', fontSize: '0.875rem' }}>
{`{
  "action": "list_accounts"
}`}
              </SyntaxHighlighter>
              <p className="text-sm text-gray-600 mt-1">Returns all created accounts with balances and public keys</p>
            </div>
          </div>

          <h4 className="font-semibold mt-4 mb-2">Output Example:</h4>
          <SyntaxHighlighter language="json" style={vscDarkPlus} customStyle={{ borderRadius: '0.5rem', fontSize: '0.875rem' }}>
{`{
  "success": true,
  "accounts": [
    {
      "index": 0,
      "account_id": "mars_rover.testnet",
      "public_key": "ed25519:...",
      "balance": "1000000000000000000000000",
      "balance_near": "1.0000"
    }
  ],
  "transactions": [
    {
      "account_id": "mars_rover.testnet",
      "tx_hash": "Abc123...",
      "success": true
    }
  ]
}`}
          </SyntaxHighlighter>

          <div className="mt-4 p-4 bg-blue-50 border-l-4 border-blue-400">
            <h4 className="font-semibold text-gray-900 mb-2">How It Works</h4>
            <ol className="list-decimal list-inside text-sm text-gray-700 space-y-1">
              <li><strong>Key Derivation in TEE:</strong> Master key (<code className="bg-white px-2 py-1 rounded text-xs">PROTECTED_MASTER_KEY</code>) is generated inside TEE (nobody ever sees it). Derived keys are created using SHA-256: <code className="bg-white px-2 py-1 rounded text-xs">SHA256(master + sender_id + index)</code> - each derived key controls one account</li>
              <li><strong>Account Discovery:</strong> Queries Fastnear API to find existing accounts by derived public keys (stateless operation)</li>
              <li><strong>AI Name Generation:</strong> Calls OpenAI API to generate creative account names based on theme prompt</li>
              <li><strong>Account Creation:</strong> Uses NEAR RPC <code className="bg-white px-2 py-1 rounded text-xs">call()</code> host function to create accounts via <code className="bg-white px-2 py-1 rounded text-xs">create_account</code> action</li>
              <li><strong>Batch Execution:</strong> Iterates through account indices and executes operations using <code className="bg-white px-2 py-1 rounded text-xs">call()</code> or <code className="bg-white px-2 py-1 rounded text-xs">transfer()</code></li>
            </ol>
          </div>

          <h4 className="font-semibold mt-4 mb-2">Setting Up Secrets</h4>
          <div className="bg-purple-50 border-l-4 border-purple-400 p-4 mb-4">
            <p className="text-sm text-purple-900 font-medium mb-2">
              ⚠️ This example is not available in Playground because it requires creating a secret with <code className="bg-purple-100 px-1 rounded text-xs">NEAR_SENDER_PRIVATE_KEY</code> - your NEAR account&apos;s private key (with NEAR tokens) to pay for account creation and funding.
            </p>
            <p className="text-sm text-gray-700 mb-3">
              To use this example, create a secret in the <Link href="/secrets" className="text-[var(--primary-orange)] hover:underline font-semibold">Secrets page</Link> with the following configuration:
            </p>

            <h5 className="font-semibold text-gray-900 mb-2 text-sm">Manual Secrets:</h5>
            <SyntaxHighlighter language="json" style={vscDarkPlus} customStyle={{ borderRadius: '0.5rem', fontSize: '0.75rem', marginBottom: '0.75rem' }}>
{`{
  "NEAR_SENDER_PRIVATE_KEY": "ed25519:your_private_key",
  "OPENAI_API_KEY": "sk-...",
  "OPENAI_ENDPOINT": "https://api.openai.com/v1/chat/completions",
  "OPENAI_MODEL": "gpt-3.5-turbo"
}`}
            </SyntaxHighlighter>

            <h5 className="font-semibold text-gray-900 mb-2 text-sm">Generated Secret (created in TEE):</h5>
            <ul className="list-disc list-inside text-sm text-gray-700 ml-4 space-y-1 mb-3">
              <li>Click <strong>&quot;Generate Secret&quot;</strong> button</li>
              <li>Secret name: <code className="bg-purple-100 px-1 rounded text-xs">PROTECTED_MASTER_KEY</code></li>
              <li>Type: <code className="bg-purple-100 px-1 rounded text-xs">ED25519</code> (generates ed25519 key pair in TEE)</li>
              <li>This key is created inside TEE and never exposed - used to derive all account keys</li>
            </ul>

            <h5 className="font-semibold text-gray-900 mb-2 text-sm">Access Control:</h5>
            <p className="text-sm text-gray-700 mb-1">
              Set <strong>👥 Whitelist</strong> with your account ID (the one that will manage created accounts)
            </p>
          </div>

          <h4 className="font-semibold mt-4 mb-2">Technical Details:</h4>
          <ul className="list-disc list-inside text-gray-700 mb-4 space-y-1 text-sm">
            <li><strong>WASI Version:</strong> Preview 2 (component model)</li>
            <li><strong>Language:</strong> Rust</li>
            <li><strong>WIT Interface:</strong> <code className="bg-gray-100 px-2 py-1 rounded">near:rpc/api</code> (call, transfer, view)</li>
            <li><strong>Secrets:</strong> Required (OpenAI key, master key, sender credentials)</li>
            <li><strong>Network:</strong> Required (OpenAI API, Fastnear API, NEAR RPC)</li>
            <li><strong>Key Feature:</strong> Demonstrates advanced host function usage - WASM provides signer credentials, worker never signs with its own key</li>
          </ul>

          <div className="mt-4 p-4 bg-yellow-50 border-l-4 border-yellow-400">
            <h4 className="font-semibold text-gray-900 mb-2">Important Security Notes</h4>
            <ul className="text-sm text-gray-700 space-y-1">
              <li>✅ <strong>WASM provides signer:</strong> User&apos;s NEAR private key is passed via secrets (not worker&apos;s key)</li>
              <li>✅ <strong>Keys created in TEE:</strong> Master key (<code className="bg-white px-2 py-1 rounded text-xs">PROTECTED_MASTER_KEY</code>) is generated inside TEE and never leaves it. Derived keys are created using deterministic SHA-256 derivation from master seed.</li>
              <li>✅ <strong>Accounts managed only in TEE:</strong> All derived accounts are controlled exclusively by keys that exist only inside TEE - nobody can export or see the private keys</li>
              <li>✅ <strong>Deterministic keys:</strong> Same master seed + sender + index always generates same account key</li>
              <li>✅ <strong>Master key isolation:</strong> Each <code className="bg-white px-2 py-1 rounded text-xs">NEAR_SENDER_ID</code> has isolated account space</li>
              <li>⚠️ <strong>Store master key safely:</strong> Loss of master key = loss of access to all derived accounts</li>
            </ul>
          </div>

          <div className="mt-4 p-4 bg-gray-50 rounded-lg">
            <h4 className="font-semibold text-gray-900 mb-2">Learn More</h4>
            <ul className="text-sm text-gray-700 space-y-1">
              <li>
                📖 <a href="https://github.com/zavodil/botfather-ark/blob/main/README.md" target="_blank" rel="noopener noreferrer" className="text-[var(--primary-orange)] hover:underline">Full Documentation on GitHub</a>
              </li>
              <li>
                🔧 <a href="https://github.com/zavodil/botfather-ark/blob/main/build.sh" target="_blank" rel="noopener noreferrer" className="text-[var(--primary-orange)] hover:underline">Build Script & Configuration</a>
              </li>
              <li>
                💡 <Link href="/docs/wasi#host-functions" className="text-[var(--primary-orange)] hover:underline">Understanding Host Functions</Link>
              </li>
            </ul>
          </div>
        </div>

        {/* oracle-ark */}
        <div id="oracle-ark" className="border border-gray-200 rounded-lg p-6 hover:shadow-lg transition-shadow scroll-mt-4">
          <AnchorHeading
            id="oracle-ark"
            badges={
              <>
                <span className="ml-3 text-sm bg-purple-100 text-purple-800 px-3 py-1 rounded">WASI P2</span>
                <span className="ml-2 text-sm bg-orange-100 text-orange-800 px-3 py-1 rounded">Advanced</span>
              </>
            }
          >
            oracle-ark
          </AnchorHeading>
          <div className="flex flex-wrap gap-3 mt-4 mb-4">
            <a
              href="https://github.com/zavodil/oracle-ark"
              target="_blank"
              rel="noopener noreferrer"
              className="inline-flex items-center gap-2 px-4 py-2 bg-gray-900 text-white rounded-lg hover:bg-gray-800 transition-colors font-medium"
            >
              <svg className="w-5 h-5" fill="currentColor" viewBox="0 0 24 24">
                <path fillRule="evenodd" d="M12 2C6.477 2 2 6.484 2 12.017c0 4.425 2.865 8.18 6.839 9.504.5.092.682-.217.682-.483 0-.237-.008-.868-.013-1.703-2.782.605-3.369-1.343-3.369-1.343-.454-1.158-1.11-1.466-1.11-1.466-.908-.62.069-.608.069-.608 1.003.07 1.531 1.032 1.531 1.032.892 1.53 2.341 1.088 2.91.832.092-.647.35-1.088.636-1.338-2.22-.253-4.555-1.113-4.555-4.951 0-1.093.39-1.988 1.029-2.688-.103-.253-.446-1.272.098-2.65 0 0 .84-.27 2.75 1.026A9.564 9.564 0 0112 6.844c.85.004 1.705.115 2.504.337 1.909-1.296 2.747-1.027 2.747-1.027.546 1.379.202 2.398.1 2.651.64.7 1.028 1.595 1.028 2.688 0 3.848-2.339 4.695-4.566 4.943.359.309.678.92.678 1.855 0 1.338-.012 2.419-.012 2.747 0 .268.18.58.688.482A10.019 10.019 0 0022 12.017C22 6.484 17.522 2 12 2z" clipRule="evenodd"/>
              </svg>
              <span>Source Code on GitHub</span>
            </a>
            <Link
              href="/playground#multi-source-data-oracle"
              className="inline-flex items-center gap-2 px-4 py-2 bg-[var(--primary-orange)] text-white rounded-lg hover:bg-orange-600 transition-colors font-medium"
            >
              <svg className="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M14.752 11.168l-3.197-2.132A1 1 0 0010 9.87v4.263a1 1 0 001.555.832l3.197-2.132a1 1 0 000-1.664z" />
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
              </svg>
              <span>Try in Playground</span>
            </Link>
          </div>
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

        {/* ethereum-api */}
        <div id="ethereum-api" className="border border-gray-200 rounded-lg p-6 hover:shadow-lg transition-shadow scroll-mt-4">
          <AnchorHeading
            id="ethereum-api"
            badges={
              <>
                <span className="ml-3 text-sm bg-purple-100 text-purple-800 px-3 py-1 rounded">WASI P2</span>
                <span className="ml-2 text-sm bg-orange-100 text-orange-800 px-3 py-1 rounded">Advanced</span>
              </>
            }
          >
            Ethereum API Oracle
          </AnchorHeading>
          <div className="flex flex-wrap gap-3 mt-4 mb-4">
            <a
              href="https://github.com/zavodil/oracle-ark"
              target="_blank"
              rel="noopener noreferrer"
              className="inline-flex items-center gap-2 px-4 py-2 bg-gray-900 text-white rounded-lg hover:bg-gray-800 transition-colors font-medium"
            >
              <svg className="w-5 h-5" fill="currentColor" viewBox="0 0 24 24">
                <path fillRule="evenodd" d="M12 2C6.477 2 2 6.484 2 12.017c0 4.425 2.865 8.18 6.839 9.504.5.092.682-.217.682-.483 0-.237-.008-.868-.013-1.703-2.782.605-3.369-1.343-3.369-1.343-.454-1.158-1.11-1.466-1.11-1.466-.908-.62.069-.608.069-.608 1.003.07 1.531 1.032 1.531 1.032.892 1.53 2.341 1.088 2.91.832.092-.647.35-1.088.636-1.338-2.22-.253-4.555-1.113-4.555-4.951 0-1.093.39-1.988 1.029-2.688-.103-.253-.446-1.272.098-2.65 0 0 .84-.27 2.75 1.026A9.564 9.564 0 0112 6.844c.85.004 1.705.115 2.504.337 1.909-1.296 2.747-1.027 2.747-1.027.546 1.379.202 2.398.1 2.651.64.7 1.028 1.595 1.028 2.688 0 3.848-2.339 4.695-4.566 4.943.359.309.678.92.678 1.855 0 1.338-.012 2.419-.012 2.747 0 .268.18.58.688.482A10.019 10.019 0 0022 12.017C22 6.484 17.522 2 12 2z" clipRule="evenodd"/>
              </svg>
              <span>Source Code on GitHub</span>
            </a>
            <Link
              href="/playground#ethereum-api"
              className="inline-flex items-center gap-2 px-4 py-2 bg-[var(--primary-orange)] text-white rounded-lg hover:bg-orange-600 transition-colors font-medium"
            >
              <svg className="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M14.752 11.168l-3.197-2.132A1 1 0 0010 9.87v4.263a1 1 0 001.555.832l3.197-2.132a1 1 0 000-1.664z" />
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
              </svg>
              <span>Try in Playground</span>
            </Link>
          </div>
          <p className="text-gray-700 mb-4">
            Query Ethereum blockchain data via JSON-RPC (balances, smart contracts, transactions). Similar to the previous oracle example but configured for Ethereum node providers like Alchemy. Bridge NEAR with Ethereum data for cross-chain applications.
          </p>

          <h4 className="font-semibold mt-4 mb-2">Key Features:</h4>
          <ul className="list-disc list-inside text-gray-700 space-y-1 mb-4">
            <li>Ethereum JSON-RPC API integration (eth_getBalance, eth_call, etc.)</li>
            <li>Support for Alchemy, Infura, and other Ethereum node providers</li>
            <li>Custom request structure with JSON path extraction</li>
            <li>Cross-chain data oracle for NEAR ↔ Ethereum bridges</li>
            <li>Encrypted API key storage (Alchemy secrets profile)</li>
            <li>Production-ready on testnet and mainnet</li>
          </ul>

          <h4 className="font-semibold mt-4 mb-2">Input Example (Check ETH Balance):</h4>
          <SyntaxHighlighter language="json" style={vscDarkPlus} customStyle={{ borderRadius: '0.5rem', fontSize: '0.875rem' }}>
{`{
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
          </SyntaxHighlighter>

          <h4 className="font-semibold mt-4 mb-2">Output Example:</h4>
          <SyntaxHighlighter language="json" style={vscDarkPlus} customStyle={{ borderRadius: '0.5rem', fontSize: '0.875rem' }}>
{`{
  "results": [
    {
      "id": "alchemy",
      "value": "0x1bc16d674ec80000",
      "sources_used": 1
    }
  ]
}`}
          </SyntaxHighlighter>

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

          <h4 className="font-semibold mt-4 mb-2">Use Cases:</h4>
          <ul className="list-disc list-inside text-gray-700 space-y-1 mb-4">
            <li><strong>Cross-Chain Bridges:</strong> Verify Ethereum transactions on NEAR</li>
            <li><strong>DeFi Integration:</strong> Monitor Ethereum token balances from NEAR contracts</li>
            <li><strong>Multi-Chain Wallets:</strong> Display ETH balances in NEAR apps</li>
            <li><strong>Smart Contract State:</strong> Read Ethereum contract data (ERC20, NFTs)</li>
            <li><strong>Block Explorer:</strong> Query Ethereum transaction history</li>
          </ul>

          <h4 className="font-semibold mt-4 mb-2">Notes:</h4>
          <ul className="list-disc list-inside text-gray-700 space-y-1">
            <li>Same codebase as <code className="bg-gray-100 px-2 py-1 rounded">oracle-ark</code>, different secrets configuration</li>
            <li>Requires Alchemy API key (free tier: 300M compute units/month)</li>
            <li>Supports any Ethereum JSON-RPC method (eth_call, eth_getTransactionReceipt, etc.)</li>
            <li>Pre-configured secrets available on testnet: <code className="bg-gray-100 px-2 py-1 rounded">zavodil2.testnet</code></li>
            <li>Mainnet secrets: <code className="bg-gray-100 px-2 py-1 rounded">zavodil.near</code></li>
          </ul>
        </div>

        {/* intents-ark */}
        <div id="intents-ark" className="border border-gray-200 rounded-lg p-6 hover:shadow-lg transition-shadow scroll-mt-4">
          <AnchorHeading
            id="intents-ark"
            badges={
              <>
                <span className="ml-3 text-sm bg-purple-100 text-purple-800 px-3 py-1 rounded">WASI P2</span>
                <span className="ml-2 text-sm bg-orange-100 text-orange-800 px-3 py-1 rounded">Advanced</span>
              </>
            }
          >
            intents-ark
          </AnchorHeading>
          <div className="flex flex-wrap gap-3 mt-4 mb-4">
            <a
              href="https://github.com/zavodil/intents-ark"
              target="_blank"
              rel="noopener noreferrer"
              className="inline-flex items-center gap-2 px-4 py-2 bg-gray-900 text-white rounded-lg hover:bg-gray-800 transition-colors font-medium"
            >
              <svg className="w-5 h-5" fill="currentColor" viewBox="0 0 24 24">
                <path fillRule="evenodd" d="M12 2C6.477 2 2 6.484 2 12.017c0 4.425 2.865 8.18 6.839 9.504.5.092.682-.217.682-.483 0-.237-.008-.868-.013-1.703-2.782.605-3.369-1.343-3.369-1.343-.454-1.158-1.11-1.466-1.11-1.466-.908-.62.069-.608.069-.608 1.003.07 1.531 1.032 1.531 1.032.892 1.53 2.341 1.088 2.91.832.092-.647.35-1.088.636-1.338-2.22-.253-4.555-1.113-4.555-4.951 0-1.093.39-1.988 1.029-2.688-.103-.253-.446-1.272.098-2.65 0 0 .84-.27 2.75 1.026A9.564 9.564 0 0112 6.844c.85.004 1.705.115 2.504.337 1.909-1.296 2.747-1.027 2.747-1.027.546 1.379.202 2.398.1 2.651.64.7 1.028 1.595 1.028 2.688 0 3.848-2.339 4.695-4.566 4.943.359.309.678.92.678 1.855 0 1.338-.012 2.419-.012 2.747 0 .268.18.58.688.482A10.019 10.019 0 0022 12.017C22 6.484 17.522 2 12 2z" clipRule="evenodd"/>
              </svg>
              <span>Source Code on GitHub</span>
            </a>
            <Link
              href="/playground#near-intents-swap"
              className="inline-flex items-center gap-2 px-4 py-2 bg-[var(--primary-orange)] text-white rounded-lg hover:bg-orange-600 transition-colors font-medium"
            >
              <svg className="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M14.752 11.168l-3.197-2.132A1 1 0 0010 9.87v4.263a1 1 0 001.555.832l3.197-2.132a1 1 0 000-1.664z" />
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
              </svg>
              <span>Try in Playground (Mainnet only)</span>
            </Link>
          </div>
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

        {/* private-dao-ark */}
        <div id="private-dao-ark" className="border border-gray-200 rounded-lg p-6 hover:shadow-lg transition-shadow scroll-mt-4">
          <AnchorHeading
            id="private-dao-ark"
            badges={
              <>
                <span className="ml-3 text-sm bg-purple-100 text-purple-800 px-3 py-1 rounded">WASI P1</span>
                <span className="ml-2 text-sm bg-red-100 text-red-800 px-3 py-1 rounded">Advanced</span>
              </>
            }
          >
            private-dao-ark
          </AnchorHeading>
          <a
            href="https://github.com/zavodil/private-dao-ark"
            target="_blank"
            rel="noopener noreferrer"
            className="inline-flex items-center gap-2 px-4 py-2 mt-4 mb-4 bg-gray-900 text-white rounded-lg hover:bg-gray-800 transition-colors font-medium"
          >
            <svg className="w-5 h-5" fill="currentColor" viewBox="0 0 24 24">
              <path fillRule="evenodd" d="M12 2C6.477 2 2 6.484 2 12.017c0 4.425 2.865 8.18 6.839 9.504.5.092.682-.217.682-.483 0-.237-.008-.868-.013-1.703-2.782.605-3.369-1.343-3.369-1.343-.454-1.158-1.11-1.466-1.11-1.466-.908-.62.069-.608.069-.608 1.003.07 1.531 1.032 1.531 1.032.892 1.53 2.341 1.088 2.91.832.092-.647.35-1.088.636-1.338-2.22-.253-4.555-1.113-4.555-4.951 0-1.093.39-1.988 1.029-2.688-.103-.253-.446-1.272.098-2.65 0 0 .84-.27 2.75 1.026A9.564 9.564 0 0112 6.844c.85.004 1.705.115 2.504.337 1.909-1.296 2.747-1.027 2.747-1.027.546 1.379.202 2.398.1 2.651.64.7 1.028 1.595 1.028 2.688 0 3.848-2.339 4.695-4.566 4.943.359.309.678.92.678 1.855 0 1.338-.012 2.419-.012 2.747 0 .268.18.58.688.482A10.019 10.019 0 0022 12.017C22 6.484 17.522 2 12 2z" clipRule="evenodd"/>
            </svg>
            <span>Source Code on GitHub</span>
          </a>
          <p className="text-gray-700 mb-4">
            Anonymous, verifiable DAO voting with cryptographic privacy. Heavy cryptography (ECIES encryption, HKDF key derivation, merkle tree proofs) executed off-chain in TEE. Each user&apos;s vote is encrypted on-chain, tallying happens in secure enclave, only aggregate counts are revealed.
          </p>

          <div className="mb-4 p-3 bg-purple-50 border-l-4 border-purple-400">
            <p className="text-sm text-gray-700 mb-2">
              🔐 <strong>Privacy Guarantees:</strong>
            </p>
            <ul className="text-sm text-gray-700 space-y-1 ml-4 list-disc">
              <li>Individual votes remain secret - only aggregate counts revealed</li>
              <li>DAO members can send encrypted noise instead of real votes to hide voting activity from observers</li>              
              <li>Merkle proofs allow voters to verify their vote was counted without revealing how they voted</li>
            </ul>
          </div>

          <h4 className="font-semibold mt-4 mb-2">Key Features:</h4>
          <ul className="list-disc list-inside text-gray-700 space-y-1 mb-4">
            <li>ECIES encryption for private votes (secp256k1)</li>
            <li>HKDF-SHA256 deterministic key derivation from single master secret</li>
            <li>Merkle tree construction for vote inclusion proofs</li>
            <li><strong>Dummy messages:</strong> Send encrypted noise to hide whether you voted (indistinguishable from real votes on-chain)</li>
            <li><strong>Vote changes:</strong> Vote multiple times, timestamp-based deduplication (latest vote wins)</li>
            <li>TEE attestation for execution integrity</li>
            <li>Full-stack React frontend with NEAR Wallet integration</li>
          </ul>

          <h4 className="font-semibold mt-4 mb-2">Architecture:</h4>
          <SyntaxHighlighter language="text" style={vscDarkPlus} customStyle={{ borderRadius: '0.5rem', fontSize: '0.75rem' }}>
{`1. Generate master secret:
   OutLayer → TEE generates random master secret (stored encrypted)

2. User joins DAO:
   Contract → OutLayer → TEE derives pubkey from master secret

3. User votes:
   Frontend encrypts vote with pubkey → Contract stores encrypted vote

4. Finalize proposal:
   Contract → OutLayer → TEE decrypts votes + tallies + builds merkle tree
   Returns aggregate counts + merkle proofs (individual votes never exposed)

5. Verify vote:
   User computes vote hash → Verifies merkle proof against root

Privacy Features:
- Individual votes never revealed (only aggregate counts)
- Dummy messages: Send encrypted noise to hide voting activity
- Vote changes: Vote multiple times, only latest counts (timestamped)
- Merkle proofs: Verify inclusion without revealing vote content

Cost: Heavy cryptography off-chain = ~$0.001 per vote`}
          </SyntaxHighlighter>

          <h4 className="font-semibold mt-4 mb-2">Cryptographic Components:</h4>
          <div className="space-y-2 mb-4">
            <div className="border-l-4 border-blue-400 pl-3">
              <strong className="text-sm">HKDF Key Derivation</strong>
              <SyntaxHighlighter language="rust" style={vscDarkPlus} customStyle={{ borderRadius: '0.5rem', fontSize: '0.75rem', marginTop: '0.5rem' }}>
{`// Single master secret → unique key per user
let info = format!("user:{}:{}", dao_account, user_account);
let user_privkey = hkdf_sha256(&master_secret, info.as_bytes());
let user_pubkey = secp256k1::derive_public_key(&user_privkey);`}
              </SyntaxHighlighter>
            </div>

            <div className="border-l-4 border-green-400 pl-3">
              <strong className="text-sm">ECIES Encryption (Frontend)</strong>
              <SyntaxHighlighter language="typescript" style={vscDarkPlus} customStyle={{ borderRadius: '0.5rem', fontSize: '0.75rem', marginTop: '0.5rem' }}>
{`import { encrypt } from 'eciesjs';

const vote = "yes";  // or "no"
const pubkeyHex = await contract.get_user_pubkey({ user });
const encrypted = encrypt(pubkeyHex, Buffer.from(vote));
await contract.cast_vote({ proposal_id, encrypted_vote: encrypted.toString('hex') });`}
              </SyntaxHighlighter>
            </div>

            <div className="border-l-4 border-purple-400 pl-3">
              <strong className="text-sm">Vote Hash Computation (Critical!)</strong>
              <SyntaxHighlighter language="typescript" style={vscDarkPlus} customStyle={{ borderRadius: '0.5rem', fontSize: '0.75rem', marginTop: '0.5rem' }}>
{`// Must preserve u64 precision - use BigInt!
const timestamp = result.receipts_outcome[0].outcome.status.SuccessValue;
const timestampStr = atob(timestamp).trim();  // Keep as string
const timestampBigInt = BigInt(timestampStr);

// Convert to 8-byte little-endian
const buffer = new ArrayBuffer(8);
new DataView(buffer).setBigUint64(0, timestampBigInt, true);

// SHA256(user + timestamp_le + encrypted)
const combined = concat(
  textEncoder.encode(accountId),
  new Uint8Array(buffer),
  textEncoder.encode(encrypted)
);
const voteHash = hex(await crypto.subtle.digest('SHA-256', combined));`}
              </SyntaxHighlighter>
            </div>

            <div className="border-l-4 border-orange-400 pl-3">
              <strong className="text-sm">Merkle Proof Verification</strong>
              <SyntaxHighlighter language="typescript" style={vscDarkPlus} customStyle={{ borderRadius: '0.5rem', fontSize: '0.75rem', marginTop: '0.5rem' }}>
{`// Try all possible paths (2^depth combinations)
async function verifyProof(voteHash, proofPath, merkleRoot) {
  async function tryPaths(hash, remaining) {
    if (!remaining.length) return hash === merkleRoot;

    const [sibling, ...rest] = remaining;

    // Try both orderings
    if (await tryPaths(await sha256(hash + sibling), rest)) return true;
    if (await tryPaths(await sha256(sibling + hash), rest)) return true;

    return false;
  }
  return await tryPaths(voteHash, proofPath);
}`}
              </SyntaxHighlighter>
            </div>
          </div>

          <h4 className="font-semibold mt-4 mb-2">How to Deploy:</h4>
          <SyntaxHighlighter language="bash" style={vscDarkPlus} customStyle={{ borderRadius: '0.5rem', fontSize: '0.875rem' }}>
{`# 1. Generate DAO master secret in TEE
# (Alternatively: generate locally and store encrypted)
openssl rand -hex 32 > dao_master_secret.txt

# 2. Store master secret via Dashboard (encrypted in keystore)
# Open https://outlayer.fastnear.com/secrets:
# - Repo: github.com/YOUR_USERNAME/private-dao-ark
# - Profile: production
# - Secrets JSON: {"DAO_MASTER_SECRET":"<paste hex from file>"}
# - Access: AllowAll (or Whitelist for DAO admin only)

# 3. Clone and build WASI module
git clone https://github.com/zavodil/private-dao-ark.git
cd private-dao-ark
cargo build --target wasm32-wasip1 --release
git push origin main

# 4. Deploy DAO contract
cd dao-contract
cargo near build
near deploy privatedao.testnet \\
  use-file res/private_dao_contract.wasm \\
  with-init-call new \\
  json-args '{
    "owner_id":"owner.testnet",
    "name":"My Private DAO",
    "outlayer_contract":"outlayer.testnet",
    "wasi_repo":"https://github.com/YOUR_USERNAME/private-dao-ark",
    "wasi_commit":"main",
    "secrets_profile":"production",
    "secrets_owner":"your.testnet",
    "membership_mode":"Public"
  }' \\
  prepaid-gas '100.0 Tgas' \\
  attached-deposit '0 NEAR'

# 5. Deploy frontend
cd ../dao-frontend
npm install
cat > .env <<EOF
REACT_APP_CONTRACT_ID=privatedao.testnet
REACT_APP_NEAR_NETWORK=testnet
EOF
npm run build
# Deploy build/ to Vercel/Netlify/Cloudflare Pages

# 6. Users can now:
# - Join DAO (get encrypted pubkey derived from master secret)
# - Create proposals with quorum requirements
# - Vote privately (votes encrypted with their pubkey)
# - Finalize proposals (OutLayer decrypts in TEE and tallies)
# - Verify their vote was counted (merkle proof verification)`}
          </SyntaxHighlighter>

          <h4 className="font-semibold mt-4 mb-2">Use Cases:</h4>
          <ul className="list-disc list-inside text-gray-700 space-y-2 mb-4 ml-4">
            <li><strong>Anonymous Governance:</strong> Board elections where individual votes should remain secret</li>
            <li><strong>Whistleblower Protection:</strong> Report issues without revealing identity</li>
            <li><strong>Salary Decisions:</strong> Vote on compensation without peer pressure</li>
            <li><strong>Grant Allocation:</strong> Fund projects while preventing vote buying</li>
            <li><strong>Conflict Resolution:</strong> Vote on sensitive matters privately</li>
          </ul>

          <h4 className="font-semibold mt-4 mb-2">Technical Highlights:</h4>
          <div className="grid grid-cols-1 md:grid-cols-2 gap-3 mb-4">
            <div className="p-3 bg-gray-50 border border-gray-200 rounded">
              <strong className="text-sm block mb-1">💰 Cost Efficiency</strong>
              <p className="text-xs text-gray-700">Heavy cryptography off-chain: ~$0.001/vote<br/>ECIES + HKDF + Merkle trees feasible with OutLayer</p>
            </div>
            <div className="p-3 bg-gray-50 border border-gray-200 rounded">
              <strong className="text-sm block mb-1">🔒 Privacy Model</strong>
              <p className="text-xs text-gray-700">Encrypted votes on-chain<br/>Dummy messages hide voting activity<br/>Vote changes allowed (latest wins)<br/>Decryption in secure enclave</p>
            </div>
            <div className="p-3 bg-gray-50 border border-gray-200 rounded">
              <strong className="text-sm block mb-1">✅ Verifiability</strong>
              <p className="text-xs text-gray-700">Merkle proofs: Voters verify inclusion<br/>TEE attestation: Verify execution integrity</p>
            </div>
            <div className="p-3 bg-gray-50 border border-gray-200 rounded">
              <strong className="text-sm block mb-1">⚡ Scalability</strong>
              <p className="text-xs text-gray-700">Master secret in TEE → unlimited users<br/>No storage overhead for keys</p>
            </div>
          </div>

          <div className="mt-4 p-3 bg-yellow-50 border-l-4 border-yellow-400">
            <p className="text-sm text-gray-700 mb-2">
              <strong>⚠️ Production Considerations:</strong>
            </p>
            <ul className="text-sm text-gray-700 space-y-1 ml-4 list-disc">
              <li>Master secret must be highly secured (hardware wallet, multi-sig, etc.)</li>
              <li>TEE attestation currently uses MVP mode - upgrade to SGX/SEV for production</li>
              <li>Frontend must correctly compute vote hash (BigInt for u64 precision!)</li>
              <li>Vote hash saved by user is CRITICAL for later verification</li>
            </ul>
          </div>
        </div>

        {/* captcha-ark */}
        <div id="captcha-ark" className="border border-gray-200 rounded-lg p-6 hover:shadow-lg transition-shadow scroll-mt-4">
          <AnchorHeading
            id="captcha-ark"
            badges={
              <>
                <span className="ml-3 text-sm bg-purple-100 text-purple-800 px-3 py-1 rounded">WASI P2</span>
                <span className="ml-2 text-sm bg-green-100 text-green-800 px-3 py-1 rounded">Full Stack</span>
              </>
            }
          >
            captcha-ark
          </AnchorHeading>
          <a
            href="https://github.com/zavodil/captcha-ark"
            target="_blank"
            rel="noopener noreferrer"
            className="inline-flex items-center gap-2 px-4 py-2 mt-4 mb-4 bg-gray-900 text-white rounded-lg hover:bg-gray-800 transition-colors font-medium"
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

      <div className="mt-8 p-6 bg-gradient-to-r from-orange-50 to-green-50 border border-orange-200 rounded-lg">
        <h3 className="text-xl font-semibold mb-3 text-gray-900">🚀 What Makes OutLayer Special?</h3>
        <p className="text-gray-700 mb-4">
          OutLayer enables <strong>complex off-chain computation</strong> that would be impossible or prohibitively expensive on-chain. These examples demonstrate the unique capabilities:
        </p>

        <div className="grid grid-cols-1 md:grid-cols-2 gap-4 mb-6">
          <div className="p-4 bg-white rounded-lg border border-gray-200">
            <h4 className="font-semibold text-sm mb-2 text-[var(--primary-orange)]">💰 Cost Efficiency</h4>
            <ul className="text-xs text-gray-700 space-y-1">
              <li><strong>Private DAO:</strong> Heavy cryptography off-chain (~$0.001/vote)</li>
              <li><strong>Oracle:</strong> Multi-source aggregation without contract bloat</li>
              <li><strong>AI:</strong> LLM calls impossible on-chain, trivial with OutLayer</li>
            </ul>
          </div>

          <div className="p-4 bg-white rounded-lg border border-gray-200">
            <h4 className="font-semibold text-sm mb-2 text-[var(--primary-green)]">🔐 Privacy & Security</h4>
            <ul className="text-xs text-gray-700 space-y-1">
              <li><strong>Private DAO:</strong> Anonymous voting with merkle proofs</li>
              <li><strong>Encrypted Secrets:</strong> API keys decrypted only in TEE</li>
              <li><strong>Intents:</strong> Private key operations in secure enclave</li>
            </ul>
          </div>

          <div className="p-4 bg-white rounded-lg border border-gray-200">
            <h4 className="font-semibold text-sm mb-2 text-blue-600">⚡ Advanced Capabilities</h4>
            <ul className="text-xs text-gray-700 space-y-1">
              <li><strong>HTTP Requests:</strong> External APIs (OpenAI, CoinGecko, etc.)</li>
              <li><strong>Heavy Crypto:</strong> ECIES, HKDF, Merkle trees, secp256k1</li>
              <li><strong>Complex Logic:</strong> Multi-source validation & aggregation</li>
            </ul>
          </div>

          <div className="p-4 bg-white rounded-lg border border-gray-200">
            <h4 className="font-semibold text-sm mb-2 text-purple-600">🎯 Real-World Integration</h4>
            <ul className="text-xs text-gray-700 space-y-1">
              <li><strong>CAPTCHA:</strong> Human verification for token sales</li>
              <li><strong>DEX Swaps:</strong> Paused FT transfers with async execution</li>
              <li><strong>Full Stack:</strong> Frontend + Backend + Contract + WASI</li>
            </ul>
          </div>
        </div>

        <h3 className="text-xl font-semibold mb-3 mt-6 text-gray-900">📚 Ready to Build?</h3>
        <ul className="list-disc list-inside space-y-2 text-gray-700">
          <li>Read the <a href="https://github.com/fastnear/near-outlayer/blob/main/wasi-examples/WASI_TUTORIAL.md" target="_blank" rel="noopener noreferrer" className="text-[var(--primary-orange)] hover:underline font-semibold">complete WASI tutorial</a></li>
          <li>Clone examples: <code className="bg-white px-2 py-1 rounded border border-gray-300">git clone https://github.com/fastnear/near-outlayer.git</code></li>
          <li><strong>Beginners:</strong> Start with <strong>random-ark</strong> or <strong>echo-ark</strong> for WASI basics</li>
          <li><strong>Quick Test:</strong> Try <strong>weather-ark</strong> with pre-configured secrets on testnet!</li>
          <li><strong>HTTPS Apps:</strong> Use <strong>ai-ark</strong> or <strong>oracle-ark</strong> for external API integration</li>
          <li><strong>Advanced Crypto:</strong> Study <strong>private-dao-ark</strong> for privacy & verifiability patterns</li>
          <li><strong>Full Stack:</strong> Deploy <strong>captcha-ark</strong> for complete production example</li>
          <li><strong>DeFi Integration:</strong> Explore <strong>intents-ark</strong> for paused transactions & swaps</li>
        </ul>
      </div>
    </div>
  );
}
