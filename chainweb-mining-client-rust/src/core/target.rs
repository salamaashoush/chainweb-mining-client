//! Target type for mining difficulty

use crate::core::Work;
use crate::error::{Error, Result};
use blake2::{Blake2s256, Digest};
use serde::{Deserialize, Serialize};
use std::fmt;

/// Represents a 256-bit mining target (difficulty threshold)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Target(pub [u8; 32]);

impl Target {
    /// Create a new Target from bytes (big-endian)
    pub fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    /// Create a Target from little-endian bytes
    pub fn from_bytes_le(bytes: &[u8]) -> Result<Self> {
        if bytes.len() != 32 {
            return Err(Error::invalid_target(format!(
                "Expected 32 bytes, got {}",
                bytes.len()
            )));
        }

        let mut array = [0u8; 32];
        array.copy_from_slice(bytes);

        // Reverse bytes to convert from little-endian to big-endian
        array.reverse();

        Ok(Self(array))
    }

    /// Create a Target from little-endian bytes without reversing (direct copy)
    pub fn from_le_bytes(bytes: [u8; 32]) -> Self {
        // Convert little-endian to big-endian for internal storage
        let mut reversed = bytes;
        reversed.reverse();
        Self(reversed)
    }

    /// Create a Target from a hex string
    pub fn from_hex(hex: &str) -> Result<Self> {
        let bytes =
            hex::decode(hex).map_err(|e| Error::invalid_target(format!("Invalid hex: {}", e)))?;

        if bytes.len() != 32 {
            return Err(Error::invalid_target(format!(
                "Expected 32 bytes, got {}",
                bytes.len()
            )));
        }

        let mut array = [0u8; 32];
        array.copy_from_slice(&bytes);
        Ok(Self(array))
    }

    /// Get the target as bytes
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    /// Convert to hex string
    pub fn to_hex(&self) -> String {
        hex::encode(self.0)
    }

    /// Check if a hash meets this target (is below it)
    pub fn meets_target(&self, hash: &[u8; 32]) -> bool {
        // Compare as big-endian integers
        for (hash_byte, target_byte) in hash.iter().zip(self.0.iter()) {
            match hash_byte.cmp(target_byte) {
                std::cmp::Ordering::Less => return true,
                std::cmp::Ordering::Greater => return false,
                std::cmp::Ordering::Equal => continue,
            }
        }
        // Equal hashes technically don't meet the target
        false
    }

    /// Convert difficulty to target
    /// Difficulty = max_target / target
    pub fn from_difficulty(difficulty: f64) -> Result<Self> {
        if difficulty <= 0.0 {
            return Err(Error::invalid_target("Difficulty must be positive"));
        }

        // Max target is 2^256 - 1
        // For practical purposes, we'll use a simplified conversion
        let target_value = (u64::MAX as f64) / difficulty;
        let target_u64 = target_value as u64;

        let mut bytes = [0xFFu8; 32]; // Start with max target
        bytes[24..32].copy_from_slice(&target_u64.to_be_bytes());

        Ok(Self(bytes))
    }

    /// Create a target with a specific number of leading zero bits
    pub fn from_difficulty_bits(leading_zeros: u32) -> Self {
        let mut bytes = [0xFFu8; 32];

        if leading_zeros >= 256 {
            return Self([0u8; 32]);
        }

        let zero_bytes = (leading_zeros / 8) as usize;
        let remaining_bits = (leading_zeros % 8) as u8;

        // Set full zero bytes
        for byte in bytes.iter_mut().take(zero_bytes.min(32)) {
            *byte = 0;
        }

        // Set partial byte if needed
        if zero_bytes < 32 && remaining_bits > 0 {
            bytes[zero_bytes] = 0xFF >> remaining_bits;
        }

        Self(bytes)
    }

    /// Get difficulty from target
    pub fn to_difficulty(&self) -> f64 {
        // Simplified difficulty calculation
        let mut value = 0u64;
        for &byte in &self.0[24..32] {
            value = (value << 8) | (byte as u64);
        }

        if value == 0 {
            return f64::MAX;
        }

        (u64::MAX as f64) / (value as f64)
    }
}

/// Check if a work meets the given target using Blake2s-256 hash
///
/// This function replicates the Haskell checkTarget logic:
/// 1. Hash the work with Blake2s-256
/// 2. Treat the hash as a 256-bit little-endian integer  
/// 3. Compare hash ≤ target
#[allow(dead_code)]
pub fn check_target(target: &Target, work: &Work) -> Result<bool> {
    // Hash the work with Blake2s-256
    let mut hasher = Blake2s256::new();
    hasher.update(work.as_bytes());
    let hash_bytes = hasher.finalize();

    // Convert little-endian hash to Target (matching Haskell logic)
    let hash_target = Target::from_le_bytes(hash_bytes.into());

    // Compare targets: hash_target <= target
    Ok(hash_target.0 <= target.0)
}

impl fmt::Display for Target {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_hex())
    }
}

impl Serialize for Target {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_hex())
    }
}

impl<'de> Deserialize<'de> for Target {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let hex = String::deserialize(deserializer)?;
        Self::from_hex(&hex).map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_target_from_bytes() {
        let bytes = [0x01; 32];
        let target = Target::from_bytes(bytes);
        assert_eq!(target.as_bytes(), &bytes);
    }

    #[test]
    fn test_target_hex_conversion() {
        let hex = "0000000000000000000000000000000000000000000000000000000000000001";
        let target = Target::from_hex(hex).unwrap();
        assert_eq!(target.to_hex(), hex);
    }

    #[test]
    fn test_target_meets_target() {
        let target_bytes = [
            0x00, 0x00, 0x00, 0x0F, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
            0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
            0xFF, 0xFF, 0xFF, 0xFF,
        ];
        let target = Target::from_bytes(target_bytes);

        // Hash that meets target
        let good_hash = [
            0x00, 0x00, 0x00, 0x0E, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
            0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
            0xFF, 0xFF, 0xFF, 0xFF,
        ];
        assert!(target.meets_target(&good_hash));

        // Hash that doesn't meet target
        let bad_hash = [
            0x00, 0x00, 0x00, 0x10, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00,
        ];
        assert!(!target.meets_target(&bad_hash));

        // Equal hash doesn't meet target
        assert!(!target.meets_target(&target_bytes));
    }

    #[test]
    fn test_target_serde() {
        let hex = "00000000ffff0000000000000000000000000000000000000000000000000000";
        let target = Target::from_hex(hex).unwrap();

        let json = serde_json::to_string(&target).unwrap();
        assert_eq!(json, format!("\"{}\"", hex));

        let deserialized: Target = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, target);
    }

    #[test]
    fn test_invalid_target_hex() {
        assert!(Target::from_hex("invalid").is_err());
        assert!(Target::from_hex("00").is_err()); // Too short
        assert!(Target::from_hex(&"00".repeat(33)).is_err()); // Too long
    }

    #[test]
    fn test_target_from_bytes_le() {
        // Test with a simple pattern
        let mut le_bytes = [0u8; 32];
        le_bytes[0] = 0x01; // Least significant byte
        le_bytes[31] = 0xFF; // Most significant byte

        let target = Target::from_bytes_le(&le_bytes).unwrap();

        // After reversal, 0xFF should be at index 0 and 0x01 at index 31
        assert_eq!(target.as_bytes()[0], 0xFF);
        assert_eq!(target.as_bytes()[31], 0x01);

        // Test with all zeros
        let zero_bytes = [0u8; 32];
        let zero_target = Target::from_bytes_le(&zero_bytes).unwrap();
        assert_eq!(zero_target.as_bytes(), &[0u8; 32]);

        // Test with invalid length
        assert!(Target::from_bytes_le(&[0u8; 31]).is_err());
        assert!(Target::from_bytes_le(&[0u8; 33]).is_err());
    }
}
