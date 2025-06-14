//! Chain ID type for identifying Chainweb chains

use serde::{Deserialize, Serialize};
use std::fmt;

/// Represents a chain ID in the Chainweb network
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ChainId(pub u16);

impl ChainId {
    /// Create a new ChainId
    pub const fn new(id: u16) -> Self {
        Self(id)
    }

    /// Get the inner value
    pub const fn value(self) -> u16 {
        self.0
    }
}

impl fmt::Display for ChainId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<u16> for ChainId {
    fn from(value: u16) -> Self {
        Self(value)
    }
}

impl From<ChainId> for u16 {
    fn from(chain_id: ChainId) -> Self {
        chain_id.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chain_id_creation() {
        let chain_id = ChainId::new(0);
        assert_eq!(chain_id.value(), 0);

        let chain_id = ChainId::new(9);
        assert_eq!(chain_id.value(), 9);
    }

    #[test]
    fn test_chain_id_display() {
        let chain_id = ChainId::new(5);
        assert_eq!(chain_id.to_string(), "5");
    }

    #[test]
    fn test_chain_id_conversions() {
        let chain_id: ChainId = 7u16.into();
        assert_eq!(chain_id.value(), 7);

        let value: u16 = chain_id.into();
        assert_eq!(value, 7);
    }

    #[test]
    fn test_chain_id_serde() {
        let chain_id = ChainId::new(3);
        let json = serde_json::to_string(&chain_id).unwrap();
        assert_eq!(json, "3");

        let deserialized: ChainId = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, chain_id);
    }
}
