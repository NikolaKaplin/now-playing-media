use serde::{Deserialize, Serialize};

use crate::{MediaSession, MediaTrack, PlaybackState};

/// A normalized change reported by a media-session subscription.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum MediaEvent {
    SessionAdded(MediaSession),
    SessionRemoved {
        session_id: String,
    },
    CurrentSessionChanged {
        session: Option<MediaSession>,
    },
    TrackChanged {
        session_id: String,
        track: Option<MediaTrack>,
    },
    PlaybackChanged {
        session_id: String,
        playback: PlaybackState,
    },
}

/// Receiving half of a media event subscription.
pub type MediaEventReceiver = tokio::sync::mpsc::Receiver<MediaEvent>;
