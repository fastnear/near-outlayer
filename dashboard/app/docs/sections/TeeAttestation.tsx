'use client';

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
      setTimeout(() => {
        const element = document.getElementById(hash);
        if (element) {
          element.scrollIntoView({ behavior: 'smooth', block: 'start' });
        }
      }, 100);
    }
  }, []);
}

export default function TeeAttestationSection() {
  useHashNavigation();

  return (
    <div className="prose max-w-none">
      <h2 className="text-3xl font-bold mb-6 text-[var(--primary-orange)]">TEE Attestation</h2>

      <div className="space-y-8">
        {/* What is TEE Attestation */}
        <section id="what-is-tee">
          <AnchorHeading id="what-is-tee">What is TEE Attestation?</AnchorHeading>

          <p className="text-gray-700 mb-3">
            <strong>TEE (Trusted Execution Environment) Attestation</strong> is cryptographic proof that your code
            was executed inside a secure, isolated hardware environment - specifically <strong>Intel TDX</strong> (Trust Domain Extensions).
          </p>

          <p className="text-gray-700 mb-3">
            Think of it like a tamper-proof black box: code goes in, computation happens in complete isolation
            from the operating system and other applications, and results come out with a cryptographic signature
            proving the execution environment wasn&apos;t compromised.
          </p>

          <div className="bg-blue-50 border-l-4 border-blue-500 p-4 my-4">
            <p className="text-blue-900 font-semibold mb-2">Why This Matters:</p>
            <ul className="list-disc list-inside space-y-1 text-blue-800 text-sm">
              <li><strong>Verifiable Computation:</strong> Prove your code ran in a secure environment</li>
              <li><strong>Tamper-Proof:</strong> No one (not even the cloud provider) can modify execution</li>
              <li><strong>Transparent:</strong> Anyone can verify the attestation without trusting the operator</li>
              <li><strong>Cryptographically Signed:</strong> Intel&apos;s private key signs every attestation</li>
            </ul>
          </div>

          <p className="text-gray-700">
            On OutLayer, every code execution is attested using Intel TDX hardware, giving you cryptographic
            proof that your WASM binary was executed exactly as compiled, with the correct inputs, in a
            secure isolated environment.
          </p>
        </section>

        {/* Worker Registration */}
        <section id="worker-registration">
          <AnchorHeading id="worker-registration">Worker Registration</AnchorHeading>

          <p className="text-gray-700 mb-3">
            Before a TEE worker can execute any code on OutLayer, it must <strong>register on the NEAR blockchain</strong>
            with proof of its TEE environment. This is a one-time process per worker.
          </p>

          <div className="bg-blue-50 border-l-4 border-blue-500 p-4 my-4">
            <p className="text-blue-900 font-semibold mb-2">Register Contract:</p>
            <p className="text-blue-800 text-sm mb-2">
              Registration is handled by a dedicated smart contract deployed at:
            </p>
            <div className="bg-white rounded p-2 font-mono text-sm">
              <p className="text-blue-900"><strong>Testnet:</strong> <code className="text-orange-600">worker.outlayer.testnet</code></p>
              <p className="text-blue-900"><strong>Mainnet:</strong> <code className="text-orange-600">worker.outlayer.near</code></p>
            </div>
            <p className="text-blue-800 text-sm mt-2">
              View on{' '}
              <a
                href="https://testnet.nearblocks.io/address/worker.outlayer.testnet"
                target="_blank"
                rel="noopener noreferrer"
                className="underline hover:text-blue-600 font-semibold"
              >
                NEAR Explorer
              </a>
            </p>
          </div>

          <div className="bg-white border-2 border-purple-300 rounded-lg p-6 my-4">
            <h4 className="text-lg font-semibold text-purple-900 mb-3">Registration Process:</h4>
            <ol className="space-y-3">
              <li className="flex items-start">
                <span className="inline-flex items-center justify-center w-6 h-6 rounded-full bg-purple-500 text-white text-sm font-bold mr-3 flex-shrink-0">1</span>
                <div>
                  <p className="font-semibold text-gray-900">Worker Generates TDX Quote</p>
                  <p className="text-gray-700 text-sm">
                    The worker running in Intel TDX hardware requests a cryptographic quote from the Intel TEE.
                    This quote contains a measurement (RTMR3) - a SHA384 hash of the worker&apos;s environment.
                  </p>
                </div>
              </li>
              <li className="flex items-start">
                <span className="inline-flex items-center justify-center w-6 h-6 rounded-full bg-purple-500 text-white text-sm font-bold mr-3 flex-shrink-0">2</span>
                <div>
                  <p className="font-semibold text-gray-900">Submit to Register Contract</p>
                  <p className="text-gray-700 text-sm">
                    The worker calls <code className="bg-gray-100 px-1 rounded">register_worker</code> on the NEAR register-contract,
                    submitting the TDX quote along with Intel&apos;s collateral (signing certificates and revocation data).
                  </p>
                </div>
              </li>
              <li className="flex items-start">
                <span className="inline-flex items-center justify-center w-6 h-6 rounded-full bg-purple-500 text-white text-sm font-bold mr-3 flex-shrink-0">3</span>
                <div>
                  <p className="font-semibold text-gray-900">On-Chain Verification</p>
                  <p className="text-gray-700 text-sm">
                    The contract uses <strong>Intel DCAP QVL library</strong> to verify the quote&apos;s cryptographic signature.
                    This proves the quote came from genuine Intel TDX hardware (not simulated).
                  </p>
                </div>
              </li>
              <li className="flex items-start">
                <span className="inline-flex items-center justify-center w-6 h-6 rounded-full bg-purple-500 text-white text-sm font-bold mr-3 flex-shrink-0">4</span>
                <div>
                  <p className="font-semibold text-gray-900">Extract & Whitelist RTMR3</p>
                  <p className="text-gray-700 text-sm">
                    Once verified, the contract extracts the RTMR3 measurement from the quote and adds it to the
                    whitelist. This RTMR3 becomes the worker&apos;s cryptographic identity.
                  </p>
                </div>
              </li>
              <li className="flex items-start">
                <span className="inline-flex items-center justify-center w-6 h-6 rounded-full bg-purple-500 text-white text-sm font-bold mr-3 flex-shrink-0">5</span>
                <div>
                  <p className="font-semibold text-gray-900">Register Public Key for Execution</p>
                  <p className="text-gray-700 text-sm">
                    During registration, the worker also submits its <strong>public key</strong>. This key is added to
                    the main OutLayer contract and is the <strong>only key authorized</strong> to call{' '}
                    <code className="bg-gray-100 px-1 rounded">resolve_execution</code> to finalize execution results.
                    This prevents unauthorized workers from submitting fake results.
                  </p>
                </div>
              </li>
              <li className="flex items-start">
                <span className="inline-flex items-center justify-center w-6 h-6 rounded-full bg-purple-500 text-white text-sm font-bold mr-3 flex-shrink-0">6</span>
                <div>
                  <p className="font-semibold text-gray-900">Registration Complete</p>
                  <p className="text-gray-700 text-sm">
                    The worker is now authorized to execute code on OutLayer. All future executions from this
                    worker will be verified against this RTMR3, and only this worker can submit results using its
                    registered public key.
                  </p>
                </div>
              </li>
            </ol>
          </div>

          <div className="bg-yellow-50 border-l-4 border-yellow-500 p-4 my-4">
            <p className="text-yellow-900 font-semibold mb-2">What is RTMR3?</p>
            <p className="text-yellow-800 text-sm">
              <strong>RTMR3 (Runtime Measurement Register 3)</strong> is a 48-byte (384-bit) SHA384 hash that uniquely
              identifies the TEE worker&apos;s environment. It&apos;s calculated by Intel TDX hardware based on the code,
              configuration, and initial state of the worker. Any change to the worker software produces a completely
              different RTMR3.
            </p>
          </div>
        </section>

        {/* Execution Attestation */}
        <section id="execution-attestation">
          <AnchorHeading id="execution-attestation">Execution Attestation</AnchorHeading>

          <p className="text-gray-700 mb-3">
            Every time a TEE worker executes WASM code on OutLayer, it generates a fresh <strong>attestation</strong>
            that cryptographically proves what was executed, with what inputs, and in what environment.
          </p>

          <div className="bg-white border-2 border-green-300 rounded-lg p-6 my-4">
            <h4 className="text-lg font-semibold text-green-900 mb-3">What Gets Attested:</h4>
            <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
              <div className="bg-green-50 p-3 rounded">
                <p className="font-semibold text-green-900 mb-1">📋 Source Code</p>
                <p className="text-green-800 text-sm">
                  GitHub repository URL and exact commit hash that was compiled.
                </p>
              </div>
              <div className="bg-green-50 p-3 rounded">
                <p className="font-semibold text-green-900 mb-1">🗜️ WASM Hash</p>
                <p className="text-green-800 text-sm">
                  SHA256 of the compiled WebAssembly binary that was executed.
                </p>
              </div>
              <div className="bg-green-50 p-3 rounded">
                <p className="font-semibold text-green-900 mb-1">📥 Input Hash</p>
                <p className="text-green-800 text-sm">
                  SHA256 of the input data sent to the WASM program.
                </p>
              </div>
              <div className="bg-green-50 p-3 rounded">
                <p className="font-semibold text-green-900 mb-1">📤 Output Hash</p>
                <p className="text-green-800 text-sm">
                  SHA256 of the output returned by the WASM execution.
                </p>
              </div>
              <div className="bg-green-50 p-3 rounded">
                <p className="font-semibold text-green-900 mb-1">🔐 Worker Measurement</p>
                <p className="text-green-800 text-sm">
                  RTMR3 hash proving which registered worker performed the execution.
                </p>
              </div>
              <div className="bg-green-50 p-3 rounded">
                <p className="font-semibold text-green-900 mb-1">🔏 TDX Quote</p>
                <p className="text-green-800 text-sm">
                  Full Intel TDX attestation quote signed by Intel&apos;s private key.
                </p>
              </div>
            </div>
          </div>

          <p className="text-gray-700 mb-3">
            All this data is stored in the <strong>Coordinator database</strong> and can be retrieved via the API.
            The attestation links the entire execution chain: from source code → compiled WASM → input data →
            TEE execution → output result.
          </p>
        </section>

        {/* Verification Process */}
        <section id="verification-process">
          <AnchorHeading id="verification-process">Verification Process</AnchorHeading>

          <p className="text-gray-700 mb-3">
            Anyone can verify an attestation without trusting OutLayer or the worker operator. The verification
            is based on cryptography and public Intel infrastructure.
          </p>

          <div className="bg-white border-2 border-blue-300 rounded-lg p-6 my-4">
            <h4 className="text-lg font-semibold text-blue-900 mb-3">How to Verify an Attestation:</h4>

            <div className="space-y-4">
              <div className="border-l-4 border-blue-500 pl-4">
                <p className="font-semibold text-gray-900 mb-1">Step 1: Verify TDX Quote</p>
                <p className="text-gray-700 text-sm mb-2">
                  Extract the RTMR3 measurement from the TDX quote (located at byte offset 256, 48 bytes long).
                  Compare it to the worker_measurement stored in the attestation. If they match, the quote is authentic.
                </p>
                <p className="text-gray-600 text-xs italic">
                  The Dashboard does this automatically when you click &quot;Verify Quote&quot; in the attestation modal.
                </p>
              </div>

              <div className="border-l-4 border-blue-500 pl-4">
                <p className="font-semibold text-gray-900 mb-1">Step 2: Verify Input/Output Hashes</p>
                <p className="text-gray-700 text-sm mb-2">
                  Fetch the NEAR transaction from the blockchain (using the transaction_hash). Extract the input data
                  from the <code className="bg-gray-100 px-1 rounded">execution_requested</code> event, and the output
                  from the final receipt&apos;s <code className="bg-gray-100 px-1 rounded">SuccessValue</code>. Calculate
                  SHA256 hashes and compare to the attestation.
                </p>
                <p className="text-gray-600 text-xs italic">
                  The Dashboard does this when you click &quot;Load & Verify from Blockchain&quot;.
                </p>
              </div>

              <div className="border-l-4 border-blue-500 pl-4">
                <p className="font-semibold text-gray-900 mb-1">Step 3: Verify Source Code</p>
                <p className="text-gray-700 text-sm mb-2">
                  Click the source code link to view the exact GitHub commit that was compiled. You can audit the
                  code yourself to see what logic was executed.
                </p>
              </div>

              <div className="border-l-4 border-blue-500 pl-4">
                <p className="font-semibold text-gray-900 mb-1">Step 4: Verify WASM Hash (Optional)</p>
                <p className="text-gray-700 text-sm mb-2">
                  Download the WASM binary from the Coordinator API and calculate its SHA256 hash. Compare to the
                  wasm_hash in the attestation. This proves the compiled binary matches the source code.
                </p>
              </div>
            </div>
          </div>

          <div className="bg-purple-50 border-l-4 border-purple-500 p-4 my-4">
            <p className="text-purple-900 font-semibold mb-2">Cryptographic Chain of Trust:</p>
            <p className="text-purple-800 text-sm">
              The attestation creates an unbreakable cryptographic chain: <strong>Source Code</strong> → compiled to →
              <strong>WASM Binary</strong> → executed with → <strong>Input Data</strong> → inside →
              <strong>TEE Worker (RTMR3)</strong> → producing → <strong>Output Data</strong>. Every link is
              cryptographically hashed and signed by Intel TDX hardware.
            </p>
          </div>
        </section>

        {/* Security Guarantees */}
        <section id="security-guarantees">
          <AnchorHeading id="security-guarantees">Security Guarantees</AnchorHeading>

          <p className="text-gray-700 mb-3">
            TEE attestation provides strong security guarantees that go beyond traditional cloud computing:
          </p>

          <div className="grid grid-cols-1 md:grid-cols-2 gap-4 my-4">
            <div className="bg-white border-2 border-green-200 rounded-lg p-4">
              <div className="flex items-center mb-2">
                <span className="text-2xl mr-2">✅</span>
                <h4 className="font-semibold text-green-900">Hardware Isolation</h4>
              </div>
              <p className="text-gray-700 text-sm">
                Code runs in Intel TDX isolated memory. The OS, hypervisor, and other applications cannot
                access or modify the execution environment.
              </p>
            </div>

            <div className="bg-white border-2 border-green-200 rounded-lg p-4">
              <div className="flex items-center mb-2">
                <span className="text-2xl mr-2">🔐</span>
                <h4 className="font-semibold text-green-900">Intel Signature</h4>
              </div>
              <p className="text-gray-700 text-sm">
                Every TDX quote is signed with Intel&apos;s private key. This proves the attestation came from
                genuine Intel hardware, not a simulation or fake environment.
              </p>
            </div>

            <div className="bg-white border-2 border-green-200 rounded-lg p-4">
              <div className="flex items-center mb-2">
                <span className="text-2xl mr-2">🛡️</span>
                <h4 className="font-semibold text-green-900">Tamper-Proof</h4>
              </div>
              <p className="text-gray-700 text-sm">
                Any modification to the TDX quote invalidates Intel&apos;s signature. The cryptography ensures
                the attestation cannot be forged or altered.
              </p>
            </div>

            <div className="bg-white border-2 border-green-200 rounded-lg p-4">
              <div className="flex items-center mb-2">
                <span className="text-2xl mr-2">🔍</span>
                <h4 className="font-semibold text-green-900">Verifiable by Anyone</h4>
              </div>
              <p className="text-gray-700 text-sm">
                You don&apos;t need to trust OutLayer. Anyone can independently verify the attestation using
                the TDX quote and public NEAR blockchain data.
              </p>
            </div>

            <div className="bg-white border-2 border-green-200 rounded-lg p-4">
              <div className="flex items-center mb-2">
                <span className="text-2xl mr-2">📜</span>
                <h4 className="font-semibold text-green-900">Code Transparency</h4>
              </div>
              <p className="text-gray-700 text-sm">
                The attestation includes the exact GitHub commit, so you can audit the source code and verify
                it matches what was executed.
              </p>
            </div>

            <div className="bg-white border-2 border-green-200 rounded-lg p-4">
              <div className="flex items-center mb-2">
                <span className="text-2xl mr-2">🔗</span>
                <h4 className="font-semibold text-green-900">Blockchain Anchoring</h4>
              </div>
              <p className="text-gray-700 text-sm">
                Input and output data are stored on NEAR blockchain, providing an immutable record that can
                be cross-verified with the attestation hashes.
              </p>
            </div>
          </div>

          <div className="bg-red-50 border-l-4 border-red-500 p-4 my-4">
            <p className="text-red-900 font-semibold mb-2">What TEE Does NOT Guarantee:</p>
            <ul className="list-disc list-inside space-y-1 text-red-800 text-sm">
              <li>TEE does not verify the <strong>correctness</strong> of your code logic</li>
              <li>TEE does not prevent bugs or vulnerabilities in your WASM program</li>
              <li>TEE does not protect against side-channel attacks (though Intel TDX has mitigations)</li>
              <li>TEE does not guarantee the code will produce the &quot;right&quot; answer - only that it ran in isolation</li>
            </ul>
          </div>
        </section>

        {/* Dashboard Verification */}
        <section id="dashboard-verification">
          <AnchorHeading id="dashboard-verification">Dashboard Verification</AnchorHeading>

          <p className="text-gray-700 mb-3">
            The OutLayer Dashboard provides an easy-to-use interface for viewing and verifying attestations
            without writing any code.
          </p>

          <div className="bg-white border-2 border-orange-300 rounded-lg p-6 my-4">
            <h4 className="text-lg font-semibold text-orange-900 mb-3">Using the Dashboard:</h4>

            <ol className="space-y-3">
              <li className="flex items-start">
                <span className="inline-flex items-center justify-center w-6 h-6 rounded-full bg-orange-500 text-white text-sm font-bold mr-3 flex-shrink-0">1</span>
                <div>
                  <p className="font-semibold text-gray-900">Navigate to Executions Page</p>
                  <p className="text-gray-700 text-sm">
                    Go to <strong>/executions</strong> to see the history of all code executions on OutLayer.
                  </p>
                </div>
              </li>
              <li className="flex items-start">
                <span className="inline-flex items-center justify-center w-6 h-6 rounded-full bg-orange-500 text-white text-sm font-bold mr-3 flex-shrink-0">2</span>
                <div>
                  <p className="font-semibold text-gray-900">Click &quot;View Attestation&quot;</p>
                  <p className="text-gray-700 text-sm">
                    For any execution with a TEE attestation, click the &quot;View Attestation&quot; button to open
                    the attestation modal.
                  </p>
                </div>
              </li>
              <li className="flex items-start">
                <span className="inline-flex items-center justify-center w-6 h-6 rounded-full bg-orange-500 text-white text-sm font-bold mr-3 flex-shrink-0">3</span>
                <div>
                  <p className="font-semibold text-gray-900">View Attestation Details</p>
                  <p className="text-gray-700 text-sm">
                    The modal shows: Worker Measurement (RTMR3), Source Code link, WASM Hash, Input Hash, Output Hash,
                    TDX Quote, and NEAR Transaction link.
                  </p>
                </div>
              </li>
              <li className="flex items-start">
                <span className="inline-flex items-center justify-center w-6 h-6 rounded-full bg-orange-500 text-white text-sm font-bold mr-3 flex-shrink-0">4</span>
                <div>
                  <p className="font-semibold text-gray-900">Verify TDX Quote</p>
                  <p className="text-gray-700 text-sm">
                    Click <strong>&quot;Verify Quote&quot;</strong> in the purple section. The Dashboard will extract RTMR3
                    from the TDX quote and compare it to the worker measurement. You&apos;ll see a live validation with
                    green checkmark if it matches.
                  </p>
                </div>
              </li>
              <li className="flex items-start">
                <span className="inline-flex items-center justify-center w-6 h-6 rounded-full bg-orange-500 text-white text-sm font-bold mr-3 flex-shrink-0">5</span>
                <div>
                  <p className="font-semibold text-gray-900">Verify Input/Output Hashes</p>
                  <p className="text-gray-700 text-sm">
                    Click <strong>&quot;Load & Verify from Blockchain&quot;</strong> in the blue section. The Dashboard will
                    fetch the NEAR transaction, extract input/output data, calculate SHA256 hashes, and show you live
                    validation results. You can edit the data to see how hashes change in real-time.
                  </p>
                </div>
              </li>
              <li className="flex items-start">
                <span className="inline-flex items-center justify-center w-6 h-6 rounded-full bg-orange-500 text-white text-sm font-bold mr-3 flex-shrink-0">6</span>
                <div>
                  <p className="font-semibold text-gray-900">Audit Source Code</p>
                  <p className="text-gray-700 text-sm">
                    Click the source code link to view the exact GitHub commit on GitHub. Review the code to understand
                    what was executed.
                  </p>
                </div>
              </li>
            </ol>
          </div>

          <div className="bg-green-50 border-l-4 border-green-500 p-4 my-4">
            <p className="text-green-900 font-semibold mb-2">Interactive Verification:</p>
            <p className="text-green-800 text-sm">
              The Dashboard provides <strong>live interactive validation</strong>. When you verify Input/Output hashes
              or TDX quotes, you can edit the data in the form fields and watch the hashes recalculate and validation
              status update in real-time. This helps you understand exactly how the cryptographic verification works.
            </p>
          </div>

          <div className="bg-blue-50 border-l-4 border-blue-500 p-4 my-4">
            <p className="text-blue-900 font-semibold mb-2">Need Help?</p>
            <p className="text-blue-800 text-sm">
              Click the <strong>&quot;❓ Help&quot;</strong> button in the attestation modal for detailed explanations
              of each field, what can be verified, and what security guarantees TEE provides. The help section explains
              everything in plain English without requiring cryptography knowledge.
            </p>
          </div>
        </section>
      </div>
    </div>
  );
}
