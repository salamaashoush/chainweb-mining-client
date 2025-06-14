//! Error types for the mining client
//!
//! This module provides a comprehensive error handling system using `thiserror`
//! for automatic error trait implementations with granular error categorization.

use thiserror::Error;
use std::time::Duration;

/// Configuration error subtypes
#[derive(Error, Debug)]
#[allow(missing_docs)]
pub enum ConfigError {
    #[error("Missing required field: {field}")]
    MissingField { field: String },
    
    #[error("Invalid value for {field}: {value} (expected: {expected})")]
    InvalidValue { field: String, value: String, expected: String },
    
    #[error("File not found: {path}")]
    FileNotFound { path: String },
    
    #[error("Parse error in {file}: {message}")]
    ParseError { file: String, message: String },
    
    #[error("Validation failed: {message}")]
    ValidationError { message: String },
    
    #[error("Environment variable error: {var} - {message}")]
    EnvironmentError { var: String, message: String },
}

/// Network error subtypes with detailed context
#[derive(Error, Debug)]
#[allow(missing_docs)]
pub enum NetworkError {
    #[error("Connection failed to {url}: {source}")]
    ConnectionFailed { url: String, source: Box<dyn std::error::Error + Send + Sync> },
    
    #[error("Request timeout after {timeout:?} to {url}")]
    Timeout { url: String, timeout: Duration },
    
    #[error("HTTP error {status} from {url}: {message}")]
    HttpError { url: String, status: u16, message: String },
    
    #[error("DNS resolution failed for {host}: {message}")]
    DnsError { host: String, message: String },
    
    #[error("TLS/SSL error connecting to {url}: {message}")]
    TlsError { url: String, message: String },
    
    #[error("Invalid URL: {url}")]
    InvalidUrl { url: String },
    
    #[error("Network unreachable: {message}")]
    NetworkUnreachable { message: String },
    
    #[error("Connection reset by peer: {url}")]
    ConnectionReset { url: String },
}

/// Protocol error subtypes for Chainweb and Stratum
#[derive(Error, Debug)]
#[allow(missing_docs)]
pub enum ProtocolError {
    #[error("Invalid message format: {message}")]
    InvalidFormat { message: String },
    
    #[error("Protocol version mismatch: expected {expected}, got {actual}")]
    VersionMismatch { expected: String, actual: String },
    
    #[error("Authentication failed: {reason}")]
    AuthenticationFailed { reason: String },
    
    #[error("Subscription failed for {topic}: {reason}")]
    SubscriptionFailed { topic: String, reason: String },
    
    #[error("Response parse error: {field} - {message}")]
    ResponseParseError { field: String, message: String },
    
    #[error("Invalid chain ID: expected {expected}, got {actual}")]
    InvalidChainId { expected: u32, actual: u32 },
    
    #[error("Work validation failed: {reason}")]
    WorkValidationFailed { reason: String },
    
    #[error("Target validation failed: {reason}")]
    TargetValidationFailed { reason: String },
    
    #[error("Mining endpoint not available: {endpoint}")]
    EndpointUnavailable { endpoint: String },
}

/// Worker error subtypes with detailed context
#[derive(Error, Debug)]
#[allow(missing_docs)]
pub enum WorkerError {
    #[error("Worker initialization failed: {worker_type} - {reason}")]
    InitializationFailed { worker_type: String, reason: String },
    
    #[error("Worker startup failed: {worker_type} - {reason}")]
    StartupFailed { worker_type: String, reason: String },
    
    #[error("Worker shutdown failed: {worker_type} - {reason}")]
    ShutdownFailed { worker_type: String, reason: String },
    
    #[error("Mining operation failed: {reason}")]
    MiningFailed { reason: String },
    
    #[error("Hash computation error: {algorithm} - {reason}")]
    HashComputationError { algorithm: String, reason: String },
    
    #[error("Thread pool error: {reason}")]
    ThreadPoolError { reason: String },
    
    #[error("Resource exhaustion: {resource} - {details}")]
    ResourceExhaustion { resource: String, details: String },
    
    #[error("Worker communication error: {reason}")]
    CommunicationError { reason: String },
    
    #[error("External worker process error: {command} - {reason}")]
    ExternalProcessError { command: String, reason: String },
    
    #[error("Work preemption failed: {reason}")]
    PreemptionFailed { reason: String },
}

/// Stratum-specific error subtypes
#[derive(Error, Debug)]
#[allow(missing_docs)]
pub enum StratumError {
    #[error("Client connection failed: {client_id} - {reason}")]
    ClientConnectionFailed { client_id: String, reason: String },
    
    #[error("Invalid client message: {client_id} - {message}")]
    InvalidClientMessage { client_id: String, message: String },
    
    #[error("Subscription error: {client_id} - {method} - {reason}")]
    SubscriptionError { client_id: String, method: String, reason: String },
    
    #[error("Job dispatch failed: {job_id} - {reason}")]
    JobDispatchFailed { job_id: String, reason: String },
    
    #[error("Difficulty adjustment failed: {current} -> {target} - {reason}")]
    DifficultyAdjustmentFailed { current: f64, target: f64, reason: String },
    
    #[error("Share validation failed: {client_id} - {share_id} - {reason}")]
    ShareValidationFailed { client_id: String, share_id: String, reason: String },
    
    #[error("Server binding failed: {address} - {reason}")]
    ServerBindingFailed { address: String, reason: String },
    
    #[error("Client limit exceeded: {current}/{max}")]
    ClientLimitExceeded { current: usize, max: usize },
}

/// Data validation error subtypes
#[derive(Error, Debug)]
#[allow(missing_docs)]
pub enum ValidationError {
    #[error("Invalid work header: expected {expected_size} bytes, got {actual_size}")]
    InvalidWorkHeader { expected_size: usize, actual_size: usize },
    
    #[error("Invalid target format: {value} - {reason}")]
    InvalidTarget { value: String, reason: String },
    
    #[error("Invalid nonce: {nonce} - {reason}")]
    InvalidNonce { nonce: u64, reason: String },
    
    #[error("Invalid hash: {hash} - {reason}")]
    InvalidHash { hash: String, reason: String },
    
    #[error("Checksum mismatch: expected {expected}, got {actual}")]
    ChecksumMismatch { expected: String, actual: String },
    
    #[error("Size validation failed: {field} - expected {expected}, got {actual}")]
    SizeValidation { field: String, expected: usize, actual: usize },
    
    #[error("Range validation failed: {field} - value {value} outside range [{min}, {max}]")]
    RangeValidation { field: String, value: i64, min: i64, max: i64 },
}

/// Communication error subtypes
#[derive(Error, Debug)]
#[allow(missing_docs)]
pub enum CommunicationError {
    #[error("Channel send failed: {channel} - {reason}")]
    ChannelSendFailed { channel: String, reason: String },
    
    #[error("Channel receive failed: {channel} - {reason}")]
    ChannelReceiveFailed { channel: String, reason: String },
    
    #[error("Channel closed unexpectedly: {channel}")]
    ChannelClosed { channel: String },
    
    #[error("Message serialization failed: {message_type} - {reason}")]
    SerializationFailed { message_type: String, reason: String },
    
    #[error("Message deserialization failed: {message_type} - {reason}")]
    DeserializationFailed { message_type: String, reason: String },
    
    #[error("Broadcast failed: {recipients} recipients - {reason}")]
    BroadcastFailed { recipients: usize, reason: String },
}

/// Main error type for the mining client with granular error hierarchy
#[derive(Error, Debug)]
#[allow(missing_docs)]
pub enum Error {
    /// Configuration errors with detailed context
    #[error("Configuration error: {0}")]
    Config(#[from] ConfigError),

    /// Network errors with connection details
    #[error("Network error: {0}")]
    Network(#[from] NetworkError),

    /// Protocol errors for Chainweb and Stratum
    #[error("Protocol error: {0}")]
    Protocol(#[from] ProtocolError),

    /// Worker operation errors
    #[error("Worker error: {0}")]
    Worker(#[from] WorkerError),

    /// Stratum protocol specific errors
    #[error("Stratum error: {0}")]
    Stratum(#[from] StratumError),

    /// Data validation errors
    #[error("Validation error: {0}")]
    Validation(#[from] ValidationError),

    /// Communication errors (channels, serialization)
    #[error("Communication error: {0}")]
    Communication(#[from] CommunicationError),

    /// JSON parsing errors
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    /// YAML parsing errors
    #[error("YAML error: {0}")]
    Yaml(#[from] serde_yaml::Error),

    /// I/O errors
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// HTTP client errors (kept for automatic conversion)
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    /// Timeout errors with context
    #[error("Operation timed out after {timeout:?}: {operation}")]
    Timeout { operation: String, timeout: Duration },

    /// External process errors
    #[error("External process error: {command} - {reason}")]
    ExternalProcess { command: String, reason: String },

    /// Generic errors with context
    #[error("Error in {context}: {message}")]
    Other { context: String, message: String },
}

/// Result type alias for the mining client
pub type Result<T> = std::result::Result<T, Error>;

impl Error {
    // === Configuration Error Constructors ===
    
    /// Create a missing field configuration error
    pub fn config_missing_field(field: impl Into<String>) -> Self {
        Self::Config(ConfigError::MissingField { field: field.into() })
    }
    
    /// Create an invalid value configuration error
    pub fn config_invalid_value(field: impl Into<String>, value: impl Into<String>, expected: impl Into<String>) -> Self {
        Self::Config(ConfigError::InvalidValue {
            field: field.into(),
            value: value.into(),
            expected: expected.into(),
        })
    }
    
    /// Create a file not found configuration error
    pub fn config_file_not_found(path: impl Into<String>) -> Self {
        Self::Config(ConfigError::FileNotFound { path: path.into() })
    }
    
    /// Create a parse error configuration error
    pub fn config_parse_error(file: impl Into<String>, message: impl Into<String>) -> Self {
        Self::Config(ConfigError::ParseError {
            file: file.into(),
            message: message.into(),
        })
    }
    
    /// Create a generic configuration error
    pub fn config(msg: impl Into<String>) -> Self {
        Self::Config(ConfigError::ValidationError { message: msg.into() })
    }
    
    // === Network Error Constructors ===
    
    /// Create a connection failed network error
    pub fn network_connection_failed(url: impl Into<String>, source: Box<dyn std::error::Error + Send + Sync>) -> Self {
        Self::Network(NetworkError::ConnectionFailed { url: url.into(), source })
    }
    
    /// Create a timeout network error
    pub fn network_timeout(url: impl Into<String>, timeout: Duration) -> Self {
        Self::Network(NetworkError::Timeout { url: url.into(), timeout })
    }
    
    /// Create an HTTP error with detailed context
    pub fn network_http_error(url: impl Into<String>, status: u16, message: impl Into<String>) -> Self {
        Self::Network(NetworkError::HttpError {
            url: url.into(),
            status,
            message: message.into(),
        })
    }
    
    /// Create a generic network error
    pub fn network(msg: impl Into<String>) -> Self {
        Self::Network(NetworkError::NetworkUnreachable { message: msg.into() })
    }
    
    // === Protocol Error Constructors ===
    
    /// Create an invalid format protocol error
    pub fn protocol_invalid_format(message: impl Into<String>) -> Self {
        Self::Protocol(ProtocolError::InvalidFormat { message: message.into() })
    }
    
    /// Create a version mismatch protocol error
    pub fn protocol_version_mismatch(expected: impl Into<String>, actual: impl Into<String>) -> Self {
        Self::Protocol(ProtocolError::VersionMismatch {
            expected: expected.into(),
            actual: actual.into(),
        })
    }
    
    /// Create an invalid chain ID protocol error
    pub fn protocol_invalid_chain_id(expected: u32, actual: u32) -> Self {
        Self::Protocol(ProtocolError::InvalidChainId { expected, actual })
    }
    
    /// Create a work validation protocol error
    pub fn protocol_work_validation_failed(reason: impl Into<String>) -> Self {
        Self::Protocol(ProtocolError::WorkValidationFailed { reason: reason.into() })
    }
    
    /// Create an endpoint unavailable protocol error
    pub fn protocol_endpoint_unavailable(endpoint: impl Into<String>) -> Self {
        Self::Protocol(ProtocolError::EndpointUnavailable { endpoint: endpoint.into() })
    }
    
    /// Create a generic protocol error
    pub fn protocol(msg: impl Into<String>) -> Self {
        Self::Protocol(ProtocolError::InvalidFormat { message: msg.into() })
    }
    
    // === Worker Error Constructors ===
    
    /// Create a worker initialization error
    pub fn worker_initialization_failed(worker_type: impl Into<String>, reason: impl Into<String>) -> Self {
        Self::Worker(WorkerError::InitializationFailed {
            worker_type: worker_type.into(),
            reason: reason.into(),
        })
    }
    
    /// Create a mining operation failed error
    pub fn worker_mining_failed(reason: impl Into<String>) -> Self {
        Self::Worker(WorkerError::MiningFailed { reason: reason.into() })
    }
    
    /// Create a hash computation error
    pub fn worker_hash_computation_error(algorithm: impl Into<String>, reason: impl Into<String>) -> Self {
        Self::Worker(WorkerError::HashComputationError {
            algorithm: algorithm.into(),
            reason: reason.into(),
        })
    }
    
    /// Create a generic worker error
    pub fn worker(msg: impl Into<String>) -> Self {
        Self::Worker(WorkerError::MiningFailed { reason: msg.into() })
    }
    
    // === Stratum Error Constructors ===
    
    /// Create a client connection failed stratum error
    pub fn stratum_client_connection_failed(client_id: impl Into<String>, reason: impl Into<String>) -> Self {
        Self::Stratum(StratumError::ClientConnectionFailed {
            client_id: client_id.into(),
            reason: reason.into(),
        })
    }
    
    /// Create a share validation failed stratum error
    pub fn stratum_share_validation_failed(client_id: impl Into<String>, share_id: impl Into<String>, reason: impl Into<String>) -> Self {
        Self::Stratum(StratumError::ShareValidationFailed {
            client_id: client_id.into(),
            share_id: share_id.into(),
            reason: reason.into(),
        })
    }
    
    /// Create a generic stratum error
    pub fn stratum(msg: impl Into<String>) -> Self {
        Self::Stratum(StratumError::InvalidClientMessage {
            client_id: "unknown".to_string(),
            message: msg.into(),
        })
    }
    
    // === Validation Error Constructors ===
    
    /// Create an invalid work header validation error
    pub fn validation_invalid_work_header(expected_size: usize, actual_size: usize) -> Self {
        Self::Validation(ValidationError::InvalidWorkHeader { expected_size, actual_size })
    }
    
    /// Create an invalid target validation error
    pub fn validation_invalid_target(value: impl Into<String>, reason: impl Into<String>) -> Self {
        Self::Validation(ValidationError::InvalidTarget {
            value: value.into(),
            reason: reason.into(),
        })
    }
    
    /// Create a size validation error
    pub fn validation_size_error(field: impl Into<String>, expected: usize, actual: usize) -> Self {
        Self::Validation(ValidationError::SizeValidation {
            field: field.into(),
            expected,
            actual,
        })
    }
    
    // === Communication Error Constructors ===
    
    /// Create a channel send failed communication error
    pub fn communication_channel_send_failed(channel: impl Into<String>, reason: impl Into<String>) -> Self {
        Self::Communication(CommunicationError::ChannelSendFailed {
            channel: channel.into(),
            reason: reason.into(),
        })
    }
    
    /// Create a channel receive failed communication error
    pub fn communication_channel_receive_failed(channel: impl Into<String>, reason: impl Into<String>) -> Self {
        Self::Communication(CommunicationError::ChannelReceiveFailed {
            channel: channel.into(),
            reason: reason.into(),
        })
    }
    
    // === Legacy Compatibility Methods ===
    
    /// Create an invalid work error (legacy compatibility)
    pub fn invalid_work(msg: impl Into<String>) -> Self {
        Self::validation_invalid_target(msg.into(), "legacy invalid work error")
    }

    /// Create an invalid target error (legacy compatibility)
    pub fn invalid_target(msg: impl Into<String>) -> Self {
        Self::validation_invalid_target(msg.into(), "legacy invalid target error")
    }

    /// Create an external process error (legacy compatibility)
    pub fn external_process(msg: impl Into<String>) -> Self {
        Self::ExternalProcess {
            command: "unknown".to_string(),
            reason: msg.into(),
        }
    }

    /// Create a timeout error (legacy compatibility)
    pub fn timeout(msg: impl Into<String>) -> Self {
        Self::Timeout {
            operation: msg.into(),
            timeout: Duration::from_secs(30),
        }
    }

    /// Create a channel send error (legacy compatibility)
    pub fn channel_send(msg: impl Into<String>) -> Self {
        Self::communication_channel_send_failed("unknown", msg)
    }

    /// Create a channel receive error (legacy compatibility)
    pub fn channel_recv(msg: impl Into<String>) -> Self {
        Self::communication_channel_receive_failed("unknown", msg)
    }

    /// Create a generic error (legacy compatibility)
    pub fn other(msg: impl Into<String>) -> Self {
        Self::Other {
            context: "unknown".to_string(),
            message: msg.into(),
        }
    }

    /// Create a JSON error (legacy compatibility)
    pub fn json(err: serde_json::Error) -> Self {
        Self::Json(err)
    }
    
    // === Error Analysis Methods ===
    
    /// Check if this is a recoverable error
    pub fn is_recoverable(&self) -> bool {
        matches!(
            self,
            Error::Network(NetworkError::Timeout { .. })
                | Error::Network(NetworkError::ConnectionFailed { .. })
                | Error::Network(NetworkError::ConnectionReset { .. })
                | Error::Communication(CommunicationError::ChannelClosed { .. })
                | Error::Worker(WorkerError::CommunicationError { .. })
                | Error::Protocol(ProtocolError::SubscriptionFailed { .. })
        )
    }
    
    /// Check if this error should trigger a retry
    pub fn should_retry(&self) -> bool {
        matches!(
            self,
            Error::Network(NetworkError::Timeout { .. })
                | Error::Network(NetworkError::ConnectionReset { .. })
                | Error::Network(NetworkError::NetworkUnreachable { .. })
                | Error::Protocol(ProtocolError::EndpointUnavailable { .. })
        )
    }
    
    /// Get the error category for metrics and logging
    pub fn category(&self) -> &'static str {
        match self {
            Error::Config(_) => "configuration",
            Error::Network(_) => "network",
            Error::Protocol(_) => "protocol",
            Error::Worker(_) => "worker",
            Error::Stratum(_) => "stratum",
            Error::Validation(_) => "validation",
            Error::Communication(_) => "communication",
            Error::Json(_) | Error::Yaml(_) => "serialization",
            Error::Io(_) => "io",
            Error::Http(_) => "http",
            Error::Timeout { .. } => "timeout",
            Error::ExternalProcess { .. } => "external_process",
            Error::Other { .. } => "other",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let err = Error::config("missing field");
        assert!(err.to_string().contains("Validation failed: missing field"));

        let err = Error::worker("worker failed");
        assert!(err.to_string().contains("Mining operation failed: worker failed"));
    }

    #[test]
    fn test_granular_error_display() {
        let err = Error::config_missing_field("timeout");
        assert!(err.to_string().contains("Missing required field: timeout"));

        let err = Error::network_timeout("http://example.com", Duration::from_secs(30));
        assert!(err.to_string().contains("Request timeout after 30s to http://example.com"));

        let err = Error::protocol_invalid_chain_id(0, 5);
        assert!(err.to_string().contains("Invalid chain ID: expected 0, got 5"));

        let err = Error::validation_invalid_work_header(286, 300);
        assert!(err.to_string().contains("Invalid work header: expected 286 bytes, got 300"));
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

    #[test]
    fn test_error_analysis() {
        let recoverable_err = Error::network_timeout("http://example.com", Duration::from_secs(30));
        assert!(recoverable_err.is_recoverable());
        assert!(recoverable_err.should_retry());

        let non_recoverable_err = Error::config_missing_field("account");
        assert!(!non_recoverable_err.is_recoverable());
        assert!(!non_recoverable_err.should_retry());
    }

    #[test]
    fn test_error_categories() {
        assert_eq!(Error::config("test").category(), "configuration");
        assert_eq!(Error::network("test").category(), "network");
        assert_eq!(Error::protocol("test").category(), "protocol");
        assert_eq!(Error::worker("test").category(), "worker");
        assert_eq!(Error::stratum("test").category(), "stratum");
    }

    #[test]
    fn test_specific_error_constructors() {
        let err = Error::worker_initialization_failed("CPU", "thread pool creation failed");
        assert!(err.to_string().contains("Worker initialization failed: CPU"));
        assert!(err.to_string().contains("thread pool creation failed"));

        let err = Error::stratum_share_validation_failed("client123", "share456", "invalid nonce");
        assert!(err.to_string().contains("Share validation failed: client123"));
        assert!(err.to_string().contains("share456"));
        assert!(err.to_string().contains("invalid nonce"));
    }

    #[test]
    fn test_legacy_compatibility() {
        // Ensure legacy methods still work
        let _err = Error::invalid_work("test");
        let _err = Error::invalid_target("test");
        let _err = Error::external_process("test");
        let _err = Error::timeout("test");
        let _err = Error::channel_send("test");
        let _err = Error::channel_recv("test");
        let _err = Error::other("test");
    }

    #[test]
    fn test_network_error_hierarchy() {
        let connection_err = NetworkError::ConnectionFailed {
            url: "http://example.com".to_string(),
            source: Box::new(std::io::Error::new(std::io::ErrorKind::ConnectionRefused, "refused")),
        };
        let err = Error::Network(connection_err);
        assert!(err.is_recoverable());
        assert_eq!(err.category(), "network");
    }

    #[test]
    fn test_worker_error_hierarchy() {
        let worker_err = WorkerError::HashComputationError {
            algorithm: "Blake2s".to_string(),
            reason: "invalid input length".to_string(),
        };
        let err = Error::Worker(worker_err);
        assert!(!err.is_recoverable());
        assert_eq!(err.category(), "worker");
    }

    #[test]
    fn test_stratum_error_hierarchy() {
        let stratum_err = StratumError::ClientLimitExceeded {
            current: 100,
            max: 50,
        };
        let err = Error::Stratum(stratum_err);
        assert!(err.to_string().contains("Client limit exceeded: 100/50"));
        assert_eq!(err.category(), "stratum");
    }

    #[test]
    fn test_validation_error_hierarchy() {
        let validation_err = ValidationError::RangeValidation {
            field: "difficulty".to_string(),
            value: -1,
            min: 0,
            max: 1000,
        };
        let err = Error::Validation(validation_err);
        assert!(err.to_string().contains("Range validation failed: difficulty"));
        assert!(err.to_string().contains("value -1 outside range [0, 1000]"));
        assert_eq!(err.category(), "validation");
    }
}
