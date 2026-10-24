//! macOS 15.4 and later deny MediaRemote to processes without Apple's entitlement.
//! `/usr/bin/perl` is entitled, so the bundled mediaremote-adapter helper runs the
//! MediaRemote calls inside it and prints JSON. This works on older macOS too.

use std::{
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    process::Stdio,
    time::Duration,
};

use serde_json::{Map, Value};
use tempfile::TempDir;
use tokio::process::Command;

use crate::{
    Error, MediaSession, MediaSource, MediaTrack, PlaybackState, PlaybackStatus, Result,
    model::{non_empty, source_name_from_id},
};

const ADAPTER_LIBRARY: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/MediaRemoteAdapter"));
const ADAPTER_SCRIPT: &[u8] =
    include_bytes!("../../mediaremote-adapter/bin/mediaremote-adapter.pl");
const PERL: &str = "/usr/bin/perl";
const BACKEND: &str = "MediaRemote";
// `get` gives up after 2 seconds on its own; this only guards against a hung process.
const ADAPTER_TIMEOUT: Duration = Duration::from_secs(5);
const FALLBACK_SOURCE_ID: &str = "com.apple.MediaRemote";

pub(crate) struct PlatformController {
    // Owns the private directory holding the helper for the controller's lifetime.
    _directory: TempDir,
    script: PathBuf,
    framework: PathBuf,
}

/// MRCommand ids accepted by the adapter's `send` function.
#[derive(Clone, Copy)]
enum Control {
    Play = 0,
    Pause = 1,
    Toggle = 2,
    Next = 4,
    Previous = 5,
}

impl Control {
    fn name(self) -> &'static str {
        match self {
            Self::Play => "play",
            Self::Pause => "pause",
            Self::Toggle => "toggle_play_pause",
            Self::Next => "next",
            Self::Previous => "previous",
        }
    }
}

impl PlatformController {
    pub(crate) async fn new() -> Result<Self> {
        if ADAPTER_LIBRARY.is_empty() || !Path::new(PERL).exists() {
            return Err(Error::BackendUnavailable { backend: BACKEND });
        }
        tokio::task::spawn_blocking(install_adapter)
            .await
            .map_err(Error::platform)?
            .map_err(|_| Error::BackendUnavailable { backend: BACKEND })
    }

    pub(crate) async fn sessions(&self) -> Result<Vec<MediaSession>> {
        Ok(self.now_playing().await?.into_iter().collect())
    }

    pub(crate) fn subscribe_changes(&self) -> Option<tokio::sync::broadcast::Receiver<()>> {
        None
    }

    pub(crate) async fn current_session_id(
        &self,
        sessions: &[MediaSession],
    ) -> Result<Option<String>> {
        Ok(sessions.first().map(|session| session.id.clone()))
    }

    pub(crate) async fn play(&self, id: &str) -> Result<()> {
        self.control(id, Control::Play).await
    }

    pub(crate) async fn pause(&self, id: &str) -> Result<()> {
        self.control(id, Control::Pause).await
    }

    pub(crate) async fn toggle_play_pause(&self, id: &str) -> Result<()> {
        self.control(id, Control::Toggle).await
    }

    pub(crate) async fn next(&self, id: &str) -> Result<()> {
        self.control(id, Control::Next).await
    }

    pub(crate) async fn previous(&self, id: &str) -> Result<()> {
        self.control(id, Control::Previous).await
    }

    pub(crate) async fn seek(&self, id: &str, position_ms: u64) -> Result<()> {
        self.ensure_current(id).await?;
        let micros = position_ms.saturating_mul(1_000).min(i64::MAX as u64);
        self.run(&["seek", &micros.to_string()])
            .await
            .map(drop)
            .map_err(|_| Error::UnsupportedOperation { operation: "seek" })
    }

    async fn control(&self, id: &str, control: Control) -> Result<()> {
        self.ensure_current(id).await?;
        self.run(&["send", &(control as i32).to_string()])
            .await
            .map(drop)
            .map_err(|_| Error::UnsupportedOperation {
                operation: control.name(),
            })
    }

    async fn ensure_current(&self, id: &str) -> Result<()> {
        let current = self.now_playing().await?;
        if current.as_ref().is_some_and(|session| session.id == id) {
            Ok(())
        } else {
            Err(Error::SessionNotFound { id: id.to_owned() })
        }
    }

    async fn now_playing(&self) -> Result<Option<MediaSession>> {
        let output = self
            .run(&["get", "--micros", "--now", "--no-artwork"])
            .await?;
        let value: Value = serde_json::from_slice(&output).map_err(Error::platform)?;
        Ok(value.as_object().map(parse_now_playing))
    }

    async fn run(&self, arguments: &[&str]) -> Result<Vec<u8>> {
        let mut command = Command::new(PERL);
        command
            .arg(&self.script)
            .arg(&self.framework)
            .args(arguments)
            // The adapter reads its options from MEDIAREMOTEADAPTER_* variables set by the
            // script; inherited ones must not leak in.
            .env_clear()
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .kill_on_drop(true);
        let output = tokio::time::timeout(ADAPTER_TIMEOUT, command.output())
            .await
            .map_err(|_| Error::Platform {
                message: "mediaremote-adapter timed out".to_owned(),
            })?
            .map_err(|_| Error::BackendUnavailable { backend: BACKEND })?;
        if output.status.success() {
            Ok(output.stdout)
        } else {
            Err(Error::Platform {
                message: format!("mediaremote-adapter exited with {}", output.status),
            })
        }
    }
}

/// Writes the helper into a fresh directory only this user can access.
fn install_adapter() -> std::io::Result<PlatformController> {
    let directory = tempfile::Builder::new()
        .prefix("now-playing-media-")
        .tempdir()?;
    let script = directory.path().join("mediaremote-adapter.pl");
    std::fs::write(&script, ADAPTER_SCRIPT)?;
    // The script expects a framework directory containing a binary of the same name.
    let framework = directory.path().join("MediaRemoteAdapter.framework");
    std::fs::create_dir(&framework)?;
    let library = framework.join("MediaRemoteAdapter");
    std::fs::write(&library, ADAPTER_LIBRARY)?;
    std::fs::set_permissions(&library, std::fs::Permissions::from_mode(0o700))?;
    Ok(PlatformController {
        _directory: directory,
        script,
        framework,
    })
}

fn parse_now_playing(payload: &Map<String, Value>) -> MediaSession {
    let text = |key: &str| payload.get(key)?.as_str().and_then(non_empty);
    let number = |key: &str| payload.get(key)?.as_f64();

    let source_id = text("parentApplicationBundleIdentifier")
        .or_else(|| text("bundleIdentifier"))
        .unwrap_or_else(|| FALLBACK_SOURCE_ID.to_owned());
    let status = match payload.get("playing").and_then(Value::as_bool) {
        Some(true) => PlaybackStatus::Playing,
        Some(false) => PlaybackStatus::Paused,
        None => PlaybackStatus::Unknown,
    };
    let position_ms = number("elapsedTimeNowMicros")
        .or_else(|| number("elapsedTimeMicros"))
        .and_then(micros_to_milliseconds)
        .unwrap_or(0);
    // A paused player reports rate 0; the rate it plays at is still 1.
    let playback_rate = number("playbackRate")
        .filter(|rate| rate.is_finite() && *rate != 0.0)
        .unwrap_or(1.0);

    MediaSession {
        id: source_id.clone(),
        source: MediaSource {
            name: source_name_from_id(&source_id),
            id: source_id,
        },
        track: text("title").map(|title| MediaTrack {
            title,
            artist: text("artist"),
            album: text("album"),
            album_artist: None,
            duration_ms: number("durationMicros").and_then(micros_to_milliseconds),
            track_number: number("trackNumber")
                .filter(|number| number.fract() == 0.0 && *number >= 1.0)
                .and_then(|number| u32::try_from(number as u64).ok()),
            genres: text("genre").into_iter().collect(),
        }),
        playback: PlaybackState {
            status,
            position_ms,
            playback_rate,
        },
    }
}

fn micros_to_milliseconds(micros: f64) -> Option<u64> {
    if !micros.is_finite() || micros < 0.0 {
        return None;
    }
    Some((micros / 1_000.0).round().min(u64::MAX as f64) as u64)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{micros_to_milliseconds, parse_now_playing};
    use crate::PlaybackStatus;

    #[test]
    fn converts_adapter_micros_safely() {
        assert_eq!(micros_to_milliseconds(42_123_456.0), Some(42_123));
        assert_eq!(micros_to_milliseconds(-1.0), None);
        assert_eq!(micros_to_milliseconds(f64::NAN), None);
    }

    #[test]
    fn parses_adapter_payload() {
        let payload = json!({
            "bundleIdentifier": "com.apple.WebKit.GPU",
            "parentApplicationBundleIdentifier": "com.apple.Safari",
            "playing": false,
            "title": "Song",
            "artist": "Artist",
            "durationMicros": 180_000_000.0,
            "elapsedTimeNowMicros": 12_345_678.0,
            "playbackRate": 0.0,
            "trackNumber": 3,
            "genre": "Jazz",
        });
        let session = parse_now_playing(payload.as_object().unwrap());
        assert_eq!(session.id, "com.apple.Safari");
        assert_eq!(session.source.name.as_deref(), Some("Apple Safari"));
        assert_eq!(session.playback.status, PlaybackStatus::Paused);
        assert_eq!(session.playback.position_ms, 12_346);
        assert_eq!(session.playback.playback_rate, 1.0);
        let track = session.track.unwrap();
        assert_eq!(track.title, "Song");
        assert_eq!(track.album, None);
        assert_eq!(track.duration_ms, Some(180_000));
        assert_eq!(track.track_number, Some(3));
        assert_eq!(track.genres, ["Jazz"]);
    }

    #[test]
    fn missing_title_and_source_are_tolerated() {
        let session = parse_now_playing(json!({ "playing": true }).as_object().unwrap());
        assert_eq!(session.id, "com.apple.MediaRemote");
        assert_eq!(session.playback.status, PlaybackStatus::Playing);
        assert!(session.track.is_none());
    }
}
