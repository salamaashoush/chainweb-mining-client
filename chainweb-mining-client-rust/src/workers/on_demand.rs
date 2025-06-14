//! On-demand mining worker with HTTP interface

use crate::core::{Nonce, Target, Work};
use crate::error::{Error, Result};
use crate::workers::{MiningResult, Worker};
use async_trait::async_trait;
use axum::{
    Router,
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Json},
    routing::post,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use tokio::sync::{RwLock, mpsc};
use tracing::{debug, error, info};

/// Configuration for on-demand mining
#[derive(Debug, Clone)]
pub struct OnDemandWorkerConfig {
    /// HTTP server port
    pub port: u16,
    /// HTTP server host
    pub host: String,
}

/// Request to mine blocks
#[derive(Debug, Clone, Deserialize)]
struct MakeBlocksRequest {
    /// Map of chain ID to number of blocks to mine
    #[serde(flatten)]
    chains: HashMap<String, u64>,
}

/// Response from make-blocks endpoint
#[derive(Debug, Clone, Serialize)]
struct MakeBlocksResponse {
    /// Number of blocks mined per chain
    blocks_mined: HashMap<u16, u64>,
    /// Any errors encountered
    errors: Vec<String>,
}

/// Shared state for the HTTP server
#[derive(Clone)]
struct ServerState {
    /// Current work and target
    work_info: Arc<RwLock<Option<(Work, Target)>>>,
    /// Channel to send mining results
    result_tx: Arc<RwLock<Option<mpsc::Sender<MiningResult>>>>,
    /// Block counter
    block_counter: Arc<AtomicU64>,
}

/// On-demand mining worker
pub struct OnDemandWorker {
    config: OnDemandWorkerConfig,
    running: Arc<AtomicBool>,
    server_state: ServerState,
}

impl OnDemandWorker {
    /// Create a new on-demand worker
    pub fn new(config: OnDemandWorkerConfig) -> Self {
        info!(
            "Initializing on-demand worker on {}:{}",
            config.host, config.port
        );

        Self {
            config,
            running: Arc::new(AtomicBool::new(false)),
            server_state: ServerState {
                work_info: Arc::new(RwLock::new(None)),
                result_tx: Arc::new(RwLock::new(None)),
                block_counter: Arc::new(AtomicU64::new(0)),
            },
        }
    }

    /// Start the HTTP server
    async fn start_server(&self) -> Result<()> {
        let app = Router::new()
            .route("/make-blocks", post(make_blocks_handler))
            .with_state(self.server_state.clone());

        let addr: SocketAddr = format!("{}:{}", self.config.host, self.config.port)
            .parse()
            .map_err(|e| Error::config(format!("Invalid address: {}", e)))?;

        let listener = tokio::net::TcpListener::bind(&addr)
            .await
            .map_err(|e| Error::network(format!("Failed to bind to {}: {}", addr, e)))?;

        info!("On-demand mining server listening on {}", addr);

        axum::serve(listener, app)
            .await
            .map_err(|e| Error::network(format!("HTTP server error: {}", e)))?;

        Ok(())
    }
}

/// Handler for /make-blocks endpoint
async fn make_blocks_handler(
    State(state): State<ServerState>,
    Json(request): Json<MakeBlocksRequest>,
) -> impl IntoResponse {
    let mut blocks_mined = HashMap::new();
    let mut errors = Vec::new();

    // Get current work info
    let work_info = state.work_info.read().await;
    let result_tx = state.result_tx.read().await;

    if work_info.is_none() || result_tx.is_none() {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(MakeBlocksResponse {
                blocks_mined,
                errors: vec!["No work available".to_string()],
            }),
        );
    }

    let (work, _target) = work_info.as_ref().unwrap();
    let tx = result_tx.as_ref().unwrap();

    // Mine requested blocks for each chain
    for (chain_id_str, count) in request.chains {
        // Parse chain ID from string
        let chain_id = match chain_id_str.parse::<u16>() {
            Ok(id) => id,
            Err(_) => {
                errors.push(format!("Invalid chain ID: {}", chain_id_str));
                continue;
            }
        };

        debug!("Mining {} blocks for chain {}", count, chain_id);

        let mut mined = 0;
        for _ in 0..count {
            // Generate unique nonce
            let nonce_value = state.block_counter.fetch_add(1, Ordering::Relaxed);
            let nonce = Nonce::new(nonce_value);

            // Create mining result
            let result = MiningResult {
                work: work.clone(),
                nonce,
                hash: [0u8; 32], // Fake hash for non-PoW mode
            };

            // Send result
            if let Err(e) = tx.send(result).await {
                errors.push(format!("Failed to submit block: {}", e));
                break;
            }

            mined += 1;
        }

        blocks_mined.insert(chain_id, mined);
        info!("Mined {} blocks for chain {}", mined, chain_id);
    }

    (
        StatusCode::OK,
        Json(MakeBlocksResponse {
            blocks_mined,
            errors,
        }),
    )
}

#[async_trait]
impl Worker for OnDemandWorker {
    async fn mine(
        &self,
        work: Work,
        target: Target,
        result_tx: mpsc::Sender<MiningResult>,
    ) -> Result<()> {
        self.running.store(true, Ordering::Relaxed);

        // Update work info
        *self.server_state.work_info.write().await = Some((work, target));
        *self.server_state.result_tx.write().await = Some(result_tx);

        // Start HTTP server if not already running
        let running = Arc::clone(&self.running);
        let worker = self.clone();

        tokio::spawn(async move {
            while running.load(Ordering::Relaxed) {
                if let Err(e) = worker.start_server().await {
                    error!("On-demand server error: {}", e);
                    tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
                }
            }
        });

        Ok(())
    }

    async fn stop(&self) -> Result<()> {
        self.running.store(false, Ordering::Relaxed);
        *self.server_state.work_info.write().await = None;
        *self.server_state.result_tx.write().await = None;
        Ok(())
    }

    fn worker_type(&self) -> &str {
        "OnDemand"
    }

    async fn hashrate(&self) -> u64 {
        // On-demand doesn't have a meaningful hashrate
        0
    }
}

impl Clone for OnDemandWorker {
    fn clone(&self) -> Self {
        Self {
            config: self.config.clone(),
            running: Arc::clone(&self.running),
            server_state: self.server_state.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::Target;

    #[tokio::test]
    async fn test_on_demand_worker() {
        let config = OnDemandWorkerConfig {
            port: 0, // Use random port for testing
            host: "127.0.0.1".to_string(),
        };

        let worker = OnDemandWorker::new(config);
        let (tx, _rx) = mpsc::channel(10);

        let work = Work::from_bytes([0u8; 286]);
        let target = Target::from_bytes([0xFF; 32]);

        // Start mining
        worker.mine(work, target, tx).await.unwrap();

        // Check that worker is running
        assert_eq!(worker.worker_type(), "OnDemand");
        assert_eq!(worker.hashrate().await, 0);

        // Stop mining
        worker.stop().await.unwrap();
    }
}
