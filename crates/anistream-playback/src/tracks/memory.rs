//! What is remembered per series: the viewer's own audio and subtitle choices and their
//! subtitle delay, and how a remembered track is recognised in the next file, where every
//! id is different.

use std::{cmp::Reverse, time::Instant};

use super::language::same_language;
use super::track::{Role, role};
use super::{
    DELAY_QUIET, SubtitleChoice, SubtitleMemory, Track, TrackAction, TrackKind, TrackPolicy,
    TrackPrint, same_delay,
};

impl TrackPolicy {
    pub(super) fn remember_selection(
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

    pub(super) fn delay_changed(&mut self, seconds: f64, now: Instant) {
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
    pub(super) fn flush_delay(&mut self, now: Option<Instant>, actions: &mut Vec<TrackAction>) {
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

/// The track that best answers a remembered one, if any answers it well enough.
///
/// Ids are useless across files, so a track is recognised by what it is. The language must
/// agree and so must the role — a signs track is never the answer to full subtitles, however
/// alike the titles. Beyond that, in order: the same title and flags; the same flags under
/// another title (a different release); the same title with different flags; the role alone.
/// Whether it is external and its codec break ties.
pub(super) fn best_match<'a>(
    print: &TrackPrint,
    candidates: &[&'a Track],
) -> Option<&'a Track> {
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
    use crate::tracks::fixtures::*;

    fn subtitles_memory(choice: SubtitleChoice, audio_lang: &str) -> TrackMemory {
        TrackMemory {
            subtitles: Some(SubtitleMemory { choice, audio_lang: Some(audio_lang.into()) }),
            ..TrackMemory::default()
        }
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
