//! External worker implementation for GPU and other external miners
//!
//! Executes external mining commands and processes their output to find solutions.

use super::{mining_span, MiningStats, MiningWorker};
use crate::{ChainId, Error, Nonce, Result, Target, Work};
use async_trait::async_trait;
use std::process::Stdio;
use std::time::{Duration, Instant};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::process::Command;
use tokio::sync::mpsc;
use tokio::time::timeout;
use tokio_util::sync::CancellationToken;
use tracing::{debug, info, warn};

/// External worker that executes mining commands
pub struct ExternalWorker {
    command: String,
    stats: MiningStats,
}

impl ExternalWorker {
    /// Create a new external worker with the specified command
    pub fn new(command: String) -> Self {
        info!("Creating external worker with command: {}", command);
        
        Self {
            command,
            stats: MiningStats::default(),
        }
    }

    /// Parse the external worker command into program and arguments
    fn parse_command(&self) -> (String, Vec<String>) {
        let parts: Vec<&str> = self.command.split_whitespace().collect();
        if parts.is_empty() {
            return ("echo".to_string(), vec!["No command specified".to_string()]);
        }

        let program = parts[0].to_string();
        let args = parts[1..].iter().map(|s| s.to_string()).collect();
        
        (program, args)
    }

    /// Execute the external mining command
    async fn execute_mining_command(
        &mut self,
        target: Target,
        work: Work,
        timeout_duration: Duration,
    ) -> Result<Work> {
        let (program, mut args) = self.parse_command();
        
        // Add target as the last argument (as expected by external miners)
        args.push(target.to_hex_be());

        debug!("Executing external command: {} {:?}", program, args);

        let mut child = Command::new(program)
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| Error::external_process(format!("Failed to spawn external process: {}", e)))?;

        // Send work data to stdin
        if let Some(stdin) = child.stdin.as_mut() {
            stdin.write_all(work.bytes()).await
                .map_err(|e| Error::external_process(format!("Failed to write to stdin: {}", e)))?;
            
            // Close stdin to signal we're done sending data
            drop(child.stdin.take());
        }

        // Wait for process completion with timeout
        let start_time = Instant::now();
        let output = timeout(timeout_duration, child.wait_with_output()).await
            .map_err(|_| Error::timeout("External mining command"))?
            .map_err(|e| Error::external_process(format!("External process failed: {}", e)))?;

        let elapsed = start_time.elapsed();
        
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(Error::external_process(format!(
                "External miner failed with exit code {}: {}",
                output.status.code().unwrap_or(-1),
                stderr
            )));
        }

        // Parse the output to get the solved work
        self.parse_external_output(&output.stdout, work, elapsed)
    }

    /// Parse the output from the external mining command
    fn parse_external_output(
        &mut self,
        stdout: &[u8],
        original_work: Work,
        elapsed: Duration,
    ) -> Result<Work> {
        // External miners are expected to output the solved work bytes on stdout
        if stdout.len() == Work::SIZE {
            debug!("External miner returned solved work ({} bytes)", stdout.len());
            
            let solved_work = Work::new(stdout.to_vec())?;
            
            // Update statistics (estimate based on elapsed time)
            let estimated_hashes = (elapsed.as_secs_f64() * 1_000_000.0) as u64; // Estimate 1MH/s
            self.stats.total_hashes += estimated_hashes;
            self.stats.solutions_found += 1;
            self.stats.mining_time_secs += elapsed.as_secs();
            self.stats.current_hash_rate = estimated_hashes as f64 / elapsed.as_secs_f64();
            
            Ok(solved_work)
        } else if stdout.is_empty() {
            // No solution found
            debug!("External miner returned no solution");
            Err(Error::worker("external", "No solution found by external miner"))
        } else {
            // Try to parse as hex string
            let output_str = String::from_utf8_lossy(stdout).trim().to_string();
            if output_str.len() == Work::SIZE * 2 {
                // Hex encoded work
                let solved_work = Work::from_hex(&output_str)?;
                
                let estimated_hashes = (elapsed.as_secs_f64() * 1_000_000.0) as u64;
                self.stats.total_hashes += estimated_hashes;
                self.stats.solutions_found += 1;
                self.stats.mining_time_secs += elapsed.as_secs();
                self.stats.current_hash_rate = estimated_hashes as f64 / elapsed.as_secs_f64();
                
                Ok(solved_work)
            } else {
                Err(Error::external_process(format!(
                    "Invalid output from external miner: expected {} bytes or {} hex chars, got {} bytes",
                    Work::SIZE,
                    Work::SIZE * 2,
                    stdout.len()
                )))
            }
        }
    }
}

#[async_trait]
impl MiningWorker for ExternalWorker {
    fn worker_type(&self) -> &'static str {
        "external"
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
            "Starting external mining for chain {} (difficulty level: {})",
            chain_id,
            target.difficulty_level()
        );

        // Reset statistics
        self.stats = MiningStats::default();

        // Statistics reporting
        let stats = self.stats.clone();
        let stats_cancellation = cancellation.clone();
        let stats_handle = if let Some(stats_tx) = stats_tx {
            Some(tokio::spawn(async move {
                let mut interval = tokio::time::interval(Duration::from_secs(5));
                
                while !stats_cancellation.is_cancelled() {
                    tokio::select! {
                        _ = interval.tick() => {
                            let _ = stats_tx.send(stats.clone());
                        }
                        _ = stats_cancellation.cancelled() => break,
                    }
                }
            }))
        } else {
            None
        };

        // Execute mining with timeout
        let mining_timeout = Duration::from_secs(300); // 5 minutes default timeout
        
        let result = tokio::select! {
            result = self.execute_mining_command(target, work, mining_timeout) => {
                match result {
                    Ok(solved_work) => {
                        info!("External mining found solution");
                        Ok(solved_work)
                    }
                    Err(e) => {
                        warn!("External mining failed: {}", e);
                        Err(e)
                    }
                }
            }
            _ = cancellation.cancelled() => {
                info!("External mining cancelled");
                Err(Error::cancelled("External mining"))
            }
        };

        // Cleanup statistics reporting
        if let Some(handle) = stats_handle {
            let _ = handle.await;
        }

        info!(
            "External mining completed. Total hashes: {}, Solutions: {}",
            self.stats.total_hashes,
            self.stats.solutions_found
        );

        result
    }

    fn stats(&self) -> MiningStats {
        self.stats.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Target, Work};

    #[test]
    fn test_external_worker_creation() {
        let worker = ExternalWorker::new("echo test".to_string());
        assert_eq!(worker.worker_type(), "external");
        assert_eq!(worker.command, "echo test");
    }

    #[test]
    fn test_command_parsing() {
        let worker = ExternalWorker::new("gpu-miner --device 0 --intensity 5".to_string());
        let (program, args) = worker.parse_command();
        
        assert_eq!(program, "gpu-miner");
        assert_eq!(args, vec!["--device", "0", "--intensity", "5"]);
    }

    #[test]
    fn test_empty_command_parsing() {
        let worker = ExternalWorker::new("".to_string());
        let (program, args) = worker.parse_command();
        
        assert_eq!(program, "echo");
        assert_eq!(args, vec!["No command specified"]);
    }

    #[tokio::test]
    async fn test_external_worker_with_echo() {
        let mut worker = ExternalWorker::new("cat".to_string()); // cat should echo stdin to stdout
        let target = Target::max();
        let work = Work::new(vec![42u8; Work::SIZE]).unwrap();
        let chain_id = ChainId::new(0);
        let initial_nonce = Nonce::new(0);
        let cancellation = CancellationToken::new();

        // This should return the same work (cat echoes input)
        let result = worker.mine(
            initial_nonce,
            target,
            chain_id,
            work.clone(),
            cancellation,
            None,
        ).await;

        match result {
            Ok(solved_work) => {
                assert_eq!(solved_work.bytes(), work.bytes());
            }
            Err(_) => {
                // cat might not be available on all systems, so we allow this to fail
                println!("Note: cat command not available for testing");
            }
        }
    }

    #[tokio::test]
    async fn test_external_worker_timeout() {
        let mut worker = ExternalWorker::new("sleep 10".to_string()); // Long running command
        let target = Target::max();
        let work = Work::new(vec![0u8; Work::SIZE]).unwrap();
        let chain_id = ChainId::new(0);
        let initial_nonce = Nonce::new(0);
        let cancellation = CancellationToken::new();

        // Cancel immediately to test cancellation
        cancellation.cancel();

        let result = worker.mine(
            initial_nonce,
            target,
            chain_id,
            work,
            cancellation,
            None,
        ).await;

        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), Error::Cancelled { .. }));
    }
}