'use client';

import { useState, useEffect, Suspense } from 'react';
import { useSearchParams } from 'next/navigation';
import { useNearWallet } from '@/contexts/NearWalletContext';
import { checkWasmExists, checkWasmExistsByChecksum } from '@/lib/api';
import { getTransactionUrl } from '@/lib/explorer';
import { actionCreators } from '@near-js/transactions';
import WalletConnectionModal from '@/components/WalletConnectionModal';

// Base preset configuration
interface BasePreset {
  name: string;
  description?: string;
  networks?: ('testnet' | 'mainnet')[];
  docsLink?: string; // Link to example in /docs/examples
}

// Direct execution preset (calls OutLayer)
interface DirectPreset extends BasePreset {
  type: 'direct';
  codeSourceType?: 'github' | 'wasmurl' | 'project'; // default: 'github'
  // GitHub source fields
  repo?: string;
  commit?: string;
  // WasmUrl source fields
  wasmUrl?: string;
  wasmHash?: string;
  // Project source fields
  projectId?: string; // e.g., "alice.near/my-app"
  versionKey?: string; // specific version key: hash for WasmUrl, "repo@commit" for GitHub (null = active version)
  // Common fields
  buildTarget: string;
  args: string;
  responseFormat: string;
  secretsProfile?: string;
  secretsOwnerTestnet?: string;
  secretsOwnerMainnet?: string;
  // Execution params
  compileOnly?: boolean;
  forceRebuild?: boolean;
  storeOnFastfs?: boolean;
}

// Proxy contract preset (calls application contract)
interface ProxyPreset extends BasePreset {
  type: 'proxy';
  proxyContractIdTestnet?: string; // Contract ID for testnet
  proxyContractIdMainnet?: string; // Contract ID for mainnet
  proxyMethod: string;
  proxyDeposit: string;
  proxyGas: string;
  args: string; // Arguments for proxy method
  // WASM cache check fields (for contracts that call OutLayer)
  wasmRepo?: string;
  wasmCommit?: string;
  wasmBuildTarget?: string;
  increaseDepositIfNoCache?: boolean; // If true, add 0.1 NEAR to deposit when WASM not cached
}

type Preset = DirectPreset | ProxyPreset;

// ============================================================================
// Direct Execution Examples (call OutLayer directly)
// ============================================================================
const DIRECT_PRESETS: DirectPreset[] = [
  {
    type: 'direct',
    name: 'Random Number Generator',
    description: '🎲 Generate truly random numbers on-chain. Great for games, lotteries, and any application needing unpredictable randomness.',
    repo: 'https://github.com/zavodil/random-ark',
    commit: 'main',
    buildTarget: 'wasm32-wasip1',
    args: '{"min":1,"max":100}',
    responseFormat: 'Json',
    networks: ['testnet', 'mainnet'],
    docsLink: '/docs/examples#random-ark',
  },
  {
    type: 'direct',
    name: 'AI Completions',
    description: '🤖 AI inference with system prompt from encrypted secrets. Demo: AI responds with all sentences starting with "O" - you can set any custom rules like language restrictions, personality, response format. Using pre-compiled WASM from FastFS for faster execution.',
    codeSourceType: 'wasmurl',
    wasmUrl: 'https://wasmhub.testnet.fastfs.io/fastfs.testnet/cbf80ed0080dd62f2041745cdc958ec0fbd192f33aeaa756f7873d742204b2f8.wasm',
    wasmHash: '41c1c7b3528565f3fd139943f439d61c0768e9abdb9b579bd0921ecbfcabeded',
    buildTarget: 'wasm32-wasip2',
    args: '{"prompt":"What could the NEAR OutLayer project do?","history":[{"role":"user","content":"Tell me about NEAR"},{"role":"assistant","content":"NEAR is the most scalable Layer 1 blockchain."}],"model_name":"accounts/fireworks/models/gpt-oss-20b","openai_endpoint":"https://api.fireworks.ai/inference/v1/chat/completions","max_tokens":16384}',
    responseFormat: 'Text',
    secretsProfile: 'secret-system-prompt',
    secretsOwnerTestnet: 'zavodil2.testnet',
    secretsOwnerMainnet: 'zavodil.near',
    networks: ['testnet', 'mainnet'],
    docsLink: '/docs/examples#ai-ark',
  },
  {
    type: 'direct',
    name: 'Echo Generator',
    description: '📢 Simple echo with blockchain context (sender, block height, timestamp). Perfect starter to understand how contracts interact with OutLayer.',
    repo: 'https://github.com/zavodil/echo-ark',
    commit: 'main',
    buildTarget: 'wasm32-wasip1',
    args: 'Hello, NEARverse!',
    responseFormat: 'Text',
    networks: ['testnet', 'mainnet'],
    docsLink: '/docs/examples#echo-ark',
  },
  {
    type: 'direct',
    name: 'Weather Data Oracle',
    description: '🌤️ Get real-time weather for any city. Build weather-based smart contracts, agricultural apps, or travel planning tools. Pre-configured secrets ready to test!',
    repo: 'https://github.com/zavodil/weather-ark',
    commit: 'main',
    buildTarget: 'wasm32-wasip2',
    args: '{"city":"London","units":"metric"}',
    responseFormat: 'Json',
    secretsProfile: 'default',
    secretsOwnerTestnet: 'zavodil2.testnet',
    secretsOwnerMainnet: 'zavodil.near',
    networks: ['testnet', 'mainnet'],
    docsLink: '/docs/examples#weather-ark',
  },
  {
    type: 'direct',
    name: 'Multi-Source Data Oracle',
    description: '📊 Get reliable crypto/commodity prices from multiple sources (CoinGecko, Binance, Pyth, etc). Automatic aggregation prevents manipulation. Great for DeFi apps.',
    repo: 'https://github.com/zavodil/oracle-ark',
    commit: '88b72003a06dc8d1972b39240e01aa9c0c7bbe24',
    buildTarget: 'wasm32-wasip2',
    args: '{"requests":[{"id":"eur_usd_rate","sources":[{"name":"custom","custom":{"url":"https://open.er-api.com/v6/latest/EUR","json_path":"rates.USD","value_type":"number","method":"GET","headers":[]}}]},{"id":"near_price","sources":[{"name":"coingecko","id":"near"},{"name":"binance","id":"NEARUSDT"},{"name":"huobi","id":"nearusdt"},{"name":"cryptocom","id":"NEAR_USDT"},{"name":"kucoin","id":"NEAR-USDT"},{"name":"gate","id":"near_usdt"},{"name":"pyth","id":"0xc415de8d2eba7db216527dff4b60e8f3a5311c740dadb233e13e12547e226750"}],"aggregation_method":"median","min_sources_num":4},{"id":"near_last_block_validator","sources":[{"name":"custom","custom":{"url":"https://api.nearblocks.io/v1/blocks/latest?limit=1","json_path":"blocks.0.author_account_id","value_type":"string","method":"GET","headers":[]}}]},{"id":"elden_ring_price_cents","sources":[{"name":"custom","custom":{"url":"https://store.steampowered.com/api/appdetails/?appids=1245620&cc=us","json_path":"1245620.data.price_overview.final","value_type":"number","method":"GET","headers":[]}}]}],"max_price_deviation_percent":10.0}',
    responseFormat: 'Json',
    secretsProfile: 'default',
    secretsOwnerTestnet: 'zavodil2.testnet',
    secretsOwnerMainnet: 'zavodil.near',
    networks: ['testnet', 'mainnet'],
    docsLink: '/docs/examples#oracle-ark',
  },
  {
    type: 'direct',
    name: 'Ethereum API',
    description: '⚡ Query Ethereum blockchain data (balances, smart contracts, transactions). Bridge NEAR with Ethereum data for cross-chain applications.',
    repo: 'https://github.com/zavodil/oracle-ark',
    commit: '7deaa4433f693424c9e47b2e12176cb7c86f504f',
    buildTarget: 'wasm32-wasip2',
    args: '{"requests":[{"id":"alchemy","sources":[{"name":"custom","custom": {"url": "https://eth-mainnet.g.alchemy.com/v2","method": "POST","body": {"method": "eth_getBalance","params": ["0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045", "latest"],"id": 1,"jsonrpc": "2.0"}, "json_path": "result", "value_type": "string"}}]}],"max_price_deviation_percent":10.0}',
    responseFormat: 'Json',
    secretsProfile: 'alchemy',
    secretsOwnerTestnet: 'zavodil2.testnet',
    secretsOwnerMainnet: 'zavodil.near',
    networks: ['testnet', 'mainnet'],
    docsLink: '/docs/examples#ethereum-api',
  },
  {
    type: 'direct',
    name: 'Publish to FastFS',
    description: '📦 Compile code and publish WASM to FastFS for permanent storage. Returns FastFS URL instead of executing. Requires force_rebuild to ensure fresh compilation.',
    repo: 'https://github.com/zavodil/random-ark',
    commit: 'main',
    buildTarget: 'wasm32-wasip1',
    args: '',
    responseFormat: 'Text',
    networks: ['testnet', 'mainnet'],
    docsLink: '/docs/near-integration#request-execution',
    compileOnly: true,
    forceRebuild: true,
    storeOnFastfs: true,
  },
  {
    type: 'direct',
    name: 'Project Storage Test',
    description: '💾 Test persistent storage API. Runs test_all command to verify storage operations (set, get, delete, list_keys, has, get_by_version). Uses project-based execution with shared storage across versions.',
    codeSourceType: 'project',
    projectId: 'zavodil2.testnet/test-storage',
    versionKey: 'fb9086ae8a805ea01be67f2508c7757aa6ece174656a71faed29cff0243541de',
    buildTarget: 'wasm32-wasip2',
    args: '{"command":"test_all"}',
    responseFormat: 'Text',
    networks: ['testnet'],
    docsLink: '/docs/projects',
  },
];

// ============================================================================
// Proxy Contract Examples (call application contracts that use OutLayer)
// ============================================================================
const PROXY_PRESETS: ProxyPreset[] = [
  {
    type: 'proxy',
    name: 'Coin Flip Game',
    args: '{"choice":"Heads"}',
    description: '🎲 Play a coin flip game! Choose Heads or Tails. The proxy contract calls OutLayer for random number generation.\n\n🔗 Contract: https://github.com/zavodil/random-ark/tree/main/random-contract',
    networks: ['testnet', 'mainnet'],
    proxyContractIdTestnet: 'coin-toss.testnet',
    proxyContractIdMainnet: 'coin-toss.near',
    proxyMethod: 'flip_coin',
    proxyDeposit: '10000000000000000000000', // 0.01 NEAR
    proxyGas: '300000000000000', // 300 TGas
    // WASM cache check
    wasmRepo: 'https://github.com/zavodil/random-ark',
    wasmCommit: 'main',
    wasmBuildTarget: 'wasm32-wasip1',
    increaseDepositIfNoCache: true, // Add 0.1 NEAR if WASM not cached
    docsLink: '/docs/examples#random-ark',
  },
  {
    type: 'proxy',
    name: 'NEAR Intents Swap',
    args: '{"receiver_id":"v1.publishintent.near","amount":"20000","msg":"{\\"Swap\\":{\\"token_out\\":\\"wrap.near\\",\\"min_amount_out\\":\\"1000000000000000000\\"}}"}',
    description: '⚠️ Swaps USDC to wNEAR via NEAR Intents protocol.\n\n📋 Before using:\n1. Ensure you have storage_deposit for wNEAR (wrap.near)\n2. Only use whitelisted tokens\n3. Adjust token_out and min_amount_out as needed\n4. This calls ft_transfer_call on USDC token contract\n\n🔗 Contract: https://github.com/zavodil/intents-ark/tree/main/intents-contract',
    networks: ['mainnet'],
    proxyContractIdMainnet: '17208628f84f5d6ad33f0da3bbbeb27ffcb398eac501a31bd6ad2011e36133a1', // USDC mainnet
    proxyMethod: 'ft_transfer_call',
    proxyDeposit: '1', // 1 yoctoNEAR
    proxyGas: '300000000000000', // 300 TGas
    // WASM cache check
    wasmRepo: 'https://github.com/zavodil/intents-ark',
    wasmCommit: 'main',
    wasmBuildTarget: 'wasm32-wasip2',
    increaseDepositIfNoCache: false, // ft_transfer_call accepts only 1 yoctoNEAR
    docsLink: '/docs/examples#intents-ark',
  },
  {
    type: 'proxy',
    name: 'ZFT Token (Init)',
    args: '{"outlayer_contract":"{{contractId}}","project_id":"zavodil2.testnet/private-ft","total_supply":"1000000000000000000000000000","name":"ZFT Token","symbol":"ZFT","decimals":18,"icon":null}',
    description: '🪙 Initialize ZFT private token. Mints total_supply to your wallet (NEAR_SENDER_ID).\n\n⚠️ Can only be called ONCE. After initialization, use Balance and Transfer methods.\n\n🔗 Project: zavodil2.testnet/private-ft',
    networks: ['testnet'],
    proxyContractIdTestnet: 'zft.testnet',
    proxyMethod: 'new',
    proxyDeposit: '200000000000000000000000', // 0.2 NEAR (for OutLayer execution)
    proxyGas: '300000000000000', // 300 TGas
    docsLink: '/docs/examples#private-token-ark',
  },
  {
    type: 'proxy',
    name: 'ZFT Token (Balance)',
    args: '{"account_id":"zavodil2.testnet"}',
    description: '🪙 Check ZFT private token balance. Balances are stored on OutLayer, not on-chain.\n\nReplace account_id with the account you want to check.\n\n🔗 Project: zavodil2.testnet/private-ft',
    networks: ['testnet'],
    proxyContractIdTestnet: 'zft.testnet',
    proxyMethod: 'ft_balance_of',
    proxyDeposit: '100000000000000000000000', // 0.1 NEAR
    proxyGas: '300000000000000', // 300 TGas
    docsLink: '/docs/examples#private-token-ark',
  },
  {
    type: 'proxy',
    name: 'ZFT Token (Transfer)',
    args: '{"receiver_id":"zavodil.testnet","amount":"1000000000000000000","memo":"test transfer"}',
    description: '🪙 Transfer ZFT private tokens. Sender is your wallet account (NEAR_SENDER_ID).\n\n📋 Amount is in smallest units (18 decimals). 1 ZFT = 1000000000000000000.\n\n🔗 Project: zavodil2.testnet/private-ft',
    networks: ['testnet'],
    proxyContractIdTestnet: 'zft.testnet',
    proxyMethod: 'ft_transfer',
    proxyDeposit: '100000000000000000000000', // 0.1 NEAR
    proxyGas: '300000000000000', // 300 TGas
    docsLink: '/docs/examples#private-token-ark',
  },
  {
    type: 'proxy',
    name: 'ZFT Token (Transfer Call)',
    args: '{"receiver_id":"zavodil.testnet","amount":"1000000000000000000","memo":"test transfer call","msg":"hello from ZFT"}',
    description: '🪙 Transfer ZFT tokens and call receiver contract (NEP-141 ft_transfer_call).\n\n📋 This method:\n1. Transfers tokens to receiver\n2. Calls ft_on_transfer on receiver contract with your msg\n3. Handles refunds if receiver returns unused tokens\n\n🔗 Project: zavodil2.testnet/private-ft',
    networks: ['testnet'],
    proxyContractIdTestnet: 'zft.testnet',
    proxyMethod: 'ft_transfer_call',
    proxyDeposit: '100000000000000000000000', // 0.1 NEAR
    proxyGas: '300000000000000', // 300 TGas
    docsLink: '/docs/examples#private-token-ark',
  },
];

// Combine all presets
const PRESETS: Preset[] = [...DIRECT_PRESETS, ...PROXY_PRESETS];

function PlaygroundContent() {
  const { accountId, isConnected, connect, signAndSendTransaction, network, contractId, viewMethod, stablecoin, shouldReopenModal, clearReopenModal } = useNearWallet();
  const searchParams = useSearchParams();

  // Filter presets by current network
  const availablePresets = PRESETS.filter(preset =>
    preset.networks && preset.networks.includes(network)
  );

  // Check for preset in URL params
  const presetFromUrl = searchParams.get('preset');
  const initialPreset = presetFromUrl
    ? availablePresets.find(p => p.name === presetFromUrl) || availablePresets[0]
    : availablePresets[0];

  // Initialize with preset from URL or first preset
  const [selectedPreset, setSelectedPreset] = useState<string>(initialPreset?.name || '');
  const [codeSourceType, setCodeSourceType] = useState<'github' | 'wasmurl' | 'project'>(initialPreset?.type === 'direct' ? (initialPreset.codeSourceType || 'github') : 'github');
  const [repo, setRepo] = useState(initialPreset?.type === 'direct' ? initialPreset.repo : '');
  const [commit, setCommit] = useState(initialPreset?.type === 'direct' ? initialPreset.commit : '');
  const [wasmUrl, setWasmUrl] = useState('');
  const [wasmHash, setWasmHash] = useState('');
  const [hashLoading, setHashLoading] = useState(false);
  const [hashError, setHashError] = useState<string | null>(null);
  const [buildTarget, setBuildTarget] = useState(initialPreset?.type === 'direct' ? initialPreset.buildTarget : 'wasm32-wasip1');
  const [args, setArgs] = useState(initialPreset?.args || '');
  const [responseFormat, setResponseFormat] = useState(initialPreset?.type === 'direct' ? initialPreset.responseFormat : 'Json');
  const [secretsProfile, setSecretsProfile] = useState(initialPreset?.type === 'direct' ? initialPreset.secretsProfile || '' : '');
  const [secretsOwner, setSecretsOwner] = useState('');
  // Execution params
  const [compileOnly, setCompileOnly] = useState(initialPreset?.type === 'direct' ? initialPreset.compileOnly || false : false);
  const [forceRebuild, setForceRebuild] = useState(initialPreset?.type === 'direct' ? initialPreset.forceRebuild || false : false);
  const [storeOnFastfs, setStoreOnFastfs] = useState(initialPreset?.type === 'direct' ? initialPreset.storeOnFastfs || false : false);

  // Proxy preset state
  const [proxyMethod, setProxyMethod] = useState(initialPreset?.type === 'proxy' ? initialPreset.proxyMethod : '');

  // Project ID for project code source (format: account/name)
  const [projectId, setProjectId] = useState('');
  // Version hash for specific version execution
  const [versionKey, setVersionKey] = useState('');
  // Attached USDC for developer payments
  const [attachedUsdc, setAttachedUsdc] = useState('');
  const [usdcBalance, setUsdcBalance] = useState<string>('0');

  const [loading, setLoading] = useState(false);
  const [result, setResult] = useState<{ transaction: Record<string, unknown>; executionOutput: string | null; transactionHash: string } | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [wasmInfo, setWasmInfo] = useState<{ exists: boolean; checksum: string | null; file_size: number | null; created_at: string | null } | null>(null);
  const [showWalletModal, setShowWalletModal] = useState(false);

  // Auto-open modal if we switched networks
  useEffect(() => {
    if (shouldReopenModal && !isConnected) {
      setShowWalletModal(true);
      clearReopenModal();
    }
  }, [shouldReopenModal, isConnected, clearReopenModal]);

  // Load stablecoin balance for attached_usd feature
  useEffect(() => {
    if (!isConnected || !accountId) {
      setUsdcBalance('0');
      return;
    }
    const loadBalance = async () => {
      try {
        const balance = await viewMethod({
          contractId,
          method: 'get_user_stablecoin_balance',
          args: { account_id: accountId },
        });
        setUsdcBalance(typeof balance === 'string' ? balance : (balance as { toString: () => string })?.toString() || '0');
      } catch {
        setUsdcBalance('0');
      }
    };
    loadBalance();
  }, [isConnected, accountId, contractId, viewMethod]);

  // Load preset from URL on mount and handle hash
  useEffect(() => {
    // Check for preset in query params
    if (presetFromUrl && initialPreset) {
      setSelectedPreset(initialPreset.name);
      setArgs(initialPreset.args);
      if (initialPreset.type === 'direct') {
        setRepo(initialPreset.repo);
        setCommit(initialPreset.commit);
        setBuildTarget(initialPreset.buildTarget);
        setResponseFormat(initialPreset.responseFormat);
        setSecretsProfile(initialPreset.secretsProfile || '');
      }
    }

    // Check for hash in URL (e.g., #random, #ai)
    if (typeof window !== 'undefined' && window.location.hash) {
      const hash = window.location.hash.substring(1); // Remove #
      const presetByHash = availablePresets.find(p => {
        // Generate ID from preset name (lowercase, replace spaces with hyphens)
        const presetId = p.name.toLowerCase().replace(/\s+/g, '-');
        return presetId === hash;
      });

      if (presetByHash) {
        applyPreset(presetByHash.name);
        // Scroll to the preset button after a short delay
        setTimeout(() => {
          const element = document.getElementById(hash);
          if (element) {
            element.scrollIntoView({ behavior: 'smooth', block: 'center' });
          }
        }, 100);
      }
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // Update form when network changes - load first available preset (only if no hash in URL)
  useEffect(() => {
    // Skip if there's a hash in URL (already handled by previous effect)
    if (typeof window !== 'undefined' && window.location.hash) {
      return;
    }

    const firstPreset = availablePresets[0];
    if (firstPreset) {
      setSelectedPreset(firstPreset.name);
      setArgs(firstPreset.args);

      if (firstPreset.type === 'direct') {
        setRepo(firstPreset.repo);
        setCommit(firstPreset.commit);
        setBuildTarget(firstPreset.buildTarget);
        setResponseFormat(firstPreset.responseFormat);
        setSecretsProfile(firstPreset.secretsProfile || '');

        const owner = network === 'testnet'
          ? (firstPreset.secretsOwnerTestnet || '')
          : (firstPreset.secretsOwnerMainnet || '');
        setSecretsOwner(owner);
      } else {
        // Proxy preset - clear direct-only fields
        setRepo('');
        setCommit('');
        setBuildTarget('wasm32-wasip1');
        setResponseFormat('Json');
        setSecretsProfile('');
        setSecretsOwner('');
      }
    }
  }, [network]); // eslint-disable-line react-hooks/exhaustive-deps

  // Update secrets owner when selected preset changes
  useEffect(() => {
    const preset = PRESETS.find(p => p.name === selectedPreset);
    if (preset && preset.type === 'direct') {
      const owner = network === 'testnet'
        ? (preset.secretsOwnerTestnet || '')
        : (preset.secretsOwnerMainnet || '');
      setSecretsOwner(owner);
    }
  }, [network, selectedPreset]);

  // Apply preset configuration
  const applyPreset = (presetName: string) => {
    const preset = PRESETS.find(p => p.name === presetName);
    if (preset) {
      setSelectedPreset(preset.name);
      // Replace {{contractId}} placeholder with actual contract ID
      setArgs(preset.args.replace(/\{\{contractId\}\}/g, contractId));

      if (preset.type === 'direct') {
        setCodeSourceType(preset.codeSourceType || 'github');
        setRepo(preset.repo || '');
        setCommit(preset.commit || '');
        setWasmUrl(preset.wasmUrl || '');
        setWasmHash(preset.wasmHash || '');
        setProjectId(preset.projectId || '');
        setVersionKey(preset.versionKey || '');
        setBuildTarget(preset.buildTarget);
        setResponseFormat(preset.responseFormat);
        setSecretsProfile(preset.secretsProfile || '');
        // Execution params
        setCompileOnly(preset.compileOnly || false);
        setForceRebuild(preset.forceRebuild || false);
        setStoreOnFastfs(preset.storeOnFastfs || false);

        // Select secrets owner based on current network
        const owner = network === 'testnet'
          ? (preset.secretsOwnerTestnet || '')
          : (preset.secretsOwnerMainnet || '');
        setSecretsOwner(owner);
      } else {
        // Proxy preset - set proxy method and clear direct-only fields
        setProxyMethod(preset.proxyMethod);
        setCodeSourceType('github');
        setRepo('');
        setCommit('');
        setWasmUrl('');
        setProjectId('');
        setVersionKey('');
        setBuildTarget('wasm32-wasip1');
        setResponseFormat('Json');
        setSecretsProfile('');
        setSecretsOwner('');
        setCompileOnly(false);
        setForceRebuild(false);
        setStoreOnFastfs(false);
      }

      setWasmInfo(null); // Clear WASM cache info

      // Update URL hash with preset ID
      if (typeof window !== 'undefined') {
        const presetId = preset.name.toLowerCase().replace(/\s+/g, '-');
        window.history.replaceState(null, '', `#${presetId}`);
      }
    }
  };

  const handleCheckWasm = async () => {
    try {
      setError(null);
      const currentPreset = PRESETS.find(p => p.name === selectedPreset);

      // Check if we're using WasmUrl for direct preset
      if (currentPreset?.type === 'direct' && codeSourceType === 'wasmurl') {
        // WasmUrl - check by hash
        if (!wasmHash) {
          setError('Please enter or calculate WASM hash first');
          return;
        }
        const info = await checkWasmExistsByChecksum(wasmHash);
        setWasmInfo(info);
        return;
      }

      let checkRepo: string;
      let checkCommit: string;
      let checkBuildTarget: string;

      if (currentPreset?.type === 'direct') {
        // Direct preset with GitHub - use form values
        if (!repo || !commit) {
          setError('Repository and commit are required for GitHub source');
          return;
        }
        checkRepo = repo;
        checkCommit = commit;
        checkBuildTarget = buildTarget;
      } else if (currentPreset?.type === 'proxy' && currentPreset.wasmRepo) {
        // Proxy preset - use preset's WASM config
        checkRepo = currentPreset.wasmRepo;
        checkCommit = currentPreset.wasmCommit || 'main';
        checkBuildTarget = currentPreset.wasmBuildTarget || 'wasm32-wasip1';
      } else {
        setError('No WASM configuration for this preset');
        return;
      }

      const info = await checkWasmExists(checkRepo, checkCommit, checkBuildTarget);
      setWasmInfo(info);
    } catch (err) {
      setError('Failed to check WASM cache');
      console.error(err);
    }
  };

  // Calculate SHA256 hash from WASM URL
  const calculateWasmHash = async () => {
    if (!wasmUrl) {
      setHashError('Please enter a WASM URL first');
      return;
    }

    setHashLoading(true);
    setHashError(null);

    try {
      // Fetch the WASM file
      const response = await fetch(wasmUrl);
      if (!response.ok) {
        throw new Error(`Failed to fetch: ${response.status} ${response.statusText}`);
      }

      // Get the binary data
      const arrayBuffer = await response.arrayBuffer();

      // Calculate SHA256 using Web Crypto API
      const hashBuffer = await crypto.subtle.digest('SHA-256', arrayBuffer);

      // Convert to hex string
      const hashArray = Array.from(new Uint8Array(hashBuffer));
      const hashHex = hashArray.map(b => b.toString(16).padStart(2, '0')).join('');

      setWasmHash(hashHex);
    } catch (err) {
      const message = err instanceof Error ? err.message : 'Failed to calculate hash';
      setHashError(message);
      console.error('Hash calculation error:', err);
    } finally {
      setHashLoading(false);
    }
  };

  const handleExecute = async () => {
    if (!isConnected) {
      connect();
      return;
    }

    setLoading(true);
    setError(null);
    setResult(null);

    try {
      let action;
      let receiverId;

      const currentPreset = PRESETS.find(p => p.name === selectedPreset);

      if (currentPreset?.type === 'proxy') {
        // PROXY CONTRACT CALL
        // Parse arguments for proxy method
        let proxyArgs;
        try {
          proxyArgs = JSON.parse(args);
        } catch {
          proxyArgs = args;
        }

        // Get contract ID based on network
        const proxyContractId = network === 'testnet'
          ? currentPreset.proxyContractIdTestnet
          : currentPreset.proxyContractIdMainnet;

        if (!proxyContractId) {
          throw new Error(`Proxy contract not available for ${network}`);
        }

        // Calculate deposit (add 0.1 NEAR if WASM not cached and flag is set)
        let finalDeposit = BigInt(currentPreset.proxyDeposit);
        if (currentPreset.increaseDepositIfNoCache && wasmInfo && !wasmInfo.exists) {
          // Add 0.1 NEAR (100000000000000000000000 yoctoNEAR)
          finalDeposit += BigInt('100000000000000000000000');
        }

        action = actionCreators.functionCall(
          proxyMethod,
          proxyArgs,
          BigInt(currentPreset.proxyGas),
          finalDeposit
        );

        receiverId = proxyContractId;
      } else if (codeSourceType === 'project') {
        // PROJECT EXECUTION
        if (!projectId || !projectId.trim()) {
          throw new Error('Please enter a project ID (e.g., account.near/project-name)');
        }

        // Parse input data (optional)
        let inputData = null;
        if (args && args.trim()) {
          try {
            const parsedArgs = JSON.parse(args);
            inputData = JSON.stringify(parsedArgs);
          } catch {
            inputData = args;
          }
        }

        // Build Project source with optional version_key
        const projectSource = {
          Project: {
            project_id: projectId.trim(),
            version_key: (versionKey && versionKey.trim()) ? versionKey.trim() : null,
          },
        };

        // Build secrets_ref if both profile and owner are provided
        let secretsRef = null;
        if (secretsProfile && secretsProfile.trim() && secretsOwner && secretsOwner.trim()) {
          secretsRef = {
            profile: secretsProfile.trim(),
            account_id: secretsOwner.trim(),
          };
        }

        // Calculate attached_usd in minimal units (6 decimals)
        const attachedUsdMinimal = attachedUsdc && parseFloat(attachedUsdc) > 0
          ? BigInt(Math.floor(parseFloat(attachedUsdc) * 10 ** stablecoin.decimals))
          : null;

        const transactionArgs: Record<string, unknown> = {
          source: projectSource,
          resource_limits: {
            max_instructions: 10000000000, // 10B instructions
            max_memory_mb: 128,
            max_execution_seconds: 60,
          },
          input_data: inputData,
          response_format: responseFormat,
        };
        if (secretsRef) {
          transactionArgs.secrets_ref = secretsRef;
        }
        // Add attached_usd for developer payment (only for Project source) - inside params!
        if (attachedUsdMinimal) {
          transactionArgs.params = {
            attached_usd: attachedUsdMinimal.toString(),
          };
        }

        action = actionCreators.functionCall(
          'request_execution',
          transactionArgs,
          BigInt('300000000000000'), // 300 TGas
          BigInt('100000000000000000000000') // 0.1 NEAR
        );

        receiverId = contractId;
      } else {
        // DIRECT OUTLAYER CALL (github or wasmurl)
        // Parse arguments (optional)
        let inputData = null;
        if (args && args.trim()) {
          try {
            const parsedArgs = JSON.parse(args);
            inputData = JSON.stringify(parsedArgs);
          } catch {
            console.log('Invalid JSON in arguments field', args);
            inputData = args;
          }
        }

        // Build secrets_ref if both profile and owner are provided
        let secretsRef = null;
        if (secretsProfile && secretsProfile.trim() && secretsOwner && secretsOwner.trim()) {
          secretsRef = {
            profile: secretsProfile.trim(),
            account_id: secretsOwner.trim(),
          };
        }

        // Build source based on type (ExecutionSource enum)
        const executionSource = codeSourceType === 'github'
          ? {
              GitHub: {
                repo,
                commit,
                build_target: buildTarget,
              },
            }
          : {
              WasmUrl: {
                url: wasmUrl,
                hash: wasmHash,
                build_target: buildTarget || null,
              },
            };

        // Build params if any are set
        const params = (compileOnly || forceRebuild || storeOnFastfs)
          ? {
              compile_only: compileOnly || undefined,
              force_rebuild: forceRebuild || undefined,
              store_on_fastfs: storeOnFastfs || undefined,
            }
          : null;

        // Prepare transaction arguments
        const transactionArgs = {
          source: executionSource,
          resource_limits: {
            max_instructions: 10000000000, // 10B instructions
            max_memory_mb: 128,
            max_execution_seconds: 60,
          },
          input_data: inputData,
          secrets_ref: secretsRef,
          response_format: responseFormat,
          params,
        };

        action = actionCreators.functionCall(
          'request_execution',
          transactionArgs,
          BigInt('300000000000000'), // 300 TGas
          BigInt('100000000000000000000000') // 0.1 NEAR
        );

        receiverId = contractId;
      }

      // Prepare transaction for wallet selector
      const transaction = {
        receiverId,
        actions: [action],
      };

      const txResult = await signAndSendTransaction(transaction);

      // Extract execution result from transaction (don't parse, show as-is)
      let executionOutput = null;
      try {
        // Check if transaction has return value
        if (txResult?.status?.SuccessValue !== undefined) {
          const returnValue = Buffer.from(txResult.status.SuccessValue, 'base64').toString();
          if (returnValue) {
            // Show the raw JSON output from contract (already in correct format)
            executionOutput = returnValue;
          }
        }
      } catch (e) {
        console.error('Failed to decode execution output:', e);
      }

      setResult({
        transaction: txResult,
        executionOutput,
        transactionHash: txResult?.transaction?.hash,
      });
    } catch (err: unknown) {
      setError((err as Error).message || 'Transaction failed');
      console.error(err);
    } finally {
      setLoading(false);
    }
  };

  return (
    <div className="max-w-4xl mx-auto">
      <div className="sm:flex sm:items-center">
        <div className="sm:flex-auto">
          <h1 className="text-3xl font-bold text-gray-900">Playground</h1>
          <p className="mt-2 text-sm text-gray-700">
            Test off-chain execution with your GitHub repository
          </p>
        </div>
      </div>

      <div className="mt-8 bg-white shadow sm:rounded-lg">
        <div className="px-4 py-5 sm:p-6">
          {/* Current Network & Contract Info */}
          <div className="mb-6 p-3 bg-gray-50 rounded-md">
            <div className="text-sm text-gray-600">
              <span className="font-medium">Network:</span> {network === 'testnet' ? 'Testnet' : 'Mainnet'}
              {' '} | {' '}
              <span className="font-medium">Contract:</span> <span className="font-mono">{contractId}</span>
            </div>
          </div>

          {/* Preset Selector */}
          {availablePresets.length > 0 && (
            <div className="mb-6">
              <label htmlFor="presetSelector" className="block text-sm font-medium text-gray-700 mb-1">
                Example Presets
              </label>
              <select
                id="presetSelector"
                value={selectedPreset}
                onChange={(e) => applyPreset(e.target.value)}
                className="mt-1 block w-full rounded-md border-gray-300 shadow-sm focus:border-blue-500 focus:ring-blue-500 sm:text-sm px-3 py-2"
              >
                <optgroup label="Direct Execution (call OutLayer)">
                  {availablePresets
                    .filter(p => p.type === 'direct')
                    .map((preset) => (
                      <option key={preset.name} value={preset.name}>
                        {preset.name}
                      </option>
                    ))}
                </optgroup>
                {availablePresets.filter(p => p.type === 'proxy').length > 0 && (
                  <optgroup label="Via Proxy Contracts">
                    {availablePresets
                      .filter(p => p.type === 'proxy')
                      .map((preset) => (
                        <option key={preset.name} value={preset.name}>
                          {preset.name}
                        </option>
                      ))}
                  </optgroup>
                )}
              </select>
              <p className="mt-1 text-xs text-gray-500">
                Select a preset to auto-fill the form with example values
              </p>

              {/* Show description for selected preset */}
              {(() => {
                const preset = availablePresets.find(p => p.name === selectedPreset);
                if (!preset?.description) return null;

                // Convert URLs to clickable links
                const formatDescription = (text: string) => {
                  // Split by URLs
                  const urlRegex = /(https?:\/\/[^\s]+)/g;
                  const parts = text.split(urlRegex);

                  return parts.map((part, index) => {
                    if (part.match(urlRegex)) {
                      return (
                        <a
                          key={index}
                          href={part}
                          target="_blank"
                          rel="noopener noreferrer"
                          className="text-blue-600 hover:text-blue-800 underline"
                        >
                          {part}
                        </a>
                      );
                    }
                    return part;
                  });
                };

                return (
                  <div className="mt-3 p-3 bg-yellow-50 border border-yellow-200 rounded-md">
                    <div className="text-sm text-yellow-800 whitespace-pre-line">
                      {formatDescription(preset.description)}
                    </div>
                    {preset.docsLink && (
                      <div className="mt-3">
                        <a
                          href={preset.docsLink}
                          target="_blank"
                          rel="noopener noreferrer"
                          className="inline-flex items-center gap-2 text-sm font-medium text-blue-600 hover:text-blue-800 hover:underline"
                        >
                          📖 View detailed documentation
                          <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M10 6H6a2 2 0 00-2 2v10a2 2 0 002 2h10a2 2 0 002-2v-4M14 4h6m0 0v6m0-6L10 14" />
                          </svg>
                        </a>
                      </div>
                    )}
                  </div>
                );
              })()}
            </div>
          )}

          {/* Show proxy contract info for proxy presets */}
          {(() => {
            const currentPreset = PRESETS.find(p => p.name === selectedPreset);
            return currentPreset?.type === 'proxy' ? (
              <>
                {/* Contract ID */}
                <div className="mb-6">
                  <label className="block text-sm font-medium text-gray-700">
                    Contract ID
                  </label>
                  <div className="mt-1 block w-full rounded-md border border-gray-300 bg-gray-50 px-3 py-2 text-sm text-gray-600 font-mono">
                    {network === 'testnet'
                      ? currentPreset.proxyContractIdTestnet
                      : currentPreset.proxyContractIdMainnet}
                  </div>
                </div>

                {/* Method Name */}
                <div className="mb-6">
                  <label htmlFor="proxyMethod" className="block text-sm font-medium text-gray-700">
                    Method Name
                  </label>
                  <input
                    type="text"
                    id="proxyMethod"
                    value={proxyMethod}
                    onChange={(e) => setProxyMethod(e.target.value)}
                    placeholder="method_name"
                    className="mt-1 block w-full rounded-md border-gray-300 shadow-sm focus:border-blue-500 focus:ring-blue-500 sm:text-sm font-mono px-3 py-2"
                  />
                </div>
              </>
            ) : null;
          })()}

          {/* Show these fields only for direct execution presets */}
          {(() => {
            const currentPreset = PRESETS.find(p => p.name === selectedPreset);
            return currentPreset?.type === 'direct' ? (
              <>
                {/* Code Source Type Selector */}
                <div className="mb-6">
                  <label className="block text-sm font-medium text-gray-700 mb-2">
                    Code Source
                  </label>
                  <div className="flex gap-4 flex-wrap">
                    <label className="inline-flex items-center">
                      <input
                        type="radio"
                        name="codeSourceType"
                        value="github"
                        checked={codeSourceType === 'github'}
                        onChange={(e) => setCodeSourceType(e.target.value as 'github' | 'wasmurl' | 'project')}
                        className="form-radio h-4 w-4 text-blue-600"
                      />
                      <span className="ml-2 text-sm text-gray-700">GitHub Repository</span>
                    </label>
                    <label className="inline-flex items-center">
                      <input
                        type="radio"
                        name="codeSourceType"
                        value="wasmurl"
                        checked={codeSourceType === 'wasmurl'}
                        onChange={(e) => setCodeSourceType(e.target.value as 'github' | 'wasmurl' | 'project')}
                        className="form-radio h-4 w-4 text-blue-600"
                      />
                      <span className="ml-2 text-sm text-gray-700">WASM URL</span>
                    </label>
                    <label className="inline-flex items-center">
                      <input
                        type="radio"
                        name="codeSourceType"
                        value="project"
                        checked={codeSourceType === 'project'}
                        onChange={(e) => setCodeSourceType(e.target.value as 'github' | 'wasmurl' | 'project')}
                        className="form-radio h-4 w-4 text-blue-600"
                      />
                      <span className="ml-2 text-sm text-gray-700">Project</span>
                    </label>
                  </div>
                </div>

                {codeSourceType === 'project' ? (
                  /* Project input */
                  <>
                    <div className="mb-6">
                      <label htmlFor="projectId" className="block text-sm font-medium text-gray-700">
                        Project ID
                      </label>
                      <input
                        type="text"
                        id="projectId"
                        value={projectId}
                        onChange={(e) => setProjectId(e.target.value)}
                        placeholder="account.near/project-name"
                        className="mt-1 block w-full rounded-md border-gray-300 shadow-sm focus:border-blue-500 focus:ring-blue-500 sm:text-sm px-3 py-2"
                      />
                      <p className="mt-1 text-xs text-gray-500">
                        Enter your project ID in format: owner_account/project_name (e.g., alice.near/my-app)
                      </p>
                    </div>
                    <div className="mb-6">
                      <label htmlFor="versionKey" className="block text-sm font-medium text-gray-700">
                        Version Key (Optional)
                      </label>
                      <input
                        type="text"
                        id="versionKey"
                        value={versionKey}
                        onChange={(e) => setVersionKey(e.target.value)}
                        placeholder="Leave empty for active version"
                        className="mt-1 block w-full rounded-md border-gray-300 shadow-sm focus:border-blue-500 focus:ring-blue-500 sm:text-sm font-mono px-3 py-2"
                      />
                      <p className="mt-1 text-xs text-gray-500">
                        For WasmUrl: use hash. For GitHub: use &quot;repo@commit&quot;. Leave empty for active version.
                      </p>
                    </div>
                    {/* Attached USDC for developer payment */}
                    <div className="mb-6">
                      <label htmlFor="attachedUsdc" className="block text-sm font-medium text-gray-700">
                        Attached {stablecoin.symbol} (Optional)
                      </label>
                      <div className="mt-1 flex gap-2 items-center">
                        <div className="relative flex-1">
                          <span className="absolute left-3 top-2 text-gray-500">$</span>
                          <input
                            type="number"
                            id="attachedUsdc"
                            min="0"
                            step="0.01"
                            value={attachedUsdc}
                            onChange={(e) => setAttachedUsdc(e.target.value)}
                            placeholder="0.00"
                            className="block w-full pl-7 pr-3 py-2 rounded-md border-gray-300 shadow-sm focus:border-blue-500 focus:ring-blue-500 sm:text-sm"
                          />
                        </div>
                        <span className="text-sm text-gray-500">
                          Balance: ${(parseInt(usdcBalance || '0') / 10 ** stablecoin.decimals).toFixed(2)}
                        </span>
                      </div>
                      <p className="mt-1 text-xs text-gray-500">
                        Payment to project developer. Deposit {stablecoin.symbol} at{' '}
                        <a href="/workspace" className="text-blue-600 hover:underline">My Workspace</a> first.
                      </p>
                    </div>
                  </>
                ) : codeSourceType === 'github' ? (
                  <>
                    {/* GitHub Repository */}
                    <div className="mb-6">
                      <div className="flex items-center justify-between mb-1">
                        <label htmlFor="repo" className="block text-sm font-medium text-gray-700">
                          GitHub Repository
                        </label>
                        {repo && (
                          <a
                            href={`${repo}`}
                            target="_blank"
                            rel="noopener noreferrer"
                            className="inline-flex items-center gap-1.5 px-3 py-1 text-xs font-medium text-gray-700 bg-white border border-gray-300 rounded-md hover:bg-gray-50 transition-colors"
                          >
                            <svg className="w-3.5 h-3.5" fill="currentColor" viewBox="0 0 24 24">
                              <path fillRule="evenodd" d="M12 2C6.477 2 2 6.484 2 12.017c0 4.425 2.865 8.18 6.839 9.504.5.092.682-.217.682-.483 0-.237-.008-.868-.013-1.703-2.782.605-3.369-1.343-3.369-1.343-.454-1.158-1.11-1.466-1.11-1.466-.908-.62.069-.608.069-.608 1.003.07 1.531 1.032 1.531 1.032.892 1.53 2.341 1.088 2.91.832.092-.647.35-1.088.636-1.338-2.22-.253-4.555-1.113-4.555-4.951 0-1.093.39-1.988 1.029-2.688-.103-.253-.446-1.272.098-2.65 0 0 .84-.27 2.75 1.026A9.564 9.564 0 0112 6.844c.85.004 1.705.115 2.504.337 1.909-1.296 2.747-1.027 2.747-1.027.546 1.379.202 2.398.1 2.651.64.7 1.028 1.595 1.028 2.688 0 3.848-2.339 4.695-4.566 4.943.359.309.678.92.678 1.855 0 1.338-.012 2.419-.012 2.747 0 .268.18.58.688.482A10.019 10.019 0 0022 12.017C22 6.484 17.522 2 12 2z" clipRule="evenodd"/>
                            </svg>
                            Fork It!
                          </a>
                        )}
                      </div>
                      <input
                        type="text"
                        id="repo"
                        value={repo}
                        onChange={(e) => setRepo(e.target.value)}
                        placeholder="https://github.com/user/repo"
                        className="mt-1 block w-full rounded-md border-gray-300 shadow-sm focus:border-blue-500 focus:ring-blue-500 sm:text-sm px-3 py-2"
                      />
                    </div>

                    {/* Commit/Branch */}
                    <div className="mb-6">
                      <label htmlFor="commit" className="block text-sm font-medium text-gray-700">
                        Commit Hash or Branch
                      </label>
                      <input
                        type="text"
                        id="commit"
                        value={commit}
                        onChange={(e) => setCommit(e.target.value)}
                        placeholder="main"
                        className="mt-1 block w-full rounded-md border-gray-300 shadow-sm focus:border-blue-500 focus:ring-blue-500 sm:text-sm px-3 py-2"
                      />
                    </div>

                  </>
                ) : (
                  /* WASM URL inputs */
                  <>
                    <div className="mb-6">
                      <label htmlFor="wasmUrl" className="block text-sm font-medium text-gray-700">
                        WASM URL
                      </label>
                      <input
                        type="text"
                        id="wasmUrl"
                        value={wasmUrl}
                        onChange={(e) => setWasmUrl(e.target.value)}
                        placeholder="https://example.com/compiled.wasm or ipfs://..."
                        className="mt-1 block w-full rounded-md border-gray-300 shadow-sm focus:border-blue-500 focus:ring-blue-500 sm:text-sm px-3 py-2"
                      />
                      <p className="mt-1 text-xs text-gray-500">
                        Direct URL to pre-compiled WASM file (HTTP/HTTPS or IPFS)
                      </p>
                    </div>
                    <div className="mb-6">
                      <label htmlFor="wasmHash" className="block text-sm font-medium text-gray-700">
                        WASM Hash (SHA256)
                      </label>
                      <div className="mt-1 flex gap-2">
                        <input
                          type="text"
                          id="wasmHash"
                          value={wasmHash}
                          onChange={(e) => setWasmHash(e.target.value)}
                          placeholder="e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
                          className="block w-full rounded-md border-gray-300 shadow-sm focus:border-blue-500 focus:ring-blue-500 sm:text-sm font-mono px-3 py-2"
                        />
                        <button
                          type="button"
                          onClick={calculateWasmHash}
                          disabled={hashLoading || !wasmUrl}
                          className="inline-flex items-center px-3 py-2 border border-gray-300 shadow-sm text-sm font-medium rounded-md text-gray-700 bg-white hover:bg-gray-50 focus:outline-none focus:ring-2 focus:ring-offset-2 focus:ring-blue-500 disabled:opacity-50 disabled:cursor-not-allowed whitespace-nowrap"
                        >
                          {hashLoading ? (
                            <>
                              <svg className="animate-spin -ml-1 mr-2 h-4 w-4 text-gray-500" xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24">
                                <circle className="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" strokeWidth="4"></circle>
                                <path className="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z"></path>
                              </svg>
                              Calculating...
                            </>
                          ) : (
                            'Calculate'
                          )}
                        </button>
                      </div>
                      {hashError && (
                        <p className="mt-1 text-xs text-red-600">{hashError}</p>
                      )}
                      <p className="mt-1 text-xs text-gray-500">
                        SHA256 hash for verification (hex encoded, 64 characters). Click &quot;Calculate&quot; to auto-fill from URL.
                      </p>
                    </div>
                  </>
                )}

                {/* Build Target and Response Format in columns - hide Build Target for project */}
                <div className={`mb-6 grid ${codeSourceType === 'project' ? 'grid-cols-1' : 'grid-cols-2'} gap-4`}>
                  {codeSourceType !== 'project' && (
                    <div>
                      <label htmlFor="buildTarget" className="block text-sm font-medium text-gray-700">
                        Build Target
                      </label>
                      <select
                        id="buildTarget"
                        value={buildTarget}
                        onChange={(e) => setBuildTarget(e.target.value)}
                        className="mt-1 block w-full rounded-md border-gray-300 shadow-sm focus:border-blue-500 focus:ring-blue-500 sm:text-sm px-3 py-2"
                      >
                        <option value="wasm32-wasip1">wasm32-wasip1</option>
                        <option value="wasm32-wasip2">wasm32-wasip2</option>
                      </select>
                    </div>
                  )}
                  <div>
                    <label htmlFor="responseFormat" className="block text-sm font-medium text-gray-700">
                      Response Format
                    </label>
                    <select
                      id="responseFormat"
                      value={responseFormat}
                      onChange={(e) => setResponseFormat(e.target.value)}
                      className="mt-1 block w-full rounded-md border-gray-300 shadow-sm focus:border-blue-500 focus:ring-blue-500 sm:text-sm px-3 py-2"
                    >
                      <option value="Json">JSON</option>
                      <option value="Text">Text</option>
                      <option value="Bytes">Bytes</option>
                    </select>
                  </div>
                </div>

                {/* Execution Parameters - collapsible, only for GitHub sources */}
                {codeSourceType === 'github' && (
                <details className="mb-6" open={compileOnly || forceRebuild || storeOnFastfs}>
                  <summary className="cursor-pointer text-sm font-medium text-gray-700 hover:text-gray-900">
                    Execution Parameters {(compileOnly || forceRebuild || storeOnFastfs) && <span className="text-blue-600">(modified)</span>}
                  </summary>
                  <div className="mt-3 space-y-2 pl-4">
                    <label className="inline-flex items-center">
                      <input
                        type="checkbox"
                        checked={compileOnly}
                        onChange={(e) => setCompileOnly(e.target.checked)}
                        className="form-checkbox h-4 w-4 text-blue-600 rounded"
                      />
                      <span className="ml-2 text-sm text-gray-700">
                        Compile Only <span className="text-gray-500">(returns checksum)</span>
                      </span>
                    </label>
                    <br />
                    <label className="inline-flex items-center">
                      <input
                        type="checkbox"
                        checked={forceRebuild}
                        onChange={(e) => setForceRebuild(e.target.checked)}
                        className="form-checkbox h-4 w-4 text-blue-600 rounded"
                      />
                      <span className="ml-2 text-sm text-gray-700">
                        Force Rebuild <span className="text-gray-500">(recompile even if cached)</span>
                      </span>
                    </label>
                    <br />
                    <label className="inline-flex items-center">
                      <input
                        type="checkbox"
                        checked={storeOnFastfs}
                        onChange={(e) => setStoreOnFastfs(e.target.checked)}
                        className="form-checkbox h-4 w-4 text-blue-600 rounded"
                      />
                      <span className="ml-2 text-sm text-gray-700">
                        Store on FastFS <span className="text-gray-500">(publish to permanent storage)</span>
                      </span>
                    </label>
                  </div>
                </details>
                )}
              </>
            ) : null;
          })()}

          {/* Arguments / Input Data */}
          <div className="mb-6">
            <label htmlFor="args" className="block text-sm font-medium text-gray-700">
              {(() => {
                const currentPreset = PRESETS.find(p => p.name === selectedPreset);
                return currentPreset?.type === 'proxy' ? 'Method Arguments (JSON)' : 'Input Data (JSON) - Optional';
              })()}
            </label>
            <textarea
              id="args"
              value={args}
              onChange={(e) => setArgs(e.target.value)}
              placeholder='{"key": "value"}'
              rows={4}
              className="mt-1 block w-full rounded-md border-gray-300 shadow-sm focus:border-blue-500 focus:ring-blue-500 sm:text-sm font-mono px-3 py-2"
            />
            <p className="mt-1 text-xs text-gray-500">
              {(() => {
                const currentPreset = PRESETS.find(p => p.name === selectedPreset);
                return currentPreset?.type === 'proxy' ? 'Arguments for the contract method call' : 'Leave empty for no input data';
              })()}
            </p>
          </div>

          {/* Deposit and Gas - only for proxy presets */}
          {(() => {
            const currentPreset = PRESETS.find(p => p.name === selectedPreset);
            if (currentPreset?.type !== 'proxy') return null;

            // Convert yoctoNEAR to NEAR for display
            let depositYocto = BigInt(currentPreset.proxyDeposit);

            // Add 0.1 NEAR if WASM not cached and flag is set
            const shouldIncreaseDeposit = currentPreset.increaseDepositIfNoCache &&
                                          wasmInfo &&
                                          !wasmInfo.exists;
            if (shouldIncreaseDeposit) {
              depositYocto += BigInt('100000000000000000000000');
            }

            const depositNear = Number(depositYocto) / 1e24;
            const depositDisplay = depositYocto === BigInt(1)
              ? '1 yoctoNEAR'
              : `${depositNear} NEAR${shouldIncreaseDeposit ? ' (includes +0.1 NEAR for compilation)' : ''}`;

            // Format gas for display
            const gasAmount = currentPreset.proxyGas;
            const gasTGas = Number(gasAmount) / 1e12;
            const gasDisplay = `${gasTGas} TGas`;

            return (
              <>
                {/* Deposit */}
                <div className="mb-6">
                  <label className="block text-sm font-medium text-gray-700">
                    Attached Deposit
                  </label>
                  <div className="mt-1 block w-full rounded-md border border-gray-300 bg-gray-50 px-3 py-2 text-sm text-gray-600">
                    {depositDisplay}
                  </div>
                  <p className="mt-2 text-xs text-gray-500">
                    💡 Deposit covers possible costs for execution and compilation. Unused resources will be refunded at the end of the transaction.
                  </p>
                </div>

                {/* Gas */}
                <div className="mb-6">
                  <label className="block text-sm font-medium text-gray-700">
                    Gas Amount
                  </label>
                  <div className="mt-1 block w-full rounded-md border border-gray-300 bg-gray-50 px-3 py-2 text-sm text-gray-600">
                    {gasDisplay}
                  </div>
                </div>
              </>
            );
          })()}

          {/* Secrets Reference - only for direct execution */}
          {(() => {
            const currentPreset = PRESETS.find(p => p.name === selectedPreset);
            return currentPreset?.type === 'direct' ? (
              <div className="mb-6">
                <div className="flex items-center justify-between mb-2">
                  <label className="block text-sm font-medium text-gray-700">
                    Secrets Reference (Optional)
                  </label>
                  <a
                    href="/secrets"
                    target="_blank"
                    rel="noopener noreferrer"
                    className="text-sm text-blue-600 hover:text-blue-800 font-medium"
                  >
                    🔐 Manage Secrets →
                  </a>
                </div>
                <div className="grid grid-cols-2 gap-4">
                  <div>
                    <label htmlFor="secretsProfile" className="block text-xs text-gray-600 mb-1">
                      Profile Name
                    </label>
                    <input
                      id="secretsProfile"
                      type="text"
                      value={secretsProfile}
                      onChange={(e) => setSecretsProfile(e.target.value)}
                      placeholder="default"
                      className="block w-full rounded-md border-gray-300 shadow-sm focus:border-blue-500 focus:ring-blue-500 sm:text-sm px-3 py-2"
                    />
                  </div>
                  <div>
                    <label htmlFor="secretsOwner" className="block text-xs text-gray-600 mb-1">
                      Owner Account
                    </label>
                    <input
                      id="secretsOwner"
                      type="text"
                      value={secretsOwner}
                      onChange={(e) => setSecretsOwner(e.target.value)}
                      placeholder="your-account.testnet"
                      className="block w-full rounded-md border-gray-300 shadow-sm focus:border-blue-500 focus:ring-blue-500 sm:text-sm px-3 py-2"
                    />
                  </div>
                </div>
                <p className="mt-2 text-xs text-gray-500">
                  Example: <code className="bg-gray-100 px-1 py-0.5 rounded">profile: &quot;default&quot;, owner: &quot;alice.testnet&quot;</code>
                  {' '}- Store secrets at <a href="/secrets" className="text-blue-600 hover:underline">/secrets</a> page first
                </p>
              </div>
            ) : null;
          })()}

          {/* Execute Button */}
          <div className="mt-6">
            {/* Deposit info for direct execution */}
            {(() => {
              const currentPreset = PRESETS.find(p => p.name === selectedPreset);
              return currentPreset?.type === 'direct' ? (
                <p className="mb-3 text-xs text-gray-500">
                  💡 The attached deposit covers possible costs for execution and compilation. Unused resources will be refunded at the end of the transaction.
                </p>
              ) : null;
            })()}

            <button
              onClick={() => {
                if (!isConnected) {
                  setShowWalletModal(true);
                } else {
                  handleExecute();
                }
              }}
              disabled={loading}
              className="btn-primary w-full inline-flex justify-center items-center px-6 py-3 text-base font-medium rounded-md text-black disabled:bg-gray-400 disabled:text-white"
            >
              {loading ? (
                <>
                  <svg
                    className="animate-spin -ml-1 mr-3 h-5 w-5 text-white"
                    fill="none"
                    viewBox="0 0 24 24"
                  >
                    <circle
                      className="opacity-25"
                      cx="12"
                      cy="12"
                      r="10"
                      stroke="currentColor"
                      strokeWidth="4"
                    />
                    <path
                      className="opacity-75"
                      fill="currentColor"
                      d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z"
                    />
                  </svg>
                  Processing...
                </>
              ) : !isConnected ? (
                'Connect Wallet'
              ) : (
                'Execute'
              )}
            </button>

            {/* Check WASM Cache - for direct execution and proxy with wasmRepo */}
            {(() => {
              const currentPreset = PRESETS.find(p => p.name === selectedPreset);
              const shouldShowButton = currentPreset?.type === 'direct' ||
                                       (currentPreset?.type === 'proxy' && currentPreset.wasmRepo);
              return shouldShowButton ? (
                <div className="mt-3 flex items-center justify-between">
                  <div className="text-xs">
                    {wasmInfo && (
                      wasmInfo.exists ? (
                        <span className="text-green-600">
                          ✓ Cached ({wasmInfo.checksum?.substring(0, 8)}...)
                        </span>
                      ) : (
                        <span className="text-yellow-600">
                          ⚠ Not cached{currentPreset?.type === 'proxy' && currentPreset.increaseDepositIfNoCache ? ' (+0.1 NEAR)' : ''}
                        </span>
                      )
                    )}
                  </div>
                  <button
                    onClick={handleCheckWasm}
                    className="text-sm text-blue-600 hover:text-blue-800 font-medium"
                  >
                    📦 Check WASM Cache →
                  </button>
                </div>
              ) : null;
            })()}
          </div>

          {/* Wallet Connection Modal */}
          <WalletConnectionModal
            isOpen={showWalletModal}
            onClose={() => setShowWalletModal(false)}
          />

          {/* Connected Account */}
          {isConnected && (
            <div className="mt-4 text-sm text-gray-600">
              Connected as: <span className="font-mono">{accountId}</span>
            </div>
          )}

          {/* Error Display */}
          {error && (
            <div className="mt-6 bg-red-50 border border-red-200 rounded-md p-4">
              <p className="text-red-800">{error}</p>
            </div>
          )}

          {/* Result Display */}
          {result && (
            <div className="mt-6 space-y-4">
              {/* Execution Output */}
              {result.executionOutput && (() => {
                // Try to parse JSON to check if execution was successful
                let parsedOutput: { success?: boolean; error?: string; output?: unknown } | null = null;
                try {
                  parsedOutput = JSON.parse(result.executionOutput);
                } catch {
                  // Not JSON, show as-is
                }

                const isError = parsedOutput && parsedOutput.success === false;
                const errorMessage = parsedOutput?.error;
                const actualOutput = parsedOutput?.output;

                if (isError) {
                  return (
                    <div className="bg-red-50 border border-red-200 rounded-md p-4">
                      <h3 className="text-sm font-medium text-red-800 mb-2">❌ Execution Failed</h3>
                      <div className="bg-white rounded p-3 border border-red-300">
                        <pre className="text-sm text-red-900 overflow-auto whitespace-pre-wrap">
                          {errorMessage || result.executionOutput}
                        </pre>
                      </div>
                    </div>
                  );
                }

                // Success case - show output field if available, otherwise raw output
                const displayOutput = actualOutput !== undefined && actualOutput !== null
                  ? (typeof actualOutput === 'string' ? actualOutput : JSON.stringify(actualOutput, null, 2))
                  : result.executionOutput;

                return (
                  <div className="bg-green-50 border border-green-200 rounded-md p-4">
                    <h3 className="text-sm font-medium text-green-800 mb-2">✅ Execution Result</h3>
                    <div className="bg-white rounded p-3 border border-green-300">
                      <pre className="text-sm text-gray-900 overflow-auto whitespace-pre-wrap">
                        {displayOutput}
                      </pre>
                    </div>
                  </div>
                );
              })()}

              {/* Transaction Details */}
              <div className="bg-blue-50 border border-blue-200 rounded-md p-4">
                <h3 className="text-sm font-medium text-blue-800 mb-2">Transaction Details</h3>
                {result.transactionHash && (
                  <p className="text-xs text-blue-700 mb-2">
                    Hash: <a
                      href={getTransactionUrl(result.transactionHash, network)}
                      target="_blank"
                      rel="noopener noreferrer"
                      className="bg-blue-100 px-1 py-0.5 rounded hover:bg-blue-200 underline font-mono"
                    >
                      {result.transactionHash}
                    </a>
                  </p>
                )}
                <details className="text-xs">
                  <summary className="cursor-pointer text-blue-700 hover:text-blue-900">
                    View full transaction data
                  </summary>
                  <pre className="mt-2 text-blue-700 overflow-auto bg-white p-2 rounded border border-blue-300">
                    {JSON.stringify(result.transaction, null, 2)}
                  </pre>
                </details>
                <p className="mt-3 text-sm text-blue-700">
                  Check execution status in the <a href="/executions" className="underline font-medium">Executions</a> page
                </p>
              </div>
            </div>
          )}
        </div>
      </div>
    </div>
  );
}

export default function PlaygroundPage() {
  return (
    <Suspense fallback={<div className="max-w-7xl mx-auto">Loading...</div>}>
      <PlaygroundContent />
    </Suspense>
  );
}
