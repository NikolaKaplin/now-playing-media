use std::{
    collections::{HashMap, HashSet},
    sync::Mutex,
    time::{SystemTime, UNIX_EPOCH},
};

use tokio::sync::broadcast;
use tracing::warn;
use windows::{
    Foundation::{TimeSpan, TypedEventHandler},
    Media::Control::{
        CurrentSessionChangedEventArgs, GlobalSystemMediaTransportControlsSession,
        GlobalSystemMediaTransportControlsSessionManager,
        GlobalSystemMediaTransportControlsSessionMediaProperties,
        GlobalSystemMediaTransportControlsSessionPlaybackStatus, MediaPropertiesChangedEventArgs,
        PlaybackInfoChangedEventArgs, SessionsChangedEventArgs, TimelinePropertiesChangedEventArgs,
    },
};

use crate::{
    Error, MediaSession, MediaSource, MediaTrack, PlaybackState, PlaybackStatus, Result,
    model::{non_empty, source_name_from_id},
};

const TICKS_PER_MILLISECOND: i64 = 10_000;
const TICKS_PER_SECOND: u64 = 10_000_000;
const WINDOWS_TO_UNIX_EPOCH_SECONDS: u64 = 11_644_473_600;

pub(crate) struct PlatformController {
    manager: GlobalSystemMediaTransportControlsSessionManager,
    changes: broadcast::Sender<()>,
    current_session_token: Option<i64>,
    sessions_token: Option<i64>,
    session_subscriptions: Mutex<HashMap<String, SessionSubscription>>,
}

struct SessionSubscription {
    session: GlobalSystemMediaTransportControlsSession,
    timeline_token: Option<i64>,
    playback_token: Option<i64>,
    media_token: Option<i64>,
}

#[derive(Clone, Copy)]
enum Control {
    Play,
    Pause,
    Toggle,
    Next,
    Previous,
    Seek(u64),
}

impl Control {
    fn name(self) -> &'static str {
        match self {
            Self::Play => "play",
            Self::Pause => "pause",
            Self::Toggle => "toggle_play_pause",
            Self::Next => "next",
            Self::Previous => "previous",
            Self::Seek(_) => "seek",
        }
    }
}

impl PlatformController {
    pub(crate) async fn new() -> Result<Self> {
        let request = GlobalSystemMediaTransportControlsSessionManager::RequestAsync()
            .map_err(Error::platform)?;
        let manager = request
            .await
            .map_err(|_| Error::BackendUnavailable { backend: "GSMTC" })?;
        let (changes, _) = broadcast::channel(32);

        let current_sender = changes.clone();
        let current_handler = TypedEventHandler::<
            GlobalSystemMediaTransportControlsSessionManager,
            CurrentSessionChangedEventArgs,
        >::new(move |_, _| {
            let _ = current_sender.send(());
            Ok(())
        });
        let current_session_token = manager.CurrentSessionChanged(&current_handler).ok();

        let sessions_sender = changes.clone();
        let sessions_handler = TypedEventHandler::<
            GlobalSystemMediaTransportControlsSessionManager,
            SessionsChangedEventArgs,
        >::new(move |_, _| {
            let _ = sessions_sender.send(());
            Ok(())
        });
        let sessions_token = manager.SessionsChanged(&sessions_handler).ok();

        Ok(Self {
            manager,
            changes,
            current_session_token,
            sessions_token,
            session_subscriptions: Mutex::new(HashMap::new()),
        })
    }

    pub(crate) async fn sessions(&self) -> Result<Vec<MediaSession>> {
        let native = {
            let native_sessions = self.manager.GetSessions().map_err(Error::platform)?;
            let count = native_sessions.Size().map_err(Error::platform)?;
            let mut native = Vec::with_capacity(count as usize);

            for index in 0..count {
                match native_sessions.GetAt(index) {
                    Ok(session) => native.push(session),
                    Err(error) => {
                        warn!(%error, index, "GSMTC session disappeared while enumerating");
                    }
                }
            }
            native
        };
        self.sync_session_subscriptions(&native);

        let mut sessions = Vec::with_capacity(native.len());
        for (index, native) in native.iter().enumerate() {
            match snapshot_session(native).await {
                Ok(session) => sessions.push(session),
                Err(error) => {
                    warn!(%error, index, "failed to read a GSMTC session");
                }
            }
        }

        Ok(sessions)
    }

    pub(crate) fn subscribe_changes(&self) -> Option<broadcast::Receiver<()>> {
        Some(self.changes.subscribe())
    }

    pub(crate) async fn current_session_id(
        &self,
        _sessions: &[MediaSession],
    ) -> Result<Option<String>> {
        let native = match self.manager.GetCurrentSession() {
            Ok(session) => session,
            Err(_) => return Ok(None),
        };
        Ok(native
            .SourceAppUserModelId()
            .ok()
            .and_then(|id| non_empty(id.to_string())))
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
        self.control(id, Control::Seek(position_ms)).await
    }

    async fn control(&self, id: &str, control: Control) -> Result<()> {
        let session = self.find_session(id)?;
        let playback_info = session.GetPlaybackInfo().map_err(Error::platform)?;
        let controls = playback_info.Controls().map_err(Error::platform)?;
        let enabled = match control {
            Control::Play => controls.IsPlayEnabled(),
            Control::Pause => controls.IsPauseEnabled(),
            Control::Toggle => controls.IsPlayPauseToggleEnabled(),
            Control::Next => controls.IsNextEnabled(),
            Control::Previous => controls.IsPreviousEnabled(),
            Control::Seek(_) => controls.IsPlaybackPositionEnabled(),
        }
        .map_err(Error::platform)?;

        if !enabled {
            return Err(Error::UnsupportedOperation {
                operation: control.name(),
            });
        }

        let operation = match control {
            Control::Play => session.TryPlayAsync(),
            Control::Pause => session.TryPauseAsync(),
            Control::Toggle => session.TryTogglePlayPauseAsync(),
            Control::Next => session.TrySkipNextAsync(),
            Control::Previous => session.TrySkipPreviousAsync(),
            Control::Seek(position_ms) => {
                session.TryChangePlaybackPositionAsync(milliseconds_to_ticks(position_ms))
            }
        }
        .map_err(Error::platform)?;

        if operation.await.map_err(Error::platform)? {
            Ok(())
        } else {
            Err(Error::UnsupportedOperation {
                operation: control.name(),
            })
        }
    }

    fn find_session(&self, id: &str) -> Result<GlobalSystemMediaTransportControlsSession> {
        let sessions = self.manager.GetSessions().map_err(Error::platform)?;
        let count = sessions.Size().map_err(Error::platform)?;
        for index in 0..count {
            let session = match sessions.GetAt(index) {
                Ok(session) => session,
                Err(_) => continue,
            };
            let matches = session
                .SourceAppUserModelId()
                .ok()
                .is_some_and(|source| source == id);
            if matches {
                return Ok(session);
            }
        }
        Err(Error::SessionNotFound { id: id.to_owned() })
    }

    fn sync_session_subscriptions(&self, sessions: &[GlobalSystemMediaTransportControlsSession]) {
        let ids: HashSet<String> = sessions
            .iter()
            .filter_map(|session| {
                session
                    .SourceAppUserModelId()
                    .ok()
                    .and_then(|id| non_empty(id.to_string()))
            })
            .collect();
        let Ok(mut subscriptions) = self.session_subscriptions.lock() else {
            warn!("GSMTC event subscription mutex is poisoned");
            return;
        };
        subscriptions.retain(|id, _| ids.contains(id));

        for session in sessions {
            let Some(id) = session
                .SourceAppUserModelId()
                .ok()
                .and_then(|id| non_empty(id.to_string()))
            else {
                continue;
            };
            if subscriptions.contains_key(&id) {
                continue;
            }

            let timeline_sender = self.changes.clone();
            let timeline_handler = TypedEventHandler::<
                GlobalSystemMediaTransportControlsSession,
                TimelinePropertiesChangedEventArgs,
            >::new(move |_, _| {
                let _ = timeline_sender.send(());
                Ok(())
            });
            let timeline_token = session.TimelinePropertiesChanged(&timeline_handler).ok();

            let playback_sender = self.changes.clone();
            let playback_handler = TypedEventHandler::<
                GlobalSystemMediaTransportControlsSession,
                PlaybackInfoChangedEventArgs,
            >::new(move |_, _| {
                let _ = playback_sender.send(());
                Ok(())
            });
            let playback_token = session.PlaybackInfoChanged(&playback_handler).ok();

            let media_sender = self.changes.clone();
            let media_handler = TypedEventHandler::<
                GlobalSystemMediaTransportControlsSession,
                MediaPropertiesChangedEventArgs,
            >::new(move |_, _| {
                let _ = media_sender.send(());
                Ok(())
            });
            let media_token = session.MediaPropertiesChanged(&media_handler).ok();

            subscriptions.insert(
                id,
                SessionSubscription {
                    session: session.clone(),
                    timeline_token,
                    playback_token,
                    media_token,
                },
            );
        }
    }
}

impl Drop for PlatformController {
    fn drop(&mut self) {
        if let Some(token) = self.current_session_token {
            let _ = self.manager.RemoveCurrentSessionChanged(token);
        }
        if let Some(token) = self.sessions_token {
            let _ = self.manager.RemoveSessionsChanged(token);
        }
    }
}

impl Drop for SessionSubscription {
    fn drop(&mut self) {
        if let Some(token) = self.timeline_token {
            let _ = self.session.RemoveTimelinePropertiesChanged(token);
        }
        if let Some(token) = self.playback_token {
            let _ = self.session.RemovePlaybackInfoChanged(token);
        }
        if let Some(token) = self.media_token {
            let _ = self.session.RemoveMediaPropertiesChanged(token);
        }
    }
}

async fn snapshot_session(
    session: &GlobalSystemMediaTransportControlsSession,
) -> Result<MediaSession> {
    let source_id = non_empty(
        session
            .SourceAppUserModelId()
            .map_err(Error::platform)?
            .to_string(),
    )
    .ok_or_else(|| Error::Platform {
        message: "GSMTC returned an empty SourceAppUserModelId".to_owned(),
    })?;

    let timeline = session.GetTimelineProperties().ok();
    let raw_position_ms = timeline
        .as_ref()
        .and_then(|value| value.Position().ok())
        .map(timespan_to_milliseconds)
        .unwrap_or(0);
    let duration_ms = timeline.as_ref().and_then(timeline_duration_ms);

    let playback_info = session.GetPlaybackInfo().ok();
    let status = playback_info
        .as_ref()
        .and_then(|value| value.PlaybackStatus().ok())
        .map(map_playback_status)
        .unwrap_or(PlaybackStatus::Unknown);
    let playback_rate = playback_info
        .as_ref()
        .and_then(|value| value.PlaybackRate().ok())
        .and_then(|value| value.Value().ok())
        .filter(|rate| rate.is_finite())
        .unwrap_or(1.0);
    let last_updated_ticks = timeline
        .as_ref()
        .and_then(|value| value.LastUpdatedTime().ok())
        .map(|value| value.UniversalTime);
    let position_ms = extrapolate_position_ms(
        raw_position_ms,
        status,
        playback_rate,
        last_updated_ticks,
        windows_now_ticks(),
    );

    let properties = match session.TryGetMediaPropertiesAsync() {
        Ok(operation) => operation.await.ok(),
        Err(_) => None,
    };
    let track = properties
        .as_ref()
        .and_then(|properties| media_track(properties, duration_ms));

    Ok(MediaSession {
        id: source_id.clone(),
        source: MediaSource {
            name: source_name_from_id(&source_id),
            id: source_id,
        },
        track,
        playback: PlaybackState {
            status,
            position_ms,
            playback_rate,
        },
    })
}

fn media_track(
    properties: &GlobalSystemMediaTransportControlsSessionMediaProperties,
    duration_ms: Option<u64>,
) -> Option<MediaTrack> {
    let title = properties
        .Title()
        .ok()
        .and_then(|value| non_empty(value.to_string()))?;
    let artist = properties
        .Artist()
        .ok()
        .and_then(|value| non_empty(value.to_string()));
    let album = properties
        .AlbumTitle()
        .ok()
        .and_then(|value| non_empty(value.to_string()));
    let album_artist = properties
        .AlbumArtist()
        .ok()
        .and_then(|value| non_empty(value.to_string()));
    let track_number = properties
        .TrackNumber()
        .ok()
        .and_then(|number| u32::try_from(number).ok())
        .filter(|number| *number > 0);
    let genres = properties
        .Genres()
        .ok()
        .map(|values| {
            let count = values.Size().unwrap_or(0);
            (0..count)
                .filter_map(|index| values.GetAt(index).ok())
                .filter_map(|value| non_empty(value.to_string()))
                .collect()
        })
        .unwrap_or_default();

    Some(MediaTrack {
        title,
        artist,
        album,
        album_artist,
        duration_ms,
        track_number,
        genres,
    })
}

fn map_playback_status(
    status: GlobalSystemMediaTransportControlsSessionPlaybackStatus,
) -> PlaybackStatus {
    match status {
        GlobalSystemMediaTransportControlsSessionPlaybackStatus::Playing => PlaybackStatus::Playing,
        GlobalSystemMediaTransportControlsSessionPlaybackStatus::Paused => PlaybackStatus::Paused,
        GlobalSystemMediaTransportControlsSessionPlaybackStatus::Stopped
        | GlobalSystemMediaTransportControlsSessionPlaybackStatus::Closed => {
            PlaybackStatus::Stopped
        }
        _ => PlaybackStatus::Unknown,
    }
}

fn timeline_duration_ms(
    timeline: &windows::Media::Control::GlobalSystemMediaTransportControlsSessionTimelineProperties,
) -> Option<u64> {
    let start = timeline.StartTime().ok()?.Duration;
    let end = timeline.EndTime().ok()?.Duration;
    let duration = end.checked_sub(start)?;
    (duration > 0).then(|| ticks_to_milliseconds(duration))
}

fn timespan_to_milliseconds(timespan: TimeSpan) -> u64 {
    ticks_to_milliseconds(timespan.Duration)
}

fn ticks_to_milliseconds(ticks: i64) -> u64 {
    u64::try_from(ticks / TICKS_PER_MILLISECOND).unwrap_or(0)
}

fn milliseconds_to_ticks(milliseconds: u64) -> i64 {
    milliseconds
        .saturating_mul(TICKS_PER_MILLISECOND as u64)
        .min(i64::MAX as u64) as i64
}

fn windows_now_ticks() -> Option<i64> {
    let since_unix_epoch = SystemTime::now().duration_since(UNIX_EPOCH).ok()?;
    let unix_ticks = since_unix_epoch
        .as_secs()
        .checked_mul(TICKS_PER_SECOND)?
        .checked_add(u64::from(since_unix_epoch.subsec_nanos()) / 100)?;
    let epoch_offset = WINDOWS_TO_UNIX_EPOCH_SECONDS.checked_mul(TICKS_PER_SECOND)?;
    i64::try_from(epoch_offset.checked_add(unix_ticks)?).ok()
}

fn extrapolate_position_ms(
    position_ms: u64,
    status: PlaybackStatus,
    playback_rate: f64,
    last_updated_ticks: Option<i64>,
    now_ticks: Option<i64>,
) -> u64 {
    if status != PlaybackStatus::Playing || !playback_rate.is_finite() {
        return position_ms;
    }
    let Some(elapsed_ticks) = now_ticks
        .zip(last_updated_ticks)
        .and_then(|(now, updated)| now.checked_sub(updated))
        .filter(|elapsed| *elapsed > 0)
    else {
        return position_ms;
    };
    let delta_ms = ticks_to_milliseconds(elapsed_ticks) as f64 * playback_rate;
    let position = position_ms as f64 + delta_ms;
    if position <= 0.0 {
        0
    } else if position >= u64::MAX as f64 {
        u64::MAX
    } else {
        position.floor() as u64
    }
}

#[cfg(test)]
mod tests {
    use windows::Media::Control::GlobalSystemMediaTransportControlsSessionPlaybackStatus;

    use super::{
        extrapolate_position_ms, map_playback_status, milliseconds_to_ticks, ticks_to_milliseconds,
    };
    use crate::PlaybackStatus;

    #[test]
    fn winrt_ticks_convert_to_milliseconds() {
        assert_eq!(ticks_to_milliseconds(10_000), 1);
        assert_eq!(ticks_to_milliseconds(1_234_560_000), 123_456);
        assert_eq!(ticks_to_milliseconds(-10_000), 0);
    }

    #[test]
    fn milliseconds_convert_to_winrt_ticks_safely() {
        assert_eq!(milliseconds_to_ticks(123), 1_230_000);
        assert_eq!(milliseconds_to_ticks(u64::MAX), i64::MAX);
    }

    #[test]
    fn playback_status_mapping_is_total() {
        assert_eq!(
            map_playback_status(GlobalSystemMediaTransportControlsSessionPlaybackStatus::Playing),
            PlaybackStatus::Playing
        );
        assert_eq!(
            map_playback_status(GlobalSystemMediaTransportControlsSessionPlaybackStatus::Paused),
            PlaybackStatus::Paused
        );
        assert_eq!(
            map_playback_status(GlobalSystemMediaTransportControlsSessionPlaybackStatus::Stopped),
            PlaybackStatus::Stopped
        );
        assert_eq!(
            map_playback_status(GlobalSystemMediaTransportControlsSessionPlaybackStatus::Changing),
            PlaybackStatus::Unknown
        );
    }

    #[test]
    fn timeline_position_is_advanced_from_last_update() {
        let updated = 1_000_000_000;
        let now = updated + 20_000_000;
        assert_eq!(
            extrapolate_position_ms(
                42_100,
                PlaybackStatus::Playing,
                1.0,
                Some(updated),
                Some(now)
            ),
            44_100
        );
        assert_eq!(
            extrapolate_position_ms(
                42_100,
                PlaybackStatus::Paused,
                1.0,
                Some(updated),
                Some(now)
            ),
            42_100
        );
    }
}
