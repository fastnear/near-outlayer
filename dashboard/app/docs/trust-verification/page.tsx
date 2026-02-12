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

export default function TrustVerificationPage() {
  return (
    <div className="prose prose-lg max-w-none">
      <h1 className="text-3xl font-bold text-gray-900 mb-6">Why Trust OutLayer?</h1>

      <p className="text-gray-700 mb-8 text-lg">
        OutLayer runs your code inside Intel TDX confidential VMs where even the operator cannot
        access your secrets or tamper with execution. Here&apos;s how to verify this yourself.
      </p>

      {/* Overview */}
      <section className="mb-12">
        <AnchorHeading id="overview">Trust Architecture</AnchorHeading>

        <p className="text-gray-700 mb-4">
          OutLayer&apos;s security rests on three independently verifiable pillars:
        </p>

        <div className="grid grid-cols-1 md:grid-cols-3 gap-4 mb-6">
          <div className="border border-blue-200 rounded-lg p-4 bg-blue-50">
            <h4 className="font-semibold text-blue-900 mb-2">1. Hardware (Intel TDX)</h4>
            <p className="text-sm text-gray-700">
              Confidential VMs with hardware-encrypted memory. Host OS cannot read worker memory.
            </p>
          </div>
          <div className="border border-green-200 rounded-lg p-4 bg-green-50">
            <h4 className="font-semibold text-green-900 mb-2">2. Open Source Code</h4>
            <p className="text-sm text-gray-700">
              All worker code is public on GitHub with Sigstore-certified release binaries.
            </p>
          </div>
          <div className="border border-purple-200 rounded-lg p-4 bg-purple-50">
            <h4 className="font-semibold text-purple-900 mb-2">3. On-Chain Verification</h4>
            <p className="text-sm text-gray-700">
              NEAR smart contract verifies TDX quotes and stores approved measurements on-chain.
            </p>
          </div>
        </div>
      </section>

      {/* Phala Trust Center */}
      <section className="mb-12">
        <AnchorHeading id="phala-trust-center">Phala Trust Center</AnchorHeading>

        <p className="text-gray-700 mb-4">
          OutLayer workers run on{' '}
          <a href="https://phala.network" target="_blank" rel="noopener noreferrer" className="text-[var(--primary-orange)] hover:underline">
            Phala Cloud
          </a>
          , which provides a Trust Center for verifying deployed applications.
        </p>

        <div className="bg-blue-50 border-l-4 border-blue-400 p-4 mb-6">
          <p className="text-sm text-blue-800 mb-2">
            <strong>How to verify:</strong> Visit the Phala Trust Center for the OutLayer apps.
            The page shows the exact <strong>Docker image hash</strong> running in the TEE. This hash
            corresponds to a specific GitHub release.
          </p>
          <ul className="text-sm text-blue-800 list-disc list-inside space-y-1">
            <li>
              <strong>Worker:</strong>{' '}
              <a href="https://trust.phala.com/app/dc3959bdba3f3681415a3022590cf434bb599a01?selected=app-code" target="_blank" rel="noopener noreferrer" className="underline">
                dc3959bdba3f3681415a3022590cf434bb599a01
              </a>
            </li>
            <li>
              <strong>Keystore:</strong>{' '}
              <a href="https://trust.phala.com/app/5319e38108c14ed325d1a22e8815527320aa3407?selected=app-code" target="_blank" rel="noopener noreferrer" className="underline">
                5319e38108c14ed325d1a22e8815527320aa3407
              </a>
            </li>
          </ul>
          <p className="text-xs text-blue-600 mt-2">
            App IDs change when workers are upgraded to a new version.
          </p>
        </div>

        <AnchorHeading id="image-hash" level={3}>What the Image Hash Proves</AnchorHeading>

        <p className="text-gray-700 mb-4">
          The Docker image hash shown in Phala Trust Center is a content-addressable identifier.
          It proves that the exact binary code running inside the TEE matches a specific build.
          You can verify this by:
        </p>

        <ol className="list-decimal list-inside text-gray-700 space-y-2 mb-6">
          <li>Checking the image hash on Phala Trust Center</li>
          <li>Finding the corresponding GitHub release with the same hash</li>
          <li>Verifying the release has Sigstore certification (see below)</li>
          <li>Inspecting the source code at that release tag</li>
        </ol>
      </section>

      {/* GitHub Releases & Sigstore */}
      <section className="mb-12">
        <AnchorHeading id="sigstore">GitHub Releases & Sigstore</AnchorHeading>

        <p className="text-gray-700 mb-4">
          OutLayer publishes releases on GitHub with{' '}
          <a href="https://www.sigstore.dev/" target="_blank" rel="noopener noreferrer" className="text-[var(--primary-orange)] hover:underline">
            Sigstore
          </a>
          {' '}certification. Sigstore provides cryptographic proof that a binary was built from specific source code.
        </p>

        <div className="bg-green-50 border-l-4 border-green-400 p-4 mb-6">
          <p className="text-sm text-green-800">
            <strong>What Sigstore proves:</strong> The release binary was built by GitHub Actions CI
            from the exact source code at that git tag. No one — not even the project maintainers —
            can substitute a different binary without the Sigstore signature failing.
          </p>
        </div>

        <AnchorHeading id="verify-release" level={3}>How to Verify a Release</AnchorHeading>

        <ol className="list-decimal list-inside text-gray-700 space-y-2 mb-6">
          <li>
            Go to{' '}
            <a href="https://github.com/fastnear/near-outlayer/releases" target="_blank" rel="noopener noreferrer" className="text-[var(--primary-orange)] hover:underline">
              github.com/fastnear/near-outlayer/releases
            </a>
          </li>
          <li>Find the release matching the version/hash from Phala Trust Center</li>
          <li>Check the Sigstore certification badge on the release</li>
          <li>Review the source code at that release tag</li>
          <li>Optionally: rebuild from source and compare the hash</li>
        </ol>
      </section>

      {/* 5-Measurement Verification */}
      <section className="mb-12">
        <AnchorHeading id="measurements">5-Measurement TDX Verification</AnchorHeading>

        <p className="text-gray-700 mb-4">
          Intel TDX produces 5 cryptographic measurements that together uniquely identify the TEE environment.
          The operator contract (<code>worker.outlayer.near</code>) verifies <strong>all 5</strong>:
        </p>

        <div className="overflow-x-auto mb-6">
          <table className="min-w-full divide-y divide-gray-200">
            <thead className="bg-gray-50">
              <tr>
                <th className="px-4 py-3 text-left text-xs font-medium text-gray-500 uppercase">Measurement</th>
                <th className="px-4 py-3 text-left text-xs font-medium text-gray-500 uppercase">What It Measures</th>
                <th className="px-4 py-3 text-left text-xs font-medium text-gray-500 uppercase">Size</th>
              </tr>
            </thead>
            <tbody className="bg-white divide-y divide-gray-200">
              <tr>
                <td className="px-4 py-3 text-sm font-mono font-semibold">MRTD</td>
                <td className="px-4 py-3 text-sm text-gray-600">TD (Trust Domain) measurement — code + configuration</td>
                <td className="px-4 py-3 text-sm text-gray-600">48 bytes (96 hex chars)</td>
              </tr>
              <tr className="bg-gray-50">
                <td className="px-4 py-3 text-sm font-mono font-semibold">RTMR0</td>
                <td className="px-4 py-3 text-sm text-gray-600">Firmware measurement</td>
                <td className="px-4 py-3 text-sm text-gray-600">48 bytes (96 hex chars)</td>
              </tr>
              <tr>
                <td className="px-4 py-3 text-sm font-mono font-semibold">RTMR1</td>
                <td className="px-4 py-3 text-sm text-gray-600">OS/kernel measurement</td>
                <td className="px-4 py-3 text-sm text-gray-600">48 bytes (96 hex chars)</td>
              </tr>
              <tr className="bg-gray-50">
                <td className="px-4 py-3 text-sm font-mono font-semibold">RTMR2</td>
                <td className="px-4 py-3 text-sm text-gray-600">Application measurement</td>
                <td className="px-4 py-3 text-sm text-gray-600">48 bytes (96 hex chars)</td>
              </tr>
              <tr>
                <td className="px-4 py-3 text-sm font-mono font-semibold">RTMR3</td>
                <td className="px-4 py-3 text-sm text-gray-600">Runtime measurement</td>
                <td className="px-4 py-3 text-sm text-gray-600">48 bytes (96 hex chars)</td>
              </tr>
            </tbody>
          </table>
        </div>

        <div className="bg-yellow-50 border-l-4 border-yellow-500 p-4 mb-6">
          <p className="text-sm text-yellow-800">
            <strong>Why all 5 matter:</strong> Checking only RTMR3 (as some systems do) is not sufficient.
            A development dstack image with SSH access enabled would have a different MRTD/RTMR0/RTMR1 but
            could share the same RTMR3. By checking all 5 measurements, OutLayer ensures the entire
            environment — from firmware to application — matches the approved configuration.
          </p>
        </div>

        <AnchorHeading id="check-onchain" level={3}>Check Approved Measurements On-Chain</AnchorHeading>

        <SyntaxHighlighter language="bash" style={vscDarkPlus} className="rounded-lg mb-4">
          {`# View all approved measurement sets
near view worker.outlayer.near get_approved_measurements

# Check if specific measurements are approved
near view worker.outlayer.near is_measurements_approved '{
  "measurements": {
    "mrtd": "abc123...",
    "rtmr0": "def456...",
    "rtmr1": "ghi789...",
    "rtmr2": "jkl012...",
    "rtmr3": "mno345..."
  }
}'`}
        </SyntaxHighlighter>
      </section>

      {/* Worker Registration */}
      <section className="mb-12">
        <AnchorHeading id="registration-flow">Worker Registration Flow</AnchorHeading>

        <p className="text-gray-700 mb-4">
          Every worker must prove its TEE identity before it can execute code or access secrets:
        </p>

        <div className="bg-white border-2 border-purple-300 rounded-lg p-6 mb-6">
          <ol className="space-y-3">
            <li className="flex items-start">
              <span className="inline-flex items-center justify-center w-6 h-6 rounded-full bg-purple-500 text-white text-sm font-bold mr-3 flex-shrink-0">1</span>
              <div>
                <p className="font-semibold text-gray-900">Generate Keypair in TEE</p>
                <p className="text-gray-700 text-sm">
                  Worker generates an ed25519 keypair inside the TDX confidential VM. The private key never leaves TEE memory.
                </p>
              </div>
            </li>
            <li className="flex items-start">
              <span className="inline-flex items-center justify-center w-6 h-6 rounded-full bg-purple-500 text-white text-sm font-bold mr-3 flex-shrink-0">2</span>
              <div>
                <p className="font-semibold text-gray-900">Generate TDX Quote</p>
                <p className="text-gray-700 text-sm">
                  TDX hardware produces a cryptographic quote with the worker&apos;s public key in <code>report_data</code>
                  and all 5 measurements. The quote is signed by Intel.
                </p>
              </div>
            </li>
            <li className="flex items-start">
              <span className="inline-flex items-center justify-center w-6 h-6 rounded-full bg-purple-500 text-white text-sm font-bold mr-3 flex-shrink-0">3</span>
              <div>
                <p className="font-semibold text-gray-900">On-Chain Verification</p>
                <p className="text-gray-700 text-sm">
                  Worker calls <code>register_worker_key()</code> on the operator contract (<code>worker.outlayer.near</code>). The contract verifies
                  the Intel signature, extracts all 5 measurements, checks them against the approved list,
                  and confirms the public key matches <code>report_data</code>.
                </p>
              </div>
            </li>
            <li className="flex items-start">
              <span className="inline-flex items-center justify-center w-6 h-6 rounded-full bg-purple-500 text-white text-sm font-bold mr-3 flex-shrink-0">4</span>
              <div>
                <p className="font-semibold text-gray-900">Scoped Access Key</p>
                <p className="text-gray-700 text-sm">
                  The contract adds the worker&apos;s public key as an access key scoped to specific methods only:
                  <code> resolve_execution</code>, <code>submit_execution_output_and_resolve</code>,
                  <code> resume_topup</code>, <code>resume_delete_payment_key</code>.
                </p>
              </div>
            </li>
          </ol>
        </div>
      </section>

      {/* Ephemeral Worker Keys */}
      <section className="mb-12">
        <AnchorHeading id="ephemeral-keys">Ephemeral Worker Keys & Blockchain Trail</AnchorHeading>

        <p className="text-gray-700 mb-4">
          Worker signing keys are <strong>ephemeral</strong> — they are generated fresh inside the TEE on every
          restart and <strong>never saved to disk or exported</strong>. When a worker restarts, it generates a
          completely new keypair and re-registers on the blockchain.
        </p>

        <div className="bg-green-50 border-l-4 border-green-400 p-4 mb-6">
          <p className="text-sm text-green-800">
            <strong>Why this matters:</strong> Every worker registration leaves a permanent trail on the blockchain.
            The operator cannot secretly spin up a worker, request a key, or access secrets without it being
            visible on-chain. If an admin tried to run unauthorized code, it would fail the 5-measurement check
            during registration — and even if they used legitimate code, the registration transaction would be
            publicly visible.
          </p>
        </div>

        <p className="text-gray-700 mb-4">
          Additionally, every worker&apos;s WASM code is tracked through GitHub — you can verify exactly which code
          a worker executed by checking the source repository and commit hash. This means even a backdoor cannot
          be introduced without leaving a visible trace in the git history.
        </p>
      </section>

      {/* CKD: Deterministic Keystore Secrets */}
      <section className="mb-12">
        <AnchorHeading id="ckd">Deterministic Keystore Secrets (CKD)</AnchorHeading>

        <p className="text-gray-700 mb-4">
          The keystore worker uses <strong>Confidential Key Derivation (CKD)</strong> via NEAR&apos;s MPC network
          to derive a deterministic master secret. This means that when the keystore restarts or upgrades,
          it recovers the same master secret — all previously encrypted secrets remain accessible.
        </p>

        <div className="bg-white border-2 border-blue-300 rounded-lg p-6 mb-6">
          <ol className="space-y-3">
            <li className="flex items-start">
              <span className="inline-flex items-center justify-center w-6 h-6 rounded-full bg-blue-500 text-white text-sm font-bold mr-3 flex-shrink-0">1</span>
              <div>
                <p className="font-semibold text-gray-900">TEE Startup</p>
                <p className="text-gray-700 text-sm">
                  Keystore generates a new ephemeral keypair in TEE memory and submits TDX attestation to the DAO contract.
                </p>
              </div>
            </li>
            <li className="flex items-start">
              <span className="inline-flex items-center justify-center w-6 h-6 rounded-full bg-blue-500 text-white text-sm font-bold mr-3 flex-shrink-0">2</span>
              <div>
                <p className="font-semibold text-gray-900">DAO Approval</p>
                <p className="text-gray-700 text-sm">
                  DAO members (<code>dao.outlayer.near</code>) vote to approve the keystore&apos;s public key, confirming its TEE attestation.
                </p>
              </div>
            </li>
            <li className="flex items-start">
              <span className="inline-flex items-center justify-center w-6 h-6 rounded-full bg-blue-500 text-white text-sm font-bold mr-3 flex-shrink-0">3</span>
              <div>
                <p className="font-semibold text-gray-900">MPC Key Derivation</p>
                <p className="text-gray-700 text-sm">
                  After approval, keystore requests its master secret from the NEAR MPC network using BLS12-381 key exchange.
                  The derived secret is deterministic — same DAO account + same derivation path always produces the same secret.
                </p>
              </div>
            </li>
            <li className="flex items-start">
              <span className="inline-flex items-center justify-center w-6 h-6 rounded-full bg-blue-500 text-white text-sm font-bold mr-3 flex-shrink-0">4</span>
              <div>
                <p className="font-semibold text-gray-900">Secret Recovery</p>
                <p className="text-gray-700 text-sm">
                  The master secret exists only in TEE memory (never persisted to disk). All per-project keys are
                  derived from it using HMAC-SHA256. On restart, the same master secret is re-derived, so all
                  secrets are automatically recoverable.
                </p>
              </div>
            </li>
          </ol>
        </div>
      </section>

      {/* What Operator Cannot Do */}
      <section className="mb-12">
        <AnchorHeading id="operator-limits">What the Operator Cannot Do</AnchorHeading>

        <p className="text-gray-700 mb-4">
          Even if the operator is malicious or compromised, Intel TDX hardware prevents:
        </p>

        <div className="overflow-x-auto mb-6">
          <table className="min-w-full divide-y divide-gray-200">
            <thead className="bg-gray-50">
              <tr>
                <th className="px-4 py-3 text-left text-xs font-medium text-gray-500 uppercase">Action</th>
                <th className="px-4 py-3 text-left text-xs font-medium text-gray-500 uppercase">Protection</th>
              </tr>
            </thead>
            <tbody className="bg-white divide-y divide-gray-200">
              <tr>
                <td className="px-4 py-3 text-sm text-gray-600">Extract decrypted secrets</td>
                <td className="px-4 py-3 text-sm text-gray-600">TEE memory encryption — host OS cannot read worker memory</td>
              </tr>
              <tr className="bg-gray-50">
                <td className="px-4 py-3 text-sm text-gray-600">Modify execution results</td>
                <td className="px-4 py-3 text-sm text-gray-600">Results signed with TEE-generated key registered on-chain</td>
              </tr>
              <tr>
                <td className="px-4 py-3 text-sm text-gray-600">Run different code</td>
                <td className="px-4 py-3 text-sm text-gray-600">All 5 TDX measurements must match approved set</td>
              </tr>
              <tr className="bg-gray-50">
                <td className="px-4 py-3 text-sm text-gray-600">Forge attestation reports</td>
                <td className="px-4 py-3 text-sm text-gray-600">TDX quotes signed by Intel&apos;s private key (hardware-embedded)</td>
              </tr>
              <tr>
                <td className="px-4 py-3 text-sm text-gray-600">Register unauthorized worker</td>
                <td className="px-4 py-3 text-sm text-gray-600">Operator contract verifies TDX quote before adding access key; every registration is visible on-chain</td>
              </tr>
            </tbody>
          </table>
        </div>

        <div className="bg-orange-50 border-l-4 border-orange-400 p-4 mb-6">
          <p className="text-sm text-orange-800">
            <strong>What the operator CAN do:</strong> Refuse to execute code (censorship) or shut down
            infrastructure (availability). These are mitigated by the ability to run multiple independent
            operators — workers are stateless and can be redeployed anywhere.
          </p>
        </div>
      </section>

      {/* Related */}
      <section className="bg-gray-50 rounded-lg p-6">
        <h3 className="text-lg font-semibold text-gray-900 mb-4">Related Documentation</h3>
        <ul className="space-y-2 text-sm">
          <li>
            <Link href="/docs/tee-attestation" className="text-[var(--primary-orange)] hover:underline">TEE Attestation</Link>
            {' '}- Technical details of execution attestation
          </li>
          <li>
            <Link href="/docs/secrets" className="text-[var(--primary-orange)] hover:underline">Secrets</Link>
            {' '}- How secrets are encrypted and decrypted in TEE
          </li>
          <li>
            <a href="https://github.com/fastnear/near-outlayer/releases" target="_blank" rel="noopener noreferrer" className="text-[var(--primary-orange)] hover:underline">
              GitHub Releases
            </a>
            {' '}- Release binaries with Sigstore certification
          </li>
        </ul>
      </section>
    </div>
  );
}
