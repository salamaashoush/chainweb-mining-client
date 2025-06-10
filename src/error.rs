//! Error handling for chainweb mining client
//!
//! Comprehensive error types covering all mining operations with proper context
//! and recovery information.

use std::fmt;
use thiserror::Error;

/// Result type alias for chainweb mining operations
pub type Result<T> = std::result::Result<T, Error>;

/// Main error type for chainweb mining client
#[derive(Error, Debug)]
pub enum Error {
    /// HTTP request errors
    #[error("HTTP request failed: {0}")]
    Http(#[from] reqwest::Error),

    /// JSON serialization/deserialization errors
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    /// YAML configuration parsing errors
    #[error("YAML error: {0}")]
    Yaml(#[from] serde_yaml::Error),

    /// I/O errors
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// Configuration errors
    #[error("Configuration error: {message}")]
    Config { message: String },

    /// Mining work errors
    #[error("Work error: {message}")]
    Work { message: String },

    /// Target validation errors
    #[error("Invalid target: {message}")]
    Target { message: String },

    /// Nonce validation errors
    #[error("Invalid nonce: {message}")]
    Nonce { message: String },

    /// Stratum protocol errors
    #[error("Stratum error: {message}")]
    Stratum { message: String },

    /// Worker errors
    #[error("Worker error: {worker_type}: {message}")]
    Worker { worker_type: String, message: String },

    /// Chainweb node communication errors
    #[error("Chainweb node error: {message}")]
    ChainwebNode { message: String },

    /// Crypto errors
    #[error("Cryptographic error: {message}")]
    Crypto { message: String },

    /// External process errors
    #[error("External process error: {message}")]
    ExternalProcess { message: String },

    /// Timeout errors
    #[error("Operation timed out: {operation}")]
    Timeout { operation: String },

    /// Network errors
    #[error("Network error: {message}")]
    Network { message: String },

    /// Generic errors with context
    #[error("Error in {context}: {message}")]
    Generic { context: String, message: String },

    /// Cancellation errors for async operations
    #[error("Operation was cancelled: {operation}")]
    Cancelled { operation: String },

    /// Invalid state errors
    #[error("Invalid state: {message}")]
    InvalidState { message: String },
}

impl Error {
    /// Create a configuration error
    pub fn config(message: impl Into<String>) -> Self {
        Self::Config {
            message: message.into(),
        }
    }

    /// Create a work error
    pub fn work(message: impl Into<String>) -> Self {
        Self::Work {
            message: message.into(),
        }
    }

    /// Create a target error
    pub fn target(message: impl Into<String>) -> Self {
        Self::Target {
            message: message.into(),
        }
    }

    /// Create a nonce error
    pub fn nonce(message: impl Into<String>) -> Self {
        Self::Nonce {
            message: message.into(),
        }
    }

    /// Create a stratum error
    pub fn stratum(message: impl Into<String>) -> Self {
        Self::Stratum {
            message: message.into(),
        }
    }

    /// Create a worker error
    pub fn worker(worker_type: impl Into<String>, message: impl Into<String>) -> Self {
        Self::Worker {
            worker_type: worker_type.into(),
            message: message.into(),
        }
    }

    /// Create a chainweb node error
    pub fn chainweb_node(message: impl Into<String>) -> Self {
        Self::ChainwebNode {
            message: message.into(),
        }
    }

    /// Create a crypto error
    pub fn crypto(message: impl Into<String>) -> Self {
        Self::Crypto {
            message: message.into(),
        }
    }

    /// Create an external process error
    pub fn external_process(message: impl Into<String>) -> Self {
        Self::ExternalProcess {
            message: message.into(),
        }
    }

    /// Create a timeout error
    pub fn timeout(operation: impl Into<String>) -> Self {
        Self::Timeout {
            operation: operation.into(),
        }
    }

    /// Create a network error
    pub fn network(message: impl Into<String>) -> Self {
        Self::Network {
            message: message.into(),
        }
    }

    /// Create a generic error with context
    pub fn generic(context: impl Into<String>, message: impl Into<String>) -> Self {
        Self::Generic {
            context: context.into(),
            message: message.into(),
        }
    }

    /// Create a cancellation error
    pub fn cancelled(operation: impl Into<String>) -> Self {
        Self::Cancelled {
            operation: operation.into(),
        }
    }

    /// Create an invalid state error
    pub fn invalid_state(message: impl Into<String>) -> Self {
        Self::InvalidState {
            message: message.into(),
        }
    }

    /// Check if error is retryable
    pub fn is_retryable(&self) -> bool {
        match self {
            Error::Http(e) => {
                if let Some(status) = e.status() {
                    status.is_server_error() || status == reqwest::StatusCode::TOO_MANY_REQUESTS
                } else {
                    // Network errors are typically retryable
                    e.is_timeout() || e.is_connect() || e.is_request()
                }
            }
            Error::Network { .. } => true,
            Error::Timeout { .. } => true,
            Error::ChainwebNode { .. } => true,
            Error::Io(_) => true,
            _ => false,
        }
    }

    /// Get error category for metrics/logging
    pub fn category(&self) -> &'static str {
        match self {
            Error::Http(_) => "http",
            Error::Json(_) => "json",
            Error::Yaml(_) => "yaml",
            Error::Io(_) => "io",
            Error::Config { .. } => "config",
            Error::Work { .. } => "work",
            Error::Target { .. } => "target",
            Error::Nonce { .. } => "nonce",
            Error::Stratum { .. } => "stratum",
            Error::Worker { .. } => "worker",
            Error::ChainwebNode { .. } => "chainweb_node",
            Error::Crypto { .. } => "crypto",
            Error::ExternalProcess { .. } => "external_process",
            Error::Timeout { .. } => "timeout",
            Error::Network { .. } => "network",
            Error::Generic { .. } => "generic",
            Error::Cancelled { .. } => "cancelled",
            Error::InvalidState { .. } => "invalid_state",
        }
    }
}