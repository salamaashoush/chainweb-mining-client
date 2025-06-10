//! Core types for chainweb mining
//!
//! Fundamental types used throughout the mining client with proper validation,
//! binary encoding, and JSON serialization.

use crate::{Error, Result};
use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::io::{Cursor, Read, Write};
use std::str::FromStr;

/// Mining target representing the difficulty threshold
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Target {
    /// 256-bit target value stored as 4 64-bit words in little-endian order
    words: [u64; 4],
}

impl Target {
    /// Create a new target from a 256-bit value
    pub fn new(words: [u64; 4]) -> Self {
        Self { words }
    }

    /// Create target from bytes (32 bytes, little-endian)
    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        if bytes.len() != 32 {
            return Err(Error::target(format!(
                "Invalid target length: expected 32 bytes, got {}",
                bytes.len()
            )));
        }

        let mut cursor = Cursor::new(bytes);
        let words = [
            cursor.read_u64::<LittleEndian>()?,
            cursor.read_u64::<LittleEndian>()?,
            cursor.read_u64::<LittleEndian>()?,
            cursor.read_u64::<LittleEndian>()?,
        ];

        Ok(Self::new(words))
    }

    /// Convert target to bytes (32 bytes, little-endian)
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(32);
        bytes.write_u64::<LittleEndian>(self.words[0]).unwrap();
        bytes.write_u64::<LittleEndian>(self.words[1]).unwrap();
        bytes.write_u64::<LittleEndian>(self.words[2]).unwrap();
        bytes.write_u64::<LittleEndian>(self.words[3]).unwrap();
        bytes
    }

    /// Convert to hexadecimal string (big-endian for display)
    pub fn to_hex_be(&self) -> String {
        format!(
            "{:016x}{:016x}{:016x}{:016x}",
            self.words[3], self.words[2], self.words[1], self.words[0]
        )
    }

    /// Convert to hexadecimal string (little-endian)
    pub fn to_hex_le(&self) -> String {
        format!(
            "{:016x}{:016x}{:016x}{:016x}",
            self.words[0], self.words[1], self.words[2], self.words[3]
        )
    }

    /// Check if a hash meets this target
    pub fn meets_target(&self, hash: &[u8]) -> bool {
        if hash.len() != 32 {
            return false;
        }

        // Compare hash with target (both in little-endian)
        for i in (0..4).rev() {
            let hash_word = (&hash[i * 8..(i + 1) * 8])
                .read_u64::<LittleEndian>()
                .unwrap_or(u64::MAX);
            
            if hash_word < self.words[i] {
                return true;
            } else if hash_word > self.words[i] {
                return false;
            }
        }
        true
    }

    /// Maximum possible target (easiest difficulty)
    pub fn max() -> Self {
        Self::new([u64::MAX; 4])
    }

    /// Minimum possible target (hardest difficulty)
    pub fn min() -> Self {
        Self::new([0; 4])
    }

    /// Get difficulty level (number of leading zero bits required)
    pub fn difficulty_level(&self) -> u8 {
        for i in (0..4).rev() {
            if self.words[i] != 0 {
                return (256 - (i * 64 + (64 - self.words[i].leading_zeros()) as usize)) as u8;
            }
        }
        256
    }
}

impl FromStr for Target {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self> {
        if s.len() != 64 {
            return Err(Error::target(format!(
                "Invalid target hex length: expected 64 chars, got {}",
                s.len()
            )));
        }

        // Parse as big-endian hex string
        let mut words = [0u64; 4];
        for i in 0..4 {
            let start = i * 16;
            let end = start + 16;
            words[3 - i] = u64::from_str_radix(&s[start..end], 16)
                .map_err(|e| Error::target(format!("Invalid hex in target: {}", e)))?;
        }

        Ok(Self::new(words))
    }
}

impl fmt::Display for Target {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_hex_be())
    }
}

impl Serialize for Target {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_hex_be())
    }
}

impl<'de> Deserialize<'de> for Target {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Target::from_str(&s).map_err(serde::de::Error::custom)
    }
}

/// Proof-of-work nonce (8 bytes)
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Nonce(pub u64);

impl Nonce {
    /// Create a new nonce
    pub fn new(value: u64) -> Self {
        Self(value)
    }

    /// Get the nonce value
    pub fn value(&self) -> u64 {
        self.0
    }

    /// Convert to bytes (little-endian)
    pub fn to_bytes(&self) -> [u8; 8] {
        self.0.to_le_bytes()
    }

    /// Create from bytes (little-endian)
    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        if bytes.len() != 8 {
            return Err(Error::nonce(format!(
                "Invalid nonce length: expected 8 bytes, got {}",
                bytes.len()
            )));
        }
        let mut array = [0u8; 8];
        array.copy_from_slice(bytes);
        Ok(Self(u64::from_le_bytes(array)))
    }

    /// Convert to hexadecimal string
    pub fn to_hex(&self) -> String {
        format!("{:016x}", self.0)
    }

    /// Increment nonce
    pub fn increment(&mut self) {
        self.0 = self.0.wrapping_add(1);
    }

    /// Add to nonce
    pub fn add(&mut self, value: u64) {
        self.0 = self.0.wrapping_add(value);
    }
}

impl fmt::Display for Nonce {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_hex())
    }
}

/// Chain identifier (4 bytes)
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ChainId(pub u32);

impl ChainId {
    /// Create a new chain ID
    pub fn new(id: u32) -> Self {
        Self(id)
    }

    /// Get the chain ID value
    pub fn value(&self) -> u32 {
        self.0
    }

    /// Convert to bytes (little-endian)
    pub fn to_bytes(&self) -> [u8; 4] {
        self.0.to_le_bytes()
    }

    /// Create from bytes (little-endian)
    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        if bytes.len() != 4 {
            return Err(Error::generic(
                "ChainId",
                format!("Invalid chain ID length: expected 4 bytes, got {}", bytes.len()),
            ));
        }
        let mut array = [0u8; 4];
        array.copy_from_slice(bytes);
        Ok(Self(u32::from_le_bytes(array)))
    }
}

impl fmt::Display for ChainId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Serialize for ChainId {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_u32(self.0)
    }
}

impl<'de> Deserialize<'de> for ChainId {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let id = u32::deserialize(deserializer)?;
        Ok(ChainId::new(id))
    }
}

/// Mining work header (286 bytes total)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Work {
    /// Raw work bytes - the block header to be mined
    pub bytes: Vec<u8>,
}

impl Work {
    /// Expected work size in bytes
    pub const SIZE: usize = 286;

    /// Create new work from bytes
    pub fn new(bytes: Vec<u8>) -> Result<Self> {
        if bytes.len() != Self::SIZE {
            return Err(Error::work(format!(
                "Invalid work size: expected {} bytes, got {}",
                Self::SIZE,
                bytes.len()
            )));
        }
        Ok(Self { bytes })
    }

    /// Get work bytes
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// Get mutable work bytes
    pub fn bytes_mut(&mut self) -> &mut [u8] {
        &mut self.bytes
    }

    /// Inject nonce into work at the correct position (last 8 bytes)
    pub fn inject_nonce(&mut self, nonce: Nonce) {
        let nonce_offset = Self::SIZE - 8;
        self.bytes[nonce_offset..].copy_from_slice(&nonce.to_bytes());
    }

    /// Extract nonce from work
    pub fn extract_nonce(&self) -> Result<Nonce> {
        let nonce_offset = Self::SIZE - 8;
        Nonce::from_bytes(&self.bytes[nonce_offset..])
    }

    /// Inject timestamp into work (at specific offset for chainweb)
    pub fn inject_timestamp(&mut self, timestamp_micros: i64) {
        // Chainweb timestamp location - adjust based on actual header format
        let timestamp_offset = 8; // This needs to match chainweb header format
        self.bytes[timestamp_offset..timestamp_offset + 8]
            .copy_from_slice(&timestamp_micros.to_le_bytes());
    }

    /// Convert to hexadecimal string
    pub fn to_hex(&self) -> String {
        hex::encode(&self.bytes)
    }

    /// Create from hexadecimal string
    pub fn from_hex(hex_str: &str) -> Result<Self> {
        let bytes = hex::decode(hex_str)
            .map_err(|e| Error::work(format!("Invalid hex in work: {}", e)))?;
        Self::new(bytes)
    }
}

impl fmt::Display for Work {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_hex())
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
        let s = String::deserialize(deserializer)?;
        Work::from_hex(&s).map_err(serde::de::Error::custom)
    }
}

/// Miner public key (hex string)
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct MinerPublicKey(pub String);

impl MinerPublicKey {
    /// Create new miner public key
    pub fn new(key: String) -> Result<Self> {
        // Validate hex format
        if key.len() != 64 {
            return Err(Error::config(format!(
                "Invalid public key length: expected 64 hex chars, got {}",
                key.len()
            )));
        }
        
        hex::decode(&key)
            .map_err(|e| Error::config(format!("Invalid hex in public key: {}", e)))?;
        
        Ok(Self(key))
    }

    /// Get the key as string
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Get the default account name (k: prefix)
    pub fn default_account(&self) -> String {
        format!("k:{}", self.0)
    }
}

impl fmt::Display for MinerPublicKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Miner account name
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct MinerAccount(pub String);

impl MinerAccount {
    /// Create new miner account
    pub fn new(account: String) -> Self {
        Self(account)
    }

    /// Get the account name
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for MinerAccount {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Miner configuration combining public key and account
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Miner {
    pub public_key: MinerPublicKey,
    pub account: Option<MinerAccount>,
}

impl Miner {
    /// Create new miner
    pub fn new(public_key: MinerPublicKey, account: Option<MinerAccount>) -> Self {
        Self { public_key, account }
    }

    /// Get the account name (using default if not specified)
    pub fn account_name(&self) -> String {
        match &self.account {
            Some(account) => account.as_str().to_string(),
            None => self.public_key.default_account(),
        }
    }
}

/// Hash rate in hashes per second
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct HashRate(pub f64);

impl HashRate {
    /// Create new hash rate
    pub fn new(rate: f64) -> Self {
        Self(rate)
    }

    /// Get the rate value
    pub fn value(&self) -> f64 {
        self.0
    }

    /// Parse from string with unit suffixes (K, M, G, T, P)
    pub fn from_str_with_units(s: &str) -> Result<Self> {
        let s = s.trim().to_uppercase();
        
        if let Some(stripped) = s.strip_suffix('K') {
            let base: f64 = stripped.parse()
                .map_err(|e| Error::config(format!("Invalid hash rate: {}", e)))?;
            Ok(Self(base * 1_000.0))
        } else if let Some(stripped) = s.strip_suffix('M') {
            let base: f64 = stripped.parse()
                .map_err(|e| Error::config(format!("Invalid hash rate: {}", e)))?;
            Ok(Self(base * 1_000_000.0))
        } else if let Some(stripped) = s.strip_suffix('G') {
            let base: f64 = stripped.parse()
                .map_err(|e| Error::config(format!("Invalid hash rate: {}", e)))?;
            Ok(Self(base * 1_000_000_000.0))
        } else if let Some(stripped) = s.strip_suffix('T') {
            let base: f64 = stripped.parse()
                .map_err(|e| Error::config(format!("Invalid hash rate: {}", e)))?;
            Ok(Self(base * 1_000_000_000_000.0))
        } else if let Some(stripped) = s.strip_suffix('P') {
            let base: f64 = stripped.parse()
                .map_err(|e| Error::config(format!("Invalid hash rate: {}", e)))?;
            Ok(Self(base * 1_000_000_000_000_000.0))
        } else {
            let base: f64 = s.parse()
                .map_err(|e| Error::config(format!("Invalid hash rate: {}", e)))?;
            Ok(Self(base))
        }
    }
}

impl fmt::Display for HashRate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.0 >= 1_000_000_000_000_000.0 {
            write!(f, "{:.2}P H/s", self.0 / 1_000_000_000_000_000.0)
        } else if self.0 >= 1_000_000_000_000.0 {
            write!(f, "{:.2}T H/s", self.0 / 1_000_000_000_000.0)
        } else if self.0 >= 1_000_000_000.0 {
            write!(f, "{:.2}G H/s", self.0 / 1_000_000_000.0)
        } else if self.0 >= 1_000_000.0 {
            write!(f, "{:.2}M H/s", self.0 / 1_000_000.0)
        } else if self.0 >= 1_000.0 {
            write!(f, "{:.2}K H/s", self.0 / 1_000.0)
        } else {
            write!(f, "{:.2} H/s", self.0)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_target_creation() {
        let target = Target::new([1, 2, 3, 4]);
        assert_eq!(target.words, [1, 2, 3, 4]);
    }

    #[test]
    fn test_target_hex_conversion() {
        let target = Target::new([0x1234567890abcdef, 0, 0, 0]);
        let hex = target.to_hex_be();
        let parsed = Target::from_str(&hex).unwrap();
        assert_eq!(target, parsed);
    }

    #[test]
    fn test_nonce_operations() {
        let mut nonce = Nonce::new(100);
        assert_eq!(nonce.value(), 100);
        
        nonce.increment();
        assert_eq!(nonce.value(), 101);
        
        nonce.add(50);
        assert_eq!(nonce.value(), 151);
    }

    #[test]
    fn test_work_nonce_injection() {
        let mut work = Work::new(vec![0u8; Work::SIZE]).unwrap();
        let nonce = Nonce::new(0x1234567890abcdef);
        
        work.inject_nonce(nonce);
        let extracted = work.extract_nonce().unwrap();
        assert_eq!(nonce, extracted);
    }

    #[test]
    fn test_hash_rate_parsing() {
        assert_eq!(HashRate::from_str_with_units("100").unwrap().value(), 100.0);
        assert_eq!(HashRate::from_str_with_units("1K").unwrap().value(), 1_000.0);
        assert_eq!(HashRate::from_str_with_units("1M").unwrap().value(), 1_000_000.0);
        assert_eq!(HashRate::from_str_with_units("1G").unwrap().value(), 1_000_000_000.0);
    }
}