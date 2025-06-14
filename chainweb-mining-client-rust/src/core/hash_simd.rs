//! SIMD-optimized Blake2s-256 hashing for mining operations
//!
//! This module provides optimized Blake2s-256 implementations that leverage
//! CPU SIMD instructions when available for improved mining performance.

use blake2::{Blake2s256, Digest};

// Note: This implementation avoids unsafe code as per project requirements
// In a real-world scenario, SIMD optimizations would require unsafe blocks

/// Optimized hasher that provides consistent interface for SIMD-style batch processing
/// Note: Actual SIMD instructions require unsafe code, so this provides safe equivalents
pub struct OptimizedHasher {
    // Placeholder for future SIMD detection when unsafe code is allowed
    batch_size_hint: usize,
}

impl OptimizedHasher {
    /// Create a new optimized hasher with optimal batch sizing
    pub fn new() -> Self {
        // In a real implementation, this would detect CPU features
        // For now, use safe optimizations with good batch sizes
        Self {
            batch_size_hint: if cfg!(target_arch = "x86_64") { 64 } else { 32 },
        }
    }
    
    /// Hash a single work item using optimized implementation
    #[inline]
    pub fn hash_single(&self, work_bytes: &[u8; 286]) -> [u8; 32] {
        // Use optimized scalar implementation
        // In real SIMD implementation, this would dispatch to SIMD variants
        self.hash_single_scalar(work_bytes)
    }
    
    /// Hash multiple work items using optimized batch processing
    #[inline]
    pub fn hash_batch(&self, work_items: &[[u8; 286]], results: &mut [[u8; 32]]) {
        assert_eq!(work_items.len(), results.len());
        
        // Use optimized batch processing (parallel when possible)
        // In real SIMD implementation, this would use vectorized instructions
        self.hash_batch_parallel(work_items, results);
    }
    
    /// Standard scalar implementation (fallback)
    #[inline(always)]
    fn hash_single_scalar(&self, work_bytes: &[u8; 286]) -> [u8; 32] {
        let mut hasher = Blake2s256::new();
        hasher.update(work_bytes);
        hasher.finalize().into()
    }
    
    /// Parallel batch implementation using rayon
    fn hash_batch_parallel(&self, work_items: &[[u8; 286]], results: &mut [[u8; 32]]) {
        use rayon::prelude::*;
        
        // For smaller batches, use sequential processing to avoid overhead
        if work_items.len() < self.batch_size_hint {
            for (work, result) in work_items.iter().zip(results.iter_mut()) {
                *result = self.hash_single_scalar(work);
            }
            return;
        }
        
        // Process in parallel chunks for better cache efficiency
        let chunk_size = self.batch_size_hint;
        
        work_items
            .par_chunks(chunk_size)
            .zip(results.par_chunks_mut(chunk_size))
            .for_each(|(work_chunk, result_chunk)| {
                for (work, result) in work_chunk.iter().zip(result_chunk.iter_mut()) {
                    *result = self.hash_single_scalar(work);
                }
            });
    }
}

/// Vectorized mining hasher for batch operations
pub struct VectorizedMiner {
    hasher: OptimizedHasher,
    /// Buffer for work items being processed
    pub work_buffer: Vec<[u8; 286]>,
    hash_buffer: Vec<[u8; 32]>,
}

impl VectorizedMiner {
    /// Create a new vectorized miner with specified batch size
    pub fn new(batch_size: usize) -> Self {
        Self {
            hasher: OptimizedHasher::new(),
            work_buffer: vec![[0u8; 286]; batch_size],
            hash_buffer: vec![[0u8; 32]; batch_size],
        }
    }
    
    /// Prepare work items with different nonces
    pub fn prepare_work_batch(
        &mut self,
        base_work: &[u8; 286],
        start_nonce: u64,
        count: usize,
    ) {
        assert!(count <= self.work_buffer.len());
        
        for (i, work_item) in self.work_buffer[..count].iter_mut().enumerate() {
            *work_item = *base_work;
            
            // Update nonce (at bytes 8-16 in work structure)
            let nonce = start_nonce + i as u64;
            work_item[8..16].copy_from_slice(&nonce.to_le_bytes());
        }
    }
    
    /// Hash the prepared batch and return results
    pub fn hash_prepared_batch(&mut self, count: usize) -> &[[u8; 32]] {
        assert!(count <= self.work_buffer.len());
        
        self.hasher.hash_batch(
            &self.work_buffer[..count],
            &mut self.hash_buffer[..count],
        );
        
        &self.hash_buffer[..count]
    }
    
    /// Complete workflow: prepare nonces and hash in one call
    pub fn mine_batch(
        &mut self,
        base_work: &[u8; 286],
        start_nonce: u64,
        count: usize,
    ) -> &[[u8; 32]] {
        self.prepare_work_batch(base_work, start_nonce, count);
        self.hash_prepared_batch(count)
    }
}

impl Default for VectorizedMiner {
    fn default() -> Self {
        Self::new(64) // Default batch size optimized for cache lines
    }
}

/// Auto-tuning hasher that adapts batch size based on performance
pub struct AdaptiveHasher {
    vectorized: VectorizedMiner,
    optimal_batch_size: usize,
    last_benchmark: std::time::Instant,
    benchmark_interval: std::time::Duration,
}

impl AdaptiveHasher {
    /// Create a new adaptive hasher
    pub fn new() -> Self {
        Self {
            vectorized: VectorizedMiner::new(128), // Start with larger buffer
            optimal_batch_size: 64,
            last_benchmark: std::time::Instant::now(),
            benchmark_interval: std::time::Duration::from_secs(30),
        }
    }
    
    /// Get the current optimal batch size
    pub fn optimal_batch_size(&self) -> usize {
        self.optimal_batch_size
    }
    
    /// Benchmark different batch sizes and update optimal size
    pub fn auto_tune(&mut self, base_work: &[u8; 286]) {
        if self.last_benchmark.elapsed() < self.benchmark_interval {
            return;
        }
        
        let batch_sizes = [16, 32, 64, 128, 256];
        let mut best_size = self.optimal_batch_size;
        let mut best_throughput = 0.0;
        
        for &batch_size in &batch_sizes {
            let start = std::time::Instant::now();
            let iterations = 1000;
            
            // Resize buffer if needed
            if batch_size > self.vectorized.work_buffer.len() {
                self.vectorized = VectorizedMiner::new(batch_size);
            }
            
            // Benchmark this batch size
            for i in 0..iterations {
                let start_nonce = i as u64 * batch_size as u64;
                self.vectorized.mine_batch(base_work, start_nonce, batch_size);
            }
            
            let elapsed = start.elapsed();
            let hashes_per_sec = (iterations * batch_size) as f64 / elapsed.as_secs_f64();
            
            if hashes_per_sec > best_throughput {
                best_throughput = hashes_per_sec;
                best_size = batch_size;
            }
        }
        
        self.optimal_batch_size = best_size;
        self.last_benchmark = std::time::Instant::now();
        
        tracing::debug!(
            "Auto-tuned batch size to {} (throughput: {:.0} H/s)",
            best_size,
            best_throughput
        );
    }
}

impl Default for AdaptiveHasher {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_optimized_hasher_creation() {
        let hasher = OptimizedHasher::new();
        let work = [0u8; 286];
        let hash = hasher.hash_single(&work);
        assert_eq!(hash.len(), 32);
    }
    
    #[test]
    fn test_hash_consistency() {
        let hasher = OptimizedHasher::new();
        let work = [0u8; 286];
        
        // Hash the same work multiple times
        let hash1 = hasher.hash_single(&work);
        let hash2 = hasher.hash_single(&work);
        
        assert_eq!(hash1, hash2);
    }
    
    #[test]
    fn test_batch_hashing() {
        let hasher = OptimizedHasher::new();
        let work_items = vec![[0u8; 286]; 8];
        let mut results = vec![[0u8; 32]; 8];
        
        hasher.hash_batch(&work_items, &mut results);
        
        // All results should be the same since all work items are identical
        for i in 1..results.len() {
            assert_eq!(results[0], results[i]);
        }
    }
    
    #[test]
    fn test_vectorized_miner() {
        let mut miner = VectorizedMiner::new(16);
        let base_work = [1u8; 286];
        
        let hashes = miner.mine_batch(&base_work, 0, 8);
        assert_eq!(hashes.len(), 8);
        
        // Each hash should be different due to different nonces
        for i in 1..hashes.len() {
            assert_ne!(hashes[0], hashes[i]);
        }
    }
    
    #[test]
    fn test_adaptive_hasher() {
        let mut hasher = AdaptiveHasher::new();
        let base_work = [2u8; 286];
        
        let initial_batch_size = hasher.optimal_batch_size();
        assert!(initial_batch_size > 0);
        
        // Force a benchmark (would normally be rate-limited)
        hasher.benchmark_interval = std::time::Duration::from_nanos(1);
        hasher.auto_tune(&base_work);
        
        let tuned_batch_size = hasher.optimal_batch_size();
        assert!(tuned_batch_size > 0);
    }
    
    #[test]
    fn test_nonce_variation() {
        let mut miner = VectorizedMiner::new(4);
        let mut base_work = [0u8; 286];
        
        // Set a known pattern in the nonce area
        base_work[8..16].copy_from_slice(&42u64.to_le_bytes());
        
        miner.prepare_work_batch(&base_work, 100, 4);
        
        // Check that nonces were properly updated
        for i in 0..4 {
            let expected_nonce = 100 + i as u64;
            let actual_nonce = u64::from_le_bytes([
                miner.work_buffer[i][8],
                miner.work_buffer[i][9],
                miner.work_buffer[i][10],
                miner.work_buffer[i][11],
                miner.work_buffer[i][12],
                miner.work_buffer[i][13],
                miner.work_buffer[i][14],
                miner.work_buffer[i][15],
            ]);
            assert_eq!(actual_nonce, expected_nonce);
        }
    }
}