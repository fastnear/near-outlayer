'use client';

import { useState, useEffect } from 'react';
import { useNearWallet } from '@/contexts/NearWalletContext';
import { checkWasmExists } from '@/lib/api';
import { getTransactionUrl } from '@/lib/explorer';
import { actionCreators } from '@near-js/transactions';
import WalletConnectionModal from '@/components/WalletConnectionModal';

// Base preset configuration
interface BasePreset {
  name: string;
  description?: string;
  networks?: ('testnet' | 'mainnet')[];
}

// Direct execution preset (calls OutLayer)
interface DirectPreset extends BasePreset {
  type: 'direct';
  repo: string;
  commit: string;
  buildTarget: string;
  args: string;
  responseFormat: string;
  secretsProfile?: string;
  secretsOwnerTestnet?: string;
  secretsOwnerMainnet?: string;
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
}

type Preset = DirectPreset | ProxyPreset;

// ============================================================================
// Direct Execution Examples (call OutLayer directly)
// ============================================================================
const DIRECT_PRESETS: DirectPreset[] = [
  {
    type: 'direct',
    name: 'Random Number Generator',
    repo: 'https://github.com/zavodil/random-ark',
    commit: 'main',
    buildTarget: 'wasm32-wasip1',
    args: '{"min":1,"max":100}',
    responseFormat: 'Json',
    networks: ['testnet', 'mainnet'],
  },
  {
    type: 'direct',
    name: 'AI Completions',
    repo: 'https://github.com/zavodil/ai-ark',
    commit: 'main',
    buildTarget: 'wasm32-wasip2',
    args: '{"prompt":"What could the NEAR OutLayer project do?","history":[{"role":"user","content":"Tell me about NEAR"},{"role":"assistant","content":"NEAR is a Layer 1 blockchain..."}],"model_name":"fireworks::accounts/fireworks/models/gpt-oss-120b","openai_endpoint":"https://api.near.ai/v1/chat/completions","max_tokens":16384}',
    responseFormat: 'Text',
    secretsProfile: 'default',
    secretsOwnerTestnet: 'zavodil2.testnet',
    secretsOwnerMainnet: 'zavodil.near',
    networks: [],
  },
  {
    type: 'direct',
    name: 'Echo Generator',
    repo: 'https://github.com/zavodil/echo-ark',
    commit: 'main',
    buildTarget: 'wasm32-wasip1',
    args: 'Hello, NEARverse!',
    responseFormat: 'Text',
    networks: ['testnet', 'mainnet'],
  },
  {
    type: 'direct',
    name: 'Multi-Source Data Oracle',
    repo: 'https://github.com/zavodil/oracle-ark',
    commit: '88b72003a06dc8d1972b39240e01aa9c0c7bbe24',
    buildTarget: 'wasm32-wasip2',
    args: '{"requests":[{"id":"eur_usd_rate","sources":[{"name":"custom","custom":{"url":"https://open.er-api.com/v6/latest/EUR","json_path":"rates.USD","value_type":"number","method":"GET","headers":[]}}]},{"id":"near_price","sources":[{"name":"coingecko","id":"near"},{"name":"binance","id":"NEARUSDT"},{"name":"huobi","id":"nearusdt"},{"name":"cryptocom","id":"NEAR_USDT"},{"name":"kucoin","id":"NEAR-USDT"},{"name":"gate","id":"near_usdt"},{"name":"pyth","id":"0xc415de8d2eba7db216527dff4b60e8f3a5311c740dadb233e13e12547e226750"}],"aggregation_method":"median","min_sources_num":4},{"id":"near_last_block_validator","sources":[{"name":"custom","custom":{"url":"https://api.nearblocks.io/v1/blocks/latest?limit=1","json_path":"blocks.0.author_account_id","value_type":"string","method":"GET","headers":[]}}]},{"id":"elden_ring_price_cents","sources":[{"name":"custom","custom":{"url":"https://store.steampowered.com/api/appdetails/?appids=1245620&cc=us","json_path":"1245620.data.price_overview.final","value_type":"number","method":"GET","headers":[]}}]}],"max_price_deviation_percent":10.0}',
    responseFormat: 'Json',
    secretsProfile: 'default',
    secretsOwnerTestnet: 'zavodil2.testnet',
    secretsOwnerMainnet: 'zavodil.near',
    networks: ['testnet', 'mainnet'],
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
    proxyContractIdMainnet: 'coin-flip.near', 
    proxyMethod: 'flip_coin',
    proxyDeposit: '10000000000000000000000', // 0.01 NEAR
    proxyGas: '300000000000000', // 300 TGas
  },
  {
    type: 'proxy',
    name: 'USDC → wNEAR Swap',
    args: '{"receiver_id":"publishintent.near","amount":"20000","msg":"{\\"Swap\\":{\\"token_out\\":\\"wrap.near\\",\\"min_amount_out\\":\\"1000000000000000000\\"}}"}',
    description: '⚠️ Swaps USDC to wNEAR via NEAR Intents protocol.\n\n📋 Before using:\n1. Ensure you have storage_deposit for wNEAR (wrap.near)\n2. Only use whitelisted tokens\n3. Adjust token_out and min_amount_out as needed\n4. This calls ft_transfer_call on USDC token contract\n\n🔗 Contract: https://github.com/zavodil/intents-ark/tree/main/intents-contract',
    networks: ['mainnet'],
    proxyContractIdMainnet: '17208628f84f5d6ad33f0da3bbbeb27ffcb398eac501a31bd6ad2011e36133a1', // USDC mainnet
    proxyMethod: 'ft_transfer_call',
    proxyDeposit: '1', // 1 yoctoNEAR
    proxyGas: '300000000000000', // 300 TGas
  },
];

// Combine all presets
const PRESETS: Preset[] = [...DIRECT_PRESETS, ...PROXY_PRESETS];

export default function PlaygroundPage() {
  const { accountId, isConnected, connect, signAndSendTransaction, network, contractId, shouldReopenModal, clearReopenModal } = useNearWallet();

  // Filter presets by current network
  const availablePresets = PRESETS.filter(preset =>
    preset.networks && preset.networks.includes(network)
  );

  // Initialize with first preset
  const firstPreset = availablePresets[0];
  const [selectedPreset, setSelectedPreset] = useState<string>(firstPreset?.name || '');
  const [repo, setRepo] = useState(firstPreset?.type === 'direct' ? firstPreset.repo : '');
  const [commit, setCommit] = useState(firstPreset?.type === 'direct' ? firstPreset.commit : '');
  const [buildTarget, setBuildTarget] = useState(firstPreset?.type === 'direct' ? firstPreset.buildTarget : 'wasm32-wasip1');
  const [args, setArgs] = useState(firstPreset?.args || '');
  const [responseFormat, setResponseFormat] = useState(firstPreset?.type === 'direct' ? firstPreset.responseFormat : 'Json');
  const [secretsProfile, setSecretsProfile] = useState(firstPreset?.type === 'direct' ? firstPreset.secretsProfile || '' : '');
  const [secretsOwner, setSecretsOwner] = useState('');

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

  // Update form when network changes - load first available preset
  useEffect(() => {
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
      setArgs(preset.args);

      if (preset.type === 'direct') {
        setRepo(preset.repo);
        setCommit(preset.commit);
        setBuildTarget(preset.buildTarget);
        setResponseFormat(preset.responseFormat);
        setSecretsProfile(preset.secretsProfile || '');

        // Select secrets owner based on current network
        const owner = network === 'testnet'
          ? (preset.secretsOwnerTestnet || '')
          : (preset.secretsOwnerMainnet || '');
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

      setWasmInfo(null); // Clear WASM cache info
    }
  };

  const handleCheckWasm = async () => {
    try {
      setError(null);
      const info = await checkWasmExists(repo, commit, buildTarget);
      setWasmInfo(info);
    } catch (err) {
      setError('Failed to check WASM cache');
      console.error(err);
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
      // Check if this is a proxy contract call
      const currentPreset = PRESETS.find(p => p.name === selectedPreset);

      let action;
      let receiverId;

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

        action = actionCreators.functionCall(
          currentPreset.proxyMethod,
          proxyArgs,
          BigInt(currentPreset.proxyGas),
          BigInt(currentPreset.proxyDeposit)
        );

        receiverId = proxyContractId;
      } else {
        // DIRECT OUTLAYER CALL
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

        // Prepare transaction arguments
        const transactionArgs = {
          code_source: {
            repo,
            commit,
            build_target: buildTarget,
          },
          resource_limits: {
            max_instructions: 10000000000, // 10B instructions
            max_memory_mb: 128,
            max_execution_seconds: 60,
          },
          input_data: inputData,
          secrets_ref: secretsRef,
          response_format: responseFormat,
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
              <label className="block text-sm font-medium text-gray-700 mb-3">
                Example Presets
              </label>

              {/* Direct Execution Group */}
              {availablePresets.filter(p => p.type === 'direct').length > 0 && (
                <div className="mb-4">
                  <h3 className="text-xs font-semibold text-gray-600 mb-2 uppercase tracking-wider">
                    Direct Execution (call OutLayer)
                  </h3>
                  <div className="flex flex-wrap gap-2">
                    {availablePresets
                      .filter(p => p.type === 'direct')
                      .map((preset) => (
                        <button
                          key={preset.name}
                          onClick={() => applyPreset(preset.name)}
                          className={`px-4 py-2 rounded-md text-sm font-medium transition-colors ${
                            selectedPreset === preset.name
                              ? 'bg-gradient-to-r from-[#c17817] to-[#d4a017] text-white'
                              : 'bg-gray-100 text-gray-700 hover:bg-gray-200'
                          }`}
                        >
                          {preset.name}
                        </button>
                      ))}
                  </div>
                </div>
              )}

              {/* Proxy Contract Group */}
              {availablePresets.filter(p => p.type === 'proxy').length > 0 && (
                <div className="mb-4">
                  <h3 className="text-xs font-semibold text-gray-600 mb-2 uppercase tracking-wider">
                    Via Proxy Contracts
                  </h3>
                  <div className="flex flex-wrap gap-2">
                    {availablePresets
                      .filter(p => p.type === 'proxy')
                      .map((preset) => (
                        <button
                          key={preset.name}
                          onClick={() => applyPreset(preset.name)}
                          className={`px-4 py-2 rounded-md text-sm font-medium transition-colors ${
                            selectedPreset === preset.name
                              ? 'bg-gradient-to-r from-[#c17817] to-[#d4a017] text-white'
                              : 'bg-gray-100 text-gray-700 hover:bg-gray-200'
                          }`}
                        >
                          {preset.name}
                        </button>
                      ))}
                  </div>
                </div>
              )}

              <p className="mt-2 text-xs text-gray-500">
                Click a preset to auto-fill the form with example values
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
                  <label className="block text-sm font-medium text-gray-700">
                    Method Name
                  </label>
                  <div className="mt-1 block w-full rounded-md border border-gray-300 bg-gray-50 px-3 py-2 text-sm text-gray-600 font-mono">
                    {currentPreset.proxyMethod}
                  </div>
                </div>
              </>
            ) : null;
          })()}

          {/* Show these fields only for direct execution presets */}
          {(() => {
            const currentPreset = PRESETS.find(p => p.name === selectedPreset);
            return currentPreset?.type === 'direct' ? (
              <>
                {/* GitHub Repository */}
                <div className="mb-6">
                  <label htmlFor="repo" className="block text-sm font-medium text-gray-700">
                    GitHub Repository
                  </label>
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

                {/* Build Target */}
                <div className="mb-6">
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

                {/* Response Format */}
                <div className="mb-6">
                  <label htmlFor="responseFormat" className="block text-sm font-medium text-gray-700">
                    Response Format
                  </label>
                  <select
                    id="responseFormat"
                    value={responseFormat}
                    onChange={(e) => setResponseFormat(e.target.value)}
                    className="mt-1 block w-full rounded-md border-gray-300 shadow-sm focus:border-blue-500 focus:ring-blue-500 sm:text-sm px-3 py-2"
                  >
                    <option value="Json">JSON (parse output as JSON)</option>
                    <option value="Text">Text (UTF-8 string)</option>
                    <option value="Bytes">Bytes (raw binary)</option>
                  </select>
                  <p className="mt-1 text-xs text-gray-500">
                    {responseFormat === 'Json' && 'Contract will receive parsed JSON object instead of string'}
                    {responseFormat === 'Text' && 'Contract will receive UTF-8 text string'}
                    {responseFormat === 'Bytes' && 'Contract will receive raw bytes array'}
                  </p>
                </div>
              </>
            ) : null;
          })()}

          {/* Arguments */}
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
            const depositYocto = BigInt(currentPreset.proxyDeposit);
            const depositNear = Number(depositYocto) / 1e24;
            const depositDisplay = depositYocto === BigInt(1)
              ? '1 yoctoNEAR'
              : `${depositNear} NEAR`;

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

          {/* Check WASM Button - only for direct execution */}
          {(() => {
            const currentPreset = PRESETS.find(p => p.name === selectedPreset);
            return currentPreset?.type === 'direct' ? (
              <div className="mb-6">
                <button
                  onClick={handleCheckWasm}
                  className="inline-flex items-center px-4 py-2 border border-gray-300 shadow-sm text-sm font-medium rounded-md text-gray-700 bg-white hover:bg-gray-50"
                >
                  Check WASM Cache
                </button>
                {wasmInfo && (
                  <div className="mt-2 text-sm">
                    {wasmInfo.exists ? (
                      <span className="text-green-600">
                        ✓ WASM exists (checksum: {wasmInfo.checksum?.substring(0, 12)}...)
                      </span>
                    ) : (
                      <span className="text-yellow-600">
                        ⚠ WASM not cached - will be compiled on first execution
                      </span>
                    )}
                  </div>
                )}
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
              {result.executionOutput && (
                <div className="bg-green-50 border border-green-200 rounded-md p-4">
                  <h3 className="text-sm font-medium text-green-800 mb-2">Execution Result</h3>
                  <div className="bg-white rounded p-3 border border-green-300">
                    <pre className="text-sm text-gray-900 overflow-auto whitespace-pre-wrap">
                      {result.executionOutput}
                    </pre>
                  </div>
                </div>
              )}

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
