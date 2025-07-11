//! Stratum session management

use super::nonce::Nonce1;
use std::collections::VecDeque;
use std::time::Instant;
use uuid::Uuid;

/// Session ID type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SessionId(Uuid);

impl SessionId {
    /// Create a new session ID
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for SessionId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for SessionId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Stratum mining session
pub struct StratumSession {
    /// Session ID
    pub id: SessionId,
    /// Worker name
    pub worker_name: Option<String>,
    /// Current difficulty
    pub difficulty: f64,
    /// Extra nonce 1
    pub extranonce1: Nonce1,
    /// Total shares submitted
    pub shares_submitted: u64,
    /// Valid shares
    pub shares_valid: u64,
    /// Last share submission time
    pub last_share_time: Option<Instant>,
    /// Sum of hash rates for averaging
    pub hash_rate_sum: f64,
    /// Number of share intervals counted
    pub share_count: u64,
    /// Recent share timestamps for smoothing (max 10 entries)
    pub recent_shares: VecDeque<(Instant, f64)>,
    /// Estimated hash rate (hashes per second)
    pub estimated_hashrate: f64,
    /// Session-specific target (may differ from work target)
    pub session_target: Option<crate::core::Target>,
}

impl StratumSession {
    /// Create a new session
    pub fn new(extranonce1: Nonce1, initial_difficulty: f64) -> Self {
        Self {
            id: SessionId::new(),
            worker_name: None,
            difficulty: initial_difficulty,
            extranonce1,
            shares_submitted: 0,
            shares_valid: 0,
            last_share_time: None,
            hash_rate_sum: 0.0,
            share_count: 0,
            recent_shares: VecDeque::with_capacity(10),
            estimated_hashrate: 0.0,
            session_target: None,
        }
    }

    /// Update hash rate estimation based on a new share
    pub fn update_hash_rate(&mut self, current_difficulty: f64) {
        let now = Instant::now();
        
        if let Some(last_time) = self.last_share_time {
            let elapsed = now.duration_since(last_time).as_secs_f64();
            
            // Avoid division by zero and ignore very fast shares (likely errors)
            if elapsed > 0.1 {
                // Calculate instantaneous hash rate
                // HashRate = Difficulty / Time
                let instant_hashrate = current_difficulty / elapsed;
                
                // Add to recent shares for smoothing
                self.recent_shares.push_back((now, instant_hashrate));
                if self.recent_shares.len() > 10 {
                    self.recent_shares.pop_front();
                }
                
                // Update running average
                self.hash_rate_sum += instant_hashrate;
                self.share_count += 1;
                
                // Calculate weighted average (recent shares have more weight)
                let mut weighted_sum = 0.0;
                let mut weight_total = 0.0;
                for (i, (_, rate)) in self.recent_shares.iter().enumerate() {
                    let weight = (i + 1) as f64; // More recent = higher weight
                    weighted_sum += rate * weight;
                    weight_total += weight;
                }
                
                if weight_total > 0.0 {
                    self.estimated_hashrate = weighted_sum / weight_total;
                }
            }
        }
        
        self.last_share_time = Some(now);
    }
}
