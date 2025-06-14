//! Stratum server implementation

use crate::config::StratumDifficulty;
use crate::core::{Nonce, Target, Work};
use crate::error::{Error, Result};
use crate::utils::monitoring::{global_monitoring, MonitoringSystem};
use crate::workers::{MiningResult, Worker};
use async_trait::async_trait;
use dashmap::DashMap;
use serde_json::Value;
use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, Instant};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{RwLock, broadcast, mpsc};
use tokio::time::interval;
use tracing::{error, info, warn};

use super::protocol::*;
use super::session::*;
use super::nonce::{Nonce1, Nonce2, NonceSize, compose_nonce};

/// Stratum server configuration
#[derive(Debug, Clone)]
pub struct StratumServerConfig {
    /// Listen port
    pub port: u16,
    /// Listen address  
    pub host: String,
    /// Max connections
    pub max_connections: usize,
    /// Difficulty setting
    pub difficulty: StratumDifficulty,
    /// Job emission rate in milliseconds
    pub rate_ms: u64,
}

/// Current mining job
#[derive(Debug, Clone)]
struct MiningJob {
    /// Job ID
    id: String,
    /// Work data
    work: Work,
    /// Target
    target: Target,
}

/// Stratum server state
struct ServerState {
    /// Active sessions
    sessions: DashMap<SessionId, Arc<RwLock<StratumSession>>>,
    /// Current job
    current_job: RwLock<Option<MiningJob>>,
    /// Job counter
    job_counter: AtomicU64,
    /// Total hashrate estimate
    total_hashrate: AtomicU64,
    /// Shutdown flag
    shutdown: AtomicBool,
    /// Result channel for submitted shares
    result_tx: RwLock<Option<mpsc::Sender<MiningResult>>>,
}

/// Stratum server for ASIC miners
pub struct StratumServer {
    config: StratumServerConfig,
    state: Arc<ServerState>,
    job_tx: broadcast::Sender<MiningJob>,
    result_tx: Option<mpsc::Sender<MiningResult>>,
}

impl StratumServer {
    /// Create a new Stratum server
    pub fn new(config: StratumServerConfig) -> Self {
        info!(
            "Initializing Stratum server on {}:{}",
            config.host, config.port
        );

        let (job_tx, _) = broadcast::channel(100);

        Self {
            config,
            state: Arc::new(ServerState {
                sessions: DashMap::new(),
                current_job: RwLock::new(None),
                job_counter: AtomicU64::new(0),
                total_hashrate: AtomicU64::new(0),
                shutdown: AtomicBool::new(false),
                result_tx: RwLock::new(None),
            }),
            job_tx,
            result_tx: None,
        }
    }

    /// Start the server
    async fn start_server(&self) -> Result<()> {
        let addr: SocketAddr = format!("{}:{}", self.config.host, self.config.port)
            .parse()
            .map_err(|e| Error::config(format!("Invalid address: {}", e)))?;

        let listener = TcpListener::bind(&addr)
            .await
            .map_err(|e| Error::network(format!("Failed to bind to {}: {}", addr, e)))?;

        info!("Stratum server listening on {}", addr);

        // Start job emitter
        let job_emitter = self.start_job_emitter();

        // Accept connections
        while !self.state.shutdown.load(Ordering::Relaxed) {
            tokio::select! {
                Ok((stream, addr)) = listener.accept() => {
                    if self.state.sessions.len() >= self.config.max_connections {
                        warn!("Max connections reached, rejecting {}", addr);
                        continue;
                    }

                    let state = Arc::clone(&self.state);
                    let job_rx = self.job_tx.subscribe();
                    let result_tx = self.result_tx.clone();

                    tokio::spawn(async move {
                        if let Err(e) = handle_client(stream, addr, state, job_rx, result_tx).await {
                            error!("Client {} error: {}", addr, e);
                        }
                    });
                }
                _ = tokio::signal::ctrl_c() => {
                    info!("Shutting down Stratum server");
                    break;
                }
            }
        }

        // Stop job emitter
        job_emitter.abort();

        Ok(())
    }

    /// Start job emitter task
    fn start_job_emitter(&self) -> tokio::task::JoinHandle<()> {
        let state = Arc::clone(&self.state);
        let job_tx = self.job_tx.clone();
        let rate = Duration::from_millis(self.config.rate_ms);

        tokio::spawn(async move {
            let mut ticker = interval(rate);
            ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

            loop {
                ticker.tick().await;

                if state.shutdown.load(Ordering::Relaxed) {
                    break;
                }

                // Get current job
                let job = state.current_job.read().await.clone();

                if let Some(job) = job {
                    // Emit job to all clients
                    let _ = job_tx.send(job);
                }
            }
        })
    }

    /// Update current work
    async fn update_work(&self, work: Work, target: Target) {
        let job_id = self.state.job_counter.fetch_add(1, Ordering::Relaxed);

        let job = MiningJob {
            id: format!("{:x}", job_id),
            work: work.clone(),
            target,
        };

        // Update current job
        *self.state.current_job.write().await = Some(job.clone());

        // Broadcast to all clients
        let _ = self.job_tx.send(job);
    }

}

/// Handle a client connection
async fn handle_client(
    stream: TcpStream,
    addr: SocketAddr,
    state: Arc<ServerState>,
    mut job_rx: broadcast::Receiver<MiningJob>,
    _result_tx: Option<mpsc::Sender<MiningResult>>,
) -> Result<()> {
    info!("New connection from {}", addr);

    let (reader, mut writer) = stream.into_split();
    let mut reader = BufReader::new(reader);

    // Create session
    let extranonce1 = generate_extranonce1();
    let session = Arc::new(RwLock::new(StratumSession::new(
        extranonce1.clone(),
        1.0, // Initial difficulty
    )));
    let session_id = session.read().await.id;

    state.sessions.insert(session_id, Arc::clone(&session));

    // Client state
    let mut authorized = false;
    let mut subscribed = false;

    loop {
        let mut line = String::new();

        tokio::select! {
            // Read from client
            result = reader.read_line(&mut line) => {
                match result {
                    Ok(0) => {
                        info!("Client {} disconnected", addr);
                        break;
                    }
                    Ok(_) => {
                        // Process message
                        match StratumMessage::from_json(&line) {
                            Ok(StratumMessage::Request(req)) => {
                                let response = handle_request(
                                    req,
                                    &mut authorized,
                                    &mut subscribed,
                                    &session,
                                    &extranonce1,
                                    &state,
                                ).await;

                                let json = serde_json::to_string(&response)? + "\n";
                                writer.write_all(json.as_bytes()).await?;
                            }
                            Ok(_) => {
                                warn!("Unexpected message type from {}", addr);
                            }
                            Err(e) => {
                                error!("Failed to parse message from {}: {}", addr, e);
                            }
                        }
                    }
                    Err(e) => {
                        error!("Read error from {}: {}", addr, e);
                        break;
                    }
                }
            }

            // Receive job updates
            Ok(job) = job_rx.recv() => {
                if subscribed && authorized {
                    // Send mining.notify
                    let params = create_job_params(&job);
                    let notify = StratumNotification::new("mining.notify", params);

                    let json = serde_json::to_string(&notify)? + "\n";
                    writer.write_all(json.as_bytes()).await?;
                }
            }
        }
    }

    // Remove session
    state.sessions.remove(&session_id);

    Ok(())
}

/// Handle a Stratum request
async fn handle_request(
    req: StratumRequest,
    authorized: &mut bool,
    subscribed: &mut bool,
    session: &Arc<RwLock<StratumSession>>,
    extranonce1: &Nonce1,
    state: &Arc<ServerState>,
) -> StratumResponse {
    match req.method_enum() {
        StratumMethod::Subscribe => {
            // mining.subscribe("miner/version", "session_id")
            *subscribed = true;

            // Response: [["mining.notify", "subscription_id"], "extranonce1", extranonce2_size]
            let subscription_id = format!("{:x}", rand::random::<u32>());
            let result = Value::Array(vec![
                Value::Array(vec![Value::Array(vec![
                    Value::String("mining.notify".to_string()),
                    Value::String(subscription_id),
                ])]),
                Value::String(extranonce1.to_hex()),
                Value::Number(extranonce1.nonce2_size().as_bytes().into()), // extranonce2 size
            ]);

            StratumResponse::success(req.id, result)
        }

        StratumMethod::Authorize => {
            // mining.authorize("username", "password")
            if !req.params.is_empty() {
                if let Some(Value::String(username)) = req.params.first() {
                    let mut session = session.write().await;
                    session.worker_name = Some(username.clone());
                    *authorized = true;

                    StratumResponse::success(req.id, Value::Bool(true))
                } else {
                    StratumResponse::error(req.id, 20, "Invalid username")
                }
            } else {
                StratumResponse::error(req.id, 20, "Missing username")
            }
        }

        StratumMethod::Submit => {
            // mining.submit("username", "job_id", "extranonce2", "ntime", "nonce")
            if !*authorized {
                return StratumResponse::error(req.id, 24, "Unauthorized worker");
            }

            // Parse submit parameters
            if req.params.len() < 5 {
                return StratumResponse::error(req.id, 20, "Missing parameters");
            }

            let params = &req.params;
            let _username = match params[0].as_str() {
                Some(u) => u,
                None => return StratumResponse::error(req.id, 20, "Invalid username"),
            };
            let job_id = match params[1].as_str() {
                Some(j) => j,
                None => return StratumResponse::error(req.id, 20, "Invalid job_id"),
            };
            let extranonce2_hex = match params[2].as_str() {
                Some(e) => e,
                None => return StratumResponse::error(req.id, 20, "Invalid extranonce2"),
            };
            let _ntime = match params[3].as_str() {
                Some(n) => n,
                None => return StratumResponse::error(req.id, 20, "Invalid ntime"),
            };
            let nonce_hex = match params[4].as_str() {
                Some(n) => n,
                None => return StratumResponse::error(req.id, 20, "Invalid nonce"),
            };

            // Update share statistics
            let mut session = session.write().await;
            session.shares_submitted += 1;

            // Get the current job
            let current_job = state.current_job.read().await;
            let job = match &*current_job {
                Some(j) if j.id == job_id => j.clone(),
                _ => {
                    drop(current_job);
                    return StratumResponse::error(req.id, 21, "Job not found");
                }
            };
            drop(current_job);

            // Parse extranonce2
            let extranonce2_bytes = match hex::decode(extranonce2_hex) {
                Ok(b) => b,
                Err(_) => return StratumResponse::error(req.id, 20, "Invalid extranonce2 hex"),
            };

            if extranonce2_bytes.len() != extranonce1.nonce2_size().as_bytes() as usize {
                // Record share rejected
                global_monitoring().record_share_submitted(false);
                return StratumResponse::error(req.id, 20, "Invalid extranonce2 size");
            }

            // Create Nonce2
            let nonce2_size = extranonce1.nonce2_size();
            let nonce2 = match Nonce2::from_bytes(nonce2_size, &extranonce2_bytes) {
                Ok(n) => n,
                Err(_) => return StratumResponse::error(req.id, 20, "Invalid extranonce2 format"),
            };

            // Compose the full nonce
            let _full_nonce = match compose_nonce(extranonce1.clone(), nonce2) {
                Ok(n) => n,
                Err(_) => return StratumResponse::error(req.id, 20, "Failed to compose nonce"),
            };

            // Parse the submitted nonce
            let submitted_nonce = match hex::decode(nonce_hex) {
                Ok(b) if b.len() == 8 => {
                    let mut arr = [0u8; 8];
                    arr.copy_from_slice(&b);
                    u64::from_le_bytes(arr)
                }
                _ => return StratumResponse::error(req.id, 20, "Invalid nonce hex"),
            };

            // Create a modified work with the composed nonce
            let mut work_bytes = job.work.as_bytes().to_vec();
            
            // The nonce should be at offset 278 in the work (286 - 8)
            if work_bytes.len() >= 286 {
                // Update the nonce in the work
                work_bytes[278..286].copy_from_slice(&submitted_nonce.to_le_bytes());
                
                // Create new work from the modified bytes
                let mut work_array = [0u8; 286];
                work_array.copy_from_slice(&work_bytes[..286]);
                let modified_work = Work::from_bytes(work_array);

                // Compute the hash for the modified work
                let hash = modified_work.hash();

                // Submit the result if we have a channel
                if let Some(ref tx) = *state.result_tx.read().await {
                    let result = MiningResult {
                        work: modified_work,
                        nonce: Nonce::new(submitted_nonce),
                        hash,
                    };

                    if tx.send(result).await.is_ok() {
                        session.shares_valid += 1;
                        
                        // Record share accepted in monitoring
                        global_monitoring().record_share_submitted(true);
                        
                        StratumResponse::success(req.id, Value::Bool(true))
                    } else {
                        // Record share rejected in monitoring
                        global_monitoring().record_share_submitted(false);
                        
                        StratumResponse::error(req.id, 20, "Failed to submit share")
                    }
                } else {
                    // No result channel, just accept the share
                    session.shares_valid += 1;
                    
                    // Record share accepted in monitoring
                    global_monitoring().record_share_submitted(true);
                    
                    StratumResponse::success(req.id, Value::Bool(true))
                }
            } else {
                StratumResponse::error(req.id, 20, "Invalid work size")
            }
        }

        _ => StratumResponse::error(req.id, 20, "Method not supported"),
    }
}

/// Generate extranonce1 for a new session
fn generate_extranonce1() -> Nonce1 {
    let mut bytes = [0u8; 4];
    getrandom::fill(&mut bytes).unwrap();
    // Convert bytes to u64 (big-endian)
    let value = u32::from_be_bytes(bytes) as u64;
    Nonce1::new(NonceSize::new(4).unwrap(), value).unwrap()
}

/// Create job parameters for mining.notify
fn create_job_params(job: &MiningJob) -> Vec<Value> {
    // For Kadena, we need to adapt the work format
    // This is a simplified version - real implementation would need proper conversion
    vec![
        Value::String(job.id.clone()),
        Value::String(hex::encode(&job.work.as_bytes()[0..32])), // Previous hash
        Value::String(hex::encode(&job.work.as_bytes()[32..64])), // Coinbase 1
        Value::String(hex::encode(&job.work.as_bytes()[64..96])), // Coinbase 2
        Value::Array(vec![]),                                    // Merkle branches (empty for now)
        Value::String("00000020".to_string()),                   // Version
        Value::String(hex::encode(job.target.0)),                // nBits
        Value::String(format!(
            "{:x}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs()
        )), // nTime
        Value::Bool(true),                                       // Clean jobs
    ]
}

#[async_trait]
impl Worker for StratumServer {
    async fn mine(
        &self,
        work: Work,
        target: Target,
        result_tx: mpsc::Sender<MiningResult>,
    ) -> Result<()> {
        // Store result channel in state
        {
            let mut tx = self.state.result_tx.write().await;
            *tx = Some(result_tx.clone());
        }

        // Store result channel
        let server = Self {
            config: self.config.clone(),
            state: Arc::clone(&self.state),
            job_tx: self.job_tx.clone(),
            result_tx: Some(result_tx),
        };

        // Update work
        server.update_work(work, target).await;

        // Start server if not already running
        tokio::spawn(async move {
            if let Err(e) = server.start_server().await {
                error!("Stratum server error: {}", e);
            }
        });

        Ok(())
    }

    async fn stop(&self) -> Result<()> {
        self.state.shutdown.store(true, Ordering::Relaxed);
        Ok(())
    }

    fn worker_type(&self) -> &str {
        "Stratum"
    }

    async fn hashrate(&self) -> u64 {
        self.state.total_hashrate.load(Ordering::Relaxed)
    }
}
