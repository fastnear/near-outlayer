'use client';

import Link from 'next/link';
import { Prism as SyntaxHighlighter } from 'react-syntax-highlighter';
import { vscDarkPlus } from 'react-syntax-highlighter/dist/esm/styles/prism';

// Anchor heading component for hash navigation
function AnchorHeading({ id, children, level = 2 }: { id: string; children: React.ReactNode; level?: 2 | 3 | 4 }) {
  const sizeClass = level === 2 ? 'text-2xl' : level === 3 ? 'text-xl' : 'text-lg';
  const className = `${sizeClass} font-bold text-gray-900 mb-4 scroll-mt-4 group`;
  const anchor = (
    <a href={`#${id}`} className="ml-2 text-gray-400 hover:text-[var(--primary-orange)] opacity-0 group-hover:opacity-100 transition-opacity">
      #
    </a>
  );

  if (level === 3) {
    return <h3 id={id} className={className}>{children}{anchor}</h3>;
  }
  if (level === 4) {
    return <h4 id={id} className={className}>{children}{anchor}</h4>;
  }
  return <h2 id={id} className={className}>{children}{anchor}</h2>;
}

export default function ProjectsPage() {
  return (
    <div className="prose prose-lg max-w-none">
      <h1 className="text-3xl font-bold text-gray-900 mb-6">
        Projects
      </h1>

      <p className="text-gray-700 mb-8">
        Projects allow you to organize your WASM code with version management, persistent storage, and project-level secrets.
        All versions of a project share the same resources, enabling seamless updates.
      </p>

      {/* What are Projects */}
      <section className="mb-12">
        <AnchorHeading id="what-are-projects">What are Projects?</AnchorHeading>

        <p className="text-gray-700 mb-4">
          A <strong>Project</strong> is a container for WASM code versions with shared resources:
        </p>

        <ul className="list-disc list-inside text-gray-700 space-y-2 mb-6">
          <li><strong>Versioning</strong>: Deploy multiple versions, switch active version anytime</li>
          <li><strong>Persistent Storage</strong>: Data survives version updates (same encryption key)</li>
          <li><strong>Project Secrets</strong>: Secrets accessible by all versions of the project</li>
          <li><strong>Storage Deposit</strong>: Pay for on-chain storage, refunded when deleted</li>
        </ul>

        <div className="bg-blue-50 border-l-4 border-blue-400 p-4 mb-6">
          <p className="text-sm text-blue-800">
            <strong>Why use Projects?</strong> Without projects, each new WASM hash gets its own storage namespace.
            With projects, you can update your code while keeping all user data intact.
          </p>
        </div>
      </section>

      {/* Project ID Format */}
      <section className="mb-12">
        <AnchorHeading id="project-id">Project ID Format</AnchorHeading>

        <p className="text-gray-700 mb-4">
          Each project has a unique ID composed of the owner account and project name:
        </p>

        <SyntaxHighlighter language="text" style={vscDarkPlus} className="rounded-lg mb-4">
          {`{owner_account_id}/{project_name}

Examples:
  alice.near/my-app
  bob.testnet/weather-bot
  mycompany.near/trading-oracle`}
        </SyntaxHighlighter>

        <p className="text-gray-700 mb-4">
          Project names must be alphanumeric with dashes and underscores only (pattern: <code>[a-zA-Z0-9_-]+</code>).
        </p>
      </section>

      {/* Creating a Project */}
      <section className="mb-12">
        <AnchorHeading id="creating-project">Creating a Project</AnchorHeading>

        <p className="text-gray-700 mb-4">
          Create projects via the <Link href="/projects" className="text-[var(--primary-orange)] hover:underline">Projects Dashboard</Link>:
        </p>

        <ol className="list-decimal list-inside text-gray-700 space-y-2 mb-6">
          <li>Go to <Link href="/projects" className="text-[var(--primary-orange)] hover:underline">/projects</Link></li>
          <li>Click &quot;New Project&quot;</li>
          <li>Enter project name</li>
          <li>Select code source (GitHub repo or WASM URL)</li>
          <li>Click &quot;Create Project&quot;</li>
        </ol>

        <div className="bg-yellow-50 border-l-4 border-yellow-400 p-4 mb-6">
          <p className="text-sm text-yellow-800">
            <strong>Important:</strong> Persistent storage requires <strong>WASI Preview 2</strong> (wasm32-wasip2).
            Make sure to select this build target when creating your project. WASI P1 does not support storage.
          </p>
        </div>
      </section>

      {/* Project Binding in WASM Code */}
      <section className="mb-12">
        <AnchorHeading id="wasm-metadata">Project Binding in WASM Code</AnchorHeading>

        <p className="text-gray-700 mb-4">
          For storage to work correctly, your WASM code <strong>must declare which project it belongs to</strong> using
          the <code>metadata!</code> macro from the OutLayer SDK:
        </p>

        <SyntaxHighlighter language="rust" style={vscDarkPlus} className="rounded-lg mb-4">
          {`use outlayer::{metadata, storage};

// REQUIRED for project-based execution and storage
// project must match your project_id on the OutLayer contract!
metadata! {
    project: "alice.near/my-app",  // Format: {owner_account_id}/{project_name}
    version: "1.0.0",               // Optional: for tracking
}`}
        </SyntaxHighlighter>

        <div className="bg-red-50 border-l-4 border-red-400 p-4 mb-6">
          <p className="text-sm text-red-800">
            <strong>Critical:</strong> The <code>project</code> field <strong>must exactly match</strong> your project ID
            on the contract (e.g., <code>alice.near/my-app</code>). If they don&apos;t match, storage operations will fail
            or use the wrong namespace.
          </p>
        </div>

        <h4 className="font-semibold text-gray-900 mb-2">Why is this needed?</h4>

        <ul className="list-disc list-inside text-gray-700 space-y-2 mb-6">
          <li><strong>Storage namespace</strong>: The project ID determines which storage namespace your code uses</li>
          <li><strong>Encryption key</strong>: Storage encryption keys are derived from <code>storage:{'{'}project_uuid{'}'}</code></li>
          <li><strong>Version continuity</strong>: All versions of the same project share storage because they have the same project ID</li>
          <li><strong>Secrets access</strong>: Project secrets are only accessible to WASM with matching project ID</li>
        </ul>

        <h4 className="font-semibold text-gray-900 mb-2">What happens without metadata?</h4>

        <ul className="list-disc list-inside text-gray-700 space-y-2 mb-6">
          <li>Storage calls will fail with &quot;project not found&quot; error</li>
          <li>Secrets won&apos;t be decrypted (project binding validation fails)</li>
          <li>Each WASM hash gets isolated storage instead of shared project storage</li>
        </ul>

        <div className="bg-blue-50 border-l-4 border-blue-400 p-4 mb-6">
          <p className="text-sm text-blue-800">
            <strong>Tip:</strong> Copy the exact project ID from the <Link href="/projects" className="text-[var(--primary-orange)] hover:underline">Projects dashboard</Link> to
            avoid typos. The format is always <code>{'{'}owner_account_id{'}'}/{'{'}project_name{'}'}</code>.
          </p>
        </div>
      </section>

      {/* Managing Versions */}
      <section className="mb-12">
        <AnchorHeading id="managing-versions">Managing Versions</AnchorHeading>

        <p className="text-gray-700 mb-4">
          Each project can have multiple versions. The <strong>active version</strong> is used for new executions by default.
        </p>

        <AnchorHeading id="add-version" level={3}>Adding a Version</AnchorHeading>

        <p className="text-gray-700 mb-4">
          In the <Link href="/projects" className="text-[var(--primary-orange)] hover:underline">Projects dashboard</Link>,
          expand your project and click &quot;Add Version&quot;. You can choose to make it active immediately or keep
          the current active version.
        </p>

        <AnchorHeading id="switch-version" level={3}>Switching Active Version</AnchorHeading>

        <p className="text-gray-700 mb-4">
          Click the checkmark icon on any version to make it active. The currently active version cannot be removed.
        </p>

        <div className="bg-yellow-50 border-l-4 border-yellow-400 p-4 mb-6">
          <p className="text-sm text-yellow-800">
            <strong>Note:</strong> You cannot remove the active version. First switch to another version, then remove.
          </p>
        </div>

        <AnchorHeading id="execute-specific-version" level={3}>Executing a Specific Version</AnchorHeading>

        <p className="text-gray-700 mb-4">
          By default, executions use the <strong>active version</strong>. But you can execute any specific version
          using the <code>version_key</code> parameter. This is useful for testing new versions before making them active.
        </p>

        <SyntaxHighlighter language="json" style={vscDarkPlus} className="rounded-lg mb-4">
          {`// Contract call: request_execution
{
  "code_source": {
    "Project": {
      "project_id": "alice.near/my-app",
      "version_key": "zavodil/my-app@v2.0.0"  // Optional: specific version
    }
  },
  "input_data": "...",
  "resource_limits": { ... }
}

// If version_key is omitted, the active version is used`}
        </SyntaxHighlighter>

        <div className="bg-blue-50 border-l-4 border-blue-400 p-4 mb-6">
          <p className="text-sm text-blue-800">
            <strong>Use Case:</strong> Deploy a new version, test it by specifying its version_key,
            and only set it as active once you confirm it works correctly. Both versions share the same storage.
          </p>
        </div>
      </section>

      {/* Persistent Storage */}
      <section className="mb-12">
        <AnchorHeading id="persistent-storage">Persistent Storage</AnchorHeading>

        <p className="text-gray-700 mb-4">
          All versions of a project share the same storage namespace. Data written by v1 is readable by v2.
          Storage is encrypted using the keystore TEE and isolated per user.
        </p>

        <div className="bg-purple-50 border-l-4 border-purple-400 p-4 mb-6">
          <p className="text-sm text-purple-800">
            <strong>Requires WASI Preview 2:</strong> Storage host functions are only available in WASI P2 components (wasm32-wasip2).
            WASI P1 modules do not have access to persistent storage.
          </p>
        </div>

        <div className="grid grid-cols-1 md:grid-cols-3 gap-4 mb-6">
          <div className="border border-green-200 rounded-lg p-4 bg-green-50">
            <h4 className="font-semibold text-green-900 mb-2">Encrypted</h4>
            <p className="text-sm text-gray-600">All data encrypted with project-specific keys in TEE</p>
          </div>
          <div className="border border-blue-200 rounded-lg p-4 bg-blue-50">
            <h4 className="font-semibold text-blue-900 mb-2">User-Isolated</h4>
            <p className="text-sm text-gray-600">Each user has their own namespace, automatic isolation</p>
          </div>
          <div className="border border-purple-200 rounded-lg p-4 bg-purple-50">
            <h4 className="font-semibold text-purple-900 mb-2">Atomic Operations</h4>
            <p className="text-sm text-gray-600">Increment, decrement, compare-and-swap for concurrency</p>
          </div>
        </div>

        <div className="bg-gray-50 rounded-lg p-6">
          <p className="text-gray-700 mb-4">
            For detailed information about storage API, methods, atomic operations, and usage examples, see the dedicated documentation:
          </p>
          <Link
            href="/docs/storage"
            className="inline-flex items-center px-4 py-2 bg-[var(--primary-orange)] text-white rounded-lg hover:bg-orange-600 transition-colors"
          >
            Storage Documentation →
          </Link>
        </div>
      </section>

      {/* Project Secrets */}
      <section className="mb-12">
        <AnchorHeading id="project-secrets">Project Secrets</AnchorHeading>

        <p className="text-gray-700 mb-4">
          Secrets can be bound to a project, making them accessible by all versions.
          See <Link href="/docs/secrets#project-binding" className="text-[var(--primary-orange)] hover:underline">Secrets Documentation</Link> for
          details on creating and managing project secrets.
        </p>

        <div className="bg-blue-50 border-l-4 border-blue-400 p-4 mb-6">
          <p className="text-sm text-blue-800">
            <strong>Benefit:</strong> When you update your code to a new version, project secrets remain accessible.
            No need to re-create or migrate secrets.
          </p>
        </div>
      </section>

      {/* Use Cases */}
      <section className="mb-12">
        <AnchorHeading id="use-cases">Use Cases</AnchorHeading>

        <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
          <div className="border border-gray-200 rounded-lg p-4">
            <h4 className="font-semibold text-gray-900 mb-2">Hot Updates</h4>
            <p className="text-sm text-gray-600">
              Deploy a new version, test it via version_key, then switch active version. Rollback instantly if needed.
            </p>
          </div>
          <div className="border border-gray-200 rounded-lg p-4">
            <h4 className="font-semibold text-gray-900 mb-2">Data Migration</h4>
            <p className="text-sm text-gray-600">
              New version reads old data format using <code>get_by_version</code>, transforms it, writes new format.
            </p>
          </div>
          <div className="border border-gray-200 rounded-lg p-4">
            <h4 className="font-semibold text-gray-900 mb-2">Shared Secrets</h4>
            <p className="text-sm text-gray-600">
              API keys and credentials available to all versions. No need to re-enter secrets for each update.
            </p>
          </div>
          <div className="border border-gray-200 rounded-lg p-4">
            <h4 className="font-semibold text-gray-900 mb-2">User State Persistence</h4>
            <p className="text-sm text-gray-600">
              Store user preferences, counters, session data that persist across executions and version updates.
            </p>
          </div>
          <div className="border border-gray-200 rounded-lg p-4">
            <h4 className="font-semibold text-gray-900 mb-2">Caching</h4>
            <p className="text-sm text-gray-600">
              Cache expensive computation results. Subsequent executions read from cache instead of recomputing.
            </p>
          </div>
          <div className="border border-gray-200 rounded-lg p-4">
            <h4 className="font-semibold text-gray-900 mb-2">Worker-Private State</h4>
            <p className="text-sm text-gray-600">
              Use <code>set_worker</code>/<code>get_worker</code> for internal WASM state that users cannot access directly.
            </p>
          </div>
          <div className="border border-purple-200 rounded-lg p-4 bg-purple-50">
            <h4 className="font-semibold text-purple-900 mb-2">Atomic Counters & CAS</h4>
            <p className="text-sm text-gray-600">
              Use <code>increment</code>/<code>decrement</code> for concurrent-safe counters, or <code>set_if_equals</code> for compare-and-swap operations.
            </p>
          </div>
        </div>
      </section>

      {/* Best Practices */}
      <section className="mb-12">
        <AnchorHeading id="best-practices">Best Practices</AnchorHeading>

        <ul className="list-disc list-inside text-gray-700 space-y-2">
          <li>Use <strong>WASI Preview 2</strong> (wasm32-wasip2) build target for storage support</li>
          <li>Use descriptive project names that reflect functionality</li>
          <li>Tag your git commits (e.g., <code>v1.0.0</code>) before adding versions</li>
          <li>Test new versions via version_key before setting as active</li>
          <li>Keep at least one known-good version as fallback</li>
          <li>Use project secrets for shared credentials instead of repo-based secrets</li>
          <li>Document your storage key format for data migrations</li>
          <li>Use key prefixes (e.g., <code>user:alice:</code>) for organization</li>
          <li>Use <code>increment</code>/<code>decrement</code> for counters instead of get+set (race-safe)</li>
          <li>Use <code>set_if_absent</code> for one-time initialization to avoid overwriting</li>
        </ul>
      </section>

      {/* Related Documentation */}
      <section className="bg-gray-50 rounded-lg p-6">
        <h3 className="text-lg font-semibold text-gray-900 mb-4">Related Documentation</h3>
        <ul className="space-y-2">
          <li>
            <Link href="/docs/storage" className="text-[var(--primary-orange)] hover:underline">
              Persistent Storage
            </Link>
            {' '}- Storage API, methods, atomic operations
          </li>
          <li>
            <Link href="/docs/secrets#project-binding" className="text-[var(--primary-orange)] hover:underline">
              Project Secrets
            </Link>
            {' '}- Binding secrets to projects
          </li>
          <li>
            <Link href="/docs/wasi" className="text-[var(--primary-orange)] hover:underline">
              Building OutLayer App
            </Link>
            {' '}- WASI P1 vs P2, building WASM modules
          </li>
          <li>
            <Link href="/docs/pricing" className="text-[var(--primary-orange)] hover:underline">
              Pricing & Limits
            </Link>
            {' '}- Understanding storage costs
          </li>
        </ul>
      </section>
    </div>
  );
}
