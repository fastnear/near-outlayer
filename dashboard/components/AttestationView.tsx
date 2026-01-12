'use client';

import { useState } from 'react';
import { AttestationResponse } from '@/lib/api';
import { getTransactionUrl } from '@/lib/explorer';
import { NetworkType } from '@/contexts/NearWalletContext';

interface AttestationViewProps {
  attestation: AttestationResponse;
  network: NetworkType;
  showHelp?: boolean;
  onToggleHelp?: () => void;
  isModal?: boolean;
}

export default function AttestationView({
  attestation,
  network,
  showHelp = false,
  onToggleHelp,
  isModal = false
}: AttestationViewProps) {
  // Validation states
  const [ioValidation, setIoValidation] = useState<{
    inputData: string;
    outputData: string;
    inputHash: string;
    outputHash: string;
    inputMatch: boolean | null;
    outputMatch: boolean | null;
    loading: boolean;
    error: string | null;
  } | null>(null);

  const [quoteValidation, setQuoteValidation] = useState<{
    quote: string;
    extractedRtmr3: string;
    expectedRtmr3: string;
    extractedTaskHash: string;
    expectedTaskHash: string;
    rtmr3Match: boolean;
    taskHashMatch: boolean;
    error: string | null;
  } | null>(null);

  // Helper functions
  const formatRtmr3 = (rtmr3: string): string => {
    return rtmr3.replace(/0+$/, '') || rtmr3;
  };

  // Escape string for Python byte literal b"..."
  const escapePyStr = (s: string | undefined | null): string => {
    if (!s) return '';
    return s.replace(/\\/g, '\\\\').replace(/"/g, '\\"');
  };

  const calculateTaskHash = async (att: AttestationResponse): Promise<string> => {
    const parts: Uint8Array[] = [];
    const encoder = new TextEncoder();

    parts.push(encoder.encode(att.task_type));

    const task_id_buffer = new ArrayBuffer(8);
    const task_id_view = new DataView(task_id_buffer);
    task_id_view.setBigInt64(0, BigInt(att.task_id), true);
    parts.push(new Uint8Array(task_id_buffer));

    if (att.repo_url) parts.push(encoder.encode(att.repo_url));
    if (att.commit_hash) parts.push(encoder.encode(att.commit_hash));
    if (att.build_target) parts.push(encoder.encode(att.build_target));
    if (att.wasm_hash) parts.push(encoder.encode(att.wasm_hash));
    if (att.input_hash) parts.push(encoder.encode(att.input_hash));

    parts.push(encoder.encode(att.output_hash));

    if (att.block_height) {
      const bh_buffer = new ArrayBuffer(8);
      const bh_view = new DataView(bh_buffer);
      bh_view.setBigUint64(0, BigInt(att.block_height), true);
      parts.push(new Uint8Array(bh_buffer));
    }

    const totalLength = parts.reduce((sum, part) => sum + part.length, 0);
    const combined = new Uint8Array(totalLength);
    let offset = 0;
    for (const part of parts) {
      combined.set(part, offset);
      offset += part.length;
    }

    const hashBuffer = await crypto.subtle.digest('SHA-256', combined);
    const hashArray = Array.from(new Uint8Array(hashBuffer));
    return hashArray.map(b => b.toString(16).padStart(2, '0')).join('');
  };

  const verifyTdxQuote = (tdxQuoteBase64: string, expectedRtmr3: string, expectedTaskHash: string): {
    valid: boolean;
    extractedRtmr3: string | null;
    extractedTaskHash: string | null;
    error: string | null
  } => {
    try {
      const binaryString = atob(tdxQuoteBase64);
      const bytes = new Uint8Array(binaryString.length);
      for (let i = 0; i < binaryString.length; i++) {
        bytes[i] = binaryString.charCodeAt(i);
      }

      const RTMR3_OFFSET = 256;
      const RTMR3_SIZE = 48;
      const REPORTDATA_OFFSET = 568;
      const REPORTDATA_SIZE = 64;

      if (bytes.length < REPORTDATA_OFFSET + REPORTDATA_SIZE) {
        return { valid: false, extractedRtmr3: null, extractedTaskHash: null, error: 'Quote too short' };
      }

      const rtmr3Bytes = bytes.slice(RTMR3_OFFSET, RTMR3_OFFSET + RTMR3_SIZE);
      const extractedRtmr3 = Array.from(rtmr3Bytes)
        .map(b => b.toString(16).padStart(2, '0'))
        .join('');

      const reportDataBytes = bytes.slice(REPORTDATA_OFFSET, REPORTDATA_OFFSET + 32);
      const extractedTaskHash = Array.from(reportDataBytes)
        .map(b => b.toString(16).padStart(2, '0'))
        .join('');

      const rtmr3Match = extractedRtmr3.toLowerCase() === expectedRtmr3.toLowerCase();
      const taskHashMatch = extractedTaskHash.toLowerCase() === expectedTaskHash.toLowerCase();
      const valid = rtmr3Match && taskHashMatch;

      return { valid, extractedRtmr3, extractedTaskHash, error: null };
    } catch (err) {
      return { valid: false, extractedRtmr3: null, extractedTaskHash: null, error: err instanceof Error ? err.message : 'Verification failed' };
    }
  };

  // Quick verification for display
  const quickVerification = verifyTdxQuote(
    attestation.tdx_quote,
    attestation.worker_measurement,
    ''
  );
  const rtmr3Valid = quickVerification.extractedRtmr3?.toLowerCase() === attestation.worker_measurement.toLowerCase();

  // Share URL for standalone page
  const shareUrl = typeof window !== 'undefined'
    ? `${window.location.origin}/attestation/${attestation.task_id}?network=${network}`
    : '';

  return (
    <div className="space-y-4">
      {/* Help Section */}
      {showHelp && (
        <div className="bg-blue-50 border border-blue-200 rounded-lg p-4">
          <h3 className="text-lg font-semibold text-blue-900 mb-3">Understanding TEE Attestations</h3>
          <div className="space-y-3 text-sm text-blue-900">
            <div>
              <p className="font-semibold mb-1">🔒 What is a TEE Attestation?</p>
              <p className="text-blue-800">
                A TEE (Trusted Execution Environment) attestation is cryptographic proof that your code
                was executed inside a secure Intel TDX hardware enclave.
              </p>
            </div>
            <div>
              <p className="font-semibold mb-1">✅ What Can You Verify?</p>
              <ul className="list-disc list-inside space-y-1 text-blue-800 ml-2">
                <li>Worker Identity (RTMR3) - TEE environment hash</li>
                <li>Task Hash - prevents attestation forgery</li>
                <li>Input/Output - verify against blockchain</li>
                <li>Source Code - exact GitHub commit</li>
              </ul>
            </div>
          </div>
        </div>
      )}

      {/* Quick Verification Status */}
      <div className="bg-blue-50 border border-blue-200 rounded-md p-4">
        <div className="flex justify-between items-start">
          <div className="flex-1">
            <p className="text-blue-800 font-semibold">
              {rtmr3Valid ? '✓' : '⚠️'} RTMR3: {rtmr3Valid ? 'Valid' : 'Invalid'} |
              Task Hash: Click &quot;Verify Quote&quot; below to check
            </p>
            <p className="text-blue-700 text-sm mt-1">Full verification (including task hash with input/output/wasm commitment) is available in the &quot;TDX Quote Verification&quot; section below.</p>
          </div>
          {onToggleHelp && (
            <button
              onClick={onToggleHelp}
              className="ml-3 px-3 py-1 bg-white hover:bg-blue-100 text-blue-700 text-sm font-medium rounded border border-blue-300"
              title="Show help about attestation fields"
            >
              ❓ Help
            </button>
          )}
        </div>
      </div>

      {/* Basic Info */}
      <div className="grid grid-cols-2 gap-4">
        <div>
          <label className="block text-sm font-semibold text-gray-700 mb-1">Task ID</label>
          <div className="bg-gray-50 p-2 rounded border font-mono text-sm">
            {attestation.task_id}
          </div>
        </div>
        <div>
          <label className="block text-sm font-semibold text-gray-700 mb-1">Task Type</label>
          <div className="bg-gray-50 p-2 rounded border font-mono text-sm">
            {attestation.task_type}
          </div>
        </div>
      </div>

      {/* Worker Measurement */}
      <div>
        <label className="block text-sm font-semibold text-gray-700 mb-1">
          Worker Measurement (RTMR3)
        </label>
        <div className="bg-gray-50 p-2 rounded border font-mono text-xs break-all">
          {formatRtmr3(attestation.worker_measurement)}
        </div>
      </div>

      {/* Source Code */}
      {attestation.repo_url && (
        <div>
          <label className="block text-sm font-semibold text-gray-700 mb-1">Source Code</label>
          <div className="bg-gray-50 p-2 rounded border text-sm">
            <a
              href={`${attestation.repo_url}/tree/${attestation.commit_hash}`}
              target="_blank"
              rel="noopener noreferrer"
              className="text-blue-600 hover:underline"
            >
              {attestation.repo_url} @ {attestation.commit_hash}
            </a>
          </div>
        </div>
      )}

      {/* Hashes */}
      <div className="grid grid-cols-2 gap-4">
        {attestation.wasm_hash && (
          <div>
            <label className="block text-sm font-semibold text-gray-700 mb-1">WASM Hash</label>
            <div className="bg-gray-50 p-2 rounded border font-mono text-xs break-all">
              {attestation.wasm_hash}
            </div>
          </div>
        )}
        {attestation.input_hash && (
          <div>
            <label className="block text-sm font-semibold text-gray-700 mb-1">Input Hash</label>
            <div className="bg-gray-50 p-2 rounded border font-mono text-xs break-all">
              {attestation.input_hash}
            </div>
          </div>
        )}
      </div>

      <div>
        <label className="block text-sm font-semibold text-gray-700 mb-1">
          {attestation.task_type === 'compile' ? 'Compiled WASM Hash (Output)' : 'Output Hash'}
        </label>
        <div className="bg-gray-50 p-2 rounded border font-mono text-xs break-all">
          {attestation.output_hash}
        </div>
      </div>

      {/* Input/Output Verification */}
      {attestation.transaction_hash && attestation.task_type === 'execute' && (
        <div className="border-2 border-blue-200 rounded-lg p-4 bg-blue-50">
          <div className="flex justify-between items-center mb-3">
            <h3 className="text-lg font-semibold text-blue-900">Input/Output Verification</h3>
            {!ioValidation && (
              <button
                onClick={async () => {
                  setIoValidation({ inputData: '', outputData: '', inputHash: '', outputHash: '', inputMatch: null, outputMatch: null, loading: true, error: null });
                  try {
                    const { fetchTransaction, extractInputFromTransaction, extractOutputFromTransaction, sha256 } = await import('@/lib/near-rpc');
                    const tx = await fetchTransaction(
                      attestation.transaction_hash!,
                      attestation.caller_account_id || 'unknown',
                      network
                    );
                    const inputData = extractInputFromTransaction(tx) ?? '';
                    const outputData = extractOutputFromTransaction(tx, network);

                    // Check if we found output from outlayer contract
                    if (!outputData) {
                      setIoValidation({
                        inputData: '',
                        outputData: '',
                        inputHash: '',
                        outputHash: '',
                        inputMatch: false,
                        outputMatch: false,
                        loading: false,
                        error: `No output found from outlayer contract (${network === 'testnet' ? 'outlayer.testnet' : 'outlayer.near'}). Transaction may be from a different contract.`
                      });
                      return;
                    }

                    const inputHash = await sha256(inputData);
                    const outputHash = await sha256(outputData);

                    const inputMatch = inputHash === (attestation.input_hash || '');
                    const outputMatch = outputHash === attestation.output_hash;
                    setIoValidation({
                      inputData,
                      outputData,
                      inputHash,
                      outputHash,
                      inputMatch,
                      outputMatch,
                      loading: false,
                      error: null
                    });
                  } catch (err) {
                    setIoValidation({
                      inputData: '',
                      outputData: '',
                      inputHash: '',
                      outputHash: '',
                      inputMatch: null,
                      outputMatch: null,
                      loading: false,
                      error: err instanceof Error ? err.message : 'Unknown error'
                    });
                  }
                }}
                className="px-4 py-2 bg-green-600 hover:bg-green-700 text-white text-sm font-medium rounded"
              >
                🔍 Load & Verify from Blockchain
              </button>
            )}
            {ioValidation && (
              <button
                onClick={() => setIoValidation(null)}
                className="px-3 py-1 bg-gray-400 hover:bg-gray-500 text-white text-sm font-medium rounded"
              >
                Close
              </button>
            )}
          </div>

          {ioValidation && (
            <>
              {ioValidation.loading && (
                <div className="flex justify-center py-4">
                  <div className="animate-spin rounded-full h-8 w-8 border-b-2 border-blue-600"></div>
                </div>
              )}

              {ioValidation.error && (
                <div className="bg-red-50 border border-red-300 rounded p-3 mb-3">
                  <p className="text-red-800 text-sm">⚠️ Error: {ioValidation.error}</p>
                </div>
              )}

              {!ioValidation.loading && !ioValidation.error && (
                <div className="space-y-4">
                  {/* Input Data Section */}
                  <div>
                    <label className="block text-sm font-semibold text-gray-800 mb-1">Input Data</label>
                    <textarea
                      value={ioValidation.inputData}
                      onChange={async (e) => {
                        const newInputData = e.target.value;
                        const { sha256 } = await import('@/lib/near-rpc');
                        const newInputHash = newInputData ? await sha256(newInputData) : '';
                        const newInputMatch = newInputHash === attestation.input_hash;
                        setIoValidation({ ...ioValidation, inputData: newInputData, inputHash: newInputHash, inputMatch: newInputMatch });
                      }}
                      className="w-full h-20 p-2 border border-gray-300 rounded font-mono text-sm"
                      placeholder="Input data from transaction..."
                    />
                    <div className="mt-2 space-y-1">
                      <div className="flex items-start gap-2">
                        <span className="text-xs font-semibold text-gray-600 w-32">Calculated Hash:</span>
                        <span className="text-xs font-mono text-gray-800 break-all flex-1">{ioValidation.inputHash || 'N/A'}</span>
                      </div>
                      <div className="flex items-start gap-2">
                        <span className="text-xs font-semibold text-gray-600 w-32">Attestation Hash:</span>
                        <span className="text-xs font-mono text-gray-800 break-all flex-1">{attestation.input_hash || 'N/A'}</span>
                      </div>
                      <div className={`px-3 py-2 rounded ${ioValidation.inputMatch ? 'bg-green-100 border border-green-300' : 'bg-red-100 border border-red-300'}`}>
                        <span className={`text-sm font-semibold ${ioValidation.inputMatch ? 'text-green-800' : 'text-red-800'}`}>
                          {ioValidation.inputMatch ? '✓ Input Hash Matches' : '✗ Input Hash Mismatch'}
                        </span>
                      </div>
                    </div>
                  </div>

                  {/* Output Data Section */}
                  <div>
                    <label className="block text-sm font-semibold text-gray-800 mb-1">Output Data</label>
                    <textarea
                      value={ioValidation.outputData}
                      onChange={async (e) => {
                        const newOutputData = e.target.value;
                        const { sha256 } = await import('@/lib/near-rpc');
                        const newOutputHash = newOutputData ? await sha256(newOutputData) : '';
                        const newOutputMatch = newOutputHash === attestation.output_hash;
                        setIoValidation({ ...ioValidation, outputData: newOutputData, outputHash: newOutputHash, outputMatch: newOutputMatch });
                      }}
                      className="w-full h-20 p-2 border border-gray-300 rounded font-mono text-sm"
                      placeholder="Output data from transaction..."
                    />
                    <div className="mt-2 space-y-1">
                      <div className="flex items-start gap-2">
                        <span className="text-xs font-semibold text-gray-600 w-32">Calculated Hash:</span>
                        <span className="text-xs font-mono text-gray-800 break-all flex-1">{ioValidation.outputHash || 'N/A'}</span>
                      </div>
                      <div className="flex items-start gap-2">
                        <span className="text-xs font-semibold text-gray-600 w-32">Attestation Hash:</span>
                        <span className="text-xs font-mono text-gray-800 break-all flex-1">{attestation.output_hash || 'N/A'}</span>
                      </div>
                      <div className={`px-3 py-2 rounded ${ioValidation.outputMatch ? 'bg-green-100 border border-green-300' : 'bg-red-100 border border-red-300'}`}>
                        <span className={`text-sm font-semibold ${ioValidation.outputMatch ? 'text-green-800' : 'text-red-800'}`}>
                          {ioValidation.outputMatch ? '✓ Output Hash Matches' : '✗ Output Hash Mismatch'}
                        </span>
                      </div>
                    </div>
                  </div>
                </div>
              )}
            </>
          )}

          {!ioValidation && (
            <p className="text-sm text-gray-700">
              Click the button to fetch transaction data from NEAR archival RPC and verify input/output hashes.
            </p>
          )}
        </div>
      )}

      {/* HTTPS Call Context */}
      {attestation.call_id && (
        <div>
          <label className="block text-sm font-semibold text-gray-700 mb-1">HTTPS Call ID</label>
          <div className="bg-orange-50 p-2 rounded border border-orange-200 font-mono text-sm">
            {attestation.call_id}
          </div>
        </div>
      )}

      {/* Manual I/O Verification for HTTPS Calls */}
      {attestation.call_id && attestation.task_type === 'execute' && (
        <div className="border-2 border-orange-200 rounded-lg p-4 bg-orange-50">
          <div className="flex justify-between items-center mb-3">
            <h3 className="text-lg font-semibold text-orange-900">Manual Input/Output Verification</h3>
            {!ioValidation && (
              <button
                onClick={() => {
                  setIoValidation({
                    inputData: '',
                    outputData: '',
                    inputHash: '',
                    outputHash: '',
                    inputMatch: null,
                    outputMatch: null,
                    loading: false,
                    error: null
                  });
                }}
                className="px-4 py-2 bg-orange-600 hover:bg-orange-700 text-white text-sm font-medium rounded"
              >
                Enter Data to Verify
              </button>
            )}
            {ioValidation && (
              <button
                onClick={() => setIoValidation(null)}
                className="px-3 py-1 bg-gray-400 hover:bg-gray-500 text-white text-sm font-medium rounded"
              >
                Close
              </button>
            )}
          </div>

          {ioValidation && (
            <div className="space-y-4">
              {/* Input Data Section */}
              <div>
                <label className="block text-sm font-semibold text-gray-800 mb-1">Input Data (paste your request body)</label>
                <textarea
                  value={ioValidation.inputData}
                  onChange={async (e) => {
                    const newInputData = e.target.value;
                    const { sha256 } = await import('@/lib/near-rpc');
                    const newInputHash = newInputData ? await sha256(newInputData) : '';
                    const newInputMatch = newInputHash === (attestation.input_hash || '');
                    setIoValidation({ ...ioValidation, inputData: newInputData, inputHash: newInputHash, inputMatch: newInputMatch });
                  }}
                  className="w-full h-20 p-2 border border-gray-300 rounded font-mono text-sm"
                  placeholder="Paste the JSON input you sent to the API..."
                />
                <div className="mt-2 space-y-1">
                  <div className="flex items-start gap-2">
                    <span className="text-xs font-semibold text-gray-600 w-32">Calculated Hash:</span>
                    <span className="text-xs font-mono text-gray-800 break-all flex-1">{ioValidation.inputHash || '(enter data above)'}</span>
                  </div>
                  <div className="flex items-start gap-2">
                    <span className="text-xs font-semibold text-gray-600 w-32">Attestation Hash:</span>
                    <span className="text-xs font-mono text-gray-800 break-all flex-1">{attestation.input_hash || 'N/A'}</span>
                  </div>
                  {ioValidation.inputHash && (
                    <div className={`px-3 py-2 rounded ${ioValidation.inputMatch ? 'bg-green-100 border border-green-300' : 'bg-red-100 border border-red-300'}`}>
                      <span className={`text-sm font-semibold ${ioValidation.inputMatch ? 'text-green-800' : 'text-red-800'}`}>
                        {ioValidation.inputMatch ? '✓ Input Hash Matches' : '✗ Input Hash Mismatch'}
                      </span>
                    </div>
                  )}
                </div>
              </div>

              {/* Output Data Section */}
              <div>
                <label className="block text-sm font-semibold text-gray-800 mb-1">Output Data (paste API response)</label>
                <textarea
                  value={ioValidation.outputData}
                  onChange={async (e) => {
                    const newOutputData = e.target.value;
                    const { sha256 } = await import('@/lib/near-rpc');
                    const newOutputHash = newOutputData ? await sha256(newOutputData) : '';
                    const newOutputMatch = newOutputHash === attestation.output_hash;
                    setIoValidation({ ...ioValidation, outputData: newOutputData, outputHash: newOutputHash, outputMatch: newOutputMatch });
                  }}
                  className="w-full h-20 p-2 border border-gray-300 rounded font-mono text-sm"
                  placeholder="Paste the JSON output you received from the API..."
                />
                <div className="mt-2 space-y-1">
                  <div className="flex items-start gap-2">
                    <span className="text-xs font-semibold text-gray-600 w-32">Calculated Hash:</span>
                    <span className="text-xs font-mono text-gray-800 break-all flex-1">{ioValidation.outputHash || '(enter data above)'}</span>
                  </div>
                  <div className="flex items-start gap-2">
                    <span className="text-xs font-semibold text-gray-600 w-32">Attestation Hash:</span>
                    <span className="text-xs font-mono text-gray-800 break-all flex-1">{attestation.output_hash || 'N/A'}</span>
                  </div>
                  {ioValidation.outputHash && (
                    <div className={`px-3 py-2 rounded ${ioValidation.outputMatch ? 'bg-green-100 border border-green-300' : 'bg-red-100 border border-red-300'}`}>
                      <span className={`text-sm font-semibold ${ioValidation.outputMatch ? 'text-green-800' : 'text-red-800'}`}>
                        {ioValidation.outputMatch ? '✓ Output Hash Matches' : '✗ Output Hash Mismatch'}
                      </span>
                    </div>
                  )}
                </div>
              </div>
            </div>
          )}

          {!ioValidation && (
            <p className="text-sm text-gray-700">
              For HTTPS calls, paste the input you sent and output you received to verify they match the attestation hashes.
            </p>
          )}
        </div>
      )}

      {/* NEAR Transaction Link */}
      {attestation.transaction_hash && (
        <div>
          <label className="block text-sm font-semibold text-gray-700 mb-1">NEAR Transaction</label>
          <div className="bg-gray-50 p-2 rounded border">
            <a
              href={getTransactionUrl(attestation.transaction_hash, network)}
              target="_blank"
              rel="noopener noreferrer"
              className="text-blue-600 hover:underline font-mono text-sm"
            >
              {attestation.transaction_hash}
            </a>
          </div>
        </div>
      )}

      {/* TDX Quote Verification */}
      <div className="border-2 border-purple-200 rounded-lg p-4 bg-purple-50">
        <div className="flex justify-between items-center mb-3">
          <h3 className="text-lg font-semibold text-purple-900">TDX Quote Verification</h3>
          {!quoteValidation && (
            <button
              onClick={async () => {
                const expectedTaskHash = await calculateTaskHash(attestation);
                const result = verifyTdxQuote(
                  attestation.tdx_quote,
                  attestation.worker_measurement,
                  expectedTaskHash
                );
                setQuoteValidation({
                  quote: attestation.tdx_quote,
                  extractedRtmr3: result.extractedRtmr3 || '',
                  expectedRtmr3: attestation.worker_measurement,
                  extractedTaskHash: result.extractedTaskHash || '',
                  expectedTaskHash,
                  rtmr3Match: result.extractedRtmr3?.toLowerCase() === attestation.worker_measurement.toLowerCase(),
                  taskHashMatch: result.extractedTaskHash?.toLowerCase() === expectedTaskHash.toLowerCase(),
                  error: result.error
                });
              }}
              className="px-4 py-2 bg-purple-600 hover:bg-purple-700 text-white text-sm font-medium rounded"
            >
              🔐 Verify Quote (RTMR3 + Task Hash)
            </button>
          )}
          {quoteValidation && (
            <button
              onClick={() => setQuoteValidation(null)}
              className="px-3 py-1 bg-gray-400 hover:bg-gray-500 text-white text-sm font-medium rounded"
            >
              Close
            </button>
          )}
        </div>

        {!quoteValidation && (
          <>
            <label className="block text-sm font-semibold text-gray-800 mb-1">TDX Quote (Base64)</label>
            <textarea
              readOnly
              value={attestation.tdx_quote}
              className="w-full h-24 bg-white p-2 rounded border border-gray-300 font-mono text-xs"
            />
            <p className="text-sm text-gray-700 mt-2">
              Click &quot;Verify Quote&quot; to extract and verify:
              <br />• RTMR3 (worker measurement) - proves which TEE environment executed the code
              <br />• Task Hash (REPORTDATA) - cryptographic commitment to input/output/wasm hashes, prevents attestation forgery
            </p>
          </>
        )}

        {quoteValidation && (
          <div className="space-y-4">
            {/* Quote Input */}
            <div>
              <label className="block text-sm font-semibold text-gray-800 mb-1">TDX Quote (Base64)</label>
              <textarea
                readOnly
                value={quoteValidation.quote}
                className="w-full h-20 p-2 border border-gray-300 rounded font-mono text-xs bg-gray-50"
              />
            </div>

            {/* Extracted RTMR3 */}
            <div>
              <label className="block text-sm font-semibold text-gray-800 mb-1">
                Extracted RTMR3 (Worker Measurement, offset 256)
              </label>
              <div className="bg-white p-2 border border-gray-300 rounded font-mono text-xs break-all">
                {formatRtmr3(quoteValidation.extractedRtmr3) || 'Failed to extract'}
              </div>
              <div className="bg-white p-2 border border-gray-300 rounded font-mono text-xs break-all mt-1">
                <span className="font-semibold">Expected:</span> {formatRtmr3(quoteValidation.expectedRtmr3)}
              </div>
              <div className={`mt-1 px-2 py-1 rounded text-xs ${quoteValidation.rtmr3Match ? 'bg-green-100 text-green-800' : 'bg-red-100 text-red-800'}`}>
                {quoteValidation.rtmr3Match ? '✓ RTMR3 Match' : '✗ RTMR3 Mismatch'}
              </div>
            </div>

            {/* Extracted Task Hash */}
            <div>
              <label className="block text-sm font-semibold text-gray-800 mb-1">
                Extracted Task Hash (REPORTDATA, offset 568)
              </label>
              <div className="bg-white p-2 border border-gray-300 rounded font-mono text-xs break-all">
                {quoteValidation.extractedTaskHash || 'Failed to extract'}
              </div>
              <div className="bg-white p-2 border border-gray-300 rounded font-mono text-xs break-all mt-1">
                <span className="font-semibold">Expected:</span> {quoteValidation.expectedTaskHash}
              </div>
              <div className={`mt-1 px-2 py-1 rounded text-xs ${quoteValidation.taskHashMatch ? 'bg-green-100 text-green-800' : 'bg-red-100 text-red-800'}`}>
                {quoteValidation.taskHashMatch ? '✓ Task Hash Match (contains commitment to input/output/wasm hashes)' : '✗ Task Hash Mismatch'}
              </div>

              {/* Expandable: Show how Task Hash is calculated */}
              <details className="mt-3 bg-purple-100 border border-purple-300 rounded p-3">
                <summary className="cursor-pointer font-semibold text-purple-900 text-sm hover:text-purple-700">
                  📊 Show Task Hash Calculation Steps
                </summary>
                <div className="mt-3 space-y-2 text-xs">
                  <p className="font-semibold text-purple-900">Binary concatenation order (then SHA256):</p>
                  <div className="space-y-1 font-mono bg-white p-2 rounded border border-purple-200">
                    <div className="flex items-start gap-2">
                      <span className="text-purple-700 font-bold min-w-[20px]">1.</span>
                      <div className="flex-1">
                        <span className="text-gray-600">task_type (string):</span>
                        <div className="text-purple-900 break-all">&quot;{attestation.task_type}&quot;</div>
                      </div>
                    </div>
                    <div className="flex items-start gap-2">
                      <span className="text-purple-700 font-bold min-w-[20px]">2.</span>
                      <div className="flex-1">
                        <span className="text-gray-600">task_id (i64, little-endian):</span>
                        <div className="text-purple-900">{attestation.task_id}</div>
                      </div>
                    </div>
                    <div className="flex items-start gap-2">
                      <span className={`font-bold min-w-[20px] ${attestation.repo_url ? 'text-purple-700' : 'text-gray-400'}`}>3.</span>
                      <div className="flex-1">
                        <span className="text-gray-600">repo_url (string, optional):</span>
                        {attestation.repo_url ? (
                          <div className="text-purple-900 break-all">&quot;{attestation.repo_url}&quot;</div>
                        ) : (
                          <div className="text-gray-400 italic">not included (null)</div>
                        )}
                      </div>
                    </div>
                    <div className="flex items-start gap-2">
                      <span className={`font-bold min-w-[20px] ${attestation.commit_hash ? 'text-purple-700' : 'text-gray-400'}`}>4.</span>
                      <div className="flex-1">
                        <span className="text-gray-600">commit_hash (string, optional):</span>
                        {attestation.commit_hash ? (
                          <div className="text-purple-900">&quot;{attestation.commit_hash}&quot;</div>
                        ) : (
                          <div className="text-gray-400 italic">not included (null)</div>
                        )}
                      </div>
                    </div>
                    <div className="flex items-start gap-2">
                      <span className={`font-bold min-w-[20px] ${attestation.build_target ? 'text-purple-700' : 'text-gray-400'}`}>5.</span>
                      <div className="flex-1">
                        <span className="text-gray-600">build_target (string, optional):</span>
                        {attestation.build_target ? (
                          <div className="text-purple-900">&quot;{attestation.build_target}&quot;</div>
                        ) : (
                          <div className="text-gray-400 italic">not included (null)</div>
                        )}
                      </div>
                    </div>
                    <div className="flex items-start gap-2">
                      <span className={`font-bold min-w-[20px] ${attestation.wasm_hash ? 'text-purple-700' : 'text-gray-400'}`}>6.</span>
                      <div className="flex-1">
                        <span className="text-gray-600">wasm_hash (string, optional):</span>
                        {attestation.wasm_hash ? (
                          <div className="text-purple-900 break-all">&quot;{attestation.wasm_hash}&quot;</div>
                        ) : (
                          <div className="text-gray-400 italic">not included (null)</div>
                        )}
                      </div>
                    </div>
                    <div className="flex items-start gap-2">
                      <span className={`font-bold min-w-[20px] ${attestation.input_hash ? 'text-purple-700' : 'text-gray-400'}`}>7.</span>
                      <div className="flex-1">
                        <span className="text-gray-600">input_hash (string, optional):</span>
                        {attestation.input_hash ? (
                          <div className="text-purple-900 break-all">&quot;{attestation.input_hash}&quot;</div>
                        ) : (
                          <div className="text-gray-400 italic">not included (null)</div>
                        )}
                      </div>
                    </div>
                    <div className="flex items-start gap-2">
                      <span className="text-purple-700 font-bold min-w-[20px]">8.</span>
                      <div className="flex-1">
                        <span className="text-gray-600">output_hash (string):</span>
                        <div className="text-purple-900 break-all">&quot;{attestation.output_hash}&quot;</div>
                      </div>
                    </div>
                    <div className="flex items-start gap-2">
                      <span className={`font-bold min-w-[20px] ${attestation.block_height ? 'text-purple-700' : 'text-gray-400'}`}>9.</span>
                      <div className="flex-1">
                        <span className="text-gray-600">block_height (u64, little-endian, optional):</span>
                        {attestation.block_height ? (
                          <div className="text-purple-900">{attestation.block_height}</div>
                        ) : (
                          <div className="text-gray-400 italic">not included (null)</div>
                        )}
                      </div>
                    </div>
                  </div>
                  <div className="mt-2 p-2 bg-purple-50 border border-purple-200 rounded">
                    <div className="text-purple-900 font-semibold">Final Hash (SHA256 of concatenated bytes):</div>
                    <div className="text-purple-800 font-mono text-xs break-all mt-1">{quoteValidation.expectedTaskHash}</div>
                  </div>
                  <p className="text-purple-800 text-xs mt-2">
                    <strong>Note:</strong> Each string is encoded as UTF-8 bytes, numbers are little-endian encoded.
                    The Task Hash binds the TDX Quote to this specific execution, preventing attestation forgery.
                  </p>
                </div>
              </details>

              {/* Python verification code */}
              <details className="mt-3 bg-green-100 border border-green-300 rounded p-3">
                <summary className="cursor-pointer font-semibold text-green-900 text-sm hover:text-green-700">
                  🐍 Python Code to Verify Task Hash
                </summary>
                <div className="mt-3">
                  <p className="text-green-800 text-xs mb-2">
                    Copy and run this Python code locally to verify the task hash calculation:
                  </p>
                  <div className="relative">
                    <pre className="bg-gray-900 text-green-400 p-3 rounded text-xs overflow-x-auto whitespace-pre">{`import hashlib
import struct

data = b""

# 1. task_type (string)
data += b"${escapePyStr(attestation.task_type)}"

# 2. task_id (i64, little-endian)
data += struct.pack("<q", ${attestation.task_id})
${attestation.repo_url ? `
# 3. repo_url (string)
data += b"${escapePyStr(attestation.repo_url)}"
` : `
# 3. repo_url - not included (null)`}
${attestation.commit_hash ? `
# 4. commit_hash (string)
data += b"${escapePyStr(attestation.commit_hash)}"
` : `
# 4. commit_hash - not included (null)`}
${attestation.build_target ? `
# 5. build_target (string)
data += b"${escapePyStr(attestation.build_target)}"
` : `
# 5. build_target - not included (null)`}
${attestation.wasm_hash ? `
# 6. wasm_hash (string, NOT raw bytes!)
data += b"${escapePyStr(attestation.wasm_hash)}"
` : `
# 6. wasm_hash - not included (null)`}
${attestation.input_hash ? `
# 7. input_hash (string, NOT raw bytes!)
data += b"${escapePyStr(attestation.input_hash)}"
` : `
# 7. input_hash - not included (null)`}

# 8. output_hash (string, NOT raw bytes!)
data += b"${escapePyStr(attestation.output_hash)}"
${attestation.block_height ? `
# 9. block_height (u64, little-endian)
data += struct.pack("<Q", ${attestation.block_height})
` : `
# 9. block_height - not included (null)`}

# Calculate SHA256
final_hash = hashlib.sha256(data).hexdigest()
print(f"Task Hash: {final_hash}")
print(f"Expected:  ${quoteValidation.expectedTaskHash}")
print(f"Match: {final_hash == '${quoteValidation.expectedTaskHash}'}")`}</pre>
                    <button
                      onClick={() => {
                        const code = `import hashlib
import struct

data = b""

# 1. task_type (string)
data += b"${escapePyStr(attestation.task_type)}"

# 2. task_id (i64, little-endian)
data += struct.pack("<q", ${attestation.task_id})
${attestation.repo_url ? `
# 3. repo_url (string)
data += b"${escapePyStr(attestation.repo_url)}"
` : `
# 3. repo_url - not included (null)`}
${attestation.commit_hash ? `
# 4. commit_hash (string)
data += b"${escapePyStr(attestation.commit_hash)}"
` : `
# 4. commit_hash - not included (null)`}
${attestation.build_target ? `
# 5. build_target (string)
data += b"${escapePyStr(attestation.build_target)}"
` : `
# 5. build_target - not included (null)`}
${attestation.wasm_hash ? `
# 6. wasm_hash (string, NOT raw bytes!)
data += b"${escapePyStr(attestation.wasm_hash)}"
` : `
# 6. wasm_hash - not included (null)`}
${attestation.input_hash ? `
# 7. input_hash (string, NOT raw bytes!)
data += b"${escapePyStr(attestation.input_hash)}"
` : `
# 7. input_hash - not included (null)`}

# 8. output_hash (string, NOT raw bytes!)
data += b"${escapePyStr(attestation.output_hash)}"
${attestation.block_height ? `
# 9. block_height (u64, little-endian)
data += struct.pack("<Q", ${attestation.block_height})
` : `
# 9. block_height - not included (null)`}

# Calculate SHA256
final_hash = hashlib.sha256(data).hexdigest()
print(f"Task Hash: {final_hash}")
print(f"Expected:  ${quoteValidation.expectedTaskHash}")
print(f"Match: {final_hash == '${quoteValidation.expectedTaskHash}'}")`;
                        navigator.clipboard.writeText(code);
                      }}
                      className="absolute top-2 right-2 px-2 py-1 bg-green-600 hover:bg-green-700 text-white text-xs rounded"
                    >
                      Copy
                    </button>
                  </div>
                </div>
              </details>
            </div>

            {/* Overall Verification Result */}
            <div className={`px-4 py-3 rounded ${quoteValidation.rtmr3Match && quoteValidation.taskHashMatch ? 'bg-green-100 border border-green-300' : 'bg-red-100 border border-red-300'}`}>
              <p className={`font-semibold ${quoteValidation.rtmr3Match && quoteValidation.taskHashMatch ? 'text-green-800' : 'text-red-800'}`}>
                {quoteValidation.rtmr3Match && quoteValidation.taskHashMatch
                  ? '✓ Full Verification Passed! The TDX Quote is valid and matches all expected values.'
                  : '✗ Verification Failed - The attestation may be invalid or tampered'}
              </p>
            </div>
          </div>
        )}
      </div>      
    </div>
  );
}