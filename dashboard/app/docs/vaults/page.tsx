'use client';

import Link from 'next/link';
import { Prism as SyntaxHighlighter } from 'react-syntax-highlighter';
import { vscDarkPlus } from 'react-syntax-highlighter/dist/esm/styles/prism';
import { AnchorHeading, useHashNavigation } from '../sections/utils';

export default function VaultsDocsPage() {
  useHashNavigation();

  return (
    <div className="max-w-5xl">
      <h1 className="text-4xl font-bold mb-3">Sovereign Vaults</h1>
      <p className="text-gray-600 mb-8">
        Per-customer master keys with on-chain recoverability. Wallet keys and
        secrets bound to a vault stay derivable by you even if OutLayer
        ceases — the only access key on the vault is a TEE function-call key
        scoped to the MPC contract, plus a contract-level recovery path that
        gives you back control under two trigger conditions: catastrophic
        cessation of OutLayer, or your own voluntary exit.
      </p>

      {/* ── 1. What & when ─────────────────────────────────────────── */}
      <section className="mb-10">
        <AnchorHeading id="overview">What are vaults &amp; when to use them</AnchorHeading>
        <p className="mb-3">
          By default, all wallet keys and encrypted secrets on OutLayer are
          derived from a shared <strong>OutLayer master</strong> held inside
          the keystore worker&rsquo;s TEE. Convenient: zero customer setup,
          shared infrastructure cost. The trust model is &ldquo;OutLayer is
          honest&rdquo; — if OutLayer disappears, so do your derived keys.
        </p>
        <p className="mb-3">
          A <strong>vault</strong> replaces that shared master with one
          that is derived per-customer via MPC and recoverable by you.
          You deploy a tiny smart contract on a sub-account of your NEAR
          account; the keystore worker derives a master keyed on
          <code> (TEE secret, your vault id) </code>and burns the
          mapping into the vault&rsquo;s on-chain record. From then on,
          every wallet key and encrypted secret you bind to that vault
          is derivable from <em>your</em> sub-account, not OutLayer&rsquo;s
          shared infrastructure.
        </p>
        <div className="border border-gray-200 rounded-lg p-4 mb-3">
          <h4 className="font-semibold mb-2">Trade-offs</h4>
          <ul className="list-disc list-inside text-sm text-gray-700 space-y-1">
            <li><strong>Default master:</strong> simpler, no setup, no on-chain footprint. Wallet keys / secrets are gone if OutLayer ceases.</li>
            <li><strong>Per-customer vault:</strong> recoverable through DAO cessation or unilateral exit. Costs ~2.5 NEAR storage stake one-time + a small recurring gas reserve, and adds one atomic transaction at setup.</li>
          </ul>
        </div>
        <p className="text-sm text-gray-600">
          Use a vault if your application&rsquo;s value-at-risk justifies
          the extra setup, or if your governance / audit requirements
          mandate sovereign control over derived keys. Stay on the
          default master for prototyping or low-value automation.
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
          <li>One transaction, five actions: <code>CreateAccount</code>{' + '}<code>Transfer 2.5 NEAR</code>{' + '}<code>DeployContract</code>{' + '}<code>new(parent, keystore_dao, mpc_contract, exit_window)</code>{' + '}<code>AddKey(tee_pubkey, FCAK on mpc_contract.request_app_private_key)</code>.</li>
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
            <strong>One-time cost:</strong> ~2.5 NEAR transferred to the
            new vault account (1.5 NEAR storage stake + ~1 NEAR gas
            reserve for outbound MPC calls). Top up the vault with
            additional NEAR if its gas reserve gets depleted by
            high-frequency derivations.
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
