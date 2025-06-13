//! Stratum session management

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
    pub extranonce1: Vec<u8>,
    /// Total shares submitted
    pub shares_submitted: u64,
    /// Valid shares
    pub shares_valid: u64,
}

impl StratumSession {
    /// Create a new session
    pub fn new(extranonce1: Vec<u8>, initial_difficulty: f64) -> Self {
        Self {
            id: SessionId::new(),
            worker_name: None,
            difficulty: initial_difficulty,
            extranonce1,
            shares_submitted: 0,
            shares_valid: 0,
        }
    }
}
