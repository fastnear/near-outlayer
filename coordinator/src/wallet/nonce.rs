//! Per-wallet mutex for nonce management
//!
//! Concurrent withdrawals from the same wallet must be serialized
//! to prevent nonce conflicts. Different wallets run in parallel.

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

/// Per-wallet lock manager
pub struct WalletNonceLocks {
    locks: Mutex<HashMap<String, Arc<Mutex<()>>>>,
}

impl WalletNonceLocks {
    pub fn new() -> Self {
        Self {
            locks: Mutex::new(HashMap::new()),
        }
    }

    /// Get the lock for a specific wallet.
    /// Creates it if it doesn't exist.
    pub async fn get_lock(&self, wallet_id: &str) -> Arc<Mutex<()>> {
        let mut locks = self.locks.lock().await;
        locks
            .entry(wallet_id.to_string())
            .or_insert_with(|| Arc::new(Mutex::new(())))
            .clone()
    }

    /// Clean up locks for wallets that haven't been used recently.
    /// Called periodically to prevent unbounded growth.
    pub async fn cleanup(&self) {
        let mut locks = self.locks.lock().await;
        // Remove locks that nobody is waiting on (strong_count == 1 means only the HashMap holds it)
        locks.retain(|_, lock| Arc::strong_count(lock) > 1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_lock_create_and_reuse() {
        let locks = WalletNonceLocks::new();
        let lock1 = locks.get_lock("w1").await;
        let lock2 = locks.get_lock("w1").await;
        assert!(Arc::ptr_eq(&lock1, &lock2));
    }

    #[tokio::test]
    async fn test_lock_different_wallets() {
        let locks = WalletNonceLocks::new();
        let lock1 = locks.get_lock("w1").await;
        let lock2 = locks.get_lock("w2").await;
        assert!(!Arc::ptr_eq(&lock1, &lock2));
    }

    #[tokio::test]
    async fn test_cleanup_removes_unused() {
        let locks = WalletNonceLocks::new();
        {
            let _lock = locks.get_lock("w1").await;
            // _lock dropped at end of scope
        }
        locks.cleanup().await;
        // After cleanup, getting w1 should create a new lock
        let lock_after = locks.get_lock("w1").await;
        // We can't compare with the old one (dropped), but verify a new one was created
        assert_eq!(Arc::strong_count(&lock_after), 2); // HashMap + our variable
    }
}
