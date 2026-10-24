use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, Mutex},
};

use futures_util::StreamExt;
use tokio::sync::broadcast;
use tracing::warn;
use zbus::{
    Connection, Proxy,
    zvariant::{OwnedObjectPath, OwnedValue},
};

use crate::{
    Error, MediaSession, MediaSource, MediaTrack, PlaybackState, PlaybackStatus, Result,
    model::{non_empty, source_name_from_id},
};

const MPRIS_PREFIX: &str = "org.mpris.MediaPlayer2.";
const MPRIS_PATH: &str = "/org/mpris/MediaPlayer2";
const ROOT_INTERFACE: &str = "org.mpris.MediaPlayer2";
const PLAYER_INTERFACE: &str = "org.mpris.MediaPlayer2.Player";
const MICROSECONDS_PER_MILLISECOND: i64 = 1_000;

pub(crate) struct PlatformController {
    connection: Connection,
    changes: broadcast::Sender<()>,
    property_watchers: Arc<Mutex<HashSet<String>>>,
}

#[derive(Clone, Copy)]
enum Control {
    Play,
    Pause,
    Toggle,
    Next,
    Previous,
}

impl Control {
    fn method(self) -> &'static str {
        match self {
            Self::Play => "Play",
            Self::Pause => "Pause",
            Self::Toggle => "PlayPause",
            Self::Next => "Next",
            Self::Previous => "Previous",
        }
    }

    fn operation(self) -> &'static str {
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
        let connection = Connection::session()
            .await
            .map_err(|_| Error::BackendUnavailable { backend: "MPRIS" })?;
        let (changes, _) = broadcast::channel(32);
        tokio::spawn(watch_name_owner_changes(
            connection.clone(),
            changes.clone(),
        ));
        Ok(Self {
            connection,
            changes,
            property_watchers: Arc::new(Mutex::new(HashSet::new())),
        })
    }

    pub(crate) async fn sessions(&self) -> Result<Vec<MediaSession>> {
        let names = self.player_names().await?;
        let mut sessions = Vec::with_capacity(names.len());
        for name in names {
            self.ensure_property_watcher(&name);
            match self.snapshot_player(&name).await {
                Ok(session) => sessions.push(session),
                Err(error) => {
                    warn!(player = name, %error, "MPRIS player disappeared or returned invalid state");
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
        sessions: &[MediaSession],
    ) -> Result<Option<String>> {
        Ok(sessions
            .iter()
            .find(|session| session.playback.status == PlaybackStatus::Playing)
            .or_else(|| sessions.last())
            .map(|session| session.id.clone()))
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
        self.ensure_player_exists(id).await?;
        let proxy = self.player_proxy(id).await?;
        let can_seek = proxy.get_property::<bool>("CanSeek").await.unwrap_or(false);
        if !can_seek {
            return Err(Error::UnsupportedOperation { operation: "seek" });
        }

        let requested = milliseconds_to_microseconds(position_ms);
        let metadata = proxy
            .get_property::<HashMap<String, OwnedValue>>("Metadata")
            .await
            .unwrap_or_default();

        if let Some(track_id) = metadata_object_path(&metadata, "mpris:trackid") {
            let result: zbus::Result<()> = proxy.call("SetPosition", &(track_id, requested)).await;
            match result {
                Ok(()) => return Ok(()),
                Err(error) if operation_is_unsupported(&error) => {}
                Err(error) => return Err(map_control_error(id, "seek", error)),
            }
        }

        let current = proxy.get_property::<i64>("Position").await.unwrap_or(0);
        let offset = requested.saturating_sub(current);
        let result: zbus::Result<()> = proxy.call("Seek", &(offset,)).await;
        result.map_err(|error| map_control_error(id, "seek", error))
    }

    async fn player_names(&self) -> Result<Vec<String>> {
        let proxy = zbus::fdo::DBusProxy::new(&self.connection)
            .await
            .map_err(Error::platform)?;
        let mut names: Vec<String> = proxy
            .list_names()
            .await
            .map_err(Error::platform)?
            .into_iter()
            .map(|name| name.to_string())
            .filter(|name| name.starts_with(MPRIS_PREFIX))
            .collect();
        names.sort_unstable();
        Ok(names)
    }

    fn ensure_property_watcher(&self, id: &str) {
        let should_start = match self.property_watchers.lock() {
            Ok(mut watchers) => watchers.insert(id.to_owned()),
            Err(_) => {
                warn!("MPRIS property watcher mutex is poisoned");
                false
            }
        };
        if !should_start {
            return;
        }

        tokio::spawn(watch_player_properties(
            self.connection.clone(),
            id.to_owned(),
            self.changes.clone(),
            Arc::clone(&self.property_watchers),
        ));
    }

    async fn ensure_player_exists(&self, id: &str) -> Result<()> {
        if self.player_names().await?.iter().any(|name| name == id) {
            Ok(())
        } else {
            Err(Error::SessionNotFound { id: id.to_owned() })
        }
    }

    async fn player_proxy<'a>(&'a self, id: &'a str) -> Result<Proxy<'a>> {
        Proxy::new(&self.connection, id, MPRIS_PATH, PLAYER_INTERFACE)
            .await
            .map_err(Error::platform)
    }

    async fn snapshot_player(&self, id: &str) -> Result<MediaSession> {
        let player = self.player_proxy(id).await?;
        let status_text = player
            .get_property::<String>("PlaybackStatus")
            .await
            .map_err(|error| map_query_error(id, error))?;
        let status = map_playback_status(&status_text);
        let position_ms = player
            .get_property::<i64>("Position")
            .await
            .ok()
            .map(microseconds_to_milliseconds)
            .unwrap_or(0);
        let playback_rate = player
            .get_property::<f64>("Rate")
            .await
            .ok()
            .filter(|rate| rate.is_finite())
            .unwrap_or(1.0);
        let metadata = player
            .get_property::<HashMap<String, OwnedValue>>("Metadata")
            .await
            .unwrap_or_default();

        let root = Proxy::new(&self.connection, id, MPRIS_PATH, ROOT_INTERFACE)
            .await
            .map_err(Error::platform)?;
        let identity = root
            .get_property::<String>("Identity")
            .await
            .ok()
            .and_then(non_empty)
            .or_else(|| source_name_from_id(id));

        Ok(MediaSession {
            id: id.to_owned(),
            source: MediaSource {
                id: id.to_owned(),
                name: identity,
            },
            track: media_track(&metadata),
            playback: PlaybackState {
                status,
                position_ms,
                playback_rate,
            },
        })
    }

    async fn control(&self, id: &str, control: Control) -> Result<()> {
        self.ensure_player_exists(id).await?;
        let proxy = self.player_proxy(id).await?;
        let enabled = match control {
            Control::Play => proxy.get_property::<bool>("CanPlay").await.unwrap_or(false),
            Control::Pause => proxy
                .get_property::<bool>("CanPause")
                .await
                .unwrap_or(false),
            Control::Toggle => {
                let can_play = proxy.get_property::<bool>("CanPlay").await.unwrap_or(false);
                let can_pause = proxy
                    .get_property::<bool>("CanPause")
                    .await
                    .unwrap_or(false);
                can_play || can_pause
            }
            Control::Next => proxy
                .get_property::<bool>("CanGoNext")
                .await
                .unwrap_or(false),
            Control::Previous => proxy
                .get_property::<bool>("CanGoPrevious")
                .await
                .unwrap_or(false),
        };

        if !enabled {
            return Err(Error::UnsupportedOperation {
                operation: control.operation(),
            });
        }

        let result: zbus::Result<()> = proxy.call(control.method(), &()).await;
        result.map_err(|error| map_control_error(id, control.operation(), error))
    }
}

async fn watch_name_owner_changes(connection: Connection, changes: broadcast::Sender<()>) {
    let proxy = match zbus::fdo::DBusProxy::new(&connection).await {
        Ok(proxy) => proxy,
        Err(error) => {
            warn!(%error, "failed to subscribe to MPRIS name changes");
            return;
        }
    };
    let mut stream = match proxy.receive_name_owner_changed().await {
        Ok(stream) => stream,
        Err(error) => {
            warn!(%error, "failed to receive MPRIS name changes");
            return;
        }
    };

    while let Some(signal) = stream.next().await {
        if signal
            .args()
            .is_ok_and(|args| args.name().as_str().starts_with(MPRIS_PREFIX))
        {
            let _ = changes.send(());
        }
    }
}

async fn watch_player_properties(
    connection: Connection,
    id: String,
    changes: broadcast::Sender<()>,
    watchers: Arc<Mutex<HashSet<String>>>,
) {
    let builder = match zbus::fdo::PropertiesProxy::builder(&connection).destination(id.as_str()) {
        Ok(builder) => builder,
        Err(error) => {
            warn!(player = id, %error, "failed to build MPRIS property watcher");
            remove_property_watcher(&watchers, &id);
            return;
        }
    };
    let builder = match builder.path(MPRIS_PATH) {
        Ok(builder) => builder,
        Err(error) => {
            warn!(player = id, %error, "failed to configure MPRIS property watcher");
            remove_property_watcher(&watchers, &id);
            return;
        }
    };
    let proxy = match builder.build().await {
        Ok(proxy) => proxy,
        Err(error) => {
            warn!(player = id, %error, "failed to create MPRIS property watcher");
            remove_property_watcher(&watchers, &id);
            return;
        }
    };
    let mut stream = match proxy.receive_properties_changed().await {
        Ok(stream) => stream,
        Err(error) => {
            warn!(player = id, %error, "failed to subscribe to MPRIS properties");
            remove_property_watcher(&watchers, &id);
            return;
        }
    };

    while let Some(signal) = stream.next().await {
        if signal
            .args()
            .is_ok_and(|args| args.interface_name().as_str() == PLAYER_INTERFACE)
        {
            let _ = changes.send(());
        }
    }
    remove_property_watcher(&watchers, &id);
}

fn remove_property_watcher(watchers: &Mutex<HashSet<String>>, id: &str) {
    if let Ok(mut watchers) = watchers.lock() {
        watchers.remove(id);
    }
}

fn media_track(metadata: &HashMap<String, OwnedValue>) -> Option<MediaTrack> {
    let title = metadata_string(metadata, "xesam:title")?;
    let artist = metadata_string_list(metadata, "xesam:artist").map(|items| items.join(", "));
    let album_artist =
        metadata_string_list(metadata, "xesam:albumArtist").map(|items| items.join(", "));
    let album = metadata_string(metadata, "xesam:album");
    let duration_ms = metadata_i64(metadata, "mpris:length").map(microseconds_to_milliseconds);
    let track_number = metadata_i32(metadata, "xesam:trackNumber")
        .and_then(|number| u32::try_from(number).ok())
        .filter(|number| *number > 0);
    let genres = metadata_string_list(metadata, "xesam:genre").unwrap_or_default();

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

fn metadata_string(metadata: &HashMap<String, OwnedValue>, key: &str) -> Option<String> {
    let value = metadata.get(key)?;
    let value = <&str>::try_from(value).ok()?;
    non_empty(value)
}

fn metadata_string_list(metadata: &HashMap<String, OwnedValue>, key: &str) -> Option<Vec<String>> {
    let value = metadata.get(key)?.try_clone().ok()?;
    let values = Vec::<String>::try_from(value).ok()?;
    let normalized: Vec<String> = values.into_iter().filter_map(non_empty).collect();
    (!normalized.is_empty()).then_some(normalized)
}

fn metadata_i64(metadata: &HashMap<String, OwnedValue>, key: &str) -> Option<i64> {
    i64::try_from(metadata.get(key)?).ok()
}

fn metadata_i32(metadata: &HashMap<String, OwnedValue>, key: &str) -> Option<i32> {
    i32::try_from(metadata.get(key)?).ok()
}

fn metadata_object_path(
    metadata: &HashMap<String, OwnedValue>,
    key: &str,
) -> Option<OwnedObjectPath> {
    let value = metadata.get(key)?.try_clone().ok()?;
    OwnedObjectPath::try_from(value).ok()
}

fn map_playback_status(status: &str) -> PlaybackStatus {
    match status {
        "Playing" => PlaybackStatus::Playing,
        "Paused" => PlaybackStatus::Paused,
        "Stopped" => PlaybackStatus::Stopped,
        _ => PlaybackStatus::Unknown,
    }
}

fn microseconds_to_milliseconds(microseconds: i64) -> u64 {
    u64::try_from(microseconds / MICROSECONDS_PER_MILLISECOND).unwrap_or(0)
}

fn milliseconds_to_microseconds(milliseconds: u64) -> i64 {
    milliseconds
        .saturating_mul(MICROSECONDS_PER_MILLISECOND as u64)
        .min(i64::MAX as u64) as i64
}

fn operation_is_unsupported(error: &zbus::Error) -> bool {
    let message = error.to_string();
    message.contains("NotSupported")
        || message.contains("UnknownMethod")
        || matches!(
            error,
            zbus::Error::Unsupported | zbus::Error::InterfaceNotFound
        )
}

fn player_disappeared(error: &zbus::Error) -> bool {
    let message = error.to_string();
    message.contains("NameHasNoOwner") || message.contains("ServiceUnknown")
}

fn map_query_error(id: &str, error: zbus::Error) -> Error {
    if player_disappeared(&error) {
        Error::SessionNotFound { id: id.to_owned() }
    } else {
        Error::platform(error)
    }
}

fn map_control_error(id: &str, operation: &'static str, error: zbus::Error) -> Error {
    if player_disappeared(&error) {
        Error::SessionNotFound { id: id.to_owned() }
    } else if operation_is_unsupported(&error) {
        Error::UnsupportedOperation { operation }
    } else {
        Error::platform(error)
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use zbus::zvariant::{OwnedValue, Str};

    use super::{
        map_playback_status, media_track, microseconds_to_milliseconds,
        milliseconds_to_microseconds,
    };
    use crate::PlaybackStatus;

    #[test]
    fn mpris_time_conversion_is_safe() {
        assert_eq!(microseconds_to_milliseconds(1_234_567), 1_234);
        assert_eq!(microseconds_to_milliseconds(-1_000), 0);
        assert_eq!(milliseconds_to_microseconds(1_234), 1_234_000);
        assert_eq!(milliseconds_to_microseconds(u64::MAX), i64::MAX);
    }

    #[test]
    fn maps_all_standard_playback_statuses() {
        assert_eq!(map_playback_status("Playing"), PlaybackStatus::Playing);
        assert_eq!(map_playback_status("Paused"), PlaybackStatus::Paused);
        assert_eq!(map_playback_status("Stopped"), PlaybackStatus::Stopped);
        assert_eq!(map_playback_status("Buffering"), PlaybackStatus::Unknown);
    }

    #[test]
    fn missing_or_empty_title_produces_no_track() {
        assert_eq!(media_track(&HashMap::new()), None);
        let mut metadata = HashMap::new();
        metadata.insert("xesam:title".to_owned(), OwnedValue::from(Str::from("")));
        assert_eq!(media_track(&metadata), None);
    }

    #[test]
    fn malformed_metadata_is_ignored_without_panicking() {
        let mut metadata = HashMap::new();
        metadata.insert(
            "xesam:title".to_owned(),
            OwnedValue::from(Str::from("Song")),
        );
        metadata.insert("xesam:artist".to_owned(), OwnedValue::from(42_i32));
        let track = media_track(&metadata);
        assert_eq!(track.and_then(|track| track.artist), None);
    }
}
