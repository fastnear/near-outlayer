import Link from 'next/link';
import { ExampleCard, UseCasesSection, KeyFeaturesSection, TechnicalDetailsSection, CodeExampleSection, HowItWorksSection, LearnMoreSection, SecurityNotesSection } from './';

export function AiExample() {
  const badges = (
    <>
      <span className="ml-3 text-sm bg-blue-100 text-blue-800 px-3 py-1 rounded">WASI P2</span>
      <span className="ml-2 text-sm bg-purple-100 text-purple-800 px-3 py-1 rounded">Secrets</span>
      <span className="ml-2 text-sm bg-yellow-100 text-yellow-800 px-3 py-1 rounded">Intermediate</span>
    </>
  );

  return (
    <ExampleCard
      id="ai-ark"
      title="ai-ark"
      badges={badges}
      githubUrl="https://github.com/zavodil/ai-ark"
      playgroundId="ai"
    >
      <p className="text-gray-700 mb-4">
        Integrate OpenAI GPT models into your NEAR smart contracts. Send prompts from on-chain and receive AI-generated responses securely.
      </p>

      <UseCasesSection items={[
        'AI-powered smart contract decision making',
        'Natural language processing for on-chain data',
        'Content generation triggered by blockchain events',
        'Intelligent chatbots with verifiable responses'
      ]} />

      <KeyFeaturesSection items={[
        <>HTTP client using <code>reqwest</code> with WASI sockets</>,
        'Encrypted API key storage using OutLayer secrets',
        'Support for GPT-4, GPT-3.5-turbo models',
        'Configurable temperature, max tokens, system prompts'
      ]} />

      <CodeExampleSection
        title="Input Format:"
        code={`{
  "prompt": "Explain NEAR Protocol in one sentence",
  "model": "gpt-4",
  "temperature": 0.7,
  "max_tokens": 100
}`}
      />

      <CodeExampleSection
        title="Output Format:"
        code={`{
  "response": "NEAR Protocol is a scalable, developer-friendly blockchain platform...",
  "model": "gpt-4",
  "tokens_used": 28,
  "finish_reason": "stop"
}`}
      />

      <HowItWorksSection items={[
        <>WASM reads <code>OPENAI_API_KEY</code> from encrypted secrets (env var)</>,
        'Constructs HTTP POST request to OpenAI API',
        'Sends request via WASI HTTP interface',
        'Receives and parses JSON response',
        'Extracts AI-generated text and metadata',
        'Returns structured result to NEAR contract'
      ]} />

      <TechnicalDetailsSection items={[
        <><strong>WASI Version:</strong> Preview 2 (component model)</>,
        <><strong>Language:</strong> Rust</>,
        <><strong>HTTP Client:</strong> <code>reqwest</code> with <code>wasi-preview2</code> feature</>,
        <><strong>Secrets:</strong> Required (<code>OPENAI_API_KEY</code>)</>,
        <><strong>Network:</strong> Required (outbound HTTPS to api.openai.com)</>,
        <><strong>Build:</strong> <code>cargo component build --release</code></>,
        <><strong>Size:</strong> ~2.5MB compiled WASM</>
      ]} />

      <div className="mt-4 p-4 bg-blue-50 border-l-4 border-blue-400">
        <h4 className="font-semibold text-gray-900 mb-2">Setting Up Secrets</h4>
        <ol className="list-decimal list-inside text-sm text-gray-700 space-y-1">
          <li>
            Navigate to <Link href="/secrets" className="text-[var(--primary-orange)] hover:underline">Secrets Management</Link>
          </li>
          <li>Create new secret with key <code>OPENAI_API_KEY</code> and your OpenAI API key as value</li>
          <li>
            Set repo to your GitHub project (e.g., <code>github.com/alice/ai-ark</code>)
          </li>
          <li>Choose access condition (e.g., AllowAll for testing, Whitelist for production)</li>
          <li>Worker will automatically decrypt and inject into WASM environment</li>
        </ol>
      </div>

      <SecurityNotesSection items={[
        <>✅ API keys stored encrypted on-chain, decrypted only in TEE</>,
        <>✅ Keys never exposed to worker logs or external parties</>,
        <>⚠️ Monitor OpenAI API usage to prevent unexpected costs</>,
        <>⚠️ Set token limits to control response sizes</>
      ]} />

      <LearnMoreSection>
        <ul className="text-sm text-gray-700 space-y-1">
          <li>
            📖 <Link href="/docs/wasi#wasi-preview-2" className="text-[var(--primary-orange)] hover:underline">
              WASI Preview 2 Documentation
            </Link>
          </li>
          <li>
            🔐 <Link href="/docs/secrets" className="text-[var(--primary-orange)] hover:underline">
              Secrets Management Guide
            </Link>
          </li>
          <li>
            🎮 <Link href="/playground#ai" className="text-[var(--primary-orange)] hover:underline">
              Try in Playground
            </Link>
          </li>
          <li>
            💻 <a href="https://github.com/zavodil/ai-ark" target="_blank" rel="noopener noreferrer" className="text-[var(--primary-orange)] hover:underline">
              View Source Code
            </a>
          </li>
        </ul>
      </LearnMoreSection>
    </ExampleCard>
  );
}
