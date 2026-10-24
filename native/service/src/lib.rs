//! The operations both front ends expose: the Node-API addon and the sidecar.
//! Errors carry a stable code and a message free of player metadata.

mod validation;

pub use validation::validate_control;

use nowplaying::MediaController;
use serde_json::Value;
use tokio::sync::OnceCell;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServiceError {
    pub code: &'static str,
    pub message: &'static str,
}

impl ServiceError {
    pub const fn new(code: &'static str, message: &'static str) -> Self {
        Self { code, message }
    }
}

pub type Result<T> = std::result::Result<T, ServiceError>;

/// Owns the platform controller, created on first use.
#[derive(Default)]
pub struct MediaService {
    controller: OnceCell<MediaController>,
}

impl MediaService {
    pub const fn new() -> Self {
        Self {
            controller: OnceCell::const_new(),
        }
    }

    async fn controller(&self) -> Result<&MediaController> {
        self.controller
            .get_or_try_init(|| async { MediaController::new().await.map_err(media_error) })
            .await
    }

    /// The current session as JSON, or `null`.
    pub async fn current(&self) -> Result<Value> {
        let session = self
            .controller()
            .await?
            .current()
            .await
            .map_err(media_error)?;
        serde_json::to_value(session).map_err(|_| serialization_error())
    }

    /// Every session as a JSON array.
    pub async fn sessions(&self) -> Result<Value> {
        let sessions = self
            .controller()
            .await?
            .sessions()
            .await
            .map_err(media_error)?;
        serde_json::to_value(sessions).map_err(|_| serialization_error())
    }

    /// An empty `session_id` selects the current player at invocation time.
    pub async fn control(
        &self,
        command: &str,
        session_id: &str,
        position_ms: Option<f64>,
    ) -> Result<()> {
        validate_control(command, session_id, position_ms)
            .map_err(|message| ServiceError::new("INVALID_ARGUMENT", message))?;
        let controller = self.controller().await?;
        let id = if session_id.is_empty() {
            controller
                .current()
                .await
                .map_err(media_error)?
                .map(|session| session.id)
                .ok_or(ServiceError::new(
                    "NO_SESSION",
                    "No media session is active.",
                ))?
        } else {
            session_id.to_owned()
        };
        let result = match command {
            "play" => controller.play(&id).await,
            "pause" => controller.pause(&id).await,
            "toggle" => controller.toggle_play_pause(&id).await,
            "next" => controller.next(&id).await,
            "previous" => controller.previous(&id).await,
            // validate_control admits only these and "seek".
            _ => {
                controller
                    .seek(&id, position_ms.unwrap_or_default() as u64)
                    .await
            }
        };
        result.map_err(media_error)
    }
}

// Platform messages can contain player metadata, so only the error kind is reported.
fn media_error(error: nowplaying::Error) -> ServiceError {
    match error {
        nowplaying::Error::UnsupportedPlatform => {
            ServiceError::new("UNSUPPORTED_PLATFORM", "This platform is not supported.")
        }
        nowplaying::Error::BackendUnavailable { .. } => ServiceError::new(
            "BACKEND_UNAVAILABLE",
            "The system media service is unavailable.",
        ),
        nowplaying::Error::SessionNotFound { .. } => {
            ServiceError::new("SESSION_NOT_FOUND", "The media session no longer exists.")
        }
        nowplaying::Error::UnsupportedOperation { .. } => ServiceError::new(
            "UNSUPPORTED_OPERATION",
            "The player does not support this command.",
        ),
        nowplaying::Error::Platform { .. } => {
            ServiceError::new("PLATFORM_ERROR", "The system media operation failed.")
        }
    }
}

fn serialization_error() -> ServiceError {
    ServiceError::new("PLATFORM_ERROR", "Could not serialize the media session.")
}

#[cfg(test)]
#[path = "tests/validation.rs"]
mod tests;
