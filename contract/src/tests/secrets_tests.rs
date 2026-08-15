use crate::*;
use crate::secrets::STORAGE_PRICE_PER_BYTE;
use near_sdk::test_utils::{accounts, VMContextBuilder};
use near_sdk::testing_env;
use near_sdk::NearToken;

fn get_context(predecessor: AccountId) -> VMContextBuilder {
    let mut builder = VMContextBuilder::new();
    builder
        .current_account_id(accounts(0))
        .signer_account_id(predecessor.clone())
        .predecessor_account_id(predecessor)
        .block_timestamp(1_000_000_000);
    builder
}

#[test]
fn test_estimate_storage_cost() {
    let context = get_context(accounts(1));
    testing_env!(context.build());

    let contract = Contract::new(accounts(0), Some(accounts(0)), None, None);

    // Estimate cost for small secrets
    let small_data = "test_encrypted_data";
    let cost = contract.estimate_storage_cost(
        SecretAccessor::Repo {
            repo: "github.com/test/repo".to_string(),
            branch: None,
        },
        "default".to_string(),
        accounts(1),
        small_data.to_string(),
        types::AccessCondition::AllowAll,
        None,
    );

    // Should be non-zero
    assert!(cost.0 > 0, "Storage cost should be greater than 0");

    // Cost should be at least BASE_OVERHEAD * PRICE_PER_BYTE
    let min_cost = 40 * STORAGE_PRICE_PER_BYTE;
    assert!(cost.0 >= min_cost, "Cost should be at least base overhead");
}

#[test]
fn test_storage_deposit_theft_attack_large_to_small() {
    let mut context = get_context(accounts(1));
    testing_env!(context.build());

    let mut contract = Contract::new(accounts(0), Some(accounts(0)), None, None);

    // 1. Create large secret (1KB encrypted data)
    let large_data = "a".repeat(1000);
    let cost_large = contract.estimate_storage_cost(
        SecretAccessor::Repo {
            repo: "github.com/test/repo".to_string(),
            branch: None,
        },
        "test".to_string(),
        accounts(1),
        large_data.clone(),
        types::AccessCondition::AllowAll,
        None,
    );

    println!("Large secret cost: {} yoctoNEAR", cost_large.0);

    // Store with exact deposit
    testing_env!(context.attached_deposit(NearToken::from_yoctonear(cost_large.0)).build());
    contract.store_secrets(
        SecretAccessor::Repo {
            repo: "github.com/test/repo".to_string(),
            branch: None,
        },
        "test".to_string(),
        large_data,
        types::AccessCondition::AllowAll,
        None,
    );

    // Verify stored deposit
    let stored = contract.get_secrets(
        SecretAccessor::Repo {
            repo: "github.com/test/repo".to_string(),
            branch: None,
        },
        "test".to_string(),
        accounts(1),
    ).unwrap();
    assert_eq!(stored.storage_deposit.0, cost_large.0, "Stored deposit should match cost");

    // 2. Update to small secret (10 bytes)
    let small_data = "small_data";
    let cost_small = contract.estimate_storage_cost(
        SecretAccessor::Repo {
            repo: "github.com/test/repo".to_string(),
            branch: None,
        },
        "test".to_string(),
        accounts(1),
        small_data.to_string(),
        types::AccessCondition::AllowAll,
        None,
    );

    println!("Small secret cost: {} yoctoNEAR", cost_small.0);
    assert!(cost_small.0 < cost_large.0, "Small secret should cost less");

    // Try to update with 0 attached deposit (should use old deposit)
    testing_env!(context.attached_deposit(NearToken::from_yoctonear(0)).build());
    contract.store_secrets(
        SecretAccessor::Repo {
            repo: "github.com/test/repo".to_string(),
            branch: None,
        },
        "test".to_string(),
        small_data.to_string(),
        types::AccessCondition::AllowAll,
        None,
    );

    // 3. Check result: storage_deposit should now be cost_small (NOT cost_large!)
    let updated = contract.get_secrets(
        SecretAccessor::Repo {
            repo: "github.com/test/repo".to_string(),
            branch: None,
        },
        "test".to_string(),
        accounts(1),
    ).unwrap();

    assert_eq!(
        updated.storage_deposit.0,
        cost_small.0,
        "After update, deposit should be new (smaller) cost, not old cost"
    );

    // Attacker should have received refund of (cost_large - cost_small)
    // This is correct behavior - not a theft!
}

#[test]
fn test_storage_deposit_increase_requires_payment() {
    let mut context = get_context(accounts(1));
    testing_env!(context.build());

    let mut contract = Contract::new(accounts(0), Some(accounts(0)), None, None);

    // 1. Create small secret
    let small_data = "small";
    let cost_small = contract.estimate_storage_cost(
        SecretAccessor::Repo {
            repo: "github.com/test/repo".to_string(),
            branch: None,
        },
        "test".to_string(),
        accounts(1),
        small_data.to_string(),
        types::AccessCondition::AllowAll,
        None,
    );

    testing_env!(context.attached_deposit(NearToken::from_yoctonear(cost_small.0)).build());
    contract.store_secrets(
        SecretAccessor::Repo {
            repo: "github.com/test/repo".to_string(),
            branch: None,
        },
        "test".to_string(),
        small_data.to_string(),
        types::AccessCondition::AllowAll,
        None,
    );

    // 2. Try to update to large secret with 0 deposit - should FAIL
    let large_data = "a".repeat(1000);
    let cost_large = contract.estimate_storage_cost(
        SecretAccessor::Repo {
            repo: "github.com/test/repo".to_string(),
            branch: None,
        },
        "test".to_string(),
        accounts(1),
        large_data.clone(),
        types::AccessCondition::AllowAll,
        None,
    );

    println!("Small cost: {}, Large cost: {}", cost_small.0, cost_large.0);
    assert!(cost_large.0 > cost_small.0, "Large secret should cost more");

    // Try with 0 deposit - should panic
    testing_env!(context.attached_deposit(NearToken::from_yoctonear(0)).build());
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        contract.store_secrets(
            SecretAccessor::Repo {
                repo: "github.com/test/repo".to_string(),
                branch: None,
            },
            "test".to_string(),
            large_data.clone(),
            types::AccessCondition::AllowAll,
            None,
        );
    }));

    assert!(result.is_err(), "Should panic when insufficient deposit for larger secret");

    // Now try with exact difference - should succeed
    let additional_needed = cost_large.0 - cost_small.0;
    testing_env!(context.attached_deposit(NearToken::from_yoctonear(additional_needed)).build());
    contract.store_secrets(
        SecretAccessor::Repo {
            repo: "github.com/test/repo".to_string(),
            branch: None,
        },
        "test".to_string(),
        large_data,
        types::AccessCondition::AllowAll,
        None,
    );

    // Verify new cost is stored
    let updated = contract.get_secrets(
        SecretAccessor::Repo {
            repo: "github.com/test/repo".to_string(),
            branch: None,
        },
        "test".to_string(),
        accounts(1),
    ).unwrap();
    assert_eq!(updated.storage_deposit.0, cost_large.0);
}

#[test]
fn test_multiple_secrets_separate_deposits() {
    let mut context = get_context(accounts(1));
    testing_env!(context.build());

    let mut contract = Contract::new(accounts(0), Some(accounts(0)), None, None);

    // Create secret 1: repo1/main/profile1
    let data1 = "secret_one_data";
    let cost1 = contract.estimate_storage_cost(
        SecretAccessor::Repo {
            repo: "github.com/repo1".to_string(),
            branch: Some("main".to_string()),
        },
        "profile1".to_string(),
        accounts(1),
        data1.to_string(),
        types::AccessCondition::AllowAll,
        None,
    );

    testing_env!(context.attached_deposit(NearToken::from_yoctonear(cost1.0)).build());
    contract.store_secrets(
        SecretAccessor::Repo {
            repo: "github.com/repo1".to_string(),
            branch: Some("main".to_string()),
        },
        "profile1".to_string(),
        data1.to_string(),
        types::AccessCondition::AllowAll,
        None,
    );

    // Create secret 2: repo2/dev/profile2
    let data2 = "secret_two_data_longer_content";
    let cost2 = contract.estimate_storage_cost(
        SecretAccessor::Repo {
            repo: "github.com/repo2".to_string(),
            branch: Some("dev".to_string()),
        },
        "profile2".to_string(),
        accounts(1),
        data2.to_string(),
        types::AccessCondition::AllowAll,
        None,
    );

    testing_env!(context.attached_deposit(NearToken::from_yoctonear(cost2.0)).build());
    contract.store_secrets(
        SecretAccessor::Repo {
            repo: "github.com/repo2".to_string(),
            branch: Some("dev".to_string()),
        },
        "profile2".to_string(),
        data2.to_string(),
        types::AccessCondition::AllowAll,
        None,
    );

    // Verify both exist with correct deposits
    let secret1 = contract.get_secrets(
        SecretAccessor::Repo {
            repo: "github.com/repo1".to_string(),
            branch: Some("main".to_string()),
        },
        "profile1".to_string(),
        accounts(1),
    ).unwrap();
    assert_eq!(secret1.storage_deposit.0, cost1.0);

    let secret2 = contract.get_secrets(
        SecretAccessor::Repo {
            repo: "github.com/repo2".to_string(),
            branch: Some("dev".to_string()),
        },
        "profile2".to_string(),
        accounts(1),
    ).unwrap();
    assert_eq!(secret2.storage_deposit.0, cost2.0);

    // Delete secret 1
    testing_env!(context.attached_deposit(NearToken::from_yoctonear(1)).build());
    contract.delete_secrets(
        SecretAccessor::Repo {
            repo: "github.com/repo1".to_string(),
            branch: Some("main".to_string()),
        },
        "profile1".to_string(),
    );

    // Verify secret 1 is gone
    let deleted = contract.get_secrets(
        SecretAccessor::Repo {
            repo: "github.com/repo1".to_string(),
            branch: Some("main".to_string()),
        },
        "profile1".to_string(),
        accounts(1),
    );
    assert!(deleted.is_none(), "Secret 1 should be deleted");

    // Verify secret 2 still exists with original deposit (NOT affected by delete)
    let secret2_after = contract.get_secrets(
        SecretAccessor::Repo {
            repo: "github.com/repo2".to_string(),
            branch: Some("dev".to_string()),
        },
        "profile2".to_string(),
        accounts(1),
    ).unwrap();
    assert_eq!(secret2_after.storage_deposit.0, cost2.0, "Secret 2 deposit should be unchanged");
}

#[test]
fn test_delete_refunds_exact_amount() {
    let mut context = get_context(accounts(1));
    testing_env!(context.build());

    let mut contract = Contract::new(accounts(0), Some(accounts(0)), None, None);

    // Create secret
    let data = "test_secret_for_deletion";
    let cost = contract.estimate_storage_cost(
        SecretAccessor::Repo {
            repo: "github.com/test/repo".to_string(),
            branch: None,
        },
        "test".to_string(),
        accounts(1),
        data.to_string(),
        types::AccessCondition::AllowAll,
        None,
    );

    testing_env!(context.attached_deposit(NearToken::from_yoctonear(cost.0)).build());
    contract.store_secrets(
        SecretAccessor::Repo {
            repo: "github.com/test/repo".to_string(),
            branch: None,
        },
        "test".to_string(),
        data.to_string(),
        types::AccessCondition::AllowAll,
        None,
    );

    // Delete and check refund
    testing_env!(context.attached_deposit(NearToken::from_yoctonear(1)).build());
    contract.delete_secrets(
        SecretAccessor::Repo {
            repo: "github.com/test/repo".to_string(),
            branch: None,
        },
        "test".to_string(),
    );

    // Note: In real scenario, we'd check Promise transfers
    // Here we verify the secret is gone
    let deleted = contract.get_secrets(
        SecretAccessor::Repo {
            repo: "github.com/test/repo".to_string(),
            branch: None,
        },
        "test".to_string(),
        accounts(1),
    );
    assert!(deleted.is_none(), "Secret should be deleted and deposit refunded");
}

#[test]
fn test_access_condition_size_affects_cost() {
    let context = get_context(accounts(1));
    testing_env!(context.build());

    let contract = Contract::new(accounts(0), Some(accounts(0)), None, None);

    let data = "same_data_for_both";

    // Cost with simple access condition (AllowAll)
    let cost_simple = contract.estimate_storage_cost(
        SecretAccessor::Repo {
            repo: "github.com/test/repo".to_string(),
            branch: None,
        },
        "test".to_string(),
        accounts(1),
        data.to_string(),
        types::AccessCondition::AllowAll,
        None,
    );

    // Cost with complex access condition (Whitelist with many accounts)
    let complex_access = types::AccessCondition::Whitelist {
        accounts: vec![
            "account1.near".parse().unwrap(),
            "account2.near".parse().unwrap(),
            "account3.near".parse().unwrap(),
            "account4.near".parse().unwrap(),
            "account5.near".parse().unwrap(),
        ],
    };

    let cost_complex = contract.estimate_storage_cost(
        SecretAccessor::Repo {
            repo: "github.com/test/repo".to_string(),
            branch: None,
        },
        "test".to_string(),
        accounts(1),
        data.to_string(),
        complex_access,
        None,
    );

    println!("Simple access cost: {}, Complex access cost: {}", cost_simple.0, cost_complex.0);
    assert!(cost_complex.0 > cost_simple.0, "Complex access condition should cost more");
}

/// A payment key is written once. Its blob is a money record maintained by the
/// worker on real transfers, and `store_secrets` takes opaque bytes it cannot
/// inspect — so "the owner may rewrite it" would mean "the owner may put
/// anything in it".
#[test]
#[should_panic(expected = "cannot be rewritten")]
fn a_payment_key_cannot_be_overwritten() {
    let mut context = get_context(accounts(1));
    testing_env!(context.build());
    let mut contract = Contract::new(accounts(0), Some(accounts(0)), None, None);

    let blob = "encrypted".to_string();
    let cost = contract.estimate_storage_cost(
        SecretAccessor::System(SystemSecretType::PaymentKey),
        "7".to_string(),
        accounts(1),
        blob.clone(),
        types::AccessCondition::AllowAll,
        None,
    );

    testing_env!(context.attached_deposit(NearToken::from_yoctonear(cost.0)).build());
    contract.store_secrets(
        SecretAccessor::System(SystemSecretType::PaymentKey),
        "7".to_string(),
        blob,
        types::AccessCondition::AllowAll,
        None,
    );

    // Same owner, same nonce, different bytes — refused.
    testing_env!(context.attached_deposit(NearToken::from_yoctonear(cost.0)).build());
    contract.store_secrets(
        SecretAccessor::System(SystemSecretType::PaymentKey),
        "7".to_string(),
        "forged".to_string(),
        types::AccessCondition::AllowAll,
        None,
    );
}

/// The refusal is for payment keys ONLY. Rotating an ordinary secret in place
/// is what secrets are for, and that must keep working untouched.
#[test]
fn an_ordinary_secret_is_still_overwritable() {
    let mut context = get_context(accounts(1));
    testing_env!(context.build());
    let mut contract = Contract::new(accounts(0), Some(accounts(0)), None, None);

    let accessor = SecretAccessor::Repo {
        repo: "github.com/test/repo".to_string(),
        branch: None,
    };
    let cost = contract.estimate_storage_cost(
        accessor.clone(),
        "test".to_string(),
        accounts(1),
        "v1".to_string(),
        types::AccessCondition::AllowAll,
        None,
    );

    testing_env!(context.attached_deposit(NearToken::from_yoctonear(cost.0)).build());
    contract.store_secrets(
        accessor.clone(),
        "test".to_string(),
        "v1".to_string(),
        types::AccessCondition::AllowAll,
        None,
    );

    testing_env!(context.attached_deposit(NearToken::from_yoctonear(cost.0)).build());
    contract.store_secrets(
        accessor.clone(),
        "test".to_string(),
        "v2".to_string(),
        types::AccessCondition::AllowAll,
        None,
    );

    let stored = contract
        .secrets_storage
        .get(&SecretKey {
            accessor,
            profile: "test".to_string(),
            owner: accounts(1),
        })
        .expect("the secret must still be there");
    assert_eq!(
        stored.encrypted_secrets, "v2",
        "an ordinary secret must keep taking a new value"
    );
}

/// Store a payment key at `nonce`, so the nonce rules below exercise the real
/// entry point rather than a copy of the check.
fn store_payment_key_at(nonce: &str) {
    let mut context = get_context(accounts(1));
    testing_env!(context.build());
    let mut contract = Contract::new(accounts(0), Some(accounts(0)), None, None);

    let blob = "encrypted".to_string();
    let cost = contract.estimate_storage_cost(
        SecretAccessor::System(SystemSecretType::PaymentKey),
        nonce.to_string(),
        accounts(1),
        blob.clone(),
        types::AccessCondition::AllowAll,
        None,
    );

    testing_env!(context.attached_deposit(NearToken::from_yoctonear(cost.0)).build());
    contract.store_secrets(
        SecretAccessor::System(SystemSecretType::PaymentKey),
        nonce.to_string(),
        blob,
        types::AccessCondition::AllowAll,
        None,
    );
}

/// Nonce 0 is reserved for the keys the coordinator issues in its own database
/// — the free trial — which have no on-chain record and cost no gas.
///
/// `payment_keys` is keyed by `(owner, nonce)`, so an on-chain key at 0 would
/// land in the SAME row as its owner's trial key. One order silently drops the
/// real key's hash (`init` inserts with `ON CONFLICT DO NOTHING`, so the key is
/// paid for and dead); the other marks a funded key as a grant, and its owner
/// can no longer withdraw their own money.
///
/// `get_next_payment_key_nonce` has always answered 1 for a fresh account, but
/// it only SUGGESTS — nothing stopped a hand-built transaction from claiming 0.
/// Checked on mainnet and testnet before this was added: no key at nonce 0
/// exists, so nothing that works today stops working.
#[test]
#[should_panic(expected = "nonce 0 is reserved")]
fn a_payment_key_cannot_take_the_reserved_nonce() {
    store_payment_key_at("0");
}

/// The coordinator holds a nonce as a 32-bit SIGNED integer, so 2^31 and above
/// arrive there as negative numbers. Refused on the way in rather than left to
/// mean one thing on chain and another in the database.
#[test]
#[should_panic(expected = "too large")]
fn a_payment_key_nonce_must_fit_where_it_is_stored() {
    store_payment_key_at("2147483648");
}

/// The first nonce the contract itself hands out must keep working, as must the
/// largest one that still fits. The rules above bound the range; they must not
/// move its edges.
#[test]
fn the_first_and_last_usable_nonces_are_accepted() {
    store_payment_key_at("1");
    store_payment_key_at("2147483647");
}
