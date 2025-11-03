import Link from 'next/link';

export default function GettingStartedSection() {
  return (
    <div className="prose max-w-none">
      <h2 className="text-3xl font-bold mb-6 text-[var(--primary-orange)]">Getting Started</h2>

      <div className="space-y-6">
        <section>
          <h3 className="text-xl font-semibold mb-3">What is OutLayer?</h3>
          <p className="text-gray-700 mb-3">
            OutLayer is a verifiable off-chain computation platform for NEAR Protocol. It allows smart contracts
            to execute arbitrary WASM code off-chain using NEAR&apos;s yield/resume mechanism.
          </p>
          <p className="text-gray-700">
            <strong>Think of it as:</strong> Run any code impossible in smart contracts (random numbers, HTTP requests, AI models)
            while keeping <strong>secure settlement on NEAR Layer 1</strong>.
          </p>
        </section>

        <section>
          <h3 className="text-xl font-semibold mb-3">Core Features</h3>
          <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
            <div className="bg-gray-50 p-4 rounded-lg">
              <h4 className="font-semibold text-gray-800 mb-2">⚡ True Off-Chain Execution</h4>
              <p className="text-sm text-gray-700">
                Execute functions impossible in smart contracts: random numbers, HTTP requests, heavy computations, AI inference
              </p>
            </div>

            <div className="bg-gray-50 p-4 rounded-lg">
              <h4 className="font-semibold text-gray-800 mb-2">🔒 Layer 1 Settlement</h4>
              <p className="text-sm text-gray-700">
                Off-chain execution is fast and cheap, but final results settle on NEAR blockchain for security and auditability
              </p>
            </div>

            <div className="bg-gray-50 p-4 rounded-lg">
              <h4 className="font-semibold text-gray-800 mb-2">💰 Dynamic Pricing</h4>
              <p className="text-sm text-gray-700">
                Pay only for actual resources consumed (instructions, time). Unused deposit automatically refunded
              </p>
            </div>

            <div className="bg-gray-50 p-4 rounded-lg">
              <h4 className="font-semibold text-gray-800 mb-2">💵 Flexible Payment</h4>
              <p className="text-sm text-gray-700">
                Choose who pays: let users pay for their requests, or sponsor execution from your contract balance
              </p>
            </div>

            <div className="bg-gray-50 p-4 rounded-lg">
              <h4 className="font-semibold text-gray-800 mb-2">🔐 Encrypted Secrets</h4>
              <p className="text-sm text-gray-700">
                Store API keys with sophisticated access control: whitelists, NEAR balance, FT/NFT ownership, complex logic (AND/OR/NOT)
              </p>
            </div>

            <div className="bg-gray-50 p-4 rounded-lg">
              <h4 className="font-semibold text-gray-800 mb-2">📦 GitHub Integration</h4>
              <p className="text-sm text-gray-700">
                Automatic WASM compilation from public GitHub repos. Reference by branch (always latest) or commit hash (immutable)
              </p>
            </div>

            <div className="bg-gray-50 p-4 rounded-lg">
              <h4 className="font-semibold text-gray-800 mb-2">🛡️ TEE-Ready Architecture</h4>
              <p className="text-sm text-gray-700">
                Built for Trusted Execution Environments with attestation support. Your code runs in secure enclaves
              </p>
            </div>

            <div className="bg-gray-50 p-4 rounded-lg">
              <h4 className="font-semibold text-gray-800 mb-2">📡 Blockchain Context</h4>
              <p className="text-sm text-gray-700">
                Pass blockchain data (block height, account ID, timestamp) as input to your off-chain code
              </p>
            </div>
          </div>
        </section>

        <section>
          <h3 className="text-xl font-semibold mb-3">Quick Example</h3>
          <p className="text-gray-700 mb-3">
            A simple flow: Your smart contract calls <code className="bg-gray-100 px-2 py-1 rounded">outlayer.near</code> (mainnet)
            or <code className="bg-gray-100 px-2 py-1 rounded">outlayer.testnet</code> with a GitHub repository URL.
            OutLayer compiles the code, executes it in a secure environment, and returns the result back to your contract via callback.
          </p>
          <div className="bg-blue-50 border-l-4 border-blue-500 p-4">
            <p className="text-sm text-blue-800">
              <strong>📚 Learn by example:</strong> Check out the <Link href="/docs/dev-guide" className="underline font-medium">Developer Guide</Link> for
              a step-by-step tutorial on building a random number generator that&apos;s impossible to implement in smart contracts alone.
            </p>
          </div>
        </section>

        <section>
          <h3 className="text-xl font-semibold mb-3">Use Cases</h3>
          <ul className="list-disc list-inside space-y-2 text-gray-700">
            <li><strong>Price Oracles:</strong> Fetch real-time prices from multiple sources (crypto, stocks, commodities)</li>
            <li><strong>AI Inference:</strong> Run ML models for predictions, classifications, or recommendations</li>
            <li><strong>Randomness:</strong> Generate verifiable random numbers for gaming or lotteries</li>
            <li><strong>Data Aggregation:</strong> Fetch and process data from external APIs (weather, sports, blockchain indexers)</li>
            <li><strong>Heavy Computation:</strong> Execute compute-intensive tasks beyond smart contract gas limits</li>
          </ul>
        </section>
      </div>
    </div>
  );
}
