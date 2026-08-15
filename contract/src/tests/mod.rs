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
    #[should_panic(expected = "costs exactly 10000")]
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

    /// EXACTLY, in both directions.
    ///
    /// Under-attaching is obviously refused. OVER-attaching is refused too, and
    /// that is the point: an exact amount is what removes the question of who
    /// returns the difference, which on chain has no good answer — the only
    /// refund path is a host function the guest calls, which would put a second
    /// copy of the price inside the guest.
    #[test]
    #[should_panic(expected = "costs exactly 10000")]
    fn under_attaching_is_refused() {
        let mut c = contract_with_project();
        c.set_project_pricing(PROJECT.to_string(), pricing(vec![op("send", 10_000)]));

        testing_env!(ctx(accounts(1), NearToken::from_near(1)).build());
        c.user_stablecoin_balances.insert(&accounts(1), &1_000_000);
        call_op(&mut c, "send", Some(9_999));
    }

    #[test]
    #[should_panic(expected = "costs exactly 10000")]
    fn over_attaching_is_refused_too() {
        let mut c = contract_with_project();
        c.set_project_pricing(PROJECT.to_string(), pricing(vec![op("send", 10_000)]));

        testing_env!(ctx(accounts(1), NearToken::from_near(1)).build());
        c.user_stablecoin_balances.insert(&accounts(1), &1_000_000);
        call_op(&mut c, "send", Some(10_001));
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
mod store_secrets_for_tests {
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
    fn repo_accessor() -> SecretAccessor {
        SecretAccessor::Repo {
            repo: "https://github.com/author/connector".to_string(),
            branch: None,
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
        // …and sent with a different one.
        contract.store_secrets_for(
            wallet_pubkey,
            SecretAccessor::WasmHash {
                hash: "aa".to_string(),
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
    fn project() -> SecretAccessor {
        SecretAccessor::Repo {
            repo: "https://github.com/author/connector".to_string(),
            branch: None,
        }
    }

    fn store(
        contract: &mut Contract,
        payer: AccountId,
        wallet_pubkey: &str,
        profile: &str,
        data: &str,
        signature: String,
    ) {
        testing_env!(get_context(payer, NearToken::from_near(1)).build());
        contract.store_secrets_for(
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

    /// The write-once rule for payment keys is enforced by the shared body, so it
    /// covers this entry point too. If it did not, this would be a second way to
    /// rewrite a money record.
    #[test]
    #[should_panic(expected = "A payment key cannot be rewritten")]
    fn a_payment_key_cannot_be_rewritten_through_this_door_either() {
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
            contract.store_secrets_for(
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
