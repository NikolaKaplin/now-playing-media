use now_playing_service::{MediaService, ServiceError};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

pub const PROTOCOL_VERSION: u32 = 1;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub id: u64,
    pub method: String,
    #[serde(default)]
    pub params: Map<String, Value>,
}

#[derive(Debug, PartialEq, Serialize)]
pub struct Response {
    pub id: Option<u64>,
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ErrorBody>,
}

#[derive(Debug, PartialEq, Serialize)]
pub struct ErrorBody {
    pub code: &'static str,
    pub message: &'static str,
}

impl Response {
    pub fn value(id: u64, value: Value) -> Self {
        Self {
            id: Some(id),
            ok: true,
            value: Some(value),
            error: None,
        }
    }

    pub fn error(id: Option<u64>, code: &'static str, message: &'static str) -> Self {
        Self {
            id,
            ok: false,
            value: None,
            error: Some(ErrorBody { code, message }),
        }
    }
}

/// Parses one line; a malformed one is answered without an id.
pub fn parse(line: &[u8]) -> Result<Request, Response> {
    serde_json::from_slice(line)
        .map_err(|_| Response::error(None, "INVALID_ARGUMENT", "The request is malformed."))
}

pub async fn handle(
    service: &MediaService,
    id: u64,
    method: &str,
    params: &Map<String, Value>,
) -> Response {
    let result = match method {
        "hello" => Ok(serde_json::json!({
            "protocol": PROTOCOL_VERSION,
            "name": "now-playing-media",
            "version": env!("CARGO_PKG_VERSION"),
        })),
        "current" => service.current().await,
        "sessions" => service.sessions().await,
        "control" => control(service, params).await.map(|()| Value::Null),
        _ => Err(ServiceError::new("INVALID_ARGUMENT", "Unknown method.")),
    };
    match result {
        Ok(value) => Response::value(id, value),
        Err(error) => Response::error(Some(id), error.code, error.message),
    }
}

async fn control(service: &MediaService, params: &Map<String, Value>) -> Result<(), ServiceError> {
    let invalid = ServiceError::new("INVALID_ARGUMENT", "Invalid control parameters.");
    let command = params
        .get("command")
        .and_then(Value::as_str)
        .ok_or(invalid.clone())?;
    let session_id = match params.get("sessionId") {
        None | Some(Value::Null) => "",
        Some(value) => value.as_str().ok_or(invalid.clone())?,
    };
    let position_ms = match params.get("positionMs") {
        None | Some(Value::Null) => None,
        Some(value) => Some(value.as_f64().ok_or(invalid)?),
    };
    service.control(command, session_id, position_ms).await
}

#[cfg(test)]
#[path = "tests/protocol.rs"]
mod tests;
