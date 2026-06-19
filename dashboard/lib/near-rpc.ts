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
 * Get NEAR Archival RPC URL for the given network (for old transactions)
 */
function getNearArchivalRpcUrl(network: NetworkType): string {
  if (network === 'mainnet') {
    return process.env.NEXT_PUBLIC_MAINNET_ARCHIVAL_RPC_URL || 'https://archival-rpc.mainnet.fastnear.com';
  }
  return process.env.NEXT_PUBLIC_TESTNET_ARCHIVAL_RPC_URL || 'https://archival-rpc.testnet.fastnear.com';
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
 * Uses archival RPC for better reliability with old transactions (>2 epochs)
 */
export async function fetchTransaction(
  txHash: string,
  accountId: string,
  network: NetworkType = 'testnet'
): Promise<TransactionOutcome> {
  // Always use archival RPC for transaction lookups (older than 2 epochs)
  const rpcUrl = getNearArchivalRpcUrl(network);

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
    // TIMEOUT_ERROR means transaction not found
    if (data.error.cause?.name === 'TIMEOUT_ERROR') {
      throw new Error(
        `Transaction not found. Please verify the transaction hash is correct. ` +
        `View on NEAR Explorer: https://${network === 'mainnet' ? '' : 'testnet.'}nearblocks.io/txns/${txHash}`
      );
    }
    throw new Error(`NEAR RPC error: ${data.error.message || JSON.stringify(data.error)}`);
  }

  return data.result;
}

/**
 * Extract a NEAR transaction hash from a wallet/RPC error message.
 *
 * near-api-js' JsonRpcProvider (bundled inside the wallet sandboxes
 * near-connect drives) formats RPC errors as
 *   "[-32000] Server error: Transaction <base58hash> doesn't exist"
 * when it broadcasts via broadcast_tx_commit, the RPC times out, and the
 * fallback `tx`/tx_status poll lands — behind a load balancer — on a node
 * that hasn't synced the just-broadcast tx yet. The tx IS on chain; the
 * hash embedded in that message lets us re-fetch the real outcome.
 *
 * The error crosses a postMessage bridge from the wallet iframe, so its
 * structured fields are usually flattened to a string — we scan every
 * plausible carrier (message, data, cause, and the JSON dump).
 */
export function extractTxHashFromError(err: unknown): string | null {
  const candidates: string[] = [];
  if (err instanceof Error) candidates.push(err.message);
  else if (typeof err === 'string') candidates.push(err);
  if (err && typeof err === 'object') {
    const o = err as Record<string, unknown>;
    for (const k of ['message', 'data', 'cause']) {
      if (typeof o[k] === 'string') candidates.push(o[k] as string);
    }
    try {
      candidates.push(JSON.stringify(err));
    } catch {
      // Circular / non-serialisable — the other candidates still apply.
    }
  }

  // base58 alphabet (no 0OIl); a 32-byte hash is 43-44 chars, allow slack.
  const re = /Transaction ([1-9A-HJ-NP-Za-km-z]{32,64}) (?:doesn'?t|does not) exist/i;
  for (const c of candidates) {
    const m = c.match(re);
    if (m) return m[1];
  }
  return null;
}

/**
 * Poll an RPC for a transaction's final outcome, retrying while the node
 * still reports it as unknown / not-yet-synced. Returns the
 * FinalExecutionOutcome (with `.status` and `.transaction.hash`) once
 * available, or null if it never surfaces within the retry budget.
 *
 * Used to recover after a wallet surfaces a false "doesn't exist" failure
 * for a tx that actually landed (see extractTxHashFromError). We poll our
 * own configured RPC (FastNEAR) rather than whatever the wallet sandbox
 * used. The interval is intrinsic to polling — we are waiting for the RPC
 * indexer to catch up, not delaying arbitrarily.
 */
export async function waitForTransactionOutcome(
  txHash: string,
  accountId: string,
  rpcUrl: string,
  { attempts = 12, intervalMs = 1500 }: { attempts?: number; intervalMs?: number } = {}
): Promise<TransactionOutcome | null> {
  for (let i = 0; i < attempts; i++) {
    try {
      const response = await fetch(rpcUrl, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          jsonrpc: '2.0',
          id: 'dontcare',
          method: 'EXPERIMENTAL_tx_status',
          params: [txHash, accountId],
        }),
      });
      const data = await response.json();
      // `status` present means the tx executed (Success or Failure). A
      // not-yet-synced node returns an error instead → keep polling.
      if (data?.result?.status) {
        return data.result;
      }
    } catch {
      // Network blip — retry.
    }
    if (i < attempts - 1) {
      await new Promise((r) => setTimeout(r, intervalMs));
    }
  }
  return null;
}

/**
 * Extract output from transaction (from outlayer contract receipt)
 * @param tx - Transaction outcome
 * @param network - Network type to determine correct contract ID
 */
export function extractOutputFromTransaction(
  tx: TransactionOutcome,
  network: NetworkType = 'testnet'
): string | null {
  // Get the outlayer contract ID based on network
  const outlayerContractId = network === 'testnet'
    ? process.env.NEXT_PUBLIC_TESTNET_CONTRACT_ID || 'outlayer.testnet'
    : process.env.NEXT_PUBLIC_MAINNET_CONTRACT_ID || 'outlayer.near';

  // Find receipt from outlayer contract - this contains the full JSON structure
  const outlayerReceipt = tx.receipts_outcome.find(
    receipt => receipt.outcome.status.SuccessValue &&
              receipt.outcome.executor_id === outlayerContractId
  );

  if (!outlayerReceipt) {
    // No receipt from outlayer contract found
    return null;
  }

  const outputBase64 = outlayerReceipt.outcome.status.SuccessValue;
  if (!outputBase64) {
    return null;
  }

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
