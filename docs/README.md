# NEAR OutLayer Documentation Index

Welcome to the NEAR OutLayer documentation. This directory contains all historical documentation, architecture deep-dives, and completed phase reports.

## 📚 Quick Navigation

### Getting Started (Project Root)
- [`../README.md`](../README.md) - Project overview and quick start
- [`../QUICK_START.md`](../QUICK_START.md) - Getting started guide
- [`../SETUP.md`](../SETUP.md) - Detailed setup instructions
- [`../CLAUDE.md`](../CLAUDE.md) - AI assistant development guide

### Project Vision & Quality Gates
- [`PROJECT_VISION.md`](PROJECT_VISION.md) - Executive summary, positioning, and strategic vision
- [`DoD-Verification-Tests.md`](DoD-Verification-Tests.md) - ✅ Definition of Done (82/82 tests, machine-verified)
- [`RELEASE_CHECKLIST.md`](RELEASE_CHECKLIST.md) - ✅ Release gate criteria (all checks passed)

---

## 🏗️ Architecture Documentation

Deep-dive technical documentation on system architecture:

- [`architecture/NEAR_RUNTIME_ARCHITECTURE.md`](architecture/NEAR_RUNTIME_ARCHITECTURE.md) - NEAR Protocol runtime integration
- [`architecture/BROWSER_TEE_IMPLEMENTATION_ROADMAP.md`](architecture/BROWSER_TEE_IMPLEMENTATION_ROADMAP.md) - Browser-based TEE roadmap
- [`architecture/WASM_REPL_INTEGRATION.md`](architecture/WASM_REPL_INTEGRATION.md) - WASM REPL integration details
- [`architecture/JOB_BASED_WORKFLOW.md`](architecture/JOB_BASED_WORKFLOW.md) - Job coordination architecture
- [`architecture/RUNTIME_EXPLORATION_INDEX.md`](architecture/RUNTIME_EXPLORATION_INDEX.md) - Runtime exploration index
- [`architecture/RUNTIME_EXPLORATION_SUMMARY.md`](architecture/RUNTIME_EXPLORATION_SUMMARY.md) - Runtime exploration summary

---

## ✅ Release Documentation

Historical documentation of completed releases and milestones:

### RPC Throttling & Infrastructure
- [`releases/PHASE_1_RPC_THROTTLING_COMPLETE.md`](releases/PHASE_1_RPC_THROTTLING_COMPLETE.md)
  - Token bucket algorithm implementation
  - Rate limit profiles (5 rps anonymous / 20 rps keyed)
  - Production-ready coordinator middleware

### Linux/WASM Integration
- [`releases/PHASE_2_LINUX_WASM_COMPLETE.md`](releases/PHASE_2_LINUX_WASM_COMPLETE.md)
  - Three-layer execution model (ContractSimulator → LinuxExecutor → Workers)
  - Demo mode with NEAR syscall mapping (400-499)
  - Native WASM kernel (not x86 emulation)

### File Organization
- [`releases/PHASE_4_FILES_INDEX.md`](releases/PHASE_4_FILES_INDEX.md)
  - Codebase organization and file structure

### Verification & Testing
- [`releases/VERIFICATION_TESTS_COMPLETE.md`](releases/VERIFICATION_TESTS_COMPLETE.md)
  - Comprehensive test suite completion (82/82 passing)
  - Machine-verifiable proof system (12/12 checks)
  - Deterministic execution verification

### TypeScript Client
- [`releases/TYPESCRIPT_AND_TESTS_COMPLETE.md`](releases/TYPESCRIPT_AND_TESTS_COMPLETE.md)
  - TypeScript client library implementation

### Hermes Enclave (TEE Implementation)
- [`releases/HERMES_ENCLAVE_PHASE_1_COMPLETE.md`](releases/HERMES_ENCLAVE_PHASE_1_COMPLETE.md)
  - Initial Hermes Enclave integration
- [`releases/HERMES_ENCLAVE_PHASE_1_PRODUCTION_READY.md`](releases/HERMES_ENCLAVE_PHASE_1_PRODUCTION_READY.md)
  - Production-ready Hermes Enclave
- [`releases/PRODUCTION_REFINEMENT_SUMMARY.md`](releases/PRODUCTION_REFINEMENT_SUMMARY.md)
  - Production refinement summary

---

## 📖 Operational Guides

Step-by-step guides for deployment and operation:

- [`guides/AUTHENTICATION.md`](guides/AUTHENTICATION.md) - Authentication system documentation
- [`guides/DEPLOYMENT_GUIDE.md`](guides/DEPLOYMENT_GUIDE.md) - Production deployment guide

---

## 📝 Proposals & Planning

Strategic proposals and planning documents:

- [`proposals/ORACLE_RFP_PROPOSAL.md`](proposals/ORACLE_RFP_PROPOSAL.md) - Oracle RFP proposal
- [`proposals/Onepager.md`](proposals/Onepager.md) - Project one-pager

---

## 📦 Archive

Historical or superseded documentation:

- [`archive/CHANGELOG_GITHUB_COMPILATION.md`](archive/CHANGELOG_GITHUB_COMPILATION.md) - GitHub compilation changelog

---

## 🔬 Research & Explorations

### Research Directory (`../research/`)

Experimental and exploratory work extending beyond OutLayer's immediate production scope:

- [`../research/README.md`](../research/README.md) - Research directory overview and scope
- [`../research/nearcore-conformance/`](../research/nearcore-conformance/) - NEAR Protocol conformance explorations
  - Primitives bindings (AccountId, Receipt → Action conversion)
  - Fee parity tests (RuntimeConfig comparison skeleton)
  - Borsh ABI prototypes (alternative to JSON for WASI plugins)

**Note**: Research code is **experimental** and provides scaffolding for future integration work.

---

## 🎯 Strategic Documentation (md-claude-chapters/)

The `../md-claude-chapters/` directory contains forward-looking strategic documentation and deep technical analysis:

- **Chapter 1**: RPC Throttling - Infrastructure Protection (Complete)
- **Chapter 2**: Linux/WASM Integration (Complete)
- **Chapter 3**: Multi-Layer Roadmap (Strategic Plan, Phases 3-6)
- **Chapter 4**: IIFE Bundling (Reference)
- **Chapter 5**: Performance Benchmarking (Methodology)
- **Chapter 6**: 4-Layer Architecture Deep Dive (Technical Analysis)
- **Chapter 7**: Daring Applications (Market Vision)

See [`../md-claude-chapters/README.md`](../md-claude-chapters/README.md) for detailed index and reading paths.

---

## 📊 Current Test Documentation

Active test suite documentation lives in the test directories:

- `../tests/verification-tests/TEST_REPORT.md` - Verification test report (82/82 passing)
- `../tests/verification-tests/README.md` - Verification test suite overview
- `../outlayer-verification-suite/README.md` - Property-based testing suite

---

## 🗂️ Documentation Organization Philosophy

### Root Level (4 files)
Keep only essential files developers/users need immediately:
- README.md - First impression, quick orientation
- QUICK_START.md - Get running fast
- SETUP.md - Detailed setup
- CLAUDE.md - AI assistant instructions (development tool)

### docs/ Directory
Everything else goes here, organized by purpose:
- **architecture/** - Deep technical documentation
- **releases/** - Historical release completion reports
- **guides/** - Operational how-tos
- **proposals/** - Strategic planning
- **archive/** - Superseded documentation

This keeps the project root clean while preserving all historical context and technical depth in organized subdirectories.

---

**Last Updated**: 2025-11-05
**Documentation Structure Version**: 2.0 (Post-cleanup)
