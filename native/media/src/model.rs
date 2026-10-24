use serde::{Deserialize, Serialize};

/// The normalized playback status of a media session.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlaybackStatus {
    Playing,
    Paused,
    Stopped,
    #[default]
    Unknown,
}

/// Platform-independent metadata for the current media item.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MediaTrack {
    pub title: String,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub album_artist: Option<String>,
    pub duration_ms: Option<u64>,
    pub track_number: Option<u32>,
    pub genres: Vec<String>,
}

/// A point-in-time playback snapshot supplied by a platform backend.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlaybackState {
    pub status: PlaybackStatus,
    pub position_ms: u64,
    pub playback_rate: f64,
}

impl Default for PlaybackState {
    fn default() -> Self {
        Self {
            status: PlaybackStatus::Unknown,
            position_ms: 0,
            playback_rate: 1.0,
        }
    }
}

/// The application that owns a media session.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MediaSource {
    /// Stable platform identifier such as `Spotify.exe` or an MPRIS bus name.
    pub id: String,
    /// Human-readable application name when the backend exposes one.
    pub name: Option<String>,
}

/// A normalized system media session.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MediaSession {
    /// Stable identifier used by the control methods.
    pub id: String,
    pub source: MediaSource,
    pub track: Option<MediaTrack>,
    pub playback: PlaybackState,
}

pub(crate) fn non_empty(value: impl Into<String>) -> Option<String> {
    let value = value.into();
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else if trimmed.len() == value.len() {
        Some(value)
    } else {
        Some(trimmed.to_owned())
    }
}

pub(crate) fn source_name_from_id(id: &str) -> Option<String> {
    let leaf = id.rsplit(['\\', '/', '!']).next().unwrap_or(id);
    let without_extension = leaf.strip_suffix(".exe").unwrap_or(leaf);
    let ignored_component = |component: &&str| {
        matches!(
            component.to_ascii_lowercase().as_str(),
            "com" | "org" | "net" | "io" | "ru" | "mpris" | "mediaplayer2" | "desktop" | "client"
        )
    };
    let components: Vec<&str> = without_extension
        .split('.')
        .filter(|component| !component.is_empty() && !ignored_component(component))
        .collect();
    let candidate = if components.is_empty() {
        without_extension.to_owned()
    } else {
        components.join(" ")
    };
    let candidate = candidate.trim();
    if candidate.is_empty() {
        return None;
    }

    let mut output = String::with_capacity(candidate.len() + 4);
    let mut previous_lowercase = false;
    let mut capitalize = true;
    for character in candidate.chars() {
        if character.is_uppercase() && previous_lowercase {
            output.push(' ');
        }
        if capitalize {
            output.extend(character.to_uppercase());
        } else {
            output.push(character);
        }
        capitalize = character == ' ';
        previous_lowercase = character.is_lowercase();
    }
    Some(output)
}

#[cfg(test)]
mod tests {
    use super::{non_empty, source_name_from_id};

    #[test]
    fn empty_and_whitespace_metadata_is_absent() {
        assert_eq!(non_empty(""), None);
        assert_eq!(non_empty("  \t"), None);
        assert_eq!(non_empty("  title  "), Some("title".to_owned()));
    }

    #[test]
    fn source_names_are_readable() {
        assert_eq!(
            source_name_from_id("YandexMusic.exe").as_deref(),
            Some("Yandex Music")
        );
        assert_eq!(
            source_name_from_id("Spotify.exe").as_deref(),
            Some("Spotify")
        );
        assert_eq!(
            source_name_from_id("org.mpris.MediaPlayer2.spotify").as_deref(),
            Some("Spotify")
        );
        assert_eq!(
            source_name_from_id("ru.yandex.desktop.music").as_deref(),
            Some("Yandex Music")
        );
    }
}
