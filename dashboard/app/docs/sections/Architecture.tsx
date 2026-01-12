'use client';

import { AnchorHeading, useHashNavigation } from './utils';

export default function ArchitectureSection() {
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
