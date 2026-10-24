use nowplaying::{
    MediaController, MediaSession, MediaSource, MediaTrack, PlaybackState, PlaybackStatus,
    SessionPreference, select_session,
};

fn sample_session() -> MediaSession {
    MediaSession {
        id: "Spotify.exe".to_owned(),
        source: MediaSource {
            id: "Spotify.exe".to_owned(),
            name: Some("Spotify".to_owned()),
        },
        track: Some(MediaTrack {
            title: "Example".to_owned(),
            artist: Some("Artist".to_owned()),
            album: Some("Album".to_owned()),
            album_artist: None,
            duration_ms: Some(180_000),
            track_number: Some(1),
            genres: vec!["Electronic".to_owned()],
        }),
        playback: PlaybackState {
            status: PlaybackStatus::Playing,
            position_ms: 42_000,
            playback_rate: 1.0,
        },
    }
}

#[test]
fn public_models_clone_and_compare_by_value() {
    let session = sample_session();
    assert_eq!(session.clone(), session);
    assert!(format!("{session:#?}").contains("Spotify"));
}

#[test]
fn explicit_source_selection_does_not_consume_sessions() {
    let sessions = vec![sample_session()];
    let selected = select_session(
        &sessions,
        None,
        &SessionPreference::Source("Spotify.exe".to_owned()),
    );
    assert_eq!(
        selected.map(|session| session.id.as_str()),
        Some("Spotify.exe")
    );
    assert_eq!(sessions.len(), 1);
}

#[test]
fn playback_defaults_are_safe() {
    assert_eq!(PlaybackStatus::default(), PlaybackStatus::Unknown);
    assert_eq!(
        PlaybackState::default(),
        PlaybackState {
            status: PlaybackStatus::Unknown,
            position_ms: 0,
            playback_rate: 1.0,
        }
    );
}

#[test]
fn controller_handle_is_send_and_sync() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<MediaController>();
}
