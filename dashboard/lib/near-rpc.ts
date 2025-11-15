/**
 * NEAR RPC utilities for transaction data fetching
 */

export type NetworkType = 'testnet' | 'mainnet';

/**
 * Get NEAR RPC URL for the given network
 */
function getNearRpcUrl(network: NetworkType): string {
  if (network === 'mainnet') {
    return process.env.NEXT_PUBLIC_MAINNET_RPC_URL || 'https://rpc.mainnet.near.org';
  }
  return process.env.NEXT_PUBLIC_TESTNET_RPC_URL || 'https://rpc.testnet.near.org';
}

/**
 * Transaction outcome structure from NEAR RPC
 */
interface TransactionOutcome {
  receipts_outcome: Array<{
    id: string;
    outcome: {
      status: {
        SuccessValue?: string; // base64 encoded
        SuccessReceiptId?: string;
        Failure?: unknown;
      };
      logs: string[];
      receipt_ids: string[];
      executor_id: string;
    };
  }>;
  transaction: {
    signer_id: string;
    receiver_id: string;
  };
}

/**
 * Fetch transaction data from NEAR RPC
 */
export async function fetchTransaction(
  txHash: string,
  accountId: string,
  network: NetworkType = 'testnet'
): Promise<TransactionOutcome> {
  const rpcUrl = getNearRpcUrl(network);

  const response = await fetch(rpcUrl, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({
      jsonrpc: '2.0',
      id: 'dontcare',
      method: 'EXPERIMENTAL_tx_status',
      params: [txHash, accountId]
    })
  });

  if (!response.ok) {
    throw new Error(`NEAR RPC request failed: ${response.statusText}`);
  }

  const data = await response.json();

  if (data.error) {
    throw new Error(`NEAR RPC error: ${data.error.message || JSON.stringify(data.error)}`);
  }

  return data.result;
}

/**
 * Extract output from transaction (from last successful receipt)
 */
export function extractOutputFromTransaction(tx: TransactionOutcome): string | null {
  // Find last receipt with SuccessValue (this is the final output)
  const receiptsWithOutput = tx.receipts_outcome.filter(
    receipt => receipt.outcome.status.SuccessValue
  );

  if (receiptsWithOutput.length === 0) {
    return null;
  }

  // Get last receipt (final result after all promises)
  const lastReceipt = receiptsWithOutput[receiptsWithOutput.length - 1];
  const outputBase64 = lastReceipt.outcome.status.SuccessValue;

  if (!outputBase64) {
    return null;
  }

  // Decode base64 to string
  try {
    const outputStr = atob(outputBase64);
    return outputStr;
  } catch (e) {
    console.error('Failed to decode output base64:', e);
    return null;
  }
}

/**
 * Extract input data from transaction execution_requested event
 */
export function extractInputFromTransaction(tx: TransactionOutcome): string | null {
  // Search all receipts for execution_requested event
  for (const receipt of tx.receipts_outcome) {
    for (const log of receipt.outcome.logs) {
      if (log.includes('EVENT_JSON:') && log.includes('execution_requested')) {
        try {
          // Parse event log
          const eventJson = log.replace('EVENT_JSON:', '');
          const event = JSON.parse(eventJson);

          if (event.event === 'execution_requested' && event.data && event.data[0]) {
            const requestData = JSON.parse(event.data[0].request_data);
            return requestData.input_data || '';
          }
        } catch (e) {
          console.error('Failed to parse execution_requested event:', e);
        }
      }
    }
  }

  return null;
}

/**
 * Calculate SHA256 hash of string (browser-compatible)
 */
export async function sha256(message: string): Promise<string> {
  // Encode message as UTF-8
  const msgBuffer = new TextEncoder().encode(message);

  // Hash the message
  const hashBuffer = await crypto.subtle.digest('SHA-256', msgBuffer);

  // Convert ArrayBuffer to hex string
  const hashArray = Array.from(new Uint8Array(hashBuffer));
  const hashHex = hashArray.map(b => b.toString(16).padStart(2, '0')).join('');

  return hashHex;
}
