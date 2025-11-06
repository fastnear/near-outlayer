//! Determinism Test WASM Module
//!
//! This module is designed to test deterministic execution properties:
//! - Takes a seed via stdin
//! - Performs deterministic computation
//! - Outputs result via stdout
//!
//! Key properties:
//! - No randomness (uses deterministic PRNG with seed)
//! - No system calls that could introduce non-determinism
//! - Pure computation based on input

use serde::{Deserialize, Serialize};
use std::io::{self, Read, Write};

#[derive(Debug, Deserialize)]
struct Input {
    seed: u64,
    iterations: Option<usize>,
}

#[derive(Debug, Serialize)]
struct Output {
    result: u64,
    checksum: String,
    iterations_run: usize,
}

/// Deterministic PRNG (Linear Congruential Generator)
/// Same seed always produces same sequence
struct DeterministicRng {
    state: u64,
}

impl DeterministicRng {
    fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    fn next(&mut self) -> u64 {
        // LCG parameters from Numerical Recipes
        const A: u64 = 1664525;
        const C: u64 = 1013904223;
        self.state = self.state.wrapping_mul(A).wrapping_add(C);
        self.state
    }
}

fn main() {
    // Read input from stdin
    let mut input_str = String::new();
    io::stdin()
        .read_to_string(&mut input_str)
        .expect("Failed to read stdin");

    // Parse input
    let input: Input = serde_json::from_str(&input_str).expect("Failed to parse JSON input");

    // Deterministic computation
    let iterations = input.iterations.unwrap_or(1000);
    let mut rng = DeterministicRng::new(input.seed);

    let mut result = 0u64;
    for _i in 0..iterations {
        let val = rng.next();
        result = result.wrapping_add(val);
    }

    // Compute checksum for verification
    let checksum = format!("{:016x}", result);

    // Output result
    let output = Output {
        result,
        checksum,
        iterations_run: iterations,
    };

    let output_json = serde_json::to_string(&output).expect("Failed to serialize output");
    io::stdout()
        .write_all(output_json.as_bytes())
        .expect("Failed to write to stdout");

    // Flush stdout to ensure data is written before program exits
    io::stdout().flush().expect("Failed to flush stdout");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deterministic_rng() {
        let mut rng1 = DeterministicRng::new(42);
        let mut rng2 = DeterministicRng::new(42);

        for _ in 0..100 {
            assert_eq!(rng1.next(), rng2.next(), "RNG should be deterministic");
        }
    }

    #[test]
    fn test_different_seeds_produce_different_results() {
        let mut rng1 = DeterministicRng::new(1);
        let mut rng2 = DeterministicRng::new(2);

        let val1 = rng1.next();
        let val2 = rng2.next();

        assert_ne!(val1, val2, "Different seeds should produce different values");
    }
}
