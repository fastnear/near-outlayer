/**
 * Jest configuration for Phase 4 Hermes Enclave tests
 * Kept simple - tests should run fast and be easy to understand
 */

export default {
  // Use jsdom environment for browser APIs (WebCrypto, performance, etc.)
  testEnvironment: 'jsdom',

  // Transform nothing - we're using plain ES6 modules
  transform: {},

  // Test file patterns
  testMatch: [
    '**/__tests__/**/*.test.js',
    '**/?(*.)+(spec|test).js'
  ],

  // Coverage settings (optional but nice to have)
  collectCoverageFrom: [
    'src/frozen-realm.js',
    'src/crypto-utils.js',
    'src/enclave-executor.js'
  ],

  // Verbose output - we want to see what's happening
  verbose: true,

  // Setup file for WebCrypto polyfill if needed
  setupFilesAfterEnv: ['<rootDir>/jest.setup.js']
};
