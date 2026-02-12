'use client';

import {
  useHashNavigation,
  RandomExample,
  EchoExample,
  AiExample,
  WeatherExample,
  OracleExample,
  EthereumExample,
  BotfatherExample,
  IntentsExample,
  PrivateDaoExample,
  CaptchaExample,
  NearEmailExample
} from './components';

export default function ExamplesPage() {
  useHashNavigation();

  return (
    <div className="max-w-5xl">
      <h1 className="text-4xl font-bold mb-3">Example Projects</h1>
      <p className="text-gray-600 mb-8">
        Production-ready examples demonstrating different WASI patterns and use cases. All examples are open-source and ready to deploy.
      </p>

      <div className="space-y-8">
        <RandomExample />
        <EchoExample />
        <AiExample />
        <WeatherExample />
        <OracleExample />
        <EthereumExample />
        <BotfatherExample />
        <IntentsExample />
        <PrivateDaoExample />
        <CaptchaExample />
        <NearEmailExample />
      </div>
    </div>
  );
}
