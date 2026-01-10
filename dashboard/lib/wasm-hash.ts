/**
 * Calculate SHA256 hash of a file from URL
 * Uses Web Crypto API for hash calculation
 */
export async function calculateWasmHashFromUrl(url: string): Promise<string> {
  // Fetch the WASM file
  const response = await fetch(url);
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

  return hashHex;
}
