/**
 * NEAR Explorer utilities
 */

export type NetworkType = 'testnet' | 'mainnet';

/**
 * Get the current network from environment
 */
export function getCurrentNetwork(): NetworkType {
  const network = process.env.NEXT_PUBLIC_DEFAULT_NETWORK;
  return (network === 'mainnet' ? 'mainnet' : 'testnet') as NetworkType;
}

/**
 * Get explorer base URL for the given network
 */
export function getExplorerBaseUrl(network?: NetworkType): string {
  const currentNetwork = network || getCurrentNetwork();

  if (currentNetwork === 'mainnet') {
    return process.env.NEXT_PUBLIC_MAINNET_EXPLORER_URL || 'https://nearblocks.io';
  }

  return process.env.NEXT_PUBLIC_TESTNET_EXPLORER_URL || 'https://testnet.nearblocks.io';
}

/**
 * Get transaction URL for the given transaction hash
 */
export function getTransactionUrl(txHash: string, network?: NetworkType): string {
  const baseUrl = getExplorerBaseUrl(network);
  return `${baseUrl}/txns/${txHash}`;
}

/**
 * Get account URL for the given account ID
 */
export function getAccountUrl(accountId: string, network?: NetworkType): string {
  const baseUrl = getExplorerBaseUrl(network);
  return `${baseUrl}/address/${accountId}`;
}
