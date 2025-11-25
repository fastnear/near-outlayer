import Link from 'next/link';
import { ExampleCard, UseCasesSection, KeyFeaturesSection, TechnicalDetailsSection, CodeExampleSection, HowItWorksSection, LearnMoreSection } from './';

export function RandomExample() {
  const badges = (
    <>
      <span className="ml-3 text-sm bg-blue-100 text-blue-800 px-3 py-1 rounded">WASI P1</span>
      <span className="ml-2 text-sm bg-green-100 text-green-800 px-3 py-1 rounded">Beginner</span>
    </>
  );

  return (
    <ExampleCard
      id="random-ark"
      title="random-ark"
      badges={badges}
      githubUrl="https://github.com/zavodil/random-ark"
      playgroundId="random"
    >
      <p className="text-gray-700 mb-4">
        Generate cryptographically secure random numbers inside TEE (Trusted Execution Environment) using WASI&apos;s random_get interface.
      </p>

      <UseCasesSection items={[
        'Fair lottery systems requiring verifiable randomness',
        'Gaming applications needing unpredictable outcomes',
        'Cryptographic key generation',
        'Random sampling for statistical analysis'
      ]} />

      <KeyFeaturesSection items={[
        <>Uses <code>getrandom</code> crate for secure randomness</>,
        'TEE environment ensures entropy source cannot be manipulated',
        'Simple JSON input/output interface',
        'Perfect starter example for WASI development'
      ]} />

      <CodeExampleSection
        title="Input Format:"
        code={`{
  "count": 5,
  "min": 1,
  "max": 100
}`}
        description="Request 5 random numbers between 1 and 100"
      />

      <CodeExampleSection
        title="Output Format:"
        code={`{
  "numbers": [42, 17, 93, 8, 55]
}`}
      />

      <HowItWorksSection items={[
        <>WASM calls <code>random_get</code> WASI function</>,
        'Worker runtime requests entropy from TEE hardware',
        "Random bytes are generated using CPU's secure random number generator",
        'Numbers are scaled to requested range (min-max)',
        'Result returned as JSON'
      ]} />

      <TechnicalDetailsSection items={[
        <><strong>WASI Version:</strong> Preview 1 (<code>wasm32-wasip1</code>)</>,
        <><strong>Language:</strong> Rust with <code>getrandom</code> crate</>,
        <><strong>Entry Point:</strong> <code>main()</code> reads stdin, writes stdout</>,
        <><strong>Dependencies:</strong> <code>serde_json</code> for JSON parsing</>,
        <><strong>Build:</strong> <code>cargo build --target wasm32-wasip1 --release</code></>,
        <><strong>Size:</strong> ~200KB compiled WASM</>
      ]} />

      <LearnMoreSection>
        <ul className="text-sm text-gray-700 space-y-1">
          <li>
            📖 <Link href="/docs/wasi#wasi-preview-1" className="text-[var(--primary-orange)] hover:underline">
              WASI Preview 1 Documentation
            </Link>
          </li>
          <li>
            🎮 <Link href="/playground#random" className="text-[var(--primary-orange)] hover:underline">
              Try in Playground
            </Link>
          </li>
          <li>
            💻 <a href="https://github.com/zavodil/random-ark" target="_blank" rel="noopener noreferrer" className="text-[var(--primary-orange)] hover:underline">
              View Source Code
            </a>
          </li>
        </ul>
      </LearnMoreSection>
    </ExampleCard>
  );
}
