'use client';

import Link from 'next/link';
import { useEffect } from 'react';

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

export default function GettingStartedSection() {
  useHashNavigation();

  return (
    <div className="prose max-w-none">
      <h2 className="text-3xl font-bold mb-6 text-[var(--primary-orange)]">Getting Started</h2>

      <div className="space-y-8">
        <section id="what-is-outlayer">
          <AnchorHeading id="what-is-outlayer">What is OutLayer?</AnchorHeading>
          <p className="text-gray-700 mb-3">
            OutLayer lets you run <strong>any off-chain code</strong> (random numbers, HTTP requests, AI models, heavy computations)
            with <strong>cryptographic proof</strong> that exactly the code you specified ran with the inputs you provided.
          </p>
          <p className="text-gray-700">
            Call OutLayer from <strong>NEAR smart contracts</strong> (yield/resume) or <strong>any app via HTTPS</strong>.
            Every execution produces verifiable TEE attestation - no trust required, just math.
          </p>
        </section>        

        <section id="tee-attestation">
          <AnchorHeading id="tee-attestation">Verifiable Execution (TEE)</AnchorHeading>
          <p className="text-gray-700 mb-3">
            Every OutLayer execution runs inside <strong>Intel TDX</strong> (Trusted Execution Environment) and produces
            a cryptographic attestation proving:
          </p>
          <div className="grid grid-cols-1 md:grid-cols-2 gap-3 mb-4">
            <div className="bg-purple-50 border-l-4 border-purple-500 p-3">
              <p className="text-sm text-purple-800"><strong>🔐 Code Integrity</strong> - SHA256 of exact WASM binary</p>
            </div>
            <div className="bg-purple-50 border-l-4 border-purple-500 p-3">
              <p className="text-sm text-purple-800"><strong>📥 Input Integrity</strong> - SHA256 of input data</p>
            </div>
            <div className="bg-purple-50 border-l-4 border-purple-500 p-3">
              <p className="text-sm text-purple-800"><strong>📤 Output Integrity</strong> - Result from that code + input</p>
            </div>
            <div className="bg-purple-50 border-l-4 border-purple-500 p-3">
              <p className="text-sm text-purple-800"><strong>🛡️ Worker Identity</strong> - Verified TEE measurements</p>
            </div>
          </div>
          <div className="bg-green-50 border-l-4 border-green-500 p-4">
            <p className="text-sm text-gray-700">
              <strong>🎯 Why this matters:</strong> Anyone can independently verify that your code ran correctly.
              No &quot;trust us&quot; - cryptographic proof signed by Intel hardware.
              <Link href="/docs/tee-attestation" className="ml-2 text-[var(--primary-orange)] hover:underline">Learn more →</Link>
            </p>
          </div>
        </section>

        <section id="tee-vault">
          <AnchorHeading id="tee-vault">Upgradeable TEE Vault</AnchorHeading>
          <p className="text-gray-700 mb-3">
            Build wallet apps where <strong>private keys live inside TEE</strong>. Update your code anytime —
            secrets persist across upgrades thanks to <strong>Confidential Key Derivation (CKD)</strong>.
          </p>
          <div className="bg-orange-50 border-2 border-orange-300 rounded-lg p-4 mb-3">
            <p className="text-sm text-orange-800">
              <strong>How it works:</strong> Your project gets a unique derived key from DAO-controlled master key.
              Only your WASM code running in TEE can access it. Change your code — same key, same secrets.
            </p>
          </div>
          <div className="bg-blue-50 border-l-4 border-blue-500 p-4">
            <p className="text-sm text-blue-800">
              💡 <Link href="/docs/secrets#confidential-key-derivation" className="underline font-medium">CKD Documentation</Link> •{' '}
              <Link href="/docs/secrets#dao-governance" className="underline font-medium">DAO Governance →</Link>
            </p>
          </div>
        </section>

        <section id="two-modes">
          <AnchorHeading id="two-modes">Two Ways to Use OutLayer</AnchorHeading>
          <p className="text-gray-700 mb-4">
            Choose based on your use case. Both provide the same verifiable execution guarantees.
          </p>

          {/* Mode comparison cards */}
          <div className="grid grid-cols-1 lg:grid-cols-2 gap-6 mb-6">
            <div className="bg-purple-50 border-2 border-purple-300 rounded-lg p-5">
              <h4 className="font-bold text-purple-900 mb-2 text-lg">🔗 Blockchain (NEAR)</h4>
              <ul className="text-sm text-purple-800 space-y-1 mb-3">
                <li>• Smart contract callbacks via yield/resume</li>
                <li>• Pay with NEAR tokens</li>
                <li>• On-chain settlement & refunds</li>
                <li>• Best for: DeFi, DAOs, on-chain apps</li>
              </ul>
              <Link href="/docs/near-integration" className="text-sm text-purple-700 font-medium hover:underline">
                Full documentation →
              </Link>
            </div>
            <div className="bg-orange-50 border-2 border-orange-300 rounded-lg p-5">
              <h4 className="font-bold text-orange-900 mb-2 text-lg">🌐 HTTPS API</h4>
              <ul className="text-sm text-orange-800 space-y-1 mb-3">
                <li>• Direct HTTP calls, instant response</li>
                <li>• Pay with USDC (Payment Keys)</li>
                <li>• No blockchain knowledge needed</li>
                <li>• Best for: Web apps, APIs, backends</li>
              </ul>
              <Link href="/docs/web2-integration" className="text-sm text-orange-700 font-medium hover:underline">
                Full documentation →
              </Link>
            </div>
          </div>
        </section>

        <section id="blockchain-flow">
          <AnchorHeading id="blockchain-flow">Blockchain Flow (NEAR Yield/Resume)</AnchorHeading>
          <p className="text-gray-700 mb-3">
            NEAR&apos;s yield/resume allows smart contracts to <strong>pause execution</strong>, wait for off-chain
            computation, then <strong>resume with the result</strong> - all in one logical transaction.
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
              <text x="205" y="95" textAnchor="middle" fontSize="11" fill="#1e40af" fontWeight="bold">1. call contract</text>

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
              <text x="605" y="225" textAnchor="middle" fontSize="11" fill="#c2410c" fontWeight="bold">3. emit event</text>

              {/* Worker execution box */}
              <rect x="630" y="250" width="140" height="100" fill="#f0fdf4" stroke="#16a34a" strokeWidth="2" rx="4" />
              <text x="640" y="268" textAnchor="start" fontSize="10" fontWeight="bold" fill="#166534">🔨 Off-chain worker:</text>
              <text x="640" y="283" textAnchor="start" fontSize="9" fill="#166534">• Clone GitHub</text>
              <text x="640" y="298" textAnchor="start" fontSize="9" fill="#166534">• Compile WASM</text>
              <text x="640" y="313" textAnchor="start" fontSize="9" fill="#166534">• Execute with tx input</text>
              <text x="645" y="328" textAnchor="start" fontSize="9" fill="#6b7280">(fast / cheap / any code)</text>              
              <text x="640" y="342" textAnchor="start" fontSize="9" fill="#166534">• Read stdout</text>              

              {/* Step 5: OutLayer -> Your Contract */}
              <line x1="700" y1="370" x2="510" y2="370" stroke="#16a34a" strokeWidth="2" markerEnd="url(#arrowGreen)" />
              <text x="605" y="385" textAnchor="middle" fontSize="11" fill="#166534" fontWeight="bold">4. yield_resume()</text>

              {/* RESUME box */}
              <rect x="450" y="400" width="120" height="40" fill="#dcfce7" stroke="#16a34a" strokeWidth="2" rx="4" />
              <text x="510" y="415" textAnchor="middle" fontSize="10" fontWeight="bold" fill="#166534">▶️ RESUME</text>
              <text x="510" y="430" textAnchor="middle" fontSize="9" fill="#166534">Process result</text>

              {/* Step 4: Worker -> OutLayer */}
              <line x1="510" y1="470" x2="310" y2="470" stroke="#c2410c" strokeWidth="2" markerEnd="url(#arrowOrange)" />
              <text x="410" y="465" textAnchor="middle" fontSize="11" fill="#92400e" fontWeight="bold">5. return result</text>

              {/* Final result */}
              <rect x="100" y="500" width="380" height="40" fill="#dcfce7" stroke="#16a34a" strokeWidth="2" rx="8" />
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

        <section id="https-flow">
          <AnchorHeading id="https-flow">HTTPS Flow (Direct API)</AnchorHeading>
          <p className="text-gray-700 mb-3">
            Call OutLayer directly via HTTPS - no blockchain transactions, instant response.
            Ideal for developers who want to <strong>monetize their APIs</strong> and provide users
            with <strong>cryptographic proof</strong> of what code actually ran.
          </p>

          {/* Desktop Diagram - Hidden on mobile */}
          <div className="hidden md:block bg-white border-2 border-gray-300 rounded-lg p-6 mb-4 overflow-x-auto">
            <svg viewBox="0 0 700 400" className="w-full" style={{ maxWidth: '700px', margin: '0 auto' }}>
              {/* Participant boxes */}
              <rect x="50" y="20" width="120" height="50" fill="#3b82f6" rx="8" />
              <text x="110" y="40" textAnchor="middle" fill="white" fontSize="12" fontWeight="bold">Your App</text>
              <text x="110" y="55" textAnchor="middle" fill="white" fontSize="10">(Web/Mobile/API)</text>

              <rect x="280" y="20" width="120" height="50" fill="#f97316" rx="8" />
              <text x="340" y="40" textAnchor="middle" fill="white" fontSize="12" fontWeight="bold">OutLayer</text>
              <text x="340" y="55" textAnchor="middle" fill="white" fontSize="12" fontWeight="bold">API</text>

              <rect x="510" y="20" width="120" height="50" fill="#16a34a" rx="8" />
              <text x="570" y="50" textAnchor="middle" fill="white" fontSize="14" fontWeight="bold">TEE Worker</text>

              {/* Lifelines */}
              <line x1="110" y1="70" x2="110" y2="370" stroke="#d1d5db" strokeWidth="2" strokeDasharray="5,5" />
              <line x1="340" y1="70" x2="340" y2="370" stroke="#d1d5db" strokeWidth="2" strokeDasharray="5,5" />
              <line x1="570" y1="70" x2="570" y2="370" stroke="#d1d5db" strokeWidth="2" strokeDasharray="5,5" />

              {/* Arrows */}
              <defs>
                <marker id="arrowBlue2" markerWidth="10" markerHeight="10" refX="9" refY="3" orient="auto" markerUnits="strokeWidth">
                  <path d="M0,0 L0,6 L9,3 z" fill="#3b82f6" />
                </marker>
                <marker id="arrowOrange2" markerWidth="10" markerHeight="10" refX="9" refY="3" orient="auto" markerUnits="strokeWidth">
                  <path d="M0,0 L0,6 L9,3 z" fill="#f97316" />
                </marker>
                <marker id="arrowGreen2" markerWidth="10" markerHeight="10" refX="9" refY="3" orient="auto" markerUnits="strokeWidth">
                  <path d="M0,0 L0,6 L9,3 z" fill="#16a34a" />
                </marker>
              </defs>

              {/* Step 1: App -> API */}
              <line x1="110" y1="100" x2="340" y2="100" stroke="#3b82f6" strokeWidth="2" markerEnd="url(#arrowBlue2)" />
              <text x="225" y="90" textAnchor="middle" fontSize="11" fill="#1e40af" fontWeight="bold">1. POST /call/owner/project</text>
              <text x="225" y="113" textAnchor="middle" fontSize="9" fill="#6b7280">X-Payment-Key + X-Attached-Deposit</text>
              <text x="225" y="125" textAnchor="middle" fontSize="8" fill="#9ca3af">(payment goes to app author)</text>

              {/* Step 2: API -> Worker */}
              <line x1="340" y1="150" x2="570" y2="150" stroke="#f97316" strokeWidth="2" markerEnd="url(#arrowOrange2)" />
              <text x="455" y="145" textAnchor="middle" fontSize="11" fill="#c2410c" fontWeight="bold">2. Execute in TEE</text>

              {/* Worker execution box */}
              <rect x="490" y="170" width="160" height="90" fill="#f0fdf4" stroke="#16a34a" strokeWidth="2" rx="4" />
              <text x="500" y="188" textAnchor="start" fontSize="10" fontWeight="bold" fill="#166534">🔨 TEE Worker:</text>
              <text x="500" y="205" textAnchor="start" fontSize="9" fill="#166534">• Load WASM (cached)</text>
              <text x="500" y="220" textAnchor="start" fontSize="9" fill="#166534">• Execute with input</text>
              <text x="500" y="235" textAnchor="start" fontSize="9" fill="#166534">• Generate attestation</text>
              <text x="500" y="250" textAnchor="start" fontSize="9" fill="#166534">• Return result + proof</text>

              {/* Step 3: Worker -> API */}
              <line x1="570" y1="280" x2="340" y2="280" stroke="#16a34a" strokeWidth="2" markerEnd="url(#arrowGreen2)" />
              <text x="455" y="295" textAnchor="middle" fontSize="11" fill="#166534" fontWeight="bold">3. Result + attestation</text>

              {/* Step 4: API -> App */}
              <line x1="340" y1="320" x2="110" y2="320" stroke="#f97316" strokeWidth="2" markerEnd="url(#arrowOrange2)" />
              <text x="225" y="315" textAnchor="middle" fontSize="11" fill="#c2410c" fontWeight="bold">4. JSON response</text>

              {/* Final result */}
              <rect x="110" y="340" width="280" height="35" fill="#dcfce7" stroke="#16a34a" strokeWidth="2" rx="8" />
              <text x="250" y="362" textAnchor="middle" fontSize="11" fontWeight="bold" fill="#166534">✅ Instant response with verifiable proof</text>
            </svg>
          </div>

          {/* Mobile Simplified Diagram */}
          <div className="md:hidden bg-gradient-to-b from-gray-50 to-orange-50 border-2 border-gray-300 rounded-lg p-4 mb-4">
            <div className="space-y-3 text-sm">
              {/* Step 1 */}
              <div className="flex items-center gap-2">
                <div className="flex-shrink-0 w-8 h-8 bg-blue-500 rounded-full flex items-center justify-center text-white font-bold text-xs">1</div>
                <div className="flex-1">
                  <div className="flex items-center gap-2">
                    <span className="font-semibold text-blue-800">Your App</span>
                    <span className="text-gray-600">→</span>
                    <span className="font-semibold text-orange-800">OutLayer API</span>
                  </div>
                  <div className="text-xs text-gray-600 ml-0 mt-0.5">
                    POST + Payment Key + optional tip to author
                  </div>
                </div>
              </div>

              {/* Step 2 */}
              <div className="flex items-center gap-2">
                <div className="flex-shrink-0 w-8 h-8 bg-orange-500 rounded-full flex items-center justify-center text-white font-bold text-xs">2</div>
                <div className="flex items-center gap-2 flex-1">
                  <span className="font-semibold text-orange-800">API</span>
                  <span className="text-gray-600">→</span>
                  <span className="font-semibold text-green-800">TEE Worker</span>
                </div>
              </div>

              {/* Execution */}
              <div className="ml-10 p-3 bg-green-50 border-l-4 border-green-500 rounded">
                <div className="font-bold text-green-900 text-xs mb-1">🔨 TEE execution:</div>
                <div className="text-xs text-green-800 space-y-0.5">
                  <div>• Load cached WASM</div>
                  <div>• Execute with input</div>
                  <div>• Generate attestation</div>
                </div>
              </div>

              {/* Step 3 */}
              <div className="flex items-center gap-2">
                <div className="flex-shrink-0 w-8 h-8 bg-green-600 rounded-full flex items-center justify-center text-white font-bold text-xs">3</div>
                <div className="flex items-center gap-2 flex-1">
                  <span className="font-semibold text-green-800">Worker</span>
                  <span className="text-gray-600">→</span>
                  <span className="font-semibold text-orange-800">API</span>
                  <span className="text-gray-600">→</span>
                  <span className="font-semibold text-blue-800">App</span>
                </div>
              </div>

              {/* Complete */}
              <div className="p-3 bg-green-100 border-2 border-green-600 rounded-lg text-center">
                <div className="font-bold text-green-900 text-xs">✅ Instant Response!</div>
                <div className="text-xs text-green-800 mt-1">JSON with result + attestation proof</div>
              </div>
            </div>
          </div>

          <div className="bg-orange-50 border-l-4 border-orange-500 p-4">
            <p className="text-sm text-gray-700">
              <strong>🎯 Key benefit:</strong> Sub-second response, no blockchain knowledge needed.
              Same verifiable execution, just via HTTP. Pay with USDC via Payment Keys.
              Optionally send payment to app author via <code className="bg-orange-100 px-1 rounded text-xs">X-Attached-Deposit</code> header.
            </p>
          </div>
        </section>

        <section id="why-outlayer">
          <AnchorHeading id="why-outlayer">Why OutLayer Makes This Easy</AnchorHeading>
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
          <AnchorHeading id="quick-start">Quick Start: 4 Steps</AnchorHeading>
          <div className="space-y-4">
            <div className="bg-white border-2 border-gray-200 rounded-lg p-4">
              <h4 className="font-semibold text-gray-800 mb-2">1️⃣ Write Your Code</h4>
              <p className="text-sm text-gray-700 mb-2">
                Create a project that compiles to WebAssembly. Rust recommended, other languages supported.
              </p>
              <p className="text-xs text-gray-600">
                📖 <Link href="/docs/dev-guide" className="text-[var(--primary-orange)] hover:underline">Tutorial</Link> •{' '}
                <Link href="/docs/examples" className="text-[var(--primary-orange)] hover:underline">Examples</Link> •{' '}
                <Link href="/docs/wasi" className="text-[var(--primary-orange)] hover:underline">Building Apps</Link>
              </p>
            </div>

            <div className="bg-white border-2 border-gray-200 rounded-lg p-4">
              <h4 className="font-semibold text-gray-800 mb-2">2️⃣ Push to GitHub or Provide WASM URL</h4>
              <p className="text-sm text-gray-700 mb-2">
                <strong>Option A:</strong> Push to GitHub (public or private with access tokens). OutLayer will clone and compile on-demand.
              </p>
              <p className="text-sm text-gray-700 mb-2">
                <strong>Option B:</strong> Host pre-compiled WASM file anywhere and provide direct URL.
              </p>
              <p className="text-xs text-gray-500">
                ⏱️ GitHub: first execution compiles (10-30 sec), then cached. WASM URL: instant execution.
              </p>
            </div>

            <div className="bg-white border-2 border-gray-200 rounded-lg p-4">
              <h4 className="font-semibold text-gray-800 mb-2">3️⃣ Call OutLayer</h4>
              <p className="text-sm text-gray-700 mb-2">
                <strong>Option A:</strong> HTTPS API — call directly from any app
              </p>
              <pre className="text-xs bg-gray-100 p-2 rounded mt-1 overflow-x-auto">
{`curl -X POST https://api.outlayer.io/call/owner/project \\
  -H "X-Payment-Key: pk_..." \\
  -d '{"param": 123}'`}
              </pre>
              <p className="text-xs text-gray-600 mt-1 mb-3">
                📖 <Link href="/docs/web2-integration" className="text-[var(--primary-orange)] hover:underline">HTTPS Guide</Link> •{' '}
                <Link href="/docs/https-api" className="text-[var(--primary-orange)] hover:underline">API Reference</Link>
              </p>
              <p className="text-sm text-gray-700 mb-2">
                <strong>Option B:</strong> NEAR transaction
              </p>
              <pre className="text-xs bg-gray-100 p-2 rounded mt-1 overflow-x-auto">
{`near call outlayer.testnet request_execution '\\
  {"source": {"GitHub": {"repo": "github.com/you/project", "commit": "main"}}, \\
   "input_data": "{\\"param\\":123}"}' \\
  --accountId you.testnet --deposit 0.1`}
              </pre>
              <p className="text-sm text-gray-700 mt-3 mb-2">
                <strong>Option C:</strong> Smart contract integration
              </p>
              <p className="text-xs text-gray-600">
                📖 <Link href="/docs/near-integration" className="text-[var(--primary-orange)] hover:underline">Contract Integration Guide</Link>
              </p>
            </div>

            <div className="bg-white border-2 border-gray-200 rounded-lg p-4">
              <h4 className="font-semibold text-gray-800 mb-2">4️⃣ Receive Result</h4>
              <p className="text-sm text-gray-700">
                HTTPS returns JSON response instantly. NEAR contract receives callback automatically.
                Excess payment refunded based on actual resources used.
              </p>
            </div>

            <div className="bg-white border-2 border-gray-200 rounded-lg p-4">
              <h4 className="font-semibold text-gray-800 mb-2">5️⃣ Verify Attestation</h4>
              <p className="text-sm text-gray-700 mb-2">
                Every execution produces TEE attestation — cryptographic proof of what code ran with what inputs.
              </p>
              <p className="text-xs text-gray-600">
                📖 <Link href="/docs/tee-attestation" className="text-[var(--primary-orange)] hover:underline">How Attestation Works</Link> •{' '}
                <Link href="/executions" className="text-[var(--primary-orange)] hover:underline">View Executions</Link>
              </p>
            </div>
          </div>
        </section>

        <section id="secrets">
          <AnchorHeading id="secrets">Secrets</AnchorHeading>
          <p className="text-gray-700 mb-3">
            Store encrypted API keys and credentials with access control. OutLayer decrypts them
            during execution — your code reads them as environment variables.
            <strong> Update your code anytime — secrets persist across upgrades.</strong>
          </p>
          <div className="bg-purple-50 border-l-4 border-purple-500 p-4 mb-3">
            <p className="text-sm text-purple-800">
              <strong>🔐 Protected Secrets (CKD):</strong> Generate keys that <em>nobody knows</em> — not even you.
              Only your WASM code running in TEE can access them. Perfect for wallet apps and signing keys.
            </p>
          </div>
          <div className="bg-blue-50 border-l-4 border-blue-500 p-4">
            <p className="text-sm text-blue-800">
              💡 <Link href="/secrets" className="underline font-medium">Manage Secrets</Link> •{' '}
              <Link href="/docs/secrets#confidential-key-derivation" className="underline font-medium">CKD Docs</Link> •{' '}
              <Link href="/docs/secrets" className="underline font-medium">Learn more →</Link>
            </p>
          </div>
        </section>

        <section id="payment">
          <AnchorHeading id="payment">Payment & Pricing</AnchorHeading>
          <p className="text-gray-700 mb-3">
            Pay per execution based on actual resources used. Unused deposit automatically refunded.
          </p>
          <div className="bg-blue-50 border-l-4 border-blue-500 p-4">
            <p className="text-sm text-blue-800">
              💡 <Link href="/docs/pricing" className="underline font-medium">Pricing details →</Link>
            </p>
          </div>
        </section>

        <section id="persistent-storage">
          <AnchorHeading id="persistent-storage">Persistent Storage</AnchorHeading>
          <p className="text-gray-700 mb-3">
            Store data between executions with encrypted persistent storage.
            Data persists across code updates and is only accessible by your code.
          </p>
          <div className="bg-blue-50 border-l-4 border-blue-500 p-4">
            <p className="text-sm text-blue-800">
              💡 <Link href="/docs/storage" className="underline font-medium">Storage Guide</Link> •{' '}
              <Link href="/docs/projects" className="underline font-medium">Projects →</Link>
            </p>
          </div>
        </section>

        <section id="ready-to-build">
          <AnchorHeading id="ready-to-build">Ready to Build?</AnchorHeading>
          <div className="grid grid-cols-1 md:grid-cols-3 gap-4">
            <Link href="/docs/dev-guide" className="block bg-gradient-to-br from-orange-50 to-red-50 border-2 border-orange-200 rounded-lg p-4 hover:shadow-lg transition-shadow">
              <h4 className="font-semibold text-gray-800 mb-2">Developer Guide</h4>
              <p className="text-sm text-gray-700">Step-by-step tutorial: Build a random number generator</p>
            </Link>

            <Link href="/docs/examples" className="block bg-gradient-to-br from-blue-50 to-purple-50 border-2 border-blue-200 rounded-lg p-4 hover:shadow-lg transition-shadow">
              <h4 className="font-semibold text-gray-800 mb-2">Working Examples</h4>
              <p className="text-sm text-gray-700">Production-ready examples with full source code</p>
            </Link>

            <Link href="/docs/near-integration" className="block bg-gradient-to-br from-green-50 to-teal-50 border-2 border-green-200 rounded-lg p-4 hover:shadow-lg transition-shadow">
              <h4 className="font-semibold text-gray-800 mb-2">NEAR Integration</h4>
              <p className="text-sm text-gray-700">Smart contract integration with yield/resume callbacks</p>
            </Link>
          </div>
        </section>
      </div>
    </div>
  );
}
