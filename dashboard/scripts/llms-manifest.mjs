// Editorial source for /llms.txt and /llms-full.txt.
//
// llms.txt follows the spec at https://llmstxt.org: H1, blockquote summary,
// optional free-form details, then H2-delimited lists of links. The "Optional"
// section has defined meaning — its links may be skipped when context is tight.
//
// Everything an LLM should be able to find is listed here. Keep summaries factual
// and specific: they are the only thing an agent sees before deciding what to fetch.

export const SITE = {
  name: 'NEAR OutLayer',
  baseUrl: 'https://outlayer.fastnear.com',
  repoUrl: 'https://github.com/fastnear/near-outlayer',
  rawBaseUrl: 'https://raw.githubusercontent.com/fastnear/near-outlayer/main',

  summary:
    'Verifiable off-chain computation for NEAR. You publish a WASI program to GitHub; ' +
    'OutLayer compiles it, runs it inside an Intel TDX enclave, and returns the result ' +
    'signed by the enclave — callable from a NEAR smart contract or over HTTPS. Agents get ' +
    'encrypted secrets, persistent storage, and TEE-held wallets that can sign for NEAR, EVM ' +
    'and Solana without a key ever leaving the enclave.',

  details: [
    'Two integration modes. **On-chain:** a NEAR contract calls `request_execution` and receives the result in a callback. **Web2:** a backend calls `POST https://api.outlayer.fastnear.com/call/{project_owner}/{project_name}` over HTTPS.',
    '**API base URL is `https://api.outlayer.fastnear.com`** — the `api.` subdomain. `https://outlayer.fastnear.com` is the dashboard and docs site and serves no API. Testnet API base is `https://testnet-api.outlayer.fastnear.com`.',
    'HTTPS authentication is either a payment key (`X-Payment-Key: {owner}:{nonce}:{secret}`) or a trial worker key (`Authorization: Bearer wk_...`). There is no `X-API-Key` header.',
    'Mainnet contract: `outlayer.near`. Testnet contract: `outlayer.testnet`. NEAR RPC: `https://rpc.mainnet.fastnear.com` / `https://rpc.testnet.fastnear.com`.',
    'Full text of every document below is available in one fetch at ' + 'https://outlayer.fastnear.com/llms-full.txt.',
  ],
}

// Documentation pages on the site, in navigation order.
export const SECTIONS = [
  {
    title: 'Start here',
    pages: [
      {
        title: 'Getting Started',
        path: '/docs/getting-started',
        summary:
          'What OutLayer is, the two integration modes (NEAR contract callback vs. HTTPS API), and a first working agent',
      },
      {
        title: 'Examples',
        path: '/docs/examples',
        summary:
          'Runnable example agents: verifiable randomness, price and weather oracles, AI inference, NEAR Intents swaps, private DAO voting, 2FA, Telegram bot, on-chain email',
      },
    ],
  },
  {
    title: 'Guides',
    pages: [
      {
        title: 'NEAR Contract Integration',
        path: '/docs/near-integration',
        summary:
          'Calling `request_execution` from a NEAR contract: arguments, callback shape, gas and payment, FastFS workflow, attestation of the returned result',
      },
      {
        title: 'Web2 Integration',
        path: '/docs/web2-integration',
        summary:
          'Using OutLayer from an ordinary backend over HTTPS: quick start, monetization, environment variables, attestation, project capabilities',
      },
      {
        title: 'WASI Guide',
        path: '/docs/wasi',
        summary:
          'Writing the agent itself: WASI Preview 1 vs. Preview 2, supported languages, host functions, resource limits, local testing, common pitfalls',
      },
      {
        title: 'Developer Guide',
        path: '/docs/dev-guide',
        summary:
          'End-to-end path: write WASI code, push to GitHub, test on testnet, integrate from a contract',
      },
    ],
  },
  {
    title: 'Features',
    pages: [
      {
        title: 'Agent Custody',
        path: '/docs/agent-custody',
        summary:
          'TEE-held wallets for agents: agent IDs, multi-chain signing, policy engine, multisig approval, confidential NEAR Intents, NEP-413 message signing, sub-agents',
      },
      {
        title: 'MPC Vaults',
        path: '/docs/vaults',
        summary:
          'Per-customer sovereign vaults: creation, vault-scoped secrets and wallets, recovery procedures, end-user verification, threat model',
      },
      {
        title: 'Payment Checks',
        path: '/docs/payment-checks',
        summary:
          'Gasless agent-to-agent payments: transfer mechanism, key derivation, check lifecycle, security model',
      },
      {
        title: 'Secrets',
        path: '/docs/secrets',
        summary:
          'Encrypted secrets decrypted only inside the enclave: binding types, access control, storage costs, confidential key derivation (CKD), DAO-governed keystore',
      },
      {
        title: 'Projects',
        path: '/docs/projects',
        summary:
          'Project IDs, how a WASM binary is bound to a project, project environment variables, version management, storage security model',
      },
      {
        title: 'Storage',
        path: '/docs/storage',
        summary:
          'Persistent key-value storage for agents: methods reference, atomic operations, per-user isolation, public storage and its external HTTP read API',
      },
      {
        title: 'VRF',
        path: '/docs/vrf',
        summary:
          'Verifiable randomness: SDK usage, on-chain proof verification, security properties, user-side verification',
      },
      {
        title: 'SDK',
        path: '/docs/sdk',
        summary:
          'The Rust `outlayer` crate: environment module, storage module, minimal project template, API reference',
      },
      {
        title: 'HTTPS API',
        path: '/docs/https-api',
        summary:
          'Request and response format for `POST /call/{project_owner}/{project_name}`: base URLs per network, headers, body, environment variables, error codes',
      },
      {
        title: 'Payment Keys',
        path: '/docs/payment-keys',
        summary:
          'Creating payment keys, key format, restrictions, balance management, rate limits, security practices',
      },
      {
        title: 'Earnings',
        path: '/docs/earnings',
        summary:
          'How developers earn from blockchain and HTTPS calls, checking attached payment inside WASM, viewing and withdrawing balances',
      },
      {
        title: 'Pricing',
        path: '/docs/pricing',
        summary: 'Dynamic pricing model, cost calculation, resource limits, refund policy',
      },
    ],
  },
  {
    title: 'Trust and verification',
    pages: [
      {
        title: 'TEE Attestation',
        path: '/docs/tee-attestation',
        summary:
          'What a worker signature proves: worker registration, post-quantum keys, per-execution attestation, data freshness, verification process',
      },
      {
        title: 'Trust and Verification',
        path: '/docs/trust-verification',
        summary:
          'Verifying the whole stack: five-measurement Intel TDX verification, Sigstore-signed release images, ephemeral keys and their on-chain trail, deterministic keystore secrets',
      },
    ],
  },
  {
    title: 'Machine-readable specs',
    pages: [
      {
        title: 'OpenAPI specification',
        url: 'https://api.outlayer.fastnear.com/openapi.json',
        summary: 'Complete HTTPS API schema — endpoints, request and response types, error codes',
      },
      {
        title: 'Agent Custody skill file',
        url: 'https://skills.outlayer.ai/agent-custody/SKILL.md',
        summary:
          'Canonical agent-framework skill for custody: registration, funding, transfers, swaps, cross-chain deposit and withdraw, NEP-413 signing, sub-agents, error handling',
      },
      {
        title: 'Full documentation text',
        url: 'https://outlayer.fastnear.com/llms-full.txt',
        summary: 'Every document listed here, inlined in a single file',
      },
    ],
  },
  {
    title: 'Optional',
    pages: [
      {
        title: 'Source repository',
        url: 'https://github.com/fastnear/near-outlayer',
        summary: 'Contract, worker, keystore, dashboard, SDK and WASI examples',
      },
      {
        title: 'Worker attestation portal',
        url: 'https://workers.outlayer.ai',
        summary: 'Live Intel TDX attestation status of the production worker fleet',
      },
      {
        title: 'Playground',
        url: 'https://outlayer.fastnear.com/playground',
        summary: 'Run an agent from the browser without writing an integration',
      },
      {
        title: 'Dashboard',
        url: 'https://outlayer.fastnear.com/dashboard',
        summary: 'Projects, secrets, payment keys, executions and earnings',
      },
    ],
  },
]

// Repository markdown inlined into llms-full.txt, in reading order.
//
// Deliberately excluded — these describe operating OutLayer, not building on it:
//   PROJECT.md, SETUP.md, DEPLOYMENT_GUIDE.md, TESTING.md, JOB_BASED_WORKFLOW.md,
//   CHANGELOG_GITHUB_COMPILATION.md, CLAUDE.md, Onepager.md, ORACLE_RFP_PROPOSAL.md,
//   RFP_RESPONSE.md, docker/*, deploy/*, scripts/*.
export const FULL_TEXT_SOURCES = [
  { path: 'README.md', title: 'Overview' },
  { path: 'QUICK_START.md', title: 'Quick Start' },
  { path: 'API.md', title: 'HTTPS API Reference' },
  { path: 'AUTHENTICATION.md', title: 'Authentication and Payment Keys' },
  { path: 'docs/CLI.md', title: 'Command-Line Interface' },

  { path: 'wasi-examples/README.md', title: 'WASI Examples Overview' },
  { path: 'wasi-examples/WASI_TUTORIAL.md', title: 'WASI Development Tutorial' },
  { path: 'wasi-examples/WASM_ENV_VARS.md', title: 'WASM Environment Variables' },
  {
    path: 'wasi-examples/BEST_PRACTICES_OUTLAYER_NEAR.md',
    title: 'Best Practices: OutLayer + NEAR',
  },
  { path: 'wasi-examples/PROXY_CONTRACTS_TUTORIAL.md', title: 'Proxy Contracts Tutorial' },

  { path: 'contract/README.md', title: 'Smart Contract API' },
  { path: 'sdk/outlayer/README.md', title: 'Rust SDK' },

  { path: 'CUSTODY.md', title: 'Agent Custody Reference' },
  { path: 'docs/MULTI_CHAIN.md', title: 'Multi-Chain Custody Wallets' },
  { path: 'docs/DETERMINISTIC_WALLETS.md', title: 'Deterministic Wallets' },
  { path: 'docs/PAYMENT_CHECKS.md', title: 'Payment Checks' },
  { path: 'docs/outlayer-custody-advantages.md', title: 'Custody Design Rationale' },

  { path: 'VAULTS.md', title: 'Sovereign Vaults' },
  { path: 'docs/LEAVING_OUTLAYER.md', title: 'Leaving OutLayer (Sovereign Exit)' },

  { path: 'VRF.md', title: 'Verifiable Random Function' },
  { path: 'WORKER_ATTESTATION.md', title: 'Worker Attestation' },
]
