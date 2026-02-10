'use client';

import Link from 'next/link';
import { AnchorHeading, useHashNavigation } from './utils';

export default function SecretsSection() {
  useHashNavigation();

  return (
    <div className="prose max-w-none">
      <h2 className="text-3xl font-bold mb-6 text-[var(--primary-orange)]">Secrets</h2>

      <div className="bg-blue-50 p-4 rounded-lg mb-6">
        <p className="text-blue-900 font-semibold mb-2">Enterprise-Grade Security with CKD & MPC Network</p>
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
                  <li className="text-amber-700">Cannot use <code className="bg-amber-100 px-1 rounded">PROTECTED_*</code> prefix (reserved for auto-generated)</li>
                </ul>
              </div>

              <div className="border-l-4 border-green-400 pl-3">
                <p className="font-semibold text-gray-800 mb-1">2. Auto-Generated Secrets</p>
                <p className="text-sm text-gray-700 mb-2">Generate cryptographically secure secrets in TEE without seeing their values</p>
                <ul className="list-disc list-inside text-sm text-gray-700 ml-4">
                  <li>Generated inside TEE (nobody ever sees the value)</li>
                  <li>Perfect for derivation keys, signing keys, encryption keys</li>
                  <li className="text-green-700">Must start with <code className="bg-green-100 px-1 rounded">PROTECTED_*</code> prefix (proves TEE generation)</li>
                  <li>Example: <code className="bg-gray-100 px-2 py-1 rounded">PROTECTED_MASTER_KEY</code></li>
                  <li>Types: hex32/64, ED25519, password:N</li>
                </ul>
              </div>
            </div>

            <div className="mt-3 p-3 bg-blue-50 border border-blue-200 rounded">
              <p className="text-sm text-blue-900 font-medium mb-1">Naming Convention for Trust</p>
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
                <li className="text-amber-700"><strong>Wildcard:</strong> Leave branch empty for secrets not tied to a specific branch</li>
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

            <div id="project-binding" className="border-l-4 border-green-400 pl-3 scroll-mt-4">
              <p className="font-semibold text-gray-800 mb-1">Project-based</p>
              <p className="text-sm text-gray-700 mb-2">Bind secrets to a Project - accessible by all versions</p>
              <ul className="list-disc list-inside text-sm text-gray-700 ml-4">
                <li>Key: <code className="bg-gray-100 px-1 rounded">project_id + profile + owner</code></li>
                <li>Example: <code className="bg-gray-100 px-1 rounded">alice.near/my-app:production</code></li>
                <li>Best for: Long-running projects with multiple versions</li>
                <li>Benefit: Secrets persist across version updates - no re-creation needed</li>
              </ul>
            </div>
          </div>

          <div className="mt-3 p-3 bg-green-50 border border-green-200 rounded">
            <p className="text-sm text-green-900 font-medium mb-1">Project Binding Recommendation</p>
            <p className="text-xs text-green-800">
              For most use cases, <strong>Project binding</strong> is recommended. It allows you to update your WASM code
              without re-creating secrets. Create a project in the <a href="/projects" className="underline">Projects dashboard</a>,
              then bind your secrets to that project.
            </p>
          </div>

          <div className="mt-3 p-3 bg-purple-50 border border-purple-200 rounded">
            <p className="text-sm text-purple-900 font-medium mb-1">WASM Hash Binding Security</p>
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
                  <text x="200" y="25" textAnchor="middle" fontSize="14" fontWeight="bold" fill="#166534">Registration Complete</text>
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
                  <text x="80" y="52" textAnchor="middle" fontSize="10" fill="#7c2d12" fontWeight="bold">Has limited key</text>
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
                  Deterministic  |  Persistent  |  Survives restarts  |  No single point of failure
                </text>
              </svg>
            </div>

            {/* Mobile Simplified View with ASCII Diagrams */}
            <div className="md:hidden bg-white border-2 border-gray-300 rounded-lg p-4 mb-3">
              <div className="space-y-6">
                {/* Phase 1: Registration */}
                <div className="bg-amber-50 p-4 rounded-lg border-2 border-amber-400">
                  <h4 className="font-bold text-amber-900 mb-3">Phase 1: Registration (One-time)</h4>

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
                  <h4 className="font-bold text-blue-900 mb-3">Phase 2: CKD Flow (Repeatable)</h4>

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
                  <h4 className="font-semibold text-green-900 mb-2 text-sm">Key Properties</h4>
                  <div className="grid grid-cols-2 gap-2 text-xs">
                    <div className="flex items-center gap-1">
                      <span className="text-green-600">&#10003;</span>
                      <span>Deterministic</span>
                    </div>
                    <div className="flex items-center gap-1">
                      <span className="text-green-600">&#10003;</span>
                      <span>Persistent</span>
                    </div>
                    <div className="flex items-center gap-1">
                      <span className="text-green-600">&#10003;</span>
                      <span>Survives restarts</span>
                    </div>
                    <div className="flex items-center gap-1">
                      <span className="text-green-600">&#10003;</span>
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
              <h4 className="font-semibold text-green-900 mb-2">Key Properties</h4>
              <ul className="text-sm text-green-800 space-y-1">
                <li>&#8226; <strong>Deterministic:</strong> Same app_id always gets same secret</li>
                <li>&#8226; <strong>Private:</strong> Secret known only to TEE app</li>
                <li>&#8226; <strong>Distributed:</strong> No single MPC node has the secret</li>
                <li>&#8226; <strong>Persistent:</strong> Works across TEE restarts</li>
              </ul>
            </div>

            <div className="border rounded-lg p-4 bg-blue-50">
              <h4 className="font-semibold text-blue-900 mb-2">Security Guarantees</h4>
              <ul className="text-sm text-blue-800 space-y-1">
                <li>&#8226; BLS signatures on BLS12-381 curves</li>
                <li>&#8226; ElGamal encryption for transport</li>
                <li>&#8226; TEE attestation verification</li>
                <li>&#8226; Threshold cryptography (t-of-n)</li>
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
            <h4 className="font-semibold text-purple-900 mb-2">DAO Controls Keystore Access to MPC</h4>
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
            <h4 className="font-semibold text-blue-900 mb-2">Cryptographic Properties</h4>
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
              <h4 className="font-semibold text-gray-900 mb-2">Security Properties</h4>
              <ul className="text-sm text-gray-700 space-y-1">
                <li>&#8226; <strong>No single point of failure:</strong> Distributed MPC nodes</li>
                <li>&#8226; <strong>Forward secrecy:</strong> Fresh key pair for each request</li>
                <li>&#8226; <strong>TEE isolation:</strong> Secrets computed inside enclave</li>
                <li>&#8226; <strong>Threshold security:</strong> Requires multiple nodes</li>
              </ul>
            </div>

            <div className="border rounded-lg p-4">
              <h4 className="font-semibold text-gray-900 mb-2">Trust Model</h4>
              <ul className="text-sm text-gray-700 space-y-1">
                <li>&#8226; Intel TDX attestation verification</li>
                <li>&#8226; MPC network consensus</li>
                <li>&#8226; Smart contract enforcement</li>
                <li>&#8226; Cryptographic correctness proofs</li>
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
