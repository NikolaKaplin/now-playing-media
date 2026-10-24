use napi::{Error, Result};
use napi_derive::napi;
use now_playing_service::{MediaService, ServiceError};

static SERVICE: MediaService = MediaService::new();

// Rejections carry `CODE: message`; the JS layer turns them into NowPlayingError.
fn coded(error: ServiceError) -> Error {
    Error::from_reason(format!("{}: {}", error.code, error.message))
}

fn json(value: serde_json::Value) -> Result<String> {
    serde_json::to_string(&value).map_err(|error| Error::from_reason(error.to_string()))
}

#[napi]
pub async fn current() -> Result<Option<String>> {
    let session = SERVICE.current().await.map_err(coded)?;
    if session.is_null() {
        Ok(None)
    } else {
        json(session).map(Some)
    }
}

#[napi]
pub async fn sessions() -> Result<String> {
    json(SERVICE.sessions().await.map_err(coded)?)
}

/// Empty id selects the current player at invocation time.
#[napi]
pub async fn control(command: String, session_id: String, position_ms: Option<f64>) -> Result<()> {
    SERVICE
        .control(&command, &session_id, position_ms)
        .await
        .map_err(coded)
}
