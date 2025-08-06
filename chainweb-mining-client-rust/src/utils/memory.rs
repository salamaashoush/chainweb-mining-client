//! Memory optimization utilities
//!
//! Provides memory management optimizations including:
//! - Object pools for frequently allocated structures
//! - Memory usage statistics

use crate::core::Work;
use crossbeam::queue::SegQueue;
use once_cell::sync::Lazy;
use std::sync::Arc;
use tracing::debug;

/// Object pool for Work structures to reduce allocations
pub struct WorkPool {
    pool: Arc<SegQueue<Box<Work>>>,
    capacity: usize,
}

impl WorkPool {
    /// Create a new work pool with the specified capacity
    pub fn new(capacity: usize) -> Self {
        let pool = Arc::new(SegQueue::new());
        
        // Pre-allocate objects
        for _ in 0..capacity {
            pool.push(Box::new(Work::default()));
        }
        
        Self { pool, capacity }
    }
    
    /// Get a work object from the pool or allocate a new one
    pub fn get(&self) -> Box<Work> {
        self.pool.pop().unwrap_or_else(|| {
            debug!("WorkPool: allocating new Work object");
            Box::new(Work::default())
        })
    }
    
    /// Return a work object to the pool
    pub fn put(&self, mut work: Box<Work>) {
        // Reset the work object before returning to pool
        *work = Work::default();
        
        // Only return to pool if under capacity
        if self.pool.len() < self.capacity {
            self.pool.push(work);
        }
    }
}

/// Global work pool instance
pub static WORK_POOL: Lazy<WorkPool> = Lazy::new(|| WorkPool::new(1024));

/// Guard for automatic return of work objects to the pool
pub struct PooledWork {
    work: Option<Box<Work>>,
}

impl PooledWork {
    /// Get a pooled work object
    pub fn get() -> Self {
        Self {
            work: Some(WORK_POOL.get()),
        }
    }
    
    /// Access the work object
    pub fn as_ref(&self) -> &Work {
        self.work.as_ref().unwrap()
    }
    
    /// Mutably access the work object
    pub fn as_mut(&mut self) -> &mut Work {
        self.work.as_mut().unwrap()
    }
    
    /// Take ownership of the work object (prevents automatic return to pool)
    pub fn take(mut self) -> Box<Work> {
        self.work.take().unwrap()
    }
}

impl Drop for PooledWork {
    fn drop(&mut self) {
        if let Some(work) = self.work.take() {
            WORK_POOL.put(work);
        }
    }
}

impl std::ops::Deref for PooledWork {
    type Target = Work;
    
    fn deref(&self) -> &Self::Target {
        self.as_ref()
    }
}

impl std::ops::DerefMut for PooledWork {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.as_mut()
    }
}

/// Memory allocation statistics
#[derive(Debug, Default, Clone)]
pub struct MemoryStats {
    /// Total number of allocations
    pub allocations: u64,
    /// Total number of deallocations
    pub deallocations: u64,
    /// Total bytes allocated
    pub bytes_allocated: u64,
    /// Total bytes deallocated
    pub bytes_deallocated: u64,
    /// Number of pool hits
    pub pool_hits: u64,
    /// Number of pool misses
    pub pool_misses: u64,
}

impl MemoryStats {
    /// Get current memory usage
    pub fn current_usage(&self) -> i64 {
        self.bytes_allocated as i64 - self.bytes_deallocated as i64
    }
    
    /// Get pool hit rate
    pub fn pool_hit_rate(&self) -> f64 {
        let total = self.pool_hits + self.pool_misses;
        if total == 0 {
            0.0
        } else {
            self.pool_hits as f64 / total as f64
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_work_pool() {
        let pool = WorkPool::new(10);
        
        // Get objects from pool
        let mut works = Vec::new();
        for _ in 0..5 {
            works.push(pool.get());
        }
        
        // Return objects to pool
        for work in works {
            pool.put(work);
        }
        
        // Verify pool has objects
        assert!(pool.pool.len() > 0);
    }
    
    #[test]
    fn test_pooled_work() {
        let initial_len = WORK_POOL.pool.len();
        
        {
            let mut pooled = PooledWork::get();
            // Use the work object
            pooled.as_mut();
        } // Automatically returned to pool
        
        // Should have at least the same number of objects
        assert!(WORK_POOL.pool.len() >= initial_len);
    }
}