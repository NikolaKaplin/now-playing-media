#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(windows)]
mod windows;

#[cfg(target_os = "linux")]
pub(crate) use linux::PlatformController;
#[cfg(target_os = "macos")]
pub(crate) use macos::PlatformController;
#[cfg(windows)]
pub(crate) use windows::PlatformController;

#[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
mod unsupported {
    use crate::{Error, MediaSession, Result};

    pub(crate) struct PlatformController;

    impl PlatformController {
        pub(crate) async fn new() -> Result<Self> {
            Err(Error::UnsupportedPlatform)
        }

        pub(crate) async fn sessions(&self) -> Result<Vec<MediaSession>> {
            Err(Error::UnsupportedPlatform)
        }

        pub(crate) fn subscribe_changes(&self) -> Option<tokio::sync::broadcast::Receiver<()>> {
            None
        }

        pub(crate) async fn current_session_id(
            &self,
            _sessions: &[MediaSession],
        ) -> Result<Option<String>> {
            Err(Error::UnsupportedPlatform)
        }

        pub(crate) async fn play(&self, _id: &str) -> Result<()> {
            Err(Error::UnsupportedPlatform)
        }

        pub(crate) async fn pause(&self, _id: &str) -> Result<()> {
            Err(Error::UnsupportedPlatform)
        }

        pub(crate) async fn toggle_play_pause(&self, _id: &str) -> Result<()> {
            Err(Error::UnsupportedPlatform)
        }

        pub(crate) async fn next(&self, _id: &str) -> Result<()> {
            Err(Error::UnsupportedPlatform)
        }

        pub(crate) async fn previous(&self, _id: &str) -> Result<()> {
            Err(Error::UnsupportedPlatform)
        }

        pub(crate) async fn seek(&self, _id: &str, _position_ms: u64) -> Result<()> {
            Err(Error::UnsupportedPlatform)
        }
    }
}

#[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
pub(crate) use unsupported::PlatformController;
