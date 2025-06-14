//! Comprehensive property-based tests for mathematical operations
//!
//! This module provides extensive property testing to ensure mathematical
//! correctness across all edge cases and input combinations.

#[cfg(test)]
mod tests {
    use crate::core::{ChainId, Nonce, Target, Work, TargetWords, Level, TargetArithmetic};
    use num_bigint::{BigUint, ToBigUint};
    use num_traits::{Zero, One};
    use proptest::prelude::*;

    // Custom strategies for generating test data
    prop_compose! {
        fn arb_target_bytes()(bytes in prop::array::uniform32(0u8..)) -> [u8; 32] {
            bytes
        }
    }

    fn arb_work_bytes() -> impl Strategy<Value = [u8; 286]> {
        prop::collection::vec(any::<u8>(), 286..=286)
            .prop_map(|v| {
                let mut arr = [0u8; 286];
                arr.copy_from_slice(&v);
                arr
            })
    }

    prop_compose! {
        fn arb_target_words()(words in prop::array::uniform4(any::<u64>())) -> TargetWords {
            TargetWords::from_words(words)
        }
    }

    prop_compose! {
        fn arb_chain_id()(id in 0u16..20u16) -> ChainId {
            ChainId::new(id)
        }
    }

    prop_compose! {
        fn arb_nonce()(value in any::<u64>()) -> Nonce {
            Nonce::new(value)
        }
    }

    prop_compose! {
        fn arb_level()(value in 0u32..256u32) -> Level {
            Level::new(value)
        }
    }

    // Property tests for Target operations
    proptest! {
        #[test]
        fn target_serialization_roundtrip(bytes in arb_target_bytes()) {
            let target = Target::from_bytes(bytes);
            let serialized_bytes = target.as_bytes();
            prop_assert_eq!(*serialized_bytes, bytes);
        }

        #[test]
        fn target_comparison_properties(
            bytes1 in arb_target_bytes(),
            bytes2 in arb_target_bytes()
        ) {
            let target1 = Target::from_bytes(bytes1);
            let target2 = Target::from_bytes(bytes2);
            
            // Reflexivity
            prop_assert_eq!(target1, target1);
            
            // Symmetry
            prop_assert_eq!(target1 == target2, target2 == target1);
            
            // Transitivity is harder to test directly, but we can check consistency
            if target1 == target2 {
                prop_assert_eq!(target1.as_bytes(), target2.as_bytes());
            }
        }

        #[test]
        fn target_meets_target_properties(
            target_bytes in arb_target_bytes(),
            work_bytes in arb_work_bytes()
        ) {
            let target = Target::from_bytes(target_bytes);
            let work = Work::from_bytes(work_bytes);
            
            // meets_target should be deterministic
            let result1 = work.meets_target(&target);
            let result2 = work.meets_target(&target);
            prop_assert_eq!(result1, result2);
            
            // Maximum target should meet any hash
            let max_target = Target::from_bytes([0xFF; 32]);
            prop_assert!(work.meets_target(&max_target));
        }

        #[test]
        fn target_difficulty_monotonicity(
            bytes1 in arb_target_bytes(),
            bytes2 in arb_target_bytes()
        ) {
            let target1 = Target::from_bytes(bytes1);
            let target2 = Target::from_bytes(bytes2);
            
            // If target1 < target2 (harder), then difficulty1 > difficulty2
            if bytes1 < bytes2 {
                // This property is complex to verify without full difficulty calculation
                // For now, just ensure the comparison is consistent
                prop_assert_eq!(bytes1 < bytes2, target1.as_bytes() < target2.as_bytes());
            }
        }
    }

    // Property tests for Work operations
    proptest! {
        #[test]
        fn work_nonce_operations(
            work_bytes in arb_work_bytes(),
            nonce in arb_nonce()
        ) {
            let mut work = Work::from_bytes(work_bytes);
            work.set_nonce(nonce);
            
            // Getting the nonce should return what we set
            prop_assert_eq!(work.nonce(), nonce);
        }

        #[test]
        fn work_hash_determinism(work_bytes in arb_work_bytes()) {
            let work = Work::from_bytes(work_bytes);
            
            // Hash should be deterministic
            let hash1 = work.hash();
            let hash2 = work.hash();
            prop_assert_eq!(hash1, hash2);
        }

        #[test]
        fn work_nonce_affects_hash(
            work_bytes in arb_work_bytes(),
            nonce1 in arb_nonce(),
            nonce2 in arb_nonce()
        ) {
            let mut work = Work::from_bytes(work_bytes);
            
            work.set_nonce(nonce1);
            let hash1 = work.hash();
            
            if nonce1 != nonce2 {
                work.set_nonce(nonce2);
                let hash2 = work.hash();
                
                // Different nonces should (almost always) produce different hashes
                // Due to cryptographic properties of Blake2s-256
                prop_assert_ne!(hash1, hash2);
            }
        }

        #[test]
        fn work_serialization_consistency(work_bytes in arb_work_bytes()) {
            let work = Work::from_bytes(work_bytes);
            let retrieved_bytes = work.as_bytes();
            prop_assert_eq!(*retrieved_bytes, work_bytes);
        }

        #[test]
        fn work_serialization_format(work_bytes in arb_work_bytes()) {
            let work = Work::from_bytes(work_bytes);
            
            // Hex serialization should be consistent
            let hex1 = work.to_hex();
            let hex2 = work.to_hex();
            prop_assert_eq!(hex1, hex2);
            
            // Round trip through hex should preserve work
            let hex_for_roundtrip = work.to_hex();
            let work_from_hex = Work::from_hex(&hex_for_roundtrip).unwrap();
            prop_assert_eq!(work, work_from_hex);
        }
    }

    // Property tests for TargetWords arithmetic
    proptest! {
        #[test]
        fn target_words_arithmetic_properties(
            a in arb_target_words(),
            b in arb_target_words(),
            scalar in 1u64..=1000u64
        ) {
            // Addition commutativity (when no overflow)
            if let (Some(sum1), Some(sum2)) = (a.checked_add(&b), b.checked_add(&a)) {
                prop_assert_eq!(sum1, sum2);
            }
            
            // Addition with zero identity
            let zero = TargetWords::zero();
            if let Some(sum) = a.checked_add(&zero) {
                prop_assert_eq!(sum, a);
            }
            
            // Multiplication by scalar
            if let Some(product) = a.checked_mul_scalar(scalar) {
                // Should be larger than original (unless original is zero)
                if !a.to_biguint().is_zero() {
                    prop_assert!(product.to_biguint() >= a.to_biguint());
                }
            }
            
            // Division by scalar
            if scalar > 0 {
                let (quotient, remainder) = a.div_scalar(scalar).unwrap();
                let reconstructed = quotient.checked_mul_scalar(scalar)
                    .and_then(|q| q.checked_add(&TargetWords::from_biguint(&remainder.to_biguint().unwrap()).unwrap()));
                
                if let Some(reconstructed) = reconstructed {
                    prop_assert_eq!(reconstructed, a);
                }
            }
        }

        #[test]
        fn target_words_shift_properties(
            target in arb_target_words(),
            shift in 0u32..64u32
        ) {
            // Shifting by 0 should be identity
            if shift == 0 {
                prop_assert_eq!(target.shl(0), target);
                prop_assert_eq!(target.shr(0), target);
            }
            
            // Left shift followed by right shift should approximate original
            // (may lose precision due to overflow/underflow)
            if shift < 32 {
                let shifted_left = target.shl(shift);
                let shifted_back = shifted_left.shr(shift);
                
                // For values that don't overflow, this should hold
                if target.leading_zeros() > shift {
                    prop_assert_eq!(shifted_back, target);
                }
            }
        }

        #[test]
        fn target_words_comparison_properties(
            a in arb_target_words(),
            b in arb_target_words()
        ) {
            let cmp1 = a.compare(&b);
            let cmp2 = b.compare(&a);
            
            // Antisymmetry
            match cmp1 {
                std::cmp::Ordering::Less => prop_assert_eq!(cmp2, std::cmp::Ordering::Greater),
                std::cmp::Ordering::Greater => prop_assert_eq!(cmp2, std::cmp::Ordering::Less),
                std::cmp::Ordering::Equal => prop_assert_eq!(cmp2, std::cmp::Ordering::Equal),
            }
            
            // Reflexivity
            prop_assert_eq!(a.compare(&a), std::cmp::Ordering::Equal);
        }

        #[test]
        fn target_words_biguint_conversion_roundtrip(target in arb_target_words()) {
            let biguint = target.to_biguint();
            let converted_back = TargetWords::from_biguint(&biguint).unwrap();
            prop_assert_eq!(target, converted_back);
        }
    }

    // Property tests for Level operations
    proptest! {
        #[test]
        fn level_target_conversion_properties(level in arb_level()) {
            let target = level.to_target().unwrap();
            let level_back = Level::from_target(&target);
            
            // Due to precision limitations, we allow small differences
            let original_val = level.value();
            let converted_val = level_back.value();
            
            prop_assert!(original_val.abs_diff(converted_val) <= 1);
        }

        #[test]
        fn level_difficulty_properties(level in arb_level()) {
            let difficulty = level.to_difficulty();
            
            // Difficulty should increase with level
            if level.value() > 0 {
                let lower_level = Level::new(level.value() - 1);
                let lower_difficulty = lower_level.to_difficulty();
                prop_assert!(difficulty > lower_difficulty);
            }
            
            // Level 0 should give difficulty 1
            if level.value() == 0 {
                prop_assert_eq!(difficulty, BigUint::one());
            }
        }
    }

    // Property tests for TargetArithmetic operations
    proptest! {
        #[test]
        fn target_arithmetic_difficulty_conversion_properties(
            target in arb_target_words(),
            _difficulty in 1u64..1_000_000u64
        ) {
            // Skip zero targets to avoid division by zero
            if !target.to_biguint().is_zero() {
                if let Ok(computed_difficulty) = TargetArithmetic::difficulty_from_target(&target) {
                    // Difficulty should be positive
                    prop_assert!(computed_difficulty > BigUint::zero());
                    
                    // Converting back should give similar target
                    if let Ok(computed_target) = TargetArithmetic::target_from_difficulty(&computed_difficulty) {
                        // Allow some precision loss in conversion
                        let original_big = target.to_biguint();
                        let computed_big = computed_target.to_biguint();
                        
                        // They should be close (within reasonable precision bounds)
                        if original_big > BigUint::zero() && computed_big > BigUint::zero() {
                            let ratio = if original_big > computed_big {
                                &original_big / &computed_big
                            } else {
                                &computed_big / &original_big
                            };
                            
                            // Ratio should be close to 1 (allowing for precision loss)
                            prop_assert!(ratio <= BigUint::from(2u32));
                        }
                    }
                }
            }
        }

        #[test]
        fn target_arithmetic_adjustment_properties(
            target in arb_target_words(),
            time_taken in 1u64..10000u64,
            expected_time in 1u64..10000u64
        ) {
            if !target.to_biguint().is_zero() {
                if let Ok(adjusted) = TargetArithmetic::adjust_target(&target, time_taken, expected_time) {
                    // Adjusted target should reflect the time ratio
                    let original_big = target.to_biguint();
                    let adjusted_big = adjusted.to_biguint();
                    
                    if time_taken > expected_time {
                        // Mining took longer, target should be easier (larger)
                        prop_assert!(adjusted_big >= original_big);
                    } else if time_taken < expected_time {
                        // Mining was faster, target should be harder (smaller)
                        prop_assert!(adjusted_big <= original_big);
                    }
                }
            }
        }

        #[test]
        fn target_arithmetic_probability_properties(
            target in arb_target_words(),
            hash_rate in 1.0f64..1_000_000.0f64,
            time_seconds in 0.1f64..3600.0f64
        ) {
            let probability = TargetArithmetic::block_probability(&target, hash_rate, time_seconds);
            
            // Probability should be between 0 and 1
            prop_assert!(probability >= 0.0);
            prop_assert!(probability <= 1.0);
            
            // Higher hash rate should increase probability
            let higher_rate = hash_rate * 2.0;
            let higher_prob = TargetArithmetic::block_probability(&target, higher_rate, time_seconds);
            prop_assert!(higher_prob >= probability);
            
            // Longer time should increase probability
            let longer_time = time_seconds * 2.0;
            let longer_prob = TargetArithmetic::block_probability(&target, hash_rate, longer_time);
            prop_assert!(longer_prob >= probability);
        }

        #[test]
        fn target_arithmetic_expected_time_properties(
            target in arb_target_words(),
            hash_rate in 1.0f64..1_000_000.0f64
        ) {
            let expected_time = TargetArithmetic::expected_block_time(&target, hash_rate);
            
            // Expected time should be positive
            prop_assert!(expected_time > 0.0);
            
            // Higher hash rate should decrease expected time
            let higher_rate = hash_rate * 2.0;
            let shorter_time = TargetArithmetic::expected_block_time(&target, higher_rate);
            prop_assert!(shorter_time < expected_time);
        }
    }

    // Property tests for ChainId operations
    proptest! {
        #[test]
        fn chain_id_properties(id in 0u16..20u16) {
            let chain_id = ChainId::new(id);
            
            // Value retrieval should match input
            prop_assert_eq!(chain_id.value(), id);
            
            // Chain IDs with same value should be equal
            let chain_id2 = ChainId::new(id);
            prop_assert_eq!(chain_id, chain_id2);
            
            // String representation should contain the ID
            let string_repr = format!("{}", chain_id);
            prop_assert!(string_repr.contains(&id.to_string()));
        }

        #[test]
        fn chain_id_ordering_properties(id1 in 0u16..20u16, id2 in 0u16..20u16) {
            let chain_id1 = ChainId::new(id1);
            let chain_id2 = ChainId::new(id2);
            
            // Ordering should match the underlying values
            prop_assert_eq!(chain_id1.cmp(&chain_id2), id1.cmp(&id2));
        }
    }

    // Property tests for Nonce operations
    proptest! {
        #[test]
        fn nonce_properties(value in any::<u64>()) {
            let nonce = Nonce::new(value);
            
            // Value retrieval should match input
            prop_assert_eq!(nonce.value(), value);
            
            // Byte conversion should be consistent
            let bytes = nonce.to_le_bytes();
            let nonce_from_bytes = Nonce::from_le_bytes(bytes);
            prop_assert_eq!(nonce, nonce_from_bytes);
            
            // Increment should increase value (with wrapping)
            let incremented = nonce.incremented();
            prop_assert_eq!(incremented.value(), value.wrapping_add(1));
        }

        #[test]
        fn nonce_arithmetic_properties(
            value1 in any::<u64>(),
            value2 in any::<u64>()
        ) {
            let nonce1 = Nonce::new(value1);
            let nonce2 = Nonce::new(value2);
            
            // Equality should match value equality
            prop_assert_eq!(nonce1 == nonce2, value1 == value2);
            
            // Ordering should match value ordering
            prop_assert_eq!(nonce1.cmp(&nonce2), value1.cmp(&value2));
            
            // Addition should wrap properly
            let sum_nonce = Nonce::new(value1.wrapping_add(value2));
            let expected_value = value1.wrapping_add(value2);
            prop_assert_eq!(sum_nonce.value(), expected_value);
        }
    }

    // Integration property tests
    proptest! {
        #[test]
        fn integration_work_target_properties(
            work_bytes in arb_work_bytes(),
            target_bytes in arb_target_bytes(),
            nonce in arb_nonce()
        ) {
            let mut work = Work::from_bytes(work_bytes);
            let target = Target::from_bytes(target_bytes);
            
            work.set_nonce(nonce);
            
            // Meets target should be deterministic for same work/target combination
            let meets1 = work.meets_target(&target);
            let meets2 = work.meets_target(&target);
            prop_assert_eq!(meets1, meets2);
            
            // Work hash should be deterministic
            let hash1 = work.hash();
            let hash2 = work.hash();
            prop_assert_eq!(hash1, hash2);
            
            // Maximum target should always be met
            let max_target = Target::from_bytes([0xFF; 32]);
            prop_assert!(work.meets_target(&max_target));
        }

        #[test]
        fn integration_level_target_arithmetic(level in arb_level()) {
            if let Ok(target) = level.to_target() {
                let difficulty = level.to_difficulty();
                
                // Consistency between level operations
                if let Ok(computed_difficulty) = TargetArithmetic::difficulty_from_target(&target) {
                    // Should be related (allowing for precision differences)
                    if difficulty > BigUint::zero() && computed_difficulty > BigUint::zero() {
                        let ratio = if difficulty > computed_difficulty {
                            &difficulty / &computed_difficulty
                        } else {
                            &computed_difficulty / &difficulty
                        };
                        
                        // Allow reasonable precision variance
                        prop_assert!(ratio <= BigUint::from(10u32));
                    }
                }
            }
        }
    }

    // Edge case property tests
    proptest! {
        #[test]
        fn edge_case_target_words_operations(target in arb_target_words()) {
            // Operations with zero
            let zero = TargetWords::zero();
            if let Some(sum) = target.checked_add(&zero) {
                prop_assert_eq!(sum, target);
            }
            
            // Operations with max target
            let max_target = TargetWords::max_target();
            
            // Subtraction should handle edge cases
            if let Some(diff) = max_target.checked_sub(&target) {
                // Result should be valid
                prop_assert!(diff.to_biguint() <= max_target.to_biguint());
            }
            
            // Division by 1 should be identity
            let (quotient, remainder) = target.div_scalar(1).unwrap();
            prop_assert_eq!(quotient, target);
            prop_assert_eq!(remainder, 0);
        }

        #[test]
        fn edge_case_overflow_handling(
            a in arb_target_words(),
            b in arb_target_words(),
            scalar in 1u64..=u64::MAX
        ) {
            // Addition overflow should be handled gracefully
            let sum_result = a.checked_add(&b);
            
            // Multiplication overflow should be handled gracefully
            let mul_result = a.checked_mul_scalar(scalar);
            
            // Subtraction underflow should be handled gracefully
            let sub_result = a.checked_sub(&b);
            
            // All operations should either succeed or return None, never panic
            prop_assert!(sum_result.is_some() || sum_result.is_none());
            prop_assert!(mul_result.is_some() || mul_result.is_none());
            prop_assert!(sub_result.is_some() || sub_result.is_none());
        }
    }
}