//! Structured logging with context tags
//!
//! This module provides enhanced logging capabilities with structured context tags
//! for better debugging and monitoring of mining operations.

use std::collections::HashMap;
use tracing::{Level, Span, field, span};
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{EnvFilter, fmt};

/// Context tags for structured logging
#[derive(Debug, Clone)]
pub struct LogContext {
    /// Worker type (cpu, stratum, external, etc.)
    pub worker_type: Option<String>,
    /// Chain ID being mined
    pub chain_id: Option<u16>,
    /// Current mining target
    pub target: Option<String>,
    /// Mining account
    pub account: Option<String>,
    /// Session ID for Stratum connections
    pub session_id: Option<String>,
    /// Additional custom fields
    pub custom_fields: HashMap<String, String>,
}

impl LogContext {
    /// Create a new empty context
    pub fn new() -> Self {
        Self {
            worker_type: None,
            chain_id: None,
            target: None,
            account: None,
            session_id: None,
            custom_fields: HashMap::new(),
        }
    }

    /// Create a context for a specific worker
    pub fn for_worker(worker_type: &str) -> Self {
        Self {
            worker_type: Some(worker_type.to_string()),
            ..Self::new()
        }
    }

    /// Add chain ID to context
    pub fn with_chain_id(mut self, chain_id: u16) -> Self {
        self.chain_id = Some(chain_id);
        self
    }

    /// Add target to context
    pub fn with_target(mut self, target: &str) -> Self {
        self.target = Some(target.to_string());
        self
    }

    /// Add account to context
    pub fn with_account(mut self, account: &str) -> Self {
        self.account = Some(account.to_string());
        self
    }

    /// Add session ID to context
    pub fn with_session_id(mut self, session_id: &str) -> Self {
        self.session_id = Some(session_id.to_string());
        self
    }

    /// Add custom field
    pub fn with_field(mut self, key: String, value: String) -> Self {
        self.custom_fields.insert(key, value);
        self
    }

    /// Create a tracing span with this context
    pub fn span(&self, name: &str) -> Span {
        let span = span!(
            Level::INFO,
            "context",
            name = %name,
            worker_type = field::Empty,
            chain_id = field::Empty,
            target = field::Empty,
            account = field::Empty,
            session_id = field::Empty
        );

        if let Some(ref worker_type) = self.worker_type {
            span.record("worker_type", &field::display(worker_type));
        }
        if let Some(chain_id) = self.chain_id {
            span.record("chain_id", &field::display(chain_id));
        }
        if let Some(ref target) = self.target {
            span.record("target", &field::display(target));
        }
        if let Some(ref account) = self.account {
            span.record("account", &field::display(account));
        }
        if let Some(ref session_id) = self.session_id {
            span.record("session_id", &field::display(session_id));
        }

        span
    }
}

impl Default for LogContext {
    fn default() -> Self {
        Self::new()
    }
}

/// Enhanced logging initialization with structured context support
pub fn init_structured_logging(level: &str, format: &str, include_target: bool) {
    let env_filter = EnvFilter::try_new(level).unwrap_or_else(|_| EnvFilter::new("info"));

    match format {
        "json" => {
            let fmt_layer = fmt::layer()
                .json()
                .with_target(include_target)
                .with_thread_ids(true)
                .with_thread_names(true)
                .with_file(true)
                .with_line_number(true);

            tracing_subscriber::registry()
                .with(env_filter)
                .with(fmt_layer)
                .init();
        }
        "pretty" => {
            let fmt_layer = fmt::layer()
                .pretty()
                .with_target(include_target)
                .with_thread_ids(true)
                .with_thread_names(true)
                .with_file(true)
                .with_line_number(true);

            tracing_subscriber::registry()
                .with(env_filter)
                .with(fmt_layer)
                .init();
        }
        _ => {
            let fmt_layer = fmt::layer()
                .with_target(include_target)
                .with_thread_ids(false)
                .with_thread_names(false)
                .with_file(false)
                .with_line_number(false);

            tracing_subscriber::registry()
                .with(env_filter)
                .with(fmt_layer)
                .init();
        }
    }
}

/// Mining-specific structured logging macros
#[macro_export]
macro_rules! log_mining_event {
    ($level:expr, $ctx:expr, $msg:expr $(, $key:expr => $value:expr)*) => {{
        let span = $ctx.span("mining_event");
        let _enter = span.enter();
        match $level {
            tracing::Level::ERROR => tracing::error!($msg $(, $key = %$value)*),
            tracing::Level::WARN => tracing::warn!($msg $(, $key = %$value)*),
            tracing::Level::INFO => tracing::info!($msg $(, $key = %$value)*),
            tracing::Level::DEBUG => tracing::debug!($msg $(, $key = %$value)*),
            tracing::Level::TRACE => tracing::trace!($msg $(, $key = %$value)*),
        }
    }};
}

/// Log a mining result with context
#[macro_export]
macro_rules! log_mining_result {
    ($ctx:expr, $work:expr, $nonce:expr, $hash:expr) => {{
        let span = $ctx.span("mining_result");
        let _enter = span.enter();
        tracing::info!(
            "Found solution",
            work = %hex::encode($work.as_bytes()),
            nonce = %$nonce,
            hash = %hex::encode($hash)
        );
    }};
}

/// Log a Stratum connection event
#[macro_export]
macro_rules! log_stratum_event {
    ($level:expr, $session_id:expr, $event:expr $(, $key:expr => $value:expr)*) => {{
        let span = tracing::span!(
            tracing::Level::INFO,
            "stratum_event",
            session_id = %$session_id
        );
        let _enter = span.enter();
        match $level {
            tracing::Level::ERROR => tracing::error!($event $(, $key = %$value)*),
            tracing::Level::WARN => tracing::warn!($event $(, $key = %$value)*),
            tracing::Level::INFO => tracing::info!($event $(, $key = %$value)*),
            tracing::Level::DEBUG => tracing::debug!($event $(, $key = %$value)*),
            tracing::Level::TRACE => tracing::trace!($event $(, $key = %$value)*),
        }
    }};
}

/// Log a worker state change
#[macro_export]
macro_rules! log_worker_state {
    ($worker_type:expr, $old_state:expr, $new_state:expr $(, $key:expr => $value:expr)*) => {{
        let span = tracing::span!(
            tracing::Level::INFO,
            "worker_state_change",
            worker_type = %$worker_type
        );
        let _enter = span.enter();
        tracing::info!(
            "Worker state changed",
            old_state = %$old_state,
            new_state = %$new_state
            $(, $key = %$value)*
        );
    }};
}

/// Performance logging for mining operations
pub struct MiningMetrics {
    start_time: std::time::Instant,
    hashes: u64,
    solutions: u64,
    rejects: u64,
    context: LogContext,
}

impl MiningMetrics {
    /// Create new mining metrics tracker
    pub fn new(context: LogContext) -> Self {
        Self {
            start_time: std::time::Instant::now(),
            hashes: 0,
            solutions: 0,
            rejects: 0,
            context,
        }
    }

    /// Record hashes computed
    pub fn record_hashes(&mut self, count: u64) {
        self.hashes += count;
    }

    /// Record a solution found
    pub fn record_solution(&mut self) {
        self.solutions += 1;
    }

    /// Record a rejected solution
    pub fn record_reject(&mut self) {
        self.rejects += 1;
    }

    /// Log current metrics
    pub fn log_metrics(&self) {
        let elapsed = self.start_time.elapsed();
        let hash_rate = if elapsed.as_secs() > 0 {
            self.hashes / elapsed.as_secs()
        } else {
            0
        };

        let span = self.context.span("mining_metrics");
        let _enter = span.enter();

        tracing::info!(
            duration_secs = elapsed.as_secs(),
            total_hashes = self.hashes,
            hash_rate = hash_rate,
            solutions = self.solutions,
            rejects = self.rejects,
            accept_rate = if self.solutions + self.rejects > 0 {
                (self.solutions as f64 / (self.solutions + self.rejects) as f64) * 100.0
            } else {
                0.0
            },
            "Mining metrics"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_log_context_creation() {
        let ctx = LogContext::for_worker("cpu")
            .with_chain_id(0)
            .with_target("0000000000000000000000000000000000000000000000000000000000000001")
            .with_account("k:1234567890abcdef")
            .with_field("custom".to_string(), "value".to_string());

        assert_eq!(ctx.worker_type, Some("cpu".to_string()));
        assert_eq!(ctx.chain_id, Some(0));
        assert_eq!(ctx.custom_fields.get("custom"), Some(&"value".to_string()));
    }

    #[test]
    fn test_mining_metrics() {
        let ctx = LogContext::for_worker("test");
        let mut metrics = MiningMetrics::new(ctx);

        metrics.record_hashes(1000);
        metrics.record_solution();
        metrics.record_hashes(500);
        metrics.record_reject();

        assert_eq!(metrics.hashes, 1500);
        assert_eq!(metrics.solutions, 1);
        assert_eq!(metrics.rejects, 1);
    }
}
