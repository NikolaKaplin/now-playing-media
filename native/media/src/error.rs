use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("the current platform is not supported")]
    UnsupportedPlatform,

    #[error("platform error: {message}")]
    Platform { message: String },

    #[error("media session `{id}` was not found")]
    SessionNotFound { id: String },

    #[error("media operation `{operation}` is not supported")]
    UnsupportedOperation { operation: &'static str },

    #[error("media backend `{backend}` is unavailable")]
    BackendUnavailable { backend: &'static str },
}

pub type Result<T> = std::result::Result<T, Error>;

impl Error {
    pub(crate) fn platform(error: impl std::fmt::Display) -> Self {
        Self::Platform {
            message: error.to_string(),
        }
    }
}
