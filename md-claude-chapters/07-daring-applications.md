# Chapter 7: Daring Applications - Novel Use Cases Enabled

**Document Type**: Strategic Vision + Technical Implementation
**Audience**: Product, BD, Ecosystem, Developers
**Prerequisites**: Chapters 3, 6

---

## Executive Summary

The 4-layer architecture unlocks three categories of applications **impossible on any other blockchain**:

1. **Autonomous AI Trading Agents** - Verifiable, deterministic ML inference on-chain
2. **Deterministic Plugin Systems** - Safe extensibility for DeFi protocols
3. **Stateful Multi-Process Edge Computing** - Full POSIX FaaS with millisecond startup

These applications position NEAR OutLayer as the **programmable offshore zone** for computation that is too complex, too dynamic, or too compute-intensive for L1 smart contracts.

---

## Application 1: Autonomous AI Trading Agents

### The Problem

**Current state**: AI-powered trading strategies exist but have trust issues:

- **Off-chain AI**: Runs on centralized servers, no verifiability
  - Users must trust the operator won't manipulate results
  - No audit trail for decisions
  - Cannot prove model wasn't changed post-facto

- **On-chain AI**: Prohibitively expensive
  - Running TensorFlow on Ethereum: ~$1000+ per inference
  - Model size limits (small models = poor accuracy)
  - No state between executions (cannot learn)

- **Oracle-based**: Slow and expensive
  - Multi-block latency (Chainlink: ~10 minutes)
  - High costs ($5-50 per query)
  - Oracle can be manipulated

**What users want**: Verifiable AI that runs fast enough for real-time trading, cheap enough for retail users, and auditable enough for institutional compliance.

### The OutLayer Solution

**Architecture**: AI agent as JavaScript contract executing in Frozen Realm (L4)

```
NEAR Blockchain (L1)
  ↓ Request execution via OutLayer contract
OutLayer Coordinator
  ↓ Assign to worker
Browser/Server Worker
  ├─ L1: Browser WASM runtime
  └─ L2: linux-wasm (full POSIX)
     └─ L3: QuickJS (JavaScript engine)
        └─ L4: Frozen Realm (deterministic)
           └─ TensorFlow.js + User Agent Code
```

**Key Innovation**: All validators execute the **same ML model** with the **same market data**, producing the **same trading decision**. This creates verifiable, deterministic AI.

### Technical Implementation

#### Step 1: Model Preparation

**Convert Python model to JavaScript**:

```python
# Train model (Python)
import tensorflow as tf

model = tf.keras.Sequential([
    tf.keras.layers.LSTM(128, input_shape=(60, 5)),  # 60 timesteps, 5 features
    tf.keras.layers.Dense(64, activation='relu'),
    tf.keras.layers.Dense(3, activation='softmax'),  # Buy/Sell/Hold
])

model.fit(training_data, labels, epochs=100)

# Export to TensorFlow.js format
import tensorflowjs as tfjs
tfjs.converters.save_keras_model(model, 'model_js/')
```

**Bundle with QuickJS**:

```bash
# Install TensorFlow.js (browser build)
npm install @tensorflow/tfjs

# Bundle for QuickJS (no module system)
esbuild node_modules/@tensorflow/tfjs/dist/tf.min.js \
  --bundle \
  --format=iife \
  --global-name=tf \
  --outfile=tfjs-bundle.js

# Include model weights
cp model_js/model.json model_js/weights.bin agent-contract/
```

#### Step 2: Agent Contract

**File**: `contracts/ai-agent/trading-agent.js`

```javascript
// Load TensorFlow.js (bundled for QuickJS)
load('/runtime/tfjs-bundle.js');

// Load pre-trained model
const modelJson = near.storageRead('model.json');
const weights = near.storageRead('weights.bin');
const model = await tf.loadLayersModel({
  load: () => ({
    modelTopology: JSON.parse(modelJson),
    weightSpecs: JSON.parse(weights.specs),
    weightData: weights.data,
  }),
});

// Risk management constraints (immutable)
const CONSTRAINTS = Object.freeze({
  maxPositionSize: 1000,     // Max 1000 NEAR per trade
  maxLeverage: 2,            // 2x max
  stopLoss: 0.05,            // 5% stop loss
  takeProfitTargets: [0.02, 0.05, 0.10],  // 2%, 5%, 10%
});

// Main trading logic
export async function execute(marketData) {
  near.log('[Agent] Starting execution');

  // Step 1: Preprocess market data
  const features = preprocessMarketData(marketData);
  const tensor = tf.tensor2d(features, [1, 60, 5]);  // 1 batch, 60 timesteps, 5 features

  // Step 2: Model inference (deterministic!)
  const predictions = model.predict(tensor);
  const probabilities = await predictions.array();  // [buy_prob, sell_prob, hold_prob]

  near.log(`[Agent] Predictions: ${JSON.stringify(probabilities[0])}`);

  // Step 3: Decision logic
  const [buyProb, sellProb, holdProb] = probabilities[0];
  const maxProb = Math.max(buyProb, sellProb, holdProb);
  const confidence = maxProb;

  let action = 'hold';
  if (buyProb === maxProb && confidence > 0.8) {
    action = 'buy';
  } else if (sellProb === maxProb && confidence > 0.8) {
    action = 'sell';
  }

  // Step 4: Risk management
  if (action !== 'hold') {
    const positionSize = calculatePositionSize(confidence, CONSTRAINTS);
    const validated = validateRiskConstraints(positionSize, CONSTRAINTS);

    if (!validated) {
      near.log('[Agent] Risk constraints violated, holding');
      action = 'hold';
    }
  }

  // Step 5: Execute trade (if not hold)
  if (action === 'buy' || action === 'sell') {
    const result = await executeTrade(action, positionSize, marketData.currentPrice);

    near.log(`[Agent] Trade executed: ${JSON.stringify(result)}`);

    return {
      action,
      confidence,
      positionSize,
      executionPrice: result.price,
      timestamp: near.blockTimestamp(),
    };
  }

  return {
    action: 'hold',
    confidence,
    reason: 'Low confidence or risk constraints',
    timestamp: near.blockTimestamp(),
  };
}

// Helper: Preprocess market data to model input format
function preprocessMarketData(marketData) {
  // Extract 60 timesteps of: [open, high, low, close, volume]
  const features = [];

  for (const candle of marketData.candles.slice(-60)) {
    features.push([
      normalize(candle.open, marketData.priceRange),
      normalize(candle.high, marketData.priceRange),
      normalize(candle.low, marketData.priceRange),
      normalize(candle.close, marketData.priceRange),
      normalize(candle.volume, marketData.volumeRange),
    ]);
  }

  return features;
}

// Helper: Calculate position size based on confidence
function calculatePositionSize(confidence, constraints) {
  // Kelly criterion-inspired sizing
  const edgeEstimate = (confidence - 0.5) * 2;  // Map [0.5, 1.0] to [0, 1.0]
  const fractionOfBankroll = edgeEstimate * 0.5;  // Conservative: 50% of Kelly

  const bankroll = near.storageRead('current_bankroll');
  const positionSize = bankroll * fractionOfBankroll;

  // Apply constraints
  return Math.min(positionSize, constraints.maxPositionSize);
}

// Helper: Validate risk constraints
function validateRiskConstraints(positionSize, constraints) {
  const currentLeverage = calculateCurrentLeverage();

  if (currentLeverage + (positionSize / getBankroll()) > constraints.maxLeverage) {
    return false;  // Would exceed leverage limit
  }

  if (positionSize > constraints.maxPositionSize) {
    return false;  // Position too large
  }

  return true;
}

// Helper: Execute trade via cross-contract call
async function executeTrade(action, amount, currentPrice) {
  const method = action === 'buy' ? 'swap_exact_in' : 'swap_exact_out';

  // Call DEX contract
  const result = await near.promiseCreate(
    'ref-finance.near',
    method,
    {
      pool_id: 'NEAR-USDC',
      token_in: action === 'buy' ? 'USDC' : 'NEAR',
      token_out: action === 'buy' ? 'NEAR' : 'USDC',
      amount_in: amount.toString(),
      min_amount_out: calculateMinAmountOut(amount, currentPrice, 0.01),  // 1% slippage
    },
    0,  // No attached deposit
    100_000_000_000_000  // 100 Tgas
  );

  return await near.promiseReturn(result);
}
```

#### Step 3: Deployment & Execution

**Deploy agent**:

```bash
# Upload model to NEAR storage
near call outlayer.near store_secrets '{
  "repo": "github.com/alice/trading-agent",
  "branch": "main",
  "profile": "model-weights",
  "encrypted_secrets": [/* model.json + weights.bin encrypted */],
  "access_condition": {"AllowAll": {}}
}' --accountId alice.near --deposit 1.0

# Request execution every hour (via cron or keeper network)
near call outlayer.near request_execution '{
  "code_source": {
    "repo": "https://github.com/alice/trading-agent",
    "commit": "main",
    "build_target": "wasm32-wasip1"
  },
  "secrets_ref": {
    "profile": "model-weights",
    "account_id": "alice.near"
  },
  "resource_limits": {
    "max_instructions": 50000000000,  // 50B instructions (~5-10 seconds)
    "max_memory_mb": 256,
    "max_execution_seconds": 60
  },
  "input_data": "{
    \"market_data\": {
      \"candles\": [...],  // Last 60 candles from oracle
      \"currentPrice\": 4.32,
      \"priceRange\": [3.8, 4.8],
      \"volumeRange\": [1000000, 5000000]
    }
  }"
}' --accountId alice.near --deposit 0.5
```

**Monitor results**:

```javascript
// Dashboard displays agent decisions
const executions = await contract.get_request_history({ account_id: 'alice.near' });

for (const exec of executions) {
  console.log(`
    Timestamp: ${exec.timestamp}
    Action: ${exec.result.action}
    Confidence: ${exec.result.confidence}
    Position: ${exec.result.positionSize} NEAR
    Price: $${exec.result.executionPrice}
  `);
}
```

### Novel Benefits

**1. Verifiable Execution**
- All validators run same model + same data → same decision
- Consensus on trading action
- Cannot claim "the AI made a different decision" post-facto
- Audit trail: Every decision recorded on-chain

**2. Deterministic ML Inference**
- Frozen Realm eliminates Date.now/Math.random
- TensorFlow.js operations are deterministic (no JIT optimization variance)
- Same inputs → same outputs always
- Enables time-travel debugging: Replay historical data, get same decisions

**3. Transparent Risk Management**
- Risk constraints coded in immutable CONSTRAINTS object
- Cannot be changed without redeploying (visible to users)
- Validates every trade against constraints
- Users can verify agent won't "go rogue"

**4. Institutional Compliance**
- Full audit trail for regulators
- Verifiable execution (all validators agree)
- Open-source model (can inspect for bias)
- Deterministic = reproducible for compliance reports

**5. Rapid Iteration**
- Update model: Just redeploy JavaScript (no WASM compilation)
- A/B testing: Run multiple models, compare results
- Load from IPFS: Fetch latest model dynamically

### Market Opportunity

**Target users**:
- **Retail traders**: Want algo trading without trust in centralized services
- **Hedge funds**: Need compliance-friendly verifiable execution
- **Market makers**: Require deterministic strategies for audit
- **DeFi protocols**: Want automated liquidity management

**Revenue model**:
- Users pay per execution (~$0.01-0.10)
- Model creators earn royalties (10% of execution fees)
- Platform takes fee (20% of total)

**Competitive advantage vs**:
- **TradingView/3Commas**: Not verifiable, centralized
- **Ethereum ML**: Too expensive ($1000+ per inference)
- **Solana ML**: No determinism guarantees

---

## Application 2: Deterministic Plugin Systems for DeFi

### The Problem

**Current state**: DeFi protocols are rigid, cannot extend without forking:

- **Uniswap V2**: Want custom AMM formulas? → Fork entire protocol
- **Aave**: Want new collateral types? → Governance vote + core upgrade
- **Compound**: Want dynamic interest rates? → Wait for protocol V3

**What protocols want**: **Safe extensibility** without sacrificing security.

**What's been tried**:
- **Ethereum hooks** (Uniswap V4): Limited to specific integration points, still Solidity
- **CosmWasm plugins**: Requires Rust, no dynamism
- **Ethereum EIP-2535 (Diamond Standard)**: Complex, gas-inefficient

**What's needed**: A plugin system where:
1. Protocols can remain immutable (no upgrades)
2. Plugins can be added without governance
3. Plugins are sandboxed (cannot steal funds)
4. Plugins are auditable (deterministic, inspectable)

### The OutLayer Solution

**Architecture**: Plugin ecosystem with Frozen Realm isolation

```
DeFi Protocol (Base Contract)
  ├─ Core logic (immutable)
  └─ Plugin registry (stores plugin IDs)

Plugin (JavaScript in Frozen Realm)
  ├─ Cannot access protocol storage directly
  ├─ Cannot call arbitrary external contracts
  └─ Can ONLY return calculations to protocol

Protocol calls plugin:
  1. Protocol: "Plugin, what swap parameters for 100 USDC → NEAR?"
  2. Plugin: Runs complex formula, returns: { amountOut: 23.4, priceImpact: 0.02 }
  3. Protocol: Validates result, executes swap if acceptable
```

**Security model**: **Plugins are pure functions**. They receive inputs, return outputs, but cannot directly modify state or make external calls.

### Technical Implementation

#### Plugin Example: Curve Stable Swap Formula

**File**: `plugins/curve-stable-swap.js`

```javascript
// Curve stable swap formula plugin
// Implements StableSwap invariant: A * n^n * sum(x_i) + D = A * D * n^n + D^(n+1) / (n^n * prod(x_i))

export const metadata = {
  name: 'Curve Stable Swap',
  version: '1.0.0',
  author: 'curve-finance.near',
  license: 'MIT',
  description: 'Low-slippage swaps for correlated assets',
};

// Main plugin entry point
export function calculateSwap(params) {
  const { tokenIn, tokenOut, amountIn, poolState } = params;

  // Extract pool balances
  const balances = poolState.balances;  // [DAI, USDC, USDT]
  const A = poolState.amplificationCoefficient;  // e.g., 100
  const fee = poolState.feeRate;  // e.g., 0.0004 (0.04%)

  // Step 1: Calculate D (total pool value invariant)
  const D = calculateD(balances, A);

  // Step 2: Calculate new balance after swap
  const indexIn = poolState.tokens.indexOf(tokenIn);
  const indexOut = poolState.tokens.indexOf(tokenOut);

  const newBalanceIn = balances[indexIn] + amountIn;
  const newBalanceOut = calculateY(indexOut, newBalanceIn, balances, A, D);

  // Step 3: Calculate amount out
  const amountOut = balances[indexOut] - newBalanceOut;

  // Step 4: Apply fee
  const feeAmount = amountOut * fee;
  const amountOutAfterFee = amountOut - feeAmount;

  // Step 5: Calculate price impact
  const priceImpact = calculatePriceImpact(amountIn, amountOut, balances);

  // Return calculation (protocol validates and executes)
  return {
    amountOut: amountOutAfterFee,
    feeAmount,
    priceImpact,
    newBalances: [
      indexIn === 0 ? newBalanceIn : (indexOut === 0 ? newBalanceOut : balances[0]),
      indexIn === 1 ? newBalanceIn : (indexOut === 1 ? newBalanceOut : balances[1]),
      indexIn === 2 ? newBalanceIn : (indexOut === 2 ? newBalanceOut : balances[2]),
    ],
  };
}

// Helper: Calculate D (StableSwap invariant)
function calculateD(balances, A) {
  const n = balances.length;
  let sum = balances.reduce((a, b) => a + b, 0);

  if (sum === 0) return 0;

  let D = sum;
  const Ann = A * n;

  for (let i = 0; i < 255; i++) {  // Newton's method, max 255 iterations
    let D_P = D;
    for (const balance of balances) {
      D_P = D_P * D / (balance * n);
    }

    const D_prev = D;
    D = (Ann * sum + D_P * n) * D / ((Ann - 1) * D + (n + 1) * D_P);

    if (Math.abs(D - D_prev) <= 1) {
      break;  // Converged
    }
  }

  return D;
}

// Helper: Calculate Y (new balance for token out)
function calculateY(index, newBalanceIn, balances, A, D) {
  const n = balances.length;
  const Ann = A * n;

  let c = D;
  let sum = 0;

  for (let i = 0; i < n; i++) {
    if (i === index) continue;

    const balance = i === indexOf(newBalanceIn, balances) ? newBalanceIn : balances[i];
    sum += balance;
    c = c * D / (balance * n);
  }

  c = c * D / (Ann * n);
  const b = sum + D / Ann;

  let y = D;
  for (let i = 0; i < 255; i++) {
    const y_prev = y;
    y = (y * y + c) / (2 * y + b - D);

    if (Math.abs(y - y_prev) <= 1) {
      break;
    }
  }

  return y;
}

// Helper: Calculate price impact
function calculatePriceImpact(amountIn, amountOut, balances) {
  const totalLiquidity = balances.reduce((a, b) => a + b, 0);
  const idealPrice = 1.0;  // For stablecoins
  const actualPrice = amountIn / amountOut;

  return Math.abs(actualPrice - idealPrice) / idealPrice;
}

// No storage access, no external calls, no Date.now/Math.random
// Pure function: same inputs → same outputs always
```

#### Protocol Integration

**File**: `contracts/extensible-dex.js`

```javascript
// DEX protocol with plugin support

// Plugin registry (stores plugin code hashes + metadata)
const plugins = new Map();

// Admin: Install plugin
export function installPlugin(pluginCode, pluginId) {
  // Only owner can install
  require(near.predecessorAccountId() === getOwner(), 'Not authorized');

  // Verify plugin code (optional: require audit signature)
  const codeHash = sha256(pluginCode);

  // Store plugin
  plugins.set(pluginId, {
    code: pluginCode,
    codeHash,
    installedAt: near.blockTimestamp(),
    active: true,
  });

  near.log(`Plugin installed: ${pluginId}`);
}

// User: Swap with plugin
export async function swapWithPlugin(pluginId, tokenIn, tokenOut, amountIn) {
  const plugin = plugins.get(pluginId);
  require(plugin && plugin.active, 'Plugin not found or inactive');

  // Get current pool state
  const poolState = {
    balances: [getBalance('DAI'), getBalance('USDC'), getBalance('USDT')],
    tokens: ['DAI', 'USDC', 'USDT'],
    amplificationCoefficient: 100,
    feeRate: 0.0004,
  };

  // Execute plugin in Frozen Realm (sandboxed!)
  const realm = new NearFrozenRealm();
  const pluginResult = await realm.execute(plugin.code, 'calculateSwap', {
    tokenIn,
    tokenOut,
    amountIn,
    poolState,
  });

  // Validate plugin result
  require(pluginResult.amountOut > 0, 'Invalid plugin output');
  require(pluginResult.priceImpact < 0.05, 'Price impact too high');
  require(pluginResult.newBalances.length === 3, 'Invalid balance array');

  // Execute swap (protocol retains control)
  const actualAmountOut = executeSwap(
    tokenIn,
    tokenOut,
    amountIn,
    pluginResult.amountOut
  );

  near.log(`Swap executed via plugin ${pluginId}: ${amountIn} ${tokenIn} → ${actualAmountOut} ${tokenOut}`);

  return {
    amountOut: actualAmountOut,
    feeAmount: pluginResult.feeAmount,
    priceImpact: pluginResult.priceImpact,
  };
}

// Internal: Execute swap (protocol logic, not plugin)
function executeSwap(tokenIn, tokenOut, amountIn, expectedAmountOut) {
  // Transfer tokens from user
  tokenTransferFrom(near.predecessorAccountId(), near.currentAccountId(), tokenIn, amountIn);

  // Update balances
  increaseBalance(tokenIn, amountIn);
  decreaseBalance(tokenOut, expectedAmountOut);

  // Transfer tokens to user
  tokenTransfer(near.predecessorAccountId(), tokenOut, expectedAmountOut);

  return expectedAmountOut;
}
```

### Novel Benefits

**1. Safe Extensibility**
- Plugins cannot directly modify protocol storage
- Plugins cannot make arbitrary external calls
- Plugins are pure functions (inputs to outputs)
- Protocol validates all plugin results before executing

**2. Rapid Innovation**
- Deploy new AMM formulas instantly (no protocol upgrade)
- A/B test multiple plugins (users choose preferred)
- Community-driven plugin marketplace
- No governance bottleneck

**3. Auditability**
- Plugin code is JavaScript (human-readable, not WASM)
- Deterministic (same inputs produce same outputs)
- Can replay historical swaps with plugin code to verify behavior
- Formal verification easier than Solidity

**4. Composability**
- Plugins can call other plugins (with protocol mediation)
- Mix-and-match: Use Curve formula for stables, Uniswap V3 for volatiles
- Protocol remains single source of truth

### Market Opportunity

**Target protocols**:
- **DEXs**: Custom AMM formulas (Curve, Balancer-style)
- **Lending**: Dynamic interest rates based on market conditions
- **Options**: Exotic payoff structures (Asian options, barrier options)
- **Yield aggregators**: Strategy plugins (YFI-style vaults)

**Plugin marketplace**:
- Developers earn fees per plugin usage (1-10% of swap fees)
- Protocols can curate approved plugins (whitelist)
- Users discover plugins via marketplace UI
- Auditors earn by certifying plugins

**Competitive advantage**:
- **Uniswap V4 hooks**: Limited to Solidity, specific integration points
- **Compound V3**: No plugin support
- **Aave**: Governance-gated additions only

---

## Application 3: Stateful Multi-Process Edge Computing

### The Problem

**Current state**: Edge computing is constrained by platform limitations:

- **AWS Lambda / Cloudflare Workers**: Stateless, single-process functions
  - Cannot run databases (no persistent state)
  - Cannot spawn child processes (no fork/exec)
  - Cannot use POSIX pipes (no IPC)
  - Startup cold: ~100ms (Lambda), ~1ms (Workers)

- **Containers** (Docker on edge): Heavy, slow to start
  - Startup: ~5-10 seconds
  - Memory: ~100 MB minimum per container
  - Not suitable for burst workloads (cold starts dominate)

**What edge apps need**: POSIX environment (databases, multi-process) + fast startup (milliseconds) + lightweight (megabytes, not gigabytes).

### The OutLayer Solution

**Architecture**: Linux/WASM provides full POSIX at edge with millisecond startup

```
Edge Device (Raspberry Pi, IoT gateway, CDN edge node)
  ├─ Single OutLayer worker process (~30-40 MB)
  └─ Instantiates multiple tenants (each in own linux-wasm instance)
     ├─ Tenant A: Web server + SQLite database
     ├─ Tenant B: Data processing pipeline (fork/exec pattern)
     └─ Tenant C: Log aggregation (syslog + awk/grep)

Each tenant:
  - Full Linux environment (BusyBox + custom binaries)
  - Persistent state (virtual filesystem backed by edge storage)
  - Multi-process workflows (vfork, pipes, signals)
  - Isolated from others (L1 WASM sandbox)
```

### Technical Implementation

#### Use Case: Edge Web Server with Database

**File**: `edge-apps/web-server-sqlite.js`

```javascript
// Edge web server with SQLite database
// Demonstrates: Multi-process, filesystem, networking

// Load SQL.js (SQLite compiled to WASM)
load('/runtime/sql.js');

// Initialize database (persistent via NEAR storage)
const dbPath = '/near/state/database.db';
let db;

if (near.storageHasKey('database.db')) {
  // Load existing database
  const dbBytes = near.storageRead('database.db');
  db = new SQL.Database(dbBytes);
} else {
  // Create new database
  db = new SQL.Database();

  // Create schema
  db.run(`
    CREATE TABLE users (
      id INTEGER PRIMARY KEY,
      name TEXT NOT NULL,
      email TEXT UNIQUE,
      created_at INTEGER
    );

    CREATE TABLE requests (
      id INTEGER PRIMARY KEY,
      path TEXT,
      method TEXT,
      ip_address TEXT,
      timestamp INTEGER
    );
  `);

  near.log('[DB] Database initialized');
}

// HTTP server (runs as POSIX process via WASI-HTTP)
export async function handleRequest(request) {
  const { method, path, headers, body } = request;

  near.log(`[HTTP] ${method} ${path}`);

  // Log request to database
  db.run(`
    INSERT INTO requests (path, method, ip_address, timestamp)
    VALUES (?, ?, ?, ?)
  `, [path, method, headers['x-forwarded-for'], Date.now()]);

  // Route handling
  if (path === '/users' && method === 'GET') {
    return await handleGetUsers();
  } else if (path === '/users' && method === 'POST') {
    return await handleCreateUser(JSON.parse(body));
  } else if (path === '/stats' && method === 'GET') {
    return await handleGetStats();
  }

  return {
    statusCode: 404,
    body: JSON.stringify({ error: 'Not found' }),
  };
}

// Handler: GET /users
async function handleGetUsers() {
  const result = db.exec('SELECT * FROM users ORDER BY created_at DESC LIMIT 100');

  const users = result[0].values.map(row => ({
    id: row[0],
    name: row[1],
    email: row[2],
    created_at: row[3],
  }));

  return {
    statusCode: 200,
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ users }),
  };
}

// Handler: POST /users
async function handleCreateUser(data) {
  const { name, email } = data;

  try {
    db.run(`
      INSERT INTO users (name, email, created_at)
      VALUES (?, ?, ?)
    `, [name, email, Date.now()]);

    // Persist database to NEAR storage
    await persistDatabase();

    return {
      statusCode: 201,
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ success: true, message: 'User created' }),
    };
  } catch (error) {
    return {
      statusCode: 400,
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ error: error.message }),
    };
  }
}

// Handler: GET /stats
async function handleGetStats() {
  // Query request stats
  const totalRequests = db.exec('SELECT COUNT(*) FROM requests')[0].values[0][0];
  const uniqueIPs = db.exec('SELECT COUNT(DISTINCT ip_address) FROM requests')[0].values[0][0];
  const recentRequests = db.exec(`
    SELECT path, COUNT(*) as count
    FROM requests
    WHERE timestamp > ?
    GROUP BY path
    ORDER BY count DESC
    LIMIT 10
  `, [Date.now() - 3600000])[0];  // Last hour

  return {
    statusCode: 200,
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({
      totalRequests,
      uniqueIPs,
      topPaths: recentRequests.values.map(r => ({ path: r[0], count: r[1] })),
    }),
  };
}

// Helper: Persist database to NEAR storage
async function persistDatabase() {
  const dbBytes = db.export();
  near.storageWrite('database.db', dbBytes);
  near.log('[DB] Database persisted');
}

// Cleanup: Save database on shutdown
export function cleanup() {
  persistDatabase();
  db.close();
}
```

#### Use Case: Data Processing Pipeline

**File**: `edge-apps/data-pipeline.js`

```javascript
// Multi-process data processing pipeline
// Demonstrates: fork/exec, pipes, BusyBox utilities

export async function processDataset(datasetUrl) {
  near.log('[Pipeline] Starting data processing');

  // Step 1: Download dataset (via fetch)
  const data = await fetch(datasetUrl).then(r => r.text());
  near.log(`[Pipeline] Downloaded ${data.length} bytes`);

  // Step 2: Split into chunks (parallel processing via fork)
  const chunks = splitIntoChunks(data, 4);  // 4 chunks

  const workers = [];
  for (let i = 0; i < chunks.length; i++) {
    // Fork worker process (POSIX vfork/exec)
    const worker = await linux.spawn('qjs', ['process-chunk.js'], {
      stdin: chunks[i],
    });

    workers.push(worker);
  }

  // Wait for all workers to complete
  const results = await Promise.all(workers.map(w => w.waitForExit()));

  near.log('[Pipeline] All workers completed');

  // Step 3: Aggregate results (BusyBox awk)
  const aggregated = await linux.spawn('awk', [
    '{sum+=$1; count++} END {print sum/count}',
  ], {
    stdin: results.map(r => r.stdout).join('\n'),
  });

  const average = parseFloat(await aggregated.stdout());

  near.log(`[Pipeline] Average: ${average}`);

  // Step 4: Store result
  near.storageWrite('pipeline_result', JSON.stringify({
    datasetUrl,
    average,
    processedAt: near.blockTimestamp(),
  }));

  return { average };
}

// Helper: Split data into N chunks
function splitIntoChunks(data, n) {
  const lines = data.split('\n');
  const chunkSize = Math.ceil(lines.length / n);

  const chunks = [];
  for (let i = 0; i < n; i++) {
    chunks.push(lines.slice(i * chunkSize, (i + 1) * chunkSize).join('\n'));
  }

  return chunks;
}
```

**Worker script** (`process-chunk.js`):

```javascript
// Worker process: Process single chunk of data

// Read from stdin (POSIX pipe)
const chunk = readStdin();

// Process each line
const numbers = chunk.split('\n')
  .map(line => {
    const parsed = parseFloat(line);
    return isNaN(parsed) ? 0 : parsed;
  })
  .filter(n => n > 0);  // Filter invalid data

// Output to stdout (will be piped to parent)
for (const num of numbers) {
  console.log(num * 2);  // Example: double each number
}
```

### Novel Benefits

**1. Full POSIX at Edge**
- Run SQLite databases (persistent state)
- Spawn child processes (parallel workflows)
- Use BusyBox utilities (awk, grep, sed for data processing)
- Traditional UNIX patterns (pipes, signals)

**2. Fast Startup**
- linux-wasm boot: <1 second (cached)
- vs Docker containers: ~5-10 seconds
- Enables serverless edge without cold start penalty

**3. Lightweight**
- Per-tenant memory: ~30-40 MB
- vs containers: ~100+ MB minimum
- Enables high-density multi-tenancy on edge devices

**4. Portable State**
- Entire filesystem backed by NEAR storage
- Snapshot running app, transfer to different edge node
- Enables edge workload migration (load balancing)

**5. Verifiable Execution**
- All operations recorded on NEAR (audit trail)
- Deterministic (replay possible)
- TEE integration (Phase 2): Hardware-verified correctness

### Market Opportunity

**Target customers**:
- **CDN providers**: Cloudflare, Fastly competitors (with POSIX support)
- **IoT platforms**: Process data at edge before cloud upload
- **5G edge**: Low-latency services for mobile users
- **Manufacturing**: Edge analytics for factory equipment

**Pricing**:
- Per-execution pricing (like AWS Lambda)
- Storage pricing (NEAR storage costs)
- No idle costs (true pay-per-use)

**Competitive advantage**:
- **Cloudflare Workers**: No POSIX, single-process only
- **AWS Lambda**: Slow cold start, expensive
- **Docker on edge**: Heavy, slow
- **OutLayer**: POSIX + fast + lightweight

---

## Summary: Market Positioning

### NEAR OutLayer Unique Value Proposition

**"The Programmable Offshore Zone for Blockchain"**

| Use Case | OutLayer Solution | Alternatives | Advantage |
|----------|-------------------|--------------|-----------|
| **AI Agents** | Verifiable ML inference (L4 determinism) | Off-chain (unverifiable) or On-chain (expensive) | Verifiable + affordable |
| **Plugin Systems** | Frozen Realm isolation (safe extensibility) | Fork protocol or Limited hooks | Safe + flexible |
| **Edge Computing** | Full POSIX + fast startup (Linux/WASM) | Containers (slow) or Workers (limited) | POSIX + performance |

### Competitive Moat

**No other blockchain has**:
1. JavaScript contracts (Ethereum/Solana: compiled only)
2. Full POSIX environment (all chains: limited runtimes)
3. Deterministic execution (most chains: non-reproducible)
4. Browser-native execution (most chains: server infrastructure only)

**This positions NEAR OutLayer as the only platform for a new category of blockchain applications: verifiable, stateful, complex computation.**

---

## Related Documentation

- **Roadmap**: [Chapter 3: Multi-Layer Roadmap](03-multi-layer-roadmap.md)
- **Architecture**: [Chapter 6: 4-Layer Architecture](06-4-layer-architecture.md)
- **Implementation**: [Chapter 2: Linux/WASM Integration](02-linux-wasm-integration.md)
