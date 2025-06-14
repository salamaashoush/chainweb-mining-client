//! Chainweb node communication protocol

use crate::core::{ChainId, Target, Work};
use crate::error::{Error, Result};
use crate::protocol::http_pool::{get_insecure_client, get_mining_client};
use crate::protocol::retry::retry_http;
use eventsource_stream::Eventsource;
use futures::StreamExt;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;
use tracing::{debug, error, info};

/// Chainweb client configuration
#[derive(Debug, Clone)]
pub struct ChainwebClientConfig {
    /// Node URL (e.g., "https://api.chainweb.com")
    pub node_url: String,
    /// Chain ID to mine on
    pub chain_id: ChainId,
    /// Miner account name
    pub account: String,
    /// Miner public key
    pub public_key: String,
    /// Request timeout
    pub timeout: Duration,
    /// Whether to use TLS
    pub use_tls: bool,
    /// Allow insecure TLS connections (self-signed certificates)
    pub insecure: bool,
}

/// Chainweb client for interacting with nodes
#[derive(Clone)]
pub struct ChainwebClient {
    config: ChainwebClientConfig,
    client: Arc<Client>,
    node_version: Option<String>,
}

/// Work request payload
#[derive(Debug, Serialize)]
struct WorkRequest {
    account: String,
    predicate: String,
    #[serde(rename = "public-keys")]
    public_keys: Vec<String>,
}

/// Node info response
#[derive(Debug, Deserialize)]
pub struct NodeInfo {
    /// Node version string
    #[serde(rename = "nodeVersion")]
    pub node_version: String,
    /// Node API version string
    #[serde(rename = "nodeApiVersion")]
    pub node_api_version: String,
    /// List of chain IDs supported by the node (as strings in the response)
    #[serde(rename = "nodeChains", default)]
    pub node_chains: Vec<String>,
    /// Total number of chains
    #[serde(rename = "nodeNumberOfChains", default)]
    pub node_number_of_chains: u16,
}

impl ChainwebClient {
    /// Create a new Chainweb client using the HTTP connection pool
    pub fn new(config: ChainwebClientConfig) -> Result<Self> {
        // Use the appropriate client from the HTTP pool
        let client = if config.insecure {
            get_insecure_client()?
        } else {
            get_mining_client()?
        };

        info!(
            "Created Chainweb client using HTTP connection pool (insecure: {})",
            config.insecure
        );

        Ok(Self {
            config,
            client,
            node_version: None,
        })
    }

    /// Set the node version (should be called after get_node_info)
    pub fn set_node_version(&mut self, version: String) {
        self.node_version = Some(version);
    }

    /// Get the node version, defaulting to "mainnet01" if not set
    fn node_version(&self) -> &str {
        self.node_version.as_deref().unwrap_or("mainnet01")
    }

    /// Get the base URL for the node
    fn base_url(&self) -> String {
        let scheme = if self.config.use_tls { "https" } else { "http" };
        format!("{}://{}", scheme, self.config.node_url)
    }

    /// Get node information with retry logic
    pub async fn get_node_info(&self) -> Result<NodeInfo> {
        retry_http(|| self.get_node_info_once()).await
    }

    /// Get node information (single attempt)
    async fn get_node_info_once(&self) -> Result<NodeInfo> {
        let url = format!("{}/info", self.base_url());

        debug!("Getting node info from: {}", url);

        let response = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| Error::network(format!("Failed to get node info: {}", e)))?;

        if !response.status().is_success() {
            return Err(Error::protocol(format!(
                "Node info request failed: {}",
                response.status()
            )));
        }

        let info = response
            .json::<NodeInfo>()
            .await
            .map_err(|e| Error::network(format!("Failed to parse node info: {}", e)))?;

        info!("Connected to Chainweb node version: {}", info.node_version);

        Ok(info)
    }

    /// Get work from the node with retry logic
    pub async fn get_work(&self) -> Result<(Work, Target)> {
        retry_http(|| self.get_work_once()).await
    }

    /// Get work from the node (single attempt)
    async fn get_work_once(&self) -> Result<(Work, Target)> {
        let url = format!(
            "{}/chainweb/0.0/{}/mining/work",
            self.base_url(),
            self.node_version()
        );

        let request = WorkRequest {
            account: self.config.account.clone(),
            predicate: "keys-all".to_string(),
            public_keys: vec![self.config.public_key.clone()],
        };

        debug!("Requesting work from: {}", url);

        let response = self
            .client
            .get(&url)
            .json(&request)
            .send()
            .await
            .map_err(|e| Error::network(format!("Failed to get work: {}", e)))?;

        if !response.status().is_success() {
            let status = response.status();
            if status == 404 {
                return Err(Error::protocol(format!(
                    "Mining work endpoint not found (404). The node may not have mining enabled. \
                     For development nodes, ensure DISABLE_POW_VALIDATION=1 is set."
                )));
            }
            return Err(Error::protocol(format!("Work request failed: {}", status)));
        }

        // The response is raw binary data: 4 bytes ChainId + 32 bytes Target + 286 bytes Work
        let response_bytes = response
            .bytes()
            .await
            .map_err(|e| Error::network(format!("Failed to read work response: {}", e)))?;

        if response_bytes.len() != 322 {
            return Err(Error::protocol(format!(
                "Invalid work response size: expected 322 bytes, got {}",
                response_bytes.len()
            )));
        }

        // Parse the binary response
        // First 4 bytes: ChainId (little-endian)
        let chain_id_bytes: [u8; 4] = response_bytes[0..4].try_into().unwrap();
        let chain_id = u32::from_le_bytes(chain_id_bytes);

        // Verify the chain ID matches what we expect
        if chain_id != self.config.chain_id.value() as u32 {
            debug!(
                "Received work for chain {}, expected {}",
                chain_id,
                self.config.chain_id.value()
            );
        }

        // Next 32 bytes: Target (little-endian, 256-bit)
        let target = Target::from_bytes_le(&response_bytes[4..36])?;

        // Remaining 286 bytes: Work header
        let work = Work::from_slice(&response_bytes[36..])?;

        debug!(
            "Received work for chain {} with target: {}",
            chain_id, target
        );

        Ok((work, target))
    }

    /// Submit a solution to the node with retry logic
    pub async fn submit_solution(&self, work: &Work) -> Result<()> {
        retry_http(|| self.submit_solution_once(work)).await
    }

    /// Submit a solution to the node (single attempt)
    async fn submit_solution_once(&self, work: &Work) -> Result<()> {
        let url = format!(
            "{}/chainweb/0.0/{}/mining/solved",
            self.base_url(),
            self.node_version()
        );

        debug!("Submitting solution to: {}", url);

        // Submit raw work bytes directly
        let response = self
            .client
            .post(&url)
            .header("Content-Type", "application/octet-stream")
            .body(work.as_bytes().to_vec())
            .send()
            .await
            .map_err(|e| Error::network(format!("Failed to submit solution: {}", e)))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(Error::protocol(format!(
                "Solution submission failed: {} - {}",
                status, body
            )));
        }

        info!("Solution accepted by node!");

        Ok(())
    }

    /// Subscribe to work updates via Server-Sent Events
    pub async fn subscribe_updates(&self) -> Result<impl futures::Stream<Item = Result<()>>> {
        let url = format!(
            "{}/chainweb/0.0/{}/mining/updates",
            self.base_url(),
            self.node_version()
        );

        debug!("Subscribing to updates at: {}", url);

        // Encode chain ID as 4-byte little-endian binary
        let body = (self.config.chain_id.value() as u32).to_le_bytes().to_vec();

        let response = self
            .client
            .get(&url)
            .header("Content-Type", "application/octet-stream")
            .body(body)
            .send()
            .await
            .map_err(|e| Error::network(format!("Failed to subscribe: {}", e)))?;

        if !response.status().is_success() {
            let status = response.status();
            if status == 404 {
                return Err(Error::protocol(format!(
                    "Mining updates endpoint not found (404). The node may not have mining enabled. \
                     For development nodes, ensure DISABLE_POW_VALIDATION=1 is set."
                )));
            }
            return Err(Error::protocol(format!(
                "Subscribe request failed: {}",
                status
            )));
        }

        let stream = response
            .bytes_stream()
            .eventsource()
            .map(|result| match result {
                Ok(event) => {
                    debug!("Received update event: {:?}", event);
                    Ok(())
                }
                Err(e) => {
                    error!("SSE error: {}", e);
                    Err(Error::network(format!("SSE error: {}", e)))
                }
            });

        Ok(stream)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chainweb_client_creation() {
        let config = ChainwebClientConfig {
            node_url: "api.chainweb.com".to_string(),
            chain_id: ChainId::new(0),
            account: "miner".to_string(),
            public_key: "abc123".to_string(),
            timeout: Duration::from_secs(30),
            use_tls: true,
            insecure: false,
        };

        let client = ChainwebClient::new(config).unwrap();
        assert_eq!(client.base_url(), "https://api.chainweb.com");
    }

    #[test]
    fn test_base_url() {
        let config = ChainwebClientConfig {
            node_url: "localhost:1848".to_string(),
            chain_id: ChainId::new(0),
            account: "test".to_string(),
            public_key: "test".to_string(),
            timeout: Duration::from_secs(30),
            use_tls: false,
            insecure: false,
        };

        let client = ChainwebClient::new(config).unwrap();
        assert_eq!(client.base_url(), "http://localhost:1848");
    }

    #[test]
    fn test_work_request_serialization() {
        let request = WorkRequest {
            account: "miner".to_string(),
            predicate: "keys-all".to_string(),
            public_keys: vec!["key1".to_string(), "key2".to_string()],
        };

        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("\"account\":\"miner\""));
        assert!(json.contains("\"predicate\":\"keys-all\""));
        assert!(json.contains("\"public-keys\""));
    }

    #[test]
    fn test_node_info_deserialization() {
        let json = r#"{
            "nodeVersion": "2.19",
            "nodeApiVersion": "0.0",
            "nodeChains": ["0", "1", "2", "3", "4", "5", "6", "7", "8", "9"],
            "nodeNumberOfChains": 10
        }"#;

        let info: NodeInfo = serde_json::from_str(json).unwrap();
        assert_eq!(info.node_version, "2.19");
        assert_eq!(info.node_api_version, "0.0");
        assert_eq!(info.node_chains.len(), 10);
        assert_eq!(info.node_number_of_chains, 10);
    }

    #[test]
    fn test_node_version_handling() {
        let config = ChainwebClientConfig {
            node_url: "api.chainweb.com".to_string(),
            chain_id: ChainId::new(0),
            account: "miner".to_string(),
            public_key: "abc123".to_string(),
            timeout: Duration::from_secs(30),
            use_tls: true,
            insecure: false,
        };

        let mut client = ChainwebClient::new(config).unwrap();

        // Test default version
        assert_eq!(client.node_version(), "mainnet01");

        // Test setting custom version
        client.set_node_version("testnet04".to_string());
        assert_eq!(client.node_version(), "testnet04");
    }
}
