//! Simple property-based tests for core mining data structures
//!
//! These tests verify basic invariants using proptest

use super::*;
use proptest::prelude::*;

// Basic property tests that should compile and run
proptest! {
    #[test]
    fn nonce_creation_consistency(value in any::<u64>()) {
        let nonce = Nonce::new(value);
        prop_assert_eq!(nonce.value(), value);
    }

    #[test]
    fn nonce_byte_roundtrip(value in any::<u64>()) {
        let nonce = Nonce::new(value);
        let bytes = nonce.to_le_bytes();
        let reconstructed = Nonce::from_le_bytes(bytes);
        prop_assert_eq!(nonce, reconstructed);
    }

    #[test]
    fn chain_id_creation(id in 0u16..=19u16) {
        let chain_id = ChainId::new(id);
        prop_assert_eq!(chain_id.value(), id);
    }

    #[test]
    fn target_roundtrip(bytes in prop::array::uniform32(any::<u8>())) {
        let target = Target::from_bytes(bytes);
        let retrieved_bytes = target.as_bytes();
        prop_assert_eq!(&bytes, retrieved_bytes);
    }

    #[test]
    fn target_difficulty_positive(bytes in prop::array::uniform32(1u8..=255u8)) {
        let target = Target::from_bytes(bytes);
        let difficulty = target.to_difficulty();
        prop_assert!(difficulty > 0.0);
    }

    #[test]
    fn work_roundtrip(bytes in prop::collection::vec(0u8..=255u8, constants::WORK_SIZE)) {
        let bytes_array: [u8; constants::WORK_SIZE] = bytes.try_into().unwrap();
        let work = Work::from_bytes(bytes_array);
        let retrieved_bytes = work.as_bytes();
        prop_assert_eq!(&bytes_array, retrieved_bytes);
    }

    #[test]
    fn work_nonce_modification(
        work_bytes in prop::collection::vec(0u8..=255u8, constants::WORK_SIZE),
        nonce_value in any::<u64>()
    ) {
        let bytes_array: [u8; constants::WORK_SIZE] = work_bytes.try_into().unwrap();
        let mut work = Work::from_bytes(bytes_array);
        let nonce = Nonce::new(nonce_value);
        work.set_nonce(nonce);
        prop_assert_eq!(work.nonce(), nonce);
    }

    #[test]
    fn work_hash_deterministic(bytes in prop::collection::vec(0u8..=255u8, constants::WORK_SIZE)) {
        let bytes_array: [u8; constants::WORK_SIZE] = bytes.try_into().unwrap();
        let work = Work::from_bytes(bytes_array);
        let hash1 = work.hash();
        let hash2 = work.hash();
        prop_assert_eq!(hash1, hash2);
    }

    #[test]
    fn work_hash_changes_with_nonce(
        work_bytes in prop::collection::vec(0u8..=255u8, constants::WORK_SIZE),
        nonce1 in any::<u64>(),
        nonce2 in any::<u64>()
    ) {
        prop_assume!(nonce1 != nonce2);

        let bytes_array: [u8; constants::WORK_SIZE] = work_bytes.try_into().unwrap();
        let mut work1 = Work::from_bytes(bytes_array);
        let mut work2 = Work::from_bytes(bytes_array);

        work1.set_nonce(Nonce::new(nonce1));
        work2.set_nonce(Nonce::new(nonce2));

        let hash1 = work1.hash();
        let hash2 = work2.hash();

        prop_assert_ne!(hash1, hash2);
    }
}

// Property tests for preemption behavior
proptest! {
    #[test]
    fn preemption_identical_work_detection(
        work_bytes in prop::collection::vec(0u8..=255u8, constants::WORK_SIZE)
    ) {
        use crate::core::{PreemptionConfig, WorkPreemptor, PreemptionDecision};
        use crate::core::preemption::PreemptionSkipReason;

        let bytes_array: [u8; constants::WORK_SIZE] = work_bytes.try_into().unwrap();
        let preemptor = WorkPreemptor::new(PreemptionConfig::default());
        let work = Work::from_bytes(bytes_array);
        let decision = preemptor.should_preempt(&work, &work);

        prop_assert_eq!(decision, PreemptionDecision::Skip(PreemptionSkipReason::IdenticalWork));
    }

    #[test]
    fn preemption_different_work_triggers(
        work1_bytes in prop::collection::vec(0u8..=255u8, constants::WORK_SIZE),
        work2_bytes in prop::collection::vec(0u8..=255u8, constants::WORK_SIZE)
    ) {
        use crate::core::{PreemptionConfig, PreemptionStrategy, WorkPreemptor, PreemptionDecision};
        use crate::core::preemption::PreemptionAction;

        prop_assume!(work1_bytes != work2_bytes);

        let bytes1_array: [u8; constants::WORK_SIZE] = work1_bytes.try_into().unwrap();
        let bytes2_array: [u8; constants::WORK_SIZE] = work2_bytes.try_into().unwrap();

        let config = PreemptionConfig {
            strategy: PreemptionStrategy::Immediate,
            ..Default::default()
        };
        let preemptor = WorkPreemptor::new(config);
        let work1 = Work::from_bytes(bytes1_array);
        let work2 = Work::from_bytes(bytes2_array);

        let decision = preemptor.should_preempt(&work2, &work1);

        prop_assert_eq!(decision, PreemptionDecision::Preempt(PreemptionAction::Immediate));
    }
}
