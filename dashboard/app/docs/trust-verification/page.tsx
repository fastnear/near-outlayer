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

        <p className="text-gray-700 mb-4">
          Whatever hardware a worker runs on, these three pillars hold. The next two sections show
          that the cryptographic guarantee is the same across both deployment methods, and how to
          verify any individual execution yourself.
        </p>
      </section>

      {/* Two ways workers run */}
      <section className="mb-12">
        <AnchorHeading id="deployment-methods">Two Ways Workers Run</AnchorHeading>

        <p className="text-gray-700 mb-4">
          OutLayer workers are deployed in two ways. <strong>Both are genuine Intel TDX</strong> —
          they differ only in who hosts the hardware and which portal renders the human-readable
          attestation.
        </p>

        <div className="grid grid-cols-1 md:grid-cols-2 gap-4 mb-6">
          <div className="border border-green-200 rounded-lg p-4 bg-green-50">
            <h4 className="font-semibold text-green-900 mb-2">Self-hosted TDX</h4>
            <p className="text-sm text-gray-700 mb-2">
              A self-hosted bare-metal dstack node. Human-verifiable attestation is rendered at{' '}
              <a href="https://workers.outlayer.ai" target="_blank" rel="noopener noreferrer" className="text-[var(--primary-orange)] hover:underline">
                workers.outlayer.ai
              </a>
              .
            </p>
          </div>
          <div className="border border-blue-200 rounded-lg p-4 bg-blue-50">
            <h4 className="font-semibold text-blue-900 mb-2">Phala Cloud</h4>
            <p className="text-sm text-gray-700 mb-2">
              Managed dstack hosted by{' '}
              <a href="https://phala.network" target="_blank" rel="noopener noreferrer" className="text-[var(--primary-orange)] hover:underline">
                Phala Cloud
              </a>
              . Human-verifiable attestation is rendered at{' '}
              <a href="https://trust.phala.com" target="_blank" rel="noopener noreferrer" className="text-[var(--primary-orange)] hover:underline">
                trust.phala.com
              </a>
              .
            </p>
          </div>
        </div>

        <p className="text-gray-700 mb-4">
          In both cases the worker runs in an Intel TDX confidential VM via dstack and emits a TDX
          quote that binds the worker&apos;s public key in <code>report_data</code> (the raw key for
          ed25519, or its SHA-256 for post-quantum ml-dsa-65) plus
          all 5 measurements. Both register on the <strong>same on-chain register-contract</strong>
          {' '}(<code>worker.outlayer.near</code> on mainnet, <code>worker.outlayer.testnet</code> on
          testnet), which verifies the Intel TDX quote signature (Intel DCAP) and checks the 5
          measurements against an admin allowlist.
        </p>

        <div className="bg-blue-50 border-l-4 border-blue-400 p-4 mb-6">
          <p className="text-sm text-blue-800">
            <strong>Workers complement each other — location doesn&apos;t matter.</strong> Workers from
            both deployment methods join the same pool; the coordinator dispatches each task to
            whichever worker is free, regardless of whether it runs on a self-hosted TDX node or on
            Phala Cloud. You don&apos;t pick a host, and you don&apos;t need to — every execution
            carries its own TDX attestation, so the result is equally verifiable no matter which
            worker produced it.
          </p>
        </div>
      </section>

      {/* Verifying each deployment */}
      <section className="mb-12">
        <AnchorHeading id="verify-deployments">Verifying Each Deployment</AnchorHeading>

        <p className="text-gray-700 mb-4">
          Side by side, the two deployment methods are cryptographically identical. Only the last
          two rows differ.
        </p>

        <div className="overflow-x-auto mb-6">
          <table className="min-w-full divide-y divide-gray-200">
            <thead className="bg-gray-50">
              <tr>
                <th className="px-4 py-3 text-left text-xs font-medium text-gray-500 uppercase">Aspect</th>
                <th className="px-4 py-3 text-left text-xs font-medium text-gray-500 uppercase">Self-hosted TDX</th>
                <th className="px-4 py-3 text-left text-xs font-medium text-gray-500 uppercase">Phala Cloud</th>
              </tr>
            </thead>
            <tbody className="bg-white divide-y divide-gray-200">
              <tr>
                <td colSpan={3} className="px-4 py-2 text-xs font-bold uppercase tracking-wide text-green-700 bg-green-50">Identical across both — the trust model</td>
              </tr>
              <tr>
                <td className="px-4 py-3 text-sm font-semibold text-gray-700">TEE hardware</td>
                <td className="px-4 py-3 text-sm text-gray-600">Intel TDX (Xeon)</td>
                <td className="px-4 py-3 text-sm text-gray-600">Intel TDX (Xeon)</td>
              </tr>
              <tr className="bg-gray-50">
                <td className="px-4 py-3 text-sm font-semibold text-gray-700">Runtime</td>
                <td className="px-4 py-3 text-sm text-gray-600">dstack confidential VM</td>
                <td className="px-4 py-3 text-sm text-gray-600">dstack confidential VM</td>
              </tr>
              <tr>
                <td className="px-4 py-3 text-sm font-semibold text-gray-700">Attestation</td>
                <td className="px-4 py-3 text-sm text-gray-600">Intel TDX quote (DCAP)</td>
                <td className="px-4 py-3 text-sm text-gray-600">Intel TDX quote (DCAP)</td>
              </tr>
              <tr className="bg-gray-50">
                <td className="px-4 py-3 text-sm font-semibold text-gray-700">Quote signature</td>
                <td className="px-4 py-3 text-sm text-gray-600">Intel-signed, verified via Intel DCAP</td>
                <td className="px-4 py-3 text-sm text-gray-600">Intel-signed, verified via Intel DCAP</td>
              </tr>
              <tr>
                <td className="px-4 py-3 text-sm font-semibold text-gray-700">Measurements</td>
                <td className="px-4 py-3 text-sm text-gray-600">MRTD + RTMR0&ndash;3 (5 registers)</td>
                <td className="px-4 py-3 text-sm text-gray-600">MRTD + RTMR0&ndash;3 (5 registers)</td>
              </tr>
              <tr className="bg-gray-50">
                <td className="px-4 py-3 text-sm font-semibold text-gray-700">On-chain registration</td>
                <td className="px-4 py-3 text-sm text-gray-600"><code>worker.outlayer.{'{near,testnet}'}</code> — same register-contract, 5-measurement allowlist</td>
                <td className="px-4 py-3 text-sm text-gray-600"><code>worker.outlayer.{'{near,testnet}'}</code> — same register-contract, 5-measurement allowlist</td>
              </tr>
              <tr>
                <td className="px-4 py-3 text-sm font-semibold text-gray-700">Secret custody</td>
                <td className="px-4 py-3 text-sm text-gray-600">NEAR MPC keystore (CKD)</td>
                <td className="px-4 py-3 text-sm text-gray-600">NEAR MPC keystore (CKD)</td>
              </tr>
              <tr>
                <td colSpan={3} className="px-4 py-2 text-xs font-bold uppercase tracking-wide text-orange-700 bg-orange-50">Differs — who operates the infrastructure (each component itself TDX-attested)</td>
              </tr>
              <tr>
                <td className="px-4 py-3 text-sm font-semibold text-gray-700">Who hosts the hardware</td>
                <td className="px-4 py-3 text-sm text-gray-700 font-medium">Bare-metal node</td>
                <td className="px-4 py-3 text-sm text-gray-700 font-medium">Phala Cloud</td>
              </tr>
              <tr className="bg-gray-50">
                <td className="px-4 py-3 text-sm font-semibold text-gray-700">Deploy-time KMS</td>
                <td className="px-4 py-3 text-sm text-gray-700 font-medium">Self-hosted dstack-kms (in TEE)</td>
                <td className="px-4 py-3 text-sm text-gray-700 font-medium">Phala&apos;s dstack KMS</td>
              </tr>
              <tr>
                <td className="px-4 py-3 text-sm font-semibold text-gray-700">Network gateway (TLS-in-TEE)</td>
                <td className="px-4 py-3 text-sm text-gray-700 font-medium"><code>gateway.dstack.outlayer.ai</code></td>
                <td className="px-4 py-3 text-sm text-gray-700 font-medium">Phala gateway</td>
              </tr>
              <tr className="bg-gray-50">
                <td className="px-4 py-3 text-sm font-semibold text-gray-700">Platform (FMSPC collateral)</td>
                <td className="px-4 py-3 text-sm text-gray-700 font-medium">Self-hosted node&apos;s FMSPC</td>
                <td className="px-4 py-3 text-sm text-gray-700 font-medium">Phala platform FMSPC</td>
              </tr>
              <tr>
                <td className="px-4 py-3 text-sm font-semibold text-gray-700">Attestation portal</td>
                <td className="px-4 py-3 text-sm text-gray-700 font-medium">
                  <a href="https://workers.outlayer.ai" target="_blank" rel="noopener noreferrer" className="text-[var(--primary-orange)] hover:underline">
                    workers.outlayer.ai
                  </a>
                </td>
                <td className="px-4 py-3 text-sm text-gray-700 font-medium">
                  <a href="https://trust.phala.com" target="_blank" rel="noopener noreferrer" className="text-[var(--primary-orange)] hover:underline">
                    trust.phala.com
                  </a>
                </td>
              </tr>
            </tbody>
          </table>
        </div>

        <div className="bg-green-50 border-l-4 border-green-400 p-4 mb-6">
          <p className="text-sm text-green-800">
            <strong>Takeaway:</strong> Every row that matters for the cryptographic guarantee — the
            TDX hardware, the Intel-signed quote, the 5 measurements, the on-chain register-contract,
            and NEAR-MPC secret custody — is <strong>identical</strong> on both. What differs is purely
            operational (who runs the hardware, KMS, and gateway), and each of those is itself a
            TDX-attested component with its own on-chain-approved measurements — not a trusted third
            party. Verify the self-hosted node at{' '}
            <a href="https://workers.outlayer.ai" target="_blank" rel="noopener noreferrer" className="underline">
              workers.outlayer.ai
            </a>
            {' '}and Phala Cloud at{' '}
            <a href="https://trust.phala.com" target="_blank" rel="noopener noreferrer" className="underline">
              trust.phala.com
            </a>
            .
          </p>
        </div>
      </section>

      {/* Verify any execution */}
      <section className="mb-12">
        <AnchorHeading id="verify-execution">Verify Any Execution</AnchorHeading>

        <p className="text-gray-700 mb-4">
          Beyond verifying that a worker is genuine, you can verify <strong>any individual
          execution</strong> — for both NEAR blockchain and HTTPS calls — directly from the
          dashboard.
        </p>

        <div className="bg-white border-2 border-purple-300 rounded-lg p-6 mb-6">
          <ol className="space-y-3">
            <li className="flex items-start">
              <span className="inline-flex items-center justify-center w-6 h-6 rounded-full bg-purple-500 text-white text-sm font-bold mr-3 flex-shrink-0">1</span>
              <div>
                <p className="font-semibold text-gray-900">Open Executions</p>
                <p className="text-gray-700 text-sm">
                  Go to <Link href="/executions" className="text-[var(--primary-orange)] hover:underline">Executions</Link>
                  {' '}(the page is titled <strong>Job History</strong>).
                </p>
              </div>
            </li>
            <li className="flex items-start">
              <span className="inline-flex items-center justify-center w-6 h-6 rounded-full bg-purple-500 text-white text-sm font-bold mr-3 flex-shrink-0">2</span>
              <div>
                <p className="font-semibold text-gray-900">Click the shield-check icon</p>
                <p className="text-gray-700 text-sm">
                  In the <strong>TEE</strong> column, click the <strong>shield-check</strong> icon
                  (hover tooltip <em>View attestation report</em>) on any row. This works for both
                  NEAR blockchain and HTTPS executions.
                </p>
              </div>
            </li>
            <li className="flex items-start">
              <span className="inline-flex items-center justify-center w-6 h-6 rounded-full bg-purple-500 text-white text-sm font-bold mr-3 flex-shrink-0">3</span>
              <div>
                <p className="font-semibold text-gray-900">Read the TEE Attestation report</p>
                <p className="text-gray-700 text-sm">
                  The <strong>TEE Attestation</strong> report opens, showing the worker measurement,
                  source code, hashes, and the raw quote (detailed below).
                </p>
              </div>
            </li>
          </ol>
        </div>

        <AnchorHeading id="report-contents" level={3}>What the Report Shows</AnchorHeading>

        <div className="overflow-x-auto mb-6">
          <table className="min-w-full divide-y divide-gray-200">
            <thead className="bg-gray-50">
              <tr>
                <th className="px-4 py-3 text-left text-xs font-medium text-gray-500 uppercase">Field</th>
                <th className="px-4 py-3 text-left text-xs font-medium text-gray-500 uppercase">What It Proves</th>
              </tr>
            </thead>
            <tbody className="bg-white divide-y divide-gray-200">
              <tr>
                <td className="px-4 py-3 text-sm font-semibold text-gray-700">Worker Measurement (RTMR3)</td>
                <td className="px-4 py-3 text-sm text-gray-600">The TEE environment hash. Ties the run to a registered worker.</td>
              </tr>
              <tr className="bg-gray-50">
                <td className="px-4 py-3 text-sm font-semibold text-gray-700">Source Code</td>
                <td className="px-4 py-3 text-sm text-gray-600">Link to the exact <code>repo@commit</code> that was executed.</td>
              </tr>
              <tr>
                <td className="px-4 py-3 text-sm font-semibold text-gray-700">WASM Hash / Input Hash / Output Hash</td>
                <td className="px-4 py-3 text-sm text-gray-600">Content-addressable hashes of the binary, the input, and the output.</td>
              </tr>
              <tr className="bg-gray-50">
                <td className="px-4 py-3 text-sm font-semibold text-gray-700">TDX Quote</td>
                <td className="px-4 py-3 text-sm text-gray-600">The raw Intel-signed quote, plus a verify button (below).</td>
              </tr>
            </tbody>
          </table>
        </div>

        <p className="text-gray-700 mb-4">
          The report includes a <strong>&quot;Verify Quote (RTMR3 + Task Hash)&quot;</strong> button.
          Client-side, it extracts <strong>RTMR3</strong> (the worker measurement) and the{' '}
          <strong>Task Hash from REPORTDATA</strong> and checks them. The Task Hash commits to this
          execution&apos;s input, output, WASM, and commit — so the Intel-signed quote cannot be
          reused for a different run.
        </p>

        <div className="bg-purple-50 border-l-4 border-purple-400 p-4 mb-6">
          <p className="text-sm text-purple-800">
            <strong>Input/Output verification:</strong> For NEAR jobs, click{' '}
            <strong>&quot;Load &amp; Verify from Blockchain&quot;</strong> — it fetches the
            transaction, re-hashes the input/output, and compares against the report. For HTTPS jobs,
            paste the request/response to verify them against the hashes.
          </p>
        </div>

        <p className="text-gray-700 mb-4">
          For NEAR jobs, a <strong>Direct Link</strong> opens a shareable standalone page at{' '}
          <code>/attestation/{'{jobId}'}</code>.
        </p>

        <div className="bg-green-50 border-l-4 border-green-400 p-4 mb-6">
          <p className="text-sm text-green-800">
            <strong>Takeaway:</strong> Anyone can open any execution, read the worker measurement,
            find the exact sources, and confirm the action ran inside a TEE.
          </p>
        </div>

        <div className="bg-yellow-50 border-l-4 border-yellow-500 p-4 mb-6">
          <p className="text-sm text-yellow-800">
            <strong>Not to be conflated:</strong> This per-execution view surfaces{' '}
            <strong>RTMR3 + the task-hash binding</strong>. The full <strong>5-measurement</strong>
            {' '}check (MRTD + RTMR0&ndash;3) happens once at <strong>worker registration
            on-chain</strong>, not per execution. See the next section for the full measurement set.
          </p>
        </div>
      </section>

      {/* 5-Measurement Verification */}
      <section className="mb-12">
        <AnchorHeading id="measurements">5-Measurement TDX Verification</AnchorHeading>

        <p className="text-gray-700 mb-4">
          Intel TDX produces 5 cryptographic measurements that together uniquely identify the TEE environment.
          The register-contract (<code>worker.outlayer.near</code>) verifies <strong>all 5</strong> at
          worker registration, for both deployment methods:
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
          <li>Find the release matching the version running in the TEE</li>
          <li>Check the Sigstore certification badge on the release</li>
          <li>Review the source code at that release tag</li>
          <li>Optionally: rebuild from source and compare the hash</li>
        </ol>
      </section>

      {/* Worker Registration */}
      <section className="mb-12">
        <AnchorHeading id="registration-flow">Worker Registration Flow</AnchorHeading>

        <p className="text-gray-700 mb-4">
          Every worker — self-hosted or on Phala Cloud — must prove its TEE identity before it can
          execute code or access secrets:
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
                  Worker calls <code>register_worker_key()</code> on the register-contract (<code>worker.outlayer.near</code>). The contract verifies
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
                <td className="px-4 py-3 text-sm text-gray-600">Register-contract verifies TDX quote before adding access key; every registration is visible on-chain</td>
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
