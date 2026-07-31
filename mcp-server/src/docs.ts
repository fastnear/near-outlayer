// Docs exposed by the MCP server as MCP resources.
//
// Intentionally excluded (not developer/technical docs):
//   - CLAUDE.md                        internal AI-assistant working instructions
//   - Onepager.md                      marketing one-pager
//   - ORACLE_RFP_PROPOSAL.md           business/sales proposal
//   - RFP_RESPONSE.md                  business/sales proposal
//   - CHANGELOG_GITHUB_COMPILATION.md  internal migration changelog
export const DOC_FILES: { path: string; description: string }[] = [
  // Root
  { path: "README.md", description: "Project overview and quick links" },
  { path: "QUICK_START.md", description: "Fastest path to a running OutLayer app" },
  { path: "SETUP.md", description: "Local development environment setup" },
  { path: "API.md", description: "HTTPS API reference" },
  { path: "AUTHENTICATION.md", description: "API authentication and payment keys" },
  { path: "VAULTS.md", description: "MPC vaults: per-customer key custody" },
  { path: "VRF.md", description: "Verifiable Random Function usage" },
  { path: "CUSTODY.md", description: "Agent custody / wallet model" },
  { path: "JOB_BASED_WORKFLOW.md", description: "Job lifecycle for off-chain execution" },
  { path: "PROJECT.md", description: "Full technical spec and implementation status" },
  { path: "TESTING.md", description: "Testing guide" },
  { path: "WORKER_ATTESTATION.md", description: "TEE worker attestation model" },
  { path: "DEPLOYMENT_GUIDE.md", description: "Deploying OutLayer components" },

  // docs/
  { path: "docs/CLI.md", description: "outlayer-cli usage" },
  { path: "docs/DETERMINISTIC_WALLETS.md", description: "Deterministic wallet derivation" },
  { path: "docs/MULTI_CHAIN.md", description: "Multi-chain support" },
  { path: "docs/PAYMENT_CHECKS.md", description: "Gasless agent-to-agent payments" },
  { path: "docs/LEAVING_OUTLAYER.md", description: "Exiting a vault from OutLayer custody" },
  { path: "docs/outlayer-custody-advantages.md", description: "Why OutLayer custody vs. alternatives" },

  // Components
  { path: "contract/README.md", description: "Main NEAR contract API" },
  { path: "worker/README.md", description: "Worker (task polling / WASM execution) config" },
  { path: "worker/PROJECT.md", description: "Worker technical spec" },
  { path: "worker/TESTING.md", description: "Worker testing guide" },
  { path: "worker/src/compiler/README.md", description: "Worker: GitHub repo compiler" },
  { path: "worker/src/executor/README.md", description: "Worker: WASM executor" },
  { path: "worker/examples/README.md", description: "Worker usage examples" },
  { path: "keystore-worker/README.md", description: "Keystore worker (TEE secrets decryption)" },
  { path: "keystore-worker/BALANCE_CHECKS_IMPLEMENTATION.md", description: "Keystore worker balance checks" },
  { path: "keystore-dao-contract/README.md", description: "Keystore governance DAO contract" },
  { path: "register-contract/README.md", description: "TEE worker registration contract" },
  { path: "register-contract/QUICK_START.md", description: "Register contract quick start" },
  { path: "register-contract/DEPLOYMENT.md", description: "Register contract deployment" },
  { path: "register-contract/COLLATERAL_TECHNICAL.md", description: "Register contract collateral mechanics" },
  { path: "vault-contract/README.md", description: "Per-customer vault contract" },
  { path: "vault-contract/GLOBAL-CONTACT.md", description: "Vault contract global-contract deploy" },
  { path: "sdk/outlayer/README.md", description: "Rust SDK" },
  { path: "dashboard/README.md", description: "Dashboard (Next.js UI + docs site) setup" },
  { path: "dashboard/DOCS_INDEX.md", description: "Map of dashboard documentation pages" },
  { path: "dashboard/wasm-verifier/README.md", description: "WASM verifier tool" },
  { path: "dashboard/public/SKILL.md", description: "AI agent skill file (Claude/MCP-compatible agents)" },
  { path: "outlayer-monitor/README.md", description: "Race-attack detector / vault-event forwarder" },
  { path: "deploy/mpc/README.md", description: "MPC deployment notes" },
  { path: "docker/README.md", description: "Docker deployment overview" },
  { path: "docker/BUILD.md", description: "Docker build guide" },
  { path: "docker/DOCKER_RELEASE.md", description: "Docker release process" },
  { path: "docker/TUTORIAL.md", description: "Docker tutorial" },
  { path: "scripts/README.md", description: "Deployment & utility scripts overview" },
  { path: "scripts/customer-recovery/README.md", description: "Customer recovery scripts" },
  { path: "tests/README.md", description: "Integration tests overview" },
  { path: "tests/TESTING.md", description: "Integration testing guide" },
  { path: "tests/WALLET_TESTS.md", description: "Wallet test coverage" },
  { path: "tests/approval-test-comparison.md", description: "Approval test comparison notes" },

  // WASI examples
  { path: "wasi-examples/README.md", description: "WASI examples overview" },
  { path: "wasi-examples/WASI_TUTORIAL.md", description: "WASI container tutorial" },
  { path: "wasi-examples/BEST_PRACTICES_OUTLAYER_NEAR.md", description: "Best practices for OutLayer + NEAR apps" },
  { path: "wasi-examples/PROXY_CONTRACTS_TUTORIAL.md", description: "Proxy contracts tutorial" },
  { path: "wasi-examples/WASM_ENV_VARS.md", description: "WASM environment variables reference" },
];
