import Link from 'next/link';
import { Prism as SyntaxHighlighter } from 'react-syntax-highlighter';
import { vscDarkPlus } from 'react-syntax-highlighter/dist/esm/styles/prism';
import { ExampleCard, KeyFeaturesSection, TechnicalDetailsSection, LearnMoreSection, SecurityNotesSection } from './';

export function CaptchaExample() {
  const badges = (
    <>
      <span className="ml-3 text-sm bg-purple-100 text-purple-800 px-3 py-1 rounded">WASI P2</span>
      <span className="ml-2 text-sm bg-green-100 text-green-800 px-3 py-1 rounded">Full Stack</span>
    </>
  );

  return (
    <ExampleCard
      id="captcha-ark"
      title="captcha-ark"
      badges={badges}
      githubUrl="https://github.com/zavodil/captcha-ark"
    >
      <p className="text-gray-700 mb-4">
        Token sale with mandatory CAPTCHA verification. Transaction won&apos;t complete until user solves CAPTCHA. WASI worker receives session ID from backend and waits for verification signal. Example implementation demonstrating async human verification pattern.
      </p>

      <KeyFeaturesSection items={[
        'Transaction blocking until CAPTCHA solved',
        'WASI worker requests CAPTCHA challenge from backend',
        'Backend sends CAPTCHA to user via WebSocket',
        'Worker waits for verification signal (long-polling)',
        'hCaptcha integration for human verification',
        'React frontend with NEAR Wallet Selector',
        'Node.js backend with Express + WebSocket server'
      ]} />

      <h4 className="font-semibold mt-4 mb-2">Architecture:</h4>
      <SyntaxHighlighter language="text" style={vscDarkPlus} customStyle={{ borderRadius: '0.5rem', fontSize: '0.75rem' }}>
{`User Browser → Token Sale Contract → OutLayer → WASI Worker
      ↑                                              ↓
      └────── Launchpad Backend (WebSocket) ←────────┘

Flow:
1. User clicks "Buy Tokens" with session_id
2. Contract calls OutLayer, transaction pauses
3. WASI worker → backend: "I received payment from session_id X.
   Verify this is a real user, not a bot. Send CAPTCHA and notify
   me when user completes it."
4. Backend sends CAPTCHA to user's browser via WebSocket
5. User solves CAPTCHA in modal
6. Backend verifies solution and marks session as verified
7. Worker receives "verified" signal, returns success to contract
8. Transaction resumes - user receives tokens`}
      </SyntaxHighlighter>

      <h4 className="font-semibold mt-4 mb-2">Components:</h4>
      <ul className="list-disc list-inside text-gray-700 space-y-1 mb-4 ml-4">
        <li><strong>WASI Worker:</strong> Rust WASM that verifies CAPTCHA (wasm32-wasip2)</li>
        <li><strong>Smart Contract:</strong> Token sale logic with OutLayer integration</li>
        <li><strong>Backend:</strong> Node.js Express server with WebSocket support</li>
        <li><strong>Frontend:</strong> React app with hCaptcha widget</li>
      </ul>

      <h4 className="font-semibold mt-4 mb-2">How to Use:</h4>
      <SyntaxHighlighter language="bash" style={vscDarkPlus} customStyle={{ borderRadius: '0.5rem', fontSize: '0.875rem' }}>
{`# 1. Get hCaptcha account (free at hcaptcha.com)
# - Create site and get Site Key + Secret Key

# 2. Clone repository
git clone https://github.com/zavodil/captcha-ark.git
cd captcha-ark

# 3. Setup backend
cd launchpad-backend
npm install
cat > .env <<EOF
PORT=3181
HCAPTCHA_SITE_KEY=your_site_key
HCAPTCHA_SECRET=your_secret_key
WORKER_API_KEY=$(openssl rand -hex 32)
EOF
npm start

# 4. Setup frontend
cd ../launchpad-app
npm install
cat > .env <<EOF
REACT_APP_CONTRACT_ID=tokensale.testnet
REACT_APP_NEAR_NETWORK=testnet
REACT_APP_HCAPTCHA_SITE_KEY=your_site_key
EOF
npm run build
# Deploy build/ to your web server

# 5. Build WASI worker
cd ../captcha-ark
cargo build --target wasm32-wasip2 --release
git push origin main

# 6. Deploy token sale contract
cd ../token-sale-contract
cargo near build
near deploy tokensale.testnet \\
  use-file res/token_sale_contract.wasm \\
  with-init-call new \\
  json-args '{"owner":"owner.testnet","total_supply":"10000","launchpad_url":"https://api.yourdomain.com"}' \\
  prepaid-gas '100.0 Tgas' \\
  attached-deposit '0 NEAR'

# 7. Users can now buy tokens - CAPTCHA required!
# Visit https://launchpad.yourdomain.com and click "Buy Tokens"`}
      </SyntaxHighlighter>

      <div className="mt-4 p-3 bg-blue-50 border-l-4 border-blue-400">
        <p className="text-sm text-gray-700 mb-2">
          <strong>Production Setup:</strong> Requires SSL certificates for both frontend and backend domains. See <a href="https://github.com/zavodil/captcha-ark/blob/main/CONFIGURATION.md" target="_blank" rel="noopener noreferrer" className="text-[var(--primary-orange)] hover:underline">CONFIGURATION.md</a> for complete deployment guide.
        </p>
      </div>

      <SecurityNotesSection items={[
        'Backend must implement worker authentication to prevent spam attacks',
        'Unauthorized requests to create CAPTCHA challenges should be rejected',
        'Use HTTPS for both frontend and backend in production',
        'Implement rate limiting on CAPTCHA challenge creation'
      ]} />

      <TechnicalDetailsSection items={[
        <><strong>WASI Version:</strong> Preview 2 (component model)</>,
        <><strong>Language:</strong> Rust (WASM), TypeScript (Frontend/Backend)</>,
        <><strong>HTTP Client:</strong> <code>reqwest</code> for backend communication</>,
        <><strong>Backend:</strong> Node.js + Express + WebSocket</>,
        <><strong>Frontend:</strong> React with NEAR Wallet Selector</>,
        <><strong>CAPTCHA:</strong> hCaptcha integration</>,
        <><strong>Build:</strong> <code>cargo component build --release</code></>,
        <><strong>Size:</strong> ~2.8MB compiled WASM</>
      ]} />

      <LearnMoreSection>
        <ul className="text-sm text-gray-700 space-y-1">
          <li>
            📖 <Link href="/docs/wasi#wasi-preview-2" className="text-[var(--primary-orange)] hover:underline">
              WASI Preview 2 Documentation
            </Link>
          </li>
          <li>
            💻 <a href="https://github.com/zavodil/captcha-ark" target="_blank" rel="noopener noreferrer" className="text-[var(--primary-orange)] hover:underline">
              View Source Code
            </a>
          </li>
          <li>
            📋 <a href="https://github.com/zavodil/captcha-ark/blob/main/CONFIGURATION.md" target="_blank" rel="noopener noreferrer" className="text-[var(--primary-orange)] hover:underline">
              Deployment Configuration Guide
            </a>
          </li>
          <li>
            🤖 <a href="https://hcaptcha.com" target="_blank" rel="noopener noreferrer" className="text-[var(--primary-orange)] hover:underline">
              hCaptcha (Free Tier Available)
            </a>
          </li>
        </ul>
      </LearnMoreSection>
    </ExampleCard>
  );
}
