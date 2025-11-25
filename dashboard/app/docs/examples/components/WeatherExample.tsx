import Link from 'next/link';
import { ExampleCard, UseCasesSection, KeyFeaturesSection, TechnicalDetailsSection, CodeExampleSection, HowItWorksSection, LearnMoreSection, SecurityNotesSection } from './';

export function WeatherExample() {
  const badges = (
    <>
      <span className="ml-3 text-sm bg-blue-100 text-blue-800 px-3 py-1 rounded">WASI P2</span>
      <span className="ml-2 text-sm bg-purple-100 text-purple-800 px-3 py-1 rounded">Secrets</span>
      <span className="ml-2 text-sm bg-yellow-100 text-yellow-800 px-3 py-1 rounded">Intermediate</span>
    </>
  );

  return (
    <ExampleCard
      id="weather-ark"
      title="weather-ark"
      badges={badges}
      githubUrl="https://github.com/zavodil/weather-ark"
      playgroundId="weather"
    >
      <p className="text-gray-700 mb-4">
        Fetch real-world weather data from OpenWeatherMap API and bring it on-chain. Perfect for parametric insurance, prediction markets, and climate-dependent DeFi.
      </p>

      <UseCasesSection items={[
        'Weather-based parametric insurance (crop insurance, travel insurance)',
        'Prediction markets requiring real-world weather data',
        'Climate data for carbon credit verification',
        'Event cancellation triggers based on weather conditions'
      ]} />

      <KeyFeaturesSection items={[
        'Fetches temperature, humidity, pressure, wind speed',
        'Supports city name or coordinates lookup',
        'Returns structured weather data with timestamps',
        <>Encrypted API key via OutLayer secrets (<code>OPENWEATHER_API_KEY</code>)</>
      ]} />

      <CodeExampleSection
        title="Input Format (by city):"
        code={`{
  "city": "London",
  "country_code": "UK"
}`}
      />

      <CodeExampleSection
        title="Input Format (by coordinates):"
        code={`{
  "lat": 51.5074,
  "lon": -0.1278
}`}
      />

      <CodeExampleSection
        title="Output Format:"
        code={`{
  "location": "London, UK",
  "temperature_celsius": 12.5,
  "humidity_percent": 76,
  "pressure_hpa": 1013,
  "wind_speed_mps": 3.2,
  "description": "partly cloudy",
  "timestamp": "2025-01-15T12:34:56Z"
}`}
      />

      <HowItWorksSection items={[
        <>WASM reads <code>OPENWEATHER_API_KEY</code> from encrypted secrets</>,
        'Constructs HTTP request to OpenWeatherMap API',
        'Sends GET request via WASI HTTP interface',
        'Parses JSON response with serde',
        'Converts units (Kelvin → Celsius, etc.)',
        'Returns normalized weather data to NEAR contract'
      ]} />

      <TechnicalDetailsSection items={[
        <><strong>WASI Version:</strong> Preview 2 (component model)</>,
        <><strong>Language:</strong> Rust</>,
        <><strong>HTTP Client:</strong> <code>reqwest</code> with WASI sockets</>,
        <><strong>Secrets:</strong> Required (<code>OPENWEATHER_API_KEY</code>)</>,
        <><strong>Network:</strong> Required (outbound HTTPS to api.openweathermap.org)</>,
        <><strong>API:</strong> OpenWeatherMap Current Weather API</>,
        <><strong>Build:</strong> <code>cargo component build --release</code></>,
        <><strong>Size:</strong> ~2.2MB compiled WASM</>
      ]} />

      <div className="mt-4 p-4 bg-blue-50 border-l-4 border-blue-400">
        <h4 className="font-semibold text-gray-900 mb-2">Setting Up Secrets</h4>
        <ol className="list-decimal list-inside text-sm text-gray-700 space-y-1">
          <li>
            Get free API key from <a href="https://openweathermap.org/api" target="_blank" rel="noopener noreferrer" className="text-[var(--primary-orange)] hover:underline">OpenWeatherMap</a>
          </li>
          <li>
            Navigate to <Link href="/secrets" className="text-[var(--primary-orange)] hover:underline">Secrets Management</Link>
          </li>
          <li>Create secret: key = <code>OPENWEATHER_API_KEY</code>, value = your API key</li>
          <li>Set repo to your GitHub project</li>
          <li>Choose access condition (AllowAll for public data, Whitelist for production)</li>
        </ol>
      </div>

      <SecurityNotesSection items={[
        <>✅ Free tier: 60 calls/minute, 1M calls/month</>,
        <>⚠️ Monitor API usage to stay within limits</>,
        <>✅ API keys encrypted and only decrypted in TEE</>,
        <>⚠️ Weather data typically updates every 10 minutes - avoid excessive calls</>
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
            🎮 <Link href="/playground#weather" className="text-[var(--primary-orange)] hover:underline">
              Try in Playground
            </Link>
          </li>
          <li>
            💻 <a href="https://github.com/zavodil/weather-ark" target="_blank" rel="noopener noreferrer" className="text-[var(--primary-orange)] hover:underline">
              View Source Code
            </a>
          </li>
        </ul>
      </LearnMoreSection>
    </ExampleCard>
  );
}
