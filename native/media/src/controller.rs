use std::{collections::HashMap, sync::Arc, time::Duration};

use tokio::sync::mpsc;
use tracing::warn;

use crate::{
    MediaEvent, MediaEventReceiver, MediaSession, Result, SessionPreference,
    platform::PlatformController, select_session,
};

const EVENT_POLL_INTERVAL: Duration = Duration::from_secs(1);
const NATIVE_EVENT_SAFETY_POLL_INTERVAL: Duration = Duration::from_secs(30);
const NATIVE_EVENT_DEBOUNCE: Duration = Duration::from_millis(75);
const EVENT_CHANNEL_CAPACITY: usize = 32;

/// Main handle for querying and controlling system media sessions.
#[derive(Clone)]
pub struct MediaController {
    platform: Arc<PlatformController>,
}

impl MediaController {
    /// Initializes the native backend for the current operating system.
    pub async fn new() -> Result<Self> {
        Ok(Self {
            platform: Arc::new(PlatformController::new().await?),
        })
    }

    /// Returns every media session currently visible to the operating system.
    pub async fn sessions(&self) -> Result<Vec<MediaSession>> {
        self.platform.sessions().await
    }

    /// Returns the automatically selected current session.
    pub async fn current(&self) -> Result<Option<MediaSession>> {
        self.current_with_preference(&SessionPreference::Auto).await
    }

    /// Selects the current session according to an explicit preference.
    pub async fn current_with_preference(
        &self,
        preference: &SessionPreference,
    ) -> Result<Option<MediaSession>> {
        let sessions = self.platform.sessions().await?;
        let platform_current = self.platform.current_session_id(&sessions).await?;
        Ok(select_session(&sessions, platform_current.as_deref(), preference).cloned())
    }

    /// Finds a session by its stable id.
    pub async fn session(&self, id: &str) -> Result<Option<MediaSession>> {
        Ok(self
            .platform
            .sessions()
            .await?
            .into_iter()
            .find(|session| session.id == id))
    }

    pub async fn play(&self, id: &str) -> Result<()> {
        self.platform.play(id).await
    }

    pub async fn pause(&self, id: &str) -> Result<()> {
        self.platform.pause(id).await
    }

    pub async fn toggle_play_pause(&self, id: &str) -> Result<()> {
        self.platform.toggle_play_pause(id).await
    }

    pub async fn next(&self, id: &str) -> Result<()> {
        self.platform.next(id).await
    }

    pub async fn previous(&self, id: &str) -> Result<()> {
        self.platform.previous(id).await
    }

    pub async fn seek(&self, id: &str, position_ms: u64) -> Result<()> {
        self.platform.seek(id, position_ms).await
    }

    /// Subscribes to normalized media changes.
    ///
    /// The implementation uses a conservative one-second polling fallback. This keeps
    /// backend-specific event handles out of the public API and avoids render-loop polling.
    pub async fn subscribe(&self) -> Result<MediaEventReceiver> {
        let (sessions, current) = self.snapshot().await?;
        let controller = self.clone();
        let changes = self.platform.subscribe_changes();
        let (sender, receiver) = mpsc::channel(EVENT_CHANNEL_CAPACITY);

        tokio::spawn(async move {
            poll_events(controller, sender, sessions, current, changes).await;
        });

        Ok(receiver)
    }

    async fn snapshot(&self) -> Result<(Vec<MediaSession>, Option<MediaSession>)> {
        let sessions = self.platform.sessions().await?;
        let platform_current = self.platform.current_session_id(&sessions).await?;
        let current = select_session(
            &sessions,
            platform_current.as_deref(),
            &SessionPreference::Auto,
        )
        .cloned();
        Ok((sessions, current))
    }
}

async fn poll_events(
    controller: MediaController,
    sender: mpsc::Sender<MediaEvent>,
    mut previous_sessions: Vec<MediaSession>,
    mut previous_current: Option<MediaSession>,
    mut changes: Option<tokio::sync::broadcast::Receiver<()>>,
) {
    let poll_interval = if changes.is_some() {
        NATIVE_EVENT_SAFETY_POLL_INTERVAL
    } else {
        EVENT_POLL_INTERVAL
    };
    let mut interval = tokio::time::interval(poll_interval);
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    interval.tick().await;

    loop {
        let (refresh, native_event) = match changes.as_mut() {
            Some(changes) => {
                tokio::select! {
                    _ = sender.closed() => break,
                    _ = interval.tick() => (true, false),
                    result = changes.recv() => {
                        (
                            !matches!(result, Err(tokio::sync::broadcast::error::RecvError::Closed)),
                            true,
                        )
                    }
                }
            }
            None => {
                tokio::select! {
                    _ = sender.closed() => break,
                    _ = interval.tick() => (true, false),
                }
            }
        };
        if !refresh {
            changes = None;
            continue;
        }
        if native_event {
            tokio::time::sleep(NATIVE_EVENT_DEBOUNCE).await;
            if let Some(changes) = changes.as_mut() {
                while changes.try_recv().is_ok() {}
            }
        }

        let (sessions, current) = match controller.snapshot().await {
            Ok(snapshot) => snapshot,
            Err(error) => {
                warn!(%error, "failed to refresh media sessions");
                continue;
            }
        };

        if emit_differences(
            &sender,
            &previous_sessions,
            &sessions,
            previous_current.as_ref(),
            current.as_ref(),
        )
        .await
        .is_none()
        {
            break;
        }

        previous_sessions = sessions;
        previous_current = current;
    }
}

async fn emit_differences(
    sender: &mpsc::Sender<MediaEvent>,
    previous: &[MediaSession],
    current: &[MediaSession],
    previous_current: Option<&MediaSession>,
    current_current: Option<&MediaSession>,
) -> Option<()> {
    let previous_by_id: HashMap<&str, &MediaSession> = previous
        .iter()
        .map(|session| (session.id.as_str(), session))
        .collect();
    let current_by_id: HashMap<&str, &MediaSession> = current
        .iter()
        .map(|session| (session.id.as_str(), session))
        .collect();

    for session in current {
        match previous_by_id.get(session.id.as_str()) {
            None => sender
                .send(MediaEvent::SessionAdded(session.clone()))
                .await
                .ok()?,
            Some(old) => {
                if old.track != session.track {
                    sender
                        .send(MediaEvent::TrackChanged {
                            session_id: session.id.clone(),
                            track: session.track.clone(),
                        })
                        .await
                        .ok()?;
                }
                if old.playback != session.playback {
                    sender
                        .send(MediaEvent::PlaybackChanged {
                            session_id: session.id.clone(),
                            playback: session.playback.clone(),
                        })
                        .await
                        .ok()?;
                }
            }
        }
    }

    for session in previous {
        if !current_by_id.contains_key(session.id.as_str()) {
            sender
                .send(MediaEvent::SessionRemoved {
                    session_id: session.id.clone(),
                })
                .await
                .ok()?;
        }
    }

    let previous_id = previous_current.map(|session| session.id.as_str());
    let current_id = current_current.map(|session| session.id.as_str());
    if previous_id != current_id {
        sender
            .send(MediaEvent::CurrentSessionChanged {
                session: current_current.cloned(),
            })
            .await
            .ok()?;
    }

    Some(())
}
