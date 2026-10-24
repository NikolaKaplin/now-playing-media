//! Deterministic Linux media player for Deno end-to-end tests. Never touches a real player.
#[cfg(target_os = "linux")]
mod fixture {
    use std::collections::HashMap;
    use zbus::zvariant::{OwnedObjectPath, OwnedValue, Value};
    pub struct Root;
    #[zbus::interface(name = "org.mpris.MediaPlayer2")]
    impl Root {
        #[zbus(property)]
        fn identity(&self) -> &str {
            "Now Playing Fixture"
        }
    }
    pub struct Player {
        playing: bool,
        position: i64,
        track: u32,
    }
    #[zbus::interface(name = "org.mpris.MediaPlayer2.Player")]
    impl Player {
        fn play(&mut self) {
            self.playing = true;
        }
        fn pause(&mut self) {
            self.playing = false;
        }
        fn play_pause(&mut self) {
            self.playing = !self.playing;
        }
        fn next(&mut self) {
            self.track += 1;
            self.position = 0;
        }
        fn previous(&mut self) {
            self.track = self.track.saturating_sub(1);
            self.position = 0;
        }
        fn set_position(&mut self, _track_id: OwnedObjectPath, position: i64) {
            self.position = position;
        }
        fn seek(&mut self, offset: i64) {
            self.position += offset;
        }
        #[zbus(property)]
        fn playback_status(&self) -> &str {
            if self.playing { "Playing" } else { "Paused" }
        }
        #[zbus(property)]
        fn position(&self) -> i64 {
            self.position
        }
        #[zbus(property)]
        fn rate(&self) -> f64 {
            1.0
        }
        #[zbus(property)]
        fn can_play(&self) -> bool {
            true
        }
        #[zbus(property)]
        fn can_pause(&self) -> bool {
            true
        }
        #[zbus(property)]
        fn can_go_next(&self) -> bool {
            true
        }
        #[zbus(property)]
        fn can_go_previous(&self) -> bool {
            true
        }
        #[zbus(property)]
        fn can_seek(&self) -> bool {
            true
        }
        #[zbus(property)]
        fn can_control(&self) -> bool {
            true
        }
        #[zbus(property)]
        fn metadata(&self) -> HashMap<String, OwnedValue> {
            HashMap::from([
                (
                    "mpris:trackid".into(),
                    Value::from(
                        OwnedObjectPath::try_from(format!("/track/{}", self.track)).unwrap(),
                    )
                    .try_into()
                    .unwrap(),
                ),
                ("mpris:length".into(), OwnedValue::from(180_000_000_i64)),
                (
                    "xesam:title".into(),
                    Value::from(format!("Track {}", self.track))
                        .try_into()
                        .unwrap(),
                ),
                (
                    "xesam:artist".into(),
                    Value::from(vec!["Fixture Artist"]).try_into().unwrap(),
                ),
                (
                    "xesam:album".into(),
                    Value::from("Fixture Album").try_into().unwrap(),
                ),
            ])
        }
    }
    pub async fn run() {
        let _connection = zbus::connection::Builder::session()
            .unwrap()
            .name("org.mpris.MediaPlayer2.now_playing_media_fixture")
            .unwrap()
            .serve_at("/org/mpris/MediaPlayer2", Root)
            .unwrap()
            .serve_at(
                "/org/mpris/MediaPlayer2",
                Player {
                    playing: true,
                    position: 12_000_000,
                    track: 1,
                },
            )
            .unwrap()
            .build()
            .await
            .unwrap();
        println!("READY");
        std::future::pending::<()>().await;
    }
}
#[tokio::main]
async fn main() {
    #[cfg(target_os = "linux")]
    fixture::run().await;
}
