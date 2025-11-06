//! Math Helpers with Checked Arithmetic
//!
//! Phase 1 Hardening: All pricing calculations use checked arithmetic to prevent:
//! - Integer overflow (e.g., u64::MAX + 1 = panic)
//! - Integer underflow (e.g., 0 - 1 = panic)
//! - Multiplication overflow (e.g., 1_000_000_000 * 1_000_000_000 = overflow)
//!
//! ## Why This Matters
//!
//! Without checked arithmetic:
//! ```rust
//! let cost = max_instructions * price_per_instruction; // Can overflow!
//! let refund = payment - cost;                          // Can underflow!
//! ```
//!
//! With checked arithmetic:
//! ```rust
//! let cost = checked_mul(max_instructions, price_per_instruction)?;
//! let refund = checked_sub(payment, cost)?;
//! ```
//!
//! ## Panic vs Error
//!
//! NEAR contracts MUST NOT panic during state-changing operations.
//! Panics = transaction failure = no state changes = funds lost.
//!
//! These helpers return `Result<T>` instead of panicking.

use near_sdk::env;

/// Checked addition (a + b)
///
/// Returns error if overflow would occur
pub fn checked_add(a: u128, b: u128) -> Result<u128, String> {
    a.checked_add(b)
        .ok_or_else(|| format!("Arithmetic overflow: {} + {}", a, b))
}

/// Checked subtraction (a - b)
///
/// Returns error if underflow would occur
pub fn checked_sub(a: u128, b: u128) -> Result<u128, String> {
    a.checked_sub(b)
        .ok_or_else(|| format!("Arithmetic underflow: {} - {}", a, b))
}

/// Checked multiplication (a * b)
///
/// Returns error if overflow would occur
pub fn checked_mul(a: u128, b: u128) -> Result<u128, String> {
    a.checked_mul(b)
        .ok_or_else(|| format!("Arithmetic overflow: {} * {}", a, b))
}

/// Checked division (a / b)
///
/// Returns error if division by zero
pub fn checked_div(a: u128, b: u128) -> Result<u128, String> {
    a.checked_div(b)
        .ok_or_else(|| format!("Division by zero: {} / {}", a, b))
}

/// Checked addition for u64
pub fn checked_add_u64(a: u64, b: u64) -> Result<u64, String> {
    a.checked_add(b)
        .ok_or_else(|| format!("Arithmetic overflow: {} + {}", a, b))
}

/// Checked subtraction for u64
pub fn checked_sub_u64(a: u64, b: u64) -> Result<u64, String> {
    a.checked_sub(b)
        .ok_or_else(|| format!("Arithmetic underflow: {} - {}", a, b))
}

/// Checked multiplication for u64
pub fn checked_mul_u64(a: u64, b: u64) -> Result<u64, String> {
    a.checked_mul(b)
        .ok_or_else(|| format!("Arithmetic overflow: {} * {}", a, b))
}

/// Compute execution cost with overflow protection
///
/// Formula: base_fee + (instructions * per_instruction_fee) + (time_ms * per_ms_fee)
pub fn compute_execution_cost(
    base_fee: u128,
    instructions: u64,
    per_instruction_fee: u128,
    time_ms: u64,
    per_ms_fee: u128,
) -> Result<u128, String> {
    // Convert u64 to u128 for safe arithmetic
    let instructions_u128 = instructions as u128;
    let time_ms_u128 = time_ms as u128;

    // Compute instruction cost
    let instruction_cost = checked_mul(instructions_u128, per_instruction_fee)?;

    // Compute time cost
    let time_cost = checked_mul(time_ms_u128, per_ms_fee)?;

    // Add all costs
    let cost = checked_add(base_fee, instruction_cost)?;
    let cost = checked_add(cost, time_cost)?;

    Ok(cost)
}

/// Compute refund amount with underflow protection
///
/// Formula: payment - cost (but return 0 if cost > payment)
pub fn compute_refund(payment: u128, cost: u128) -> u128 {
    payment.saturating_sub(cost)
}

/// Estimate cost with limit bounds
///
/// Used for upfront payment estimation
pub fn estimate_cost(
    base_fee: u128,
    max_instructions: u64,
    per_instruction_fee: u128,
    max_execution_seconds: u64,
    per_ms_fee: u128,
) -> Result<u128, String> {
    let max_time_ms = checked_mul_u64(max_execution_seconds, 1000)?;

    compute_execution_cost(
        base_fee,
        max_instructions,
        per_instruction_fee,
        max_time_ms,
        per_ms_fee,
    )
}

/// Convert instructions to cost (for instruction-only pricing)
pub fn instructions_to_cost(instructions: u64, per_instruction_fee: u128) -> Result<u128, String> {
    checked_mul(instructions as u128, per_instruction_fee)
}

/// Convert time to cost (for time-only pricing)
pub fn time_to_cost(time_ms: u64, per_ms_fee: u128) -> Result<u128, String> {
    checked_mul(time_ms as u128, per_ms_fee)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_checked_add() {
        assert_eq!(checked_add(100, 200).unwrap(), 300);
        assert!(checked_add(u128::MAX, 1).is_err());
    }

    #[test]
    fn test_checked_sub() {
        assert_eq!(checked_sub(200, 100).unwrap(), 100);
        assert!(checked_sub(100, 200).is_err());
    }

    #[test]
    fn test_checked_mul() {
        assert_eq!(checked_mul(10, 20).unwrap(), 200);
        assert!(checked_mul(u128::MAX, 2).is_err());
    }

    #[test]
    fn test_checked_div() {
        assert_eq!(checked_div(100, 10).unwrap(), 10);
        assert!(checked_div(100, 0).is_err());
    }

    #[test]
    fn test_compute_execution_cost() {
        let cost = compute_execution_cost(
            1_000_000,        // base_fee
            10_000_000,       // instructions
            10,               // per_instruction_fee
            1000,             // time_ms
            1000,             // per_ms_fee
        )
        .unwrap();

        // Expected: 1_000_000 + (10_000_000 * 10) + (1000 * 1000)
        //         = 1_000_000 + 100_000_000 + 1_000_000
        //         = 102_000_000
        assert_eq!(cost, 102_000_000);
    }

    #[test]
    fn test_compute_execution_cost_overflow() {
        // Should detect overflow
        let result = compute_execution_cost(
            u128::MAX,
            u64::MAX,
            u128::MAX,
            u64::MAX,
            u128::MAX,
        );

        assert!(result.is_err());
    }

    #[test]
    fn test_compute_refund() {
        // Normal case
        assert_eq!(compute_refund(1000, 600), 400);

        // Underflow protection (saturating_sub returns 0)
        assert_eq!(compute_refund(600, 1000), 0);
    }

    #[test]
    fn test_estimate_cost() {
        let cost = estimate_cost(
            1_000_000,        // base_fee
            10_000_000,       // max_instructions
            10,               // per_instruction_fee
            60,               // max_execution_seconds
            1000,             // per_ms_fee
        )
        .unwrap();

        // Expected: 1_000_000 + (10_000_000 * 10) + (60_000 * 1000)
        //         = 1_000_000 + 100_000_000 + 60_000_000
        //         = 161_000_000
        assert_eq!(cost, 161_000_000);
    }

    #[test]
    fn test_instructions_to_cost() {
        assert_eq!(instructions_to_cost(1_000_000, 100).unwrap(), 100_000_000);
    }

    #[test]
    fn test_time_to_cost() {
        assert_eq!(time_to_cost(60_000, 1000).unwrap(), 60_000_000);
    }
}
