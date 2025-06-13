//! Core types and structures for the mining client
//!
//! This module contains the fundamental types used throughout the mining client,
//! including Work, Target, Nonce, and ChainId.

mod chain_id;
mod nonce;
mod target;
mod work;

pub use chain_id::ChainId;
pub use nonce::Nonce;
pub use target::Target;
pub use work::Work;

/// Constants for the mining protocol
pub mod constants {
    /// Size of a work header in bytes
    pub const WORK_SIZE: usize = 286;

    /// Size of the nonce in bytes
    pub const NONCE_SIZE: usize = 8;

    /// Offset of the nonce in the work header
    pub const NONCE_OFFSET: usize = WORK_SIZE - NONCE_SIZE;

    /// Size of a hash in bytes (Blake2s-256)
    pub const HASH_SIZE: usize = 32;

    /// Size of the target in bytes
    pub const TARGET_SIZE: usize = 32;
}

#[cfg(test)]
mod tests {
    use super::constants::*;

    #[test]
    fn test_constants() {
        assert_eq!(WORK_SIZE, 286);
        assert_eq!(NONCE_SIZE, 8);
        assert_eq!(NONCE_OFFSET, 278);
        assert_eq!(HASH_SIZE, 32);
        assert_eq!(TARGET_SIZE, 32);
    }
}

#[cfg(test)]
mod tests_extended;
