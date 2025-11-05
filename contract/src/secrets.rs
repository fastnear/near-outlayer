use crate::*;
use near_sdk::env;

/// Storage cost per byte in NEAR
pub const STORAGE_PRICE_PER_BYTE: Balance = 10_000_000_000_000_000_000; // 0.00001 NEAR per byte

#[near_bindgen]
impl Contract {
    /// Store secrets for a repository with access control
    ///
    /// User must attach storage deposit to cover the cost of storing secrets.
    /// The deposit will be refunded when secrets are deleted.
    ///
    /// # Arguments
    /// * `repo` - Repository identifier (e.g., "owner/repo")
    /// * `branch` - Optional branch name (None = all branches)
    /// * `profile` - Profile name (e.g., "default", "premium", "staging")
    /// * `encrypted_secrets_base64` - Base64-encoded encrypted secrets
    /// * `access` - Access control rules
    #[payable]
    pub fn store_secrets(
        &mut self,
        repo: String,
        branch: Option<String>,
        profile: String,
        encrypted_secrets_base64: String,
        access: types::AccessCondition,
    ) {
        let caller = env::predecessor_account_id();

        // Validate inputs
        assert!(!repo.is_empty(), "Repository cannot be empty");
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

        // Validate branch name if provided
        if let Some(ref b) = branch {
            assert!(!b.is_empty(), "Branch name cannot be empty if provided");
            assert!(b.len() <= 255, "Branch name too long (max 255 chars)");
        }

        // Create secret key
        let key = SecretKey {
            repo: repo.clone(),
            branch: branch.clone(),
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
                    "Updating secrets: repo={}, branch={:?}, profile={}, old_deposit={}, attached={}, new_required={}, refund={}",
                    repo, branch, profile,
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
            "Secrets stored: repo={}, branch={:?}, profile={}, owner={}, deposit={} yoctoNEAR",
            repo,
            branch,
            profile,
            caller,
            required_deposit
        );
    }

    /// Delete secrets and refund storage deposit
    pub fn delete_secrets(
        &mut self,
        repo: String,
        branch: Option<String>,
        profile: String,
    ) {
        let caller = env::predecessor_account_id();

        let key = SecretKey {
            repo: repo.clone(),
            branch: branch.clone(),
            profile: profile.clone(),
            owner: caller.clone(),
        };

        let profile_data = self.secrets_storage.get(&key)
            .expect("Secrets not found");

        // Remove from storage
        self.secrets_storage.remove(&key);

        // Remove from user index
        if let Some(mut user_secrets) = self.user_secrets_index.get(&caller) {
            user_secrets.remove(&key);
            if user_secrets.is_empty() {
                // Remove empty set
                self.user_secrets_index.remove(&caller);
            } else {
                self.user_secrets_index.insert(&caller, &user_secrets);
            }
        }

        // Refund storage deposit
        if profile_data.storage_deposit > 0 {
            near_sdk::Promise::new(caller.clone())
                .transfer(NearToken::from_yoctonear(profile_data.storage_deposit));
        }

        log!(
            "Secrets deleted: repo={}, branch={:?}, profile={}, refunded={} yoctoNEAR",
            repo,
            branch,
            profile,
            profile_data.storage_deposit
        );
    }

    /// Update access control rules for existing secrets
    pub fn update_access(
        &mut self,
        repo: String,
        branch: Option<String>,
        profile: String,
        new_access: types::AccessCondition,
    ) {
        let caller = env::predecessor_account_id();

        let key = SecretKey {
            repo: repo.clone(),
            branch: branch.clone(),
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
            "Access control updated: repo={}, branch={:?}, profile={}",
            repo,
            branch,
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
        // - SecretKey: repo + branch + profile + owner (Borsh serialized)
        // - SecretProfile: encrypted_secrets + access + timestamps + deposit (Borsh serialized)
        // - User index entry: UnorderedSet overhead (for new entries)
        // - Base overhead: LookupMap entry overhead

        const BASE_STORAGE_OVERHEAD: u64 = 40; // LookupMap entry overhead (key hash + pointer)
        const INDEX_ENTRY_OVERHEAD: u64 = 64; // UnorderedSet entry overhead

        // Key size (Borsh serialization adds length prefixes)
        let key_size = (4 + key.repo.len() // String with u32 length prefix
            + 1 + key.branch.as_ref().map(|b| 4 + b.len()).unwrap_or(0) // Option<String>
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
    /// * `repo` - Repository identifier (e.g., "github.com/owner/repo")
    /// * `branch` - Optional branch name
    /// * `profile` - Profile name (e.g., "default", "production")
    /// * `owner` - Account that will own the secrets
    /// * `encrypted_secrets_base64` - Base64-encoded encrypted secrets
    /// * `access` - Access control rules
    ///
    /// # Example
    /// ```bash
    /// near view outlayer.testnet estimate_storage_cost '{
    ///   "repo": "github.com/alice/project",
    ///   "branch": null,
    ///   "profile": "production",
    ///   "owner": "alice.testnet",
    ///   "encrypted_secrets_base64": "YWJjZGVm...",
    ///   "access": "AllowAll"
    /// }'
    /// ```
    pub fn estimate_storage_cost(
        &self,
        repo: String,
        branch: Option<String>,
        profile: String,
        owner: AccountId,
        encrypted_secrets_base64: String,
        access: types::AccessCondition,
    ) -> U128 {
        let key = SecretKey {
            repo,
            branch,
            profile,
            owner,
        };

        let storage_bytes = self.calculate_secret_storage_size(&key, &encrypted_secrets_base64, &access);
        U128((storage_bytes as u128) * STORAGE_PRICE_PER_BYTE)
    }

    /// Get secrets for a repository (for keystore worker to read)
    ///
    /// If querying with a specific branch returns None, automatically tries
    /// with branch=null to find wildcard secrets (secrets that work for all branches).
    pub fn get_secrets(
        &self,
        repo: String,
        branch: Option<String>,
        profile: String,
        owner: AccountId,
    ) -> Option<SecretProfileView> {
        let key = SecretKey {
            repo: repo.clone(),
            branch: branch.clone(),
            profile: profile.clone(),
            owner: owner.clone(),
        };

        // Try with specified branch first
        if let Some(profile) = self.secrets_storage.get(&key) {
            return Some(SecretProfileView {
                encrypted_secrets: profile.encrypted_secrets,
                access: profile.access,
                created_at: profile.created_at,
                updated_at: profile.updated_at,
                storage_deposit: U128(profile.storage_deposit),
                branch: key.branch, // Return actual branch from key
            });
        }

        // If not found and branch was specified, try with branch=null (wildcard)
        if branch.is_some() {
            let wildcard_key = SecretKey {
                repo,
                branch: None,
                profile,
                owner,
            };
            if let Some(profile) = self.secrets_storage.get(&wildcard_key) {
                return Some(SecretProfileView {
                    encrypted_secrets: profile.encrypted_secrets,
                    access: profile.access,
                    created_at: profile.created_at,
                    updated_at: profile.updated_at,
                    storage_deposit: U128(profile.storage_deposit),
                    branch: wildcard_key.branch, // Return None (wildcard)
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
    pub fn secrets_exist(
        &self,
        repo: String,
        branch: Option<String>,
        profile: String,
        owner: AccountId,
    ) -> bool {
        let key = SecretKey {
            repo,
            branch,
            profile,
            owner,
        };

        self.secrets_storage.get(&key).is_some()
    }

    /// List all secrets for a user
    ///
    /// Returns array of secret metadata with repository, branch, profile info
    pub fn list_user_secrets(&self, account_id: AccountId) -> Vec<UserSecretInfo> {
        let user_secrets = self.user_secrets_index.get(&account_id);

        match user_secrets {
            Some(secrets_set) => {
                secrets_set
                    .iter()
                    .filter_map(|key| {
                        self.secrets_storage.get(&key).map(|profile| UserSecretInfo {
                            repo: key.repo.clone(),
                            branch: key.branch.clone(),
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

/// User secret metadata for list view
#[derive(Clone, Debug)]
#[near(serializers = [json])]
pub struct UserSecretInfo {
    pub repo: String,
    pub branch: Option<String>,
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
    fn test_store_secrets() {
        let owner = accounts(0);
        let operator = accounts(1);
        let user = accounts(2);

        let context = get_context(owner.clone(), NearToken::from_near(0));
        testing_env!(context.build());

        let mut contract = Contract::new(owner.clone(), Some(operator));

        // Store secrets with sufficient deposit
        let context = get_context(user.clone(), NearToken::from_near(1));
        testing_env!(context.build());

        contract.store_secrets(
            "github.com/alice/project".to_string(),
            None,
            "default".to_string(),
            "base64encodeddata".to_string(),
            types::AccessCondition::AllowAll,
        );

        // Verify secrets exist
        assert!(contract.secrets_exist(
            "github.com/alice/project".to_string(),
            None,
            "default".to_string(),
            user.clone(),
        ));
    }

    #[test]
    #[should_panic(expected = "Profile name must contain only alphanumeric")]
    fn test_invalid_profile_name() {
        let owner = accounts(0);
        let user = accounts(2);

        let context = get_context(owner.clone(), NearToken::from_near(0));
        testing_env!(context.build());

        let mut contract = Contract::new(owner.clone(), None);

        let context = get_context(user.clone(), NearToken::from_near(1));
        testing_env!(context.build());

        contract.store_secrets(
            "github.com/alice/project".to_string(),
            None,
            "invalid profile!".to_string(), // Invalid characters
            "base64encodeddata".to_string(),
            types::AccessCondition::AllowAll,
        );
    }

    #[test]
    fn test_delete_secrets() {
        let owner = accounts(0);
        let user = accounts(2);

        let context = get_context(owner.clone(), NearToken::from_near(0));
        testing_env!(context.build());

        let mut contract = Contract::new(owner.clone(), None);

        // Store secrets
        let context = get_context(user.clone(), NearToken::from_near(1));
        testing_env!(context.build());

        contract.store_secrets(
            "github.com/alice/project".to_string(),
            None,
            "default".to_string(),
            "base64encodeddata".to_string(),
            types::AccessCondition::AllowAll,
        );

        // Delete secrets
        contract.delete_secrets(
            "github.com/alice/project".to_string(),
            None,
            "default".to_string(),
        );

        // Verify secrets don't exist
        assert!(!contract.secrets_exist(
            "github.com/alice/project".to_string(),
            None,
            "default".to_string(),
            user.clone(),
        ));
    }
}
