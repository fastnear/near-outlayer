import { ExampleCard, UseCasesSection, KeyFeaturesSection, TechnicalDetailsSection, CodeExampleSection, HowItWorksSection, LearnMoreSection, SecurityNotesSection } from './';

export function EthProofExample() {
  const badges = (
    <>
      <span className="ml-3 text-sm bg-purple-100 text-purple-800 px-3 py-1 rounded">WASI P2</span>
      <span className="ml-2 text-sm bg-red-100 text-red-800 px-3 py-1 rounded">Advanced</span>
    </>
  );

  return (
    <ExampleCard
      id="eth-proof"
      title="eth-proof-example"
      badges={badges}
      githubUrl="https://github.com/out-layer/eth-proof-example"
      playgroundId="ethereum-proof"
    >
      <p className="text-gray-700 mb-4">
        Bring an <strong>Ethereum value onto NEAR with a cryptographic receipt</strong>. The module reads a
        Chainlink price straight out of Ethereum contract storage together with its Merkle-Patricia proof, and
        re-derives the hashes inside the TEE until they meet the block&apos;s state root. A value that arrives
        this way cannot have been altered on the way &mdash; not by the node that served it, not by anyone.
      </p>

      <UseCasesSection items={[
        'Cross-chain price feeds where a wrong number costs real money (lending, liquidations, perps)',
        'Bridging any Ethereum contract state to NEAR — balances, ownership, governance results',
        'Auditable oracles: every answer ships the block hash it came from',
        'Reading an L1 value without running your own Ethereum node',
      ]} />

      <KeyFeaturesSection items={[
        <>Reads storage via <code>eth_getProof</code> (EIP-1186) and verifies the trie walk in-enclave</>,
        <>Two nested proofs per value: the <strong>account</strong> in the state trie, then the <strong>slot</strong> in that account&apos;s storage trie</>,
        <>The state root is agreed by several <strong>independent RPC providers</strong> before anything is checked against it</>,
        'Disagreement is a hard failure with who-said-what, never a silent fallback',
        <>Follows the Chainlink proxy to its current aggregator <em>with a proof</em>, so a feed upgrade cannot freeze the price</>,
        'No secrets and no API keys — every endpoint is public',
      ]} />

      <HowItWorksSection items={[
        <>Pin a block a few behind the head, so honest nodes have converged on it</>,
        <>Ask several RPCs for that exact block and keep the header they <strong>all</strong> return</>,
        <>Prove which aggregator the feed points at, then the latest round, then that round&apos;s answer</>,
        <>Walk each proof: every node must hash to the value its parent published, down to the leaf</>,
        <>Return the price together with the block hash, so anyone can re-check it later</>,
      ]} />

      <CodeExampleSection
        title="Input Format:"
        code={`{
  "proxy": "0x5f4eC3Df9cbd43714FE2740f5E3616155c5b8419",
  "pair": "ETH/USD"
}`}
        description="The feed address from Chainlink's own docs. Everything else has sane defaults."
      />

      <CodeExampleSection
        title="Output Format:"
        code={`{
  "pair": "ETH/USD",
  "price": 1833.50773289,
  "verified": {
    "summary": "ETH/USD = 1833.50773289 is the value Chainlink had stored on
                Ethereum at block 25661863. Read out of the contract's storage
                with a Merkle proof and re-checked here, so no node in the path
                could have altered it.",
    "storage_proofs": 3,
    "trie_nodes_checked": 40,
    "rpcs_agreeing": "3 of 3",
    "explorer": "https://etherscan.io/block/25661863",
    "not_proven": "That this is the right price. Chainlink published it; the
                   proof shows it was delivered unaltered, not that it is accurate."
  },
  "updated_at": 1785609107,
  "age_secs": 495,
  "evidence": {
    "block_hash": "0x91c86cb04cc2a2c6017b57553e10d72535b5c7a76f5f79887f4f771f64347da6",
    "state_root": "0x0262cc93a28916ad7b440af278cfdbac77ed8dcedd3902a95e77c33775ae1077",
    "storage_hash": "0x932006b08c2f45306c06b7e1c230869dc800e3ee7c9f090635a0d41e11f88d91",
    "aggregator": "0x7d4e742018fb52e48b08be73d041c18b21de6fb5",
    "phase_id": 7,
    "round_id": 32113,
    "transmission_slot": "0x7336674795583596f42aea4a5e2272fac73d81c3ea39393d121041b0f03ae74a",
    "agreed_by": ["ethereum-rpc.publicnode.com", "eth.drpc.org", "rpc.mevblocker.io"],
    "rejected": []
  }
}`}
        description="What to look at: trie_nodes_checked is how many hashes had to line up; rpcs_agreeing is how much independent backing the state root has; block_hash is the receipt — paste it into any explorer."
      />

      <TechnicalDetailsSection items={[
        <><strong>WASI Version:</strong> Preview 2 (<code>wasm32-wasip2</code>), outbound HTTP</>,
        <><strong>Dependencies:</strong> <code>tiny-keccak</code> and <code>rlp</code> only — the trie walk is ~150 lines, no EVM library</>,
        <><strong>Calls:</strong> N+3 sequential round trips to Ethereum; latency dominates, the hashing is negligible</>,
        <><strong>Storage layout:</strong> derived empirically against the live aggregator and pinned by unit tests, since Chainlink does not document it</>,
      ]} />

      <SecurityNotesSection items={[
        <><strong>What is proven:</strong> the value really is in Ethereum&apos;s state under the agreed block. Tampering anywhere in the delivery path breaks the hash chain.</>,
        <><strong>What is not:</strong> that the price is <em>correct</em>. A proof will faithfully prove a bad Chainlink number — establishing that the oracle committee vouched for it means verifying their report signatures.</>,
        <><strong>The remaining trust:</strong> the state root rests on independent providers agreeing. Pick endpoints run by different operators; the strong version is a beacon-chain light client.</>,
        <><strong>Freshness:</strong> a proven price can still be stale. The module rejects anything older than <code>max_age_secs</code> (default 2h) and always reports <code>age_secs</code>.</>,
      ]} />

      <LearnMoreSection>
        <ul className="text-sm text-gray-700 space-y-1 list-disc list-inside">
          <li>
            <a href="https://eips.ethereum.org/EIPS/eip-1186" target="_blank" rel="noopener noreferrer" className="text-[var(--primary-orange)] hover:underline">
              EIP-1186
            </a>{' '}&mdash; the <code>eth_getProof</code> specification
          </li>
          <li>
            <a href="https://docs.chain.link/data-feeds/price-feeds/addresses" target="_blank" rel="noopener noreferrer" className="text-[var(--primary-orange)] hover:underline">
              Chainlink feed addresses
            </a>{' '}&mdash; the proxy address to pass as <code>proxy</code>
          </li>
        </ul>
      </LearnMoreSection>
    </ExampleCard>
  );
}
