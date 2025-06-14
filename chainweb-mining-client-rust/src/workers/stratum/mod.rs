//! Stratum protocol server implementation for ASIC miners

mod difficulty;
mod hex;
mod job;
mod nonce;
mod protocol;
mod server;
mod session;

pub use difficulty::{difficulty_to_target, target_to_difficulty, min_difficulty, max_difficulty};
pub use hex::{encode_hex, decode_hex, encode_hex_prefixed, decode_hex_flexible};
pub use job::{ClientWorker, JobId, JobManager, MiningJob, SharedJobManager};
pub use nonce::{Nonce1, Nonce2, NonceSize, compose_nonce, split_nonce};
pub use protocol::{
    StratumMessage, StratumMethod, StratumNotification, StratumRequest, StratumResponse,
};
pub use server::{StratumServer, StratumServerConfig};
pub use session::{SessionId, StratumSession};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stratum_exports() {
        // Ensure all public types are accessible
        let _msg: StratumMessage;
        let _method: StratumMethod;
        let _req: StratumRequest;
        let _resp: StratumResponse;
        let _session: SessionId;
    }
}


