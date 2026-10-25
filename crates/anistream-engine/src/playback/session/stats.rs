//! Stats for nerds as one file plays: the figures as the player reports them, the panel's
//! pace, and the TV's summary said whenever it changes.
//!
//! The panel's own wording is [`crate::playback::stats`]. While the panel is open the player is
//! asked for [`SAMPLED`] once a second, and the rows are said a moment after each ask, once the
//! answers are in. With it shut nothing is asked, and only the summary is said, from what the
//! player observes anyway.

use std::{collections::HashMap, time::Duration};

use anistream_core::stream::Stream;
use anistream_model::hosted::PlayerStats;
use anistream_playback::PlaybackEvent;
use serde_json::{Value, json};
use tokio::time::Instant;

use super::{Run, upscaling::PictureState};
use crate::playback::{
    shaders,
    stats::{self, Drops, Figures, SAMPLED, Screen, Upscale},
};

/// How long after asking the player for its figures the rows are said.
const ANSWERED: Duration = Duration::from_millis(250);
/// How often the panel is said, open.
const EVERY: Duration = Duration::from_secs(1);

/// What the panel needs next.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Next {
    /// Ask the player for its figures.
    Ask,
    /// Say the rows.
    Say,
}

/// One file's stats.
pub(super) struct StatsWatch {
    /// mpv's figures by property, as reported.
    mpv: HashMap<String, Value>,
    screen: Option<Screen>,
    drops: Drops,
    drops_rising: bool,
    /// The panel is open: when it next needs something, and what.
    due: Option<(Instant, Next)>,
    /// The summary as last said, so it is said again only when it changes.
    said: Option<Vec<String>>,
    clock: std::time::Instant,
}

impl StatsWatch {
    /// A file starting, with the panel open or not as the window has it.
    pub(super) fn new(open: bool) -> Self {
        let mut watch = Self {
            mpv: HashMap::new(),
            screen: None,
            drops: Drops::default(),
            drops_rising: false,
            due: None,
            said: None,
            clock: std::time::Instant::now(),
        };
        watch.open(open);
        watch
    }

    /// Open the panel, which asks the player at once, or shut it.
    pub(super) fn open(&mut self, open: bool) {
        self.due = open.then(|| (Instant::now(), Next::Ask));
    }

    pub(super) fn is_open(&self) -> bool {
        self.due.is_some()
    }

    /// When the panel next needs something, while it is open.
    pub(super) fn due(&self) -> Option<Instant> {
        self.due.map(|(at, _)| at)
    }

    /// The panel's turn has come: `true` to ask the player for its figures, `false` to say the
    /// rows.
    pub(super) fn turn(&mut self) -> bool {
        let now = Instant::now();
        match self.due {
            Some((_, Next::Ask)) => {
                self.due = Some((now + ANSWERED, Next::Say));
                true
            }
            _ => {
                self.due = Some((now + EVERY - ANSWERED, Next::Ask));
                let total = ["decoder-frame-drop-count", "frame-drop-count"]
                    .iter()
                    .filter_map(|p| self.mpv.get(*p).and_then(Value::as_u64))
                    .sum();
                self.drops_rising = self.drops.note(self.clock.elapsed().as_secs_f64(), total);
                false
            }
        }
    }

    /// Take in what the player said, for the figures it carries.
    pub(super) fn observe(&mut self, event: &PlaybackEvent) {
        let (name, value) = match event {
            PlaybackEvent::Sampled { name, value } => (name.as_str(), value.clone()),
            PlaybackEvent::VideoParams(params) => ("video-params", params.clone()),
            PlaybackEvent::VideoTrack(track) => ("current-tracks/video", track.clone()),
            PlaybackEvent::FrameDrops(count) => ("frame-drop-count", json!(count)),
            PlaybackEvent::SubDelay(delay) => ("sub-delay", json!(delay)),
            PlaybackEvent::Display { width, height, refresh_hz, edr_headroom, .. } => {
                self.screen = Some(Screen {
                    width: *width,
                    height: *height,
                    refresh_hz: *refresh_hz,
                    edr_headroom: *edr_headroom,
                });
                return;
            }
            _ => return,
        };
        self.mpv.insert(name.to_owned(), value);
    }

    /// The summary, when it differs from what was last said.
    pub(super) fn summary_changed(&mut self, stats: &PlayerStats) -> bool {
        if self.said.as_ref() == Some(&stats.summary) {
            return false;
        }
        self.said = Some(stats.summary.clone());
        true
    }
}

/// What the stats are made from beside the player's figures, as the file loop has it.
pub(super) struct Playing<'a> {
    pub picture: &'a PictureState,
    pub source: Option<&'a shaders::Source>,
    pub stream: &'a Stream,
    pub streams: &'a [Stream],
    pub index: usize,
    pub paused: bool,
    pub buffering: bool,
}

impl Run<'_> {
    /// The stats as they stand: the rows too while the panel is open, or always with `rows`.
    pub(super) fn stats(
        &self,
        watch: &StatsWatch,
        playing: &Playing<'_>,
        rows: bool,
    ) -> PlayerStats {
        let picture = playing.picture;
        let upscaling = playing.source.map(|source| {
            stats::upscaling(&Upscale {
                look: self.look,
                shaders: self.settings.shader_dir.is_some(),
                source: *source,
                display: picture.display,
                budget: shaders::Budget {
                    on_battery: picture.on_battery,
                    eased: picture.governor.eased(),
                },
                hook_failed: picture.failed.is_some(),
            })
        });
        let url = &playing.stream.url;
        let figures = Figures {
            mpv: watch.mpv.clone(),
            screen: watch.screen,
            upscaling,
            source: playing.streams.get(playing.index).map(|s| s.provider_id.clone()),
            // A download plays from its path; a stream, even the torrent's loopback one, from
            // an address.
            local: !url.contains("://") || url.starts_with("file:"),
            offline: self.offline.load(std::sync::atomic::Ordering::Relaxed),
            paused: playing.paused,
            buffering: playing.buffering,
            drops_rising: watch.drops_rising,
        };
        if rows || watch.is_open() { stats::stats(&figures) } else { stats::closed(&figures) }
    }
}

/// What the player is asked for, as the link takes it.
pub(super) fn sampled() -> Vec<String> {
    SAMPLED.iter().map(|p| (*p).to_owned()).collect()
}
