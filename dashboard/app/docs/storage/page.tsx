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

export default function StoragePage() {
  return (
    <div className="prose prose-lg max-w-none">
      <h1 className="text-3xl font-bold text-gray-900 mb-6">
        Persistent Storage
      </h1>

      <p className="text-gray-700 mb-6">
        OutLayer provides encrypted persistent storage for your WASM projects. Data survives across executions and version updates,
        with automatic user isolation and atomic operations for concurrent-safe updates.
      </p>

      <div className="bg-yellow-50 border-l-4 border-yellow-400 p-4 mb-8">
        <p className="text-sm text-yellow-800">
          <strong>Requires Projects:</strong> Storage is only available for code deployed as a{' '}
          <Link href="/docs/projects" className="text-[var(--primary-orange)] hover:underline font-medium">Project</Link>.
          You must use <strong>WASI Preview 2</strong> (wasm32-wasip2) build target.
        </p>
      </div>

      {/* Overview */}
      <section className="mb-12">
        <AnchorHeading id="overview">Overview</AnchorHeading>

        <p className="text-gray-700 mb-4">
          All versions of a project share the same storage namespace. Data written by v1 is readable by v2.
          Storage is encrypted using the keystore TEE and isolated per user.
        </p>

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
      </section>

      {/* Quick Start */}
      <section className="mb-12">
        <AnchorHeading id="quick-start">Quick Start</AnchorHeading>

        <SyntaxHighlighter language="rust" style={vscDarkPlus} className="rounded-lg mb-4">
          {`use outlayer::storage;

fn main() {
    // Store data
    storage::set("counter", &42i64.to_le_bytes()).unwrap();

    // Read data
    if let Some(data) = storage::get("counter").unwrap() {
        let value = i64::from_le_bytes(data.try_into().unwrap());
        println!("Counter: {}", value);
    }

    // Atomic increment
    let new_value = storage::increment("visits", 1).unwrap();
    println!("Visit count: {}", new_value);
}`}
        </SyntaxHighlighter>

        <div className="bg-blue-50 border-l-4 border-blue-400 p-4 mb-6">
          <p className="text-sm text-blue-800">
            <strong>Note:</strong> Storage automatically uses your project context. When you call
            <code>request_execution(Project &#123; project_id &#125;)</code>, the contract passes the project UUID
            to the worker, which uses it for storage namespace and encryption.
          </p>
        </div>
      </section>

      {/* Storage API */}
      <section className="mb-12">
        <AnchorHeading id="api">Storage API</AnchorHeading>

        <p className="text-gray-700 mb-4">
          The storage interface is defined in <code>worker/wit/world.wit</code> and imported as <code>near:rpc/storage@0.1.0</code>:
        </p>

        <SyntaxHighlighter language="text" style={vscDarkPlus} className="rounded-lg mb-4">
          {`interface api {
    // Basic operations
    set: func(key: string, value: list<u8>) -> string;
    get: func(key: string) -> tuple<list<u8>, string>;
    has: func(key: string) -> bool;
    delete: func(key: string) -> bool;
    list-keys: func(prefix: string) -> tuple<string, string>;

    // Conditional writes (atomic operations)
    set-if-absent: func(key: string, value: list<u8>) -> tuple<bool, string>;
    set-if-equals: func(key: string, expected: list<u8>, new-value: list<u8>) -> tuple<bool, list<u8>, string>;
    increment: func(key: string, delta: s64) -> tuple<s64, string>;
    decrement: func(key: string, delta: s64) -> tuple<s64, string>;

    // Worker storage (with public option for cross-project reads)
    // is-encrypted: true (default) = encrypted, false = plaintext (public)
    set-worker: func(key: string, value: list<u8>, is-encrypted: option<bool>) -> string;
    // project-uuid: none = current project, some("p...") = read from another project
    get-worker: func(key: string, project-uuid: option<string>) -> tuple<list<u8>, string>;

    // Version migration
    get-by-version: func(key: string, wasm-hash: string) -> tuple<list<u8>, string>;

    // Cleanup
    clear-all: func() -> string;
    clear-version: func(wasm-hash: string) -> string;
}`}
        </SyntaxHighlighter>
      </section>

      {/* Methods Reference */}
      <section className="mb-12">
        <AnchorHeading id="methods">Methods Reference</AnchorHeading>

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
              <tr className="bg-purple-50">
                <td className="px-4 py-3 text-sm font-mono">set-if-absent(key, value)</td>
                <td className="px-4 py-3 text-sm text-gray-600">Set only if key doesn&apos;t exist</td>
                <td className="px-4 py-3 text-sm text-gray-600">(inserted: bool, error)</td>
              </tr>
              <tr className="bg-purple-50">
                <td className="px-4 py-3 text-sm font-mono">set-if-equals(key, expected, new)</td>
                <td className="px-4 py-3 text-sm text-gray-600">Compare-and-swap (atomic update)</td>
                <td className="px-4 py-3 text-sm text-gray-600">(success, current, error)</td>
              </tr>
              <tr className="bg-purple-50">
                <td className="px-4 py-3 text-sm font-mono">increment(key, delta)</td>
                <td className="px-4 py-3 text-sm text-gray-600">Atomic increment (i64)</td>
                <td className="px-4 py-3 text-sm text-gray-600">(new_value: i64, error)</td>
              </tr>
              <tr className="bg-purple-50">
                <td className="px-4 py-3 text-sm font-mono">decrement(key, delta)</td>
                <td className="px-4 py-3 text-sm text-gray-600">Atomic decrement (i64)</td>
                <td className="px-4 py-3 text-sm text-gray-600">(new_value: i64, error)</td>
              </tr>
              <tr>
                <td className="px-4 py-3 text-sm font-mono">set-worker(key, value, is_encrypted)</td>
                <td className="px-4 py-3 text-sm text-gray-600">Store worker data (public if is_encrypted=false)</td>
                <td className="px-4 py-3 text-sm text-gray-600">Error string</td>
              </tr>
              <tr>
                <td className="px-4 py-3 text-sm font-mono">get-worker(key, project_uuid)</td>
                <td className="px-4 py-3 text-sm text-gray-600">Get worker data (cross-project if project_uuid set)</td>
                <td className="px-4 py-3 text-sm text-gray-600">(data, error)</td>
              </tr>
            </tbody>
          </table>
        </div>
      </section>

      {/* Atomic Operations */}
      <section className="mb-12">
        <AnchorHeading id="atomic-operations">Atomic Operations</AnchorHeading>

        <p className="text-gray-700 mb-4">
          Use atomic operations for concurrent-safe updates. These are essential for counters, rate limiters,
          and any state that multiple executions might modify simultaneously.
        </p>

        <div className="bg-green-50 border-l-4 border-green-400 p-4 mb-6">
          <p className="text-sm text-green-800">
            <strong>Why use atomic operations?</strong> Regular <code>set()</code> can cause race conditions
            when multiple users execute simultaneously. Atomic operations ensure data integrity.
          </p>
        </div>

        <SyntaxHighlighter language="rust" style={vscDarkPlus} className="rounded-lg mb-4">
          {`use outlayer::storage;

// ============ set_if_absent ============
// Only inserts if key doesn't exist - perfect for initialization
if storage::set_if_absent("counter", &0i64.to_le_bytes())? {
    println!("Counter initialized to 0");
} else {
    println!("Counter already exists");
}

// ============ increment / decrement ============
// Atomic counters - handles concurrent updates automatically
let views = storage::increment("page_views", 1)?;
println!("Page views: {}", views);

let stock = storage::decrement("stock:item_123", 1)?;
if stock < 0 {
    println!("Out of stock!");
}

// ============ set_if_equals (Compare-and-Swap) ============
// Update only if current value matches expected
let current = storage::get("balance")?.unwrap_or(vec![0; 8]);
let balance = i64::from_le_bytes(current.clone().try_into().unwrap());
let new_balance = balance + 100;

match storage::set_if_equals("balance", &current, &new_balance.to_le_bytes())? {
    (true, _) => println!("Balance updated!"),
    (false, Some(actual)) => println!("Concurrent update! Retry with {:?}", actual),
    (false, None) => println!("Key was deleted"),
}`}
        </SyntaxHighlighter>

        <div className="bg-yellow-50 border-l-4 border-yellow-400 p-4 mb-6">
          <p className="text-sm text-yellow-800">
            <strong>Important:</strong> <code>increment</code>/<code>decrement</code> expect values stored as 8-byte little-endian i64.
            If you store counters differently (e.g., as string), use <code>set_if_equals</code> instead.
          </p>
        </div>
      </section>

      {/* User Data Isolation */}
      <section className="mb-12">
        <AnchorHeading id="user-isolation">User Data Isolation</AnchorHeading>

        <p className="text-gray-700 mb-4">
          Storage is automatically isolated per user at the protocol level. Each user has their own namespace:
        </p>

        <ul className="list-disc list-inside text-gray-700 space-y-2 mb-6">
          <li><strong>Automatic isolation</strong>: <code>alice.near</code> cannot read data stored by <code>bob.near</code></li>
          <li><strong>Per-user encryption</strong>: Different encryption keys for each user&apos;s data</li>
          <li><strong>Transparent to WASM</strong>: Your code uses simple keys like <code>balance</code> - the platform handles namespacing</li>
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
storage::get("balance")  // -> "100" (her data)

// bob.near reads:
storage::get("balance")  // -> "200" (his data)

// WASM code CANNOT read another user's data!`}
        </SyntaxHighlighter>
      </section>

      {/* Worker Storage */}
      <section className="mb-12">
        <AnchorHeading id="worker-storage">Worker Storage (Shared State)</AnchorHeading>

        <p className="text-gray-700 mb-4">
          Worker-private storage uses <code>@worker</code> as account_id, making it shared across all users.
          Users cannot directly access this storage - only your WASM code can read/write it.
        </p>

        <SyntaxHighlighter language="rust" style={vscDarkPlus} className="rounded-lg mb-4">
          {`// Any user calls execution:
storage::set_worker("total_count", b"100");
// Database key: project_uuid:@worker:total_count = "100"

// Any other user calls:
storage::get_worker("total_count")  // -> "100" (same shared data!)

// Use case: Private Token balances
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

      {/* Public Storage */}
      <section className="mb-12">
        <AnchorHeading id="public-storage">Public Storage (Cross-Project Reads)</AnchorHeading>

        <p className="text-gray-700 mb-4">
          Public storage is <strong>unencrypted</strong> worker storage that can be read by other projects.
          This enables use cases like shared oracle price feeds, public configuration, or cross-project data sharing.
        </p>

        <div className="bg-orange-50 border-l-4 border-orange-400 p-4 mb-6">
          <p className="text-sm text-orange-800">
            <strong>Important:</strong> Public storage is NOT encrypted. Only store data you want to be readable
            by other projects and external HTTP clients. Use regular worker storage for private data.
          </p>
        </div>

        <SyntaxHighlighter language="rust" style={vscDarkPlus} className="rounded-lg mb-4">
          {`use outlayer::storage;

// Store PUBLIC data (is_encrypted = false)
storage::set_worker_with_options(
    "oracle:ETH",
    price_json.as_bytes(),
    Some(false)  // <-- Makes it public!
)?;

// Read from current project (works for both public and private)
let data = storage::get_worker("oracle:ETH")?;

// Read PUBLIC data from ANOTHER project by UUID
let price = storage::get_worker_from_project(
    "oracle:ETH",
    Some("p0000000000000001")  // Target project UUID
)?;`}
        </SyntaxHighlighter>

        <AnchorHeading id="public-http-api" level={3}>External HTTP API</AnchorHeading>

        <p className="text-gray-700 mb-4">
          Public storage can also be read by external clients via HTTP:
        </p>

        <SyntaxHighlighter language="bash" style={vscDarkPlus} className="rounded-lg mb-4">
          {`# JSON format (default) - value is base64-encoded
curl "https://api.outlayer.fastnear.com/public/storage/get?project_uuid=p0000000000000001&key=oracle:ETH"
# {"exists":true,"value":"eyJwcmljZSI6IjM1MDAuMDAifQ=="}

# Raw format - returns raw bytes directly
curl "https://api.outlayer.fastnear.com/public/storage/get?project_uuid=p0000000000000001&key=oracle:ETH&format=raw"
# {"price":"3500.00"}`}
        </SyntaxHighlighter>

        <div className="grid grid-cols-1 md:grid-cols-2 gap-4 mb-6">
          <div className="border border-orange-200 rounded-lg p-4 bg-orange-50">
            <h4 className="font-semibold text-orange-900 mb-2">Oracle Price Feeds</h4>
            <p className="text-sm text-gray-600">Share price data across projects without API calls</p>
          </div>
          <div className="border border-orange-200 rounded-lg p-4 bg-orange-50">
            <h4 className="font-semibold text-orange-900 mb-2">Public Configuration</h4>
            <p className="text-sm text-gray-600">Share settings that other projects can read</p>
          </div>
        </div>

        <div className="bg-blue-50 border-l-4 border-blue-400 p-4 mb-6">
          <p className="text-sm text-blue-800">
            <strong>Note:</strong> Encrypted (default) worker data is NOT accessible via cross-project reads.
            Only data stored with <code>is_encrypted=false</code> can be read by other projects.
          </p>
        </div>
      </section>

      {/* Security */}
      <section className="mb-12">
        <AnchorHeading id="security">Security</AnchorHeading>

        <ul className="list-disc list-inside text-gray-700 space-y-2 mb-6">
          <li>All data is encrypted using keystore TEE before storage</li>
          <li>Encryption key derived from: <code>storage:{'{'}project_uuid{'}'}:{'{'}account_id{'}'}</code></li>
          <li>Worker-private storage uses <code>@worker</code> as account_id</li>
          <li>Data is automatically deleted when project is deleted</li>
        </ul>

        <div className="bg-red-50 border-l-4 border-red-400 p-4 mb-6">
          <p className="text-sm text-red-800">
            <strong>Warning:</strong> Deleting a project permanently removes all storage data.
            This action cannot be undone. Export any important data before deletion.
          </p>
        </div>
      </section>

      {/* Use Cases */}
      <section className="mb-12">
        <AnchorHeading id="use-cases">Use Cases</AnchorHeading>

        <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
          <div className="border border-gray-200 rounded-lg p-4">
            <h4 className="font-semibold text-gray-900 mb-2">User Preferences</h4>
            <p className="text-sm text-gray-600">Store settings, themes, language preferences per user</p>
          </div>
          <div className="border border-gray-200 rounded-lg p-4">
            <h4 className="font-semibold text-gray-900 mb-2">Counters & Analytics</h4>
            <p className="text-sm text-gray-600">Page views, API calls, usage metrics with atomic increments</p>
          </div>
          <div className="border border-gray-200 rounded-lg p-4">
            <h4 className="font-semibold text-gray-900 mb-2">Caching</h4>
            <p className="text-sm text-gray-600">Cache expensive computation results for subsequent calls</p>
          </div>
          <div className="border border-gray-200 rounded-lg p-4">
            <h4 className="font-semibold text-gray-900 mb-2">Session Data</h4>
            <p className="text-sm text-gray-600">Store temporary state between executions</p>
          </div>
          <div className="border border-purple-200 rounded-lg p-4 bg-purple-50">
            <h4 className="font-semibold text-purple-900 mb-2">Private Tokens</h4>
            <p className="text-sm text-gray-600">Use worker storage for token balances users cannot directly access</p>
          </div>
          <div className="border border-purple-200 rounded-lg p-4 bg-purple-50">
            <h4 className="font-semibold text-purple-900 mb-2">Rate Limiting</h4>
            <p className="text-sm text-gray-600">Track API calls per user with atomic counters</p>
          </div>
          <div className="border border-orange-200 rounded-lg p-4 bg-orange-50">
            <h4 className="font-semibold text-orange-900 mb-2">Oracle Price Feeds</h4>
            <p className="text-sm text-gray-600">Share public data across projects (set is_encrypted=false)</p>
          </div>
          <div className="border border-orange-200 rounded-lg p-4 bg-orange-50">
            <h4 className="font-semibold text-orange-900 mb-2">Distributed Locks</h4>
            <p className="text-sm text-gray-600">Use set_if_absent for implementing locks</p>
          </div>
        </div>
      </section>

      {/* Best Practices */}
      <section className="mb-12">
        <AnchorHeading id="best-practices">Best Practices</AnchorHeading>

        <ul className="list-disc list-inside text-gray-700 space-y-2">
          <li>Use <strong>WASI Preview 2</strong> (wasm32-wasip2) build target</li>
          <li>Use key prefixes for organization (e.g., <code>user:alice:settings</code>)</li>
          <li>Use <code>increment</code>/<code>decrement</code> for counters instead of get+set</li>
          <li>Use <code>set_if_absent</code> for one-time initialization</li>
          <li>Document your key format for future data migrations</li>
          <li>Use <code>get_by_version</code> when migrating data between versions</li>
        </ul>
      </section>

      {/* Related */}
      <section className="bg-gray-50 rounded-lg p-6">
        <h3 className="text-lg font-semibold text-gray-900 mb-4">Related Documentation</h3>
        <ul className="space-y-2">
          <li>
            <Link href="/docs/projects" className="text-[var(--primary-orange)] hover:underline">
              Projects & Versions
            </Link>
            {' '}- Create and manage projects for storage access
          </li>
          <li>
            <Link href="/docs/wasi" className="text-[var(--primary-orange)] hover:underline">
              Building OutLayer App
            </Link>
            {' '}- WASI P1 vs P2, building WASM modules
          </li>
          <li>
            <Link href="/docs/examples" className="text-[var(--primary-orange)] hover:underline">
              Examples
            </Link>
            {' '}- See private-token-ark for storage usage
          </li>
        </ul>
      </section>
    </div>
  );
}
