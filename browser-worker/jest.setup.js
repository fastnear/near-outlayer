/**
 * Jest setup file
 * Ensures WebCrypto is available in test environment
 */

// jsdom should provide crypto, but just in case:
if (typeof global.crypto === 'undefined') {
  const { webcrypto } = await import('node:crypto');
  global.crypto = webcrypto;
}

// Ensure TextEncoder/TextDecoder are available
if (typeof global.TextEncoder === 'undefined') {
  const { TextEncoder, TextDecoder } = await import('node:util');
  global.TextEncoder = TextEncoder;
  global.TextDecoder = TextDecoder;
}

// Ensure performance API is available
if (typeof global.performance === 'undefined') {
  const { performance } = await import('node:perf_hooks');
  global.performance = performance;
}
