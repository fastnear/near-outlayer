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
                <p className="font-semibold text-gray-900 mb-1">Step 1: Verify TDX Quote & Task Hash</p>
                <p className="text-gray-700 text-sm mb-2">
                  Extract the RTMR3 measurement from the TDX quote (located at byte offset 256, 48 bytes long).
                  Compare it to the worker_measurement stored in the attestation. Also extract the Task Hash from
                  REPORTDATA (offset 568, 32 bytes) and compare it to the calculated hash of all execution parameters.
                  If both match, the quote is authentic and bound to this specific execution.
                </p>
                <p className="text-gray-600 text-xs italic">
                  The Dashboard does this automatically when you click &quot;Verify Quote&quot; in the attestation modal.
                  See the <a href="#task-hash" className="underline hover:text-blue-600">Task Hash section</a> below for
                  detailed explanation of how this prevents attestation forgery.
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

        {/* Task Hash (REPORTDATA) */}
        <section id="task-hash">
          <AnchorHeading id="task-hash">Task Hash (REPORTDATA) - Preventing Attestation Forgery</AnchorHeading>

          <p className="text-gray-700 mb-3">
            The <strong>Task Hash</strong> is a critical security feature that prevents attestation forgery. It ensures
            that a valid TDX quote from one execution cannot be swapped or reused for a different execution.
          </p>

          <div className="bg-orange-50 border-l-4 border-orange-500 p-4 my-4">
            <p className="text-orange-900 font-semibold mb-2">The Problem:</p>
            <p className="text-orange-800 text-sm">
              Without Task Hash, a malicious worker could execute code honestly (generating a valid TDX quote), but then
              claim that same quote applies to a <em>different</em> execution with different input/output/WASM hashes.
              This would allow them to forge results while still presenting a cryptographically valid Intel signature.
            </p>
          </div>

          <div className="bg-green-50 border-l-4 border-green-500 p-4 my-4">
            <p className="text-green-900 font-semibold mb-2">The Solution:</p>
            <p className="text-green-800 text-sm mb-2">
              Every TDX quote contains a 64-byte <strong>REPORTDATA</strong> field where custom data can be embedded
              <em>before</em> Intel signs the quote. OutLayer uses the first 32 bytes of this field to store the
              <strong> Task Hash</strong> - a SHA256 cryptographic commitment to ALL execution parameters.
            </p>
            <p className="text-green-800 text-sm">
              Because the Task Hash is embedded in the quote <em>before signing</em>, Intel&apos;s signature covers it.
              This creates an unbreakable cryptographic link between the TDX quote and the specific execution parameters.
            </p>
          </div>

          <div className="bg-white border-2 border-purple-300 rounded-lg p-6 my-4">
            <h4 className="text-lg font-semibold text-purple-900 mb-3">Task Hash Algorithm:</h4>

            <p className="text-gray-700 text-sm mb-3">
              The Task Hash is calculated using <strong>binary concatenation + SHA256</strong>:
            </p>

            <div className="bg-gray-50 p-4 rounded border border-gray-300 font-mono text-xs mb-3">
              <p className="text-gray-900 mb-1">task_hash = SHA256(</p>
              <div className="pl-4 space-y-1">
                <p className="text-gray-700">task_type (UTF-8 string) +</p>
                <p className="text-gray-700">task_id (i64, little-endian) +</p>
                <p className="text-gray-700">repo_url (UTF-8 string, optional) +</p>
                <p className="text-gray-700">commit_hash (UTF-8 string, optional) +</p>
                <p className="text-gray-700">build_target (UTF-8 string, optional) +</p>
                <p className="text-gray-700">wasm_hash (hex string, optional) +</p>
                <p className="text-gray-700">input_hash (hex string, optional) +</p>
                <p className="text-gray-700">output_hash (hex string, always present) +</p>
                <p className="text-gray-700">block_height (u64, little-endian, optional)</p>
              </div>
              <p className="text-gray-900">)</p>
            </div>

            <div className="bg-yellow-50 border border-yellow-300 rounded p-3 mb-3">
              <p className="text-yellow-900 text-sm font-semibold mb-1">Important Details:</p>
              <ul className="list-disc list-inside space-y-1 text-yellow-800 text-xs">
                <li>Hashes (wasm_hash, input_hash, output_hash) are included as <strong>hex strings</strong> (e.g., &quot;abc123...&quot;), not decoded bytes</li>
                <li>Strings are UTF-8 encoded bytes</li>
                <li>Integers (task_id, block_height) are little-endian binary encoding</li>
                <li>Fields are concatenated in exact order - changing order produces different hash</li>
                <li>Optional fields are skipped if not present (no null bytes or placeholders)</li>
              </ul>
            </div>

            <div className="space-y-3">
              <div className="border-l-4 border-purple-500 pl-4">
                <p className="font-semibold text-gray-900 mb-1">Step 1: Worker Calculates Task Hash</p>
                <p className="text-gray-700 text-sm">
                  Before requesting a TDX quote from Intel hardware, the worker calculates the Task Hash by concatenating
                  all execution parameters (task type, repo, commit, wasm hash, input hash, output hash, etc.) and
                  computing SHA256. This produces a 32-byte hash.
                </p>
              </div>

              <div className="border-l-4 border-purple-500 pl-4">
                <p className="font-semibold text-gray-900 mb-1">Step 2: Embed in REPORTDATA</p>
                <p className="text-gray-700 text-sm">
                  The worker creates a 64-byte REPORTDATA buffer. The first 32 bytes are the Task Hash, and the remaining
                  32 bytes are zeros. This REPORTDATA is passed to Intel TDX hardware when requesting a quote.
                </p>
              </div>

              <div className="border-l-4 border-purple-500 pl-4">
                <p className="font-semibold text-gray-900 mb-1">Step 3: Intel Signs the Quote</p>
                <p className="text-gray-700 text-sm">
                  Intel TDX hardware generates the quote with the REPORTDATA embedded inside (at offset 568 in the quote
                  structure). Intel then signs the entire quote with its private key. This signature covers the REPORTDATA,
                  so the Task Hash is now cryptographically bound to Intel&apos;s signature.
                </p>
              </div>

              <div className="border-l-4 border-purple-500 pl-4">
                <p className="font-semibold text-gray-900 mb-1">Step 4: Verification</p>
                <p className="text-gray-700 text-sm">
                  Anyone can extract bytes [568:600] from the TDX quote (the first 32 bytes of REPORTDATA at offset 568)
                  to get the embedded Task Hash. They can also independently calculate the expected Task Hash from the
                  attestation parameters. If the hashes match, the attestation is genuine and cannot have been forged or swapped.
                </p>
              </div>
            </div>
          </div>

          <div className="bg-blue-50 border-l-4 border-blue-500 p-4 my-4">
            <p className="text-blue-900 font-semibold mb-2">Interactive Verification in Dashboard:</p>
            <p className="text-blue-800 text-sm mb-2">
              When you click <strong>&quot;Verify Quote&quot;</strong> in the attestation modal, the Dashboard performs
              full Task Hash verification:
            </p>
            <ol className="list-decimal list-inside space-y-1 text-blue-800 text-sm ml-2">
              <li>Extracts RTMR3 from TDX quote (offset 256, 48 bytes) - verifies worker identity</li>
              <li>Extracts Task Hash from REPORTDATA (offset 568, first 32 bytes) - extracts commitment</li>
              <li>Calculates expected Task Hash from attestation parameters - computes what it should be</li>
              <li>Compares extracted vs. expected hashes - validates cryptographic binding</li>
              <li>Shows ✓ green checkmark if both match, ✗ red error if mismatch</li>
            </ol>
            <p className="text-blue-800 text-sm mt-2">
              You can also click <strong>&quot;📊 Show Task Hash Calculation Steps&quot;</strong> to see the exact
              step-by-step breakdown of how the Task Hash is computed from the attestation data.
            </p>
          </div>

          <div className="bg-purple-50 border-l-4 border-purple-500 p-4 my-4">
            <p className="text-purple-900 font-semibold mb-2">Why This Matters:</p>
            <p className="text-purple-800 text-sm">
              Without Task Hash verification, a malicious worker could execute code once honestly (getting a valid TDX
              quote), then <strong>reuse that same quote</strong> for multiple executions with different inputs/outputs,
              claiming all of them were executed in TEE. The Task Hash makes this impossible because each execution has
              a unique Task Hash embedded in its quote. A quote from execution A cannot be used to validate execution B
              because their Task Hashes will differ.
            </p>
          </div>

          <div className="bg-white border-2 border-gray-300 rounded-lg p-4 my-4">
            <h4 className="text-md font-semibold text-gray-900 mb-2">Example: Preventing Quote Swapping</h4>
            <div className="space-y-2 text-sm">
              <div className="bg-red-50 border border-red-200 rounded p-2">
                <p className="text-red-900 font-semibold">❌ Attack (without Task Hash):</p>
                <p className="text-red-800 text-xs">
                  Worker honestly executes Task A (input=&quot;1&quot;, output=&quot;10&quot;) → gets valid TDX Quote A.
                  Then worker claims Quote A also proves execution of Task B (input=&quot;2&quot;, output=&quot;999&quot;) -
                  forging result &quot;999&quot; while presenting a cryptographically valid Intel signature.
                </p>
              </div>
              <div className="bg-green-50 border border-green-200 rounded p-2">
                <p className="text-green-900 font-semibold">✓ Defense (with Task Hash):</p>
                <p className="text-green-800 text-xs">
                  Quote A contains Task Hash A = SHA256(&quot;execute&quot; + &quot;1&quot; + ... + hash(&quot;10&quot;)).
                  Quote B would need Task Hash B = SHA256(&quot;execute&quot; + &quot;2&quot; + ... + hash(&quot;999&quot;)).
                  Since SHA256(A) ≠ SHA256(B), Quote A cannot be used for Task B. The verification fails immediately.
                </p>
              </div>
            </div>
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
