# Dashboard Documentation Index

This file describes the dashboard documentation structure and source → rendered docs mapping.

## Quick Reference

- **Main Docs**: `/dashboard/app/docs/` - All documentation pages
- **Sections**: `/dashboard/app/docs/sections/` - Reusable documentation components
- **Examples**: `/wasi-examples/*/README.md` - Source for example documentation
- **Navigation**: `/dashboard/app/docs/layout.tsx` - Sidebar menu configuration
- **Live Site**: https://dashboard.outlayer.io/docs

## Documentation Structure

```
dashboard/app/docs/
├── page.tsx                      # Main documentation page (overview)
├── layout.tsx                    # Sidebar navigation menu
├── getting-started/page.tsx      # Getting Started (from sections/GettingStarted.tsx)
├── architecture/page.tsx         # Architecture overview
├── contract-integration/page.tsx # Contract integration guide
├── dev-guide/page.tsx            # Developer Guide (from sections/DeveloperGuide.tsx)
├── wasi/page.tsx                 # Writing WASI Code (comprehensive guide)
├── secrets/page.tsx              # Secrets Management
├── pricing/page.tsx              # Pricing model
├── tee-attestation/page.tsx      # TEE Attestation (from sections/TeeAttestation.tsx)
├── examples/page.tsx             # ✨ Example Projects (all examples)
└── sections/
    ├── index.tsx                 # Export all sections
    ├── GettingStarted.tsx        # Source for getting-started/page.tsx
    ├── DeveloperGuide.tsx        # Source for dev-guide/page.tsx
    └── TeeAttestation.tsx        # Source for tee-attestation/page.tsx
```

## Example Mapping: Source → Dashboard

| Example Name | Source Location | README | Dashboard Page | Status | WASI | Level |
|--------------|----------------|--------|----------------|--------|------|-------|
| **random-ark** | `wasi-examples/random-ark/` | ✅ | `/docs/examples#random-ark` | ✅ Shown | P1 | Beginner |
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
| `/docs/getting-started` | `dashboard/app/docs/sections/GettingStarted.tsx` | - | Quick start guide, first contract |
| `/docs/architecture` | `dashboard/app/docs/architecture/page.tsx` | - | System design, components |
| `/docs/contract-integration` | `dashboard/app/docs/contract-integration/page.tsx` | `contract/README.md` | Contract API, integration |
| `/docs/dev-guide` | `dashboard/app/docs/sections/DeveloperGuide.tsx` | - | Development workflow, best practices |
| `/docs/wasi` | `dashboard/app/docs/sections/index.tsx` (WasiSection) | `wasi-examples/WASI_TUTORIAL.md`, `worker/wit/world.wit` | WASI programming, host functions |
| `/docs/secrets` | `dashboard/app/docs/sections/index.tsx` (SecretsSection) | `keystore-dao-contract/README.md` | Secrets management, CKD, Keystore DAO |
| `/docs/pricing` | `dashboard/app/docs/sections/index.tsx` (PricingSection) | - | Cost model, resource limits |
| `/docs/tee-attestation` | `dashboard/app/docs/sections/TeeAttestation.tsx` | `TEE_ATTESTATION_FLOW.md` | TEE verification, attestation |
| `/docs/examples` | `dashboard/app/docs/examples/page.tsx` | `wasi-examples/*/README.md` | All example projects |

## Navigation (layout.tsx)

**IMPORTANT**: When adding a new example, you MUST update `pageStructure['/docs/examples']` in `dashboard/app/docs/layout.tsx`!

**IMPORTANT**: When adding a new section to any page (e.g., `/docs/wasi#host-functions`), you MUST add it to `pageStructure` for that page!

Sidebar menu in `dashboard/app/docs/layout.tsx` is managed through the `pageStructure` object:

```tsx
const pageStructure = {
  '/docs/examples': [
    { id: 'random-ark', title: 'Random Number' },
    { id: 'echo-ark', title: 'Echo' },
    { id: 'ai-ark', title: 'AI Integration' },
    { id: 'weather-ark', title: 'Weather Oracle' },
    { id: 'botfather-ark', title: 'Bot Father' },  // ← added
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
