//! What the track policy's tests share: mpv's own list for a dual-audio release, tracks
//! built by hand, the two kinds of viewer, and a policy wired to a fake mpv on a clock the
//! test moves by hand, with the names the tests are written in.

use std::collections::VecDeque;

pub(super) use super::*;

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

pub(super) fn dual_audio() -> Vec<Track> {
    parse_track_list(&serde_json::from_str(DUAL_AUDIO).unwrap())
}

pub(super) fn track(
    kind: TrackKind,
    id: i64,
    lang: Option<&str>,
    title: Option<&str>,
) -> Track {
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

pub(super) fn video() -> Track {
    track(TrackKind::Video, 1, None, None)
}

pub(super) fn audio(id: i64, lang: &str, title: &str) -> Track {
    track(TrackKind::Audio, id, Some(lang), Some(title))
}

pub(super) fn sub(id: i64, lang: &str, title: &str) -> Track {
    track(TrackKind::Sub, id, Some(lang), Some(title))
}

/// An external file as mpv lists one it was handed with `--sub-file`: titled by its file
/// name, with no language.
pub(super) fn external(id: i64, url: &str) -> Track {
    let name = url.rsplit('/').next().unwrap();
    Track {
        external: true,
        external_filename: Some(url.into()),
        codec: Some("subrip".into()),
        ..track(TrackKind::Sub, id, None, Some(name))
    }
}

pub(super) fn forced(mut t: Track) -> Track {
    t.forced = true;
    t
}

pub(super) fn flagged_default(mut t: Track) -> Track {
    t.default = true;
    t
}

pub(super) fn sdh(mut t: Track) -> Track {
    t.hearing_impaired = true;
    t
}

pub(super) fn sub_watcher() -> TrackPreferences {
    TrackPreferences { translation: Translation::Sub, language: Some("eng".into()) }
}

pub(super) fn dub_watcher() -> TrackPreferences {
    TrackPreferences { translation: Translation::Dub, language: Some("eng".into()) }
}

/// Just enough of mpv: it holds a selection, and reports a change the way 0.41 does —
/// the whole list restated, then the property — and reports nothing when nothing changed.
pub(super) struct FakeMpv {
    pub(super) tracks: Vec<Track>,
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
pub(super) struct Session {
    pub(super) policy: TrackPolicy,
    pub(super) mpv: FakeMpv,
    start: Instant,
    elapsed: Duration,
}

impl Session {
    /// A fresh mpv process: observers greeted with an empty list and a zero delay.
    pub(super) fn new(preferences: TrackPreferences, memory: Option<TrackMemory>) -> Self {
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

    pub(super) fn wait(&mut self, seconds: f64) {
        self.elapsed += Duration::from_secs_f64(seconds);
    }

    pub(super) fn feed(&mut self, input: TrackInput) -> Vec<TrackAction> {
        self.feed_all(vec![input])
    }

    /// Feed reports mpv has already sent, carry out what comes back, and feed mpv's
    /// reports of that in turn until everything is quiet.
    ///
    /// One queue, because that is the order on the socket: what mpv had already said is
    /// read before the echo of anything we send in reply to the first of it.
    pub(super) fn feed_all(&mut self, inputs: Vec<TrackInput>) -> Vec<TrackAction> {
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
    pub(super) fn load(
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
    pub(super) fn next_episode(&mut self) -> Vec<TrackAction> {
        let mut actions = self.feed(TrackInput::EndFile);
        self.mpv.tracks.clear();
        actions.extend(self.feed(TrackInput::TrackList(Vec::new())));
        self.wait(1.0);
        actions
    }

    /// The viewer picks a track in the player.
    pub(super) fn viewer_selects(
        &mut self,
        kind: TrackKind,
        id: Option<i64>,
    ) -> Vec<TrackAction> {
        let reports = self.mpv.select(kind, id);
        self.feed_all(reports)
    }

    /// The viewer picks a track in a hosted player's own menu, which the engine carries out:
    /// the policy's answer is done to mpv, and mpv's reports of it fed back.
    pub(super) fn viewer_picks(
        &mut self,
        kind: TrackKind,
        id: Option<i64>,
    ) -> Vec<TrackAction> {
        let mut actions = self.policy.viewer_chose(kind, id);
        let reports: Vec<_> = actions.iter().flat_map(|a| self.mpv.carry_out(a)).collect();
        actions.extend(self.feed_all(reports));
        actions
    }

    /// The viewer sets the delay in a hosted player's own controls.
    pub(super) fn viewer_sets_delay(&mut self, seconds: f64) -> Vec<TrackAction> {
        let now = self.now();
        let mut actions = self.policy.viewer_delay(seconds, now);
        let reports: Vec<_> = actions.iter().flat_map(|a| self.mpv.carry_out(a)).collect();
        actions.extend(self.feed_all(reports));
        actions
    }

    pub(super) fn viewer_moves_delay(&mut self, by: f64) -> Vec<TrackAction> {
        let reports = self.mpv.set_delay(self.mpv.delay + by);
        self.feed_all(reports)
    }

    pub(super) fn selection(&self) -> (Option<i64>, Option<i64>) {
        (self.mpv.current(TrackKind::Audio), self.mpv.current(TrackKind::Sub))
    }
}

pub(super) fn sets(actions: &[TrackAction]) -> Vec<&TrackAction> {
    actions.iter().filter(|a| !matches!(a, TrackAction::Remember(_))).collect()
}

pub(super) fn remembered(actions: &[TrackAction]) -> Vec<&TrackMemory> {
    actions
        .iter()
        .filter_map(|a| match a {
            TrackAction::Remember(memory) => Some(memory),
            _ => None,
        })
        .collect()
}
