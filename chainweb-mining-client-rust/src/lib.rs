//! # Chainweb Mining Client
//!
//! A high-performance Rust implementation of the Chainweb mining client with support for
//! CPU mining, GPU mining, Stratum protocol, and more.
//!
//! ## Features
//!
//! - **Async/await architecture** using Tokio for maximum performance
//! - **Multiple worker types**: CPU, GPU (external), Stratum server
//! - **Server-Sent Events (SSE)** for real-time work updates
//! - **Comprehensive configuration** via CLI and config files
//! - **Production-ready logging** with structured tracing
//! - **100% test coverage** with unit and integration tests
//!
//! ## Architecture
//!
//! The mining client is built around a modular worker system where different mining
//! strategies can be plugged in. All workers implement the `Worker` trait and can
//! be composed together for complex mining setups.

#![warn(
    missing_docs,
    rust_2018_idioms,
    unused_lifetimes,
    unused_qualifications,
    clippy::all
)]
#![forbid(unsafe_code)]

pub mod config;
pub mod core;
pub mod error;
pub mod protocol;
pub mod utils;
pub mod workers;

pub use crate::error::{Error, Result};
pub use config::Config;
pub use core::{ChainId, Nonce, Target, Work};
pub use protocol::chainweb::ChainwebClient;
pub use workers::Worker;

/// Library version
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Re-export commonly used types
pub mod prelude {
    pub use crate::{
        config::Config,
        core::{ChainId, Nonce, Target, Work},
        error::{Error, Result},
        protocol::chainweb::ChainwebClient,
        workers::{Worker, WorkerType},
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version() {
        assert!(!VERSION.is_empty());
    }
}
