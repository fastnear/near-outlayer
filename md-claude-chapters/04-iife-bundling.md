# Chapter 4: IIFE Bundling - Zero-Config Browser Distribution

**Phase**: Reference Architecture (Not Yet Implemented)
**Status**: Documented, Ready for Implementation

---

## Overview

IIFE (Immediately Invoked Function Expression) bundling enables **zero-build-step browser distribution** for NEAR OutLayer. Users can include a single `<script>` tag and immediately access the full ContractSimulator API without any bundler, transpiler, or Node.js tooling.

### The Problem

Modern JavaScript libraries typically require:
- npm package installation
- Bundler setup (Webpack, Rollup, Vite)
- Build configuration
- Development server

This creates a **high barrier to entry** for quick prototyping and educational use cases.

### The Solution

IIFE bundles provide **drop-in browser usage**:

```html
<!-- Single script tag -->
<script src="https://cdn.outlayer.near/near-outlayer.iife.js"></script>

<script>
  // Library immediately available as global
  const simulator = new OutLayer.ContractSimulator();
  await simulator.execute('counter.wasm', 'increment', {});
</script>
```

**No build step. No configuration. Works everywhere.**

---

## IIFE Pattern Architecture

### Basic Structure

From fastnear-js-monorepo exploration, the canonical IIFE pattern:

```javascript
var OutLayer = (() => {
  "use strict";

  // 1. Module system helpers
  var __exports = {};
  var __module = { exports: __exports };

  // 2. Dependency resolution
  function __require(name) {
    if (name === 'near-api-js') {
      return window.nearApi;  // Expect peer dependency
    }
    throw new Error(`Module not found: ${name}`);
  }

  // 3. Transpiled module code
  (function(exports, require, module) {
    // Original: export class ContractSimulator {}
    // Transpiled:
    exports.ContractSimulator = class ContractSimulator {
      constructor(options = {}) {
        this.executionMode = options.executionMode || 'direct';
      }

      async execute(wasmSource, methodName, args) {
        // ... implementation
      }
    };
  })(__exports, __require, __module);

  // 4. Return public API
  return __module.exports;
})();
```

### Key Components

**1. Namespace Isolation**
```javascript
var OutLayer = (() => {
  // Everything inside is private scope
  // Only `OutLayer` is exposed to global scope
})();
```

**Benefits**:
- Prevents global scope pollution
- Avoids naming conflicts with other libraries
- Clean single-variable API surface

**2. CommonJS Compatibility Layer**
```javascript
var __exports = {};
var __module = { exports: __exports };

function __require(name) {
  const globals = {
    'near-api-js': window.nearApi,
    'bn.js': window.BN,
  };

  if (globals[name]) return globals[name];
  throw new Error(`Peer dependency not found: ${name}`);
}
```

**Purpose**: Maps Node.js module system to browser globals.

**3. Helper Functions**

tsup auto-generates helper functions for module interop:

```javascript
// Define property helper
var __defProp = Object.defineProperty;

// Export helper (lazy evaluation)
var __export = (target, all) => {
  for (var name in all)
    __defProp(target, name, {
      get: all[name],
      enumerable: true
    });
};

// Copy properties helper (for re-exports)
var __copyProps = (to, from, except) => {
  if (from && typeof from === "object" || typeof from === "function") {
    for (let key of Object.getOwnPropertyNames(from))
      if (!Object.hasOwnProperty.call(to, key) && key !== except)
        __defProp(to, key, {
          get: () => from[key],
          enumerable: true
        });
  }
  return to;
};
```

---

## Build Configuration with tsup

### Minimal Development Build

**File**: `browser-worker/tsup.config.ts`

```typescript
import { defineConfig } from 'tsup';

export default defineConfig({
  entry: ['src/index.ts'],
  format: ['iife'],
  outDir: 'dist',
  globalName: 'OutLayer',
  platform: 'browser',
  target: 'es2020',

  // Development settings
  sourcemap: true,
  clean: true,
  minify: false,  // Readable for debugging

  // External large dependencies
  external: [
    'near-api-js',  // User provides via CDN
  ],
});
```

**Build command**:
```bash
npm install -D tsup
npx tsup
# Output: dist/index.global.js (~50 KB unminified)
```

**Browser usage**:
```html
<!-- Load peer dependencies first -->
<script src="https://cdn.jsdelivr.net/npm/near-api-js@2.1.0/dist/near-api-js.min.js"></script>

<!-- Load OutLayer -->
<script src="dist/index.global.js"></script>

<script>
  console.log('OutLayer ready:', new OutLayer.ContractSimulator());
</script>
```

### Production Build with Optimization

```typescript
import { defineConfig } from 'tsup';

export default defineConfig({
  entry: ['src/index.ts'],
  format: ['iife'],
  outDir: 'dist',
  globalName: 'OutLayer',
  platform: 'browser',
  target: 'es2020',

  // Production settings
  sourcemap: true,        // Keep source maps for debugging
  clean: true,
  minify: true,           // Terser minification
  treeshake: true,        // Remove unused code
  splitting: false,       // Single file output

  // Banner with version info
  banner: {
    js: `/* OutLayer v${require('./package.json').version} | MIT License */`,
  },

  external: ['near-api-js'],
});
```

**Bundle size results**:
```
near-outlayer.iife.js (unminified):   ~200 KB
near-outlayer.iife.js (minified):     ~80 KB
near-outlayer.iife.js (gzipped):      ~25 KB
```

### Multi-Format Build (CJS + ESM + IIFE)

Generate all module formats in a single build:

```typescript
import { defineConfig } from 'tsup';

export default defineConfig({
  entry: ['src/index.ts'],
  format: ['cjs', 'esm', 'iife'],
  outDir: 'dist',
  globalName: 'OutLayer',  // IIFE only
  platform: 'neutral',
  target: ['es2020', 'node16'],
  dts: true,               // Generate TypeScript definitions
  sourcemap: true,
  clean: true,
  minify: true,
  treeshake: true,
  external: ['near-api-js'],
});
```

**Outputs**:
- `dist/index.js` - CommonJS (Node.js, older bundlers)
- `dist/index.mjs` - ESM (modern bundlers, Node.js)
- `dist/index.global.js` - IIFE (browsers, no bundler)
- `dist/index.d.ts` - TypeScript definitions

**package.json configuration**:
```json
{
  "name": "near-outlayer",
  "version": "1.0.0",
  "main": "./dist/index.js",
  "module": "./dist/index.mjs",
  "browser": "./dist/index.global.js",
  "types": "./dist/index.d.ts",
  "exports": {
    ".": {
      "import": "./dist/index.mjs",
      "require": "./dist/index.js",
      "browser": "./dist/index.global.js",
      "types": "./dist/index.d.ts"
    }
  }
}
```

---

## WASM Integration Strategies

### Strategy 1: Dynamic WASM Loading (Recommended)

**Concept**: Load WASM contracts on-demand from CDN or user URL.

```javascript
var OutLayer = (() => {
  // WASM cache to avoid re-fetching
  const wasmCache = new Map();

  class ContractSimulator {
    async loadContract(wasmSource) {
      // Check cache first
      if (wasmCache.has(wasmSource)) {
        return wasmCache.get(wasmSource);
      }

      // Fetch and cache
      const response = await fetch(wasmSource);
      if (!response.ok) {
        throw new Error(`Failed to load WASM: ${response.status}`);
      }

      const bytes = new Uint8Array(await response.arrayBuffer());
      wasmCache.set(wasmSource, bytes);
      return bytes;
    }

    // Preload common contracts
    static async preload(urls) {
      const sim = new ContractSimulator();
      await Promise.all(urls.map(url => sim.loadContract(url)));
    }
  }

  return { ContractSimulator };
})();
```

**Usage**:
```html
<script src="near-outlayer.iife.js"></script>
<script>
  // Preload contracts during page load
  OutLayer.ContractSimulator.preload([
    'https://cdn.outlayer.near/contracts/counter.wasm',
    'https://cdn.outlayer.near/contracts/nft.wasm',
  ]);

  // Execute when needed
  const sim = new OutLayer.ContractSimulator();
  await sim.execute(
    'https://cdn.outlayer.near/contracts/counter.wasm',
    'increment',
    {}
  );
</script>
```

**Trade-offs**:
- ✅ Small bundle size (~25 KB gzipped)
- ✅ Flexible contract sources (CDN, local, data URLs)
- ✅ Caching reduces redundant fetches
- ❌ Network dependency
- ❌ Latency on first load (~100-500ms per contract)

### Strategy 2: Embedded Data URLs

**Concept**: Embed small WASM files directly as base64 data URLs.

```javascript
var OutLayer = (() => {
  // Embed WASM as base64 data URL
  const COUNTER_WASM = 'data:application/wasm;base64,AGFzbQEAAAAB...';

  class ContractSimulator {
    async loadContract(wasmSource) {
      if (wasmSource.startsWith('data:')) {
        // Decode base64 data URL
        const base64 = wasmSource.split(',')[1];
        const binary = atob(base64);
        const bytes = new Uint8Array(binary.length);
        for (let i = 0; i < binary.length; i++) {
          bytes[i] = binary.charCodeAt(i);
        }
        return bytes;
      }

      // Otherwise fetch from URL
      const response = await fetch(wasmSource);
      return new Uint8Array(await response.arrayBuffer());
    }
  }

  return { ContractSimulator, COUNTER_WASM };
})();
```

**Usage**:
```html
<script src="near-outlayer.iife.js"></script>
<script>
  // Use embedded contract (no network request)
  const sim = new OutLayer.ContractSimulator();
  await sim.execute(OutLayer.COUNTER_WASM, 'increment', {});
</script>
```

**Trade-offs**:
- ✅ No network requests
- ✅ Works offline
- ✅ Instant availability
- ❌ Large bundle size (WASM is binary-heavy)
- ❌ Not suitable for multiple contracts

**Size impact**:
```
Base IIFE bundle:           ~25 KB (gzipped)
+ counter.wasm (20 KB):     ~40 KB (gzipped)
+ nft.wasm (150 KB):        ~180 KB (gzipped)
```

### Strategy 3: Virtual Filesystem (Hybrid)

**Concept**: Bundle frequently-used contracts in VFS, allow dynamic loading for others.

```javascript
var OutLayer = (() => {
  // Virtual filesystem
  const vfs = {
    '/contracts/counter.wasm': new Uint8Array([/* embedded bytes */]),
    '/contracts/nft.wasm': new Uint8Array([/* embedded bytes */]),
  };

  class ContractSimulator {
    async loadContract(wasmSource) {
      // Check if virtual path
      if (wasmSource.startsWith('/contracts/')) {
        const bytes = vfs[wasmSource];
        if (!bytes) {
          throw new Error(`Contract not found in VFS: ${wasmSource}`);
        }
        return bytes;
      }

      // Otherwise fetch from network
      const response = await fetch(wasmSource);
      return new Uint8Array(await response.arrayBuffer());
    }

    // List available contracts
    static listContracts() {
      return Object.keys(vfs);
    }

    // Add contract at runtime
    static addContract(path, bytes) {
      vfs[path] = bytes;
    }
  }

  return { ContractSimulator };
})();
```

**Usage**:
```html
<script src="near-outlayer.iife.js"></script>
<script>
  // Use bundled contract (instant)
  const sim = new OutLayer.ContractSimulator();
  await sim.execute('/contracts/counter.wasm', 'increment', {});

  // Or add custom contract at runtime
  const customWasm = await fetch('/my-contract.wasm')
    .then(r => r.arrayBuffer())
    .then(b => new Uint8Array(b));

  OutLayer.ContractSimulator.addContract('/contracts/custom.wasm', customWasm);
  await sim.execute('/contracts/custom.wasm', 'myMethod', {});
</script>
```

**Trade-offs**:
- ✅ Clean API (path-based)
- ✅ Mix bundled + dynamic contracts
- ✅ Extensible at runtime
- ⚠️ Medium bundle size (depends on contracts included)

---

## Module System Integration

### Handling External Dependencies

**Strategy 1: Peer Dependencies**

Externalize large dependencies and expect user to provide via CDN:

```typescript
// tsup.config.ts
export default defineConfig({
  format: ['iife'],
  external: [
    'near-api-js',  // 500+ KB, let user provide
  ],
});
```

```javascript
// Generated IIFE with peer dependency resolution
function __require(name) {
  const globals = {
    'near-api-js': window.nearApi,
    'bn.js': window.BN,
  };

  if (globals[name]) return globals[name];
  throw new Error(`Peer dependency not found: ${name}. Please include it via <script> tag.`);
}
```

**Usage**:
```html
<!-- User provides peer dependencies -->
<script src="https://cdn.jsdelivr.net/npm/near-api-js@2.1.0/dist/near-api-js.min.js"></script>
<script src="https://cdn.jsdelivr.net/npm/bn.js@5.2.0/dist/bn.min.js"></script>

<!-- Then load OutLayer -->
<script src="near-outlayer.iife.js"></script>
```

**Strategy 2: Bundle Dependencies**

For small utilities, bundle them to reduce user setup:

```typescript
// tsup.config.ts
export default defineConfig({
  format: ['iife'],
  external: ['near-api-js'],   // Large: externalize
  noExternal: ['buffer'],       // Small: bundle
});
```

**Trade-off matrix**:
| Dependency | Size | Strategy | Rationale |
|------------|------|----------|-----------|
| near-api-js | 500 KB | External | Too large, user likely already has it |
| bn.js | 80 KB | External | Common peer dependency |
| buffer | 50 KB | Bundle | Polyfill, convenient to include |
| tiny utils | <10 KB | Bundle | Not worth external loading |

### Polyfills for Node.js APIs

Many WASM tools expect Node.js APIs. Provide browser polyfills:

```javascript
// In IIFE preamble (before main code)
(function setupPolyfills() {
  // Buffer polyfill
  if (typeof Buffer === 'undefined') {
    window.Buffer = {
      from: (data, encoding) => {
        if (typeof data === 'string') {
          return new TextEncoder().encode(data);
        }
        return new Uint8Array(data);
      },
      isBuffer: (obj) => obj instanceof Uint8Array,
      alloc: (size) => new Uint8Array(size),
    };
  }

  // process.env polyfill
  if (typeof process === 'undefined') {
    window.process = {
      env: {},
      version: 'browser',
      platform: 'browser',
    };
  }

  // TextEncoder/TextDecoder (IE11 compatibility)
  if (typeof TextEncoder === 'undefined') {
    // Import text-encoding polyfill
    console.warn('TextEncoder not available. Please include polyfill.');
  }
})();
```

---

## Browser Compatibility

### Target Environments

**Modern Browsers** (es2020 target):
```typescript
// Can use:
// - async/await
// - BigInt
// - Optional chaining (?.)
// - Nullish coalescing (??)
// - Dynamic import()
// - WebAssembly

export default defineConfig({
  target: 'es2020',
});
```

**Browser support**: Chrome 80+, Firefox 74+, Safari 13.1+, Edge 80+

**Legacy Support** (es2015 target):
```typescript
// Transpiles to ES5 + polyfills
// - No async/await (uses regenerator-runtime)
// - No BigInt (needs polyfill or limitation)
// - No optional chaining (transpiled)

export default defineConfig({
  target: 'es2015',
  inject: ['./polyfills.ts'],  // Inject polyfills
});
```

**Browser support**: IE11+, all modern browsers

### Feature Detection

Add runtime checks for critical features:

```javascript
var OutLayer = (() => {
  // Feature detection
  const features = {
    wasm: typeof WebAssembly !== 'undefined',
    bigint: typeof BigInt !== 'undefined',
    sharedArrayBuffer: typeof SharedArrayBuffer !== 'undefined',
    worker: typeof Worker !== 'undefined',
  };

  class ContractSimulator {
    constructor(options = {}) {
      // Check required features
      if (!features.wasm) {
        throw new Error('WebAssembly not supported in this browser');
      }

      // Warn about optional features
      if (!features.sharedArrayBuffer && options.executionMode === 'linux') {
        console.warn('SharedArrayBuffer not available. Linux mode requires COOP/COEP headers.');
      }

      this.features = features;
    }

    static checkCompatibility() {
      return {
        compatible: features.wasm,
        features,
        recommendations: [
          !features.bigint && 'Update browser for BigInt support',
          !features.sharedArrayBuffer && 'Enable COOP/COEP headers for Linux mode',
        ].filter(Boolean),
      };
    }
  }

  return { ContractSimulator };
})();
```

**Usage**:
```html
<script src="near-outlayer.iife.js"></script>
<script>
  const compat = OutLayer.ContractSimulator.checkCompatibility();

  if (!compat.compatible) {
    alert('Your browser does not support WebAssembly. Please upgrade.');
  } else {
    console.log('Browser features:', compat.features);
    if (compat.recommendations.length > 0) {
      console.warn('Recommendations:', compat.recommendations);
    }
  }
</script>
```

---

## Performance Considerations

### Bundle Size Analysis

**Typical sizes** (for OutLayer IIFE):
```
Core library (unminified):          ~200 KB
Core library (minified):            ~80 KB
Core library (gzipped):             ~25 KB

With embedded contracts:
  + counter.wasm (20 KB):           ~100 KB (gzipped)
  + nft.wasm (150 KB):              ~180 KB (gzipped)
```

**Size optimization strategies**:

1. **Tree-shaking**: Remove unused exports
```typescript
export default defineConfig({
  treeshake: true,  // tsup enables by default
});
```

2. **Lazy loading**: Load heavy features on-demand
```javascript
class ContractSimulator {
  async enableLinuxMode() {
    if (!this.linuxExecutor) {
      // Dynamically load Linux runtime
      await this.loadScript('near-outlayer.linux.iife.js');
      this.linuxExecutor = new OutLayer.LinuxExecutor();
    }
    return this.linuxExecutor;
  }

  loadScript(url) {
    return new Promise((resolve, reject) => {
      const script = document.createElement('script');
      script.src = url;
      script.onload = resolve;
      script.onerror = reject;
      document.head.appendChild(script);
    });
  }
}
```

3. **Code splitting** (multi-file IIFE):
```
near-outlayer.core.iife.js (~25 KB)     - Core + direct mode
near-outlayer.linux.iife.js (~500 KB)   - Linux executor
linux-worker.iife.js (~100 KB)          - Worker bundle
```

### Loading Performance

**Benchmark** (typical 3G connection):
```
Download near-outlayer.iife.js (25 KB gzip):  ~500 ms
Parse + execute JavaScript:                   ~100 ms
Initialize ContractSimulator:                 ~50 ms
-----------------------------------------------------------
Total time to interactive:                    ~650 ms
```

**Optimization: Preload**
```html
<!-- Start downloading early -->
<link rel="preload" href="near-outlayer.iife.js" as="script">

<!-- Later in <body> -->
<script src="near-outlayer.iife.js"></script>
```

**Optimization: Async Loading**
```html
<!-- Non-blocking load -->
<script async src="near-outlayer.iife.js" onload="initOutLayer()"></script>

<script>
  function initOutLayer() {
    window.simulator = new OutLayer.ContractSimulator();
    console.log('OutLayer ready');
  }
</script>
```

### Runtime Performance

**Direct WASM Execution**:
```
Contract instantiation:  ~5 ms
Method call overhead:    ~0.1 ms
Execution time:          varies by contract
```

**Memory Usage**:
```
ContractSimulator instance:  ~1 MB
WASM contract loaded:        ~size of .wasm file
Linux executor (demo):       ~2 MB
Linux executor (prod):       ~30 MB (kernel + workers)
```

---

## Implementation Roadmap

### Phase 1: Proof-of-Concept (2-3 days)

**Goal**: Create working IIFE bundle for ContractSimulator

**Tasks**:
1. Setup tsup configuration in `browser-worker/`
2. Convert TypeScript to IIFE output
3. Test in browser without bundler
4. Measure bundle size

**Success criteria**:
```html
<script src="near-outlayer.iife.js"></script>
<script>
  const sim = new OutLayer.ContractSimulator();
  await sim.execute('counter.wasm', 'increment', {});
  // Works without any build step on user's side
</script>
```

### Phase 2: WASM Integration (3-4 days)

**Goal**: Seamless WASM loading in IIFE environment

**Tasks**:
1. Implement virtual filesystem for contracts
2. Add dynamic WASM loading with caching
3. Error handling for network failures
4. Preload API for common contracts

**Success criteria**:
- VFS paths: `/contracts/counter.wasm` loads instantly
- Dynamic URLs: `https://...` fetches and caches
- Preload API reduces latency

### Phase 3: Linux Mode Support (1 week)

**Goal**: Enable Linux execution mode in IIFE

**Challenges**:
- Linux kernel is ~24 MB (too large for main bundle)
- SharedArrayBuffer requires COOP/COEP headers
- Worker scripts need separate IIFE bundles

**Approach**:
```javascript
class ContractSimulator {
  async setExecutionMode(mode) {
    if (mode === 'linux' && !this.linuxExecutor) {
      // Dynamically load Linux bundle
      await this.loadScript('near-outlayer.linux.iife.js');
      this.linuxExecutor = new OutLayer.LinuxExecutor();
    }
    this.executionMode = mode;
  }
}
```

### Phase 4: CDN Distribution (2-3 days)

**Goal**: Production-ready CDN hosting

**Tasks**:
1. Setup CDN (Cloudflare, jsDelivr, or NEAR CDN)
2. Versioned releases with SRI hashes
3. Documentation for CDN usage

**Example**:
```html
<!-- Load from CDN with SRI -->
<script
  src="https://cdn.outlayer.near/v1.0.0/near-outlayer.iife.min.js"
  integrity="sha384-..."
  crossorigin="anonymous"
></script>
```

### Phase 5: TypeScript Definitions (1 day)

**Goal**: Full TypeScript support for IIFE users

**Solution**: Augment global namespace
```typescript
// near-outlayer.d.ts (published to DefinitelyTyped)
declare namespace OutLayer {
  class ContractSimulator {
    constructor(options?: ContractSimulatorOptions);
    execute(
      wasmSource: string | Uint8Array,
      methodName: string,
      args?: Record<string, any>,
      context?: ExecutionContext
    ): Promise<ExecutionResult>;
  }

  interface ContractSimulatorOptions {
    executionMode?: 'direct' | 'linux';
    verboseLogging?: boolean;
    defaultGasLimit?: number;
  }
}
```

**Usage**:
```typescript
/// <reference types="near-outlayer" />

const sim = new OutLayer.ContractSimulator();
await sim.execute('counter.wasm', 'increment', {});
```

---

## Complete Example

### Project Structure

```
near-outlayer/
├── browser-worker/
│   ├── src/
│   │   ├── index.ts              # Entry point
│   │   ├── contract-simulator.ts # Core class
│   │   ├── linux-executor.ts     # Linux mode (optional)
│   │   └── types.ts              # Type definitions
│   ├── dist/
│   │   ├── index.js              # CJS
│   │   ├── index.mjs             # ESM
│   │   └── index.global.js       # IIFE
│   ├── tsup.config.ts
│   └── package.json
```

### Entry Point (`src/index.ts`)

```typescript
// Re-export main classes
export { ContractSimulator } from './contract-simulator';
export { LinuxExecutor } from './linux-executor';

// Re-export types
export type {
  ContractSimulatorOptions,
  ExecutionResult,
  ExecutionContext,
} from './types';

// Version info
export const VERSION = '1.0.0';
```

### Browser Usage (`index.html`)

```html
<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <title>NEAR OutLayer - IIFE Example</title>
</head>
<body>
  <h1>NEAR OutLayer IIFE Example</h1>

  <button id="runDirect">Run Direct Execution</button>
  <button id="runLinux">Run Linux Execution</button>

  <pre id="output"></pre>

  <!-- Load peer dependencies -->
  <script src="https://cdn.jsdelivr.net/npm/near-api-js@2.1.0/dist/near-api-js.min.js"></script>

  <!-- Load OutLayer IIFE -->
  <script src="dist/index.global.js"></script>

  <script>
    // OutLayer is now available as global
    console.log('OutLayer version:', OutLayer.VERSION);

    // Initialize simulator
    const simulator = new OutLayer.ContractSimulator({
      verboseLogging: true,
      executionMode: 'direct',
    });

    const output = document.getElementById('output');

    // Direct execution
    document.getElementById('runDirect').addEventListener('click', async () => {
      output.textContent = 'Executing in direct mode...';

      try {
        const result = await simulator.execute(
          'test-contracts/counter/res/counter.wasm',
          'increment',
          {}
        );

        output.textContent = JSON.stringify(result, null, 2);
      } catch (error) {
        output.textContent = `Error: ${error.message}`;
      }
    });

    // Linux execution
    document.getElementById('runLinux').addEventListener('click', async () => {
      output.textContent = 'Switching to Linux mode...';

      try {
        await simulator.setExecutionMode('linux');

        const result = await simulator.execute(
          'test-contracts/counter/res/counter.wasm',
          'increment',
          {}
        );

        output.textContent = JSON.stringify(result, null, 2);
      } catch (error) {
        output.textContent = `Error: ${error.message}`;
      }
    });
  </script>
</body>
</html>
```

---

## Key Takeaways

1. **Pattern is well-established**: fastnear-js-monorepo proves IIFE works for NEAR libraries
2. **tsup is the right tool**: Zero-config, fast, generates all formats (CJS/ESM/IIFE)
3. **WASM integration is feasible**: Multiple strategies available (embedded, dynamic, VFS)
4. **Phased approach recommended**: Start with core IIFE, add Linux mode later via lazy loading
5. **Performance is acceptable**: ~25 KB gzipped for core, ~650ms time to interactive

### Competitive Advantage

**IIFE bundling enables**:
- Educational use cases (workshops, tutorials)
- Rapid prototyping without build tools
- Embedded usage (documentation, playgrounds)
- Reduced onboarding friction

**Comparison**:
| Library | IIFE Bundle | CDN Ready | Zero Config |
|---------|-------------|-----------|-------------|
| **OutLayer** | Planned | Planned | Yes |
| near-api-js | Yes | Yes | Yes |
| Ethereum web3.js | Yes | Yes | Yes |
| Solana web3.js | ESM only | Bundler required | No |

---

## References

### Build Tools

- **tsup**: https://tsup.egoist.dev/ - Zero-config bundler (powered by esbuild)
- **esbuild**: https://esbuild.github.io/ - Ultra-fast JavaScript bundler
- **Rollup**: https://rollupjs.org/ - Alternative with excellent tree-shaking

### NEAR Resources

- **near-api-js**: https://github.com/near/near-api-js - Already provides IIFE build
- **fastnear-js-monorepo**: Internal reference (`/Users/mikepurvis/near/fastnear-js-monorepo`)

### Standards

- **Module Systems**: CommonJS, ESM, UMD patterns
- **WebAssembly API**: https://developer.mozilla.org/en-US/docs/WebAssembly
- **SharedArrayBuffer**: https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/SharedArrayBuffer
- **COOP/COEP**: https://web.dev/coop-coep/ - Required for SharedArrayBuffer

---

## Next Steps

**When ready to implement**:
1. Setup tsup configuration in `browser-worker/`
2. Create IIFE build and test in browser
3. Measure bundle size and optimize
4. Document CDN usage for end users

**Status**: Reference complete, awaiting implementation phase

---

**Related Documentation**:
- [Chapter 2: Linux/WASM Integration](02-linux-wasm-integration.md) - Understanding execution modes
- [Chapter 3: Multi-Layer Roadmap](03-multi-layer-roadmap.md) - Strategic phases
- Full reference: `browser-worker/docs/IIFE_BUNDLING_REFERENCE.md`
