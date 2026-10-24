use std::time::Instant;

use crate::{PlaybackState, PlaybackStatus};

/// Interpolates playback position between relatively infrequent OS snapshots.
#[derive(Debug, Clone)]
pub struct MediaClock {
    snapshot: PlaybackState,
    observed_at: Instant,
}

impl MediaClock {
    /// Builds a clock from a snapshot observed now.
    pub fn new(snapshot: PlaybackState) -> Self {
        Self::from_snapshot(snapshot, Instant::now())
    }

    /// Builds a clock from a snapshot and its monotonic observation time.
    pub fn from_snapshot(snapshot: PlaybackState, observed_at: Instant) -> Self {
        Self {
            snapshot,
            observed_at,
        }
    }

    /// Re-synchronizes the clock with a newly observed platform snapshot.
    pub fn sync(&mut self, snapshot: PlaybackState) {
        self.sync_at(snapshot, Instant::now());
    }

    /// Re-synchronizes the clock at an explicitly supplied monotonic time.
    pub fn sync_at(&mut self, snapshot: PlaybackState, observed_at: Instant) {
        self.snapshot = snapshot;
        self.observed_at = observed_at;
    }

    /// Returns the interpolated position at the current instant.
    pub fn position(&self) -> u64 {
        self.position_at(Instant::now())
    }

    /// Returns the interpolated position at a supplied monotonic instant.
    pub fn position_at(&self, now: Instant) -> u64 {
        if self.snapshot.status != PlaybackStatus::Playing {
            return self.snapshot.position_ms;
        }

        let rate = self.snapshot.playback_rate;
        if !rate.is_finite() || rate == 0.0 {
            return self.snapshot.position_ms;
        }

        let elapsed = now
            .saturating_duration_since(self.observed_at)
            .as_secs_f64();
        let delta_ms = elapsed * 1_000.0 * rate;
        if !delta_ms.is_finite() {
            return self.snapshot.position_ms;
        }

        let position = self.snapshot.position_ms as f64 + delta_ms;
        if position <= 0.0 {
            0
        } else if position >= u64::MAX as f64 {
            u64::MAX
        } else {
            position.floor() as u64
        }
    }

    /// Returns the last platform snapshot used by this clock.
    pub fn snapshot(&self) -> &PlaybackState {
        &self.snapshot
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use super::MediaClock;
    use crate::{PlaybackState, PlaybackStatus};

    fn state(status: PlaybackStatus, position_ms: u64, playback_rate: f64) -> PlaybackState {
        PlaybackState {
            status,
            position_ms,
            playback_rate,
        }
    }

    #[test]
    fn playing_position_advances_smoothly() {
        let start = Instant::now();
        let clock = MediaClock::from_snapshot(state(PlaybackStatus::Playing, 42_100, 1.0), start);
        assert_eq!(clock.position_at(start + Duration::from_millis(64)), 42_164);
    }

    #[test]
    fn playback_rate_is_applied() {
        let start = Instant::now();
        let clock = MediaClock::from_snapshot(state(PlaybackStatus::Playing, 1_000, 1.5), start);
        assert_eq!(clock.position_at(start + Duration::from_millis(200)), 1_300);
    }

    #[test]
    fn paused_and_stopped_positions_are_frozen() {
        let start = Instant::now();
        for status in [PlaybackStatus::Paused, PlaybackStatus::Stopped] {
            let clock = MediaClock::from_snapshot(state(status, 9_000, 1.0), start);
            assert_eq!(clock.position_at(start + Duration::from_secs(30)), 9_000);
        }
    }

    #[test]
    fn sync_replaces_the_interpolation_baseline() {
        let start = Instant::now();
        let mut clock =
            MediaClock::from_snapshot(state(PlaybackStatus::Playing, 10_000, 1.0), start);
        let seek_time = start + Duration::from_secs(1);
        clock.sync_at(state(PlaybackStatus::Playing, 80_000, 1.0), seek_time);
        assert_eq!(
            clock.position_at(seek_time + Duration::from_millis(250)),
            80_250
        );
    }

    #[test]
    fn reverse_rate_never_underflows() {
        let start = Instant::now();
        let clock = MediaClock::from_snapshot(state(PlaybackStatus::Playing, 500, -1.0), start);
        assert_eq!(clock.position_at(start + Duration::from_secs(1)), 0);
    }

    #[test]
    fn invalid_rate_does_not_corrupt_position() {
        let start = Instant::now();
        let clock = MediaClock::from_snapshot(state(PlaybackStatus::Playing, 500, f64::NAN), start);
        assert_eq!(clock.position_at(start + Duration::from_secs(1)), 500);
    }
}
