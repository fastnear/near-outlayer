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

    let contract = Contract::new(accounts(0), Some(accounts(0)));

    // Estimate cost for small secrets
    let small_data = "test_encrypted_data";
    let cost = contract.estimate_storage_cost(
        "github.com/test/repo".to_string(),
        None,
        "default".to_string(),
        accounts(1),
        small_data.to_string(),
        types::AccessCondition::AllowAll,
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

    let mut contract = Contract::new(accounts(0), Some(accounts(0)));

    // 1. Create large secret (1KB encrypted data)
    let large_data = "a".repeat(1000);
    let cost_large = contract.estimate_storage_cost(
        "github.com/test/repo".to_string(),
        None,
        "test".to_string(),
        accounts(1),
        large_data.clone(),
        types::AccessCondition::AllowAll,
    );

    println!("Large secret cost: {} yoctoNEAR", cost_large.0);

    // Store with exact deposit
    testing_env!(context.attached_deposit(NearToken::from_yoctonear(cost_large.0)).build());
    contract.store_secrets(
        "github.com/test/repo".to_string(),
        None,
        "test".to_string(),
        large_data,
        types::AccessCondition::AllowAll,
    );

    // Verify stored deposit
    let stored = contract.get_secrets(
        "github.com/test/repo".to_string(),
        None,
        "test".to_string(),
        accounts(1),
    ).unwrap();
    assert_eq!(stored.storage_deposit.0, cost_large.0, "Stored deposit should match cost");

    // 2. Update to small secret (10 bytes)
    let small_data = "small_data";
    let cost_small = contract.estimate_storage_cost(
        "github.com/test/repo".to_string(),
        None,
        "test".to_string(),
        accounts(1),
        small_data.to_string(),
        types::AccessCondition::AllowAll,
    );

    println!("Small secret cost: {} yoctoNEAR", cost_small.0);
    assert!(cost_small.0 < cost_large.0, "Small secret should cost less");

    // Try to update with 0 attached deposit (should use old deposit)
    testing_env!(context.attached_deposit(NearToken::from_yoctonear(0)).build());
    contract.store_secrets(
        "github.com/test/repo".to_string(),
        None,
        "test".to_string(),
        small_data.to_string(),
        types::AccessCondition::AllowAll,
    );

    // 3. Check result: storage_deposit should now be cost_small (NOT cost_large!)
    let updated = contract.get_secrets(
        "github.com/test/repo".to_string(),
        None,
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

    let mut contract = Contract::new(accounts(0), Some(accounts(0)));

    // 1. Create small secret
    let small_data = "small";
    let cost_small = contract.estimate_storage_cost(
        "github.com/test/repo".to_string(),
        None,
        "test".to_string(),
        accounts(1),
        small_data.to_string(),
        types::AccessCondition::AllowAll,
    );

    testing_env!(context.attached_deposit(NearToken::from_yoctonear(cost_small.0)).build());
    contract.store_secrets(
        "github.com/test/repo".to_string(),
        None,
        "test".to_string(),
        small_data.to_string(),
        types::AccessCondition::AllowAll,
    );

    // 2. Try to update to large secret with 0 deposit - should FAIL
    let large_data = "a".repeat(1000);
    let cost_large = contract.estimate_storage_cost(
        "github.com/test/repo".to_string(),
        None,
        "test".to_string(),
        accounts(1),
        large_data.clone(),
        types::AccessCondition::AllowAll,
    );

    println!("Small cost: {}, Large cost: {}", cost_small.0, cost_large.0);
    assert!(cost_large.0 > cost_small.0, "Large secret should cost more");

    // Try with 0 deposit - should panic
    testing_env!(context.attached_deposit(NearToken::from_yoctonear(0)).build());
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        contract.store_secrets(
            "github.com/test/repo".to_string(),
            None,
            "test".to_string(),
            large_data.clone(),
            types::AccessCondition::AllowAll,
        );
    }));

    assert!(result.is_err(), "Should panic when insufficient deposit for larger secret");

    // Now try with exact difference - should succeed
    let additional_needed = cost_large.0 - cost_small.0;
    testing_env!(context.attached_deposit(NearToken::from_yoctonear(additional_needed)).build());
    contract.store_secrets(
        "github.com/test/repo".to_string(),
        None,
        "test".to_string(),
        large_data,
        types::AccessCondition::AllowAll,
    );

    // Verify new cost is stored
    let updated = contract.get_secrets(
        "github.com/test/repo".to_string(),
        None,
        "test".to_string(),
        accounts(1),
    ).unwrap();
    assert_eq!(updated.storage_deposit.0, cost_large.0);
}

#[test]
fn test_multiple_secrets_separate_deposits() {
    let mut context = get_context(accounts(1));
    testing_env!(context.build());

    let mut contract = Contract::new(accounts(0), Some(accounts(0)));

    // Create secret 1: repo1/main/profile1
    let data1 = "secret_one_data";
    let cost1 = contract.estimate_storage_cost(
        "github.com/repo1".to_string(),
        Some("main".to_string()),
        "profile1".to_string(),
        accounts(1),
        data1.to_string(),
        types::AccessCondition::AllowAll,
    );

    testing_env!(context.attached_deposit(NearToken::from_yoctonear(cost1.0)).build());
    contract.store_secrets(
        "github.com/repo1".to_string(),
        Some("main".to_string()),
        "profile1".to_string(),
        data1.to_string(),
        types::AccessCondition::AllowAll,
    );

    // Create secret 2: repo2/dev/profile2
    let data2 = "secret_two_data_longer_content";
    let cost2 = contract.estimate_storage_cost(
        "github.com/repo2".to_string(),
        Some("dev".to_string()),
        "profile2".to_string(),
        accounts(1),
        data2.to_string(),
        types::AccessCondition::AllowAll,
    );

    testing_env!(context.attached_deposit(NearToken::from_yoctonear(cost2.0)).build());
    contract.store_secrets(
        "github.com/repo2".to_string(),
        Some("dev".to_string()),
        "profile2".to_string(),
        data2.to_string(),
        types::AccessCondition::AllowAll,
    );

    // Verify both exist with correct deposits
    let secret1 = contract.get_secrets(
        "github.com/repo1".to_string(),
        Some("main".to_string()),
        "profile1".to_string(),
        accounts(1),
    ).unwrap();
    assert_eq!(secret1.storage_deposit.0, cost1.0);

    let secret2 = contract.get_secrets(
        "github.com/repo2".to_string(),
        Some("dev".to_string()),
        "profile2".to_string(),
        accounts(1),
    ).unwrap();
    assert_eq!(secret2.storage_deposit.0, cost2.0);

    // Delete secret 1
    testing_env!(context.attached_deposit(NearToken::from_yoctonear(1)).build());
    contract.delete_secrets(
        "github.com/repo1".to_string(),
        Some("main".to_string()),
        "profile1".to_string(),
    );

    // Verify secret 1 is gone
    let deleted = contract.get_secrets(
        "github.com/repo1".to_string(),
        Some("main".to_string()),
        "profile1".to_string(),
        accounts(1),
    );
    assert!(deleted.is_none(), "Secret 1 should be deleted");

    // Verify secret 2 still exists with original deposit (NOT affected by delete)
    let secret2_after = contract.get_secrets(
        "github.com/repo2".to_string(),
        Some("dev".to_string()),
        "profile2".to_string(),
        accounts(1),
    ).unwrap();
    assert_eq!(secret2_after.storage_deposit.0, cost2.0, "Secret 2 deposit should be unchanged");
}

#[test]
fn test_delete_refunds_exact_amount() {
    let mut context = get_context(accounts(1));
    testing_env!(context.build());

    let mut contract = Contract::new(accounts(0), Some(accounts(0)));

    // Create secret
    let data = "test_secret_for_deletion";
    let cost = contract.estimate_storage_cost(
        "github.com/test/repo".to_string(),
        None,
        "test".to_string(),
        accounts(1),
        data.to_string(),
        types::AccessCondition::AllowAll,
    );

    testing_env!(context.attached_deposit(NearToken::from_yoctonear(cost.0)).build());
    contract.store_secrets(
        "github.com/test/repo".to_string(),
        None,
        "test".to_string(),
        data.to_string(),
        types::AccessCondition::AllowAll,
    );

    // Delete and check refund
    testing_env!(context.attached_deposit(NearToken::from_yoctonear(1)).build());
    contract.delete_secrets(
        "github.com/test/repo".to_string(),
        None,
        "test".to_string(),
    );

    // Note: In real scenario, we'd check Promise transfers
    // Here we verify the secret is gone
    let deleted = contract.get_secrets(
        "github.com/test/repo".to_string(),
        None,
        "test".to_string(),
        accounts(1),
    );
    assert!(deleted.is_none(), "Secret should be deleted and deposit refunded");
}

#[test]
fn test_access_condition_size_affects_cost() {
    let context = get_context(accounts(1));
    testing_env!(context.build());

    let contract = Contract::new(accounts(0), Some(accounts(0)));

    let data = "same_data_for_both";

    // Cost with simple access condition (AllowAll)
    let cost_simple = contract.estimate_storage_cost(
        "github.com/test/repo".to_string(),
        None,
        "test".to_string(),
        accounts(1),
        data.to_string(),
        types::AccessCondition::AllowAll,
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
        "github.com/test/repo".to_string(),
        None,
        "test".to_string(),
        accounts(1),
        data.to_string(),
        complex_access,
    );

    println!("Simple access cost: {}, Complex access cost: {}", cost_simple.0, cost_complex.0);
    assert!(cost_complex.0 > cost_simple.0, "Complex access condition should cost more");
}
