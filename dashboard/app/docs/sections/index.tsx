'use client';

import Link from 'next/link';
import { useEffect } from 'react';

export { default as GettingStartedSection } from './GettingStarted';
export { default as DeveloperGuideSection } from './DeveloperGuide';
export { default as TeeAttestationSection } from './TeeAttestation';

// Anchor heading component with clickable link
function AnchorHeading({ id, children }: { id: string; children: React.ReactNode }) {
  const handleClick = (e: React.MouseEvent) => {
    e.preventDefault();
    const element = document.getElementById(id);
    if (element) {
      element.scrollIntoView({ behavior: 'smooth', block: 'start' });
      window.history.pushState(null, '', `#${id}`);
    }
  };

  return (
    <h3 id={id} className="text-xl font-semibold mb-3 group relative">
      <a href={`#${id}`} onClick={handleClick} className="hover:text-[var(--primary-orange)] transition-colors">
        {children}
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
      // Delay to ensure content is rendered
      setTimeout(() => {
        const element = document.getElementById(hash);
        if (element) {
          element.scrollIntoView({ behavior: 'smooth', block: 'start' });
        }
      }, 100);
    }
  }, []);
}

export function ContractIntegrationSection() {
  useHashNavigation();

  return (
    <div className="prose max-w-none">
      <h2 className="text-3xl font-bold mb-6 text-[var(--primary-orange)]">Contract Integration</h2>

      <p className="text-gray-700 mb-6">
        Call OutLayer from your smart contract to execute off-chain WASM code and receive the result via callback. You stay in full control - no need to grant special access or delegate permissions to workers.
      </p>

      <div className="space-y-8">
        <section id="request-execution">
          <AnchorHeading id="request-execution">Method: request_execution</AnchorHeading>
          <p className="text-gray-700 mb-4">
            Call <code className="bg-gray-100 px-2 py-1 rounded">outlayer.testnet</code> (testnet) or <code className="bg-gray-100 px-2 py-1 rounded">outlayer.near</code> (mainnet)
          </p>

          <div className="bg-gray-50 border border-gray-200 rounded-lg p-4 mb-4">
            <h4 className="font-semibold mb-3 text-gray-800">Parameters (all fields):</h4>

            <div className="space-y-4">
              <div className="border-l-4 border-blue-400 pl-4">
                <p className="font-mono text-sm text-gray-800 mb-1"><strong>code_source</strong>: CodeSource <span className="text-red-600">(required)</span></p>
                <p className="text-sm text-gray-600 mb-2">Specifies where to get WASM code. Two variants available:</p>

                <div className="mt-2 mb-2">
                  <p className="text-sm font-semibold text-gray-800 mb-1">Variant 1: GitHub (compile from source)</p>
                  <ul className="list-disc list-inside text-sm text-gray-700 ml-4 space-y-1">
                    <li><code className="bg-gray-100 px-2 py-1 rounded">repo</code>: GitHub repository URL (e.g., &quot;https://github.com/user/project&quot;)</li>
                    <li><code className="bg-gray-100 px-2 py-1 rounded">commit</code>: Branch name (&quot;main&quot;) or commit hash (40-char SHA)</li>
                    <li><code className="bg-gray-100 px-2 py-1 rounded">build_target</code>: Optional. &quot;wasm32-wasip1&quot; or &quot;wasm32-wasip2&quot; (default: wasip1)</li>
                  </ul>
                </div>

                <div className="mt-2">
                  <p className="text-sm font-semibold text-gray-800 mb-1">Variant 2: WasmUrl (pre-compiled WASM)</p>
                  <ul className="list-disc list-inside text-sm text-gray-700 ml-4 space-y-1">
                    <li><code className="bg-gray-100 px-2 py-1 rounded">url</code>: URL to pre-compiled WASM file (FastFS, IPFS, etc.)</li>
                    <li><code className="bg-gray-100 px-2 py-1 rounded">hash</code>: SHA256 hash of WASM file for verification</li>
                    <li><code className="bg-gray-100 px-2 py-1 rounded">build_target</code>: &quot;wasm32-wasip1&quot; or &quot;wasm32-wasip2&quot;</li>
                  </ul>
                  <p className="text-xs text-gray-500 mt-1">Use WasmUrl for instant execution without compilation. Ideal for closed-source WASM or permanent deployments on FastFS/IPFS.</p>
                </div>
              </div>

              <div className="border-l-4 border-gray-400 pl-4">
                <p className="font-mono text-sm text-gray-800 mb-1"><strong>resource_limits</strong>: ResourceLimits <span className="text-gray-500">(optional)</span></p>
                <p className="text-sm text-gray-600 mb-2">Maximum resources to allocate. Defaults shown below:</p>
                <ul className="list-disc list-inside text-sm text-gray-700 ml-4 space-y-1">
                  <li><code className="bg-gray-100 px-2 py-1 rounded">max_instructions</code>: u64 (default: 1 billion, max: 500 billion)</li>
                  <li><code className="bg-gray-100 px-2 py-1 rounded">max_memory_mb</code>: u32 (default: 128 MB)</li>
                  <li><code className="bg-gray-100 px-2 py-1 rounded">max_execution_seconds</code>: u64 (default: 60s, max: 180s)</li>
                </ul>
              </div>

              <div className="border-l-4 border-gray-400 pl-4">
                <p className="font-mono text-sm text-gray-800 mb-1"><strong>input_data</strong>: String <span className="text-gray-500">(optional)</span></p>
                <p className="text-sm text-gray-600">JSON string passed to WASM as stdin. Your code reads it via <code className="bg-gray-100 px-1 rounded">std::io::stdin()</code></p>
                <p className="text-xs text-gray-500 mt-1">Example: <code className="bg-gray-100 px-1 rounded">{`{"min":1,"max":100}`}</code></p>
              </div>

              <div className="border-l-4 border-gray-400 pl-4">
                <p className="font-mono text-sm text-gray-800 mb-1"><strong>secrets_ref</strong>: SecretsReference <span className="text-gray-500">(optional)</span></p>
                <p className="text-sm text-gray-600 mb-2">Reference to encrypted secrets stored on-chain</p>
                <ul className="list-disc list-inside text-sm text-gray-700 ml-4 space-y-1">
                  <li><code className="bg-gray-100 px-2 py-1 rounded">profile</code>: Profile name (e.g., &quot;production&quot;, &quot;staging&quot;)</li>
                  <li><code className="bg-gray-100 px-2 py-1 rounded">account_id</code>: Account that owns the secrets</li>
                </ul>
                <p className="text-xs text-gray-500 mt-2">Worker decrypts secrets and injects as environment variables accessible via <code className="bg-gray-100 px-1 rounded">std::env::var()</code></p>
                <p className="text-xs text-gray-500 mt-1">Secrets can be bound to GitHub repo+branch OR to a WASM hash. When using WasmUrl, secrets bound to the hash are automatically matched.</p>
              </div>

              <div className="border-l-4 border-gray-400 pl-4">
                <p className="font-mono text-sm text-gray-800 mb-1"><strong>response_format</strong>: ResponseFormat <span className="text-gray-500">(optional)</span></p>
                <p className="text-sm text-gray-600 mb-2">How to parse WASM stdout (default: Text)</p>
                <ul className="list-disc list-inside text-sm text-gray-700 ml-4 space-y-1">
                  <li><code className="bg-gray-100 px-2 py-1 rounded">Text</code>: Return raw stdout as string (default)</li>
                  <li><code className="bg-gray-100 px-2 py-1 rounded">Json</code>: Parse stdout as JSON value</li>
                  <li><code className="bg-gray-100 px-2 py-1 rounded">Bytes</code>: Return raw bytes</li>
                </ul>
              </div>

              <div className="border-l-4 border-green-400 pl-4">
                <p className="font-mono text-sm text-gray-800 mb-1"><strong>payer_account_id</strong>: AccountId <span className="text-gray-500">(optional)</span></p>
                <p className="text-sm text-gray-600">Who receives the refund for unused resources:</p>
                <ul className="list-disc list-inside text-sm text-gray-700 ml-4 space-y-1 mt-1">
                  <li><strong>None</strong> (default): Sender pays and receives refund</li>
                  <li><strong>Some(user_account)</strong>: Charge end user, refund to user</li>
                </ul>
                <p className="text-xs text-gray-500 mt-2">💡 Useful for contracts that want users to pay for their own executions</p>
              </div>

              <div className="border-l-4 border-purple-400 pl-4">
                <p className="font-mono text-sm text-gray-800 mb-1"><strong>params</strong>: ExecutionParams <span className="text-gray-500">(optional)</span></p>
                <p className="text-sm text-gray-600 mb-2">Advanced execution control parameters:</p>
                <ul className="list-disc list-inside text-sm text-gray-700 ml-4 space-y-1">
                  <li><code className="bg-gray-100 px-2 py-1 rounded">force_rebuild</code>: bool (default: false) - Force recompilation even if WASM exists in cache. Useful when you updated code but kept the same commit hash, or need a fresh build for debugging.</li>
                  <li><code className="bg-gray-100 px-2 py-1 rounded">compile_only</code>: bool (default: false) - Only compile, don&apos;t execute. Returns checksum of compiled WASM. Useful for pre-warming cache or verifying compilation.</li>
                  <li><code className="bg-gray-100 px-2 py-1 rounded">store_on_fastfs</code>: bool (default: false) - Store compiled WASM on FastFS after compilation. Returns FastFS URL instead of execution result. Useful for permanent storage and sharing.</li>
                </ul>
                <p className="text-xs text-gray-500 mt-2">💡 Example: <code className="bg-gray-100 px-1 rounded">{`"params": {"force_rebuild": true, "compile_only": false, "store_on_fastfs": false}`}</code></p>
              </div>
            </div>
          </div>
        </section>

        <section id="callback">
          <AnchorHeading id="callback">What You Get Back (Callback)</AnchorHeading>
          <p className="text-gray-700 mb-3">
            OutLayer reads <strong>stdout</strong> from your WASM code and returns it to your contract&apos;s callback.
            You can write anything to stdout - a number, text, JSON object - and parse it however you want.
          </p>
          <ul className="list-disc list-inside space-y-2 text-gray-700 ml-4">
            <li><strong>Success case</strong>: Your stdout output (parsed according to response_format: Text/Json/Bytes)</li>
            <li><strong>Failure case</strong>: Error information (compilation failed, timeout, etc.)</li>
            <li><strong>Resource metrics</strong>: Actual instructions used, execution time</li>
            <li><strong>Automatic refund</strong>: Unused deposit sent to payer_account_id</li>
          </ul>
          <div className="bg-yellow-50 border-l-4 border-yellow-400 p-3 mt-3">
            <p className="text-sm text-gray-700">
              <strong>Important:</strong> You stay in full control. OutLayer just executes your code and returns stdout - no special permissions needed on your contract. You decide what to do with the results in your callback logic.
            </p>
          </div>
        </section>

        <section id="pricing-payment">
          <AnchorHeading id="pricing-payment">Pricing & Payment</AnchorHeading>
          <p className="text-gray-700 mb-3">
            Attach NEAR tokens when calling <code className="bg-gray-100 px-2 py-1 rounded">request_execution</code>. Cost is calculated dynamically based on resources used:
          </p>
          <ul className="list-disc list-inside space-y-2 text-gray-700 ml-4">
            <li>Base fee + (instructions used × per-instruction fee) + (execution time × per-millisecond fee)</li>
            <li>Unused funds automatically refunded to <code className="bg-gray-100 px-2 py-1 rounded">payer_account_id</code></li>
            <li>No refunds on execution failure (anti-DoS protection)</li>
            <li>Query <code className="bg-gray-100 px-2 py-1 rounded">estimate_execution_cost()</code> before calling to estimate required deposit</li>
          </ul>
        </section>

        <section id="performance">
          <AnchorHeading id="performance">Performance Tips</AnchorHeading>
          <div className="bg-blue-50 border-l-4 border-blue-500 p-4">
            <p className="text-sm text-gray-700 mb-2">
              <strong>GitHub compilation:</strong> ~10 seconds for simple projects, up to few minutes for complex ones (full Rust build with dependencies)
            </p>
            <p className="text-sm text-gray-700 mb-2">
              <strong>Execution time:</strong> Depends on your code - from milliseconds to minutes based on complexity and resource limits
            </p>
            <p className="text-sm text-gray-700">
              <strong>Pre-compiled WASM:</strong> Skip compilation entirely. Use WasmUrl with FastFS/IPFS for instant ~1s execution.
            </p>
          </div>
        </section>

        <section id="fastfs-workflow">
          <AnchorHeading id="fastfs-workflow">FastFS Workflow</AnchorHeading>
          <p className="text-gray-700 mb-3">
            For production deployments, use FastFS to store pre-compiled WASM for instant execution:
          </p>
          <ol className="list-decimal list-inside space-y-2 text-gray-700 ml-4">
            <li><strong>Compile with store_on_fastfs:</strong> Set <code className="bg-gray-100 px-1 rounded">{`"params": {"store_on_fastfs": true, "compile_only": true}`}</code></li>
            <li><strong>Get FastFS URL:</strong> Response contains <code className="bg-gray-100 px-1 rounded">fastfs_url</code> and <code className="bg-gray-100 px-1 rounded">wasm_hash</code></li>
            <li><strong>Execute via WasmUrl:</strong> Use the URL and hash in <code className="bg-gray-100 px-1 rounded">code_source</code> for instant execution</li>
          </ol>
          <div className="bg-green-50 border-l-4 border-green-400 p-3 mt-3">
            <p className="text-sm text-gray-700">
              <strong>Benefits:</strong> Instant execution (no compilation), immutable code (hash-verified), works with closed-source WASM, permanent storage on-chain.
            </p>
          </div>
        </section>
      </div>
    </div>
  );
}

export function WasiSection() {
  useHashNavigation();

  return (
    <div className="prose max-w-none">
      <h2 className="text-3xl font-bold mb-6 text-[var(--primary-orange)]">Writing WASI Code</h2>

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
            <h4 className="font-semibold text-yellow-900 mb-2">⚠️ Requirements</h4>
            <ul className="list-disc list-inside text-sm text-yellow-800 space-y-1">
              <li><strong>WASI Preview 2:</strong> Host functions require <code className="bg-yellow-100 px-1 rounded">wasm32-wasip2</code> target</li>
              <li><strong>Signer credentials:</strong> Must provide <code className="bg-yellow-100 px-1 rounded">NEAR_SENDER_PRIVATE_KEY</code> via secrets</li>
              <li><strong>NEAR tokens:</strong> Signer account must have sufficient balance for gas and deposits</li>
            </ul>
          </div>

          <div className="bg-blue-50 border-l-4 border-blue-400 p-4 mt-4">
            <h4 className="font-semibold text-blue-900 mb-2">🔄 API Versioning</h4>
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
              📚 Detailed Documentation Available
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

export function SecretsSection() {
  useHashNavigation();

  return (
    <div className="prose max-w-none">
      <h2 className="text-3xl font-bold mb-6 text-[var(--primary-orange)]">Secrets</h2>

      <div className="bg-blue-50 p-4 rounded-lg mb-6">
        <p className="text-blue-900 font-semibold mb-2">🔐 Enterprise-Grade Security with CKD & MPC Network</p>
        <p className="text-blue-800">
          Secrets are protected using <strong>Confidential Key Derivation (CKD)</strong> - a cutting-edge primitive that leverages
          the NEAR MPC Network to provide deterministic secrets for TEE applications. Each app gets cryptographically isolated keys
          that persist across TEE restarts, derived through distributed computation where no single node knows the final secret.
        </p>
      </div>

      <div className="space-y-6">
        <section id="what-are-secrets">
          <AnchorHeading id="what-are-secrets">What are Secrets?</AnchorHeading>
          <p className="text-gray-700">
            Secrets are encrypted API keys, tokens, or sensitive data stored on-chain. They are automatically decrypted
            and injected as environment variables when your WASM code executes. The keystore service running in TEE
            handles all encryption/decryption operations.
          </p>
        </section>

        <section id="creating-secrets">
          <AnchorHeading id="creating-secrets">Creating Secrets</AnchorHeading>
          <p className="text-gray-700 mb-3">
            Use the <Link href="/secrets" className="text-[var(--primary-orange)] hover:underline">Secrets</Link> page
            to create encrypted secrets. Specify repository, branch (optional), and profile name. Secrets are encrypted
            client-side before being stored on-chain.
          </p>

          <div className="bg-gray-50 p-4 rounded-lg mt-3">
            <h4 className="font-semibold mb-2 text-gray-800">Two Ways to Create Secrets:</h4>

            <div className="space-y-3">
              <div className="border-l-4 border-blue-400 pl-3">
                <p className="font-semibold text-gray-800 mb-1">1. Manual Secrets</p>
                <p className="text-sm text-gray-700 mb-2">Provide key-value pairs directly (e.g., API keys you already have)</p>
                <ul className="list-disc list-inside text-sm text-gray-700 ml-4">
                  <li>Encrypted client-side with ChaCha20-Poly1305</li>
                  <li>Example: <code className="bg-gray-100 px-2 py-1 rounded">{`{"OPENAI_KEY": "sk-..."}`}</code></li>
                  <li className="text-amber-700">⚠️ Cannot use <code className="bg-amber-100 px-1 rounded">PROTECTED_*</code> prefix (reserved for auto-generated)</li>
                </ul>
              </div>

              <div className="border-l-4 border-green-400 pl-3">
                <p className="font-semibold text-gray-800 mb-1">2. Auto-Generated Secrets</p>
                <p className="text-sm text-gray-700 mb-2">Generate cryptographically secure secrets in TEE without seeing their values</p>
                <ul className="list-disc list-inside text-sm text-gray-700 ml-4">
                  <li>Generated inside TEE (nobody ever sees the value)</li>
                  <li>Perfect for derivation keys, signing keys, encryption keys</li>
                  <li className="text-green-700">✅ Must start with <code className="bg-green-100 px-1 rounded">PROTECTED_*</code> prefix (proves TEE generation)</li>
                  <li>Example: <code className="bg-gray-100 px-2 py-1 rounded">PROTECTED_MASTER_KEY</code></li>
                  <li>Types: hex32/64, ED25519, password:N</li>
                </ul>
              </div>
            </div>

            <div className="mt-3 p-3 bg-blue-50 border border-blue-200 rounded">
              <p className="text-sm text-blue-900 font-medium mb-1">🔐 Naming Convention for Trust</p>
              <p className="text-xs text-blue-800">
                The <code className="bg-blue-100 px-1 rounded">PROTECTED_*</code> prefix proves a secret was generated in TEE and never seen by anyone (including developers).
                Manual secrets cannot use this prefix - enforced by keystore validation.
              </p>
            </div>
          </div>
        </section>

        <section id="secrets-binding">
          <AnchorHeading id="secrets-binding">Secrets Binding Types</AnchorHeading>
          <p className="text-gray-700 mb-3">
            Secrets can be bound to different identifiers depending on your use case:
          </p>

          <div className="space-y-3">
            <div className="border-l-4 border-blue-400 pl-3">
              <p className="font-semibold text-gray-800 mb-1">Repository-based (GitHub)</p>
              <p className="text-sm text-gray-700 mb-2">Bind secrets to a GitHub repository and optional branch</p>
              <ul className="list-disc list-inside text-sm text-gray-700 ml-4">
                <li>Key: <code className="bg-gray-100 px-1 rounded">repo + branch + profile + owner</code></li>
                <li>Example: <code className="bg-gray-100 px-1 rounded">github.com/user/repo:main:production</code></li>
                <li>Best for: Development, CI/CD workflows, version-specific secrets</li>
              </ul>
            </div>

            <div className="border-l-4 border-purple-400 pl-3">
              <p className="font-semibold text-gray-800 mb-1">WASM Hash-based</p>
              <p className="text-sm text-gray-700 mb-2">Bind secrets to a specific compiled WASM binary (SHA256 hash)</p>
              <ul className="list-disc list-inside text-sm text-gray-700 ml-4">
                <li>Key: <code className="bg-gray-100 px-1 rounded">wasm_hash + profile + owner</code></li>
                <li>Example: <code className="bg-gray-100 px-1 rounded">cbf80ed0...2f8:production</code></li>
                <li>Best for: Pre-compiled WASM from FastFS/IPFS, immutable deployments</li>
                <li>Guarantees: Only this exact binary can access the secrets</li>
              </ul>
            </div>
          </div>

          <div className="mt-3 p-3 bg-purple-50 border border-purple-200 rounded">
            <p className="text-sm text-purple-900 font-medium mb-1">🔒 WASM Hash Binding Security</p>
            <p className="text-xs text-purple-800">
              When using WASM hash binding, secrets are cryptographically tied to the exact binary.
              Any modification to the code produces a different hash, preventing unauthorized access.
              This is ideal for production deployments where code immutability is required.
            </p>
          </div>
        </section>

        <section id="access-control">
          <AnchorHeading id="access-control">Access Control</AnchorHeading>
          <p className="text-gray-700">
            Control who can decrypt your secrets using flexible access conditions:
          </p>
          <ul className="list-disc list-inside space-y-2 text-gray-700 mt-2">
            <li><strong>AllowAll:</strong> Anyone can use (suitable for public data)</li>
            <li><strong>Whitelist:</strong> Specific NEAR accounts only</li>
            <li><strong>NEAR Balance:</strong> Accounts with minimum NEAR balance</li>
            <li><strong>FT/NFT Balance:</strong> Token holders only</li>
            <li><strong>Account Pattern:</strong> Regex-based account filtering</li>
            <li><strong>Logic:</strong> Complex AND/OR/NOT conditions</li>
          </ul>
        </section>

        <section id="using-secrets">
          <AnchorHeading id="using-secrets">Using Secrets in Code</AnchorHeading>
          <p className="text-gray-700">
            Access secrets in your WASM code using standard environment variable functions. In Rust:
            <code className="bg-gray-100 px-2 py-1 rounded ml-1">std::env::var(&quot;API_KEY&quot;)</code>
          </p>
        </section>

        <section id="storage-costs">
          <AnchorHeading id="storage-costs">Storage Costs</AnchorHeading>
          <p className="text-gray-700">
            Secrets storage costs are proportional to data size plus indexing overhead (~64 bytes). Storage fees
            are refunded when you delete secrets.
          </p>
        </section>

        <section id="security-model">
          <AnchorHeading id="security-model">Security Model</AnchorHeading>
          <p className="text-gray-700">
            Secrets are encrypted with ChaCha20-Poly1305 AEAD (authenticated encryption with associated data).
            Decryption happens in TEE workers with attestation verification. Your secrets never leave the secure enclave.
          </p>
        </section>

        <section id="confidential-key-derivation">
          <AnchorHeading id="confidential-key-derivation">Confidential Key Derivation (CKD)</AnchorHeading>

          <div className="bg-gray-50 p-4 rounded-lg mb-4">
            <h4 className="font-semibold text-gray-900 mb-2">How Keystore Gets Its Derivation Key via MPC</h4>
            <p className="text-gray-700 mb-3">
              The keystore worker itself is a TEE application that obtains its derivation key through NEAR MPC Network via DAO governance. 
              <strong>Critically, the keystore uses a functional key (not a full access key)</strong> that can ONLY call the MPC signer
              through the DAO contract's <code className="bg-gray-100 px-1 rounded">request_key</code> method. This architectural decision ensures
              the keystore cannot directly access the MPC network - it must go through DAO governance, making all operations auditable on-chain.
              Once authorized by the DAO, the keystore requests a deterministic derivation key from MPC nodes using Confidential Key Derivation.
              This derivation key is then used to decrypt secrets for other applications, ensuring all cryptographic operations stay within the TEE.
            </p>

            {/* Desktop Diagram 1: Registration */}
            <div className="hidden md:block bg-white border-2 border-gray-300 rounded-lg p-6 mb-6 overflow-x-auto">
              <svg viewBox="0 0 900 305" className="w-full" style={{ maxWidth: '900px', margin: '0 auto' }}>
                {/* Define gradients and arrow markers */}
                <defs>
                  <linearGradient id="teeGradient" x1="0%" y1="0%" x2="0%" y2="100%">
                    <stop offset="0%" stopColor="#fbbf24" stopOpacity="0.8" />
                    <stop offset="100%" stopColor="#f59e0b" stopOpacity="0.8" />
                  </linearGradient>
                  <linearGradient id="daoGradient" x1="0%" y1="0%" x2="0%" y2="100%">
                    <stop offset="0%" stopColor="#a78bfa" stopOpacity="0.8" />
                    <stop offset="100%" stopColor="#8b5cf6" stopOpacity="0.8" />
                  </linearGradient>
                  <marker id="arrowPurple" markerWidth="10" markerHeight="10" refX="8" refY="5" orient="auto">
                    <polygon points="0 0, 10 5, 0 10" fill="#8b5cf6" />
                  </marker>
                  <marker id="arrowGreen" markerWidth="10" markerHeight="10" refX="8" refY="5" orient="auto">
                    <polygon points="0 0, 10 5, 0 10" fill="#10b981" />
                  </marker>
                  <marker id="arrowOrange" markerWidth="10" markerHeight="10" refX="8" refY="5" orient="auto">
                    <polygon points="0 0, 10 5, 0 10" fill="#f97316" />
                  </marker>
                </defs>

                {/* Title */}
                <text x="450" y="20" textAnchor="middle" fontSize="20" fontWeight="bold" fill="#1f2937">
                  Phase 1: Keystore Registration
                </text>

                {/* Keystore TEE */}
                <g transform="translate(50, 60)">
                  <rect x="0" y="0" width="180" height="120" fill="url(#teeGradient)" rx="8" stroke="#f59e0b" strokeWidth="2" />
                  <text x="90" y="25" textAnchor="middle" fill="#7c2d12" fontSize="14" fontWeight="bold">Keystore TEE</text>
                  <line x1="10" y1="35" x2="170" y2="35" stroke="#92400e" strokeOpacity="0.5" />
                  <text x="90" y="55" textAnchor="middle" fill="#7c2d12" fontSize="11">1. Generate keypair</text>
                  <text x="90" y="75" textAnchor="middle" fill="#7c2d12" fontSize="11">2. Generate attestation</text>
                  <text x="90" y="95" textAnchor="middle" fill="#7c2d12" fontSize="11">3. RTMR3: 0x2641ff...</text>
                </g>

                {/* DAO Contract */}
                <g transform="translate(360, 60)">
                  <rect x="0" y="0" width="180" height="120" fill="url(#daoGradient)" rx="8" stroke="#8b5cf6" strokeWidth="2" />
                  <text x="90" y="25" textAnchor="middle" fill="white" fontSize="14" fontWeight="bold">DAO Contract</text>                  
                  <line x1="10" y1="35" x2="170" y2="35" stroke="white" strokeOpacity="0.5" />
                  <text x="90" y="55" textAnchor="middle" fill="white" fontSize="11">Verifies TEE attestation</text>
                  <text x="90" y="75" textAnchor="middle" fill="white" fontSize="11">Verifies TEE environment</text>
                  <text x="90" y="95" textAnchor="middle" fill="white" fontSize="11">Creates proposal</text>
                </g>

                {/* DAO Members */}
                <g transform="translate(670, 60)">
                  <rect x="0" y="0" width="180" height="120" fill="#e9d5ff" stroke="#8b5cf6" strokeWidth="2" rx="8" />
                  <text x="90" y="25" textAnchor="middle" fontSize="14" fontWeight="bold" fill="#6b21a8">DAO Members</text>
                  <line x1="10" y1="35" x2="170" y2="35" stroke="#8b5cf6" strokeOpacity="0.5" />
                  <text x="90" y="55" textAnchor="middle" fontSize="11" fill="#6b21a8">Review input data</text>
                  <text x="90" y="75" textAnchor="middle" fontSize="11" fill="#6b21a8">Vote on proposal</text>
                  <text x="90" y="95" textAnchor="middle" fontSize="11" fill="#6b21a8">Need &gt;50% approval</text>
                </g>

                {/* Arrows */}
                {/* 1: Submit attestation */}
                <path d="M 230 120 L 355 120" stroke="#8b5cf6" strokeWidth="2" markerEnd="url(#arrowPurple)" />
                <text x="295" y="135" textAnchor="middle" fontSize="11" fill="#8b5cf6">1. Submit</text>
                <text x="295" y="150" textAnchor="middle" fontSize="11" fill="#8b5cf6">attestation</text>

                {/* 2: Create proposal */}
                <path d="M 540 120 L 665 120" stroke="#8b5cf6" strokeWidth="2" markerEnd="url(#arrowPurple)" />
                <text x="605" y="135" textAnchor="middle" fontSize="11" fill="#8b5cf6">2. Create</text>
                <text x="605" y="150" textAnchor="middle" fontSize="11" fill="#8b5cf6">proposal</text>

                {/* 3: Vote approval */}
                <path d="M 760 180 L 760 225" stroke="#10b981" strokeWidth="2" markerEnd="url(#arrowGreen)" />
                <text x="770" y="200" fontSize="11" fill="#166534">3. Approve</text>                

                {/* Result: Functional Key */}
                <g transform="translate(400, 230)">
                  <rect x="0" y="0" width="400" height="70" fill="#f0fdf4" stroke="#166534" strokeWidth="2" rx="8" />
                  <text x="200" y="25" textAnchor="middle" fontSize="14" fontWeight="bold" fill="#166534">✓ Registration Complete</text>
                  <line x1="20" y1="35" x2="380" y2="35" stroke="#166534" strokeOpacity="0.5" />
                  <text x="200" y="55" textAnchor="middle" fontSize="12" fill="#166534">Functional key added to DAO contract. Keystore can now call CKD</text>                  
                </g>

                {/* 4: Adds key */}
                <path d="M 460 230 L 460 185" stroke="#10b981" strokeWidth="2" markerEnd="url(#arrowGreen)" />
                <text x="470" y="215" fontSize="11" fill="#166534">4. Adds key</text>
              </svg>
            </div>

            {/* Desktop Diagram 2: CKD Flow */}
            <div className="hidden md:block bg-white border-2 border-gray-300 rounded-lg p-6 mb-3 overflow-x-auto">
              <svg viewBox="0 0 1000 375" className="w-full" style={{ maxWidth: '1000px', margin: '0 auto' }}>
                {/* Define gradients and arrow markers */}
                <defs>
                  <linearGradient id="teeGradient2" x1="0%" y1="0%" x2="0%" y2="100%">
                    <stop offset="0%" stopColor="#fbbf24" stopOpacity="0.8" />
                    <stop offset="100%" stopColor="#f59e0b" stopOpacity="0.8" />
                  </linearGradient>
                  <linearGradient id="daoGradient2" x1="0%" y1="0%" x2="0%" y2="100%">
                    <stop offset="0%" stopColor="#a78bfa" stopOpacity="0.8" />
                    <stop offset="100%" stopColor="#8b5cf6" stopOpacity="0.8" />
                  </linearGradient>
                  <linearGradient id="mpcGradient2" x1="0%" y1="0%" x2="0%" y2="100%">
                    <stop offset="0%" stopColor="#60a5fa" stopOpacity="0.8" />
                    <stop offset="100%" stopColor="#3b82f6" stopOpacity="0.8" />
                  </linearGradient>
                  <marker id="arrowBlue2" markerWidth="10" markerHeight="10" refX="8" refY="5" orient="auto">
                    <polygon points="0 0, 10 5, 0 10" fill="#3b82f6" />
                  </marker>
                  <marker id="arrowPurple2" markerWidth="10" markerHeight="10" refX="8" refY="5" orient="auto">
                    <polygon points="0 0, 10 5, 0 10" fill="#8b5cf6" />
                  </marker>
                  <marker id="arrowGreen2" markerWidth="10" markerHeight="10" refX="8" refY="5" orient="auto">
                    <polygon points="0 0, 10 5, 0 10" fill="#10b981" />
                  </marker>
                </defs>

                {/* Title */}
                <text x="500" y="20" textAnchor="middle" fontSize="20" fontWeight="bold" fill="#1f2937">
                  Phase 2: CKD Flow
                </text>

                {/* Keystore TEE with functional key */}
                <g transform="translate(50, 60)">
                  <rect x="0" y="0" width="160" height="120" fill="url(#teeGradient2)" rx="8" stroke="#f59e0b" strokeWidth="2" />
                  <text x="80" y="25" textAnchor="middle" fill="#7c2d12" fontSize="14" fontWeight="bold">Keystore TEE</text>
                  <rect x="20" y="35" width="120" height="25" fill="#fff7ed" rx="5" />
                  <text x="80" y="52" textAnchor="middle" fontSize="10" fill="#7c2d12" fontWeight="bold">✓ Has limited key</text>
                  <line x1="10" y1="70" x2="150" y2="70" stroke="#92400e" strokeOpacity="0.5" />
                  <text x="80" y="90" textAnchor="middle" fill="#7c2d12" fontSize="11">Needs CKD for</text>
                  <text x="80" y="108" textAnchor="middle" fill="#7c2d12" fontSize="11">app secrets</text>
                </g>

                {/* DAO Contract Gateway */}
                <g transform="translate(300, 60)">
                  <rect x="0" y="0" width="160" height="120" fill="url(#daoGradient2)" rx="8" stroke="#8b5cf6" strokeWidth="2" />
                  <text x="80" y="25" textAnchor="middle" fill="white" fontSize="14" fontWeight="bold">DAO Contract</text>
                  <text x="80" y="45" textAnchor="middle" fill="white" fontSize="10">(Gateway)</text>
                  <line x1="10" y1="55" x2="150" y2="55" stroke="white" strokeOpacity="0.5" />
                  <text x="80" y="75" textAnchor="middle" fill="white" fontSize="11">Only functional</text>
                  <text x="80" y="93" textAnchor="middle" fill="white" fontSize="11">key can call</text>
                  <text x="80" y="110" textAnchor="middle" fill="white" fontSize="10">request_key()</text>
                </g>

                {/* MPC Contract */}
                <g transform="translate(540, 60)">
                  <rect x="0" y="0" width="160" height="120" fill="url(#mpcGradient2)" rx="8" stroke="#3b82f6" strokeWidth="2" />
                  <text x="80" y="25" textAnchor="middle" fill="white" fontSize="14" fontWeight="bold">MPC Contract</text>
                  <text x="80" y="45" textAnchor="middle" fill="white" fontSize="10">v1.signer-prod</text>
                  <line x1="10" y1="55" x2="150" y2="55" stroke="white" strokeOpacity="0.5" />
                  <text x="80" y="75" textAnchor="middle" fill="white" fontSize="11">Coordinates</text>
                  <text x="80" y="93" textAnchor="middle" fill="white" fontSize="11">key derivation</text>
                </g>

                {/* MPC Network */}
                <g transform="translate(710, 60)">
                  <text x="110" y="15" textAnchor="middle" fontSize="13" fontWeight="bold" fill="#1e40af">MPC Network</text>                  

                  {/* Nodes in circle */}
                  <g transform="translate(110, 70)">
                    <circle cx="0" cy="-30" r="15" fill="#dbeafe" stroke="#3b82f6" strokeWidth="2" />
                    <text x="0" y="-26" textAnchor="middle" fontSize="8" fill="#1e40af">N1</text>

                    <circle cx="26" cy="-15" r="15" fill="#dbeafe" stroke="#3b82f6" strokeWidth="2" />
                    <text x="26" y="-11" textAnchor="middle" fontSize="8" fill="#1e40af">N2</text>

                    <circle cx="26" cy="15" r="15" fill="#dbeafe" stroke="#3b82f6" strokeWidth="2" />
                    <text x="26" y="19" textAnchor="middle" fontSize="8" fill="#1e40af">N3</text>

                    <circle cx="0" cy="30" r="15" fill="#dbeafe" stroke="#3b82f6" strokeWidth="2" />
                    <text x="0" y="34" textAnchor="middle" fontSize="8" fill="#1e40af">N4</text>

                    <circle cx="-26" cy="15" r="15" fill="#dbeafe" stroke="#3b82f6" strokeWidth="2" />
                    <text x="-26" y="19" textAnchor="middle" fontSize="8" fill="#1e40af">N5</text>

                    <circle cx="-26" cy="-15" r="15" fill="#dbeafe" stroke="#3b82f6" strokeWidth="2" />
                    <text x="-26" y="-11" textAnchor="middle" fontSize="8" fill="#1e40af">N6</text>

                    <circle cx="0" cy="0" r="15" fill="#60a5fa" stroke="#3b82f6" strokeWidth="2" />
                    <text x="0" y="4" textAnchor="middle" fontSize="8" fill="white">N7</text>
                  </g>

                  <text x="110" y="155" textAnchor="middle" fontSize="12" fill="#64748b">Distributed</text>
                  <text x="110" y="170" textAnchor="middle" fontSize="12" fill="#64748b">computation</text>
                </g>

                {/* Flow arrows */}
                {/* 1: Request CKD */}
                <path d="M 210 120 L 300 120" stroke="#8b5cf6" strokeWidth="2" markerEnd="url(#arrowPurple2)" />
                <text x="255" y="140" textAnchor="middle" fontSize="11" fill="#8b5cf6">1. Request CKD</text>
                <text x="255" y="155" textAnchor="middle" fontSize="11" fill="#8b5cf6">(limited key)</text>

                {/* 2: Forward to MPC */}
                <path d="M 460 120 L 540 120" stroke="#3b82f6" strokeWidth="2" markerEnd="url(#arrowBlue2)" />
                <text x="495" y="140" textAnchor="middle" fontSize="11" fill="#3b82f6">2. Forward</text>
                <text x="495" y="155" textAnchor="middle" fontSize="11" fill="#3b82f6">to MPC</text>

                {/* 3: Distribute */}
                <path d="M 710 120 L 770 120" stroke="#3b82f6" strokeWidth="2" markerEnd="url(#arrowBlue2)" />
                <text x="740" y="140" textAnchor="middle" fontSize="11" fill="#3b82f6">3. Compute</text>

                {/* 4: Return encrypted key */}
                <path d="M 820 190 Q 480 320 130 190"
                      fill="none"
                      stroke="#10b981"
                      strokeWidth="3"
                      strokeDasharray="8,4"
                      markerEnd="url(#arrowGreen2)" />

                <rect x="350" y="270" width="300" height="50" fill="white" stroke="#10b981" strokeWidth="2" rx="8" />
                <text x="500" y="290" textAnchor="middle" fontSize="14" fontWeight="bold" fill="#166534">4. Encrypted Derivation Key</text>
                <text x="500" y="308" textAnchor="middle" fontSize="14" fill="#166534">(Only TEE can decrypt)</text>

                {/* Properties */}
                <rect x="100" y="340" width="800" height="30" fill="#f0fdf4" stroke="#10b981" strokeWidth="2" rx="8" />
                <text x="500" y="360" textAnchor="middle" fontSize="12" fill="#166534" fontWeight="bold">
                  ✓ Deterministic  •  ✓ Persistent  •  ✓ Survives restarts  •  ✓ No single point of failure
                </text>
              </svg>
            </div>

            {/* Mobile Simplified View with ASCII Diagrams */}
            <div className="md:hidden bg-white border-2 border-gray-300 rounded-lg p-4 mb-3">
              <div className="space-y-6">
                {/* Phase 1: Registration */}
                <div className="bg-amber-50 p-4 rounded-lg border-2 border-amber-400">
                  <h4 className="font-bold text-amber-900 mb-3">📝 Phase 1: Registration (One-time)</h4>

                  <div className="bg-white p-3 rounded font-mono text-xs">
                    <pre className="whitespace-pre">{`┌──────────┐
│ Keystore │
│   TEE    │
│          │
│• Generate│
│  keypair │
│• RTMR3   │
│ 0x2641.. │
└────┬─────┘
     │
     │ 1. Submit
     │ attestation
     ▼
┌──────────┐
│   DAO    │
│ Contract │
│          │
│• Verify  │
│  attest. │
│• Create  │
│ proposal │
└────┬─────┘
     │
     │ 2. Send
     │ proposal
     ▼
┌──────────┐
│   DAO    │
│ Members  │
│          │
│• Review  │
│  RTMR3   │
│• Vote    │
│  (>50%)  │
└────┬─────┘
     │
     │ 3. Approve
     ▼
┌──────────┐
│    ✓     │
│APPROVED  │
│          │
│Functional│
│key added │
│ to DAO   │
└──────────┘`}</pre>
                  </div>
                </div>

                {/* Phase 2: CKD Flow */}
                <div className="bg-blue-50 p-4 rounded-lg border-2 border-blue-400">
                  <h4 className="font-bold text-blue-900 mb-3">🔑 Phase 2: CKD Flow (Repeatable)</h4>

                  <div className="bg-white p-3 rounded font-mono text-xs">
                    <pre className="whitespace-pre">{`┌──────────┐
│ Keystore │
│   TEE    │
│          │
│✓ Has     │
│  func key│
│• Needs   │
│  CKD     │
└────┬─────┘
     │
     │ 1. Request
     │ CKD with
     │ func key
     ▼
┌──────────┐
│   DAO    │
│ Contract │
│(Gateway) │
│          │
│Only func │
│key can   │
│call      │
└────┬─────┘
     │
     │ 2. Forward
     │ request_key()
     ▼
┌──────────┐
│   MPC    │
│ Contract │
│          │
│v1.signer-│
│   prod   │
│          │
│Coordinate│
│derivation│
└────┬─────┘
     │
     │ 3. Distribute
     │ to nodes
     ▼
┌──────────┐
│   MPC    │
│  Nodes   │
│          │
│  ● ● ●   │
│   ● ●    │
│  ● ● ●   │
│          │
│ Compute  │
│BLS12-381 │
└────┬─────┘
     │
     │ 4. Return
     │ encrypted
     │ derivation key
     ▼
┌──────────┐
│ Keystore │
│ receives │
│encrypted │
│   key    │
│          │
│ Only TEE │
│   can    │
│ decrypt  │
└──────────┘`}</pre>
                  </div>
                </div>

                {/* Key Properties */}
                <div className="bg-green-50 p-3 rounded-lg border border-green-400">
                  <h4 className="font-semibold text-green-900 mb-2 text-sm">🔐 Key Properties</h4>
                  <div className="grid grid-cols-2 gap-2 text-xs">
                    <div className="flex items-center gap-1">
                      <span className="text-green-600">✓</span>
                      <span>Deterministic</span>
                    </div>
                    <div className="flex items-center gap-1">
                      <span className="text-green-600">✓</span>
                      <span>Persistent</span>
                    </div>
                    <div className="flex items-center gap-1">
                      <span className="text-green-600">✓</span>
                      <span>Survives restarts</span>
                    </div>
                    <div className="flex items-center gap-1">
                      <span className="text-green-600">✓</span>
                      <span>No single point of failure</span>
                    </div>
                  </div>
                </div>
              </div>
            </div>

            <p className="text-sm text-gray-700 mb-2">
              <strong>Two-Level Architecture:</strong>
            </p>
            <ul className="list-disc list-inside text-sm text-gray-700 space-y-1 mb-3">
              <li><strong>Level 1:</strong> Keystore obtains derivation key from NEAR MPC via CKD protocol through DAO contract</li>
              <li><strong>Level 2:</strong> Keystore uses derivation key to decrypt app secrets</li>
              <li>All operations happen inside TEE - keys never leave the enclave</li>
              <li>DAO governance ensures only legitimate keystores get derivation keys</li>
              <li>Functional keys restrict keystore operations through DAO contract</li>
              <li>All key derivation requests are logged on-chain for auditability</li>
              <li>MPC Network ensures no single entity controls the derivation key generation</li>
            </ul>
          </div>

          <div className="grid md:grid-cols-2 gap-4 mb-4">
            <div className="border rounded-lg p-4 bg-green-50">
              <h4 className="font-semibold text-green-900 mb-2">✅ Key Properties</h4>
              <ul className="text-sm text-green-800 space-y-1">
                <li>• <strong>Deterministic:</strong> Same app_id always gets same secret</li>
                <li>• <strong>Private:</strong> Secret known only to TEE app</li>
                <li>• <strong>Distributed:</strong> No single MPC node has the secret</li>
                <li>• <strong>Persistent:</strong> Works across TEE restarts</li>
              </ul>
            </div>

            <div className="border rounded-lg p-4 bg-blue-50">
              <h4 className="font-semibold text-blue-900 mb-2">🔒 Security Guarantees</h4>
              <ul className="text-sm text-blue-800 space-y-1">
                <li>• BLS signatures on BLS12-381 curves</li>
                <li>• ElGamal encryption for transport</li>
                <li>• TEE attestation verification</li>
                <li>• Threshold cryptography (t-of-n)</li>
              </ul>
            </div>
          </div>

          <div className="bg-yellow-50 border-l-4 border-yellow-400 p-4">
            <h4 className="font-semibold text-yellow-900 mb-2">Why MPC-based CKD is Revolutionary</h4>
            <p className="text-sm text-yellow-800">
              Traditional approaches either store keys (security risk) or lose them on restart (no persistence).
              NEAR&apos;s MPC-based CKD is unique: it provides deterministic secrets through distributed computation where
              no single entity ever has the complete key. This combines the benefits of persistence, security, and
              decentralization - a combination not available in other systems.
            </p>
          </div>
        </section>

        <section id="dao-governance">
          <AnchorHeading id="dao-governance">DAO Governance & Keystore Authorization</AnchorHeading>

          <div className="bg-purple-50 p-4 rounded-lg mb-4">
            <h4 className="font-semibold text-purple-900 mb-2">🏛️ DAO Controls Keystore Access to MPC</h4>
            <p className="text-purple-800">
              The DAO governs which keystore workers can receive derivation keys from the NEAR MPC Network. Only TEE-verified keystores
              that pass DAO voting can request CKD from MPC nodes. This ensures that derivation keys are only given to legitimate,
              attestation-verified keystores running in secure enclaves, preventing any unauthorized access to user secrets.
            </p>
          </div>

          <h4 className="font-semibold text-gray-900 mb-3">Keystore Authorization Flow</h4>

          <ol className="list-decimal list-inside space-y-3 text-gray-700 mb-4">
            <li>
              <strong>On-Chain TEE Verification:</strong> Keystore submits Intel TDX/SGX attestation directly to DAO contract.
              The contract cryptographically verifies the Intel certificate and TEE environment hash (RTMR3/MRENCLAVE) on-chain.
              This ensures submissions can only come from genuine TEE with verified binary.
            </li>
            <li>
              <strong>Automated Validation:</strong> DAO contract automatically rejects any submission that:
              <ul className="list-disc list-inside ml-6 mt-1 text-sm">
                <li>Doesn&apos;t have valid Intel signature</li>
                <li>Comes from unverified RTMR3/MRENCLAVE</li>
                <li>Attempts to bypass TEE requirements</li>
              </ul>
            </li>
            <li>
              <strong>DAO Voting:</strong> Only after passing on-chain TEE verification, DAO members vote to authorize keystore
              based on operator reputation, stake, and network capacity needs
            </li>
            <li>
              <strong>MPC Key Request:</strong> Once approved, keystore requests derivation key from MPC Network
              using CKD protocol with its unique keystore_id
            </li>
            <li>
              <strong>Derivation Key Receipt:</strong> Keystore receives encrypted derivation key, decrypts it in TEE,
              and can now decrypt user secrets while keeping all keys inside the enclave
            </li>
          </ol>

          <div className="bg-blue-50 border border-blue-200 rounded-lg p-4 mb-4">
            <h4 className="font-semibold text-blue-900 mb-2">🔍 Cryptographic Properties</h4>
            <p className="text-sm text-blue-800 mb-2">
              The CKD protocol ensures strong security through:
            </p>
            <ul className="list-disc list-inside text-sm text-blue-800 space-y-1">
              <li>BLS signatures on pairing-friendly BLS12-381 curves</li>
              <li>Threshold cryptography - requires t-of-n nodes to cooperate</li>
              <li>ElGamal encryption for secure transport</li>
              <li>HKDF for key derivation from BLS signatures</li>
            </ul>
            <p className="text-sm text-blue-800 mt-2">
              This combination ensures that secrets are deterministic yet unpredictable, persistent yet secure,
              distributed yet accessible only to authorized TEE apps.
            </p>
          </div>

          <div className="grid md:grid-cols-2 gap-4 mb-4">
            <div className="border rounded-lg p-4">
              <h4 className="font-semibold text-gray-900 mb-2">🛡️ Security Properties</h4>
              <ul className="text-sm text-gray-700 space-y-1">
                <li>• <strong>No single point of failure:</strong> Distributed MPC nodes</li>
                <li>• <strong>Forward secrecy:</strong> Fresh key pair for each request</li>
                <li>• <strong>TEE isolation:</strong> Secrets computed inside enclave</li>
                <li>• <strong>Threshold security:</strong> Requires multiple nodes</li>
              </ul>
            </div>

            <div className="border rounded-lg p-4">
              <h4 className="font-semibold text-gray-900 mb-2">✅ Trust Model</h4>
              <ul className="text-sm text-gray-700 space-y-1">
                <li>• Intel TDX attestation verification</li>
                <li>• MPC network consensus</li>
                <li>• Smart contract enforcement</li>
                <li>• Cryptographic correctness proofs</li>
              </ul>
            </div>
          </div>

          <div className="bg-green-50 border-l-4 border-green-400 p-4">
            <h4 className="font-semibold text-green-900 mb-2">Example: CKD Request</h4>
            <p className="text-sm text-green-800 mb-2">
              How a TEE app requests a deterministic secret:
            </p>
            <pre className="bg-green-100 p-2 rounded text-xs overflow-x-auto">
{`// TEE app generates key pair
let (a, A) = generate_elgamal_keypair();

// Include A in attestation report_data
let attestation = get_tdx_attestation(A);

// Call developer contract
developer_contract.get_key(attestation, A);

// Developer contract validates and calls MPC
mpc_contract.gen_app_private_key(A);

// Receive encrypted secret (Y, C)
// Decrypt: sig = C - a·Y
// Derive: secret = HKDF(sig)`}
            </pre>
            <p className="text-xs text-green-700 mt-2">
              The final secret is deterministic for app_id but known only to the TEE app.
            </p>
          </div>
        </section>

        <section id="ckd-faq">
          <AnchorHeading id="ckd-faq">CKD & MPC FAQ</AnchorHeading>

          <div className="space-y-4">
            <details className="border rounded-lg p-4">
              <summary className="font-semibold cursor-pointer text-gray-900">
                What happens if the keystore restarts?
              </summary>
              <p className="mt-2 text-gray-700">
                The keystore can request the same derivation key again from NEAR MPC using its keystore_id. Since CKD
                is deterministic, it will receive the same derivation key. This allows the keystore to continue decrypting
                user secrets after restarts without storing keys on disk.
              </p>
            </details>

            <details className="border rounded-lg p-4">
              <summary className="font-semibold cursor-pointer text-gray-900">
                Can MPC nodes or DAO see my secrets?
              </summary>
              <p className="mt-2 text-gray-700">
                No. MPC nodes only generate the derivation key for the keystore when requested by the DAO contract - they never
                see user secrets. Importantly, MPC Network only responds to requests that come through the DAO contract transaction,
                not direct requests. The DAO governs which keystores can receive derivation keys but has no access to the keys themselves.
                User secrets are encrypted and only the keystore (running in TEE) can decrypt them. No entity outside the TEE ever has access to plaintext secrets.
              </p>
            </details>

            <details className="border rounded-lg p-4">
              <summary className="font-semibold cursor-pointer text-gray-900">
                How is this different from regular key storage?
              </summary>
              <p className="mt-2 text-gray-700">
                Traditional systems either store keys (security risk) or generate random keys that are lost on restart.
                CKD provides deterministic secrets through distributed computation - persistent yet secure, distributed
                yet accessible, a unique combination enabled by MPC and TEE technologies.
              </p>
            </details>

            <details className="border rounded-lg p-4">
              <summary className="font-semibold cursor-pointer text-gray-900">
                What prevents unauthorized access to secrets?
              </summary>
              <p className="mt-2 text-gray-700">
                Multiple layers: (1) DAO governance controls which keystores can receive derivation keys,
                (2) TEE attestation verification ensures only genuine TEE apps run the keystore,
                (3) MPC Network only responds to requests from DAO contract (not direct requests),
                (4) Threshold cryptography requires multiple MPC nodes to cooperate,
                (5) All cryptographic operations happen inside TEE enclave.
              </p>
            </details>

            <details className="border rounded-lg p-4">
              <summary className="font-semibold cursor-pointer text-gray-900">
                Why use BLS signatures on BLS12-381 curves?
              </summary>
              <p className="mt-2 text-gray-700">
                BLS signatures provide unique properties: deterministic, aggregatable, and efficient verification.
                BLS12-381 is a pairing-friendly curve specifically designed for cryptographic protocols, offering
                128-bit security with optimal performance for threshold cryptography and MPC operations.
              </p>
            </details>
          </div>
        </section>
      </div>
    </div>
  );
}

export function PricingSection() {
  useHashNavigation();

  return (
    <div className="prose max-w-none">
      <h2 className="text-3xl font-bold mb-6 text-[var(--primary-orange)]">Pricing & Limits</h2>

      <div className="space-y-6">
        <section id="dynamic-pricing">
          <AnchorHeading id="dynamic-pricing">Dynamic Pricing Model</AnchorHeading>
          <p className="text-gray-700">
            Pay only for resources you use. Pricing is based on requested resource limits, not fixed fees.
            Excess payment is automatically refunded after execution.
          </p>
        </section>

        <section id="cost-calculation">
          <AnchorHeading id="cost-calculation">Cost Calculation</AnchorHeading>
          <p className="text-gray-700">
            Execution cost = <code className="bg-gray-100 px-2 py-1 rounded">base_fee + (instructions × instruction_rate) + (time_ms × time_rate)</code>
          </p>
          <p className="text-gray-700 mt-2">
            Use the <code className="bg-gray-100 px-2 py-1 rounded">estimate_execution_cost</code> view method to calculate
            costs before submitting a request.
          </p>
        </section>

        <section id="resource-limits">
          <AnchorHeading id="resource-limits">Resource Limits</AnchorHeading>
          <ul className="list-disc list-inside space-y-2 text-gray-700">
            <li><strong>Max Instructions:</strong> 100 billion instructions per execution</li>
            <li><strong>Max Memory:</strong> Configurable up to platform limits</li>
            <li><strong>Max Execution Time:</strong> 60 seconds per execution</li>
            <li><strong>Max Compilation Time:</strong> Enforced during GitHub compilation</li>
          </ul>
        </section>

        <section id="refund-policy">
          <AnchorHeading id="refund-policy">Refund Policy</AnchorHeading>
          <p className="text-gray-700">
            If your execution uses less resources than requested, the difference is automatically refunded.
            However, failed executions are not refunded (anti-DoS protection).
          </p>
        </section>

        <section id="optimization-tips">
          <AnchorHeading id="optimization-tips">Optimization Tips</AnchorHeading>
          <ul className="list-disc list-inside space-y-2 text-gray-700">
            <li>Request only the resources you need to minimize upfront costs</li>
            <li>Optimize your WASM code to reduce instruction count</li>
            <li>Use immutable WASM storage to avoid repeated compilation costs</li>
            <li>Consider caching results in your smart contract for frequently-accessed data</li>
          </ul>
        </section>
      </div>
    </div>
  );
}

export function ArchitectureSection() {
  useHashNavigation();

  return (
    <div className="prose max-w-none">
      <h2 className="text-3xl font-bold mb-6 text-[var(--primary-orange)]">Architecture</h2>

      <div className="space-y-6">
        <section id="system-components">
          <AnchorHeading id="system-components">System Components</AnchorHeading>
          <ul className="list-disc list-inside space-y-2 text-gray-700">
            <li><strong>Smart Contract:</strong> On-chain state management, payment handling, and event emission (outlayer.near / outlayer.testnet)</li>
            <li><strong>Coordinator API:</strong> Task queue management, WASM caching, and distributed locking</li>
            <li><strong>Workers:</strong> Event monitoring, WASM compilation, and secure execution in TEE</li>
            <li><strong>Keystore:</strong> Secret management, encryption/decryption, and access control validation</li>
          </ul>
        </section>

        <section id="execution-flow">
          <AnchorHeading id="execution-flow">Execution Flow</AnchorHeading>
          <ol className="list-decimal list-inside space-y-2 text-gray-700">
            <li>Smart contract calls <code className="bg-gray-100 px-2 py-1 rounded">outlayer.near</code> / <code className="bg-gray-100 px-2 py-1 rounded">outlayer.testnet</code> with execution request</li>
            <li>Contract emits event and enters yield state</li>
            <li>Workers detect event and fetch task from coordinator</li>
            <li>Worker compiles WASM from GitHub (or loads from cache/immutable storage)</li>
            <li>Worker decrypts secrets (if provided) via keystore</li>
            <li>Worker executes WASM in TEE with input data and secrets</li>
            <li>Worker submits result and metrics back to contract</li>
            <li>Contract resumes with result, calculates actual cost, refunds excess, and calls back to original caller</li>
          </ol>
        </section>

        <section id="security-guarantees">
          <AnchorHeading id="security-guarantees">Security Guarantees</AnchorHeading>
          <ul className="list-disc list-inside space-y-2 text-gray-700">
            <li><strong>TEE Execution:</strong> Code runs in Trusted Execution Environments with attestation</li>
            <li><strong>Encrypted Secrets:</strong> Secrets are encrypted at rest and decrypted only in TEE</li>
            <li><strong>Resource Limits:</strong> Hard caps prevent DoS attacks and excessive resource usage</li>
            <li><strong>On-Chain Settlement:</strong> All results are verified and finalized on NEAR blockchain</li>
          </ul>
        </section>

        <section id="scalability">
          <AnchorHeading id="scalability">Scalability</AnchorHeading>
          <p className="text-gray-700">
            OutLayer scales horizontally by adding more workers. Workers are stateless and coordinate through the
            Coordinator API. Task distribution is handled via Redis queues with automatic load balancing.
          </p>
        </section>

        <section id="wasm-caching">
          <AnchorHeading id="wasm-caching">WASM Caching Strategy</AnchorHeading>
          <ul className="list-disc list-inside space-y-2 text-gray-700">
            <li><strong>First execution:</strong> Compile from GitHub (10-300 seconds)</li>
            <li><strong>Subsequent executions:</strong> Load from coordinator cache (~1-2 seconds)</li>
            <li><strong>Immutable storage:</strong> Load from on-chain storage (~2-3 seconds)</li>
            <li><strong>LRU eviction:</strong> Old unused WASM files are automatically removed</li>
          </ul>
        </section>

        <section id="high-availability">
          <AnchorHeading id="high-availability">High Availability</AnchorHeading>
          <p className="text-gray-700">
            Multiple independent workers monitor for events. If one worker fails, others can pick up the task.
            Automatic failover ensures execution continues even if some workers are offline.
          </p>
        </section>
      </div>
    </div>
  );
}
