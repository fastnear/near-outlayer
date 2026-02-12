'use client';

import { AnchorHeading, useHashNavigation } from './utils';

export default function PricingSection() {
  useHashNavigation();

  return (
    <div className="prose max-w-none">
      <h2 className="text-3xl font-bold mb-6 text-[var(--primary-orange)]">Pricing & Limits</h2>

      <div className="space-y-6">
        <section id="dynamic-pricing">
          <AnchorHeading id="dynamic-pricing">Dynamic Pricing Model</AnchorHeading>
          <p className="text-gray-700">
            Pay only for resources you use. Pricing is based on requested resource limits, not fixed fees.
            Excess payment is automatically refunded after execution.
          </p>
        </section>

        <section id="cost-calculation">
          <AnchorHeading id="cost-calculation">Cost Calculation</AnchorHeading>
          <p className="text-gray-700">
            Execution cost = <code className="bg-gray-100 px-2 py-1 rounded">base_fee + (instructions × instruction_rate) + (time_ms × time_rate)</code>
          </p>
          <p className="text-gray-700 mt-2">
            Use the <code className="bg-gray-100 px-2 py-1 rounded">estimate_execution_cost</code> view method to calculate
            costs before submitting a request.
          </p>
        </section>

        <section id="resource-limits">
          <AnchorHeading id="resource-limits">Resource Limits</AnchorHeading>
          <ul className="list-disc list-inside space-y-2 text-gray-700">
            <li><strong>Max Instructions:</strong> 500 billion instructions per execution</li>
            <li><strong>Max Memory:</strong> Configurable up to platform limits</li>
            <li><strong>Max Execution Time:</strong> 180 seconds per execution (default: 60 seconds)</li>
            <li><strong>Max Compilation Time:</strong> Enforced during GitHub compilation</li>
          </ul>
          <p className="text-sm text-gray-500 mt-3">
            These limits are configured in the smart contract and may change without documentation updates.
            See current values at <a href="/stats" className="text-[var(--primary-orange)] underline">Stats</a>.
          </p>
        </section>

        <section id="refund-policy">
          <AnchorHeading id="refund-policy">Refund Policy</AnchorHeading>
          <p className="text-gray-700">
            If your execution uses less resources than requested, the difference is automatically refunded.
            However, failed executions are not refunded (anti-DoS protection).
          </p>
        </section>

        <section id="optimization-tips">
          <AnchorHeading id="optimization-tips">Optimization Tips</AnchorHeading>
          <ul className="list-disc list-inside space-y-2 text-gray-700">
            <li>Request only the resources you need to minimize upfront costs</li>
            <li>Optimize your WASM code to reduce instruction count</li>
            <li>Use immutable WASM storage to avoid repeated compilation costs</li>
            <li>Consider caching results in your smart contract for frequently-accessed data</li>
          </ul>
        </section>
      </div>
    </div>
  );
}
