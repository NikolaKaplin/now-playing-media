use crate::validation::validate_control;
use napi::{Error, Result};
use napi_derive::napi;
use nowplaying::MediaController;
use tokio::sync::OnceCell;

static CONTROLLER: OnceCell<MediaController> = OnceCell::const_new();

// Rejections carry `CODE: message`; the JS layer turns them into NowPlayingError.
fn coded(code: &str, message: &str) -> Error {
    Error::from_reason(format!("{code}: {message}"))
}

async fn controller() -> Result<&'static MediaController> {
    CONTROLLER
        .get_or_try_init(|| async { MediaController::new().await.map_err(media_error) })
        .await
}

#[napi]
pub async fn current() -> Result<Option<String>> {
    let session = controller().await?.current().await.map_err(media_error)?;
    session
        .map(|session| serde_json::to_string(&session).map_err(|_| serialization_error()))
        .transpose()
}

#[napi]
pub async fn sessions() -> Result<String> {
    let sessions = controller().await?.sessions().await.map_err(media_error)?;
    serde_json::to_string(&sessions).map_err(|_| serialization_error())
}

/// Empty id selects the current player at invocation time.
#[napi]
pub async fn control(command: String, session_id: String, position_ms: Option<f64>) -> Result<()> {
    validate_control(&command, &session_id, position_ms)
        .map_err(|message| coded("INVALID_ARGUMENT", message))?;
    let controller = controller().await?;
    let id = if session_id.is_empty() {
        controller
            .current()
            .await
            .map_err(media_error)?
            .map(|session| session.id)
            .ok_or_else(|| coded("NO_SESSION", "No media session is active."))?
    } else {
        session_id
    };
    let result = match command.as_str() {
        "play" => controller.play(&id).await,
        "pause" => controller.pause(&id).await,
        "toggle" => controller.toggle_play_pause(&id).await,
        "next" => controller.next(&id).await,
        "previous" => controller.previous(&id).await,
        "seek" => {
            controller
                .seek(&id, position_ms.unwrap_or_default() as u64)
                .await
        }
        _ => unreachable!("validated above"),
    };
    result.map_err(media_error)
}

// Platform messages can contain player metadata, so only the error kind is reported.
fn media_error(error: nowplaying::Error) -> Error {
    match error {
        nowplaying::Error::UnsupportedPlatform => {
            coded("UNSUPPORTED_PLATFORM", "This platform is not supported.")
        }
        nowplaying::Error::BackendUnavailable { .. } => coded(
            "BACKEND_UNAVAILABLE",
            "The system media service is unavailable.",
        ),
        nowplaying::Error::SessionNotFound { .. } => {
            coded("SESSION_NOT_FOUND", "The media session no longer exists.")
        }
        nowplaying::Error::UnsupportedOperation { .. } => coded(
            "UNSUPPORTED_OPERATION",
            "The player does not support this command.",
        ),
        nowplaying::Error::Platform { .. } => {
            coded("PLATFORM_ERROR", "The system media operation failed.")
        }
    }
}

fn serialization_error() -> Error {
    coded("PLATFORM_ERROR", "Could not serialize the media session.")
}
