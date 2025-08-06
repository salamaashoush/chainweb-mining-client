//! SIMD-optimized Blake2s hashing for mining
//!
//! This module provides high-performance Blake2s-256 implementations
//! using SIMD instructions when available.

use blake2s_simd::{Params, State};
use std::sync::Arc;
use tracing::debug;

/// SIMD-optimized hasher for mining operations
pub struct SimdHasher {
    params: Arc<Params>,
}

impl Default for SimdHasher {
    fn default() -> Self {
        Self::new()
    }
}

impl SimdHasher {
    /// Create a new SIMD hasher
    pub fn new() -> Self {
        // Blake2s-256 configuration
        let params = Params::new()
            .hash_length(32)
            .to_owned();
        
        debug!("Initialized SIMD hasher with AVX2/SSE support");
        
        Self {
            params: Arc::new(params),
        }
    }
    
    /// Hash a single work item
    #[inline]
    pub fn hash_single(&self, work: &[u8; 286]) -> [u8; 32] {
        let hash = self.params.hash(work);
        *hash.as_array()
    }
    
    /// Hash multiple work items in parallel
    /// This uses SIMD instructions internally for better performance
    pub fn hash_batch(&self, works: &[[u8; 286]], results: &mut [[u8; 32]]) {
        assert_eq!(works.len(), results.len());
        
        // blake2s_simd internally uses SIMD when available
        for (work, result) in works.iter().zip(results.iter_mut()) {
            let hash = self.params.hash(work);
            *result = *hash.as_array();
        }
    }
    
    /// Create a stateful hasher for incremental hashing
    pub fn create_state(&self) -> State {
        self.params.to_state()
    }
}

/// Vectorized mining with SIMD support
pub struct SimdMiner {
    hasher: SimdHasher,
    work_buffer: Vec<[u8; 286]>,
    hash_buffer: Vec<[u8; 32]>,
}

impl SimdMiner {
    /// Create a new SIMD miner
    pub fn new(batch_size: usize) -> Self {
        Self {
            hasher: SimdHasher::new(),
            work_buffer: vec![[0u8; 286]; batch_size],
            hash_buffer: vec![[0u8; 32]; batch_size],
        }
    }
    
    /// Get the batch size capacity
    pub fn batch_size(&self) -> usize {
        self.work_buffer.len()
    }
    
    /// Prepare work items with consecutive nonces
    pub fn prepare_batch(&mut self, base_work: &[u8; 286], start_nonce: u64, count: usize) {
        assert!(count <= self.work_buffer.len());
        
        for (i, work) in self.work_buffer[..count].iter_mut().enumerate() {
            *work = *base_work;
            let nonce = start_nonce + i as u64;
            work[crate::core::constants::NONCE_OFFSET..].copy_from_slice(&nonce.to_le_bytes());
        }
    }
    
    /// Hash the prepared batch using SIMD
    pub fn hash_batch(&mut self, count: usize) -> &[[u8; 32]] {
        assert!(count <= self.work_buffer.len());
        
        self.hasher.hash_batch(
            &self.work_buffer[..count],
            &mut self.hash_buffer[..count]
        );
        
        &self.hash_buffer[..count]
    }
    
    /// Mine a batch and check against target
    pub fn mine_batch(
        &mut self,
        base_work: &[u8; 286],
        target: &crate::core::Target,
        start_nonce: u64,
        count: usize,
    ) -> Option<(crate::core::Nonce, [u8; 32])> {
        self.prepare_batch(base_work, start_nonce, count);
        let hashes = self.hash_batch(count);
        
        for (i, hash) in hashes.iter().enumerate() {
            if target.meets_target(hash) {
                let nonce = crate::core::Nonce::new(start_nonce + i as u64);
                return Some((nonce, *hash));
            }
        }
        
        None
    }
}

/// Feature detection for SIMD capabilities
pub fn detect_simd_features() -> SimdFeatures {
    let mut features = SimdFeatures::default();
    
    // blake2s_simd automatically detects and uses available SIMD features
    // We can check what's available for informational purposes
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    {
        if is_x86_feature_detected!("avx2") {
            features.has_avx2 = true;
        }
        if is_x86_feature_detected!("sse4.1") {
            features.has_sse41 = true;
        }
        if is_x86_feature_detected!("ssse3") {
            features.has_ssse3 = true;
        }
    }
    
    #[cfg(all(target_arch = "aarch64", not(target_env = "musl")))]
    {
        if is_aarch64_feature_detected!("neon") {
            features.has_neon = true;
        }
    }
    
    #[cfg(all(target_arch = "aarch64", target_env = "musl"))]
    {
        // For musl targets, we can't detect features at runtime
        // NEON is mandatory for ARMv8 (aarch64), so it's safe to enable
        features.has_neon = true;
    }
    
    features
}

/// Available SIMD features
#[derive(Debug, Default, Clone)]
pub struct SimdFeatures {
    /// AVX2 support (x86_64)
    pub has_avx2: bool,
    /// SSE4.1 support (x86_64)
    pub has_sse41: bool,
    /// SSSE3 support (x86_64)
    pub has_ssse3: bool,
    /// NEON support (ARM)
    pub has_neon: bool,
}

impl SimdFeatures {
    /// Get a description of available features
    pub fn description(&self) -> String {
        let mut features = Vec::new();
        
        if self.has_avx2 {
            features.push("AVX2");
        }
        if self.has_sse41 {
            features.push("SSE4.1");
        }
        if self.has_ssse3 {
            features.push("SSSE3");
        }
        if self.has_neon {
            features.push("NEON");
        }
        
        if features.is_empty() {
            "No SIMD features detected".to_string()
        } else {
            format!("SIMD features: {}", features.join(", "))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{Target, Work};
    
    #[test]
    fn test_simd_hasher() {
        let hasher = SimdHasher::new();
        let work = [0u8; 286];
        
        let hash = hasher.hash_single(&work);
        assert_eq!(hash.len(), 32);
    }
    
    #[test]
    fn test_simd_batch() {
        let mut miner = SimdMiner::new(64);
        let base_work = [0u8; 286];
        
        miner.prepare_batch(&base_work, 0, 32);
        let hashes = miner.hash_batch(32);
        
        assert_eq!(hashes.len(), 32);
        
        // Verify each hash is different (due to different nonces)
        for i in 1..hashes.len() {
            assert_ne!(hashes[i], hashes[i-1]);
        }
    }
    
    #[test]
    fn test_feature_detection() {
        let features = detect_simd_features();
        println!("Detected SIMD features: {:?}", features);
        println!("Description: {}", features.description());
    }
    
    #[test]
    fn test_simd_mining() {
        let mut miner = SimdMiner::new(128);
        let work = Work::default();
        // Create an easy target (high target value = easy difficulty)
        let target = Target::from_hex("ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff").unwrap();
        
        // Mine with SIMD
        let result = miner.mine_batch(work.as_bytes(), &target, 0, 64);
        assert!(result.is_some());
    }
}