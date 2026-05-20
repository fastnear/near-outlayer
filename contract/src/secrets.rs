use crate::*;
use crate::payment::SystemEvent;
use near_sdk::{env, require};
use near_sdk::json_types::U128;

/// Storage cost per byte in NEAR
pub const STORAGE_PRICE_PER_BYTE: Balance = 10_000_000_000_000_000_000; // 0.00001 NEAR per byte

#[near_bindgen]
impl Contract {
    /// Store secrets with access control.
    ///
    /// User must attach storage deposit to cover the cost of storing
    /// secrets. The deposit is refunded when secrets are deleted.
    ///
    /// # Arguments
    /// * `accessor` - What code can access these secrets (Repo /
    ///   WasmHash / Project / System)
    /// * `profile` - Profile name (e.g., "default", "premium", "staging")
    /// * `encrypted_secrets_base64` - Base64-encoded encrypted secrets
    /// * `access` - Access control rules
    /// * `vault_id` - Optional per-customer vault binding.
    ///   * `null` — secret was encrypted with the default OutLayer
    ///     master. **Existing vault bindings on the same `(accessor,
    ///     profile, owner)` key are LEFT UNTOUCHED** so a re-store
    ///     that just rotates the ciphertext does not silently break
    ///     decryption. To opt out of an existing binding call
    ///     [`Contract::unbind_secret_vault`] explicitly (typically
    ///     paired with re-encrypting under the default master).
    ///   * `"vault.alice.near"` — secret was encrypted with that
    ///     vault's master. The worker resolves the master through MPC
    ///     CKD using the vault account id as the signer.
    ///
    /// Storage deposit cost includes the binding entry when
    /// `vault_id = Some(_)`.
    ///
    /// **Off-chain callers MUST pass `vault_id` explicitly** (set to
    /// `null` for the default-master path). near-sdk's argument
    /// deserialiser rejects JSON that omits a required `Option` field.
    #[payable]
    pub fn store_secrets(
        &mut self,
        accessor: SecretAccessor,
        profile: String,
        encrypted_secrets_base64: String,
        access: types::AccessCondition,
        vault_id: Option<AccountId>,
    ) {
        let caller = env::predecessor_account_id();

        // Validate accessor
        match &accessor {
            SecretAccessor::Repo { repo, branch } => {
                assert!(!repo.is_empty(), "Repository cannot be empty");
                if let Some(ref b) = branch {
                    assert!(!b.is_empty(), "Branch name cannot be empty if provided");
                    assert!(b.len() <= 255, "Branch name too long (max 255 chars)");
                }
            }
            SecretAccessor::WasmHash { hash } => {
                assert!(!hash.is_empty(), "WASM hash cannot be empty");
                assert!(hash.len() == 64, "WASM hash must be 64 hex characters (SHA256)");
                assert!(
                    hash.chars().all(|c| c.is_ascii_hexdigit()),
                    "WASM hash must be hex encoded"
                );
            }
            SecretAccessor::Project { project_id } => {
                assert!(!project_id.is_empty(), "Project ID cannot be empty");
                assert!(project_id.contains('/'), "Project ID must be in format 'owner.near/name'");
                // Verify project exists
                assert!(
                    self.projects.get(project_id).is_some(),
                    "Project '{}' does not exist",
                    project_id
                );
            }
            SecretAccessor::System(secret_type) => {
                // System secrets (Payment Keys) - just validate the type exists
                match secret_type {
                    SystemSecretType::PaymentKey => {
                        // PaymentKey secrets are valid, no additional validation needed
                        // The profile is used as nonce (e.g., "0", "1", "2")
                    }
                }
            }
        }

        // Validate common inputs
        assert!(!profile.is_empty(), "Profile cannot be empty");
        assert!(profile.len() <= 64, "Profile name too long (max 64 chars)");
        assert!(
            profile.chars().all(|c| c.is_alphanumeric() || c == '-' || c == '_'),
            "Profile name must contain only alphanumeric, dash, or underscore"
        );
        assert!(
            !encrypted_secrets_base64.is_empty(),
            "Encrypted secrets cannot be empty"
        );

        // Create secret key
        let key = SecretKey {
            accessor: accessor.clone(),
            profile: profile.clone(),
            owner: caller.clone(),
        };

        // Calculate storage cost. The size MUST reflect the
        // post-call state of the side-table, not the delta — otherwise
        // an update from `Some(v) → None` (binding survives per the
        // B-3 invariant) would re-quote the size as if the binding
        // were gone, refund the binding overhead to the caller, and
        // leave the entry sitting on chain unfunded. Same trap on
        // `Some(v1) → Some(v2)` rebinds.
        let vault_bound_after_call =
            vault_id.is_some() || self.secret_vault_bindings.get(&key).is_some();
        let storage_usage = self.calculate_secret_storage_size(
            &key,
            &encrypted_secrets_base64,
            &access,
            vault_bound_after_call,
        );
        let required_deposit = storage_usage as u128 * STORAGE_PRICE_PER_BYTE;
        let attached_deposit = env::attached_deposit().as_yoctonear();

        // Check if updating existing secrets
        let is_new = self.secrets_storage.get(&key).is_none();

        if let Some(existing) = self.secrets_storage.get(&key) {
            // Updating existing: combine attached + old deposit, require only new cost
            let total_available = attached_deposit + existing.storage_deposit;

            assert!(
                total_available >= required_deposit,
                "Insufficient deposit for update. Required: {} yoctoNEAR, available (attached {} + old {}): {} yoctoNEAR",
                required_deposit,
                attached_deposit,
                existing.storage_deposit,
                total_available
            );

            // Refund excess
            let refund = total_available - required_deposit;
            if refund > 0 {
                near_sdk::Promise::new(caller.clone()).transfer(NearToken::from_yoctonear(refund));
                log!(
                    "Updating secrets: accessor={:?}, profile={}, old_deposit={}, attached={}, new_required={}, refund={}",
                    accessor, profile,
                    existing.storage_deposit,
                    attached_deposit,
                    required_deposit,
                    refund
                );
            }
        } else {
            // Check attached deposit
            assert!(
                attached_deposit >= required_deposit,
                "Insufficient storage deposit. Required: {} yoctoNEAR, attached: {} yoctoNEAR",
                required_deposit,
                attached_deposit
            );

            // Refund excess if any
            if attached_deposit > required_deposit {
                let refund = attached_deposit - required_deposit;
                near_sdk::Promise::new(caller.clone()).transfer(NearToken::from_yoctonear(refund));
            }
        }

        // Store secret profile
        let profile_data = SecretProfile {
            encrypted_secrets: encrypted_secrets_base64,
            access,
            created_at: env::block_timestamp(),
            updated_at: env::block_timestamp(),
            storage_deposit: required_deposit,
        };

        self.secrets_storage.insert(&key, &profile_data);

        // Side-table for the optional vault binding.
        //
        // Semantics:
        //   * `Some(v)` → set/overwrite the binding to vault `v`.
        //   * `None`    → DO NOT TOUCH the side-table. An existing
        //     binding survives the update; that is the back-compat
        //     contract for legacy callers that never pass `vault_id`.
        //     If a customer wants to opt out of an existing binding
        //     they must call `unbind_secret_vault(...)` explicitly,
        //     usually paired with re-encrypting the ciphertext under
        //     the default master.
        if let Some(v) = vault_id {
            self.secret_vault_bindings.insert(&key, &v);
        }

        // Add to user index if new
        if is_new {
            let mut user_secrets = self
                .user_secrets_index
                .get(&caller)
                .unwrap_or_else(|| UnorderedSet::new(StorageKey::UserSecretsList { account_id: caller.clone() }));

            user_secrets.insert(&key);
            self.user_secrets_index.insert(&caller, &user_secrets);
        }

        log!(
            "Secrets stored: accessor={:?}, profile={}, owner={}, deposit={} yoctoNEAR",
            accessor,
            profile,
            caller,
            required_deposit
        );

        // Emit TopUp event with amount=0 for PaymentKey creation
        // Worker will create payment_keys record with initial_balance=0
        // Key cannot be used until real TopUp or admin grant
        if let SecretAccessor::System(SystemSecretType::PaymentKey) = &accessor {
            let nonce: u32 = profile.parse()
                .expect("PaymentKey profile must be a valid u32 nonce");
            self.emit_system_event(SystemEvent::TopUpPaymentKey {
                data_id: [0u8; 32], // No yield promise - dummy data_id
                owner: caller.clone(),
                nonce,
                amount: U128(0),
                encrypted_data: profile_data.encrypted_secrets.clone(),
            });
            log!(
                "PaymentKey created event emitted: owner={}, nonce={}",
                caller,
                nonce
            );
        }
    }

    /// Delete secrets and refund storage deposit
    ///
    /// # Arguments
    /// * `accessor` - What code can access these secrets (Repo or WasmHash)
    /// * `profile` - Profile name
    pub fn delete_secrets(
        &mut self,
        accessor: SecretAccessor,
        profile: String,
    ) {
        let caller = env::predecessor_account_id();

        let key = SecretKey {
            accessor: accessor.clone(),
            profile: profile.clone(),
            owner: caller.clone(),
        };

        self.delete_secrets_internal(key, &caller);

        log!(
            "Secrets deleted: accessor={:?}, profile={}, owner={}",
            accessor,
            profile,
            caller
        );
    }

    /// Internal method to delete secrets by key
    /// pub(crate) to allow access from payment.rs for delete_payment_key
    pub(crate) fn delete_secrets_internal(&mut self, key: SecretKey, caller: &AccountId) {
        let profile_data = self.secrets_storage.get(&key)
            .expect("Secrets not found");

        // Remove from storage
        self.secrets_storage.remove(&key);

        // Drop the vault binding alongside the secret. Idempotent
        // — `remove()` on a missing key is a no-op.
        self.secret_vault_bindings.remove(&key);

        // Remove from user index
        if let Some(mut user_secrets) = self.user_secrets_index.get(caller) {
            user_secrets.remove(&key);
            if user_secrets.is_empty() {
                // Remove empty set
                self.user_secrets_index.remove(caller);
            } else {
                self.user_secrets_index.insert(caller, &user_secrets);
            }
        }

        // Refund storage deposit
        if profile_data.storage_deposit > 0 {
            near_sdk::Promise::new(caller.clone())
                .transfer(NearToken::from_yoctonear(profile_data.storage_deposit));
            log!("Refunded {} yoctoNEAR", profile_data.storage_deposit);
        }
    }

    /// Drop the vault binding for an existing secret without touching
    /// the ciphertext.
    ///
    /// Use this when re-encrypting a secret under the default OutLayer
    /// master after it had previously been bound to a vault. Calling
    /// `store_secrets(..., vault_id: None)` does NOT clear an existing
    /// binding (that's the back-compat invariant for legacy callers);
    /// this method is the explicit opt-out.
    ///
    /// Idempotent: succeeds silently if no binding exists.
    pub fn unbind_secret_vault(&mut self, accessor: SecretAccessor, profile: String) {
        let caller = env::predecessor_account_id();
        let key = SecretKey {
            accessor,
            profile,
            owner: caller,
        };
        require!(
            self.secrets_storage.get(&key).is_some(),
            "secret not found or not owned by caller"
        );
        self.secret_vault_bindings.remove(&key);
    }

    /// Update access control rules for existing secrets
    ///
    /// # Arguments
    /// * `accessor` - What code can access these secrets (Repo or WasmHash)
    /// * `profile` - Profile name
    /// * `new_access` - New access control rules
    pub fn update_access(
        &mut self,
        accessor: SecretAccessor,
        profile: String,
        new_access: types::AccessCondition,
    ) {
        let caller = env::predecessor_account_id();

        let key = SecretKey {
            accessor: accessor.clone(),
            profile: profile.clone(),
            owner: caller.clone(),
        };

        let mut profile_data = self.secrets_storage.get(&key)
            .expect("Secrets not found");

        // Update access rules and timestamp
        profile_data.access = new_access;
        profile_data.updated_at = env::block_timestamp();

        self.secrets_storage.insert(&key, &profile_data);

        log!(
            "Access control updated: accessor={:?}, profile={}",
            accessor,
            profile
        );
    }

    /// Calculate storage size for secrets (in bytes).
    ///
    /// `vault_bound` toggles the side-table contribution: when true,
    /// the cost of a `secret_vault_bindings` entry (a duplicate of the
    /// SecretKey plus an AccountId value) is added to the result.
    fn calculate_secret_storage_size(
        &self,
        key: &SecretKey,
        encrypted_secrets: &str,
        access: &types::AccessCondition,
        vault_bound: bool,
    ) -> u64 {
        // Storage calculation:
        // - SecretKey: key_type (enum) + profile + owner (Borsh serialized)
        // - SecretProfile: encrypted_secrets + access + timestamps + deposit (Borsh serialized)
        // - User index entry: UnorderedSet overhead (for new entries)
        // - Base overhead: LookupMap entry overhead

        const BASE_STORAGE_OVERHEAD: u64 = 40; // LookupMap entry overhead (key hash + pointer)
        const INDEX_ENTRY_OVERHEAD: u64 = 64; // UnorderedSet entry overhead

        // Accessor size (Borsh serialization adds enum discriminant + data)
        let accessor_size = match &key.accessor {
            SecretAccessor::Repo { repo, branch } => {
                1 + // enum discriminant
                4 + repo.len() + // String with u32 length prefix
                1 + branch.as_ref().map(|b| 4 + b.len()).unwrap_or(0) // Option<String>
            }
            SecretAccessor::WasmHash { hash } => {
                1 + // enum discriminant
                4 + hash.len() // String with u32 length prefix
            }
            SecretAccessor::Project { project_id } => {
                1 + // enum discriminant
                4 + project_id.len() // String with u32 length prefix
            }
            SecretAccessor::System(_secret_type) => {
                1 + // enum discriminant for System
                1   // enum discriminant for SystemSecretType (PaymentKey = 0)
            }
        };

        // Key size
        let key_size = (accessor_size
            + 4 + key.profile.len() // String with u32 length prefix
            + 4 + key.owner.as_str().len()) as u64; // AccountId (String with u32 length prefix)

        // Value size
        let encrypted_size = (4 + encrypted_secrets.len()) as u64; // String with u32 length prefix

        // AccessCondition size (serialize to estimate actual size)
        let access_json = serde_json::to_string(access).unwrap_or_default();
        let access_size = (access_json.len() + 10) as u64; // JSON + Borsh overhead

        let timestamps_and_deposit_size = 8 + 8 + 16; // created_at + updated_at + storage_deposit

        let value_size = encrypted_size + access_size + timestamps_and_deposit_size;

        // Add overhead for user index entry (only for new entries)
        let index_overhead = if self.secrets_storage.get(key).is_none() {
            INDEX_ENTRY_OVERHEAD
        } else {
            0 // Updating existing entry, no new index entry
        };

        // Side-table entry for the optional vault binding. The
        // `secret_vault_bindings: LookupMap<SecretKey, AccountId>` map
        // re-stores the full SecretKey as its key plus an AccountId
        // value, so the contribution mirrors `key_size` plus a small
        // AccountId payload. Plus its own LookupMap entry overhead.
        let binding_overhead = if vault_bound {
            BASE_STORAGE_OVERHEAD + key_size + 4 + 64 // AccountId max ~64 bytes (String length-prefixed)
        } else {
            0
        };

        BASE_STORAGE_OVERHEAD + key_size + value_size + index_overhead + binding_overhead
    }
}

// View methods
#[near_bindgen]
impl Contract {
    /// Estimate storage cost for secrets (before storing).
    ///
    /// Returns cost in yoctoNEAR. Call this before `store_secrets` to
    /// know the exact deposit amount required.
    ///
    /// # Arguments
    /// * `accessor` - What code can access these secrets
    /// * `profile` - Profile name
    /// * `owner` - Account that will own the secrets
    /// * `encrypted_secrets_base64` - Base64-encoded encrypted secrets
    /// * `access` - Access control rules
    /// * `vault_id` - Match the value the caller will pass to
    ///   `store_secrets`. `Some(_)` includes the side-table binding
    ///   entry in the cost; `None` excludes it. Off-chain callers MUST
    ///   pass this explicitly (no missing-field-as-default).
    pub fn estimate_storage_cost(
        &self,
        accessor: SecretAccessor,
        profile: String,
        owner: AccountId,
        encrypted_secrets_base64: String,
        access: types::AccessCondition,
        vault_id: Option<AccountId>,
    ) -> U128 {
        let key = SecretKey {
            accessor,
            profile,
            owner,
        };
        // Match the post-call state of the side-table, not the delta.
        // See `internal_store_secrets` for the rationale — the same
        // bug would surface here as inaccurate quotes for updates
        // that keep an existing binding implicitly.
        let vault_bound_after_call =
            vault_id.is_some() || self.secret_vault_bindings.get(&key).is_some();
        let storage_bytes = self.calculate_secret_storage_size(
            &key,
            &encrypted_secrets_base64,
            &access,
            vault_bound_after_call,
        );
        U128((storage_bytes as u128) * STORAGE_PRICE_PER_BYTE)
    }

    /// Get secrets (for keystore worker to read)
    ///
    /// For Repo accessor: if querying with a specific branch returns None,
    /// automatically tries with branch=null to find wildcard secrets.
    ///
    /// # Arguments
    /// * `accessor` - What code can access these secrets (Repo or WasmHash)
    /// * `profile` - Profile name
    /// * `owner` - Account that owns the secrets
    pub fn get_secrets(
        &self,
        accessor: SecretAccessor,
        profile: String,
        owner: AccountId,
    ) -> Option<SecretProfileView> {
        let key = SecretKey {
            accessor: accessor.clone(),
            profile: profile.clone(),
            owner: owner.clone(),
        };

        // Try with exact accessor first
        if let Some(profile_data) = self.secrets_storage.get(&key) {
            return Some(SecretProfileView {
                encrypted_secrets: profile_data.encrypted_secrets,
                access: profile_data.access,
                created_at: profile_data.created_at,
                updated_at: profile_data.updated_at,
                storage_deposit: U128(profile_data.storage_deposit),
                accessor: key.accessor,
            });
        }

        // For Repo with branch, try wildcard (branch=null)
        if let SecretAccessor::Repo { repo, branch: Some(_) } = accessor {
            let wildcard_key = SecretKey {
                accessor: SecretAccessor::Repo {
                    repo,
                    branch: None,
                },
                profile,
                owner,
            };
            if let Some(profile_data) = self.secrets_storage.get(&wildcard_key) {
                return Some(SecretProfileView {
                    encrypted_secrets: profile_data.encrypted_secrets,
                    access: profile_data.access,
                    created_at: profile_data.created_at,
                    updated_at: profile_data.updated_at,
                    storage_deposit: U128(profile_data.storage_deposit),
                    accessor: wildcard_key.accessor,
                });
            }
        }

        None
    }

    /// List all profile names for a repository owned by caller
    pub fn list_profiles(
        &self,
        _repo: String,
        _branch: Option<String>,
    ) -> Vec<String> {
        // Note: This is inefficient and should be optimized with indexing in production
        // For MVP, we accept O(n) iteration
        let profiles = Vec::new();

        // We can't iterate LookupMap directly, so this would require maintaining
        // a separate index. For now, return empty vector with a note.
        log!("WARNING: list_profiles requires indexing implementation");

        profiles
    }

    /// View — return the vault binding for a given secret, if any.
    ///
    /// A `Some(v)` result means the secret was
    /// encrypted with vault `v`'s master and the keystore-worker MUST
    /// resolve `v`'s per-vault master to decrypt it. `None` means the
    /// secret was encrypted with the default OutLayer master (legacy
    /// path).
    ///
    /// Off-chain consumers (keystore-worker, dashboards) typically want
    /// both `SecretProfile` and the binding in one shot; use
    /// [`Contract::get_secret_with_vault`] to save a round-trip.
    pub fn get_secret_vault(
        &self,
        accessor: SecretAccessor,
        profile: String,
        owner: AccountId,
    ) -> Option<AccountId> {
        let key = SecretKey {
            accessor,
            profile,
            owner,
        };
        self.secret_vault_bindings.get(&key)
    }

    /// View — combined secret profile + vault binding lookup. Saves an
    /// RPC round-trip for the keystore-worker, which always needs both
    /// fields together to decide which master to use for decryption.
    ///
    /// `profile.is_none()` and `vault_id.is_none()` are independent —
    /// a missing secret returns `profile = None`, while
    /// `vault_id = None` simply means "default master".
    pub fn get_secret_with_vault(
        &self,
        accessor: SecretAccessor,
        profile: String,
        owner: AccountId,
    ) -> SecretWithVault {
        let key = SecretKey {
            accessor: accessor.clone(),
            profile: profile.clone(),
            owner: owner.clone(),
        };

        // Reuse get_secrets' wildcard-fallback behaviour for Repo entries
        // by calling it directly. The vault binding is keyed against the
        // exact-match SecretKey first, falling back to the wildcard if
        // that's where the actual secret lives.
        let secret = self.get_secrets(accessor, profile, owner);

        let vault_id = if let Some(ref view) = secret {
            // get_secrets may have returned a wildcard match. Re-key the
            // binding lookup using whatever accessor it actually
            // resolved to.
            let resolved_key = SecretKey {
                accessor: view.accessor.clone(),
                profile: key.profile.clone(),
                owner: key.owner.clone(),
            };
            self.secret_vault_bindings.get(&resolved_key)
        } else {
            None
        };

        SecretWithVault {
            profile: secret,
            vault_id,
        }
    }

    /// Check if secrets exist for a given key
    ///
    /// # Arguments
    /// * `accessor` - What code can access these secrets (Repo or WasmHash)
    /// * `profile` - Profile name
    /// * `owner` - Account that owns the secrets
    pub fn secrets_exist(
        &self,
        accessor: SecretAccessor,
        profile: String,
        owner: AccountId,
    ) -> bool {
        let key = SecretKey {
            accessor,
            profile,
            owner,
        };

        self.secrets_storage.get(&key).is_some()
    }

    /// List all secrets for a user
    ///
    /// Returns array of secret metadata with accessor, profile info
    pub fn list_user_secrets(&self, account_id: AccountId) -> Vec<UserSecretInfo> {
        let user_secrets = self.user_secrets_index.get(&account_id);

        match user_secrets {
            Some(secrets_set) => {
                secrets_set
                    .iter()
                    .filter_map(|key| {
                        self.secrets_storage.get(&key).map(|profile| UserSecretInfo {
                            accessor: key.accessor.clone(),
                            profile: key.profile.clone(),
                            created_at: profile.created_at,
                            updated_at: profile.updated_at,
                            storage_deposit: U128(profile.storage_deposit),
                            access: profile.access,
                        })
                    })
                    .collect()
            }
            None => vec![],
        }
    }
}

/// Combined response for [`Contract::get_secret_with_vault`]. Returning
/// both fields in one structure lets the keystore-worker make a single
/// RPC call to learn (a) whether a secret exists and what its profile
/// looks like, and (b) which master should be used to decrypt it.
#[derive(Clone, Debug)]
#[near(serializers = [json])]
pub struct SecretWithVault {
    pub profile: Option<SecretProfileView>,
    pub vault_id: Option<AccountId>,
}

/// Project secrets storage info
#[derive(Clone, Debug)]
#[near(serializers = [json])]
pub struct ProjectSecretsStorage {
    pub project_id: String,
    pub owner: AccountId,
    pub total_bytes: u64,
    pub profiles_count: u32,
}

/// User secret metadata for list view
#[derive(Clone, Debug)]
#[near(serializers = [json])]
pub struct UserSecretInfo {
    pub accessor: SecretAccessor,
    pub profile: String,
    pub created_at: u64,
    pub updated_at: u64,
    pub storage_deposit: U128,
    pub access: types::AccessCondition,
}

#[cfg(test)]
mod tests {
    use super::*;
    use near_sdk::test_utils::{accounts, VMContextBuilder};
    use near_sdk::testing_env;

    fn get_context(predecessor: AccountId, attached_deposit: NearToken) -> VMContextBuilder {
        let mut builder = VMContextBuilder::new();
        builder
            .predecessor_account_id(predecessor)
            .attached_deposit(attached_deposit);
        builder
    }

    #[test]
    fn test_store_secrets_repo() {
        let owner = accounts(0);
        let operator = accounts(1);
        let user = accounts(2);

        let context = get_context(owner.clone(), NearToken::from_near(0));
        testing_env!(context.build());

        let mut contract = Contract::new(owner.clone(), Some(operator), None, None);

        // Store secrets with sufficient deposit
        let context = get_context(user.clone(), NearToken::from_near(1));
        testing_env!(context.build());

        contract.store_secrets(
            SecretAccessor::Repo {
                repo: "github.com/alice/project".to_string(),
                branch: None,
            },
            "default".to_string(),
            "base64encodeddata".to_string(),
            types::AccessCondition::AllowAll,
            None,
        );

        // Verify secrets exist
        assert!(contract.secrets_exist(
            SecretAccessor::Repo {
                repo: "github.com/alice/project".to_string(),
                branch: None,
            },
            "default".to_string(),
            user.clone(),
        ));
    }

    #[test]
    fn test_store_secrets_wasm_hash() {
        let owner = accounts(0);
        let user = accounts(2);

        let context = get_context(owner.clone(), NearToken::from_near(0));
        testing_env!(context.build());

        let mut contract = Contract::new(owner.clone(), None, None, None);

        // Store secrets by wasm hash
        let context = get_context(user.clone(), NearToken::from_near(1));
        testing_env!(context.build());

        let wasm_hash = "a".repeat(64); // Valid SHA256 hex hash
        contract.store_secrets(
            SecretAccessor::WasmHash {
                hash: wasm_hash.clone(),
            },
            "default".to_string(),
            "base64encodeddata".to_string(),
            types::AccessCondition::AllowAll,
            None,
        );

        // Verify secrets exist
        assert!(contract.secrets_exist(
            SecretAccessor::WasmHash {
                hash: wasm_hash.clone(),
            },
            "default".to_string(),
            user.clone(),
        ));

        // Verify can retrieve
        let secrets = contract.get_secrets(
            SecretAccessor::WasmHash {
                hash: wasm_hash,
            },
            "default".to_string(),
            user.clone(),
        );
        assert!(secrets.is_some());
    }

    #[test]
    #[should_panic(expected = "WASM hash must be 64 hex characters")]
    fn test_invalid_wasm_hash_length() {
        let owner = accounts(0);
        let user = accounts(2);

        let context = get_context(owner.clone(), NearToken::from_near(0));
        testing_env!(context.build());

        let mut contract = Contract::new(owner.clone(), None, None, None);

        let context = get_context(user.clone(), NearToken::from_near(1));
        testing_env!(context.build());

        contract.store_secrets(
            SecretAccessor::WasmHash {
                hash: "tooshort".to_string(), // Invalid length
            },
            "default".to_string(),
            "base64encodeddata".to_string(),
            types::AccessCondition::AllowAll,
            None,
        );
    }

    #[test]
    #[should_panic(expected = "Profile name must contain only alphanumeric")]
    fn test_invalid_profile_name() {
        let owner = accounts(0);
        let user = accounts(2);

        let context = get_context(owner.clone(), NearToken::from_near(0));
        testing_env!(context.build());

        let mut contract = Contract::new(owner.clone(), None, None, None);

        let context = get_context(user.clone(), NearToken::from_near(1));
        testing_env!(context.build());

        contract.store_secrets(
            SecretAccessor::Repo {
                repo: "github.com/alice/project".to_string(),
                branch: None,
            },
            "invalid profile!".to_string(), // Invalid characters
            "base64encodeddata".to_string(),
            types::AccessCondition::AllowAll,
            None,
        );
    }

    #[test]
    fn test_delete_secrets_repo() {
        let owner = accounts(0);
        let user = accounts(2);

        let context = get_context(owner.clone(), NearToken::from_near(0));
        testing_env!(context.build());

        let mut contract = Contract::new(owner.clone(), None, None, None);

        // Store secrets
        let context = get_context(user.clone(), NearToken::from_near(1));
        testing_env!(context.build());

        contract.store_secrets(
            SecretAccessor::Repo {
                repo: "github.com/alice/project".to_string(),
                branch: None,
            },
            "default".to_string(),
            "base64encodeddata".to_string(),
            types::AccessCondition::AllowAll,
            None,
        );

        // Delete secrets
        contract.delete_secrets(
            SecretAccessor::Repo {
                repo: "github.com/alice/project".to_string(),
                branch: None,
            },
            "default".to_string(),
        );

        // Verify secrets don't exist
        assert!(!contract.secrets_exist(
            SecretAccessor::Repo {
                repo: "github.com/alice/project".to_string(),
                branch: None,
            },
            "default".to_string(),
            user.clone(),
        ));
    }

    #[test]
    fn test_delete_secrets_wasm_hash() {
        let owner = accounts(0);
        let user = accounts(2);

        let context = get_context(owner.clone(), NearToken::from_near(0));
        testing_env!(context.build());

        let mut contract = Contract::new(owner.clone(), None, None, None);

        // Store secrets by wasm hash
        let context = get_context(user.clone(), NearToken::from_near(1));
        testing_env!(context.build());

        let wasm_hash = "b".repeat(64);
        contract.store_secrets(
            SecretAccessor::WasmHash {
                hash: wasm_hash.clone(),
            },
            "default".to_string(),
            "base64encodeddata".to_string(),
            types::AccessCondition::AllowAll,
            None,
        );

        // Delete secrets
        contract.delete_secrets(
            SecretAccessor::WasmHash {
                hash: wasm_hash.clone(),
            },
            "default".to_string(),
        );

        // Verify secrets don't exist
        assert!(!contract.secrets_exist(
            SecretAccessor::WasmHash {
                hash: wasm_hash,
            },
            "default".to_string(),
            user.clone(),
        ));
    }

    // ===== Per-customer vault binding =====

    #[test]
    fn store_secrets_with_vault_id_records_binding() {
        let owner = accounts(0);
        let user = accounts(2);

        testing_env!(get_context(owner.clone(), NearToken::from_near(0)).build());
        let mut contract = Contract::new(owner.clone(), None, None, None);

        testing_env!(get_context(user.clone(), NearToken::from_near(1)).build());
        let vault: AccountId = "vault.alice.near".parse().unwrap();
        contract.store_secrets(
            SecretAccessor::Repo {
                repo: "github.com/alice/p".to_string(),
                branch: None,
            },
            "default".to_string(),
            "ciphertext".to_string(),
            types::AccessCondition::AllowAll,
            Some(vault.clone()),
        );

        let bound = contract.get_secret_vault(
            SecretAccessor::Repo {
                repo: "github.com/alice/p".to_string(),
                branch: None,
            },
            "default".to_string(),
            user.clone(),
        );
        assert_eq!(bound, Some(vault));
    }

    #[test]
    fn store_secrets_without_vault_id_records_no_binding() {
        let owner = accounts(0);
        let user = accounts(2);

        testing_env!(get_context(owner.clone(), NearToken::from_near(0)).build());
        let mut contract = Contract::new(owner.clone(), None, None, None);

        testing_env!(get_context(user.clone(), NearToken::from_near(1)).build());
        contract.store_secrets(
            SecretAccessor::Repo {
                repo: "github.com/alice/p".to_string(),
                branch: None,
            },
            "default".to_string(),
            "ciphertext".to_string(),
            types::AccessCondition::AllowAll,
            None,
        );

        let bound = contract.get_secret_vault(
            SecretAccessor::Repo {
                repo: "github.com/alice/p".to_string(),
                branch: None,
            },
            "default".to_string(),
            user.clone(),
        );
        assert!(bound.is_none(), "no binding expected for default-master secret");
    }

    #[test]
    fn update_with_vault_id_none_keeps_binding_overhead_funded() {
        // Round-2 audit B-NEW-1 regression. Customer initially binds
        // to a vault, then updates the ciphertext with `vault_id: None`
        // (B-3 says the binding survives). The contract MUST quote
        // storage cost for the *post-call* state — i.e. with the
        // binding still on chain — otherwise the binding side-table
        // entry stays funded by nothing.
        //
        // The invariant we check: after the update, the deposit
        // recorded on the secret matches what `estimate_storage_cost`
        // says the cost is RIGHT NOW (binding present). If the bug
        // were back, the actual deposit (post-update) would be
        // smaller than the estimate — i.e. the contract under-funded
        // itself by the binding overhead.
        let owner = accounts(0);
        let user = accounts(2);

        testing_env!(get_context(owner.clone(), NearToken::from_near(0)).build());
        let mut contract = Contract::new(owner.clone(), None, None, None);

        let vault: AccountId = "vault.alice.near".parse().unwrap();
        let accessor = SecretAccessor::Repo {
            repo: "github.com/alice/p".to_string(),
            branch: None,
        };
        let ciphertext = "ciphertext".to_string();

        testing_env!(get_context(user.clone(), NearToken::from_near(1)).build());
        contract.store_secrets(
            accessor.clone(),
            "default".to_string(),
            ciphertext.clone(),
            types::AccessCondition::AllowAll,
            Some(vault.clone()),
        );
        // Sanity: binding really is in the side-table after the first
        // store. Pre-condition for the post-call check at
        // `internal_store_secrets` to see `vault_bound_after_call =
        // true` on the update.
        assert_eq!(
            contract.get_secret_vault(accessor.clone(), "default".to_string(), user.clone()),
            Some(vault.clone())
        );

        testing_env!(get_context(user.clone(), NearToken::from_near(1)).build());
        contract.store_secrets(
            accessor.clone(),
            "default".to_string(),
            ciphertext.clone(),
            types::AccessCondition::AllowAll,
            None,
        );

        // The deposit AFTER the no-op-on-binding update must match
        // what an estimate-with-vault-still-present would quote. If
        // the bug regressed, the actual post-update deposit would be
        // smaller than this estimate by the binding overhead.
        let post_update_deposit = contract
            .get_secrets(accessor.clone(), "default".to_string(), user.clone())
            .unwrap()
            .storage_deposit
            .0;
        let estimate_with_binding = contract
            .estimate_storage_cost(
                accessor,
                "default".to_string(),
                user,
                ciphertext,
                types::AccessCondition::AllowAll,
                None, // vault_id arg = None — binding-existence comes from side-table check
            )
            .0;
        assert_eq!(
            post_update_deposit, estimate_with_binding,
            "storage_deposit must match the post-call estimate (which sees the existing binding); \
             stored={post_update_deposit}, estimated={estimate_with_binding}"
        );
    }

    #[test]
    fn restore_with_vault_id_none_preserves_existing_binding() {
        // Plan back-compat invariant: an update that re-stores the
        // ciphertext but passes `vault_id: None` must NOT silently
        // clear an existing binding. Otherwise a forgetful caller
        // (legacy dashboard, half-migrated CLI) would brick decryption
        // by walking the binding back to default-master while the
        // ciphertext still requires the vault master.
        let owner = accounts(0);
        let user = accounts(2);

        testing_env!(get_context(owner.clone(), NearToken::from_near(0)).build());
        let mut contract = Contract::new(owner.clone(), None, None, None);

        testing_env!(get_context(user.clone(), NearToken::from_near(1)).build());
        let vault: AccountId = "vault.alice.near".parse().unwrap();
        contract.store_secrets(
            SecretAccessor::Repo {
                repo: "github.com/alice/p".to_string(),
                branch: None,
            },
            "default".to_string(),
            "ciphertext".to_string(),
            types::AccessCondition::AllowAll,
            Some(vault.clone()),
        );

        // Update with vault_id = None — binding must persist.
        testing_env!(get_context(user.clone(), NearToken::from_near(1)).build());
        contract.store_secrets(
            SecretAccessor::Repo {
                repo: "github.com/alice/p".to_string(),
                branch: None,
            },
            "default".to_string(),
            "ciphertext_v2".to_string(),
            types::AccessCondition::AllowAll,
            None,
        );

        let bound = contract.get_secret_vault(
            SecretAccessor::Repo {
                repo: "github.com/alice/p".to_string(),
                branch: None,
            },
            "default".to_string(),
            user.clone(),
        );
        assert_eq!(
            bound,
            Some(vault),
            "existing vault binding must survive a vault_id=None update"
        );
    }

    #[test]
    fn unbind_secret_vault_clears_existing_binding() {
        // Explicit opt-out path: customer wants to re-encrypt under
        // the default master and drop the vault binding.
        let owner = accounts(0);
        let user = accounts(2);

        testing_env!(get_context(owner.clone(), NearToken::from_near(0)).build());
        let mut contract = Contract::new(owner.clone(), None, None, None);

        testing_env!(get_context(user.clone(), NearToken::from_near(1)).build());
        let vault: AccountId = "vault.alice.near".parse().unwrap();
        contract.store_secrets(
            SecretAccessor::Repo {
                repo: "github.com/alice/p".to_string(),
                branch: None,
            },
            "default".to_string(),
            "ciphertext".to_string(),
            types::AccessCondition::AllowAll,
            Some(vault.clone()),
        );

        testing_env!(get_context(user.clone(), NearToken::from_near(0)).build());
        contract.unbind_secret_vault(
            SecretAccessor::Repo {
                repo: "github.com/alice/p".to_string(),
                branch: None,
            },
            "default".to_string(),
        );

        let bound = contract.get_secret_vault(
            SecretAccessor::Repo {
                repo: "github.com/alice/p".to_string(),
                branch: None,
            },
            "default".to_string(),
            user.clone(),
        );
        assert!(bound.is_none(), "unbind_secret_vault must clear the binding");
    }

    #[test]
    #[should_panic(expected = "secret not found or not owned by caller")]
    fn unbind_secret_vault_panics_on_missing_secret() {
        let owner = accounts(0);
        let user = accounts(2);

        testing_env!(get_context(owner.clone(), NearToken::from_near(0)).build());
        let mut contract = Contract::new(owner.clone(), None, None, None);

        testing_env!(get_context(user.clone(), NearToken::from_near(0)).build());
        contract.unbind_secret_vault(
            SecretAccessor::Repo {
                repo: "github.com/alice/p".to_string(),
                branch: None,
            },
            "default".to_string(),
        );
    }

    #[test]
    fn delete_secrets_cleans_up_binding() {
        let owner = accounts(0);
        let user = accounts(2);

        testing_env!(get_context(owner.clone(), NearToken::from_near(0)).build());
        let mut contract = Contract::new(owner.clone(), None, None, None);

        testing_env!(get_context(user.clone(), NearToken::from_near(1)).build());
        let vault: AccountId = "vault.alice.near".parse().unwrap();
        contract.store_secrets(
            SecretAccessor::Repo {
                repo: "github.com/alice/p".to_string(),
                branch: None,
            },
            "default".to_string(),
            "ciphertext".to_string(),
            types::AccessCondition::AllowAll,
            Some(vault),
        );

        contract.delete_secrets(
            SecretAccessor::Repo {
                repo: "github.com/alice/p".to_string(),
                branch: None,
            },
            "default".to_string(),
        );

        let bound = contract.get_secret_vault(
            SecretAccessor::Repo {
                repo: "github.com/alice/p".to_string(),
                branch: None,
            },
            "default".to_string(),
            user.clone(),
        );
        assert!(bound.is_none(), "delete must remove the binding");
    }

    #[test]
    fn get_secret_with_vault_combined_returns_both_fields() {
        let owner = accounts(0);
        let user = accounts(2);

        testing_env!(get_context(owner.clone(), NearToken::from_near(0)).build());
        let mut contract = Contract::new(owner.clone(), None, None, None);

        testing_env!(get_context(user.clone(), NearToken::from_near(1)).build());
        let vault: AccountId = "vault.alice.near".parse().unwrap();
        contract.store_secrets(
            SecretAccessor::Repo {
                repo: "github.com/alice/p".to_string(),
                branch: None,
            },
            "default".to_string(),
            "ciphertext".to_string(),
            types::AccessCondition::AllowAll,
            Some(vault.clone()),
        );

        let combined = contract.get_secret_with_vault(
            SecretAccessor::Repo {
                repo: "github.com/alice/p".to_string(),
                branch: None,
            },
            "default".to_string(),
            user.clone(),
        );
        assert!(combined.profile.is_some());
        assert_eq!(combined.vault_id, Some(vault));
    }

    #[test]
    fn pre_migration_secret_returns_no_vault_via_combined_view() {
        // Back-compat invariant. Secrets stored without a vault
        // binding have no entry in `secret_vault_bindings`. The
        // combined view must report `vault_id = None` for them so
        // the keystore-worker falls through to the default OutLayer
        // master.
        let owner = accounts(0);
        let user = accounts(2);

        testing_env!(get_context(owner.clone(), NearToken::from_near(0)).build());
        let mut contract = Contract::new(owner.clone(), None, None, None);

        // Simulate "stored before migration" by calling store_secrets
        // with vault_id = None (post-migration the side-table entry is
        // simply absent for this key, which is exactly the
        // pre-migration state).
        testing_env!(get_context(user.clone(), NearToken::from_near(1)).build());
        contract.store_secrets(
            SecretAccessor::Repo {
                repo: "github.com/legacy/p".to_string(),
                branch: None,
            },
            "default".to_string(),
            "ciphertext".to_string(),
            types::AccessCondition::AllowAll,
            None,
        );

        let combined = contract.get_secret_with_vault(
            SecretAccessor::Repo {
                repo: "github.com/legacy/p".to_string(),
                branch: None,
            },
            "default".to_string(),
            user.clone(),
        );
        assert!(combined.profile.is_some(), "secret must exist");
        assert!(
            combined.vault_id.is_none(),
            "pre-migration / unbound secret must report vault_id = None"
        );
    }

    #[test]
    fn get_secret_with_vault_returns_no_profile_for_missing_secret() {
        let owner = accounts(0);
        let user = accounts(2);

        testing_env!(get_context(owner.clone(), NearToken::from_near(0)).build());
        let contract = Contract::new(owner.clone(), None, None, None);

        let combined = contract.get_secret_with_vault(
            SecretAccessor::Repo {
                repo: "github.com/alice/p".to_string(),
                branch: None,
            },
            "default".to_string(),
            user.clone(),
        );
        assert!(combined.profile.is_none());
        assert!(combined.vault_id.is_none());
    }

    #[test]
    fn test_list_user_secrets() {
        let owner = accounts(0);
        let user = accounts(2);

        let context = get_context(owner.clone(), NearToken::from_near(0));
        testing_env!(context.build());

        let mut contract = Contract::new(owner.clone(), None, None, None);

        // Store both repo and wasm_hash secrets
        let context = get_context(user.clone(), NearToken::from_near(1));
        testing_env!(context.build());

        contract.store_secrets(
            SecretAccessor::Repo {
                repo: "github.com/alice/project".to_string(),
                branch: Some("main".to_string()),
            },
            "default".to_string(),
            "base64encodeddata".to_string(),
            types::AccessCondition::AllowAll,
            None,
        );

        let wasm_hash = "c".repeat(64);
        contract.store_secrets(
            SecretAccessor::WasmHash {
                hash: wasm_hash.clone(),
            },
            "production".to_string(),
            "base64encodeddata2".to_string(),
            types::AccessCondition::AllowAll,
            None,
        );

        // List user secrets
        let secrets = contract.list_user_secrets(user.clone());
        assert_eq!(secrets.len(), 2);

        // Verify we have both types
        let has_repo = secrets.iter().any(|s| matches!(&s.accessor, SecretAccessor::Repo { .. }));
        let has_wasm = secrets.iter().any(|s| matches!(&s.accessor, SecretAccessor::WasmHash { .. }));
        assert!(has_repo);
        assert!(has_wasm);
    }
}
