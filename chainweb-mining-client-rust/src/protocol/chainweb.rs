//! Chainweb node communication protocol

use crate::core::{ChainId, Target, Work};
use crate::error::{Error, Result};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use eventsource_stream::Eventsource;
use futures::StreamExt;
use reqwest::Client;
use serde::{Deserialize, Serialize};
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
}

/// Chainweb client for interacting with nodes
#[derive(Clone)]
pub struct ChainwebClient {
    config: ChainwebClientConfig,
    client: Client,
}

/// Work request payload
#[derive(Debug, Serialize)]
struct WorkRequest {
    account: String,
    predicate: String,
    #[serde(rename = "public-keys")]
    public_keys: Vec<String>,
}

/// Work response from node
#[derive(Debug, Deserialize)]
struct WorkResponse {
    #[serde(rename = "work-bytes")]
    work_bytes: String,
    target: String,
}

/// Solution submission payload
#[derive(Debug, Serialize)]
struct SolutionRequest {
    #[serde(rename = "work-bytes")]
    work_bytes: String,
}

/// Node info response
#[derive(Debug, Deserialize)]
pub struct NodeInfo {
    #[serde(rename = "nodeVersion")]
    pub node_version: String,
    #[serde(rename = "nodeApiVersion")]
    pub node_api_version: String,
    #[serde(rename = "nodeChains")]
    pub node_chains: Vec<u16>,
    #[serde(rename = "nodeNumberOfChains")]
    pub node_number_of_chains: u16,
}

impl ChainwebClient {
    /// Create a new Chainweb client
    pub fn new(config: ChainwebClientConfig) -> Result<Self> {
        let client = Client::builder()
            .timeout(config.timeout)
            .build()
            .map_err(|e| Error::network(format!("Failed to create HTTP client: {}", e)))?;

        Ok(Self { config, client })
    }

    /// Get the base URL for the node
    fn base_url(&self) -> String {
        let scheme = if self.config.use_tls { "https" } else { "http" };
        format!("{}://{}", scheme, self.config.node_url)
    }

    /// Get node information
    pub async fn get_node_info(&self) -> Result<NodeInfo> {
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

    /// Get work from the node
    pub async fn get_work(&self) -> Result<(Work, Target)> {
        let url = format!(
            "{}/chainweb/0.0/mainnet01/mining/work?chain={}",
            self.base_url(),
            self.config.chain_id.value()
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
            return Err(Error::protocol(format!(
                "Work request failed: {}",
                response.status()
            )));
        }

        let work_response = response
            .json::<WorkResponse>()
            .await
            .map_err(|e| Error::network(format!("Failed to parse work response: {}", e)))?;

        // Decode work bytes from base64
        let work_bytes = URL_SAFE_NO_PAD
            .decode(&work_response.work_bytes)
            .map_err(|e| Error::protocol(format!("Invalid work bytes: {}", e)))?;

        let work = Work::from_slice(&work_bytes)?;
        let target = Target::from_hex(&work_response.target)?;

        debug!("Received work with target: {}", target);

        Ok((work, target))
    }

    /// Submit a solution to the node
    pub async fn submit_solution(&self, work: &Work) -> Result<()> {
        let url = format!(
            "{}/chainweb/0.0/mainnet01/mining/solved",
            self.base_url()
        );

        // Encode work bytes to base64
        let work_bytes = URL_SAFE_NO_PAD.encode(work.as_bytes());

        let request = SolutionRequest { work_bytes };

        debug!("Submitting solution to: {}", url);

        let response = self
            .client
            .post(&url)
            .json(&request)
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
            "{}/chainweb/0.0/mainnet01/mining/updates",
            self.base_url()
        );

        debug!("Subscribing to updates at: {}", url);

        let response = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| Error::network(format!("Failed to subscribe: {}", e)))?;

        if !response.status().is_success() {
            return Err(Error::protocol(format!(
                "Subscribe request failed: {}",
                response.status()
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
            "nodeChains": [0, 1, 2, 3, 4, 5, 6, 7, 8, 9],
            "nodeNumberOfChains": 10
        }"#;

        let info: NodeInfo = serde_json::from_str(json).unwrap();
        assert_eq!(info.node_version, "2.19");
        assert_eq!(info.node_api_version, "0.0");
        assert_eq!(info.node_chains.len(), 10);
        assert_eq!(info.node_number_of_chains, 10);
    }
}