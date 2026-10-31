//! What a remote shows: the TV's screen, and what is playing on it.
//!
//! [`StateFolder`] reads it off what the TV already has, the engine's own [`Event`]s and the
//! hosted player's [`PlayerReport`]s, so the TV app keeps nothing of its own for its remotes. It
//! says which changes are worth a message: every change of what is playing, of pause, speed,
//! volume, tracks or skips, and the playhead only when it jumps or has moved [`POSITION_STEP`].
//! Between those, a phone moves its own playhead on its own clock, from `position`, `paused` and
//! `speed`. That keeps a playing episode to a message every few seconds per phone, and none while
//! paused.

use anistream_core::ids::AnilistId;
use anistream_model::{
    event::Event,
    hosted::{PlayerReport, PlayerUi},
    playback::SkipKind,
};
use serde::{Deserialize, Serialize};

use crate::command::Track;

/// How far the playhead moves, in media seconds, before a phone is told again.
pub const POSITION_STEP: f64 = 5.0;

/// The whole of what a remote shows.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct RemoteState {
    /// Counts up with every state sent, so a phone can tell a stale one.
    pub rev: u64,
    pub screen: Screen,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub now_playing: Option<NowPlaying>,
    /// The watch party the TV is in, while it is in one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub party: Option<PartyInfo>,
}

/// The watch party the TV is in: what a phone shows on the TV's remote page.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PartyInfo {
    pub room: String,
    /// The room's code, when it is one on the suggested server.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
    /// The https invite, which the phone can share on.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub link: Option<String>,
    /// The title and episode the room is set to.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub episode: Option<String>,
    pub ready: bool,
    pub playing: bool,
}

/// What the TV has on screen, as its front end says: see [`StateFolder::screen`].
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Screen {
    #[default]
    Home,
    /// A search, with the text in its field: what a phone's field mirrors.
    Search {
        query: String,
    },
    Title {
        id: AnilistId,
    },
    Player,
    Settings,
    #[serde(other)]
    Other,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NowPlaying {
    pub id: AnilistId,
    pub title: String,
    pub episode: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub episode_title: Option<String>,
    /// Still finding the stream: nothing has played yet.
    pub loading: bool,
    pub position: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration: Option<f64>,
    pub paused: bool,
    pub speed: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub volume: Option<f64>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tracks: Vec<TrackInfo>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub skip: Option<SkipOffer>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TrackInfo {
    pub id: i64,
    pub kind: Track,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
    pub selected: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SkipOffer {
    pub kind: SkipPart,
    pub to: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SkipPart {
    Opening,
    Ending,
}

impl From<SkipKind> for SkipPart {
    fn from(kind: SkipKind) -> Self {
        match kind {
            SkipKind::Opening => Self::Opening,
            SkipKind::Ending => Self::Ending,
        }
    }
}

/// [`RemoteState`], kept up from what the TV's engine and player say.
#[derive(Debug, Default)]
pub struct StateFolder {
    state: RemoteState,
    /// What was playing before the play just staged, for when that play finds nothing and the
    /// old one keeps playing ([`Event::PlaybackKept`]).
    replaced: Option<NowPlaying>,
    /// The playhead and pause last told, which the next position is weighed against.
    told: Option<(f64, bool)>,
    /// The TV's own volume, once its front end has said it: what a remote's volume shows in
    /// place of the player's.
    system_volume: Option<f64>,
}

impl StateFolder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn state(&self) -> &RemoteState {
        &self.state
    }

    /// The state to send, counted: call it after a change this folder said was worth sending.
    pub fn publish(&mut self) -> RemoteState {
        self.state.rev += 1;
        if let Some(now) = &self.state.now_playing {
            self.told = Some((now.position, now.paused));
        }
        self.state.clone()
    }

    /// What the TV front end has on screen now. True when it changed.
    pub fn screen(&mut self, screen: Screen) -> bool {
        if self.state.screen == screen {
            return false;
        }
        self.state.screen = screen;
        true
    }

    /// The TV's own volume, out of 100, as its front end reads it from the system. From then on
    /// it is the volume a remote shows, not the player's. True when a remote should hear of it.
    pub fn volume(&mut self, percent: f64) -> bool {
        self.system_volume = Some(percent);
        self.with_now(|now| replace(&mut now.volume, Some(percent)))
    }

    /// An engine event, as the TV front end reads it. True when a remote should hear of it.
    pub fn event(&mut self, event: &Event) -> bool {
        match event {
            Event::PlaybackStaged { id, title, episode, episode_title, speed } => {
                let carried = self
                    .system_volume
                    .or_else(|| self.state.now_playing.as_ref().and_then(|now| now.volume));
                self.replaced = self.state.now_playing.take();
                self.told = None;
                self.state.now_playing = Some(NowPlaying {
                    id: *id,
                    title: title.clone(),
                    episode: episode.clone(),
                    episode_title: episode_title.clone(),
                    loading: true,
                    position: 0.0,
                    duration: None,
                    paused: false,
                    speed: *speed,
                    volume: carried,
                    tracks: Vec::new(),
                    skip: None,
                });
                true
            }
            Event::PlaybackKept => {
                self.state.now_playing = self.replaced.take();
                self.told = None;
                true
            }
            Event::PlaybackEnded { rolling_on, .. } => {
                if *rolling_on {
                    return false;
                }
                self.replaced = None;
                self.told = None;
                self.state.now_playing.take().is_some()
            }
            Event::Playback { position, duration, paused } => {
                let told = self.told;
                let Some(now) = &mut self.state.now_playing else { return false };
                let was_loading = std::mem::replace(&mut now.loading, false);
                let duration_changed = now.duration != *duration;
                now.position = *position;
                now.duration = *duration;
                now.paused = *paused;
                was_loading || duration_changed || Self::moved(told, *position, *paused)
            }
            Event::Resumed { position } => self.with_now(|now| {
                now.position = *position;
                true
            }),
            Event::PlaybackSpeed(speed) => self.with_now(|now| replace(&mut now.speed, *speed)),
            Event::PlaybackVolume(volume) if self.system_volume.is_none() => {
                self.with_now(|now| replace(&mut now.volume, Some(*volume)))
            }
            Event::SkipAvailable { kind, to } => self.with_now(|now| {
                replace(&mut now.skip, Some(SkipOffer { kind: (*kind).into(), to: *to }))
            }),
            Event::SkipCleared => self.with_now(|now| replace(&mut now.skip, None)),
            Event::PlayerUi { ui: PlayerUi::Episode { number, title, show, .. }, .. } => self
                .with_now(|now| {
                    let a = replace(&mut now.episode, number.clone());
                    let b = replace(&mut now.episode_title, title.clone());
                    let c = replace(&mut now.title, show.clone());
                    a || b || c
                }),
            Event::PartyJoined(view) | Event::PartyState(view) => {
                let info = PartyInfo {
                    room: view.room.clone(),
                    code: view.code.clone(),
                    link: view.link.clone(),
                    title: view.room_episode.as_ref().map(|r| r.title.clone()),
                    episode: view.room_episode.as_ref().map(|r| r.episode.clone()),
                    ready: view.ready,
                    playing: view.playing,
                };
                replace(&mut self.state.party, Some(info))
            }
            Event::PartyLeft { .. } => self.state.party.take().is_some(),
            _ => false,
        }
    }

    /// What the TV's hosted player reported, as the front end hands it to the engine: only its
    /// track list matters here. True when it changed.
    pub fn report(&mut self, report: &PlayerReport) -> bool {
        let PlayerReport::Property { name, json } = report else { return false };
        if name != "track-list" {
            return false;
        }
        let tracks = parse_track_list(json);
        self.with_now(|now| replace(&mut now.tracks, tracks))
    }

    fn with_now(&mut self, change: impl FnOnce(&mut NowPlaying) -> bool) -> bool {
        self.state.now_playing.as_mut().is_some_and(change)
    }

    fn moved(told: Option<(f64, bool)>, position: f64, paused: bool) -> bool {
        let Some((at, was_paused)) = told else { return true };
        paused != was_paused || position < at || position - at >= POSITION_STEP
    }
}

fn replace<T: PartialEq>(slot: &mut T, value: T) -> bool {
    if *slot == value {
        return false;
    }
    *slot = value;
    true
}

/// The audio and subtitle tracks in mpv's `track-list`, as the hosted player reports it.
fn parse_track_list(json: &str) -> Vec<TrackInfo> {
    let Ok(serde_json::Value::Array(tracks)) = serde_json::from_str(json) else {
        return Vec::new();
    };
    tracks
        .iter()
        .filter_map(|track| {
            let kind = match track.get("type")?.as_str()? {
                "audio" => Track::Audio,
                "sub" => Track::Subtitle,
                _ => return None,
            };
            let text = |key| track.get(key).and_then(|v| v.as_str()).map(str::to_owned);
            Some(TrackInfo {
                id: track.get("id")?.as_i64()?,
                kind,
                title: text("title"),
                language: text("lang"),
                selected: track.get("selected").and_then(|v| v.as_bool()).unwrap_or(false),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn staged() -> Event {
        Event::PlaybackStaged {
            id: AnilistId::new(42),
            title: "Harbour Lights".into(),
            episode: "3".into(),
            episode_title: None,
            speed: 1.0,
        }
    }

    fn at(position: f64, paused: bool) -> Event {
        Event::Playback { position, duration: Some(1440.0), paused }
    }

    #[test]
    fn the_playhead_is_told_when_it_jumps_not_every_tick() {
        let mut folder = StateFolder::new();
        assert!(folder.event(&staged()));
        folder.publish();
        assert!(folder.event(&at(1.0, false)), "the first position ends the loading");
        folder.publish();
        assert!(!folder.event(&at(2.0, false)));
        assert!(!folder.event(&at(5.5, false)));
        assert!(folder.event(&at(6.0, false)), "five seconds on");
        folder.publish();
        assert!(folder.event(&at(6.5, true)), "paused");
        folder.publish();
        assert!(folder.event(&at(2.0, true)), "a seek back");
        assert_eq!(folder.state().now_playing.as_ref().unwrap().position, 2.0);
    }

    #[test]
    fn an_ending_that_rolls_on_keeps_now_playing() {
        let mut folder = StateFolder::new();
        folder.event(&staged());
        let ended = |rolling_on| Event::PlaybackEnded {
            watched: true,
            ending: anistream_model::playback::Ending::Complete { next: None },
            rolling_on,
        };
        assert!(!folder.event(&ended(true)));
        assert!(folder.state().now_playing.is_some());
        assert!(folder.event(&ended(false)));
        assert!(folder.state().now_playing.is_none());
    }

    #[test]
    fn the_tvs_own_volume_is_shown_over_the_players() {
        let mut folder = StateFolder::new();
        folder.volume(30.0);
        folder.event(&staged());
        assert_eq!(folder.state().now_playing.as_ref().unwrap().volume, Some(30.0));
        assert!(!folder.event(&Event::PlaybackVolume(100.0)));
        assert!(folder.volume(35.0));
        assert_eq!(folder.state().now_playing.as_ref().unwrap().volume, Some(35.0));
    }

    #[test]
    fn a_kept_playback_comes_back() {
        let mut folder = StateFolder::new();
        folder.event(&staged());
        folder.event(&Event::PlaybackStaged {
            id: AnilistId::new(43),
            title: "Paper Orchard".into(),
            episode: "1".into(),
            episode_title: None,
            speed: 1.0,
        });
        folder.event(&Event::PlaybackKept);
        assert_eq!(folder.state().now_playing.as_ref().unwrap().id, AnilistId::new(42));
    }

    #[test]
    fn tracks_come_from_the_players_track_list() {
        let mut folder = StateFolder::new();
        folder.event(&staged());
        let json = r#"[{"id":1,"type":"video"},{"id":1,"type":"audio","lang":"jpn","selected":true},
            {"id":2,"type":"sub","lang":"eng","title":"Full","selected":false}]"#;
        let report = PlayerReport::Property { name: "track-list".into(), json: json.into() };
        assert!(folder.report(&report));
        assert!(!folder.report(&report), "the same list again is no change");
        let tracks = &folder.state().now_playing.as_ref().unwrap().tracks;
        assert_eq!(tracks.len(), 2);
        assert_eq!(tracks[1].kind, Track::Subtitle);
        assert_eq!(tracks[1].title.as_deref(), Some("Full"));
    }
}
