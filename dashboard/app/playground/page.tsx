'use client';

import { useState } from 'react';
import { useNearWallet } from '@/contexts/NearWalletContext';
import { checkWasmExists } from '@/lib/api';
import { actionCreators } from '@near-js/transactions';

// Preset configurations
interface Preset {
  name: string;
  repo: string;
  commit: string;
  buildTarget: string;
  args: string;
  responseFormat: string;
  secretsProfile?: string; // Optional secrets profile
  secretsOwner?: string; // Optional secrets owner account
}

const PRESETS: Preset[] = [
  {
    name: 'Random Number Generator',
    repo: 'https://github.com/zavodil/random-ark',
    commit: 'main',
    buildTarget: 'wasm32-wasip1',
    args: '{"min":1,"max":100}',
    responseFormat: 'Json',
  },
  {
    name: 'AI Completions',
    repo: 'https://github.com/zavodil/ai-ark',
    commit: 'main',
    buildTarget: 'wasm32-wasip2',
    args: '{"prompt":"What could the NEAR OutLayer project do?","history":[{"role":"user","content":"Tell me about NEAR"},{"role":"assistant","content":"NEAR is a Layer 1 blockchain..."}],"model_name":"fireworks::accounts/fireworks/models/gpt-oss-120b","openai_endpoint":"https://api.near.ai/v1/chat/completions","max_tokens":16384}',
    responseFormat: 'Text',
    secretsProfile: 'default',
    secretsOwner: 'zavodil2.testnet',
  },
  {
    name: 'Echo Generator',
    repo: 'https://github.com/zavodil/echo-ark',
    commit: 'main',
    buildTarget: 'wasm32-wasip1',
    args: 'Hello, NEARverse!',
    responseFormat: 'Text'
  },
  {
    name: 'Multi-Source Data Oracle',
    repo: 'https://github.com/zavodil/oracle-ark',
    commit: '88b72003a06dc8d1972b39240e01aa9c0c7bbe24',
    buildTarget: 'wasm32-wasip2',
    args: '{"requests":[{"id":"eur_usd_rate","sources":[{"name":"custom","custom":{"url":"https://open.er-api.com/v6/latest/EUR","json_path":"rates.USD","value_type":"number","method":"GET","headers":[]}}]},{"id":"near_price","sources":[{"name":"coingecko","id":"near"},{"name":"binance","id":"NEARUSDT"},{"name":"huobi","id":"nearusdt"},{"name":"cryptocom","id":"NEAR_USDT"},{"name":"kucoin","id":"NEAR-USDT"},{"name":"gate","id":"near_usdt"},{"name":"pyth","id":"0xc415de8d2eba7db216527dff4b60e8f3a5311c740dadb233e13e12547e226750"}],"aggregation_method":"median","min_sources_num":4},{"id":"near_last_block_validator","sources":[{"name":"custom","custom":{"url":"https://api.nearblocks.io/v1/blocks/latest?limit=1","json_path":"blocks.0.author_account_id","value_type":"string","method":"GET","headers":[]}}]},{"id":"elden_ring_price_cents","sources":[{"name":"custom","custom":{"url":"https://store.steampowered.com/api/appdetails/?appids=1245620&cc=us","json_path":"1245620.data.price_overview.final","value_type":"number","method":"GET","headers":[]}}]}],"max_price_deviation_percent":10.0}',
    responseFormat: 'Json',
    secretsProfile: 'default',
    secretsOwner: 'zavodil2.testnet',
  },
  // Add more presets here
];

export default function PlaygroundPage() {
  const { accountId, isConnected, connect, signAndSendTransaction, network, contractId, switchNetwork } = useNearWallet();

  // Initialize with first preset
  const [selectedPreset, setSelectedPreset] = useState<string>(PRESETS[0].name);
  const [repo, setRepo] = useState(PRESETS[0].repo);
  const [commit, setCommit] = useState(PRESETS[0].commit);
  const [buildTarget, setBuildTarget] = useState(PRESETS[0].buildTarget);
  const [args, setArgs] = useState(PRESETS[0].args);
  const [responseFormat, setResponseFormat] = useState(PRESETS[0].responseFormat);
  const [secretsProfile, setSecretsProfile] = useState(PRESETS[0].secretsProfile || '');
  const [secretsOwner, setSecretsOwner] = useState(PRESETS[0].secretsOwner || '');

  const [loading, setLoading] = useState(false);
  const [result, setResult] = useState<{ transaction: Record<string, unknown>; executionOutput: string | null; transactionHash: string } | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [wasmInfo, setWasmInfo] = useState<{ exists: boolean; checksum: string | null; file_size: number | null; created_at: string | null } | null>(null);

  // Apply preset configuration
  const applyPreset = (presetName: string) => {
    const preset = PRESETS.find(p => p.name === presetName);
    if (preset) {
      setSelectedPreset(preset.name);
      setRepo(preset.repo);
      setCommit(preset.commit);
      setBuildTarget(preset.buildTarget);
      setArgs(preset.args);
      setResponseFormat(preset.responseFormat);
      setSecretsProfile(preset.secretsProfile || '');
      setSecretsOwner(preset.secretsOwner || '');
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

      // Create function call action using actionCreators
      const action = actionCreators.functionCall(
        'request_execution',
        transactionArgs,
        BigInt('300000000000000'), // 300 TGas
        BigInt('100000000000000000000000') // 0.1 NEAR
      );

      // Prepare transaction for wallet selector
      const transaction = {
        receiverId: contractId,
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
          {/* Network Selector */}
          <div className="mb-6">
            <label className="text-sm font-medium text-gray-700">Network</label>
            <div className="mt-2 flex space-x-4">
              <button
                onClick={() => switchNetwork('testnet')}
                className={`px-4 py-2 rounded-md ${
                  network === 'testnet'
                    ? 'bg-blue-600 text-white'
                    : 'bg-gray-200 text-gray-700'
                }`}
              >
                Testnet
              </button>
              <button
                onClick={() => switchNetwork('mainnet')}
                className={`px-4 py-2 rounded-md ${
                  network === 'mainnet'
                    ? 'bg-blue-600 text-white'
                    : 'bg-gray-200 text-gray-700'
                }`}
              >
                Mainnet
              </button>
            </div>
            <div className="mt-2 text-sm text-gray-600">
              Contract: <span className="font-mono">{contractId}</span>
            </div>
          </div>

          {/* Preset Selector */}
          <div className="mb-6">
            <label className="block text-sm font-medium text-gray-700 mb-2">
              Example Presets
            </label>
            <div className="flex flex-wrap gap-2">
              {PRESETS.map((preset) => (
                <button
                  key={preset.name}
                  onClick={() => applyPreset(preset.name)}
                  className={`px-4 py-2 rounded-md text-sm font-medium transition-colors ${
                    selectedPreset === preset.name
                      ? 'bg-blue-600 text-white'
                      : 'bg-gray-100 text-gray-700 hover:bg-gray-200'
                  }`}
                >
                  {preset.name}
                </button>
              ))}
            </div>
            <p className="mt-2 text-xs text-gray-500">
              Click a preset to auto-fill the form with example values
            </p>
          </div>

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
              className="mt-1 block w-full rounded-md border-gray-300 shadow-sm focus:border-blue-500 focus:ring-blue-500 sm:text-sm"
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
              className="mt-1 block w-full rounded-md border-gray-300 shadow-sm focus:border-blue-500 focus:ring-blue-500 sm:text-sm"
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
              className="mt-1 block w-full rounded-md border-gray-300 shadow-sm focus:border-blue-500 focus:ring-blue-500 sm:text-sm"
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
              className="mt-1 block w-full rounded-md border-gray-300 shadow-sm focus:border-blue-500 focus:ring-blue-500 sm:text-sm"
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

          {/* Arguments */}
          <div className="mb-6">
            <label htmlFor="args" className="block text-sm font-medium text-gray-700">
              Input Data (JSON) - Optional
            </label>
            <textarea
              id="args"
              value={args}
              onChange={(e) => setArgs(e.target.value)}
              placeholder='{"key": "value"}'
              rows={4}
              className="mt-1 block w-full rounded-md border-gray-300 shadow-sm focus:border-blue-500 focus:ring-blue-500 sm:text-sm font-mono"
            />
            <p className="mt-1 text-xs text-gray-500">Leave empty for no input data</p>
          </div>

          {/* Secrets Reference */}
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
                  className="block w-full rounded-md border-gray-300 shadow-sm focus:border-blue-500 focus:ring-blue-500 sm:text-sm"
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
                  className="block w-full rounded-md border-gray-300 shadow-sm focus:border-blue-500 focus:ring-blue-500 sm:text-sm"
                />
              </div>
            </div>
            <p className="mt-2 text-xs text-gray-500">
              Example: <code className="bg-gray-100 px-1 py-0.5 rounded">profile: &quot;default&quot;, owner: &quot;alice.testnet&quot;</code>
              {' '}- Store secrets at <a href="/secrets" className="text-blue-600 hover:underline">/secrets</a> page first
            </p>
          </div>

          {/* Check WASM Button */}
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

          {/* Execute Button */}
          <div className="mt-6">
            <button
              onClick={handleExecute}
              disabled={loading}
              className="w-full inline-flex justify-center items-center px-6 py-3 border border-transparent text-base font-medium rounded-md text-white bg-blue-600 hover:bg-blue-700 disabled:bg-gray-400"
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

          {/* Connected Account */}
          {isConnected && (
            <div className="mt-4 text-sm text-gray-600">
              Connected as: <span className="font-mono">{accountId}</span>
            </div>
          )}

          {/* Cost Estimate */}
          <div className="mt-4 text-sm text-gray-500">
            Estimated cost: ~0.1 NEAR (will be refunded if unused)
          </div>

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
                    Hash: <code className="bg-blue-100 px-1 py-0.5 rounded">{result.transactionHash}</code>
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
