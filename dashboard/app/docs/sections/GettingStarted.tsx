import Link from 'next/link';

export default function GettingStartedSection() {
  return (
    <div className="prose max-w-none">
      <h2 className="text-3xl font-bold mb-6 text-[var(--primary-orange)]">Getting Started</h2>

      <div className="space-y-8">
        <section id="what-is-outlayer">
          <h3 className="text-xl font-semibold mb-3">What is OutLayer?</h3>
          <p className="text-gray-700 mb-3">
            OutLayer lets you run <strong>any off-chain code</strong> (random numbers, HTTP requests, AI models, heavy computations)
            and get the result back in your NEAR smart contract using NEAR&apos;s <strong>yield/resume mechanism</strong>.
          </p>
          <p className="text-gray-700">
            You don&apos;t need to build infrastructure, manage workers, or deal with access control. Just write your code,
            push to GitHub, and call it from your contract. OutLayer handles compilation, execution, and returning results.
          </p>
        </section>

        <section id="how-yield-resume-works">
          <h3 className="text-xl font-semibold mb-3">How Yield/Resume Works on NEAR</h3>
          <p className="text-gray-700 mb-3">
            NEAR Protocol&apos;s yield/resume feature allows smart contracts to <strong>pause execution</strong>, wait for external
            computation to complete, then <strong>resume with the result</strong> - all within a single logical transaction.
          </p>

          {/* Desktop Diagram - Hidden on mobile */}
          <div className="hidden md:block bg-white border-2 border-gray-300 rounded-lg p-6 mb-4 overflow-x-auto">
            <svg viewBox="0 0 800 600" className="w-full" style={{ maxWidth: '800px', margin: '0 auto' }}>
              {/* Participant boxes */}
              <rect x="50" y="20" width="100" height="50" fill="#3b82f6" rx="8" />
              <text x="100" y="50" textAnchor="middle" fill="white" fontSize="14" fontWeight="bold">User</text>

              <rect x="250" y="20" width="120" height="50" fill="#a855f7" rx="8" />
              <text x="310" y="40" textAnchor="middle" fill="white" fontSize="12" fontWeight="bold">Your</text>
              <text x="310" y="55" textAnchor="middle" fill="white" fontSize="12" fontWeight="bold">Contract</text>

              <rect x="450" y="20" width="120" height="50" fill="#f97316" rx="8" />
              <text x="510" y="40" textAnchor="middle" fill="white" fontSize="12" fontWeight="bold">OutLayer</text>
              <text x="510" y="55" textAnchor="middle" fill="white" fontSize="12" fontWeight="bold">Contract</text>

              <rect x="650" y="20" width="100" height="50" fill="#16a34a" rx="8" />
              <text x="700" y="50" textAnchor="middle" fill="white" fontSize="14" fontWeight="bold">TEE Worker</text>

              {/* Lifelines */}
              <line x1="100" y1="70" x2="100" y2="570" stroke="#d1d5db" strokeWidth="2" strokeDasharray="5,5" />
              <line x1="310" y1="70" x2="310" y2="570" stroke="#d1d5db" strokeWidth="2" strokeDasharray="5,5" />
              <line x1="510" y1="70" x2="510" y2="570" stroke="#d1d5db" strokeWidth="2" strokeDasharray="5,5" />
              <line x1="700" y1="70" x2="700" y2="570" stroke="#d1d5db" strokeWidth="2" strokeDasharray="5,5" />

              {/* Step 1: User -> Your Contract */}
              <defs>
                <marker id="arrowBlue" markerWidth="10" markerHeight="10" refX="9" refY="3" orient="auto" markerUnits="strokeWidth">
                  <path d="M0,0 L0,6 L9,3 z" fill="#3b82f6" />
                </marker>
                <marker id="arrowPurple" markerWidth="10" markerHeight="10" refX="9" refY="3" orient="auto" markerUnits="strokeWidth">
                  <path d="M0,0 L0,6 L9,3 z" fill="#a855f7" />
                </marker>
                <marker id="arrowOrange" markerWidth="10" markerHeight="10" refX="9" refY="3" orient="auto" markerUnits="strokeWidth">
                  <path d="M0,0 L0,6 L9,3 z" fill="#f97316" />
                </marker>
                <marker id="arrowGreen" markerWidth="10" markerHeight="10" refX="9" refY="3" orient="auto" markerUnits="strokeWidth">
                  <path d="M0,0 L0,6 L9,3 z" fill="#16a34a" />
                </marker>
              </defs>

              <line x1="100" y1="100" x2="310" y2="100" stroke="#3b82f6" strokeWidth="2" markerEnd="url(#arrowBlue)" />
              <text x="205" y="95" textAnchor="middle" fontSize="11" fill="#1e40af">1. call contract</text>

              {/* Step 2: Your Contract -> OutLayer */}
              <line x1="310" y1="140" x2="510" y2="140" stroke="#a855f7" strokeWidth="2" markerEnd="url(#arrowPurple)" />
              <text x="410" y="135" textAnchor="middle" fontSize="11" fill="#7e22ce" fontWeight="bold">2. request_execution()</text>
              <text x="410" y="150" textAnchor="middle" fontSize="9" fill="#6b21a8">(repo, input_data, limits, secrets)</text>

              {/* YIELD STATE box */}
              <rect x="450" y="160" width="120" height="40" fill="#fef3c7" stroke="#f59e0b" strokeWidth="2" rx="4" />
              <text x="510" y="175" textAnchor="middle" fontSize="10" fontWeight="bold" fill="#92400e">⏸️ YIELD STATE</text>
              <text x="510" y="190" textAnchor="middle" fontSize="9" fill="#92400e">Transaction paused</text>

              {/* Step 3: OutLayer -> Worker */}
              <line x1="510" y1="230" x2="700" y2="230" stroke="#f97316" strokeWidth="2" markerEnd="url(#arrowOrange)" />
              <text x="605" y="225" textAnchor="middle" fontSize="11" fill="#c2410c">3. emit event</text>

              {/* Worker execution box */}
              <rect x="630" y="250" width="140" height="100" fill="#f0fdf4" stroke="#16a34a" strokeWidth="2" rx="4" />
              <text x="700" y="268" textAnchor="middle" fontSize="10" fontWeight="bold" fill="#166534">🔨 Off-chain work:</text>
              <text x="700" y="283" textAnchor="middle" fontSize="9" fill="#166534">• Clone GitHub</text>
              <text x="700" y="298" textAnchor="middle" fontSize="9" fill="#166534">• Compile Rust</text>
              <text x="700" y="313" textAnchor="middle" fontSize="9" fill="#166534">• Execute WASM</text>
              <text x="700" y="328" textAnchor="middle" fontSize="9" fill="#166534">• Read stdout</text>
              <text x="700" y="343" textAnchor="middle" fontSize="8" fill="#6b7280" fontStyle="italic">(10 sec - few min)</text>              

              {/* Step 5: OutLayer -> Your Contract */}
              <line x1="700" y1="370" x2="510" y2="370" stroke="#f97316" strokeWidth="2" markerEnd="url(#arrowOrange)" />
              <text x="605" y="385" textAnchor="middle" fontSize="11" fill="#c2410c">4. yield_resume()</text>

              {/* RESUME box */}
              <rect x="450" y="400" width="120" height="40" fill="#dcfce7" stroke="#16a34a" strokeWidth="2" rx="4" />
              <text x="510" y="415" textAnchor="middle" fontSize="10" fontWeight="bold" fill="#166534">▶️ RESUME</text>
              <text x="510" y="430" textAnchor="middle" fontSize="9" fill="#166534">Process result</text>

              {/* Step 4: Worker -> OutLayer */}
              <line x1="510" y1="470" x2="310" y2="470" stroke="#16a34a" strokeWidth="2" markerEnd="url(#arrowGreen)" />
              <text x="410" y="465" textAnchor="middle" fontSize="11" fill="#15803d">5. return result</text>

              {/* Final result */}
              <rect x="100" y="500" width="400" height="40" fill="#dcfce7" stroke="#16a34a" strokeWidth="2" rx="8" />
              <text x="300" y="525" textAnchor="middle" fontSize="12" fontWeight="bold" fill="#166534">✅ Transaction Complete - User receives final result</text>
            </svg>
          </div>

          {/* Mobile Simplified Diagram */}
          <div className="md:hidden bg-gradient-to-b from-gray-50 to-blue-50 border-2 border-gray-300 rounded-lg p-4 mb-4">
            <div className="space-y-3 text-sm">
              {/* Step 1 */}
              <div className="flex items-center gap-2">
                <div className="flex-shrink-0 w-8 h-8 bg-blue-500 rounded-full flex items-center justify-center text-white font-bold text-xs">1</div>
                <div className="flex items-center gap-2 flex-1">
                  <span className="font-semibold text-blue-800">User</span>
                  <span className="text-gray-600">→</span>
                  <span className="font-semibold text-purple-800">Contract</span>
                </div>
              </div>

              {/* Step 2 */}
              <div className="flex items-center gap-2">
                <div className="flex-shrink-0 w-8 h-8 bg-purple-500 rounded-full flex items-center justify-center text-white font-bold text-xs">2</div>
                <div className="flex-1">
                  <div className="flex items-center gap-2">
                    <span className="font-semibold text-purple-800">Contract</span>
                    <span className="text-gray-600">→</span>
                    <span className="font-semibold text-orange-800">OutLayer</span>
                  </div>
                  <div className="text-xs text-purple-600 ml-8 mt-0.5">
                    request_execution(repo, input_data, limits, secrets)
                  </div>
                </div>
              </div>

              {/* YIELD */}
              <div className="ml-10 p-3 bg-yellow-100 border-l-4 border-yellow-500 rounded">
                <div className="font-bold text-yellow-900 text-xs">⏸️ YIELD - Transaction paused</div>
              </div>

              {/* Step 3 */}
              <div className="flex items-center gap-2">
                <div className="flex-shrink-0 w-8 h-8 bg-orange-500 rounded-full flex items-center justify-center text-white font-bold text-xs">3</div>
                <div className="flex items-center gap-2 flex-1">
                  <span className="font-semibold text-orange-800">OutLayer</span>
                  <span className="text-gray-600">→</span>
                  <span className="font-semibold text-green-800">Worker</span>
                </div>
              </div>

              {/* Execution */}
              <div className="ml-10 p-3 bg-green-50 border-l-4 border-green-500 rounded">
                <div className="font-bold text-green-900 text-xs mb-1">🔨 Off-chain work:</div>
                <div className="text-xs text-green-800 space-y-0.5">
                  <div>• Clone from GitHub</div>
                  <div>• Compile code (10 sec+)</div>
                  <div>• Execute WASM (&lt;1 sec - few min)</div>
                  <div>• Read stdout output</div>
                </div>
              </div>

              {/* Step 4 */}
              <div className="flex items-center gap-2">
                <div className="flex-shrink-0 w-8 h-8 bg-green-600 rounded-full flex items-center justify-center text-white font-bold text-xs">4</div>
                <div className="flex items-center gap-2 flex-1">
                  <span className="font-semibold text-green-800">Worker</span>
                  <span className="text-gray-600">→</span>
                  <span className="font-semibold text-orange-800">OutLayer</span>
                </div>
              </div>

              <div className="ml-10 p-3 bg-green-100 border-l-4 border-green-600 rounded">
                <div className="font-bold text-green-900 text-xs">Return execution output</div>
              </div>

              {/* Step 5 */}
              <div className="flex items-center gap-2">
                <div className="flex-shrink-0 w-8 h-8 bg-orange-600 rounded-full flex items-center justify-center text-white font-bold text-xs">5</div>
                <div className="flex items-center gap-2 flex-1">
                  <span className="font-semibold text-orange-800">OutLayer</span>
                  <span className="text-gray-600">→</span>
                  <span className="font-semibold text-purple-800">Contract</span>
                </div>
              </div>

              {/* RESUME */}
              <div className="ml-10 p-3 bg-green-100 border-l-4 border-green-600 rounded">
                <div className="font-bold text-green-900 text-xs">▶️ RESUME - Process result in callback</div>
              </div>

              {/* Complete */}
              <div className="p-3 bg-green-100 border-2 border-green-600 rounded-lg text-center">
                <div className="font-bold text-green-900 text-xs">✅ Transaction Complete!</div>
                <div className="text-xs text-green-800 mt-1">User receives final result</div>
              </div>
            </div>
          </div>

          <div className="bg-green-50 border-l-4 border-green-500 p-4">
            <p className="text-sm text-gray-700">
              <strong>🎯 Key benefit:</strong> From the user&apos;s perspective, it&apos;s still ONE transaction. They click once,
              wait a few seconds, and get the final result. No manual follow-up transactions needed.
            </p>
          </div>
        </section>

        <section id="why-outlayer">
          <h3 className="text-xl font-semibold mb-3">Why OutLayer Makes This Easy</h3>
          <div className="space-y-3">
            <div className="bg-gray-50 p-4 rounded-lg border-l-4 border-orange-400">
              <p className="text-sm text-gray-700">
                <strong>✅ No infrastructure setup</strong> - We run the workers, you just write code
              </p>
            </div>
            <div className="bg-gray-50 p-4 rounded-lg border-l-4 border-orange-400">
              <p className="text-sm text-gray-700">
                <strong>✅ No access control headaches</strong> - Your contract stays in control. We just return results,
                you decide what to do with them in your callback
              </p>
            </div>
            <div className="bg-gray-50 p-4 rounded-lg border-l-4 border-orange-400">
              <p className="text-sm text-gray-700">
                <strong>✅ No worker delegation</strong> - You don&apos;t give us permissions on your contract.
                We can&apos;t do anything except call your callback with results
              </p>
            </div>
            <div className="bg-gray-50 p-4 rounded-lg border-l-4 border-orange-400">
              <p className="text-sm text-gray-700">
                <strong>✅ GitHub-based deployment</strong> - Push code to GitHub, reference the repo in your contract call.
                We compile and execute automatically
              </p>
            </div>
            <div className="bg-gray-50 p-4 rounded-lg border-l-4 border-orange-400">
              <p className="text-sm text-gray-700">
                <strong>✅ Encrypted secrets support</strong> - Need API keys? Store them encrypted on-chain with access control,
                we inject them as environment variables during execution
              </p>
            </div>
          </div>
        </section>

        <section id="quick-start">
          <h3 className="text-xl font-semibold mb-3">Quick Start: 4 Steps</h3>
          <div className="space-y-4">
            <div className="bg-white border-2 border-gray-200 rounded-lg p-4">
              <h4 className="font-semibold text-gray-800 mb-2">1️⃣ Write WASI Code</h4>
              <p className="text-sm text-gray-700 mb-2">
                Create Rust project that compiles to WebAssembly. Read input from stdin, write output to stdout.
              </p>
              <p className="text-xs text-gray-600">
                📖 <Link href="/docs/dev-guide" className="text-[var(--primary-orange)] hover:underline">Follow the Developer Guide</Link> for step-by-step tutorial
              </p>
              <p className="text-xs text-gray-600 mt-1">
                💡 <Link href="/docs/examples" className="text-[var(--primary-orange)] hover:underline">See Working Examples</Link> for inspiration
              </p>
            </div>

            <div className="bg-white border-2 border-gray-200 rounded-lg p-4">
              <h4 className="font-semibold text-gray-800 mb-2">2️⃣ Push to GitHub</h4>
              <p className="text-sm text-gray-700">
                Make your repo public (or private with access tokens). OutLayer will clone and compile it on-demand.
              </p>
            </div>

            <div className="bg-white border-2 border-gray-200 rounded-lg p-4">
              <h4 className="font-semibold text-gray-800 mb-2">3️⃣ Call from Your Contract or Testnet</h4>
              <p className="text-sm text-gray-700 mb-2">
                <strong>Option A:</strong> Test directly from CLI (no contract needed)
              </p>
              <pre className="text-xs bg-gray-100 p-2 rounded mt-1 overflow-x-auto">
{`near call outlayer.testnet request_execution '\\
  {"code_source": {"repo": "github.com/you/project", "commit": "main"}, \\
   "input_data": "{\\"param\\":123}"}' \\
  --accountId you.testnet --deposit 0.1`}
              </pre>
              <p className="text-sm text-gray-700 mt-3 mb-2">
                <strong>Option B:</strong> Integrate in your smart contract
              </p>
              <p className="text-xs text-gray-600">
                📖 <Link href="/docs/contract-integration" className="text-[var(--primary-orange)] hover:underline">Contract Integration Guide</Link> -
                See all parameters and callback handling
              </p>
            </div>

            <div className="bg-white border-2 border-gray-200 rounded-lg p-4">
              <h4 className="font-semibold text-gray-800 mb-2">4️⃣ Receive Result in Callback</h4>
              <p className="text-sm text-gray-700 mb-2">
                Your contract receives the result automatically. You control what happens next - no external permissions needed.
              </p>
              <p className="text-xs text-gray-600">
                Excess payment automatically refunded. Payment based on actual resources used (instructions + time).
              </p>
            </div>
          </div>
        </section>

        <section id="secrets">
          <h3 className="text-xl font-semibold mb-3">Need API Keys or Secrets?</h3>
          <p className="text-gray-700 mb-3">
            Store encrypted secrets on-chain with access control (whitelists, NEAR balance checks, NFT ownership, etc).
            OutLayer workers decrypt and inject them as environment variables - your WASM code accesses them via <code className="bg-gray-100 px-2 py-1 rounded">std::env::var()</code>.
          </p>
          <div className="bg-blue-50 border-l-4 border-blue-500 p-4">
            <p className="text-sm text-blue-800">
              💡 <strong>Use the Dashboard:</strong> <Link href="/secrets" className="underline font-medium">Manage Secrets</Link> -
              Encrypt API keys client-side, store on-chain, use in any execution
            </p>
          </div>
        </section>

        <section id="payment">
          <h3 className="text-xl font-semibold mb-3">Payment & Pricing</h3>
          <p className="text-gray-700 mb-3">
            Attach NEAR tokens when calling <code className="bg-gray-100 px-2 py-1 rounded">request_execution</code>.
            Cost = base fee + (actual instructions used × price) + (execution time × price).
          </p>
          <ul className="list-disc list-inside space-y-2 text-gray-700 ml-4 text-sm">
            <li>Unused deposit automatically refunded</li>
            <li>Users can pay for their own executions (set <code className="bg-gray-100 px-1 rounded">payer_account_id</code>)</li>
            <li>Or contracts can sponsor execution costs</li>
            <li>Query <code className="bg-gray-100 px-1 rounded">estimate_execution_cost()</code> to see pricing before calling</li>
          </ul>
        </section>

        <section>
          <h3 className="text-xl font-semibold mb-3">Ready to Build?</h3>
          <div className="grid grid-cols-1 md:grid-cols-3 gap-4">
            <Link href="/docs/dev-guide" className="block bg-gradient-to-br from-orange-50 to-red-50 border-2 border-orange-200 rounded-lg p-4 hover:shadow-lg transition-shadow">
              <h4 className="font-semibold text-gray-800 mb-2">📖 Developer Guide</h4>
              <p className="text-sm text-gray-700">Step-by-step tutorial: Build a random number generator</p>
            </Link>

            <Link href="/docs/examples" className="block bg-gradient-to-br from-blue-50 to-purple-50 border-2 border-blue-200 rounded-lg p-4 hover:shadow-lg transition-shadow">
              <h4 className="font-semibold text-gray-800 mb-2">💡 Working Examples</h4>
              <p className="text-sm text-gray-700">7 production-ready examples with full source code</p>
            </Link>

            <Link href="/docs/contract-integration" className="block bg-gradient-to-br from-green-50 to-teal-50 border-2 border-green-200 rounded-lg p-4 hover:shadow-lg transition-shadow">
              <h4 className="font-semibold text-gray-800 mb-2">🔌 Contract Integration</h4>
              <p className="text-sm text-gray-700">All parameters, callback handling, best practices</p>
            </Link>
          </div>
        </section>
      </div>
    </div>
  );
}
