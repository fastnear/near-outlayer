import Link from 'next/link';
import { ExampleCard, UseCasesSection, KeyFeaturesSection, TechnicalDetailsSection, HowItWorksSection, LearnMoreSection, SecurityNotesSection } from './';

export function NearEmailExample() {
  const badges = (
    <>
      <span className="ml-3 text-sm bg-purple-100 text-purple-800 px-3 py-1 rounded">WASI P2</span>
      <span className="ml-2 text-sm bg-red-100 text-red-800 px-3 py-1 rounded">Production</span>
      <span className="ml-2 text-sm bg-yellow-100 text-yellow-800 px-3 py-1 rounded">Full Stack</span>
    </>
  );

  return (
    <ExampleCard
      id="near-email"
      title="near-email"
      badges={badges}
      githubUrl="https://github.com/zavodil/near-email"
    >
      <p className="text-gray-700 mb-4">
        Blockchain-native email for NEAR accounts. Every NEAR account automatically has an email address:{' '}
        <code>alice.near</code> becomes <code>alice@near.email</code>. Emails are encrypted on receipt and
        can only be decrypted inside the OutLayer TEE by the wallet owner.
      </p>

      <div className="bg-blue-50 border-l-4 border-blue-400 p-4 mb-6">
        <p className="text-sm text-blue-800">
          <strong>Live at:</strong>{' '}
          <a href="https://near.email" target="_blank" rel="noopener noreferrer" className="underline hover:text-blue-600">
            near.email
          </a>
          {' '}&mdash; A full production application demonstrating OutLayer&apos;s TEE capabilities, persistent storage,
          key derivation, and both blockchain and HTTPS API modes.
        </p>
      </div>

      <KeyFeaturesSection items={[
        'Wallet-based identity: your NEAR account = your email',
        'End-to-end encryption: ECIES with secp256k1, emails encrypted before storage',
        'TEE-protected: Intel TDX ensures operators cannot read emails',
        'BIP32-style key derivation from master keypair in TEE',
        'Supports both blockchain mode and Payment Key HTTPS mode',
        'External email compatibility (Gmail, Outlook, etc.)',
        'Up to 18 MB attachments in Payment Key mode'
      ]} />

      <HowItWorksSection items={[
        'External email arrives via SMTP (port 25) to smtp-server',
        'SMTP server derives recipient\'s public key from master public key (no secret needed)',
        'Email encrypted with ECIES and stored in PostgreSQL',
        'User connects wallet on web UI and signs authentication message',
        'OutLayer WASI module (running in TEE) derives private key from master secret',
        'Module decrypts emails and returns them to the user',
        'Master private key never leaves TEE memory'
      ]} />

      <div className="mb-6">
        <h4 className="font-semibold text-gray-900 mb-3">Architecture</h4>
        <pre className="text-sm bg-gray-50 p-4 rounded-lg overflow-x-auto whitespace-pre">
{`External World (Gmail, etc.)
        │ SMTP (port 25)
        ▼
┌─────────────────────────────────┐
│  smtp-server (Rust)             │
│  Derives public key, encrypts   │
│  email, stores encrypted blob   │
└─────────────────────────────────┘
        │
        ▼
┌─────────────────────────────────┐
│  PostgreSQL                     │
│  Encrypted emails only          │
└─────────────────────────────────┘
        │ HTTP API
        ▼
┌─────────────────────────────────┐
│  OutLayer TEE (WASI module)     │
│  Verifies NEAR signature        │
│  Derives private key            │
│  Decrypts emails for owner      │
└─────────────────────────────────┘
        │
        ▼
┌─────────────────────────────────┐
│  web-ui (Next.js)               │
│  Connect wallet, view inbox     │
└─────────────────────────────────┘`}
        </pre>
      </div>

      <div className="mb-6">
        <h4 className="font-semibold text-gray-900 mb-3">Key Derivation (BIP32-style)</h4>
        <p className="text-gray-700 text-sm mb-2">
          The critical feature: the SMTP server can derive per-user <strong>public keys</strong> without knowing the master secret.
        </p>
        <pre className="text-sm bg-gray-50 p-4 rounded-lg overflow-x-auto whitespace-pre">
{`Public Key Derivation (SMTP server, no secret needed):
  user_pubkey = master_pubkey + SHA256("near-email:v1:" + account_id) * G

Private Key Derivation (OutLayer TEE, requires master secret):
  user_privkey = master_privkey + SHA256("near-email:v1:" + account_id)`}
        </pre>
      </div>

      <div className="mb-6">
        <h4 className="font-semibold text-gray-900 mb-3">Dual-Mode Integration</h4>
        <p className="text-gray-700 text-sm mb-3">
          near.email demonstrates how one app can seamlessly use both integration modes:
        </p>
        <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
          <div className="border border-gray-200 rounded-lg p-4">
            <h5 className="font-semibold text-gray-900 mb-2">Blockchain Mode</h5>
            <ul className="text-sm text-gray-700 space-y-1 list-disc list-inside">
              <li>User signs with NEAR wallet</li>
              <li>1.5 MB max output</li>
              <li>Calls <code>request_execution()</code> on-chain</li>
              <li>No payment key needed</li>
            </ul>
          </div>
          <div className="border border-blue-200 rounded-lg p-4 bg-blue-50">
            <h5 className="font-semibold text-blue-900 mb-2">Payment Key Mode</h5>
            <ul className="text-sm text-gray-700 space-y-1 list-disc list-inside">
              <li>No wallet popup required</li>
              <li>25 MB max output (18 MB attachments)</li>
              <li>Direct HTTPS call to OutLayer API</li>
              <li>Prepaid with USDC/USDT or NEAR</li>
            </ul>
          </div>
        </div>
        <p className="text-gray-600 text-sm mt-3">
          The UI automatically routes through the correct mode based on a toggle in account settings.
          This pattern shows how OutLayer changes what apps can do &mdash; a single WASI module serves
          both blockchain and Web2 users transparently.
        </p>
      </div>

      <SecurityNotesSection items={[
        'Server compromise: Emails are encrypted, server only has public key',
        'Master key leak: Key stored only in OutLayer TEE (hardware-enforced)',
        'NEAR-to-NEAR emails never touch external SMTP servers',
        'TEE attestation proves correct decryption code is running'
      ]} />

      <UseCasesSection items={[
        'Secure communication for NEAR ecosystem',
        'Notification system for DeFi positions and DAO proposals',
        'Privacy-preserving email without trusting a provider',
        'Template for building TEE-secured communication apps'
      ]} />

      <TechnicalDetailsSection items={[
        <><strong>WASI Version:</strong> Preview 2 (<code>wasm32-wasip2</code>)</>,
        <><strong>Language:</strong> Rust</>,
        <><strong>Encryption:</strong> ECIES (secp256k1) + AES-GCM</>,
        <><strong>Key Derivation:</strong> BIP32-style from master keypair</>,
        <><strong>TEE:</strong> Intel TDX via Phala Cloud</>,
        <><strong>Components:</strong> smtp-server, db-api, wasi-near-email-ark, web-ui</>,
        <><strong>Modes:</strong> Blockchain (wallet signature) + HTTPS (Payment Key)</>
      ]} />

      <LearnMoreSection>
        <ul className="text-sm text-gray-700 space-y-1">
          <li>
            <a href="https://near.email" target="_blank" rel="noopener noreferrer" className="text-[var(--primary-orange)] hover:underline">
              Try near.email (live)
            </a>
          </li>
          <li>
            <a href="https://github.com/zavodil/near-email" target="_blank" rel="noopener noreferrer" className="text-[var(--primary-orange)] hover:underline">
              View Source Code
            </a>
          </li>
          <li>
            <Link href="/docs/secrets" className="text-[var(--primary-orange)] hover:underline">
              Secrets & Key Derivation Docs
            </Link>
          </li>
          <li>
            <Link href="/docs/tee-attestation" className="text-[var(--primary-orange)] hover:underline">
              TEE Attestation Docs
            </Link>
          </li>
        </ul>
      </LearnMoreSection>
    </ExampleCard>
  );
}
