'use client';

import { useState, useEffect, useCallback } from 'react';
import { ChaCha20Poly1305 } from '@stablelib/chacha20poly1305';
import { randomBytes } from '@stablelib/random';
import { AccessConditionBuilder } from './AccessConditionBuilder';
import { AccessCondition, FormData, SecretSourceType } from './types';
import { convertAccessToContractFormat } from './utils';
import { useNearWallet } from '@/contexts/NearWalletContext';

// Update mode for existing secrets - preserves PROTECTED_ secrets
type UpdateMode = 'append' | 'reset';

interface SecretsFormProps {
  isConnected: boolean;
  accountId: string | null;
  onSubmit: (formData: FormData, encryptedSecrets: number[]) => Promise<void>;
  coordinatorUrl: string;
  // For edit mode (replace all secrets - old behavior)
  initialData?: {
    sourceType?: SecretSourceType;
    repo: string;
    branch: string;
    wasmHash?: string;
    profile: string;
  };
  // For update mode (preserve PROTECTED_ secrets via signMessage)
  updateMode?: {
    accessor: {
      type: 'Repo' | 'WasmHash' | 'Project';
      repo?: string;
      branch?: string | null;
      hash?: string;
      project_id?: string;
    };
    profile: string;
  };
  onUpdateComplete?: () => void;
  onCancelUpdate?: () => void;
}

interface SecretToGenerate {
  id: string;
  name: string;
  generationType: string;
}

const GENERATION_TYPES = [
  { value: 'hex32', label: 'Hex 32 bytes (64 chars)' },
  { value: 'hex16', label: 'Hex 16 bytes (32 chars)' },
  { value: 'hex64', label: 'Hex 64 bytes (128 chars)' },
  { value: 'ed25519', label: 'ED25519 Private Key (NEAR format)' },
  { value: 'ed25519_seed', label: 'ED25519 Seed (32 bytes)' },
  { value: 'password', label: 'Password (32 chars)' },
  { value: 'password:16', label: 'Password (16 chars)' },
  { value: 'password:64', label: 'Password (64 chars)' },
  { value: 'password:128', label: 'Password (128 chars)' },
];

export function SecretsForm({
  isConnected,
  accountId,
  onSubmit,
  coordinatorUrl,
  initialData,
  updateMode,
  onUpdateComplete,
  onCancelUpdate,
}: SecretsFormProps) {
  const { signMessage } = useNearWallet();

  // Determine if we're in update mode (preserve PROTECTED_ secrets)
  const isUpdateMode = !!updateMode;

  const [sourceType, setSourceType] = useState<SecretSourceType>('repo');
  const [repo, setRepo] = useState('');
  const [branch, setBranch] = useState('');
  const [wasmHash, setWasmHash] = useState('');
  const [projectId, setProjectId] = useState('');
  const [userProjects, setUserProjects] = useState<{ project_id: string; name: string }[]>([]);
  const [loadingProjects, setLoadingProjects] = useState(false);
  const [profile, setProfile] = useState('default');
  const [plaintextSecrets, setPlaintextSecrets] = useState('{\n  "API_KEY": "your-api-key"\n}');
  const [accessCondition, setAccessCondition] = useState<AccessCondition>({ type: 'AllowAll' });
  const [encrypting, setEncrypting] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [secretsToGenerate, setSecretsToGenerate] = useState<SecretToGenerate[]>([]);
  const [generatedKeys, setGeneratedKeys] = useState<string[]>([]);

  // Pending update data (after keystore processed, waiting for contract store)
  const [pendingUpdate, setPendingUpdate] = useState<{
    encryptedArray: number[];
    formData: FormData;
    summary: { protected_preserved: number; updated: number; removed: number };
  } | null>(null);

  // Update mode specific state
  const [secretsUpdateMode, setSecretsUpdateMode] = useState<UpdateMode>('append');

  // Load initial data if provided (for edit mode)
  useEffect(() => {
    if (initialData) {
      setSourceType(initialData.sourceType || 'repo');
      setRepo(initialData.repo);
      setBranch(initialData.branch || '');
      setWasmHash(initialData.wasmHash || '');
      setProfile(initialData.profile);
      setPlaintextSecrets('{\n  "API_KEY": "your-new-api-key"\n}');
    }
  }, [initialData]);

  // Load update mode data
  useEffect(() => {
    if (updateMode) {
      if (updateMode.accessor.type === 'Repo') {
        setSourceType('repo');
        setRepo(updateMode.accessor.repo || '');
        setBranch(updateMode.accessor.branch || '');
      } else if (updateMode.accessor.type === 'WasmHash') {
        setSourceType('wasm_hash');
        setWasmHash(updateMode.accessor.hash || '');
      } else if (updateMode.accessor.type === 'Project') {
        setSourceType('project');
        setProjectId(updateMode.accessor.project_id || '');
      }
      setProfile(updateMode.profile);
      setPlaintextSecrets('{\n  "API_KEY": "your-new-api-key"\n}');
    }
  }, [updateMode]);

  // Get viewMethod and contractId from wallet context for loading projects
  const { viewMethod, contractId } = useNearWallet();

  // Load user's projects when source type is 'project'
  const loadProjects = useCallback(async () => {
    if (!accountId || sourceType !== 'project') return;

    setLoadingProjects(true);
    try {
      const result = await viewMethod({
        contractId,
        method: 'list_user_projects',
        args: { account_id: accountId },
      });
      setUserProjects(Array.isArray(result) ? result.map((p: { project_id: string; name: string }) => ({
        project_id: p.project_id,
        name: p.name,
      })) : []);
    } catch (err) {
      console.error('Failed to load projects:', err);
      setUserProjects([]);
    } finally {
      setLoadingProjects(false);
    }
  }, [accountId, sourceType, viewMethod, contractId]);

  useEffect(() => {
    if (sourceType === 'project' && accountId) {
      loadProjects();
    }
  }, [sourceType, accountId, loadProjects]);

  // Check if accessor or profile was changed in update mode (will create new secret instead of updating)
  const isAccessorChanged = (): boolean => {
    if (!updateMode) return false;

    // Profile change = new secret
    if (profile !== updateMode.profile) return true;

    if (updateMode.accessor.type === 'Repo') {
      const originalRepo = updateMode.accessor.repo || '';
      const originalBranch = updateMode.accessor.branch || '';
      return sourceType !== 'repo' || repo !== originalRepo || branch !== originalBranch;
    } else if (updateMode.accessor.type === 'WasmHash') {
      const originalHash = updateMode.accessor.hash || '';
      return sourceType !== 'wasm_hash' || wasmHash !== originalHash;
    } else if (updateMode.accessor.type === 'Project') {
      const originalProjectId = updateMode.accessor.project_id || '';
      return sourceType !== 'project' || projectId !== originalProjectId;
    }
    return false;
  };

  const accessorChanged = isUpdateMode && isAccessorChanged();

  const addSecretRow = () => {
    const newId = String(Date.now());
    setSecretsToGenerate([
      ...secretsToGenerate,
      { id: newId, name: '', generationType: 'hex32' }
    ]);
  };

  const removeSecretRow = (id: string) => {
    setSecretsToGenerate(secretsToGenerate.filter(s => s.id !== id));
  };

  const updateSecretRow = (id: string, field: 'name' | 'generationType', value: string) => {
    setSecretsToGenerate(
      secretsToGenerate.map(s =>
        s.id === id ? { ...s, [field]: value } : s
      )
    );
  };

  const handleEncryptAndSubmit = async () => {
    setError(null);
    setGeneratedKeys([]);

    if (!isConnected || !accountId) {
      setError('Please connect your wallet first');
      return;
    }

    // Validate based on source type
    if (sourceType === 'repo') {
      if (!repo.trim()) {
        setError('Repository is required');
        return;
      }
    } else if (sourceType === 'wasm_hash') {
      if (!wasmHash.trim()) {
        setError('WASM hash is required');
        return;
      }
      if (wasmHash.trim().length !== 64) {
        setError('WASM hash must be 64 hex characters (SHA256)');
        return;
      }
      if (!/^[a-fA-F0-9]{64}$/.test(wasmHash.trim())) {
        setError('WASM hash must be hex encoded');
        return;
      }
    } else if (sourceType === 'project') {
      if (!projectId.trim()) {
        setError('Project is required');
        return;
      }
    }

    if (!profile.trim()) {
      setError('Profile is required');
      return;
    }

    // Validate that at least one type of secrets is provided
    const hasManualSecrets = plaintextSecrets.trim() !== '' && plaintextSecrets.trim() !== '{\n  "API_KEY": "your-api-key"\n}';
    const validSecretsToGenerate = secretsToGenerate.filter(s => s.name.trim() !== '');
    const hasGeneratedSecrets = validSecretsToGenerate.length > 0;

    if (!hasManualSecrets && !hasGeneratedSecrets) {
      setError('Please provide either manual secrets or secrets to generate');
      return;
    }

    // Validate JSON format if manual secrets provided
    if (hasManualSecrets) {
      try {
        const parsed = JSON.parse(plaintextSecrets);
        if (typeof parsed !== 'object' || Array.isArray(parsed)) {
          throw new Error('Secrets must be a JSON object');
        }
      } catch (err) {
        setError(`Invalid JSON format: ${(err as Error).message}`);
        return;
      }
    }

    // Check for duplicate names in generated secrets
    if (hasGeneratedSecrets) {
      const names = validSecretsToGenerate.map(s => s.name.trim());
      const uniqueNames = new Set(names);
      if (names.length !== uniqueNames.size) {
        setError('Duplicate secret names are not allowed in generated secrets');
        return;
      }
    }

    setEncrypting(true);

    try {
      // If we have both manual and generated, OR only generated, use /secrets/add_generated_secret
      if (hasGeneratedSecrets) {
        // Step 1: Encrypt manual secrets if provided
        let encryptedSecretsBase64: string | null = null;

        if (hasManualSecrets) {
          // Build accessor based on source type
          const accessor = sourceType === 'wasm_hash'
            ? { type: 'WasmHash', hash: wasmHash.trim() }
            : sourceType === 'project'
            ? { type: 'Project', project_id: projectId.trim() }
            : { type: 'Repo', repo: repo.trim(), branch: branch.trim() || null };

          // Get public key and encrypt
          const pubkeyResp = await fetch(`${coordinatorUrl}/secrets/pubkey`, {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({
              accessor,
              owner: accountId,
              secrets_json: plaintextSecrets,
            }),
          });

          if (!pubkeyResp.ok) {
            const errorText = await pubkeyResp.text();
            try {
              const errorJson = JSON.parse(errorText);
              throw new Error(errorJson.error || errorText);
            } catch {
              throw new Error(errorText);
            }
          }

          const pubkeyData = await pubkeyResp.json();
          const pubkeyHex = pubkeyData.pubkey;

          // Encrypt with ChaCha20-Poly1305
          const keyMaterial = hexToBytes(pubkeyHex);
          const encoder = new TextEncoder();
          const plaintextBytes = encoder.encode(plaintextSecrets);
          const cipher = new ChaCha20Poly1305(keyMaterial);
          const nonce = randomBytes(12);
          const ciphertextWithTag = cipher.seal(nonce, plaintextBytes);
          const encrypted = new Uint8Array(12 + ciphertextWithTag.length);
          encrypted.set(nonce, 0);
          encrypted.set(ciphertextWithTag, 12);

          // Convert to base64
          encryptedSecretsBase64 = btoa(String.fromCharCode(...Array.from(encrypted)));
        }

        // Step 2: Call /secrets/add_generated_secret
        // Build accessor for generated secrets endpoint
        const generatedAccessor = sourceType === 'wasm_hash'
          ? { type: 'WasmHash', hash: wasmHash.trim() }
          : sourceType === 'project'
          ? { type: 'Project', project_id: projectId.trim() }
          : { type: 'Repo', repo: repo.trim(), branch: branch.trim() || null };

        const response = await fetch(`${coordinatorUrl}/secrets/add_generated_secret`, {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({
            accessor: generatedAccessor,
            owner: accountId,
            encrypted_secrets_base64: encryptedSecretsBase64,
            new_secrets: validSecretsToGenerate.map(s => ({
              name: s.name.trim(),
              generation_type: s.generationType,
            })),
          }),
        });

        if (!response.ok) {
          const errorText = await response.text();
          try {
            const errorJson = JSON.parse(errorText);
            throw new Error(errorJson.error || errorText);
          } catch {
            throw new Error(errorText);
          }
        }

        const data = await response.json();

        // Use all_keys from keystore response (authoritative source)
        const allKeys: string[] = data.all_keys || [];

        setGeneratedKeys(allKeys);

        console.log('🔑 ENCRYPTED SECRETS:', {
          total_keys: allKeys.length,
          all_keys: allKeys,
          encrypted_data_length: data.encrypted_data_base64.length,
        });

        // Extract normalized values from response accessor
        const repoNormalized = data.accessor?.type === 'Repo'
          ? data.accessor.repo_normalized
          : repo.trim();
        const wasmHashNormalized = data.accessor?.type === 'WasmHash'
          ? data.accessor.hash
          : wasmHash.trim();
        const projectIdNormalized = data.accessor?.type === 'Project'
          ? data.accessor.project_id
          : projectId.trim();

        // Convert base64 to array and submit to contract
        const encryptedArray = Array.from(atob(data.encrypted_data_base64), c => c.charCodeAt(0));
        const contractAccess = convertAccessToContractFormat(accessCondition);
        const formData: FormData = {
          sourceType,
          repo: repoNormalized,
          branch: branch.trim() || null,
          wasmHash: wasmHashNormalized,
          projectId: projectIdNormalized,
          profile: profile.trim(),
          access: contractAccess,
        };

        await onSubmit(formData, encryptedArray);

        // Clear form on success
        setSourceType('repo');
        setRepo('');
        setBranch('');
        setWasmHash('');
        setProjectId('');
        setProfile('default');
        setPlaintextSecrets('{\n  "API_KEY": "your-api-key"\n}');
        setAccessCondition({ type: 'AllowAll' });
        setSecretsToGenerate([]);
        setError(null);
      } else {
        // Only manual secrets - use original flow
        // Build accessor based on source type
        const accessor = sourceType === 'wasm_hash'
          ? { type: 'WasmHash', hash: wasmHash.trim() }
          : sourceType === 'project'
          ? { type: 'Project', project_id: projectId.trim() }
          : { type: 'Repo', repo: repo.trim(), branch: branch.trim() || null };

        const pubkeyResp = await fetch(`${coordinatorUrl}/secrets/pubkey`, {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({
            accessor,
            owner: accountId,
            secrets_json: plaintextSecrets,
          }),
        });

        if (!pubkeyResp.ok) {
          const errorText = await pubkeyResp.text();
          try {
            const errorJson = JSON.parse(errorText);
            throw new Error(errorJson.error || errorText);
          } catch {
            throw new Error(errorText);
          }
        }

        const pubkeyData = await pubkeyResp.json();
        const pubkeyHex = pubkeyData.pubkey;
        // Extract normalized values from response accessor
        const repoNormalized = pubkeyData.accessor?.repo_normalized || repo.trim();

        const keyMaterial = hexToBytes(pubkeyHex);
        const encoder = new TextEncoder();
        const plaintextBytes = encoder.encode(plaintextSecrets);
        const cipher = new ChaCha20Poly1305(keyMaterial);
        const nonce = randomBytes(12);
        const ciphertextWithTag = cipher.seal(nonce, plaintextBytes);
        const encrypted = new Uint8Array(12 + ciphertextWithTag.length);
        encrypted.set(nonce, 0);
        encrypted.set(ciphertextWithTag, 12);
        const encryptedArray = Array.from(encrypted);

        const contractAccess = convertAccessToContractFormat(accessCondition);
        const formData: FormData = {
          sourceType,
          repo: repoNormalized,
          branch: branch.trim() || null,
          wasmHash: wasmHash.trim(),
          projectId: projectId.trim(),
          profile: profile.trim(),
          access: contractAccess,
        };

        await onSubmit(formData, encryptedArray);

        // Clear form on success
        setSourceType('repo');
        setRepo('');
        setBranch('');
        setWasmHash('');
        setProjectId('');
        setProfile('default');
        setPlaintextSecrets('{\n  "API_KEY": "your-api-key"\n}');
        setAccessCondition({ type: 'AllowAll' });
        setError(null);
      }
    } catch (err) {
      console.error('Encryption error:', err);
      setError(`Failed to encrypt secrets: ${(err as Error).message}`);
    } finally {
      setEncrypting(false);
    }
  };

  // Handle update mode with signMessage (preserves PROTECTED_ secrets)
  const handleUpdateWithSignature = async () => {
    if (!updateMode || !accountId || !signMessage) {
      setError('Wallet not connected or update mode not active');
      return;
    }

    setError(null);
    setEncrypting(true);

    try {
      // Parse manual secrets
      const hasManualSecrets = plaintextSecrets.trim() !== '' &&
        plaintextSecrets.trim() !== '{\n  "API_KEY": "your-api-key"\n}' &&
        plaintextSecrets.trim() !== '{\n  "API_KEY": "your-new-api-key"\n}';

      let secretsObj: Record<string, string> = {};
      if (hasManualSecrets) {
        try {
          secretsObj = JSON.parse(plaintextSecrets);
          if (typeof secretsObj !== 'object' || Array.isArray(secretsObj)) {
            throw new Error('Secrets must be a JSON object');
          }
          // Check for PROTECTED_ prefix in user secrets
          for (const key of Object.keys(secretsObj)) {
            if (key.startsWith('PROTECTED_')) {
              throw new Error('Cannot manually add PROTECTED_ secrets. Use the generation section below.');
            }
          }
        } catch (err) {
          setError(`Invalid JSON format: ${(err as Error).message}`);
          setEncrypting(false);
          return;
        }
      }

      // Get generated secrets
      const validSecretsToGenerate = secretsToGenerate.filter(s => s.name.trim() !== '');

      // Allow empty secrets only if accessor changed (migration mode)
      // In migration mode, user may just want to move existing secrets to new accessor
      const accessorWasChanged = isAccessorChanged();
      if (Object.keys(secretsObj).length === 0 && validSecretsToGenerate.length === 0 && !accessorWasChanged) {
        setError('Please provide either manual secrets or secrets to generate (or change accessor to migrate existing secrets)');
        setEncrypting(false);
        return;
      }

      // 1. Generate nonce
      const nonceBytes = new Uint8Array(32);
      crypto.getRandomValues(nonceBytes);
      const nonce = btoa(String.fromCharCode(...nonceBytes));

      // 2. Create message to sign with secrets payload for verification
      // Backend will reconstruct this message from request data and verify signature
      const secretKeys = Object.keys(secretsObj).sort();
      const protectedNames = validSecretsToGenerate.map(s => s.name.trim()).sort();

      // Format: "Update Outlayer secrets for {owner}:{profile}\nkeys:{key1,key2}\nprotected:{PROTECTED_A,PROTECTED_B}"
      let messageToSign = `Update Outlayer secrets for ${accountId}:${profile}`;
      if (secretKeys.length > 0) {
        messageToSign += `\nkeys:${secretKeys.join(',')}`;
      }
      if (protectedNames.length > 0) {
        messageToSign += `\nprotected:${protectedNames.join(',')}`;
      }

      // 3. Sign with NEAR wallet (NEP-413)
      console.log('📝 Message to sign:', messageToSign);
      console.log('📝 Secret keys in message:', secretKeys);
      console.log('📝 Protected names in message:', protectedNames);

      const signed = await signMessage({
        message: messageToSign,
        recipient: 'keystore.outlayer.near',
        nonce: nonce,
      });

      if (!signed) {
        throw new Error('User cancelled signature');
      }

      // 4. Call keystore API via coordinator proxy
      // Build current accessor from form values
      const currentAccessor = sourceType === 'wasm_hash'
        ? { type: 'WasmHash', hash: wasmHash.trim() }
        : sourceType === 'project'
        ? { type: 'Project', project_id: projectId.trim() }
        : { type: 'Repo', repo: repo.trim(), branch: branch.trim() || null };

      // Build original accessor from updateMode (for decryption)
      const originalAccessor = updateMode.accessor.type === 'WasmHash'
        ? { type: 'WasmHash', hash: updateMode.accessor.hash || '' }
        : updateMode.accessor.type === 'Project'
        ? { type: 'Project', project_id: updateMode.accessor.project_id || '' }
        : { type: 'Repo', repo: updateMode.accessor.repo || '', branch: updateMode.accessor.branch || null };

      // Determine if this is a migration (accessor or profile changed)
      const isMigration = accessorChanged;

      const recipient = 'keystore.outlayer.near';
      const response = await fetch(`${coordinatorUrl}/secrets/update_user_secrets`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          // Always send original accessor for decryption
          accessor: originalAccessor,
          // If accessor changed, send new_accessor for encryption
          new_accessor: isMigration ? currentAccessor : undefined,
          profile: profile.trim(),
          owner: accountId,
          mode: secretsUpdateMode,
          secrets: secretsObj,
          generate_protected: validSecretsToGenerate.length > 0
            ? validSecretsToGenerate.map(s => ({
                name: s.name.trim(),
                generation_type: s.generationType,
              }))
            : undefined,
          signed_message: messageToSign,
          signature: signed.signature,
          public_key: signed.publicKey,
          nonce,
          recipient,
        }),
      });

      if (!response.ok) {
        const errorText = await response.text();
        console.error('Update secrets API error:', response.status, errorText);
        let errorMessage = `HTTP ${response.status}: ${response.statusText}`;
        try {
          const errorData = JSON.parse(errorText);
          errorMessage = errorData.error || errorData.message || errorMessage;
        } catch {
          if (errorText) errorMessage = errorText;
        }
        throw new Error(errorMessage);
      }

      const result = await response.json();

      // 5. Prepare data for contract storage (will be triggered by user click)
      const encryptedArray = Array.from(atob(result.encrypted_secrets_base64), c => c.charCodeAt(0));
      const contractAccess = convertAccessToContractFormat(accessCondition);

      // Use current form values (user may have changed accessor)
      const formData: FormData = {
        sourceType,
        repo: repo.trim(),
        branch: branch.trim() || null,
        wasmHash: wasmHash.trim(),
        projectId: projectId.trim(),
        profile: profile.trim(),
        access: contractAccess,
      };

      // Store pending update - user needs to click again to store in contract
      // This avoids popup blocker since the wallet popup will open on direct user action
      // Note: keystore returns arrays, convert to counts
      const summary = result.summary || {};
      setPendingUpdate({
        encryptedArray,
        formData,
        summary: {
          protected_preserved: Array.isArray(summary.protected_keys_preserved)
            ? summary.protected_keys_preserved.length
            : (summary.protected_preserved || 0),
          updated: Array.isArray(summary.updated_keys)
            ? summary.updated_keys.length
            : (summary.updated || 0),
          removed: Array.isArray(summary.removed_keys)
            ? summary.removed_keys.length
            : (summary.removed || 0),
        },
      });

    } catch (err) {
      console.error('Update error:', err);
      setError(`Failed to update secrets: ${(err as Error).message}`);
    } finally {
      setEncrypting(false);
    }
  };

  // Step 2: Store pending update in contract (triggered by user click)
  const handleStorePendingUpdate = async () => {
    if (!pendingUpdate) return;

    setEncrypting(true);
    setError(null);

    try {
      await onSubmit(pendingUpdate.formData, pendingUpdate.encryptedArray);

      // Clear pending, reset form, and call completion callback
      setPendingUpdate(null);
      setSecretsToGenerate([]);
      setPlaintextSecrets('{\n  "API_KEY": "your-api-key"\n}');
      if (onUpdateComplete) {
        onUpdateComplete();
      }
    } catch (err) {
      console.error('Store error:', err);
      setError(`Failed to store in contract: ${(err as Error).message}`);
    } finally {
      setEncrypting(false);
    }
  };

  const hexToBytes = (hex: string): Uint8Array => {
    const bytes = new Uint8Array(hex.length / 2);
    for (let i = 0; i < hex.length; i += 2) {
      bytes[i / 2] = parseInt(hex.substring(i, i + 2), 16);
    }
    return bytes;
  };

  return (
    <div className="bg-white shadow sm:rounded-lg">
      <div className="px-4 py-5 sm:p-6">
        {/* Header with cancel button for update mode */}
        <div className="flex justify-between items-center mb-4">
          <h2 className="text-lg font-medium text-gray-900">
            {isUpdateMode ? 'Update Secrets (Preserve PROTECTED_)' : initialData ? 'Replace Secrets' : 'Create New Secrets'}
          </h2>
          {isUpdateMode && onCancelUpdate && (
            <button
              onClick={onCancelUpdate}
              disabled={encrypting}
              className="text-sm text-gray-500 hover:text-gray-700"
            >
              Cancel
            </button>
          )}
        </div>

        {/* Update mode info banner */}
        {isUpdateMode && (
          <div className="mb-4 p-3 bg-purple-50 border border-purple-200 rounded-lg">
            <p className="text-sm text-purple-800">
              <strong>Update Mode:</strong> Your existing <code className="bg-purple-100 px-1 rounded">PROTECTED_*</code> entries will remain unchanged and cannot be modified.
              You can add or update user secrets (non-PROTECTED) and generate new <code className="bg-purple-100 px-1 rounded">PROTECTED_*</code> secrets.
            </p>
          </div>
        )}

        {/* Update mode selector (append/reset) - only in update mode */}
        {isUpdateMode && (
          <div className="mb-4">
            <label className="block text-sm font-medium text-gray-700 mb-2">
              Update Mode *
            </label>
            <select
              value={secretsUpdateMode}
              onChange={(e) => setSecretsUpdateMode(e.target.value as UpdateMode)}
              className="w-full px-3 py-2 border border-gray-300 rounded-md shadow-sm focus:outline-none focus:ring-blue-500 focus:border-blue-500"
              disabled={encrypting}
            >
              <option value="append">Append - Add or update secrets, keep existing user secrets</option>
              <option value="reset">Reset - Replace all user secrets (keeps PROTECTED_)</option>
            </select>
            {secretsUpdateMode === 'reset' && (
              <p className="mt-1 text-xs text-amber-600">
                ⚠️ This will remove all current user secrets except PROTECTED_ ones
              </p>
            )}
          </div>
        )}

        {/* Source Type Selector */}
        <div className="mb-4">
          <label className="block text-sm font-medium text-gray-700 mb-2">
            Secret Binding Type *
          </label>
          <div className="flex flex-wrap gap-4">
            <label className="inline-flex items-center">
              <input
                type="radio"
                value="repo"
                checked={sourceType === 'repo'}
                onChange={(e) => setSourceType(e.target.value as SecretSourceType)}
                className="form-radio h-4 w-4 text-blue-600"
                disabled={encrypting}
              />
              <span className="ml-2 text-sm text-gray-700">GitHub Repository</span>
            </label>
            <label className="inline-flex items-center">
              <input
                type="radio"
                value="wasm_hash"
                checked={sourceType === 'wasm_hash'}
                onChange={(e) => setSourceType(e.target.value as SecretSourceType)}
                className="form-radio h-4 w-4 text-blue-600"
                disabled={encrypting}
              />
              <span className="ml-2 text-sm text-gray-700">WASM Hash</span>
            </label>
            <label className="inline-flex items-center">
              <input
                type="radio"
                value="project"
                checked={sourceType === 'project'}
                onChange={(e) => setSourceType(e.target.value as SecretSourceType)}
                className="form-radio h-4 w-4 text-[#cc6600]"
                disabled={encrypting}
              />
              <span className="ml-2 text-sm text-gray-700">Project</span>
            </label>
          </div>
          <p className="mt-1 text-xs text-gray-500">
            {sourceType === 'repo'
              ? 'Bind secrets to a GitHub repository (for CodeSource::GitHub)'
              : sourceType === 'wasm_hash'
              ? 'Bind secrets to a WASM binary hash (for CodeSource::WasmUrl)'
              : 'Bind secrets to a project (shared across all versions)'}
          </p>
        </div>

        {/* Warning when accessor changed in update mode */}
        {accessorChanged && (
          <div className="mb-4 p-3 bg-amber-50 border border-amber-200 rounded-lg">
            <p className="text-sm text-amber-800">
              ⚠️ <strong>Migration mode!</strong> Accessor changed - secrets will be decrypted with old accessor and re-encrypted with new accessor.
              This will create a <strong>new</strong> secret entry. You can delete the old secret manually after saving.
            </p>
            <p className="text-xs text-amber-700 mt-1">
              💡 You can leave secrets empty to just migrate existing secrets to the new accessor.
            </p>
            <p className="text-xs text-amber-700 mt-1">
              ℹ️ Note: Migration only works between different repos/branches or WASM files. To copy secrets to a different profile, create a new secret manually.
            </p>
          </div>
        )}

        {/* Repository fields - shown when sourceType is 'repo' */}
        {sourceType === 'repo' && (
          <>
            <div className="mb-4">
              <label className="block text-sm font-medium text-gray-700 mb-2">
                GitHub Repository *
              </label>
              <input
                type="text"
                value={repo}
                onChange={(e) => setRepo(e.target.value)}
                placeholder="owner/repo or https://github.com/owner/repo"
                className="w-full px-3 py-2 border border-gray-300 rounded-md shadow-sm focus:outline-none focus:ring-blue-500 focus:border-blue-500"
                disabled={encrypting}
              />
              <p className="mt-1 text-xs text-gray-500">
                Examples: alice/project, https://github.com/alice/project
              </p>
            </div>

            <div className="mb-4">
              <label className="block text-sm font-medium text-gray-700 mb-2">
                Branch (optional)
              </label>
              <input
                type="text"
                value={branch}
                onChange={(e) => setBranch(e.target.value)}
                placeholder="main, develop, etc. (leave empty for all branches)"
                className="w-full px-3 py-2 border border-gray-300 rounded-md shadow-sm focus:outline-none focus:ring-blue-500 focus:border-blue-500"
                disabled={encrypting}
              />
            </div>
          </>
        )}

        {/* WASM Hash field - shown when sourceType is 'wasm_hash' */}
        {sourceType === 'wasm_hash' && (
          <div className="mb-4">
            <label className="block text-sm font-medium text-gray-700 mb-2">
              WASM SHA256 Hash *
            </label>
            <input
              type="text"
              value={wasmHash}
              onChange={(e) => setWasmHash(e.target.value.toLowerCase())}
              placeholder="64-character hex hash (e.g., a1b2c3d4...)"
              className="w-full px-3 py-2 border border-gray-300 rounded-md shadow-sm font-mono text-sm focus:outline-none focus:ring-blue-500 focus:border-blue-500"
              disabled={encrypting}
              maxLength={64}
            />
            <p className="mt-1 text-xs text-gray-500">
              The SHA256 hash of your compiled WASM binary (used with CodeSource::WasmUrl)
            </p>
          </div>
        )}

        {/* Project selector - shown when sourceType is 'project' */}
        {sourceType === 'project' && (
          <div className="mb-4">
            <label className="block text-sm font-medium text-gray-700 mb-2">
              Project *
            </label>
            {loadingProjects ? (
              <div className="flex items-center py-2">
                <svg className="animate-spin h-4 w-4 text-gray-400 mr-2" fill="none" viewBox="0 0 24 24">
                  <circle className="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" strokeWidth="4" />
                  <path className="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4z" />
                </svg>
                <span className="text-sm text-gray-500">Loading your projects...</span>
              </div>
            ) : userProjects.length === 0 ? (
              <div className="p-3 bg-yellow-50 border border-yellow-200 rounded-md">
                <p className="text-sm text-yellow-800">
                  You don&apos;t have any projects yet. <a href="/projects" className="text-[#cc6600] hover:underline">Create a project first</a>.
                </p>
              </div>
            ) : (
              <select
                value={projectId}
                onChange={(e) => setProjectId(e.target.value)}
                className="w-full px-3 py-2 border border-gray-300 rounded-md shadow-sm focus:outline-none focus:ring-[#cc6600] focus:border-[#cc6600]"
                disabled={encrypting}
              >
                <option value="">Select a project...</option>
                {userProjects.map((p) => (
                  <option key={p.project_id} value={p.project_id}>
                    {p.name} ({p.project_id})
                  </option>
                ))}
              </select>
            )}
            <p className="mt-1 text-xs text-gray-500">
              Secrets will be available to all versions of this project
            </p>
          </div>
        )}

        {/* Profile */}
        <div className="mb-4">
          <label className="block text-sm font-medium text-gray-700 mb-2">
            Profile *
          </label>
          <input
            type="text"
            value={profile}
            onChange={(e) => setProfile(e.target.value)}
            placeholder="default, prod, staging, etc."
            className="w-full px-3 py-2 border border-gray-300 rounded-md shadow-sm focus:outline-none focus:ring-blue-500 focus:border-blue-500"
            disabled={encrypting}
          />
          <p className="mt-1 text-xs text-gray-500">
            Profile name for organizing multiple secret sets per repo
          </p>
        </div>

        {/* Plaintext Secrets (JSON) */}
        <div className="mb-4">
          <label className="block text-sm font-medium text-gray-700 mb-2">
            Secrets (JSON format, optional)
          </label>
          <textarea
            value={plaintextSecrets}
            onChange={(e) => setPlaintextSecrets(e.target.value)}
            rows={8}
            className="w-full px-3 py-2 border border-gray-300 rounded-md shadow-sm font-mono text-sm focus:outline-none focus:ring-blue-500 focus:border-blue-500"
            placeholder='{\n  "OPENAI_KEY": "sk-...",\n  "DATABASE_URL": "postgres://..."\n}'
            disabled={encrypting}
          />
          <p className="mt-1 text-xs text-gray-500">
            JSON object with key-value pairs. Keys will be available as environment variables in WASM execution.
          </p>
          <p className="mt-1 text-xs text-amber-600">
            ⚠️ Cannot use <code className="bg-amber-100 px-1 rounded">PROTECTED_*</code> prefix (reserved for auto-generated secrets)
          </p>
        </div>

        {/* Secrets to Generate */}
        <div className="mb-4">
          <label className="block text-sm font-medium text-gray-700 mb-2">
            Secrets to Generate (optional)
          </label>
          <p className="text-xs text-gray-600 mb-2">
            Auto-generate cryptographically secure secrets without seeing their values. Perfect for MASTER_KEY, API tokens, passwords, and ED25519 keys.
          </p>
          <p className="text-xs text-green-600 mb-2">
            ✅ Must start with <code className="bg-green-100 px-1 rounded">PROTECTED_*</code> prefix (proves secret was generated in TEE)
          </p>

          {secretsToGenerate.length > 0 && (
            <div className="space-y-2 mb-2">
              {secretsToGenerate.map((secret) => (
                <div key={secret.id} className="flex gap-2">
                  <input
                    type="text"
                    value={secret.name}
                    onChange={(e) => updateSecretRow(secret.id, 'name', e.target.value)}
                    placeholder="e.g., PROTECTED_MASTER_KEY"
                    className="flex-1 px-3 py-2 border border-gray-300 rounded-md shadow-sm focus:outline-none focus:ring-blue-500 focus:border-blue-500"
                    disabled={encrypting}
                  />
                  <select
                    value={secret.generationType}
                    onChange={(e) => updateSecretRow(secret.id, 'generationType', e.target.value)}
                    className="w-64 px-3 py-2 border border-gray-300 rounded-md shadow-sm focus:outline-none focus:ring-blue-500 focus:border-blue-500"
                    disabled={encrypting}
                  >
                    {GENERATION_TYPES.map((type) => (
                      <option key={type.value} value={type.value}>
                        {type.label}
                      </option>
                    ))}
                  </select>
                  <button
                    onClick={() => removeSecretRow(secret.id)}
                    disabled={encrypting}
                    className="px-3 py-2 border border-red-300 text-red-700 rounded-md hover:bg-red-50 disabled:opacity-50"
                  >
                    ✕
                  </button>
                </div>
              ))}
            </div>
          )}

          <button
            onClick={addSecretRow}
            disabled={encrypting}
            className="px-3 py-1 text-sm border border-gray-300 text-gray-700 rounded-md hover:bg-gray-50 disabled:opacity-50"
          >
            + Add Generated Secret
          </button>
        </div>

        {/* All Encrypted Keys Display */}
        {generatedKeys.length > 0 && (
          <div className="mb-4 bg-green-50 border border-green-200 rounded-lg p-4">
            <h3 className="text-sm font-medium text-green-900 mb-2">
              ✅ Encrypted Keys ({generatedKeys.length} total)
            </h3>
            <ul className="text-xs text-green-800 space-y-1 list-disc list-inside">
              {generatedKeys.map((key) => (
                <li key={key}>{key}</li>
              ))}
            </ul>
            <p className="mt-2 text-xs text-green-700">
              All secrets are now encrypted and stored. You cannot retrieve their values later.
            </p>
          </div>
        )}

        {/* Access Condition */}
        <div className="mb-6">
          <label className="block text-sm font-medium text-gray-700 mb-2">
            Access Control *
          </label>
          <AccessConditionBuilder condition={accessCondition} onChange={setAccessCondition} />
        </div>

        {/* Error Display */}
        {error && (
          <div className="mb-4 bg-red-50 border border-red-200 rounded-lg p-4">
            <p className="text-sm text-red-800">{error}</p>
          </div>
        )}

        {/* Pending Update Banner - Step 2 */}
        {pendingUpdate && (
          <div className="mb-4 p-4 bg-green-50 border border-green-200 rounded-lg">
            <h4 className="text-sm font-semibold text-green-900 mb-2">
              ✅ Secrets encrypted by TEE keystore. Ready to store in contract.
            </h4>
            <p className="text-xs text-green-800 mb-3">
              PROTECTED_ preserved: {pendingUpdate.summary.protected_preserved} |
              Updated/added: {pendingUpdate.summary.updated} |
              Removed: {pendingUpdate.summary.removed}
            </p>
            <div className="flex space-x-3">
              <button
                onClick={handleStorePendingUpdate}
                disabled={encrypting}
                className="inline-flex items-center px-4 py-2 border border-transparent text-sm font-medium rounded-md shadow-sm text-white bg-green-600 hover:bg-green-700 focus:outline-none focus:ring-2 focus:ring-offset-2 focus:ring-green-500 disabled:bg-gray-300"
              >
                {encrypting ? '🔄 Storing...' : '🔐 Store Secrets'}
              </button>
              <button
                onClick={() => setPendingUpdate(null)}
                disabled={encrypting}
                className="inline-flex items-center px-4 py-2 border border-gray-300 text-sm font-medium rounded-md text-gray-700 bg-white hover:bg-gray-50 focus:outline-none focus:ring-2 focus:ring-offset-2 focus:ring-gray-500 disabled:bg-gray-100"
              >
                Cancel
              </button>
            </div>
          </div>
        )}

        {/* Submit Button */}
        {!pendingUpdate && (
          <div className="flex items-center justify-between">
            <button
              onClick={isUpdateMode ? handleUpdateWithSignature : handleEncryptAndSubmit}
              disabled={!isConnected || encrypting}
              className={`inline-flex items-center px-4 py-2 border border-transparent text-sm font-medium rounded-md shadow-sm text-white focus:outline-none focus:ring-2 focus:ring-offset-2 disabled:bg-gray-300 disabled:cursor-not-allowed ${
                isUpdateMode
                  ? 'bg-purple-600 hover:bg-purple-700 focus:ring-purple-500'
                  : 'bg-blue-600 hover:bg-blue-700 focus:ring-blue-500'
              }`}
            >
              {encrypting
                ? '🔄 Processing...'
                : isUpdateMode
                ? '✍️ Sign Message to Update'
                : initialData
                ? '💾 Replace Secrets'
                : '🔐 Encrypt & Store Secrets'}
            </button>

            {!isConnected && (
              <p className="text-sm text-red-600">
                Please connect your wallet to {isUpdateMode ? 'update' : 'create'} secrets
              </p>
            )}
          </div>
        )}

        {/* Step indicator for update mode */}
        {isUpdateMode && isConnected && !pendingUpdate && (
          <div className="mt-4 p-3 bg-purple-50 border border-purple-200 rounded-lg">
            <p className="text-xs text-purple-700 mb-2 font-medium">Update requires 2 steps:</p>
            <div className="flex items-center space-x-3">
              <div className="flex items-center space-x-2">
                <span className="flex items-center justify-center w-6 h-6 rounded-full bg-purple-600 text-white text-xs font-bold">1</span>
                <span className="text-sm font-medium text-purple-800">Sign message</span>
              </div>
              <span className="text-purple-400">→</span>
              <div className="flex items-center space-x-2">
                <span className="flex items-center justify-center w-6 h-6 rounded-full bg-gray-300 text-gray-600 text-xs font-bold">2</span>
                <span className="text-sm text-gray-500">Store in contract</span>
              </div>
            </div>
          </div>
        )}

        {/* Step 2 indicator when pending */}
        {pendingUpdate && (
          <div className="mt-4 p-3 bg-green-50 border border-green-200 rounded-lg">
            <p className="text-xs text-green-700 mb-2 font-medium">Step 2 of 2:</p>
            <div className="flex items-center space-x-3">
              <div className="flex items-center space-x-2">
                <span className="flex items-center justify-center w-6 h-6 rounded-full bg-green-500 text-white text-xs">✓</span>
                <span className="text-sm text-green-700">Signed</span>
              </div>
              <span className="text-green-400">→</span>
              <div className="flex items-center space-x-2">
                <span className="flex items-center justify-center w-6 h-6 rounded-full bg-green-600 text-white text-xs font-bold">2</span>
                <span className="text-sm font-medium text-green-800">Store in contract</span>
              </div>
            </div>
          </div>
        )}

        {/* Info Box */}
        <div className="mt-6 bg-blue-50 border border-blue-200 rounded-lg p-4">
          <h3 className="text-sm font-medium text-blue-900 mb-2">💡 How it works</h3>
          <ul className="text-xs text-blue-800 space-y-1 list-disc list-inside">
            <li><strong>Manual secrets</strong>: Encrypted client-side with ChaCha20-Poly1305 AEAD</li>
            <li><strong>Generated secrets</strong>: Keystore generates inside TEE (you never see values)</li>
            <li><strong>Combined flow</strong>: Manual secrets encrypted → sent with generation specs → keystore decrypts, generates, merges, re-encrypts</li>
            <li>Uses keystore&apos;s public key (coordinator never sees plaintext)</li>
            <li>Encrypted data is stored on NEAR contract</li>
            <li>Only verified TEE workers can decrypt secrets during execution</li>
            <li>You can provide manual secrets, generated secrets, or both</li>
            <li>Storage costs ~0.01 NEAR per secret set</li>
          </ul>
        </div>

      </div>
    </div>
  );
}
