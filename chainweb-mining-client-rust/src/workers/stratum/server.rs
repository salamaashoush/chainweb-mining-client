//! Stratum server implementation

use crate::config::StratumDifficulty;
use crate::core::{Target, Work};
use crate::error::{Error, Result};
use crate::workers::{MiningResult, Worker};
use async_trait::async_trait;
use dashmap::DashMap;
use serde_json::Value;
use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{RwLock, broadcast, mpsc};
use tokio::time::interval;
use tracing::{error, info, warn};

use super::protocol::*;
use super::session::*;

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
    /// Creation time
    #[allow(dead_code)]
    created_at: SystemTime,
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
            created_at: SystemTime::now(),
        };

        // Update current job
        *self.state.current_job.write().await = Some(job.clone());

        // Broadcast to all clients
        let _ = self.job_tx.send(job);
    }

    /// Get current difficulty based on configuration
    #[allow(dead_code)]
    fn get_difficulty(&self, block_target: &Target) -> Target {
        match &self.config.difficulty {
            StratumDifficulty::Block => *block_target,
            StratumDifficulty::Fixed(zeros) => {
                // Create target with specified number of leading zeros
                Target::from_difficulty_bits(*zeros as u32)
            }
        }
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
    extranonce1: &[u8],
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
                Value::String(hex::encode(extranonce1)),
                Value::Number(4.into()), // extranonce2 size
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

            // TODO: Validate and process share submission
            let mut session = session.write().await;
            session.shares_submitted += 1;

            // For now, accept all shares
            session.shares_valid += 1;
            StratumResponse::success(req.id, Value::Bool(true))
        }

        _ => StratumResponse::error(req.id, 20, "Method not supported"),
    }
}

/// Generate extranonce1 for a new session
fn generate_extranonce1() -> Vec<u8> {
    let mut bytes = vec![0u8; 4];
    getrandom::fill(&mut bytes).unwrap();
    bytes
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
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
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
