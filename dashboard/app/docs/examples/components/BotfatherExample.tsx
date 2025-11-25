import Link from 'next/link';
import { ExampleCard, UseCasesSection, KeyFeaturesSection, CodeExampleSection, HowItWorksSection, TechnicalDetailsSection, SecurityNotesSection } from './index';

export function BotfatherExample() {
  const badges = (
    <>
      <span className="ml-3 text-sm bg-purple-100 text-purple-800 px-3 py-1 rounded">WASI P2</span>
      <span className="ml-2 text-sm bg-orange-100 text-orange-800 px-3 py-1 rounded">Host Functions</span>
      <span className="ml-2 text-sm bg-red-100 text-red-800 px-3 py-1 rounded">Advanced</span>
    </>
  );

  return (
    <ExampleCard
      id="botfather-ark"
      title="botfather-ark"
      badges={badges}
      githubUrl="https://github.com/zavodil/botfather-ark"
    >
      <p className="text-gray-700 mb-4">
        Account factory pattern for NEAR - create and manage multiple NEAR accounts with AI-generated names using hierarchical key derivation. Demonstrates advanced host function usage with <code className="bg-gray-100 px-2 py-1 rounded">call()</code> and <code className="bg-gray-100 px-2 py-1 rounded">transfer()</code>, providing access to private NEAR RPC endpoints (powered by Fastnear).
      </p>

      <UseCasesSection items={[
        <><strong>Account Factory:</strong> Generate multiple NEAR accounts programmatically</>,
        <><strong>Batch Operations:</strong> Execute contract calls on multiple accounts simultaneously (e.g., buy tokens, delegate to staking pool)</>,
        <><strong>Onboarding Platform:</strong> Create named accounts for users via Telegram bots or web apps</>,
        <><strong>Sub-Account Management:</strong> Manage hierarchical account structures with deterministic key derivation</>,
      ]} />

      <KeyFeaturesSection items={[
        <>AI-powered account name generation (GPT integration)</>,
        <>Deterministic key derivation from master seed using SHA-256</>,
        <>Account discovery via Fastnear API</>,
        <>Batch contract calls with template variables (<code className="bg-gray-100 px-2 py-1 rounded">{`{{account_id}}`}</code>)</>,
        <>Fund distribution across multiple accounts</>,
        <>Uses <code className="bg-gray-100 px-2 py-1 rounded">near:rpc/api@0.1.0</code> host functions: <code className="bg-gray-100 px-2 py-1 rounded">call()</code>, <code className="bg-gray-100 px-2 py-1 rounded">transfer()</code></>,
      ]} />

      <h4 className="font-semibold mt-4 mb-2">Actions:</h4>
      <div className="space-y-3">
        <CodeExampleSection
          title="1. Create Accounts:"
          code={`{
  "action": "create_accounts",
  "prompt": "space exploration theme",
  "count": 3,
  "deposit_per_account": "1000000000000000000000000"
}`}
          description={
            <>
              Creates accounts like <code className="bg-gray-100 px-2 py-1 rounded">mars-rover.testnet</code>, <code className="bg-gray-100 px-2 py-1 rounded">moon-base.testnet</code>
            </>
          }
        />

        <CodeExampleSection
          title="2. Fund Accounts:"
          code={`{
  "action": "fund_accounts",
  "total_amount": "30000000000000000000000000",
  "indices": []
}`}
          description={
            <>
              Empty <code className="bg-gray-100 px-2 py-1 rounded">indices</code> = fund all accounts equally (30 NEAR ÷ 3 accounts = 10 NEAR each)
            </>
          }
        />

        <CodeExampleSection
          title="3. Batch Contract Calls:"
          code={`{
  "action": "batch_call",
  "contract_id": "token.near",
  "method_name": "transfer",
  "args": {"receiver_id": "{{account_id}}", "amount": "1000"},
  "deposit": "1",
  "gas": "30000000000000",
  "indices": [0, 2]
}`}
          description={
            <>
              Execute on accounts at indices 0 and 2. Use <code className="bg-gray-100 px-2 py-1 rounded">{`{{account_id}}`}</code> placeholder for dynamic account ID
            </>
          }
        />

        <CodeExampleSection
          title="4. List Accounts:"
          code={`{
  "action": "list_accounts"
}`}
          description="Returns all created accounts with balances and public keys"
        />
      </div>

      <h4 className="font-semibold mt-4 mb-2">Output Example:</h4>
      <CodeExampleSection
        title=""
        code={`{
  "success": true,
  "accounts": [
    {
      "index": 0,
      "account_id": "mars_rover.testnet",
      "public_key": "ed25519:...",
      "balance": "1000000000000000000000000",
      "balance_near": "1.0000"
    }
  ],
  "transactions": [
    {
      "account_id": "mars_rover.testnet",
      "tx_hash": "Abc123...",
      "success": true
    }
  ]
}`}
      />

      <HowItWorksSection items={[
        <><strong>Key Derivation in TEE:</strong> Master key (<code className="bg-white px-2 py-1 rounded text-xs">PROTECTED_MASTER_KEY</code>) is generated inside TEE (nobody ever sees it). Derived keys are created using SHA-256: <code className="bg-white px-2 py-1 rounded text-xs">SHA256(master + sender_id + index)</code> - each derived key controls one account</>,
        <><strong>Account Discovery:</strong> Queries Fastnear API to find existing accounts by derived public keys (stateless operation)</>,
        <><strong>AI Name Generation:</strong> Calls OpenAI API to generate creative account names based on theme prompt</>,
        <><strong>Account Creation:</strong> Uses NEAR RPC <code className="bg-white px-2 py-1 rounded text-xs">call()</code> host function to create accounts via <code className="bg-white px-2 py-1 rounded text-xs">create_account</code> action</>,
        <><strong>Batch Execution:</strong> Iterates through account indices and executes operations using <code className="bg-white px-2 py-1 rounded text-xs">call()</code> or <code className="bg-white px-2 py-1 rounded text-xs">transfer()</code></>,
      ]} />

      <h4 className="font-semibold mt-4 mb-2">Setting Up Secrets</h4>
      <div className="bg-purple-50 border-l-4 border-purple-400 p-4 mb-4">
        <p className="text-sm text-purple-900 font-medium mb-2">
          ⚠️ This example is not available in Playground because it requires creating a secret with <code className="bg-purple-100 px-1 rounded text-xs">NEAR_SENDER_PRIVATE_KEY</code> - your NEAR account&apos;s private key (with NEAR tokens) to pay for account creation and funding.
        </p>
        <p className="text-sm text-gray-700 mb-3">
          To use this example, create a secret in the <Link href="/secrets" className="text-[var(--primary-orange)] hover:underline font-semibold">Secrets page</Link> with the following configuration:
        </p>

        <h5 className="font-semibold text-gray-900 mb-2 text-sm">Manual Secrets:</h5>
        <CodeExampleSection
          title=""
          code={`{
  "NEAR_SENDER_PRIVATE_KEY": "ed25519:your_private_key",
  "OPENAI_API_KEY": "sk-...",
  "OPENAI_ENDPOINT": "https://api.openai.com/v1/chat/completions",
  "OPENAI_MODEL": "gpt-3.5-turbo"
}`}
        />

        <h5 className="font-semibold text-gray-900 mb-2 text-sm">Generated Secret (created in TEE):</h5>
        <ul className="list-disc list-inside text-sm text-gray-700 ml-4 space-y-1 mb-3">
          <li>Click <strong>&quot;Generate Secret&quot;</strong> button</li>
          <li>Secret name: <code className="bg-purple-100 px-1 rounded text-xs">PROTECTED_MASTER_KEY</code></li>
          <li>Type: <code className="bg-purple-100 px-1 rounded text-xs">ED25519</code> (generates ed25519 key pair in TEE)</li>
          <li>This key is created inside TEE and never exposed - used to derive all account keys</li>
        </ul>

        <h5 className="font-semibold text-gray-900 mb-2 text-sm">Access Control:</h5>
        <p className="text-sm text-gray-700 mb-1">
          Set <strong>👥 Whitelist</strong> with your account ID (the one that will manage created accounts)
        </p>
      </div>

      <TechnicalDetailsSection items={[
        <><strong>WASI Version:</strong> Preview 2 (component model)</>,
        <><strong>Language:</strong> Rust</>,
        <><strong>WIT Interface:</strong> <code className="bg-gray-100 px-2 py-1 rounded">near:rpc/api@0.1.0</code> (call, transfer, view)</>,
        <><strong>Secrets:</strong> Required (<code className="bg-gray-100 px-2 py-1 rounded">NEAR_SENDER_PRIVATE_KEY</code>, <code className="bg-gray-100 px-2 py-1 rounded">OPENAI_API_KEY</code>, <code className="bg-gray-100 px-2 py-1 rounded">PROTECTED_MASTER_KEY</code>)</>,
        <><strong>Network:</strong> Required (Fastnear API, OpenAI API, NEAR RPC via host functions)</>,
      ]} />

      <SecurityNotesSection items={[
        <>✅ <strong>WASM provides signer:</strong> User&apos;s NEAR private key is passed via secrets (not worker&apos;s key)</>,
        <>✅ <strong>Keys created in TEE:</strong> Master key (<code className="bg-white px-2 py-1 rounded text-xs">PROTECTED_MASTER_KEY</code>) is generated inside TEE and never leaves it. Derived keys are created using deterministic SHA-256 derivation from master seed.</>,
        <>✅ <strong>Accounts managed only in TEE:</strong> All derived accounts are controlled exclusively by keys that exist only inside TEE - nobody can export or see the private keys</>,
        <>✅ <strong>Deterministic keys:</strong> Same master seed + sender + index always generates same account key</>,
        <>✅ <strong>Master key isolation:</strong> Each <code className="bg-white px-2 py-1 rounded text-xs">NEAR_SENDER_ID</code> has isolated account space</>,
        <>⚠️ <strong>Store master key safely:</strong> Loss of master key = loss of access to all derived accounts</>,
      ]} />
    </ExampleCard>
  );
}
