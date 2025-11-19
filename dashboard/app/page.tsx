'use client';

import Link from 'next/link';
import Image from 'next/image';
import { useEffect, useRef } from 'react';

export default function Home() {
  const matrixRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    // Matrix rain effect with click explosion
    const container = matrixRef.current;
    if (!container) return;

    // Create canvas element
    const canvas = document.createElement('canvas');
    const ctx = canvas.getContext('2d');
    if (!ctx) return;

    // Set canvas size to match hexagon
    canvas.width = 400;
    canvas.height = 346;
    canvas.style.width = '100%';
    canvas.style.height = '100%';
    container.appendChild(canvas);

    // Fill canvas with black immediately
    ctx.fillStyle = 'rgba(0, 0, 0, 1)';
    ctx.fillRect(0, 0, canvas.width, canvas.height);

    // Characters for the rain
    const katakana = 'アァカサタナハマヤャラワガザダバパイィキシチニヒミリヰギジヂビピウゥクスツヌフムユュルグズブヅプエェケセテネヘメレヱゲゼデベペオォコソトノホモヨョロヲゴゾドボポヴッン';
    const latin = 'ABCDEFGHIJKLMNOPQRSTUVWXYZ';
    const nums = '0123456789';
    const alphabet = katakana + latin + nums;

    const fontSize = 16;
    const columns = canvas.width / fontSize;

    // Array to track drops
    const rainDrops: number[] = [];
    for (let x = 0; x < columns; x++) {
      rainDrops[x] = Math.floor(Math.random() * (canvas.height / fontSize));
    }

    const draw = () => {
      // Semi-transparent black to create fade effect
      ctx.fillStyle = 'rgba(0, 0, 0, 0.03)';
      ctx.fillRect(0, 0, canvas.width, canvas.height);

      ctx.font = fontSize + 'px monospace';

      // Draw falling rain
      ctx.fillStyle = '#4a7c2c';
      for (let i = 0; i < rainDrops.length; i++) {
        const text = alphabet.charAt(Math.floor(Math.random() * alphabet.length));
        ctx.fillText(text, i * fontSize, rainDrops[i] * fontSize);

        if (rainDrops[i] * fontSize > canvas.height && Math.random() > 0.975) {
          rainDrops[i] = 0;
        }
        rainDrops[i]++;
      }

    };

    const interval = setInterval(draw, 60);

    return () => {
      clearInterval(interval);
      container.innerHTML = '';
    };
  }, []);

  return (
    <div className="relative min-h-screen">
      {/* Hero section */}
      <div className="relative z-10 px-4 py-8 sm:py-16 sm:px-6 lg:px-8">
        <div className="text-center">
          {/* Hexagon with logo and matrix rain inside */}
          <div className="flex justify-center mb-8 sm:mb-12">
            <div className="relative inline-block">
              <div className="hexagon" />
              {/* Matrix rain container - clipped to hexagon shape */}
              <div ref={matrixRef} className="matrix-container" />
              <div className="absolute inset-0 flex items-center justify-center logo-emerge pointer-events-none">
                <Image
                  src="/outlayer.png"
                  alt="NEAR OutLayer"
                  width={350}
                  height={200}
                  className="relative z-10 max-w-[280px] sm:max-w-[350px]"
                  priority
                />
              </div>
            </div>
          </div>

          {/* Tagline */}
          <h2 className="text-2xl font-bold tracking-tight text-gray-900 sm:text-3xl md:text-4xl mb-4">
            <span className="block text-transparent bg-clip-text bg-gradient-to-r from-[var(--primary-green)] to-[var(--neon-green)]">
              Keep your security on-chain.
            </span>
            <span className="block text-transparent bg-clip-text bg-gradient-to-r from-[var(--primary-orange)] to-[var(--primary-yellow)] mt-2">
              Scale computation off-chain — out of the blockchain layer.
            </span>
          </h2>

          <div className="mt-6 max-w-3xl mx-auto space-y-4">
            <p className="text-base text-gray-700 sm:text-lg md:text-xl">
              Pause your smart contract execution at any point and attach your GitHub repository as an argument.
            </p>
            <p className="text-base text-gray-700 sm:text-lg md:text-xl">
              We compile your code, execute it off-chain in TEE, and finalize your transaction on-chain — enabling AI inference, real-world data oracles, secure randomness, or any computation beyond smart contract limits.
            </p>
          </div>

          {/* CTA Buttons */}
          <div className="mt-10 max-w-md mx-auto sm:flex sm:justify-center gap-4">
            <Link
              href="/playground"
              className="btn-primary w-full sm:w-auto flex items-center justify-center px-8 py-3 text-base font-bold rounded-lg text-black md:py-4 md:text-lg md:px-10 whitespace-nowrap"
            >
              Try Playground
            </Link>
            <Link
              href="/executions"
              className="btn-secondary w-full sm:w-auto flex items-center justify-center px-8 py-3 text-base font-bold rounded-lg md:py-4 md:text-lg md:px-10 mt-3 sm:mt-0 whitespace-nowrap"
            >
              View Executions
            </Link>
          </div>
        </div>

        {/* Features Grid */}
        <div className="mt-24">
          <h3 className="text-3xl font-bold text-center mb-12 text-transparent bg-clip-text bg-gradient-to-r from-[var(--primary-orange)] to-[var(--primary-green)]">
            OutLayer Features
          </h3>
          <div className="grid grid-cols-1 gap-8 sm:grid-cols-2 lg:grid-cols-3">

            {/* Fast Execution */}
            <div className="feature-card rounded-lg px-6 py-8 transform transition-all">
              <div className="feature-icon inline-flex items-center justify-center p-4 rounded-lg mb-6">
                <svg className="h-8 w-8 text-white" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                  <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M13 10V3L4 14h7v7l9-11h-7z" />
                </svg>
              </div>
              <h4 className="text-xl font-bold text-[var(--primary-orange)] mb-3">Fast Execution</h4>
              <p className="text-gray-600">
                Off-chain execution 100x faster with Layer 1 settlement. Run code impossible in smart contracts while final results settle securely on NEAR blockchain.
              </p>
            </div>

            {/* Secure & Verifiable */}
            <div className="feature-card rounded-lg px-6 py-8 transform transition-all">
              <div className="feature-icon inline-flex items-center justify-center p-4 rounded-lg mb-6">
                <svg className="h-8 w-8 text-white" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                  <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M12 15v2m-6 4h12a2 2 0 002-2v-6a2 2 0 00-2-2H6a2 2 0 00-2 2v6a2 2 0 002 2zm10-10V7a4 4 0 00-8 0v4h8z" />
                </svg>
              </div>
              <h4 className="text-xl font-bold text-[var(--primary-green)] mb-3">Secure & Verifiable</h4>
              <p className="text-gray-600">
                TEE-ready architecture with attestation support. Trust minimized with cryptographic proofs and NEAR settlement.
              </p>
            </div>

            {/* Any WASM Code */}
            <div className="feature-card rounded-lg px-6 py-8 transform transition-all">
              <div className="feature-icon inline-flex items-center justify-center p-4 rounded-lg mb-6">
                <svg className="h-8 w-8 text-white" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                  <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M10 20l4-16m4 4l4 4-4 4M6 16l-4-4 4-4" />
                </svg>
              </div>
              <h4 className="text-xl font-bold text-[var(--primary-orange)] mb-3">Any WASM Code</h4>
              <p className="text-gray-600">
                Execute AI inference, fetch price oracles, generate randomness, or run any computation. Write in Rust, C++, Go, or any language that compiles to WASM.
              </p>
            </div>

            {/* Encrypted Secrets */}
            <div className="feature-card rounded-lg px-6 py-8 transform transition-all">
              <div className="feature-icon inline-flex items-center justify-center p-4 rounded-lg mb-6">
                <svg className="h-8 w-8 text-white" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                  <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M15 7a2 2 0 012 2m4 0a6 6 0 01-7.743 5.743L11 17H9v2H7v2H4a1 1 0 01-1-1v-2.586a1 1 0 01.293-.707l5.964-5.964A6 6 0 1121 9z" />
                </svg>
              </div>
              <h4 className="text-xl font-bold text-[var(--primary-green)] mb-3">Encrypted Secrets</h4>
              <p className="text-gray-600">
                Store API keys encrypted on-chain with sophisticated access control (whitelists, token balance, complex logic). Automatically injected as WASI environment variables.
              </p>
            </div>

            {/* Dynamic Pricing */}
            <div className="feature-card rounded-lg px-6 py-8 transform transition-all">
              <div className="feature-icon inline-flex items-center justify-center p-4 rounded-lg mb-6">
                <svg className="h-8 w-8 text-white" fill="currentColor" viewBox="0 0 24 24">
                  <path d="M12 2C6.48 2 2 6.48 2 12s4.48 10 10 10 10-4.48 10-10S17.52 2 12 2zm1.41 16.09V20h-2.67v-1.93c-1.71-.36-3.16-1.46-3.27-3.4h1.96c.1 1.05.82 1.87 2.65 1.87 1.96 0 2.4-.98 2.4-1.59 0-.83-.44-1.61-2.67-2.14-2.48-.6-4.18-1.62-4.18-3.67 0-1.72 1.39-2.84 3.11-3.21V4h2.67v1.95c1.86.45 2.79 1.86 2.85 3.39H14.3c-.05-1.11-.64-1.87-2.22-1.87-1.5 0-2.4.68-2.4 1.64 0 .84.65 1.39 2.67 1.91s4.18 1.39 4.18 3.91c-.01 1.83-1.38 2.83-3.12 3.16z"/>
                </svg>
              </div>
              <h4 className="text-xl font-bold text-[var(--primary-orange)] mb-3">Flexible Payment</h4>
              <p className="text-gray-600">
                Pay only for actual resources consumed. Choose who pays: let users cover their requests or sponsor execution from contract balance. Unused funds automatically refunded.
              </p>
            </div>

            {/* Resource Metering */}
            <div className="feature-card rounded-lg px-6 py-8 transform transition-all">
              <div className="feature-icon inline-flex items-center justify-center p-4 rounded-lg mb-6">
                <svg className="h-8 w-8 text-white" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                  <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M9 19v-6a2 2 0 00-2-2H5a2 2 0 00-2 2v6a2 2 0 002 2h2a2 2 0 002-2zm0 0V9a2 2 0 012-2h2a2 2 0 012 2v10m-6 0a2 2 0 002 2h2a2 2 0 002-2m0 0V5a2 2 0 012-2h2a2 2 0 012 2v14a2 2 0 01-2 2h-2a2 2 0 01-2-2z" />
                </svg>
              </div>
              <h4 className="text-xl font-bold text-[var(--primary-green)] mb-3">Real Metrics</h4>
              <p className="text-gray-600">
                Actual instruction counting via fuel metering. Precise timing and resource usage tracking. No fake metrics.
              </p>
            </div>

            {/* GitHub Integration */}
            <div className="feature-card rounded-lg px-6 py-8 transform transition-all">
              <div className="feature-icon inline-flex items-center justify-center p-4 rounded-lg mb-6">
                <svg className="h-8 w-8 text-white" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                  <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M10 6H6a2 2 0 00-2 2v10a2 2 0 002 2h10a2 2 0 002-2v-4M14 4h6m0 0v6m0-6L10 14" />
                </svg>
              </div>
              <h4 className="text-xl font-bold text-[var(--primary-orange)] mb-3">GitHub Integration</h4>
              <p className="text-gray-600">
                Compile directly from public GitHub repos. Reference by branch or commit hash (immutable). WASM cache with distributed locking. GitLab support coming soon.
              </p>
            </div>

            {/* Access Control */}
            <div className="feature-card rounded-lg px-6 py-8 transform transition-all">
              <div className="feature-icon inline-flex items-center justify-center p-4 rounded-lg mb-6">
                <svg className="h-8 w-8 text-white" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                  <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M9 12l2 2 4-4m5.618-4.016A11.955 11.955 0 0112 2.944a11.955 11.955 0 01-8.618 3.04A12.02 12.02 0 003 9c0 5.591 3.824 10.29 9 11.622 5.176-1.332 9-6.03 9-11.622 0-1.042-.133-2.052-.382-3.016z" />
                </svg>
              </div>
              <h4 className="text-xl font-bold text-[var(--primary-green)] mb-3">Access Control</h4>
              <p className="text-gray-600">
                Flexible secret access: whitelist, NEAR/FT/NFT balance checks, DAO memebership, account patterns, and complex logic conditions (AND/OR/NOT).
              </p>
            </div>

            {/* WASI Support */}
            <div className="feature-card rounded-lg px-6 py-8 transform transition-all">
              <div className="feature-icon inline-flex items-center justify-center p-4 rounded-lg mb-6">
                <svg className="h-8 w-8 text-white" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                  <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M8 9l3 3-3 3m5 0h3M5 20h14a2 2 0 002-2V6a2 2 0 00-2-2H5a2 2 0 00-2 2v12a2 2 0 002 2z" />
                </svg>
              </div>
              <h4 className="text-xl font-bold text-[var(--primary-orange)] mb-3">WASI Environment</h4>
              <p className="text-gray-600">
                Full WASI support with environment variables and blockchain context. Access secrets via std::env::var(). Pass block height, account ID, timestamp as input parameters.
              </p>
            </div>

            {/* Real-World Data */}
            <div className="feature-card rounded-lg px-6 py-8 transform transition-all">
              <div className="feature-icon inline-flex items-center justify-center p-4 rounded-lg mb-6">
                <svg className="h-8 w-8 text-white" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                  <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M3.055 11H5a2 2 0 012 2v1a2 2 0 002 2 2 2 0 012 2v2.945M8 3.935V5.5A2.5 2.5 0 0010.5 8h.5a2 2 0 012 2 2 2 0 104 0 2 2 0 012-2h1.064M15 20.488V18a2 2 0 012-2h3.064M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
                </svg>
              </div>
              <h4 className="text-xl font-bold text-[var(--primary-green)] mb-3">Real-World Data</h4>
              <p className="text-gray-600">
                Fetch any API data: asset prices, weather forecasts, sports results, Dota match outcomes, historical on-chain data from indexers, flight status, and more.
              </p>
            </div>

            {/* Pre-compiled WASM */}
            <div className="feature-card rounded-lg px-6 py-8 transform transition-all">
              <div className="feature-icon inline-flex items-center justify-center p-4 rounded-lg mb-6">
                <svg className="h-8 w-8 text-white" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                  <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M5 12h14M5 12a2 2 0 01-2-2V6a2 2 0 012-2h14a2 2 0 012 2v4a2 2 0 01-2 2M5 12a2 2 0 00-2 2v4a2 2 0 002 2h14a2 2 0 002-2v-4a2 2 0 00-2-2m-2-4h.01M17 16h.01" />
                </svg>
              </div>
              <h4 className="text-xl font-bold text-[var(--primary-orange)] mb-3">Pre-compiled WASM</h4>
              <p className="text-gray-600">
                Run pre-compiled WASM files without source code. Store immutable binaries on FastFS/IPFS - no middlemen, no compilation dependencies. Perfect for closed-source or proprietary code.
              </p>
            </div>

            {/* Unlimited Possibilities */}
            <div className="feature-card rounded-lg px-6 py-8 transform transition-all">
              <div className="feature-icon inline-flex items-center justify-center p-4 rounded-lg mb-6">
                <svg className="h-8 w-8 text-white" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                  <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M9.663 17h4.673M12 3v1m6.364 1.636l-.707.707M21 12h-1M4 12H3m3.343-5.657l-.707-.707m2.828 9.9a5 5 0 117.072 0l-.548.547A3.374 3.374 0 0014 18.469V19a2 2 0 11-4 0v-.531c0-.895-.356-1.754-.988-2.386l-.548-.547z" />
                </svg>
              </div>
              <h4 className="text-xl font-bold text-[var(--primary-green)] mb-3">Unlimited Possibilities</h4>
              <p className="text-gray-600">
                Break free from blockchain constraints. Invent entirely new concepts and use cases previously impossible in decentralized applications. The only limit is your imagination.
              </p>
            </div>

          </div>
        </div>

        {/* Only Possible on NEAR banner */}
        <div className="mt-32 mb-24">
          <div className="relative py-12 overflow-hidden">
            <div className="absolute inset-0 bg-gradient-to-r from-[var(--primary-orange)] via-[var(--primary-yellow)] to-[var(--primary-green)] opacity-10"></div>
            <h2 className="relative text-4xl sm:text-5xl md:text-6xl font-light text-center text-transparent bg-clip-text bg-gradient-to-r from-[var(--primary-orange)] to-[var(--primary-green)] tracking-wide">
              Only Possible on NEAR
            </h2>
          </div>
        </div>

        {/* Quick Links */}
        <div className="mt-24 text-center">
          <h3 className="text-3xl font-bold mb-12 text-transparent bg-clip-text bg-gradient-to-r from-[var(--primary-orange)] to-[var(--primary-yellow)]">
            Get Started
          </h3>
          <div className="grid grid-cols-1 gap-6 sm:grid-cols-2 lg:grid-cols-4 max-w-5xl mx-auto">
            <Link href="/playground" className="feature-card rounded-lg px-6 py-6 hover:scale-105 transition-transform">
              <h4 className="text-lg font-bold text-[var(--primary-orange)] mb-2">Playground</h4>
              <p className="text-sm text-gray-600">Test execution</p>
            </Link>
            <Link href="/secrets" className="feature-card rounded-lg px-6 py-6 hover:scale-105 transition-transform">
              <h4 className="text-lg font-bold text-[var(--primary-green)] mb-2">Secrets</h4>
              <p className="text-sm text-gray-600">Manage API keys</p>
            </Link>
            <Link href="/executions" className="feature-card rounded-lg px-6 py-6 hover:scale-105 transition-transform">
              <h4 className="text-lg font-bold text-[var(--primary-orange)] mb-2">Executions</h4>
              <p className="text-sm text-gray-600">View results</p>
            </Link>
            <Link href="/stats" className="feature-card rounded-lg px-6 py-6 hover:scale-105 transition-transform">
              <h4 className="text-lg font-bold text-[var(--primary-green)] mb-2">Analytics</h4>
              <p className="text-sm text-gray-600">Platform stats</p>
            </Link>
          </div>
        </div>

        {/* Social Links */}
        <div className="mt-24 pb-16 text-center">
          <div className="flex justify-center gap-8">
            <a
              href="https://x.com/out_layer"
              target="_blank"
              rel="noopener noreferrer"
              className="flex items-center gap-2 text-gray-700 hover:text-[var(--primary-orange)] transition-colors text-lg font-semibold"
            >
              <svg className="w-6 h-6" fill="currentColor" viewBox="0 0 24 24">
                <path d="M18.244 2.25h3.308l-7.227 8.26 8.502 11.24H16.17l-5.214-6.817L4.99 21.75H1.68l7.73-8.835L1.254 2.25H8.08l4.713 6.231zm-1.161 17.52h1.833L7.084 4.126H5.117z"/>
              </svg>
              Twitter
            </a>
            <a
              href="https://github.com/fastnear/near-outlayer"
              target="_blank"
              rel="noopener noreferrer"
              className="flex items-center gap-2 text-gray-700 hover:text-[var(--primary-green)] transition-colors text-lg font-semibold"
            >
              <svg className="w-6 h-6" fill="currentColor" viewBox="0 0 24 24">
                <path fillRule="evenodd" d="M12 2C6.477 2 2 6.484 2 12.017c0 4.425 2.865 8.18 6.839 9.504.5.092.682-.217.682-.483 0-.237-.008-.868-.013-1.703-2.782.605-3.369-1.343-3.369-1.343-.454-1.158-1.11-1.466-1.11-1.466-.908-.62.069-.608.069-.608 1.003.07 1.531 1.032 1.531 1.032.892 1.53 2.341 1.088 2.91.832.092-.647.35-1.088.636-1.338-2.22-.253-4.555-1.113-4.555-4.951 0-1.093.39-1.988 1.029-2.688-.103-.253-.446-1.272.098-2.65 0 0 .84-.27 2.75 1.026A9.564 9.564 0 0112 6.844c.85.004 1.705.115 2.504.337 1.909-1.296 2.747-1.027 2.747-1.027.546 1.379.202 2.398.1 2.651.64.7 1.028 1.595 1.028 2.688 0 3.848-2.339 4.695-4.566 4.943.359.309.678.92.678 1.855 0 1.338-.012 2.419-.012 2.747 0 .268.18.58.688.482A10.019 10.019 0 0022 12.017C22 6.484 17.522 2 12 2z" clipRule="evenodd"/>
              </svg>
              GitHub
            </a>
          </div>
        </div>

      </div>
    </div>
  );
}
