//! Configuration management for the mining client

use crate::error::{Error, Result};
use crate::protocol::http_pool::get_config_client;
use crate::utils::units;
use crate::workers::WorkerType;
use clap::Parser;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::str::FromStr;

/// Stratum difficulty setting
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum StratumDifficulty {
    /// Use the block difficulty
    Block,
    /// Fixed difficulty (number of leading zeros)
    Fixed(u8),
    /// Dynamic difficulty adjustment based on target period (seconds)
    Period(f64),
}

impl FromStr for StratumDifficulty {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self> {
        match s {
            "block" => Ok(StratumDifficulty::Block),
            n => {
                // Try to parse as integer first (for fixed difficulty)
                if let Ok(difficulty) = n.parse::<u16>() {
                    if difficulty > 256 {
                        return Err(Error::config_invalid_value(
                            "stratum_difficulty",
                            difficulty.to_string(),
                            "value between 0 and 256"
                        ));
                    }
                    Ok(StratumDifficulty::Fixed(difficulty as u8))
                } else if let Ok(period) = n.parse::<f64>() {
                    // Try to parse as float for period-based difficulty
                    if period <= 0.0 {
                        return Err(Error::config_invalid_value(
                            "stratum_difficulty",
                            period.to_string(),
                            "positive number (seconds)"
                        ));
                    }
                    Ok(StratumDifficulty::Period(period))
                } else {
                    Err(Error::config_invalid_value("stratum_difficulty", n, "number, 'block', or period in seconds"))
                }
            }
        }
    }
}

/// Command-line arguments
#[derive(Parser, Debug)]
#[clap(
    name = "chainweb-mining-client",
    about = "Kadena Chainweb Mining Client",
    version,
    author
)]
pub struct Args {
    /// Print program info message and exit
    #[clap(long = "info", help = "Print program info message and exit")]
    pub info: bool,

    /// Print detailed program info message and exit
    #[clap(
        long = "long-info",
        help = "Print detailed program info message and exit"
    )]
    pub long_info: bool,

    /// Print version string and exit
    #[clap(long = "show-version", help = "Print version string and exit")]
    pub show_version: bool,

    /// Print license of the program and exit
    #[clap(long = "license", help = "Print license of the program and exit")]
    pub license: bool,

    /// Show monitoring status and exit
    #[clap(long = "monitoring-status", help = "Show monitoring status and exit")]
    pub monitoring_status: bool,

    /// Print the parsed configuration to standard out and exit
    #[clap(
        long = "print-config-as",
        value_name = "full|minimal|diff",
        help = "Print the parsed configuration to standard out and exit"
    )]
    pub print_config_as: Option<String>,

    /// Print the parsed configuration to standard out and exit. This is an alias for --print-config-as=full
    #[clap(
        long = "print-config",
        help = "Print the parsed configuration to standard out and exit. This is an alias for --print-config-as=full"
    )]
    pub print_config: bool,

    /// Configuration file in YAML or JSON format
    #[clap(
        long = "config-file",
        value_name = "FILE",
        help = "Configuration file in YAML or JSON format. If more than a single config file option is present files are loaded in the order in which they appear on the command line."
    )]
    pub config_file: Vec<PathBuf>,

    /// Hashes per second (only relevant for mining simulation, ignored by the cpu worker)
    #[clap(
        short = 'r',
        long = "hash-rate",
        help = "hashes per second (only relevant for mining simulation, ignored by the cpu worker). Supports unit prefixes: K/M/G/T/P/E/Z/Y (SI) and Ki/Mi/Gi/Ti/Pi/Ei/Zi/Yi (binary). Example: 1M, 2.5G, 100Ki"
    )]
    pub hash_rate: Option<String>,

    /// Node to which to connect
    #[clap(
        short = 'n',
        long = "node",
        value_name = "DOMAIN:PORT",
        help = "node to which to connect"
    )]
    pub node: Option<String>,

    /// Use TLS to connect to node
    #[clap(short = 't', long = "tls", help = "use TLS to connect to node")]
    pub tls: bool,

    /// Unset flag tls
    #[clap(long = "no-tls", help = "unset flag tls")]
    pub no_tls: bool,

    /// Accept self-signed TLS certificates
    #[clap(
        short = 'x',
        long = "insecure",
        help = "accept self-signed TLS certificates"
    )]
    pub insecure: bool,

    /// Unset flag insecure
    #[clap(long = "no-insecure", help = "unset flag insecure")]
    pub no_insecure: bool,

    /// Public-key for the mining rewards account
    #[clap(
        short = 'k',
        long = "public-key",
        help = "public-key for the mining rewards account"
    )]
    pub public_key: Option<String>,

    /// Account for the mining rewards (default: public-key prefixed with 'k:')
    #[clap(
        short = 'a',
        long = "account",
        help = "account for the mining rewards (default: public-key prefixed with 'k:')"
    )]
    pub account: Option<String>,

    /// Number of concurrent mining threads
    #[clap(
        short = 'c',
        long = "thread-count",
        help = "number of concurrent mining threads"
    )]
    pub thread_count: Option<usize>,

    /// Generate a new key pair and exit
    #[clap(long = "generate-key", help = "Generate a new key pair and exit")]
    pub generate_key: bool,

    /// Unset flag generate-key
    #[clap(long = "no-generate-key", help = "unset flag generate-key")]
    pub no_generate_key: bool,

    /// Level at which log messages are written to the console
    #[clap(
        short = 'l',
        long = "log-level",
        value_name = "error|warn|info|debug",
        help = "Level at which log messages are written to the console"
    )]
    pub log_level: Option<String>,

    /// The type of mining worker that is used
    #[clap(
        short = 'w',
        long = "worker",
        value_name = "cpu|external|simulation|stratum|constant-delay|on-demand",
        help = "The type of mining worker that is used"
    )]
    pub worker: Option<String>,

    /// Command that is used to call an external worker
    #[clap(
        long = "external-worker-cmd",
        help = "command that is used to call an external worker. When the command is called the target value is added as last parameter to the command line."
    )]
    pub external_worker_cmd: Option<String>,

    /// The port on which the stratum server listens
    #[clap(
        long = "stratum-port",
        help = "the port on which the stratum server listens"
    )]
    pub stratum_port: Option<u16>,

    /// Network interface that the stratum server binds to
    #[clap(
        long = "stratum-interface",
        help = "network interface that the stratum server binds to"
    )]
    pub stratum_interface: Option<String>,

    /// How the difficulty for stratum mining shares is chosen
    #[clap(
        long = "stratum-difficulty",
        help = "How the difficulty for stratum mining shares is chosen. Possible values are \"block\" for using the block target of the most recent notification of new work, or number between 0 and 256 for specifying a fixed difficulty as logarithm of base 2 (number of leading zeros)."
    )]
    pub stratum_difficulty: Option<String>,

    /// Rate (in milliseconds) at which a stratum worker thread emits jobs
    #[clap(
        short = 's',
        long = "stratum-rate",
        help = "Rate (in milliseconds) at which a stratum worker thread emits jobs."
    )]
    pub stratum_rate: Option<u64>,

    /// Time at which a constant-delay worker emits blocks
    #[clap(
        long = "constant-delay-block-time",
        help = "time at which a constant-delay worker emits blocks"
    )]
    pub constant_delay_block_time: Option<u64>,

    /// Network interface that the on-demand mining server binds to
    #[clap(
        long = "on-demand-interface",
        help = "network interface that the on-demand mining server binds to"
    )]
    pub on_demand_interface: Option<String>,

    /// Port on which the on-demand mining server listens
    #[clap(
        long = "on-demand-port",
        help = "port on which the on-demand mining server listens"
    )]
    pub on_demand_port: Option<u16>,

    /// Default HTTP timeout in microseconds
    #[clap(
        long = "default-http-timeout",
        help = "default HTTP timeout in microseconds"
    )]
    pub default_http_timeout: Option<u64>,
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

/// Flat configuration structure (Haskell-compatible)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlatConfig {
    /// Chainweb node URL
    #[serde(rename = "node")]
    pub node: Option<String>,
    /// Use TLS for node connection
    #[serde(rename = "useTls")]
    pub use_tls: Option<bool>,
    /// Allow insecure TLS connections
    #[serde(rename = "insecure")]
    pub insecure: Option<bool>,
    /// Mining public key
    #[serde(rename = "publicKey")]
    pub public_key: Option<String>,
    /// Mining account
    #[serde(rename = "account")]
    pub account: Option<String>,
    /// Number of mining threads
    #[serde(rename = "threadCount")]
    pub thread_count: Option<usize>,
    /// Log level (debug, info, warn, error)
    #[serde(rename = "logLevel")]
    pub log_level: Option<String>,
    /// Worker type
    #[serde(rename = "worker")]
    pub worker: Option<String>,
    /// External worker command
    #[serde(rename = "externalWorkerCommand")]
    pub external_worker_command: Option<String>,
    /// Stratum server port
    #[serde(rename = "stratumPort")]
    pub stratum_port: Option<u16>,
    /// Stratum server interface
    #[serde(rename = "stratumInterface")]
    pub stratum_interface: Option<String>,
    /// Stratum difficulty setting
    #[serde(rename = "stratumDifficulty")]
    pub stratum_difficulty: Option<String>,
    /// Stratum job rate
    #[serde(rename = "stratumRate")]
    pub stratum_rate: Option<u64>,
    /// Simulated hash rate
    #[serde(rename = "hashRate")]
    pub hash_rate: Option<f64>,
    /// Constant delay block time in seconds
    #[serde(rename = "constantDelayBlockTime")]
    pub constant_delay_block_time: Option<u64>,
    /// On-demand server interface
    #[serde(rename = "onDemandInterface")]
    pub on_demand_interface: Option<String>,
    /// On-demand server port
    #[serde(rename = "onDemandPort")]
    pub on_demand_port: Option<u16>,
    /// Default HTTP timeout in milliseconds
    #[serde(rename = "defaultHTTPTimeout")]
    pub default_http_timeout: Option<u64>,
}

/// Node connection configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeConfig {
    /// Node URL
    pub url: String,

    /// Use TLS
    #[serde(default = "default_true")]
    pub use_tls: bool,

    /// Allow insecure TLS connections
    #[serde(default)]
    pub insecure: bool,

    /// Request timeout in seconds
    #[serde(default = "default_timeout")]
    pub timeout_secs: u64,

    /// Chain ID to mine on (optional, will use all chains if not specified)
    pub chain_id: Option<u16>,
}

impl NodeConfig {
    /// Merge another node config into this one
    fn merge(&mut self, other: NodeConfig) {
        // Determine if we should update TLS before consuming the URL
        let should_update_tls = other.use_tls == other.url.starts_with("http://");

        // URL: only override if it's not the default localhost value
        if other.url != "localhost:1848" {
            self.url = other.url;
        }

        // TLS settings: only override if they differ from defaults
        if should_update_tls {
            self.use_tls = other.use_tls;
        }
        if other.insecure {
            self.insecure = other.insecure;
        }

        // Timeout: use other if it's not the default
        if other.timeout_secs != default_timeout() {
            self.timeout_secs = other.timeout_secs;
        }

        // Chain ID: use other if specified, otherwise keep current
        if other.chain_id.is_some() {
            self.chain_id = other.chain_id;
        }
    }
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

impl MiningConfig {
    /// Merge another mining config into this one
    pub fn merge(&mut self, other: MiningConfig) {
        // Account and public key should generally override (unless empty)
        if !other.account.is_empty() {
            self.account = other.account;
        }
        if !other.public_key.is_empty() {
            self.public_key = other.public_key;
        }

        // Update interval: use other if it's not the default
        if other.update_interval_secs != default_update_interval() {
            self.update_interval_secs = other.update_interval_secs;
        }
    }
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
        command: String,
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
        /// Difficulty setting
        #[serde(default = "default_stratum_difficulty")]
        difficulty: StratumDifficulty,
        /// Job emission rate in milliseconds
        #[serde(default = "default_stratum_rate")]
        rate_ms: u64,
    },

    /// Simulation worker configuration
    #[serde(rename = "simulation")]
    Simulation {
        /// Target hash rate (hashes per second)
        hash_rate: f64,
    },

    /// Constant delay worker configuration
    #[serde(rename = "constant-delay")]
    ConstantDelay {
        /// Block time in seconds
        block_time_secs: u64,
    },

    /// On-demand worker configuration
    #[serde(rename = "on-demand")]
    OnDemand {
        /// Listen port
        #[serde(default = "default_on_demand_port")]
        port: u16,
        /// Listen address
        #[serde(default = "default_on_demand_host")]
        host: String,
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

impl LoggingConfig {
    /// Merge another logging config into this one
    pub fn merge(&mut self, other: LoggingConfig) {
        // Log level: use other if it's not the default
        if other.level != default_log_level() {
            self.level = other.level;
        }

        // Log format: use other if it's not the default
        if other.format != default_log_format() {
            self.format = other.format;
        }

        // Use other file if specified, otherwise keep current
        if other.file.is_some() {
            self.file = other.file;
        }
    }
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

fn default_stratum_difficulty() -> StratumDifficulty {
    StratumDifficulty::Block
}

fn default_stratum_rate() -> u64 {
    1000
}

fn default_on_demand_port() -> u16 {
    1917
}

fn default_on_demand_host() -> String {
    "0.0.0.0".to_string()
}

fn default_log_level() -> String {
    "info".to_string()
}

fn default_log_format() -> String {
    "plain".to_string()
}

impl Config {
    /// Load configuration from file or URL
    pub fn from_file(path: &PathBuf) -> Result<Self> {
        let path_str = path.to_string_lossy();

        // Check if it's a URL
        if path_str.starts_with("http://") || path_str.starts_with("https://") {
            return Self::from_url(&path_str);
        }

        let contents = std::fs::read_to_string(path)
            .map_err(|e| Error::config(format!("Failed to read config file: {}", e)))?;

        Self::from_contents(&contents, &path_str)
    }

    /// Load configuration from URL (HTTP/HTTPS)
    pub fn from_url(url: &str) -> Result<Self> {
        // Use blocking approach for synchronous interface
        let runtime = tokio::runtime::Runtime::new()
            .map_err(|e| Error::config(format!("Failed to create async runtime: {}", e)))?;

        runtime.block_on(Self::from_url_async(url))
    }

    /// Load configuration from URL asynchronously
    pub async fn from_url_async(url: &str) -> Result<Self> {
        let client = get_config_client()
            .map_err(|e| Error::config(format!("Failed to get HTTP client: {}", e)))?;

        let response =
            client.get(url).send().await.map_err(|e| {
                Error::config(format!("Failed to fetch config from {}: {}", url, e))
            })?;

        if !response.status().is_success() {
            return Err(Error::config(format!(
                "HTTP error {} when fetching config from {}",
                response.status(),
                url
            )));
        }

        let contents = response.text().await.map_err(|e| {
            Error::config(format!("Failed to read response body from {}: {}", url, e))
        })?;

        Self::from_contents(&contents, url)
    }

    /// Parse configuration from contents with format detection
    pub fn from_contents(contents: &str, source: &str) -> Result<Self> {
        // Try to determine format from URL extension or content
        let format = if source.ends_with(".yaml") || source.ends_with(".yml") {
            "yaml"
        } else if source.ends_with(".json") {
            "json"
        } else if source.ends_with(".toml") {
            "toml"
        } else {
            // Auto-detect format based on content
            Self::detect_format(contents)
        };

        let config = match format {
            "yaml" => {
                // Try to parse as nested config first
                match serde_yaml::from_str::<Self>(contents) {
                    Ok(config) => config,
                    Err(_) => {
                        // Try flat config format (Haskell-compatible)
                        let flat: FlatConfig = serde_yaml::from_str(contents).map_err(|e| {
                            Error::config(format!(
                                "Failed to parse YAML config from {}: {}",
                                source, e
                            ))
                        })?;
                        Self::from_flat_config(flat)?
                    }
                }
            }
            "json" => {
                // Try to parse as nested config first
                match serde_json::from_str::<Self>(contents) {
                    Ok(config) => config,
                    Err(_) => {
                        // Try flat config format
                        let flat: FlatConfig = serde_json::from_str(contents).map_err(|e| {
                            Error::config(format!(
                                "Failed to parse JSON config from {}: {}",
                                source, e
                            ))
                        })?;
                        Self::from_flat_config(flat)?
                    }
                }
            }
            "toml" => {
                // Try to parse as nested config first
                match toml::from_str::<Self>(contents) {
                    Ok(config) => config,
                    Err(_) => {
                        // Try flat config format
                        let flat: FlatConfig = toml::from_str(contents).map_err(|e| {
                            Error::config(format!(
                                "Failed to parse TOML config from {}: {}",
                                source, e
                            ))
                        })?;
                        Self::from_flat_config(flat)?
                    }
                }
            }
            _ => {
                return Err(Error::config(format!(
                    "Unknown config format for source: {}",
                    source
                )));
            }
        };

        config.validate()?;
        Ok(config)
    }

    /// Detect configuration format from content
    fn detect_format(contents: &str) -> &'static str {
        let trimmed = contents.trim();

        if trimmed.starts_with('{') {
            "json"
        } else if trimmed.starts_with('[') && trimmed.contains('=') {
            // TOML section headers like [section] followed by key=value
            "toml"
        } else if trimmed.starts_with('[') && !trimmed.contains('=') {
            // JSON arrays start with [ but don't contain =
            "json"
        } else if trimmed.contains('[') && trimmed.contains(']') && trimmed.contains('=') {
            // TOML with sections
            "toml"
        } else if trimmed.contains('=') && trimmed.contains('"') {
            // TOML flat format with key="value" syntax
            "toml"
        } else {
            "yaml"
        }
    }

    /// Convert from flat (Haskell-style) config
    fn from_flat_config(flat: FlatConfig) -> Result<Self> {
        let node_url = flat.node.unwrap_or_else(|| "localhost:1848".to_string());
        let use_tls = flat
            .use_tls
            .unwrap_or_else(|| !node_url.starts_with("http://"));
        let clean_url = node_url
            .trim_start_matches("https://")
            .trim_start_matches("http://")
            .to_string();

        let public_key = flat
            .public_key
            .ok_or_else(|| Error::config("Public key is required"))?;

        let account = flat.account.unwrap_or_else(|| format!("k:{}", public_key));

        let worker_name = flat.worker.as_deref().unwrap_or("stratum");
        let worker_config = match worker_name {
            "cpu" => WorkerConfig::Cpu {
                threads: flat.thread_count.unwrap_or(2),
                batch_size: default_batch_size(),
            },
            "external" => WorkerConfig::External {
                command: flat
                    .external_worker_command
                    .unwrap_or_else(|| "echo 'no external worker' && false".to_string()),
                args: vec![],
                env: vec![],
                timeout_secs: default_external_timeout(),
            },
            "stratum" => WorkerConfig::Stratum {
                port: flat.stratum_port.unwrap_or(1917),
                host: flat.stratum_interface.unwrap_or_else(|| "*".to_string()),
                max_connections: default_max_connections(),
                difficulty: flat
                    .stratum_difficulty
                    .as_deref()
                    .map(StratumDifficulty::from_str)
                    .transpose()?
                    .unwrap_or(StratumDifficulty::Block),
                rate_ms: flat.stratum_rate.unwrap_or(1000),
            },
            "simulation" => WorkerConfig::Simulation {
                hash_rate: flat.hash_rate.unwrap_or(1_000_000.0),
            },
            "constant-delay" => WorkerConfig::ConstantDelay {
                block_time_secs: flat.constant_delay_block_time.unwrap_or(30),
            },
            "on-demand" => WorkerConfig::OnDemand {
                port: flat.on_demand_port.unwrap_or(1917),
                host: flat.on_demand_interface.unwrap_or_else(|| "*".to_string()),
            },
            _ => {
                return Err(Error::config(format!(
                    "Unknown worker type: {}",
                    worker_name
                )));
            }
        };

        Ok(Config {
            node: NodeConfig {
                url: clean_url,
                use_tls,
                insecure: flat.insecure.unwrap_or(false),
                timeout_secs: flat
                    .default_http_timeout
                    .map(|us| us / 1_000_000)
                    .unwrap_or(30),
                chain_id: None,
            },
            mining: MiningConfig {
                account,
                public_key,
                update_interval_secs: default_update_interval(),
            },
            worker: worker_config,
            logging: LoggingConfig {
                level: flat.log_level.unwrap_or_else(|| "info".to_string()),
                format: default_log_format(),
                file: None,
            },
        })
    }

    /// Create configuration from command-line arguments
    pub fn from_args(args: Args) -> Result<Self> {
        // Handle special flags that cause early exit
        if args.generate_key {
            // This should be handled in main.rs
            return Err(Error::config("Key generation should be handled in main"));
        }

        // Load from config files if specified
        if !args.config_file.is_empty() {
            let mut config: Option<Config> = None;
            for path in &args.config_file {
                let file_config = Self::from_file(path)?;
                config = Some(match config {
                    None => file_config,
                    Some(mut base) => {
                        // Merge configurations
                        base.merge(file_config);
                        base
                    }
                });
            }
            if let Some(mut config) = config {
                // Apply CLI overrides
                config.apply_args(&args)?;
                return Ok(config);
            }
        }

        // Build from CLI args
        let node_url = args
            .node
            .ok_or_else(|| Error::config("Node URL is required (use -n or --node)"))?;

        let public_key = args
            .public_key
            .ok_or_else(|| Error::config("Public key is required (use -k or --public-key)"))?;

        // Account defaults to k:<public-key> if not specified
        let account = args.account.unwrap_or_else(|| format!("k:{}", public_key));

        // Determine TLS usage
        let use_tls = if args.no_tls {
            false
        } else if args.tls {
            true
        } else {
            node_url.starts_with("https://") || !node_url.starts_with("http://")
        };

        let insecure = args.insecure && !args.no_insecure;

        let clean_url = node_url
            .trim_start_matches("https://")
            .trim_start_matches("http://")
            .to_string();

        // Parse worker type
        let worker_name = args.worker.as_deref().unwrap_or("stratum");
        let worker_config = match worker_name {
            "cpu" => WorkerConfig::Cpu {
                threads: args.thread_count.unwrap_or(2),
                batch_size: default_batch_size(),
            },
            "external" => WorkerConfig::External {
                command: args.external_worker_cmd.ok_or_else(|| {
                    Error::config(
                        "External command required for external worker (use --external-worker-cmd)",
                    )
                })?,
                args: vec![],
                env: vec![],
                timeout_secs: default_external_timeout(),
            },
            "stratum" => WorkerConfig::Stratum {
                port: args.stratum_port.unwrap_or(1917),
                host: args.stratum_interface.unwrap_or_else(|| "*".to_string()),
                max_connections: default_max_connections(),
                difficulty: args
                    .stratum_difficulty
                    .as_deref()
                    .map(StratumDifficulty::from_str)
                    .transpose()?
                    .unwrap_or(StratumDifficulty::Block),
                rate_ms: args.stratum_rate.unwrap_or(1000),
            },
            "simulation" => {
                let hash_rate = args
                    .hash_rate
                    .as_deref()
                    .map(units::parse_hash_rate)
                    .transpose()?
                    .unwrap_or(1_000_000.0);
                WorkerConfig::Simulation { hash_rate }
            }
            "constant-delay" => WorkerConfig::ConstantDelay {
                block_time_secs: args.constant_delay_block_time.unwrap_or(30),
            },
            "on-demand" => WorkerConfig::OnDemand {
                port: args.on_demand_port.unwrap_or(1917),
                host: args.on_demand_interface.unwrap_or_else(|| "*".to_string()),
            },
            _ => {
                return Err(Error::config(format!(
                    "Unknown worker type: {}",
                    worker_name
                )));
            }
        };

        let config = Config {
            node: NodeConfig {
                url: clean_url,
                use_tls,
                insecure,
                timeout_secs: default_timeout(),
                chain_id: None, // Will mine on all chains by default
            },
            mining: MiningConfig {
                account,
                public_key,
                update_interval_secs: default_update_interval(),
            },
            worker: worker_config,
            logging: LoggingConfig {
                level: args.log_level.unwrap_or_else(|| "info".to_string()),
                format: default_log_format(),
                file: None,
            },
        };

        config.validate()?;
        Ok(config)
    }

    /// Apply command-line arguments to existing config
    fn apply_args(&mut self, args: &Args) -> Result<()> {
        // Override node settings
        if let Some(node) = &args.node {
            let use_tls = if args.no_tls {
                false
            } else if args.tls {
                true
            } else {
                node.starts_with("https://") || !node.starts_with("http://")
            };
            self.node.url = node
                .trim_start_matches("https://")
                .trim_start_matches("http://")
                .to_string();
            self.node.use_tls = use_tls;
        }

        if args.tls && !args.no_tls {
            self.node.use_tls = true;
        } else if args.no_tls {
            self.node.use_tls = false;
        }

        if args.insecure && !args.no_insecure {
            self.node.insecure = true;
        } else if args.no_insecure {
            self.node.insecure = false;
        }

        // Override mining settings
        if let Some(public_key) = &args.public_key {
            self.mining.public_key = public_key.clone();
            // Auto-generate account from public key if not explicitly set
            if args.account.is_none() {
                self.mining.account = format!("k:{}", public_key);
            }
        }
        if let Some(account) = &args.account {
            self.mining.account = account.clone();
        }

        // Override logging
        if let Some(log_level) = &args.log_level {
            self.logging.level = log_level.clone();
        }

        // Override worker config based on worker type
        if let Some(_worker_type) = &args.worker {
            // This would require rebuilding the entire worker config
            // For now, we'll skip this in merge scenarios
        }

        Ok(())
    }

    /// Merge another config into this one
    /// Fields from 'other' will override fields in 'self' where they differ
    pub fn merge(&mut self, other: Config) {
        // Merge node configuration
        self.node.merge(other.node);

        // Merge mining configuration
        self.mining.merge(other.mining);

        // Worker configuration: use the other worker config if it's different
        // This is complex to merge properly, so we replace it entirely
        self.worker = other.worker;

        // Merge logging configuration
        self.logging.merge(other.logging);
    }

    /// Validate configuration
    pub fn validate(&self) -> Result<()> {
        // Validate chain ID if specified
        if let Some(chain_id) = self.node.chain_id {
            if chain_id > 19 {
                return Err(Error::config("Chain ID must be between 0 and 19"));
            }
        }

        // Validate worker config
        match &self.worker {
            WorkerConfig::Cpu { batch_size, .. } => {
                if *batch_size == 0 {
                    return Err(Error::config("Batch size must be greater than 0"));
                }
            }
            WorkerConfig::External { command, .. } => {
                // Command validation would happen at runtime
                if command.is_empty() {
                    return Err(Error::config("External command cannot be empty"));
                }
            }
            WorkerConfig::Stratum { port, .. } => {
                if *port == 0 {
                    return Err(Error::config("Stratum port must be greater than 0"));
                }
            }
            WorkerConfig::Simulation { hash_rate } => {
                if *hash_rate <= 0.0 {
                    return Err(Error::config("Hash rate must be greater than 0"));
                }
            }
            WorkerConfig::ConstantDelay { block_time_secs } => {
                if *block_time_secs == 0 {
                    return Err(Error::config("Block time must be greater than 0"));
                }
            }
            WorkerConfig::OnDemand { port, .. } => {
                if *port == 0 {
                    return Err(Error::config("On-demand port must be greater than 0"));
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
            WorkerConfig::Simulation { .. } => WorkerType::Simulation,
            WorkerConfig::ConstantDelay { .. } => WorkerType::ConstantDelay,
            WorkerConfig::OnDemand { .. } => WorkerType::OnDemand,
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            node: NodeConfig {
                url: "api.chainweb.com".to_string(),
                use_tls: true,
                insecure: false,
                timeout_secs: 30,
                chain_id: Some(0),
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
        assert_eq!(config.node.chain_id, Some(0));
    }

    #[test]
    fn test_config_validation() {
        let mut config = Config::default();

        // Valid config
        assert!(config.validate().is_ok());

        // Invalid chain ID
        config.node.chain_id = Some(20);
        assert!(config.validate().is_err());
        config.node.chain_id = Some(0);

        // Invalid batch size
        config.worker = WorkerConfig::Cpu {
            threads: 4,
            batch_size: 0,
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_worker_type() {
        let config = Config {
            worker: WorkerConfig::Cpu {
                threads: 4,
                batch_size: 1000,
            },
            ..Default::default()
        };
        assert_eq!(config.worker_type(), WorkerType::Cpu);

        let config = Config {
            worker: WorkerConfig::Stratum {
                port: 3333,
                host: "localhost".to_string(),
                max_connections: 100,
                difficulty: StratumDifficulty::Block,
                rate_ms: 1000,
            },
            ..Default::default()
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
