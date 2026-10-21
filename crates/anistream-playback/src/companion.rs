//! The companion's shapes: the state the player's own interface is shown, and what the viewer
//! did in it.
//!
//! The state is serialised as the companion script reads it, so a key renamed here is a blank
//! card there. The tests beside the messages that carry it to the script pin those forms.

use serde::Serialize;

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Level {
    Info,
    Alert,
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
