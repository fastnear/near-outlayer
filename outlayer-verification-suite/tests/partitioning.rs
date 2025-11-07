//! Property Test: Batch Partitioning Metamorphic Test
//!
//! **Property**: Applying receipts in one batch vs partitioned batches → identical final state
//!
//! This metamorphic test verifies that our deterministic execution model is truly
//! order-preserving and batch-agnostic. If you have a sequence of receipts [R1, R2, R3, R4]:
//!
//! - Apply all 4 at once: S0 → R1 → R2 → R3 → R4 → S_final
//! - Apply in chunks [R1,R2] then [R3,R4]: S0 → R1 → R2 → S_mid → R3 → R4 → S_final
//!
//! Both paths MUST yield identical S_final (bit-for-bit).
//!
//! This models NEAR's async receipt model where receipts may be processed in different
//! blocks/chunks but must maintain deterministic ordering within a shard.

use outlayer_verification_suite::strategies::*;
use outlayer_verification_suite::*;
use proptest::prelude::*;

/// Apply receipts sequentially to state
fn apply_seq(
    mut state: SealedState,
    tee: &MockOutlayerTEE,
    seq: &[Receipt],
) -> Result<SealedState, EnclaveError> {
    for r in seq {
        let out = tee.execute(state, r.clone())?;
        state = out.new_state;
    }
    Ok(state)
}

/// Partition indices into ranges based on chunk sizes
fn partition_indices(n: usize, cuts: &[usize]) -> Vec<std::ops::Range<usize>> {
    let mut last = 0;
    let mut ranges = Vec::new();
    for &c in cuts {
        let end = (last + c).min(n);
        if last < end {
            ranges.push(last..end);
        }
        last = end;
        if last == n {
            break;
        }
    }
    if last < n {
        ranges.push(last..n);
    }
    ranges
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: 256,
        max_shrink_iters: 1000,
        .. ProptestConfig::default()
    })]

    /// Apply receipts in different batch sizes - final state must be identical
    #[test]
    fn batch_partition_equivalence(
        state in arb_sealed_state(),
        seq in prop::collection::vec(arb_receipt(), 0..64),
        chunks in prop::collection::vec(1usize..8, 0..8)
    ) {
        let tee = mock_tee();

        // Apply all receipts in one shot
        let s1 = apply_seq(state.clone(), &tee, &seq);

        // Apply same receipts but partitioned into batches
        let mut s2 = Ok(state.clone());
        let parts = partition_indices(seq.len(), &chunks);
        for range in parts {
            if s2.is_err() {
                break;
            }
            let st = s2.unwrap();
            s2 = apply_seq(st, &tee, &seq[range]);
        }

        // CRITICAL: Both paths must yield identical final state
        prop_assert_eq!(
            s1, s2,
            "Same ordered receipts must yield same final state regardless of batching. \
             Sequence length: {}, Partitions: {:?}",
            seq.len(),
            partition_indices(seq.len(), &chunks)
        );
    }

    /// Empty sequence should be no-op
    #[test]
    fn empty_sequence_is_noop(state in arb_sealed_state()) {
        let tee = mock_tee();
        let empty_seq: Vec<Receipt> = vec![];

        let result = apply_seq(state.clone(), &tee, &empty_seq);

        prop_assert_eq!(
            result,
            Ok(state.clone()),
            "Empty receipt sequence should not modify state"
        );
    }

    /// Single receipt should be identical to batch of size 1
    #[test]
    fn single_receipt_no_batching_effect(
        state in arb_sealed_state(),
        receipt in arb_receipt()
    ) {
        let tee = mock_tee();

        // Execute directly
        let r1 = tee.execute(state.clone(), receipt.clone());

        // Execute via apply_seq helper
        let r2 = apply_seq(state.clone(), &tee, &[receipt.clone()]);

        prop_assert_eq!(
            r1.map(|out| out.new_state),
            r2,
            "Single receipt execution should match batch execution"
        );
    }
}

#[cfg(test)]
mod unit_tests {
    use super::*;

    #[test]
    fn test_partition_indices() {
        // Empty
        assert_eq!(
            partition_indices(0, &[]),
            Vec::<std::ops::Range<usize>>::new()
        );

        // Single partition
        assert_eq!(partition_indices(10, &[10]), vec![0..10]);

        // Multiple partitions
        assert_eq!(partition_indices(10, &[3, 3, 4]), vec![0..3, 3..6, 6..10]);

        // Oversized chunks (should cap at n)
        assert_eq!(partition_indices(5, &[10, 10]), vec![0..5]);

        // Many small chunks
        assert_eq!(
            partition_indices(10, &[1, 1, 1, 1]),
            vec![0..1, 1..2, 2..3, 3..4, 4..10]
        );
    }
}
