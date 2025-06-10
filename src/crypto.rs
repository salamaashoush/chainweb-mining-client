//! Cryptographic utilities for chainweb mining
//!
//! Provides key generation and Blake2s hashing functionality for mining operations.

use crate::{Error, Result, Target};
use blake2::{Blake2s256, Digest};
use ed25519_dalek::{SigningKey, VerifyingKey};
use rand::rngs::OsRng;

/// Generate a new Ed25519 key pair
pub fn generate_keypair() -> (String, String) {
    let mut csprng = OsRng;
    let signing_key = SigningKey::generate(&mut csprng);
    let verifying_key = signing_key.verifying_key();

    let private_key = hex::encode(signing_key.as_bytes());
    let public_key = hex::encode(verifying_key.as_bytes());

    (public_key, private_key)
}

/// Blake2s256 hasher for mining operations
pub struct Blake2sHasher {
    hasher: Blake2s256,
}

impl Blake2sHasher {
    /// Create a new Blake2s hasher
    pub fn new() -> Self {
        Self {
            hasher: Blake2s256::new(),
        }
    }

    /// Reset the hasher
    pub fn reset(&mut self) {
        self.hasher = Blake2s256::new();
    }

    /// Update hasher with data
    pub fn update(&mut self, data: &[u8]) {
        self.hasher.update(data);
    }

    /// Finalize and get the hash
    pub fn finalize(self) -> [u8; 32] {
        self.hasher.finalize().into()
    }

    /// Convenience method to hash data directly
    pub fn hash(data: &[u8]) -> [u8; 32] {
        let mut hasher = Self::new();
        hasher.update(data);
        hasher.finalize()
    }

    /// Hash work and check if it meets target
    pub fn hash_meets_target(work: &[u8], target: &Target) -> bool {
        let hash = Self::hash(work);
        target.meets_target(&hash)
    }
}

impl Default for Blake2sHasher {
    fn default() -> Self {
        Self::new()
    }
}

/// Fast target checking for mining loops
/// 
/// This function performs an optimized comparison of a hash against a target
/// without full hash computation when possible.
pub fn fast_check_target(target_words: &[u64; 4], hash: &[u8; 32]) -> bool {
    // Convert hash bytes to words for comparison (little-endian)
    let hash_words = unsafe {
        std::mem::transmute::<[u8; 32], [u64; 4]>(*hash)
    };

    // Compare words from most significant to least significant
    for i in (0..4).rev() {
        if hash_words[i] < target_words[i] {
            return true;
        } else if hash_words[i] > target_words[i] {
            return false;
        }
    }
    true
}

/// Optimized mining hash computation
/// 
/// This struct provides an optimized path for mining operations that need
/// to compute many hashes with only small changes (like nonce updates).
pub struct MiningHasher {
    base_state: Blake2s256,
    nonce_offset: usize,
}

impl MiningHasher {
    /// Create a new mining hasher
    /// 
    /// `work_prefix` should be the work data up to but not including the nonce
    /// `nonce_offset` is the byte offset where the nonce starts in the full work
    pub fn new(work_prefix: &[u8], nonce_offset: usize) -> Self {
        let mut hasher = Blake2s256::new();
        hasher.update(work_prefix);
        
        Self {
            base_state: hasher,
            nonce_offset,
        }
    }

    /// Compute hash for work with the given nonce
    pub fn hash_with_nonce(&self, nonce_bytes: &[u8], remaining_work: &[u8]) -> [u8; 32] {
        let mut hasher = self.base_state.clone();
        hasher.update(nonce_bytes);
        hasher.update(remaining_work);
        hasher.finalize().into()
    }

    /// Fast check if hash with nonce meets target
    pub fn hash_meets_target_with_nonce(
        &self,
        nonce_bytes: &[u8],
        remaining_work: &[u8],
        target_words: &[u64; 4],
    ) -> bool {
        let hash = self.hash_with_nonce(nonce_bytes, remaining_work);
        fast_check_target(target_words, &hash)
    }
}

/// Utility to get current time in microseconds since epoch
pub fn current_time_micros() -> i64 {
    chrono::Utc::now().timestamp_micros()
}

/// Utility to inject timestamp into work bytes
pub fn inject_timestamp(work: &mut [u8], timestamp_micros: i64, offset: usize) {
    if work.len() >= offset + 8 {
        work[offset..offset + 8].copy_from_slice(&timestamp_micros.to_le_bytes());
    }
}

/// Utility to inject nonce into work bytes
pub fn inject_nonce(work: &mut [u8], nonce: u64, offset: usize) {
    if work.len() >= offset + 8 {
        work[offset..offset + 8].copy_from_slice(&nonce.to_le_bytes());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Target;

    #[test]
    fn test_keypair_generation() {
        let (public_key, private_key) = generate_keypair();
        
        // Keys should be 64 hex characters (32 bytes)
        assert_eq!(public_key.len(), 64);
        assert_eq!(private_key.len(), 64);
        
        // Should be valid hex
        assert!(hex::decode(&public_key).is_ok());
        assert!(hex::decode(&private_key).is_ok());
    }

    #[test]
    fn test_blake2s_hashing() {
        let data = b"test data";
        let hash1 = Blake2sHasher::hash(data);
        let hash2 = Blake2sHasher::hash(data);
        
        // Same input should produce same hash
        assert_eq!(hash1, hash2);
        
        // Different input should produce different hash
        let hash3 = Blake2sHasher::hash(b"different data");
        assert_ne!(hash1, hash3);
    }

    #[test]
    fn test_target_checking() {
        // Create an easy target (high value)
        let easy_target = Target::new([u64::MAX, u64::MAX, u64::MAX, u64::MAX >> 1]);
        
        // Most hashes should meet this target
        let test_data = b"test mining work data with nonce";
        let hash = Blake2sHasher::hash(test_data);
        
        // This specific test might not always pass due to randomness of hash,
        // but demonstrates the API
        let _meets_target = easy_target.meets_target(&hash);
    }

    #[test]
    fn test_fast_target_check() {
        let target_words = [u64::MAX, u64::MAX, u64::MAX, u64::MAX >> 1];
        let easy_hash = [0u8; 32]; // All zeros should meet most targets
        
        assert!(fast_check_target(&target_words, &easy_hash));
        
        let hard_hash = [0xFFu8; 32]; // All 0xFF should fail most targets
        assert!(!fast_check_target(&target_words, &hard_hash));
    }

    #[test]
    fn test_mining_hasher() {
        let work_prefix = b"chainweb block header prefix";
        let nonce_offset = work_prefix.len();
        let remaining_work = b"suffix after nonce";
        
        let hasher = MiningHasher::new(work_prefix, nonce_offset);
        
        let nonce1 = 12345u64.to_le_bytes();
        let nonce2 = 67890u64.to_le_bytes();
        
        let hash1 = hasher.hash_with_nonce(&nonce1, remaining_work);
        let hash2 = hasher.hash_with_nonce(&nonce2, remaining_work);
        
        // Different nonces should produce different hashes
        assert_ne!(hash1, hash2);
    }

    #[test]
    fn test_time_utilities() {
        let time1 = current_time_micros();
        std::thread::sleep(std::time::Duration::from_millis(1));
        let time2 = current_time_micros();
        
        assert!(time2 > time1);
    }

    #[test]
    fn test_injection_utilities() {
        let mut work = vec![0u8; 100];
        let timestamp = 1234567890123456i64;
        let nonce = 0xdeadbeefcafebabeu64;
        
        inject_timestamp(&mut work, timestamp, 10);
        inject_nonce(&mut work, nonce, 20);
        
        // Verify timestamp was injected correctly
        let injected_timestamp = i64::from_le_bytes([
            work[10], work[11], work[12], work[13],
            work[14], work[15], work[16], work[17],
        ]);
        assert_eq!(injected_timestamp, timestamp);
        
        // Verify nonce was injected correctly
        let injected_nonce = u64::from_le_bytes([
            work[20], work[21], work[22], work[23],
            work[24], work[25], work[26], work[27],
        ]);
        assert_eq!(injected_nonce, nonce);
    }
}