import Link from 'next/link';
import { ExampleCard, UseCasesSection, KeyFeaturesSection, TechnicalDetailsSection, CodeExampleSection, HowItWorksSection, LearnMoreSection } from './';

export function EchoExample() {
  const badges = (
    <>
      <span className="ml-3 text-sm bg-blue-100 text-blue-800 px-3 py-1 rounded">WASI P1</span>
      <span className="ml-2 text-sm bg-green-100 text-green-800 px-3 py-1 rounded">Beginner</span>
    </>
  );

  return (
    <ExampleCard
      id="echo-ark"
      title="echo-ark"
      badges={badges}
      githubUrl="https://github.com/zavodil/echo-ark"
      playgroundId="echo"
    >
      <p className="text-gray-700 mb-4">
        Simple echo service that accepts JSON input and returns it with a timestamp. Demonstrates basic WASI I/O and data processing.
      </p>

      <UseCasesSection items={[
        'Testing WASI input/output pipeline',
        'Verifying data serialization/deserialization',
        'Template for building more complex data processing tasks',
        'Learning WASI development fundamentals'
      ]} />

      <KeyFeaturesSection items={[
        'JSON input validation',
        'Timestamp generation using WASI clock interface',
        'Error handling and structured responses',
        'Minimal dependencies - great learning example'
      ]} />

      <CodeExampleSection
        title="Input Format:"
        code={`{
  "message": "Hello, OutLayer!",
  "metadata": {
    "user_id": "alice.near"
  }
}`}
      />

      <CodeExampleSection
        title="Output Format:"
        code={`{
  "echo": {
    "message": "Hello, OutLayer!",
    "metadata": {
      "user_id": "alice.near"
    }
  },
  "timestamp": "2025-01-15T12:34:56Z",
  "processed_by": "echo-ark v1.0"
}`}
      />

      <HowItWorksSection items={[
        'WASM reads JSON from stdin',
        'Deserializes input using serde_json',
        'Generates timestamp using WASI clock_time_get',
        'Constructs response object with original data + metadata',
        'Serializes and writes to stdout'
      ]} />

      <TechnicalDetailsSection items={[
        <><strong>WASI Version:</strong> Preview 1 (<code>wasm32-wasip1</code>)</>,
        <><strong>Language:</strong> Rust</>,
        <><strong>Entry Point:</strong> <code>main()</code></>,
        <><strong>Dependencies:</strong> <code>serde</code>, <code>serde_json</code>, <code>chrono</code></>,
        <><strong>Build:</strong> <code>cargo build --target wasm32-wasip1 --release</code></>,
        <><strong>Size:</strong> ~180KB compiled WASM</>
      ]} />

      <LearnMoreSection>
        <ul className="text-sm text-gray-700 space-y-1">
          <li>
            📖 <Link href="/docs/wasi#wasi-preview-1" className="text-[var(--primary-orange)] hover:underline">
              WASI Preview 1 Documentation
            </Link>
          </li>
          <li>
            🎮 <Link href="/playground#echo" className="text-[var(--primary-orange)] hover:underline">
              Try in Playground
            </Link>
          </li>
          <li>
            💻 <a href="https://github.com/zavodil/echo-ark" target="_blank" rel="noopener noreferrer" className="text-[var(--primary-orange)] hover:underline">
              View Source Code
            </a>
          </li>
        </ul>
      </LearnMoreSection>
    </ExampleCard>
  );
}
