//! Difficulty conversion utilities for Stratum protocol
//!
//! Handles conversion between Stratum difficulty values and Chainweb targets.

use crate::core::Target;
use num_bigint::BigUint;
use num_traits::Zero;

/// Maximum target value (lowest difficulty)
const MAX_TARGET: [u8; 32] = [0xFF; 32];

/// Convert a Stratum difficulty to a Chainweb target
pub fn difficulty_to_target(difficulty: u64) -> Target {
    if difficulty == 0 {
        return Target::from_bytes([0u8; 32]); // Impossible target
    }

    // Convert max target to BigUint
    let max_target_big = BigUint::from_bytes_be(&MAX_TARGET);

    // Calculate target = max_target / difficulty
    let difficulty_big = BigUint::from(difficulty);
    let target_big = &max_target_big / &difficulty_big;

    // Convert back to bytes, padding with zeros if necessary
    let target_bytes = target_big.to_bytes_be();
    let mut result = [0u8; 32];

    if target_bytes.len() <= 32 {
        let offset = 32 - target_bytes.len();
        result[offset..].copy_from_slice(&target_bytes);
    } else {
        // If somehow the target is larger than max, cap it at max
        result.copy_from_slice(&MAX_TARGET);
    }

    Target::from_bytes(result)
}

/// Convert a Chainweb target to approximate Stratum difficulty
pub fn target_to_difficulty(target: &Target) -> u64 {
    let target_bytes = target.as_bytes();

    // Check for zero target (infinite difficulty)
    if target_bytes.iter().all(|&b| b == 0) {
        return u64::MAX;
    }

    // Convert target to BigUint
    let target_big = BigUint::from_bytes_be(target_bytes);

    // Handle zero target
    if target_big.is_zero() {
        return u64::MAX;
    }

    // Convert max target to BigUint
    let max_target_big = BigUint::from_bytes_be(&MAX_TARGET);

    // Calculate difficulty = max_target / target
    let difficulty_big = &max_target_big / &target_big;

    // Convert to u64, capping at max value
    if difficulty_big > BigUint::from(u64::MAX) {
        u64::MAX
    } else {
        difficulty_big.try_into().unwrap_or(u64::MAX)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_difficulty_one_gives_max_target() {
        let target = difficulty_to_target(1);
        assert_eq!(target.as_bytes(), &MAX_TARGET);
    }

    #[test]
    fn test_higher_difficulty_gives_smaller_target() {
        let target_1 = difficulty_to_target(1);
        let target_2 = difficulty_to_target(2);

        // Convert to big integers for comparison
        let target_1_big = BigUint::from_bytes_be(target_1.as_bytes());
        let target_2_big = BigUint::from_bytes_be(target_2.as_bytes());

        assert!(target_2_big < target_1_big);
    }

    #[test]
    fn test_zero_difficulty_gives_zero_target() {
        let target = difficulty_to_target(0);
        assert_eq!(target.as_bytes(), &[0u8; 32]);
    }

    #[test]
    fn test_target_to_difficulty_max_target() {
        let max_target = Target::from_bytes(MAX_TARGET);
        let difficulty = target_to_difficulty(&max_target);
        assert_eq!(difficulty, 1);
    }

    #[test]
    fn test_target_to_difficulty_zero_target() {
        let zero_target = Target::from_bytes([0u8; 32]);
        let difficulty = target_to_difficulty(&zero_target);
        assert_eq!(difficulty, u64::MAX);
    }

    #[test]
    fn test_difficulty_conversion_roundtrip_approximate() {
        let original_difficulty = 1000u64;
        let target = difficulty_to_target(original_difficulty);
        let converted_difficulty = target_to_difficulty(&target);

        // Should be reasonably close (within 10% due to integer rounding)
        let ratio = if original_difficulty > converted_difficulty {
            original_difficulty as f64 / converted_difficulty as f64
        } else {
            converted_difficulty as f64 / original_difficulty as f64
        };

        assert!(
            ratio <= 1.1,
            "Difficulty conversion too inaccurate: {} -> {} (ratio: {})",
            original_difficulty,
            converted_difficulty,
            ratio
        );
    }

    #[test]
    fn test_difficulty_ordering_preserves_target_ordering() {
        let diff_low = 100u64;
        let diff_high = 1000u64;

        let target_low = difficulty_to_target(diff_low);
        let target_high = difficulty_to_target(diff_high);

        // Higher difficulty should give smaller target
        assert!(target_high.as_bytes() < target_low.as_bytes());
    }
}
