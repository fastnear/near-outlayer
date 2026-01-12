'use client';

import Link from 'next/link';
import { Prism as SyntaxHighlighter } from 'react-syntax-highlighter';
import { vscDarkPlus } from 'react-syntax-highlighter/dist/esm/styles/prism';

function AnchorHeading({ id, children, level = 2 }: { id: string; children: React.ReactNode; level?: 2 | 3 | 4 }) {
  const sizeClass = level === 2 ? 'text-2xl' : level === 3 ? 'text-xl' : 'text-lg';
  const className = `${sizeClass} font-bold text-gray-900 mb-4 scroll-mt-4 group`;
  const anchor = (
    <a href={`#${id}`} className="ml-2 text-gray-400 hover:text-[var(--primary-orange)] opacity-0 group-hover:opacity-100 transition-opacity">
      #
    </a>
  );

  if (level === 3) return <h3 id={id} className={className}>{children}{anchor}</h3>;
  if (level === 4) return <h4 id={id} className={className}>{children}{anchor}</h4>;
  return <h2 id={id} className={className}>{children}{anchor}</h2>;
}

export default function EarningsPage() {
  return (
    <div className="prose prose-lg max-w-none">
      <h1 className="text-3xl font-bold text-gray-900 mb-6">Developer Earnings</h1>

      <p className="text-gray-700 mb-8 text-lg">
        Earn USD stablecoins when users call your OutLayer projects via{' '}
        <Link href="/docs/https-api" className="text-[var(--primary-orange)] hover:underline">HTTPS API</Link>.
        Users pay you directly through the <code>X-Attached-Deposit</code> header.
      </p>

      {/* How It Works */}
      <section className="mb-12">
        <AnchorHeading id="how-it-works">How Earnings Work</AnchorHeading>

        <p className="text-gray-700 mb-4">
          When users call your project via HTTPS API, they can attach a payment to you using
          the <code>X-Attached-Deposit</code> header. This payment goes directly to your earnings balance.
        </p>

        <div className="bg-green-50 border-l-4 border-green-400 p-4 mb-6">
          <p className="text-sm text-green-800">
            <strong>Revenue model:</strong> You set your own pricing. Communicate required payment
            amounts in your documentation. Your WASM code can check the payment and adjust
            functionality accordingly (free tier, basic, premium, etc.).
          </p>
        </div>

        <AnchorHeading id="payment-flow" level={3}>Payment Flow</AnchorHeading>

        <ol className="list-decimal list-inside text-gray-700 space-y-3 mb-6">
          <li>
            <strong>User makes API call</strong> with <code>X-Attached-Deposit: 100000</code> ($0.10)
          </li>
          <li>
            <strong>Payment is charged immediately</strong> from user&apos;s Payment Key balance
          </li>
          <li>
            <strong>Your WASM receives</strong> <code>USD_PAYMENT=&quot;100000&quot;</code> environment variable
          </li>
          <li>
            <strong>On completion</strong>, payment is added to your <code>project_owner_earnings</code>
          </li>
          <li>
            <strong>You can withdraw</strong> accumulated earnings to your NEAR wallet
          </li>
        </ol>

        <div className="bg-yellow-50 border-l-4 border-yellow-400 p-4 mb-6">
          <p className="text-sm text-yellow-800">
            <strong>Important:</strong> X-Attached-Deposit is charged even if execution fails.
            This prevents abuse where users trigger errors to avoid payment.
          </p>
        </div>
      </section>

      {/* Checking Payment in WASM */}
      <section className="mb-12">
        <AnchorHeading id="checking-payment">Checking Payment in WASM</AnchorHeading>

        <p className="text-gray-700 mb-4">
          Your WASM code accesses the attached payment via the <code>USD_PAYMENT</code> environment variable.
          The value is in <strong>micro-units</strong> (1,000,000 = $1.00).
        </p>

        <SyntaxHighlighter language="rust" style={vscDarkPlus} className="rounded-lg mb-4">
          {`// Read USD payment (micro-units: 1000000 = $1.00)
let usd_payment: u64 = std::env::var("USD_PAYMENT")
    .unwrap_or_else(|_| "0".to_string())
    .parse()
    .unwrap_or(0);

// Define pricing tiers
const FREE_TIER: u64 = 0;
const BASIC_TIER: u64 = 10_000;      // $0.01
const PREMIUM_TIER: u64 = 100_000;   // $0.10
const ENTERPRISE_TIER: u64 = 1_000_000; // $1.00

// Implement tiered functionality
if usd_payment >= ENTERPRISE_TIER {
    // Full enterprise features
    enterprise_analysis(&input)?;
} else if usd_payment >= PREMIUM_TIER {
    // Premium features
    premium_processing(&input)?;
} else if usd_payment >= BASIC_TIER {
    // Basic paid features
    basic_processing(&input)?;
} else {
    // Free tier - limited functionality
    free_preview(&input)?;
}`}
        </SyntaxHighlighter>

        <AnchorHeading id="payment-validation" level={3}>Payment Validation</AnchorHeading>

        <p className="text-gray-700 mb-4">
          You can reject requests that don&apos;t meet your minimum payment requirement:
        </p>

        <SyntaxHighlighter language="rust" style={vscDarkPlus} className="rounded-lg mb-4">
          {`const MIN_PAYMENT_USD: u64 = 50_000; // $0.05 minimum

let usd_payment: u64 = std::env::var("USD_PAYMENT")
    .unwrap_or_else(|_| "0".to_string())
    .parse()
    .unwrap_or(0);

if usd_payment < MIN_PAYMENT_USD {
    eprintln!("{{\"error\": \"Minimum payment is $0.05. Please set X-Attached-Deposit: 50000\"}}");
    std::process::exit(1);
}

// Continue with paid functionality...`}
        </SyntaxHighlighter>

        <AnchorHeading id="near-vs-https" level={3}>NEAR vs HTTPS Payments</AnchorHeading>

        <p className="text-gray-700 mb-4">
          For NEAR transactions, check <code>NEAR_PAYMENT_YOCTO</code> instead:
        </p>

        <SyntaxHighlighter language="rust" style={vscDarkPlus} className="rounded-lg mb-4">
          {`let execution_type = std::env::var("OUTLAYER_EXECUTION_TYPE")
    .unwrap_or_else(|_| "NEAR".to_string());

let payment_sufficient = match execution_type.as_str() {
    "HTTPS" => {
        let usd = std::env::var("USD_PAYMENT")
            .unwrap_or_else(|_| "0".to_string())
            .parse::<u64>()
            .unwrap_or(0);
        usd >= 50_000 // $0.05 in USD
    }
    "NEAR" => {
        let near = std::env::var("NEAR_PAYMENT_YOCTO")
            .unwrap_or_else(|_| "0".to_string())
            .parse::<u128>()
            .unwrap_or(0);
        near >= 50_000_000_000_000_000_000_000 // 0.05 NEAR in yocto
    }
    _ => false,
};

if !payment_sufficient {
    eprintln!("Insufficient payment");
    std::process::exit(1);
}`}
        </SyntaxHighlighter>
      </section>

      {/* Viewing Earnings */}
      <section className="mb-12">
        <AnchorHeading id="viewing-earnings">Viewing Your Earnings</AnchorHeading>

        <AnchorHeading id="via-dashboard" level={3}>Via Dashboard</AnchorHeading>

        <ol className="list-decimal list-inside text-gray-700 space-y-2 mb-6">
          <li>Go to <Link href="/earnings" className="text-[var(--primary-orange)] hover:underline">/earnings</Link></li>
          <li>Connect your NEAR wallet</li>
          <li>View your accumulated balance and total earned</li>
          <li>See detailed history of all payments received</li>
        </ol>

        <AnchorHeading id="earnings-data" level={3}>What You Can See</AnchorHeading>

        <div className="overflow-x-auto mb-6">
          <table className="min-w-full divide-y divide-gray-200">
            <thead className="bg-gray-50">
              <tr>
                <th className="px-4 py-3 text-left text-xs font-medium text-gray-500 uppercase">Field</th>
                <th className="px-4 py-3 text-left text-xs font-medium text-gray-500 uppercase">Description</th>
              </tr>
            </thead>
            <tbody className="bg-white divide-y divide-gray-200">
              <tr>
                <td className="px-4 py-3 text-sm font-semibold">Current Balance</td>
                <td className="px-4 py-3 text-sm text-gray-600">
                  Amount available to withdraw (total earned minus withdrawn)
                </td>
              </tr>
              <tr>
                <td className="px-4 py-3 text-sm font-semibold">Total Earned</td>
                <td className="px-4 py-3 text-sm text-gray-600">
                  Lifetime earnings from all API calls (for statistics)
                </td>
              </tr>
              <tr>
                <td className="px-4 py-3 text-sm font-semibold">Payment History</td>
                <td className="px-4 py-3 text-sm text-gray-600">
                  Individual payments with timestamps, caller info, and amounts
                </td>
              </tr>
              <tr>
                <td className="px-4 py-3 text-sm font-semibold">Per-Project Stats</td>
                <td className="px-4 py-3 text-sm text-gray-600">
                  Breakdown of earnings by project
                </td>
              </tr>
            </tbody>
          </table>
        </div>
      </section>

      {/* Withdrawing Earnings */}
      <section className="mb-12">
        <AnchorHeading id="withdrawing">Withdrawing Earnings</AnchorHeading>

        <p className="text-gray-700 mb-4">
          Withdraw your accumulated earnings to receive stablecoins (USDT/USDC) to your NEAR wallet:
        </p>

        <ol className="list-decimal list-inside text-gray-700 space-y-2 mb-6">
          <li>Go to <Link href="/earnings" className="text-[var(--primary-orange)] hover:underline">/earnings</Link></li>
          <li>Click <strong>&quot;Withdraw&quot;</strong></li>
          <li>Enter amount (or withdraw full balance)</li>
          <li>Sign the transaction</li>
          <li>Stablecoins are transferred to your connected wallet</li>
        </ol>

        <div className="bg-blue-50 border-l-4 border-blue-400 p-4 mb-6">
          <p className="text-sm text-blue-800">
            <strong>Minimum withdrawal:</strong> $1.00 (1,000,000 micro-units).
            Smaller amounts can accumulate until the minimum is reached.
          </p>
        </div>
      </section>

      {/* Pricing Strategies */}
      <section className="mb-12">
        <AnchorHeading id="pricing-strategies">Pricing Strategies</AnchorHeading>

        <AnchorHeading id="tiered-pricing" level={3}>Tiered Pricing</AnchorHeading>

        <p className="text-gray-700 mb-4">
          Offer different functionality levels based on payment amount:
        </p>

        <div className="overflow-x-auto mb-6">
          <table className="min-w-full divide-y divide-gray-200">
            <thead className="bg-gray-50">
              <tr>
                <th className="px-4 py-3 text-left text-xs font-medium text-gray-500 uppercase">Tier</th>
                <th className="px-4 py-3 text-left text-xs font-medium text-gray-500 uppercase">Payment</th>
                <th className="px-4 py-3 text-left text-xs font-medium text-gray-500 uppercase">X-Attached-Deposit</th>
                <th className="px-4 py-3 text-left text-xs font-medium text-gray-500 uppercase">Features</th>
              </tr>
            </thead>
            <tbody className="bg-white divide-y divide-gray-200">
              <tr>
                <td className="px-4 py-3 text-sm">Free</td>
                <td className="px-4 py-3 text-sm">$0.00</td>
                <td className="px-4 py-3 text-sm font-mono">0</td>
                <td className="px-4 py-3 text-sm text-gray-600">Limited preview, rate limited</td>
              </tr>
              <tr>
                <td className="px-4 py-3 text-sm">Basic</td>
                <td className="px-4 py-3 text-sm">$0.01</td>
                <td className="px-4 py-3 text-sm font-mono">10000</td>
                <td className="px-4 py-3 text-sm text-gray-600">Standard functionality</td>
              </tr>
              <tr>
                <td className="px-4 py-3 text-sm">Premium</td>
                <td className="px-4 py-3 text-sm">$0.10</td>
                <td className="px-4 py-3 text-sm font-mono">100000</td>
                <td className="px-4 py-3 text-sm text-gray-600">Enhanced features, priority</td>
              </tr>
              <tr>
                <td className="px-4 py-3 text-sm">Enterprise</td>
                <td className="px-4 py-3 text-sm">$1.00</td>
                <td className="px-4 py-3 text-sm font-mono">1000000</td>
                <td className="px-4 py-3 text-sm text-gray-600">Full features, extended limits</td>
              </tr>
            </tbody>
          </table>
        </div>

        <AnchorHeading id="per-operation-pricing" level={3}>Per-Operation Pricing</AnchorHeading>

        <p className="text-gray-700 mb-4">
          Charge based on what the user requests:
        </p>

        <SyntaxHighlighter language="rust" style={vscDarkPlus} className="rounded-lg mb-4">
          {`#[derive(Deserialize)]
struct Input {
    operation: String,
    data: serde_json::Value,
}

fn main() {
    let input: Input = serde_json::from_reader(std::io::stdin()).unwrap();
    let usd_payment = get_usd_payment();

    // Different operations have different prices
    let required_payment = match input.operation.as_str() {
        "simple_query" => 5_000,       // $0.005
        "data_analysis" => 50_000,     // $0.05
        "ai_generation" => 200_000,    // $0.20
        "batch_processing" => 500_000, // $0.50
        _ => {
            eprintln!("Unknown operation");
            std::process::exit(1);
        }
    };

    if usd_payment < required_payment {
        let price = required_payment as f64 / 1_000_000.0;
        eprintln!(
            "Operation '{}' requires {} USD. Set X-Attached-Deposit: {}",
            input.operation, price, required_payment
        );
        std::process::exit(1);
    }

    // Execute the paid operation
    execute_operation(&input);
}`}
        </SyntaxHighlighter>

        <AnchorHeading id="usage-based-pricing" level={3}>Usage-Based Pricing</AnchorHeading>

        <p className="text-gray-700 mb-4">
          Calculate payment based on input size or complexity:
        </p>

        <SyntaxHighlighter language="rust" style={vscDarkPlus} className="rounded-lg mb-4">
          {`// Price per 1000 characters of input
const PRICE_PER_1K_CHARS: u64 = 1_000; // $0.001

let input_text: String = /* read from stdin */;
let input_len = input_text.len() as u64;
let required_payment = (input_len / 1000 + 1) * PRICE_PER_1K_CHARS;

let usd_payment = get_usd_payment();
if usd_payment < required_payment {
    let price = required_payment as f64 / 1_000_000.0;
    eprintln!("Input of {} chars requires {} USD", input_len, price);
    std::process::exit(1);
}`}
        </SyntaxHighlighter>
      </section>

      {/* Best Practices */}
      <section className="mb-12">
        <AnchorHeading id="best-practices">Best Practices</AnchorHeading>

        <div className="space-y-4">
          <div className="border-l-4 border-green-400 pl-4">
            <h4 className="font-semibold text-gray-900">1. Document your pricing clearly</h4>
            <p className="text-sm text-gray-600">
              Include pricing info in your project description and API documentation.
              Tell users exactly what X-Attached-Deposit values to use.
            </p>
          </div>

          <div className="border-l-4 border-green-400 pl-4">
            <h4 className="font-semibold text-gray-900">2. Provide helpful error messages</h4>
            <p className="text-sm text-gray-600">
              When payment is insufficient, tell users the exact amount needed
              and the header value to set.
            </p>
          </div>

          <div className="border-l-4 border-green-400 pl-4">
            <h4 className="font-semibold text-gray-900">3. Offer a free tier</h4>
            <p className="text-sm text-gray-600">
              Let users try your project with limited functionality before paying.
              This builds trust and increases conversions.
            </p>
          </div>

          <div className="border-l-4 border-green-400 pl-4">
            <h4 className="font-semibold text-gray-900">4. Be consistent across modes</h4>
            <p className="text-sm text-gray-600">
              If you support both NEAR transactions and HTTPS API, offer equivalent
              pricing in both modes (convert between NEAR and USD appropriately).
            </p>
          </div>

          <div className="border-l-4 border-green-400 pl-4">
            <h4 className="font-semibold text-gray-900">5. Monitor your earnings</h4>
            <p className="text-sm text-gray-600">
              Regularly check the earnings dashboard to understand usage patterns
              and optimize your pricing strategy.
            </p>
          </div>
        </div>
      </section>

      {/* Data Storage */}
      <section className="mb-12">
        <AnchorHeading id="storage">How Earnings are Tracked</AnchorHeading>

        <p className="text-gray-700 mb-4">
          Earnings data is stored in the coordinator database:
        </p>

        <div className="overflow-x-auto mb-6">
          <table className="min-w-full divide-y divide-gray-200">
            <thead className="bg-gray-50">
              <tr>
                <th className="px-4 py-3 text-left text-xs font-medium text-gray-500 uppercase">Table</th>
                <th className="px-4 py-3 text-left text-xs font-medium text-gray-500 uppercase">Purpose</th>
              </tr>
            </thead>
            <tbody className="bg-white divide-y divide-gray-200">
              <tr>
                <td className="px-4 py-3 text-sm font-mono">project_owner_earnings</td>
                <td className="px-4 py-3 text-sm text-gray-600">
                  Current balance and total earned per project owner
                </td>
              </tr>
              <tr>
                <td className="px-4 py-3 text-sm font-mono">payment_key_usage</td>
                <td className="px-4 py-3 text-sm text-gray-600">
                  Individual payment records with attached_deposit, call_id, timestamp
                </td>
              </tr>
              <tr>
                <td className="px-4 py-3 text-sm font-mono">project_owner_withdrawals</td>
                <td className="px-4 py-3 text-sm text-gray-600">
                  History of withdrawals with transaction hashes
                </td>
              </tr>
            </tbody>
          </table>
        </div>

        <p className="text-gray-700 mb-4">
          When an API call completes, the coordinator atomically:
        </p>

        <ol className="list-decimal list-inside text-gray-700 space-y-2 mb-6">
          <li>Records the payment in <code>payment_key_usage</code></li>
          <li>Updates <code>project_owner_earnings.balance</code> and <code>total_earned</code></li>
          <li>Deducts from user&apos;s Payment Key balance</li>
        </ol>
      </section>

      {/* Related Documentation */}
      <section className="bg-gray-50 rounded-lg p-6">
        <h3 className="text-lg font-semibold text-gray-900 mb-4">Related Documentation</h3>

        <ul className="space-y-2 text-sm">
          <li>
            <Link href="/docs/https-api" className="text-[var(--primary-orange)] hover:underline">HTTPS API</Link>
            {' '}- Full API reference with X-Attached-Deposit
          </li>
          <li>
            <Link href="/docs/payment-keys" className="text-[var(--primary-orange)] hover:underline">Payment Keys</Link>
            {' '}- How users fund their API access
          </li>
          <li>
            <Link href="/docs/integration-guide" className="text-[var(--primary-orange)] hover:underline">Integration Guide</Link>
            {' '}- Complete project monetization setup
          </li>
          <li>
            <Link href="/docs/wasi" className="text-[var(--primary-orange)] hover:underline">Writing WASI Code</Link>
            {' '}- Environment variables and payment checking
          </li>
        </ul>
      </section>
    </div>
  );
}
