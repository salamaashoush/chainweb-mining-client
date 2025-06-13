//! Error types for the mining client
//!
//! This module provides a comprehensive error handling system using `thiserror`
//! for automatic error trait implementations.

use thiserror::Error;

/// Main error type for the mining client
#[derive(Error, Debug)]
pub enum Error {
    /// Configuration errors
    #[error("Configuration error: {0}")]
    Config(String),

    /// HTTP/Network errors
    #[error("Network error: {0}")]
    Network(#[from] reqwest::Error),

    /// JSON parsing errors
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    /// I/O errors
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// Worker errors
    #[error("Worker error: {0}")]
    Worker(String),

    /// Mining protocol errors
    #[error("Protocol error: {0}")]
    Protocol(String),

    /// Stratum protocol errors
    #[error("Stratum error: {0}")]
    Stratum(String),

    /// Invalid work format
    #[error("Invalid work: {0}")]
    InvalidWork(String),

    /// Invalid target format
    #[error("Invalid target: {0}")]
    InvalidTarget(String),

    /// External process errors
    #[error("External process error: {0}")]
    ExternalProcess(String),

    /// Timeout errors
    #[error("Operation timed out: {0}")]
    Timeout(String),

    /// Channel send errors
    #[error("Channel send error: {0}")]
    ChannelSend(String),

    /// Channel receive errors
    #[error("Channel receive error: {0}")]
    ChannelRecv(String),

    /// Generic errors
    #[error("{0}")]
    Other(String),
}

/// Result type alias for the mining client
pub type Result<T> = std::result::Result<T, Error>;

impl Error {
    /// Create a configuration error
    pub fn config(msg: impl Into<String>) -> Self {
        Self::Config(msg.into())
    }

    /// Create a network error
    pub fn network(msg: impl Into<String>) -> Self {
        Self::Other(msg.into())
    }

    /// Create a worker error
    pub fn worker(msg: impl Into<String>) -> Self {
        Self::Worker(msg.into())
    }

    /// Create a protocol error
    pub fn protocol(msg: impl Into<String>) -> Self {
        Self::Protocol(msg.into())
    }

    /// Create a stratum error
    pub fn stratum(msg: impl Into<String>) -> Self {
        Self::Stratum(msg.into())
    }

    /// Create an invalid work error
    pub fn invalid_work(msg: impl Into<String>) -> Self {
        Self::InvalidWork(msg.into())
    }

    /// Create an invalid target error
    pub fn invalid_target(msg: impl Into<String>) -> Self {
        Self::InvalidTarget(msg.into())
    }

    /// Create an external process error
    pub fn external_process(msg: impl Into<String>) -> Self {
        Self::ExternalProcess(msg.into())
    }

    /// Create a timeout error
    pub fn timeout(msg: impl Into<String>) -> Self {
        Self::Timeout(msg.into())
    }

    /// Create a channel send error
    pub fn channel_send(msg: impl Into<String>) -> Self {
        Self::ChannelSend(msg.into())
    }

    /// Create a channel receive error
    pub fn channel_recv(msg: impl Into<String>) -> Self {
        Self::ChannelRecv(msg.into())
    }

    /// Create a generic error
    pub fn other(msg: impl Into<String>) -> Self {
        Self::Other(msg.into())
    }

    /// Create a JSON error
    pub fn json(err: serde_json::Error) -> Self {
        Self::Json(err)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let err = Error::config("missing field");
        assert_eq!(err.to_string(), "Configuration error: missing field");

        let err = Error::worker("worker failed");
        assert_eq!(err.to_string(), "Worker error: worker failed");
    }

    #[test]
    fn test_error_conversions() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
        let err: Error = io_err.into();
        assert!(matches!(err, Error::Io(_)));

        let json_err = serde_json::from_str::<String>("invalid").unwrap_err();
        let err: Error = json_err.into();
        assert!(matches!(err, Error::Json(_)));
    }
}