//! The engine's hosted player protocol in mpv's terms: each `PlayerOp` as the one call on mpv it
//! is, and mpv's events as the `PlayerReport`s the engine reads. After the Android app's
//! `HostedOps.kt` and the Apple app's `HostedOps.swift`, so all three hosts agree.

use anistream_model::hosted::{MpvValue, PlayerEnd, PlayerOp};
use serde_json::{Value, json};

/// How `seek` lands: exactly where asked, or on the keyframe nearest, for scrubbing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SeekMode {
    Absolute,
    Relative,
    Keyframe,
}

impl SeekMode {
    /// The flags `seek` takes for it.
    pub fn flags(self) -> &'static str {
        match self {
            Self::Absolute => "absolute+exact",
            Self::Relative => "relative+exact",
            Self::Keyframe => "absolute+keyframes",
        }
    }
}

/// One call on mpv, for one of the engine's ops.
#[derive(Debug, Clone, PartialEq)]
pub enum MpvCall {
    /// `loadfile <url> replace` with per-file options. A later option of the same name wins, as
    /// it would on mpv's command line.
    Load {
        url: String,
        options: Vec<(String, String)>,
    },
    /// `sub-add <url> auto`: listed, not selected.
    AddSubtitle {
        url: String,
        title: Option<String>,
        language: Option<String>,
    },
    /// Set a property, its value as JSON: a node for a list or a map, a scalar otherwise.
    Set {
        property: String,
        value: Value,
    },
    /// An input command as mpv spells it: `cycle pause`, `add volume 5`, `af add ...`, `stop`.
    Command(Vec<String>),
    Seek {
        seconds: f64,
        mode: SeekMode,
    },
    /// `glsl-shaders` as one list, so no path is split on a separator.
    Shaders(Vec<String>),
    /// Read a property and report it as it stands: `chapter-list` for `RestateChapters`.
    Restate(String),
}

impl MpvCall {
    /// The call as a named-argument command node, for the calls that are commands: `loadfile`
    /// and `sub-add` take their options this way, which spares positional arguments that have
    /// moved between mpv releases.
    pub fn command_node(&self) -> Option<Value> {
        match self {
            Self::Load { url, options } => {
                let options: serde_json::Map<String, Value> = options
                    .iter()
                    .map(|(k, v)| (k.clone(), Value::String(v.clone())))
                    .collect();
                let mut node = json!({"name": "loadfile", "url": url, "flags": "replace"});
                if !options.is_empty() {
                    node["options"] = Value::Object(options);
                }
                Some(node)
            }
            Self::AddSubtitle { url, title, language } => {
                let mut node = json!({"name": "sub-add", "url": url, "flags": "auto"});
                if let Some(title) = title {
                    node["title"] = json!(title);
                }
                if let Some(language) = language {
                    node["lang"] = json!(language);
                }
                Some(node)
            }
            Self::Seek { seconds, mode } => {
                Some(json!({"name": "seek", "target": seconds, "flags": mode.flags()}))
            }
            Self::Command(_) | Self::Set { .. } | Self::Shaders(_) | Self::Restate(_) => None,
        }
    }
}

/// What the host does for one of the engine's ops.
#[derive(Debug, Clone, PartialEq)]
pub enum Step {
    /// Show the player, at the viewer's speed and volume.
    Open {
        fullscreen: bool,
        speed: Option<f64>,
        volume: Option<f64>,
    },
    Call(MpvCall),
    /// Full screen on, off, or (`None`) the other way: the window's, which mpv knows nothing of.
    Fullscreen(Option<bool>),
    /// The viewer stopped: end the file, report that it ended by quitting and that the player
    /// closed.
    Quit,
    /// The playback is over: close the player, and say nothing more.
    Close,
}

impl Step {
    pub fn of(op: &PlayerOp) -> Self {
        match op {
            PlayerOp::Open { fullscreen, speed, volume } => {
                Self::Open { fullscreen: *fullscreen, speed: *speed, volume: *volume }
            }
            PlayerOp::Load { url, options } => {
                // A later option of the same name wins.
                let mut kept: Vec<(String, String)> = Vec::with_capacity(options.len());
                for option in options {
                    kept.retain(|(name, _)| name != &option.name);
                    kept.push((option.name.clone(), option.value.clone()));
                }
                Self::Call(MpvCall::Load { url: url.clone(), options: kept })
            }
            PlayerOp::AddSubtitle { url, title, language } => {
                Self::Call(MpvCall::AddSubtitle {
                    url: url.clone(),
                    title: title.clone(),
                    language: language.clone(),
                })
            }
            PlayerOp::Set { property, value } => match (property.as_str(), value) {
                ("fullscreen", MpvValue::Flag(on)) => Self::Fullscreen(Some(*on)),
                _ => Self::Call(MpvCall::Set {
                    property: property.clone(),
                    value: json_of(value),
                }),
            },
            PlayerOp::Cycle { property } if property == "fullscreen" => Self::Fullscreen(None),
            PlayerOp::Cycle { property } => {
                Self::Call(MpvCall::Command(vec!["cycle".into(), property.clone()]))
            }
            PlayerOp::ToggleFullscreen => Self::Fullscreen(None),
            PlayerOp::Add { property, delta } => Self::Call(MpvCall::Command(vec![
                "add".into(),
                property.clone(),
                delta.to_string(),
            ])),
            PlayerOp::Seek { seconds, absolute } => Self::Call(MpvCall::Seek {
                seconds: *seconds,
                mode: if *absolute { SeekMode::Absolute } else { SeekMode::Relative },
            }),
            PlayerOp::Shaders { paths } => Self::Call(MpvCall::Shaders(paths.clone())),
            PlayerOp::AudioFilter { add, filter } => Self::Call(MpvCall::Command(vec![
                "af".into(),
                if *add { "add" } else { "remove" }.into(),
                filter.clone(),
            ])),
            PlayerOp::RestateChapters => Self::Call(MpvCall::Restate("chapter-list".into())),
            PlayerOp::Stop => Self::Call(MpvCall::Command(vec!["stop".into()])),
            PlayerOp::Quit => Self::Quit,
            PlayerOp::Close => Self::Close,
        }
    }
}

/// The engine's value as JSON, as mpv takes it: a node arrives as JSON text, and text that is
/// not JSON is `null`, which sets nothing.
pub fn json_of(value: &MpvValue) -> Value {
    match value {
        MpvValue::Flag(b) => Value::Bool(*b),
        MpvValue::Int(i) => Value::Number((*i).into()),
        MpvValue::Double(d) => {
            serde_json::Number::from_f64(*d).map_or(Value::Null, Value::Number)
        }
        MpvValue::String(s) => Value::String(s.clone()),
        MpvValue::Node(text) => serde_json::from_str(text).unwrap_or(Value::Null),
    }
}

/// The properties the engine reads, which the player observes from the start.
pub const REPORTED_PROPERTIES: &[&str] = &[
    "time-pos",
    "duration",
    "pause",
    "speed",
    "volume",
    "chapter-list",
    "video-params",
    "current-tracks/video",
    "track-list",
    "aid",
    "sid",
    "sub-delay",
    "paused-for-cache",
    "frame-drop-count",
    "seeking",
];

/// mpv's `end-file` reason, by its number in `mpv_end_file_reason`.
pub fn player_end(reason: i32) -> PlayerEnd {
    match reason {
        0 => PlayerEnd::Eof,
        2 => PlayerEnd::Stop,
        3 => PlayerEnd::Quit,
        4 => PlayerEnd::Error,
        5 => PlayerEnd::Redirect,
        _ => PlayerEnd::Unknown,
    }
}

#[cfg(test)]
mod tests {
    use anistream_model::hosted::MpvOption;

    use super::*;

    fn option(name: &str, value: &str) -> MpvOption {
        MpvOption { name: name.into(), value: value.into() }
    }

    #[test]
    fn a_load_keeps_the_last_of_each_option_and_goes_as_named_arguments() {
        let op = PlayerOp::Load {
            url: "http://127.0.0.1:8123/mend/a".into(),
            options: vec![
                option("start", "10"),
                option("title", "Episode 2"),
                option("start", "12.5"),
            ],
        };
        let Step::Call(call) = Step::of(&op) else { panic!("a load is a call") };
        assert_eq!(
            call,
            MpvCall::Load {
                url: "http://127.0.0.1:8123/mend/a".into(),
                options: vec![
                    ("title".into(), "Episode 2".into()),
                    ("start".into(), "12.5".into())
                ],
            }
        );
        assert_eq!(
            call.command_node().unwrap(),
            json!({"name": "loadfile", "url": "http://127.0.0.1:8123/mend/a", "flags": "replace",
                "options": {"title": "Episode 2", "start": "12.5"}})
        );
    }

    #[test]
    fn fullscreen_is_the_windows_never_mpvs() {
        assert_eq!(Step::of(&PlayerOp::ToggleFullscreen), Step::Fullscreen(None));
        assert_eq!(
            Step::of(&PlayerOp::Cycle { property: "fullscreen".into() }),
            Step::Fullscreen(None)
        );
        let set = PlayerOp::Set { property: "fullscreen".into(), value: MpvValue::Flag(true) };
        assert_eq!(Step::of(&set), Step::Fullscreen(Some(true)));
    }

    #[test]
    fn each_op_is_the_one_call_it_names() {
        let cases = [
            (
                PlayerOp::Cycle { property: "pause".into() },
                MpvCall::Command(vec!["cycle".into(), "pause".into()]),
            ),
            (
                PlayerOp::Add { property: "volume".into(), delta: 5.0 },
                MpvCall::Command(vec!["add".into(), "volume".into(), "5".into()]),
            ),
            (
                PlayerOp::Seek { seconds: -10.0, absolute: false },
                MpvCall::Seek { seconds: -10.0, mode: SeekMode::Relative },
            ),
            (
                PlayerOp::AudioFilter { add: true, filter: "@night:dynaudnorm".into() },
                MpvCall::Command(vec!["af".into(), "add".into(), "@night:dynaudnorm".into()]),
            ),
            (PlayerOp::RestateChapters, MpvCall::Restate("chapter-list".into())),
            (PlayerOp::Stop, MpvCall::Command(vec!["stop".into()])),
            (
                PlayerOp::Shaders { paths: vec!["/s/a b.glsl".into(), "/s/c.glsl".into()] },
                MpvCall::Shaders(vec!["/s/a b.glsl".into(), "/s/c.glsl".into()]),
            ),
            (
                PlayerOp::Set { property: "sid".into(), value: MpvValue::Int(2) },
                MpvCall::Set { property: "sid".into(), value: json!(2) },
            ),
            (
                PlayerOp::Set {
                    property: "chapter-list".into(),
                    value: MpvValue::Node(r#"[{"title":"A","time":0}]"#.into()),
                },
                MpvCall::Set {
                    property: "chapter-list".into(),
                    value: json!([{"title": "A", "time": 0}]),
                },
            ),
        ];
        for (op, call) in cases {
            assert_eq!(Step::of(&op), Step::Call(call), "{op:?}");
        }
        assert_eq!(Step::of(&PlayerOp::Quit), Step::Quit);
        assert_eq!(Step::of(&PlayerOp::Close), Step::Close);
    }

    #[test]
    fn a_seek_and_a_subtitle_go_as_named_arguments() {
        let seek = MpvCall::Seek { seconds: 90.0, mode: SeekMode::Absolute };
        assert_eq!(
            seek.command_node().unwrap(),
            json!({"name": "seek", "target": 90.0, "flags": "absolute+exact"})
        );
        let sub = MpvCall::AddSubtitle {
            url: "/tmp/a.ass".into(),
            title: Some("Signs".into()),
            language: None,
        };
        assert_eq!(
            sub.command_node().unwrap(),
            json!({"name": "sub-add", "url": "/tmp/a.ass", "flags": "auto", "title": "Signs"})
        );
    }

    #[test]
    fn a_node_that_is_not_json_sets_nothing() {
        assert_eq!(json_of(&MpvValue::Node("not json".into())), Value::Null);
        assert_eq!(json_of(&MpvValue::Double(f64::INFINITY)), Value::Null);
    }

    #[test]
    fn end_reasons_are_mpvs() {
        assert_eq!(player_end(0), PlayerEnd::Eof);
        assert_eq!(player_end(2), PlayerEnd::Stop);
        assert_eq!(player_end(3), PlayerEnd::Quit);
        assert_eq!(player_end(4), PlayerEnd::Error);
        assert_eq!(player_end(5), PlayerEnd::Redirect);
        assert_eq!(player_end(1), PlayerEnd::Unknown);
    }
}
