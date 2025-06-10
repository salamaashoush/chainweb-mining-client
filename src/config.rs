//! Configuration management for chainweb mining client
//!
//! Supports configuration via command line arguments, environment variables,
//! and configuration files (YAML/JSON) with proper validation and defaults.

use crate::{Error, Result, HashRate, MinerAccount, MinerPublicKey};
use clap::{Parser, ValueEnum};
use serde::{Deserialize, Serialize};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::PathBuf;
use std::str::FromStr;
use std::time::Duration;
use url::Url;

/// Worker types supported by the mining client
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum WorkerType {
    /// Multi-threaded CPU mining
    Cpu,
    /// External worker command (e.g., GPU miner)
    External,
    /// Stratum server for ASIC miners
    Stratum,
    /// Simulated mining for testing
    Simulation,
    /// Constant delay mining for testing
    ConstantDelay,
    /// On-demand mining via HTTP API
    OnDemand,
}

impl fmt::Display for WorkerType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            WorkerType::Cpu => write!(f, "cpu"),
            WorkerType::External => write!(f, "external"),
            WorkerType::Stratum => write!(f, "stratum"),
            WorkerType::Simulation => write!(f, "simulation"),
            WorkerType::ConstantDelay => write!(f, "constant-delay"),
            WorkerType::OnDemand => write!(f, "on-demand"),
        }
    }
}

/// Log levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LogLevel {
    Error,
    Warn,
    Info,
    Debug,
    Trace,
}

impl From<LogLevel> for tracing::Level {
    fn from(level: LogLevel) -> Self {
        match level {
            LogLevel::Error => tracing::Level::ERROR,
            LogLevel::Warn => tracing::Level::WARN,
            LogLevel::Info => tracing::Level::INFO,
            LogLevel::Debug => tracing::Level::DEBUG,
            LogLevel::Trace => tracing::Level::TRACE,
        }
    }
}

/// Stratum difficulty configuration
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum StratumDifficulty {
    /// Use block difficulty
    Block,
    /// Fixed difficulty level (0-256, log2 of leading zeros)
    Fixed(u8),
}

impl FromStr for StratumDifficulty {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self> {
        if s.eq_ignore_ascii_case("block") {
            Ok(StratumDifficulty::Block)
        } else {
            let level: u8 = s.parse()
                .map_err(|e| Error::config(format!("Invalid stratum difficulty: {}", e)))?;
            if level > 256 {
                return Err(Error::config("Stratum difficulty level must be 0-256"));
            }
            Ok(StratumDifficulty::Fixed(level))
        }
    }
}

/// Complete configuration for the mining client
#[derive(Debug, Clone, Parser, Serialize, Deserialize)]
#[command(
    name = "chainweb-mining-client",
    version = env!("CARGO_PKG_VERSION"),
    about = "Kadena Chainweb Mining Client",
    long_about = "A high-performance mining client for Kadena's Chainweb blockchain supporting ASIC, CPU, and testing modes"
)]
pub struct Config {
    /// Print program info and exit
    #[arg(long)]
    pub info: bool,

    /// Print detailed program info and exit
    #[arg(long)]
    pub long_info: bool,

    /// Generate a new key pair and exit
    #[arg(long)]
    pub generate_key: bool,

    /// Print the parsed configuration and exit
    #[arg(long)]
    pub print_config: bool,

    /// Configuration file path (YAML or JSON)
    #[arg(long, value_name = "FILE")]
    pub config_file: Option<PathBuf>,

    /// Hash rate for simulation mode
    #[arg(short = 'r', long, default_value = "1000000")]
    #[serde(default = "default_hash_rate")]
    pub hash_rate: String,

    /// Chainweb node address
    #[arg(short = 'n', long, default_value = "localhost:1848")]
    #[serde(default = "default_node")]
    pub node: String,

    /// Use TLS to connect to node
    #[arg(short = 't', long)]
    #[serde(default)]
    pub tls: bool,

    /// Accept self-signed TLS certificates
    #[arg(short = 'x', long)]
    #[serde(default)]
    pub insecure: bool,

    /// Public key for mining rewards account
    #[arg(short = 'k', long)]
    pub public_key: Option<String>,

    /// Account name for mining rewards (default: k:{public_key})
    #[arg(short = 'a', long)]
    pub account: Option<String>,

    /// Number of concurrent mining threads
    #[arg(short = 'c', long, default_value = "2")]
    #[serde(default = "default_thread_count")]
    pub thread_count: usize,

    /// Log level
    #[arg(short = 'l', long, default_value = "info")]
    #[serde(default = "default_log_level")]
    pub log_level: LogLevel,

    /// Mining worker type
    #[arg(short = 'w', long, default_value = "stratum")]
    #[serde(default = "default_worker")]
    pub worker: WorkerType,

    /// External worker command
    #[arg(long, default_value = "echo 'no external worker command configured' && exit 1")]
    #[serde(default = "default_external_cmd")]
    pub external_worker_cmd: String,

    /// Stratum server port
    #[arg(long, default_value = "1917")]
    #[serde(default = "default_stratum_port")]
    pub stratum_port: u16,

    /// Stratum server interface
    #[arg(long, default_value = "0.0.0.0")]
    #[serde(default = "default_stratum_interface")]
    pub stratum_interface: String,

    /// Stratum mining difficulty
    #[arg(long, default_value = "block")]
    #[serde(default = "default_stratum_difficulty")]
    pub stratum_difficulty: String,

    /// Stratum job rate in milliseconds
    #[arg(short = 's', long, default_value = "1000")]
    #[serde(default = "default_stratum_rate")]
    pub stratum_rate: u64,

    /// Constant delay block time in seconds
    #[arg(long, default_value = "30")]
    #[serde(default = "default_constant_delay")]
    pub constant_delay_block_time: u64,

    /// On-demand server interface
    #[arg(long, default_value = "0.0.0.0")]
    #[serde(default = "default_on_demand_interface")]
    pub on_demand_interface: String,

    /// On-demand server port
    #[arg(long, default_value = "1917")]
    #[serde(default = "default_on_demand_port")]
    pub on_demand_port: u16,

    /// Default HTTP timeout in milliseconds
    #[arg(long, default_value = "30000")]
    #[serde(default = "default_http_timeout")]
    pub http_timeout: u64,

    /// Update stream timeout in seconds
    #[arg(long, default_value = "150")]
    #[serde(default = "default_update_timeout")]
    pub update_timeout: u64,

    /// Maximum retry attempts for HTTP requests
    #[arg(long, default_value = "10")]
    #[serde(default = "default_max_retries")]
    pub max_retries: usize,

    /// Base retry delay in milliseconds
    #[arg(long, default_value = "100")]
    #[serde(default = "default_retry_delay")]
    pub retry_delay: u64,

    /// Maximum retry delay in milliseconds
    #[arg(long, default_value = "5000")]
    #[serde(default = "default_max_retry_delay")]
    pub max_retry_delay: u64,
}

impl Config {
    /// Load configuration from file if specified
    pub async fn load() -> Result<Self> {
        let mut config = Self::parse();

        // Load from config file if specified
        if let Some(config_file) = &config.config_file {
            let file_config = Self::load_from_file(config_file).await?;
            config = config.merge_with_file(file_config)?;
        }

        config.validate()?;
        Ok(config)
    }

    /// Load configuration from file
    async fn load_from_file(path: &PathBuf) -> Result<Self> {
        let content = tokio::fs::read_to_string(path).await?;
        
        if path.extension().and_then(|s| s.to_str()) == Some("json") {
            serde_json::from_str(&content).map_err(Error::from)
        } else {
            // Default to YAML
            serde_yaml::from_str(&content).map_err(Error::from)
        }
    }

    /// Merge CLI config with file config (CLI takes precedence)
    fn merge_with_file(mut self, file_config: Self) -> Result<Self> {
        // Only override CLI defaults with file values if not explicitly set
        
        if self.public_key.is_none() {
            self.public_key = file_config.public_key;
        }
        
        if self.account.is_none() {
            self.account = file_config.account;
        }

        // For other fields, keep CLI values (they include defaults)
        Ok(self)
    }

    /// Validate configuration
    pub fn validate(&self) -> Result<()> {
        // Validate public key if provided
        if let Some(key) = &self.public_key {
            MinerPublicKey::new(key.clone())?;
        }

        // Validate node URL
        let node_url = if self.tls {
            format!("https://{}", self.node)
        } else {
            format!("http://{}", self.node)
        };
        Url::parse(&node_url)
            .map_err(|e| Error::config(format!("Invalid node URL: {}", e)))?;

        // Validate stratum difficulty
        StratumDifficulty::from_str(&self.stratum_difficulty)?;

        // Validate hash rate
        HashRate::from_str_with_units(&self.hash_rate)?;

        // Validate thread count
        if self.thread_count == 0 {
            return Err(Error::config("Thread count must be greater than 0"));
        }

        // Validate stratum interface
        if !self.stratum_interface.parse::<IpAddr>().is_ok() {
            return Err(Error::config("Invalid stratum interface address"));
        }

        // Validate on-demand interface
        if !self.on_demand_interface.parse::<IpAddr>().is_ok() {
            return Err(Error::config("Invalid on-demand interface address"));
        }

        Ok(())
    }

    /// Get parsed hash rate
    pub fn hash_rate(&self) -> Result<HashRate> {
        HashRate::from_str_with_units(&self.hash_rate)
    }

    /// Get parsed stratum difficulty
    pub fn stratum_difficulty(&self) -> Result<StratumDifficulty> {
        StratumDifficulty::from_str(&self.stratum_difficulty)
    }

    /// Get node URL
    pub fn node_url(&self) -> String {
        if self.tls {
            format!("https://{}", self.node)
        } else {
            format!("http://{}", self.node)
        }
    }

    /// Get stratum socket address
    pub fn stratum_socket_addr(&self) -> Result<SocketAddr> {
        let ip: IpAddr = self.stratum_interface.parse()
            .map_err(|e| Error::config(format!("Invalid stratum interface: {}", e)))?;
        Ok(SocketAddr::new(ip, self.stratum_port))
    }

    /// Get on-demand socket address
    pub fn on_demand_socket_addr(&self) -> Result<SocketAddr> {
        let ip: IpAddr = self.on_demand_interface.parse()
            .map_err(|e| Error::config(format!("Invalid on-demand interface: {}", e)))?;
        Ok(SocketAddr::new(ip, self.on_demand_port))
    }

    /// Get miner configuration
    pub fn miner(&self) -> Result<Option<(MinerPublicKey, Option<MinerAccount>)>> {
        if let Some(key_str) = &self.public_key {
            let public_key = MinerPublicKey::new(key_str.clone())?;
            let account = self.account.as_ref().map(|a| MinerAccount::new(a.clone()));
            Ok(Some((public_key, account)))
        } else {
            Ok(None)
        }
    }

    /// Get HTTP timeout duration
    pub fn http_timeout_duration(&self) -> Duration {
        Duration::from_millis(self.http_timeout)
    }

    /// Get update timeout duration
    pub fn update_timeout_duration(&self) -> Duration {
        Duration::from_secs(self.update_timeout)
    }

    /// Get retry delay duration
    pub fn retry_delay_duration(&self) -> Duration {
        Duration::from_millis(self.retry_delay)
    }

    /// Get max retry delay duration
    pub fn max_retry_delay_duration(&self) -> Duration {
        Duration::from_millis(self.max_retry_delay)
    }

    /// Get stratum rate duration
    pub fn stratum_rate_duration(&self) -> Duration {
        Duration::from_millis(self.stratum_rate)
    }

    /// Get constant delay duration
    pub fn constant_delay_duration(&self) -> Duration {
        Duration::from_secs(self.constant_delay_block_time)
    }
}

// Default value functions for serde
fn default_hash_rate() -> String { "1000000".to_string() }
fn default_node() -> String { "localhost:1848".to_string() }
fn default_thread_count() -> usize { 2 }
fn default_log_level() -> LogLevel { LogLevel::Info }
fn default_worker() -> WorkerType { WorkerType::Stratum }
fn default_external_cmd() -> String { "echo 'no external worker command configured' && exit 1".to_string() }
fn default_stratum_port() -> u16 { 1917 }
fn default_stratum_interface() -> String { "0.0.0.0".to_string() }
fn default_stratum_difficulty() -> String { "block".to_string() }
fn default_stratum_rate() -> u64 { 1000 }
fn default_constant_delay() -> u64 { 30 }
fn default_on_demand_interface() -> String { "0.0.0.0".to_string() }
fn default_on_demand_port() -> u16 { 1917 }
fn default_http_timeout() -> u64 { 30000 }
fn default_update_timeout() -> u64 { 150 }
fn default_max_retries() -> usize { 10 }
fn default_retry_delay() -> u64 { 100 }
fn default_max_retry_delay() -> u64 { 5000 }

use std::fmt;

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;
    use std::io::Write;

    #[tokio::test]
    async fn test_config_defaults() {
        let args = vec!["chainweb-mining-client"];
        let config = Config::try_parse_from(args).unwrap();
        
        assert_eq!(config.worker, WorkerType::Stratum);
        assert_eq!(config.thread_count, 2);
        assert_eq!(config.log_level, LogLevel::Info);
        assert!(!config.tls);
        assert!(!config.insecure);
    }

    #[tokio::test]
    async fn test_config_from_yaml() {
        let yaml_content = r#"
public_key: "87ef8fdb229ad10285ae191a168ea2ec0794621a127df21e372f41fd0246e4cf"
node: "example.com:1848"
tls: true
worker: cpu
thread_count: 4
"#;

        let mut temp_file = NamedTempFile::new().unwrap();
        write!(temp_file, "{}", yaml_content).unwrap();
        
        let config = Config::load_from_file(&temp_file.path().to_path_buf()).await.unwrap();
        
        assert_eq!(config.public_key.unwrap(), "87ef8fdb229ad10285ae191a168ea2ec0794621a127df21e372f41fd0246e4cf");
        assert_eq!(config.node, "example.com:1848");
        assert!(config.tls);
        assert_eq!(config.worker, WorkerType::Cpu);
        assert_eq!(config.thread_count, 4);
    }

    #[test]
    fn test_stratum_difficulty_parsing() {
        assert!(matches!(StratumDifficulty::from_str("block").unwrap(), StratumDifficulty::Block));
        assert!(matches!(StratumDifficulty::from_str("50").unwrap(), StratumDifficulty::Fixed(50)));
        assert!(StratumDifficulty::from_str("300").is_err());
    }

    #[test]
    fn test_worker_type_display() {
        assert_eq!(WorkerType::Cpu.to_string(), "cpu");
        assert_eq!(WorkerType::Stratum.to_string(), "stratum");
        assert_eq!(WorkerType::ConstantDelay.to_string(), "constant-delay");
    }
}