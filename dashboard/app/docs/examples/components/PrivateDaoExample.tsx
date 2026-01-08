import Link from 'next/link';
import { Prism as SyntaxHighlighter } from 'react-syntax-highlighter';
import { vscDarkPlus } from 'react-syntax-highlighter/dist/esm/styles/prism';
import { ExampleCard, UseCasesSection, KeyFeaturesSection, TechnicalDetailsSection, LearnMoreSection, SecurityNotesSection } from './';

export function PrivateDaoExample() {
  const badges = (
    <>
      <span className="ml-3 text-sm bg-purple-100 text-purple-800 px-3 py-1 rounded">WASI P1</span>
      <span className="ml-2 text-sm bg-red-100 text-red-800 px-3 py-1 rounded">Advanced</span>
    </>
  );

  return (
    <ExampleCard
      id="private-dao-ark"
      title="private-dao-ark"
      badges={badges}
      githubUrl="https://github.com/zavodil/private-dao-ark"
    >
      <p className="text-gray-700 mb-4">
        Anonymous, verifiable DAO voting with cryptographic privacy. Heavy cryptography (ECIES encryption, HKDF key derivation, merkle tree proofs) executed off-chain in TEE. Each user&apos;s vote is encrypted on-chain, tallying happens in secure enclave, only aggregate counts are revealed.
      </p>

      <div className="mb-4 p-3 bg-purple-50 border-l-4 border-purple-400">
        <p className="text-sm text-gray-700 mb-2">
          🔐 <strong>Privacy Guarantees:</strong>
        </p>
        <ul className="text-sm text-gray-700 space-y-1 ml-4 list-disc">
          <li>Individual votes remain secret - only aggregate counts revealed</li>
          <li>DAO members can send encrypted noise instead of real votes to hide voting activity from observers</li>
          <li>Merkle proofs allow voters to verify their vote was counted without revealing how they voted</li>
        </ul>
      </div>

      <KeyFeaturesSection items={[
        'ECIES encryption for private votes (secp256k1)',
        'HKDF-SHA256 deterministic key derivation from single master secret',
        'Merkle tree construction for vote inclusion proofs',
        <><strong>Dummy messages:</strong> Send encrypted noise to hide whether you voted (indistinguishable from real votes on-chain)</>,
        <><strong>Vote changes:</strong> Vote multiple times, timestamp-based deduplication (latest vote wins)</>,
        'TEE attestation for execution integrity',
        'Full-stack React frontend with NEAR Wallet integration'
      ]} />

      <h4 className="font-semibold mt-4 mb-2">Architecture:</h4>
      <SyntaxHighlighter language="text" style={vscDarkPlus} customStyle={{ borderRadius: '0.5rem', fontSize: '0.75rem' }}>
{`1. Generate master secret:
   OutLayer → TEE generates random master secret (stored encrypted)

2. User joins DAO:
   Contract → OutLayer → TEE derives pubkey from master secret

3. User votes:
   Frontend encrypts vote with pubkey → Contract stores encrypted vote

4. Finalize proposal:
   Contract → OutLayer → TEE decrypts votes + tallies + builds merkle tree
   Returns aggregate counts + merkle proofs (individual votes never exposed)

5. Verify vote:
   User computes vote hash → Verifies merkle proof against root

Privacy Features:
- Individual votes never revealed (only aggregate counts)
- Dummy messages: Send encrypted noise to hide voting activity
- Vote changes: Vote multiple times, only latest counts (timestamped)
- Merkle proofs: Verify inclusion without revealing vote content

Cost: Heavy cryptography off-chain = ~$0.001 per vote`}
      </SyntaxHighlighter>

      <h4 className="font-semibold mt-4 mb-2">Cryptographic Components:</h4>
      <div className="space-y-2 mb-4">
        <div className="border-l-4 border-blue-400 pl-3">
          <strong className="text-sm">HKDF Key Derivation</strong>
          <SyntaxHighlighter language="rust" style={vscDarkPlus} customStyle={{ borderRadius: '0.5rem', fontSize: '0.75rem', marginTop: '0.5rem' }}>
{`// Single master secret → unique key per user
let info = format!("user:{}:{}", dao_account, user_account);
let user_privkey = hkdf_sha256(&master_secret, info.as_bytes());
let user_pubkey = secp256k1::derive_public_key(&user_privkey);`}
          </SyntaxHighlighter>
        </div>

        <div className="border-l-4 border-green-400 pl-3">
          <strong className="text-sm">ECIES Encryption (Frontend)</strong>
          <SyntaxHighlighter language="typescript" style={vscDarkPlus} customStyle={{ borderRadius: '0.5rem', fontSize: '0.75rem', marginTop: '0.5rem' }}>
{`import { encrypt } from 'eciesjs';

const vote = "yes";  // or "no"
const pubkeyHex = await contract.get_user_pubkey({ user });
const encrypted = encrypt(pubkeyHex, Buffer.from(vote));
await contract.cast_vote({ proposal_id, encrypted_vote: encrypted.toString('hex') });`}
          </SyntaxHighlighter>
        </div>

        <div className="border-l-4 border-purple-400 pl-3">
          <strong className="text-sm">Vote Hash Computation (Critical!)</strong>
          <SyntaxHighlighter language="typescript" style={vscDarkPlus} customStyle={{ borderRadius: '0.5rem', fontSize: '0.75rem', marginTop: '0.5rem' }}>
{`// Must preserve u64 precision - use BigInt!
const timestamp = result.receipts_outcome[0].outcome.status.SuccessValue;
const timestampStr = atob(timestamp).trim();  // Keep as string
const timestampBigInt = BigInt(timestampStr);

// Convert to 8-byte little-endian
const buffer = new ArrayBuffer(8);
new DataView(buffer).setBigUint64(0, timestampBigInt, true);

// SHA256(user + timestamp_le + encrypted)
const combined = concat(
  textEncoder.encode(accountId),
  new Uint8Array(buffer),
  textEncoder.encode(encrypted)
);
const voteHash = hex(await crypto.subtle.digest('SHA-256', combined));`}
          </SyntaxHighlighter>
        </div>

        <div className="border-l-4 border-orange-400 pl-3">
          <strong className="text-sm">Merkle Proof Verification</strong>
          <SyntaxHighlighter language="typescript" style={vscDarkPlus} customStyle={{ borderRadius: '0.5rem', fontSize: '0.75rem', marginTop: '0.5rem' }}>
{`// Try all possible paths (2^depth combinations)
async function verifyProof(voteHash, proofPath, merkleRoot) {
  async function tryPaths(hash, remaining) {
    if (!remaining.length) return hash === merkleRoot;

    const [sibling, ...rest] = remaining;

    // Try both orderings
    if (await tryPaths(await sha256(hash + sibling), rest)) return true;
    if (await tryPaths(await sha256(sibling + hash), rest)) return true;

    return false;
  }
  return await tryPaths(voteHash, proofPath);
}`}
          </SyntaxHighlighter>
        </div>
      </div>

      <h4 className="font-semibold mt-4 mb-2">How to Deploy:</h4>
      <SyntaxHighlighter language="bash" style={vscDarkPlus} customStyle={{ borderRadius: '0.5rem', fontSize: '0.875rem' }}>
{`# 1. Generate DAO master secret in TEE
# (Alternatively: generate locally and store encrypted)
openssl rand -hex 32 > dao_master_secret.txt

# 2. Store master secret via Dashboard (encrypted in keystore)
# Open https://outlayer.fastnear.com/secrets:
# - Repo: github.com/YOUR_USERNAME/private-dao-ark
# - Profile: production
# - Secrets JSON: {"DAO_MASTER_SECRET":"<paste hex from file>"}
# - Access: AllowAll (or Whitelist for DAO admin only)

# 3. Clone and build WASI module
git clone https://github.com/zavodil/private-dao-ark.git
cd private-dao-ark
cargo build --target wasm32-wasip1 --release
git push origin main

# 4. Deploy DAO contract
cd dao-contract
cargo near build
near deploy privatedao.testnet \\
  use-file res/private_dao_contract.wasm \\
  with-init-call new \\
  json-args '{
    "owner_id":"owner.testnet",
    "name":"My Private DAO",
    "outlayer_contract":"outlayer.testnet",
    "wasi_repo":"https://github.com/YOUR_USERNAME/private-dao-ark",
    "wasi_commit":"main",
    "secrets_profile":"production",
    "secrets_owner":"your.testnet",
    "membership_mode":"Public"
  }' \\
  prepaid-gas '100.0 Tgas' \\
  attached-deposit '0 NEAR'

# 5. Deploy frontend
cd ../dao-frontend
npm install
cat > .env <<EOF
REACT_APP_CONTRACT_ID=privatedao.testnet
REACT_APP_NEAR_NETWORK=testnet
EOF
npm run build
# Deploy build/ to Vercel/Netlify/Cloudflare Pages

# 6. Users can now:
# - Join DAO (get encrypted pubkey derived from master secret)
# - Create proposals with quorum requirements
# - Vote privately (votes encrypted with their pubkey)
# - Finalize proposals (OutLayer decrypts in TEE and tallies)
# - Verify their vote was counted (merkle proof verification)`}
      </SyntaxHighlighter>

      <UseCasesSection items={[
        <><strong>Anonymous Governance:</strong> Board elections where individual votes should remain secret</>,
        <><strong>Whistleblower Protection:</strong> Report issues without revealing identity</>,
        <><strong>Salary Decisions:</strong> Vote on compensation without peer pressure</>,
        <><strong>Grant Allocation:</strong> Fund projects while preventing vote buying</>,
        <><strong>Conflict Resolution:</strong> Vote on sensitive matters privately</>
      ]} />

      <h4 className="font-semibold mt-4 mb-2">Technical Highlights:</h4>
      <div className="grid grid-cols-1 md:grid-cols-2 gap-3 mb-4">
        <div className="p-3 bg-gray-50 border border-gray-200 rounded">
          <strong className="text-sm block mb-1">💰 Cost Efficiency</strong>
          <p className="text-xs text-gray-700">Heavy cryptography off-chain: ~$0.001/vote<br/>ECIES + HKDF + Merkle trees feasible with OutLayer</p>
        </div>
        <div className="p-3 bg-gray-50 border border-gray-200 rounded">
          <strong className="text-sm block mb-1">🔒 Privacy Model</strong>
          <p className="text-xs text-gray-700">Encrypted votes on-chain<br/>Dummy messages hide voting activity<br/>Vote changes allowed (latest wins)<br/>Decryption in secure enclave</p>
        </div>
        <div className="p-3 bg-gray-50 border border-gray-200 rounded">
          <strong className="text-sm block mb-1">✅ Verifiability</strong>
          <p className="text-xs text-gray-700">Merkle proofs: Voters verify inclusion<br/>TEE attestation: Verify execution integrity</p>
        </div>
        <div className="p-3 bg-gray-50 border border-gray-200 rounded">
          <strong className="text-sm block mb-1">⚡ Scalability</strong>
          <p className="text-xs text-gray-700">Master secret in TEE → unlimited users<br/>No storage overhead for keys</p>
        </div>
      </div>

      <SecurityNotesSection items={[
        'Master secret must be highly secured (hardware wallet, multi-sig, etc.)',
        'TEE attestation uses Intel TDX via Phala Network',
        'Frontend must correctly compute vote hash (BigInt for u64 precision!)',
        'Vote hash saved by user is CRITICAL for later verification'
      ]} />

      <TechnicalDetailsSection items={[
        <><strong>WASI Version:</strong> Preview 1 (<code>wasm32-wasip1</code>)</>,
        <><strong>Language:</strong> Rust</>,
        <><strong>Cryptography:</strong> ECIES (secp256k1), HKDF-SHA256, Merkle trees</>,
        <><strong>Secrets:</strong> Required (<code>DAO_MASTER_SECRET</code>)</>,
        <><strong>Frontend:</strong> React with NEAR Wallet integration</>,
        <><strong>Build:</strong> <code>cargo build --target wasm32-wasip1 --release</code></>,
        <><strong>Size:</strong> ~1.2MB compiled WASM</>
      ]} />

      <LearnMoreSection>
        <ul className="text-sm text-gray-700 space-y-1">
          <li>
            📖 <Link href="/docs/wasi#wasi-preview-1" className="text-[var(--primary-orange)] hover:underline">
              WASI Preview 1 Documentation
            </Link>
          </li>
          <li>
            🔐 <Link href="/docs/secrets" className="text-[var(--primary-orange)] hover:underline">
              Secrets Management Guide
            </Link>
          </li>
          <li>
            💻 <a href="https://github.com/zavodil/private-dao-ark" target="_blank" rel="noopener noreferrer" className="text-[var(--primary-orange)] hover:underline">
              View Source Code
            </a>
          </li>
        </ul>
      </LearnMoreSection>
    </ExampleCard>
  );
}
