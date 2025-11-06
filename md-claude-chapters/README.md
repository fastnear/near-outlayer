# NEAR OutLayer Documentation Chapters

Comprehensive technical documentation for NEAR OutLayer's multi-layer architecture.

---

## Overview

This directory contains focused documentation chapters covering the complete NEAR OutLayer architecture—from infrastructure protection (Phase 1) through Linux/WASM integration (Phase 2) to the strategic multi-layer vision (Phases 3-6).

Each chapter is self-contained yet interconnected, providing both implementation details and strategic context. The chapters are organized by completion status and strategic priority.

---

## Chapter Index

### Phase 1-2: Completed Work

**[Chapter 1: RPC Throttling - Infrastructure Protection](01-rpc-throttling.md)** (Complete)
- **Phase**: 1 (Production Ready)
- **Topics**: Token bucket algorithm, rate limit profiles, automatic retry, coordinator middleware
- **Implementation**: Protected infrastructure from burst traffic with 5 rps (anonymous) / 20 rps (keyed) limits
- **Length**: ~500 lines (Rust coordinator + JavaScript client)

**[Chapter 2: Linux/WASM Integration](02-linux-wasm-integration.md)** (Complete)
- **Phase**: 2 (Demo Mode Functional)
- **Topics**: Three-layer execution model, NEAR syscall mapping (400-499), demo vs production mode
- **Implementation**: POSIX environment in browser via native WASM kernel (not x86 emulation)
- **Architecture**: ContractSimulator → LinuxExecutor → Workers
- **Challenge**: NOMMU (no MMU) architecture requires careful memory management patterns

### Strategic Vision: Phases 3-6

**[Chapter 3: Multi-Layer Roadmap](03-multi-layer-roadmap.md)** (Strategic Plan)
- **Phases**: 3-6 Strategic Plan
- **Timeline**: 11-16 weeks total
- **Topics**:
  - Phase 3: QuickJS integration (2-3 weeks) - JavaScript runtime as POSIX process
  - Phase 4: Frozen Realms/SES (2-3 weeks) - Deterministic, hardened execution environment
  - Phase 5: Production Linux kernel (3-4 weeks) - Replace demo mode with real kernel
  - Phase 6: Applications (4-6 weeks) - Flagship use cases
- **Includes**: Task breakdowns with specific implementation steps, code examples, go/no-go decision points

**[Chapter 6: 4-Layer Architecture Deep Dive](06-4-layer-architecture.md)** (Technical Analysis)
- **Scope**: Complete L1→L2→L3→L4 technical analysis
- **Topics**:
  - L1: Host WASM Runtime (root security boundary)
  - L2: Guest OS (linux-wasm, NOMMU, POSIX API provider)
  - L3: Guest Runtime (QuickJS as POSIX process)
  - L4: Guest Code (Frozen Realm determinism)
- **Key Challenge**: I/O Trombone problem - every I/O traverses 4 layers (~1.6ms per operation vs 0.01ms direct)
- **Competitive Analysis**: Detailed comparison vs Ethereum/Solana/NEAR direct contracts
- **Note**: This is the most complex chapter - explains why multi-layer architecture provides capabilities impossible in other systems

**[Chapter 7: Daring Applications](07-daring-applications.md)** (Market Vision)
- **Scope**: Three flagship applications demonstrating unique capabilities
- **Applications**:
  1. Autonomous AI Trading Agents (TensorFlow.js + deterministic ML) - verifiable inference
  2. Deterministic Plugin Systems (Frozen Realm isolation) - safe DeFi extensibility
  3. Stateful Multi-Process Edge Computing (POSIX FaaS) - full OS at edge with millisecond startup
- **Market Analysis**: Competitive positioning, revenue potential
- **Code**: Complete implementation examples (~1000+ lines)
- **Note**: These applications are not possible on Ethereum, Solana, or standard NEAR contracts

### Reference Documentation

**[Chapter 4: IIFE Bundling](04-iife-bundling.md)** (Reference Architecture)
- **Status**: Not Yet Implemented
- **Topics**: Zero-config browser distribution, tsup configuration, WASM integration strategies
- **Benefit**: Drop-in `<script>` tag usage without bundler (lowers barrier to entry)
- **Implementation Roadmap**: 5 phases (POC → WASM → Linux → CDN → TypeScript)
- **Challenge**: Linux kernel is ~24 MB - too large for main bundle, requires lazy loading strategy

**[Chapter 5: Performance Benchmarking](05-performance-benchmarking.md)** (Methodology)
- **Status**: Framework Complete (Awaiting Production Data)
- **Topics**: Benchmark framework, test scenarios, comparison matrix
- **Key Metrics**: Latency, throughput, memory, overhead
- **Targets**:
  - Direct mode: < 10ms latency, 2-3x overhead vs raw WASM
  - Linux mode: < 50ms cold start, 4-6x overhead vs raw WASM
- **Note**: Performance targets are projections - real measurements await production Linux kernel implementation

---

## Reading Paths

### For Developers (Implementation Focus)

1. **Start**: [Chapter 1: RPC Throttling](01-rpc-throttling.md) - Working Phase 1 code
   - Shows token bucket implementation, coordinator middleware, automatic retry patterns
2. **Next**: [Chapter 2: Linux/WASM Integration](02-linux-wasm-integration.md) - Understand execution modes
   - Critical: NOMMU architecture section explains why linux-wasm differs from traditional Linux
3. **Then**: [Chapter 4: IIFE Bundling](04-iife-bundling.md) - Browser distribution patterns
4. **Finally**: [Chapter 5: Performance Benchmarking](05-performance-benchmarking.md) - Measurement methodology

### For Architects (Strategic Vision)

1. **Start**: [Chapter 6: 4-Layer Architecture](06-4-layer-architecture.md) - Complete stack explanation
   - Most complex chapter - budget 30-45 minutes for careful reading
   - I/O Trombone section explains the fundamental performance trade-off
2. **Next**: [Chapter 3: Multi-Layer Roadmap](03-multi-layer-roadmap.md) - Strategic roadmap
   - Contains go/no-go decision points for each phase
3. **Then**: [Chapter 7: Daring Applications](07-daring-applications.md) - Market opportunity
4. **Finally**: [Chapter 1: RPC Throttling](01-rpc-throttling.md) - Foundation infrastructure

### For Decision Makers (Business Case)

1. **Start**: [Chapter 7: Daring Applications](07-daring-applications.md) - Unique value propositions
   - Each application includes competitive analysis ("Why competitors can't do this")
2. **Next**: [Chapter 6: 4-Layer Architecture](06-4-layer-architecture.md) - Read "Competitive Analysis" section
   - Explains technical moat
3. **Then**: [Chapter 3: Multi-Layer Roadmap](03-multi-layer-roadmap.md) - Phases 3-6 breakdown
   - 11-16 week timeline with specific milestones
4. **Finally**: [Chapter 1](01-rpc-throttling.md) + [Chapter 2](02-linux-wasm-integration.md) - Proven execution

---

## Cross-Chapter Connections

### Layer Dependencies

```
L1: Host WASM Runtime (browser or Wasmtime)
  ↓ Provides security boundary (Chapter 2, 4, 6)
L2: Guest OS (linux-wasm kernel)
  ↓ Provides POSIX API (Chapter 2, 3, 6)
L3: Guest Runtime (QuickJS interpreter)
  ↓ Provides JavaScript execution (Chapter 3, 6)
L4: Guest Code (Frozen Realm)
  ↓ Powers Applications (Chapter 7)
```

Note: Each layer adds overhead but provides new capabilities. L1 is security boundary, L2-L4 are API abstraction layers.

### Infrastructure Stack

```
RPC Throttling (Chapter 1)
  ↓ Protects coordinator from DoS
Coordinator API
  ↓ Serves browser clients
Browser Client (Chapter 4)
  ↓ Chooses execution mode
Direct Mode (Chapter 2) OR Linux Mode (Chapter 2)
  ↓ Measured by
Performance Benchmarks (Chapter 5)
```

### Strategic Timeline

```
Phase 1: RPC Throttling (Chapter 1) - Complete
  ↓
Phase 2: Linux/WASM Integration (Chapter 2) - Complete
  ↓
Phase 3: QuickJS Integration (Chapter 3) - Planned (2-3 weeks)
  ↓ Go/no-go: Can we achieve <300μs startup?
Phase 4: Frozen Realms/SES (Chapter 3) - Planned (2-3 weeks)
  ↓ Go/no-go: Can we eliminate all non-determinism?
Phase 5: Production Linux Kernel (Chapter 3) - Planned (3-4 weeks)
  ↓ Go/no-go: Can we achieve <50ms cold start?
Phase 6: Applications (Chapter 7) - Planned (4-6 weeks)
```

---

## Documentation Metrics

| Chapter | Lines | Status | Phase | Complexity |
|---------|-------|--------|-------|------------|
| 01: RPC Throttling | ~500 | Complete | 1 | Low |
| 02: Linux/WASM Integration | ~800 | Complete | 2 | High |
| 03: Multi-Layer Roadmap | ~1200 | Strategic | 3-6 | Medium |
| 04: IIFE Bundling | ~600 | Reference | Future | Low |
| 05: Performance Benchmarking | ~700 | Framework | Ongoing | Medium |
| 06: 4-Layer Architecture | ~1500 | Analysis | 3-6 | Very High |
| 07: Daring Applications | ~1000+ | Vision | 6 | High |

**Total**: ~6,300+ lines of technical documentation

---

## Chapter Format

Each chapter follows a consistent structure:

1. **Overview**: Phase, status, what was achieved or is planned
2. **Architecture**: System design, data flows, critical components
3. **Implementation**: Code snippets, configuration, specific techniques
4. **Testing/Validation**: Results, metrics, what was proven
5. **Strategic Context**: Why this matters, competitive analysis, next steps
6. **Related Documentation**: Cross-chapter links

Chapters prioritize clarity over brevity. Where concepts are subtle (NOMMU, I/O Trombone, Frozen Realms), extra explanation is provided.

---

## Key Concepts Reference

### NEAR OutLayer Unique Capabilities

- **Native WASM Linux** (not x86 emulation): [Chapter 2](02-linux-wasm-integration.md), [Chapter 6](06-4-layer-architecture.md)
  - Critical distinction: linux-wasm compiles Linux kernel to WASM (not running x86 code in emulator)
- **NOMMU Architecture**: [Chapter 2](02-linux-wasm-integration.md), [Chapter 6](06-4-layer-architecture.md)
  - Explains why fork() doesn't work, must use vfork() or posix_spawn()
- **NEAR Syscall Integration**: [Chapter 2](02-linux-wasm-integration.md) (syscall range 400-499)
  - Maps NEAR blockchain functions to Linux syscalls
- **Frozen Realms**: [Chapter 3](03-multi-layer-roadmap.md), [Chapter 6](06-4-layer-architecture.md), [Chapter 7](07-daring-applications.md)
  - Deterministic JavaScript environment (no Date.now, Math.random, fetch)
- **QuickJS Integration**: [Chapter 3](03-multi-layer-roadmap.md), [Chapter 6](06-4-layer-architecture.md)
  - Small JS engine (~2 MB binary, <300μs startup, no JIT for determinism)
- **RPC Throttling**: [Chapter 1](01-rpc-throttling.md)
  - Token bucket algorithm with per-route, per-auth-level bucketing
- **I/O Trombone Problem**: [Chapter 6](06-4-layer-architecture.md)
  - Explains fundamental performance trade-off of multi-layer architecture

### Performance Characteristics

- **Direct Mode**: 2-3x overhead, <10ms latency - [Chapter 5](05-performance-benchmarking.md)
  - Suitable for simple contracts, minimal syscall needs
- **Linux Mode (Production)**: 4-6x overhead, <50ms cold start - [Chapter 5](05-performance-benchmarking.md)
  - Required for complex applications, full POSIX support
- **RPC Throttling**: ~5ms overhead - [Chapter 1](01-rpc-throttling.md)
  - Negligible impact on user experience
- **Multi-Layer Stack**: 10-50ms latency - [Chapter 6](06-4-layer-architecture.md)
  - Still faster than full VM solutions (100-500ms)

### Strategic Milestones

- **Phase 1 (Complete)**: [Chapter 1](01-rpc-throttling.md) - Infrastructure protection
- **Phase 2 (Complete)**: [Chapter 2](02-linux-wasm-integration.md) - Linux/WASM integration
- **Phases 3-6 (Planned)**: [Chapter 3](03-multi-layer-roadmap.md) - Multi-layer stack
- **Applications (Vision)**: [Chapter 7](07-daring-applications.md) - Flagship use cases

---

## Quick Start

### New to OutLayer?

**Start with**: [Chapter 1: RPC Throttling](01-rpc-throttling.md)

This chapter shows working production code for Phase 1. Reading it helps you understand:
- Coordinator architecture and API design
- Token bucket rate limiting implementation
- How browser clients integrate with coordinator
- Testing methodology for concurrent requests

### Want to Understand the Vision?

**Start with**: [Chapter 7: Daring Applications](07-daring-applications.md)

This chapter shows three flagship applications that are not possible on other blockchain platforms:
- Each application includes complete implementation
- Competitive analysis explains why alternatives can't provide these capabilities
- Market opportunity section sizes revenue potential

### Need Technical Deep Dive?

**Start with**: [Chapter 6: 4-Layer Architecture](06-4-layer-architecture.md)

Warning: This is the most complex chapter. It explains:
- Complete L1→L4 stack (each layer's role and overhead)
- Security model (which layers provide security vs API abstraction)
- I/O Trombone problem (fundamental performance trade-off)
- Competitive positioning (detailed comparison table)

Budget 30-45 minutes for careful reading. The I/O Trombone section is particularly important for understanding why multi-layer architecture makes sense despite performance overhead.

---

## Contributing

These chapters are living documents. When implementing features:

1. **Update relevant chapters** with actual results (especially benchmarks)
2. **Add cross-references** when new chapters are created
3. **Maintain consistent format** (see Chapter Format above)
4. **Include code snippets** from working implementations (not pseudocode)
5. **Explain trade-offs** - document what didn't work and why

When concepts are subtle or counterintuitive, add explanation rather than removing detail.

---

## External References

### Full Documentation

- **Main README**: `../README.md` - Quick start guide
- **CLAUDE.md**: `../CLAUDE.md` - Capability-based architecture principles
- **PROJECT.md**: `../PROJECT.md` - Complete technical specification

### Component Documentation

- **Contract**: `../contract/README.md` - Smart contract API reference
- **Coordinator**: `../coordinator/README.md` - API server documentation
- **Worker**: `../worker/README.md` - Execution worker setup
- **Browser Worker**: `../browser-worker/README.md` - Browser client integration

### Research Documents

- **IIFE Bundling**: `../browser-worker/docs/IIFE_BUNDLING_REFERENCE.md`
- **Performance Benchmarking**: `../browser-worker/docs/PERFORMANCE_BENCHMARKING.md`

---

## Last Updated

**Date**: 2025-11-05
**Version**: v1.0
**Status**: All chapters complete for Phases 1-2 plus strategic vision (Phases 3-6)

---

## Acknowledgments

These chapters synthesize work from:
- Phase 1: RPC Throttling completion
- Phase 2: Linux/WASM integration completion
- Research: Multi-layer virtualization analysis
- Vision: Application exploration
- Reference: fastnear-js-monorepo patterns, QuickJS/SES/Frozen Realms research

---

Pick your reading path above based on your role and interests. Each chapter includes cross-references to related content.
