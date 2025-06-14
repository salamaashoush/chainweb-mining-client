//! HTTP retry logic with exponential backoff

use crate::error::{Error, Result};
use std::future::Future;
use std::time::Duration;
use tracing::{debug, warn};

/// Maximum delay between retries (5 seconds, matching Haskell implementation)
const MAX_DELAY: Duration = Duration::from_secs(5);

/// Base delay for exponential backoff (100ms, matching Haskell implementation)
const BASE_DELAY: Duration = Duration::from_millis(100);

/// Maximum number of retry attempts
const MAX_ATTEMPTS: usize = 10;

/// HTTP retry policy for network operations
pub struct RetryPolicy {
    max_attempts: usize,
    base_delay: Duration,
    max_delay: Duration,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_attempts: MAX_ATTEMPTS,
            base_delay: BASE_DELAY,
            max_delay: MAX_DELAY,
        }
    }
}

impl RetryPolicy {
    /// Create a new retry policy with custom parameters
    pub fn new(max_attempts: usize, base_delay: Duration, max_delay: Duration) -> Self {
        Self {
            max_attempts,
            base_delay,
            max_delay,
        }
    }

    /// Create a conservative retry policy for critical operations
    pub fn conservative() -> Self {
        Self {
            max_attempts: 20,
            base_delay: Duration::from_millis(50),
            max_delay: Duration::from_secs(10),
        }
    }

    /// Create an aggressive retry policy for non-critical operations
    pub fn aggressive() -> Self {
        Self {
            max_attempts: 5,
            base_delay: Duration::from_millis(200),
            max_delay: Duration::from_secs(2),
        }
    }

    /// Execute an operation with retry logic
    pub async fn execute<F, Fut, T>(&self, mut operation: F) -> Result<T>
    where
        F: FnMut() -> Fut,
        Fut: Future<Output = Result<T>>,
    {
        let mut delay = self.base_delay;
        
        for attempt in 1..=self.max_attempts {
            match operation().await {
                Ok(result) => {
                    if attempt > 1 {
                        debug!("Operation succeeded on attempt {}", attempt);
                    }
                    return Ok(result);
                }
                Err(e) => {
                    if should_retry(&e) && attempt < self.max_attempts {
                        warn!(
                            "Operation failed on attempt {}: {}. Retrying in {:?}...",
                            attempt, e, delay
                        );
                        
                        // Add jitter to prevent thundering herd
                        let jitter = delay.as_millis() as f64 * 0.1 * (rand::random::<f64>() - 0.5);
                        let actual_delay = delay + Duration::from_millis(jitter.abs() as u64);
                        
                        tokio::time::sleep(actual_delay).await;
                        
                        // Exponential backoff
                        delay = std::cmp::min(delay * 2, self.max_delay);
                    } else {
                        if attempt == self.max_attempts {
                            warn!("Operation failed after {} attempts: {}", attempt, e);
                        } else {
                            debug!("Non-retryable error: {}", e);
                        }
                        return Err(e);
                    }
                }
            }
        }
        
        unreachable!()
    }
}

/// Determine if an error should trigger a retry
/// Based on Haskell implementation's httpRetries function
pub fn should_retry(error: &Error) -> bool {
    let error_msg = error.to_string();
    let lower_msg = error_msg.to_lowercase();
    
    match error {
        Error::Network(_) => {
            // Retry on network-related errors
            lower_msg.contains("timeout") 
                || lower_msg.contains("connection") 
                || lower_msg.contains("server error")
                || lower_msg.contains("502")
                || lower_msg.contains("503")
                || lower_msg.contains("504")
                || lower_msg.contains("520")
                || lower_msg.contains("521")
                || lower_msg.contains("522")
                || lower_msg.contains("523")
                || lower_msg.contains("524")
        }
        Error::Protocol(_) => {
            // Retry on certain protocol errors
            lower_msg.contains("server error")
                || lower_msg.contains("service unavailable")
                || lower_msg.contains("gateway timeout")
                || lower_msg.contains("bad gateway")
        }
        Error::Timeout(_) => {
            // Retry on timeout errors
            true
        }
        Error::Io(_) => {
            // Retry on I/O errors that might be transient
            lower_msg.contains("timeout") 
                || lower_msg.contains("connection")
                || lower_msg.contains("broken pipe")
        }
        Error::ChannelSend(_) | Error::ChannelRecv(_) => {
            // Don't retry on channel errors (usually unrecoverable)
            false
        }
        Error::InvalidWork(_) | Error::InvalidTarget(_) | Error::Config(_) | Error::Json(_) => {
            // Don't retry on validation or configuration errors
            false
        }
        Error::Worker(_) | Error::Stratum(_) | Error::ExternalProcess(_) => {
            // Don't retry on worker-specific errors
            false
        }
        Error::Other(_) => {
            // Retry on generic errors that might be transient (including network errors created via Error::network)
            lower_msg.contains("timeout") 
                || lower_msg.contains("connection")
                || lower_msg.contains("server error")
                || lower_msg.contains("502")
                || lower_msg.contains("503")
                || lower_msg.contains("504")
                || lower_msg.contains("520")
                || lower_msg.contains("521")
                || lower_msg.contains("522")
                || lower_msg.contains("523")
                || lower_msg.contains("524")
        }
    }
}

/// Retry an HTTP operation with default policy
pub async fn retry_http<F, Fut, T>(operation: F) -> Result<T>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = Result<T>>,
{
    RetryPolicy::default().execute(operation).await
}

/// Retry a critical operation with conservative policy
pub async fn retry_critical<F, Fut, T>(operation: F) -> Result<T>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = Result<T>>,
{
    RetryPolicy::conservative().execute(operation).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    #[tokio::test]
    async fn test_retry_success_on_first_attempt() {
        let policy = RetryPolicy::new(3, Duration::from_millis(10), Duration::from_millis(100));
        
        let result = policy.execute(|| async { Ok::<i32, Error>(42) }).await;
        
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 42);
    }

    #[tokio::test]
    async fn test_retry_success_after_failures() {
        let policy = RetryPolicy::new(5, Duration::from_millis(10), Duration::from_millis(100));
        let attempt_count = Arc::new(AtomicUsize::new(0));
        
        let attempt_count_clone = Arc::clone(&attempt_count);
        let result = policy.execute(move || {
            let count = attempt_count_clone.fetch_add(1, Ordering::SeqCst);
            async move {
                if count < 2 {
                    Err(Error::network("Connection timeout"))
                } else {
                    Ok(42)
                }
            }
        }).await;
        
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 42);
        assert_eq!(attempt_count.load(Ordering::SeqCst), 3);
    }

    #[tokio::test]
    async fn test_retry_exhaustion() {
        let policy = RetryPolicy::new(3, Duration::from_millis(10), Duration::from_millis(100));
        
        let result = policy.execute(|| async { 
            Err::<i32, Error>(Error::network("Persistent connection error"))
        }).await;
        
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_non_retryable_error() {
        let policy = RetryPolicy::new(3, Duration::from_millis(10), Duration::from_millis(100));
        
        let result = policy.execute(|| async { 
            Err::<i32, Error>(Error::config("Invalid configuration"))
        }).await;
        
        assert!(result.is_err());
        // Should fail immediately without retries for non-retryable errors
    }

    #[test]
    fn test_should_retry_logic() {
        // Network errors should be retried
        assert!(should_retry(&Error::network("Connection timeout")));
        assert!(should_retry(&Error::network("server error 503")));
        
        // Protocol errors with server issues should be retried
        assert!(should_retry(&Error::protocol("Gateway Timeout")));
        
        // Configuration errors should not be retried
        assert!(!should_retry(&Error::config("Invalid key")));
        
        // Validation errors should not be retried
        assert!(!should_retry(&Error::invalid_work("Invalid work format")));
    }
}
