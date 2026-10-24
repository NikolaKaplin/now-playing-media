use serde::{Deserialize, Serialize};

use crate::{MediaSession, PlaybackStatus};

/// How a session should be selected from the complete session list.
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SessionPreference {
    /// Prefer a playing session, then the OS current session, then the last session.
    #[default]
    Auto,
    /// Select a session by session id or source id.
    Source(String),
}

/// Selects one session without hiding or consuming the complete input list.
pub fn select_session<'a>(
    sessions: &'a [MediaSession],
    platform_current_id: Option<&str>,
    preference: &SessionPreference,
) -> Option<&'a MediaSession> {
    match preference {
        SessionPreference::Source(source) => sessions
            .iter()
            .find(|session| session.id == *source || session.source.id == *source),
        SessionPreference::Auto => sessions
            .iter()
            .find(|session| session.playback.status == PlaybackStatus::Playing)
            .or_else(|| {
                platform_current_id.and_then(|id| sessions.iter().find(|session| session.id == id))
            })
            .or_else(|| sessions.last()),
    }
}

#[cfg(test)]
mod tests {
    use super::{SessionPreference, select_session};
    use crate::{MediaSession, MediaSource, PlaybackState, PlaybackStatus};

    fn session(id: &str, status: PlaybackStatus) -> MediaSession {
        MediaSession {
            id: id.to_owned(),
            source: MediaSource {
                id: id.to_owned(),
                name: None,
            },
            track: None,
            playback: PlaybackState {
                status,
                position_ms: 0,
                playback_rate: 1.0,
            },
        }
    }

    #[test]
    fn auto_prefers_playing_over_platform_current() {
        let sessions = [
            session("paused", PlaybackStatus::Paused),
            session("playing", PlaybackStatus::Playing),
        ];
        assert_eq!(
            select_session(&sessions, Some("paused"), &SessionPreference::Auto)
                .map(|session| session.id.as_str()),
            Some("playing")
        );
    }

    #[test]
    fn auto_uses_platform_current_when_nothing_is_playing() {
        let sessions = [
            session("one", PlaybackStatus::Stopped),
            session("two", PlaybackStatus::Paused),
        ];
        assert_eq!(
            select_session(&sessions, Some("one"), &SessionPreference::Auto)
                .map(|session| session.id.as_str()),
            Some("one")
        );
    }

    #[test]
    fn auto_falls_back_to_last_available_session() {
        let sessions = [
            session("one", PlaybackStatus::Stopped),
            session("two", PlaybackStatus::Paused),
        ];
        assert_eq!(
            select_session(&sessions, None, &SessionPreference::Auto)
                .map(|session| session.id.as_str()),
            Some("two")
        );
    }

    #[test]
    fn source_selects_by_source_id() {
        let sessions = [session("spotify", PlaybackStatus::Paused)];
        assert_eq!(
            select_session(
                &sessions,
                None,
                &SessionPreference::Source("spotify".to_owned())
            )
            .map(|session| session.id.as_str()),
            Some("spotify")
        );
    }
}
