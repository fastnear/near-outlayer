//! Phase 1-5 Integration Test Runner
//!
//! Runs all integration tests for hardening work completed in Phases 1-5.
//!
//! Usage:
//!   cargo test  (runs all 82 integration tests)
//!
//! This binary exists for organizational purposes but all tests are
//! executed via cargo's test framework, not custom runners.

mod common;
mod determinism;
mod contract_events;
mod coordinator_hardening;
mod wasi_helpers;
mod typescript_client;

fn main() {
    println!("Phase 1-5 Integration Tests");
    println!("============================");
    println!();
    println!("To run all tests:");
    println!("  cargo test");
    println!();
    println!("To run tests by phase:");
    println!("  cargo test determinism::       # Phase 1");
    println!("  cargo test contract_events::   # Phase 2");
    println!("  cargo test coordinator_hardening::  # Phase 3");
    println!("  cargo test wasi_helpers::      # Phase 4");
    println!("  cargo test typescript_client:: # Phase 5");
    println!();
    println!("See README.md for complete documentation.");
}
