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
      {/* Help toggle button - only if handler provided */}
      {onToggleHelp && (
        <div className="flex justify-end">
          <button
            onClick={onToggleHelp}
            className="px-3 py-1 bg-blue-100 hover:bg-blue-200 text-blue-700 text-sm font-medium rounded"
            title="Show help about attestation fields"
          >
            ❓ Help
          </button>
        </div>
      )}

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
        <p className="text-blue-800 font-semibold">
          {rtmr3Valid ? '✓' : '⚠️'} RTMR3: {rtmr3Valid ? 'Valid' : 'Invalid'} |
          Task Hash: Click &quot;Verify Quote&quot; below to check
        </p>
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
                    const outputData = extractOutputFromTransaction(tx) ?? '';
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
                🔍 Verify from Blockchain
              </button>
            )}
          </div>
          {ioValidation && !ioValidation.loading && !ioValidation.error && (
            <div className={`px-3 py-2 rounded ${ioValidation.inputMatch && ioValidation.outputMatch ? 'bg-green-100' : 'bg-red-100'}`}>
              <span className={`text-sm font-semibold ${ioValidation.inputMatch && ioValidation.outputMatch ? 'text-green-800' : 'text-red-800'}`}>
                {ioValidation.inputMatch ? '✓ Input Match' : '✗ Input Mismatch'} |
                {ioValidation.outputMatch ? ' ✓ Output Match' : ' ✗ Output Mismatch'}
              </span>
            </div>
          )}
        </div>
      )}

      {/* Transaction Link */}
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
              🔐 Verify Quote
            </button>
          )}
        </div>

        {!quoteValidation ? (
          <p className="text-sm text-gray-700">
            Click to verify RTMR3 and Task Hash from TDX quote
          </p>
        ) : (
          <div className={`px-4 py-3 rounded ${quoteValidation.rtmr3Match && quoteValidation.taskHashMatch ? 'bg-green-100' : 'bg-red-100'}`}>
            <p className={`font-semibold ${quoteValidation.rtmr3Match && quoteValidation.taskHashMatch ? 'text-green-800' : 'text-red-800'}`}>
              {quoteValidation.rtmr3Match && quoteValidation.taskHashMatch
                ? '✓ Full Verification Passed!'
                : '✗ Verification Failed'}
            </p>
          </div>
        )}
      </div>

      {/* Share Link - only for non-modal views */}
      {!isModal && shareUrl && (
        <div className="bg-gray-50 rounded-lg p-4 text-center">
          <p className="text-sm text-gray-600 mb-2">Direct link to this attestation:</p>
          <div className="flex justify-center gap-2">
            <button
              onClick={() => {
                navigator.clipboard.writeText(shareUrl);
                alert('Link copied!');
              }}
              className="px-3 py-1 bg-blue-600 hover:bg-blue-700 text-white text-sm rounded"
            >
              📋 Copy Link
            </button>
            <a
              href={shareUrl}
              target="_blank"
              rel="noopener noreferrer"
              className="px-3 py-1 bg-gray-600 hover:bg-gray-700 text-white text-sm rounded"
            >
              🔗 Open in New Tab
            </a>
          </div>
        </div>
      )}
    </div>
  );
}