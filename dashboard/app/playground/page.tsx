'use client';

import { useState } from 'react';
import { useNearWallet } from '@/contexts/NearWalletContext';
import { checkWasmExists } from '@/lib/api';
import { actionCreators } from '@near-js/transactions';
import { encryptSecrets } from '@/lib/encryption';

// Preset configurations
interface Preset {
  name: string;
  repo: string;
  commit: string;
  buildTarget: string;
  args: string;
  responseFormat: string;
  encryptedSecrets?: string; // Optional encrypted secrets array as JSON string
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
    args: '{"prompt":"What could the NEAR offshore project do?","history":[{"role":"user","content":"Tell me about NEAR"},{"role":"assistant","content":"NEAR is a Layer 1 blockchain..."}],"model_name":"fireworks::accounts/fireworks/models/gpt-oss-120b","openai_endpoint":"https://api.near.ai/v1/chat/completions","max_tokens":16384}',
    responseFormat: 'Text',
    encryptedSecrets: '[65, 74, 1, 216, 62, 160, 23, 21, 111, 91, 45, 97, 149, 158, 151, 121, 173, 13, 180, 53, 246, 89, 235, 165, 179, 247, 198, 81, 126, 18, 143, 11, 102, 74, 116, 212, 89, 128, 57, 113, 93, 117, 19, 77, 179, 248, 179, 67, 236, 88, 227, 32, 222, 85, 228, 163, 177, 234, 239, 29, 38, 17, 196, 31, 79, 10, 34, 225, 24, 177, 61, 57, 73, 70, 95, 18, 150, 247, 183, 68, 189, 2, 163, 127, 147, 65, 201, 174, 152, 250, 253, 94, 80, 58, 156, 14, 9, 57, 9, 202, 21, 221, 1, 29, 71, 125, 41, 120, 143, 231, 164, 83, 251, 77, 165, 122, 228, 46, 207, 160, 189, 236, 226, 106, 65, 28, 215, 14, 88, 90, 18, 170, 87, 178, 116, 47, 89, 125, 19, 73, 190, 160, 160, 69, 211, 21, 172, 18, 136, 9, 204, 147, 178, 209, 226, 126, 78, 20, 209, 7, 75, 34, 41, 237, 41, 153, 16, 53, 82, 105, 50, 78, 129, 228, 230, 71, 255, 116, 209, 58, 227, 17, 236, 244, 166, 161, 235, 16, 93, 55, 144, 53, 89, 48, 0, 187, 44, 155, 110, 21, 92, 111, 22, 101, 251, 157, 182, 68, 225, 118, 218, 62, 196, 43, 249, 128, 129, 219, 138, 110, 114, 3, 223, 61, 124, 49, 6, 227, 15, 180, 14, 46, 119, 91, 26, 21, 247, 137, 240, 12, 211, 21, 245, 47, 198, 23, 232, 167, 179, 243, 236, 74, 120, 33, 186, 77, 0, 52, 108, 224, 15, 154, 38, 47, 10, 53, 82, 73, 191, 161, 186, 14, 225, 82, 247, 60, 132, 26, 227, 249, 189, 253, 192, 76, 107, 42, 131, 82, 109, 13, 34, 235, 20, 131, 51, 119, 68, 117, 86, 102, 143, 148, 128, 11, 206, 126, 176, 60, 207, 24, 227, 182, 185, 253, 221, 75, 55, 44, 143, 65, 84, 13, 47, 250, 93, 128, 57, 50, 83, 127, 64, 24, 250, 229, 226, 16, 191, 7, 166, 126, 154, 75, 186, 246, 224, 168, 131, 15, 58, 125, 215, 88, 15, 90, 126, 177, 79, 218, 110, 104, 1, 42, 77, 14, 190, 172, 162, 69, 178, 69, 243, 35, 197, 15, 239, 154, 242, 180, 239, 29, 103, 40, 149, 28, 91, 15, 43, 212, 89, 212, 10, 126, 103, 127, 17, 75, 165, 184, 183, 0, 251, 88, 182, 0, 239, 58, 216, 230, 145, 209, 239, 29, 38, 17, 196, 29, 95, 11, 39, 248, 18, 139, 56, 40, 108, 56, 71, 116, 232, 180, 187, 14, 225, 82, 247, 60, 246, 89, 166, 154, 242, 246, 220, 81, 105, 40, 186, 77, 0, 52, 108, 184, 75, 222, 102, 108, 0, 42, 77, 24, 250, 229, 226, 16, 191, 7, 166, 126, 154, 75, 187, 241, 229, 170, 131, 6, 62, 121, 222, 91, 11, 88, 126, 212, 89, 147, 116, 33]',
  },
  {
    name: 'Echo Generator',
    repo: 'https://github.com/zavodil/echo-ark',
    commit: 'main',
    buildTarget: 'wasm32-wasip1',
    args: 'Hello, NEARverse!',
    responseFormat: 'Text'
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
  const [encryptedSecrets, setEncryptedSecrets] = useState(PRESETS[0].encryptedSecrets || '');

  const [loading, setLoading] = useState(false);
  const [result, setResult] = useState<any>(null);
  const [error, setError] = useState<string | null>(null);
  const [wasmInfo, setWasmInfo] = useState<any>(null);

  // Encryption modal state
  const [showEncryptModal, setShowEncryptModal] = useState(false);
  const [plaintextSecrets, setPlaintextSecrets] = useState('{"OPENAI_KEY":"sk-..."}');
  const [encrypting, setEncrypting] = useState(false);
  const [encryptError, setEncryptError] = useState<string | null>(null);

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
      setEncryptedSecrets(preset.encryptedSecrets || '');
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

  const handleEncryptSecrets = async () => {
    setEncrypting(true);
    setEncryptError(null);

    try {
      // Get keystore public key from contract
      const viewResult = await fetch(`https://rpc.${network}.near.org`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          jsonrpc: '2.0',
          id: 'dontcare',
          method: 'query',
          params: {
            request_type: 'call_function',
            finality: 'final',
            account_id: contractId,
            method_name: 'get_keystore_pubkey',
            args_base64: btoa('{}'),
          },
        }),
      });

      const viewData = await viewResult.json();

      if (viewData.error) {
        throw new Error(viewData.error.message || 'Failed to fetch public key from contract');
      }

      const resultBytes = viewData.result?.result;
      if (!resultBytes || resultBytes.length === 0) {
        throw new Error('Keystore public key not set in contract. Contact administrator.');
      }

      // Decode result (JSON string with optional public key)
      const resultStr = new TextDecoder().decode(new Uint8Array(resultBytes));
      const pubkeyData = JSON.parse(resultStr);

      if (!pubkeyData) {
        throw new Error('Keystore public key not configured in contract');
      }

      const pubkeyHex = pubkeyData;

      // Encrypt secrets
      const encrypted = await encryptSecrets(pubkeyHex, plaintextSecrets);
      const encryptedJson = JSON.stringify(encrypted);

      // Set encrypted secrets and close modal
      setEncryptedSecrets(encryptedJson);
      setShowEncryptModal(false);
      setPlaintextSecrets('{"OPENAI_KEY":"sk-..."}'); // Reset
    } catch (err: any) {
      setEncryptError(err.message || 'Failed to encrypt secrets');
      console.error('Encryption error:', err);
    } finally {
      setEncrypting(false);
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

      // Parse encrypted secrets if provided
      let encryptedSecretsArray = null;
      if (encryptedSecrets && encryptedSecrets.trim()) {
        try {
          encryptedSecretsArray = JSON.parse(encryptedSecrets);
          if (!Array.isArray(encryptedSecretsArray)) {
            throw new Error('Encrypted secrets must be an array');
          }
        } catch (e) {
          console.error('Invalid encrypted secrets format', e);
          setError('Invalid encrypted secrets format. Expected array like [65, 74, 1, ...]');
          setLoading(false);
          return;
        }
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
        encrypted_secrets: encryptedSecretsArray,
        response_format: responseFormat,
      };

      // Create function call action using actionCreators
      const action = actionCreators.functionCall(
        'request_execution',
        transactionArgs,
        '300000000000000', // 300 TGas
        '100000000000000000000000' // 0.1 NEAR
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
    } catch (err: any) {
      setError(err.message || 'Transaction failed');
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

          {/* Encrypted Secrets */}
          <div className="mb-6">
            <div className="flex items-center justify-between mb-1">
              <label htmlFor="encryptedSecrets" className="block text-sm font-medium text-gray-700">
                Encrypted Secrets (Optional)
              </label>
              <button
                onClick={() => setShowEncryptModal(true)}
                className="text-sm text-blue-600 hover:text-blue-800 font-medium"
              >
                🔐 Encrypt Secrets
              </button>
            </div>
            <textarea
              id="encryptedSecrets"
              value={encryptedSecrets}
              onChange={(e) => setEncryptedSecrets(e.target.value)}
              placeholder='[65, 74, 1, 216, ...]'
              rows={4}
              className="mt-1 block w-full rounded-md border-gray-300 shadow-sm focus:border-blue-500 focus:ring-blue-500 sm:text-sm font-mono"
            />
            <p className="mt-1 text-xs text-gray-500">
              Encrypted array of secrets that will be injected as environment variables into WASM execution
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

      {/* Encryption Modal */}
      {showEncryptModal && (
        <div className="fixed inset-0 bg-black bg-opacity-50 flex items-center justify-center p-4 z-50">
          <div className="bg-white rounded-lg shadow-xl max-w-2xl w-full max-h-[90vh] overflow-y-auto">
            <div className="p-6">
              <div className="flex items-center justify-between mb-4">
                <h2 className="text-xl font-bold text-gray-900">Encrypt Secrets</h2>
                <button
                  onClick={() => setShowEncryptModal(false)}
                  className="text-gray-400 hover:text-gray-600"
                >
                  <svg className="w-6 h-6" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M6 18L18 6M6 6l12 12" />
                  </svg>
                </button>
              </div>

              {/* How it works */}
              <div className="mb-6 p-4 bg-blue-50 border border-blue-200 rounded-md">
                <h3 className="text-sm font-semibold text-blue-900 mb-2">🔐 How Encrypted Secrets Work</h3>
                <ol className="text-xs text-blue-800 space-y-1 list-decimal list-inside">
                  <li>Enter your secrets as JSON (e.g. API keys, tokens)</li>
                  <li>Secrets are encrypted in your browser using keystore&apos;s public key from the smart contract</li>
                  <li>Only the off-chain worker with matching TEE attestation can decrypt them</li>
                  <li>Decrypted secrets are injected as environment variables into WASM execution</li>
                  <li>Your WASM code can access them via <code className="bg-blue-100 px-1">std::env::var(&quot;KEY_NAME&quot;)</code></li>
                </ol>
              </div>

              {/* Input form */}
              <div className="mb-4">
                <label htmlFor="plaintextSecrets" className="block text-sm font-medium text-gray-700 mb-2">
                  Secrets (Plain JSON)
                </label>
                <textarea
                  id="plaintextSecrets"
                  value={plaintextSecrets}
                  onChange={(e) => setPlaintextSecrets(e.target.value)}
                  placeholder='{"OPENAI_KEY":"sk-...","DATABASE_URL":"postgres://..."}'
                  rows={8}
                  className="block w-full rounded-md border-gray-300 shadow-sm focus:border-blue-500 focus:ring-blue-500 sm:text-sm font-mono"
                />
                <p className="mt-2 text-xs text-gray-500">
                  Enter secrets as JSON object. Example: {`{"API_KEY":"your-key","SECRET":"value"}`}
                </p>
              </div>

              {/* Error display */}
              {encryptError && (
                <div className="mb-4 p-3 bg-red-50 border border-red-200 rounded-md">
                  <p className="text-sm text-red-800">{encryptError}</p>
                </div>
              )}

              {/* Action buttons */}
              <div className="flex justify-end space-x-3">
                <button
                  onClick={() => setShowEncryptModal(false)}
                  className="px-4 py-2 border border-gray-300 rounded-md text-sm font-medium text-gray-700 hover:bg-gray-50"
                >
                  Cancel
                </button>
                <button
                  onClick={handleEncryptSecrets}
                  disabled={encrypting}
                  className="px-4 py-2 bg-blue-600 border border-transparent rounded-md text-sm font-medium text-white hover:bg-blue-700 disabled:bg-gray-400"
                >
                  {encrypting ? (
                    <>
                      <svg className="animate-spin -ml-1 mr-2 h-4 w-4 text-white inline" fill="none" viewBox="0 0 24 24">
                        <circle className="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" strokeWidth="4" />
                        <path className="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z" />
                      </svg>
                      Encrypting...
                    </>
                  ) : (
                    '🔐 Encrypt & Use'
                  )}
                </button>
              </div>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
