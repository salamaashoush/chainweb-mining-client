//! Stratum server implementation

use crate::config::StratumDifficulty;
use crate::core::{adjust_difficulty, Difficulty, HashRate, Nonce, Period, Target, Work};
use crate::error::{Error, Result};
use crate::utils::monitoring::global_monitoring;
use crate::workers::{MiningResult, Worker};
use async_trait::async_trait;
use dashmap::DashMap;
use serde_json::Value;
use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream, tcp::OwnedWriteHalf};
use tokio::sync::{RwLock, broadcast, mpsc};
use tokio::time::interval;
use tracing::{error, info, warn, debug};

use super::nonce::{Nonce1, Nonce2, NonceSize, compose_nonce};
use super::protocol::{StratumErrorCode, *};
use super::session::*;

/// Constants for dynamic difficulty adjustment
const TARGET_PERIOD: f64 = 10.0;  // Target 10 seconds between shares
const PERIOD_TOLERANCE: f64 = 0.25;  // 25% tolerance before adjusting
const MAX_SESSION_TARGET_LEVEL: u8 = 42;  // Minimum difficulty level

/// Authorization callback type
/// Returns Ok(()) if authorized, Err(message) if not
pub type AuthorizeCallback = Box<dyn Fn(&str, &str) -> std::result::Result<(), String> + Send + Sync>;

/// Stratum server configuration
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
    /// Optional authorization callback
    pub authorize_callback: Option<AuthorizeCallback>,
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

impl MiningJob {
    /// Increment the job time by the given microseconds
    /// This matches Haskell's incrementJobTime function
    pub fn increment_job_time(&mut self, micros: i64) {
        self.work.increment_time_micros(micros);
    }
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
    /// Difficulty configuration
    difficulty_config: StratumDifficulty,
    /// Authorization callback
    authorize_callback: Option<AuthorizeCallback>,
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
            config: StratumServerConfig {
                port: config.port,
                host: config.host.clone(),
                max_connections: config.max_connections,
                difficulty: config.difficulty.clone(),
                rate_ms: config.rate_ms,
                authorize_callback: None, // Callbacks can't be cloned, so we don't store it here
            },
            state: Arc::new(ServerState {
                sessions: DashMap::new(),
                current_job: RwLock::new(None),
                job_counter: AtomicU64::new(0),
                total_hashrate: AtomicU64::new(0),
                shutdown: AtomicBool::new(false),
                result_tx: RwLock::new(None),
                difficulty_config: config.difficulty.clone(),
                authorize_callback: config.authorize_callback,
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
            
            let mut last_job_update = std::time::Instant::now();
            let job_update_interval = Duration::from_secs(30); // Update job time every 30 seconds

            loop {
                ticker.tick().await;

                if state.shutdown.load(Ordering::Relaxed) {
                    break;
                }

                // Check if we need to update job time
                let now = std::time::Instant::now();
                let should_update_time = now.duration_since(last_job_update) >= job_update_interval;

                // Get current job
                let job = state.current_job.read().await.clone();

                if let Some(mut current_job) = job {
                    // Update job time if needed
                    if should_update_time {
                        let micros_to_add = job_update_interval.as_micros() as i64;
                        current_job.increment_job_time(micros_to_add);
                        
                        // Update the stored job
                        *state.current_job.write().await = Some(current_job.clone());
                        last_job_update = now;
                        
                        debug!("Updated job time by {} microseconds", micros_to_add);
                    }
                    
                    // Emit job to all clients
                    let _ = job_tx.send(current_job);
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

    // Create session with initial difficulty based on config
    let extranonce1 = generate_extranonce1();
    let initial_difficulty = match &state.difficulty_config {
        StratumDifficulty::Block => 1.0, // Will be updated with actual work
        StratumDifficulty::Fixed(level) => 2f64.powi(*level as i32),
        StratumDifficulty::Period(_) => 1.0, // Start with low difficulty, will adjust
    };
    
    let session = Arc::new(RwLock::new(StratumSession::new(
        extranonce1,
        initial_difficulty,
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
                                    &mut writer,
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
                    
                    // If this is the first job and we're using period-based difficulty,
                    // set initial session target
                    let mut session = session.write().await;
                    if session.session_target.is_none() {
                        match &state.difficulty_config {
                            StratumDifficulty::Block => {
                                session.session_target = Some(job.target);
                                session.difficulty = Difficulty::from(job.target).0;
                            }
                            StratumDifficulty::Fixed(level) => {
                                let target = Target::mk_target_level(*level);
                                session.session_target = Some(target);
                                session.difficulty = Difficulty::from(target).0;
                                // Send initial difficulty
                                drop(session);
                                send_set_target(&mut writer, &target).await?;
                            }
                            StratumDifficulty::Period(_) => {
                                // Start with a reasonable initial difficulty
                                let initial_target = Target::mk_target_level(20); // Reasonable starting point
                                session.session_target = Some(initial_target);
                                session.difficulty = Difficulty::from(initial_target).0;
                                // Send initial difficulty
                                drop(session);
                                send_set_target(&mut writer, &initial_target).await?;
                            }
                        }
                    }
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
    writer: &mut OwnedWriteHalf,
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
            if req.params.len() >= 1 {
                let username = match req.params[0].as_str() {
                    Some(u) => u,
                    None => return StratumResponse::error(req.id, 20, "Invalid username"),
                };
                
                let password = req.params.get(1)
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                
                // Check authorization if callback is provided
                if let Some(ref callback) = state.authorize_callback {
                    match callback(username, password) {
                        Ok(()) => {
                            let mut session = session.write().await;
                            session.worker_name = Some(username.to_string());
                            *authorized = true;
                            StratumResponse::success(req.id, Value::Bool(true))
                        }
                        Err(msg) => {
                            StratumResponse::error_with_code_and_message(req.id, StratumErrorCode::UnauthorizedWorker, &msg)
                        }
                    }
                } else {
                    // No callback, always authorize
                    let mut session = session.write().await;
                    session.worker_name = Some(username.to_string());
                    *authorized = true;
                    StratumResponse::success(req.id, Value::Bool(true))
                }
            } else {
                StratumResponse::error_with_code_and_message(req.id, StratumErrorCode::Other, "Missing username")
            }
        }

        StratumMethod::Submit => {
            // mining.submit("username", "job_id", "extranonce2", "ntime", "nonce")
            if !*authorized {
                return StratumResponse::error_with_code(req.id, StratumErrorCode::UnauthorizedWorker);
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
                    return StratumResponse::error_with_code(req.id, StratumErrorCode::JobNotFound);
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
            let _full_nonce = match compose_nonce(*extranonce1, nonce2) {
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

            // The nonce should be at the correct offset in the work
            if work_bytes.len() >= crate::core::constants::WORK_SIZE {
                // Update the nonce in the work
                work_bytes[crate::core::constants::NONCE_OFFSET..]
                    .copy_from_slice(&submitted_nonce.to_le_bytes());

                // Create new work from the modified bytes
                let mut work_array = [0u8; 286];
                work_array.copy_from_slice(&work_bytes[..286]);
                let modified_work = Work::from_bytes(work_array);

                // Compute the hash for the modified work
                let hash = modified_work.hash();

                // Get session target (or job target if not set)
                let session_target = session.session_target.as_ref().unwrap_or(&job.target);
                
                // Check if share meets session difficulty
                if !session_target.meets_target(&hash.into()) {
                    // Share doesn't meet session difficulty
                    global_monitoring().record_share_submitted(false);
                    return StratumResponse::error_with_code(req.id, StratumErrorCode::LowDifficultyShare);
                }

                // Check if share meets job target (potential block)
                let is_block = job.target.meets_target(&hash.into());

                // Submit the result if we have a channel and it's a potential block
                if is_block {
                    if let Some(ref tx) = *state.result_tx.read().await {
                        let result = MiningResult {
                            work: modified_work,
                            nonce: Nonce::new(submitted_nonce),
                            hash,
                        };

                        if tx.send(result).await.is_err() {
                            // Record share rejected in monitoring
                            global_monitoring().record_share_submitted(false);
                            return StratumResponse::error(req.id, 20, "Failed to submit share");
                        }
                    }
                }

                // Share is valid
                session.shares_valid += 1;

                // Update hash rate and difficulty for dynamic adjustment
                if matches!(state.difficulty_config, StratumDifficulty::Period(_)) {
                    // Need to clone values to avoid holding the write lock
                    let difficulty_config = state.difficulty_config.clone();
                    
                    // Update session target if needed
                    if let Err(e) = update_session_target(
                        &mut session,
                        &job,
                        writer,
                        &difficulty_config,
                    ).await {
                        warn!("Failed to update session target: {}", e);
                    }
                }

                // Record share accepted in monitoring
                global_monitoring().record_share_submitted(true);

                StratumResponse::success(req.id, Value::Bool(true))
            } else {
                StratumResponse::error(req.id, 20, "Invalid work size")
            }
        }

        _ => StratumResponse::error_with_code_and_message(req.id, StratumErrorCode::Other, "Method not supported"),
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

/// Send mining.set_target notification to client
async fn send_set_target(writer: &mut OwnedWriteHalf, target: &Target) -> Result<()> {
    let params = vec![Value::String(target.to_hex())];
    let notify = StratumNotification::new("mining.set_target", params);
    
    let json = serde_json::to_string(&notify)? + "\n";
    writer.write_all(json.as_bytes()).await?;
    writer.flush().await?;
    
    Ok(())
}

/// Get new session target based on difficulty strategy
fn get_new_session_target(
    difficulty_config: &StratumDifficulty,
    current_hash_rate: HashRate,
    current_target: &Target,
    job_target: &Target,
) -> Option<Target> {
    match difficulty_config {
        StratumDifficulty::Block => None, // Use job target
        StratumDifficulty::Fixed(level) => {
            let new_target = Target::mk_target_level(*level);
            if &new_target != current_target {
                Some(new_target)
            } else {
                None
            }
        }
        StratumDifficulty::Period(target_period) => {
            // Calculate new target based on hash rate
            let current_difficulty = Difficulty::from(*current_target);
            let new_difficulty = adjust_difficulty(
                PERIOD_TOLERANCE,
                current_hash_rate,
                Period(*target_period),
                current_difficulty,
            );
            
            let candidate = new_difficulty.to_target().leveled();
            
            // Ensure target is between job target and max session target
            let max_session_target = Target::mk_target_level(MAX_SESSION_TARGET_LEVEL);
            
            // The final target must be harder than max_session_target but easier than job_target
            let new_target = if candidate.meets_target(job_target.as_bytes()) {
                *job_target
            } else if max_session_target.meets_target(candidate.as_bytes()) {
                max_session_target
            } else {
                candidate
            };
            
            if &new_target != current_target {
                Some(new_target)
            } else {
                None
            }
        }
    }
}

/// Update session target after valid share
async fn update_session_target(
    session: &mut StratumSession,
    job: &MiningJob,
    writer: &mut OwnedWriteHalf,
    difficulty_config: &StratumDifficulty,
) -> Result<()> {
    // Update hash rate estimate
    session.update_hash_rate(session.difficulty);
    
    // Check if we need to adjust difficulty
    if let Some(new_target) = get_new_session_target(
        difficulty_config,
        HashRate(session.estimated_hashrate),
        session.session_target.as_ref().unwrap_or(&job.target),
        &job.target,
    ) {
        // Update session target
        session.session_target = Some(new_target);
        session.difficulty = Difficulty::from(new_target).0;
        
        // Send mining.set_target notification
        send_set_target(writer, &new_target).await?;
        
        debug!(
            "Updated session {} difficulty to {} (hashrate: {})",
            session.id, session.difficulty, session.estimated_hashrate
        );
    }
    
    Ok(())
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

        // Create a new server instance for the spawn (without callback since it's already in state)
        let server = Self {
            config: StratumServerConfig {
                port: self.config.port,
                host: self.config.host.clone(),
                max_connections: self.config.max_connections,
                difficulty: self.config.difficulty.clone(),
                rate_ms: self.config.rate_ms,
                authorize_callback: None,
            },
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
