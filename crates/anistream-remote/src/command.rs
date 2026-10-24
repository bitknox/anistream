//! What a phone can ask a TV to do, and what each becomes there.
//!
//! The vocabulary is the engine's: nearly every command is one [`Command`] or [`PlayerCommand`],
//! named after it and carrying the same values. It is a list of its own rather than the model's
//! enums on the wire for three reasons. The model has no serde and its shape follows the engine,
//! while the wire has to stay readable by a TV a release older than the phone. A phone must reach
//! only the part of the engine a remote should: playing and controlling, never a sign-in, a
//! setting, a store or a download. And a few things a remote does are not the engine's at all, but
//! the TV front end's: typing into the field on screen, moving its focus, and the TV's own volume.
//!
//! [`RemoteCommand::route`] is that boundary, and the only place the two meet.

use anistream_core::ids::AnilistId;
pub use anistream_model::remote::{FrontIntent, RemoteKey};
use anistream_model::{
    command::{Command, PlayerCommand},
    hosted::TrackKind,
};
use serde::{Deserialize, Serialize};

/// One thing a phone asks of a TV.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "name", rename_all = "snake_case")]
pub enum RemoteCommand {
    /// Play on TV: [`Command::Play`]. The TV finds the streams itself.
    Play { id: AnilistId, episode: String },
    /// Open a title's page on the TV: what the front end does with [`Command::LoadDetail`].
    Open { id: AnilistId },
    /// [`PlayerCommand::TogglePause`].
    TogglePause,
    /// [`PlayerCommand::Seek`], by seconds; negative goes back.
    Seek { by: f64 },
    /// [`PlayerCommand::SeekTo`].
    SeekTo { position: f64 },
    /// [`PlayerCommand::Step`] forward one episode.
    Next,
    /// [`PlayerCommand::Step`] back one episode.
    Previous,
    /// [`PlayerCommand::TakeSkip`]: past the opening or ending on offer.
    Skip,
    /// [`PlayerCommand::Stop`].
    Stop,
    /// A step of the TV's own volume, the system's, which the TV front end sets: not the
    /// player's volume, so it works with nothing playing.
    Volume { step: f64 },
    /// [`PlayerCommand::SpeedTo`].
    Speed { speed: f64 },
    /// [`PlayerCommand::ChooseTrack`]: `id` from the state's track list, or none to turn
    /// subtitles off.
    Track {
        kind: Track,
        #[serde(default)]
        id: Option<i64>,
    },
    /// The whole text of the field focused on the TV, as the phone's field now reads. Whole
    /// rather than keystrokes, so a lost or repeated message leaves the same text. `submit`
    /// presses the field's action, a search's Enter.
    Text {
        text: String,
        #[serde(default)]
        submit: bool,
    },
    /// Move the TV's focus, as its own remote would.
    Navigate { key: Key },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Track {
    Audio,
    Subtitle,
}

/// The keys of a TV's own remote, as the wire names them: the model's [`RemoteKey`] once on the
/// TV.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Key {
    Up,
    Down,
    Left,
    Right,
    Select,
    Back,
    Home,
}

/// Where a remote command goes on the TV.
#[derive(Debug, Clone, PartialEq)]
pub enum Routed {
    /// To the engine, as the TV's own front end would send it.
    Engine(Command),
    /// To the TV front end, which alone knows what is on its screen.
    Front(FrontIntent),
}

impl From<Key> for RemoteKey {
    fn from(key: Key) -> Self {
        match key {
            Key::Up => Self::Up,
            Key::Down => Self::Down,
            Key::Left => Self::Left,
            Key::Right => Self::Right,
            Key::Select => Self::Select,
            Key::Back => Self::Back,
            Key::Home => Self::Home,
        }
    }
}

impl RemoteCommand {
    pub fn route(self) -> Routed {
        let player = |command| Routed::Engine(Command::Player(command));
        match self {
            Self::Play { id, episode } => Routed::Engine(Command::Play { id, episode }),
            Self::Open { id } => Routed::Front(FrontIntent::Open { id }),
            Self::TogglePause => player(PlayerCommand::TogglePause),
            Self::Seek { by } => player(PlayerCommand::Seek(by)),
            Self::SeekTo { position } => player(PlayerCommand::SeekTo(position.max(0.0))),
            Self::Next => player(PlayerCommand::Step(1)),
            Self::Previous => player(PlayerCommand::Step(-1)),
            Self::Skip => player(PlayerCommand::TakeSkip),
            Self::Stop => player(PlayerCommand::Stop),
            Self::Volume { step } => {
                Routed::Front(FrontIntent::Volume { step: step.clamp(-100.0, 100.0) })
            }
            Self::Speed { speed } => player(PlayerCommand::SpeedTo(speed.clamp(0.25, 4.0))),
            Self::Track { kind, id } => player(PlayerCommand::ChooseTrack {
                kind: match kind {
                    Track::Audio => TrackKind::Audio,
                    Track::Subtitle => TrackKind::Subtitle,
                },
                id,
            }),
            Self::Text { text, submit } => Routed::Front(FrontIntent::Text { text, submit }),
            Self::Navigate { key } => Routed::Front(FrontIntent::Navigate { key: key.into() }),
        }
    }

    /// Whether this needs something playing. Answered `not_now` when nothing is, rather than
    /// sent to an engine that would drop it.
    pub fn needs_playback(&self) -> bool {
        !matches!(
            self,
            Self::Play { .. }
                | Self::Open { .. }
                | Self::Text { .. }
                | Self::Navigate { .. }
                | Self::Volume { .. }
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn player_commands_become_the_engines_own() {
        let routed = RemoteCommand::Track { kind: Track::Subtitle, id: None }.route();
        assert_eq!(
            routed,
            Routed::Engine(Command::Player(PlayerCommand::ChooseTrack {
                kind: TrackKind::Subtitle,
                id: None
            }))
        );
        let routed = RemoteCommand::Play { id: AnilistId::new(7), episode: "3".into() }.route();
        assert!(
            matches!(routed, Routed::Engine(Command::Play { id, episode }) if id.get() == 7 && episode == "3")
        );
    }

    #[test]
    fn typing_and_focus_go_to_the_front_end() {
        let routed = RemoteCommand::Text { text: "moon".into(), submit: true }.route();
        assert_eq!(
            routed,
            Routed::Front(FrontIntent::Text { text: "moon".into(), submit: true })
        );
    }

    #[test]
    fn volume_is_the_tvs_own_and_needs_nothing_playing() {
        let command = RemoteCommand::Volume { step: 500.0 };
        assert!(!command.needs_playback());
        assert_eq!(command.route(), Routed::Front(FrontIntent::Volume { step: 100.0 }));
    }

    #[test]
    fn out_of_range_values_are_clamped_before_the_engine() {
        let routed = RemoteCommand::Speed { speed: 40.0 }.route();
        assert_eq!(routed, Routed::Engine(Command::Player(PlayerCommand::SpeedTo(4.0))));
    }

    #[test]
    fn commands_read_as_the_doc_writes_them() {
        let read: RemoteCommand = serde_json::from_str(r#"{"name":"seek","by":-10}"#).unwrap();
        assert_eq!(read, RemoteCommand::Seek { by: -10.0 });
        let read: RemoteCommand =
            serde_json::from_str(r#"{"name":"play","id":101,"episode":"12"}"#).unwrap();
        assert_eq!(read, RemoteCommand::Play { id: AnilistId::new(101), episode: "12".into() });
    }
}
