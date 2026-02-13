'use client';

import Link from 'next/link';
import { Prism as SyntaxHighlighter } from 'react-syntax-highlighter';
import { vscDarkPlus } from 'react-syntax-highlighter/dist/esm/styles/prism';
import { AnchorHeading, useHashNavigation } from '../sections/utils';

export default function VrfPage() {
  useHashNavigation();

  return (
    <div className="max-w-5xl">
      <h1 className="text-4xl font-bold mb-3">VRF &mdash; Verifiable Random Function</h1>
      <p className="text-gray-600 mb-8">
        Cryptographically provable randomness for NEAR smart contracts. No oracle trust required &mdash; anyone can verify the proof on-chain.
      </p>

      {/* Overview */}
      <section id="overview" className="mb-10 scroll-mt-4">
        <AnchorHeading id="overview">Overview</AnchorHeading>
        <p className="text-gray-700 mb-4">
          OutLayer VRF provides <strong>verifiable random numbers</strong> for WASI modules. Unlike plain randomness (e.g. <code>getrandom</code>),
          VRF produces a cryptographic proof alongside each random output. Anyone can verify this proof &mdash; no trust in the server required.
        </p>

        <div className="bg-blue-50 border-l-4 border-blue-400 p-4 mb-4">
          <p className="text-sm text-gray-700">
            <strong>VRF vs plain random:</strong> With plain random numbers, you trust the server gave you honest results.
            With VRF, the result includes an Ed25519 signature that proves the output was computed correctly from a known key &mdash;
            verifiable by anyone, on-chain or off-chain.
          </p>
        </div>

        <p className="text-gray-700 mb-4">Each VRF call returns three values:</p>
        <ul className="list-disc list-inside text-gray-700 space-y-1 mb-4">
          <li><strong>output_hex</strong> &mdash; 32-byte random value (SHA256 of the signature)</li>
          <li><strong>signature_hex</strong> &mdash; Ed25519 signature (the proof)</li>
          <li><strong>alpha</strong> &mdash; the signed message: <code>vrf:&#123;request_id&#125;:&#123;sender_id&#125;:&#123;user_seed&#125;</code></li>
        </ul>
      </section>

      {/* How It Works */}
      <section id="how-it-works" className="mb-10 scroll-mt-4">
        <AnchorHeading id="how-it-works">How It Works</AnchorHeading>

        <SyntaxHighlighter language="text" style={vscDarkPlus} customStyle={{ borderRadius: '0.5rem', fontSize: '0.875rem' }}>
{`WASI Module                    Worker (TEE)                  Keystore (TEE)
    |                              |                              |
    |  vrf::random("coin-flip")    |                              |
    |----------------------------->|                              |
    |                              |  alpha = "vrf:42:alice.near:coin-flip"
    |                              |  POST /vrf/generate {alpha}  |
    |                              |----------------------------->|
    |                              |                              |  signature = Ed25519_sign(vrf_sk, alpha)
    |                              |                              |  output    = SHA256(signature)
    |                              |  {output_hex, signature_hex} |
    |                              |<-----------------------------|
    |  VrfOutput {                 |
    |    output_hex,               |
    |    signature_hex,            |
    |    alpha                     |
    |  }                           |
    |<-----------------------------|`}
        </SyntaxHighlighter>

        <h3 className="text-lg font-semibold mt-6 mb-2">Alpha Format</h3>
        <SyntaxHighlighter language="text" style={vscDarkPlus} customStyle={{ borderRadius: '0.5rem', fontSize: '0.875rem' }}>
{`vrf:{request_id}:{sender_id}:{user_seed}`}
        </SyntaxHighlighter>
        <ul className="list-disc list-inside text-gray-700 space-y-1 mt-2 mb-4">
          <li><strong>request_id</strong> &mdash; from blockchain event or HTTPS call ID. Auto-injected by worker, WASM cannot set it.</li>
          <li><strong>sender_id</strong> &mdash; signer account (blockchain) or payment key owner (HTTPS). Auto-injected by worker.</li>
          <li><strong>user_seed</strong> &mdash; arbitrary string from your WASM module. Must not contain <code>:</code>.</li>
        </ul>
        <p className="text-sm text-gray-600">Example: <code>vrf:98321:alice.near:coin-flip</code></p>

        <h3 className="text-lg font-semibold mt-6 mb-2">Cryptographic Primitives</h3>
        <div className="overflow-x-auto">
          <table className="min-w-full text-sm border border-gray-200 mb-4">
            <thead className="bg-gray-50">
              <tr>
                <th className="text-left px-4 py-2 border-b">Primitive</th>
                <th className="text-left px-4 py-2 border-b">Usage</th>
              </tr>
            </thead>
            <tbody>
              <tr>
                <td className="px-4 py-2 border-b font-mono">HMAC-SHA256</td>
                <td className="px-4 py-2 border-b">Key derivation: <code>HMAC-SHA256(master_secret, &quot;vrf-key&quot;)</code> &rarr; Ed25519 keypair</td>
              </tr>
              <tr>
                <td className="px-4 py-2 border-b font-mono">Ed25519 (RFC 8032)</td>
                <td className="px-4 py-2 border-b">Deterministic signature: <code>sign(vrf_sk, alpha)</code> &rarr; 64-byte signature</td>
              </tr>
              <tr>
                <td className="px-4 py-2 border-b font-mono">SHA-256</td>
                <td className="px-4 py-2 border-b">Output derivation: <code>SHA256(signature)</code> &rarr; 32-byte random output</td>
              </tr>
            </tbody>
          </table>
        </div>
      </section>

      {/* SDK Usage */}
      <section id="sdk-usage" className="mb-10 scroll-mt-4">
        <AnchorHeading id="sdk-usage">SDK Usage</AnchorHeading>
        <p className="text-gray-700 mb-4">
          Add the OutLayer SDK to your <code>Cargo.toml</code>:
        </p>

        <SyntaxHighlighter language="toml" style={vscDarkPlus} customStyle={{ borderRadius: '0.5rem', fontSize: '0.875rem' }}>
{`[dependencies]
outlayer = "0.2"`}
        </SyntaxHighlighter>

        <h3 className="text-lg font-semibold mt-6 mb-2">Generate Random Output</h3>
        <SyntaxHighlighter language="rust" style={vscDarkPlus} customStyle={{ borderRadius: '0.5rem', fontSize: '0.875rem' }}>
{`use outlayer::vrf;

// Get verifiable random output
let result = vrf::random("my-seed")?;
println!("Random: {}", result.output_hex);       // SHA256(signature), 32 bytes hex
println!("Proof:  {}", result.signature_hex);     // Ed25519 signature, 64 bytes hex
println!("Alpha:  {}", result.alpha);             // "vrf:{request_id}:{sender_id}:my-seed"

// Or get raw bytes
let (bytes, signature_hex, alpha) = vrf::random_bytes("my-seed")?;
// bytes: [u8; 32]

// Get VRF public key (for including in output)
let pubkey = vrf::public_key()?;`}
        </SyntaxHighlighter>

        <h3 className="text-lg font-semibold mt-6 mb-2">Map to a Range</h3>
        <SyntaxHighlighter language="rust" style={vscDarkPlus} customStyle={{ borderRadius: '0.5rem', fontSize: '0.875rem' }}>
{`let result = vrf::random("roll")?;
let first_4_bytes = u32::from_be_bytes(hex_to_bytes(&result.output_hex[..8]));
let roll = (first_4_bytes as u64 * 100 / (u32::MAX as u64 + 1)) as u32; // 0..=99`}
        </SyntaxHighlighter>

        <h3 className="text-lg font-semibold mt-6 mb-2">Multiple Random Values</h3>
        <p className="text-gray-700 mb-2">Use unique sub-seeds for independent values:</p>
        <SyntaxHighlighter language="rust" style={vscDarkPlus} customStyle={{ borderRadius: '0.5rem', fontSize: '0.875rem' }}>
{`for i in 0..5 {
    let result = vrf::random(&format!("card:{}", i))?;
    // Each call gets a unique alpha -> unique output
}`}
        </SyntaxHighlighter>

        <h3 className="text-lg font-semibold mt-6 mb-2">Constraints</h3>
        <ul className="list-disc list-inside text-gray-700 space-y-1 mb-4">
          <li><code>user_seed</code> must not contain <code>:</code> (used as alpha delimiter)</li>
          <li>Max 10 VRF calls per execution</li>
          <li>VRF requires keystore &mdash; project must be deployed on OutLayer</li>
        </ul>
      </section>

      {/* On-Chain Verification */}
      <section id="on-chain-verification" className="mb-10 scroll-mt-4">
        <AnchorHeading id="on-chain-verification">On-Chain Verification</AnchorHeading>
        <p className="text-gray-700 mb-4">
          Verify VRF output in a NEAR smart contract using native <code>ed25519_verify</code> (~1 TGas):
        </p>

        <SyntaxHighlighter language="rust" style={vscDarkPlus} customStyle={{ borderRadius: '0.5rem', fontSize: '0.875rem' }}>
{`use near_sdk::env;

fn verify_vrf(
    vrf_pubkey: &[u8; 32],   // from GET /vrf/pubkey or hardcoded
    alpha: &str,              // from VRF output
    signature: &[u8; 64],    // from VRF output (signature_hex decoded)
) -> bool {
    env::ed25519_verify(signature, alpha.as_bytes(), vrf_pubkey)
}`}
        </SyntaxHighlighter>

        <h3 className="text-lg font-semibold mt-6 mb-2">Deploying a Contract with VRF</h3>

        <p className="text-gray-700 mb-2"><strong>Step 1.</strong> Get the VRF public key:</p>
        <SyntaxHighlighter language="bash" style={vscDarkPlus} customStyle={{ borderRadius: '0.5rem', fontSize: '0.875rem' }}>
{`# Mainnet
curl -s https://api.outlayer.fastnear.com/vrf/pubkey | jq -r .vrf_public_key_hex

# Testnet
curl -s https://testnet-api.outlayer.fastnear.com/vrf/pubkey | jq -r .vrf_public_key_hex`}
        </SyntaxHighlighter>

        <p className="text-gray-700 mt-4 mb-2"><strong>Step 2.</strong> Initialize contract with the pubkey:</p>
        <SyntaxHighlighter language="bash" style={vscDarkPlus} customStyle={{ borderRadius: '0.5rem', fontSize: '0.875rem' }}>
{`near call my-vrf.near new '{
  "outlayer_contract_id": "outlayer.near",
  "project_id": "alice.near/vrf-ark",
  "vrf_pubkey_hex": "a1b2c3d4..."
}' --accountId my-vrf.near`}
        </SyntaxHighlighter>

        <p className="text-gray-700 mt-4 mb-2"><strong>Step 3.</strong> Verify it was stored:</p>
        <SyntaxHighlighter language="bash" style={vscDarkPlus} customStyle={{ borderRadius: '0.5rem', fontSize: '0.875rem' }}>
{`near view my-vrf.near get_vrf_pubkey
# "a1b2c3d4..."`}
        </SyntaxHighlighter>

        <div className="bg-yellow-50 border-l-4 border-yellow-400 p-4 mt-4">
          <p className="text-sm text-gray-700">
            If the keystore rotates the VRF key (rare), update via <code>set_vrf_pubkey</code> (contract owner only).
          </p>
        </div>

        <h3 className="text-lg font-semibold mt-6 mb-2">Complete Example: Coin Flip</h3>
        <p className="text-gray-700 mb-2">WASI module generates VRF, contract verifies:</p>
        <SyntaxHighlighter language="rust" style={vscDarkPlus} customStyle={{ borderRadius: '0.5rem', fontSize: '0.875rem' }}>
{`// 1. Contract requests execution
ext_outlayer::ext(outlayer_contract_id)
    .with_attached_deposit(NearToken::from_millinear(50))
    .request_execution(
        json!({"Project": {"project_id": "alice.near/vrf-ark"}}),
        Some(resource_limits),
        Some(r#"{"seed":"coin-flip","max":1}"#.to_string()),
        None,
        Some("Json".to_string()),
        Some(player.clone()),
    )
    .then(ext_self::ext(current_account_id()).on_vrf_result(player, choice));

// 2. In callback - verify proof
let entry = &vrf_response.results[0];
let sig_bytes: [u8; 64] = hex::decode(&entry.signature_hex).try_into().unwrap();
let valid = env::ed25519_verify(&sig_bytes, entry.alpha.as_bytes(), &self.vrf_pubkey);
assert!(valid, "VRF proof verification failed");`}
        </SyntaxHighlighter>

        <p className="text-sm text-gray-600 mt-2">
          Full contract example: <a href="https://github.com/zavodil/vrf-ark/tree/main/vrf-contract" target="_blank" rel="noopener noreferrer" className="text-[var(--primary-orange)] hover:underline">vrf-contract on GitHub</a>
        </p>
      </section>

      {/* Security Properties */}
      <section id="security" className="mb-10 scroll-mt-4">
        <AnchorHeading id="security">Security Properties</AnchorHeading>

        <div className="space-y-4">
          <div className="border border-gray-200 rounded-lg p-4">
            <h4 className="font-semibold text-gray-900 mb-1">1. Deterministic &mdash; no re-rolling</h4>
            <p className="text-sm text-gray-700">Ed25519 signatures are deterministic per RFC 8032. Same key + same alpha = same signature = same output. The worker cannot retry to get a different result.</p>
          </div>

          <div className="border border-gray-200 rounded-lg p-4">
            <h4 className="font-semibold text-gray-900 mb-1">2. Unpredictable without the key</h4>
            <p className="text-sm text-gray-700">The VRF private key lives only inside TEE (Intel TDX via Phala Cloud). It is derived from the master secret via <code>HMAC-SHA256(master_secret, &quot;vrf-key&quot;)</code>. The master secret is distributed through MPC key ceremony &mdash; no single party holds it.</p>
          </div>

          <div className="border border-gray-200 rounded-lg p-4">
            <h4 className="font-semibold text-gray-900 mb-1">3. Non-manipulable alpha</h4>
            <p className="text-sm text-gray-700">The WASM module only provides <code>user_seed</code>. The worker auto-prepends <code>request_id</code> (from the blockchain event) and <code>sender_id</code> (the caller&apos;s account). Same seed by different users or different requests produces different output.</p>
          </div>

          <div className="border border-gray-200 rounded-lg p-4">
            <h4 className="font-semibold text-gray-900 mb-1">4. Publicly verifiable</h4>
            <p className="text-sm text-gray-700">Anyone can verify the VRF output &mdash; no trust in the TEE required: <code>ed25519_verify(vrf_pubkey, alpha, signature)</code> and <code>SHA256(signature) == output</code>.</p>
          </div>

          <div className="border border-gray-200 rounded-lg p-4">
            <h4 className="font-semibold text-gray-900 mb-1">5. Consistent across keystore instances</h4>
            <p className="text-sm text-gray-700">All keystore instances derive the VRF keypair from the same master secret with fixed seed <code>&quot;vrf-key&quot;</code>. All instances produce the same public key and same output for same alpha.</p>
          </div>

          <div className="border border-gray-200 rounded-lg p-4">
            <h4 className="font-semibold text-gray-900 mb-1">6. Rate-limited</h4>
            <p className="text-sm text-gray-700">Max 10 VRF calls per WASM execution. Prevents abuse of the signing endpoint.</p>
          </div>
        </div>
      </section>

      {/* User Verification Guide */}
      <section id="user-verification" className="mb-10 scroll-mt-4">
        <AnchorHeading id="user-verification">User Verification Guide</AnchorHeading>

        <h3 className="text-lg font-semibold mt-4 mb-2">1. Get the VRF Public Key</h3>
        <SyntaxHighlighter language="bash" style={vscDarkPlus} customStyle={{ borderRadius: '0.5rem', fontSize: '0.875rem' }}>
{`curl https://api.outlayer.fastnear.com/vrf/pubkey
# {"vrf_public_key_hex":"a1b2c3d4..."}  (64 hex chars = 32 bytes)`}
        </SyntaxHighlighter>

        <h3 className="text-lg font-semibold mt-6 mb-2">2. Verify with Python (PyNaCl)</h3>
        <SyntaxHighlighter language="python" style={vscDarkPlus} customStyle={{ borderRadius: '0.5rem', fontSize: '0.875rem' }}>
{`from nacl.signing import VerifyKey
import hashlib

vrf_pubkey_hex = "..."   # from /vrf/pubkey
signature_hex = "..."     # from result
alpha = "vrf:98321:alice.near:coin-flip"

vrf_pubkey = bytes.fromhex(vrf_pubkey_hex)
signature = bytes.fromhex(signature_hex)

# Verify: Ed25519 signature over alpha
verify_key = VerifyKey(vrf_pubkey)
verify_key.verify(alpha.encode(), signature)  # raises if invalid
print("Signature VALID")

# Verify: output = SHA256(signature)
output = hashlib.sha256(signature).hexdigest()
print(f"Output: {output}")

# If mapped to range: first 4 bytes -> u32 -> scale
first_4 = int(output[:8], 16)
mapped = first_4 * (max_value + 1) // (2**32)
print(f"Mapped value: {mapped}")`}
        </SyntaxHighlighter>

        <h3 className="text-lg font-semibold mt-6 mb-2">3. Verify with JavaScript (tweetnacl)</h3>
        <SyntaxHighlighter language="javascript" style={vscDarkPlus} customStyle={{ borderRadius: '0.5rem', fontSize: '0.875rem' }}>
{`import nacl from 'tweetnacl';
import { createHash } from 'crypto';

const vrfPubkey = Buffer.from(vrfPubkeyHex, 'hex');
const signature = Buffer.from(signatureHex, 'hex');
const alpha = Buffer.from('vrf:98321:alice.near:coin-flip');

// Verify signature
const valid = nacl.sign.detached.verify(alpha, signature, vrfPubkey);
console.log('Valid:', valid);

// Verify output
const output = createHash('sha256').update(signature).digest('hex');
console.log('Output:', output);`}
        </SyntaxHighlighter>

        <h3 className="text-lg font-semibold mt-6 mb-2">Verification Checklist</h3>
        <ol className="list-decimal list-inside text-gray-700 space-y-1 mb-4">
          <li><code>ed25519_verify(vrf_pubkey, alpha, signature)</code> &mdash; signature is valid</li>
          <li><code>SHA256(signature) == output_hex</code> &mdash; output matches signature</li>
          <li>Alpha contains correct <code>request_id</code> from blockchain event</li>
          <li>Alpha contains correct <code>sender_id</code> (the caller)</li>
          <li>VRF public key matches <code>GET /vrf/pubkey</code></li>
        </ol>
        <p className="text-sm text-gray-600">If all 5 checks pass, the random output is provably correct and was not manipulated.</p>
      </section>

      {/* API Reference */}
      <section id="api-reference" className="mb-10 scroll-mt-4">
        <AnchorHeading id="api-reference">API Reference</AnchorHeading>

        <h3 className="text-lg font-semibold mt-4 mb-2">Endpoint</h3>
        <div className="overflow-x-auto">
          <table className="min-w-full text-sm border border-gray-200 mb-4">
            <thead className="bg-gray-50">
              <tr>
                <th className="text-left px-4 py-2 border-b">Endpoint</th>
                <th className="text-left px-4 py-2 border-b">Method</th>
                <th className="text-left px-4 py-2 border-b">Auth</th>
                <th className="text-left px-4 py-2 border-b">Response</th>
              </tr>
            </thead>
            <tbody>
              <tr>
                <td className="px-4 py-2 border-b font-mono">/vrf/pubkey</td>
                <td className="px-4 py-2 border-b">GET</td>
                <td className="px-4 py-2 border-b">Public</td>
                <td className="px-4 py-2 border-b"><code>{`{"vrf_public_key_hex": "..."}`}</code></td>
              </tr>
            </tbody>
          </table>
        </div>

        <h3 className="text-lg font-semibold mt-4 mb-2">SDK Functions</h3>
        <div className="overflow-x-auto">
          <table className="min-w-full text-sm border border-gray-200 mb-4">
            <thead className="bg-gray-50">
              <tr>
                <th className="text-left px-4 py-2 border-b">Function</th>
                <th className="text-left px-4 py-2 border-b">Returns</th>
                <th className="text-left px-4 py-2 border-b">Description</th>
              </tr>
            </thead>
            <tbody>
              <tr>
                <td className="px-4 py-2 border-b font-mono">vrf::random(seed)</td>
                <td className="px-4 py-2 border-b font-mono">Result&lt;VrfOutput&gt;</td>
                <td className="px-4 py-2 border-b">Random output + proof</td>
              </tr>
              <tr>
                <td className="px-4 py-2 border-b font-mono">vrf::random_bytes(seed)</td>
                <td className="px-4 py-2 border-b font-mono">Result&lt;([u8; 32], String, String)&gt;</td>
                <td className="px-4 py-2 border-b">Raw bytes + signature + alpha</td>
              </tr>
              <tr>
                <td className="px-4 py-2 border-b font-mono">vrf::public_key()</td>
                <td className="px-4 py-2 border-b font-mono">Result&lt;String&gt;</td>
                <td className="px-4 py-2 border-b">VRF public key hex</td>
              </tr>
            </tbody>
          </table>
        </div>

        <h3 className="text-lg font-semibold mt-4 mb-2">Related Resources</h3>
        <ul className="text-sm text-gray-700 space-y-1">
          <li>
            <Link href="/docs/examples#vrf-ark" className="text-[var(--primary-orange)] hover:underline">
              VRF Example Project (vrf-ark)
            </Link>
          </li>
          <li>
            <Link href="/playground#vrf-coin-flip" className="text-[var(--primary-orange)] hover:underline">
              Try VRF in Playground
            </Link>
          </li>
          <li>
            <a href="https://github.com/zavodil/vrf-ark" target="_blank" rel="noopener noreferrer" className="text-[var(--primary-orange)] hover:underline">
              Source Code on GitHub
            </a>
          </li>
        </ul>
      </section>
    </div>
  );
}
