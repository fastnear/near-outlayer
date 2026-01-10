use crate::*;
use near_sdk::env;

/// Storage cost per byte in NEAR
pub const STORAGE_PRICE_PER_BYTE: Balance = 10_000_000_000_000_000_000; // 0.00001 NEAR per byte

#[near_bindgen]
impl Contract {
    /// Store secrets with access control
    ///
    /// User must attach storage deposit to cover the cost of storing secrets.
    /// The deposit will be refunded when secrets are deleted.
    ///
    /// # Arguments
    /// * `accessor` - What code can access these secrets (Repo or WasmHash)
    /// * `profile` - Profile name (e.g., "default", "premium", "staging")
    /// * `encrypted_secrets_base64` - Base64-encoded encrypted secrets
    /// * `access` - Access control rules
    #[payable]
    pub fn store_secrets(
        &mut self,
        accessor: SecretAccessor,
        profile: String,
        encrypted_secrets_base64: String,
        access: types::AccessCondition,
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

        // Calculate storage cost
        let storage_usage = self.calculate_secret_storage_size(&key, &encrypted_secrets_base64, &access);
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

    /// Calculate storage size for secrets (in bytes)
    fn calculate_secret_storage_size(
        &self,
        key: &SecretKey,
        encrypted_secrets: &str,
        access: &types::AccessCondition,
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

        BASE_STORAGE_OVERHEAD + key_size + value_size + index_overhead
    }
}

// View methods
#[near_bindgen]
impl Contract {
    /// Estimate storage cost for secrets (before storing)
    ///
    /// Returns cost in yoctoNEAR. Call this before `store_secrets` to know
    /// the exact deposit amount required.
    ///
    /// # Arguments
    /// * `accessor` - What code can access these secrets (Repo or WasmHash)
    /// * `profile` - Profile name
    /// * `owner` - Account that will own the secrets
    /// * `encrypted_secrets_base64` - Base64-encoded encrypted secrets
    /// * `access` - Access control rules
    pub fn estimate_storage_cost(
        &self,
        accessor: SecretAccessor,
        profile: String,
        owner: AccountId,
        encrypted_secrets_base64: String,
        access: types::AccessCondition,
    ) -> U128 {
        let key = SecretKey {
            accessor,
            profile,
            owner,
        };

        let storage_bytes = self.calculate_secret_storage_size(&key, &encrypted_secrets_base64, &access);
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
        );

        let wasm_hash = "c".repeat(64);
        contract.store_secrets(
            SecretAccessor::WasmHash {
                hash: wasm_hash.clone(),
            },
            "production".to_string(),
            "base64encodeddata2".to_string(),
            types::AccessCondition::AllowAll,
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
