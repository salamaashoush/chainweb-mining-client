//! Stratum protocol message definitions

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Stratum protocol methods
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StratumMethod {
    /// Client subscribes to mining notifications
    Subscribe,
    /// Client authorizes with credentials
    Authorize,
    /// Server notifies client of new work
    Notify,
    /// Client submits a share
    Submit,
    /// Server sets difficulty/target
    SetTarget,
    /// Server sets extra nonce
    SetExtranonce,
    /// Client requests version
    GetVersion,
    /// Unknown method
    Unknown(String),
}

impl StratumMethod {
    /// Parse method from string
    pub fn parse_method(s: &str) -> Self {
        match s {
            "mining.subscribe" => Self::Subscribe,
            "mining.authorize" => Self::Authorize,
            "mining.notify" => Self::Notify,
            "mining.submit" => Self::Submit,
            "mining.set_target" => Self::SetTarget,
            "mining.set_extranonce" => Self::SetExtranonce,
            "mining.get_version" => Self::GetVersion,
            _ => Self::Unknown(s.to_string()),
        }
    }

    /// Convert to string representation
    pub fn as_str(&self) -> &str {
        match self {
            Self::Subscribe => "mining.subscribe",
            Self::Authorize => "mining.authorize",
            Self::Notify => "mining.notify",
            Self::Submit => "mining.submit",
            Self::SetTarget => "mining.set_target",
            Self::SetExtranonce => "mining.set_extranonce",
            Self::GetVersion => "mining.get_version",
            Self::Unknown(s) => s,
        }
    }
}

/// Stratum request message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StratumRequest {
    /// Request ID
    pub id: Value,
    /// Method name
    pub method: String,
    /// Method parameters
    pub params: Vec<Value>,
}

impl StratumRequest {
    /// Create a new request
    pub fn new(id: impl Into<Value>, method: &str, params: Vec<Value>) -> Self {
        Self {
            id: id.into(),
            method: method.to_string(),
            params,
        }
    }

    /// Get the method as enum
    pub fn method_enum(&self) -> StratumMethod {
        StratumMethod::parse_method(&self.method)
    }
}

/// Stratum response message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StratumResponse {
    /// Request ID this responds to
    pub id: Value,
    /// Result if successful
    pub result: Option<Value>,
    /// Error if failed
    pub error: Option<Value>,
}

impl StratumResponse {
    /// Create a successful response
    pub fn success(id: Value, result: Value) -> Self {
        Self {
            id,
            result: Some(result),
            error: None,
        }
    }

    /// Create an error response
    pub fn error(id: Value, code: i32, message: &str) -> Self {
        Self {
            id,
            result: None,
            error: Some(Value::Array(vec![
                Value::Number(code.into()),
                Value::String(message.to_string()),
                Value::Null,
            ])),
        }
    }
}

/// Stratum notification (no ID)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StratumNotification {
    /// Method name
    pub method: String,
    /// Method parameters
    pub params: Vec<Value>,
    /// Always null for notifications
    pub id: Value,
}

impl StratumNotification {
    /// Create a new notification
    pub fn new(method: &str, params: Vec<Value>) -> Self {
        Self {
            method: method.to_string(),
            params,
            id: Value::Null,
        }
    }
}

/// Generic Stratum message
#[derive(Debug, Clone)]
pub enum StratumMessage {
    /// Request from client
    Request(StratumRequest),
    /// Response to request
    Response(StratumResponse),
    /// Notification (no response expected)
    Notification(StratumNotification),
}

impl StratumMessage {
    /// Parse a JSON string into a Stratum message
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        let value: Value = serde_json::from_str(json)?;

        // Check if it has a method field (request or notification)
        if value.get("method").is_some() {
            // Check if it has null ID (notification)
            if value.get("id") == Some(&Value::Null) {
                let notification: StratumNotification = serde_json::from_value(value)?;
                Ok(StratumMessage::Notification(notification))
            } else {
                let request: StratumRequest = serde_json::from_value(value)?;
                Ok(StratumMessage::Request(request))
            }
        } else {
            // It's a response
            let response: StratumResponse = serde_json::from_value(value)?;
            Ok(StratumMessage::Response(response))
        }
    }

    /// Convert to JSON string
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        match self {
            StratumMessage::Request(req) => serde_json::to_string(req),
            StratumMessage::Response(resp) => serde_json::to_string(resp),
            StratumMessage::Notification(notif) => serde_json::to_string(notif),
        }
    }
}

/// Job parameters for mining.notify
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobParams {
    /// Job ID
    pub job_id: String,
    /// Previous block hash
    pub prevhash: String,
    /// Coinbase part 1
    pub coinb1: String,
    /// Coinbase part 2
    pub coinb2: String,
    /// Merkle branches
    pub merkle_branch: Vec<String>,
    /// Block version
    pub version: String,
    /// nBits (target)
    pub nbits: String,
    /// nTime
    pub ntime: String,
    /// Clean jobs flag
    pub clean_jobs: bool,
}

impl JobParams {
    /// Convert to params array for notification
    #[allow(dead_code)]
    pub fn to_params(&self) -> Vec<Value> {
        vec![
            Value::String(self.job_id.clone()),
            Value::String(self.prevhash.clone()),
            Value::String(self.coinb1.clone()),
            Value::String(self.coinb2.clone()),
            Value::Array(
                self.merkle_branch
                    .iter()
                    .map(|s| Value::String(s.clone()))
                    .collect(),
            ),
            Value::String(self.version.clone()),
            Value::String(self.nbits.clone()),
            Value::String(self.ntime.clone()),
            Value::Bool(self.clean_jobs),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stratum_method() {
        assert_eq!(
            StratumMethod::parse_method("mining.subscribe"),
            StratumMethod::Subscribe
        );
        assert_eq!(StratumMethod::Subscribe.as_str(), "mining.subscribe");

        let unknown = StratumMethod::parse_method("custom.method");
        assert!(matches!(unknown, StratumMethod::Unknown(_)));
    }

    #[test]
    fn test_stratum_request() {
        let req = StratumRequest::new(
            1,
            "mining.subscribe",
            vec![Value::String("miner/1.0".to_string())],
        );

        assert_eq!(req.id, Value::Number(1.into()));
        assert_eq!(req.method, "mining.subscribe");
        assert_eq!(req.method_enum(), StratumMethod::Subscribe);
    }

    #[test]
    fn test_stratum_response() {
        let success = StratumResponse::success(Value::Number(1.into()), Value::Bool(true));
        assert!(success.result.is_some());
        assert!(success.error.is_none());

        let error = StratumResponse::error(Value::Number(2.into()), 20, "Invalid params");
        assert!(error.result.is_none());
        assert!(error.error.is_some());
    }

    #[test]
    fn test_stratum_message_parsing() {
        // Test request parsing
        let req_json = r#"{"id":1,"method":"mining.subscribe","params":[]}"#;
        let msg = StratumMessage::from_json(req_json).unwrap();
        assert!(matches!(msg, StratumMessage::Request(_)));

        // Test response parsing
        let resp_json = r#"{"id":1,"result":true,"error":null}"#;
        let msg = StratumMessage::from_json(resp_json).unwrap();
        assert!(matches!(msg, StratumMessage::Response(_)));

        // Test notification parsing
        let notif_json = r#"{"id":null,"method":"mining.notify","params":[]}"#;
        let msg = StratumMessage::from_json(notif_json).unwrap();
        assert!(matches!(msg, StratumMessage::Notification(_)));
    }

    #[test]
    fn test_job_params() {
        let job = JobParams {
            job_id: "123".to_string(),
            prevhash: "00000000".to_string(),
            coinb1: "01000000".to_string(),
            coinb2: "ffffffff".to_string(),
            merkle_branch: vec!["aabbccdd".to_string()],
            version: "20000000".to_string(),
            nbits: "1a2b3c4d".to_string(),
            ntime: "5a1b2c3d".to_string(),
            clean_jobs: true,
        };

        let params = job.to_params();
        assert_eq!(params.len(), 9);
        assert_eq!(params[0], Value::String("123".to_string()));
        assert_eq!(params[8], Value::Bool(true));
    }
}
