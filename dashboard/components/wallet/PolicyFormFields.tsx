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

  return (
    <div className="space-y-4">
      {/* Spending Limits */}
      <div>
        <h3 className="text-sm font-semibold text-gray-800 mb-2">Spending Limits</h3>
        <p className="text-xs text-gray-400 mb-2">In minimal token units (e.g. yoctoNEAR). Leave empty for no limit.</p>
        <div className="grid grid-cols-2 gap-4">
          <div>
            <label className="block text-xs font-medium text-gray-600 mb-1">Per-Transaction</label>
            <input
              type="text"
              value={policyForm.per_transaction_limit}
              onChange={(e) => update({ per_transaction_limit: e.target.value })}
              placeholder="e.g. 100000000000000000000000"
              className="w-full border border-gray-300 rounded px-3 py-2 text-sm"
            />
          </div>
          <div>
            <label className="block text-xs font-medium text-gray-600 mb-1">Hourly</label>
            <input
              type="text"
              value={policyForm.hourly_limit}
              onChange={(e) => update({ hourly_limit: e.target.value })}
              placeholder="e.g. 500000000000000000000000"
              className="w-full border border-gray-300 rounded px-3 py-2 text-sm"
            />
          </div>
          <div>
            <label className="block text-xs font-medium text-gray-600 mb-1">Daily</label>
            <input
              type="text"
              value={policyForm.daily_limit}
              onChange={(e) => update({ daily_limit: e.target.value })}
              placeholder="e.g. 1000000000000000000000000"
              className="w-full border border-gray-300 rounded px-3 py-2 text-sm"
            />
          </div>
          <div>
            <label className="block text-xs font-medium text-gray-600 mb-1">Monthly</label>
            <input
              type="text"
              value={policyForm.monthly_limit}
              onChange={(e) => update({ monthly_limit: e.target.value })}
              placeholder="e.g. 10000000000000000000000000"
              className="w-full border border-gray-300 rounded px-3 py-2 text-sm"
            />
          </div>
        </div>
      </div>

      {/* Address Restrictions */}
      <div>
        <h3 className="text-sm font-semibold text-gray-800 mb-2">Address Restrictions</h3>
        <div className="grid grid-cols-3 gap-2 mb-2">
          {(['none', 'whitelist', 'blacklist'] as const).map((mode) => (
            <button
              key={mode}
              type="button"
              onClick={() => update({ address_mode: mode })}
              className={`px-3 py-1.5 text-xs rounded border ${
                policyForm.address_mode === mode
                  ? 'bg-[#cc6600] text-white border-[#cc6600]'
                  : 'bg-white text-gray-700 border-gray-300 hover:bg-gray-50'
              }`}
            >
              {mode === 'none' ? 'No restriction' : mode.charAt(0).toUpperCase() + mode.slice(1)}
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
          <div className="flex gap-4 mt-1">
            {['withdraw', 'call'].map((txType) => {
              const types = policyForm.transaction_types.split(',').map((t) => t.trim()).filter(Boolean);
              const checked = types.includes(txType);
              return (
                <label key={txType} className="flex items-center gap-1.5 text-sm cursor-pointer">
                  <input
                    type="checkbox"
                    checked={checked}
                    onChange={() => {
                      const next = checked ? types.filter((t) => t !== txType) : [...types, txType];
                      update({ transaction_types: next.join(',') });
                    }}
                    className="rounded border-gray-300"
                  />
                  {txType}
                </label>
              );
            })}
          </div>
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
        </div>
        <div>
          <label className="block text-sm font-medium text-gray-700 mb-1">Webhook URL</label>
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
