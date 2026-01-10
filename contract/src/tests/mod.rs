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
