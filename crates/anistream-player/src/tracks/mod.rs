//! Audio and subtitle track selection, remembered per series.
//!
//! mpv's own selection works from `--alang` and `--slang`, which gets a single-audio release
//! with one subtitle track right and not much else. A dual-audio release routinely carries two
//! or three subtitle tracks in the same language — signs and songs, full dialogue, SDH — and a
//! language list cannot tell them apart; a commentary track tagged Japanese satisfies
//! `--alang=ja` exactly as well as the programme does. External files are worse. mpv knows
//! nothing about the language of a file it is handed by URL, and it prefers external tracks to
//! embedded ones: measured on 0.41, a `spa.forced.srt` passed with `--sub-file` was chosen over
//! the release's own default English track, and an autoloaded SDH file over full subtitles.
//!
//! So this reads the track list itself, picks by what each track *is*, and remembers what the
//! viewer changes so the next episode starts from their choice. Pure, like
//! [`crate::tracker`]: it takes [`TrackInput`]s and returns [`TrackAction`]s, touching no
//! socket and no database, so every rule here is testable without a player.
//!
//! # Wiring
//!
//! Observe the properties in [`OBSERVED`] and turn each change into an input with
//! [`TrackInput::from_property`]; send `file-loaded` and `end-file` as [`TrackInput::FileLoaded`]
//! and [`TrackInput::EndFile`], and [`TrackInput::Tick`] on any regular cadence — each
//! `time-pos` report will do — so a subtitle delay the viewer has stopped nudging gets
//! remembered. Carry out each action with [`TrackAction::property`], and write
//! [`TrackAction::Remember`] to the store under the series and translation the policy was
//! prepared for.
//!
//! One policy is meant to live as long as the mpv process. Between episodes,
//! [`TrackPolicy::prepare`] hands it the next file's preferences and memory.
//!
//! # Layout
//!
//! The policy itself is here: what it is fed and what it answers with, and how it follows
//! mpv's selection and tells the viewer's changes from its own. What a track is
//! ([`Track`]), how languages are compared, the rules that choose a file's tracks, and what
//! a series remembers and how a remembered track is found again each have a file of their
//! own.

use std::time::{Duration, Instant};

use anistream_core::Translation;
pub use anistream_core::track_memory::{
    SubtitleChoice, SubtitleMemory, TrackMemory, TrackPrint,
};
use serde_json::Value;

mod choice;
#[cfg(test)]
mod fixtures;
mod language;
mod memory;
mod track;

pub use language::language_matches;
pub use track::{Track, TrackKind, parse_track_id, parse_track_list};

/// The mpv properties the policy needs observed, in `observe_property`'s `native` form.
pub const OBSERVED: [&str; 4] = ["track-list", "aid", "sid", "sub-delay"];

/// How long after we have chosen a file's tracks a selection change is still taken for mpv or
/// a script finishing its own work rather than the viewer.
///
/// mpv selects before it announces `file-loaded` and reports the result within milliseconds
/// (about one, measured on 0.41), so our own pick lands almost at once. What can still move the
/// selection after that is a late `sub-add`, which selects the file it adds, or a script
/// reacting to the load. Two seconds covers those with room, and a viewer is rarely already
/// reaching for the subtitle key in the first two seconds of an episode.
pub const SETTLE: Duration = Duration::from_secs(2);

/// How long the subtitle delay has to sit still before it is remembered.
///
/// The viewer moves it in 0.1 s steps, several presses a second; remembering each step would
/// write a row per press, and the steps on the way are not the answer.
pub const DELAY_QUIET: Duration = Duration::from_millis(1500);

/// Something mpv told us that bears on track choice.
#[derive(Debug, Clone, PartialEq)]
pub enum TrackInput {
    /// mpv's `file-loaded`.
    FileLoaded,
    /// mpv's `end-file`. The file's tracks are gone with it.
    EndFile,
    TrackList(Vec<Track>),
    Aid(Option<i64>),
    Sid(Option<i64>),
    /// `sub-delay`, in seconds.
    SubDelay(f64),
    /// Nothing happened; time passed.
    Tick,
}

impl TrackInput {
    /// The input a `property-change` of `name` carries, if it is one of [`OBSERVED`] and says
    /// something definite.
    pub fn from_property(name: &str, data: &Value) -> Option<Self> {
        match name {
            "track-list" => Some(Self::TrackList(parse_track_list(data))),
            "aid" => parse_track_id(data).map(Self::Aid),
            "sid" => parse_track_id(data).map(Self::Sid),
            "sub-delay" => data.as_f64().map(Self::SubDelay),
            _ => None,
        }
    }
}

/// Something the caller should do.
#[derive(Debug, Clone, PartialEq)]
pub enum TrackAction {
    SetAid(i64),
    /// `None` turns subtitles off.
    SetSid(Option<i64>),
    SetSubDelay(f64),
    /// Store this as the series' memory, replacing what was there.
    Remember(TrackMemory),
}

impl TrackAction {
    /// The property and value for mpv's `set_property`; `None` for [`Self::Remember`], which
    /// is the store's.
    pub fn property(&self) -> Option<(&'static str, Value)> {
        match self {
            Self::SetAid(id) => Some(("aid", (*id).into())),
            Self::SetSid(Some(id)) => Some(("sid", (*id).into())),
            // Verified on 0.41: `"no"` and `false` both turn subtitles off and read back as
            // `false`.
            Self::SetSid(None) => Some(("sid", "no".into())),
            Self::SetSubDelay(seconds) => Some(("sub-delay", (*seconds).into())),
            Self::Remember(_) => None,
        }
    }
}

/// What the viewer wants, before any particular file is considered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrackPreferences {
    pub translation: Translation,
    /// The subtitle language — and for a dub watcher, the audio language too. `en`, `eng` and
    /// `english` are the same language here. `None` leaves subtitles to mpv.
    pub language: Option<String>,
}

/// Chooses a file's tracks, and notices when the viewer chooses differently.
#[derive(Debug, Clone)]
pub struct TrackPolicy {
    preferences: TrackPreferences,
    memory: TrackMemory,
    tracks: Vec<Track>,
    aid: Slot,
    sid: Slot,
    sub_delay: Option<f64>,
    /// The delay mpv had before any file was chosen for — the user's own `mpv.conf` value, or
    /// zero. What a series with no remembered delay returns to, because in a single mpv process
    /// a delay set during one series would otherwise carry into the next.
    baseline_delay: Option<f64>,
    /// A delay we set and have not yet seen come back.
    expect_delay: Option<f64>,
    /// When the viewer last moved the delay, while that move is still waiting to be remembered.
    delay_moved: Option<Instant>,
    /// Between `file-loaded` and `end-file`.
    loaded: bool,
    /// Whether this file's tracks have been chosen. Until then selection changes are mpv's.
    chosen: bool,
    settle_until: Option<Instant>,
    /// Whether any file has been chosen for yet; until then a delay report is the baseline.
    chosen_once: bool,
}

/// mpv's current selection of one kind, and the value we last asked for.
#[derive(Debug, Clone, Copy, Default)]
struct Slot {
    /// `None` until mpv has said.
    current: Option<Option<i64>>,
    /// Set by our own action, cleared when mpv reports it back.
    expect: Option<Option<i64>>,
}

impl TrackPolicy {
    pub fn new(preferences: TrackPreferences, memory: Option<TrackMemory>) -> Self {
        Self {
            preferences,
            memory: memory.unwrap_or_default(),
            tracks: Vec::new(),
            aid: Slot::default(),
            sid: Slot::default(),
            sub_delay: None,
            baseline_delay: None,
            expect_delay: None,
            delay_moved: None,
            loaded: false,
            chosen: false,
            settle_until: None,
            chosen_once: false,
        }
    }

    /// Set up for the next file — another episode, or another series or translation.
    ///
    /// Returns what is still owed to the previous one: a delay the viewer moved that has not
    /// yet gone quiet. Store it under the *previous* series and translation.
    pub fn prepare(
        &mut self,
        preferences: TrackPreferences,
        memory: Option<TrackMemory>,
    ) -> Vec<TrackAction> {
        let mut actions = Vec::new();
        self.flush_delay(None, &mut actions);
        self.preferences = preferences;
        self.memory = memory.unwrap_or_default();
        actions
    }

    /// The series' memory as it stands, including choices made this session.
    pub fn memory(&self) -> &TrackMemory {
        &self.memory
    }

    /// Feed an input, and get back what to do. `now` is any monotonic reading; only the
    /// distance between readings matters.
    pub fn observe(&mut self, input: &TrackInput, now: Instant) -> Vec<TrackAction> {
        let mut actions = Vec::new();
        // Every input is a clock reading, so a delay that has gone quiet is remembered on
        // whatever arrives next, not only on a tick.
        self.flush_delay(Some(now), &mut actions);

        match input {
            TrackInput::FileLoaded => {
                self.flush_delay(None, &mut actions);
                self.loaded = true;
                self.chosen = false;
                self.settle_until = None;
                self.aid.expect = None;
                self.sid.expect = None;
                self.expect_delay = None;
                // The list usually arrives just after this, carrying mpv's own pick, and the
                // choice waits for it. If it arrived first, it is this file's — `end-file`
                // emptied the previous one — and there is no reason to wait.
                if self.has_selection() {
                    self.choose(now, &mut actions);
                }
            }

            TrackInput::EndFile => {
                self.flush_delay(None, &mut actions);
                self.loaded = false;
                self.chosen = false;
                self.settle_until = None;
                self.tracks.clear();
                self.aid = Slot::default();
                self.sid = Slot::default();
                self.expect_delay = None;
            }

            TrackInput::TrackList(list) => {
                let added = list.iter().any(|t| self.find(t.kind, t.id).is_none());
                self.tracks = list.clone();
                // mpv restates the whole list on every selection change, so the list alone is
                // enough to follow the selection. A list with nothing selected at all is a file
                // still opening, not a file with everything turned off.
                if self.has_selection() {
                    let aid = self.selected(TrackKind::Audio);
                    let sid = self.selected(TrackKind::Sub);
                    self.selection_changed(TrackKind::Audio, aid, now, &mut actions);
                    self.selection_changed(TrackKind::Sub, sid, now, &mut actions);
                    if self.loaded && !self.chosen {
                        self.choose(now, &mut actions);
                    } else if self.loaded && added && self.settling(now) {
                        // A track arriving just after the load — a late `sub-add`, which also
                        // selects what it adds — is part of the file, and the choice is
                        // remade with it in view.
                        self.choose_tracks(&mut actions);
                    }
                }
            }

            TrackInput::Aid(id) => {
                self.selection_reported(TrackKind::Audio, *id, now, &mut actions)
            }
            TrackInput::Sid(id) => {
                self.selection_reported(TrackKind::Sub, *id, now, &mut actions)
            }
            TrackInput::SubDelay(seconds) => self.delay_changed(*seconds, now),
            TrackInput::Tick => {}
        }

        actions
    }

    fn has_selection(&self) -> bool {
        self.tracks.iter().any(|t| t.selected)
    }

    fn settling(&self, now: Instant) -> bool {
        self.settle_until.is_some_and(|until| now < until)
    }

    fn find(&self, kind: TrackKind, id: i64) -> Option<&Track> {
        self.tracks.iter().find(|t| t.kind == kind && t.id == id)
    }

    fn of_kind(&self, kind: TrackKind) -> Vec<&Track> {
        self.tracks.iter().filter(|t| t.kind == kind).collect()
    }

    /// The selected track of a kind, by the list. A secondary subtitle is not the subtitle.
    fn selected(&self, kind: TrackKind) -> Option<i64> {
        self.tracks
            .iter()
            .find(|t| t.kind == kind && t.selected && t.main_selection != Some(1))
            .map(|t| t.id)
    }

    fn slot(&mut self, kind: TrackKind) -> &mut Slot {
        if kind == TrackKind::Audio { &mut self.aid } else { &mut self.sid }
    }

    /// An `aid`/`sid` report.
    fn selection_reported(
        &mut self,
        kind: TrackKind,
        id: Option<i64>,
        now: Instant,
        actions: &mut Vec<TrackAction>,
    ) {
        // A track we have not seen listed cannot be described, and so cannot be remembered.
        // mpv restates the list on every selection change, so the list that introduces it
        // carries the same selection and is the one to act on.
        if id.is_some_and(|id| self.find(kind, id).is_none()) {
            return;
        }
        self.selection_changed(kind, id, now, actions);
    }

    fn selection_changed(
        &mut self,
        kind: TrackKind,
        id: Option<i64>,
        now: Instant,
        actions: &mut Vec<TrackAction>,
    ) {
        let viewer_can_choose = self.loaded && self.chosen && !self.settling(now);
        let slot = self.slot(kind);
        if slot.current == Some(id) {
            return;
        }
        slot.current = Some(id);
        // Our own choice coming back. Reading it as the viewer's would remember the heuristic
        // as though someone had chosen it, and the memory would never again be the viewer's.
        if slot.expect == Some(id) {
            slot.expect = None;
            return;
        }
        // Before the choice, it is mpv's own pick; just after, mpv or a script settling.
        if !viewer_can_choose {
            return;
        }
        slot.expect = None;
        self.remember_selection(kind, id, actions);
    }
}

fn same_delay(a: f64, b: f64) -> bool {
    // mpv steps the delay in tenths and reports back what it was given; a millisecond is far
    // below anything a viewer could set or see.
    (a - b).abs() < 1e-3
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::fixtures::*;

    fn remembered_subtitles(actions: &[TrackAction]) -> SubtitleMemory {
        remembered(actions)
            .last()
            .and_then(|m| m.subtitles.clone())
            .expect("a subtitle choice was remembered")
    }

    fn remembered_track(actions: &[TrackAction]) -> TrackPrint {
        match remembered_subtitles(actions).choice {
            SubtitleChoice::Track(print) => print,
            SubtitleChoice::Off => panic!("remembered off, not a track"),
        }
    }

    // --- Parsing ---------------------------------------------------------------------------

    #[test]
    fn only_the_observed_properties_become_inputs() {
        for name in OBSERVED {
            let data = if name == "track-list" { json!([]) } else { json!(1) };
            assert!(TrackInput::from_property(name, &data).is_some(), "{name} is not mapped");
        }
        assert_eq!(
            TrackInput::from_property("sid", &json!(false)),
            Some(TrackInput::Sid(None))
        );
        assert_eq!(TrackInput::from_property("aid", &json!("auto")), None);
        assert_eq!(
            TrackInput::from_property("sub-delay", &json!(-0.2)),
            Some(TrackInput::SubDelay(-0.2))
        );
        assert_eq!(TrackInput::from_property("volume", &json!(50)), None);
    }

    #[test]
    fn actions_become_the_properties_mpv_takes() {
        assert_eq!(TrackAction::SetAid(2).property(), Some(("aid", json!(2))));
        assert_eq!(TrackAction::SetSid(Some(3)).property(), Some(("sid", json!(3))));
        assert_eq!(TrackAction::SetSid(None).property(), Some(("sid", json!("no"))));
        assert_eq!(TrackAction::SetSubDelay(0.4).property(), Some(("sub-delay", json!(0.4))));
        assert_eq!(TrackAction::Remember(TrackMemory::default()).property(), None);
    }

    // --- The viewer's choices --------------------------------------------------------------

    #[test]
    fn our_own_choice_coming_back_is_not_the_viewers() {
        // Each set echoes back as a list and a property; none of that is a choice.
        let mut session = Session::new(sub_watcher(), None);
        let actions = session.load(dual_audio(), Some(1), Some(5));
        assert_eq!(sets(&actions).len(), 2);
        session.wait(5.0);
        // A late duplicate report of the same selection is no change either.
        let late = session.feed_all(vec![TrackInput::Aid(Some(2)), TrackInput::Sid(Some(2))]);
        assert!(late.is_empty(), "{late:?}");
        assert!(session.policy.memory().is_empty());
    }

    #[test]
    fn a_change_after_the_settle_window_is_the_viewers_and_is_remembered() {
        let mut session = Session::new(sub_watcher(), None);
        session.load(dual_audio(), Some(2), Some(2));
        session.wait(30.0);

        let actions = session.viewer_selects(TrackKind::Sub, Some(3));
        assert!(sets(&actions).is_empty(), "the viewer's choice is not fought: {actions:?}");
        assert_eq!(remembered(&actions).len(), 1, "remembered once, not once per report");
        let memory = remembered_subtitles(&actions);
        assert_eq!(
            memory.audio_lang.as_deref(),
            Some("jpn"),
            "with the audio it was made over"
        );
        let SubtitleChoice::Track(print) = memory.choice else { panic!("not a track") };
        assert_eq!(print.lang.as_deref(), Some("eng"));
        assert_eq!(print.title, "english sdh");
        assert!(print.hearing_impaired && !print.external);
    }

    #[test]
    fn a_change_inside_the_settle_window_is_neither_remembered_nor_fought() {
        // Just after the load, a change is far more often a script finishing its own work
        // than a viewer — and if it is the viewer, overriding them would be worse than merely
        // not remembering.
        let mut session = Session::new(sub_watcher(), None);
        session.load(dual_audio(), Some(2), Some(2));
        session.wait(0.5);
        assert!(session.viewer_selects(TrackKind::Sub, Some(3)).is_empty());
        assert_eq!(session.selection(), (Some(2), Some(3)));
    }

    #[test]
    fn a_track_added_just_after_the_load_is_considered() {
        // A late `sub-add` selects the file it adds. Inside the settle window the choice is
        // remade with the new track in view — here, back to the release's own English.
        let mut tracks = dual_audio();
        tracks.truncate(8);
        let mut session = Session::new(sub_watcher(), None);
        session.load(tracks, Some(2), Some(2));
        session.wait(0.5);

        for t in session.mpv.tracks.iter_mut().filter(|t| t.kind == TrackKind::Sub) {
            t.selected = false;
        }
        let mut added = external(5, "https://cdn.example/subs/spa.vtt");
        added.selected = true;
        added.main_selection = Some(0);
        session.mpv.tracks.push(added);
        let list = TrackInput::TrackList(session.mpv.tracks.clone());
        let actions = session.feed_all(vec![list, TrackInput::Sid(Some(5))]);
        assert_eq!(sets(&actions), [&TrackAction::SetSid(Some(2))]);
        assert!(remembered(&actions).is_empty());
    }

    #[test]
    fn turning_subtitles_off_is_remembered_as_off() {
        let mut session = Session::new(sub_watcher(), None);
        session.load(dual_audio(), Some(2), Some(2));
        session.wait(10.0);
        let actions = session.viewer_selects(TrackKind::Sub, None);
        assert_eq!(remembered_subtitles(&actions).choice, SubtitleChoice::Off);

        // The next episode starts with them off, though the heuristic would turn them on.
        session.next_episode();
        let actions = session.load(dual_audio(), Some(2), Some(2));
        assert_eq!(sets(&actions), [&TrackAction::SetSid(None)]);
        assert!(remembered(&actions).is_empty(), "applying a memory is not a new choice");
    }

    #[test]
    fn a_selection_reported_before_its_track_is_listed_waits_for_the_list() {
        let mut session = Session::new(sub_watcher(), None);
        session.load(dual_audio(), Some(2), Some(2));
        session.wait(10.0);

        // The viewer drags a file in, and `sid` names a track not yet listed.
        assert!(session.feed(TrackInput::Sid(Some(6))).is_empty());
        for t in session.mpv.tracks.iter_mut().filter(|t| t.kind == TrackKind::Sub) {
            t.selected = false;
        }
        let mut dropped = external(6, "/media/English Full.srt");
        dropped.selected = true;
        session.mpv.tracks.push(dropped);
        let actions = session.feed(TrackInput::TrackList(session.mpv.tracks.clone()));
        let print = remembered_track(&actions);
        assert_eq!(print.title, "english full");
        assert!(print.external);
    }

    #[test]
    fn reports_between_files_are_not_choices() {
        let mut session = Session::new(sub_watcher(), None);
        session.load(dual_audio(), Some(2), Some(2));
        session.wait(10.0);
        session.feed(TrackInput::EndFile);
        let actions = session.feed_all(vec![
            TrackInput::TrackList(Vec::new()),
            TrackInput::Aid(None),
            TrackInput::Sid(None),
        ]);
        assert!(actions.is_empty(), "{actions:?}");
    }

    #[test]
    fn commentary_is_listened_to_not_remembered() {
        let mut session = Session::new(sub_watcher(), None);
        session.load(dual_audio(), Some(2), Some(2));
        session.wait(10.0);
        assert!(session.viewer_selects(TrackKind::Audio, Some(3)).is_empty());
        assert!(session.policy.memory().audio.is_none());
    }

    #[test]
    fn a_new_choice_keeps_the_rest_of_the_memory() {
        let mut session = Session::new(sub_watcher(), None);
        session.load(dual_audio(), Some(2), Some(2));
        session.wait(10.0);
        session.viewer_selects(TrackKind::Sub, Some(3));
        let actions = session.viewer_selects(TrackKind::Audio, Some(1));
        let memory = remembered(&actions).pop().cloned().unwrap();
        assert_eq!(memory.audio.as_ref().map(|a| a.title.as_str()), Some("english"));
        assert!(memory.subtitles.is_some(), "the subtitle choice survived the audio one");
    }
}
