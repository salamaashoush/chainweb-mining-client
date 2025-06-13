//! Configuration management for the mining client

use crate::error::{Error, Result};
use crate::workers::WorkerType;
use clap::Parser;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Command-line arguments
#[derive(Parser, Debug)]
#[clap(
    name = "chainweb-mining-client",
    about = "High-performance Chainweb mining client",
    version,
    author
)]
pub struct Args {
    /// Configuration file path
    #[clap(short, long, value_name = "FILE")]
    pub config: Option<PathBuf>,

    /// Node URL
    #[clap(short, long, env = "CHAINWEB_NODE_URL")]
    pub node: Option<String>,

    /// Chain ID to mine on
    #[clap(short = 'i', long, env = "CHAINWEB_CHAIN_ID")]
    pub chain_id: Option<u16>,

    /// Miner account name
    #[clap(short, long, env = "CHAINWEB_ACCOUNT")]
    pub account: Option<String>,

    /// Miner public key
    #[clap(short = 'k', long, env = "CHAINWEB_PUBLIC_KEY")]
    pub public_key: Option<String>,

    /// Worker type
    #[clap(short, long, default_value = "cpu")]
    pub worker: String,

    /// Number of threads (CPU worker)
    #[clap(short, long, default_value = "0")]
    pub threads: usize,

    /// Log level
    #[clap(short, long, default_value = "info")]
    pub log_level: String,

    /// Stratum server port
    #[clap(long, default_value = "3333")]
    pub stratum_port: u16,

    /// External worker command
    #[clap(long)]
    pub external_command: Option<PathBuf>,
}

/// Main configuration structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Node configuration
    pub node: NodeConfig,
    
    /// Mining configuration
    pub mining: MiningConfig,
    
    /// Worker configuration
    pub worker: WorkerConfig,
    
    /// Logging configuration
    pub logging: LoggingConfig,
}

/// Node connection configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeConfig {
    /// Node URL
    pub url: String,
    
    /// Use TLS
    #[serde(default = "default_true")]
    pub use_tls: bool,
    
    /// Request timeout in seconds
    #[serde(default = "default_timeout")]
    pub timeout_secs: u64,
    
    /// Chain ID to mine on
    pub chain_id: u16,
}

/// Mining configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MiningConfig {
    /// Miner account
    pub account: String,
    
    /// Public key
    pub public_key: String,
    
    /// Update interval in seconds
    #[serde(default = "default_update_interval")]
    pub update_interval_secs: u64,
}

/// Worker configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum WorkerConfig {
    /// CPU worker configuration
    #[serde(rename = "cpu")]
    Cpu {
        /// Number of threads (0 = all cores)
        threads: usize,
        /// Batch size for nonce checking
        #[serde(default = "default_batch_size")]
        batch_size: u64,
    },
    
    /// External worker configuration
    #[serde(rename = "external")]
    External {
        /// Command to execute
        command: PathBuf,
        /// Command arguments
        #[serde(default)]
        args: Vec<String>,
        /// Environment variables
        #[serde(default)]
        env: Vec<(String, String)>,
        /// Timeout in seconds
        #[serde(default = "default_external_timeout")]
        timeout_secs: u64,
    },
    
    /// Stratum server configuration
    #[serde(rename = "stratum")]
    Stratum {
        /// Listen port
        #[serde(default = "default_stratum_port")]
        port: u16,
        /// Listen address
        #[serde(default = "default_stratum_host")]
        host: String,
        /// Max connections
        #[serde(default = "default_max_connections")]
        max_connections: usize,
        /// Initial difficulty
        #[serde(default = "default_initial_difficulty")]
        initial_difficulty: f64,
    },
}

/// Logging configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingConfig {
    /// Log level
    #[serde(default = "default_log_level")]
    pub level: String,
    
    /// Log format (plain, json)
    #[serde(default = "default_log_format")]
    pub format: String,
    
    /// Log to file
    pub file: Option<PathBuf>,
}

// Default value functions
fn default_true() -> bool {
    true
}

fn default_timeout() -> u64 {
    30
}

fn default_update_interval() -> u64 {
    5
}

fn default_batch_size() -> u64 {
    100_000
}

fn default_external_timeout() -> u64 {
    60
}

fn default_stratum_port() -> u16 {
    3333
}

fn default_stratum_host() -> String {
    "0.0.0.0".to_string()
}

fn default_max_connections() -> usize {
    100
}

fn default_initial_difficulty() -> f64 {
    1.0
}

fn default_log_level() -> String {
    "info".to_string()
}

fn default_log_format() -> String {
    "plain".to_string()
}

impl Config {
    /// Load configuration from file
    pub fn from_file(path: &PathBuf) -> Result<Self> {
        let contents = std::fs::read_to_string(path)
            .map_err(|e| Error::config(format!("Failed to read config file: {}", e)))?;

        let config: Self = toml::from_str(&contents)
            .map_err(|e| Error::config(format!("Failed to parse config file: {}", e)))?;

        config.validate()?;
        Ok(config)
    }

    /// Create configuration from command-line arguments
    pub fn from_args(args: Args) -> Result<Self> {
        // Load from file if specified
        if let Some(config_path) = &args.config {
            return Self::from_file(config_path);
        }

        // Otherwise build from CLI args
        let node_url = args.node
            .ok_or_else(|| Error::config("Node URL is required"))?;
        
        let chain_id = args.chain_id
            .ok_or_else(|| Error::config("Chain ID is required"))?;
        
        let account = args.account
            .ok_or_else(|| Error::config("Account is required"))?;
        
        let public_key = args.public_key
            .ok_or_else(|| Error::config("Public key is required"))?;

        let use_tls = node_url.starts_with("https://") || !node_url.starts_with("http://");
        let clean_url = node_url
            .trim_start_matches("https://")
            .trim_start_matches("http://")
            .to_string();

        let worker_config = match args.worker.as_str() {
            "cpu" => WorkerConfig::Cpu {
                threads: args.threads,
                batch_size: default_batch_size(),
            },
            "external" => WorkerConfig::External {
                command: args.external_command
                    .ok_or_else(|| Error::config("External command required for external worker"))?,
                args: vec![],
                env: vec![],
                timeout_secs: default_external_timeout(),
            },
            "stratum" => WorkerConfig::Stratum {
                port: args.stratum_port,
                host: default_stratum_host(),
                max_connections: default_max_connections(),
                initial_difficulty: default_initial_difficulty(),
            },
            _ => return Err(Error::config(format!("Unknown worker type: {}", args.worker))),
        };

        let config = Config {
            node: NodeConfig {
                url: clean_url,
                use_tls,
                timeout_secs: default_timeout(),
                chain_id,
            },
            mining: MiningConfig {
                account,
                public_key,
                update_interval_secs: default_update_interval(),
            },
            worker: worker_config,
            logging: LoggingConfig {
                level: args.log_level,
                format: default_log_format(),
                file: None,
            },
        };

        config.validate()?;
        Ok(config)
    }

    /// Validate configuration
    pub fn validate(&self) -> Result<()> {
        // Validate chain ID
        if self.node.chain_id > 19 {
            return Err(Error::config("Chain ID must be between 0 and 19"));
        }

        // Validate worker config
        match &self.worker {
            WorkerConfig::Cpu { batch_size, .. } => {
                if *batch_size == 0 {
                    return Err(Error::config("Batch size must be greater than 0"));
                }
            }
            WorkerConfig::External { command, .. } => {
                if !command.exists() {
                    return Err(Error::config(format!(
                        "External command not found: {:?}",
                        command
                    )));
                }
            }
            WorkerConfig::Stratum { port, .. } => {
                if *port == 0 {
                    return Err(Error::config("Stratum port must be greater than 0"));
                }
            }
        }

        Ok(())
    }

    /// Get the worker type
    pub fn worker_type(&self) -> WorkerType {
        match &self.worker {
            WorkerConfig::Cpu { .. } => WorkerType::Cpu,
            WorkerConfig::External { .. } => WorkerType::External,
            WorkerConfig::Stratum { .. } => WorkerType::Stratum,
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            node: NodeConfig {
                url: "api.chainweb.com".to_string(),
                use_tls: true,
                timeout_secs: 30,
                chain_id: 0,
            },
            mining: MiningConfig {
                account: "miner".to_string(),
                public_key: "".to_string(),
                update_interval_secs: 5,
            },
            worker: WorkerConfig::Cpu {
                threads: 0,
                batch_size: 100_000,
            },
            logging: LoggingConfig {
                level: "info".to_string(),
                format: "plain".to_string(),
                file: None,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert_eq!(config.node.url, "api.chainweb.com");
        assert!(config.node.use_tls);
        assert_eq!(config.node.chain_id, 0);
    }

    #[test]
    fn test_config_validation() {
        let mut config = Config::default();
        
        // Valid config
        assert!(config.validate().is_ok());

        // Invalid chain ID
        config.node.chain_id = 20;
        assert!(config.validate().is_err());
        config.node.chain_id = 0;

        // Invalid batch size
        config.worker = WorkerConfig::Cpu {
            threads: 4,
            batch_size: 0,
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_worker_type() {
        let mut config = Config::default();
        
        config.worker = WorkerConfig::Cpu {
            threads: 4,
            batch_size: 1000,
        };
        assert_eq!(config.worker_type(), WorkerType::Cpu);

        config.worker = WorkerConfig::Stratum {
            port: 3333,
            host: "localhost".to_string(),
            max_connections: 100,
            initial_difficulty: 1.0,
        };
        assert_eq!(config.worker_type(), WorkerType::Stratum);
    }

    #[test]
    fn test_config_serialization() {
        let config = Config::default();
        let toml = toml::to_string(&config).unwrap();
        assert!(toml.contains("[node]"));
        assert!(toml.contains("[mining]"));
        assert!(toml.contains("[worker]"));
        assert!(toml.contains("[logging]"));
    }
}