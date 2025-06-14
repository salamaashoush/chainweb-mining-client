//! Nonce type for mining operations

use serde::{Deserialize, Serialize};
use std::fmt;

/// Represents a 64-bit nonce used in mining
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, Default)]
#[serde(transparent)]
pub struct Nonce(pub u64);

impl Nonce {
    /// Create a new Nonce
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    /// Get the inner value
    pub const fn value(self) -> u64 {
        self.0
    }

    /// Increment the nonce by 1 in place
    pub fn increment(&mut self) {
        self.0 = self.0.wrapping_add(1);
    }
    
    /// Increment the nonce by 1 and return the result
    pub fn incremented(self) -> Self {
        Self(self.0.wrapping_add(1))
    }

    /// Create a nonce from little-endian bytes
    pub fn from_le_bytes(bytes: [u8; 8]) -> Self {
        Self(u64::from_le_bytes(bytes))
    }

    /// Convert nonce to little-endian bytes
    pub fn to_le_bytes(self) -> [u8; 8] {
        self.0.to_le_bytes()
    }
}

impl fmt::Display for Nonce {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<u64> for Nonce {
    fn from(value: u64) -> Self {
        Self(value)
    }
}

impl From<Nonce> for u64 {
    fn from(nonce: Nonce) -> Self {
        nonce.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_nonce_creation() {
        let nonce = Nonce::new(12345);
        assert_eq!(nonce.value(), 12345);
    }

    #[test]
    fn test_nonce_increment() {
        let mut nonce = Nonce::new(100);
        nonce.increment();
        assert_eq!(nonce.value(), 101);

        // Test wrapping
        let mut nonce = Nonce::new(u64::MAX);
        nonce.increment();
        assert_eq!(nonce.value(), 0);
    }

    #[test]
    fn test_nonce_bytes() {
        let nonce = Nonce::new(0x0123456789ABCDEF);
        let bytes = nonce.to_le_bytes();
        let reconstructed = Nonce::from_le_bytes(bytes);
        assert_eq!(nonce, reconstructed);
    }

    #[test]
    fn test_nonce_display() {
        let nonce = Nonce::new(42);
        assert_eq!(nonce.to_string(), "42");
    }

    #[test]
    fn test_nonce_conversions() {
        let nonce: Nonce = 999u64.into();
        assert_eq!(nonce.value(), 999);

        let value: u64 = nonce.into();
        assert_eq!(value, 999);
    }

    #[test]
    fn test_nonce_default() {
        let nonce = Nonce::default();
        assert_eq!(nonce.value(), 0);
    }
}
