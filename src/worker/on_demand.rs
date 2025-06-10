//! On-demand worker for HTTP-triggered mining
//!
//! Provides an HTTP server that allows triggering block production on demand,
//! useful for testing and development scenarios.

use super::{mining_span, MiningStats, MiningWorker};
use crate::{ChainId, Error, Nonce, Result, Target, Work};
use async_trait::async_trait;
use rand::Rng;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, Mutex};
use tokio_util::sync::CancellationToken;
use tracing::{debug, info, warn};
use warp::{Filter, Reply};

/// Request to produce blocks on specific chains
#[derive(Debug, Deserialize, Serialize)]
pub struct MakeBlocksRequest {
    /// Map of chain ID to number of blocks to produce
    pub chains: HashMap<u32, u32>,
}

/// Response from block production request
#[derive(Debug, Serialize)]
pub struct MakeBlocksResponse {
    /// Number of blocks produced per chain
    pub blocks_produced: HashMap<u32, u32>,
    /// Status message
    pub status: String,
}

/// Shared state for the on-demand worker
#[derive(Debug)]
struct OnDemandState {
    /// Pending block requests
    pending_requests: Arc<Mutex<Vec<(ChainId, u32)>>>, // (chain_id, count)
    /// Current mining work if active
    current_work: Arc<Mutex<Option<(Target, ChainId, Work)>>>,
    /// Statistics
    stats: Arc<Mutex<MiningStats>>,
}

/// On-demand worker that responds to HTTP requests
pub struct OnDemandWorker {
    interface: String,
    port: u16,
    state: OnDemandState,
}

impl OnDemandWorker {
    /// Create a new on-demand worker
    pub fn new(interface: String, port: u16) -> Result<Self> {
        // Validate interface
        interface
            .parse::<std::net::IpAddr>()
            .map_err(|e| Error::config(format!("Invalid interface address: {}", e)))?;

        info!("Creating on-demand worker on {}:{}", interface, port);

        let state = OnDemandState {
            pending_requests: Arc::new(Mutex::new(Vec::new())),
            current_work: Arc::new(Mutex::new(None)),
            stats: Arc::new(Mutex::new(MiningStats::default())),
        };

        Ok(Self {
            interface,
            port,
            state,
        })
    }

    /// Get the socket address for the HTTP server
    fn socket_addr(&self) -> Result<SocketAddr> {
        let ip = self
            .interface
            .parse()
            .map_err(|e| Error::config(format!("Invalid interface: {}", e)))?;
        Ok(SocketAddr::new(ip, self.port))
    }

    /// Start the HTTP server
    async fn start_server(&self, cancellation: CancellationToken) -> Result<()> {
        let addr = self.socket_addr()?;
        let state = self.state.pending_requests.clone();

        // Create the /make-blocks endpoint
        let make_blocks = warp::path("make-blocks")
            .and(warp::post())
            .and(warp::body::json())
            .and(warp::any().map(move || state.clone()))
            .and_then(handle_make_blocks_request);

        // Create the /status endpoint
        let status = warp::path("status")
            .and(warp::get())
            .map(|| warp::reply::with_status("OK", warp::http::StatusCode::OK));

        let routes = make_blocks.or(status);

        info!("Starting on-demand HTTP server on {}", addr);

        let server = warp::serve(routes).bind(addr);

        tokio::select! {
            _ = server => {
                info!("On-demand HTTP server stopped");
            }
            _ = cancellation.cancelled() => {
                info!("On-demand HTTP server cancelled");
            }
        }

        Ok(())
    }

    /// Process pending block requests
    async fn process_requests(
        &mut self,
        target: Target,
        chain_id: ChainId,
        work: Work,
    ) -> Result<Option<Work>> {
        let mut pending = self.state.pending_requests.lock().await;

        // Find requests for this chain
        let mut remaining_requests = Vec::new();
        let mut blocks_to_produce = 0u32;

        for (req_chain_id, count) in pending.drain(..) {
            if req_chain_id == chain_id {
                blocks_to_produce += count;
            } else {
                remaining_requests.push((req_chain_id, count));
            }
        }

        *pending = remaining_requests;
        drop(pending);

        if blocks_to_produce > 0 {
            info!(
                "Processing request to produce {} blocks for chain {}",
                blocks_to_produce, chain_id
            );

            // Produce a block (for simplicity, we only produce one at a time)
            let mut solved_work = work;
            let mut rng = rand::thread_rng();
            let solution_nonce = Nonce::new(rng.gen());
            solved_work.inject_nonce(solution_nonce);

            // Update statistics
            let mut stats = self.state.stats.lock().await;
            stats.solutions_found += 1;
            stats.total_hashes += 1; // Nominal hash count
            stats.current_hash_rate = 1.0; // Nominal rate
            stats.average_hash_rate = if stats.mining_time_secs > 0 {
                stats.solutions_found as f64 / stats.mining_time_secs as f64
            } else {
                stats.solutions_found as f64
            };
            drop(stats);

            info!(
                "Produced block for chain {} with nonce {}",
                chain_id, solution_nonce
            );
            return Ok(Some(solved_work));
        }

        Ok(None)
    }
}

/// Handle /make-blocks HTTP requests
async fn handle_make_blocks_request(
    request: MakeBlocksRequest,
    state: Arc<Mutex<Vec<(ChainId, u32)>>>,
) -> std::result::Result<impl Reply, warp::Rejection> {
    debug!("Received make-blocks request: {:?}", request);

    let mut pending = state.lock().await;
    let mut total_blocks = 0;

    for (chain_id, count) in request.chains {
        if count > 0 {
            pending.push((ChainId::new(chain_id), count));
            total_blocks += count;
        }
    }

    drop(pending);

    let response = MakeBlocksResponse {
        blocks_produced: request.chains,
        status: format!("Queued {} block requests", total_blocks),
    };

    info!("Queued {} block production requests", total_blocks);

    Ok(warp::reply::json(&response))
}

#[async_trait]
impl MiningWorker for OnDemandWorker {
    fn worker_type(&self) -> &'static str {
        "on-demand"
    }

    async fn mine(
        &mut self,
        _initial_nonce: Nonce,
        target: Target,
        chain_id: ChainId,
        work: Work,
        cancellation: CancellationToken,
        stats_tx: Option<mpsc::UnboundedSender<MiningStats>>,
    ) -> Result<Work> {
        let _span = mining_span(self.worker_type(), chain_id);

        info!(
            "Starting on-demand mining for chain {} on {}:{} (difficulty level: {})",
            chain_id,
            self.interface,
            self.port,
            target.difficulty_level()
        );

        // Reset statistics
        {
            let mut stats = self.state.stats.lock().await;
            *stats = MiningStats::default();
        }

        // Store current work
        {
            let mut current = self.state.current_work.lock().await;
            *current = Some((target, chain_id, work.clone()));
        }

        // Start HTTP server
        let server_cancellation = cancellation.clone();
        let server_handle = tokio::spawn({
            let worker = self.clone_for_server();
            async move {
                if let Err(e) = worker.start_server(server_cancellation).await {
                    warn!("HTTP server error: {}", e);
                }
            }
        });

        // Statistics reporting
        let stats_clone = self.state.stats.clone();
        let stats_cancellation = cancellation.clone();
        let stats_handle = if let Some(stats_tx) = stats_tx {
            Some(tokio::spawn(async move {
                let mut interval = tokio::time::interval(Duration::from_secs(5));

                while !stats_cancellation.is_cancelled() {
                    tokio::select! {
                        _ = interval.tick() => {
                            let stats = stats_clone.lock().await.clone();
                            let _ = stats_tx.send(stats);
                        }
                        _ = stats_cancellation.cancelled() => break,
                    }
                }
            }))
        } else {
            None
        };

        // Main mining loop - wait for requests
        let mut check_interval = tokio::time::interval(Duration::from_millis(100));
        let start_time = Instant::now();

        let result = loop {
            tokio::select! {
                _ = check_interval.tick() => {
                    // Update mining time
                    {
                        let mut stats = self.state.stats.lock().await;
                        stats.mining_time_secs = start_time.elapsed().as_secs();
                    }

                    // Check for and process requests
                    if let Ok(Some(solved_work)) = self.process_requests(target, chain_id, work.clone()).await {
                        break Ok(solved_work);
                    }
                }
                _ = cancellation.cancelled() => {
                    info!("On-demand mining cancelled");
                    break Err(Error::cancelled("On-demand mining"));
                }
            }
        };

        // Cleanup
        server_handle.abort();
        if let Some(handle) = stats_handle {
            let _ = handle.await;
        }

        // Clear current work
        {
            let mut current = self.state.current_work.lock().await;
            *current = None;
        }

        match &result {
            Ok(_) => {
                let stats = self.state.stats.lock().await;
                info!(
                    "On-demand mining completed successfully. Blocks produced: {}",
                    stats.solutions_found
                );
            }
            Err(e) => {
                info!("On-demand mining failed: {}", e);
            }
        }

        result
    }

    fn stats(&self) -> MiningStats {
        // This is a blocking call, but for the interface we need a sync method
        // In practice, this should rarely be called directly
        MiningStats::default()
    }
}

impl OnDemandWorker {
    /// Clone for server (simplified clone for server task)
    fn clone_for_server(&self) -> Self {
        Self {
            interface: self.interface.clone(),
            port: self.port,
            state: OnDemandState {
                pending_requests: Arc::clone(&self.state.pending_requests),
                current_work: Arc::clone(&self.state.current_work),
                stats: Arc::clone(&self.state.stats),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Target, Work};

    #[test]
    fn test_on_demand_worker_creation() {
        let worker = OnDemandWorker::new("127.0.0.1".to_string(), 8080);
        assert!(worker.is_ok());

        let worker = worker.unwrap();
        assert_eq!(worker.worker_type(), "on-demand");
        assert_eq!(worker.interface, "127.0.0.1");
        assert_eq!(worker.port, 8080);
    }

    #[test]
    fn test_invalid_interface() {
        let worker = OnDemandWorker::new("invalid-interface".to_string(), 8080);
        assert!(worker.is_err());
    }

    #[test]
    fn test_socket_addr() {
        let worker = OnDemandWorker::new("0.0.0.0".to_string(), 8080).unwrap();
        let addr = worker.socket_addr().unwrap();
        assert_eq!(addr.ip().to_string(), "0.0.0.0");
        assert_eq!(addr.port(), 8080);
    }

    #[tokio::test]
    async fn test_make_blocks_request_serialization() {
        let mut chains = HashMap::new();
        chains.insert(0, 1);
        chains.insert(1, 2);

        let request = MakeBlocksRequest { chains };
        let json = serde_json::to_string(&request).unwrap();
        let deserialized: MakeBlocksRequest = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.chains.len(), 2);
        assert_eq!(deserialized.chains[&0], 1);
        assert_eq!(deserialized.chains[&1], 2);
    }

    #[tokio::test]
    async fn test_process_requests() {
        let mut worker = OnDemandWorker::new("127.0.0.1".to_string(), 8080).unwrap();
        let target = Target::max();
        let chain_id = ChainId::new(0);
        let work = Work::new(vec![0u8; Work::SIZE]).unwrap();

        // Add a request for this chain
        {
            let mut pending = worker.state.pending_requests.lock().await;
            pending.push((chain_id, 1));
        }

        let result = worker
            .process_requests(target, chain_id, work)
            .await
            .unwrap();
        assert!(result.is_some());

        // Check statistics were updated
        let stats = worker.state.stats.lock().await;
        assert_eq!(stats.solutions_found, 1);
    }

    #[tokio::test]
    async fn test_process_requests_different_chain() {
        let mut worker = OnDemandWorker::new("127.0.0.1".to_string(), 8080).unwrap();
        let target = Target::max();
        let chain_id = ChainId::new(0);
        let work = Work::new(vec![0u8; Work::SIZE]).unwrap();

        // Add a request for a different chain
        {
            let mut pending = worker.state.pending_requests.lock().await;
            pending.push((ChainId::new(1), 1)); // Different chain
        }

        let result = worker
            .process_requests(target, chain_id, work)
            .await
            .unwrap();
        assert!(result.is_none());

        // Request should still be pending
        let pending = worker.state.pending_requests.lock().await;
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].0, ChainId::new(1));
    }

    // Note: Full HTTP server testing would require more complex integration tests
    // and is beyond the scope of unit tests
}
