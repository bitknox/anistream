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

use std::{
    cmp::Reverse,
    time::{Duration, Instant},
};

use anistream_core::Translation;
pub use anistream_core::track_memory::{
    SubtitleChoice, SubtitleMemory, TrackMemory, TrackPrint,
};
use serde_json::Value;

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

/// The kinds of track mpv lists.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrackKind {
    Video,
    Audio,
    Sub,
}

/// One entry of mpv's `track-list`, as far as choosing tracks goes.
#[derive(Debug, Clone, PartialEq)]
pub struct Track {
    /// mpv's id, as `aid`/`sid` take it. Unique per kind, and only within this file.
    pub id: i64,
    pub kind: TrackKind,
    /// As the file declared it; absent for most external files.
    pub lang: Option<String>,
    /// As the file declared it. For an external file mpv uses the file name instead.
    pub title: Option<String>,
    pub default: bool,
    pub forced: bool,
    pub hearing_impaired: bool,
    pub visual_impaired: bool,
    pub external: bool,
    pub external_filename: Option<String>,
    pub codec: Option<String>,
    pub selected: bool,
    /// For a selected subtitle track, `0` when it is the primary one and `1` when secondary.
    pub main_selection: Option<i64>,
}

/// mpv's `track-list`, parsed.
///
/// Field names are mpv 0.41's, checked against a live `track-list` (see the fixtures in the
/// tests). Entries of a kind we do not choose between, or with no id, are dropped rather than
/// guessed at; a missing flag reads as unset, which is how mpv omits what a file did not say.
pub fn parse_track_list(value: &Value) -> Vec<Track> {
    let Some(entries) = value.as_array() else {
        return Vec::new();
    };
    entries
        .iter()
        .filter_map(|entry| {
            let kind = match entry.get("type")?.as_str()? {
                "video" => TrackKind::Video,
                "audio" => TrackKind::Audio,
                "sub" => TrackKind::Sub,
                _ => return None,
            };
            let text = |key: &str| {
                entry
                    .get(key)
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .map(str::to_owned)
            };
            // Native IPC gives booleans; "yes" is accepted as well, which is how the same
            // flags read through mpv's string interface.
            let flag = |key: &str| {
                entry.get(key).is_some_and(|v| v.as_bool().unwrap_or(v.as_str() == Some("yes")))
            };
            Some(Track {
                id: entry.get("id")?.as_i64()?,
                kind,
                lang: text("lang"),
                title: text("title"),
                default: flag("default"),
                forced: flag("forced"),
                hearing_impaired: flag("hearing-impaired"),
                visual_impaired: flag("visual-impaired"),
                external: flag("external"),
                external_filename: text("external-filename"),
                codec: text("codec"),
                selected: flag("selected"),
                main_selection: entry.get("main-selection").and_then(Value::as_i64),
            })
        })
        .collect()
}

/// An `aid` or `sid` value: `Some(Some(id))` for a track, `Some(None)` for none at all.
///
/// `None` means mpv has not decided — it reports `"auto"` between files and before playback
/// starts — which is not a selection of anything and must not be read as "off".
pub fn parse_track_id(value: &Value) -> Option<Option<i64>> {
    match value {
        Value::Number(n) => n.as_i64().map(Some),
        Value::Bool(false) => Some(None),
        Value::String(s) if s == "no" => Some(None),
        _ => None,
    }
}

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

impl TrackPolicy {
    fn remember_selection(
        &mut self,
        kind: TrackKind,
        id: Option<i64>,
        actions: &mut Vec<TrackAction>,
    ) {
        let track = id.and_then(|id| self.find(kind, id));
        // Commentary is something you listen to once, for an episode, not a way of watching
        // the series; remembering it would start the next episode on the commentary.
        if track.is_some_and(|t| t.role() == Role::Commentary) {
            return;
        }
        match kind {
            TrackKind::Audio => {
                // Audio off is not a way of watching anything either.
                let Some(track) = track else { return };
                self.memory.audio = Some(track.print());
            }
            TrackKind::Sub => {
                let choice = match (id, track) {
                    (None, _) => SubtitleChoice::Off,
                    (Some(_), Some(track)) => SubtitleChoice::Track(track.print()),
                    (Some(_), None) => return,
                };
                let audio_lang = self
                    .aid
                    .current
                    .flatten()
                    .and_then(|id| self.find(TrackKind::Audio, id))
                    .and_then(Track::language);
                self.memory.subtitles = Some(SubtitleMemory { choice, audio_lang });
            }
            TrackKind::Video => return,
        }
        actions.push(TrackAction::Remember(self.memory.clone()));
    }

    fn delay_changed(&mut self, seconds: f64, now: Instant) {
        let changed = self.sub_delay.is_none_or(|current| !same_delay(current, seconds));
        self.sub_delay = Some(seconds);
        if !self.chosen_once && self.baseline_delay.is_none() {
            self.baseline_delay = Some(seconds);
        }
        if !changed {
            return;
        }
        if self.expect_delay.is_some_and(|expected| same_delay(expected, seconds)) {
            self.expect_delay = None;
            return;
        }
        // mpv only reports a change, so once a file is chosen for, any change that is not ours
        // is the viewer's — or the companion script's two-point sync, which is the viewer's by
        // another route. No settle window: nothing but a person moves the delay.
        if !self.loaded || !self.chosen {
            return;
        }
        self.expect_delay = None;
        self.delay_moved = Some(now);
    }

    /// Remember a delay the viewer has stopped moving; with `None`, whether or not it has.
    fn flush_delay(&mut self, now: Option<Instant>, actions: &mut Vec<TrackAction>) {
        let Some(moved) = self.delay_moved else { return };
        if now.is_some_and(|now| now.saturating_duration_since(moved) < DELAY_QUIET) {
            return;
        }
        self.delay_moved = None;
        if let Some(seconds) = self.sub_delay {
            self.memory.sub_delay = Some(seconds);
            actions.push(TrackAction::Remember(self.memory.clone()));
        }
    }
}

impl TrackPolicy {
    /// Make this file's choice.
    fn choose(&mut self, now: Instant, actions: &mut Vec<TrackAction>) {
        self.chosen = true;
        self.chosen_once = true;
        self.settle_until = Some(now + SETTLE);
        self.choose_tracks(actions);

        // A remembered delay applies; failing that, the delay mpv started with, so one series'
        // delay does not ride along into the next in the same process.
        let wanted = self.memory.sub_delay.or(self.baseline_delay);
        if let Some(wanted) = wanted
            && self.sub_delay.is_none_or(|current| !same_delay(current, wanted))
        {
            self.expect_delay = Some(wanted);
            actions.push(TrackAction::SetSubDelay(wanted));
        }
    }

    fn choose_tracks(&mut self, actions: &mut Vec<TrackAction>) {
        let (audio, subtitles) = {
            let audio = self.pick_audio();
            (audio.map(|t| t.id), self.pick_subtitles(audio))
        };
        // Only differences are sent: mpv's own pick is often right, and a redundant set is
        // a redundant echo to tell apart from the viewer.
        if let Some(id) = audio
            && self.aid.current != Some(Some(id))
        {
            self.aid.expect = Some(Some(id));
            actions.push(TrackAction::SetAid(id));
        }
        if let Some(choice) = subtitles
            && self.sid.current != Some(choice)
        {
            self.sid.expect = Some(choice);
            actions.push(TrackAction::SetSid(choice));
        }
    }

    /// The audio track this file should play. `None` when there is no audio at all.
    fn pick_audio(&self) -> Option<&Track> {
        let audio = self.of_kind(TrackKind::Audio);
        if let Some(print) = &self.memory.audio
            && let Some(track) = best_match(print, &audio)
        {
            return Some(track);
        }

        // The programme rather than a commentary or a description, unless that is all there
        // is. Then the language that is wanted; then a track that does not say, which in a
        // file that tags its other audio is usually the one that matters; then any.
        let candidates = facts(&audio);
        let main = |c: &Facts| c.role == Role::Main;
        let any_main = candidates.iter().any(main);
        let programme = |c: &Facts| !any_main || main(c);
        let wanted = match self.preferences.translation {
            Translation::Sub => Some("jpn"),
            Translation::Dub => self.preferences.language.as_deref(),
        };
        let in_wanted = |c: &Facts| {
            programme(c)
                && wanted
                    .is_some_and(|w| c.lang.as_deref().is_some_and(|l| language_matches(l, w)))
        };
        let untagged = |c: &Facts| programme(c) && wanted.is_some() && c.lang.is_none();
        pick(&candidates, &[&in_wanted, &untagged, &programme], self.aid.current.flatten())
    }

    /// The subtitle choice for this file: `Some(None)` for off, `None` to leave mpv's alone.
    fn pick_subtitles(&self, audio: Option<&Track>) -> Option<Option<i64>> {
        let subs = self.of_kind(TrackKind::Sub);
        let audio_lang = audio.and_then(Track::language);

        if let Some(SubtitleMemory { choice, audio_lang: chosen_over }) = &self.memory.subtitles
            && same_language(chosen_over.as_deref(), audio_lang.as_deref())
        {
            match choice {
                SubtitleChoice::Off => return Some(None),
                SubtitleChoice::Track(print) => {
                    if let Some(track) = best_match(print, &subs) {
                        return Some(Some(track.id));
                    }
                }
            }
        }

        let language = self.preferences.language.as_deref()?;
        let candidates = facts(&subs);
        let current = self.sid.current.flatten();
        let ours = |c: &Facts| c.lang.as_deref().is_some_and(|l| language_matches(l, language));
        let untagged = |c: &Facts| c.lang.is_none();

        // A dub watcher hearing their own language needs only what the audio does not carry:
        // signs, and the songs left in Japanese. Audio that does not say its language is taken
        // to be the dub that was asked for. Audio plainly in another language — a source with
        // no dub after all — needs full subtitles like anyone else.
        let hears_own_language = self.preferences.translation == Translation::Dub
            && audio.is_some()
            && audio_lang.as_deref().is_none_or(|l| language_matches(l, language));
        if hears_own_language {
            let signs = |c: &Facts| c.role == Role::Signs;
            let signs_proper = |c: &Facts| ours(c) && signs(c) && c.signs_proper;
            let ours_signs = |c: &Facts| ours(c) && signs(c);
            let untagged_signs = |c: &Facts| untagged(c) && signs(c);
            let picked =
                pick(&candidates, &[&signs_proper, &ours_signs, &untagged_signs], current);
            return Some(picked.map(|t| t.id));
        }

        // Everyone else wants the dialogue: a full track, then SDH — dialogue with captions
        // added — and signs only if nothing else in the language exists. A file that does not
        // say its language ranks below one that says it is ours. Commentary is never chosen:
        // its subtitles transcribe a different soundtrack from the one playing.
        let full = |c: &Facts| c.role == Role::Main;
        let sdh = |c: &Facts| c.role == Role::HearingImpaired;
        let signs = |c: &Facts| c.role == Role::Signs;
        let tiers: [&dyn Fn(&Facts) -> bool; 6] = [
            &|c| ours(c) && full(c),
            &|c| ours(c) && sdh(c),
            &|c| untagged(c) && full(c),
            &|c| untagged(c) && sdh(c),
            &|c| ours(c) && signs(c),
            &|c| untagged(c) && signs(c),
        ];
        // Nothing in the language at all: whatever mpv chose stands. There is no telling
        // what the viewer would rather have, and turning subtitles off is a guess too.
        pick(&candidates, &tiers, current).map(|t| Some(t.id))
    }
}

/// What a track is for, as far as choosing between tracks goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Role {
    /// The programme: an ordinary audio track, or full dialogue subtitles.
    Main,
    /// Signs, songs, karaoke, forced — the subtitles for what the audio does not translate.
    Signs,
    /// SDH and closed captions: dialogue with sound cues added.
    HearingImpaired,
    Commentary,
    /// Audio description, for viewers who cannot see the picture.
    Description,
}

/// Words that mark a subtitle track as partial: only signs, songs or forced text.
const SIGNS_WORDS: &[&str] =
    &["signs", "sign", "songs", "song", "forced", "karaoke", "lyrics", "onscreen"];

impl Track {
    /// The title as words: lowercase, punctuation gone. For an external file, which mpv titles
    /// by its name, without the directory, query string, extension or anything with a digit
    /// in it — episode numbers differ per file and would stop the same track matching itself.
    fn words(&self) -> Vec<String> {
        let raw = self.title.as_deref().or(self.external_filename.as_deref()).unwrap_or("");
        title_words(raw, self.external)
    }

    /// The track's language: as declared, or for a track that does not say, as its title or
    /// file name implies.
    pub fn language(&self) -> Option<String> {
        match self.lang.as_deref() {
            Some(lang) if !undetermined(lang) => Some(lang.to_lowercase()),
            _ => guess_language(&self.words(), self.external),
        }
    }

    fn role(&self) -> Role {
        role(self.kind, &self.words(), self.forced, self.hearing_impaired, self.visual_impaired)
    }

    /// What the track is, for remembering.
    pub fn print(&self) -> TrackPrint {
        TrackPrint {
            lang: self.language(),
            title: self.words().join(" "),
            forced: self.forced,
            hearing_impaired: self.hearing_impaired,
            visual_impaired: self.visual_impaired,
            external: self.external,
            codec: self.codec.clone(),
        }
    }
}

fn role(
    kind: TrackKind,
    words: &[String],
    forced: bool,
    hearing_impaired: bool,
    visual_impaired: bool,
) -> Role {
    let has = |wanted: &[&str]| words.iter().any(|w| wanted.contains(&w.as_str()));
    // Japanese releases title it in katakana, which is one word to the splitter.
    if words.iter().any(|w| w.starts_with("commentar") || w.contains("コメンタリー")) {
        return Role::Commentary;
    }
    match kind {
        TrackKind::Sub => {
            // "Full" outranks everything partial: "Full Subtitles + Songs" is the dialogue
            // with the songs included, and a full track flagged forced is a mislabel.
            let full = has(&["full", "dialogue", "dialog"]);
            if !full && (forced || has(SIGNS_WORDS)) {
                Role::Signs
            } else if hearing_impaired || has(&["sdh", "cc", "captions", "hearing"]) {
                Role::HearingImpaired
            } else {
                Role::Main
            }
        }
        TrackKind::Audio | TrackKind::Video => {
            if visual_impaired || has(&["description", "descriptive", "described"]) {
                Role::Description
            } else {
                Role::Main
            }
        }
    }
}

/// File extensions mpv's own subtitle loaders take, which are noise in a title.
const SUBTITLE_EXTENSIONS: &[&str] =
    &["srt", "ass", "ssa", "vtt", "webvtt", "sub", "sup", "idx", "smi", "ttml", "dfxp", "txt"];

fn title_words(raw: &str, external: bool) -> Vec<String> {
    let mut text = raw;
    if external {
        // mpv titles a file it is handed by its name — for a URL, query string and all.
        text = text.rsplit(['/', '\\']).next().unwrap_or(text);
        text = text.split(['?', '#']).next().unwrap_or(text);
        if let Some((stem, extension)) = text.rsplit_once('.')
            && SUBTITLE_EXTENSIONS.contains(&extension.to_ascii_lowercase().as_str())
        {
            text = stem;
        }
    }
    let text = text.to_lowercase().replace("%20", " ");
    let mut words: Vec<String> = text
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .filter(|w| !(external && w.chars().any(|c| c.is_ascii_digit())))
        .map(str::to_owned)
        .collect();
    // "S&S" is the conventional shorthand, and splits into two lone letters; spelled out, it
    // is found the way the words are.
    for i in 1..words.len() {
        if words[i - 1] == "s" && words[i] == "s" {
            words[i - 1] = "signs".into();
            words[i] = "songs".into();
        }
    }
    words
}

/// Spellings of the languages anime is subtitled and dubbed in: ISO 639-1, then 639-2 (the
/// canonical spelling, at index 1), then any other form sources use.
///
/// A superset of what `protocol::stream_args` widens, under the same rule — `en`, `eng` and
/// `english` are one language. Norwegian's `no` is left out: it is an English word often
/// enough in a title ("No Honorifics") to be guessed wrong.
const LANGUAGES: &[&[&str]] = &[
    &["en", "eng", "english"],
    &["ja", "jpn", "jp", "japanese", "日本語"],
    &["es", "spa", "spanish", "espanol", "español", "castellano", "latino"],
    &["pt", "por", "portuguese", "portugues", "português"],
    &["fr", "fra", "fre", "french", "francais", "français"],
    &["de", "deu", "ger", "german", "deutsch"],
    &["it", "ita", "italian", "italiano"],
    &["ru", "rus", "russian"],
    &["ar", "ara", "arabic"],
    &["zh", "zho", "chi", "chinese", "chs", "cht"],
    &["ko", "kor", "korean"],
    &["pl", "pol", "polish"],
    &["tr", "tur", "turkish"],
    &["id", "ind", "indonesian"],
    &["th", "tha", "thai"],
    &["vi", "vie", "vietnamese"],
    &["ms", "msa", "malay"],
    &["hi", "hin", "hindi"],
    &["nl", "nld", "dut", "dutch"],
    &["sv", "swe", "swedish"],
    &["da", "dan", "danish"],
    &["nb", "nor", "nob", "norwegian"],
    &["fi", "fin", "finnish"],
];

fn family(code: &str) -> Option<usize> {
    LANGUAGES.iter().position(|spellings| spellings.contains(&code))
}

/// The primary subtag, lowercased: `en-US` is `en`, `pt_BR` is `pt`.
///
/// Some sources send a label where the code belongs, `English (CR English)`; its first word is
/// the language, so that is what is read.
fn primary_subtag(code: &str) -> String {
    code.trim().split(['-', '_', ' ', '(']).next().unwrap_or_default().to_lowercase()
}

/// Labels that say the language is not known, which is not a language to match.
fn undetermined(code: &str) -> bool {
    matches!(
        primary_subtag(code).as_str(),
        "" | "und" | "mul" | "mis" | "zxx" | "unk" | "unknown"
    )
}

/// Whether two language labels name the same language.
///
/// The way `stream_args` matches: sources label one language `en`, `eng` and `english`
/// interchangeably, and an exact comparison would miss two of the three. Region subtags do not
/// separate (`es-419` is Spanish), and an undetermined label matches nothing — not even itself.
pub fn language_matches(have: &str, want: &str) -> bool {
    if undetermined(have) || undetermined(want) {
        return false;
    }
    let (have, want) = (primary_subtag(have), primary_subtag(want));
    match (family(&have), family(&want)) {
        (Some(a), Some(b)) => a == b,
        // Outside the table, `stream_args`' fallback: one a prefix of the other.
        (None, None) => have.starts_with(&want) || want.starts_with(&have),
        _ => false,
    }
}

/// Two possibly unknown languages that agree: the same language, or both unknown.
fn same_language(a: Option<&str>, b: Option<&str>) -> bool {
    match (a, b) {
        (Some(a), Some(b)) => language_matches(a, b),
        (None, None) => true,
        _ => false,
    }
}

/// A language a title or file name implies, as its 639-2 code.
///
/// Two-letter codes are ordinary words often enough ("it", "hi") that only a file name, where
/// `episode.en.srt` is the convention, gets to use them.
fn guess_language(words: &[String], external: bool) -> Option<String> {
    words.iter().find_map(|word| {
        if word.chars().count() < 3 && !external {
            return None;
        }
        family(word).map(|f| LANGUAGES[f][1].to_owned())
    })
}

fn same_delay(a: f64, b: f64) -> bool {
    // mpv steps the delay in tenths and reports back what it was given; a millisecond is far
    // below anything a viewer could set or see.
    (a - b).abs() < 1e-3
}

/// A track with what choosing needs worked out once.
struct Facts<'a> {
    track: &'a Track,
    lang: Option<String>,
    role: Role,
    /// A partial track that is signs proper — flagged forced, or titled signs — rather than
    /// songs or karaoke alone, which is less of what a dub watcher needs.
    signs_proper: bool,
}

fn facts<'a>(tracks: &[&'a Track]) -> Vec<Facts<'a>> {
    tracks
        .iter()
        .map(|track| {
            let words = track.words();
            Facts {
                track,
                lang: track.language(),
                role: role(
                    track.kind,
                    &words,
                    track.forced,
                    track.hearing_impaired,
                    track.visual_impaired,
                ),
                signs_proper: track.forced
                    || words.iter().any(|w| matches!(w.as_str(), "sign" | "signs" | "forced")),
            }
        })
        .collect()
}

/// The first tier with anything in it decides. Within it, mpv's own pick stands if it is
/// there — it may know something we do not, like the user's `--alang` order — and otherwise
/// the track the file flags default, then the first.
fn pick<'a>(
    candidates: &[Facts<'a>],
    tiers: &[&dyn Fn(&Facts) -> bool],
    current: Option<i64>,
) -> Option<&'a Track> {
    tiers.iter().find_map(|tier| {
        let pool: Vec<&Facts> = candidates.iter().filter(|c| tier(c)).collect();
        pool.iter()
            .find(|c| Some(c.track.id) == current)
            .or_else(|| pool.iter().find(|c| c.track.default))
            .or_else(|| pool.first())
            .map(|c| c.track)
    })
}

/// The track that best answers a remembered one, if any answers it well enough.
///
/// Ids are useless across files, so a track is recognised by what it is. The language must
/// agree and so must the role — a signs track is never the answer to full subtitles, however
/// alike the titles. Beyond that, in order: the same title and flags; the same flags under
/// another title (a different release); the same title with different flags; the role alone.
/// Whether it is external and its codec break ties.
fn best_match<'a>(print: &TrackPrint, candidates: &[&'a Track]) -> Option<&'a Track> {
    let kind = candidates.first()?.kind;
    let words: Vec<String> = print.title.split(' ').map(str::to_owned).collect();
    let remembered_role =
        role(kind, &words, print.forced, print.hearing_impaired, print.visual_impaired);

    candidates
        .iter()
        .enumerate()
        .filter_map(|(index, track)| {
            let theirs = track.print();
            if !same_language(print.lang.as_deref(), theirs.lang.as_deref())
                || track.role() != remembered_role
            {
                return None;
            }
            let title = !print.title.is_empty() && print.title == theirs.title;
            let flags = print.forced == theirs.forced
                && print.hearing_impaired == theirs.hearing_impaired
                && print.visual_impaired == theirs.visual_impaired;
            let tier = match (title, flags) {
                (true, true) => 3,
                (false, true) => 2,
                (true, false) => 1,
                (false, false) => 0,
            };
            let external = print.external == theirs.external;
            let codec = print.codec.is_some() && print.codec == theirs.codec;
            Some(((tier, external, codec, Reverse(index)), *track))
        })
        .max_by_key(|(score, _)| *score)
        .map(|(_, track)| track)
}

#[cfg(test)]
mod tests {
    use std::collections::VecDeque;

    use serde_json::json;

    use super::*;

    /// A `track-list` as mpv 0.41 reported it over IPC for a test file built with ffmpeg —
    /// dual audio, three English subtitle tracks and a Spanish one, plus an external file
    /// handed over by URL — trimmed of decoder and demuxer fields, with nothing selected yet.
    const DUAL_AUDIO: &str = r#"[
        {"id": 1, "type": "video", "src-id": 1, "image": false, "albumart": false,
         "default": false, "forced": false, "dependent": false, "visual-impaired": false,
         "hearing-impaired": false, "external": false, "selected": false, "ff-index": 0,
         "codec": "h264", "demux-w": 320, "demux-h": 240, "demux-fps": 24.0},
        {"id": 1, "type": "audio", "src-id": 2, "title": "English", "lang": "eng",
         "audio-channels": 2, "image": false, "albumart": false, "default": true,
         "forced": false, "dependent": false, "visual-impaired": false,
         "hearing-impaired": false, "external": false, "selected": false, "ff-index": 1,
         "codec": "aac", "demux-samplerate": 48000},
        {"id": 2, "type": "audio", "src-id": 3, "title": "Japanese", "lang": "jpn",
         "audio-channels": 2, "image": false, "albumart": false, "default": false,
         "forced": false, "dependent": false, "visual-impaired": false,
         "hearing-impaired": false, "external": false, "selected": false, "ff-index": 2,
         "codec": "aac", "demux-samplerate": 48000},
        {"id": 3, "type": "audio", "src-id": 4, "title": "Commentary", "lang": "jpn",
         "audio-channels": 2, "image": false, "albumart": false, "default": false,
         "forced": false, "dependent": false, "visual-impaired": false,
         "hearing-impaired": false, "external": false, "selected": false, "ff-index": 3,
         "codec": "aac", "demux-samplerate": 48000},
        {"id": 1, "type": "sub", "src-id": 5, "title": "Signs & Songs", "lang": "eng",
         "image": false, "albumart": false, "default": false, "forced": true,
         "dependent": false, "visual-impaired": false, "hearing-impaired": false,
         "external": false, "selected": false, "ff-index": 4, "codec": "ass"},
        {"id": 2, "type": "sub", "src-id": 6, "title": "Full Subtitles", "lang": "eng",
         "image": false, "albumart": false, "default": true, "forced": false,
         "dependent": false, "visual-impaired": false, "hearing-impaired": false,
         "external": false, "selected": false, "ff-index": 5, "codec": "ass"},
        {"id": 3, "type": "sub", "src-id": 7, "title": "English SDH", "lang": "eng",
         "image": false, "albumart": false, "default": false, "forced": false,
         "dependent": false, "visual-impaired": false, "hearing-impaired": true,
         "external": false, "selected": false, "ff-index": 6, "codec": "ass"},
        {"id": 4, "type": "sub", "src-id": 8, "title": "Spanish", "lang": "spa",
         "image": false, "albumart": false, "default": false, "forced": false,
         "dependent": false, "visual-impaired": false, "hearing-impaired": false,
         "external": false, "selected": false, "ff-index": 7, "codec": "ass"},
        {"id": 5, "type": "sub", "title": "spa.forced.srt?token=abc", "image": false,
         "albumart": false, "default": false, "forced": false, "dependent": false,
         "visual-impaired": false, "hearing-impaired": false, "external": true,
         "selected": false,
         "external-filename": "https://cdn.example/subs/spa.forced.srt?token=abc",
         "ff-index": 0, "codec": "subrip"}
    ]"#;

    fn dual_audio() -> Vec<Track> {
        parse_track_list(&serde_json::from_str(DUAL_AUDIO).unwrap())
    }

    fn track(kind: TrackKind, id: i64, lang: Option<&str>, title: Option<&str>) -> Track {
        Track {
            id,
            kind,
            lang: lang.map(str::to_owned),
            title: title.map(str::to_owned),
            default: false,
            forced: false,
            hearing_impaired: false,
            visual_impaired: false,
            external: false,
            external_filename: None,
            codec: Some(if kind == TrackKind::Sub { "ass" } else { "aac" }.into()),
            selected: false,
            main_selection: None,
        }
    }

    fn video() -> Track {
        track(TrackKind::Video, 1, None, None)
    }

    fn audio(id: i64, lang: &str, title: &str) -> Track {
        track(TrackKind::Audio, id, Some(lang), Some(title))
    }

    fn sub(id: i64, lang: &str, title: &str) -> Track {
        track(TrackKind::Sub, id, Some(lang), Some(title))
    }

    /// An external file as mpv lists one it was handed with `--sub-file`: titled by its file
    /// name, with no language.
    fn external(id: i64, url: &str) -> Track {
        let name = url.rsplit('/').next().unwrap();
        Track {
            external: true,
            external_filename: Some(url.into()),
            codec: Some("subrip".into()),
            ..track(TrackKind::Sub, id, None, Some(name))
        }
    }

    fn forced(mut t: Track) -> Track {
        t.forced = true;
        t
    }

    fn flagged_default(mut t: Track) -> Track {
        t.default = true;
        t
    }

    fn sdh(mut t: Track) -> Track {
        t.hearing_impaired = true;
        t
    }

    fn sub_watcher() -> TrackPreferences {
        TrackPreferences { translation: Translation::Sub, language: Some("eng".into()) }
    }

    fn dub_watcher() -> TrackPreferences {
        TrackPreferences { translation: Translation::Dub, language: Some("eng".into()) }
    }

    /// Just enough of mpv: it holds a selection, and reports a change the way 0.41 does —
    /// the whole list restated, then the property — and reports nothing when nothing changed.
    struct FakeMpv {
        tracks: Vec<Track>,
        delay: f64,
    }

    impl FakeMpv {
        fn current(&self, kind: TrackKind) -> Option<i64> {
            self.tracks
                .iter()
                .find(|t| t.kind == kind && t.selected && t.main_selection != Some(1))
                .map(|t| t.id)
        }

        fn select(&mut self, kind: TrackKind, id: Option<i64>) -> Vec<TrackInput> {
            if self.current(kind) == id {
                return Vec::new();
            }
            for t in self.tracks.iter_mut().filter(|t| t.kind == kind) {
                t.selected = Some(t.id) == id;
                t.main_selection = t.selected.then_some(0);
            }
            let property = match kind {
                TrackKind::Audio => TrackInput::Aid(id),
                _ => TrackInput::Sid(id),
            };
            vec![TrackInput::TrackList(self.tracks.clone()), property]
        }

        fn set_delay(&mut self, seconds: f64) -> Vec<TrackInput> {
            if same_delay(self.delay, seconds) {
                return Vec::new();
            }
            self.delay = seconds;
            vec![TrackInput::SubDelay(seconds)]
        }

        fn carry_out(&mut self, action: &TrackAction) -> Vec<TrackInput> {
            match action {
                TrackAction::SetAid(id) => self.select(TrackKind::Audio, Some(*id)),
                TrackAction::SetSid(id) => self.select(TrackKind::Sub, *id),
                TrackAction::SetSubDelay(seconds) => self.set_delay(*seconds),
                TrackAction::Remember(_) => Vec::new(),
            }
        }
    }

    /// A policy wired to a fake mpv, on a clock the test moves by hand.
    struct Session {
        policy: TrackPolicy,
        mpv: FakeMpv,
        start: Instant,
        elapsed: Duration,
    }

    impl Session {
        /// A fresh mpv process: observers greeted with an empty list and a zero delay.
        fn new(preferences: TrackPreferences, memory: Option<TrackMemory>) -> Self {
            let mut session = Self {
                policy: TrackPolicy::new(preferences, memory),
                mpv: FakeMpv { tracks: Vec::new(), delay: 0.0 },
                start: Instant::now(),
                elapsed: Duration::ZERO,
            };
            assert!(session.feed(TrackInput::TrackList(Vec::new())).is_empty());
            assert!(session.feed(TrackInput::SubDelay(0.0)).is_empty());
            session
        }

        fn now(&self) -> Instant {
            self.start + self.elapsed
        }

        fn wait(&mut self, seconds: f64) {
            self.elapsed += Duration::from_secs_f64(seconds);
        }

        fn feed(&mut self, input: TrackInput) -> Vec<TrackAction> {
            self.feed_all(vec![input])
        }

        /// Feed reports mpv has already sent, carry out what comes back, and feed mpv's
        /// reports of that in turn until everything is quiet.
        ///
        /// One queue, because that is the order on the socket: what mpv had already said is
        /// read before the echo of anything we send in reply to the first of it.
        fn feed_all(&mut self, inputs: Vec<TrackInput>) -> Vec<TrackAction> {
            let mut queue = VecDeque::from(inputs);
            let mut all = Vec::new();
            let mut rounds = 0;
            while let Some(input) = queue.pop_front() {
                rounds += 1;
                assert!(rounds < 100, "the policy is fighting the player: {all:?}");
                let actions = self.policy.observe(&input, self.now());
                for action in &actions {
                    queue.extend(self.mpv.carry_out(action));
                }
                all.extend(actions);
            }
            all
        }

        /// Load a file the way mpv does: the list while it opens, `file-loaded`, then the list
        /// again with mpv's own pick in it, then the properties.
        fn load(
            &mut self,
            tracks: Vec<Track>,
            aid: Option<i64>,
            sid: Option<i64>,
        ) -> Vec<TrackAction> {
            self.mpv.tracks = tracks;
            let mut actions = self.feed(TrackInput::TrackList(self.mpv.tracks.clone()));
            for t in &mut self.mpv.tracks {
                let pick = match t.kind {
                    TrackKind::Video => Some(1),
                    TrackKind::Audio => aid,
                    TrackKind::Sub => sid,
                };
                t.selected = pick == Some(t.id);
                t.main_selection = t.selected.then_some(0);
            }
            actions.extend(self.feed_all(vec![
                TrackInput::FileLoaded,
                TrackInput::TrackList(self.mpv.tracks.clone()),
                TrackInput::Aid(aid),
                TrackInput::Sid(sid),
            ]));
            actions
        }

        /// End this file, the way mpv does before the next one opens.
        fn next_episode(&mut self) -> Vec<TrackAction> {
            let mut actions = self.feed(TrackInput::EndFile);
            self.mpv.tracks.clear();
            actions.extend(self.feed(TrackInput::TrackList(Vec::new())));
            self.wait(1.0);
            actions
        }

        /// The viewer picks a track in the player.
        fn viewer_selects(&mut self, kind: TrackKind, id: Option<i64>) -> Vec<TrackAction> {
            let reports = self.mpv.select(kind, id);
            self.feed_all(reports)
        }

        fn viewer_moves_delay(&mut self, by: f64) -> Vec<TrackAction> {
            let reports = self.mpv.set_delay(self.mpv.delay + by);
            self.feed_all(reports)
        }

        fn selection(&self) -> (Option<i64>, Option<i64>) {
            (self.mpv.current(TrackKind::Audio), self.mpv.current(TrackKind::Sub))
        }
    }

    fn sets(actions: &[TrackAction]) -> Vec<&TrackAction> {
        actions.iter().filter(|a| !matches!(a, TrackAction::Remember(_))).collect()
    }

    fn remembered(actions: &[TrackAction]) -> Vec<&TrackMemory> {
        actions
            .iter()
            .filter_map(|a| match a {
                TrackAction::Remember(memory) => Some(memory),
                _ => None,
            })
            .collect()
    }

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

    fn subtitles_memory(choice: SubtitleChoice, audio_lang: &str) -> TrackMemory {
        TrackMemory {
            subtitles: Some(SubtitleMemory { choice, audio_lang: Some(audio_lang.into()) }),
            ..TrackMemory::default()
        }
    }

    // --- Parsing ---------------------------------------------------------------------------

    #[test]
    fn a_live_track_list_parses_into_tracks() {
        let tracks = dual_audio();
        assert_eq!(tracks.len(), 9, "every audio, subtitle and video entry");
        assert_eq!(tracks.iter().filter(|t| t.kind == TrackKind::Audio).count(), 3);
        assert_eq!(tracks.iter().filter(|t| t.kind == TrackKind::Sub).count(), 5);

        let english = &tracks[1];
        assert_eq!((english.id, english.kind), (1, TrackKind::Audio));
        assert_eq!(english.lang.as_deref(), Some("eng"));
        assert_eq!(english.title.as_deref(), Some("English"));
        assert!(english.default && !english.selected);

        assert!(tracks[4].forced && !tracks[4].hearing_impaired);
        assert_eq!(tracks[6].title.as_deref(), Some("English SDH"));
        assert!(tracks[6].hearing_impaired);

        // An external file mpv was handed by URL: titled by its name, with no language.
        let file = &tracks[8];
        assert!(file.external);
        assert_eq!(file.lang, None);
        assert_eq!(
            file.external_filename.as_deref(),
            Some("https://cdn.example/subs/spa.forced.srt?token=abc")
        );
        assert_eq!(file.codec.as_deref(), Some("subrip"));
    }

    #[test]
    fn the_selection_and_its_slot_are_read_from_the_list() {
        let list = json!([
            {"id": 2, "type": "sub", "lang": "eng", "selected": true, "main-selection": 1},
            {"id": 3, "type": "sub", "lang": "eng", "selected": "yes", "main-selection": 0},
            {"id": 9, "type": "attachment"},
            {"type": "audio", "lang": "jpn"}
        ]);
        let tracks = parse_track_list(&list);
        assert_eq!(tracks.len(), 2, "unknown kinds and id-less entries are dropped");
        assert_eq!(tracks[0].main_selection, Some(1), "the secondary subtitle");
        assert!(tracks[1].selected, "the string form of a flag reads too");
        assert!(parse_track_list(&Value::Null).is_empty());
    }

    #[test]
    fn a_track_id_distinguishes_off_from_undecided() {
        // mpv says "auto" between files. Reading that as "off" would remember an episode
        // boundary as the viewer turning subtitles off.
        assert_eq!(parse_track_id(&json!(3)), Some(Some(3)));
        assert_eq!(parse_track_id(&json!(false)), Some(None));
        assert_eq!(parse_track_id(&json!("no")), Some(None));
        assert_eq!(parse_track_id(&json!("auto")), None);
        assert_eq!(parse_track_id(&Value::Null), None);
    }

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

    #[test]
    fn a_language_matches_across_its_spellings() {
        for have in ["en", "eng", "english", "English", "en-US", "en_GB"] {
            assert!(language_matches(have, "eng"), "{have} lost its match");
            assert!(language_matches("en", have), "{have} lost its match the other way");
        }
        for have in ["ja", "jpn", "jp", "japanese"] {
            assert!(language_matches(have, "ja"), "{have} lost its match");
        }
        assert!(language_matches("es-419", "spa"), "a region does not make another language");
        assert!(!language_matches("jpn", "eng"));
        assert!(!language_matches("spa", "por"));
        // Undetermined is not a language, so it matches nothing — not even itself.
        assert!(!language_matches("und", "und"));
        assert!(!language_matches("und", "eng"));
        // Outside the table, the prefix rule `stream_args` uses.
        assert!(language_matches("tgl", "tg"));
        assert!(!language_matches("tgl", "eng"));
    }

    #[test]
    fn a_title_is_reduced_to_its_words() {
        let words = |raw: &str, external: bool| title_words(raw, external).join(" ");
        assert_eq!(words("Signs & Songs", false), "signs songs");
        assert_eq!(words("English (S&S)", false), "english signs songs");
        assert_eq!(
            words("Japanese 5.1", false),
            "japanese 5 1",
            "embedded titles keep numbers"
        );
        // An external file: no directory, query string, extension or episode number.
        assert_eq!(words("https://cdn.example/subs/ep05-eng.srt?token=abc", true), "eng");
        assert_eq!(words("en.forced.srt", true), "en forced");
        assert_eq!(words("Full%20Subtitles.vtt", true), "full subtitles");
    }

    #[test]
    fn a_track_that_does_not_say_its_language_is_read_from_its_name() {
        let named = |url: &str| external(5, url).language();
        assert_eq!(named("https://cdn.example/subs/English.vtt").as_deref(), Some("eng"));
        assert_eq!(named("https://cdn.example/eng.srt?t=1").as_deref(), Some("eng"));
        assert_eq!(named("https://cdn.example/a8f3c2.vtt"), None);
        // Two letters are a language only in a file name; in a title, "it" is a word.
        assert_eq!(named("https://cdn.example/episode.it.srt").as_deref(), Some("ita"));
        assert_eq!(track(TrackKind::Sub, 1, None, Some("Sign It Up")).language(), None);
        // A declared language wins over the title, and "und" is no declaration.
        assert_eq!(sub(1, "spa", "English").language().as_deref(), Some("spa"));
        assert_eq!(sub(1, "und", "English").language().as_deref(), Some("eng"));
    }

    #[test]
    fn a_track_role_is_read_from_its_title_and_flags() {
        let cases: &[(Track, Role)] = &[
            (sub(1, "eng", "Signs & Songs"), Role::Signs),
            (sub(1, "eng", "English [Forced]"), Role::Signs),
            (forced(sub(1, "eng", "English")), Role::Signs),
            (sub(1, "eng", "Songs"), Role::Signs),
            (sub(1, "eng", "Karaoke"), Role::Signs),
            (sub(1, "eng", "Full Subtitles"), Role::Main),
            (sub(1, "eng", "Full Subtitles + Songs"), Role::Main),
            (forced(sub(1, "eng", "Dialogue")), Role::Main),
            (sub(1, "eng", "English"), Role::Main),
            (sub(1, "eng", "English (No Honorifics)"), Role::Main),
            (sub(1, "eng", "English SDH"), Role::HearingImpaired),
            (sub(1, "eng", "English [CC]"), Role::HearingImpaired),
            (sdh(sub(1, "eng", "English")), Role::HearingImpaired),
            (sub(1, "eng", "Commentary"), Role::Commentary),
            (audio(1, "jpn", "Commentary with the Director"), Role::Commentary),
            (audio(1, "jpn", "オーディオコメンタリー"), Role::Commentary),
            (audio(1, "eng", "Audio Description"), Role::Description),
            (audio(1, "jpn", "Japanese 2.0"), Role::Main),
            (external(1, "https://cdn.example/en.srt"), Role::Main),
        ];
        for (track, expected) in cases {
            assert_eq!(track.role(), *expected, "{:?}", track.title);
        }
        // mpv reads `.forced.` out of an autoloaded file's name and sets the flag itself.
        assert_eq!(
            forced(external(1, "https://cdn.example/en.forced.srt")).role(),
            Role::Signs
        );
    }

    // --- Choosing --------------------------------------------------------------------------

    #[test]
    fn a_sub_watcher_on_a_dual_audio_release_gets_japanese_and_full_subtitles() {
        // What mpv does with this file on its own: the default-flagged English audio, and —
        // preferring external files — the Spanish one handed over by URL.
        let mut session = Session::new(sub_watcher(), None);
        let actions = session.load(dual_audio(), Some(1), Some(5));
        assert_eq!(sets(&actions), [&TrackAction::SetAid(2), &TrackAction::SetSid(Some(2))]);
        assert_eq!(session.selection(), (Some(2), Some(2)));
        assert!(remembered(&actions).is_empty(), "the policy's own pick is not a memory");
    }

    #[test]
    fn mpvs_own_mistakes_with_external_files_are_corrected() {
        // Measured on 0.41 with `--alang=ja --slang=en`: handed `--sub-file`s, mpv chose the
        // external Spanish forced file over the release's own English default…
        let mut session = Session::new(sub_watcher(), None);
        let actions = session.load(dual_audio(), Some(2), Some(5));
        assert_eq!(sets(&actions), [&TrackAction::SetSid(Some(2))]);

        // …and, autoloading files beside the video, an SDH file over full subtitles.
        let mut tracks = dual_audio();
        tracks.truncate(8);
        let mut autoloaded = sdh(external(5, "en.sdh.srt"));
        autoloaded.lang = Some("en".into());
        tracks.push(autoloaded);
        let mut session = Session::new(sub_watcher(), None);
        let actions = session.load(tracks, Some(2), Some(5));
        assert_eq!(sets(&actions), [&TrackAction::SetSid(Some(2))]);
    }

    #[test]
    fn a_japanese_commentary_track_is_not_the_programme() {
        // `--alang=ja` cannot tell these apart; here the commentary even comes first and is
        // flagged default.
        let tracks = vec![
            video(),
            flagged_default(audio(1, "jpn", "Commentary")),
            audio(2, "jpn", "Japanese"),
            audio(3, "eng", "English"),
            sub(1, "eng", "English"),
        ];
        let mut session = Session::new(sub_watcher(), None);
        let actions = session.load(tracks, Some(1), Some(1));
        assert_eq!(sets(&actions), [&TrackAction::SetAid(2)]);
    }

    #[test]
    fn a_dub_watcher_gets_their_audio_and_the_signs_track() {
        // mpv with `--alang=en`: English audio, and the default — full — subtitles.
        let mut session = Session::new(dub_watcher(), None);
        let actions = session.load(dual_audio(), Some(1), Some(2));
        assert_eq!(sets(&actions), [&TrackAction::SetSid(Some(1))]);
        assert_eq!(session.selection(), (Some(1), Some(1)));
    }

    #[test]
    fn a_dub_watcher_on_a_release_without_signs_gets_no_subtitles() {
        let tracks = vec![
            video(),
            flagged_default(audio(1, "jpn", "Japanese")),
            audio(2, "eng", "English"),
            flagged_default(sub(1, "eng", "Full Subtitles")),
            sdh(sub(2, "eng", "English SDH")),
        ];
        let mut session = Session::new(dub_watcher(), None);
        let actions = session.load(tracks, Some(1), Some(1));
        assert_eq!(sets(&actions), [&TrackAction::SetAid(2), &TrackAction::SetSid(None)]);
    }

    #[test]
    fn a_dub_watcher_whose_file_has_no_dub_still_gets_full_subtitles() {
        // The source had no dub after all. Japanese audio with signs only would be unwatchable
        // for someone who asked for English.
        let tracks = vec![
            video(),
            audio(1, "jpn", "Japanese"),
            forced(sub(1, "eng", "Signs & Songs")),
            sub(2, "eng", "Full Subtitles"),
        ];
        let mut session = Session::new(dub_watcher(), None);
        let actions = session.load(tracks, Some(1), Some(1));
        assert_eq!(sets(&actions), [&TrackAction::SetSid(Some(2))]);
    }

    #[test]
    fn audio_that_does_not_say_its_language_is_taken_for_what_was_asked() {
        // A web dub stream rarely tags its one audio track. A dub watcher on it gets signs.
        let tracks = vec![
            video(),
            track(TrackKind::Audio, 1, None, None),
            sub(1, "eng", "English"),
            forced(sub(2, "eng", "Signs")),
        ];
        let mut session = Session::new(dub_watcher(), None);
        let actions = session.load(tracks, Some(1), Some(1));
        assert_eq!(sets(&actions), [&TrackAction::SetSid(Some(2))]);

        // In a file that tags its other audio, the untagged track is the likelier original.
        let tracks = vec![
            video(),
            flagged_default(audio(1, "eng", "English")),
            track(TrackKind::Audio, 2, None, None),
            sub(1, "eng", "English"),
        ];
        let mut session = Session::new(sub_watcher(), None);
        let actions = session.load(tracks, Some(1), Some(1));
        assert_eq!(sets(&actions), [&TrackAction::SetAid(2)]);
    }

    #[test]
    fn when_mpv_already_picked_right_nothing_is_sent() {
        // A redundant set is a redundant echo to tell apart from the viewer.
        let mut session = Session::new(sub_watcher(), None);
        assert!(session.load(dual_audio(), Some(2), Some(2)).is_empty());

        let mut session = Session::new(dub_watcher(), None);
        assert!(session.load(dual_audio(), Some(1), Some(1)).is_empty());
    }

    #[test]
    fn mpvs_pick_stands_among_equals() {
        // Two full English tracks: which one is not ours to second-guess. mpv may be following
        // the user's own `--slang` order, and either is right.
        let tracks = vec![
            video(),
            audio(1, "jpn", "Japanese"),
            flagged_default(sub(1, "eng", "English")),
            sub(2, "eng", "English (Honorifics)"),
        ];
        let mut session = Session::new(sub_watcher(), None);
        assert!(session.load(tracks, Some(1), Some(2)).is_empty());
    }

    #[test]
    fn a_single_audio_file_keeps_its_audio() {
        let tracks = vec![video(), audio(1, "eng", "English"), sub(1, "eng", "English")];
        let mut session = Session::new(sub_watcher(), None);
        assert!(session.load(tracks, Some(1), Some(1)).is_empty());
    }

    #[test]
    fn a_file_without_subtitles_is_left_alone() {
        let tracks = vec![video(), audio(1, "jpn", "Japanese")];
        let mut session = Session::new(sub_watcher(), None);
        assert!(session.load(tracks, Some(1), None).is_empty());

        let tracks = vec![video(), audio(1, "eng", "English")];
        let mut session = Session::new(dub_watcher(), None);
        assert!(session.load(tracks, Some(1), None).is_empty());
    }

    #[test]
    fn a_forced_only_file_still_gets_its_one_track() {
        // Signs are not what a sub watcher wants, but they are better than nothing — and mpv
        // shows nothing here, because the track is not flagged default.
        let tracks = vec![video(), audio(1, "jpn", "Japanese"), forced(sub(1, "eng", "Signs"))];
        let mut session = Session::new(sub_watcher(), None);
        let actions = session.load(tracks, Some(1), None);
        assert_eq!(sets(&actions), [&TrackAction::SetSid(Some(1))]);

        let tracks = vec![video(), audio(1, "eng", "English"), forced(sub(1, "eng", "Signs"))];
        let mut session = Session::new(dub_watcher(), None);
        let actions = session.load(tracks, Some(1), None);
        assert_eq!(sets(&actions), [&TrackAction::SetSid(Some(1))]);
    }

    #[test]
    fn full_subtitles_outrank_sdh_and_sdh_outranks_signs() {
        let signs = forced(sub(1, "eng", "Signs & Songs"));
        let captions = sdh(sub(2, "eng", "English SDH"));
        let full = sub(3, "eng", "Full Subtitles");
        let jpn = audio(1, "jpn", "Japanese");

        let tracks = vec![video(), jpn.clone(), signs.clone(), captions.clone()];
        let mut session = Session::new(sub_watcher(), None);
        let actions = session.load(tracks, Some(1), Some(1));
        assert_eq!(sets(&actions), [&TrackAction::SetSid(Some(2))]);

        let tracks = vec![video(), jpn, signs, captions, full];
        let mut session = Session::new(sub_watcher(), None);
        let actions = session.load(tracks, Some(1), Some(1));
        assert_eq!(sets(&actions), [&TrackAction::SetSid(Some(3))]);
    }

    #[test]
    fn a_dub_watcher_prefers_signs_proper_to_songs_alone() {
        let tracks = vec![
            video(),
            audio(1, "eng", "English"),
            sub(1, "eng", "Songs"),
            sub(2, "eng", "Signs & Songs"),
            sub(3, "eng", "Full Subtitles"),
        ];
        let mut session = Session::new(dub_watcher(), None);
        let actions = session.load(tracks, Some(1), Some(3));
        assert_eq!(sets(&actions), [&TrackAction::SetSid(Some(2))]);
    }

    #[test]
    fn an_external_file_named_for_the_language_counts_as_that_language() {
        let tracks = vec![
            video(),
            audio(1, "jpn", "Japanese"),
            sub(1, "spa", "Spanish"),
            external(2, "https://cdn.example/subs/ep05-eng.vtt?token=abc"),
        ];
        let mut session = Session::new(sub_watcher(), None);
        let actions = session.load(tracks, Some(1), Some(1));
        assert_eq!(sets(&actions), [&TrackAction::SetSid(Some(2))]);
    }

    #[test]
    fn an_untagged_file_stands_in_when_nothing_says_it_is_ours() {
        // A source's one subtitle file, named by a hash. Ranked below anything that says it is
        // English, but above a track that plainly is not.
        let tracks = vec![
            video(),
            audio(1, "jpn", "Japanese"),
            sub(1, "spa", "Spanish"),
            external(2, "https://cdn.example/a8f3c2.vtt"),
        ];
        let mut session = Session::new(sub_watcher(), None);
        let actions = session.load(tracks, Some(1), Some(1));
        assert_eq!(sets(&actions), [&TrackAction::SetSid(Some(2))]);
    }

    #[test]
    fn a_label_sent_as_the_language_is_read_as_one() {
        assert!(language_matches("English (CR English)", "eng"));
        assert!(language_matches("Portuguese (CR Portuguese(Brazil))", "pt"));
        assert!(!language_matches("Italian (CR Italian)", "eng"));

        // As one source lists its tracks, with mpv having settled on one of the others.
        let label = |id, name: &str| sub(id, name, name);
        let tracks = vec![
            video(),
            audio(1, "jpn", "Japanese"),
            label(1, "Portuguese (CR Portuguese(Brazil))"),
            label(2, "English (CR English)"),
            label(3, "Italian (CR Italian)"),
        ];
        let mut session = Session::new(sub_watcher(), None);
        let actions = session.load(tracks, Some(1), Some(3));
        assert_eq!(sets(&actions), [&TrackAction::SetSid(Some(2))]);
    }

    #[test]
    fn subtitles_in_no_wanted_language_are_left_to_mpv() {
        let tracks = vec![video(), audio(1, "jpn", "Japanese"), sub(1, "spa", "Spanish")];
        let mut session = Session::new(sub_watcher(), None);
        assert!(session.load(tracks.clone(), Some(1), Some(1)).is_empty());
        let mut session = Session::new(sub_watcher(), None);
        assert!(session.load(tracks, Some(1), None).is_empty());
    }

    #[test]
    fn with_no_language_preference_subtitles_are_left_to_mpv() {
        let preferences = TrackPreferences { translation: Translation::Sub, language: None };
        let mut session = Session::new(preferences, None);
        // Audio still follows the translation: a sub watcher wants the original.
        let actions = session.load(dual_audio(), Some(1), Some(5));
        assert_eq!(sets(&actions), [&TrackAction::SetAid(2)]);
    }

    #[test]
    fn a_list_that_arrived_before_the_load_is_used_at_the_load() {
        // Coalesced reports can deliver the list with mpv's pick before `file-loaded`, and no
        // list after it — the selection has not changed since.
        let mut tracks = dual_audio();
        for t in &mut tracks {
            t.selected = t.kind == TrackKind::Video || t.id == 1;
        }
        let mut session = Session::new(sub_watcher(), None);
        session.mpv.tracks = tracks.clone();
        assert!(session.feed(TrackInput::TrackList(tracks)).is_empty(), "not loaded yet");
        let actions = session.feed(TrackInput::FileLoaded);
        assert_eq!(sets(&actions), [&TrackAction::SetAid(2), &TrackAction::SetSid(Some(2))]);
    }

    #[test]
    fn a_file_still_opening_is_not_chosen_for() {
        // Before mpv has picked, nothing in the list is selected; choosing then would send a
        // set for every kind, most of them redundant once mpv's own pick lands.
        let mut session = Session::new(sub_watcher(), None);
        session.mpv.tracks = dual_audio();
        assert!(session.feed(TrackInput::FileLoaded).is_empty());
        assert!(session.feed(TrackInput::TrackList(dual_audio())).is_empty());
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

    // --- Across episodes -------------------------------------------------------------------

    /// The same release, next episode: the same tracks in another order, so every id differs.
    fn reordered() -> Vec<Track> {
        vec![
            video(),
            audio(1, "jpn", "Japanese"),
            flagged_default(audio(2, "eng", "English")),
            sdh(sub(1, "eng", "English SDH")),
            sub(2, "spa", "Spanish"),
            forced(sub(3, "eng", "Signs & Songs")),
            flagged_default(sub(4, "eng", "Full Subtitles")),
        ]
    }

    #[test]
    fn a_remembered_track_is_found_again_under_another_id() {
        let mut session = Session::new(sub_watcher(), None);
        session.load(dual_audio(), Some(2), Some(2));
        session.wait(10.0);
        session.viewer_selects(TrackKind::Sub, Some(3));
        session.next_episode();

        // The heuristic would keep Full Subtitles, now id 4; the viewer chose SDH, now id 1.
        let actions = session.load(reordered(), Some(1), Some(4));
        assert_eq!(sets(&actions), [&TrackAction::SetSid(Some(1))]);
    }

    #[test]
    fn a_remembered_track_is_recognised_in_another_release() {
        // Another source: other titles, but the same language, role and flags.
        let print = sdh(sub(3, "eng", "English SDH")).print();
        let memory = subtitles_memory(SubtitleChoice::Track(print), "jpn");
        let tracks = vec![
            video(),
            audio(1, "ja", "Original"),
            flagged_default(sub(1, "en", "Dialogue")),
            sdh(sub(2, "en", "Closed Captions")),
        ];
        let mut session = Session::new(sub_watcher(), Some(memory));
        let actions = session.load(tracks, Some(1), Some(1));
        assert_eq!(sets(&actions), [&TrackAction::SetSid(Some(2))]);
    }

    #[test]
    fn a_remembered_external_file_is_recognised_by_what_its_name_says() {
        // Per-episode file names: only the language survives normalising, and that is enough.
        let print = external(5, "https://cdn.example/subs/ep01-eng.vtt?token=a").print();
        assert_eq!(print.title, "eng");
        let memory = subtitles_memory(SubtitleChoice::Track(print), "jpn");
        let tracks = vec![
            video(),
            audio(1, "jpn", "Japanese"),
            flagged_default(sub(1, "eng", "English")),
            external(2, "https://cdn.example/subs/ep02-eng.vtt?token=b"),
        ];
        let mut session = Session::new(sub_watcher(), Some(memory));
        let actions = session.load(tracks, Some(1), Some(1));
        assert_eq!(sets(&actions), [&TrackAction::SetSid(Some(2))]);
    }

    #[test]
    fn a_remembered_track_with_no_counterpart_falls_back_to_the_heuristic() {
        let print = sdh(sub(3, "eng", "English SDH")).print();
        let memory = subtitles_memory(SubtitleChoice::Track(print), "jpn");
        let tracks = vec![
            video(),
            audio(1, "jpn", "Japanese"),
            forced(sub(1, "eng", "Signs & Songs")),
            sub(2, "eng", "Full Subtitles"),
        ];
        let mut session = Session::new(sub_watcher(), Some(memory));
        let actions = session.load(tracks, Some(1), Some(1));
        assert_eq!(sets(&actions), [&TrackAction::SetSid(Some(2))]);
    }

    #[test]
    fn a_remembered_signs_track_is_never_answered_with_full_subtitles() {
        // Releases title both tracks "English" often enough. The title alone would match the
        // wrong one; the role has to agree.
        let print = forced(sub(1, "eng", "English")).print();
        let memory = subtitles_memory(SubtitleChoice::Track(print), "jpn");
        let tracks = vec![
            video(),
            audio(1, "jpn", "Japanese"),
            flagged_default(sub(1, "eng", "English")),
            forced(sub(2, "eng", "English")),
        ];
        let mut session = Session::new(sub_watcher(), Some(memory));
        let actions = session.load(tracks, Some(1), Some(1));
        assert_eq!(sets(&actions), [&TrackAction::SetSid(Some(2))]);
    }

    #[test]
    fn a_subtitle_choice_applies_only_over_the_audio_it_was_made_over() {
        // Off, chosen while the dub played. On an episode with only Japanese audio that would
        // leave nothing to read.
        let memory = subtitles_memory(SubtitleChoice::Off, "eng");
        let tracks = vec![video(), audio(1, "jpn", "Japanese"), sub(1, "eng", "English")];
        let mut session = Session::new(dub_watcher(), Some(memory.clone()));
        let actions = session.load(tracks, Some(1), None);
        assert_eq!(sets(&actions), [&TrackAction::SetSid(Some(1))]);

        // Over English audio it applies, signs track or not.
        let tracks = vec![video(), audio(1, "eng", "English"), forced(sub(1, "eng", "Signs"))];
        let mut session = Session::new(dub_watcher(), Some(memory));
        assert!(session.load(tracks, Some(1), None).is_empty());
    }

    #[test]
    fn a_remembered_audio_choice_carries_over() {
        // A sub watcher who prefers this one series dubbed, without changing their setting.
        let mut session = Session::new(sub_watcher(), None);
        session.load(dual_audio(), Some(2), Some(2));
        session.wait(10.0);
        session.viewer_selects(TrackKind::Audio, Some(1));
        session.viewer_selects(TrackKind::Sub, Some(1));
        session.next_episode();

        let actions = session.load(reordered(), Some(1), Some(4));
        assert_eq!(sets(&actions), [&TrackAction::SetAid(2), &TrackAction::SetSid(Some(3))]);
    }

    #[test]
    fn prepare_swaps_in_the_next_series_memory() {
        let mut session = Session::new(sub_watcher(), None);
        session.load(dual_audio(), Some(2), Some(2));
        session.wait(10.0);
        session.viewer_selects(TrackKind::Sub, None);
        session.next_episode();

        // Another series, nothing remembered: the heuristic again, not the last one's "off".
        assert!(session.policy.prepare(sub_watcher(), None).is_empty());
        let actions = session.load(dual_audio(), Some(2), None);
        assert_eq!(sets(&actions), [&TrackAction::SetSid(Some(2))]);
    }

    // --- Subtitle delay --------------------------------------------------------------------

    #[test]
    fn mpvs_first_delay_report_is_not_a_choice() {
        // The observer greeting states mpv's starting value; remembering it would only save
        // the user's own mpv.conf back to them.
        let mut session = Session::new(sub_watcher(), None);
        session.load(dual_audio(), Some(2), Some(2));
        session.wait(10.0);
        assert!(session.feed(TrackInput::Tick).is_empty());
        assert_eq!(session.policy.memory().sub_delay, None);
    }

    #[test]
    fn nudges_are_remembered_once_the_viewer_stops() {
        let mut session = Session::new(sub_watcher(), None);
        session.load(dual_audio(), Some(2), Some(2));
        session.wait(10.0);

        let mut actions = Vec::new();
        for _ in 0..3 {
            actions.extend(session.viewer_moves_delay(0.1));
            session.wait(0.2);
        }
        assert!(actions.is_empty(), "remembered mid-nudge: {actions:?}");
        session.wait(1.0);
        assert!(session.feed(TrackInput::Tick).is_empty(), "still inside the quiet period");
        session.wait(0.5);
        let actions = session.feed(TrackInput::Tick);
        let memory = remembered(&actions);
        assert_eq!(memory.len(), 1);
        let delay = memory[0].sub_delay.unwrap();
        assert!((delay - 0.3).abs() < 1e-9, "remembered {delay}");
        assert!(session.feed(TrackInput::Tick).is_empty(), "and only once");
    }

    #[test]
    fn a_delay_we_set_is_not_read_back_as_the_viewers() {
        let memory = TrackMemory { sub_delay: Some(0.4), ..TrackMemory::default() };
        let mut session = Session::new(sub_watcher(), Some(memory));
        let actions = session.load(dual_audio(), Some(2), Some(2));
        assert_eq!(sets(&actions), [&TrackAction::SetSubDelay(0.4)]);
        session.wait(10.0);
        assert!(session.feed(TrackInput::Tick).is_empty());
    }

    #[test]
    fn a_carried_delay_that_is_already_right_is_not_set_again() {
        // One mpv process for the binge: a delay set in one episode is still set in the next.
        let mut session = Session::new(sub_watcher(), None);
        session.load(dual_audio(), Some(2), Some(2));
        session.wait(10.0);
        session.viewer_moves_delay(-0.5);
        session.wait(2.0);
        assert_eq!(remembered(&session.feed(TrackInput::Tick)).len(), 1);

        session.next_episode();
        assert!(session.load(dual_audio(), Some(2), Some(2)).is_empty());
    }

    #[test]
    fn a_delay_from_one_series_does_not_ride_into_the_next() {
        let mut session = Session::new(sub_watcher(), None);
        session.load(dual_audio(), Some(2), Some(2));
        session.wait(10.0);
        session.viewer_moves_delay(1.5);
        session.wait(2.0);
        session.feed(TrackInput::Tick);
        session.next_episode();

        session.policy.prepare(sub_watcher(), None);
        let actions = session.load(dual_audio(), Some(2), Some(2));
        assert_eq!(sets(&actions), [&TrackAction::SetSubDelay(0.0)], "back to mpv's own");
    }

    #[test]
    fn a_delay_still_settling_is_handed_over_rather_than_lost() {
        // The viewer nudges, and the episode ends before the quiet period is up.
        let mut session = Session::new(sub_watcher(), None);
        session.load(dual_audio(), Some(2), Some(2));
        session.wait(10.0);
        session.viewer_moves_delay(0.2);
        let actions = session.feed(TrackInput::EndFile);
        assert_eq!(remembered(&actions).len(), 1, "flushed at the end of the file");

        // Or the next series is prepared first: what is owed goes back to the caller.
        session.load(dual_audio(), Some(2), Some(2));
        session.wait(10.0);
        session.viewer_moves_delay(0.2);
        let owed = session.policy.prepare(dub_watcher(), None);
        let delay = remembered(&owed)[0].sub_delay.unwrap();
        assert!((delay - 0.4).abs() < 1e-9, "remembered {delay}");
    }
}
