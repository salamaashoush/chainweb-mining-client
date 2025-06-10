//! Cryptographic utilities for mining
//!
//! Provides optimized Blake2s hashing and Ed25519 key generation.

use crate::Target;
use blake2::{Blake2s256, Digest};
use ed25519_dalek::{Keypair, PublicKey, SecretKey};
use rand::{CryptoRng, RngCore};
use std::convert::TryInto;

/// Optimized Blake2s hasher for mining operations
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

    /// Hash data and return the result
    pub fn hash(&mut self, data: &[u8]) -> [u8; 32] {
        self.hasher.update(data);
        let result = self.hasher.finalize_reset();
        result.into()
    }

    /// Hash work header and check against target
    /// Returns true if the hash meets the target difficulty
    pub fn hash_and_check(&mut self, data: &[u8], target: &Target) -> (bool, [u8; 32]) {
        let hash = self.hash(data);
        let meets_target = target.check_hash(&hash);
        (meets_target, hash)
    }

    /// Batch hash multiple nonces
    /// Returns the first nonce that meets the target, if any
    pub fn batch_hash_check(
        &mut self,
        base_data: &[u8],
        nonce_start: u64,
        count: u32,
        target: &Target,
    ) -> Option<(u64, [u8; 32])> {
        let mut work_data = base_data.to_vec();
        let nonce_offset = work_data.len() - 8; // Nonce is last 8 bytes

        for i in 0..count {
            let nonce = nonce_start + i as u64;
            
            // Write nonce to work data
            work_data[nonce_offset..].copy_from_slice(&nonce.to_le_bytes());
            
            let (meets_target, hash) = self.hash_and_check(&work_data, target);
            if meets_target {
                return Some((nonce, hash));
            }
        }

        None
    }
}

impl Default for Blake2sHasher {
    fn default() -> Self {
        Self::new()
    }
}

/// Generate a new Ed25519 keypair for mining account
pub fn generate_keypair<R: CryptoRng + RngCore>(rng: &mut R) -> (SecretKey, PublicKey) {
    let keypair = Keypair::generate(rng);
    (keypair.secret, keypair.public)
}

/// Convert a public key to its hex representation
pub fn public_key_to_hex(public_key: &PublicKey) -> String {
    hex::encode(public_key.as_bytes())
}

/// Fast target checking optimized for mining
impl Target {
    /// Check if a hash meets this target (lower is better)
    pub fn check_hash(&self, hash: &[u8; 32]) -> bool {
        // Compare hash bytes in big-endian order
        for i in 0..32 {
            match hash[i].cmp(&self.as_bytes()[i]) {
                std::cmp::Ordering::Less => return true,
                std::cmp::Ordering::Greater => return false,
                std::cmp::Ordering::Equal => continue,
            }
        }
        true // Hash equals target
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::thread_rng;

    #[test]
    fn test_blake2s_hasher() {
        let mut hasher = Blake2sHasher::new();
        let data = b"test data";
        let hash = hasher.hash(data);
        
        // Hash should be deterministic
        let mut hasher2 = Blake2sHasher::new();
        let hash2 = hasher2.hash(data);
        assert_eq!(hash, hash2);
        
        // Different data should produce different hashes
        let hash3 = hasher.hash(b"different data");
        assert_ne!(hash, hash3);
    }

    #[test]
    fn test_hash_and_check() {
        let mut hasher = Blake2sHasher::new();
        let data = b"test data";
        let target = Target::max(); // Easiest target
        
        let (meets_target, _hash) = hasher.hash_and_check(data, &target);
        assert!(meets_target); // Max target should always be met
    }

    #[test]
    fn test_batch_hash_check() {
        let mut hasher = Blake2sHasher::new();
        let mut base_data = vec![0u8; 286]; // Chainweb work header size
        
        // Add 8 bytes for nonce
        base_data.extend_from_slice(&0u64.to_le_bytes());
        
        let target = Target::max();
        let result = hasher.batch_hash_check(&base_data, 0, 10, &target);
        
        // With max target, we should find a solution quickly
        assert!(result.is_some());
        
        let (nonce, _hash) = result.unwrap();
        assert!(nonce < 10);
    }

    #[test]
    fn test_keypair_generation() {
        let mut rng = thread_rng();
        let (_secret1, public1) = generate_keypair(&mut rng);
        let (_secret2, public2) = generate_keypair(&mut rng);
        
        // Different keypairs should have different public keys
        assert_ne!(public1.as_bytes(), public2.as_bytes());
    }

    #[test]
    fn test_public_key_to_hex() {
        let mut rng = thread_rng();
        let (_secret, public) = generate_keypair(&mut rng);
        let hex_str = public_key_to_hex(&public);
        
        // Should be 64 hex characters (32 bytes * 2)
        assert_eq!(hex_str.len(), 64);
        
        // Should be valid hex
        assert!(hex::decode(&hex_str).is_ok());
    }

    #[test]
    fn test_target_check_hash() {
        let target = Target::max();
        let zero_hash = [0u8; 32];
        let max_hash = [0xFFu8; 32];
        
        // Zero hash should always meet any target
        assert!(target.check_hash(&zero_hash));
        
        // Max hash should only meet max target
        assert!(target.check_hash(&max_hash));
    }
}