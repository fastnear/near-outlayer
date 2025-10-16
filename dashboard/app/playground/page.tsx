'use client';

import { useState } from 'react';
import { useNearWallet } from '@/contexts/NearWalletContext';
import { checkWasmExists } from '@/lib/api';
import { actionCreators } from '@near-js/transactions';

export default function PlaygroundPage() {
  const { accountId, isConnected, connect, signAndSendTransaction, network, contractId, switchNetwork } = useNearWallet();
  const [repo, setRepo] = useState('https://github.com/zavodil/random-ark');
  const [commit, setCommit] = useState('main');
  const [buildTarget, setBuildTarget] = useState('wasm32-wasip1');
  const [args, setArgs] = useState('{"min":1,"max":100}');
  const [responseFormat, setResponseFormat] = useState('Json');
  const [loading, setLoading] = useState(false);
  const [result, setResult] = useState<any>(null);
  const [error, setError] = useState<string | null>(null);
  const [wasmInfo, setWasmInfo] = useState<any>(null);

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
        encrypted_secrets: null,
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
