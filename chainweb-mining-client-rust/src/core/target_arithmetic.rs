//! Advanced target arithmetic for precise 256-bit operations
//!
//! This module provides high-precision arithmetic operations for mining targets,
//! including TargetWords for 256-bit operations and Level for difficulty calculations.

use crate::error::{Error, Result};
use num_bigint::{BigUint, ToBigUint};
use num_traits::{One, Zero, ToPrimitive};
use std::cmp::Ordering;
use std::fmt;

/// Number of 64-bit words in a 256-bit target
const TARGET_WORDS: usize = 4;

/// Maximum possible target value (2^256 - 1)
const MAX_TARGET_HEX: &str = "ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff";

/// Represents a 256-bit target as an array of 64-bit words for precise arithmetic
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TargetWords {
    /// Words stored in little-endian order (least significant word first)
    words: [u64; TARGET_WORDS],
}

impl TargetWords {
    /// Create a new TargetWords from 64-bit words (little-endian)
    pub fn from_words(words: [u64; TARGET_WORDS]) -> Self {
        Self { words }
    }

    /// Create from bytes (big-endian)
    pub fn from_bytes(bytes: [u8; 32]) -> Self {
        let mut words = [0u64; TARGET_WORDS];
        
        // Convert big-endian bytes to little-endian words
        for i in 0..TARGET_WORDS {
            let start = (TARGET_WORDS - 1 - i) * 8;
            words[i] = u64::from_be_bytes([
                bytes[start],
                bytes[start + 1],
                bytes[start + 2],
                bytes[start + 3],
                bytes[start + 4],
                bytes[start + 5],
                bytes[start + 6],
                bytes[start + 7],
            ]);
        }
        
        Self { words }
    }

    /// Convert to bytes (big-endian)
    pub fn to_bytes(&self) -> [u8; 32] {
        let mut bytes = [0u8; 32];
        
        // Convert little-endian words to big-endian bytes
        for i in 0..TARGET_WORDS {
            let word_bytes = self.words[i].to_be_bytes();
            let start = (TARGET_WORDS - 1 - i) * 8;
            bytes[start..start + 8].copy_from_slice(&word_bytes);
        }
        
        bytes
    }

    /// Create from a BigUint
    pub fn from_biguint(value: &BigUint) -> Result<Self> {
        // Check if value fits in 256 bits
        if value.bits() > 256 {
            return Err(Error::invalid_target("Value exceeds 256 bits"));
        }

        let bytes = value.to_bytes_be();
        let mut target_bytes = [0u8; 32];
        
        // Copy bytes, right-aligned
        let start = 32usize.saturating_sub(bytes.len());
        target_bytes[start..].copy_from_slice(&bytes);
        
        Ok(Self::from_bytes(target_bytes))
    }

    /// Convert to BigUint for arbitrary precision arithmetic
    pub fn to_biguint(&self) -> BigUint {
        BigUint::from_bytes_be(&self.to_bytes())
    }

    /// Get the maximum possible target
    pub fn max_target() -> Self {
        Self {
            words: [u64::MAX; TARGET_WORDS],
        }
    }

    /// Get zero target
    pub fn zero() -> Self {
        Self {
            words: [0u64; TARGET_WORDS],
        }
    }

    /// Compare two targets
    pub fn compare(&self, other: &TargetWords) -> Ordering {
        // Compare from most significant word to least
        for i in (0..TARGET_WORDS).rev() {
            match self.words[i].cmp(&other.words[i]) {
                Ordering::Equal => continue,
                other => return other,
            }
        }
        Ordering::Equal
    }

    /// Add two targets (with overflow detection)
    pub fn checked_add(&self, other: &TargetWords) -> Option<TargetWords> {
        let mut result = [0u64; TARGET_WORDS];
        let mut carry = 0u64;

        for i in 0..TARGET_WORDS {
            let (sum1, overflow1) = self.words[i].overflowing_add(other.words[i]);
            let (sum2, overflow2) = sum1.overflowing_add(carry);
            result[i] = sum2;
            carry = (overflow1 as u64) + (overflow2 as u64);
        }

        if carry > 0 {
            None // Overflow
        } else {
            Some(TargetWords::from_words(result))
        }
    }

    /// Subtract two targets (with underflow detection)
    pub fn checked_sub(&self, other: &TargetWords) -> Option<TargetWords> {
        let mut result = [0u64; TARGET_WORDS];
        let mut borrow = 0u64;

        for i in 0..TARGET_WORDS {
            let (diff1, borrow1) = self.words[i].overflowing_sub(other.words[i]);
            let (diff2, borrow2) = diff1.overflowing_sub(borrow);
            result[i] = diff2;
            borrow = (borrow1 as u64) + (borrow2 as u64);
        }

        if borrow > 0 {
            None // Underflow
        } else {
            Some(TargetWords::from_words(result))
        }
    }

    /// Multiply by a scalar (with overflow detection)
    pub fn checked_mul_scalar(&self, scalar: u64) -> Option<TargetWords> {
        let mut result = [0u64; TARGET_WORDS];
        let mut carry = 0u64;

        for i in 0..TARGET_WORDS {
            let product = self.words[i] as u128 * scalar as u128 + carry as u128;
            result[i] = product as u64;
            carry = (product >> 64) as u64;
        }

        if carry > 0 {
            None // Overflow
        } else {
            Some(TargetWords::from_words(result))
        }
    }

    /// Divide by a scalar
    pub fn div_scalar(&self, divisor: u64) -> Result<(TargetWords, u64)> {
        if divisor == 0 {
            return Err(Error::invalid_target("Division by zero"));
        }

        let mut result = [0u64; TARGET_WORDS];
        let mut remainder = 0u64;

        // Divide from most significant word to least
        for i in (0..TARGET_WORDS).rev() {
            let dividend = ((remainder as u128) << 64) | (self.words[i] as u128);
            result[i] = (dividend / divisor as u128) as u64;
            remainder = (dividend % divisor as u128) as u64;
        }

        Ok((TargetWords::from_words(result), remainder))
    }

    /// Shift left by n bits
    pub fn shl(&self, n: u32) -> TargetWords {
        if n >= 256 {
            return TargetWords::zero();
        }

        let word_shift = (n / 64) as usize;
        let bit_shift = n % 64;

        let mut result = [0u64; TARGET_WORDS];

        if bit_shift == 0 {
            // Simple word shift
            for i in word_shift..TARGET_WORDS {
                result[i] = self.words[i - word_shift];
            }
        } else {
            // Shift with carry
            for i in word_shift..TARGET_WORDS {
                let src_idx = i - word_shift;
                result[i] = self.words[src_idx] << bit_shift;
                if src_idx > 0 {
                    result[i] |= self.words[src_idx - 1] >> (64 - bit_shift);
                }
            }
        }

        TargetWords::from_words(result)
    }

    /// Shift right by n bits
    pub fn shr(&self, n: u32) -> TargetWords {
        if n >= 256 {
            return TargetWords::zero();
        }

        let word_shift = (n / 64) as usize;
        let bit_shift = n % 64;

        let mut result = [0u64; TARGET_WORDS];

        if bit_shift == 0 {
            // Simple word shift
            for i in 0..(TARGET_WORDS - word_shift) {
                result[i] = self.words[i + word_shift];
            }
        } else {
            // Shift with carry
            for i in 0..(TARGET_WORDS - word_shift) {
                let src_idx = i + word_shift;
                result[i] = self.words[src_idx] >> bit_shift;
                if src_idx + 1 < TARGET_WORDS {
                    result[i] |= self.words[src_idx + 1] << (64 - bit_shift);
                }
            }
        }

        TargetWords::from_words(result)
    }

    /// Count leading zeros
    pub fn leading_zeros(&self) -> u32 {
        for i in (0..TARGET_WORDS).rev() {
            if self.words[i] != 0 {
                return (TARGET_WORDS - 1 - i) as u32 * 64 + self.words[i].leading_zeros();
            }
        }
        256
    }

    /// Convert to hex string
    pub fn to_hex(&self) -> String {
        hex::encode(self.to_bytes())
    }
}

/// Difficulty level representation with precise arithmetic
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Level(u32);

impl Level {
    /// Create a new Level
    pub fn new(value: u32) -> Self {
        Self(value)
    }

    /// Get the raw level value
    pub fn value(&self) -> u32 {
        self.0
    }

    /// Convert level to target using the formula: target = max_target >> level
    pub fn to_target(&self) -> Result<TargetWords> {
        if self.0 >= 256 {
            return Ok(TargetWords::zero());
        }
        Ok(TargetWords::max_target().shr(self.0))
    }

    /// Convert target to level (approximate)
    pub fn from_target(target: &TargetWords) -> Self {
        let leading_zeros = target.leading_zeros();
        Self(leading_zeros)
    }

    /// Get the effective difficulty as 2^level
    pub fn to_difficulty(&self) -> BigUint {
        if self.0 >= 256 {
            BigUint::zero()
        } else {
            BigUint::one() << self.0
        }
    }
}

impl fmt::Display for TargetWords {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_hex())
    }
}

impl fmt::Display for Level {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Level({})", self.0)
    }
}

/// Advanced target arithmetic operations
pub struct TargetArithmetic;

impl TargetArithmetic {
    /// Calculate difficulty from target: difficulty = max_target / target
    pub fn difficulty_from_target(target: &TargetWords) -> Result<BigUint> {
        let max_target = TargetWords::max_target().to_biguint();
        let target_value = target.to_biguint();
        
        if target_value.is_zero() {
            return Err(Error::invalid_target("Target cannot be zero"));
        }
        
        Ok(max_target / target_value)
    }

    /// Calculate target from difficulty: target = max_target / difficulty
    pub fn target_from_difficulty(difficulty: &BigUint) -> Result<TargetWords> {
        if difficulty.is_zero() {
            return Err(Error::invalid_target("Difficulty cannot be zero"));
        }
        
        let max_target = TargetWords::max_target().to_biguint();
        let target_value = max_target / difficulty;
        
        TargetWords::from_biguint(&target_value)
    }

    /// Adjust target for new difficulty
    pub fn adjust_target(
        current_target: &TargetWords,
        time_taken: u64,
        expected_time: u64,
    ) -> Result<TargetWords> {
        if expected_time == 0 {
            return Err(Error::invalid_target("Expected time cannot be zero"));
        }

        let current = current_target.to_biguint();
        let adjusted = &current * time_taken.to_biguint().unwrap() / expected_time.to_biguint().unwrap();
        
        // Ensure we don't exceed max target
        let max_target = TargetWords::max_target().to_biguint();
        let clamped = if adjusted > max_target {
            max_target
        } else {
            adjusted
        };
        
        TargetWords::from_biguint(&clamped)
    }

    /// Calculate the probability of finding a block with given target and hash rate
    pub fn block_probability(
        target: &TargetWords,
        hash_rate: f64,
        time_seconds: f64,
    ) -> f64 {
        let target_ratio = target.to_biguint().to_f64().unwrap_or(0.0) / TargetWords::max_target().to_biguint().to_f64().unwrap_or(1.0);
        let attempts = hash_rate * time_seconds;
        
        // Probability = 1 - (1 - p)^n where p = target/max_target and n = attempts
        // For small p and large n, this approximates to 1 - e^(-p*n)
        let exponent = -target_ratio * attempts;
        1.0 - exponent.exp()
    }

    /// Calculate expected time to find a block
    pub fn expected_block_time(target: &TargetWords, hash_rate: f64) -> f64 {
        if hash_rate <= 0.0 {
            return f64::INFINITY;
        }

        let difficulty = Self::difficulty_from_target(target)
            .unwrap_or_else(|_| BigUint::one())
            .to_f64()
            .unwrap_or(1.0);
        
        difficulty / hash_rate
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_target_words_conversion() {
        let bytes = [0xFF; 32];
        let target = TargetWords::from_bytes(bytes);
        assert_eq!(target.to_bytes(), bytes);
    }

    #[test]
    fn test_target_words_arithmetic() {
        let one = TargetWords::from_words([1, 0, 0, 0]);
        let two = TargetWords::from_words([2, 0, 0, 0]);
        
        // Addition
        let three = one.checked_add(&two).unwrap();
        assert_eq!(three.words[0], 3);
        
        // Subtraction
        let one_again = three.checked_sub(&two).unwrap();
        assert_eq!(one_again, one);
        
        // Multiplication
        let four = two.checked_mul_scalar(2).unwrap();
        assert_eq!(four.words[0], 4);
        
        // Division
        let (two_again, remainder) = four.div_scalar(2).unwrap();
        assert_eq!(two_again, two);
        assert_eq!(remainder, 0);
    }

    #[test]
    fn test_target_words_shifts() {
        let one = TargetWords::from_words([1, 0, 0, 0]);
        
        // Left shift
        let two = one.shl(1);
        assert_eq!(two.words[0], 2);
        
        let large = one.shl(64);
        assert_eq!(large.words[0], 0);
        assert_eq!(large.words[1], 1);
        
        // Right shift
        let one_again = two.shr(1);
        assert_eq!(one_again, one);
    }

    #[test]
    fn test_level_conversion() {
        let level = Level::new(10);
        let target = level.to_target().unwrap();
        
        // Max target shifted right by 10 bits
        let expected = TargetWords::max_target().shr(10);
        assert_eq!(target, expected);
        
        // Convert back (approximately)
        let level_again = Level::from_target(&target);
        assert_eq!(level_again.value(), 10);
    }

    #[test]
    fn test_difficulty_calculations() {
        let max_target = TargetWords::max_target();
        let difficulty = TargetArithmetic::difficulty_from_target(&max_target).unwrap();
        assert_eq!(difficulty, BigUint::one());
        
        // Half target = double difficulty
        let half_target = max_target.shr(1);
        let double_difficulty = TargetArithmetic::difficulty_from_target(&half_target).unwrap();
        assert_eq!(double_difficulty, BigUint::from(2u32));
    }

    #[test]
    fn test_target_adjustment() {
        let current = TargetWords::from_words([1000, 0, 0, 0]);
        
        // If mining took twice as long, target should double (easier)
        let adjusted = TargetArithmetic::adjust_target(&current, 200, 100).unwrap();
        assert_eq!(adjusted.words[0], 2000);
        
        // If mining was twice as fast, target should halve (harder)
        let adjusted = TargetArithmetic::adjust_target(&current, 50, 100).unwrap();
        assert_eq!(adjusted.words[0], 500);
    }

    #[test]
    fn test_overflow_detection() {
        let max = TargetWords::max_target();
        
        // Addition overflow
        assert!(max.checked_add(&max).is_none());
        
        // Multiplication overflow
        assert!(max.checked_mul_scalar(2).is_none());
        
        // Subtraction underflow
        let zero = TargetWords::zero();
        let one = TargetWords::from_words([1, 0, 0, 0]);
        assert!(zero.checked_sub(&one).is_none());
    }
}

// Property-based tests
#[cfg(test)]
mod property_tests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn target_words_roundtrip(words in prop::array::uniform4(any::<u64>())) {
            let target = TargetWords::from_words(words);
            let bytes = target.to_bytes();
            let target2 = TargetWords::from_bytes(bytes);
            prop_assert_eq!(target, target2);
        }

        #[test]
        fn shift_inverse(
            words in prop::array::uniform4(any::<u64>()),
            shift in 0u32..256u32
        ) {
            let target = TargetWords::from_words(words);
            let shifted = target.shl(shift);
            let unshifted = shifted.shr(shift);
            
            // Due to information loss, we can only check if the remaining bits match
            let mask = if shift >= 256 {
                TargetWords::zero()
            } else {
                TargetWords::max_target().shr(shift).shl(shift)
            };
            
            let masked_original = TargetWords::from_words([
                target.words[0] & mask.words[0],
                target.words[1] & mask.words[1],
                target.words[2] & mask.words[2],
                target.words[3] & mask.words[3],
            ]);
            
            prop_assert_eq!(unshifted, masked_original);
        }

        #[test]
        fn level_target_consistency(level in 0u32..256u32) {
            let level_obj = Level::new(level);
            let target = level_obj.to_target().unwrap();
            let level_again = Level::from_target(&target);
            
            // Due to rounding, we might lose precision
            prop_assert!(level_again.value() >= level.saturating_sub(1));
            prop_assert!(level_again.value() <= level.saturating_add(1));
        }

        #[test]
        fn difficulty_target_inverse(difficulty in 1u64..=1_000_000u64) {
            let diff_biguint = BigUint::from(difficulty);
            let target = TargetArithmetic::target_from_difficulty(&diff_biguint).unwrap();
            let diff_again = TargetArithmetic::difficulty_from_target(&target).unwrap();
            
            // Due to integer division, we might lose precision
            let ratio = if difficulty > 1 {
                diff_again.to_u64().unwrap_or(0) as f64 / difficulty as f64
            } else {
                1.0
            };
            
            prop_assert!(ratio > 0.9 && ratio < 1.1);
        }
    }
}