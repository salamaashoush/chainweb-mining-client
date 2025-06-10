//! Chainweb Mining Client
//!
//! A high-performance, async mining client for Kadena's Chainweb blockchain supporting:
//! - ASIC mining through Stratum protocol
//! - Multi-threaded CPU mining  
//! - External worker integration (GPU miners)
//! - Simulation and testing modes
//! - Rock-solid reliability with comprehensive error handling

pub mod config;
pub mod error;
pub mod types;
pub mod client;
pub mod worker;
pub mod stratum;
pub mod crypto;
pub mod utils;

pub use config::Config;
pub use error::{Error, Result};
pub use types::*;

/// Application information
pub const APP_NAME: &str = "chainweb-mining-client";
pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");
pub const APP_DESCRIPTION: &str = env!("CARGO_PKG_DESCRIPTION");