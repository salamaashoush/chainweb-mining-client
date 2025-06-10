//! Chainweb node client for mining operations
//!
//! Handles communication with chainweb nodes including work requests,
//! solution submission, and real-time update streams.

use crate::{ChainId, Error, Miner, Nonce, Result, Target, Work};
use futures::Stream;
use reqwest::{Client, ClientBuilder, Response};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::Duration;
use tokio::time::{sleep, timeout, Instant};
use tracing::{debug, error, info, instrument, warn};
use url::Url;

/// Chainweb version identifier
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChainwebVersion(pub String);

/// Node information response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeInfo {
    #[serde(rename = "nodeVersion")]
    pub node_version: ChainwebVersion,
    #[serde(rename = "nodeNumberOfChains")]
    pub number_of_chains: u32,
    #[serde(rename = "nodeApiVersion")]
    pub api_version: String,
    #[serde(flatten)]
    pub other: HashMap<String, serde_json::Value>,
}

/// Mining work job from chainweb node
#[derive(Debug, Clone)]
pub struct MiningJob {
    pub chain_id: ChainId,
    pub target: Target,
    pub work: Work,
}

/// Update stream event types
#[derive(Debug, Clone)]
pub enum UpdateEvent {
    /// New work available for chain
    NewWork(ChainId),
    /// Stream connection closed
    Closed,
    /// Stream error occurred
    Error(String),
}

/// Exponential backoff configuration
#[derive(Debug, Clone)]
pub struct BackoffConfig {
    pub initial_delay: Duration,
    pub max_delay: Duration,
    pub multiplier: f64,
    pub max_retries: usize,
}

impl Default for BackoffConfig {
    fn default() -> Self {
        Self {
            initial_delay: Duration::from_millis(100),
            max_delay: Duration::from_secs(5),
            multiplier: 2.0,
            max_retries: 10,
        }
    }
}

/// Chainweb node client
pub struct ChainwebClient {
    client: Client,
    base_url: Url,
    version: Option<ChainwebVersion>,
    backoff_config: BackoffConfig,
}

impl ChainwebClient {
    /// Create a new chainweb client
    pub fn new(
        base_url: impl AsRef<str>,
        timeout: Duration,
        insecure: bool,
    ) -> Result<Self> {
        let base_url = Url::parse(base_url.as_ref())
            .map_err(|e| Error::config(format!("Invalid base URL: {}", e)))?;

        let client = ClientBuilder::new()
            .timeout(timeout)
            .danger_accept_invalid_certs(insecure)
            .build()
            .map_err(Error::from)?;

        Ok(Self {
            client,
            base_url,
            version: None,
            backoff_config: BackoffConfig::default(),
        })
    }

    /// Set custom backoff configuration
    pub fn with_backoff_config(mut self, config: BackoffConfig) -> Self {
        self.backoff_config = config;
        self
    }

    /// Get node information
    #[instrument(skip(self))]
    pub async fn get_node_info(&mut self) -> Result<NodeInfo> {
        let url = self.base_url.join("info")
            .map_err(|e| Error::network(format!("Failed to build info URL: {}", e)))?;

        debug!("Fetching node info from: {}", url);

        let response = self.get_with_retry(&url).await?;
        let info: NodeInfo = response.json().await
            .map_err(|e| Error::chainweb_node(format!("Failed to parse node info: {}", e)))?;

        info!("Connected to chainweb node version: {}", info.node_version.0);
        self.version = Some(info.node_version.clone());

        Ok(info)
    }

    /// Get mining job from the node
    #[instrument(skip(self, miner))]
    pub async fn get_mining_job(&self, miner: &Miner) -> Result<MiningJob> {
        let version = self.version.as_ref()
            .ok_or_else(|| Error::invalid_state("Node version not fetched yet"))?;

        let url = self.base_url
            .join(&format!("chainweb/0.0/{}/mining/work", version.0))
            .map_err(|e| Error::network(format!("Failed to build work URL: {}", e)))?;

        debug!("Requesting mining job from: {}", url);

        let miner_data = serde_json::json!({
            "account": miner.account_name(),
            "public-keys": [miner.public_key.as_str()],
            "predicate": "keys-all"
        });

        let response = self.client
            .post(url)
            .json(&miner_data)
            .send()
            .await
            .map_err(Error::from)?;

        if !response.status().is_success() {
            return Err(Error::chainweb_node(format!(
                "Failed to get work: HTTP {}",
                response.status()
            )));
        }

        let bytes = response.bytes().await
            .map_err(|e| Error::chainweb_node(format!("Failed to read work response: {}", e)))?;

        // Parse the binary response according to chainweb mining API format
        self.parse_mining_job(&bytes)
    }

    /// Submit solved work to the node
    #[instrument(skip(self, work))]
    pub async fn submit_work(&self, work: &Work) -> Result<()> {
        let version = self.version.as_ref()
            .ok_or_else(|| Error::invalid_state("Node version not fetched yet"))?;

        let url = self.base_url
            .join(&format!("chainweb/0.0/{}/mining/solved", version.0))
            .map_err(|e| Error::network(format!("Failed to build solved URL: {}", e)))?;

        debug!("Submitting solved work to: {}", url);

        let response = self.post_with_retry(&url, work.bytes()).await?;

        if response.status().is_success() {
            info!("Successfully submitted solved work");
            Ok(())
        } else {
            warn!("Failed to submit work: HTTP {}", response.status());
            Err(Error::chainweb_node(format!(
                "Failed to submit work: HTTP {}",
                response.status()
            )))
        }
    }

    /// Create update stream for a specific chain
    #[instrument(skip(self))]
    pub async fn update_stream(&self, chain_id: ChainId) -> Result<Pin<Box<dyn Stream<Item = UpdateEvent> + Send>>> {
        let version = self.version.as_ref()
            .ok_or_else(|| Error::invalid_state("Node version not fetched yet"))?;

        let url = self.base_url
            .join(&format!("chainweb/0.0/{}/mining/updates", version.0))
            .map_err(|e| Error::network(format!("Failed to build updates URL: {}", e)))?;

        debug!("Creating update stream for chain {} at: {}", chain_id, url);

        let chain_bytes = chain_id.to_bytes();
        
        let stream = UpdateStream::new(
            self.client.clone(),
            url,
            chain_bytes.to_vec(),
            chain_id,
        );

        Ok(Box::pin(stream))
    }

    /// Parse binary mining job response
    fn parse_mining_job(&self, bytes: &[u8]) -> Result<MiningJob> {
        if bytes.len() < 4 + 32 + Work::SIZE {
            return Err(Error::work(format!(
                "Invalid work response length: expected at least {}, got {}",
                4 + 32 + Work::SIZE,
                bytes.len()
            )));
        }

        let mut offset = 0;

        // Parse chain ID (4 bytes, little-endian)
        let chain_id = ChainId::from_bytes(&bytes[offset..offset + 4])?;
        offset += 4;

        // Parse target (32 bytes)
        let target = Target::from_bytes(&bytes[offset..offset + 32])?;
        offset += 32;

        // Parse work (286 bytes)
        let work_bytes = bytes[offset..offset + Work::SIZE].to_vec();
        let work = Work::new(work_bytes)?;

        debug!(
            "Parsed mining job: chain={}, target={}, difficulty_level={}",
            chain_id,
            target,
            target.difficulty_level()
        );

        Ok(MiningJob {
            chain_id,
            target,
            work,
        })
    }

    /// GET request with exponential backoff retry
    async fn get_with_retry(&self, url: &Url) -> Result<Response> {
        let mut delay = self.backoff_config.initial_delay;
        let mut attempts = 0;

        loop {
            match self.client.get(url.clone()).send().await {
                Ok(response) => {
                    if response.status().is_success() || !response.status().is_server_error() {
                        return Ok(response);
                    }
                    
                    if attempts >= self.backoff_config.max_retries {
                        return Err(Error::http(reqwest::Error::from(response.error_for_status().unwrap_err())));
                    }
                }
                Err(e) => {
                    if !e.is_timeout() && !e.is_connect() || attempts >= self.backoff_config.max_retries {
                        return Err(Error::from(e));
                    }
                }
            }

            warn!("Request failed, retrying in {:?} (attempt {}/{})", delay, attempts + 1, self.backoff_config.max_retries);
            sleep(delay).await;
            
            delay = Duration::from_millis(
                ((delay.as_millis() as f64) * self.backoff_config.multiplier) as u64
            ).min(self.backoff_config.max_delay);
            
            attempts += 1;
        }
    }

    /// POST request with exponential backoff retry
    async fn post_with_retry(&self, url: &Url, body: &[u8]) -> Result<Response> {
        let mut delay = self.backoff_config.initial_delay;
        let mut attempts = 0;

        loop {
            match self.client.post(url.clone()).body(body.to_vec()).send().await {
                Ok(response) => {
                    if response.status().is_success() || !response.status().is_server_error() {
                        return Ok(response);
                    }
                    
                    if attempts >= self.backoff_config.max_retries {
                        return Err(Error::http(reqwest::Error::from(response.error_for_status().unwrap_err())));
                    }
                }
                Err(e) => {
                    if !e.is_timeout() && !e.is_connect() || attempts >= self.backoff_config.max_retries {
                        return Err(Error::from(e));
                    }
                }
            }

            warn!("POST request failed, retrying in {:?} (attempt {}/{})", delay, attempts + 1, self.backoff_config.max_retries);
            sleep(delay).await;
            
            delay = Duration::from_millis(
                ((delay.as_millis() as f64) * self.backoff_config.multiplier) as u64
            ).min(self.backoff_config.max_delay);
            
            attempts += 1;
        }
    }
}

/// Server-sent events update stream
struct UpdateStream {
    client: Client,
    url: Url,
    chain_bytes: Vec<u8>,
    chain_id: ChainId,
    response: Option<Pin<Box<dyn Stream<Item = Result<bytes::Bytes, reqwest::Error>> + Send>>>,
    reconnect_delay: Duration,
    last_reconnect: Instant,
}

impl UpdateStream {
    fn new(client: Client, url: Url, chain_bytes: Vec<u8>, chain_id: ChainId) -> Self {
        Self {
            client,
            url,
            chain_bytes,
            chain_id,
            response: None,
            reconnect_delay: Duration::from_secs(1),
            last_reconnect: Instant::now(),
        }
    }

    async fn connect(&mut self) -> Result<()> {
        debug!("Connecting to update stream for chain {}", self.chain_id);

        let response = self.client
            .post(self.url.clone())
            .body(self.chain_bytes.clone())
            .send()
            .await
            .map_err(Error::from)?;

        if !response.status().is_success() {
            return Err(Error::network(format!(
                "Failed to connect to update stream: HTTP {}",
                response.status()
            )));
        }

        let stream = response.bytes_stream();
        self.response = Some(Box::pin(stream));
        
        info!("Connected to update stream for chain {}", self.chain_id);
        Ok(())
    }
}

impl Stream for UpdateStream {
    type Item = UpdateEvent;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        // Ensure we have a connection
        if self.response.is_none() {
            // Don't reconnect too frequently
            if self.last_reconnect.elapsed() < self.reconnect_delay {
                cx.waker().wake_by_ref();
                return Poll::Pending;
            }

            // Attempt to connect
            let mut connect_future = Box::pin(self.connect());
            match connect_future.as_mut().poll(cx) {
                Poll::Ready(Ok(())) => {
                    self.last_reconnect = Instant::now();
                }
                Poll::Ready(Err(e)) => {
                    error!("Failed to connect to update stream: {}", e);
                    self.reconnect_delay = (self.reconnect_delay * 2).min(Duration::from_secs(30));
                    cx.waker().wake_by_ref();
                    return Poll::Pending;
                }
                Poll::Pending => return Poll::Pending,
            }
        }

        // Poll the existing stream
        if let Some(ref mut stream) = self.response {
            match stream.as_mut().poll_next(cx) {
                Poll::Ready(Some(Ok(_bytes))) => {
                    // We received data - this indicates an update
                    self.reconnect_delay = Duration::from_secs(1); // Reset reconnect delay
                    Poll::Ready(Some(UpdateEvent::NewWork(self.chain_id)))
                }
                Poll::Ready(Some(Err(e))) => {
                    warn!("Update stream error: {}", e);
                    self.response = None; // Force reconnection
                    Poll::Ready(Some(UpdateEvent::Error(e.to_string())))
                }
                Poll::Ready(None) => {
                    info!("Update stream closed for chain {}", self.chain_id);
                    self.response = None; // Force reconnection
                    Poll::Ready(Some(UpdateEvent::Closed))
                }
                Poll::Pending => Poll::Pending,
            }
        } else {
            // No connection, wake to retry
            cx.waker().wake_by_ref();
            Poll::Pending
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{MinerAccount, MinerPublicKey};

    #[tokio::test]
    async fn test_client_creation() {
        let client = ChainwebClient::new(
            "http://localhost:1848",
            Duration::from_secs(30),
            false,
        );
        assert!(client.is_ok());
    }

    #[test]
    fn test_mining_job_parsing() {
        let client = ChainwebClient::new(
            "http://localhost:1848",
            Duration::from_secs(30),
            false,
        ).unwrap();

        // Create test data: chain_id (4) + target (32) + work (286)
        let mut test_data = Vec::new();
        test_data.extend_from_slice(&5u32.to_le_bytes()); // chain_id = 5
        test_data.extend_from_slice(&[0u8; 32]); // easy target
        test_data.extend_from_slice(&vec![0u8; Work::SIZE]); // work bytes

        let job = client.parse_mining_job(&test_data).unwrap();
        assert_eq!(job.chain_id.value(), 5);
    }

    #[test]
    fn test_backoff_config() {
        let config = BackoffConfig {
            initial_delay: Duration::from_millis(50),
            max_delay: Duration::from_secs(10),
            multiplier: 1.5,
            max_retries: 5,
        };

        let client = ChainwebClient::new(
            "http://localhost:1848",
            Duration::from_secs(30),
            false,
        ).unwrap().with_backoff_config(config);

        assert_eq!(client.backoff_config.max_retries, 5);
        assert_eq!(client.backoff_config.multiplier, 1.5);
    }
}