use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};
use near_sdk::collections::LookupMap;
use near_sdk::serde::{Deserialize, Serialize};
use near_sdk::{
    env, log, near_bindgen, AccountId, BorshStorageKey, NearToken, PanicOnDefault, PublicKey,
};
use schemars::JsonSchema;

const ONE_YOCTO: NearToken = NearToken::from_yoctonear(1);

#[derive(BorshSerialize, BorshStorageKey)]
#[borsh(crate = "near_sdk::borsh")]
enum StorageKey {
    ExpectedKeys,
}

#[derive(
    BorshDeserialize,
    BorshSerialize,
    Serialize,
    Deserialize,
    JsonSchema,
    Clone,
    Debug,
    PartialEq,
    Eq,
)]
#[serde(crate = "near_sdk::serde")]
pub struct ProofEvidence {
    #[schemars(with = "String")]
    pub user_id: AccountId,
    #[schemars(with = "String")]
    pub public_key: PublicKey,
    pub key_label: String,
    pub challenge: String,
    pub proved_at_block: u64,
    #[schemars(with = "String")]
    pub signer_id: AccountId,
    #[schemars(with = "String")]
    pub predecessor_id: AccountId,
    pub attached_deposit_yocto: String,
    pub status: ProofStatus,
}

#[derive(
    BorshDeserialize,
    BorshSerialize,
    Serialize,
    Deserialize,
    JsonSchema,
    Clone,
    Debug,
    PartialEq,
    Eq,
)]
#[serde(crate = "near_sdk::serde")]
pub enum ProofStatus {
    Pending,
    Proved,
    Revoked,
    Expired,
}

#[derive(
    BorshDeserialize,
    BorshSerialize,
    Serialize,
    Deserialize,
    JsonSchema,
    Clone,
    Debug,
    PartialEq,
    Eq,
)]
#[serde(crate = "near_sdk::serde")]
pub struct KeyStatus {
    #[schemars(with = "String")]
    pub user_id: AccountId,
    #[schemars(with = "String")]
    pub public_key: PublicKey,
    pub key_label: String,
    pub registered_at_block: u64,
    pub expires_at_block: Option<u64>,
    pub revoked_at_block: Option<u64>,
    pub last_proof: Option<ProofEvidence>,
}

#[near_bindgen]
#[derive(BorshDeserialize, BorshSerialize, PanicOnDefault)]
#[borsh(crate = "near_sdk::borsh")]
pub struct DirectUserProof {
    owner_id: AccountId,
    expected_keys: LookupMap<String, KeyStatus>,
}

#[near_bindgen]
impl DirectUserProof {
    #[init]
    pub fn new(owner_id: AccountId) -> Self {
        Self {
            owner_id,
            expected_keys: LookupMap::new(StorageKey::ExpectedKeys),
        }
    }

    pub fn register_expected_key(
        &mut self,
        user_id: AccountId,
        public_key: PublicKey,
        key_label: String,
        expires_at_block: Option<u64>,
    ) {
        self.assert_owner_or_user(&user_id);
        assert_valid_key_label(&key_label);

        if let Some(expires_at_block) = expires_at_block {
            assert!(
                expires_at_block > env::block_height(),
                "expires_at_block must be in the future"
            );
        }

        let status = KeyStatus {
            user_id: user_id.clone(),
            public_key: public_key.clone(),
            key_label: key_label.clone(),
            registered_at_block: env::block_height(),
            expires_at_block,
            revoked_at_block: None,
            last_proof: None,
        };
        self.expected_keys
            .insert(&key_id(&user_id, &key_label), &status);

        log!(
            "direct-user-proof:key_registered:user_id={}:key_label={}:public_key={}",
            user_id,
            key_label,
            String::from(&public_key)
        );
    }

    #[payable]
    pub fn prove_full_access(
        &mut self,
        user_id: AccountId,
        key_label: String,
        challenge: String,
    ) -> ProofEvidence {
        assert_valid_key_label(&key_label);
        assert!(!challenge.trim().is_empty(), "challenge must not be empty");
        assert_eq!(
            env::attached_deposit(),
            ONE_YOCTO,
            "prove_full_access requires exactly 1 yoctoNEAR"
        );

        let signer_id = env::signer_account_id();
        let predecessor_id = env::predecessor_account_id();
        assert_eq!(signer_id, user_id, "signer must match user_id");
        assert_eq!(
            predecessor_id, user_id,
            "predecessor must match user_id; forwarded calls do not prove true predecessor"
        );

        let id = key_id(&user_id, &key_label);
        let mut status = self
            .expected_keys
            .get(&id)
            .unwrap_or_else(|| env::panic_str("expected key is not registered"));
        assert!(
            status.revoked_at_block.is_none(),
            "expected key has been revoked"
        );
        if let Some(expires_at_block) = status.expires_at_block {
            assert!(
                env::block_height() <= expires_at_block,
                "expected key registration has expired"
            );
        }

        let signer_public_key = env::signer_account_pk();
        assert_eq!(
            signer_public_key, status.public_key,
            "signer public key does not match registered key"
        );

        let evidence = ProofEvidence {
            user_id: user_id.clone(),
            public_key: signer_public_key.clone(),
            key_label: key_label.clone(),
            challenge,
            proved_at_block: env::block_height(),
            signer_id,
            predecessor_id,
            attached_deposit_yocto: "1".to_string(),
            status: ProofStatus::Proved,
        };

        status.last_proof = Some(evidence.clone());
        self.expected_keys.insert(&id, &status);

        log!(
            "direct-user-proof:proved:user_id={}:key_label={}:public_key={}:block_height={}",
            user_id,
            key_label,
            String::from(&signer_public_key),
            evidence.proved_at_block
        );

        evidence
    }

    pub fn revoke_expected_key(&mut self, user_id: AccountId, key_label: String) {
        self.assert_owner_or_user(&user_id);
        assert_valid_key_label(&key_label);

        let id = key_id(&user_id, &key_label);
        let mut status = self
            .expected_keys
            .get(&id)
            .unwrap_or_else(|| env::panic_str("expected key is not registered"));
        status.revoked_at_block = Some(env::block_height());
        self.expected_keys.insert(&id, &status);

        log!(
            "direct-user-proof:key_revoked:user_id={}:key_label={}",
            user_id,
            key_label
        );
    }

    pub fn get_key_status(&self, user_id: AccountId, key_label: String) -> Option<KeyStatus> {
        self.expected_keys.get(&key_id(&user_id, &key_label))
    }

    pub fn get_owner(&self) -> AccountId {
        self.owner_id.clone()
    }

    fn assert_owner_or_user(&self, user_id: &AccountId) {
        let predecessor = env::predecessor_account_id();
        assert!(
            predecessor == self.owner_id || &predecessor == user_id,
            "only owner or user can manage expected keys"
        );
    }
}

fn key_id(user_id: &AccountId, key_label: &str) -> String {
    format!("{}:{}", user_id, key_label)
}

fn assert_valid_key_label(key_label: &str) {
    assert!(!key_label.trim().is_empty(), "key_label must not be empty");
}

#[cfg(test)]
mod tests {
    use super::*;
    use near_sdk::test_utils::VMContextBuilder;
    use near_sdk::testing_env;
    use near_sdk::CurveType;

    fn owner() -> AccountId {
        "sequential.test".parse().unwrap()
    }

    fn mike() -> AccountId {
        "mike.test".parse().unwrap()
    }

    fn alice() -> AccountId {
        "alice.test".parse().unwrap()
    }

    fn proof_contract_account() -> AccountId {
        "mike.sequential.test".parse().unwrap()
    }

    fn key_label() -> String {
        "direct-user-fa:mike.test".to_string()
    }

    fn public_key(byte: u8) -> PublicKey {
        PublicKey::from_parts(CurveType::ED25519, vec![byte; 32]).unwrap()
    }

    fn context(
        predecessor: AccountId,
        signer: AccountId,
        signer_public_key: PublicKey,
        attached_deposit_yocto: u128,
        block_height: u64,
    ) {
        testing_env!(VMContextBuilder::new()
            .predecessor_account_id(predecessor)
            .signer_account_id(signer)
            .signer_account_pk(signer_public_key)
            .attached_deposit(NearToken::from_yoctonear(attached_deposit_yocto))
            .block_height(block_height)
            .build());
    }

    fn contract() -> DirectUserProof {
        context(owner(), owner(), public_key(9), 0, 1);
        DirectUserProof::new(owner())
    }

    fn register_key(contract: &mut DirectUserProof, key: PublicKey, expires_at_block: Option<u64>) {
        context(owner(), owner(), public_key(9), 0, 5);
        contract.register_expected_key(mike(), key, key_label(), expires_at_block);
    }

    #[test]
    fn records_proof_when_full_access_evidence_matches() {
        let mut contract = contract();
        let key = public_key(1);
        register_key(&mut contract, key.clone(), Some(50));

        context(mike(), mike(), key.clone(), 1, 12);
        let evidence = contract.prove_full_access(
            mike(),
            key_label(),
            "wallet-direct-user-proof:mainnet-dry-run".to_string(),
        );

        assert_eq!(evidence.user_id, mike());
        assert_eq!(evidence.signer_id, mike());
        assert_eq!(evidence.predecessor_id, mike());
        assert_eq!(evidence.public_key, key);
        assert_eq!(evidence.proved_at_block, 12);
        assert_eq!(evidence.attached_deposit_yocto, "1");
        assert_eq!(evidence.status, ProofStatus::Proved);

        let status = contract.get_key_status(mike(), key_label()).unwrap();
        assert_eq!(status.last_proof, Some(evidence));
        assert_eq!(status.revoked_at_block, None);
    }

    #[test]
    #[should_panic(expected = "signer public key does not match registered key")]
    fn rejects_wrong_public_key() {
        let mut contract = contract();
        register_key(&mut contract, public_key(1), None);

        context(mike(), mike(), public_key(2), 1, 12);
        contract.prove_full_access(mike(), key_label(), "wrong-key".to_string());
    }

    #[test]
    #[should_panic(expected = "requires exactly 1 yoctoNEAR")]
    fn rejects_zero_deposit() {
        let mut contract = contract();
        let key = public_key(1);
        register_key(&mut contract, key.clone(), None);

        context(mike(), mike(), key, 0, 12);
        contract.prove_full_access(mike(), key_label(), "zero-deposit".to_string());
    }

    #[test]
    #[should_panic(expected = "signer must match user_id")]
    fn rejects_caller_where_signer_is_not_user() {
        let mut contract = contract();
        let key = public_key(1);
        register_key(&mut contract, key.clone(), None);

        context(alice(), alice(), key, 1, 12);
        contract.prove_full_access(mike(), key_label(), "wrong-signer".to_string());
    }

    #[test]
    #[should_panic(expected = "predecessor must match user_id")]
    fn rejects_forwarded_subaccount_call() {
        let mut contract = contract();
        let key = public_key(1);
        register_key(&mut contract, key.clone(), None);

        context(proof_contract_account(), mike(), key, 1, 12);
        contract.prove_full_access(mike(), key_label(), "forwarded-call".to_string());
    }

    #[test]
    fn revocation_and_status_views_work() {
        let mut contract = contract();
        let key = public_key(1);
        register_key(&mut contract, key.clone(), None);

        let registered = contract.get_key_status(mike(), key_label()).unwrap();
        assert_eq!(registered.public_key, key);
        assert_eq!(registered.registered_at_block, 5);
        assert_eq!(registered.revoked_at_block, None);
        assert_eq!(registered.last_proof, None);

        context(mike(), mike(), public_key(9), 0, 20);
        contract.revoke_expected_key(mike(), key_label());

        let revoked = contract.get_key_status(mike(), key_label()).unwrap();
        assert_eq!(revoked.revoked_at_block, Some(20));
    }

    #[test]
    #[should_panic(expected = "expected key has been revoked")]
    fn revoked_key_cannot_prove_full_access() {
        let mut contract = contract();
        let key = public_key(1);
        register_key(&mut contract, key.clone(), None);

        context(owner(), owner(), public_key(9), 0, 20);
        contract.revoke_expected_key(mike(), key_label());

        context(mike(), mike(), key, 1, 21);
        contract.prove_full_access(mike(), key_label(), "revoked".to_string());
    }

    #[test]
    #[should_panic(expected = "expected key registration has expired")]
    fn expired_key_cannot_prove_full_access() {
        let mut contract = contract();
        let key = public_key(1);
        register_key(&mut contract, key.clone(), Some(10));

        context(mike(), mike(), key, 1, 11);
        contract.prove_full_access(mike(), key_label(), "expired".to_string());
    }

    #[test]
    #[should_panic(expected = "only owner or user can manage expected keys")]
    fn third_party_cannot_register_expected_key() {
        let mut contract = contract();

        context(alice(), alice(), public_key(9), 0, 6);
        contract.register_expected_key(mike(), public_key(1), key_label(), None);
    }
}
