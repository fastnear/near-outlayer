import { useState, useEffect, useMemo } from 'react';
import { PolicyForm, DEFAULT_POLICY, buildPolicyRules } from '@/lib/wallet-policy';

export interface UsePolicyFormOptions {
  apiKeyHash?: string;
  /** Callback to add approval (or other fields) to the built policy. */
  augmentPolicy?: (base: Record<string, unknown>) => Record<string, unknown>;
}

export function usePolicyForm(options: UsePolicyFormOptions = {}) {
  const { apiKeyHash, augmentPolicy } = options;

  const [policyForm, setPolicyForm] = useState<PolicyForm>(DEFAULT_POLICY);
  const [policyJsonText, setPolicyJsonText] = useState('');
  const [jsonEdited, setJsonEdited] = useState(false);

  const builtPolicyJson = useMemo(() => {
    let policy = buildPolicyRules(policyForm, apiKeyHash);
    if (augmentPolicy) {
      policy = augmentPolicy(policy);
    }
    return policy;
  }, [policyForm, apiKeyHash, augmentPolicy]);

  useEffect(() => {
    if (!jsonEdited) {
      setPolicyJsonText(JSON.stringify(builtPolicyJson, null, 2));
    }
  }, [builtPolicyJson, jsonEdited]);

  const resetJson = () => {
    setJsonEdited(false);
    setPolicyJsonText(JSON.stringify(builtPolicyJson, null, 2));
  };

  return {
    policyForm,
    setPolicyForm,
    builtPolicyJson,
    policyJsonText,
    setPolicyJsonText,
    jsonEdited,
    setJsonEdited,
    resetJson,
  };
}
