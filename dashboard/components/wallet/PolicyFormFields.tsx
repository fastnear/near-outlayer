import { useState } from 'react';
import { PolicyForm } from '@/lib/wallet-policy';
import { AuthorizedKeysSection } from './AuthorizedKeysSection';

interface PolicyFormFieldsProps {
  policyForm: PolicyForm;
  onChange: (form: PolicyForm) => void;
  /** SHA256 hash of the current API key (auto-included, shown as read-only) */
  apiKeyHash?: string;
  /** Map of hash → label for hashes we can identify (from localStorage etc.) */
  knownKeyHashes?: Map<string, string>;
  /** Callback to save a generated/entered key to localStorage */
  onSaveKey?: (apiKey: string) => void;
}

export function PolicyFormFields({ policyForm, onChange, apiKeyHash, knownKeyHashes, onSaveKey }: PolicyFormFieldsProps) {
  const update = (patch: Partial<PolicyForm>) => onChange({ ...policyForm, ...patch });
  const [showWebhookInfo, setShowWebhookInfo] = useState(false);

  return (
    <div className="space-y-4">
      {/* Spending Limits */}
      <div>
        <h3 className="text-sm font-semibold text-gray-800 mb-2">Spending Limits (NEAR)</h3>
        <p className="text-xs text-gray-400 mb-2">Native NEAR only (not wNEAR or NEP-141 tokens). Leave empty for no limit. For token-specific limits, use the JSON editor.</p>
        <div className="grid grid-cols-2 gap-4">
          <div>
            <label className="block text-xs font-medium text-gray-600 mb-1">Per-Transaction</label>
            <input
              type="text"
              value={policyForm.per_transaction_limit}
              onChange={(e) => update({ per_transaction_limit: e.target.value })}
              placeholder="e.g. 10"
              className="w-full border border-gray-300 rounded px-3 py-2 text-sm"
            />
          </div>
          <div>
            <label className="block text-xs font-medium text-gray-600 mb-1">Hourly</label>
            <input
              type="text"
              value={policyForm.hourly_limit}
              onChange={(e) => update({ hourly_limit: e.target.value })}
              placeholder="e.g. 50"
              className="w-full border border-gray-300 rounded px-3 py-2 text-sm"
            />
          </div>
          <div>
            <label className="block text-xs font-medium text-gray-600 mb-1">Daily</label>
            <input
              type="text"
              value={policyForm.daily_limit}
              onChange={(e) => update({ daily_limit: e.target.value })}
              placeholder="e.g. 100"
              className="w-full border border-gray-300 rounded px-3 py-2 text-sm"
            />
          </div>
          <div>
            <label className="block text-xs font-medium text-gray-600 mb-1">Monthly</label>
            <input
              type="text"
              value={policyForm.monthly_limit}
              onChange={(e) => update({ monthly_limit: e.target.value })}
              placeholder="e.g. 1000"
              className="w-full border border-gray-300 rounded px-3 py-2 text-sm"
            />
          </div>
        </div>
      </div>

      {/* Address Restrictions */}
      <div className="border border-gray-200 rounded-lg p-4">
        <h3 className="text-sm font-semibold text-gray-800 mb-2">Address Restrictions</h3>
        <p className="text-xs text-gray-400 mb-2">Restrict which destination accounts the wallet can interact with (withdraw, transfer, contract call).</p>
        <div className="grid grid-cols-3 gap-2 mb-2">
          {([
            { mode: 'none' as const, label: 'No restriction', desc: 'Any address allowed' },
            { mode: 'whitelist' as const, label: 'Whitelist', desc: 'Only listed addresses allowed' },
            { mode: 'blacklist' as const, label: 'Blacklist', desc: 'Listed addresses blocked' },
          ]).map(({ mode, label, desc }) => (
            <button
              key={mode}
              type="button"
              onClick={() => update({ address_mode: mode })}
              title={desc}
              className={`px-3 py-1.5 text-xs rounded border ${
                policyForm.address_mode === mode
                  ? 'bg-[#cc6600] text-white border-[#cc6600]'
                  : 'bg-white text-gray-700 border-gray-300 hover:bg-gray-50'
              }`}
            >
              {label}
            </button>
          ))}
        </div>
        {policyForm.address_mode !== 'none' && (
          <input
            type="text"
            value={policyForm.addresses}
            onChange={(e) => update({ addresses: e.target.value })}
            placeholder="bob.near, alice.near (comma-separated)"
            className="w-full border border-gray-300 rounded px-3 py-2 text-sm"
          />
        )}
      </div>

      {/* Allowed Tokens & Transaction Types */}
      <div className="grid grid-cols-2 gap-4">
        <div>
          <label className="block text-sm font-medium text-gray-700 mb-1">Allowed Tokens</label>
          <input
            type="text"
            value={policyForm.allowed_tokens}
            onChange={(e) => update({ allowed_tokens: e.target.value })}
            placeholder="* for all, or: native, nep141:usdt..."
            className="w-full border border-gray-300 rounded px-3 py-2 text-sm"
          />
        </div>
        <div>
          <label className="block text-sm font-medium text-gray-700 mb-1">Allowed Transaction Types</label>
          {(() => {
            const txTypeLabels: Record<string, string> = {
              transfer: 'Transfer (send native token)',
              call: 'Contract call (incl. Deposit to Intents)',
              delete: 'Delete wallet',
              intents_withdraw: 'Withdraw (same-chain)',
              intents_swap: 'Swap',
              cross_chain_withdraw: 'Send cross-chain (bridge off NEAR)',
            };
            const types = policyForm.transaction_types.split(',').map((t) => t.trim()).filter(Boolean);
            const renderCheckbox = (txType: string) => {
              const checked = types.includes(txType);
              return (
                <label key={txType} className="flex items-center gap-1.5 text-sm cursor-pointer">
                  <input
                    type="checkbox"
                    checked={checked}
                    onChange={() => {
                      const next = checked ? types.filter((t: string) => t !== txType) : [...types, txType];
                      update({ transaction_types: next.join(',') });
                    }}
                    className="rounded border-gray-300"
                  />
                  {txTypeLabels[txType] || txType}
                </label>
              );
            };
            return (
              <div className="space-y-2 mt-1">
                <div>
                  <span className="text-xs text-gray-400">Direct on-chain:</span>
                  <div className="flex flex-col gap-0.5 mt-0.5">
                    {['transfer', 'call', 'delete'].map(renderCheckbox)}
                  </div>
                </div>
                <div>
                  <span className="text-xs text-gray-400">NEAR Intents:</span>
                  <div className="flex flex-col gap-0.5 mt-0.5">
                    {['intents_withdraw', 'intents_swap', 'cross_chain_withdraw'].map(renderCheckbox)}
                  </div>
                  {types.includes('cross_chain_withdraw') && (
                    <p className="text-xs text-red-600 mt-1">
                      ⚠ Cross-chain withdraw bridges funds OFF NEAR irreversibly. It is a
                      separate, opt-in type — enabling it does not require enabling
                      same-chain withdraw, and vice versa.
                    </p>
                  )}
                </div>
              </div>
            );
          })()}
        </div>
      </div>

      {/* Capabilities (advanced, default-DENY) */}
      <div className="border border-amber-200 bg-amber-50 rounded-lg p-4">
        <h3 className="text-sm font-semibold text-gray-800 mb-1">Capabilities (advanced)</h3>
        <p className="text-xs text-gray-500 mb-3">
          Powerful primitives that bypass parts of the structured policy above. All are
          OFF by default — enable only with intent. A wallet with NO policy at all trusts
          the coordinator fully; set a policy before it holds significant value.
        </p>

        {/* raw_sign */}
        <div className="mb-3">
          <label className="flex items-center gap-1.5 text-sm cursor-pointer">
            <input
              type="checkbox"
              checked={policyForm.raw_sign_enabled}
              onChange={(e) => update({ raw_sign_enabled: e.target.checked })}
              className="rounded border-gray-300"
            />
            <span className="font-medium">raw_sign — sign arbitrary payloads</span>
          </label>
          <p className="text-xs text-red-600 mt-0.5 ml-5">
            ⚠ Lets the agent (and a compromised coordinator) sign raw transactions on the
            enabled chains, bypassing the transfer/call/limit policy. Leaving chains empty
            allows ALL chains <strong>including NEAR</strong> (which shares the structured
            key — a raw NEAR signature can move funds outside this policy).
          </p>
          {policyForm.raw_sign_enabled && (
            <div className="ml-5 mt-1 space-y-1">
              <input
                type="text"
                value={policyForm.raw_sign_chains}
                onChange={(e) => update({ raw_sign_chains: e.target.value })}
                placeholder="Chain allowlist, e.g. ethereum, solana (empty = ALL incl. near)"
                className="w-full border border-gray-300 rounded px-3 py-2 text-sm"
              />
              <label className="flex items-center gap-1.5 text-xs cursor-pointer">
                <input
                  type="checkbox"
                  checked={policyForm.raw_sign_requires_approval}
                  onChange={(e) => update({ raw_sign_requires_approval: e.target.checked })}
                  className="rounded border-gray-300"
                />
                Require multisig approval for raw_sign
              </label>
            </div>
          )}
        </div>

        {/* confidential */}
        <div className="mb-3">
          <label className="flex items-center gap-1.5 text-sm cursor-pointer">
            <input
              type="checkbox"
              checked={policyForm.confidential_enabled}
              onChange={(e) => update({ confidential_enabled: e.target.checked })}
              className="rounded border-gray-300"
            />
            <span className="font-medium">confidential — confidential-intents flows</span>
          </label>
          <p className="text-xs text-red-600 mt-0.5 ml-5">
            ⚠ Trusted: the route/deposit-address/artifact is coordinator-supplied and NOT
            bound to this policy, so enabling it = full coordinator-trust of this token's
            balance <strong>even single-sig</strong>. (Shield / unshield / confidential
            withdraw, transfer, swap on the Defuse confidential shard.)
          </p>
          {policyForm.confidential_enabled && (
            <p className="text-xs text-gray-400 ml-5 mt-1">
              Multisig approval is not available for trusted ops (the post-approval artifact
              can't be bound to the approved op) — gate with limits + capability instead.
            </p>
          )}
        </div>

        {/* swap */}
        <div className="mb-3">
          <label className="flex items-center gap-1.5 text-sm cursor-pointer">
            <input
              type="checkbox"
              checked={policyForm.swap_enabled}
              onChange={(e) => update({ swap_enabled: e.target.checked })}
              className="rounded border-gray-300"
            />
            <span className="font-medium">swap — 1Click token swaps</span>
          </label>
          <p className="text-xs text-red-600 mt-0.5 ml-5">
            ⚠ Trusted: the quote/route/deposit-address is coordinator-supplied and NOT bound
            to this policy, so enabling it = full coordinator-trust of the input token's
            balance <strong>even single-sig</strong>. Default-OFF — without this, swaps are
            denied regardless of the transaction-type list.
          </p>
          {policyForm.swap_enabled && (
            <p className="text-xs text-gray-400 ml-5 mt-1">
              Multisig approval is not available for trusted ops — gate with limits +
              capability instead.
            </p>
          )}
        </div>

        {/* payment_check */}
        <div className="mb-3">
          <label className="flex items-center gap-1.5 text-sm cursor-pointer">
            <input
              type="checkbox"
              checked={policyForm.payment_check_enabled}
              onChange={(e) => update({ payment_check_enabled: e.target.checked })}
              className="rounded border-gray-300"
            />
            <span className="font-medium">payment_check — claimable payment links</span>
          </label>
          <p className="text-xs text-red-600 mt-0.5 ml-5">
            ⚠ Whitelist-bypass AND Trusted: funds move into an escrow ANY link holder can
            claim (the address whitelist does NOT constrain the recipient), and the transfer
            artifact is coordinator-supplied and NOT bound to this policy — so the
            per-transaction amount limit only constrains an HONEST coordinator. Enabling this
            trusts the coordinator with this token's whole balance, even single-sig.
          </p>
          {policyForm.payment_check_enabled && (
            <p className="text-xs text-gray-400 ml-5 mt-1">
              Multisig approval is not available for payment checks — gate with the
              per-transaction amount limit instead.
            </p>
          )}
        </div>

        {/* cross_chain_withdraw */}
        <div className="mb-3">
          <label className="flex items-center gap-1.5 text-sm cursor-pointer">
            <input
              type="checkbox"
              checked={policyForm.cross_chain_withdraw_enabled}
              onChange={(e) => update({ cross_chain_withdraw_enabled: e.target.checked })}
              className="rounded border-gray-300"
            />
            <span className="font-medium">cross_chain_withdraw — bridge funds off NEAR</span>
          </label>
          <p className="text-xs text-red-600 mt-0.5 ml-5">
            ⚠ The riskiest, irreversible exit, AND Trusted. Default-DENY: without this
            capability, cross-chain withdraw is denied even if listed in the transaction
            types. The 1Click swap+bridge artifact is coordinator-supplied and NOT bound to
            this policy, so the amount limit only constrains an HONEST coordinator — enabling
            this trusts the coordinator with the whole token balance, even single-sig.
          </p>
          {policyForm.cross_chain_withdraw_enabled && (
            <p className="text-xs text-gray-400 ml-5 mt-1">
              Can require multisig approval: check &quot;Send cross-chain&quot; under &quot;Require
              approval for&quot; and set a threshold. The quote is re-fetched fresh at execution,
              so approval delays don&apos;t invalidate it.
            </p>
          )}
        </div>

        {/* sign_message recipients */}
        <div>
          <label className="block text-sm font-medium text-gray-700 mb-1">
            sign_message — allowed NEP-413 recipients
          </label>
          <input
            type="text"
            value={policyForm.sign_message_allowed_recipients}
            onChange={(e) => update({ sign_message_allowed_recipients: e.target.value })}
            placeholder="comma-separated, e.g. app.example.near (empty = sign_message denied)"
            className="w-full border border-gray-300 rounded px-3 py-2 text-sm"
          />
          <p className="text-xs text-red-600 mt-0.5">
            ⚠ Default-DENY allowlist. Only list non-fund-moving verifiers (e.g. a dApp
            login contract). NEVER list a fund-moving verifier like <code>intents.near</code> —
            a login signature could then be replayed to move funds.
          </p>
        </div>

        {/* evm_sign (default-DENY at the engine; this form opts in explicitly) */}
        <div className="mt-3 pt-3 border-t border-amber-200">
          <label className="flex items-center gap-1.5 text-sm cursor-pointer">
            <input
              type="checkbox"
              checked={policyForm.evm_sign_enabled}
              onChange={(e) => update({ evm_sign_enabled: e.target.checked })}
              className="rounded border-gray-300"
            />
            <span className="font-medium">evm_sign — sign on EVM chains (EIP-712 / EIP-191)</span>
          </label>
          <p className="text-xs text-gray-500 mt-0.5 ml-5">
            Checked = allow EVM signing (this form writes <code>evm_sign.allowed=true</code>
            explicitly). Like the other capabilities, a policy that omits it denies EVM signing;
            uncheck to write <code>allowed=false</code>.
          </p>
          <p className="text-xs text-red-600 mt-0.5 ml-5">
            ⚠ An EIP-712 signature is itself fund-moving (EIP-3009 ≈ transfer, EIP-2612 ≈
            approve), so this grants full authority over the EVM address&apos;s float. The risk
            is bounded to what you bridge onto that address — your NEAR-intents balance is never
            exposed to an EVM signature. Keep the on-chain float small.
          </p>
          {policyForm.evm_sign_enabled && (
            <label className="flex items-center gap-1.5 text-xs cursor-pointer ml-5 mt-1">
              <input
                type="checkbox"
                checked={policyForm.evm_sign_raw_tx}
                onChange={(e) => update({ evm_sign_raw_tx: e.target.checked })}
                className="rounded border-gray-300"
              />
              Also allow raw EVM transactions (evm_sign.raw_tx) — arbitrary contract calls /
              native value; default OFF
            </label>
          )}
        </div>
      </div>

      {/* Time Restrictions */}
      <div>
        <h3 className="text-sm font-semibold text-gray-800 mb-2">Time Restrictions (UTC)</h3>
        <div className="grid grid-cols-3 gap-4">
          <div>
            <label className="block text-xs font-medium text-gray-600 mb-1">Allowed Hours Start</label>
            <input
              type="number"
              min="0"
              max="23"
              value={policyForm.allowed_hours_start}
              onChange={(e) => update({ allowed_hours_start: e.target.value })}
              placeholder="9"
              className="w-full border border-gray-300 rounded px-3 py-2 text-sm"
            />
          </div>
          <div>
            <label className="block text-xs font-medium text-gray-600 mb-1">Allowed Hours End</label>
            <input
              type="number"
              min="0"
              max="24"
              value={policyForm.allowed_hours_end}
              onChange={(e) => update({ allowed_hours_end: e.target.value })}
              placeholder="17"
              className="w-full border border-gray-300 rounded px-3 py-2 text-sm"
            />
          </div>
          <div>
            <label className="block text-xs font-medium text-gray-600 mb-1">Allowed Days</label>
            <input
              type="text"
              value={policyForm.allowed_days}
              onChange={(e) => update({ allowed_days: e.target.value })}
              placeholder="1,2,3,4,5 (Mon-Fri)"
              className="w-full border border-gray-300 rounded px-3 py-2 text-sm"
            />
            <p className="text-xs text-gray-400 mt-1">1=Mon ... 7=Sun</p>
          </div>
        </div>
      </div>

      {/* Rate Limit & Webhook */}
      <div className="grid grid-cols-2 gap-4">
        <div>
          <label className="block text-sm font-medium text-gray-700 mb-1">Max Transactions per Hour</label>
          <input
            type="number"
            value={policyForm.max_per_hour}
            onChange={(e) => update({ max_per_hour: e.target.value })}
            placeholder="e.g. 10"
            className="w-full border border-gray-300 rounded px-3 py-2 text-sm"
          />
          <p className="text-xs text-gray-400 mt-1">Counts all operation types including intents deposit and swap.</p>
        </div>
        <div>
          <label className="block text-sm font-medium text-gray-700 mb-1">
            Webhook URL
            <button
              type="button"
              className="ml-1.5 inline-flex items-center justify-center w-4 h-4 rounded-full bg-gray-200 text-gray-500 text-[10px] font-bold hover:bg-gray-300 align-middle"
              onClick={() => setShowWebhookInfo((v) => !v)}
            >
              i
            </button>
            {showWebhookInfo && (
              <span className="block mt-1 text-xs font-normal text-gray-500">
                Receive POST notifications on transaction events (approval_needed, approval_received, request_completed).
                Must be HTTPS. Requests include HMAC-SHA256 signature for verification. Failed deliveries are retried up to 3 times.
              </span>
            )}
          </label>
          <input
            type="text"
            value={policyForm.webhook_url}
            onChange={(e) => update({ webhook_url: e.target.value })}
            placeholder="https://..."
            className="w-full border border-gray-300 rounded px-3 py-2 text-sm"
          />
        </div>
      </div>

      {/* Authorized API Keys */}
      <AuthorizedKeysSection
        additionalKeyHashes={policyForm.additional_key_hashes}
        onChangeHashes={(hashes) => update({ additional_key_hashes: hashes })}
        apiKeyHash={apiKeyHash}
        knownKeyHashes={knownKeyHashes}
        onSaveKey={onSaveKey}
      />
    </div>
  );
}
