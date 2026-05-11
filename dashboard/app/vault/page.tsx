'use client';

import { useState, useCallback, useEffect, useRef } from 'react';
import { actionCreators } from '@near-js/transactions';
import { PublicKey } from '@near-js/crypto';

import { useNearWallet } from '@/contexts/NearWalletContext';
import WalletConnectionModal from '@/components/WalletConnectionModal';
import {
  buildVaultDeployActions,
  customerRegister,
  deriveVaultTeeKey,
  formatSeconds,
  getVaultCodeHash,
  getVaultNetworkConfig,
  nsToDate,
  parseExitWindow,
  signVaultVerification,
  VAULT_CALL_GAS,
  VAULT_INITIAL_YOCTO,
  VAULT_LOW_BALANCE_YOCTO,
  VAULT_PARENT_BUDGET_YOCTO,
  VAULT_TOPUP_SUGGESTED_YOCTO,
  verifyVault,
  viewAccountInfo,
  type VerifyReport,
} from '@/lib/vault';

const EXIT_WINDOW_OPTIONS = [
  { label: '24 hours (default)', value: '24h' },
  { label: '7 days', value: '7d' },
  { label: '30 days', value: '30d' },
] as const;

export default function VaultPage() {
  const {
    accountId,
    isConnected,
    signAndSendTransaction,
    network,
    rpcUrl,
    viewMethod,
    shouldReopenModal,
    clearReopenModal,
  } = useNearWallet();

  // ── UI state ──────────────────────────────────────────────────────────
  const [showWalletModal, setShowWalletModal] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [success, setSuccess] = useState<string | null>(null);
  const [busy, setBusy] = useState<string | null>(null);
  // Synchronous double-submit guard. React's `setBusy` is async (next
  // render), so a fast double-click on Create can fire two atomic-deploy
  // tx requests before the disabled state is applied. The first
  // succeeds, the second hits CreateAccount on the now-existing vault
  // and panics — wallet UI shows two confusing prompts. The ref blocks
  // re-entry within the same event tick.
  const inFlight = useRef(false);

  // Create-vault form
  const [name, setName] = useState('vault');
  const [exitWindow, setExitWindow] = useState<string>('24h');

  // API key returned by /customer/register — show once, never persist.
  const [issuedApiKey, setIssuedApiKey] = useState<{
    vault: string;
    apiKey: string;
    nearAccountId: string;
  } | null>(null);

  // Find / inspect vault
  const [findInput, setFindInput] = useState('');
  const [activeVaultId, setActiveVaultId] = useState<string | null>(null);
  const [report, setReport] = useState<VerifyReport | null>(null);

  // ── Modal handling matches existing pages ─────────────────────────────
  useEffect(() => {
    if (shouldReopenModal) {
      setShowWalletModal(true);
      clearReopenModal();
    }
  }, [shouldReopenModal, clearReopenModal]);

  const guard = useCallback(
    (msg: string) => {
      if (!isConnected || !accountId) {
        setShowWalletModal(true);
        setError(msg);
        return false;
      }
      return true;
    },
    [isConnected, accountId],
  );

  const refreshReport = useCallback(
    async (vaultId: string) => {
      try {
        const r = await verifyVault(viewMethod, rpcUrl, network, vaultId);
        setReport(r);
        setActiveVaultId(vaultId);
      } catch (e) {
        setReport(null);
        setError(`Failed to load vault state: ${(e as Error).message}`);
      }
    },
    [viewMethod, rpcUrl, network],
  );

  // ── Create vault ──────────────────────────────────────────────────────
  const handleCreate = async () => {
    if (inFlight.current) return; // synchronous double-submit guard
    if (!guard('Connect a NEAR wallet to deploy a vault.')) return;
    if (!accountId) return;
    setError(null);
    setSuccess(null);
    setIssuedApiKey(null);

    // NEAR sub-account name rule: lowercase a-z, 0-9, `_`, `-`,
    // 2-64 chars per label, no leading/trailing/consecutive separators.
    // Matching the parser at near-account-id ensures the wallet popup
    // doesn't surface a cryptic "InvalidAccountId" after the user
    // already approved the deploy.
    const NAME_RE = /^[a-z0-9](?:[a-z0-9_-]{0,62}[a-z0-9])?$/;
    if (!NAME_RE.test(name)) {
      setError(
        "Name must be 1-64 lowercase letters, digits, '_' or '-' " +
          "(no dots, uppercase, leading/trailing separators). E.g. 'vault'.",
      );
      return;
    }

    let exitSecs: number;
    try {
      exitSecs = parseExitWindow(exitWindow);
    } catch (e) {
      setError((e as Error).message);
      return;
    }

    const vaultAccountId = `${name}.${accountId}`;
    inFlight.current = true;
    setBusy('Pre-flight checks…');

    try {
      // 0a. Vault must NOT already exist.
      const existing = await viewAccountInfo(rpcUrl, vaultAccountId);
      if (existing.exists) {
        throw new Error(
          `${vaultAccountId} already exists. If a previous deploy crashed before \
registration, use the "Resume" button below. Otherwise pick a different name.`,
        );
      }

      // 0b. Parent must hold enough NEAR.
      const parentInfo = await viewAccountInfo(rpcUrl, accountId);
      if (!parentInfo.exists) {
        throw new Error(`Parent account ${accountId} does not exist on ${network}.`);
      }
      const balance = BigInt(parentInfo.amountYocto);
      if (balance < VAULT_PARENT_BUDGET_YOCTO) {
        throw new Error(
          `Parent ${accountId} has only ${(Number(balance) / 1e24).toFixed(3)} NEAR; \
deploy requires at least ${(Number(VAULT_PARENT_BUDGET_YOCTO) / 1e24).toFixed(2)} NEAR \
(${(Number(VAULT_INITIAL_YOCTO) / 1e24).toFixed(2)} for the vault + ~0.1 NEAR gas).`,
        );
      }

      // 1. Resolve vault code hash from keystore-DAO.
      //
      // `getVaultCodeHash` view-calls `list_approved_vault_versions`
      // and picks the most recently approved non-deprecated entry.
      // No env-var to keep in sync — when the DAO whitelists a new
      // vault version, the dashboard picks it up automatically.
      // Bails with a clear error if no non-deprecated version exists.
      setBusy('Resolving vault code hash from keystore-DAO…');
      const { hashB58, hashBytes } = await getVaultCodeHash(viewMethod, network);

      // 2. TEE pubkey BEFORE deploy.
      setBusy('Fetching TEE function-call pubkey…');
      const teePubkey = await deriveVaultTeeKey(network, vaultAccountId);

      // 3. Atomic deploy via UseGlobalContract — references the
      //    on-chain global vault contract by hash instead of shipping
      //    the 150 KB WASM in this tx. Tx payload < 1 KB so it fits
      //    inside MyNearWallet's URL limit.
      setBusy('Signing atomic deploy (5 actions, global contract by hash)…');
      const cfg = getVaultNetworkConfig(network);
      const actions = buildVaultDeployActions({
        parent: accountId,
        vaultAccountId,
        keystoreDaoId: cfg.keystoreDaoId,
        mpcContractId: cfg.mpcContractId,
        exitWindowSecs: exitSecs,
        wasmCodeHash: hashBytes,
        teePublicKey: teePubkey,
      });
      const outcome = await signAndSendTransaction({
        receiverId: vaultAccountId,
        actions,
      });
      const txHash = outcome?.transaction?.hash || outcome?.transaction_outcome?.id || '<unknown>';

      // 4. Drive sign-verification (mark_vault_verified).
      setBusy('Triggering on-chain mark_vault_verified…');
      await signVaultVerification(network, vaultAccountId);

      // 5. Mint API key.
      setBusy('Registering with coordinator…');
      const reg = await customerRegister(network, vaultAccountId);

      setIssuedApiKey({
        vault: reg.vault_id,
        apiKey: reg.api_key,
        nearAccountId: reg.near_account_id,
      });
      setSuccess(`Vault deployed and verified (tx ${txHash}). Save the API key below — it is shown once.`);
      await refreshReport(vaultAccountId);
    } catch (e) {
      setError(`Vault init failed: ${(e as Error).message}`);
    } finally {
      setBusy(null);
      inFlight.current = false;
    }
  };

  // ── Resume an interrupted init ────────────────────────────────────────
  const handleResume = async (vaultAccountId: string) => {
    if (!vaultAccountId) return;
    setError(null);
    setSuccess(null);
    setIssuedApiKey(null);
    setBusy(`Resuming ${vaultAccountId}…`);
    try {
      // Step 4 — idempotent on the keystore side.
      await signVaultVerification(network, vaultAccountId);
      // Step 5 — surface unique-violation specifically.
      try {
        const reg = await customerRegister(network, vaultAccountId);
        setIssuedApiKey({
          vault: reg.vault_id,
          apiKey: reg.api_key,
          nearAccountId: reg.near_account_id,
        });
        setSuccess('Vault registration completed. Save the API key below — it is shown once.');
      } catch (e) {
        const msg = (e as Error).message;
        // Coordinator's UNIQUE-violation branch surfaces a 400 with one
        // of these phrases. We match on whichever survived the wire.
        // Phrase coupling is fragile — see PROJECT.md / coordinator
        // handlers if these stop matching.
        const REGISTERED_PHRASES = [
          'already bound to a wallet',
          'revoke its API keys',
          'already registered',
          '23505',
        ];
        if (REGISTERED_PHRASES.some((p) => msg.includes(p))) {
          throw new Error(
            `${vaultAccountId} is already registered on the coordinator (a previous \
run committed but the API key was never returned). On-chain state is intact; \
contact OutLayer support to rotate the API key — no funds at risk.`,
          );
        }
        throw e;
      }
      await refreshReport(vaultAccountId);
    } catch (e) {
      setError(`Resume failed: ${(e as Error).message}`);
    } finally {
      setBusy(null);
    }
  };

  // ── Inspect / refresh ─────────────────────────────────────────────────
  const handleFind = async () => {
    setError(null);
    setSuccess(null);
    if (!findInput.trim()) {
      setError('Enter a vault account id (e.g. vault.alice.near).');
      return;
    }
    setBusy(`Loading ${findInput.trim()}…`);
    try {
      await refreshReport(findInput.trim());
    } finally {
      setBusy(null);
    }
  };

  // ── Recovery / window / add-key actions ──────────────────────────────
  // All call directly into the vault contract; the parent NEAR account is
  // the only signer that can use the predecessor-gated paths.
  const callVault = async (
    vaultId: string,
    method: string,
    args: Record<string, unknown>,
    label: string,
  ) => {
    if (!guard(`Connect a NEAR wallet to call ${method}.`)) return;
    setError(null);
    setSuccess(null);
    setBusy(label);
    try {
      const action = actionCreators.functionCall(
        method,
        new TextEncoder().encode(JSON.stringify(args)),
        VAULT_CALL_GAS,
        BigInt(0),
      );
      const outcome = await signAndSendTransaction({
        receiverId: vaultId,
        actions: [action],
      });
      const tx = outcome?.transaction?.hash || outcome?.transaction_outcome?.id || '<ok>';
      setSuccess(`${label} → tx ${tx}`);
      await refreshReport(vaultId);
    } catch (e) {
      setError(`${label} failed: ${(e as Error).message}`);
    } finally {
      setBusy(null);
    }
  };

  const initiateRecovery = (vaultId: string) =>
    callVault(vaultId, 'initiate_recovery', {}, 'initiate_recovery (cessation)');

  const initiateUnilateralRecovery = (vaultId: string) =>
    callVault(
      vaultId,
      'unilateral_initiate_recovery',
      {},
      'unilateral_initiate_recovery',
    );

  const finalizeRecovery = (vaultId: string) =>
    callVault(vaultId, 'finalize_recovery', {}, 'finalize_recovery');

  const setExitWindowOnVault = async (vaultId: string, window: string) => {
    let secs: number;
    try {
      secs = parseExitWindow(window);
    } catch (e) {
      setError((e as Error).message);
      return;
    }
    await callVault(
      vaultId,
      'set_exit_window',
      { new_window_secs: secs },
      `set_exit_window (${formatSeconds(secs)})`,
    );
  };

  const unlockedAddKey = async (vaultId: string, pubkey: string, fullAccess: boolean) => {
    try {
      // Reject malformed pubkeys client-side — a contract panic costs
      // gas. Same pre-flight as the CLI.
      PublicKey.fromString(pubkey);
    } catch {
      setError(`'${pubkey}' is not a valid NEAR public key (expected 'ed25519:...').`);
      return;
    }
    await callVault(
      vaultId,
      'unlocked_add_key',
      {
        public_key: pubkey,
        full_access: fullAccess,
        // null = contract default (1 NEAR allowance for FCAK).
        allowance: null,
      },
      `unlocked_add_key (${fullAccess ? 'FULL' : 'FCAK'})`,
    );
  };

  // ── Render ────────────────────────────────────────────────────────────
  return (
    <div className="container mx-auto p-6 max-w-5xl">
      <h1 className="text-3xl font-bold mb-2">MPC Vaults</h1>
      <p className="text-gray-700 mb-2">
        Deploy a CKD-issuer contract bound to your NEAR account. OutLayer&apos;s
        keystore TEE derives your per-customer master <em>inside the enclave</em>
        via NEAR&apos;s MPC network; from that master it generates keys for
        your agents&apos; wallets, encrypted secrets, and payment checks on
        demand &mdash; all without anyone seeing the raw master.
      </p>
      <p className="text-gray-700 mb-2">
        You either let OutLayer&apos;s TEE manage this vault, or later take it
        over yourself (run it from your own TEE / runtime, or use the master
        manually). It&apos;s a one-way switch: once you take over, OutLayer
        stops serving this vault &mdash; but you keep every derived key,
        because the same MPC path reproduces the same master.
      </p>
      <details className="mb-6 text-sm text-gray-700 bg-gray-50 border border-gray-200 rounded p-3">
        <summary className="cursor-pointer font-medium text-gray-800">
          What is CKD?
        </summary>
        <div className="mt-2 space-y-2">
          <p>
            <strong>Conditional Key Derivation</strong> is a NEAR MPC primitive.
            The MPC network&apos;s threshold-key holders jointly derive a
            private key for a given <em>app id</em> &mdash; deterministically,
            without any single node ever assembling the secret. The key is
            unique to the predecessor account that requested it.
          </p>
          <p>
            Here, the predecessor is your vault contract and the app id is
            an HMAC of <code>vault-master:{'<your_vault_id>'}</code>. The
            keystore TEE asks NEAR MPC for the 32 bytes; same inputs &rArr;
            same master, every time. From that master, all your wallet keys
            and secret-encryption keys are HKDF-derived inside the enclave.
            Detaching from OutLayer = you query the same MPC path from the
            vault account and get the same master back.{' '}
            <a className="text-blue-600 hover:underline" href="/docs/vaults">
              Full explanation
            </a>.
          </p>
        </div>
      </details>

      {!isConnected && (
        <div className="bg-yellow-50 border border-yellow-300 rounded p-4 mb-6">
          <p className="text-sm">Connect a NEAR wallet to create or manage vaults.</p>
          <button
            onClick={() => setShowWalletModal(true)}
            className="mt-2 px-4 py-2 bg-blue-600 text-white rounded hover:bg-blue-700"
          >
            Connect Wallet
          </button>
        </div>
      )}

      {error && (
        <div className="bg-red-50 border border-red-300 rounded p-3 mb-4 text-sm">
          <strong>Error:</strong> {error}
        </div>
      )}
      {success && (
        <div className="bg-green-50 border border-green-300 rounded p-3 mb-4 text-sm">
          {success}
        </div>
      )}
      {busy && (
        <div className="bg-blue-50 border border-blue-300 rounded p-3 mb-4 text-sm">
          ⏳ {busy}
        </div>
      )}

      {issuedApiKey && (
        <IssuedVaultPanel data={issuedApiKey} />
      )}

      {/* ── Create vault ─────────────────────────────────────────────── */}
      <section className="border border-gray-200 rounded p-4 mb-6">
        <h2 className="text-xl font-semibold mb-3">Create vault</h2>
        <div className="text-sm text-gray-600 mb-3">
          Deploys <code>{name || 'vault'}.{accountId || '&lt;your-account&gt;'}</code> with a single atomic
          NEAR transaction (CreateAccount + Transfer{' '}
          {(Number(VAULT_INITIAL_YOCTO) / 1e24).toFixed(2)} NEAR + DeployContract +{' '}
          new() + AddKey TEE function-call key).
        </div>
        <div className="grid grid-cols-1 md:grid-cols-3 gap-3 mb-3">
          <label className="block">
            <span className="text-sm">Sub-account name</span>
            <input
              type="text"
              value={name}
              onChange={(e) => setName(e.target.value)}
              className="mt-1 block w-full rounded-md border border-gray-300 bg-white px-3 py-2 text-gray-900 shadow-sm focus:border-[#cc6600] focus:ring-[#cc6600]"
              placeholder="vault"
            />
          </label>
          <label className="block">
            <span className="text-sm">Unilateral exit window</span>
            <select
              value={exitWindow}
              onChange={(e) => setExitWindow(e.target.value)}
              className="mt-1 block w-full rounded-md border border-gray-300 bg-white px-3 py-2 text-gray-900 shadow-sm focus:border-[#cc6600] focus:ring-[#cc6600]"
            >
              {EXIT_WINDOW_OPTIONS.map((o) => (
                <option key={o.value} value={o.value}>
                  {o.label}
                </option>
              ))}
            </select>
          </label>
          <div className="flex items-end">
            <button
              onClick={handleCreate}
              disabled={!isConnected || !!busy}
              className="w-full px-4 py-2 bg-blue-600 text-white rounded hover:bg-blue-700 disabled:bg-gray-400"
            >
              Create vault
            </button>
          </div>
        </div>
        <p className="text-xs text-gray-500">
          Parent (= your account, immutable post-deploy) is the only NEAR account
          that can call <code>unilateral_initiate_recovery</code>,{' '}
          <code>set_exit_window</code>, or <code>unlocked_add_key</code>.
        </p>
      </section>

      {/* ── Find / inspect vault ─────────────────────────────────────── */}
      <section className="border border-gray-200 rounded p-4 mb-6">
        <h2 className="text-xl font-semibold mb-3">Inspect a vault</h2>
        <div className="flex gap-2 mb-3">
          <input
            type="text"
            value={findInput}
            onChange={(e) => setFindInput(e.target.value)}
            placeholder="vault.alice.near"
            className="flex-1 rounded-md border border-gray-300 bg-white px-3 py-2 text-gray-900 shadow-sm focus:border-[#cc6600] focus:ring-[#cc6600]"
          />
          <button
            onClick={handleFind}
            disabled={!!busy}
            className="px-4 py-2 bg-gray-600 text-white rounded hover:bg-gray-700 disabled:bg-gray-400"
          >
            Load
          </button>
          <button
            onClick={() => findInput.trim() && handleResume(findInput.trim())}
            disabled={!isConnected || !!busy || !findInput.trim()}
            className="px-4 py-2 bg-amber-600 text-white rounded hover:bg-amber-700 disabled:bg-gray-400"
            title="Run sign-verification + customer/register against an already-deployed vault"
          >
            Resume
          </button>
        </div>

        {report && activeVaultId && (
          <VaultDetailPanel
            report={report}
            onInitiateRecovery={() => initiateRecovery(activeVaultId)}
            onInitiateUnilateral={() => initiateUnilateralRecovery(activeVaultId)}
            onFinalize={() => finalizeRecovery(activeVaultId)}
            onSetExitWindow={(w) => setExitWindowOnVault(activeVaultId, w)}
            onAddKey={(pk, fa) => unlockedAddKey(activeVaultId, pk, fa)}
            onRefresh={() => refreshReport(activeVaultId)}
            disabled={!!busy || !isConnected}
          />
        )}
      </section>

      <WalletConnectionModal
        isOpen={showWalletModal}
        onClose={() => setShowWalletModal(false)}
      />
    </div>
  );
}

// ─── Issued vault panel ────────────────────────────────────────────────────
//
// Shown right after a successful CKD-issuer (a.k.a. "vault") deploy.
// The contract itself is just an on-chain admin/governance container
// that binds a per-customer master inside the keystore TEE via MPC CKD.
// From that master OutLayer derives an unbounded family of keypairs on
// demand (`wallet:{wallet_id}:near`, `wallet:{wallet_id}:eth`,
// `check:{counter}`, `vault-master:...`, etc.). Treating any individual
// derivation as "the wallet" is misleading — there is no canonical
// wallet, the keystore mints whichever address the current call needs.
//
// We do NOT show:
//   - the `wk_...` trial token from `/customer/register` — coordinator
//     trial-quota artifact, irrelevant to vault sovereignty;
//   - the `wallet:{wallet_id}:near` derivation — happens to exist
//     because /customer/register asks for it, but it's just one of many
//     possible derivations and the user has no reason to deposit funds
//     there as if it were an account.
//
// What matters is the contract account and its recoverability.
function IssuedVaultPanel({
  data,
}: {
  data: { vault: string };
}) {
  return (
    <div className="bg-green-50 border-2 border-green-600 rounded p-4 mb-6">
      <h3 className="font-bold text-green-900 mb-2">
        ✓ Custody contract deployed and verified
      </h3>
      <div className="text-sm text-gray-800 space-y-2">
        <div>
          <code className="block bg-white px-2 py-1 rounded text-xs break-all">{data.vault}</code>
        </div>
        <div className="text-xs text-gray-700">
          On-chain CKD issuer. Binds your per-customer master inside the
          keystore TEE (via MPC CKD) so OutLayer can derive keys for your
          agents, secrets, and payment checks on demand. No funds live on
          this contract — it's a governance/recovery root.
        </div>
        <div className="text-xs text-gray-700">
          If OutLayer stops serving, the parent account regains control via{' '}
          <code>initiate_unilateral_recovery</code> →{' '}
          <code>finalize_recovery</code>, and the per-customer master is
          recoverable via the <code>customer-recovery</code> script.
        </div>
        <div className="text-xs text-amber-800 bg-amber-50 border border-amber-200 rounded px-2 py-1.5">
          <strong>Heads up:</strong> outbound MPC-CKD calls
          (<code>vault.request_master</code>) burn gas from <em>this</em>{' '}
          account. The vault was funded with ~0.1 NEAR at deploy — when the
          balance gets low you&rsquo;ll need to top it up by sending NEAR to{' '}
          <code>{data.vault}</code>. The dashboard surfaces a top-up
          prompt on the vault detail page below the threshold.
        </div>
      </div>
    </div>
  );
}

// ─── Detail panel ───────────────────────────────────────────────────────────

function VaultDetailPanel(props: {
  report: VerifyReport;
  onInitiateRecovery: () => void;
  onInitiateUnilateral: () => void;
  onFinalize: () => void;
  onSetExitWindow: (w: string) => void;
  onAddKey: (pubkey: string, fullAccess: boolean) => void;
  onRefresh: () => void;
  disabled: boolean;
}) {
  const { report, disabled } = props;
  const [newWindow, setNewWindow] = useState('24h');
  const [newPubkey, setNewPubkey] = useState('');
  const [newKeyFullAccess, setNewKeyFullAccess] = useState(false);
  // Recovery / parent-only admin controls live behind a toggle so the
  // happy path doesn't open with three orange "Initiate recovery"
  // buttons screaming at the user. The state itself stays visible
  // above; only the action buttons are hidden by default.
  const [showAdvanced, setShowAdvanced] = useState(false);

  if (!report.exists) {
    return (
      <div className="bg-gray-50 rounded p-3 text-sm">
        Account <code>{report.vaultId}</code> does not exist on chain.
      </div>
    );
  }
  const s = report.state;

  return (
    <div className="border border-gray-200 rounded p-3 text-sm">
      <div className="flex justify-between items-center mb-2">
        <h3 className="font-semibold">{report.vaultId}</h3>
        <button
          onClick={props.onRefresh}
          disabled={disabled}
          className="text-xs px-2 py-1 bg-gray-200 rounded"
        >
          Refresh
        </button>
      </div>

      <div
        className={`px-2 py-1 rounded mb-3 font-medium text-sm ${
          report.safe
            ? 'bg-green-100 text-green-800'
            : 'bg-red-100 text-red-800'
        }`}
      >
        {report.safe
          ? 'PASS — verified, all defense-in-depth checks OK'
          : report.isVerified
            ? 'NOT SAFE — defense-in-depth checks failed (do not deposit)'
            : 'NOT VERIFIED — vault is not in keystore-DAO verified set'}
      </div>

      {report.warnings.length > 0 && (
        <ul className="text-xs text-amber-700 mb-2 list-disc list-inside">
          {report.warnings.map((w, i) => (
            <li key={i}>{w}</li>
          ))}
        </ul>
      )}

      {s && (
        <table className="text-xs w-full mb-3">
          <tbody>
            <tr>
              <td className="text-gray-500 pr-3">Parent</td>
              <td><code>{s.parent}</code></td>
            </tr>
            <tr>
              <td className="text-gray-500 pr-3">keystore-DAO</td>
              <td><code>{s.keystore_dao}</code></td>
            </tr>
            <tr>
              <td className="text-gray-500 pr-3">MPC contract</td>
              <td><code>{s.mpc_contract}</code></td>
            </tr>
            <tr>
              <td className="text-gray-500 pr-3">Status</td>
              <td>{s.unlocked ? 'UNLOCKED (recovered)' : 'locked (TEE-controlled)'}</td>
            </tr>
            <tr>
              <td className="text-gray-500 pr-3">Exit window</td>
              <td>{formatSeconds(s.unilateral_exit_window_secs)}</td>
            </tr>
            <tr>
              <td
                className="text-gray-500 pr-3"
                title="Vault account balance. Outbound MPC-CKD calls (vault.request_master → mpc.request_app_private_key) burn gas from this balance. Top up if low."
              >
                Balance
              </td>
              <td>
                {(Number(report.amountYocto) / 1e24).toFixed(4)} NEAR
                {BigInt(report.amountYocto) < VAULT_LOW_BALANCE_YOCTO && (
                  <span className="ml-2 text-amber-700 font-medium">
                    ⚠ low — top up below
                  </span>
                )}
              </td>
            </tr>
            <tr>
              <td
                className="text-gray-500 pr-3"
                title="Informational rotation registry inside the contract.
The authoritative list is the account's access keys (see vault-checker).
Atomic deploy adds the FC access key at the account level only — this Vec
fills up later if/when someone calls propose_tee_key for explicit rotation
tracking."
              >
                Registered TEE keys (registry)
              </td>
              <td>
                {s.registered_tee_keys.length}
                {s.registered_tee_keys.length === 0 && report.safe && (
                  <span className="text-gray-400"> — informational; active TEE access key is on the account</span>
                )}
              </td>
            </tr>
            {s.recovery && (
              <tr>
                <td className="text-gray-500 pr-3">Recovery</td>
                <td>
                  {s.recovery.trigger} — finalize after{' '}
                  {nsToDate(s.recovery.finalize_after).toLocaleString()}
                </td>
              </tr>
            )}
          </tbody>
        </table>
      )}

      {/* Top-up prompt — shown whenever the vault balance is low. The
          gas to refresh the per-customer master via MPC-CKD is paid
          out of this balance, so an empty vault wedges the keystore's
          ability to keep serving derived keys for this customer until
          the parent (or anyone, NEAR transfers are permissionless)
          tops it up. */}
      {report.exists && BigInt(report.amountYocto) < VAULT_LOW_BALANCE_YOCTO && (
        <div className="bg-amber-50 border border-amber-300 rounded p-3 mb-3 text-sm">
          <div className="font-medium text-amber-900 mb-1">⚠ Vault balance is low</div>
          <p className="text-gray-700 mb-2">
            <code>{report.vaultId}</code> has{' '}
            <strong>{(Number(report.amountYocto) / 1e24).toFixed(4)} NEAR</strong>.
            Outbound MPC-CKD calls (<code>vault.request_master</code>) burn gas
            from this account. Once the balance goes below storage stake the
            keystore stops being able to refresh your master, and any
            derived-key request that requires re-fetching it will stall.
          </p>
          <p className="text-gray-700 mb-2">
            Top up by transferring NEAR to <code>{report.vaultId}</code> from
            any account — it&rsquo;s a plain on-chain transfer, no contract
            method. Suggested:{' '}
            <strong>{(Number(VAULT_TOPUP_SUGGESTED_YOCTO) / 1e24).toFixed(2)} NEAR</strong>{' '}
            (~100 MPC calls of headroom).
          </p>
          <pre className="text-xs bg-white border border-gray-200 rounded p-2 mt-2 overflow-x-auto">
{`# CLI:
near send <your_account> ${report.vaultId} ${(Number(VAULT_TOPUP_SUGGESTED_YOCTO) / 1e24).toFixed(2)}

# or any wallet — Send NEAR to ${report.vaultId}`}
          </pre>
        </div>
      )}

      <button
        onClick={() => setShowAdvanced((v) => !v)}
        className="text-xs text-gray-600 underline hover:text-gray-900 mb-2"
      >
        {showAdvanced ? '▾ Hide advanced (recovery, exit window, post-unlock keys)' : '▸ Advanced (recovery, exit window, post-unlock keys)'}
      </button>

      {showAdvanced && (
        <>
          <div className="flex flex-wrap gap-2 mb-3">
            <button
              onClick={props.onInitiateRecovery}
              disabled={disabled || !!s?.unlocked || !!s?.recovery}
              className="px-3 py-1 bg-orange-600 text-white rounded text-xs hover:bg-orange-700 disabled:bg-gray-400"
              title="Cessation-triggered (DAO must have declared cessation)"
            >
              Initiate cessation recovery
            </button>
            <button
              onClick={props.onInitiateUnilateral}
              disabled={disabled || !!s?.unlocked || !!s?.recovery}
              className="px-3 py-1 bg-orange-600 text-white rounded text-xs hover:bg-orange-700 disabled:bg-gray-400"
              title="Parent-only voluntary exit"
            >
              Initiate unilateral recovery
            </button>
            <button
              onClick={props.onFinalize}
              disabled={disabled || !s?.recovery}
              className="px-3 py-1 bg-purple-600 text-white rounded text-xs hover:bg-purple-700 disabled:bg-gray-400"
            >
              Finalize recovery
            </button>
          </div>

          <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
            <div className="border border-gray-200 rounded p-2">
              <div className="text-xs font-medium mb-1">Update exit window (parent only)</div>
              <div className="flex gap-2">
                <select
                  value={newWindow}
                  onChange={(e) => setNewWindow(e.target.value)}
                  className="flex-1 text-xs rounded-md border border-gray-300 bg-white px-2 py-1 text-gray-900 shadow-sm focus:border-[#cc6600] focus:ring-[#cc6600]"
                >
                  {EXIT_WINDOW_OPTIONS.map((o) => (
                    <option key={o.value} value={o.value}>{o.label}</option>
                  ))}
                </select>
                <button
                  onClick={() => props.onSetExitWindow(newWindow)}
                  disabled={disabled}
                  className="px-3 py-1 bg-gray-600 text-white rounded text-xs hover:bg-gray-700 disabled:bg-gray-400"
                >
                  Set
                </button>
              </div>
            </div>

            <div className="border border-gray-200 rounded p-2">
              <div className="text-xs font-medium mb-1">Add key (post-unlock, parent only)</div>
              <input
                type="text"
                value={newPubkey}
                onChange={(e) => setNewPubkey(e.target.value)}
                placeholder="ed25519:..."
                className="block w-full text-xs rounded-md border border-gray-300 bg-white px-2 py-1 text-gray-900 shadow-sm focus:border-[#cc6600] focus:ring-[#cc6600] mb-1"
              />
              <label className="text-xs flex items-center gap-1 mb-1">
                <input
                  type="checkbox"
                  checked={newKeyFullAccess}
                  onChange={(e) => setNewKeyFullAccess(e.target.checked)}
                />
                Full-access key (default: function-call, 1 NEAR allowance)
              </label>
              <button
                onClick={() => props.onAddKey(newPubkey, newKeyFullAccess)}
                disabled={disabled || !s?.unlocked}
                className="w-full px-3 py-1 bg-gray-600 text-white rounded text-xs hover:bg-gray-700 disabled:bg-gray-400"
              >
                unlocked_add_key
              </button>
              {s && !s.unlocked && (
                <p className="text-xs text-gray-500 mt-1">Vault must be unlocked first.</p>
              )}
            </div>
          </div>
        </>
      )}
    </div>
  );
}
