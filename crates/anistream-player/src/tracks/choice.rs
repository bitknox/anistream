//! Choosing a file's tracks: the programme's audio in the language the translation asks
//! for, and the subtitles that go with it, signs for a dub watcher and dialogue for everyone
//! else, with what the series remembers ahead of either.

use std::time::Instant;

use anistream_core::Translation;

use super::language::same_language;
use super::memory::best_match;
use super::track::{Role, role};
use super::{
    SETTLE, SubtitleChoice, SubtitleMemory, Track, TrackAction, TrackKind, TrackPolicy,
    language_matches, same_delay,
};

impl TrackPolicy {
    /// Make this file's choice.
    pub(super) fn choose(&mut self, now: Instant, actions: &mut Vec<TrackAction>) {
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

    pub(super) fn choose_tracks(&mut self, actions: &mut Vec<TrackAction>) {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tracks::fixtures::*;

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
}
