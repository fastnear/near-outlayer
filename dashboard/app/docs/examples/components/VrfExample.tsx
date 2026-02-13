import Link from 'next/link';
import { ExampleCard, UseCasesSection, KeyFeaturesSection, TechnicalDetailsSection, CodeExampleSection, HowItWorksSection, LearnMoreSection, SecurityNotesSection } from './';

export function VrfExample() {
  const badges = (
    <>
      <span className="ml-3 text-sm bg-blue-100 text-blue-800 px-3 py-1 rounded">WASI P1</span>
      <span className="ml-2 text-sm bg-yellow-100 text-yellow-800 px-3 py-1 rounded">Intermediate</span>
    </>
  );

  return (
    <ExampleCard
      id="vrf-ark"
      title="vrf-ark"
      badges={badges}
      githubUrl="https://github.com/zavodil/vrf-ark"
      playgroundId="vrf-coin-flip"
    >
      <p className="text-gray-700 mb-4">
        Generate <strong>verifiable random numbers</strong> with cryptographic proof. Unlike plain randomness,
        each VRF output includes an Ed25519 signature that anyone can verify on-chain &mdash; no trust in the server required.
        Includes a companion NEAR smart contract for provably fair coin flip with on-chain proof verification.
      </p>

      <UseCasesSection items={[
        'Provably fair gaming and gambling (coin flip, dice, card games)',
        'Fair lottery and raffle systems with verifiable draws',
        'Random NFT trait/rarity assignment',
        'Unbiased leader election and committee selection',
        'Any application where users must trust the randomness is genuine'
      ]} />

      <KeyFeaturesSection items={[
        <>Cryptographic proof via <strong>Ed25519 signature</strong> &mdash; verifiable by anyone</>,
        <>On-chain verification using native <code>env::ed25519_verify</code> (~1 TGas)</>,
        'Generate up to 10 random values per execution with unique sub-seeds',
        <>Range mapping: map raw output to <code>0..=max</code> (e.g. coin flip: max=1)</>,
        'Non-manipulable alpha: request_id + sender_id auto-injected by worker',
        'Includes VRF public key and verification instructions in output'
      ]} />

      <CodeExampleSection
        title="Input Format:"
        code={`{
  "seed": "coin-flip",
  "max": 1,
  "count": 1
}`}
        description="Generate 1 random number mapped to 0 (Heads) or 1 (Tails)"
      />

      <CodeExampleSection
        title="Output Format:"
        code={`{
  "signer": "alice.near",
  "total_calls": 5,
  "results": [
    {
      "value": 0,
      "signature_hex": "abcd...1234",
      "alpha": "vrf:98321:alice.near:coin-flip"
    }
  ],
  "verification": {
    "vrf_public_key": "a1b2c3d4...",
    "steps": ["1. Get VRF public key...", "2. Verify ed25519...", "..."],
    "pubkey_endpoint": "https://api.outlayer.fastnear.com/vrf/pubkey",
    "near_code": "let valid = env::ed25519_verify(...);"
  }
}`}
      />

      <CodeExampleSection
        title="WASI Code (Rust):"
        code={`use outlayer::vrf;

let result = vrf::random("coin-flip")?;
// result.output_hex  -> SHA256(signature), 32 bytes hex
// result.signature_hex -> Ed25519 signature (the proof)
// result.alpha -> "vrf:{request_id}:{sender_id}:coin-flip"

// Map to range 0..=max
let bytes = hex_to_u32(&result.output_hex);
let side = (bytes as u64 * 2 / (u32::MAX as u64 + 1)) as u32; // 0 or 1`}
        language="rust"
      />

      <HowItWorksSection items={[
        <>WASM module calls <code>vrf::random(&quot;coin-flip&quot;)</code> via OutLayer SDK</>,
        <>Worker constructs alpha: <code>vrf:&#123;request_id&#125;:&#123;sender_id&#125;:coin-flip</code></>,
        'Worker sends alpha to Keystore TEE for signing',
        <>Keystore signs with Ed25519: <code>signature = sign(vrf_sk, alpha)</code></>,
        <>Output derived: <code>SHA256(signature)</code> &rarr; 32 random bytes</>,
        <>Contract callback verifies proof: <code>env::ed25519_verify(sig, alpha, pubkey)</code></>
      ]} />

      <TechnicalDetailsSection items={[
        <><strong>WASI Version:</strong> Preview 1 (<code>wasm32-wasip1</code>)</>,
        <><strong>Language:</strong> Rust with <code>outlayer</code> SDK</>,
        <><strong>Entry Point:</strong> <code>main()</code> reads stdin JSON, writes stdout JSON</>,
        <><strong>Crypto:</strong> Ed25519 (deterministic, RFC 8032) + SHA-256</>,
        <><strong>Verification Cost:</strong> ~1 TGas (native NEAR ed25519_verify)</>,
        <><strong>Rate Limit:</strong> Max 10 VRF calls per execution</>
      ]} />

      <SecurityNotesSection items={[
        <>Alpha includes <code>request_id</code> and <code>sender_id</code> auto-injected by the worker &mdash; WASM module cannot forge these values</>,
        'Ed25519 is deterministic: same key + same alpha = same output, preventing re-rolling',
        'VRF private key lives only inside TEE (Intel TDX), derived via MPC key ceremony',
        <>Anyone can verify: <code>ed25519_verify(pubkey, alpha, signature)</code> + <code>SHA256(signature) == output</code></>
      ]} />

      <LearnMoreSection>
        <ul className="text-sm text-gray-700 space-y-1">
          <li>
            <Link href="/docs/vrf" className="text-[var(--primary-orange)] hover:underline">
              VRF Documentation &mdash; full technical guide
            </Link>
          </li>
          <li>
            <Link href="/playground#vrf-coin-flip" className="text-[var(--primary-orange)] hover:underline">
              Try VRF in Playground
            </Link>
          </li>
          <li>
            <a href="https://github.com/zavodil/vrf-ark" target="_blank" rel="noopener noreferrer" className="text-[var(--primary-orange)] hover:underline">
              View Source Code
            </a>
          </li>
          <li>
            <a href="https://github.com/zavodil/vrf-ark/tree/main/vrf-contract" target="_blank" rel="noopener noreferrer" className="text-[var(--primary-orange)] hover:underline">
              Coin Flip Contract Example
            </a>
          </li>
        </ul>
      </LearnMoreSection>
    </ExampleCard>
  );
}
