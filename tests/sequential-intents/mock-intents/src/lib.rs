use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};
use near_sdk::collections::UnorderedMap;
use near_sdk::serde::{Deserialize, Serialize};
use near_sdk::{env, log, near_bindgen, AccountId, BorshStorageKey, PanicOnDefault};
use schemars::JsonSchema;

#[derive(BorshSerialize, BorshStorageKey)]
#[borsh(crate = "near_sdk::borsh")]
enum StorageKey {
    AccountStates,
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
pub struct AccountState {
    pub total_deposited: String,
    pub deposit_count: u32,
    pub intent_count: u32,
    pub settle_count: u32,
    pub last_intent_seq: Option<u64>,
    pub last_intent_settled: bool,
}

impl Default for AccountState {
    fn default() -> Self {
        Self {
            total_deposited: "0".to_string(),
            deposit_count: 0,
            intent_count: 0,
            settle_count: 0,
            last_intent_seq: None,
            last_intent_settled: false,
        }
    }
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
pub enum IntentAction {
    Deposit {
        #[schemars(with = "String")]
        token: AccountId,
        amount: String,
    },
    SubmitIntent {
        payload: String,
    },
    Settle {
        target_seq: u64,
    },
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
pub struct IntentRecord {
    pub seq: u64,
    #[schemars(with = "String")]
    pub predecessor: AccountId,
    #[schemars(with = "String")]
    pub sender_arg: AccountId,
    pub action: IntentAction,
    pub executed_at_block: u64,
}

#[near_bindgen]
#[derive(BorshDeserialize, BorshSerialize, PanicOnDefault)]
#[borsh(crate = "near_sdk::borsh")]
pub struct MockIntents {
    allowed_gate_id: AccountId,
    allow_direct: bool,
    next_seq: u64,
    state_by_account: UnorderedMap<AccountId, AccountState>,
    intents_log: Vec<IntentRecord>,
}

#[near_bindgen]
impl MockIntents {
    #[init]
    pub fn new(allowed_gate_id: AccountId, allow_direct: Option<bool>) -> Self {
        log!(
            "sequential mock-intents initialized: allowed_gate_id={}, allow_direct={}",
            allowed_gate_id,
            allow_direct.unwrap_or(false)
        );

        Self {
            allowed_gate_id,
            allow_direct: allow_direct.unwrap_or(false),
            next_seq: 0,
            state_by_account: UnorderedMap::new(StorageKey::AccountStates),
            intents_log: Vec::new(),
        }
    }

    pub fn deposit(&mut self, sender: AccountId, token: AccountId, amount: String) {
        let predecessor = self.assert_authorized(&sender);
        let amount_value = parse_amount(&amount);
        let seq = self.next_sequence();

        let mut state = self.state_by_account.get(&sender).unwrap_or_default();
        let current_total = parse_amount(&state.total_deposited);
        state.total_deposited = current_total
            .checked_add(amount_value)
            .unwrap_or_else(|| env::panic_str("deposit overflow"))
            .to_string();
        state.deposit_count += 1;
        state.last_intent_seq = None;
        state.last_intent_settled = false;
        self.state_by_account.insert(&sender, &state);

        self.push_record(
            seq,
            predecessor.clone(),
            sender.clone(),
            IntentAction::Deposit {
                token: token.clone(),
                amount: amount.clone(),
            },
        );

        log!(
            "mock-intents:deposit:seq={}:predecessor={}:sender_arg={}:token={}:amount={}",
            seq,
            predecessor,
            sender,
            token,
            amount
        );
    }

    pub fn submit_intent(&mut self, sender: AccountId, payload: String) {
        let predecessor = self.assert_authorized(&sender);
        let seq = self.next_sequence();

        let mut state = self
            .state_by_account
            .get(&sender)
            .unwrap_or_else(|| env::panic_str("submit_intent: no deposits recorded for sender"));
        if state.deposit_count <= state.intent_count {
            env::panic_str(
                "submit_intent: a deposit must precede each submit_intent for the same sender",
            );
        }

        state.intent_count += 1;
        state.last_intent_seq = Some(seq);
        state.last_intent_settled = false;
        self.state_by_account.insert(&sender, &state);

        self.push_record(
            seq,
            predecessor.clone(),
            sender.clone(),
            IntentAction::SubmitIntent {
                payload: payload.clone(),
            },
        );

        log!(
            "mock-intents:submit_intent:seq={}:predecessor={}:sender_arg={}:payload_len={}",
            seq,
            predecessor,
            sender,
            payload.len()
        );
    }

    pub fn settle(&mut self, sender: AccountId, target_seq: u64) {
        let predecessor = self.assert_authorized(&sender);
        let seq = self.next_sequence();

        let mut state = self
            .state_by_account
            .get(&sender)
            .unwrap_or_else(|| env::panic_str("settle: no prior activity for sender"));
        let expected = state
            .last_intent_seq
            .unwrap_or_else(|| env::panic_str("settle: no submitted intent to settle"));
        if expected != target_seq {
            env::panic_str(&format!(
                "settle: target_seq mismatch (expected last intent seq {}, got {})",
                expected, target_seq
            ));
        }
        if state.last_intent_settled {
            env::panic_str("settle: intent already settled");
        }

        state.last_intent_settled = true;
        state.settle_count += 1;
        self.state_by_account.insert(&sender, &state);

        self.push_record(
            seq,
            predecessor.clone(),
            sender.clone(),
            IntentAction::Settle { target_seq },
        );

        log!(
            "mock-intents:settle:seq={}:predecessor={}:sender_arg={}:target_seq={}",
            seq,
            predecessor,
            sender,
            target_seq
        );
    }

    pub fn get_state(&self, account: AccountId) -> Option<AccountState> {
        self.state_by_account.get(&account)
    }

    pub fn get_log(&self, offset: Option<u64>, limit: Option<u32>) -> Vec<IntentRecord> {
        let limit = limit.unwrap_or(50).min(200) as usize;
        let len = self.intents_log.len();
        let start = offset
            .map(|value| value as usize)
            .unwrap_or_else(|| len.saturating_sub(limit))
            .min(len);
        let end = (start + limit).min(len);
        self.intents_log[start..end].to_vec()
    }

    pub fn get_next_seq(&self) -> u64 {
        self.next_seq
    }

    fn assert_authorized(&self, sender: &AccountId) -> AccountId {
        let predecessor = env::predecessor_account_id();
        if predecessor == self.allowed_gate_id || (self.allow_direct && &predecessor == sender) {
            return predecessor;
        }

        env::panic_str(&format!(
            "unauthorized predecessor {} for sender {}",
            predecessor, sender
        ));
    }

    fn next_sequence(&mut self) -> u64 {
        let seq = self.next_seq;
        self.next_seq += 1;
        seq
    }

    fn push_record(
        &mut self,
        seq: u64,
        predecessor: AccountId,
        sender_arg: AccountId,
        action: IntentAction,
    ) {
        self.intents_log.push(IntentRecord {
            seq,
            predecessor,
            sender_arg,
            action,
            executed_at_block: env::block_height(),
        });
    }
}

fn parse_amount(amount: &str) -> u128 {
    amount
        .parse()
        .unwrap_or_else(|_| env::panic_str("amount must be a base-10 u128 string"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use near_sdk::test_utils::VMContextBuilder;
    use near_sdk::testing_env;

    fn mike() -> AccountId {
        "mike.test".parse().unwrap()
    }

    fn gate() -> AccountId {
        "gate.test".parse().unwrap()
    }

    fn token() -> AccountId {
        "wrap.test".parse().unwrap()
    }

    fn alice() -> AccountId {
        "alice.test".parse().unwrap()
    }

    fn context(predecessor: AccountId, block_height: u64) {
        testing_env!(VMContextBuilder::new()
            .predecessor_account_id(predecessor)
            .block_height(block_height)
            .build());
    }

    fn contract(allow_direct: bool) -> MockIntents {
        context(gate(), 1);
        MockIntents::new(gate(), Some(allow_direct))
    }

    #[test]
    fn direct_predecessor_path_records_sender_as_predecessor() {
        let mut contract = contract(true);

        context(mike(), 10);
        contract.deposit(mike(), token(), "1000".to_string());

        let log = contract.get_log(None, None);
        assert_eq!(log.len(), 1);
        assert_eq!(log[0].predecessor, mike());
        assert_eq!(log[0].sender_arg, mike());
        assert_eq!(log[0].executed_at_block, 10);
    }

    #[test]
    fn direct_and_gate_modes_record_different_authoritative_predecessors() {
        let mut contract = contract(true);

        context(mike(), 10);
        contract.deposit(mike(), token(), "1000".to_string());

        context(gate(), 20);
        contract.deposit(mike(), token(), "1000".to_string());

        let log = contract.get_log(None, None);
        assert_eq!(log.len(), 2);
        assert_eq!(log[0].predecessor, mike());
        assert_eq!(log[0].sender_arg, mike());
        assert_eq!(log[1].predecessor, gate());
        assert_eq!(log[1].sender_arg, mike());
    }

    #[test]
    fn gate_proxy_path_records_gate_predecessor_and_user_sender_arg() {
        let mut contract = contract(false);

        context(gate(), 20);
        contract.deposit(mike(), token(), "1000".to_string());

        let log = contract.get_log(None, None);
        assert_eq!(log.len(), 1);
        assert_eq!(log[0].predecessor, gate());
        assert_eq!(log[0].sender_arg, mike());
        assert!(contract.get_state(gate()).is_none());
        assert_eq!(contract.get_state(mike()).unwrap().total_deposited, "1000");
    }

    #[test]
    #[should_panic(expected = "submit_intent: no deposits recorded for sender")]
    fn submit_intent_before_deposit_fails() {
        let mut contract = contract(false);

        context(gate(), 30);
        contract.submit_intent(mike(), "swap".to_string());
    }

    #[test]
    #[should_panic(expected = "settle: no submitted intent to settle")]
    fn settle_before_submit_fails() {
        let mut contract = contract(false);

        context(gate(), 40);
        contract.deposit(mike(), token(), "1000".to_string());
        context(gate(), 43);
        contract.settle(mike(), 1);
    }

    #[test]
    #[should_panic(expected = "a deposit must precede each submit_intent")]
    fn duplicate_submit_without_second_deposit_fails() {
        let mut contract = contract(false);

        context(gate(), 50);
        contract.deposit(mike(), token(), "1000".to_string());
        context(gate(), 53);
        contract.submit_intent(mike(), "first".to_string());
        context(gate(), 56);
        contract.submit_intent(mike(), "second".to_string());
    }

    #[test]
    #[should_panic(expected = "intent already settled")]
    fn duplicate_settle_fails() {
        let mut contract = contract(false);

        context(gate(), 60);
        contract.deposit(mike(), token(), "1000".to_string());
        context(gate(), 63);
        contract.submit_intent(mike(), "first".to_string());
        context(gate(), 66);
        contract.settle(mike(), 1);
        context(gate(), 69);
        contract.settle(mike(), 1);
    }

    #[test]
    fn non_commutative_happy_path_updates_state_and_log_order() {
        let mut contract = contract(false);

        context(gate(), 100);
        contract.deposit(mike(), token(), "1000".to_string());
        context(gate(), 103);
        contract.submit_intent(mike(), "transfer".to_string());
        context(gate(), 106);
        contract.settle(mike(), 1);

        let state = contract.get_state(mike()).unwrap();
        assert_eq!(state.total_deposited, "1000");
        assert_eq!(state.deposit_count, 1);
        assert_eq!(state.intent_count, 1);
        assert_eq!(state.settle_count, 1);
        assert_eq!(state.last_intent_seq, Some(1));
        assert!(state.last_intent_settled);

        let log = contract.get_log(None, None);
        assert_eq!(
            log.iter().map(|record| record.seq).collect::<Vec<_>>(),
            [0, 1, 2]
        );
        assert!(matches!(log[0].action, IntentAction::Deposit { .. }));
        assert!(matches!(log[1].action, IntentAction::SubmitIntent { .. }));
        assert!(matches!(
            log[2].action,
            IntentAction::Settle { target_seq: 1 }
        ));
        assert_eq!(
            log.iter()
                .map(|record| record.executed_at_block)
                .collect::<Vec<_>>(),
            [100, 103, 106]
        );
    }

    #[test]
    fn log_pagination_and_tail_views_work() {
        let mut contract = contract(true);

        context(mike(), 200);
        contract.deposit(mike(), token(), "100".to_string());
        context(mike(), 203);
        contract.submit_intent(mike(), "first".to_string());
        context(mike(), 206);
        contract.settle(mike(), 1);

        let page = contract.get_log(Some(1), Some(2));
        assert_eq!(page.len(), 2);
        assert_eq!(page[0].seq, 1);
        assert_eq!(page[1].seq, 2);
        assert_eq!(page[0].executed_at_block, 203);
        assert_eq!(page[1].executed_at_block, 206);

        let tail = contract.get_log(None, Some(1));
        assert_eq!(tail.len(), 1);
        assert_eq!(tail[0].seq, 2);
    }

    #[test]
    fn multiple_senders_are_tracked_independently() {
        let mut contract = contract(true);

        context(mike(), 300);
        contract.deposit(mike(), token(), "100".to_string());
        context(alice(), 301);
        contract.deposit(alice(), token(), "250".to_string());
        context(mike(), 302);
        contract.deposit(mike(), token(), "400".to_string());

        let mike_state = contract.get_state(mike()).unwrap();
        let alice_state = contract.get_state(alice()).unwrap();
        assert_eq!(mike_state.total_deposited, "500");
        assert_eq!(mike_state.deposit_count, 2);
        assert_eq!(alice_state.total_deposited, "250");
        assert_eq!(alice_state.deposit_count, 1);

        let log = contract.get_log(None, None);
        assert_eq!(log.len(), 3);
        assert_eq!(log[0].predecessor, mike());
        assert_eq!(log[0].sender_arg, mike());
        assert_eq!(log[1].predecessor, alice());
        assert_eq!(log[1].sender_arg, alice());
        assert_eq!(log[2].predecessor, mike());
        assert_eq!(log[2].sender_arg, mike());
    }

    #[test]
    #[should_panic(expected = "unauthorized predecessor")]
    fn unauthorized_non_gate_predecessor_fails_when_direct_mode_disabled() {
        let mut contract = contract(false);

        context(alice(), 400);
        contract.deposit(mike(), token(), "1000".to_string());
    }
}
