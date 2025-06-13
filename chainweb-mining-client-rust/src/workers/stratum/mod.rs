//! Stratum protocol server implementation for ASIC miners

mod protocol;
mod server;
mod session;

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
