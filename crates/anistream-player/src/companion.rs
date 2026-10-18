//! The host's half of the companion-script protocol.
//!
//! The companion is the Lua script anistream ships into mpv (`assets/mpv/scripts/anistream`).
//! It draws the skip button, the Up Next card, the loading card and the in-player menus, and it
//! takes the viewer's input there — but it decides nothing. Every choice comes back here as an
//! event, because the decisions need what only the host has: skip intervals, the episode list,
//! the alternate streams, the watch history.
//!
//! Three channels, all plain mpv:
//!
//! - **State** the script should always see goes in `user-data/anistream/<key>`, which it
//!   observes. A property rather than a message, so a value set before the script finished
//!   loading is still there when it looks.
//! - **One-off instructions** ("offer a skip now") are `script-message-to anistream`.
//! - **What the viewer did** arrives as `script-message anistream <event> <json>`, which
//!   reaches the IPC socket as a `client-message`.
//!
//! The shapes here are the contract with the Lua side; the script's own header lists the same
//! keys. Changing one without the other fails quietly, which is why the serialised forms are
//! pinned in tests below.

use serde::Serialize;

use crate::protocol::Command;

/// The protocol revision, published so a script from another build can tell it is mismatched.
pub const PROTOCOL: u32 = 1;

/// The episode on screen.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct EpisodeInfo {
    pub number: String,
    pub label: String,
    pub title: Option<String>,
    pub show: String,
    pub translation: &'static str,
}

/// What plays after this episode, for the Up Next card.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct NextUp {
    pub number: String,
    pub label: String,
    pub title: Option<String>,
    /// Where the card appears: the ending's start when aniskip knows it. `None` leaves the
    /// script to place it near the end of the file.
    pub at: Option<f64>,
    /// Whether the episode will roll on by itself at the end.
    pub auto: bool,
    /// The episode's still, once it has been fetched and written out for `overlay-add`.
    pub thumbnail: Option<Still>,
}

/// An image the companion can show with `overlay-add`: raw BGRA, `width * 4` bytes a row.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Still {
    pub path: String,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Phase {
    /// A stream is being found or opened: the loading card is up.
    Loading,
    Playing,
    Idle,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Status {
    pub phase: Phase,
    pub text: String,
    pub detail: Option<String>,
}

impl Status {
    pub fn loading(text: impl Into<String>, detail: Option<String>) -> Self {
        Self { phase: Phase::Loading, text: text.into(), detail }
    }

    pub fn playing() -> Self {
        Self { phase: Phase::Playing, text: String::new(), detail: None }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct EpisodeItem {
    pub number: String,
    pub label: String,
    pub title: Option<String>,
    pub watched: bool,
    pub filler: bool,
    pub current: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SourceItem {
    pub id: String,
    pub label: String,
    pub detail: Option<String>,
    pub current: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct TranslationMenu {
    pub current: &'static str,
    pub options: Vec<&'static str>,
}

/// Everything the in-player menus list.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Menus {
    pub episodes: Vec<EpisodeItem>,
    pub sources: Vec<SourceItem>,
    pub translation: TranslationMenu,
    /// `{current, options: [{id, label, hint}]}`, built by the upscaling chooser.
    pub upscaling: serde_json::Value,
    /// The same shape, or null while the picture is not HDR — the choice means nothing then.
    pub tone_mapping: serde_json::Value,
    pub night_mode: bool,
}

/// Badges for what the picture is and what is being done to it.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct Media {
    pub hdr: Option<&'static str>,
    pub upscaler: Option<String>,
}

/// Publish one state key.
pub fn state(key: &'static str, value: &impl Serialize) -> Command {
    Command::SetUserData(key, serde_json::to_value(value).unwrap_or(serde_json::Value::Null))
}

/// Clear one state key; the script treats null as absent.
pub fn clear(key: &'static str) -> Command {
    Command::SetUserData(key, serde_json::Value::Null)
}

/// Show the skip button for the interval the playhead just entered.
pub fn skip_offer(kind: &str, label: &str, to: f64) -> Command {
    message("skip-offer", serde_json::json!({ "kind": kind, "label": label, "to": to }))
}

pub fn skip_clear() -> Command {
    message("skip-clear", serde_json::json!({}))
}

/// An automatic skip just happened; the script offers to undo it.
pub fn skipped(kind: &str, label: &str, from: f64, to: f64) -> Command {
    message(
        "skipped",
        serde_json::json!({ "kind": kind, "label": label, "from": from, "to": to }),
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Level {
    Info,
    Alert,
}

/// A themed message on the video — what `show-text` was used for before the companion.
pub fn toast(text: &str, level: Level) -> Command {
    message("toast", serde_json::json!({ "text": text, "level": level, "duration": 3 }))
}

/// The stream has been held waiting for data this long, mid-episode.
pub fn stall(seconds: u64) -> Command {
    message("stall", serde_json::json!({ "seconds": seconds }))
}

pub fn stall_clear() -> Command {
    message("stall-clear", serde_json::json!({}))
}

fn message(command: &'static str, payload: serde_json::Value) -> Command {
    Command::ScriptMessage { command, payload }
}

/// A menu pick.
#[derive(Debug, Clone, PartialEq)]
pub enum Choice {
    Episode(String),
    Source(String),
    Translation(String),
    Upscaling(String),
    ToneMapping(String),
    NightMode(bool),
}

/// Something the viewer did inside the player.
#[derive(Debug, Clone, PartialEq)]
pub enum ScriptEvent {
    /// The script started; `uosc` says whether it found uosc to draw menus with.
    Ready {
        uosc: bool,
    },
    Next,
    Previous,
    /// Take the skip currently on offer.
    Skip,
    /// Undo the last automatic skip by going back to where it started.
    UndoSkip {
        to: f64,
    },
    /// Up Next's "play now".
    PlayNext,
    /// Do not roll on when this episode ends.
    CancelNext,
    Choose(Choice),
    /// Answers to the stall prompt.
    StallSwitch,
    StallWait,
}

/// Read a `client-message` from the companion.
///
/// `None` for messages that are not ours or that carry a shape this build does not know —
/// another script on the bus, or a companion from a newer build. Neither is an error worth
/// interrupting playback for.
pub fn parse_event(args: &[String]) -> Option<ScriptEvent> {
    let [tag, event, rest @ ..] = args else { return None };
    if tag != "anistream" {
        return None;
    }
    let payload: serde_json::Value =
        rest.first().and_then(|raw| serde_json::from_str(raw).ok()).unwrap_or_default();
    Some(match event.as_str() {
        "ready" => ScriptEvent::Ready {
            uosc: payload.get("uosc").and_then(|v| v.as_bool()).unwrap_or(false),
        },
        "next" => ScriptEvent::Next,
        "previous" => ScriptEvent::Previous,
        "skip" => ScriptEvent::Skip,
        "undo-skip" => ScriptEvent::UndoSkip { to: payload.get("to")?.as_f64()? },
        "play-next" => ScriptEvent::PlayNext,
        "cancel-next" => ScriptEvent::CancelNext,
        "stall-switch" => ScriptEvent::StallSwitch,
        "stall-wait" => ScriptEvent::StallWait,
        "choose" => ScriptEvent::Choose(parse_choice(&payload)?),
        _ => return None,
    })
}

fn parse_choice(payload: &serde_json::Value) -> Option<Choice> {
    let menu = payload.get("menu")?.as_str()?;
    let value = payload.get("value")?;
    // Ids arrive as strings, but a script that sent a bare number for an episode means the
    // same thing; refusing it would make the menu silently do nothing.
    let text = || match value {
        serde_json::Value::String(s) => Some(s.clone()),
        serde_json::Value::Number(n) => Some(n.to_string()),
        _ => None,
    };
    Some(match menu {
        "episode" => Choice::Episode(text()?),
        "source" => Choice::Source(text()?),
        "translation" => Choice::Translation(text()?),
        "upscaling" => Choice::Upscaling(text()?),
        "tone_mapping" => Choice::ToneMapping(text()?),
        "night_mode" => Choice::NightMode(value.as_bool()?),
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(parts: &[&str]) -> Vec<String> {
        parts.iter().map(|p| (*p).to_owned()).collect()
    }

    #[test]
    fn the_existing_episode_keys_still_arrive() {
        // N and P inside the player predate the companion; the old shim sent them bare.
        assert_eq!(parse_event(&args(&["anistream", "next"])), Some(ScriptEvent::Next));
        assert_eq!(parse_event(&args(&["anistream", "previous"])), Some(ScriptEvent::Previous));
    }

    #[test]
    fn another_scripts_messages_are_not_ours() {
        assert_eq!(parse_event(&args(&["uosc", "next"])), None);
        assert_eq!(parse_event(&args(&["anistream"])), None);
    }

    #[test]
    fn an_undo_without_a_position_is_refused_rather_than_sent_to_zero() {
        // Seeking to 0 on a malformed undo would throw the viewer back to the cold open.
        assert_eq!(parse_event(&args(&["anistream", "undo-skip", "{}"])), None);
        assert_eq!(
            parse_event(&args(&["anistream", "undo-skip", r#"{"to":12.5}"#])),
            Some(ScriptEvent::UndoSkip { to: 12.5 })
        );
    }

    #[test]
    fn menu_picks_are_typed_by_menu() {
        let pick = |json: &str| parse_event(&args(&["anistream", "choose", json]));
        assert_eq!(
            pick(r#"{"menu":"episode","value":"7"}"#),
            Some(ScriptEvent::Choose(Choice::Episode("7".into())))
        );
        assert_eq!(
            pick(r#"{"menu":"episode","value":7}"#),
            Some(ScriptEvent::Choose(Choice::Episode("7".into())))
        );
        assert_eq!(
            pick(r#"{"menu":"night_mode","value":true}"#),
            Some(ScriptEvent::Choose(Choice::NightMode(true)))
        );
        assert_eq!(pick(r#"{"menu":"night_mode","value":"on"}"#), None);
        assert_eq!(pick(r#"{"menu":"nonsense","value":"x"}"#), None);
    }

    #[test]
    fn readiness_reports_whether_uosc_draws_the_menus() {
        assert_eq!(
            parse_event(&args(&["anistream", "ready", r#"{"protocol":1,"uosc":true}"#])),
            Some(ScriptEvent::Ready { uosc: true })
        );
    }

    #[test]
    fn state_serialises_to_the_shapes_the_script_reads() {
        // These key names are read by the Lua side; a rename here is a blank card there.
        let next = NextUp {
            number: "6".into(),
            label: "Episode 6".into(),
            title: None,
            at: Some(1310.0),
            auto: true,
            thumbnail: None,
        };
        let Command::SetUserData(key, value) = state("next", &next) else { panic!() };
        assert_eq!(key, "next");
        assert_eq!(value["label"], "Episode 6");
        assert_eq!(value["at"], 1310.0);
        assert_eq!(value["auto"], true);

        let Command::SetUserData(_, value) =
            state("status", &Status::loading("Opening episode 6", None))
        else {
            panic!()
        };
        assert_eq!(value["phase"], "loading");

        let Command::ScriptMessage { command, payload } = toast("hello", Level::Alert) else {
            panic!()
        };
        assert_eq!(command, "toast");
        assert_eq!(payload["level"], "alert");
    }
}
