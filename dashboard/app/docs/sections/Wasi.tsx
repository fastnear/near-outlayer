'use client';

import Link from 'next/link';
import { AnchorHeading, useHashNavigation } from './utils';

export default function WasiSection() {
  useHashNavigation();

  return (
    <div className="prose max-w-none">
      <h2 className="text-3xl font-bold mb-6 text-[var(--primary-orange)]">Building OutLayer App</h2>

      <div className="space-y-6">
        <section id="what-is-wasi">
          <AnchorHeading id="what-is-wasi">What is WASI?</AnchorHeading>
          <p className="text-gray-700 mb-3">
            <strong>WASI</strong> (WebAssembly System Interface) is a standardized API that allows WebAssembly modules to interact with the outside world - read files, access environment variables, make network requests, and generate random numbers.
          </p>
          <p className="text-gray-700">
            Think of WASI as a &quot;syscall interface for WebAssembly&quot; - it provides the basic building blocks your WASM code needs to do real work, like reading input data or calling external APIs, while maintaining security through sandboxing.
          </p>
        </section>

        <section id="supported-languages">
          <AnchorHeading id="supported-languages">Supported Languages</AnchorHeading>
          <p className="text-gray-700">
            Any language that compiles to WASM with WASI support: Rust, C/C++, Go, AssemblyScript, and more.
            Rust is recommended for best tooling and ecosystem support.
          </p>
        </section>

        <section id="wasi-preview">
          <AnchorHeading id="wasi-preview">WASI Preview 1 vs Preview 2</AnchorHeading>
          <p className="text-gray-700 mb-3">
            OutLayer supports both WASI P1 and P2 standards. Choose based on your requirements:
          </p>
          <div className="bg-gray-50 p-4 rounded-lg space-y-3">
            <div>
              <strong className="text-gray-900">WASI Preview 1 (P1)</strong>
              <ul className="list-disc list-inside text-gray-700 ml-4 mt-1">
                <li>Target: <code className="bg-gray-100 px-2 py-1 rounded">wasm32-wasip1</code></li>
                <li>Use for: Simple computations, random numbers, basic I/O</li>
                <li>Binary size: Smaller (~100-200KB)</li>
                <li>Compilation: Faster</li>
                <li>Stability: Mature and stable</li>
              </ul>
            </div>
            <div>
              <strong className="text-gray-900">WASI Preview 2 (P2)</strong>
              <ul className="list-disc list-inside text-gray-700 ml-4 mt-1">
                <li>Target: <code className="bg-gray-100 px-2 py-1 rounded">wasm32-wasip2</code></li>
                <li>Use for: HTTP requests, complex I/O, modern features</li>
                <li>Binary size: Larger (~500KB-1MB)</li>
                <li>Features: HTTP client, advanced filesystem, sockets</li>
                <li>Requires: wasmtime 28+</li>
              </ul>
            </div>
          </div>
          <p className="text-gray-700 mt-3">
            <strong>Rule of thumb:</strong> Use P1 unless you need HTTP or advanced I/O.
          </p>
        </section>

        <section id="wasi-interface">
          <AnchorHeading id="wasi-interface">WASI Interface</AnchorHeading>
          <p className="text-gray-700">
            OutLayer provides a minimal WASI environment with support for:
          </p>
          <ul className="list-disc list-inside space-y-2 text-gray-700 mt-2">
            <li><strong>stdin/stdout:</strong> Read JSON input data, write JSON output results</li>
            <li><strong>Environment variables:</strong> Access encrypted secrets via <code className="bg-gray-100 px-2 py-1 rounded">std::env::var()</code></li>
            <li><strong>Random numbers:</strong> Cryptographically secure random generation (WASI P1 & P2)</li>
            <li><strong>HTTP requests:</strong> Make external API calls (WASI P2 only, via <code className="bg-gray-100 px-2 py-1 rounded">wasi-http-client</code>)</li>
            <li><strong>Persistent storage:</strong> Encrypted key-value storage (WASI P2 only, via <code className="bg-gray-100 px-2 py-1 rounded">outlayer</code> SDK from <a href="https://crates.io/crates/outlayer" target="_blank" rel="noopener noreferrer" className="text-[var(--primary-orange)] hover:underline">crates.io</a>)</li>
            <li><strong>File I/O (limited):</strong> Basic file operations in sandboxed environment</li>
            <li><strong>NEAR context:</strong> Access execution metadata via env vars (<code className="bg-gray-100 px-2 py-1 rounded">NEAR_SENDER_ID</code>, <code className="bg-gray-100 px-2 py-1 rounded">NEAR_BLOCK_HEIGHT</code>, etc.)</li>
          </ul>
        </section>

        <section id="host-functions">
          <AnchorHeading id="host-functions">Host Functions (Advanced)</AnchorHeading>
          <p className="text-gray-700 mb-3">
            OutLayer provides advanced host functions for direct NEAR RPC access from WASM. These functions enable your code to interact with the NEAR blockchain without relying on external HTTP APIs.
          </p>

          <div className="bg-blue-50 border-l-4 border-blue-400 p-4 mb-4">
            <h4 className="font-semibold text-blue-900 mb-2">What are Host Functions?</h4>
            <p className="text-sm text-blue-800">
              Host functions are native functions provided by the worker runtime that WASM code can call directly. They bypass HTTP and give your code privileged access to private NEAR RPC endpoints (powered by Fastnear), enabling operations like sending transactions and querying blockchain state.
            </p>
          </div>

          <h4 className="font-semibold text-gray-900 mb-2">Available Functions</h4>
          <div className="space-y-3 mb-4">
            <div className="border-l-4 border-orange-400 pl-3">
              <p className="font-mono text-sm text-gray-800 mb-1">
                <strong>call()</strong> - Execute NEAR contract call
              </p>
              <p className="text-sm text-gray-700 mb-2">
                Send function calls to NEAR contracts with attached deposit and gas. Your WASM provides the signer credentials via secrets.
              </p>
              <p className="text-xs text-gray-600 font-mono bg-gray-50 p-2 rounded">
                call(signer_id, signer_key, receiver_id, method_name, args_json, deposit_yocto, gas) → (tx_hash, status)
              </p>
            </div>

            <div className="border-l-4 border-orange-400 pl-3">
              <p className="font-mono text-sm text-gray-800 mb-1">
                <strong>transfer()</strong> - Send NEAR tokens
              </p>
              <p className="text-sm text-gray-700 mb-2">
                Transfer NEAR tokens from one account to another.
              </p>
              <p className="text-xs text-gray-600 font-mono bg-gray-50 p-2 rounded">
                transfer(signer_id, signer_key, receiver_id, amount_yocto) → (tx_hash, status)
              </p>
            </div>

            <div className="border-l-4 border-gray-400 pl-3">
              <p className="font-mono text-sm text-gray-800 mb-1">
                <strong>view()</strong> - Query contract state
              </p>
              <p className="text-sm text-gray-700 mb-2">
                Read-only view calls to query contract state without sending transactions.
              </p>
              <p className="text-xs text-gray-600 font-mono bg-gray-50 p-2 rounded">
                view(contract_id, method_name, args_json) → (result, status)
              </p>
            </div>
          </div>

          <h4 className="font-semibold text-gray-900 mb-2">Key Security Features</h4>
          <ul className="list-disc list-inside text-gray-700 space-y-1 mb-4 text-sm">
            <li><strong>WASM provides signer:</strong> Your code passes <code className="bg-gray-100 px-1 rounded">signer_key</code> from secrets - worker never uses its own keys</li>
            <li><strong>Private RPC access:</strong> Fastnear-powered endpoints with higher rate limits and reliability</li>
            <li><strong>Transaction tracking:</strong> All transactions are logged and can be verified on-chain</li>
            <li><strong>TEE isolation:</strong> Signing keys remain inside TEE and never leave the secure enclave</li>
          </ul>

          <div className="bg-green-50 border-l-4 border-green-400 p-4 mb-4">
            <h4 className="font-semibold text-green-900 mb-2">Example: botfather-ark</h4>
            <p className="text-sm text-green-800 mb-2">
              The <Link href="/docs/examples#botfather-ark" className="text-[var(--primary-orange)] hover:underline font-semibold">botfather-ark</Link> example demonstrates host functions in action:
            </p>
            <ul className="list-disc list-inside text-sm text-green-800 ml-4 space-y-1">
              <li>Creates multiple NEAR accounts programmatically using <code className="bg-green-100 px-1 rounded">call()</code></li>
              <li>Distributes NEAR tokens across accounts using <code className="bg-green-100 px-1 rounded">transfer()</code></li>
              <li>Executes batch contract calls (e.g., token purchases, staking delegation)</li>
              <li>Queries account balances via <code className="bg-green-100 px-1 rounded">view()</code></li>
            </ul>
          </div>

          <h4 className="font-semibold text-gray-900 mb-2">WIT Interface Definition</h4>
          <p className="text-sm text-gray-700 mb-2">
            Host functions are defined in <code className="bg-gray-100 px-1 rounded">worker/wit/world.wit</code>:
          </p>
          <pre className="bg-gray-900 text-gray-100 p-3 rounded-lg overflow-x-auto text-xs mb-3">
{`package near:rpc@0.1.0;

interface api {
    view: func(
        contract-id: string,
        method-name: string,
        args-json: string
    ) -> tuple<string, string>;

    call: func(
        signer-id: string,
        signer-key: string,
        receiver-id: string,
        method-name: string,
        args-json: string,
        deposit-yocto: string,
        gas: string
    ) -> tuple<string, string>;

    transfer: func(
        signer-id: string,
        signer-key: string,
        receiver-id: string,
        amount-yocto: string
    ) -> tuple<string, string>;
}

world rpc-host {
    import api;
}`}
          </pre>

          <div className="bg-yellow-50 border-l-4 border-yellow-400 p-4">
            <h4 className="font-semibold text-yellow-900 mb-2">Requirements</h4>
            <ul className="list-disc list-inside text-sm text-yellow-800 space-y-1">
              <li><strong>WASI Preview 2:</strong> Host functions require <code className="bg-yellow-100 px-1 rounded">wasm32-wasip2</code> target</li>
              <li><strong>Signer credentials:</strong> Must provide <code className="bg-yellow-100 px-1 rounded">NEAR_SENDER_PRIVATE_KEY</code> via secrets</li>
              <li><strong>NEAR tokens:</strong> Signer account must have sufficient balance for gas and deposits</li>
            </ul>
          </div>

          <div className="bg-blue-50 border-l-4 border-blue-400 p-4 mt-4">
            <h4 className="font-semibold text-blue-900 mb-2">API Versioning</h4>
            <p className="text-sm text-blue-800 mb-2">
              Host functions are versioned using semantic versioning (<code className="bg-blue-100 px-1 rounded">@0.1.0</code>). This ensures backward compatibility when the API evolves.
            </p>
            <ul className="list-disc list-inside text-sm text-blue-800 space-y-1">
              <li><strong>Current version:</strong> <code className="bg-blue-100 px-1 rounded">near:rpc@0.1.0</code></li>
              <li><strong>Multiple versions:</strong> Workers can run WASM compiled with different API versions simultaneously</li>
              <li><strong>No breaking changes:</strong> Your WASM will continue working even when new versions are released</li>
            </ul>
          </div>
        </section>

        <section id="critical-requirements">
          <AnchorHeading id="critical-requirements">Critical Requirements</AnchorHeading>
          <div className="bg-yellow-50 border-l-4 border-yellow-400 p-4">
            <ul className="list-disc list-inside space-y-2 text-gray-700">
              <li><strong>Binary format:</strong> Must use <code className="bg-gray-100 px-2 py-1 rounded">[[bin]]</code> in Cargo.toml, NOT <code className="bg-gray-100 px-2 py-1 rounded">[lib]</code></li>
              <li><strong>Entry point:</strong> Must have <code className="bg-gray-100 px-2 py-1 rounded">fn main()</code> function</li>
              <li><strong>Input:</strong> Always read from stdin (not command-line arguments)</li>
              <li><strong>Output:</strong> Always write to stdout (not stderr)</li>
              <li><strong>Format:</strong> JSON only (UTF-8 encoded)</li>
              <li><strong>Size limit:</strong> Output must be ≤900 bytes (NEAR Protocol limit)</li>
              <li><strong>Flush:</strong> Call <code className="bg-gray-100 px-2 py-1 rounded">stdout().flush()</code> after writing</li>
            </ul>
          </div>
        </section>

        <section id="working-examples">
          <AnchorHeading id="working-examples">Working Examples</AnchorHeading>
          <p className="text-gray-700 mb-4">
            We provide 9 complete, open-source examples demonstrating different WASI patterns:
          </p>

          <div className="grid grid-cols-1 md:grid-cols-2 gap-4 mb-6">
            <div className="border border-gray-200 rounded-lg p-4 hover:shadow-md transition-shadow">
              <h4 className="font-semibold mb-1">
                <Link href="/docs/examples#random-ark" className="text-[var(--primary-orange)] hover:underline">
                  random-ark
                </Link>
                <a href="https://github.com/zavodil/random-ark" target="_blank" rel="noopener noreferrer" className="ml-2 text-gray-500 hover:text-gray-700" title="View source on GitHub">
                  <svg className="inline-block w-4 h-4" fill="currentColor" viewBox="0 0 24 24" xmlns="http://www.w3.org/2000/svg">
                    <path d="M12 0C5.37 0 0 5.37 0 12c0 5.31 3.435 9.795 8.205 11.385.6.105.825-.255.825-.57 0-.285-.015-1.23-.015-2.235-3.015.555-3.795-.735-4.035-1.41-.135-.345-.72-1.41-1.23-1.695-.42-.225-1.02-.78-.015-.795.945-.015 1.62.87 1.845 1.23 1.08 1.815 2.805 1.305 3.495.99.105-.78.42-1.305.765-1.605-2.67-.3-5.46-1.335-5.46-5.925 0-1.305.465-2.385 1.23-3.225-.12-.3-.54-1.53.12-3.18 0 0 1.005-.315 3.3 1.23.96-.27 1.98-.405 3-.405s2.04.135 3 .405c2.295-1.56 3.3-1.23 3.3-1.23.66 1.65.24 2.88.12 3.18.765.84 1.23 1.905 1.23 3.225 0 4.605-2.805 5.625-5.475 5.925.435.375.81 1.095.81 2.22 0 1.605-.015 2.895-.015 3.3 0 .315.225.69.825.57A12.02 12.02 0 0024 12c0-6.63-5.37-12-12-12z"/>
                  </svg>
                </a>
                <span className="ml-2 text-xs bg-blue-100 text-blue-800 px-2 py-1 rounded">P1</span>
              </h4>
              <p className="text-sm text-gray-600">Random number generation (starter example)</p>
            </div>

            <div className="border border-gray-200 rounded-lg p-4 hover:shadow-md transition-shadow">
              <h4 className="font-semibold mb-1">
                <Link href="/docs/examples#echo-ark" className="text-[var(--primary-orange)] hover:underline">
                  echo-ark
                </Link>
                <a href="https://github.com/zavodil/echo-ark" target="_blank" rel="noopener noreferrer" className="ml-2 text-gray-500 hover:text-gray-700" title="View source on GitHub">
                  <svg className="inline-block w-4 h-4" fill="currentColor" viewBox="0 0 24 24" xmlns="http://www.w3.org/2000/svg">
                    <path d="M12 0C5.37 0 0 5.37 0 12c0 5.31 3.435 9.795 8.205 11.385.6.105.825-.255.825-.57 0-.285-.015-1.23-.015-2.235-3.015.555-3.795-.735-4.035-1.41-.135-.345-.72-1.41-1.23-1.695-.42-.225-1.02-.78-.015-.795.945-.015 1.62.87 1.845 1.23 1.08 1.815 2.805 1.305 3.495.99.105-.78.42-1.305.765-1.605-2.67-.3-5.46-1.335-5.46-5.925 0-1.305.465-2.385 1.23-3.225-.12-.3-.54-1.53.12-3.18 0 0 1.005-.315 3.3 1.23.96-.27 1.98-.405 3-.405s2.04.135 3 .405c2.295-1.56 3.3-1.23 3.3-1.23.66 1.65.24 2.88.12 3.18.765.84 1.23 1.905 1.23 3.225 0 4.605-2.805 5.625-5.475 5.925.435.375.81 1.095.81 2.22 0 1.605-.015 2.895-.015 3.3 0 .315.225.69.825.57A12.02 12.02 0 0024 12c0-6.63-5.37-12-12-12z"/>
                  </svg>
                </a>
                <span className="ml-2 text-xs bg-blue-100 text-blue-800 px-2 py-1 rounded">P1</span>
              </h4>
              <p className="text-sm text-gray-600">NEAR context & environment variables</p>
            </div>

            <div className="border border-gray-200 rounded-lg p-4 hover:shadow-md transition-shadow">
              <h4 className="font-semibold mb-1">
                <Link href="/docs/examples#ai-ark" className="text-[var(--primary-orange)] hover:underline">
                  ai-ark
                </Link>
                <a href="https://github.com/zavodil/ai-ark" target="_blank" rel="noopener noreferrer" className="ml-2 text-gray-500 hover:text-gray-700" title="View source on GitHub">
                  <svg className="inline-block w-4 h-4" fill="currentColor" viewBox="0 0 24 24" xmlns="http://www.w3.org/2000/svg">
                    <path d="M12 0C5.37 0 0 5.37 0 12c0 5.31 3.435 9.795 8.205 11.385.6.105.825-.255.825-.57 0-.285-.015-1.23-.015-2.235-3.015.555-3.795-.735-4.035-1.41-.135-.345-.72-1.41-1.23-1.695-.42-.225-1.02-.78-.015-.795.945-.015 1.62.87 1.845 1.23 1.08 1.815 2.805 1.305 3.495.99.105-.78.42-1.305.765-1.605-2.67-.3-5.46-1.335-5.46-5.925 0-1.305.465-2.385 1.23-3.225-.12-.3-.54-1.53.12-3.18 0 0 1.005-.315 3.3 1.23.96-.27 1.98-.405 3-.405s2.04.135 3 .405c2.295-1.56 3.3-1.23 3.3-1.23.66 1.65.24 2.88.12 3.18.765.84 1.23 1.905 1.23 3.225 0 4.605-2.805 5.625-5.475 5.925.435.375.81 1.095.81 2.22 0 1.605-.015 2.895-.015 3.3 0 .315.225.69.825.57A12.02 12.02 0 0024 12c0-6.63-5.37-12-12-12z"/>
                  </svg>
                </a>
                <span className="ml-2 text-xs bg-purple-100 text-purple-800 px-2 py-1 rounded">P2</span>
              </h4>
              <p className="text-sm text-gray-600">OpenAI API integration (HTTPS requests)</p>
            </div>

            <div className="border border-gray-200 rounded-lg p-4 hover:shadow-md transition-shadow">
              <h4 className="font-semibold mb-1">
                <Link href="/docs/examples#weather-ark" className="text-[var(--primary-orange)] hover:underline">
                  weather-ark
                </Link>
                <a href="https://github.com/zavodil/weather-ark" target="_blank" rel="noopener noreferrer" className="ml-2 text-gray-500 hover:text-gray-700" title="View source on GitHub">
                  <svg className="inline-block w-4 h-4" fill="currentColor" viewBox="0 0 24 24" xmlns="http://www.w3.org/2000/svg">
                    <path d="M12 0C5.37 0 0 5.37 0 12c0 5.31 3.435 9.795 8.205 11.385.6.105.825-.255.825-.57 0-.285-.015-1.23-.015-2.235-3.015.555-3.795-.735-4.035-1.41-.135-.345-.72-1.41-1.23-1.695-.42-.225-1.02-.78-.015-.795.945-.015 1.62.87 1.845 1.23 1.08 1.815 2.805 1.305 3.495.99.105-.78.42-1.305.765-1.605-2.67-.3-5.46-1.335-5.46-5.925 0-1.305.465-2.385 1.23-3.225-.12-.3-.54-1.53.12-3.18 0 0 1.005-.315 3.3 1.23.96-.27 1.98-.405 3-.405s2.04.135 3 .405c2.295-1.56 3.3-1.23 3.3-1.23.66 1.65.24 2.88.12 3.18.765.84 1.23 1.905 1.23 3.225 0 4.605-2.805 5.625-5.475 5.925.435.375.81 1.095.81 2.22 0 1.605-.015 2.895-.015 3.3 0 .315.225.69.825.57A12.02 12.02 0 0024 12c0-6.63-5.37-12-12-12z"/>
                  </svg>
                </a>
                <span className="ml-2 text-xs bg-purple-100 text-purple-800 px-2 py-1 rounded">P2</span>
              </h4>
              <p className="text-sm text-gray-600">OpenWeather API oracle (specialized price oracle)</p>
            </div>

            <div className="border border-gray-200 rounded-lg p-4 hover:shadow-md transition-shadow">
              <h4 className="font-semibold mb-1">
                <Link href="/docs/examples#oracle-ark" className="text-[var(--primary-orange)] hover:underline">
                  oracle-ark
                </Link>
                <a href="https://github.com/zavodil/oracle-ark" target="_blank" rel="noopener noreferrer" className="ml-2 text-gray-500 hover:text-gray-700" title="View source on GitHub">
                  <svg className="inline-block w-4 h-4" fill="currentColor" viewBox="0 0 24 24" xmlns="http://www.w3.org/2000/svg">
                    <path d="M12 0C5.37 0 0 5.37 0 12c0 5.31 3.435 9.795 8.205 11.385.6.105.825-.255.825-.57 0-.285-.015-1.23-.015-2.235-3.015.555-3.795-.735-4.035-1.41-.135-.345-.72-1.41-1.23-1.695-.42-.225-1.02-.78-.015-.795.945-.015 1.62.87 1.845 1.23 1.08 1.815 2.805 1.305 3.495.99.105-.78.42-1.305.765-1.605-2.67-.3-5.46-1.335-5.46-5.925 0-1.305.465-2.385 1.23-3.225-.12-.3-.54-1.53.12-3.18 0 0 1.005-.315 3.3 1.23.96-.27 1.98-.405 3-.405s2.04.135 3 .405c2.295-1.56 3.3-1.23 3.3-1.23.66 1.65.24 2.88.12 3.18.765.84 1.23 1.905 1.23 3.225 0 4.605-2.805 5.625-5.475 5.925.435.375.81 1.095.81 2.22 0 1.605-.015 2.895-.015 3.3 0 .315.225.69.825.57A12.02 12.02 0 0024 12c0-6.63-5.37-12-12-12z"/>
                  </svg>
                </a>
                <span className="ml-2 text-xs bg-purple-100 text-purple-800 px-2 py-1 rounded">P2</span>
              </h4>
              <p className="text-sm text-gray-600">Multi-source price oracle with aggregation</p>
            </div>

            <div className="border border-gray-200 rounded-lg p-4 hover:shadow-md transition-shadow">
              <h4 className="font-semibold mb-1">
                <Link href="/docs/examples#ethereum-api" className="text-[var(--primary-orange)] hover:underline">
                  ethereum-api
                </Link>
                <a href="https://github.com/zavodil/ethereum-api" target="_blank" rel="noopener noreferrer" className="ml-2 text-gray-500 hover:text-gray-700" title="View source on GitHub">
                  <svg className="inline-block w-4 h-4" fill="currentColor" viewBox="0 0 24 24" xmlns="http://www.w3.org/2000/svg">
                    <path d="M12 0C5.37 0 0 5.37 0 12c0 5.31 3.435 9.795 8.205 11.385.6.105.825-.255.825-.57 0-.285-.015-1.23-.015-2.235-3.015.555-3.795-.735-4.035-1.41-.135-.345-.72-1.41-1.23-1.695-.42-.225-1.02-.78-.015-.795.945-.015 1.62.87 1.845 1.23 1.08 1.815 2.805 1.305 3.495.99.105-.78.42-1.305.765-1.605-2.67-.3-5.46-1.335-5.46-5.925 0-1.305.465-2.385 1.23-3.225-.12-.3-.54-1.53.12-3.18 0 0 1.005-.315 3.3 1.23.96-.27 1.98-.405 3-.405s2.04.135 3 .405c2.295-1.56 3.3-1.23 3.3-1.23.66 1.65.24 2.88.12 3.18.765.84 1.23 1.905 1.23 3.225 0 4.605-2.805 5.625-5.475 5.925.435.375.81 1.095.81 2.22 0 1.605-.015 2.895-.015 3.3 0 .315.225.69.825.57A12.02 12.02 0 0024 12c0-6.63-5.37-12-12-12z"/>
                  </svg>
                </a>
                <span className="ml-2 text-xs bg-purple-100 text-purple-800 px-2 py-1 rounded">P2</span>
              </h4>
              <p className="text-sm text-gray-600">Ethereum blockchain data access via RPC</p>
            </div>

            <div className="border border-gray-200 rounded-lg p-4 hover:shadow-md transition-shadow">
              <h4 className="font-semibold mb-1">
                <Link href="/docs/examples#botfather-ark" className="text-[var(--primary-orange)] hover:underline">
                  botfather-ark
                </Link>
                <a href="https://github.com/zavodil/botfather-ark" target="_blank" rel="noopener noreferrer" className="ml-2 text-gray-500 hover:text-gray-700" title="View source on GitHub">
                  <svg className="inline-block w-4 h-4" fill="currentColor" viewBox="0 0 24 24" xmlns="http://www.w3.org/2000/svg">
                    <path d="M12 0C5.37 0 0 5.37 0 12c0 5.31 3.435 9.795 8.205 11.385.6.105.825-.255.825-.57 0-.285-.015-1.23-.015-2.235-3.015.555-3.795-.735-4.035-1.41-.135-.345-.72-1.41-1.23-1.695-.42-.225-1.02-.78-.015-.795.945-.015 1.62.87 1.845 1.23 1.08 1.815 2.805 1.305 3.495.99.105-.78.42-1.305.765-1.605-2.67-.3-5.46-1.335-5.46-5.925 0-1.305.465-2.385 1.23-3.225-.12-.3-.54-1.53.12-3.18 0 0 1.005-.315 3.3 1.23.96-.27 1.98-.405 3-.405s2.04.135 3 .405c2.295-1.56 3.3-1.23 3.3-1.23.66 1.65.24 2.88.12 3.18.765.84 1.23 1.905 1.23 3.225 0 4.605-2.805 5.625-5.475 5.925.435.375.81 1.095.81 2.22 0 1.605-.015 2.895-.015 3.3 0 .315.225.69.825.57A12.02 12.02 0 0024 12c0-6.63-5.37-12-12-12z"/>
                  </svg>
                </a>
                <span className="ml-2 text-xs bg-purple-100 text-purple-800 px-2 py-1 rounded">P2</span>
                <span className="ml-2 text-xs bg-orange-100 text-orange-800 px-2 py-1 rounded">Host Functions</span>
              </h4>
              <p className="text-sm text-gray-600">Account factory with AI names & batch operations. Demonstrates <code className="bg-gray-100 px-1 rounded text-xs">call()</code> and <code className="bg-gray-100 px-1 rounded text-xs">transfer()</code> host functions for NEAR RPC access.</p>
            </div>

            <div className="border border-gray-200 rounded-lg p-4 hover:shadow-md transition-shadow">
              <h4 className="font-semibold mb-1">
                <Link href="/docs/examples#intents-ark" className="text-[var(--primary-orange)] hover:underline">
                  intents-ark
                </Link>
                <a href="https://github.com/zavodil/intents-ark" target="_blank" rel="noopener noreferrer" className="ml-2 text-gray-500 hover:text-gray-700" title="View source on GitHub">
                  <svg className="inline-block w-4 h-4" fill="currentColor" viewBox="0 0 24 24" xmlns="http://www.w3.org/2000/svg">
                    <path d="M12 0C5.37 0 0 5.37 0 12c0 5.31 3.435 9.795 8.205 11.385.6.105.825-.255.825-.57 0-.285-.015-1.23-.015-2.235-3.015.555-3.795-.735-4.035-1.41-.135-.345-.72-1.41-1.23-1.695-.42-.225-1.02-.78-.015-.795.945-.015 1.62.87 1.845 1.23 1.08 1.815 2.805 1.305 3.495.99.105-.78.42-1.305.765-1.605-2.67-.3-5.46-1.335-5.46-5.925 0-1.305.465-2.385 1.23-3.225-.12-.3-.54-1.53.12-3.18 0 0 1.005-.315 3.3 1.23.96-.27 1.98-.405 3-.405s2.04.135 3 .405c2.295-1.56 3.3-1.23 3.3-1.23.66 1.65.24 2.88.12 3.18.765.84 1.23 1.905 1.23 3.225 0 4.605-2.805 5.625-5.475 5.925.435.375.81 1.095.81 2.22 0 1.605-.015 2.895-.015 3.3 0 .315.225.69.825.57A12.02 12.02 0 0024 12c0-6.63-5.37-12-12-12z"/>
                  </svg>
                </a>
                <span className="ml-2 text-xs bg-purple-100 text-purple-800 px-2 py-1 rounded">P2</span>
              </h4>
              <p className="text-sm text-gray-600">DEX swaps via NEAR Intents (paused FT transfer)</p>
            </div>

            <div className="border border-gray-200 rounded-lg p-4 hover:shadow-md transition-shadow">
              <h4 className="font-semibold mb-1">
                <Link href="/docs/examples#private-dao-ark" className="text-[var(--primary-orange)] hover:underline">
                  private-dao-ark
                </Link>
                <a href="https://github.com/zavodil/private-dao-ark" target="_blank" rel="noopener noreferrer" className="ml-2 text-gray-500 hover:text-gray-700" title="View source on GitHub">
                  <svg className="inline-block w-4 h-4" fill="currentColor" viewBox="0 0 24 24" xmlns="http://www.w3.org/2000/svg">
                    <path d="M12 0C5.37 0 0 5.37 0 12c0 5.31 3.435 9.795 8.205 11.385.6.105.825-.255.825-.57 0-.285-.015-1.23-.015-2.235-3.015.555-3.795-.735-4.035-1.41-.135-.345-.72-1.41-1.23-1.695-.42-.225-1.02-.78-.015-.795.945-.015 1.62.87 1.845 1.23 1.08 1.815 2.805 1.305 3.495.99.105-.78.42-1.305.765-1.605-2.67-.3-5.46-1.335-5.46-5.925 0-1.305.465-2.385 1.23-3.225-.12-.3-.54-1.53.12-3.18 0 0 1.005-.315 3.3 1.23.96-.27 1.98-.405 3-.405s2.04.135 3 .405c2.295-1.56 3.3-1.23 3.3-1.23.66 1.65.24 2.88.12 3.18.765.84 1.23 1.905 1.23 3.225 0 4.605-2.805 5.625-5.475 5.925.435.375.81 1.095.81 2.22 0 1.605-.015 2.895-.015 3.3 0 .315.225.69.825.57A12.02 12.02 0 0024 12c0-6.63-5.37-12-12-12z"/>
                  </svg>
                </a>
                <span className="ml-2 text-xs bg-blue-100 text-blue-800 px-2 py-1 rounded">P1</span>
                <span className="ml-2 text-xs bg-red-100 text-red-800 px-2 py-1 rounded">Advanced</span>
              </h4>
              <p className="text-sm text-gray-600">Anonymous DAO voting with cryptographic privacy (ECIES, HKDF, Merkle trees)</p>
            </div>

            <div className="border border-gray-200 rounded-lg p-4 hover:shadow-md transition-shadow">
              <h4 className="font-semibold mb-1">
                <Link href="/docs/examples#captcha-ark" className="text-[var(--primary-orange)] hover:underline">
                  captcha-ark
                </Link>
                <a href="https://github.com/zavodil/captcha-ark" target="_blank" rel="noopener noreferrer" className="ml-2 text-gray-500 hover:text-gray-700" title="View source on GitHub">
                  <svg className="inline-block w-4 h-4" fill="currentColor" viewBox="0 0 24 24" xmlns="http://www.w3.org/2000/svg">
                    <path d="M12 0C5.37 0 0 5.37 0 12c0 5.31 3.435 9.795 8.205 11.385.6.105.825-.255.825-.57 0-.285-.015-1.23-.015-2.235-3.015.555-3.795-.735-4.035-1.41-.135-.345-.72-1.41-1.23-1.695-.42-.225-1.02-.78-.015-.795.945-.015 1.62.87 1.845 1.23 1.08 1.815 2.805 1.305 3.495.99.105-.78.42-1.305.765-1.605-2.67-.3-5.46-1.335-5.46-5.925 0-1.305.465-2.385 1.23-3.225-.12-.3-.54-1.53.12-3.18 0 0 1.005-.315 3.3 1.23.96-.27 1.98-.405 3-.405s2.04.135 3 .405c2.295-1.56 3.3-1.23 3.3-1.23.66 1.65.24 2.88.12 3.18.765.84 1.23 1.905 1.23 3.225 0 4.605-2.805 5.625-5.475 5.925.435.375.81 1.095.81 2.22 0 1.605-.015 2.895-.015 3.3 0 .315.225.69.825.57A12.02 12.02 0 0024 12c0-6.63-5.37-12-12-12z"/>
                  </svg>
                </a>
                <span className="ml-2 text-xs bg-purple-100 text-purple-800 px-2 py-1 rounded">P2</span>
                <span className="ml-2 text-xs bg-green-100 text-green-800 px-2 py-1 rounded">Full Stack</span>
              </h4>
              <p className="text-sm text-gray-600">Token launchpad with CAPTCHA verification</p>
            </div>
          </div>

          <div className="p-5 bg-gradient-to-r from-blue-50 to-indigo-50 border border-blue-200 rounded-lg">
            <p className="text-gray-800 font-medium mb-2">
              Detailed Documentation Available
            </p>
            <p className="text-gray-700 mb-3">
              Each example includes complete source code, input/output examples, build instructions, and deployment guides.
            </p>
            <Link href="/docs/examples" className="inline-block px-4 py-2 bg-[var(--primary-orange)] text-white font-semibold rounded-lg hover:opacity-90 transition-opacity">
              View All Examples →
            </Link>
          </div>
        </section>

        <section id="resource-considerations">
          <AnchorHeading id="resource-considerations">Resource Considerations</AnchorHeading>
          <p className="text-gray-700">
            Be mindful of resource limits: instruction counts, memory usage, and execution time. Optimize your code
            to stay within requested limits to avoid failures and minimize costs.
          </p>
          <ul className="list-disc list-inside space-y-2 text-gray-700 mt-2">
            <li><strong>Max Instructions:</strong> 100 billion per execution (fuel metering enforced)</li>
            <li><strong>Max Execution Time:</strong> 60 seconds per execution</li>
            <li><strong>Max Memory:</strong> Configurable up to platform limits</li>
            <li><strong>Output Size:</strong> Must be ≤900 bytes (NEAR limit)</li>
          </ul>
        </section>

        <section id="testing-locally">
          <AnchorHeading id="testing-locally">Testing Locally</AnchorHeading>

          <h4 className="text-lg font-semibold mb-2 mt-4">Option 1: Test Compiler Script (Quick Compilation Test)</h4>
          <p className="text-gray-700 mb-3">
            Use <strong>test_compiler.sh</strong> to test compilation of your GitHub repository locally without running the full worker infrastructure.
            This script uses the same Docker image (<code className="bg-gray-100 px-1 rounded">zavodil/wasmedge-compiler:latest</code> with Rust 1.85) and compilation logic as the production worker.
          </p>

          <div className="bg-green-50 border-l-4 border-green-400 p-4 mb-3">
            <p className="text-sm text-green-800 mb-2">
              <strong>Perfect for:</strong> Testing if your repository compiles correctly before deploying to OutLayer.
            </p>
          </div>

          <pre className="bg-gray-900 text-gray-100 p-4 rounded-lg overflow-x-auto text-sm mb-3">
{`# Test compilation for WASI Preview 1
./scripts/test_compiler.sh \\
  https://github.com/zavodil/random-ark main wasm32-wasip1

# Test compilation for WASI Preview 2
./scripts/test_compiler.sh \\
  https://github.com/zavodil/ai-ark main wasm32-wasip2

# Custom output file
./scripts/test_compiler.sh \\
  https://github.com/user/myproject main wasm32-wasip1 myapp.wasm

# The script will:
# 1. Pull zavodil/wasmedge-compiler:latest Docker image (instant if already up to date)
# 2. Clone your repository and checkout the commit
# 3. Run cargo build with the exact same flags as worker
# 4. Optimize WASM (wasm-opt for P1, wasm-tools for P2)
# 5. Output compiled WASM with SHA256 checksum`}
          </pre>

          <p className="text-gray-700 mb-3">
            <strong>Key features:</strong>
          </p>
          <ul className="list-disc list-inside text-gray-700 ml-4 space-y-1 mb-3">
            <li>Uses official Docker image from Docker Hub (no local builds needed)</li>
            <li>Exactly mirrors worker compiler behavior</li>
            <li>Supports all three targets: <code className="bg-gray-100 px-1 rounded">wasm32-wasip1</code>, <code className="bg-gray-100 px-1 rounded">wasm32-wasip2</code>, <code className="bg-gray-100 px-1 rounded">wasm32-wasi</code></li>
            <li>Shows compilation time, file size, and SHA256 checksum</li>
            <li>Configurable memory/CPU limits via environment variables</li>
          </ul>

          <div className="bg-yellow-50 border-l-4 border-yellow-400 p-3 mb-4">
            <p className="text-sm text-yellow-800 mb-2">
              <strong>Troubleshooting:</strong> If you get <code className="bg-yellow-100 px-1 rounded">cabi_realloc</code> error with wasm32-wasip2,
              it means the project is not configured as a WASI P2 component. WASI Preview 2 requires projects to be built as components
              (using cargo-component) and export special memory management functions. Most existing projects are written for WASI P1.
              Solution: Use <code className="bg-yellow-100 px-1 rounded">wasm32-wasip1</code> instead.
            </p>
            <p className="text-sm text-yellow-800">
              <strong>Note on Rust version:</strong> The Docker image uses Rust 1.85 for maximum compatibility. While newer Rust versions (1.88+)
              exist, they may have breaking changes with certain dependencies. The production worker uses 1.85 to ensure broad compatibility.
            </p>
          </div>

          <h4 className="text-lg font-semibold mb-2 mt-4">Option 2: WASI Test Runner (Full Validation)</h4>
          <p className="text-gray-700 mb-3">
            We provide <strong>wasi-test-runner</strong> - a universal test tool that validates your WASM modules for OutLayer compatibility.
            It tests binary format, fuel metering, I/O handling, resource limits, JSON validation, and output size.
          </p>

          <div className="bg-blue-50 border-l-4 border-blue-400 p-4 mb-3">
            <p className="text-sm text-blue-800 mb-2">
              <strong>GitHub Repository:</strong> <a href="https://github.com/fastnear/near-outlayer/tree/main/wasi-examples/wasi-test-runner" target="_blank" rel="noopener noreferrer" className="underline hover:text-blue-900">wasi-examples/wasi-test-runner</a>
            </p>
          </div>

          <pre className="bg-gray-900 text-gray-100 p-4 rounded-lg overflow-x-auto text-sm mb-3">
{`# Install test runner
cd wasi-examples/wasi-test-runner
cargo build --release

# Test your WASM module
./target/release/wasi-test \\
  --wasm path/to/your-app.wasm \\
  --input '{"test":"data"}' \\
  --verbose

# Example output:
# ✓ Detected: WASI Preview 1 Module
# ✅ Execution successful!
# 📊 Fuel consumed: 456789 instructions
# 📤 Output: {"result":"success"}
# ✅ All checks passed!`}
          </pre>

          <p className="text-gray-700 mb-3">
            <strong>What it validates:</strong>
          </p>
          <ul className="list-disc list-inside text-gray-700 ml-4 space-y-1 mb-3">
            <li>Binary format (WASI P1 or P2)</li>
            <li>Fuel metering (instruction counting)</li>
            <li>Input/output handling (stdin → stdout)</li>
            <li>Resource limits enforcement</li>
            <li>JSON validation</li>
            <li>Output size limits (&lt;900 bytes)</li>
          </ul>

          <h4 className="text-lg font-semibold mb-2 mt-4">Option 3: Manual Testing with wasmtime</h4>
          <p className="text-gray-700 mb-2">
            Test directly using <code className="bg-gray-100 px-2 py-1 rounded">wasmtime</code>:
          </p>
          <pre className="bg-gray-900 text-gray-100 p-4 rounded-lg overflow-x-auto text-sm">
{`# Install wasmtime
curl https://wasmtime.dev/install.sh -sSf | bash

# Test WASI P1 binary
echo '{"value":21}' | wasmtime your-app.wasm

# Test WASI P2 component
echo '{"prompt":"test"}' | wasmtime your-app.wasm

# Test with environment variables
echo '{"message":"test"}' | wasmtime --env SECRET=my-key your-app.wasm`}
          </pre>
        </section>

        <section id="common-pitfalls">
          <AnchorHeading id="common-pitfalls">Common Pitfalls</AnchorHeading>
          <div className="space-y-3">
            <div className="bg-red-50 border-l-4 border-red-400 p-3">
              <strong className="text-red-800">Error: &quot;entry symbol not defined: _initialize&quot;</strong>
              <p className="text-gray-700 mt-1">Using <code>[lib]</code> instead of <code>[[bin]]</code> in Cargo.toml</p>
            </div>
            <div className="bg-red-50 border-l-4 border-red-400 p-3">
              <strong className="text-red-800">Empty output</strong>
              <p className="text-gray-700 mt-1">Forgot to call <code>io::stdout().flush()?</code> after writing</p>
            </div>
            <div className="bg-red-50 border-l-4 border-red-400 p-3">
              <strong className="text-red-800">HTTP requests fail</strong>
              <p className="text-gray-700 mt-1">Using WASI P1 instead of P2 - HTTP requires <code>wasm32-wasip2</code> target</p>
            </div>
            <div className="bg-red-50 border-l-4 border-red-400 p-3">
              <strong className="text-red-800">Output truncated in explorer</strong>
              <p className="text-gray-700 mt-1">Output exceeds 900 bytes - truncate before returning</p>
            </div>
          </div>
        </section>

        <section id="next-steps">
          <AnchorHeading id="next-steps">Next Steps</AnchorHeading>
          <ul className="list-disc list-inside space-y-2 text-gray-700">
            <li>Explore <Link href="/docs/examples" className="text-[var(--primary-orange)] hover:underline font-semibold">working examples</Link> with complete source code and deployment guides</li>
            <li>Read the <a href="https://github.com/fastnear/near-outlayer/blob/main/wasi-examples/WASI_TUTORIAL.md" target="_blank" rel="noopener noreferrer" className="text-[var(--primary-orange)] hover:underline">complete WASI tutorial</a></li>
            <li>Clone examples: <code className="bg-gray-100 px-2 py-1 rounded">git clone https://github.com/fastnear/near-outlayer.git</code></li>
            <li>Test your WASM locally with wasmtime before deploying</li>
            <li>Start with <strong>random-ark</strong> or <strong>echo-ark</strong> for simple use cases</li>
            <li>Use <strong>ai-ark</strong> or <strong>oracle-ark</strong> for HTTPS-based applications</li>
            <li>Study <strong>private-dao-ark</strong> for advanced cryptography (ECIES, HKDF, Merkle trees) and privacy patterns</li>
            <li>Deploy <strong>captcha-ark</strong> for full-stack async human verification</li>
          </ul>
        </section>
      </div>
    </div>
  );
}
