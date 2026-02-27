# Dashboard Documentation Index

This file describes the dashboard documentation structure and source → rendered docs mapping.

> **IMPORTANT**: This file is an INDEX only - it describes the documentation structure but is NOT the documentation itself.
> When adding new features or updating docs, you MUST:
> 1. Create/update the actual documentation pages in `/dashboard/app/docs/`
> 2. Update navigation in `/dashboard/app/docs/layout.tsx`
> 3. Then update this index to reflect the changes
>
> Users read documentation at https://outlayer.fastnear.com/docs, not this file!

## Core Value Proposition

OutLayer provides **verifiable off-chain computation** with TEE attestation. Two equal integration modes:

1. **HTTPS API** - Direct HTTP calls, pay with USDC, instant response. Ideal for web apps, APIs, monetization.
2. **Blockchain (NEAR)** - Smart contract integration via yield/resume. Ideal for DeFi, DAOs, on-chain apps.

Both modes provide the same cryptographic proof via Intel TDX attestation.

## Quick Reference

- **Main Docs**: `/dashboard/app/docs/` - All documentation pages
- **Sections**: `/dashboard/app/docs/sections/` - Reusable documentation components
- **Examples**: `/wasi-examples/*/README.md` - Source for example documentation
- **Navigation**: `/dashboard/app/docs/layout.tsx` - Sidebar menu configuration
- **Live Site**: https://outlayer.fastnear.com/docs

## Documentation Structure

```
dashboard/app/docs/
├── page.tsx                      # Main documentation page (overview)
├── layout.tsx                    # Sidebar navigation menu
├── agent-custody/page.tsx        # Agent Custody — multi-chain wallet for AI agents
├── getting-started/page.tsx      # Getting Started (from sections/GettingStarted.tsx)
├── web2-integration/page.tsx     # Web2 Integration - HTTPS API with TEE attestation
├── https-api/page.tsx            # ✨ HTTPS API (NEW) - full API reference
├── payment-keys/page.tsx         # ✨ Payment Keys (NEW) - prepaid USD keys for API access
├── earnings/page.tsx             # ✨ Earnings (NEW) - developer monetization
├── near-integration/page.tsx     # NEAR Integration - smart contract integration
├── dev-guide/page.tsx            # Developer Guide (from sections/DeveloperGuide.tsx)
├── wasi/page.tsx                 # Building OutLayer App (comprehensive guide)
├── secrets/page.tsx              # Secrets Management
├── projects/page.tsx             # Projects & Versions
├── pricing/page.tsx              # Pricing model
├── tee-attestation/page.tsx      # TEE Attestation (from sections/TeeAttestation.tsx)
├── vrf/page.tsx                  # VRF (Verifiable Random Function)
├── examples/page.tsx             # Example Projects (all examples)
├── trust-verification/page.tsx   # Trust & Verification - why trust OutLayer
├── storage/page.tsx              # Persistent Storage
└── sections/
    ├── index.tsx                 # Re-exports all sections
    ├── utils.tsx                 # Shared components (AnchorHeading, useHashNavigation)
    ├── GettingStarted.tsx        # Getting started content
    ├── DeveloperGuide.tsx        # Developer guide content
    ├── ContractIntegration.tsx   # Contract integration content
    ├── Wasi.tsx                  # WASI programming content
    ├── Secrets.tsx               # Secrets management content
    ├── Pricing.tsx               # Pricing content
    └── TeeAttestation.tsx        # TEE attestation content
```

## Example Mapping: Source → Dashboard

| Example Name | Source Location | README | Dashboard Page | Status | WASI | Level |
|--------------|----------------|--------|----------------|--------|------|-------|
| **random-ark** | `wasi-examples/random-ark/` | ✅ | `/docs/examples#random-ark` | ✅ Shown | P1 | Beginner |
| **vrf-ark** | `wasi-examples/vrf-ark/` | ✅ | `/docs/examples#vrf-ark` | ✅ Shown | P1 | Intermediate |
| **echo-ark** | `wasi-examples/echo-ark/` | ✅ | `/docs/examples#echo-ark` | ✅ Shown | P1 | Beginner |
| **ai-ark** | `wasi-examples/ai-ark/` | ✅ | `/docs/examples#ai-ark` | ✅ Shown | P2 | Intermediate |
| **weather-ark** | `wasi-examples/weather-ark/` | ✅ | `/docs/examples#weather-ark` | ✅ Shown | P2 | Intermediate |
| **oracle-ark** | `wasi-examples/oracle-ark/` | ✅ | `/docs/examples#oracle-ark` | ✅ Shown | P2 | Advanced |
| **ethereum-api** | `wasi-examples/ethereum-api/` | ✅ | `/docs/examples#ethereum-api` | ✅ Shown | P2 | Intermediate |
| **botfather-ark** | `wasi-examples/botfather-ark/` | ✅ | `/docs/examples#botfather-ark` | ✅ Shown | P2 | Advanced |
| **intents-ark** | `wasi-examples/intents-ark/` | ✅ | `/docs/examples#intents-ark` | ✅ Shown | P2 | Advanced |
| **private-dao-ark** | `wasi-examples/private-dao-ark/` | ✅ | `/docs/examples#private-dao-ark` | ✅ Shown | P2 | Advanced |
| **captcha-ark** | `wasi-examples/captcha-ark/` | ✅ | `/docs/examples#captcha-ark` | ✅ Shown | P2 | Advanced |
| **test-secrets-ark** | `wasi-examples/test-secrets-ark/` | ✅ | - | ❌ Hidden (test) | P2 | - |
| **rpc-test-ark** | `wasi-examples/rpc-test-ark/` | ✅ | - | ❌ Hidden (test) | P2 | - |
| **test-storage-ark** | `wasi-examples/test-storage-ark/` | ✅ | - | ❌ Hidden (test) | P2 | - |
| **near-email** | `wasi-examples/near-email/` | ✅ | `/docs/examples#near-email` | ✅ Shown | P2 | Advanced |
| **private-token-ark** | `wasi-examples/private-token-ark/` | ✅ | `/docs/examples#private-token-ark` | ✅ Shown | P2 | Advanced |
| **wasi-test-runner** | `wasi-examples/wasi-test-runner/` | ❌ | - | ❌ Hidden (infra) | - | - |

## Example Block Format in examples/page.tsx

```tsx
<div id="example-name-ark" className="border border-gray-200 rounded-lg p-6 hover:shadow-lg transition-shadow scroll-mt-4">
  <AnchorHeading
    id="example-name-ark"
    badges={
      <>
        <span className="ml-3 text-sm bg-blue-100 text-blue-800 px-3 py-1 rounded">WASI P1/P2</span>
        <span className="ml-2 text-sm bg-green-100 text-green-800 px-3 py-1 rounded">Beginner/Intermediate/Advanced</span>
      </>
    }
  >
    example-name-ark
  </AnchorHeading>

  {/* Links to GitHub and Playground */}
  <div className="flex flex-wrap gap-3 mt-4 mb-4">
    <a href="https://github.com/zavodil/example-name-ark" ...>Source Code on GitHub</a>
    <Link href="/playground#example-name">Try in Playground</Link>
  </div>

  {/* Description */}
  <p className="text-gray-700 mb-4">
    Short description of what this example does.
  </p>

  {/* Use Cases */}
  <h4 className="font-semibold text-gray-900 mb-2">Use Cases</h4>
  <ul className="list-disc list-inside text-gray-700 mb-4 space-y-1">
    <li>Use case 1</li>
    <li>Use case 2</li>
  </ul>

  {/* Key Features */}
  <h4 className="font-semibold text-gray-900 mb-2">Key Features</h4>
  <ul className="list-disc list-inside text-gray-700 mb-4 space-y-1">
    <li>Feature 1</li>
    <li>Feature 2</li>
  </ul>

  {/* Code Example (optional) */}
  <div className="mb-4">
    <h4 className="font-semibold text-gray-900 mb-2">Code Example</h4>
    <SyntaxHighlighter language="rust" style={vscDarkPlus}>
      {`code here`}
    </SyntaxHighlighter>
  </div>

  {/* How It Works (optional) */}
  <div className="mb-4 bg-blue-50 border-l-4 border-blue-400 p-4">
    <h4 className="font-semibold text-gray-900 mb-2">How It Works</h4>
    <ol className="list-decimal list-inside text-sm text-gray-700 space-y-1">
      <li>Step 1</li>
      <li>Step 2</li>
    </ol>
  </div>

  {/* Technical Details (optional) */}
  <h4 className="font-semibold text-gray-900 mb-2">Technical Details</h4>
  <ul className="list-disc list-inside text-gray-700 mb-4 space-y-1 text-sm">
    <li><strong>WASI Version:</strong> Preview 1 / Preview 2</li>
    <li><strong>Language:</strong> Rust</li>
    <li><strong>Secrets:</strong> Required / Optional / Not needed</li>
    <li><strong>Network:</strong> Required / Not needed</li>
  </ul>

  {/* Learn More */}
  <div className="mt-4 p-4 bg-gray-50 rounded-lg">
    <h4 className="font-semibold text-gray-900 mb-2">Learn More</h4>
    <ul className="text-sm text-gray-700 space-y-1">
      <li>
        📖 <a href="..." className="text-[var(--primary-orange)] hover:underline">Full Documentation</a>
      </li>
      <li>
        🎮 <Link href="/playground#example" className="text-[var(--primary-orange)] hover:underline">Try in Playground</Link>
      </li>
    </ul>
  </div>
</div>
```

## How to Add a New Example

1. **Create source code** in `wasi-examples/new-ark/`
2. **Write README.md** in `wasi-examples/new-ark/README.md` with:
   - Description
   - Use Cases
   - Key Features
   - Technical Details
   - Usage Examples
3. **Add section** to `dashboard/app/docs/examples/page.tsx`:
   - Copy existing example block
   - Replace `id`, title, description, use cases, features
   - Add GitHub repo link
   - Add Playground link (if applicable)
4. **Update this index** (DOCS_INDEX.md)

## Source Files for Each Page

| Dashboard Page | Primary Source | Secondary Sources | Key Topics |
|----------------|---------------|-------------------|------------|
| `/docs` | `dashboard/app/docs/page.tsx` | - | Overview, getting started |
| `/docs/agent-custody` | `dashboard/app/docs/agent-custody/page.tsx` | `.ignore/FIREBLOCKS.md`, `dashboard/public/SKILL.md` | **Agent Custody: multi-chain wallet, policy engine, gasless transfers, MPC security** |
| `/docs/getting-started` | `dashboard/app/docs/sections/GettingStarted.tsx` | - | **TEE attestation, two integration modes (HTTPS & Blockchain), quick start** |
| `/docs/web2-integration` | `dashboard/app/docs/web2-integration/page.tsx` | - | HTTPS integration overview, TEE attestation |
| `/docs/https-api` | `dashboard/app/docs/https-api/page.tsx` | `DESIGN_HTTPS_API.md` | **Full HTTPS API reference, headers, responses** |
| `/docs/payment-keys` | `dashboard/app/docs/payment-keys/page.tsx` | `DESIGN_HTTPS_API.md` | **Payment Keys: creation, restrictions, balance** |
| `/docs/earnings` | `dashboard/app/docs/earnings/page.tsx` | `DESIGN_HTTPS_API.md` | **Developer earnings, USD_PAYMENT, monetization** |
| `/docs/near-integration` | `dashboard/app/docs/near-integration/page.tsx` | `contract/README.md` | NEAR smart contract integration, yield/resume |
| `/docs/dev-guide` | `dashboard/app/docs/sections/DeveloperGuide.tsx` | - | Development workflow, best practices |
| `/docs/wasi` | `dashboard/app/docs/sections/Wasi.tsx` | `wasi-examples/WASI_TUTORIAL.md`, `worker/wit/world.wit` | WASI programming, host functions |
| `/docs/secrets` | `dashboard/app/docs/sections/Secrets.tsx` | `keystore-dao-contract/README.md` | Secrets management, CKD, Keystore DAO |
| `/docs/projects` | `dashboard/app/docs/projects/page.tsx` | `contract/src/projects.rs` | Projects, versions, persistent storage, project secrets |
| `/docs/pricing` | `dashboard/app/docs/sections/Pricing.tsx` | - | Cost model, resource limits |
| `/docs/tee-attestation` | `dashboard/app/docs/sections/TeeAttestation.tsx` | `TEE_ATTESTATION_FLOW.md` | TEE verification, attestation |
| `/docs/vrf` | `dashboard/app/docs/vrf/page.tsx` | `VRF.md`, `sdk/outlayer/src/vrf.rs` | **VRF: verifiable randomness, SDK, on-chain verification** |
| `/docs/examples` | `dashboard/app/docs/examples/page.tsx` | `wasi-examples/*/README.md` | All example projects |

## Navigation (layout.tsx)

**IMPORTANT**: When adding a new example, you MUST update `pageStructure['/docs/examples']` in `dashboard/app/docs/layout.tsx`!

**IMPORTANT**: When adding a new section to any page (e.g., `/docs/wasi#host-functions`), you MUST add it to `pageStructure` for that page!

Sidebar menu in `dashboard/app/docs/layout.tsx` is managed through the `pageStructure` object:

```tsx
const pageStructure = {
  '/docs/getting-started': [
    { id: 'what-is-outlayer', title: 'What is OutLayer?' },
    { id: 'tee-attestation', title: 'Verifiable Execution (TEE)' },
    { id: 'two-modes', title: 'Two Ways to Use' },
    { id: 'blockchain-flow', title: 'Blockchain Flow' },
    { id: 'https-flow', title: 'HTTPS Flow' },
    { id: 'why-outlayer', title: 'Why OutLayer' },
    { id: 'quick-start', title: 'Quick Start' },
    { id: 'secrets', title: 'Secrets' },
    { id: 'payment', title: 'Payment & Pricing' },
    { id: 'persistent-storage', title: 'Persistent Storage' },
  ],
  '/docs/agent-custody': [
    { id: 'how-it-works', title: 'How It Works' },
    { id: 'agent-id', title: 'Agent ID' },
    { id: 'multi-chain', title: 'Multi-Chain Support' },
    { id: 'policy-engine', title: 'Policy Engine' },
    { id: 'multisig', title: 'Multisig Approval' },
    { id: 'quick-start', title: 'Quick Start' },
    { id: 'api-reference', title: 'API Reference' },
    { id: 'security', title: 'Security Model' },
    { id: 'ai-skill', title: 'AI Agent Skill File' },
    { id: 'comparison', title: 'Comparison' },
    { id: 'dashboard', title: 'Dashboard' },
  ],
  '/docs/examples': [
    { id: 'random-ark', title: 'Random Number' },
    { id: 'echo-ark', title: 'Echo' },
    { id: 'ai-ark', title: 'AI Integration' },
    { id: 'weather-ark', title: 'Weather Oracle' },
    { id: 'botfather-ark', title: 'Bot Father' },
    { id: 'oracle-ark', title: 'Price Oracle' },
    { id: 'ethereum-api', title: 'Ethereum API' },
    { id: 'intents-ark', title: 'NEAR Intents swap' },
    { id: 'private-dao-ark', title: 'Private DAO Voting' },
    { id: 'captcha-ark', title: '2FA Verification' },
  ],
  // ... other pages
}
```

**Adding Order**:
1. Determine difficulty level (Beginner → after echo-ark, Intermediate → after weather-ark, Advanced → at end)
2. Add entry to `pageStructure['/docs/examples']` array in `layout.tsx`
3. Use same `id` as in `page.tsx` (e.g., `botfather-ark`)
4. Provide clear `title` for menu (e.g., `Bot Father`)

**Working Examples Links** (in `sections/index.tsx`):
- Each example card has TWO links:
  - Primary link: `<Link href="/docs/examples#example-ark">` - internal navigation to detailed docs
  - Secondary link: GitHub icon (SVG) - external link to source code repository
- This structure allows users to quickly navigate to detailed documentation or view source code

## Badges

### WASI Version
- `WASI P1` - WASI Preview 1 (`wasm32-wasip1`) - blue badge
- `WASI P2` - WASI Preview 2 (`wasm32-wasip2`) - blue badge

### Difficulty Level
- `Beginner` - green badge (green-100/green-800)
- `Intermediate` - yellow badge (yellow-100/yellow-800)
- `Advanced` - red badge (red-100/red-800)

### Feature Badges (optional)
- `Secrets` - purple badge (purple-100/purple-800)
- `Network` - cyan badge (cyan-100/cyan-800)
- `Transactions` - orange badge (orange-100/orange-800)
- `Host Functions` - orange badge (orange-100/orange-800)

## Key Security Features Documentation

### Keystore DAO Architecture
- **Location**: `/docs/secrets#confidential-key-derivation`
- **Contract**: `keystore-dao.outlayer.testnet`
- **Key Points**:
  - Keystore uses functional keys (not full access keys)
  - Can ONLY call MPC through DAO contract's `request_key` method
  - All key derivation requests are auditable on-chain
  - DAO governance controls keystore approval
  - MPC Contract: `v1.signer-prod.testnet`

### CKD (Confidential Key Derivation)
- **Location**: `/docs/secrets#confidential-key-derivation`
- **Purpose**: Deterministic key generation via NEAR MPC Network
- **Flow**: Keystore TEE → DAO Contract → MPC Contract → MPC Network
- **Security**: Keys never leave TEE, distributed computation

### Access Control
- **Manual Secrets**: User-provided, cannot use `PROTECTED_*` prefix
- **Auto-Generated Secrets**: TEE-generated, must use `PROTECTED_*` prefix
- **Validation**: Keystore validates access conditions before decryption

## Projects & Versions (NEW)

### Overview
Projects allow users to organize WASM code versions with shared persistent storage and secrets.

### Key Features
- **Versioning**: Multiple WASM versions per project, switch active version anytime
- **Persistent Storage**: Data persists across version updates (same encryption key)
- **Project Secrets**: Secrets bound to project, accessible by all versions
- **Storage Deposit**: Pay for on-chain storage, refunded when deleted

### Project ID Format
```
{owner_account_id}/{project_name}
```
Example: `alice.near/my-app`

### How Project Binding Works

When you call `request_execution(Project { project_id, ... })`, the contract:
1. Looks up your project and resolves the active version's `CodeSource`
2. Passes `project_uuid` to the worker along with the execution request
3. Worker uses `project_uuid` for storage namespace and encryption

**You don't need to declare project ID in your WASM** - the contract determines which WASM runs for which project. This is secure because WASM cannot fake its project binding.

```rust
use outlayer::storage;

fn main() {
    // Storage just works - project_uuid comes from contract
    storage::set("my-key", b"my-value").unwrap();
}
```

### Dashboard Pages
- `/projects` - List, create, manage projects
- `/secrets` (Project tab) - Create secrets bound to a project

### Contract Methods
- `create_project(name, source)` - Create project with first version
- `add_version(project_name, source, set_active)` - Add new version
- `set_active_version(project_name, wasm_hash)` - Switch active version
- `remove_version(project_name, wasm_hash)` - Remove a version
- `delete_project(project_name)` - Delete entire project
- `list_user_projects(account_id)` - List user's projects
- `get_project(project_id)` - Get project details
- `get_version(project_id, wasm_hash)` - Get version details

### Secrets with SecretAccessor::Project
When creating secrets for a project, use:
```json
{
  "accessor": { "Project": { "project_id": "alice.near/my-app" } },
  "profile": "production",
  ...
}
```

### Documentation Status
- **Dashboard page**: `/projects` - ✅ Implemented
- **Secrets page update**: Project accessor - ✅ Implemented
- **Docs page**: `/docs/projects` - ✅ Created (`dashboard/app/docs/projects/page.tsx`)

## Persistent Storage (NEW)

### Overview
OutLayer provides encrypted persistent storage for WASM modules via host functions.

### Architecture
```
WASM Code
    │ WIT host function calls (near:rpc/storage)
    ▼
OutLayer Worker (host_functions.rs)
    │ calls StorageClient
    ▼
StorageClient → Keystore (encrypt/decrypt via TEE)
    │ encrypted data
    ▼
Coordinator API (/storage/*)
    │
    ▼
PostgreSQL (storage_data table)
```

### OutLayer SDK
Published on crates.io: [`outlayer`](https://crates.io/crates/outlayer)

```toml
[dependencies]
outlayer = "0.1"
```

### Storage API (WIT Interface)
Located at: `sdk/outlayer/wit/deps/storage.wit`

```wit
interface api {
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
    // is-encrypted: true (default) = encrypted, only this project can read
    //               false = plaintext, other projects can read via get-worker with project-uuid
    set-worker: func(key: string, value: list<u8>, is-encrypted: option<bool>) -> string;
    // project-uuid: none = current project, some("p0000000000000001") = read from another project
    get-worker: func(key: string, project-uuid: option<string>) -> tuple<list<u8>, string>;

    // Version migration
    get-by-version: func(key: string, wasm-hash: string) -> tuple<list<u8>, string>;

    // Cleanup
    clear-all: func() -> string;
    clear-version: func(wasm-hash: string) -> string;
}
```

### Conditional Writes (Atomic Operations)

| Method | Description | Returns |
|--------|-------------|---------|
| `set_if_absent(key, value)` | Insert only if key doesn't exist | `(inserted: bool, error)` |
| `set_if_equals(key, expected, new)` | Compare-and-swap (CAS) | `(success, current_value, error)` |
| `increment(key, delta)` | Atomic i64 increment | `(new_value: i64, error)` |
| `decrement(key, delta)` | Atomic i64 decrement | `(new_value: i64, error)` |

**Use cases:**
- `set_if_absent`: One-time initialization, default values
- `set_if_equals`: Optimistic locking, complex state transitions
- `increment`/`decrement`: Counters, rate limiters, inventory management

### Storage Key Structure

Understanding how storage keys work is essential for using OutLayer storage correctly.

**User Storage (isolated per account)**

When a user calls `storage::set("balance", "100")`, the actual database key includes the account ID:

```
// alice.near calls execution:
storage::set("balance", "100")
// Database key: project_uuid:alice.near:balance = "100"

// bob.near calls execution:
storage::set("balance", "200")
// Database key: project_uuid:bob.near:balance = "200"

// alice.near reads:
storage::get("balance")  // → "100" (her data)

// bob.near reads:
storage::get("balance")  // → "200" (his data)
```

**Key points:**
- WASM code CANNOT read another user's data
- There is no function like `storage::get_for_account("bob.near", "balance")`
- User data is only accessible when that user triggers the execution

**Worker Storage (shared across all users)**

When WASM calls `storage::set_worker("key", value)`, the account is replaced with `@worker`:

```
// Any user calls execution:
storage::set_worker("total_count", "100")
// Database key: project_uuid:@worker:total_count = "100"

// Any other user reads:
storage::get_worker("total_count")  // → "100" (same data)
```

**Key point:** Worker storage is shared, but users cannot directly access it. Only WASM code can call `get_worker`/`set_worker`. Users interact with worker data only through WASM logic (e.g., calling a method that returns aggregated stats).

**Public Storage (cross-project readable)**

Public storage is unencrypted worker storage that can be read by other projects. Use case: oracle price feeds, shared configuration.

```rust
use outlayer::storage;

// Store public data (is_encrypted = false)
storage::set_worker_with_options("oracle:ETH", price_json.as_bytes(), Some(false))?;

// Read from current project
let data = storage::get_worker("oracle:ETH")?;

// Read from another project by UUID (public data only)
let data = storage::get_worker_from_project("oracle:ETH", Some("p0000000000000001"))?;
```

**External HTTP API:**
```bash
# JSON format (default) - base64 encoded value
curl "https://api.outlayer.fastnear.com/public/storage/get?project_uuid=p0000000000000001&key=oracle:ETH"
# {"exists":true,"value":"<base64-encoded-value>"}

# Raw format - returns raw bytes directly
curl "https://api.outlayer.fastnear.com/public/storage/get?...&format=raw"
```

**Key points:**
- `is_encrypted=false` makes data readable by other projects
- Other projects read via `project_uuid` (e.g., `p0000000000000001`)
- External clients read via HTTP endpoint (returns base64-encoded value)
- Encrypted (default) worker data is NOT accessible cross-project

### Version Migration

The `wasm_hash` is stored with each record but NOT included in the unique key constraint. This means:
- New WASM versions automatically read data written by old versions
- Use `storage::get_by_version("key", "old_wasm_hash")` to explicitly read old version's data

### Encryption
- **Project-based**: `storage:{project_uuid}:{account_id}`
- **Worker-private**: Uses `@worker` as account_id
- All encryption handled by Keystore TEE (not local worker)
- User isolation is automatic at protocol level

### Test Example
- **test-storage-ark**: `wasi-examples/test-storage-ark/`
- Tests all storage host functions including public storage
- Basic: set, get, delete, has, list, set_worker, get_worker, clear_all
- Conditional: set_if_absent, set_if_equals, increment, decrement
- Public: set_public, get_public_cross, verify_public_http
- Tests: test_all, test_public_storage

### Use Cases
- User preferences across executions
- Counters and state persistence (use `increment`/`decrement` for thread-safe counters)
- Caching expensive computations
- Session data storage
- Private WASM-only state
- **Oracle price feeds** (public storage for cross-project reads)
- **Distributed locks** (use `set_if_absent`)
- **Optimistic updates** (use `set_if_equals` for compare-and-swap)

## HTTPS API & Payment Keys

### Overview
HTTPS API is one of two equal ways to use OutLayer (alongside Blockchain/NEAR integration). Both provide the same TEE attestation guarantees.

**HTTPS mode is ideal for:**
- **Monetization** - Developers can charge for API access
- **Proof to users** - Cryptographic proof of what code ran
- **Web/Mobile apps** - No blockchain knowledge needed
- **USD payments** - Via prepaid Payment Keys

### Documentation Pages

| Page | Description |
|------|-------------|
| `/docs/web2-integration` | HTTPS integration overview, TEE attestation |
| `/docs/https-api` | **Full HTTPS API reference** - endpoints, headers, responses, examples |
| `/docs/payment-keys` | **Payment Keys** - creation, restrictions, balance management |
| `/docs/earnings` | **Developer earnings** - USD_PAYMENT, monetization strategies |

### Dashboard Pages
- `/payment-keys` - Create and manage Payment Keys, top up balance
- `/earnings` - View accumulated earnings, withdraw

### Quick Summary

**How it works:**
1. Create Payment Key at `/payment-keys` with USD deposit
2. Call: `POST https://api.outlayer.fastnear.com/call/{owner}/{project}` with `X-Payment-Key` header
3. Optionally attach payment to project author via `X-Attached-Deposit` header
4. WASM reads payment via `USD_PAYMENT` env var

**Key headers:**
- `X-Payment-Key` (required): `owner:nonce:secret`
- `X-Compute-Limit` (optional): max compute budget in USD micro-units
- `X-Attached-Deposit` (optional): payment to project author

**See detailed documentation:**
- [HTTPS API](/docs/https-api) - Full API reference
- [Payment Keys](/docs/payment-keys) - Key management
- [Earnings](/docs/earnings) - Monetization guide

### Implementation Status

**Two Integration Modes (equal priority):**
- ✅ `/docs/web2-integration` - HTTPS API overview
- ✅ `/docs/https-api` - Full HTTPS API reference
- ✅ `/docs/near-integration` - NEAR smart contract integration (yield/resume)

**Payment & Monetization:**
- ✅ `/docs/payment-keys` - Payment Keys documentation
- ✅ `/docs/earnings` - Developer earnings documentation
- ✅ Dashboard `/payment-keys` - UI for key management
- ✅ Dashboard `/earnings` - UI for earnings

## Documentation Update Checklist

- [ ] Update source file (`examples/page.tsx`)
- [ ] **REQUIRED**: Add example to `pageStructure['/docs/examples']` in `layout.tsx` (otherwise won't appear in menu!)
- [ ] **REQUIRED**: Add example to "Working Examples" section in `sections/index.tsx` (WasiSection file, `/docs/wasi#working-examples`)
- [ ] Update related README.md (if exists)
- [ ] Check layout (npm run build)
- [ ] Verify all links (internal and external)
- [ ] Update this index (DOCS_INDEX.md)
- [ ] Add anchor links for new sections
- [ ] Test hash navigation (#section-name)
