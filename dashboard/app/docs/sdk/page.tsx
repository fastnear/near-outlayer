'use client';

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

export default function SdkPage() {
  return (
    <div className="prose prose-lg max-w-none">
      <h1 className="text-3xl font-bold text-gray-900 mb-6">OutLayer SDK</h1>

      <p className="text-gray-700 mb-8 text-lg">
        The <code>outlayer</code> crate provides a Rust SDK for building WASI applications on OutLayer.
        It gives you access to persistent encrypted storage, execution context (caller identity, secrets),
        and structured I/O.
      </p>

      {/* Installation */}
      <section className="mb-12">
        <AnchorHeading id="installation">Installation</AnchorHeading>

        <p className="text-gray-700 mb-4">
          Add the crate to your <code>Cargo.toml</code>:
        </p>

        <SyntaxHighlighter language="toml" style={vscDarkPlus} className="rounded-lg mb-4">
          {`[dependencies]
outlayer = "0.1"
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"`}
        </SyntaxHighlighter>

        <p className="text-gray-700 mb-4">
          Published at{' '}
          <a href="https://crates.io/crates/outlayer" target="_blank" rel="noopener noreferrer" className="text-[var(--primary-orange)] hover:underline">
            crates.io/crates/outlayer
          </a>.
          Build with the <code>wasm32-wasip2</code> target:
        </p>

        <SyntaxHighlighter language="bash" style={vscDarkPlus} className="rounded-lg mb-4">
          {`rustup target add wasm32-wasip2
cargo build --target wasm32-wasip2 --release`}
        </SyntaxHighlighter>

        <div className="bg-red-50 border-l-4 border-red-400 p-4 mb-6">
          <p className="text-sm text-red-800">
            <strong>WASI Preview 2 required.</strong> The SDK will fail to compile if you target
            <code> wasm32-wasip1</code> or <code>wasm32-wasi</code>. You must use <code>wasm32-wasip2</code>.
          </p>
        </div>
      </section>

      {/* When to use */}
      <section className="mb-12">
        <AnchorHeading id="when-to-use">When Do You Need the SDK?</AnchorHeading>

        <div className="overflow-x-auto mb-6">
          <table className="min-w-full divide-y divide-gray-200">
            <thead className="bg-gray-50">
              <tr>
                <th className="px-4 py-3 text-left text-xs font-medium text-gray-500 uppercase">Use Case</th>
                <th className="px-4 py-3 text-left text-xs font-medium text-gray-500 uppercase">SDK Required?</th>
              </tr>
            </thead>
            <tbody className="bg-white divide-y divide-gray-200">
              <tr>
                <td className="px-4 py-3 text-sm text-gray-600">Persistent storage across executions</td>
                <td className="px-4 py-3 text-sm font-semibold text-green-700">Yes</td>
              </tr>
              <tr className="bg-gray-50">
                <td className="px-4 py-3 text-sm text-gray-600">Caller identity (who signed the transaction)</td>
                <td className="px-4 py-3 text-sm font-semibold text-green-700">Yes (or read env vars directly)</td>
              </tr>
              <tr>
                <td className="px-4 py-3 text-sm text-gray-600">Atomic operations (counters, compare-and-swap)</td>
                <td className="px-4 py-3 text-sm font-semibold text-green-700">Yes</td>
              </tr>
              <tr className="bg-gray-50">
                <td className="px-4 py-3 text-sm text-gray-600">Cross-project public data sharing</td>
                <td className="px-4 py-3 text-sm font-semibold text-green-700">Yes</td>
              </tr>
              <tr>
                <td className="px-4 py-3 text-sm text-gray-600">Pure computation (no state)</td>
                <td className="px-4 py-3 text-sm text-gray-500">No</td>
              </tr>
              <tr className="bg-gray-50">
                <td className="px-4 py-3 text-sm text-gray-600">HTTP requests to external APIs</td>
                <td className="px-4 py-3 text-sm text-gray-500">No</td>
              </tr>
              <tr>
                <td className="px-4 py-3 text-sm text-gray-600">Reading secrets via env vars</td>
                <td className="px-4 py-3 text-sm text-gray-500">No (use <code>std::env::var</code>)</td>
              </tr>
            </tbody>
          </table>
        </div>

        <p className="text-gray-700 mb-4">
          In short: if your WASI app needs <strong>persistent storage</strong> or <strong>structured access to execution context</strong>,
          use the SDK. For stateless computation (like API calls, random numbers, weather data), you can use plain Rust with stdin/stdout.
        </p>
      </section>

      {/* Environment Module */}
      <section className="mb-12">
        <AnchorHeading id="env-module">Environment Module (<code>outlayer::env</code>)</AnchorHeading>

        <p className="text-gray-700 mb-4">
          Access execution context: who called you, what input they sent, and environment variables (including secrets).
        </p>

        <AnchorHeading id="env-context" level={3}>Caller Context</AnchorHeading>

        <SyntaxHighlighter language="rust" style={vscDarkPlus} className="rounded-lg mb-4">
          {`use outlayer::env;

// Who initiated the execution
let signer = env::signer_account_id();     // e.g., Some("alice.near")
let predecessor = env::predecessor_account_id(); // e.g., Some("token.near")
let tx_hash = env::transaction_hash();
let request_id = env::request_id();`}
        </SyntaxHighlighter>

        <AnchorHeading id="env-io" level={3}>Input / Output</AnchorHeading>

        <SyntaxHighlighter language="rust" style={vscDarkPlus} className="rounded-lg mb-4">
          {`use outlayer::env;
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
struct Request { action: String }

#[derive(Serialize)]
struct Response { result: String }

fn main() {
    // Read JSON input from stdin
    let req: Request = match env::input_json() {
        Ok(Some(r)) => r,
        _ => return env::output_string("No valid input"),
    };

    // Write JSON output to stdout
    env::output_json(&Response {
        result: format!("Got action: {}", req.action),
    }).ok();
}`}
        </SyntaxHighlighter>

        <AnchorHeading id="env-vars" level={3}>Environment Variables</AnchorHeading>

        <p className="text-gray-700 mb-4">
          Secrets stored via the dashboard are injected as environment variables. You can read them
          with <code>env::var()</code> or <code>std::env::var()</code>:
        </p>

        <SyntaxHighlighter language="rust" style={vscDarkPlus} className="rounded-lg mb-4">
          {`// SDK helper
let api_key = outlayer::env::var("OPENAI_API_KEY");

// Standard library (also works)
let api_key = std::env::var("OPENAI_API_KEY").ok();`}
        </SyntaxHighlighter>

        <div className="bg-blue-50 border-l-4 border-blue-400 p-4 mb-6">
          <p className="text-sm text-blue-800">
            <strong>Injected environment variables:</strong>{' '}
            <code>NEAR_SENDER_ID</code>, <code>NEAR_PREDECESSOR_ID</code>,{' '}
            <code>NEAR_TRANSACTION_HASH</code>, <code>OUTLAYER_REQUEST_ID</code>,{' '}
            <code>OUTLAYER_PROJECT_UUID</code>, <code>OUTLAYER_PROJECT_OWNER</code>,{' '}
            <code>OUTLAYER_PROJECT_NAME</code>, <code>USD_PAYMENT</code>,{' '}
            plus any custom secrets stored via the dashboard.
          </p>
        </div>
      </section>

      {/* Storage Module */}
      <section className="mb-12">
        <AnchorHeading id="storage-module">Storage Module (<code>outlayer::storage</code>)</AnchorHeading>

        <p className="text-gray-700 mb-4">
          Persistent encrypted key-value storage that survives across executions.
          Storage is automatically isolated per caller — <code>alice.near</code> cannot read <code>bob.near</code>&apos;s data.
        </p>

        <AnchorHeading id="storage-basic" level={3}>Basic Operations</AnchorHeading>

        <SyntaxHighlighter language="rust" style={vscDarkPlus} className="rounded-lg mb-4">
          {`use outlayer::storage;

// String convenience methods
storage::set_string("greeting", "hello")?;
let val = storage::get_string("greeting")?; // Some("hello")

// JSON convenience methods
storage::set_json("config", &my_config)?;
let config: MyConfig = storage::get_json("config")?.unwrap();

// Binary data
storage::set("data", &bytes)?;
let data = storage::get("data")?;

// Check existence and delete
if storage::has("key") {
    storage::delete("key");
}

// List keys by prefix
let keys = storage::list_keys("user:")?;

// Clear all data for current caller
storage::clear_all()?;`}
        </SyntaxHighlighter>

        <AnchorHeading id="storage-atomic" level={3}>Atomic Operations</AnchorHeading>

        <p className="text-gray-700 mb-4">
          For concurrent-safe operations (e.g., counters, voting):
        </p>

        <SyntaxHighlighter language="rust" style={vscDarkPlus} className="rounded-lg mb-4">
          {`// Atomic counter
let new_count = storage::increment("visits", 1)?;  // Returns new value
let new_count = storage::decrement("credits", 10)?;

// Insert only if key doesn't exist (mutex-like)
let inserted = storage::set_if_absent("lock", b"held")?;

// Compare-and-swap
let (success, old) = storage::set_if_equals(
    "version", b"v1", b"v2"
)?;`}
        </SyntaxHighlighter>

        <AnchorHeading id="storage-worker" level={3}>Worker Storage (Shared Across Users)</AnchorHeading>

        <p className="text-gray-700 mb-4">
          Worker storage is shared across all callers and accessible only from within the WASI module.
          Use it for global state like price feeds, configuration, or cached data.
        </p>

        <SyntaxHighlighter language="rust" style={vscDarkPlus} className="rounded-lg mb-4">
          {`// Encrypted worker storage (default)
storage::set_worker("cache:key", &data)?;
let cached = storage::get_worker("cache:key")?;

// Public (unencrypted) worker storage — readable by other projects
storage::set_worker_with_options("oracle:ETH", &price, Some(false))?;

// Read public data from another project
let price = storage::get_worker_from_project(
    "oracle:ETH",
    Some("p0000000000000042"),  // target project UUID
)?;`}
        </SyntaxHighlighter>

        <AnchorHeading id="storage-migration" level={3}>Version Migration</AnchorHeading>

        <p className="text-gray-700 mb-4">
          When upgrading your WASM binary, data from the previous version is accessible via its hash:
        </p>

        <SyntaxHighlighter language="rust" style={vscDarkPlus} className="rounded-lg mb-4">
          {`// Read data from a previous version
let old_data = storage::get_by_version("key", "abc123...")?;

// Clean up old version data after migration
storage::clear_version("abc123...")?;`}
        </SyntaxHighlighter>
      </section>

      {/* Examples */}
      <section className="mb-12">
        <AnchorHeading id="examples">Examples Using the SDK</AnchorHeading>

        <div className="overflow-x-auto mb-6">
          <table className="min-w-full divide-y divide-gray-200">
            <thead className="bg-gray-50">
              <tr>
                <th className="px-4 py-3 text-left text-xs font-medium text-gray-500 uppercase">Example</th>
                <th className="px-4 py-3 text-left text-xs font-medium text-gray-500 uppercase">SDK Features Used</th>
              </tr>
            </thead>
            <tbody className="bg-white divide-y divide-gray-200">
              <tr>
                <td className="px-4 py-3 text-sm font-mono">oracle-ark</td>
                <td className="px-4 py-3 text-sm text-gray-600">
                  <code>storage::set_worker_with_options()</code> — stores prices as public (unencrypted) worker data for cross-project reads
                </td>
              </tr>
              <tr className="bg-gray-50">
                <td className="px-4 py-3 text-sm font-mono">private-token-ark</td>
                <td className="px-4 py-3 text-sm text-gray-600">
                  <code>storage::get_worker()</code>, <code>set_worker()</code> — private FT balances in worker storage;
                  <code> env::signer_account_id()</code> for authorization
                </td>
              </tr>
              <tr>
                <td className="px-4 py-3 text-sm font-mono">test-storage-ark</td>
                <td className="px-4 py-3 text-sm text-gray-600">
                  All storage operations — comprehensive test suite for every API method
                </td>
              </tr>
              <tr className="bg-gray-50">
                <td className="px-4 py-3 text-sm font-mono">payment-keys-with-intents</td>
                <td className="px-4 py-3 text-sm text-gray-600">
                  <code>env::input_json()</code>, <code>env::output_json()</code> — structured I/O for token swap orchestration
                </td>
              </tr>
            </tbody>
          </table>
        </div>
      </section>

      {/* Minimal Project */}
      <section className="mb-12">
        <AnchorHeading id="minimal-project">Minimal Project Template</AnchorHeading>

        <AnchorHeading id="cargo-toml" level={3}>Cargo.toml</AnchorHeading>

        <SyntaxHighlighter language="toml" style={vscDarkPlus} className="rounded-lg mb-4">
          {`[package]
name = "my-outlayer-app"
version = "0.1.0"
edition = "2021"

[[bin]]
name = "my-outlayer-app"
path = "src/main.rs"

[dependencies]
outlayer = "0.1"
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"

[profile.release]
opt-level = "s"
lto = true
strip = true`}
        </SyntaxHighlighter>

        <AnchorHeading id="main-rs" level={3}>src/main.rs</AnchorHeading>

        <SyntaxHighlighter language="rust" style={vscDarkPlus} className="rounded-lg mb-4">
          {`use outlayer::{env, storage};
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
struct Request {
    action: String,
}

#[derive(Serialize)]
struct Response {
    count: i64,
    message: String,
}

fn main() {
    let req: Request = match env::input_json() {
        Ok(Some(r)) => r,
        _ => return env::output_string("No valid input"),
    };

    match req.action.as_str() {
        "increment" => {
            let count = storage::increment("counter", 1)
                .unwrap_or(0);
            env::output_json(&Response {
                count,
                message: format!("Counter incremented by {}",
                    env::signer_account_id().unwrap_or_default()),
            }).ok();
        }
        "get" => {
            let val = storage::get_string("counter")
                .ok().flatten()
                .and_then(|s| s.parse::<i64>().ok())
                .unwrap_or(0);
            env::output_json(&Response {
                count: val,
                message: "Current count".into(),
            }).ok();
        }
        _ => env::output_string("Unknown action"),
    }
}`}
        </SyntaxHighlighter>

        <SyntaxHighlighter language="bash" style={vscDarkPlus} className="rounded-lg mb-4">
          {`# Build
cargo build --target wasm32-wasip2 --release

# Output: target/wasm32-wasip2/release/my-outlayer-app.wasm`}
        </SyntaxHighlighter>
      </section>

      {/* API Reference */}
      <section className="mb-12">
        <AnchorHeading id="api-reference">API Reference</AnchorHeading>

        <AnchorHeading id="env-api" level={3}>outlayer::env</AnchorHeading>

        <div className="overflow-x-auto mb-6">
          <table className="min-w-full divide-y divide-gray-200 text-sm">
            <thead className="bg-gray-50">
              <tr>
                <th className="px-4 py-2 text-left text-xs font-medium text-gray-500 uppercase">Function</th>
                <th className="px-4 py-2 text-left text-xs font-medium text-gray-500 uppercase">Returns</th>
                <th className="px-4 py-2 text-left text-xs font-medium text-gray-500 uppercase">Description</th>
              </tr>
            </thead>
            <tbody className="bg-white divide-y divide-gray-200">
              <tr><td className="px-4 py-2 font-mono text-xs">signer_account_id()</td><td className="px-4 py-2 text-xs">Option&lt;String&gt;</td><td className="px-4 py-2 text-xs text-gray-600">User who initiated execution</td></tr>
              <tr className="bg-gray-50"><td className="px-4 py-2 font-mono text-xs">predecessor_account_id()</td><td className="px-4 py-2 text-xs">Option&lt;String&gt;</td><td className="px-4 py-2 text-xs text-gray-600">Contract that called OutLayer</td></tr>
              <tr><td className="px-4 py-2 font-mono text-xs">transaction_hash()</td><td className="px-4 py-2 text-xs">Option&lt;String&gt;</td><td className="px-4 py-2 text-xs text-gray-600">Transaction hash (if applicable)</td></tr>
              <tr className="bg-gray-50"><td className="px-4 py-2 font-mono text-xs">request_id()</td><td className="px-4 py-2 text-xs">Option&lt;String&gt;</td><td className="px-4 py-2 text-xs text-gray-600">OutLayer-assigned request ID</td></tr>
              <tr><td className="px-4 py-2 font-mono text-xs">input()</td><td className="px-4 py-2 text-xs">Vec&lt;u8&gt;</td><td className="px-4 py-2 text-xs text-gray-600">Raw stdin bytes</td></tr>
              <tr className="bg-gray-50"><td className="px-4 py-2 font-mono text-xs">input_string()</td><td className="px-4 py-2 text-xs">Option&lt;String&gt;</td><td className="px-4 py-2 text-xs text-gray-600">Stdin as UTF-8 string</td></tr>
              <tr><td className="px-4 py-2 font-mono text-xs">input_json&lt;T&gt;()</td><td className="px-4 py-2 text-xs">Result&lt;Option&lt;T&gt;&gt;</td><td className="px-4 py-2 text-xs text-gray-600">Deserialize stdin as JSON</td></tr>
              <tr className="bg-gray-50"><td className="px-4 py-2 font-mono text-xs">output(data)</td><td className="px-4 py-2 text-xs">()</td><td className="px-4 py-2 text-xs text-gray-600">Write bytes to stdout</td></tr>
              <tr><td className="px-4 py-2 font-mono text-xs">output_string(s)</td><td className="px-4 py-2 text-xs">()</td><td className="px-4 py-2 text-xs text-gray-600">Write string to stdout</td></tr>
              <tr className="bg-gray-50"><td className="px-4 py-2 font-mono text-xs">output_json(value)</td><td className="px-4 py-2 text-xs">Result&lt;()&gt;</td><td className="px-4 py-2 text-xs text-gray-600">Serialize and write JSON</td></tr>
              <tr><td className="px-4 py-2 font-mono text-xs">var(key)</td><td className="px-4 py-2 text-xs">Option&lt;String&gt;</td><td className="px-4 py-2 text-xs text-gray-600">Read env var (incl. secrets)</td></tr>
              <tr className="bg-gray-50"><td className="px-4 py-2 font-mono text-xs">has_var(key)</td><td className="px-4 py-2 text-xs">bool</td><td className="px-4 py-2 text-xs text-gray-600">Check if env var exists</td></tr>
            </tbody>
          </table>
        </div>

        <AnchorHeading id="storage-api" level={3}>outlayer::storage</AnchorHeading>

        <div className="overflow-x-auto mb-6">
          <table className="min-w-full divide-y divide-gray-200 text-sm">
            <thead className="bg-gray-50">
              <tr>
                <th className="px-4 py-2 text-left text-xs font-medium text-gray-500 uppercase">Function</th>
                <th className="px-4 py-2 text-left text-xs font-medium text-gray-500 uppercase">Description</th>
              </tr>
            </thead>
            <tbody className="bg-white divide-y divide-gray-200">
              <tr><td className="px-4 py-2 font-mono text-xs">set(key, value)</td><td className="px-4 py-2 text-xs text-gray-600">Store bytes (user-isolated)</td></tr>
              <tr className="bg-gray-50"><td className="px-4 py-2 font-mono text-xs">get(key)</td><td className="px-4 py-2 text-xs text-gray-600">Read bytes</td></tr>
              <tr><td className="px-4 py-2 font-mono text-xs">has(key)</td><td className="px-4 py-2 text-xs text-gray-600">Check existence</td></tr>
              <tr className="bg-gray-50"><td className="px-4 py-2 font-mono text-xs">delete(key)</td><td className="px-4 py-2 text-xs text-gray-600">Delete key</td></tr>
              <tr><td className="px-4 py-2 font-mono text-xs">list_keys(prefix)</td><td className="px-4 py-2 text-xs text-gray-600">List keys by prefix</td></tr>
              <tr className="bg-gray-50"><td className="px-4 py-2 font-mono text-xs">set_string / get_string</td><td className="px-4 py-2 text-xs text-gray-600">String convenience methods</td></tr>
              <tr><td className="px-4 py-2 font-mono text-xs">set_json / get_json</td><td className="px-4 py-2 text-xs text-gray-600">JSON convenience methods</td></tr>
              <tr className="bg-gray-50"><td className="px-4 py-2 font-mono text-xs">increment(key, delta)</td><td className="px-4 py-2 text-xs text-gray-600">Atomic counter increment</td></tr>
              <tr><td className="px-4 py-2 font-mono text-xs">decrement(key, delta)</td><td className="px-4 py-2 text-xs text-gray-600">Atomic counter decrement</td></tr>
              <tr className="bg-gray-50"><td className="px-4 py-2 font-mono text-xs">set_if_absent(key, value)</td><td className="px-4 py-2 text-xs text-gray-600">Insert only if key doesn&apos;t exist</td></tr>
              <tr><td className="px-4 py-2 font-mono text-xs">set_if_equals(key, expected, new)</td><td className="px-4 py-2 text-xs text-gray-600">Compare-and-swap</td></tr>
              <tr className="bg-gray-50"><td className="px-4 py-2 font-mono text-xs">set_worker(key, value)</td><td className="px-4 py-2 text-xs text-gray-600">Encrypted shared storage</td></tr>
              <tr><td className="px-4 py-2 font-mono text-xs">get_worker(key)</td><td className="px-4 py-2 text-xs text-gray-600">Read shared storage</td></tr>
              <tr className="bg-gray-50"><td className="px-4 py-2 font-mono text-xs">set_worker_with_options(key, val, encrypted)</td><td className="px-4 py-2 text-xs text-gray-600">Shared storage with encryption toggle</td></tr>
              <tr><td className="px-4 py-2 font-mono text-xs">get_worker_from_project(key, uuid)</td><td className="px-4 py-2 text-xs text-gray-600">Cross-project public data read</td></tr>
              <tr className="bg-gray-50"><td className="px-4 py-2 font-mono text-xs">get_by_version(key, hash)</td><td className="px-4 py-2 text-xs text-gray-600">Read data from previous WASM version</td></tr>
              <tr><td className="px-4 py-2 font-mono text-xs">clear_all()</td><td className="px-4 py-2 text-xs text-gray-600">Delete all data for current caller</td></tr>
              <tr className="bg-gray-50"><td className="px-4 py-2 font-mono text-xs">clear_version(hash)</td><td className="px-4 py-2 text-xs text-gray-600">Clean up old version data</td></tr>
            </tbody>
          </table>
        </div>
      </section>

      {/* Storage types */}
      <section className="mb-12">
        <AnchorHeading id="storage-types">Storage Types Overview</AnchorHeading>

        <div className="grid grid-cols-1 md:grid-cols-3 gap-4 mb-6">
          <div className="border border-blue-200 rounded-lg p-4 bg-blue-50">
            <h4 className="font-semibold text-blue-900 mb-2">User Storage</h4>
            <p className="text-sm text-gray-700 mb-2">
              <code>set()</code> / <code>get()</code>
            </p>
            <ul className="text-xs text-gray-600 space-y-1 list-disc list-inside">
              <li>Isolated per caller account</li>
              <li>Encrypted at rest</li>
              <li>alice.near can&apos;t read bob.near&apos;s data</li>
            </ul>
          </div>
          <div className="border border-green-200 rounded-lg p-4 bg-green-50">
            <h4 className="font-semibold text-green-900 mb-2">Worker Storage</h4>
            <p className="text-sm text-gray-700 mb-2">
              <code>set_worker()</code> / <code>get_worker()</code>
            </p>
            <ul className="text-xs text-gray-600 space-y-1 list-disc list-inside">
              <li>Shared across all callers</li>
              <li>Encrypted by default</li>
              <li>Only accessible within WASI module</li>
            </ul>
          </div>
          <div className="border border-purple-200 rounded-lg p-4 bg-purple-50">
            <h4 className="font-semibold text-purple-900 mb-2">Public Storage</h4>
            <p className="text-sm text-gray-700 mb-2">
              <code>set_worker_with_options(..., false)</code>
            </p>
            <ul className="text-xs text-gray-600 space-y-1 list-disc list-inside">
              <li>Unencrypted, readable by other projects</li>
              <li>Cross-project data sharing</li>
              <li>Useful for oracles, shared state</li>
            </ul>
          </div>
        </div>
      </section>
    </div>
  );
}
