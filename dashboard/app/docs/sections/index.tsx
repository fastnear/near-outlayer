import Link from 'next/link';

export { default as GettingStartedSection } from './GettingStarted';
export { default as DeveloperGuideSection } from './DeveloperGuide';

export function ContractIntegrationSection() {
  return (
    <div className="prose max-w-none">
      <h2 className="text-3xl font-bold mb-6 text-[var(--primary-orange)]">Contract Integration</h2>

      <div className="space-y-6">
        <section>
          <h3 className="text-xl font-semibold mb-3">Basic Flow</h3>
          <p className="text-gray-700">
            Your smart contract calls <code className="bg-gray-100 px-2 py-1 rounded">outlayer.near</code> (mainnet) or <code className="bg-gray-100 px-2 py-1 rounded">outlayer.testnet</code> using
            the <code className="bg-gray-100 px-2 py-1 rounded">request_execution</code> method. After off-chain execution,
            OutLayer calls back to your contract with the result. Final settlement stays on NEAR Layer 1 for security.
          </p>
        </section>

        <section>
          <h3 className="text-xl font-semibold mb-3">Request Parameters</h3>
          <ul className="list-disc list-inside space-y-2 text-gray-700">
            <li><strong>code_source:</strong> GitHub repository URL, commit/branch, and build target (wasm32-wasip1/wasip2)</li>
            <li><strong>input_data:</strong> JSON string passed to your WASM code as stdin</li>
            <li><strong>resource_limits:</strong> Max instructions, memory, and execution time</li>
            <li><strong>secrets_ref (optional):</strong> Reference to encrypted secrets stored on-chain</li>
            <li><strong>payer_account_id (optional):</strong> Specify who receives the refund - pass user&apos;s account to let them pay, or None to pay from contract balance</li>
            <li><strong>response_format:</strong> &quot;Json&quot; to parse result as JSON, or &quot;String&quot; for raw output</li>
          </ul>
        </section>

        <section>
          <h3 className="text-xl font-semibold mb-3">Callback Handling</h3>
          <p className="text-gray-700">
            Implement a callback function in your contract to receive the execution result. The callback receives
            the output data, execution status, and resource metrics.
          </p>
        </section>

        <section>
          <h3 className="text-xl font-semibold mb-3">Error Handling</h3>
          <p className="text-gray-700">
            Handle execution failures gracefully. Common failure reasons include: compilation errors, execution timeout,
            resource limit exceeded, or worker unavailability.
          </p>
        </section>

        <section>
          <h3 className="text-xl font-semibold mb-3">Immutable WASM Storage</h3>
          <p className="text-gray-700">
            For production use, consider storing your compiled WASM on-chain. This provides instant execution (1 second vs 2-3 minutes),
            eliminates GitHub dependencies, and ensures DAO-governed updates.
          </p>
        </section>
      </div>
    </div>
  );
}

export function WasiSection() {
  return (
    <div className="prose max-w-none">
      <h2 className="text-3xl font-bold mb-6 text-[var(--primary-orange)]">Writing WASI Code</h2>

      <div className="space-y-6">
        <section>
          <h3 className="text-xl font-semibold mb-3">Supported Languages</h3>
          <p className="text-gray-700">
            Any language that compiles to WASM with WASI support: Rust, C/C++, Go, AssemblyScript, and more.
            Rust is recommended for best tooling and ecosystem support.
          </p>
        </section>

        <section>
          <h3 className="text-xl font-semibold mb-3">WASI Interface</h3>
          <p className="text-gray-700">
            OutLayer provides a minimal WASI environment with support for:
          </p>
          <ul className="list-disc list-inside space-y-2 text-gray-700 mt-2">
            <li><strong>stdin/stdout:</strong> Read input data, write output results</li>
            <li><strong>Environment variables:</strong> Access encrypted secrets via <code className="bg-gray-100 px-2 py-1 rounded">std::env::var()</code></li>
            <li><strong>Random numbers:</strong> Cryptographically secure random generation</li>
            <li><strong>File I/O (limited):</strong> Basic file operations in sandboxed environment</li>
          </ul>
        </section>

        <section>
          <h3 className="text-xl font-semibold mb-3">Input/Output Format</h3>
          <p className="text-gray-700">
            Your WASM code reads JSON input from stdin and writes JSON output to stdout. This makes integration
            simple and language-agnostic.
          </p>
        </section>

        <section>
          <h3 className="text-xl font-semibold mb-3">Build Configuration</h3>
          <p className="text-gray-700">
            Use <code className="bg-gray-100 px-2 py-1 rounded">wasm32-wasip1</code> or <code className="bg-gray-100 px-2 py-1 rounded">wasm32-wasip2</code> as
            build targets.
          </p>
        </section>

        <section>
          <h3 className="text-xl font-semibold mb-3">Resource Considerations</h3>
          <p className="text-gray-700">
            Be mindful of resource limits: instruction counts, memory usage, and execution time. Optimize your code
            to stay within requested limits to avoid failures and minimize costs.
          </p>
        </section>

        <section>
          <h3 className="text-xl font-semibold mb-3">Testing Locally</h3>
          <p className="text-gray-700">
            Test your WASM code locally using <code className="bg-gray-100 px-2 py-1 rounded">wasmtime</code> or similar
            WASI runtimes before deploying to OutLayer. This helps catch issues early.
          </p>
        </section>
      </div>
    </div>
  );
}

export function SecretsSection() {
  return (
    <div className="prose max-w-none">
      <h2 className="text-3xl font-bold mb-6 text-[var(--primary-orange)]">Managing Secrets</h2>

      <div className="space-y-6">
        <section>
          <h3 className="text-xl font-semibold mb-3">What are Secrets?</h3>
          <p className="text-gray-700">
            Secrets are encrypted API keys, tokens, or sensitive data stored on-chain. They are automatically decrypted
            and injected as environment variables when your WASM code executes.
          </p>
        </section>

        <section>
          <h3 className="text-xl font-semibold mb-3">Creating Secrets</h3>
          <p className="text-gray-700">
            Use the <Link href="/secrets" className="text-[var(--primary-orange)] hover:underline">Secrets</Link> page
            to create encrypted secrets. Specify repository, branch (optional), and profile name. Secrets are encrypted
            client-side before being stored on-chain.
          </p>
        </section>

        <section>
          <h3 className="text-xl font-semibold mb-3">Access Control</h3>
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

        <section>
          <h3 className="text-xl font-semibold mb-3">Using Secrets in Code</h3>
          <p className="text-gray-700">
            Access secrets in your WASM code using standard environment variable functions. In Rust:
            <code className="bg-gray-100 px-2 py-1 rounded ml-1">std::env::var(&quot;API_KEY&quot;)</code>
          </p>
        </section>

        <section>
          <h3 className="text-xl font-semibold mb-3">Storage Costs</h3>
          <p className="text-gray-700">
            Secrets storage costs are proportional to data size plus indexing overhead (~64 bytes). Storage fees
            are refunded when you delete secrets.
          </p>
        </section>

        <section>
          <h3 className="text-xl font-semibold mb-3">Security Model</h3>
          <p className="text-gray-700">
            Secrets are encrypted with XOR (MVP phase) and will be upgraded to ChaCha20-Poly1305 in production.
            Decryption happens in TEE workers with attestation verification. Your secrets never leave the secure enclave.
          </p>
        </section>
      </div>
    </div>
  );
}

export function PricingSection() {
  return (
    <div className="prose max-w-none">
      <h2 className="text-3xl font-bold mb-6 text-[var(--primary-orange)]">Pricing & Limits</h2>

      <div className="space-y-6">
        <section>
          <h3 className="text-xl font-semibold mb-3">Dynamic Pricing Model</h3>
          <p className="text-gray-700">
            Pay only for resources you use. Pricing is based on requested resource limits, not fixed fees.
            Excess payment is automatically refunded after execution.
          </p>
        </section>

        <section>
          <h3 className="text-xl font-semibold mb-3">Cost Calculation</h3>
          <p className="text-gray-700">
            Execution cost = <code className="bg-gray-100 px-2 py-1 rounded">base_fee + (instructions × instruction_rate) + (time_ms × time_rate)</code>
          </p>
          <p className="text-gray-700 mt-2">
            Use the <code className="bg-gray-100 px-2 py-1 rounded">estimate_execution_cost</code> view method to calculate
            costs before submitting a request.
          </p>
        </section>

        <section>
          <h3 className="text-xl font-semibold mb-3">Resource Limits</h3>
          <ul className="list-disc list-inside space-y-2 text-gray-700">
            <li><strong>Max Instructions:</strong> 100 billion instructions per execution</li>
            <li><strong>Max Memory:</strong> Configurable up to platform limits</li>
            <li><strong>Max Execution Time:</strong> 60 seconds per execution</li>
            <li><strong>Max Compilation Time:</strong> Enforced during GitHub compilation</li>
          </ul>
        </section>

        <section>
          <h3 className="text-xl font-semibold mb-3">Refund Policy</h3>
          <p className="text-gray-700">
            If your execution uses less resources than requested, the difference is automatically refunded.
            However, failed executions are not refunded (anti-DoS protection).
          </p>
        </section>

        <section>
          <h3 className="text-xl font-semibold mb-3">Optimization Tips</h3>
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
  return (
    <div className="prose max-w-none">
      <h2 className="text-3xl font-bold mb-6 text-[var(--primary-orange)]">Architecture</h2>

      <div className="space-y-6">
        <section>
          <h3 className="text-xl font-semibold mb-3">System Components</h3>
          <ul className="list-disc list-inside space-y-2 text-gray-700">
            <li><strong>Smart Contract:</strong> On-chain state management, payment handling, and event emission (outlayer.near / outlayer.testnet)</li>
            <li><strong>Coordinator API:</strong> Task queue management, WASM caching, and distributed locking</li>
            <li><strong>Workers:</strong> Event monitoring, WASM compilation, and secure execution in TEE</li>
            <li><strong>Keystore:</strong> Secret management, encryption/decryption, and access control validation</li>
          </ul>
        </section>

        <section>
          <h3 className="text-xl font-semibold mb-3">Execution Flow</h3>
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

        <section>
          <h3 className="text-xl font-semibold mb-3">Security Guarantees</h3>
          <ul className="list-disc list-inside space-y-2 text-gray-700">
            <li><strong>TEE Execution:</strong> Code runs in Trusted Execution Environments with attestation</li>
            <li><strong>Encrypted Secrets:</strong> Secrets are encrypted at rest and decrypted only in TEE</li>
            <li><strong>Resource Limits:</strong> Hard caps prevent DoS attacks and excessive resource usage</li>
            <li><strong>On-Chain Settlement:</strong> All results are verified and finalized on NEAR blockchain</li>
          </ul>
        </section>

        <section>
          <h3 className="text-xl font-semibold mb-3">Scalability</h3>
          <p className="text-gray-700">
            OutLayer scales horizontally by adding more workers. Workers are stateless and coordinate through the
            Coordinator API. Task distribution is handled via Redis queues with automatic load balancing.
          </p>
        </section>

        <section>
          <h3 className="text-xl font-semibold mb-3">WASM Caching Strategy</h3>
          <ul className="list-disc list-inside space-y-2 text-gray-700">
            <li><strong>First execution:</strong> Compile from GitHub (10-300 seconds)</li>
            <li><strong>Subsequent executions:</strong> Load from coordinator cache (~1-2 seconds)</li>
            <li><strong>Immutable storage:</strong> Load from on-chain storage (~2-3 seconds)</li>
            <li><strong>LRU eviction:</strong> Old unused WASM files are automatically removed</li>
          </ul>
        </section>

        <section>
          <h3 className="text-xl font-semibold mb-3">High Availability</h3>
          <p className="text-gray-700">
            Multiple independent workers monitor for events. If one worker fails, others can pick up the task.
            Automatic failover ensures execution continues even if some workers are offline.
          </p>
        </section>
      </div>
    </div>
  );
}
