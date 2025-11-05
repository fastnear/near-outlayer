'use client';

import Link from 'next/link';
import { Prism as SyntaxHighlighter } from 'react-syntax-highlighter';
import { vscDarkPlus } from 'react-syntax-highlighter/dist/esm/styles/prism';

export default function DeveloperGuideSection() {
  return (
    <div className="prose max-w-none">
      <h2 className="text-3xl font-bold mb-6 text-[var(--primary-orange)]">Developer Guide: Random Numbers</h2>

      {/* TL;DR */}
      <div className="bg-gradient-to-r from-orange-50 to-yellow-50 border-l-4 border-[var(--primary-orange)] p-6 mb-8 rounded-r-lg">
        <h3 className="text-lg font-bold mb-3 text-gray-800">⚡ TL;DR</h3>
        <ul className="space-y-2 text-sm text-gray-700">
          <li>Write WebAssembly project using functions impossible in smart contracts</li>
          <li>Push to public GitHub repo</li>
          <li>Call Outlayer contract from your contract</li>
          <li>Receive the off-chain execution result in the callback response</li>
          <li>Payment based on actual resources used, unused funds auto-refunded</li>
          <li>Settlement stays on Layer 1 (secure NEAR blockchain)</li>
        </ul>
      </div>

      <div className="space-y-8">
        {/* Problem */}
        <section id="problem">
          <h3 className="text-xl font-semibold mb-3">🎯 The Problem</h3>
          <p className="text-gray-700 mb-3">
            Smart contracts need randomness for gaming, lotteries, and fair selection. However, on-chain randomness is fundamentally <strong>deterministic</strong> and <strong>controllable by validators</strong>.
          </p>
          <p className="text-gray-700 mb-3">
            Validators can skip blocks until they get favorable random outcomes. Any randomness derived from block data
            (timestamp, hash, height) is predictable and exploitable by those who control block production.
          </p>
          <p className="text-gray-700">
            To get truly random numbers, we can run external code off-chain with OutLayer. This code executes outside
            the blockchain environment where real entropy sources are available, then returns verifiable results back to your contract.
          </p>
        </section>

        {/* Step 1: Write WASI Code */}
        <section id="step-1">
          <h3 className="text-xl font-semibold mb-3">📝 Step 1: Write WASI Code</h3>
          <p className="text-gray-700 mb-3">
            You need a project that compiles to WebAssembly. OutLayer currently supports <strong>wasm32-wasip1</strong> and <strong>wasm32-wasip2</strong> targets.
          </p>
          <p className="text-gray-700 mb-3">
            Write code that accepts parameters from <code className="bg-gray-100 px-2 py-1 rounded">stdin</code> and
            outputs results to <code className="bg-gray-100 px-2 py-1 rounded">stdout</code>. Here&apos;s an example:
          </p>
          <SyntaxHighlighter language="rust" style={vscDarkPlus} customStyle={{ borderRadius: '0.5rem', fontSize: '0.875rem' }}>
{`use serde::{Deserialize, Serialize};
use rand::Rng;
use std::io::{self, Read};

// Input structure from stdin
#[derive(Deserialize)]
struct Input {
    min: u32,
    max: u32,
}

// Output structure to stdout
#[derive(Serialize)]
struct Output {
    random_number: u32,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Read JSON from stdin
    let mut input_string = String::new();
    io::stdin().read_to_string(&mut input_string)?;
    let input: Input = serde_json::from_str(&input_string)?;

    // Generate TRUE random number (impossible on-chain!)
    let mut rng = rand::thread_rng();
    let random_number = rng.gen_range(input.min..=input.max);

    // Output JSON to stdout
    let output = Output { random_number };
    print!("{}", serde_json::to_string(&output)?);
    Ok(())
}`}
          </SyntaxHighlighter>
          <p className="text-sm text-gray-600 mt-2">
            💡 Key point: <code className="bg-gray-100 px-2 py-1 rounded">rand::thread_rng()</code> works here but <strong>fails in smart contracts</strong>.
          </p>
          <p className="text-gray-700 mt-3 mb-3">
            Add <code className="bg-gray-100 px-2 py-1 rounded">Cargo.toml</code>:
          </p>
          <SyntaxHighlighter language="ini" style={vscDarkPlus} customStyle={{ borderRadius: '0.5rem', fontSize: '0.875rem' }}>
{`[package]
name = "random-ark"
version = "0.1.0"
edition = "2021"

[dependencies]
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
rand = "0.8"

[[bin]]
name = "random-ark"
path = "src/main.rs"`}
          </SyntaxHighlighter>
        </section>

        {/* Step 2: Push to GitHub */}
        <section id="step-2">
          <h3 className="text-xl font-semibold mb-3">📤 Step 2: Make Code Publicly Available</h3>
          <p className="text-gray-700 mb-3">
            Your code must be accessible in a <strong>public GitHub repository</strong>. OutLayer workers will compile it on-demand from the source.
          </p>
          <p className="text-gray-700 mb-3">
            You can reference your code by:
          </p>
          <ul className="list-disc list-inside space-y-2 text-gray-700 mb-3">
            <li><strong>Branch name</strong> (e.g., <code className="bg-gray-100 px-2 py-1 rounded">main</code>, <code className="bg-gray-100 px-2 py-1 rounded">develop</code>) - always uses the latest code</li>
            <li><strong>Commit hash</strong> (e.g., <code className="bg-gray-100 px-2 py-1 rounded">a1b2c3d</code>) - immutable, guarantees exact version</li>
          </ul>
          <p className="text-sm text-gray-600">
            📝 Example repo: <a href="https://github.com/zavodil/random-ark" target="_blank" rel="noopener" className="text-[var(--primary-orange)] hover:underline">github.com/zavodil/random-ark</a>
          </p>
          <p className="text-sm text-gray-600 mt-2">
            🔮 Coming soon: GitLab, Bitbucket, and other git hosting platforms
          </p>
        </section>

        {/* Step 3: Call from CLI */}
        <section id="step-3">
          <h3 className="text-xl font-semibold mb-3">🚀 Step 3: Call OutLayer Contract</h3>
          <p className="text-gray-700 mb-3">
            Now you can run your code by calling a smart contract. This will <strong>pause the blockchain transaction</strong>,
            execute your code off-chain, and send the result back into the blockchain transaction:
          </p>
          <SyntaxHighlighter language="bash" style={vscDarkPlus} customStyle={{ borderRadius: '0.5rem', fontSize: '0.875rem' }}>
{`near contract call-function as-transaction outlayer.testnet request_execution \\
  json-args '{
    "code_source": {
      "repo": "https://github.com/zavodil/random-ark",
      "commit": "main",
      "build_target": "wasm32-wasip1"
    },
    "input_data": "{\\"min\\": 1, \\"max\\": 100}",
    "resource_limits": {
      "max_instructions": 10000000000,
      "max_memory_mb": 128,
      "max_execution_seconds": 60
    },
    "response_format": "Json"
  }' \\
  prepaid-gas '300.0 Tgas' \\
  attached-deposit '0.1 NEAR' \\
  sign-as alice.testnet \\
  network-config testnet \\
  sign-with-keychain \\
  send`}
          </SyntaxHighlighter>
          <div className="bg-blue-50 border-l-4 border-blue-500 p-4 my-4">
            <p className="text-sm text-blue-800 mb-2">
              <strong>💰 Dynamic Pricing:</strong> Payment is calculated based on actual resources consumed (instructions executed, time spent).
              Unused deposit is automatically refunded at the end of the transaction.
            </p>
            <p className="text-sm text-blue-800">
              <strong>💡 Try it now:</strong> <a href="/playground?preset=Random%20Number%20Generator" className="underline font-medium">Open in Playground →</a>
            </p>
          </div>
        </section>

        {/* Step 4: Result */}
        <section id="step-4">
          <h3 className="text-xl font-semibold mb-3">✅ Step 4: Get Result</h3>
          <p className="text-gray-700 mb-3">
            Check execution result on the <Link href="/executions" className="text-[var(--primary-orange)] hover:underline">Executions</Link> page.
            Since we specified <code className="bg-gray-100 px-2 py-1 rounded">response_format: &quot;Json&quot;</code>, the result will be parsed as JSON:
          </p>
          <SyntaxHighlighter language="json" style={vscDarkPlus} customStyle={{ borderRadius: '0.5rem', fontSize: '0.875rem' }}>
{`{
  "status": "Completed",
  "result": {
    "random_number": 42
  },
  "resources_used": {
    "instructions": 1234567,
    "time_ms": 45
  }
}`}
          </SyntaxHighlighter>
        </section>

        {/* Step 5: Advanced - Contract Integration */}
        <section>
          <h3 className="text-xl font-semibold mb-3">🔥 Step 5: Use in Your Contract</h3>
          <p className="text-gray-700 mb-3">
            The most exciting part: you can integrate this off-chain code into smart contracts!
            For example, let&apos;s build a coin flip game where players guess heads or tails through a <code className="bg-gray-100 px-2 py-1 rounded">coin-toss</code> contract:
          </p>
          <SyntaxHighlighter language="rust" style={vscDarkPlus} customStyle={{ borderRadius: '0.5rem', fontSize: '0.875rem' }}>
{`use near_sdk::*;

// OutLayer contract address
// For testnet: "outlayer.testnet"
// For mainnet: "outlayer.near"
const OUTLAYER_CONTRACT_ID: &str = "outlayer.testnet";
const MIN_DEPOSIT: u128 = 10_000_000_000_000_000_000_000; // 0.01 NEAR

// External contract interface for OutLayer
#[ext_contract(ext_outlayer)]
trait OutLayer {
    fn request_execution(
        &mut self,
        code_source: serde_json::Value,
        resource_limits: serde_json::Value,
        input_data: String,
        secrets_ref: Option<serde_json::Value>,
        response_format: String,
        payer_account_id: Option<AccountId>,
    );
}

// Callback interface
#[ext_contract(ext_self)]
trait ExtSelf {
    fn on_random_result(
        &mut self,
        player: AccountId,
        choice: String,
        #[callback_result] result: Result<Option<RandomResponse>, PromiseError>,
    ) -> String;
}

#[near_bindgen]
impl CoinFlipContract {
    // Player calls this to flip the coin
    #[payable]
    pub fn flip_coin(&mut self, choice: String) -> Promise {
        let player = env::predecessor_account_id();
        let attached = env::attached_deposit().as_yoctonear();

        assert!(attached >= MIN_DEPOSIT, "Attach 0.01 NEAR");

        // Request random number from OutLayer
        ext_outlayer::ext(OUTLAYER_CONTRACT_ID.parse().unwrap())
            .with_attached_deposit(NearToken::from_yoctonear(attached))
            .with_unused_gas_weight(1)
            .request_execution(
                json!({"repo": "https://github.com/zavodil/random-ark",
                       "commit": "main", "build_target": "wasm32-wasip1"}),
                json!({"max_instructions": 10000000000,
                       "max_memory_mb": 128, "max_execution_seconds": 60}),
                "{\\"min\\": 0, \\"max\\": 1}".to_string(),
                None,
                "Json".to_string(),
                Some(player.clone()), // ✅ Refund goes to player, not to this contract
            )
            .then(
                ext_self::ext(env::current_account_id())
                    .with_static_gas(Gas::from_gas(5_000_000_000_000))
                    .on_random_result(player, choice)
            )
    }

    // Callback receives the random result
    // NOTE: This example just displays the result without any bet payout logic,
    // but you can easily add token transfers, NFT minting, or other game mechanics here
    #[private]
    pub fn on_random_result(
        &mut self,
        player: AccountId,
        choice: String,
        #[callback_result] result: Result<Option<RandomResponse>, PromiseError>,
    ) -> String {
        match result {
            Ok(Some(response)) => {
                let result = if response.random_number == 0 { "Heads" } else { "Tails" };
                if choice == result {
                    format!("🎉 You won! Result: {}", result)
                } else {
                    format!("😢 You lost. Result: {}", result)
                }
            }
            _ => "Error: OutLayer execution failed".to_string()
        }
    }
}`}
          </SyntaxHighlighter>
          <div className="bg-green-50 border-l-4 border-green-500 p-4 my-4">
            <p className="text-sm text-green-800">
              <strong>🎮 Try it with your testnet account:</strong> <a href="/playground?preset=Coin%20Flip%20Game" className="underline font-medium">Open Coin Flip in Playground →</a>
              <br />
              (Mainnet is also supported)
            </p>
          </div>
          <p className="text-sm text-gray-600 mt-3">
            📝 Example transaction: <a href="https://testnet.nearblocks.io/txns/Gq3cN9ePX6s5pt5z4snfAtjszQ9mjzzWhwEUThmHfaDm?tab=execution" target="_blank" rel="noopener" className="text-[var(--primary-orange)] hover:underline">View on NEAR Explorer</a>
          </p>
        </section>

        {/* Important Details for Developers */}
        <section>
          <h3 className="text-xl font-semibold mb-3">🔧 Important Details for Developers</h3>
          <div className="space-y-4">
            <div className="bg-gray-50 p-4 rounded-lg">
              <h4 className="font-semibold text-gray-800 mb-2">💰 Payment Flexibility</h4>
              <p className="text-gray-700 text-sm">
                You can choose who pays for off-chain execution: user or your contract.
              </p>              
            </div>

            <div className="bg-gray-50 p-4 rounded-lg">
              <h4 className="font-semibold text-gray-800 mb-2">🔒 Layer 1 Settlement</h4>
              <p className="text-gray-700 text-sm">
                Off-chain execution is fast and cheap, but <strong>final settlement stays on NEAR Layer 1</strong>.
                Your contract receives results via callback on-chain, ensuring security and auditability.
              </p>
            </div>

            <div className="bg-gray-50 p-4 rounded-lg">
              <h4 className="font-semibold text-gray-800 mb-2">🔐 Encrypted Secrets</h4>
              <p className="text-gray-700 text-sm">
                Store API keys and credentials with <Link href="/docs/secrets" className="text-[var(--primary-orange)] underline">sophisticated access control</Link>:
                whitelists, NEAR balance requirements, FT/NFT ownership, and complex logic conditions (AND/OR/NOT).
              </p>
            </div>

            <div className="bg-gray-50 p-4 rounded-lg">
              <h4 className="font-semibold text-gray-800 mb-2">📡 Blockchain Data Access</h4>
              <p className="text-gray-700 text-sm mb-2">
                WASI containers can receive blockchain context as input parameters (block height, account ID, timestamp, etc.).
              </p>
              <p className="text-gray-700 text-sm">
                Example: <a href="https://github.com/zavodil/echo-ark" target="_blank" rel="noopener" className="text-[var(--primary-orange)] underline">echo-ark</a> -
                demonstrates how to pass and process blockchain data in off-chain code.
              </p>
            </div>
          </div>
        </section>

        {/* Key Takeaways */}
        <section>
          <h3 className="text-xl font-semibold mb-3">🎓 Key Takeaways</h3>
          <div className="space-y-3">
            <div className="flex items-start space-x-3">
              <span className="text-xl">✅</span>
              <p className="text-gray-700"><strong>True randomness</strong> - impossible on-chain, trivial with OutLayer</p>
            </div>
            <div className="flex items-start space-x-3">
              <span className="text-xl">✅</span>
              <p className="text-gray-700"><strong>Simple integration</strong> - write Rust, push to GitHub, call from contract</p>
            </div>
            <div className="flex items-start space-x-3">
              <span className="text-xl">✅</span>
              <p className="text-gray-700"><strong>Secure execution</strong> - runs in TEE with attestation (coming soon)</p>
            </div>
            <div className="flex items-start space-x-3">
              <span className="text-xl">✅</span>
              <p className="text-gray-700"><strong>Automatic refunds</strong> - unused resources returned to caller</p>
            </div>
          </div>
        </section>

        {/* Next Steps */}
        <section>
          <h3 className="text-xl font-semibold mb-3">🚀 Next Steps</h3>
          <ul className="space-y-2">
            <li className="flex items-center space-x-2">
              <span className="text-[var(--primary-orange)]">→</span>
              <span>Fork <a href="https://github.com/zavodil/random-ark" target="_blank" rel="noopener" className="text-[var(--primary-orange)] hover:underline">random-ark</a> and customize it</span>
            </li>
            <li className="flex items-center space-x-2">
              <span className="text-[var(--primary-orange)]">→</span>
              <span>Explore <a href="https://github.com/zavodil/random-ark/tree/main/random-contract" target="_blank" rel="noopener" className="text-[var(--primary-orange)] hover:underline">coin flip contract</a> source code</span>
            </li>
            <li className="flex items-center space-x-2">
              <span className="text-[var(--primary-orange)]">→</span>
              <span>Learn about <Link href="/docs/secrets" className="text-[var(--primary-orange)] hover:underline">encrypted secrets</Link> for API keys</span>
            </li>
            <li className="flex items-center space-x-2">
              <span className="text-[var(--primary-orange)]">→</span>
              <span>Read <Link href="/docs/wasi" className="text-[var(--primary-orange)] hover:underline">WASI documentation</Link> for HTTP requests and more</span>
            </li>
          </ul>
        </section>
      </div>
    </div>
  );
}
