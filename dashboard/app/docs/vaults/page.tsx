'use client';

import Link from 'next/link';
import { Prism as SyntaxHighlighter } from 'react-syntax-highlighter';
import { vscDarkPlus } from 'react-syntax-highlighter/dist/esm/styles/prism';
import { AnchorHeading, useHashNavigation } from '../sections/utils';
import { VaultArchitectureDiagram } from '@/components/VaultArchitectureDiagram';

export default function VaultsDocsPage() {
  useHashNavigation();

  return (
    <div className="max-w-5xl">
      <h1 className="text-4xl font-bold mb-3">MPC Vaults</h1>

      {/* ── 0. Quick CKD primer ──────────────────────────────────────── */}
      <div className="bg-blue-50 border-l-4 border-blue-500 p-4 mb-6 text-sm">
        <p className="font-semibold text-blue-900 mb-1">
          First, how OutLayer key custody works
        </p>
        <p className="text-gray-800 mb-2">
          Every wallet key, every secret-encryption key, every payment-check
          ephemeral on OutLayer is <strong>not stored anywhere</strong>. It is
          derived on demand inside the keystore worker&rsquo;s TEE from a
          single 32-byte master. The master itself is also not stored on
          disk &mdash; on every keystore start it is requested from{' '}
          <strong>NEAR&rsquo;s MPC network</strong> via a primitive called{' '}
          <strong>CKD</strong> (Conditional Key Derivation): threshold-key
          holders in the MPC network jointly hand back the master for a
          given on-chain identifier, deterministically, without any single
          MPC node ever assembling the secret.
        </p>
        <p className="text-gray-700 mb-0">
          See <Link href="/docs/agent-custody" className="text-blue-700 underline">Agent Custody</Link>{' '}
          and <Link href="/docs/secrets" className="text-blue-700 underline">Secrets</Link> for
          how derived keys are used per-feature; the rest of this page
          focuses on <strong>whose</strong> master it is.
        </p>
      </div>

      <p className="text-gray-700 mb-3">
        By default the master is <strong>bound to OutLayer&rsquo;s
        keystore-DAO contract</strong>. The keystore TEE asks NEAR MPC
        for that master, derives every customer&rsquo;s keys from it
        inside the enclave, and never persists it. Nobody manages or
        holds the master directly: it is reproduced from MPC on every
        keystore restart, lives only in TEE memory, and the DAO contract
        enforces hardware-attestation verification before MPC releases
        the bytes &mdash; so even OutLayer operators cannot request it
        outside an attested keystore.
      </p>
      <p className="text-gray-700 mb-3">
        An <strong>MPC vault</strong> swaps that DAO-bound root for a
        master <strong>bound to a contract you deploy</strong> on a
        sub-account of your NEAR account. Same CKD primitive, same
        keystore, same TEE flow &mdash; but the binding moves to your
        vault, so only code that controls the vault account can ever
        ask MPC for that master.
      </p>
      <p className="text-gray-700 mb-3">
        The vault contract&rsquo;s only access key is a TEE function-call
        key scoped to a single proxy method that calls NEAR MPC&rsquo;s{' '}
        <code>request_app_private_key</code>. As long as that key is in
        place, OutLayer&rsquo;s TEE is the only party that can ask MPC
        for your master.
      </p>

      {/* ── 0b. Interactive architecture diagram ─────────────────────── */}
      <VaultArchitectureDiagram />

      {/* ── 0c. Side-by-side comparison table ────────────────────────── */}
      <div className="border border-gray-200 rounded-lg overflow-hidden mb-6">
        <table className="w-full text-sm">
          <thead className="bg-gray-100">
            <tr>
              <th className="text-left px-3 py-2 font-semibold w-1/4">Aspect</th>
              <th className="text-left px-3 py-2 font-semibold">Default (OutLayer master)</th>
              <th className="text-left px-3 py-2 font-semibold bg-blue-50">MPC vault (yours)</th>
            </tr>
          </thead>
          <tbody className="text-gray-700">
            <tr className="border-t border-gray-200">
              <td className="px-3 py-2 font-medium align-top">Master bound to</td>
              <td className="px-3 py-2 align-top">OutLayer&rsquo;s keystore-DAO contract</td>
              <td className="px-3 py-2 align-top bg-blue-50/50">
                Your vault contract
              </td>
            </tr>
            <tr className="border-t border-gray-200">
              <td className="px-3 py-2 font-medium align-top">Runtime that holds master</td>
              <td className="px-3 py-2 align-top">OutLayer keystore TEE only</td>
              <td className="px-3 py-2 align-top bg-blue-50/50">
                <strong>Swappable</strong>: OutLayer TEE today, your own attested runtime after recovery
              </td>
            </tr>
            <tr className="border-t border-gray-200">
              <td className="px-3 py-2 font-medium align-top">Takeover path</td>
              <td className="px-3 py-2 align-top">None &mdash; you depend on OutLayer continuing to serve</td>
              <td className="px-3 py-2 align-top bg-blue-50/50">
                Cessation recovery (DAO declares <code>is_ceased</code>) <em>or</em> unilateral exit (parent-only, configurable window)
              </td>
            </tr>
            <tr className="border-t border-gray-200">
              <td className="px-3 py-2 font-medium align-top">After takeover</td>
              <td className="px-3 py-2 align-top">&mdash;</td>
              <td className="px-3 py-2 align-top bg-blue-50/50">
                Your runtime calls NEAR MPC from the vault account &rArr; same 32 bytes &rArr; same derived keys
              </td>
            </tr>
            <tr className="border-t border-gray-200">
              <td className="px-3 py-2 font-medium align-top">One-time cost</td>
              <td className="px-3 py-2 align-top">$0</td>
              <td className="px-3 py-2 align-top bg-blue-50/50">~0.1 NEAR (storage stake + MPC gas reserve)</td>
            </tr>
          </tbody>
        </table>
      </div>

      <div className="border-l-4 border-blue-500 bg-blue-50 p-4 mb-6">
        <h4 className="font-semibold mb-2 text-blue-900">Two modes &mdash; one-way switch</h4>
        <p className="text-sm text-gray-800 mb-2">
          Once your vault is deployed you choose how to operate it. You
          can change modes later, but it&rsquo;s a one-way move:
        </p>
        <div className="grid grid-cols-1 md:grid-cols-2 gap-3 text-sm">
          <div className="bg-white border border-gray-200 rounded p-3">
            <div className="font-semibold mb-1">A. OutLayer-managed (default)</div>
            <p className="text-gray-700 mb-1">
              OutLayer&rsquo;s TEE holds the FC key, derives the master via
              MPC CKD, and runs your agents on its infrastructure. You keep
              full sovereignty (parent account + recovery path), but the
              keystore TEE is what does the actual key derivation and
              decryption.
            </p>
          </div>
          <div className="bg-white border border-gray-200 rounded p-3">
            <div className="font-semibold mb-1">B. Self-managed (you take over)</div>
            <p className="text-gray-700 mb-1">
              You initiate recovery (cessation or unilateral exit), the
              vault becomes <code>unlocked</code>, OutLayer&rsquo;s
              keystore refuses to serve it. You add a FullAccess key and
              run the same MPC derivation yourself &mdash; from your own
              TEE, your own attested runtime, or by hand. Every derived
              key is regenerable from the same master.
            </p>
          </div>
        </div>
        <p className="text-sm text-gray-700 mt-3">
          <strong>One-way:</strong> once a vault is unlocked,
          OutLayer&rsquo;s keystore won&rsquo;t serve it again. You
          can&rsquo;t come back to mode A &mdash; the FC-key + MPC-CKD
          binding only holds while the contract&rsquo;s
          <code> unlocked = false</code>. To use OutLayer with a fresh
          managed vault you would deploy a new vault on a new sub-account.
        </p>
      </div>

      <div className="border border-gray-200 rounded-lg p-4 mb-6 bg-gray-50">
        <h4 className="font-semibold mb-2">What is &ldquo;CKD&rdquo;?</h4>
        <p className="text-sm text-gray-700 mb-2">
          <strong>Conditional Key Derivation</strong> is a primitive of
          NEAR&rsquo;s MPC service. Threshold-key holders in the MPC
          network jointly derive a private key for a given on-chain
          identifier, deterministically, without ever assembling the
          secret on any single node. The resulting key is unique to the
          contract that requested it; another contract asking for the
          same path gets a completely different key.
        </p>
        <p className="text-sm text-gray-700 mb-0">
          For an MPC vault, the requesting contract is the vault
          itself. The TEE keystore calls{' '}
          <code>request_app_private_key</code> through the vault&rsquo;s
          proxy method; NEAR MPC returns 32 bytes that become your
          per-customer master inside the enclave. Same inputs &rArr;
          same master, deterministically &mdash; so you can reproduce
          it later by querying the same MPC network from the vault
          account, even if OutLayer is gone.
        </p>
      </div>

      {/* ── 1. When to use ─────────────────────────────────────────── */}
      <section className="mb-10">
        <AnchorHeading id="overview">When to use a vault</AnchorHeading>
        <div className="border border-gray-200 rounded-lg p-4 mb-3">
          <h4 className="font-semibold mb-2">Trade-offs</h4>
          <ul className="list-disc list-inside text-sm text-gray-700 space-y-1">
            <li>
              <strong>Default master:</strong> shared keystore-DAO root, zero
              customer setup, no on-chain footprint. Best for prototyping
              and low-value automation. You rely on OutLayer&rsquo;s
              keystore as the only path to your derived keys.
            </li>
            <li>
              <strong>Per-customer MPC vault:</strong> on-chain CKD-issuer
              contract bound to your account. ~0.1 NEAR one-time
              (storage stake + gas reserve for outbound MPC calls) thanks
              to <code>UseGlobalContract</code>; one atomic tx at setup.
              You can later take the vault over yourself via NEAR MPC and
              keep deriving every key independently of OutLayer.
            </li>
          </ul>
        </div>
        <p className="text-sm text-gray-600">
          Use a vault if your application&rsquo;s value-at-risk justifies
          the extra setup, or if your governance / audit requirements
          mandate independent control over derived keys.
        </p>
      </section>

      {/* ── 2. Creating a vault ────────────────────────────────────── */}
      <section className="mb-10">
        <AnchorHeading id="create">Creating a vault</AnchorHeading>
        <p className="mb-3">
          A single atomic NEAR transaction creates the sub-account,
          deploys the vault WASM, calls <code>new()</code>, and adds
          the TEE function-call key. If any of the five actions fails,
          the entire sub-account state rolls back — there is no
          half-deployed state.
        </p>

        <h4 className="font-semibold mt-4 mb-2">Dashboard</h4>
        <ol className="list-decimal list-inside text-sm space-y-1 mb-4">
          <li>Open <Link href="/vault" className="text-blue-600 hover:underline">Vaults</Link> from My Workspace.</li>
          <li>Pick a sub-account name (default: <code>vault</code>) and an exit window (24h / 7d / 30d).</li>
          <li>Click <strong>Create vault</strong>; sign the atomic-deploy tx in your wallet.</li>
          <li>Save the API key shown ONCE — it is not recoverable.</li>
        </ol>

        <h4 className="font-semibold mt-4 mb-2">CLI</h4>
        <SyntaxHighlighter language="bash" style={vscDarkPlus}>
{`# defaults: --name vault --exit-window 24h
outlayer vault init

# custom
outlayer vault init --name treasury --exit-window 7d`}
        </SyntaxHighlighter>

        <h4 className="font-semibold mt-4 mb-2">What happens technically</h4>
        <ol className="list-decimal list-inside text-sm space-y-1 mb-4">
          <li>CLI/dashboard probes <code>is_vault_code_approved(hash)</code> on keystore-DAO so you don&rsquo;t pay gas to deploy a deprecated WASM.</li>
          <li>Coordinator returns the deterministic TEE function-call public key for your vault id (HMAC-derived inside the TEE).</li>
          <li>One transaction, five actions: <code>CreateAccount</code>{' + '}<code>Transfer 0.1 NEAR</code>{' + '}<code>UseGlobalContract(code_hash)</code>{' + '}<code>new(parent, keystore_dao, mpc_contract, exit_window)</code>{' + '}<code>AddKey(tee_pubkey, FCAK on vault.request_master)</code>.</li>
          <li>Coordinator triggers keystore re-verification — keystore-worker independently re-runs five RPC checks and signs <code>mark_vault_verified</code> on chain.</li>
          <li>Coordinator binds an API key to the verified vault and returns it.</li>
        </ol>
        <p className="text-sm text-gray-600">
          If init fails between step 4 and step 5 (e.g. transient
          network failure), use <code>outlayer vault resume &lt;account&gt;</code>
          {' '}or the <strong>Resume</strong> button on the dashboard. Steps 4
          and 5 are idempotent on the keystore side.
        </p>
      </section>

      {/* ── 3. Using vault for secrets ─────────────────────────────── */}
      <section className="mb-10">
        <AnchorHeading id="secrets">Using a vault for secrets</AnchorHeading>
        <p className="mb-3">
          Secrets encrypted with your vault&rsquo;s pubkey are decryptable
          only by the keystore-worker holding your per-customer master.
          The encryption pubkey for your vault is fetched by the
          dashboard and CLI by including the
          <code> X-Customer-Vault: &lt;vault_id&gt; </code>header on the
          pubkey request; the keystore derives a per-vault sub-key for
          encryption and returns the matching public half.
        </p>
        <p className="mb-3">
          Existing secrets encrypted under the default master continue
          to work without migration — the toggle is per-secret, not
          per-account.
        </p>
      </section>

      {/* ── 4. Using vault for custody wallets ─────────────────────── */}
      <section className="mb-10">
        <AnchorHeading id="custody">Using a vault for custody wallets</AnchorHeading>
        <p className="mb-3">
          The API key returned by <code>vault init</code> is bound to
          your vault: every wallet API call carries
          <code> X-Customer-Vault </code>under the hood and the
          coordinator forwards it to the keystore. Wallet addresses
          for that API key are derived from your per-customer master,
          so a future cessation or unilateral recovery puts the
          private keys back in your hands without a migration step.
        </p>
        <p className="mb-3">
          You can run multiple wallets — some on the default master,
          some on different vaults. The agent code does not change;
          the API key fully determines which master is used.
        </p>
      </section>

      {/* ── 5. Recovery procedures ─────────────────────────────────── */}
      <section className="mb-10">
        <AnchorHeading id="recovery">Recovery procedures</AnchorHeading>

        <h4 className="font-semibold mt-3 mb-2">Cessation recovery (catastrophic)</h4>
        <p className="text-sm text-gray-700 mb-3">
          If the OutLayer DAO declares cessation
          (<code>is_ceased() == true </code>on the keystore-DAO),
          anyone can call <code>initiate_recovery</code> on the vault.
          The vault contract re-checks the DAO flag inside its callback,
          starts a fixed seven-day timer, and on
          <code> finalize_recovery </code>flips the vault to
          {' '}<code>unlocked = true</code>. After unlock, the parent
          account can add its own key and withdraw funds or migrate
          custody.
        </p>
        <SyntaxHighlighter language="bash" style={vscDarkPlus}>
{`outlayer vault initiate-recovery vault.alice.near
# (wait 7 days)
outlayer vault finalize-recovery  vault.alice.near
outlayer vault unlocked-add-key   vault.alice.near ed25519:...`}
        </SyntaxHighlighter>

        <h4 className="font-semibold mt-4 mb-2">Unilateral recovery (voluntary)</h4>
        <p className="text-sm text-gray-700 mb-3">
          The parent account can exit at any time without DAO
          involvement. The delay is the
          {' '}<code>unilateral_exit_window_secs</code> chosen at
          deploy (default 24h, configurable 24h-30d). Changing the
          window via <code>set_exit_window</code> only affects future
          recoveries — an in-flight recovery&rsquo;s finalize timestamps
          are frozen at initiate time.
        </p>
        <SyntaxHighlighter language="bash" style={vscDarkPlus}>
{`outlayer vault set-exit-window               vault.alice.near 24h
outlayer vault initiate-unilateral-recovery  vault.alice.near
# (wait the configured window)
outlayer vault finalize-recovery             vault.alice.near
outlayer vault unlocked-add-key              vault.alice.near ed25519:...`}
        </SyntaxHighlighter>

        <div className="bg-yellow-50 border border-yellow-300 rounded p-3 mt-4 text-sm text-gray-800">
          <strong>Trust model note:</strong> end-users of your application
          interact with you, not OutLayer. You are the trusted party for
          your end-users. Unilateral exit is a <em>customer&rsquo;s</em> escape
          hatch from OutLayer, not an end-user protection mechanism.
        </div>
      </section>

      {/* ── 5b. Threat model for end-users ─────────────────────────── */}
      <section className="mb-10">
        <AnchorHeading id="threat-model">What end-users should know</AnchorHeading>
        <p className="mb-3">
          Two questions any end-user transacting with a customer&rsquo;s
          vault-bound app should ask before depositing funds:
        </p>

        <h4 className="font-semibold mt-3 mb-1">1. Could the customer have rigged the vault during deploy?</h4>
        <p className="text-sm text-gray-700 mb-3">
          <strong>No.</strong> The vault becomes immutable immediately
          after the atomic deploy. Its only access key is the TEE
          function-call key restricted to <code>mpc_contract.request_app_private_key</code>;
          that key cannot add keys, deploy code, or call any vault
          method. The approved vault contract has no method that
          allows self-upgrading or installing additional keys outside
          the recovery flow. Any tampering during the atomic deploy
          either (a) rolls back atomically, or (b) produces a final
          state that&nbsp;
          <code>vault-checker</code> observably rejects (extra access
          keys, unapproved code hash, malformed state). End-users can
          confirm this themselves with <code>outlayer vault verify
          &lt;vault_id&gt;</code>.
        </p>

        <h4 className="font-semibold mt-3 mb-1">2. Could the customer drain the vault later?</h4>
        <p className="text-sm text-gray-700 mb-3">
          <strong>Yes &mdash; after the configured unilateral exit
          window.</strong> This is the explicit sovereignty feature
          the vault provides. After waiting <code>unilateral_exit_window_secs</code>
          (24h&ndash;30d, chosen at deploy and visible on chain), the
          customer&rsquo;s parent account can finalize a unilateral
          recovery, unlock the vault, install a full-access key, and
          re-derive the per-vault master themselves.
        </p>
        <p className="text-sm text-gray-700 mb-3">
          This is <em>not</em> a vulnerability in OutLayer&rsquo;s TEE
          infrastructure &mdash; it&rsquo;s the customer exercising the
          escape hatch they built the vault to have. From the
          protocol&rsquo;s perspective the customer was always able to
          recover their own vault.
        </p>
        <div className="bg-amber-50 border border-amber-300 rounded p-3 text-sm text-gray-800">
          <strong>Practical guidance:</strong>
          <ul className="list-disc list-inside mt-1 space-y-1">
            <li>Read <code>unilateral_exit_window_secs</code> with{' '}
              <code>outlayer vault verify</code> BEFORE depositing.
              Treat it as &ldquo;this is how much warning I get before
              the customer can drain.&rdquo;</li>
            <li>The vault&rsquo;s <code>recovery</code> state is observable
              in real time. A tool that watches{' '}
              <code>vault.get_recovery_state()</code> alerts you the
              moment a recovery starts, giving you the configured
              window to react.</li>
            <li>For high-value deployments, prefer customers whose{' '}
              <code>parent</code> account is a multisig contract &mdash;
              that shifts the trust assumption from &ldquo;customer is
              honest&rdquo; to &ldquo;customer&rsquo;s multisig signers are
              honest.&rdquo;</li>
          </ul>
        </div>
      </section>

      {/* ── 6. End-user verification ───────────────────────────────── */}
      <section className="mb-10">
        <AnchorHeading id="verify">End-user verification</AnchorHeading>
        <p className="mb-3">
          Anyone can verify a vault&rsquo;s state without deploying or
          signing anything. The CLI runs five defense-in-depth
          checks against on-chain state:
        </p>
        <SyntaxHighlighter language="bash" style={vscDarkPlus}>
{`outlayer vault verify vault.alice.near`}
        </SyntaxHighlighter>
        <ol className="list-decimal list-inside text-sm space-y-1 mt-3">
          <li><code>keystore-dao.is_vault_verified</code> — primary trust signal.</li>
          <li><code>is_vault_code_approved</code> — vault WASM hash is in the DAO whitelist.</li>
          <li><code>vault.get_state()</code> matches the network&rsquo;s expected keystore_dao + mpc_contract ids and is not unlocked.</li>
          <li>The on-chain access keys are bounded and TEE-only — no full-access key, no out-of-scope FCAK.</li>
          <li><code>registered_tee_keys</code> is a subset of the account&rsquo;s access keys.</li>
        </ol>
        <p className="text-sm text-gray-600 mt-3">
          Red flags: vault is <strong>banned</strong>{' '}
          (<code>is_vault_verified</code> returns false even after
          cleanup), vault is <strong>unlocked</strong> (parent has post-recovery
          key authority — funds are NOT under TEE control anymore), or a
          <strong>recovery is in progress</strong>.
        </p>
      </section>

      {/* ── 7. Operations ──────────────────────────────────────────── */}
      <section className="mb-10">
        <AnchorHeading id="ops">Operational considerations</AnchorHeading>
        <ul className="list-disc list-inside text-sm space-y-2">
          <li>
            <strong>One-time cost:</strong> ~0.1 NEAR transferred to the
            new vault account. With <code>UseGlobalContract</code> the
            WASM lives in the global registry, so storage stake is just
            the contract state (~0.004 NEAR). The remainder is the gas
            reserve for outbound MPC calls
            (<code>vault.request_master → mpc.request_app_private_key</code>,
            ~0.001 NEAR/call; the master is cached in the keystore TEE
            after the first call).
          </li>
          <li>
            <strong>Top-ups when the gas reserve runs low:</strong> gas
            for <code>vault.request_master</code> is paid from the
            vault account itself (it owns the TEE function-call key
            that signs the call). If the balance falls below the
            reserve threshold, the keystore eventually fails to
            refresh your per-customer master in enclave memory and
            derived-key requests stall until top-up. Top-up is a plain
            on-chain NEAR transfer to the vault account from any
            wallet — no contract method, no signature on a special
            endpoint. The dashboard surfaces a banner with a suggested
            transfer amount when the balance is below the threshold.
            Operationally we recommend customers monitor the vault
            balance the same way they monitor a hot wallet.
          </li>
          <li>
            <strong>Race-attack protection:</strong> a malicious customer
            who tries to also-derive the per-vault master with a backup
            key sneaked into the deploy is detected by the OutLayer
            monitor (Phase 8) and banned automatically. The DAO ban
            applies retroactively — view-call <code>is_vault_verified </code>
            returns false even though the vault was once verified.
          </li>
          <li>
            <strong>TEE worker rotation:</strong> the vault contract caps
            registered TEE keys at 32. Each keystore-worker upgrade
            registers a new key via <code>propose_tee_key</code>. There
            is no revoke method in v1 — for any rotation that needs to
            retire the old key, deploy a fresh vault with the new
            approved code hash.
          </li>
        </ul>
      </section>
    </div>
  );
}
