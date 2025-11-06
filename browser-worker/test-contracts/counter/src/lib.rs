/**
 * Counter Contract - Simple test contract for browser WASM execution
 *
 * Demonstrates basic NEAR contract functionality:
 * - State storage
 * - View and change methods
 * - Logging
 */

use near_sdk::{env, log, near, near_bindgen, AccountId, PanicOnDefault};

#[near(contract_state)]
#[derive(PanicOnDefault)]
pub struct Counter {
    count: u64,
}

#[near]
impl Counter {
    /// Initialize contract with count = 0
    #[init]
    pub fn new() -> Self {
        log!("Counter initialized");
        Self { count: 0 }
    }

    /// Initialize contract with custom starting count
    #[init]
    pub fn new_with_count(count: u64) -> Self {
        log!("Counter initialized with count: {}", count);
        Self { count }
    }

    /// Increment counter by 1
    pub fn increment(&mut self) {
        self.count += 1;
        log!("Count incremented to: {}", self.count);
    }

    /// Increment counter by a specific amount
    pub fn increment_by(&mut self, amount: u64) {
        self.count += amount;
        log!("Count incremented by {} to: {}", amount, self.count);
    }

    /// Decrement counter by 1
    pub fn decrement(&mut self) {
        if self.count == 0 {
            env::panic_str("Cannot decrement below zero");
        }
        self.count -= 1;
        log!("Count decremented to: {}", self.count);
    }

    /// Reset counter to 0
    pub fn reset(&mut self) {
        let old_count = self.count;
        self.count = 0;
        log!("Count reset from {} to 0", old_count);
    }

    /// Set counter to specific value
    pub fn set_count(&mut self, count: u64) {
        let old_count = self.count;
        self.count = count;
        log!("Count changed from {} to {}", old_count, count);
    }

    /// Get current count (view method)
    pub fn get_count(&self) -> u64 {
        log!("Current count: {}", self.count);
        self.count
    }

    /// Check if count is zero (view method)
    pub fn is_zero(&self) -> bool {
        self.count == 0
    }

    /// Check if count is even (view method)
    pub fn is_even(&self) -> bool {
        self.count % 2 == 0
    }

    /// Get count with metadata (view method)
    pub fn get_info(&self) -> CounterInfo {
        CounterInfo {
            count: self.count,
            is_zero: self.count == 0,
            is_even: self.count % 2 == 0,
            signer: env::signer_account_id().to_string(),
        }
    }
}

#[near(serializers = [json])]
pub struct CounterInfo {
    pub count: u64,
    pub is_zero: bool,
    pub is_even: bool,
    pub signer: String,
}
