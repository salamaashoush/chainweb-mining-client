//! Job management for Stratum protocol
//! 
//! This module handles job creation, tracking, and management for the Stratum mining pool.

use crate::core::{ChainId, Target, Work};
use crate::error::{Error, Result};
use crate::workers::stratum::nonce::{Nonce1, NonceSize};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::SystemTime;

/// Job identifier with hex encoding support
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct JobId(String);

impl JobId {
    /// Create a new job ID from integer
    pub fn new(id: u64) -> Self {
        JobId(format!("{:x}", id))
    }

    /// Create job ID from string
    pub fn from_string(s: String) -> Self {
        JobId(s)
    }

    /// Get the job ID as string
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Parse job ID from hex string
    pub fn from_hex(hex: &str) -> Result<Self> {
        // Validate it's valid hex
        u64::from_str_radix(hex, 16)
            .map_err(|_| Error::config(format!("Invalid job ID hex: {}", hex)))?;
        Ok(JobId(hex.to_string()))
    }

    /// Convert to u64 (for internal use)
    pub fn to_u64(&self) -> Result<u64> {
        u64::from_str_radix(&self.0, 16)
            .map_err(|_| Error::config(format!("Invalid job ID format: {}", self.0)))
    }
}

impl Default for JobId {
    fn default() -> Self {
        JobId("0".to_string())
    }
}

impl std::fmt::Display for JobId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Client worker identification
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ClientWorker {
    /// Client username (typically the public key)
    pub username: String,
    /// Optional worker identifier
    pub worker_id: Option<String>,
}

impl ClientWorker {
    /// Create a new client worker
    pub fn new(username: String, worker_id: Option<String>) -> Self {
        Self { username, worker_id }
    }

    /// Parse from username string (format: "username" or "username.worker_id")
    pub fn from_username(username: &str) -> Self {
        if let Some(dot_pos) = username.find('.') {
            let (user, worker) = username.split_at(dot_pos);
            let worker_id = worker.strip_prefix('.').unwrap_or(worker);
            Self {
                username: user.to_string(),
                worker_id: if worker_id.is_empty() { None } else { Some(worker_id.to_string()) },
            }
        } else {
            Self {
                username: username.to_string(),
                worker_id: None,
            }
        }
    }

    /// Convert to username string
    pub fn to_username(&self) -> String {
        match &self.worker_id {
            Some(worker_id) => format!("{}.{}", self.username, worker_id),
            None => self.username.clone(),
        }
    }
}

/// Mining job information
#[derive(Debug, Clone)]
pub struct MiningJob {
    /// Job identifier
    pub job_id: JobId,
    /// Chain ID for this job
    pub chain_id: ChainId,
    /// Mining target
    pub target: Target,
    /// Work header template
    pub work: Work,
    /// Nonce1 for this job (pool-controlled)
    pub nonce1: Nonce1,
    /// Nonce1 size used
    pub nonce1_size: NonceSize,
    /// Job creation timestamp
    pub created_at: SystemTime,
    /// Whether this job cleans previous jobs
    pub clean_jobs: bool,
}

impl MiningJob {
    /// Create a new mining job
    pub fn new(
        job_id: JobId,
        chain_id: ChainId,
        target: Target,
        work: Work,
        nonce1: Nonce1,
        clean_jobs: bool,
    ) -> Self {
        Self {
            job_id,
            chain_id,
            target,
            work,
            nonce1_size: nonce1.size(),
            nonce1,
            created_at: SystemTime::now(),
            clean_jobs,
        }
    }

    /// Get job age in seconds
    pub fn age_seconds(&self) -> u64 {
        self.created_at
            .elapsed()
            .unwrap_or_default()
            .as_secs()
    }

    /// Check if job is expired (older than max_age_seconds)
    pub fn is_expired(&self, max_age_seconds: u64) -> bool {
        self.age_seconds() > max_age_seconds
    }

    /// Convert to Stratum notify parameters
    pub fn to_notify_params(&self) -> serde_json::Value {
        serde_json::json!([
            self.job_id.as_str(),
            hex::encode(self.work.as_bytes()),
            hex::encode(self.target.as_bytes()),
            hex::encode(self.chain_id.value().to_le_bytes()),
            self.nonce1.to_hex(),
            self.clean_jobs
        ])
    }
}

/// Job manager for tracking active mining jobs
#[derive(Debug)]
pub struct JobManager {
    /// Job counter for generating unique IDs
    job_counter: AtomicU64,
    /// Active jobs by job ID
    jobs: parking_lot::RwLock<HashMap<JobId, MiningJob>>,
    /// Maximum number of jobs to keep
    max_jobs: usize,
    /// Maximum job age in seconds
    max_job_age_seconds: u64,
}

impl JobManager {
    /// Create a new job manager
    pub fn new(max_jobs: usize, max_job_age_seconds: u64) -> Self {
        Self {
            job_counter: AtomicU64::new(0),
            jobs: parking_lot::RwLock::new(HashMap::new()),
            max_jobs,
            max_job_age_seconds,
        }
    }

    /// Generate next job ID
    pub fn next_job_id(&self) -> JobId {
        let id = self.job_counter.fetch_add(1, Ordering::Relaxed);
        JobId::new(id)
    }

    /// Add a new job
    pub fn add_job(&self, job: MiningJob) {
        let mut jobs = self.jobs.write();
        
        // If this is a clean job, clear all existing jobs first
        if job.clean_jobs {
            jobs.clear();
        } else {
            // Clean up old/expired jobs before adding
            self.cleanup_jobs_internal(&mut jobs);
        }
        
        jobs.insert(job.job_id.clone(), job);
        
        // If we exceed max jobs, remove oldest (only if not a clean job)
        if jobs.len() > self.max_jobs {
            let oldest_job_id = jobs
                .iter()
                .min_by_key(|(_, job)| job.created_at)
                .map(|(id, _)| id.clone());
            
            if let Some(job_id) = oldest_job_id {
                jobs.remove(&job_id);
            }
        }
    }

    /// Get a job by ID
    pub fn get_job(&self, job_id: &JobId) -> Option<MiningJob> {
        let jobs = self.jobs.read();
        jobs.get(job_id).cloned()
    }

    /// Get all active jobs
    pub fn get_all_jobs(&self) -> Vec<MiningJob> {
        let jobs = self.jobs.read();
        jobs.values().cloned().collect()
    }

    /// Remove a job
    pub fn remove_job(&self, job_id: &JobId) -> Option<MiningJob> {
        let mut jobs = self.jobs.write();
        jobs.remove(job_id)
    }

    /// Clean up expired jobs
    pub fn cleanup_jobs(&self) -> usize {
        let mut jobs = self.jobs.write();
        self.cleanup_jobs_internal(&mut jobs)
    }

    /// Internal cleanup (assumes write lock is held)
    fn cleanup_jobs_internal(&self, jobs: &mut HashMap<JobId, MiningJob>) -> usize {
        let initial_count = jobs.len();
        jobs.retain(|_, job| !job.is_expired(self.max_job_age_seconds));
        initial_count - jobs.len()
    }

    /// Clean all jobs (for clean_jobs=true)
    pub fn clean_all_jobs(&self) {
        let mut jobs = self.jobs.write();
        jobs.clear();
    }

    /// Get job count
    pub fn job_count(&self) -> usize {
        let jobs = self.jobs.read();
        jobs.len()
    }

    /// Get job statistics
    pub fn get_stats(&self) -> JobStats {
        let jobs = self.jobs.read();
        let _now = SystemTime::now();
        
        let mut total_age = 0u64;
        let mut oldest_age = 0u64;
        
        for job in jobs.values() {
            let age = job.age_seconds();
            total_age += age;
            oldest_age = oldest_age.max(age);
        }
        
        JobStats {
            total_jobs: jobs.len(),
            average_age_seconds: if jobs.is_empty() { 0.0 } else { total_age as f64 / jobs.len() as f64 },
            oldest_age_seconds: oldest_age,
        }
    }
}

impl Default for JobManager {
    fn default() -> Self {
        Self::new(1000, 300) // 1000 jobs, 5 minutes max age
    }
}

/// Job manager statistics
#[derive(Debug, Clone)]
pub struct JobStats {
    pub total_jobs: usize,
    pub average_age_seconds: f64,
    pub oldest_age_seconds: u64,
}

/// Shared job manager instance
pub type SharedJobManager = Arc<JobManager>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_job_id() {
        let job_id = JobId::new(255);
        assert_eq!(job_id.as_str(), "ff");
        assert_eq!(job_id.to_u64().unwrap(), 255);

        let parsed = JobId::from_hex("ff").unwrap();
        assert_eq!(parsed, job_id);
    }

    #[test]
    fn test_client_worker() {
        let worker = ClientWorker::from_username("alice.worker1");
        assert_eq!(worker.username, "alice");
        assert_eq!(worker.worker_id, Some("worker1".to_string()));
        assert_eq!(worker.to_username(), "alice.worker1");

        let simple = ClientWorker::from_username("bob");
        assert_eq!(simple.username, "bob");
        assert_eq!(simple.worker_id, None);
        assert_eq!(simple.to_username(), "bob");
    }

    #[test]
    fn test_mining_job() {
        let job_id = JobId::new(1);
        let chain_id = ChainId::new(0);
        let target = Target::from_bytes([0xFF; 32]);
        let work = Work::from_bytes([0x42; 286]);
        let nonce1 = Nonce1::new(NonceSize::new(4).unwrap(), 0x12345678).unwrap();

        let job = MiningJob::new(job_id.clone(), chain_id, target, work, nonce1, false);
        
        assert_eq!(job.job_id, job_id);
        assert_eq!(job.chain_id, chain_id);
        assert!(!job.clean_jobs);
        assert!(job.age_seconds() < 1); // Should be very recent
    }

    #[test]
    fn test_job_manager() {
        let manager = JobManager::new(2, 60); // Max 2 jobs, 60 seconds age
        
        // Add first job
        let job1_id = manager.next_job_id();
        let job1 = MiningJob::new(
            job1_id.clone(),
            ChainId::new(0),
            Target::from_bytes([0xFF; 32]),
            Work::from_bytes([0x42; 286]),
            Nonce1::new(NonceSize::new(4).unwrap(), 0x11111111).unwrap(),
            false,
        );
        manager.add_job(job1);
        
        // Add second job
        let job2_id = manager.next_job_id();
        let job2 = MiningJob::new(
            job2_id.clone(),
            ChainId::new(1),
            Target::from_bytes([0xEE; 32]),
            Work::from_bytes([0x43; 286]),
            Nonce1::new(NonceSize::new(4).unwrap(), 0x22222222).unwrap(),
            false,
        );
        manager.add_job(job2);
        
        assert_eq!(manager.job_count(), 2);
        
        // Get jobs
        let retrieved_job1 = manager.get_job(&job1_id).unwrap();
        assert_eq!(retrieved_job1.job_id, job1_id);
        
        // Add third job (should remove oldest due to max_jobs=2)
        let job3_id = manager.next_job_id();
        let job3 = MiningJob::new(
            job3_id.clone(),
            ChainId::new(2),
            Target::from_bytes([0xDD; 32]),
            Work::from_bytes([0x44; 286]),
            Nonce1::new(NonceSize::new(4).unwrap(), 0x33333333).unwrap(),
            false,
        );
        manager.add_job(job3);
        
        assert_eq!(manager.job_count(), 2);
        
        // First job should be gone
        assert!(manager.get_job(&job1_id).is_none());
        assert!(manager.get_job(&job2_id).is_some());
        assert!(manager.get_job(&job3_id).is_some());
    }

    #[test]
    fn test_job_cleanup() {
        let manager = JobManager::new(100, 60); // Normal max age
        
        // Create an old job by manually setting created_at to past
        let job_id = manager.next_job_id();
        let mut job = MiningJob::new(
            job_id.clone(),
            ChainId::new(0),
            Target::from_bytes([0xFF; 32]),
            Work::from_bytes([0x42; 286]),
            Nonce1::new(NonceSize::new(4).unwrap(), 0x12345678).unwrap(),
            false,
        );
        // Make the job appear to be 2 minutes old (older than 60 seconds)
        job.created_at = SystemTime::now() - std::time::Duration::from_secs(120);
        
        // Directly insert the old job to bypass automatic cleanup
        {
            let mut jobs = manager.jobs.write();
            jobs.insert(job_id.clone(), job);
        }
        
        assert_eq!(manager.job_count(), 1);
        
        // Now cleanup should remove the expired job
        let cleaned = manager.cleanup_jobs();
        assert_eq!(cleaned, 1);
        assert_eq!(manager.job_count(), 0);
    }

    #[test]
    fn test_job_stats() {
        let manager = JobManager::new(10, 60);
        
        // Add a few jobs
        for i in 0..3 {
            let job_id = manager.next_job_id();
            let job = MiningJob::new(
                job_id,
                ChainId::new(i),
                Target::from_bytes([0xFF; 32]),
                Work::from_bytes([0x42; 286]),
                Nonce1::new(NonceSize::new(4).unwrap(), 0x12345678).unwrap(),
                false,
            );
            manager.add_job(job);
        }
        
        let stats = manager.get_stats();
        assert_eq!(stats.total_jobs, 3);
        assert!(stats.average_age_seconds < 1.0);
        assert!(stats.oldest_age_seconds < 1);
    }

    #[test]
    fn test_clean_all_jobs() {
        let manager = JobManager::new(10, 60);
        
        // Add jobs
        for i in 0..5 {
            let job_id = manager.next_job_id();
            let job = MiningJob::new(
                job_id,
                ChainId::new(i),
                Target::from_bytes([0xFF; 32]),
                Work::from_bytes([0x42; 286]),
                Nonce1::new(NonceSize::new(4).unwrap(), 0x12345678).unwrap(),
                false,
            );
            manager.add_job(job);
        }
        
        assert_eq!(manager.job_count(), 5);
        
        manager.clean_all_jobs();
        assert_eq!(manager.job_count(), 0);
    }
}