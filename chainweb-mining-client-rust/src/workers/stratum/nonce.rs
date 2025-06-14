//! Advanced nonce handling for Stratum protocol
//!
//! This module implements the Nonce1/Nonce2 splitting system used by ASIC mining pools.
//! - Nonce1: Pool-controlled nonce (most significant bytes)
//! - Nonce2: Miner-controlled nonce (least significant bytes)

use crate::core::Nonce;
use crate::error::{Error, Result};
use serde::{Deserialize, Serialize};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

/// Size of a nonce in bytes (0-8)
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct NonceSize(u8);

impl NonceSize {
    /// Create a new nonce size (0-8 bytes)
    pub fn new(size: u8) -> Result<Self> {
        if size <= 8 {
            Ok(NonceSize(size))
        } else {
            Err(Error::config(format!(
                "Invalid nonce size: {}, must be 0-8",
                size
            )))
        }
    }

    /// Get the size in bytes
    pub fn as_bytes(&self) -> u8 {
        self.0
    }

    /// Get the complement size (8 - self)
    pub fn complement(&self) -> NonceSize {
        NonceSize(8 - self.0)
    }

    /// Get the maximum value for this nonce size
    pub fn max_value(&self) -> u64 {
        if self.0 == 0 {
            0
        } else if self.0 >= 8 {
            u64::MAX
        } else {
            (1u64 << (self.0 * 8)) - 1
        }
    }
}

impl Default for NonceSize {
    fn default() -> Self {
        NonceSize(4) // Default to 4 bytes each for Nonce1 and Nonce2
    }
}

/// Pool-controlled nonce (most significant bytes)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Nonce1 {
    size: NonceSize,
    value: u64,
}

impl Nonce1 {
    /// Create a new Nonce1
    pub fn new(size: NonceSize, value: u64) -> Result<Self> {
        if value <= size.max_value() {
            Ok(Nonce1 { size, value })
        } else {
            Err(Error::config(format!(
                "Nonce1 value {} exceeds maximum {} for size {} bytes",
                value,
                size.max_value(),
                size.as_bytes()
            )))
        }
    }

    /// Get the nonce size
    pub fn size(&self) -> NonceSize {
        self.size
    }

    /// Get the nonce value
    pub fn value(&self) -> u64 {
        self.value
    }

    /// Convert to hex string (big-endian for Stratum protocol)
    pub fn to_hex(&self) -> String {
        let bytes = (self.size.as_bytes() * 2) as usize;
        format!("{:0width$x}", self.value, width = bytes)
    }

    /// Parse from hex string (big-endian)
    pub fn from_hex(size: NonceSize, hex: &str) -> Result<Self> {
        let value = u64::from_str_radix(hex, 16)
            .map_err(|e| Error::config(format!("Invalid Nonce1 hex: {}", e)))?;
        Self::new(size, value)
    }

    /// Derive Nonce1 from client identifier and session salt
    pub fn derive(size: NonceSize, client_id: &str, salt: u64) -> Result<Self> {
        let mut hasher = DefaultHasher::new();
        client_id.hash(&mut hasher);
        salt.hash(&mut hasher);
        let hash = hasher.finish();

        // Shift right to get the most significant bits for the specified size
        let shift_bits = 8 * (8 - size.as_bytes());
        let value = hash >> shift_bits;

        Self::new(size, value)
    }

    /// Get the Nonce2 size that complements this Nonce1
    pub fn nonce2_size(&self) -> NonceSize {
        self.size.complement()
    }
}

/// Miner-controlled nonce (least significant bytes)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Nonce2 {
    size: NonceSize,
    value: u64,
}

impl Nonce2 {
    /// Create a new Nonce2
    pub fn new(size: NonceSize, value: u64) -> Result<Self> {
        if value <= size.max_value() {
            Ok(Nonce2 { size, value })
        } else {
            Err(Error::config(format!(
                "Nonce2 value {} exceeds maximum {} for size {} bytes",
                value,
                size.max_value(),
                size.as_bytes()
            )))
        }
    }

    /// Get the nonce size
    pub fn size(&self) -> NonceSize {
        self.size
    }

    /// Get the nonce value
    pub fn value(&self) -> u64 {
        self.value
    }

    /// Convert to hex string (big-endian for Stratum protocol)
    pub fn to_hex(&self) -> String {
        let bytes = (self.size.as_bytes() * 2) as usize;
        format!("{:0width$x}", self.value, width = bytes)
    }

    /// Parse from hex string (big-endian)
    pub fn from_hex(size: NonceSize, hex: &str) -> Result<Self> {
        let value = u64::from_str_radix(hex, 16)
            .map_err(|e| Error::config(format!("Invalid Nonce2 hex: {}", e)))?;
        Self::new(size, value)
    }

    /// Increment nonce2 value (for mining iteration)
    pub fn increment(&mut self) -> bool {
        if self.value < self.size.max_value() {
            self.value += 1;
            true
        } else {
            false // Overflow
        }
    }

    /// Create from bytes (big-endian)
    pub fn from_bytes(size: NonceSize, bytes: &[u8]) -> Result<Self> {
        if bytes.len() != size.as_bytes() as usize {
            return Err(Error::config(format!(
                "Invalid byte length for Nonce2: expected {}, got {}",
                size.as_bytes(),
                bytes.len()
            )));
        }

        let mut value = 0u64;
        for &byte in bytes.iter() {
            value = (value << 8) | (byte as u64);
        }

        Self::new(size, value)
    }
}

/// Compose Nonce1 and Nonce2 into final nonce
pub fn compose_nonce(nonce1: Nonce1, nonce2: Nonce2) -> Result<Nonce> {
    let total_size = nonce1.size().as_bytes() + nonce2.size().as_bytes();
    if total_size != 8 {
        return Err(Error::config(format!(
            "Combined nonce size must be 8 bytes, got {} + {} = {}",
            nonce1.size().as_bytes(),
            nonce2.size().as_bytes(),
            total_size
        )));
    }

    // Nonce layout in little-endian: Nonce2 (low) || Nonce1 (high)
    let shift_bits = nonce1.size().as_bytes() * 8;
    let composed = (nonce2.value() << shift_bits) | nonce1.value();

    Ok(Nonce::new(composed))
}

/// Split a nonce into Nonce1 and Nonce2 components
pub fn split_nonce(nonce: Nonce, nonce1_size: NonceSize) -> Result<(Nonce1, Nonce2)> {
    let nonce2_size = nonce1_size.complement();
    let shift_bits = nonce1_size.as_bytes() * 8;
    let nonce1_mask = nonce1_size.max_value();

    let nonce1_value = nonce.value() & nonce1_mask;
    let nonce2_value = nonce.value() >> shift_bits;

    let nonce1 = Nonce1::new(nonce1_size, nonce1_value)?;
    let nonce2 = Nonce2::new(nonce2_size, nonce2_value)?;

    Ok((nonce1, nonce2))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_nonce_size() {
        let size = NonceSize::new(4).unwrap();
        assert_eq!(size.as_bytes(), 4);
        assert_eq!(size.complement().as_bytes(), 4);
        assert_eq!(size.max_value(), 0xFFFFFFFF);

        assert!(NonceSize::new(9).is_err());
    }

    #[test]
    fn test_nonce1_creation() {
        let size = NonceSize::new(4).unwrap();
        let nonce1 = Nonce1::new(size, 0x12345678).unwrap();

        assert_eq!(nonce1.value(), 0x12345678);
        assert_eq!(nonce1.to_hex(), "12345678");

        // Test overflow
        assert!(Nonce1::new(size, 0x123456789).is_err());
    }

    #[test]
    fn test_nonce2_creation() {
        let size = NonceSize::new(4).unwrap();
        let mut nonce2 = Nonce2::new(size, 0x87654321).unwrap();

        assert_eq!(nonce2.value(), 0x87654321);
        assert_eq!(nonce2.to_hex(), "87654321");

        // Test increment
        assert!(nonce2.increment());
        assert_eq!(nonce2.value(), 0x87654322);
    }

    #[test]
    fn test_hex_conversion() {
        let size = NonceSize::new(4).unwrap();
        let nonce1 = Nonce1::new(size, 0x12345678).unwrap();

        let hex = nonce1.to_hex();
        assert_eq!(hex, "12345678");

        let parsed = Nonce1::from_hex(size, &hex).unwrap();
        assert_eq!(parsed, nonce1);
    }

    #[test]
    fn test_nonce_composition() {
        let nonce1_size = NonceSize::new(4).unwrap();
        let nonce2_size = NonceSize::new(4).unwrap();

        let nonce1 = Nonce1::new(nonce1_size, 0x12345678).unwrap();
        let nonce2 = Nonce2::new(nonce2_size, 0x87654321).unwrap();

        let composed = compose_nonce(nonce1, nonce2).unwrap();
        assert_eq!(composed.value(), 0x8765432112345678);
    }

    #[test]
    fn test_nonce_split_and_compose() {
        let original = Nonce::new(0x123456789ABCDEF0);
        let nonce1_size = NonceSize::new(4).unwrap();

        let (nonce1, nonce2) = split_nonce(original, nonce1_size).unwrap();
        let recomposed = compose_nonce(nonce1, nonce2).unwrap();

        assert_eq!(original.value(), recomposed.value());
    }

    #[test]
    fn test_nonce1_derivation() {
        let size = NonceSize::new(4).unwrap();
        let nonce1 = Nonce1::derive(size, "client123", 42).unwrap();

        // Should produce consistent results
        let nonce1_same = Nonce1::derive(size, "client123", 42).unwrap();
        assert_eq!(nonce1, nonce1_same);

        // Different clients should produce different nonces
        let nonce1_different = Nonce1::derive(size, "client456", 42).unwrap();
        assert_ne!(nonce1, nonce1_different);
    }

    #[test]
    fn test_nonce2_overflow() {
        let size = NonceSize::new(1).unwrap(); // Only 255 values
        let mut nonce2 = Nonce2::new(size, 255).unwrap();

        // Should not increment beyond max
        assert!(!nonce2.increment());
        assert_eq!(nonce2.value(), 255);
    }

    #[test]
    fn test_invalid_composition() {
        let nonce1 = Nonce1::new(NonceSize::new(3).unwrap(), 0x123456).unwrap();
        let nonce2 = Nonce2::new(NonceSize::new(4).unwrap(), 0x87654321).unwrap();

        // Total size is 7, not 8
        assert!(compose_nonce(nonce1, nonce2).is_err());
    }
}
