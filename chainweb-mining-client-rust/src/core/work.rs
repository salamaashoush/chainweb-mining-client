//! Work type representing a mining job

use crate::core::constants::{NONCE_OFFSET, NONCE_SIZE, WORK_SIZE};
use crate::core::{ChainId, Nonce, Target};
use crate::error::{Error, Result};
use blake2::{Blake2s256, Digest};
use serde::{Deserialize, Serialize};
use std::fmt;

/// Represents a 286-byte mining work header
#[derive(Clone, PartialEq, Eq)]
pub struct Work {
    bytes: [u8; WORK_SIZE],
}

impl Work {
    /// Create a new Work from bytes
    pub fn from_bytes(bytes: [u8; WORK_SIZE]) -> Self {
        Self { bytes }
    }

    /// Create Work from a byte slice
    pub fn from_slice(slice: &[u8]) -> Result<Self> {
        if slice.len() != WORK_SIZE {
            return Err(Error::validation_invalid_work_header(WORK_SIZE, slice.len()));
        }

        let mut bytes = [0u8; WORK_SIZE];
        bytes.copy_from_slice(slice);
        Ok(Self { bytes })
    }

    /// Get the work as bytes
    pub fn as_bytes(&self) -> &[u8; WORK_SIZE] {
        &self.bytes
    }

    /// Get a mutable reference to the bytes
    pub fn as_bytes_mut(&mut self) -> &mut [u8; WORK_SIZE] {
        &mut self.bytes
    }

    /// Get the nonce from the work
    pub fn nonce(&self) -> Nonce {
        let mut nonce_bytes = [0u8; NONCE_SIZE];
        nonce_bytes.copy_from_slice(&self.bytes[NONCE_OFFSET..]);
        Nonce::from_le_bytes(nonce_bytes)
    }

    /// Set the nonce in the work
    pub fn set_nonce(&mut self, nonce: Nonce) {
        self.bytes[NONCE_OFFSET..].copy_from_slice(&nonce.to_le_bytes());
    }

    /// Compute the Blake2s-256 hash of the work
    pub fn hash(&self) -> [u8; 32] {
        let mut hasher = Blake2s256::new();
        hasher.update(self.bytes);
        hasher.finalize().into()
    }

    /// Check if the work meets the given target
    pub fn meets_target(&self, target: &Target) -> bool {
        let hash = self.hash();
        target.meets_target(&hash)
    }

    /// Update the timestamp in the work (if supported)
    /// The timestamp is typically at a fixed offset in the header
    pub fn update_timestamp(&mut self, timestamp: u64) {
        // This is a simplified implementation
        // The actual offset would depend on the Chainweb header format
        const TIMESTAMP_OFFSET: usize = 8; // Example offset
        self.bytes[TIMESTAMP_OFFSET..TIMESTAMP_OFFSET + 8]
            .copy_from_slice(&timestamp.to_le_bytes());
    }

    /// Create a hex representation of the work
    pub fn to_hex(&self) -> String {
        hex::encode(self.bytes)
    }

    /// Create Work from a hex string
    pub fn from_hex(hex: &str) -> Result<Self> {
        let bytes = hex::decode(hex).map_err(|e| {
            Error::validation_invalid_target(hex, format!("Invalid hex encoding: {}", e))
        })?;
        Self::from_slice(&bytes)
    }
}

impl fmt::Debug for Work {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Work")
            .field("hex", &self.to_hex())
            .field("nonce", &self.nonce())
            .finish()
    }
}

impl fmt::Display for Work {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Work(nonce={})", self.nonce())
    }
}

impl Serialize for Work {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_hex())
    }
}

impl<'de> Deserialize<'de> for Work {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let hex = String::deserialize(deserializer)?;
        Self::from_hex(&hex).map_err(serde::de::Error::custom)
    }
}

/// Builder for constructing Work headers
#[allow(dead_code)]
pub struct WorkBuilder {
    bytes: Vec<u8>,
}

#[allow(dead_code)]
impl WorkBuilder {
    /// Create a new WorkBuilder
    pub fn new() -> Self {
        Self {
            bytes: vec![0u8; WORK_SIZE],
        }
    }

    /// Set the chain ID
    pub fn chain_id(mut self, chain_id: ChainId) -> Self {
        // Chain ID would be at a specific offset in the header
        // This is a simplified implementation
        self.bytes[0..2].copy_from_slice(&chain_id.value().to_le_bytes());
        self
    }

    /// Set the timestamp
    pub fn timestamp(mut self, timestamp: u64) -> Self {
        const TIMESTAMP_OFFSET: usize = 8;
        self.bytes[TIMESTAMP_OFFSET..TIMESTAMP_OFFSET + 8]
            .copy_from_slice(&timestamp.to_le_bytes());
        self
    }

    /// Set the nonce
    pub fn nonce(mut self, nonce: Nonce) -> Self {
        self.bytes[NONCE_OFFSET..].copy_from_slice(&nonce.to_le_bytes());
        self
    }

    /// Build the Work
    pub fn build(self) -> Result<Work> {
        Work::from_slice(&self.bytes)
    }
}

impl Default for WorkBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_work_creation() {
        let bytes = [0u8; WORK_SIZE];
        let work = Work::from_bytes(bytes);
        assert_eq!(work.as_bytes(), &bytes);
    }

    #[test]
    fn test_work_from_slice() {
        let slice = vec![0u8; WORK_SIZE];
        let work = Work::from_slice(&slice).unwrap();
        assert_eq!(work.as_bytes().len(), WORK_SIZE);

        // Test invalid size
        let invalid = vec![0u8; WORK_SIZE - 1];
        assert!(Work::from_slice(&invalid).is_err());
    }

    #[test]
    fn test_work_nonce() {
        let mut work = Work::from_bytes([0u8; WORK_SIZE]);

        // Set nonce
        let nonce = Nonce::new(12345);
        work.set_nonce(nonce);

        // Get nonce
        assert_eq!(work.nonce(), nonce);
    }

    #[test]
    fn test_work_hash() {
        let work = Work::from_bytes([0x42u8; WORK_SIZE]);
        let hash = work.hash();
        assert_eq!(hash.len(), 32);

        // Hash should be deterministic
        let hash2 = work.hash();
        assert_eq!(hash, hash2);
    }

    #[test]
    fn test_work_hex_conversion() {
        let mut bytes = [0u8; WORK_SIZE];
        bytes[0] = 0xFF;
        bytes[WORK_SIZE - 1] = 0xAA;

        let work = Work::from_bytes(bytes);
        let hex = work.to_hex();

        let work2 = Work::from_hex(&hex).unwrap();
        assert_eq!(work, work2);
    }

    #[test]
    fn test_work_builder() {
        let work = WorkBuilder::new()
            .chain_id(ChainId::new(5))
            .timestamp(1234567890)
            .nonce(Nonce::new(999))
            .build()
            .unwrap();

        assert_eq!(work.nonce(), Nonce::new(999));
    }

    #[test]
    fn test_work_serde() {
        let work = Work::from_bytes([0x11u8; WORK_SIZE]);

        let json = serde_json::to_string(&work).unwrap();
        let deserialized: Work = serde_json::from_str(&json).unwrap();

        assert_eq!(work, deserialized);
    }
}
