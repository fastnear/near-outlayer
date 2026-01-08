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
        Projects & Persistent Storage
      </h1>

      <p className="text-gray-700 mb-8">
        Projects allow you to organize your WASM code with version management, persistent storage, and project-level secrets.
        All versions of a project share the same storage encryption key, so data persists across updates.
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

        <AnchorHeading id="storage-api" level={3}>Storage API (WIT Interface)</AnchorHeading>

        <p className="text-gray-700 mb-4">
          The storage interface is defined in <code>worker/wit/world.wit</code>. Your WASM code imports these functions
          as <code>near:rpc/storage@0.1.0</code>:
        </p>

        <SyntaxHighlighter language="text" style={vscDarkPlus} className="rounded-lg mb-4">
          {`interface storage {
    // Basic operations
    set: func(key: string, value: list<u8>) -> string;
    get: func(key: string) -> tuple<list<u8>, string>;
    has: func(key: string) -> bool;
    delete: func(key: string) -> bool;
    list-keys: func(prefix: string) -> tuple<string, string>;

    // Worker-private storage (not accessible by users)
    set-worker: func(key: string, value: list<u8>) -> string;
    get-worker: func(key: string) -> tuple<list<u8>, string>;

    // Version migration - read from a specific WASM version
    get-by-version: func(key: string, wasm-hash: string) -> tuple<list<u8>, string>;

    // Cleanup
    clear-all: func() -> string;
    clear-version: func(wasm-hash: string) -> string;
}`}
        </SyntaxHighlighter>

        <AnchorHeading id="storage-methods" level={3}>Storage Methods Reference</AnchorHeading>

        <div className="overflow-x-auto mb-6">
          <table className="min-w-full divide-y divide-gray-200">
            <thead className="bg-gray-50">
              <tr>
                <th className="px-4 py-3 text-left text-xs font-medium text-gray-500 uppercase">Method</th>
                <th className="px-4 py-3 text-left text-xs font-medium text-gray-500 uppercase">Description</th>
                <th className="px-4 py-3 text-left text-xs font-medium text-gray-500 uppercase">Returns</th>
              </tr>
            </thead>
            <tbody className="bg-white divide-y divide-gray-200">
              <tr>
                <td className="px-4 py-3 text-sm font-mono">set(key, value)</td>
                <td className="px-4 py-3 text-sm text-gray-600">Store a key-value pair</td>
                <td className="px-4 py-3 text-sm text-gray-600">Error string (empty on success)</td>
              </tr>
              <tr>
                <td className="px-4 py-3 text-sm font-mono">get(key)</td>
                <td className="px-4 py-3 text-sm text-gray-600">Retrieve value by key</td>
                <td className="px-4 py-3 text-sm text-gray-600">(data, error)</td>
              </tr>
              <tr>
                <td className="px-4 py-3 text-sm font-mono">has(key)</td>
                <td className="px-4 py-3 text-sm text-gray-600">Check if key exists</td>
                <td className="px-4 py-3 text-sm text-gray-600">bool</td>
              </tr>
              <tr>
                <td className="px-4 py-3 text-sm font-mono">delete(key)</td>
                <td className="px-4 py-3 text-sm text-gray-600">Delete a key</td>
                <td className="px-4 py-3 text-sm text-gray-600">bool (true if existed)</td>
              </tr>
              <tr>
                <td className="px-4 py-3 text-sm font-mono">list-keys(prefix)</td>
                <td className="px-4 py-3 text-sm text-gray-600">List keys with prefix</td>
                <td className="px-4 py-3 text-sm text-gray-600">(JSON array string, error)</td>
              </tr>
              <tr>
                <td className="px-4 py-3 text-sm font-mono">set-worker(key, value)</td>
                <td className="px-4 py-3 text-sm text-gray-600">Store worker-private data</td>
                <td className="px-4 py-3 text-sm text-gray-600">Error string</td>
              </tr>
              <tr>
                <td className="px-4 py-3 text-sm font-mono">get-worker(key)</td>
                <td className="px-4 py-3 text-sm text-gray-600">Get worker-private data</td>
                <td className="px-4 py-3 text-sm text-gray-600">(data, error)</td>
              </tr>
              <tr>
                <td className="px-4 py-3 text-sm font-mono">get-by-version(key, hash)</td>
                <td className="px-4 py-3 text-sm text-gray-600">Read from specific WASM version</td>
                <td className="px-4 py-3 text-sm text-gray-600">(data, error)</td>
              </tr>
              <tr>
                <td className="px-4 py-3 text-sm font-mono">clear-all()</td>
                <td className="px-4 py-3 text-sm text-gray-600">Delete all user&apos;s storage</td>
                <td className="px-4 py-3 text-sm text-gray-600">Error string</td>
              </tr>
            </tbody>
          </table>
        </div>

        <AnchorHeading id="storage-example" level={3}>Usage Example</AnchorHeading>

        <SyntaxHighlighter language="rust" style={vscDarkPlus} className="rounded-lg mb-4">
          {`// In your WASM code (WASI P2)
use near::rpc::storage;

// Store data
let err = storage::set("user:alice:balance", "100".as_bytes().to_vec());
if !err.is_empty() {
    eprintln!("Storage error: {}", err);
}

// Read data
let (data, err) = storage::get("user:alice:balance");
if err.is_empty() {
    let value = String::from_utf8(data).unwrap();
    println!("Balance: {}", value);
}

// Check existence
if storage::has("user:alice:balance") {
    println!("Key exists!");
}

// List keys with prefix
let (keys_json, err) = storage::list_keys("user:");
// keys_json = '["user:alice:balance", "user:bob:balance"]'

// Read from old version (for migrations)
let (old_data, err) = storage::get_by_version("legacy_key", "abc123...");

// Worker-private storage (other users cannot read this)
storage::set_worker("internal_state", state_bytes);`}
        </SyntaxHighlighter>

        <AnchorHeading id="storage-monitoring" level={3}>Storage Monitoring</AnchorHeading>

        <p className="text-gray-700 mb-4">
          The <Link href="/projects" className="text-[var(--primary-orange)] hover:underline">Projects dashboard</Link> shows
          storage usage for each project: total bytes and number of keys. This data is cached and may not reflect
          the most recent changes immediately.
        </p>

        <AnchorHeading id="storage-cleanup" level={3}>Storage Cleanup</AnchorHeading>

        <p className="text-gray-700 mb-4">
          When you <strong>delete a project</strong>, all associated storage data is automatically cleared.
          The contract emits a <code>project_storage_cleanup</code> event, and the worker processes it
          to remove all stored keys for that project.
        </p>

        <div className="bg-red-50 border-l-4 border-red-400 p-4 mb-6">
          <p className="text-sm text-red-800">
            <strong>Warning:</strong> Deleting a project permanently removes all storage data.
            This action cannot be undone. Export any important data before deletion.
          </p>
        </div>

        <AnchorHeading id="storage-security" level={3}>Storage Security</AnchorHeading>

        <ul className="list-disc list-inside text-gray-700 space-y-2 mb-6">
          <li>All data is encrypted using keystore TEE before storage</li>
          <li>Encryption key derived from: <code>storage:{'{'}project_uuid{'}'}:{'{'}account_id{'}'}</code></li>
          <li>Worker-private storage uses <code>@worker</code> as account_id</li>
        </ul>

        <AnchorHeading id="user-data-isolation" level={3}>User Data Isolation</AnchorHeading>

        <p className="text-gray-700 mb-4">
          Storage is automatically isolated per user at the protocol level. Each user has their own namespace:
        </p>

        <ul className="list-disc list-inside text-gray-700 space-y-2 mb-6">
          <li><strong>Automatic isolation</strong>: <code>alice.near</code> cannot read or overwrite data stored by <code>bob.near</code></li>
          <li><strong>Per-user encryption</strong>: Different encryption keys for each user&apos;s data</li>
          <li><strong>Transparent to WASM</strong>: Your code uses simple keys like <code>balance</code> - the platform handles namespacing</li>
          <li><strong>No code changes needed</strong>: Isolation is enforced by the platform, not by your application</li>
          <li><strong>Caller-triggered access</strong>: WASM can only read user data when that user triggers the execution</li>
        </ul>

        <SyntaxHighlighter language="text" style={vscDarkPlus} className="rounded-lg mb-4">
          {`// alice.near calls execution:
storage::set("balance", b"100");
// Database key: project_uuid:alice.near:balance = "100"

// bob.near calls execution:
storage::set("balance", b"200");
// Database key: project_uuid:bob.near:balance = "200"

// alice.near reads:
storage::get("balance")  // → "100" (her data)

// bob.near reads:
storage::get("balance")  // → "200" (his data)

// WASM code CANNOT read another user's data!
// There is no storage::get_for_account("bob.near", "balance")
// User data is only accessible when that user triggers the execution`}
        </SyntaxHighlighter>

        <AnchorHeading id="worker-storage" level={3}>Worker Storage (Shared State)</AnchorHeading>

        <p className="text-gray-700 mb-4">
          Worker-private storage uses <code>@worker</code> as account_id, making it shared across all users.
          Users cannot directly access this storage - only your WASM code can read/write it.
        </p>

        <SyntaxHighlighter language="text" style={vscDarkPlus} className="rounded-lg mb-4">
          {`// Any user calls execution:
storage::set_worker("total_count", b"100");
// Database key: project_uuid:@worker:total_count = "100"

// Any other user calls:
storage::get_worker("total_count")  // → "100" (same shared data!)

// Use case: Private Token balances (see private-token-ark example)
storage::set_worker("balances", balances_json);  // Shared across all users
// Users cannot directly read balances - only through your WASM methods`}
        </SyntaxHighlighter>

        <div className="bg-green-50 border-l-4 border-green-400 p-4 mb-6">
          <p className="text-sm text-green-800">
            <strong>Use case:</strong> Worker storage is ideal for shared state like token balances,
            global counters, or application configuration. Users interact with this data only through
            your WASM methods - they cannot bypass your logic.
          </p>
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
        </ul>
      </section>

      {/* Related Documentation */}
      <section className="bg-gray-50 rounded-lg p-6">
        <h3 className="text-lg font-semibold text-gray-900 mb-4">Related Documentation</h3>
        <ul className="space-y-2">
          <li>
            <Link href="/docs/secrets#project-binding" className="text-[var(--primary-orange)] hover:underline">
              Project Secrets
            </Link>
            {' '}- Binding secrets to projects
          </li>
          <li>
            <Link href="/docs/wasi" className="text-[var(--primary-orange)] hover:underline">
              Writing WASI Code
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
