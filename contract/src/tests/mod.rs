#[cfg(test)]
pub mod execution_tests;

#[cfg(test)]
pub mod secrets_tests;

#[cfg(test)]
use crate::*;
#[cfg(test)]
use near_sdk::test_utils::{accounts, VMContextBuilder};
#[cfg(test)]
use near_sdk::{testing_env, NearToken};

#[cfg(test)]
pub fn get_context(predecessor: AccountId, deposit: NearToken) -> VMContextBuilder {
    let mut builder = VMContextBuilder::new();
    builder
        .predecessor_account_id(predecessor)
        .attached_deposit(deposit);
    builder
}

#[cfg(test)]
pub fn setup_contract() -> Contract {
    let context = get_context(accounts(0), NearToken::from_near(0));
    testing_env!(context.build());

    Contract::new(accounts(0), Some(accounts(1)), None, None)
}

#[cfg(test)]
mod basic_tests {
    use super::*;

    #[test]
    fn test_initialization() {
        let contract = setup_contract();

        assert_eq!(contract.owner_id, accounts(0));
        assert_eq!(contract.operator_id, accounts(1));
        assert_eq!(contract.next_request_id, 0);
        assert!(!contract.paused);
        assert_eq!(contract.total_executions, 0);
        assert_eq!(contract.total_fees_collected, 0);
    }

    #[test]
    fn test_get_config() {
        let contract = setup_contract();
        let (owner, operator) = contract.get_config();

        assert_eq!(owner, accounts(0));
        assert_eq!(operator, accounts(1));
    }

    #[test]
    fn test_get_pricing() {
        let contract = setup_contract();
        let (base, per_inst, per_ms, per_compile_ms) = contract.get_pricing();

        assert_eq!(base.0, 1_000_000_000_000_000_000_000); // 0.001 NEAR
        assert_eq!(per_inst.0, 100_000_000_000_000); // 0.0000001 NEAR per million instructions
        assert_eq!(per_ms.0, 100_000_000_000_000_000); // 0.0001 NEAR per second (execution)
        assert_eq!(per_compile_ms.0, 100_000_000_000_000_000); // 0.0001 NEAR per second (compilation)
    }

    #[test]
    fn test_is_paused() {
        let contract = setup_contract();
        assert!(!contract.is_paused());
    }

    #[test]
    fn test_get_stats() {
        let contract = setup_contract();
        let (executions, fees) = contract.get_stats();

        assert_eq!(executions, 0);
        assert_eq!(fees.0, 0);
    }
}

#[cfg(test)]
mod admin_tests {
    use super::*;

    #[test]
    fn test_set_operator() {
        let mut contract = setup_contract();
        let new_operator = accounts(2);

        // Owner sets new operator
        let context = get_context(accounts(0), NearToken::from_near(0));
        testing_env!(context.build());

        contract.set_operator(new_operator.clone());

        let (_, operator) = contract.get_config();
        assert_eq!(operator, new_operator);
    }

    #[test]
    #[should_panic(expected = "Only owner can call this method")]
    fn test_set_operator_unauthorized() {
        let mut contract = setup_contract();

        // Non-owner tries to set operator
        let context = get_context(accounts(2), NearToken::from_near(0));
        testing_env!(context.build());

        contract.set_operator(accounts(3));
    }

    #[test]
    fn test_set_owner() {
        let mut contract = setup_contract();
        let new_owner = accounts(2);

        // Current owner sets new owner
        let context = get_context(accounts(0), NearToken::from_near(0));
        testing_env!(context.build());

        contract.set_owner(new_owner.clone());

        let (owner, _) = contract.get_config();
        assert_eq!(owner, new_owner);
    }

    #[test]
    fn test_set_paused() {
        let mut contract = setup_contract();

        // Owner pauses contract
        let context = get_context(accounts(0), NearToken::from_near(0));
        testing_env!(context.build());

        contract.set_paused(true);
        assert!(contract.is_paused());

        contract.set_paused(false);
        assert!(!contract.is_paused());
    }

    #[test]
    fn test_set_pricing() {
        let mut contract = setup_contract();

        // Owner updates pricing
        let context = get_context(accounts(0), NearToken::from_near(0));
        testing_env!(context.build());

        let new_base = U128(20_000_000_000_000_000_000_000);
        contract.set_pricing(Some(new_base), None, None, None, None, None, None, None);

        let (base, _, _, _) = contract.get_pricing();
        assert_eq!(base, new_base);
    }

    #[test]
    #[should_panic(expected = "Only owner can call this method")]
    fn test_set_paused_unauthorized() {
        let mut contract = setup_contract();

        // Non-owner tries to pause
        let context = get_context(accounts(2), NearToken::from_near(0));
        testing_env!(context.build());

        contract.set_paused(true);
    }
}

/// Storage-layout invariants.
///
/// Borsh has no schema evolution: it writes fields in declaration order and
/// enum variants as their ORDINAL, with no names and no length prefix. So an
/// upgrade that reorders an enum or appends a struct field does not fail to
/// compile and does not fail any behavioural test — it silently makes state
/// written by the previous contract unreadable, or reads it as something else.
///
/// These tests pin the two shapes that are already on chain.
#[cfg(test)]
mod subscription_plan_tests {
    use crate::payment::SubscriptionPlan;
    use crate::*;
    use near_sdk::test_utils::{accounts, VMContextBuilder};
    use near_sdk::{testing_env, NearToken};

    fn ctx(predecessor: AccountId) -> VMContextBuilder {
        let mut b = VMContextBuilder::new();
        b.predecessor_account_id(predecessor)
            .attached_deposit(NearToken::from_near(0));
        b
    }

    fn plan(index: u8, name: &str, price_usd: u128, active: bool) -> SubscriptionPlan {
        SubscriptionPlan {
            index,
            name: name.to_string(),
            price_usd: U128(price_usd),
            active,
        }
    }

    fn contract_with_plans(plans: Vec<SubscriptionPlan>) -> Contract {
        testing_env!(ctx(accounts(0)).build());
        let mut c = Contract::new(accounts(0), Some(accounts(0)), None, None);
        c.set_subscription_plans(plans);
        c
    }

    /// The price list is the CONTRACT's, and a fresh deployment has none.
    ///
    /// Empty must sell nothing rather than everything: a deployment whose
    /// operator has not set prices yet has to refuse and hand the money back,
    /// not hand out a subscription at a price nobody chose.
    #[test]
    fn a_contract_starts_with_no_plans_and_therefore_sells_none() {
        testing_env!(ctx(accounts(0)).build());
        let c = Contract::new(accounts(0), Some(accounts(0)), None, None);
        assert!(c.get_subscription_plans().is_empty());
    }

    #[test]
    fn the_owner_sets_the_price_list_and_it_reads_back() {
        let c = contract_with_plans(vec![
            plan(0, "starter", 10_000_000, true),
            plan(1, "pro", 50_000_000, true),
        ]);

        let read = c.get_subscription_plans();
        assert_eq!(read.len(), 2);
        assert_eq!(read[0].name, "starter");
        assert_eq!(read[0].price_usd.0, 10_000_000);
        assert!(read[1].active);
    }

    /// A withdrawn plan stays in the list. Removing the row would free its
    /// index, and an index is what a payment names.
    #[test]
    fn a_withdrawn_plan_keeps_its_index() {
        let c = contract_with_plans(vec![
            plan(0, "starter", 10_000_000, true),
            plan(1, "promo-nov", 5_000_000, false),
        ]);

        let promo = c
            .get_subscription_plans()
            .into_iter()
            .find(|p| p.index == 1)
            .expect("a withdrawn plan is still readable");
        assert!(!promo.active);
    }

    /// The trap this closes: a payment is signed before it lands. If index 1
    /// meant `promo` when the transaction was built and `pro` when it arrived,
    /// the payer buys something they did not choose, at a price they did not
    /// agree.
    #[test]
    #[should_panic(expected = "may not be repointed")]
    fn an_index_cannot_be_repointed_at_another_offer() {
        let mut c = contract_with_plans(vec![plan(1, "promo-nov", 5_000_000, true)]);
        testing_env!(ctx(accounts(0)).build());
        c.set_subscription_plans(vec![plan(1, "pro", 50_000_000, true)]);
    }

    /// Retiring a plan outright is allowed — that is an index disappearing, not
    /// one changing meaning.
    #[test]
    fn a_plan_may_be_dropped_from_the_list() {
        let mut c = contract_with_plans(vec![
            plan(0, "starter", 10_000_000, true),
            plan(1, "promo-nov", 5_000_000, true),
        ]);
        testing_env!(ctx(accounts(0)).build());
        c.set_subscription_plans(vec![plan(0, "starter", 10_000_000, true)]);

        assert_eq!(c.get_subscription_plans().len(), 1);
    }

    /// The price list is read on EVERY call — it lives in the contract's root
    /// state — so its size is a cost paid by users who are not buying anything.
    #[test]
    #[should_panic(expected = "at most 16 plans")]
    fn the_price_list_is_bounded() {
        let many: Vec<SubscriptionPlan> = (0..17)
            .map(|i| plan(i, &format!("plan-{i}"), 1_000_000, true))
            .collect();
        contract_with_plans(many);
    }

    #[test]
    #[should_panic(expected = "duplicate plan index")]
    fn two_plans_cannot_share_an_index() {
        contract_with_plans(vec![
            plan(3, "starter", 10_000_000, true),
            plan(3, "pro", 50_000_000, true),
        ]);
    }

    /// A free plan is a grant, and grants do not go through a payment path at
    /// all. A zero here would mean any transfer, however small, buys it.
    #[test]
    #[should_panic(expected = "has no price")]
    fn a_plan_must_cost_something() {
        contract_with_plans(vec![plan(0, "free", 0, true)]);
    }

    #[test]
    #[should_panic(expected = "Only owner")]
    fn only_the_owner_sets_prices() {
        testing_env!(ctx(accounts(0)).build());
        let mut c = Contract::new(accounts(0), Some(accounts(0)), None, None);
        testing_env!(ctx(accounts(2)).build());
        c.set_subscription_plans(vec![plan(0, "starter", 10_000_000, true)]);
    }
}

/// What a curated project charges, and what that does to admission.
mod project_pricing_tests {
    use crate::payment::{max_price, OperationPrice, ProjectPricing};
    use crate::*;
    use near_sdk::collections::UnorderedMap;
    use near_sdk::test_utils::{accounts, VMContextBuilder};
    use near_sdk::{testing_env, NearToken};

    const PROJECT: &str = "connectors.outlayer.near/near-email";

    fn ctx(predecessor: AccountId, deposit: NearToken) -> VMContextBuilder {
        let mut b = VMContextBuilder::new();
        b.predecessor_account_id(predecessor).attached_deposit(deposit);
        b
    }

    fn op(operation: &str, price_usd: u128) -> OperationPrice {
        OperationPrice {
            operation: operation.to_string(),
            price_usd: U128(price_usd),
            developer_share_bp: 7_000,
        }
    }

    fn pricing(operations: Vec<OperationPrice>) -> ProjectPricing {
        ProjectPricing {
            author_account_id: accounts(3),
            operations,
        }
    }

    /// A contract with `PROJECT` registered and runnable. Built by writing the
    /// maps directly: `create_project` wants a storage deposit and a whole
    /// version flow, and none of that is what these tests are about.
    fn contract_with_project() -> Contract {
        testing_env!(ctx(accounts(0), NearToken::from_near(0)).build());
        let mut c = Contract::new(accounts(0), Some(accounts(0)), None, None);

        let uuid = "p0000000000000001".to_string();
        c.projects.insert(
            &PROJECT.to_string(),
            &Project {
                uuid: uuid.clone(),
                owner: accounts(0),
                name: "near-email".to_string(),
                active_version: "v1".to_string(),
                created_at: 0,
                storage_deposit: 0,
            },
        );
        let mut versions = UnorderedMap::new(StorageKey::ProjectVersions {
            project_uuid: uuid.clone(),
        });
        versions.insert(
            &"v1".to_string(),
            &VersionInfo {
                source: CodeSource::WasmUrl {
                    url: "https://example.invalid/near-email.wasm".to_string(),
                    hash: "ab".to_string(),
                    build_target: None,
                },
                added_at: 0,
                storage_deposit: 0,
            },
        );
        c.project_versions.insert(&uuid, &versions);
        c
    }

    /// Call the project with a body naming `operation`, the one universal field.
    fn call_op(c: &mut Contract, operation: &str, attached_usd: Option<u128>) {
        call_body(
            c,
            Some(format!("{{\"operation\":\"{}\"}}", operation)),
            attached_usd,
        );
    }

    fn call_body(c: &mut Contract, input_data: Option<String>, attached_usd: Option<u128>) {
        c.request_execution(
            ExecutionSource::Project {
                project_id: PROJECT.to_string(),
                version_key: None,
            },
            Some(ResourceLimits::default()),
            input_data,
            None,
            None,
            None,
            Some(RequestParams {
                attached_usd: attached_usd.map(U128),
                ..Default::default()
            }),
        );
    }

    // ---- the price list itself -------------------------------------------

    /// Admission charges the dearest operation, so that number has to come out
    /// of the list rather than being written beside it.
    #[test]
    fn the_maximum_is_the_dearest_operation() {
        let p = pricing(vec![op("list", 0), op("send", 10_000), op("verify", 2_500)]);
        assert_eq!(max_price(&p), 10_000);
    }

    /// Every operation free is a real price list, and it admits a call that
    /// attaches nothing.
    #[test]
    fn an_all_free_list_costs_nothing() {
        assert_eq!(max_price(&pricing(vec![op("list", 0), op("read", 0)])), 0);
    }

    // ---- who may set it, and what it must look like -----------------------

    #[test]
    fn the_owner_prices_a_project_and_it_reads_back() {
        let mut c = contract_with_project();
        c.set_project_pricing(PROJECT.to_string(), pricing(vec![op("send", 10_000), op("list", 0)]));

        let read = c.get_project_pricing(PROJECT.to_string()).expect("priced");
        assert_eq!(read.author_account_id, accounts(3));
        assert_eq!(read.operations.len(), 2);
        assert_eq!(c.get_project_max_price(PROJECT.to_string()).0, 10_000);
    }

    #[test]
    fn an_unpriced_project_reads_back_as_nothing() {
        let c = contract_with_project();
        assert!(c.get_project_pricing(PROJECT.to_string()).is_none());
        assert_eq!(c.get_project_max_price(PROJECT.to_string()).0, 0);
    }

    /// The price decides what a subscription's allowance may be spent against,
    /// so setting it is ours, not the project owner's.
    #[test]
    #[should_panic(expected = "Only owner can call this method")]
    fn a_stranger_cannot_price_a_project() {
        let mut c = contract_with_project();
        testing_env!(ctx(accounts(2), NearToken::from_near(0)).build());
        c.set_project_pricing(PROJECT.to_string(), pricing(vec![op("send", 10_000)]));
    }

    /// A price on a name nobody registered would start applying the day
    /// somebody else registered it.
    #[test]
    #[should_panic(expected = "does not exist")]
    fn an_unregistered_project_cannot_be_priced() {
        let mut c = contract_with_project();
        c.set_project_pricing("nobody.near/ghost".to_string(), pricing(vec![op("send", 1)]));
    }

    /// Which of the two applies would otherwise depend on lookup order.
    #[test]
    #[should_panic(expected = "duplicate operation")]
    fn one_operation_has_one_price() {
        let mut c = contract_with_project();
        c.set_project_pricing(
            PROJECT.to_string(),
            pricing(vec![op("send", 10_000), op("send", 1)]),
        );
    }

    #[test]
    #[should_panic(expected = "exceeding 10000")]
    fn an_author_cannot_be_owed_more_than_the_whole_price() {
        let mut c = contract_with_project();
        let mut p = pricing(vec![op("send", 10_000)]);
        p.operations[0].developer_share_bp = 10_001;
        c.set_project_pricing(PROJECT.to_string(), p);
    }

    /// The share is per OPERATION, and two operations of one project may split
    /// differently.
    ///
    /// Not a shape detail: the economics of sending mail and of moving money
    /// are not the same number, so one share per project would have to be wrong
    /// for at least one operation. The testnet connector relies on it — it
    /// carries different shares per operation precisely so a run proves the
    /// share is read per operation rather than applied as a constant.
    #[test]
    fn two_operations_of_one_project_can_split_differently() {
        let mut c = contract_with_project();
        let mut p = pricing(vec![op("send", 10_000), op("verify", 15_000)]);
        p.operations[0].developer_share_bp = 7_000;
        p.operations[1].developer_share_bp = 3_333;
        c.set_project_pricing(PROJECT.to_string(), p);

        let read = c.get_project_pricing(PROJECT.to_string()).expect("priced");
        assert_eq!(read.operations[0].developer_share_bp, 7_000);
        assert_eq!(read.operations[1].developer_share_bp, 3_333);
        assert_eq!(read.author_account_id, accounts(3), "one author, whatever the split");
    }

    /// "Priced at nothing" and "not priced" are the same state, and the second
    /// spelling of it is the one that stays.
    #[test]
    #[should_panic(expected = "remove_project_pricing")]
    fn an_empty_price_list_is_not_how_a_project_is_unpriced() {
        let mut c = contract_with_project();
        c.set_project_pricing(PROJECT.to_string(), pricing(vec![]));
    }

    // ---- reading the operation -------------------------------------------

    /// The format, pinned. These vectors are the standard, and the coordinator
    /// and the worker run the same list against their own copies of the rule —
    /// three trivial parsers that must agree on every edge, not one shared
    /// function (the contract cannot depend on the crate the other two share,
    /// and it is the one that decides the money).
    #[test]
    fn the_operation_format_is_one_field_and_fails_closed() {
        use crate::payment::{operation_from_input, OperationError, MAX_PRICED_INPUT_BYTES};

        // The only accepted shape, and surrounding whitespace is trimmed.
        assert_eq!(
            operation_from_input(Some(r#"{"operation":"send"}"#)).unwrap(),
            "send"
        );
        assert_eq!(
            operation_from_input(Some(r#"{"operation":" send ","to":"a@b"}"#)).unwrap(),
            "send"
        );

        // Everything else refuses. None of these may default to an operation:
        // a defaulted operation is a defaulted PRICE, and the cheapest is the
        // one an attacker would pick.
        for (body, expected) in [
            (r#"{"to":"a@b"}"#, OperationError::Missing),
            (r#"{"operation":""}"#, OperationError::Missing),
            (r#"{"operation":"   "}"#, OperationError::Missing),
            (r#"{"operation":42}"#, OperationError::Missing),
            (r#"{"operation":null}"#, OperationError::Missing),
            (r#"{"operation":["send"]}"#, OperationError::Missing),
            // Nested does NOT count. One place, top level, or the format is a
            // convention again.
            (r#"{"input":{"operation":"send"}}"#, OperationError::Missing),
            ("not json at all", OperationError::NotJson),
            ("", OperationError::NotJson),
        ] {
            assert_eq!(
                operation_from_input(Some(body)),
                Err(expected),
                "body {body:?} must be refused"
            );
        }

        // No body at all is the same as an empty one.
        assert_eq!(operation_from_input(None), Err(OperationError::NotJson));

        // `op` is NOT accepted. The old near-email spelling is not the
        // standard, and silently honouring it would leave two field names in
        // circulation forever.
        assert_eq!(
            operation_from_input(Some(r#"{"op":"send"}"#)),
            Err(OperationError::Missing)
        );

        // Oversized is refused before it is parsed: parsing is linear in the
        // body and the caller's gas pays for it.
        let huge = format!(
            r#"{{"operation":"send","pad":"{}"}}"#,
            "x".repeat(MAX_PRICED_INPUT_BYTES)
        );
        assert!(matches!(
            operation_from_input(Some(&huge)),
            Err(OperationError::TooLarge { .. })
        ));
    }

    // ---- splitting what was paid ------------------------------------------

    /// The author's cut and the owner's are one decision, and they must add up.
    ///
    /// Returned together for that reason: computed apart, they can be derived
    /// from different numbers and quietly stop summing to what the caller
    /// actually paid — a shortfall nobody notices and an overpayment nobody can
    /// fund.
    #[test]
    fn a_payment_is_split_and_the_halves_add_up() {
        use crate::payment::{split_payment, Split};

        // A dollar at 40%: forty cents to the author, sixty to us.
        assert_eq!(
            split_payment(1_000_000, 4_000),
            Split { author_usd: 400_000, owner_usd: 600_000 }
        );

        // The two ends.
        assert_eq!(
            split_payment(10_000, 0),
            Split { author_usd: 0, owner_usd: 10_000 },
            "our own connectors keep the whole fee"
        );
        assert_eq!(
            split_payment(10_000, 10_000),
            Split { author_usd: 10_000, owner_usd: 0 }
        );

        // Rounding goes to the OWNER. A split that paid out more than came in
        // is a balance nobody can explain.
        let odd = split_payment(15_000, 3_333);
        assert_eq!(odd, Split { author_usd: 4_999, owner_usd: 10_001 });
        assert_eq!(odd.author_usd + odd.owner_usd, 15_000);

        // A share above 100% cannot make the author's half exceed the payment,
        // whatever a row says.
        assert_eq!(
            split_payment(10_000, 20_000),
            Split { author_usd: 10_000, owner_usd: 0 }
        );

        // The invariant, over a spread of values.
        for paid in [0u128, 1, 3, 999, 10_000, 15_000, 1_000_000] {
            for bp in [0u16, 1, 3_333, 4_000, 7_000, 9_999, 10_000] {
                let s = split_payment(paid, bp);
                assert_eq!(
                    s.author_usd + s.owner_usd,
                    paid,
                    "split of {paid} at {bp}bp must add up"
                );
                assert!(s.author_usd <= paid);
            }
        }
    }

    /// Change goes back to the caller, and only the price is ever split.
    ///
    /// Two refunds meet in one settlement and they answer to different people:
    /// the change is the caller's because they attached it, and the guest's
    /// `refund_usd` is the module's judgement about the work it did. Mixing
    /// them lets a module hand back money it was never paid.
    #[test]
    fn change_comes_back_and_only_the_price_is_split() {
        use crate::payment::settle_attached;

        // Exactly the price: nothing to return, everything to the developers.
        assert_eq!(settle_attached(10_000, 10_000, 0), (0, 10_000));

        // Over-attached: the difference comes back and the split is on the
        // PRICE, not on what was sent.
        assert_eq!(settle_attached(15_000, 10_000, 0), (5_000, 10_000));

        // The guest hands back part of the price as well — both refunds add.
        assert_eq!(settle_attached(15_000, 10_000, 4_000), (9_000, 6_000));

        // A guest cannot reach past the price into the caller's change. Asked
        // for more than the price, it returns the price and no more — the
        // 5_000 of change is returned because it was the caller's, not because
        // the guest gave it away.
        assert_eq!(settle_attached(15_000, 10_000, 99_999), (15_000, 0));

        // A free operation: whatever was attached is change.
        assert_eq!(settle_attached(7_000, 0, 0), (7_000, 0));

        // An UNPRICED project keeps the old meaning — the caller's attachment
        // IS the payment — which is what the caller passes as the price there.
        assert_eq!(settle_attached(3_000, 3_000, 1_000), (1_000, 2_000));

        // A price ABOVE what was attached. Admission cannot produce this, but
        // settlement can: the price is re-read from the table in the callback,
        // so an operation that got dearer while the execution was in flight
        // arrives here costing more than the caller sent. There is nothing to
        // return and nothing to subtract — every unit is chargeable, and the
        // subtraction that would go negative is the one this must never do.
        assert_eq!(settle_attached(10_000, 15_000, 0), (0, 10_000));

        // The same, with the guest also asking for money back: it may still
        // reach into what was attached, because all of it is the price now.
        assert_eq!(settle_attached(10_000, 15_000, 4_000), (4_000, 6_000));

        // Nothing is created or destroyed, over a spread. The price is NOT
        // clamped to what was attached — the case above is reachable, and
        // clamping it away here would be testing an assumption instead of the
        // function.
        for attached in [0u128, 1, 999, 10_000, 15_000, 1_000_000] {
            for price in [0u128, 1, 9_999, 10_000, 1_000_001, u128::MAX] {
                for guest in [0u128, 1, 5_000, u128::MAX / 2, u128::MAX] {
                    let (back, developers) = settle_attached(attached, price, guest);
                    assert_eq!(
                        back + developers,
                        attached,
                        "settling {attached} at price {price} with a {guest} guest refund must add up"
                    );
                    // Stated separately from the sum, because it is the
                    // question that matters: the contract can never hand back
                    // more than it was given, whatever a price row or a guest
                    // asks for.
                    assert!(
                        back <= attached,
                        "returned {back} of an attached {attached} at price {price}"
                    );
                }
            }
        }
    }

    // ---- admission --------------------------------------------------------

    /// The gate follows the PROJECT. A project with no price behaves exactly as
    /// before — no operation required, nothing attached.
    #[test]
    fn an_unpriced_project_is_callable_with_nothing_attached() {
        let mut c = contract_with_project();
        testing_env!(ctx(accounts(1), NearToken::from_near(1)).build());
        call_body(&mut c, None, None);
    }

    #[test]
    #[should_panic(expected = "attach at least that")]
    fn a_priced_operation_refuses_a_call_that_attaches_nothing() {
        let mut c = contract_with_project();
        c.set_project_pricing(PROJECT.to_string(), pricing(vec![op("send", 10_000), op("list", 0)]));

        testing_env!(ctx(accounts(1), NearToken::from_near(1)).build());
        call_op(&mut c, "send", None);
    }

    /// The price is the OPERATION's, so a free one is free — the whole reason
    /// the operation is read here rather than charging the dearest.
    #[test]
    fn a_free_operation_costs_nothing_on_chain() {
        let mut c = contract_with_project();
        c.set_project_pricing(PROJECT.to_string(), pricing(vec![op("send", 10_000), op("list", 0)]));

        testing_env!(ctx(accounts(1), NearToken::from_near(1)).build());
        call_op(&mut c, "list", None);
    }

    /// At least the price, and never less.
    ///
    /// Under-attaching is refused because the operation has a price and this is
    /// where it is taken.
    #[test]
    #[should_panic(expected = "attach at least that")]
    fn under_attaching_is_refused() {
        let mut c = contract_with_project();
        c.set_project_pricing(PROJECT.to_string(), pricing(vec![op("send", 10_000)]));

        testing_env!(ctx(accounts(1), NearToken::from_near(1)).build());
        c.user_stablecoin_balances.insert(&accounts(1), &1_000_000);
        call_op(&mut c, "send", Some(9_999));
    }

    /// Over-attaching is ALLOWED, and the excess comes back at settlement.
    ///
    /// It used to be refused, on the grounds that an exact amount removes the
    /// question of who returns the difference. The answer turned out to be
    /// already written: the callback re-reads the same `operation` to find the
    /// author's share, so it knows the price too, and the refund path it would
    /// use — a credit to the caller's stablecoin balance — is the one a guest's
    /// `refund_usd` already takes. Nothing new is looked up and no second copy
    /// of the price exists.
    ///
    /// What it buys: a caller no longer has to read the price list before every
    /// call, nor get it wrong when a price moves between the reading and the
    /// call.
    #[test]
    fn over_attaching_is_accepted_and_the_excess_is_taken_for_now() {
        let mut c = contract_with_project();
        c.set_project_pricing(PROJECT.to_string(), pricing(vec![op("send", 10_000)]));

        testing_env!(ctx(accounts(1), NearToken::from_near(1)).build());
        c.user_stablecoin_balances.insert(&accounts(1), &1_000_000);
        call_op(&mut c, "send", Some(15_000));

        // Admission takes what was attached; the callback is what hands the
        // difference back, and it has not run here.
        assert_eq!(
            c.user_stablecoin_balances.get(&accounts(1)).unwrap(),
            985_000,
            "admission takes what was attached, and settlement returns the change",
        );
    }

    #[test]
    fn attaching_the_operations_price_gets_in() {
        let mut c = contract_with_project();
        c.set_project_pricing(PROJECT.to_string(), pricing(vec![op("send", 10_000), op("list", 0)]));

        testing_env!(ctx(accounts(1), NearToken::from_near(1)).build());
        c.user_stablecoin_balances.insert(&accounts(1), &1_000_000);
        call_op(&mut c, "send", Some(10_000));

        // And it was taken, not merely checked.
        assert_eq!(c.user_stablecoin_balances.get(&accounts(1)).unwrap(), 990_000);
    }

    /// An operation with no row is refused, never run for free. Otherwise
    /// naming an operation we never priced is how a caller runs our workers for
    /// nothing.
    #[test]
    #[should_panic(expected = "does not sell operation 'exfiltrate'")]
    fn an_unlisted_operation_is_refused_rather_than_free() {
        let mut c = contract_with_project();
        c.set_project_pricing(PROJECT.to_string(), pricing(vec![op("send", 10_000)]));

        testing_env!(ctx(accounts(1), NearToken::from_near(1)).build());
        call_op(&mut c, "exfiltrate", None);
    }

    /// A priced project called with no operation at all is refused before
    /// anything is charged or run.
    #[test]
    // Matched without the quoted field name: `env::panic_str` reaches the test
    // harness inside a Debug rendering, where the quotes are escaped.
    #[should_panic(expected = "is priced per operation")]
    fn a_priced_project_refuses_a_body_that_names_no_operation() {
        let mut c = contract_with_project();
        c.set_project_pricing(PROJECT.to_string(), pricing(vec![op("send", 10_000)]));

        testing_env!(ctx(accounts(1), NearToken::from_near(1)).build());
        call_body(&mut c, Some(r#"{"to":"a@b"}"#.to_string()), None);
    }

    /// Compile-only compiles and stops: no operation happens, so none is
    /// required and nothing is charged.
    #[test]
    fn compile_only_needs_no_operation() {
        let mut c = contract_with_project();
        c.set_project_pricing(PROJECT.to_string(), pricing(vec![op("send", 10_000)]));

        testing_env!(ctx(accounts(1), NearToken::from_near(1)).build());
        c.request_execution(
            ExecutionSource::Project {
                project_id: PROJECT.to_string(),
                version_key: None,
            },
            None, // no limits => compile-only
            None,
            None,
            None,
            None,
            None,
        );
    }

    /// Unpricing puts the project back where it started.
    #[test]
    fn removing_the_price_makes_the_project_free_again() {
        let mut c = contract_with_project();
        c.set_project_pricing(PROJECT.to_string(), pricing(vec![op("send", 10_000)]));
        c.remove_project_pricing(PROJECT.to_string());

        assert!(c.get_project_pricing(PROJECT.to_string()).is_none());
        testing_env!(ctx(accounts(1), NearToken::from_near(1)).build());
        call_body(&mut c, None, None);
    }

    /// The list the coordinator syncs prices from.
    ///
    /// Untested until now, and it is not a cosmetic view: a priced project this
    /// omits is one the coordinator never learns a price for, so it is charged
    /// on chain and given away over HTTPS. The bounds are the whole content —
    /// an unpaginated read stops answering once the table outgrows the gas
    /// ceiling, which fails in exactly the same direction.
    #[test]
    fn the_priced_project_list_pages_and_is_bounded() {
        let mut c = contract_with_project();
        c.set_project_pricing(PROJECT.to_string(), pricing(vec![op("send", 10_000)]));

        assert_eq!(c.get_priced_project_count(), 1);
        assert_eq!(c.get_priced_projects(None, None), vec![PROJECT.to_string()]);

        // Past the end is empty, not a panic: a caller sizing pages off the
        // count will ask for the page after the last one.
        assert!(c.get_priced_projects(Some(1), None).is_empty());
        assert!(c.get_priced_projects(Some(99), Some(10)).is_empty());

        // A caller naming no limit gets a page rather than the table, and one
        // naming a huge limit is capped instead of obeyed.
        assert_eq!(c.get_priced_projects(None, Some(0)).len(), 0);
        assert_eq!(c.get_priced_projects(None, Some(u64::MAX)).len(), 1);

        // Unpricing removes it from the list as well as from the lookup, so a
        // sync cannot keep charging for a price that was withdrawn.
        c.remove_project_pricing(PROJECT.to_string());
        assert_eq!(c.get_priced_project_count(), 0);
        assert!(c.get_priced_projects(None, None).is_empty());
    }

    // ---- settlement --------------------------------------------------------

    /// Run the callback over a request admission created.
    ///
    /// Admission only takes the money; every question about where it ENDS UP —
    /// change, the author's share, the refunds — is answered in the callback,
    /// so a test that stops at admission proves nothing about any of them.
    fn settle(c: &mut Contract, request_id: u64, guest_refund: Option<u64>) {
        let request = c
            .pending_requests
            .get(&request_id)
            .expect("nothing pending under that id");

        testing_env!(ctx(accounts(0), NearToken::from_near(0)).build());
        c.on_execution_response(
            request_id,
            request.sender_id.clone(),
            request.resolved_source.clone(),
            request.resource_limits.clone(),
            U128(request.payment),
            Ok(ExecutionResponse {
                success: true,
                output: None,
                error: None,
                resources_used: ResourceMetrics {
                    instructions: 0,
                    time_ms: 0,
                    compile_time_ms: None,
                },
                compilation_note: None,
                refund_usd: guest_refund,
            }),
        );
    }

    /// The whole money story of one over-attached call, end to end.
    ///
    /// Admission takes what was sent, settlement returns the difference, and
    /// the split is computed on the PRICE. Held here rather than only in
    /// `settle_attached` because the pure function cannot show which of the two
    /// numbers the callback actually passes it.
    #[test]
    fn settling_returns_the_change_and_splits_only_the_price() {
        let mut c = contract_with_project();
        c.set_project_pricing(PROJECT.to_string(), pricing(vec![op("send", 10_000)]));

        testing_env!(ctx(accounts(1), NearToken::from_near(1)).build());
        c.user_stablecoin_balances.insert(&accounts(1), &1_000_000);
        call_op(&mut c, "send", Some(15_000));
        settle(&mut c, 0, None);

        assert_eq!(
            c.user_stablecoin_balances.get(&accounts(1)).unwrap(),
            990_000,
            "the caller pays the price, not what they attached"
        );
        // 7_000bp of the PRICE, not of the 15_000 that was sent.
        assert_eq!(c.developer_earnings.get(&accounts(3)).unwrap(), 7_000);
        assert_eq!(c.developer_earnings.get(&accounts(0)).unwrap(), 3_000);
    }

    /// Deleting a project mid-flight does not swallow the caller's money.
    ///
    /// The pricing row and the project are separate pieces of state, so an
    /// execution can arrive at settlement priced, chargeable, and with nobody
    /// left to credit. Keeping the money then would leave tokens on the
    /// contract that no balance and no earnings row accounts for — invisible,
    /// because nothing anyone can read would say they are missing.
    #[test]
    fn money_for_a_deleted_project_goes_back_to_the_caller() {
        let mut c = contract_with_project();
        c.set_project_pricing(PROJECT.to_string(), pricing(vec![op("send", 10_000)]));

        testing_env!(ctx(accounts(1), NearToken::from_near(1)).build());
        c.user_stablecoin_balances.insert(&accounts(1), &1_000_000);
        call_op(&mut c, "send", Some(10_000));
        assert_eq!(c.user_stablecoin_balances.get(&accounts(1)).unwrap(), 990_000);

        // The project goes away while the execution is in flight. Removed from
        // the map directly, the same way `contract_with_project` writes it:
        // `delete_project` derives the id from the caller, and this fixture's
        // project lives under a namespace that no test account can be.
        c.projects.remove(&PROJECT.to_string());

        settle(&mut c, 0, None);

        assert_eq!(
            c.user_stablecoin_balances.get(&accounts(1)).unwrap(),
            1_000_000,
            "with nobody to credit, the money is the caller's again"
        );
        assert!(
            c.developer_earnings.get(&accounts(3)).is_none(),
            "the author is not paid for a project that no longer exists"
        );
        assert!(c.developer_earnings.get(&accounts(0)).is_none());
    }
}

mod subscription_sale_tests {
    use crate::payment::{plan_for_sale, SubscriptionPlan};
    use crate::*;
    use near_sdk::json_types::U128;

    fn plan(index: u8, name: &str, price_usd: u128, active: bool) -> SubscriptionPlan {
        SubscriptionPlan {
            index,
            name: name.to_string(),
            price_usd: U128(price_usd),
            active,
        }
    }

    fn list() -> Vec<SubscriptionPlan> {
        vec![
            plan(0, "starter", 10_000_000, true),
            plan(1, "pro", 50_000_000, true),
            plan(2, "promo-nov", 5_000_000, false),
        ]
    }

    #[test]
    fn the_exact_price_buys_the_named_plan() {
        let plans = list();
        let sold = plan_for_sale(&plans, 0, 10_000_000).expect("exact payment buys it");
        assert_eq!(sold.name, "starter");
    }

    /// Overpaying buys the plan that was NAMED — not a bigger one from the
    /// catalogue. What the extra money buys is more of this plan, and that is
    /// worked out by the coordinator from the amount the event carries.
    ///
    /// The previous version of this test asserted `60_000_000 - price ==
    /// 50_000_000` and called it "the rest is change". That is a subtraction of
    /// two numbers: it never touched `ft_on_transfer`, never looked at what is
    /// returned to the payer, and would have passed whatever the contract did
    /// with the money. It sat next to a doc-comment promising change while the
    /// code kept everything, and said nothing.
    #[test]
    fn overpaying_still_buys_the_plan_that_was_named() {
        let plans = list();

        let sold = plan_for_sale(&plans, 0, 60_000_000).expect("more than enough");
        assert_eq!(sold.name, "starter", "the plan the payer asked for");
        assert_eq!(
            sold.index, 0,
            "and not the dearest one the money would have covered"
        );

        // Paying exactly buys it too — the boundary, since `plan_for_sale`
        // filters on `amount >= price`.
        assert!(plan_for_sale(&plans, 0, sold.price_usd.0).is_some());
        assert!(
            plan_for_sale(&plans, 0, sold.price_usd.0 - 1).is_none(),
            "a unit short buys nothing and is returned whole"
        );
    }

    /// A payment short of the named plan buys NOTHING and is returned whole.
    ///
    /// Deliberately unlike the coordinator's purchase endpoint, which sells the
    /// best plan the money covers. An API caller can be told and try again; a
    /// transfer that has landed can only be answered with the product or the
    /// money, and substituting a cheaper plan would be choosing for the payer.
    #[test]
    fn short_of_the_named_plan_buys_nothing() {
        assert!(plan_for_sale(&list(), 1, 10_000_000).is_none());
        assert!(plan_for_sale(&list(), 0, 9_999_999).is_none());
    }

    #[test]
    fn a_withdrawn_plan_cannot_be_bought_at_any_price() {
        assert!(plan_for_sale(&list(), 2, 5_000_000).is_none());
        assert!(plan_for_sale(&list(), 2, 1_000_000_000).is_none());
    }

    #[test]
    fn an_unknown_index_buys_nothing() {
        assert!(plan_for_sale(&list(), 9, 1_000_000_000).is_none());
    }

    /// A deployment with no price list sells nothing, however much arrives.
    #[test]
    fn an_empty_price_list_sells_nothing() {
        assert!(plan_for_sale(&[], 0, 1_000_000_000).is_none());
    }
}

mod subscription_purchase_path_tests {
    use crate::payment::{FtTransferAction, SubscriptionPlan};
    use crate::*;
    use near_sdk::test_utils::{accounts, get_logs, VMContextBuilder};
    use near_sdk::{testing_env, NearToken};

    const TOKEN: &str = "usdc.test.near";

    fn ctx(predecessor: AccountId) -> VMContextBuilder {
        let mut b = VMContextBuilder::new();
        b.predecessor_account_id(predecessor)
            .attached_deposit(NearToken::from_near(0));
        b
    }

    fn token() -> AccountId {
        TOKEN.parse().unwrap()
    }

    /// A contract that sells `starter` at $10 and has a payment key to sell it
    /// for, when `with_key` is set.
    fn shop(with_key: bool) -> Contract {
        testing_env!(ctx(accounts(0)).build());
        let mut c = Contract::new(accounts(0), Some(accounts(0)), None, None);
        c.set_payment_token_contract(Some(token()));
        c.set_subscription_plans(vec![SubscriptionPlan {
            index: 0,
            name: "starter".to_string(),
            price_usd: U128(10_000_000),
            active: true,
        }]);

        if with_key {
            let blob = "encrypted".to_string();
            let mut buyer = ctx(accounts(1));
            testing_env!(buyer.build());
            let cost = c.estimate_storage_cost(
                SecretAccessor::System(SystemSecretType::PaymentKey),
                "1".to_string(),
                accounts(1),
                blob.clone(),
                types::AccessCondition::AllowAll,
                None,
            );
            testing_env!(buyer
                .attached_deposit(NearToken::from_yoctonear(cost.0))
                .build());
            c.store_secrets(
                SecretAccessor::System(SystemSecretType::PaymentKey),
                "1".to_string(),
                blob,
                types::AccessCondition::AllowAll,
                None,
            );
        }
        c
    }

    fn buy(c: &mut Contract, amount: u128, plan: u8) {
        testing_env!(ctx(token()).build());
        let msg = serde_json::to_string(&FtTransferAction::BuySubscription {
            nonce: 1,
            owner: Some(accounts(1)),
            plan,
        })
        .unwrap();
        c.ft_on_transfer(accounts(2), U128(amount), msg);
    }

    /// The whole point of the on-chain path: a purchase emits an EVENT and
    /// touches no encrypted blob. No yield, no worker, no keystore — which is
    /// why this is simpler than a top-up rather than a variation on it.
    #[test]
    fn a_paid_purchase_emits_the_event() {
        let mut c = shop(true);
        buy(&mut c, 10_000_000, 0);

        let logs = get_logs();
        let event = logs
            .iter()
            .find(|l| l.contains("SubscriptionPurchased"))
            .expect("a sale must tell the coordinator what was sold");

        assert!(event.contains("\"plan\":0"));
        assert!(event.contains("\"paid_usd\":\"10000000\""));
        // The payer is recorded separately from the owner: one wallet paying
        // for somebody else's agent is the ordinary case.
        assert!(event.contains(accounts(2).as_str()), "the payer is named");
        assert!(event.contains(accounts(1).as_str()), "so is whose key it is");
    }

    /// The event carries what was PAID, not what the plan costs.
    ///
    /// They are the same number at exact payment, which is why the test above
    /// cannot tell the two apart — and for a while they were not the same, and
    /// nothing said so: the event reported the plan's price while the contract
    /// kept the whole transfer, so an overpayment vanished. The coordinator
    /// scales the terms by this figure, so it decides what the customer gets.
    #[test]
    fn the_event_reports_the_amount_transferred_not_the_price() {
        let mut c = shop(true);
        buy(&mut c, 60_000_000, 0); // six times the $10 plan

        let logs = get_logs();
        let event = logs
            .iter()
            .find(|l| l.contains("SubscriptionPurchased"))
            .expect("a sale must tell the coordinator what was sold");

        assert!(
            event.contains("\"paid_usd\":\"60000000\""),
            "the event must carry the transfer, not the price: {event}"
        );
        assert!(
            !event.contains("\"paid_usd\":\"10000000\""),
            "reporting the price here is how the difference used to disappear"
        );
        assert!(
            event.contains("\"plan\":0"),
            "and it is still the plan the payer named, not a dearer one"
        );
    }

    /// Money for a key that does not exist buys nothing.
    ///
    /// The panic IS the refund: NEP-141 returns the full amount when
    /// `ft_on_transfer` fails, so refusing this way needs no arithmetic and
    /// cannot end in keeping somebody's money for nothing. The payer reads the
    /// reason in their own transaction outcome.
    #[test]
    #[should_panic(expected = "No payment key")]
    fn a_purchase_for_a_missing_key_is_refused() {
        let mut c = shop(false);
        buy(&mut c, 10_000_000, 0);
    }

    /// A subscription cannot be bought for a TRIAL key, and the reason is
    /// structural rather than a rule written here.
    ///
    /// A trial lives at nonce 0, in the coordinator's database only — it has no
    /// on-chain record, which is why `store_secrets` refuses to create one
    /// there at all (see `secrets_tests`). So the key this purchase requires
    /// can never exist, and the money goes back.
    ///
    /// Worth its own test because it is the obvious thing a trial user does
    /// next — "I like this, let me buy the subscription for my key" — and the
    /// answer has to be the whole payment back rather than a subscription
    /// granted onto a row that means something else. The route up from a trial
    /// is to create a real key first.
    #[test]
    #[should_panic(expected = "No payment key")]
    fn a_subscription_cannot_be_bought_for_a_trial_key() {
        let mut c = shop(true);

        // Same shop, same money, same live plan — the only difference is the
        // nonce, so nothing else can be what refuses it.
        testing_env!(ctx(token()).build());
        let msg = serde_json::to_string(&FtTransferAction::BuySubscription {
            nonce: 0,
            owner: Some(accounts(1)),
            plan: 0,
        })
        .unwrap();
        c.ft_on_transfer(accounts(2), U128(10_000_000), msg);
    }

    /// And nothing was told to the coordinator on the way out — a refusal that
    /// emitted the event would have the allowance granted for a payment that
    /// was handed straight back.
    #[test]
    fn a_refused_purchase_tells_the_coordinator_nothing() {
        let mut c = shop(false);
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            buy(&mut c, 10_000_000, 0);
        }));
        assert!(result.is_err(), "a missing key must refuse");
        assert!(
            !get_logs().iter().any(|l| l.contains("SubscriptionPurchased")),
            "a refusal must not announce a sale"
        );
    }

    #[test]
    #[should_panic(expected = "No plan 0 on sale")]
    fn a_payment_short_of_the_plan_sells_nothing() {
        let mut c = shop(true);
        buy(&mut c, 9_999_999, 0);
    }

    #[test]
    #[should_panic(expected = "No plan 9 on sale")]
    fn an_unknown_plan_sells_nothing() {
        let mut c = shop(true);
        buy(&mut c, 10_000_000, 9);
    }

    /// A subscription is NOT a top-up: the money never becomes the key's
    /// balance, so it can never be withdrawn back out. Nothing in this path
    /// writes a balance anywhere, and this is what pins that.
    #[test]
    fn a_purchase_leaves_no_balance_anywhere() {
        let mut c = shop(true);
        buy(&mut c, 10_000_000, 0);

        assert_eq!(
            c.get_user_stablecoin_balance(accounts(1)).0,
            0,
            "a subscription must not land in a spendable balance"
        );
        assert_eq!(c.get_user_stablecoin_balance(accounts(2)).0, 0);
    }
}

mod state_compatibility_tests {
    use crate::*;
    use near_sdk::borsh;

    /// `SecretAccessor` is borsh-encoded inside `SecretKey`, which is a storage
    /// KEY. Change a variant's ordinal and every secret ever stored moves to a
    /// key nobody looks up: every payment key reads as "not found", and the
    /// secrets are unreachable rather than merely mis-typed.
    ///
    /// New variants go at the END. Always.
    #[test]
    fn secret_accessor_variant_ordinals_are_frozen() {
        let ordinal = |a: &SecretAccessor| borsh::to_vec(a).unwrap()[0];

        assert_eq!(
            ordinal(&SecretAccessor::Repo {
                repo: "github.com/a/b".to_string(),
                branch: None
            }),
            0
        );
        assert_eq!(
            ordinal(&SecretAccessor::WasmHash {
                hash: "aa".to_string()
            }),
            1
        );
        assert_eq!(
            ordinal(&SecretAccessor::Project {
                project_id: "a.near/b".to_string()
            }),
            2
        );
        // The one that matters most: every payment key on mainnet is stored
        // under this ordinal.
        assert_eq!(
            ordinal(&SecretAccessor::System(SystemSecretType::PaymentKey)),
            3,
            "moving System renames the storage key of every payment key in existence"
        );
    }

    /// `SystemSecretType` is borsh-encoded INSIDE `SecretAccessor::System`, so
    /// it is part of the same storage key. Inserting a variant before
    /// `PaymentKey` renumbers it and moves every payment key ever stored.
    ///
    /// New variants go at the END. Always.
    #[test]
    fn system_secret_type_variant_ordinals_are_frozen() {
        let ordinal = |t: &SystemSecretType| borsh::to_vec(t).unwrap()[0];

        assert_eq!(
            ordinal(&SystemSecretType::PaymentKey),
            0,
            "moving PaymentKey renames the storage key of every payment key in existence"
        );
    }

    /// `VersionInfo` is the borsh-encoded VALUE of `project_versions`. Appending
    /// a field — even an `Option`, which is one byte and looks harmless — leaves
    /// every entry written by an earlier contract one byte short, and borsh
    /// answers a short buffer with an error, not a default. Reading any existing
    /// project's versions would then panic.
    ///
    /// If this test fails, a field was added: move it to a side map instead.
    #[test]
    fn version_info_byte_layout_is_frozen() {
        let info = VersionInfo {
            source: CodeSource::WasmUrl {
                url: "https://x/y".to_string(),
                hash: "ab".to_string(),
                build_target: None,
            },
            added_at: 1,
            storage_deposit: 2,
        };

        let encoded = borsh::to_vec(&info).unwrap();

        // Built by hand from the field order, so a new field changes the length
        // and this test says so.
        let expected_len = 1                       // CodeSource variant (WasmUrl)
            + 4 + "https://x/y".len()              // url
            + 4 + "ab".len()                       // hash
            + 1                                    // build_target: None
            + 8                                    // added_at: u64
            + 16; // storage_deposit: u128
        assert_eq!(
            encoded.len(),
            expected_len,
            "VersionInfo grew a field — every version already on chain becomes undecodable"
        );

        let decoded: VersionInfo = borsh::from_slice(&encoded).unwrap();
        assert_eq!(decoded.added_at, 1);
        assert_eq!(decoded.storage_deposit, 2);
    }
}

/// The `ft_on_transfer` msg format.
#[cfg(test)]
mod ft_transfer_action_tests {
    use crate::payment::FtTransferAction;

    /// The existing actions must keep parsing byte-for-byte as before: every
    /// dashboard top-up and every deposit in flight uses them.
    #[test]
    fn the_older_actions_still_parse() {
        let top_up: FtTransferAction = near_sdk::serde_json::from_str(
            r#"{"action":"top_up_payment_key","nonce":3,"owner":"alice.near"}"#,
        )
        .unwrap();
        assert!(matches!(
            top_up,
            FtTransferAction::TopUpPaymentKey { nonce: 3, .. }
        ));

        let deposit: FtTransferAction =
            near_sdk::serde_json::from_str(r#"{"action":"deposit_balance"}"#).unwrap();
        assert!(matches!(deposit, FtTransferAction::DepositBalance));
    }
}

/// Storing a secret OWNED BY A WALLET while somebody else pays for it.
///
/// This is what lets an agent hold a credential without ever holding NEAR, and
/// what stops a stranger from planting one under an agent's public account. Both
/// halves are load-bearing, so both are tested against real signatures — a test
/// that fabricated one would be testing the test.
#[cfg(test)]
mod store_agent_secret_tests {
    use super::*;
    use ed25519_dalek::{Signer, SigningKey};
    use near_sdk::serde_json::json;
    use sha2::{Digest, Sha256};

    fn wallet() -> (SigningKey, String, AccountId) {
        // Fixed bytes rather than random: a failing test must fail the same way
        // twice, and nothing here depends on the key being unpredictable.
        let signing = SigningKey::from_bytes(&[7u8; 32]);
        let pubkey_hex = hex::encode(signing.verifying_key().to_bytes());
        let account: AccountId = pubkey_hex.parse().unwrap();
        (signing, format!("ed25519:{}", pubkey_hex), account)
    }

    /// The `Repo` accessor these tests use, as the contract renders it into the
    /// signed message.
    /// What this door takes: a PROJECT.
    ///
    /// These fixtures were `Repo` accessors, which `store_agent_secret` now
    /// refuses — a secret for an agent is sealed to `project:{id}:{agent}`, and
    /// a repository has no seed to be sealed to. A fixture using a shape the
    /// method rejects is a fixture nobody can copy.
    fn repo_accessor() -> SecretAccessor {
        SecretAccessor::Project {
            project_id: "author.near/connector".to_string(),
        }
    }

    /// The message the contract rebuilds. `binding` is the accessor's
    /// rendering — passed in rather than assumed, because a signature is
    /// specific to the accessor and a test that hid that would not be testing
    /// the thing.
    ///
    /// Built by the PRODUCTION function, not by a `format!` copied here. A test
    /// that rebuilt the message itself would sign whatever it believed the
    /// format to be and keep passing while the real one drifted — which is a
    /// signature covering less than it claims, held up by a test that cannot
    /// see it.
    fn sign(
        signing: &SigningKey,
        wallet_pubkey: &str,
        accessor: &SecretAccessor,
        profile: &str,
        data: &str,
        payer: &AccountId,
    ) -> String {
        let message = crate::secrets::secret_store_message(
            wallet_pubkey,
            accessor,
            profile,
            data,
            payer,
            None,
            &types::AccessCondition::AllowAll,
        );
        let mut hasher = Sha256::new();
        hasher.update(message.as_bytes());
        let hash: [u8; 32] = hasher.finalize().into();
        hex::encode(signing.sign(&hash).to_bytes())
    }

    /// One signature must name exactly one tuple — and the accessor may hold
    /// colons again.
    ///
    /// The fields are colon-joined, so a colon inside a variable field once
    /// moved a boundary: a signature over `(branch "main", profile "x:y")`
    /// produced byte-identical output to `(branch "main:x", profile "y")`, and
    /// the payer — who submits the call and did not sign it — could land the
    /// store under a different accessor of the same owner.
    ///
    /// The accessor is LENGTH-PREFIXED now, so its content cannot move
    /// anything, and the shapes that were briefly refused work again:
    /// `git@github.com:owner/repo` is an ordinary way to write a repository.
    /// `profile` is still required to be colon-free — it is the one variable
    /// field with no length in front, and on the signed path it is derived
    /// rather than supplied.
    #[test]
    fn the_length_prefix_lets_an_accessor_hold_colons_and_still_name_one_tuple() {
        use crate::secrets::{assert_profile_is_unambiguous, secret_store_message};

        let msg = |repo: &str, branch: &str, profile: &str| {
            secret_store_message(
                "ed25519:ab",
                &SecretAccessor::Repo {
                    repo: repo.to_string(),
                    branch: Some(branch.to_string()),
                },
                profile,
                "cipher",
                &"payer.near".parse().unwrap(),
                None,
                &types::AccessCondition::AllowAll,
            )
        };

        // The collision that started this. Same bytes before the prefix; two
        // different strings now, because the accessors are different lengths.
        assert_ne!(
            msg("github.com/o/r", "main", "x:y"),
            msg("github.com/o/r", "main:x", "y"),
            "a colon traded between accessor and profile must not produce one string",
        );

        // Every git spelling somebody might use makes its OWN message — which
        // is the claim worth testing, and the only one the length prefix is
        // responsible for. Asserting a constant four times, as this once did,
        // exercised the spellings not at all.
        let spellings = [
            "git@github.com:owner/repo",
            "https://github.com/owner/repo",
            "git.company.com:8443/owner/repo",
            "github.com/owner/repo",
        ];
        for (i, a) in spellings.iter().enumerate() {
            for b in spellings.iter().skip(i + 1) {
                assert_ne!(
                    msg(a, "main", "prod"),
                    msg(b, "main", "prod"),
                    "two different repositories must not sign the same message",
                );
            }
        }

        // And two accessors that differ only by where a colon sits still make
        // two different messages — the length is what says so.
        assert_ne!(
            msg("git@github.com:o/r", "main", "prod"),
            msg("git@github.com:o", "r:main", "prod"),
        );

        // The profile is the field still policed.
        assert!(std::panic::catch_unwind(|| {
            assert_profile_is_unambiguous("x:y")
        })
        .is_err(), "a colon in the profile must still be refused");
    }

    /// What the one remaining ban costs, measured rather than hoped for.
    ///
    /// Only `profile` is policed now, and a rule that refuses input is only as
    /// good as the list of real input it does NOT refuse. Every value here is a
    /// shape somebody uses; if one starts failing, a legitimate secret has
    /// become unstorable and whoever hits it will have no idea why.
    #[test]
    fn the_profile_rule_does_not_touch_anything_legitimate() {
        use crate::secrets::assert_profile_is_unambiguous;

        let agent = "a1b2c3d4".repeat(8);

        for profile in [
            // What the signed path actually passes.
            agent.as_str(),
            // What humans type on the paths that share this rule.
            "production", "default", "staging", "dev", "test-1",
            "prod-eu-west-1", "my_profile", "app.prod", "v1.2.3", "7", "",
            // Non-ASCII is not the rule's business.
            "производство", "本番",
        ] {
            assert_profile_is_unambiguous(profile);
        }
    }

    /// The refusal has to tell the holder of the credential what to change.
    ///
    /// Whoever hits it is mid-way through storing a secret, and "invalid field"
    /// would send them re-encrypting something that was never the problem.
    #[test]
    fn the_profile_refusal_names_the_field_the_value_and_the_way_out() {
        use crate::secrets::assert_profile_is_unambiguous;

        let err = std::panic::catch_unwind(|| {
            assert_profile_is_unambiguous("x:y")
        })
        .expect_err("a colon in the profile must be refused");
        let m = err
            .downcast_ref::<String>()
            .cloned()
            .or_else(|| err.downcast_ref::<&str>().map(|s| s.to_string()))
            .expect("a panic message");

        assert!(m.contains("profile must not contain ':'"), "names the field: {m}");
        assert!(m.contains("x:y"), "shows the value: {m}");
        assert!(m.contains("64-character account"), "says what a profile is: {m}");
        assert!(m.contains("production"), "and gives an example: {m}");
    }

    /// A wasm hash is accepted; GitHub is not.
    ///
    /// The hash is NOT checked against anything, deliberately: there is nothing
    /// to check a commitment against, and a secret sealed to bytes that have
    /// not run yet is simply a secret waiting for them.
    #[test]
    fn a_wasm_hash_is_allowed() {
        let mut contract = setup_contract();
        let (signing, agent_pubkey, agent) = wallet();
        let payer = accounts(2);
        let hash_accessor = SecretAccessor::WasmHash { hash: "ab".repeat(32) };

        let sig = sign(&signing, &agent_pubkey, &hash_accessor, "prod", "cipher", &payer);
        testing_env!(get_context(payer.clone(), NearToken::from_near(1)).build());
        contract.store_agent_secret(
            agent_pubkey.clone(),
            hash_accessor.clone(),
            "prod".to_string(),
            "cipher".to_string(),
            types::AccessCondition::AllowAll,
            None,
            sig,
        );

        assert!(
            contract.get_secrets(hash_accessor, "prod".to_string(), agent).is_some(),
            "a hash nobody has deployed yet is still a hash the secret can be bound to",
        );
    }

    #[test]
    #[should_panic(expected = "GitHub repositories are not supported")]
    fn a_github_repo_is_refused() {
        let mut contract = setup_contract();
        let (signing, agent_pubkey, _) = wallet();
        let payer = accounts(2);
        let repo = SecretAccessor::Repo {
            repo: "github.com/o/r".to_string(),
            branch: Some("main".to_string()),
        };

        let sig = sign(&signing, &agent_pubkey, &repo, "prod", "cipher", &payer);
        testing_env!(get_context(payer, NearToken::from_near(1)).build());
        contract.store_agent_secret(
            agent_pubkey,
            repo,
            "prod".to_string(),
            "cipher".to_string(),
            types::AccessCondition::AllowAll,
            None,
            sig,
        );
    }

    /// Sign a DELETE, through the production message builder.
    fn sign_delete(
        signing: &SigningKey,
        agent_pubkey: &str,
        accessor: &SecretAccessor,
        profile: &str,
        payer: &AccountId,
    ) -> String {
        let message = crate::secrets::secret_delete_message(agent_pubkey, accessor, profile, payer);
        let mut hasher = Sha256::new();
        hasher.update(message.as_bytes());
        let hash: [u8; 32] = hasher.finalize().into();
        hex::encode(signing.sign(&hash).to_bytes())
    }

    /// Store an agent secret and hand back what a delete needs.
    fn stored_agent_secret(
        contract: &mut Contract,
        payer: &AccountId,
    ) -> (SigningKey, String, SecretAccessor, AccountId) {
        let (signing, agent_pubkey, agent) = wallet();
        let accessor = SecretAccessor::WasmHash { hash: "cd".repeat(32) };
        let sig = sign(&signing, &agent_pubkey, &accessor, "prod", "cipher", payer);
        testing_env!(get_context(payer.clone(), NearToken::from_near(1)).build());
        contract.store_agent_secret(
            agent_pubkey.clone(),
            accessor.clone(),
            "prod".to_string(),
            "cipher".to_string(),
            types::AccessCondition::AllowAll,
            None,
            sig,
        );
        (signing, agent_pubkey, accessor, agent)
    }

    /// The key that owns the secret authorises its removal.
    ///
    /// Without this the secret was permanent: the ordinary `delete_secrets`
    /// deletes under `owner = caller`, and no caller can ever be an implicit
    /// account whose key lives inside the keystore.
    #[test]
    fn the_agents_key_authorises_a_delete_and_the_owners_index_is_cleaned() {
        let mut contract = setup_contract();
        let payer = accounts(2);
        let (signing, agent_pubkey, accessor, agent) = stored_agent_secret(&mut contract, &payer);

        let del = sign_delete(&signing, &agent_pubkey, &accessor, "prod", &payer);
        testing_env!(get_context(payer, NearToken::from_near(0)).build());
        contract.delete_agent_secret(agent_pubkey, accessor.clone(), "prod".to_string(), del);

        assert!(
            contract.get_secrets(accessor, "prod".to_string(), agent.clone()).is_none(),
            "the secret must be gone"
        );
        // Read the index DIRECTLY: `list_user_secrets` filters out keys whose
        // secret is gone, so a dangling entry is invisible through it.
        assert!(
            contract
                .user_secrets_index
                .get(&agent)
                .map(|set| set.is_empty())
                .unwrap_or(true),
            "the OWNER's index must be cleaned — cleaning the submitter's instead \
             leaves an entry pointing at a secret that no longer exists",
        );
    }

    /// The storage deposit comes back, and to the account that sent the delete.
    ///
    /// The same refund the ordinary delete makes. Asserted on the RECEIPT
    /// rather than on a balance, because a unit test's balances do not move —
    /// and a delete that quietly kept the deposit would pass every other test
    /// here, since the secret would still be gone.
    #[test]
    fn deleting_refunds_the_storage_deposit_to_the_submitter() {
        use near_sdk::test_utils::get_created_receipts;

        let mut contract = setup_contract();
        let payer = accounts(2);
        let (signing, agent_pubkey, accessor, agent) = stored_agent_secret(&mut contract, &payer);

        // What the store actually locked up.
        let held = contract
            .get_secrets(accessor.clone(), "prod".to_string(), agent)
            .expect("stored")
            .storage_deposit
            .0;
        assert!(held > 0, "the store must have taken a deposit for this to mean anything");

        let del = sign_delete(&signing, &agent_pubkey, &accessor, "prod", &payer);
        testing_env!(get_context(payer.clone(), NearToken::from_near(0)).build());
        contract.delete_agent_secret(agent_pubkey, accessor, "prod".to_string(), del);

        let refunds: Vec<_> = get_created_receipts()
            .into_iter()
            .filter(|r| r.receiver_id == payer)
            .collect();
        assert!(
            !refunds.is_empty(),
            "the deposit must go back to whoever sent the delete, not stay on the contract",
        );
    }

    /// A payment key cannot be destroyed through the agent-secret door.
    ///
    /// An agent's payment key IS a secret under the agent's own account —
    /// accessor `System(PaymentKey)` — so a valid agent signature would have
    /// erased it from the chain. That bypasses `delete_payment_key`, the path
    /// the write-once rule names because the coordinator sees it, and bypasses
    /// the coordinator's refusal to delete an agent key at all, which exists
    /// because the balance behind it would be stranded.
    #[test]
    #[should_panic(expected = "delete it with delete_payment_key")]
    fn a_payment_key_cannot_be_destroyed_through_this_door() {
        let mut contract = setup_contract();
        let (signing, agent_pubkey, _) = wallet();
        let payer = accounts(2);
        let accessor = SecretAccessor::System(SystemSecretType::PaymentKey);

        let del = sign_delete(&signing, &agent_pubkey, &accessor, "1", &payer);
        testing_env!(get_context(payer, NearToken::from_near(0)).build());
        contract.delete_agent_secret(agent_pubkey, accessor, "1".to_string(), del);
    }

    /// The agent may remove its own secret, and that is deliberate.
    ///
    /// An agent that has finished with a credential should be able to take it
    /// out of the world rather than leaving it on chain because only somebody
    /// else could. Nothing in the method singles the agent out — it is simply
    /// another submitter the signature can name.
    #[test]
    fn the_agent_may_delete_its_own_secret() {
        let mut contract = setup_contract();
        let payer = accounts(2);
        let (signing, agent_pubkey, accessor, agent) = stored_agent_secret(&mut contract, &payer);

        let del = sign_delete(&signing, &agent_pubkey, &accessor, "prod", &agent);
        testing_env!(get_context(agent.clone(), NearToken::from_near(0)).build());
        contract.delete_agent_secret(agent_pubkey, accessor.clone(), "prod".to_string(), del);

        assert!(contract.get_secrets(accessor, "prod".to_string(), agent).is_none());
    }

    /// The two messages cannot be confused for one another.
    ///
    /// Checked at the level where it is actually decided — the strings — and
    /// not only through the handler. Behaviourally a store signature fails a
    /// delete because the two messages carry a different NUMBER of fields, so
    /// the handler test below would pass even if both used one domain. The
    /// domain is the part that has to keep holding when the shapes converge,
    /// which is exactly the change nobody would think to re-test.
    #[test]
    fn the_delete_domain_is_distinct_from_the_store_domain() {
        let (_, agent_pubkey, _) = wallet();
        let accessor = SecretAccessor::WasmHash { hash: "ab".repeat(32) };
        let payer: AccountId = "payer.near".parse().unwrap();

        let del = crate::secrets::secret_delete_message(&agent_pubkey, &accessor, "prod", &payer);
        let store = crate::secrets::secret_store_message(
            &agent_pubkey,
            &accessor,
            "prod",
            "cipher",
            &payer,
            None,
            &types::AccessCondition::AllowAll,
        );

        assert!(del.starts_with("delete_agent_secret:v1:"), "{del}");
        assert!(store.starts_with("store_secrets_for:v1:"), "{store}");
        assert_ne!(
            del.split(':').next(),
            store.split(':').next(),
            "one domain for both would make a signed store a signed deletion the day \
             the two message shapes ever line up",
        );
    }

    /// And through the handler: a store signature does not delete.
    #[test]
    #[should_panic(expected = "Invalid Ed25519 wallet signature")]
    fn a_store_signature_cannot_delete() {
        let mut contract = setup_contract();
        let payer = accounts(2);
        let (signing, agent_pubkey, accessor, _) = stored_agent_secret(&mut contract, &payer);

        let store_sig = sign(&signing, &agent_pubkey, &accessor, "prod", "cipher", &payer);
        testing_env!(get_context(payer, NearToken::from_near(0)).build());
        contract.delete_agent_secret(agent_pubkey, accessor, "prod".to_string(), store_sig);
    }

    /// A delete signed for one submitter cannot be sent by another.
    ///
    /// The signature names the payer, so it cannot be lifted off chain and
    /// replayed — and a replayed delete is not a rollback, it is a deletion at
    /// a moment somebody else chooses.
    #[test]
    #[should_panic(expected = "Invalid Ed25519 wallet signature")]
    fn a_delete_cannot_be_replayed_by_another_submitter() {
        let mut contract = setup_contract();
        let payer = accounts(2);
        let (signing, agent_pubkey, accessor, _) = stored_agent_secret(&mut contract, &payer);

        let del = sign_delete(&signing, &agent_pubkey, &accessor, "prod", &payer);
        testing_env!(get_context(accounts(3), NearToken::from_near(0)).build());
        contract.delete_agent_secret(agent_pubkey, accessor, "prod".to_string(), del);
    }

    /// The accessor is part of what the signature authorises.
    ///
    /// Without it, a payer holding a signed call could store the same bytes
    /// under a different accessor. Nothing leaks — the ciphertext is sealed to
    /// one project's seed — but the authorisation would be resting on that
    /// second fact rather than on itself.
    #[test]
    #[should_panic(expected = "Invalid Ed25519 wallet signature")]
    fn a_signature_does_not_carry_over_to_another_accessor() {
        let (signing, wallet_pubkey, _) = wallet();
        let mut ctx = get_context(accounts(1), NearToken::from_near(1));
        testing_env!(ctx.build());
        let mut contract = Contract::new(accounts(0), Some(accounts(0)), None, None);

        let data = "Y2lwaGVy".to_string();
        // Signed for the Repo accessor these tests use…
        let signature = sign(&signing, &wallet_pubkey, &repo_accessor(), "profile", &data, &accounts(1));

        testing_env!(ctx.attached_deposit(NearToken::from_near(1)).build());
        // …and sent with a different one. Another PROJECT, since that is the
        // only kind this door takes — which is the realistic attempt anyway:
        // moving a signed secret to a project the signer does not own.
        contract.store_agent_secret(
            wallet_pubkey,
            SecretAccessor::Project {
                project_id: "someone-else.near/app".to_string(),
            },
            "profile".to_string(),
            data,
            types::AccessCondition::AllowAll,
            None,
            signature,
        );
    }

    // A Repo accessor, because a Project one has to exist on chain first and
    // this section is about ownership and signatures, not about accessors.
    /// The same accessor `store` writes under — see [`repo_accessor`].
    fn project() -> SecretAccessor {
        SecretAccessor::Project {
            project_id: "author.near/connector".to_string(),
        }
    }

    /// Register the project these fixtures address.
    ///
    /// A `Project` accessor is checked against the projects map — a secret
    /// cannot be stored against one that does not exist — so a test using the
    /// accessor this door takes has to put the project there first. Written
    /// straight into the map: `create_project` wants a storage deposit and a
    /// version flow, and neither is what these tests are about.
    fn register_project(contract: &mut Contract) {
        let uuid = "p0000000000000009".to_string();
        contract.projects.insert(
            &"author.near/connector".to_string(),
            &Project {
                uuid,
                owner: accounts(0),
                name: "connector".to_string(),
                active_version: "v1".to_string(),
                created_at: 0,
                storage_deposit: 0,
            },
        );
    }

    fn store(
        contract: &mut Contract,
        payer: AccountId,
        wallet_pubkey: &str,
        profile: &str,
        data: &str,
        signature: String,
    ) {
        register_project(contract);
        testing_env!(get_context(payer, NearToken::from_near(1)).build());
        contract.store_agent_secret(
            wallet_pubkey.to_string(),
            project(),
            profile.to_string(),
            data.to_string(),
            types::AccessCondition::AllowAll,
            None,
            signature,
        );
    }

    /// The point of the method: the WALLET owns the secret, the CALLER only pays.
    #[test]
    fn the_wallet_owns_it_and_the_caller_merely_pays() {
        let mut contract = setup_contract();
        let (signing, wallet_pubkey, wallet_account) = wallet();
        let payer = accounts(2);
        let sig = sign(&signing, &wallet_pubkey, &repo_accessor(), &wallet_account.to_string(), "cipher", &payer);

        store(&mut contract, payer.clone(), &wallet_pubkey, &wallet_account.to_string(), "cipher", sig);

        let stored = contract.get_secrets(project(), wallet_account.to_string(), wallet_account.clone());
        assert!(
            stored.is_some(),
            "the secret must be owned by the wallet, not by whoever paid"
        );
        assert!(
            contract
                .get_secrets(project(), wallet_account.to_string(), payer)
                .is_none(),
            "and must NOT be owned by the payer"
        );
    }

    /// The owner is the KEY's account, and a named account holding the same key
    /// is a different party.
    ///
    /// A human can hold one ed25519 key as a full-access key on `user.near` AND
    /// as the wallet key here. Those are two identities, not one: this method
    /// does not take an owner, it derives `hex(pubkey)` — so the secret lands in
    /// the implicit account's namespace and nowhere else. `user.near` cannot be
    /// named as the owner, which is what makes "store a secret for another user"
    /// impossible rather than merely guarded.
    ///
    /// The same fact is a trap for the person who owns both: a secret stored
    /// this way is NOT readable by a job running as `user.near`, because the
    /// keystore's rule requires the profile to equal the caller. Pinned from
    /// both ends — here, that the row is only under the derived account; and in
    /// the keystore, that a named caller is refused a profile of this shape.
    #[test]
    fn the_owner_is_the_keys_account_and_not_a_named_one_holding_the_same_key() {
        let mut contract = setup_contract();
        let (signing, wallet_pubkey, wallet_account) = wallet();
        let payer = accounts(2);

        // The account a key derives is its hex, and nothing else.
        let raw_hex = wallet_pubkey.strip_prefix("ed25519:").expect("test key is ed25519");
        assert_eq!(
            wallet_account.to_string(),
            raw_hex,
            "the derived owner must be the key itself, in hex"
        );

        let sig = sign(&signing, &wallet_pubkey, &repo_accessor(), &wallet_account.to_string(), "cipher", &payer);
        store(&mut contract, payer, &wallet_pubkey, &wallet_account.to_string(), "cipher", sig);

        // A named account that holds this very key owns nothing here. There is
        // no argument through which it could have been named.
        let named: AccountId = "user.near".parse().unwrap();
        assert!(
            contract
                .get_secrets(project(), wallet_account.to_string(), named)
                .is_none(),
            "a named account must never own a secret stored through this door",
        );

        // And the row is where the derivation says it is.
        assert!(contract
            .get_secrets(project(), wallet_account.to_string(), wallet_account)
            .is_some());
    }

    /// A signature over other content must not carry this content. Otherwise a
    /// valid signature seen once would authorise storing anything.
    #[test]
    #[should_panic(expected = "Invalid Ed25519 wallet signature")]
    fn a_signature_over_different_content_is_refused() {
        let mut contract = setup_contract();
        let (signing, wallet_pubkey, wallet_account) = wallet();
        let payer = accounts(2);
        let sig = sign(&signing, &wallet_pubkey, &repo_accessor(), &wallet_account.to_string(), "cipher", &payer);

        store(&mut contract, payer, &wallet_pubkey, &wallet_account.to_string(), "OTHER cipher", sig);
    }

    /// Another wallet's signature buys nothing: an agent account is public, and
    /// this is what stops a stranger planting a credential under it.
    #[test]
    #[should_panic(expected = "Invalid Ed25519 wallet signature")]
    fn a_signature_by_another_wallet_is_refused() {
        let mut contract = setup_contract();
        let (_, wallet_pubkey, wallet_account) = wallet();
        let stranger = SigningKey::from_bytes(&[9u8; 32]);
        let payer = accounts(2);
        let sig = sign(&stranger, &wallet_pubkey, &repo_accessor(), &wallet_account.to_string(), "cipher", &payer);

        store(&mut contract, payer, &wallet_pubkey, &wallet_account.to_string(), "cipher", sig);
    }

    /// The signed message binds the PAYER, so the pair (data, signature) that
    /// stays on chain forever cannot be replayed by anyone else. Without this a
    /// stranger could re-file an OLD ciphertext over a rotated credential —
    /// `store_wallet_policy` has exactly that hole and this must not repeat it.
    #[test]
    #[should_panic(expected = "Invalid Ed25519 wallet signature")]
    fn a_signature_made_for_one_payer_cannot_be_replayed_by_another() {
        let mut contract = setup_contract();
        let (signing, wallet_pubkey, wallet_account) = wallet();
        let sig = sign(&signing, &wallet_pubkey, &repo_accessor(), &wallet_account.to_string(), "cipher", &accounts(2));

        // Same everything, different sender.
        store(&mut contract, accounts(3), &wallet_pubkey, &wallet_account.to_string(), "cipher", sig);
    }

    /// A payment key cannot be created OR rewritten through this door.
    ///
    /// The shared body's write-once rule was assumed to cover this entry point,
    /// and it does not: it refuses a REWRITE, so the FIRST store went through.
    /// This test passed on that second iteration and read as if the door were
    /// shut. It is shut now, at the door — payment keys are created by their
    /// owner through `store_secrets`, never here.
    #[test]
    #[should_panic(expected = "A payment key is not an agent secret")]
    fn a_payment_key_cannot_be_created_or_rewritten_through_this_door() {
        let mut contract = setup_contract();
        let (signing, wallet_pubkey, wallet_account) = wallet();
        let payer = accounts(2);

        for data in ["blob-v1", "blob-v2"] {
            let sig = sign(
                &signing,
                &wallet_pubkey,
                &SecretAccessor::System(SystemSecretType::PaymentKey),
                "1",
                data,
                &payer,
            );
            testing_env!(get_context(payer.clone(), NearToken::from_near(1)).build());
            contract.store_agent_secret(
                wallet_pubkey.clone(),
                SecretAccessor::System(SystemSecretType::PaymentKey),
                "1".to_string(),
                data.to_string(),
                types::AccessCondition::AllowAll,
                None,
                sig,
            );
        }
        let _ = wallet_account;
    }

    /// A secp256k1 wallet key addresses EVM or Bitcoin and has no NEAR account,
    /// so there is no owner to derive. Refused rather than answered with an
    /// invented account.
    #[test]
    #[should_panic(expected = "Only an ed25519 wallet key has a NEAR account")]
    fn a_secp256k1_wallet_key_has_no_near_account() {
        let mut contract = setup_contract();
        let pubkey = format!("secp256k1:{}", hex::encode([2u8; 33]));

        store(&mut contract, accounts(2), &pubkey, "profile", "cipher", hex::encode([0u8; 64]));
    }
}

