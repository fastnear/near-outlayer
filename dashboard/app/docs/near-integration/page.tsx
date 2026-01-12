'use client';

import Link from 'next/link';
import { Prism as SyntaxHighlighter } from 'react-syntax-highlighter';
import { vscDarkPlus } from 'react-syntax-highlighter/dist/esm/styles/prism';

function AnchorHeading({ id, children, level = 2 }: { id: string; children: React.ReactNode; level?: 2 | 3 | 4 }) {
  const sizeClass = level === 2 ? 'text-2xl' : level === 3 ? 'text-xl' : 'text-lg';
  const className = `${sizeClass} font-bold text-gray-900 mb-4 scroll-mt-4 group`;
  const anchor = (
    <a href={`#${id}`} className="ml-2 text-gray-400 hover:text-[var(--primary-orange)] opacity-0 group-hover:opacity-100 transition-opacity">
      #
    </a>
  );

  if (level === 3) return <h3 id={id} className={className}>{children}{anchor}</h3>;
  if (level === 4) return <h4 id={id} className={className}>{children}{anchor}</h4>;
  return <h2 id={id} className={className}>{children}{anchor}</h2>;
}

export default function NearIntegrationPage() {
  return (
    <div className="prose prose-lg max-w-none">
      <h1 className="text-3xl font-bold text-gray-900 mb-6">
        NEAR Integration
      </h1>

      <p className="text-gray-700 mb-6">
        Call OutLayer from your <strong>NEAR smart contract</strong> to execute off-chain WASM code and receive the result via callback.
        You stay in full control — no need to grant special access or delegate permissions to workers.
      </p>

      <div className="bg-blue-50 border-l-4 border-blue-400 p-4 mb-8">
        <p className="text-sm text-blue-800">
          <strong>For Web2 projects</strong> (web apps, mobile apps, APIs) see{' '}
          <Link href="/docs/web2-integration" className="underline font-medium">Web2 Integration</Link> —
          call OutLayer via HTTPS without blockchain transactions.
        </p>
      </div>

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
                <p className="font-mono text-sm text-gray-800 mb-1"><strong>source</strong>: ExecutionSource <span className="text-red-600">(required)</span></p>
                <p className="text-sm text-gray-600 mb-2">Specifies where to get WASM code. Three variants available:</p>

                <div className="mt-2 mb-2">
                  <p className="text-sm font-semibold text-gray-800 mb-1">Variant 1: GitHub (compile from source)</p>
                  <ul className="list-disc list-inside text-sm text-gray-700 ml-4 space-y-1">
                    <li><code className="bg-gray-100 px-2 py-1 rounded">repo</code>: GitHub repository URL (e.g., &quot;https://github.com/user/project&quot;)</li>
                    <li><code className="bg-gray-100 px-2 py-1 rounded">commit</code>: Branch name (&quot;main&quot;) or commit hash (40-char SHA)</li>
                    <li><code className="bg-gray-100 px-2 py-1 rounded">build_target</code>: Optional. &quot;wasm32-wasip1&quot; or &quot;wasm32-wasip2&quot; (default: wasip1)</li>
                  </ul>
                </div>

                <div className="mt-2 mb-2">
                  <p className="text-sm font-semibold text-gray-800 mb-1">Variant 2: WasmUrl (pre-compiled WASM)</p>
                  <ul className="list-disc list-inside text-sm text-gray-700 ml-4 space-y-1">
                    <li><code className="bg-gray-100 px-2 py-1 rounded">url</code>: URL to pre-compiled WASM file (FastFS, IPFS, etc.)</li>
                    <li><code className="bg-gray-100 px-2 py-1 rounded">hash</code>: SHA256 hash of WASM file for verification</li>
                    <li><code className="bg-gray-100 px-2 py-1 rounded">build_target</code>: &quot;wasm32-wasip1&quot; or &quot;wasm32-wasip2&quot;</li>
                  </ul>
                  <p className="text-xs text-gray-500 mt-1">Use WasmUrl for instant execution without compilation. Ideal for closed-source WASM or permanent deployments on FastFS/IPFS.</p>
                </div>

                <div className="mt-2">
                  <p className="text-sm font-semibold text-gray-800 mb-1">Variant 3: Project (registered project)</p>
                  <ul className="list-disc list-inside text-sm text-gray-700 ml-4 space-y-1">
                    <li><code className="bg-gray-100 px-2 py-1 rounded">project_id</code>: Project ID (e.g., &quot;alice.near/my-app&quot;)</li>
                    <li><code className="bg-gray-100 px-2 py-1 rounded">version_key</code>: Optional. Specific version key (null = active version)</li>
                  </ul>
                  <p className="text-xs text-gray-500 mt-1">Use Project for registered apps with persistent storage. Automatically uses project secrets if available.</p>
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
                <p className="text-xs text-gray-500 mt-2">Useful for contracts that want users to pay for their own executions</p>
              </div>

              <div className="border-l-4 border-purple-400 pl-4">
                <p className="font-mono text-sm text-gray-800 mb-1"><strong>params</strong>: ExecutionParams <span className="text-gray-500">(optional)</span></p>
                <p className="text-sm text-gray-600 mb-2">Advanced execution control parameters:</p>
                <ul className="list-disc list-inside text-sm text-gray-700 ml-4 space-y-1">
                  <li><code className="bg-gray-100 px-2 py-1 rounded">force_rebuild</code>: bool (default: false) - Force recompilation even if WASM exists in cache. Useful when you updated code but kept the same commit hash, or need a fresh build for debugging.</li>
                  <li><code className="bg-gray-100 px-2 py-1 rounded">compile_only</code>: bool (default: false) - Only compile, don&apos;t execute. Returns checksum of compiled WASM. Useful for pre-warming cache or verifying compilation.</li>
                  <li><code className="bg-gray-100 px-2 py-1 rounded">store_on_fastfs</code>: bool (default: false) - Store compiled WASM on FastFS after compilation. Returns FastFS URL instead of execution result. Useful for permanent storage and sharing.</li>
                </ul>
                <p className="text-xs text-gray-500 mt-2">Example: <code className="bg-gray-100 px-1 rounded">{`"params": {"force_rebuild": true, "compile_only": false, "store_on_fastfs": false}`}</code></p>
              </div>
            </div>
          </div>
        </section>

        <section id="callback">
          <AnchorHeading id="callback">What You Get Back (Callback)</AnchorHeading>
          <p className="text-gray-700 mb-3">
            OutLayer reads <strong>stdout</strong> from your WASM code and returns it to your contract&apos;s callback.
            You can write anything to stdout — a number, text, JSON object — and parse it however you want.
          </p>
          <ul className="list-disc list-inside space-y-2 text-gray-700 ml-4">
            <li><strong>Success case</strong>: Your stdout output (parsed according to response_format: Text/Json/Bytes)</li>
            <li><strong>Failure case</strong>: Error information (compilation failed, timeout, etc.)</li>
            <li><strong>Resource metrics</strong>: Actual instructions used, execution time</li>
            <li><strong>Automatic refund</strong>: Unused deposit sent to payer_account_id</li>
          </ul>
          <div className="bg-yellow-50 border-l-4 border-yellow-400 p-3 mt-3">
            <p className="text-sm text-gray-700">
              <strong>Important:</strong> You stay in full control. OutLayer just executes your code and returns stdout — no special permissions needed on your contract. You decide what to do with the results in your callback logic.
            </p>
          </div>
        </section>

        <section id="example">
          <AnchorHeading id="example">Example Contract Call</AnchorHeading>

          <SyntaxHighlighter language="rust" style={vscDarkPlus} className="rounded-lg mb-4">
            {`use near_sdk::{near, env, Promise, Gas};
use serde_json::json;

#[near(contract_state)]
pub struct Contract {}

#[near]
impl Contract {
    pub fn call_outlayer(&mut self, city: String) -> Promise {
        // Call OutLayer with a project
        Promise::new("outlayer.near".parse().unwrap())
            .function_call(
                "request_execution".to_string(),
                json!({
                    "source": {
                        "Project": {
                            "project_id": "alice.near/weather-api"
                        }
                    },
                    "input_data": json!({"city": city}).to_string(),
                    "resource_limits": {
                        "max_instructions": 1_000_000_000,
                        "max_execution_seconds": 30
                    }
                }).to_string().into_bytes(),
                env::attached_deposit(),
                Gas::from_tgas(100)
            )
            .then(
                Self::ext(env::current_account_id())
                    .with_static_gas(Gas::from_tgas(10))
                    .on_outlayer_result()
            )
    }

    #[private]
    pub fn on_outlayer_result(&mut self) {
        // Handle the result from OutLayer
        match env::promise_result(0) {
            near_sdk::PromiseResult::Successful(data) => {
                let result: String = String::from_utf8(data).unwrap();
                env::log_str(&format!("Weather result: {}", result));
            }
            _ => {
                env::log_str("OutLayer execution failed");
            }
        }
    }
}`}
          </SyntaxHighlighter>
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
              <strong>Execution time:</strong> Depends on your code — from milliseconds to minutes based on complexity and resource limits
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
            <li><strong>Execute via WasmUrl:</strong> Use the URL and hash in <code className="bg-gray-100 px-1 rounded">source</code> for instant execution</li>
          </ol>
          <div className="bg-green-50 border-l-4 border-green-400 p-3 mt-3">
            <p className="text-sm text-gray-700">
              <strong>Benefits:</strong> Instant execution (no compilation), immutable code (hash-verified), works with closed-source WASM, permanent storage on-chain.
            </p>
          </div>
        </section>

        <section id="tee-attestation">
          <AnchorHeading id="tee-attestation">TEE Attestation</AnchorHeading>
          <p className="text-gray-700 mb-3">
            Every execution produces a cryptographic attestation from Intel TDX proving:
          </p>
          <ul className="list-disc list-inside space-y-2 text-gray-700 ml-4">
            <li><strong>Code integrity</strong> — Exact WASM binary that ran (SHA256 hash)</li>
            <li><strong>Input integrity</strong> — Exact input data received</li>
            <li><strong>Output integrity</strong> — Result was produced by that code with that input</li>
            <li><strong>Worker identity</strong> — Registered TEE worker with verified measurements</li>
          </ul>
          <p className="text-gray-700 mt-3">
            View attestations at <Link href="/executions" className="text-[var(--primary-orange)] hover:underline">/executions</Link> → &quot;View Attestation&quot;.
            Learn more: <Link href="/docs/tee-attestation" className="text-[var(--primary-orange)] hover:underline">TEE Attestation Documentation</Link>
          </p>
        </section>

        <section className="bg-gray-50 rounded-lg p-6 mt-8">
          <h3 className="text-lg font-semibold text-gray-900 mb-4">Related Documentation</h3>
          <ul className="space-y-2">
            <li>
              <Link href="/docs/projects" className="text-[var(--primary-orange)] hover:underline">Projects & Storage</Link>
              {' '}- Version management, persistent storage
            </li>
            <li>
              <Link href="/docs/secrets" className="text-[var(--primary-orange)] hover:underline">Secrets</Link>
              {' '}- Store API keys for your WASM code
            </li>
            <li>
              <Link href="/docs/pricing" className="text-[var(--primary-orange)] hover:underline">Pricing</Link>
              {' '}- Cost calculation and resource limits
            </li>
            <li>
              <Link href="/docs/examples" className="text-[var(--primary-orange)] hover:underline">Examples</Link>
              {' '}- Working code examples
            </li>
          </ul>
        </section>
      </div>
    </div>
  );
}
