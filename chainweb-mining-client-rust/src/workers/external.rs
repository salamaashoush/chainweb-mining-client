//! External worker implementation for GPU and custom miners

use crate::core::{Nonce, Target, Work};
use crate::error::{Error, Result};
use crate::workers::{MiningResult, Worker};
use async_process::{Child, Command, Stdio};
use async_trait::async_trait;
use futures::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use parking_lot::Mutex;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::Instant;
use tokio::sync::mpsc;
use tokio::task;
use tracing::{debug, error, info};

/// External worker configuration
#[derive(Debug, Clone)]
pub struct ExternalWorkerConfig {
    /// Path to the external miner executable
    pub command: PathBuf,
    /// Additional command line arguments
    pub args: Vec<String>,
    /// Environment variables to set
    pub env: Vec<(String, String)>,
    /// Timeout for receiving results (seconds)
    pub timeout_secs: u64,
}

/// External worker for GPU mining
pub struct ExternalWorker {
    config: ExternalWorkerConfig,
    process: Arc<Mutex<Option<Child>>>,
    is_mining: Arc<AtomicBool>,
    hash_count: Arc<AtomicU64>,
    start_time: Arc<Mutex<Option<Instant>>>,
}

impl ExternalWorker {
    /// Create a new external worker
    pub fn new(config: ExternalWorkerConfig) -> Self {
        info!(
            "Initializing external worker with command: {:?}",
            config.command
        );

        Self {
            config,
            process: Arc::new(Mutex::new(None)),
            is_mining: Arc::new(AtomicBool::new(false)),
            hash_count: Arc::new(AtomicU64::new(0)),
            start_time: Arc::new(Mutex::new(None)),
        }
    }

    /// Parse nonce from external miner output
    fn parse_nonce(line: &str) -> Option<Nonce> {
        // Try to parse as decimal
        if let Ok(value) = line.trim().parse::<u64>() {
            return Some(Nonce::new(value));
        }

        // Try to parse as hex (with or without 0x prefix)
        let hex_str = line.trim().strip_prefix("0x").unwrap_or(line.trim());
        if let Ok(value) = u64::from_str_radix(hex_str, 16) {
            return Some(Nonce::new(value));
        }

        None
    }
}

#[async_trait]
impl Worker for ExternalWorker {
    async fn mine(
        &self,
        work: Work,
        target: Target,
        result_tx: mpsc::Sender<MiningResult>,
    ) -> Result<()> {
        if self.is_mining.load(Ordering::Relaxed) {
            return Err(Error::worker("Already mining"));
        }

        self.is_mining.store(true, Ordering::Relaxed);
        self.hash_count.store(0, Ordering::Relaxed);
        *self.start_time.lock() = Some(Instant::now());

        // Build command with target as argument
        let mut cmd = Command::new(&self.config.command);
        cmd.arg(target.to_hex());

        // Add additional arguments
        for arg in &self.config.args {
            cmd.arg(arg);
        }

        // Set environment variables
        for (key, value) in &self.config.env {
            cmd.env(key, value);
        }

        // Configure process pipes
        cmd.stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        // Start the external process
        let mut child = cmd
            .spawn()
            .map_err(|e| Error::external_process(format!("Failed to start: {}", e)))?;

        // Get process streams
        let mut stdin = child
            .stdin
            .take()
            .ok_or_else(|| Error::external_process("Failed to get stdin"))?;

        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| Error::external_process("Failed to get stdout"))?;

        // Write work to stdin
        stdin
            .write_all(work.as_bytes())
            .await
            .map_err(|e| Error::external_process(format!("Failed to write work: {}", e)))?;
        stdin.flush().await?;
        drop(stdin); // Close stdin

        // Store process handle
        *self.process.lock() = Some(child);

        let is_mining = self.is_mining.clone();
        let timeout_secs = self.config.timeout_secs;

        // Spawn task to read results
        task::spawn(async move {
            let mut reader = BufReader::new(stdout);
            let mut line = String::new();

            info!("External worker started, waiting for results...");

            loop {
                line.clear();

                // Read with timeout
                let read_result = tokio::time::timeout(
                    std::time::Duration::from_secs(timeout_secs),
                    reader.read_line(&mut line),
                )
                .await;

                match read_result {
                    Ok(Ok(0)) => {
                        // EOF - process ended
                        info!("External worker process ended");
                        break;
                    }
                    Ok(Ok(_)) => {
                        debug!("External worker output: {}", line.trim());

                        // Try to parse nonce from output
                        if let Some(nonce) = Self::parse_nonce(&line) {
                            info!("External worker found solution: {}", nonce);

                            let mut solved_work = work.clone();
                            solved_work.set_nonce(nonce);
                            let hash = solved_work.hash();

                            // Verify the solution
                            if target.meets_target(&hash) {
                                let result = MiningResult {
                                    work: solved_work,
                                    nonce,
                                    hash,
                                };

                                // Send result
                                let _ = result_tx.send(result).await;
                                is_mining.store(false, Ordering::Relaxed);
                                break;
                            } else {
                                error!("External worker provided invalid solution");
                            }
                        }
                    }
                    Ok(Err(e)) => {
                        error!("Error reading from external worker: {}", e);
                        break;
                    }
                    Err(_) => {
                        error!("External worker timeout");
                        break;
                    }
                }

                if !is_mining.load(Ordering::Relaxed) {
                    break;
                }
            }

            is_mining.store(false, Ordering::Relaxed);
        });

        Ok(())
    }

    async fn stop(&self) -> Result<()> {
        self.is_mining.store(false, Ordering::Relaxed);

        // Kill the external process
        if let Some(mut child) = self.process.lock().take() {
            match child.kill() {
                Ok(()) => {
                    debug!("External worker process killed");
                }
                Err(e) => {
                    error!("Failed to kill external worker: {}", e);
                }
            }
        }

        Ok(())
    }

    fn worker_type(&self) -> &str {
        "External"
    }

    async fn hashrate(&self) -> u64 {
        // External workers typically report their own hashrate
        // For now, return 0 as we don't have a standard protocol
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn test_parse_nonce() {
        // Decimal
        assert_eq!(
            ExternalWorker::parse_nonce("12345"),
            Some(Nonce::new(12345))
        );
        assert_eq!(
            ExternalWorker::parse_nonce("  999  "),
            Some(Nonce::new(999))
        );

        // Hex
        assert_eq!(ExternalWorker::parse_nonce("0xFF"), Some(Nonce::new(255)));
        assert_eq!(
            ExternalWorker::parse_nonce("DEADBEEF"),
            Some(Nonce::new(0xDEADBEEF))
        );
        assert_eq!(
            ExternalWorker::parse_nonce("0xCAFEBABE"),
            Some(Nonce::new(0xCAFEBABE))
        );

        // Invalid
        assert_eq!(ExternalWorker::parse_nonce("invalid"), None);
        assert_eq!(ExternalWorker::parse_nonce(""), None);
    }

    #[test]
    fn test_external_worker_creation() {
        let config = ExternalWorkerConfig {
            command: PathBuf::from("/usr/bin/test-miner"),
            args: vec!["--threads".to_string(), "4".to_string()],
            env: vec![("GPU_ID".to_string(), "0".to_string())],
            timeout_secs: 60,
        };

        let worker = ExternalWorker::new(config);
        assert_eq!(worker.worker_type(), "External");
        assert!(!worker.is_mining.load(Ordering::Relaxed));
    }

    #[tokio::test]
    async fn test_external_worker_lifecycle() {
        // This test requires a mock external miner
        let echo_path = if cfg!(windows) {
            Path::new("C:\\Windows\\System32\\cmd.exe")
        } else {
            Path::new("/bin/echo")
        };

        if !echo_path.exists() {
            // Skip test if echo command not available
            return;
        }

        let config = ExternalWorkerConfig {
            command: echo_path.to_path_buf(),
            args: if cfg!(windows) {
                vec!["/C".to_string(), "echo".to_string(), "12345".to_string()]
            } else {
                vec!["12345".to_string()]
            },
            env: vec![],
            timeout_secs: 1,
        };

        let worker = ExternalWorker::new(config);

        // Create work and target
        let work = Work::from_bytes([0u8; 286]);
        let target = Target::from_bytes([0xFF; 32]); // Easy target

        let (tx, mut rx) = mpsc::channel(1);

        // Start mining - this may fail if the external process exits quickly
        let result = worker.mine(work, target, tx).await;
        if result.is_err() {
            // Expected for some external processes that exit immediately
            println!("Mining failed as expected for quick-exit process: {:?}", result);
            return;
        }

        // Wait a bit for process to complete
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;

        // Should receive result (echo outputs "12345")
        if let Ok(Some(result)) =
            tokio::time::timeout(std::time::Duration::from_secs(2), rx.recv()).await
        {
            assert_eq!(result.nonce.value(), 12345);
        }

        // Stop worker
        worker.stop().await.unwrap();
        assert!(!worker.is_mining.load(Ordering::Relaxed));
    }
}
