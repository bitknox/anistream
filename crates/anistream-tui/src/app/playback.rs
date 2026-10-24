//! Now Playing: a playback staged, the reports the player sends, the keys that drive it, and
//! its end.

use anistream_core::ids::AnilistId;
use anistream_model::{
    command::{Command, PlayerCommand},
    notice::Notice,
    playback::{Ending, SkipKind},
};

use super::App;
use crate::{eyecatch::Eyecatch, keymap::Action, nav::StageView};

/// Live playback state for the Now Playing surface.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct NowPlaying {
    /// Which title is playing, so a watch is reflected onto that entry and no other —
    /// the Home list can hold several shows whose next episode shares a number.
    pub id: Option<AnilistId>,
    pub title: String,
    pub episode: String,
    pub episode_title: Option<String>,
    pub position: f64,
    pub duration: Option<f64>,
    pub paused: bool,
    pub speed: f64,
    /// Set while a skip is offered: past what, and where it would seek to.
    pub skip: Option<(SkipKind, f64)>,
}

/// How far `[` and `]` step the speed.
const SPEED_STEP: f64 = 0.25;

impl NowPlaying {
    pub fn fraction(&self) -> f64 {
        match self.duration {
            Some(d) if d > 0.0 => (self.position / d).clamp(0.0, 1.0),
            _ => 0.0,
        }
    }

    /// `m:ss` — the form a player uses, not a wall clock.
    pub fn clock(seconds: f64) -> String {
        let total = seconds.max(0.0) as u64;
        let (hours, minutes, secs) = (total / 3600, (total % 3600) / 60, total % 60);
        if hours > 0 {
            format!("{hours}:{minutes:02}:{secs:02}")
        } else {
            format!("{minutes}:{secs:02}")
        }
    }

    /// Elapsed, padded to the width of the total.
    ///
    /// `9:12 / 23:55` is ragged; ` 9:12 / 23:55` is a clock. The pair sits in a fixed field so
    /// the digits do not shuffle sideways once a minute.
    pub fn elapsed(&self) -> String {
        let elapsed = Self::clock(self.position);
        let width = self.total().chars().count();
        format!("{elapsed:>width$}")
    }

    pub fn total(&self) -> String {
        self.duration.map_or_else(|| "--:--".into(), Self::clock)
    }
}

impl App {
    /// Whether a mark made here is waiting on the engine's word: see `marking`.
    pub fn awaiting_mark(&self) -> bool {
        self.marking.is_some()
    }

    /// Whether the screen is waiting on the engine's word before it is drawn again or reads
    /// another key: a mark made here, or the staging of the episode an ending rolled on to.
    /// See `marking` and `rolling`.
    pub fn awaiting_engine(&self) -> bool {
        self.marking.is_some() || self.rolling
    }

    /// Dispatch a playback binding.
    ///
    /// Returns `None` when the action is not a playback one, so the caller falls through to
    /// normal navigation. `Some(None)` means it was handled with no async work.
    #[allow(clippy::option_option)]
    pub(super) fn handle_playback(&mut self, action: Action) -> Option<Option<Command>> {
        use PlayerCommand as P;
        let command = match action {
            Action::PlayPause => P::TogglePause,
            Action::SeekBack => P::Seek(-5.0),
            Action::SeekForward => P::Seek(5.0),
            Action::SeekBackFar => P::Seek(-30.0),
            Action::SeekForwardFar => P::Seek(30.0),
            Action::SpeedDown => P::SpeedTo(self.speed_step(false)),
            Action::SpeedUp => P::SpeedTo(self.speed_step(true)),
            Action::VolumeDown => P::Volume(-5.0),
            Action::VolumeUp => P::Volume(5.0),
            Action::Fullscreen => P::Fullscreen,
            Action::Detach => {
                // mpv keeps playing; we just stop looking at it. Progress recording continues,
                // because the session outlives the screen.
                self.push_toast(Notice::info("detached — mpv is still playing"));
                self.nav.pop();
                return Some(None);
            }
            Action::StopPlayback => P::Stop,
            Action::SkipOpening => {
                let Some(playing) = &self.playing else { return Some(None) };
                match playing.skip {
                    Some((_, to)) => P::SeekTo(to),
                    // Better to say nothing is offered than to seek somewhere arbitrary.
                    None => {
                        self.push_toast(Notice::info("no skip here"));
                        return Some(None);
                    }
                }
            }
            Action::NextEpisode => return Some(self.step_episode(1)),
            Action::PreviousEpisode => return Some(self.step_episode(-1)),
            _ => return None,
        };
        Some(Some(Command::Player(command)))
    }

    /// Play the first episode local history has not completed.
    ///
    /// Driven off the watch log rather than a tracker, so this works with no account at all.
    pub(super) fn play_next_unwatched(&mut self) -> Option<Command> {
        let Some(entry) = self.target() else {
            self.push_toast(Notice::info("nothing selected"));
            return None;
        };
        let id = entry.id;
        // Its table, loaded, is AniList's alone: there is nothing to play it from yet.
        if self.unlisted(id) {
            return self.go_to_section(crate::nav::Section::Providers);
        }
        let episode =
            entry.progress.map(|p| p.next.to_string()).unwrap_or_else(|| "1".to_string());
        Some(self.begin_playback(id, episode))
    }

    /// The episode Enter would play, with the Syncplay room.
    pub(super) fn play_party(&mut self) -> Option<Command> {
        // The same episode Enter would play, but with the room. No Now Playing
        // view: Syncplay owns the player, and a dead session screen would lie.
        let (id, episode) = match self.nav.current() {
            StageView::Episodes(id) => {
                (*id, self.episodes.get(self.cursors.episodes.selected)?.number.clone())
            }
            _ => {
                let entry = self.target()?;
                let episode = entry.progress.map_or(1, |p| p.next).to_string();
                (entry.id, episode)
            }
        };
        Some(Command::PlayParty { id, episode })
    }

    /// Step to an adjacent episode of what is playing, from whichever view is up.
    ///
    /// Asked of the playback rather than worked out here. The open title page can be another
    /// show entirely, or no show at all after starting from Continue, and answering from it
    /// played that show's episode, or nothing. The playback holds the episode list of the title
    /// it plays, so it is also the side that can say there is no next episode instead of
    /// handing the finale's window to a search for one.
    fn step_episode(&mut self, delta: i64) -> Option<Command> {
        self.playing.as_ref()?;
        Some(Command::Player(PlayerCommand::Step(delta)))
    }

    /// Push Now Playing, raise the eyecatch, and ask for the stream.
    ///
    /// The order matters: the wipe goes up *before* resolution starts, so the user never sees
    /// a frozen episode table while a provider is being tried.
    pub(super) fn begin_playback(&mut self, id: AnilistId, episode: String) -> Command {
        self.raise_now_playing(id, &episode);
        Command::play(id, episode)
    }

    /// The visual half of starting playback, shared with the Sources pick: eyecatch up,
    /// Now Playing staged, view pushed.
    pub(super) fn raise_now_playing(&mut self, id: AnilistId, episode: &str) {
        // Named by the title being played, never by whichever was opened last: Space on a list
        // row put the previously opened show's name over the wipe.
        let title = [self.detail.as_ref(), self.selected_entry()]
            .into_iter()
            .flatten()
            .find(|e| e.id == id)
            .map_or_else(|| "playing".to_string(), |e| e.title.clone());
        self.raise_now_playing_titled(id, title, episode);
    }

    /// The same staging with the title stated by the caller — the Downloads screen knows
    /// what its rows are called without any list selection being involved.
    pub(super) fn raise_now_playing_titled(
        &mut self,
        id: AnilistId,
        title: String,
        episode: &str,
    ) {
        let playing = NowPlaying {
            id: Some(id),
            title,
            episode: episode.to_owned(),
            // From this title's table only: the open one can be another show's.
            episode_title: self
                .episode_rows_for(id)
                .iter()
                .find(|e| e.number == episode)
                .and_then(|e| e.title.clone()),
            speed: self.settings.speed.unwrap_or(1.0),
            ..NowPlaying::default()
        };
        self.put_up(playing);
    }

    /// Now Playing for a playback just staged: the eyecatch over it while its stream is found,
    /// and the view pushed.
    fn put_up(&mut self, playing: NowPlaying) {
        self.eyecatch =
            Some(Eyecatch::new(format!("{}  ·  ep {}", playing.title, playing.episode)));
        // One that has reported keeps playing until this one is found; one merely staged before
        // this was never going to play.
        let replaced = self.playing.take().filter(|p| p.duration.is_some());
        if replaced.is_some() && self.superseded.is_none() {
            self.superseded = replaced;
        }
        self.playing = Some(playing);
        self.nav.push(StageView::NowPlaying);
    }

    /// A playback the engine has staged, staged here as it says: see [`Event::PlaybackStaged`].
    ///
    /// Idempotent. A playback staged here already — as this asked for it, or as the engine said
    /// it once before — is left as it is, eyecatch and all: it is the same playback while it has
    /// not started playing, and the same title and episode.
    ///
    /// The episode an ending rolled on to takes the finished one's place: that one comes down
    /// as any ending takes it, Now Playing with it, and goes back up for this one at once.
    ///
    /// [`Event::PlaybackStaged`]: anistream_model::event::Event::PlaybackStaged
    pub(super) fn take_staged(
        &mut self,
        id: AnilistId,
        title: String,
        episode: String,
        episode_title: Option<String>,
        speed: f64,
    ) {
        if std::mem::take(&mut self.rolling) {
            self.playing = None;
            if matches!(self.nav.current(), StageView::NowPlaying) {
                self.nav.pop();
            }
        } else if self.playing.as_ref().is_some_and(|playing| {
            playing.id == Some(id) && playing.episode == episode && playing.duration.is_none()
        }) {
            return;
        }
        let playing = NowPlaying {
            id: Some(id),
            title,
            episode,
            episode_title,
            speed,
            ..NowPlaying::default()
        };
        self.put_up(playing);
    }

    /// A report from the player: where it is, how long the episode runs, and whether it is
    /// paused.
    pub(super) fn apply_playback(
        &mut self,
        position: f64,
        duration: Option<f64>,
        paused: bool,
    ) {
        // Playback updates arrive about once a second and must never resurrect a
        // finished session, so they only ever mutate an existing one.
        // What the playhead moves of the title's progress, the engine says ahead of it.
        if let Some(playing) = self.superseded.as_mut().or(self.playing.as_mut()) {
            playing.position = position;
            playing.paused = paused;
            if duration.is_some() {
                playing.duration = duration;
            }
        }
    }

    /// A skip on offer: past what, and where it would seek to.
    pub(super) fn apply_skip_available(&mut self, kind: SkipKind, to: f64) {
        if let Some(playing) = &mut self.playing {
            playing.skip = Some((kind, to));
        }
    }

    /// The skip on offer is gone.
    pub(super) fn apply_skip_cleared(&mut self) {
        if let Some(playing) = &mut self.playing {
            playing.skip = None;
        }
    }

    /// The speed the player plays at now.
    pub(super) fn apply_playback_speed(&mut self, speed: f64) {
        if let Some(playing) = &mut self.playing {
            playing.speed = speed;
        }
    }

    /// The speed a step faster (`up`) or slower from what plays now goes to: the next quarter
    /// that way, within ¼× and 4×. Kept as the speed at once, so a second press before the
    /// player has said so steps on from it.
    fn speed_step(&mut self, up: bool) -> f64 {
        let current = self.playing.as_ref().map_or(1.0, |playing| playing.speed);
        // A hair of slack, for a speed that is a quarter but not quite in binary.
        let quarters = current / SPEED_STEP;
        let target =
            if up { (quarters + 1e-6).floor() + 1.0 } else { (quarters - 1e-6).ceil() - 1.0 };
        let target = (target * SPEED_STEP).clamp(SPEED_STEP, 4.0);
        if let Some(playing) = &mut self.playing {
            playing.speed = target;
        }
        target
    }

    /// The playback staged over the one in the window found nothing to play, so the one in the
    /// window is what Now Playing shows again.
    pub(super) fn apply_playback_kept(&mut self) {
        if let Some(kept) = self.superseded.take() {
            self.playing = Some(kept);
        }
    }

    /// An episode the window has moved on from.
    pub(super) fn apply_episode_left(&mut self, id: AnilistId, episode: String) {
        if self.superseded.as_ref().is_some_and(|s| s.id == Some(id) && s.episode == episode) {
            self.superseded = None;
        }
    }

    /// The end of the playback on screen, with nothing rolling on from it.
    pub(super) fn apply_playback_ended(&mut self, ending: Ending) {
        self.playing = None;
        // Leaving Now Playing on screen with nothing playing would be a dead end.
        if matches!(self.nav.current(), StageView::NowPlaying) {
            self.nav.pop();
        }
        if matches!(ending, Ending::Complete { .. }) {
            self.push_toast(Notice::info("episode finished"));
        }
    }

    /// A playback that picked up where it was left.
    pub(super) fn apply_resumed(&mut self, position: f64) {
        // Resume happens without asking — mpv is already at the right place by the time
        // this arrives. Saying so is what keeps it from feeling like a glitch.
        self.push_toast(Notice::info(format!("resumed at {}", NowPlaying::clock(position))));
        if let Some(playing) = &mut self.playing {
            playing.position = position;
        }
    }

    /// Advance the eyecatch one frame, dropping it when the wipe finishes.
    ///
    /// A wipe that reaches its deadline reveals without a word. Its clock starts at the keypress,
    /// so most of what it covers is the search for a source, which the activity row is already
    /// narrating — alerting on it said the player never started before mpv had been asked to.
    /// A player that does start and then shows nothing is the playback's to say, timed from
    /// when mpv was handed the file.
    pub fn tick_animation(&mut self) {
        if let Some(eyecatch) = &mut self.eyecatch
            && !eyecatch.advance()
        {
            self.eyecatch = None;
        }
    }

    /// Whether the loop should tick at animation rate rather than idle rate.
    pub fn is_animating(&self) -> bool {
        self.eyecatch.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::fixtures::*;

    #[test]
    fn the_eyecatch_goes_up_before_resolution_starts() {
        // The wipe exists to cover resolution. Raising it after the provider walk finished
        // would put the motion in the one place it is not needed.
        let mut a = app_at_episodes();
        a.handle(Action::Open, 20);
        let eyecatch = a.eyecatch.as_ref().expect("no eyecatch was raised");
        assert_eq!(eyecatch.stage(), crate::eyecatch::Stage::Covering);
        assert!(a.is_animating(), "the loop has to know to tick faster");
    }

    #[test]
    fn a_failure_releases_the_eyecatch_rather_than_holding_amber() {
        // Held forever, the band would leave the user staring at a solid amber screen with no
        // way to know the stream failed.
        let mut a = app_at_episodes();
        a.handle(Action::Open, 20);
        a.apply(Event::Toast(Notice::alert("all providers failed")));
        for _ in 0..crate::eyecatch::SWEEP_FRAMES * 3 {
            a.tick_animation();
        }
        assert!(a.eyecatch.is_none(), "the wipe never finished");
    }

    #[test]
    fn a_wipe_that_reaches_its_deadline_reveals_without_blaming_the_player() {
        // The deadline is counted from the keypress, so a slow source search reached it before
        // mpv had been started. The playback says when a started player shows nothing.
        let mut a = app_at_episodes();
        a.handle(Action::Open, 20);
        for _ in 0..crate::eyecatch::MAX_HOLD_FRAMES + crate::eyecatch::SWEEP_FRAMES * 2 {
            a.tick_animation();
        }
        assert!(a.eyecatch.is_none(), "the wipe never ended");
        assert!(a.toasts.iter().all(|t| t.notice.kind != NoticeKind::Alert), "{:?}", a.toasts);
    }

    #[test]
    fn the_resume_toast_offers_nothing_it_cannot_do() {
        let mut a = app_at_episodes();
        a.handle(Action::Open, 20);
        a.apply(Event::Resumed { position: 87.0 });
        let toast = a.toasts.last().expect("a toast");
        assert_eq!(toast.notice.text, "resumed at 1:27");
    }

    #[test]
    fn n_while_playing_asks_the_playback_from_any_view() {
        // From Continue there is no title open at all, and with another show open the open one
        // is the wrong show: the playback holds the playing title's list, so it answers.
        let mut a = app();
        a.apply(a.content_here(entries(3)));
        a.nav.focus_stage();
        a.handle(Action::Open, 20);
        a.playing = Some(NowPlaying {
            id: Some(AnilistId::new(2)),
            title: "Title 1".into(),
            episode: "4".into(),
            ..NowPlaying::default()
        });
        a.nav.pop();
        assert_eq!(
            a.handle(Action::NextEpisode, 20),
            Some(Command::Player(PlayerCommand::Step(1)))
        );
        a.detail = a.content.entries().first().cloned();
        assert_eq!(
            a.handle(Action::PreviousEpisode, 20),
            Some(Command::Player(PlayerCommand::Step(-1)))
        );
    }

    #[test]
    fn an_episode_that_cannot_be_found_leaves_the_one_playing_on_screen() {
        let mut a = app();
        a.playing = Some(NowPlaying {
            id: Some(AnilistId::new(2)),
            title: "Title 1".into(),
            episode: "4".into(),
            ..NowPlaying::default()
        });
        a.apply(Event::Playback { position: 300.0, duration: Some(1440.0), paused: false });
        a.apply(staged(2, "Title 1", "9", None));
        assert_eq!(a.playing.as_ref().unwrap().episode, "9", "staged while it is found");

        // Episode 4 keeps the window meanwhile, and its reports are its own.
        a.apply(Event::Playback { position: 301.0, duration: Some(1440.0), paused: false });
        assert_eq!(a.playing.as_ref().unwrap().position, 0.0);
        assert!(a.eyecatch.is_some(), "a report from 4 is not 9 starting");

        a.apply(Event::Toast(Notice::alert("a: timed out")));
        a.apply(Event::PlaybackKept);
        let playing = a.playing.as_ref().unwrap();
        assert_eq!((playing.episode.as_str(), playing.position), ("4", 301.0));

        // Found, the window changes hands and what reports from then on is the successor.
        a.apply(staged(2, "Title 1", "5", None));
        a.apply(Event::EpisodeLeft {
            id: AnilistId::new(2),
            episode: "4".into(),
            position: 302.0,
            duration: Some(1440.0),
            watched: false,
        });
        a.apply(Event::Playback { position: 1.0, duration: Some(1420.0), paused: false });
        let playing = a.playing.as_ref().unwrap();
        assert_eq!((playing.episode.as_str(), playing.position), ("5", 1.0));
    }

    #[test]
    fn a_playback_the_engine_stages_is_put_up_as_it_says() {
        // An episode picked in the player, of Title 1, while Title 0's table is open: staged
        // under the name and the episode title the engine gives it, not the open table's.
        let mut a = app_at_episodes();
        a.apply(Event::PlaybackStaged {
            id: AnilistId::new(2),
            title: "Title 1".into(),
            episode: "5".into(),
            episode_title: Some("Fifth".into()),
            speed: 1.25,
        });
        let playing = a.playing.as_ref().expect("staged");
        assert_eq!(
            (playing.id, playing.title.as_str(), playing.episode.as_str()),
            (Some(AnilistId::new(2)), "Title 1", "5")
        );
        assert_eq!(playing.episode_title.as_deref(), Some("Fifth"));
        assert_eq!((playing.speed, playing.duration), (1.25, None));
        assert_eq!(a.eyecatch.as_ref().map(|e| e.label.as_str()), Some("Title 1  ·  ep 5"));
        assert_eq!(a.nav.current(), &StageView::NowPlaying);
    }

    #[test]
    fn a_staging_made_here_or_said_before_changes_nothing() {
        // Enter stages episode 1 here as it asks for it; the engine says it staged the same,
        // once and then again. Nothing moves, the wipe least of all.
        let mut a = app_at_episodes();
        a.handle(Action::Open, 20);
        let eyecatch = a.eyecatch.as_mut().expect("the wipe is up");
        eyecatch.advance();
        let (before, wipe) = (a.playing.clone(), a.eyecatch.clone());
        for _ in 0..2 {
            a.apply(staged(1, "Title 0", "1", Some("Episode 1")));
            assert_eq!(a.playing, before);
            assert_eq!(a.eyecatch, wipe, "the wipe started over");
        }
        // Once it plays, the same episode staged again is a new playback of it: the restart in
        // the other translation.
        a.apply(Event::Playback { position: 60.0, duration: Some(1435.0), paused: false });
        a.apply(staged(1, "Title 0", "1", Some("Episode 1")));
        assert_eq!(a.playing.as_ref().map(|p| p.duration), Some(None), "staged anew");
        assert_ne!(a.eyecatch, wipe);
    }

    #[test]
    fn playback_updates_never_resurrect_a_finished_session() {
        // A position tick can arrive after the `Ended` event that preceded it. Recreating
        // `playing` from it would leave a phantom player on screen.
        let mut a = app_at_episodes();
        a.handle(Action::Open, 20);
        a.apply(Event::playback_ended(false, Ending::Closed));
        assert!(a.playing.is_none());

        a.apply(Event::Playback { position: 100.0, duration: Some(1435.0), paused: false });
        assert!(a.playing.is_none(), "a stray tick brought the player back");
    }

    #[test]
    fn quitting_partway_does_not_roll_on_to_the_next_episode() {
        let mut a = app_at_episodes();
        a.handle(Action::Open, 20);
        a.apply(Event::playback_ended(false, Ending::Closed));
        assert!(a.take_commands().is_empty());
        assert!(a.playing.is_none() && !a.awaiting_engine(), "Now Playing came down");
    }

    #[test]
    fn an_ending_that_rolls_on_keeps_now_playing_up_for_the_next_episode() {
        // Another show's page is open by the time the episode ends. Until the engine stages the
        // episode it rolled on to, Now Playing stays as it was, and the loop waits for it; then
        // the next one is up under the name and the episode title the engine gives it.
        let mut a = app_at_episodes();
        a.handle(Action::Open, 20);
        a.apply(Event::Playback { position: 1430.0, duration: Some(1435.0), paused: false });
        a.detail = a.content.entries().get(1).cloned();
        a.apply(finished(Some("2")));
        assert_eq!(a.playing.as_ref().map(|p| p.episode.as_str()), Some("1"));
        assert_eq!(a.nav.current(), &StageView::NowPlaying);
        assert!(a.awaiting_engine(), "nothing is drawn or read until the next one is staged");

        a.apply(staged(1, "Title 0", "2", Some("Episode 2")));
        assert!(!a.awaiting_engine());
        let playing = a.playing.as_ref().expect("the next episode is staged");
        assert_eq!((playing.title.as_str(), playing.episode.as_str()), ("Title 0", "2"));
        assert_eq!(playing.episode_title.as_deref(), Some("Episode 2"));
        assert_eq!(a.eyecatch.as_ref().map(|e| e.label.as_str()), Some("Title 0  ·  ep 2"));
        assert_eq!(a.nav.current(), &StageView::NowPlaying);
        // The finished one is gone, not kept in the window: what reports now is the next one.
        a.apply(Event::Playback { position: 1.0, duration: Some(1420.0), paused: false });
        assert_eq!(a.playing.as_ref().map(|p| p.position), Some(1.0));
        assert!(
            a.take_commands().is_empty(),
            "the engine started it; nothing is asked for here"
        );
    }

    #[test]
    fn rolling_on_hands_the_keys_back_to_the_screen_as_an_ending_does() {
        // The rail had the keyboard over Now Playing. The finished episode came down, and the
        // next one went up, so Now Playing has it again, as it had when the interface rolled on.
        let mut a = app_at_episodes();
        a.handle(Action::Open, 20);
        a.nav.focus_rail();
        a.apply(finished(Some("2")));
        a.apply(staged(1, "Title 0", "2", Some("Episode 2")));
        assert_eq!(a.nav.focus(), Focus::Stage);
        assert_eq!(a.nav.current(), &StageView::NowPlaying);
    }

    #[test]
    fn an_ending_asks_for_the_next_episode_without_taking_a_busy_slot() {
        // The next episode's streams found ahead, and an episode picked in the player, are the
        // engine's to start: neither takes the slot, so nothing waiting in it is displaced.
        let mut a = app();
        a.apply(Event::PrefetchNext { id: AnilistId::new(3), episode: "6".into() });
        assert!(a.take_commands().is_empty());
        a.playing = Some(NowPlaying {
            id: Some(AnilistId::new(3)),
            title: "Hoshikuzu no Tabibito".into(),
            episode: "5".into(),
            ..NowPlaying::default()
        });
        a.apply(Event::PlayerPlayEpisode { episode: "7".into(), title: None });
        a.apply(Event::PrefetchNext { id: AnilistId::new(3), episode: "6".into() });
        assert!(a.take_commands().is_empty());
        assert_eq!(a.playing.as_ref().map(|p| p.episode.as_str()), Some("5"), "until staged");
    }

    #[test]
    fn a_choice_made_in_the_player_takes_no_slot_and_waits_behind_none() {
        // The engine keeps and saves what the player changes, as it passes. Queued here, each
        // save waited in the one slot, and one that found it taken was skipped.
        let mut a = app();
        a.apply(Event::PrefetchNext { id: AnilistId::new(3), episode: "6".into() });
        for choice in [
            Event::PlayerNightMode(true),
            Event::PlayerUpscaling("sharp".into()),
            Event::PlayerToneMapping("bt.2390".into()),
            Event::PlaybackVolume(80.0),
            Event::PlaybackSpeed(1.5),
        ] {
            a.apply(choice);
        }
        assert!(
            a.take_commands().is_empty(),
            "the ending's work is the engine's, and no save waits"
        );
        // Nor kept here: what is in effect is the engine's to say, in `Settings`.
        assert_eq!(a.settings, SettingsView::default());
    }

    #[test]
    fn nothing_follows_an_ending_the_player_kept_no_window_for() {
        // The finale, auto-next turned off, or Up Next cancelled: the playback names no next
        // episode, and nothing is searched for. Guessing "the number after" here searched for
        // an episode the list said did not exist, after the finale.
        let mut a = app_at_episodes();
        a.set_episode_cursor(11);
        a.handle(Action::Open, 20);
        a.apply(finished(None));
        assert!(a.take_commands().is_empty());
        assert!(a.toasts.iter().any(|t| t.notice.text == "episode finished"));
    }

    #[test]
    fn playback_keys_become_player_commands() {
        let mut a = app_at_episodes();
        a.handle(Action::Open, 20);
        for (action, expected) in [
            (Action::PlayPause, PlayerCommand::TogglePause),
            (Action::SeekForward, PlayerCommand::Seek(5.0)),
            (Action::SeekBackFar, PlayerCommand::Seek(-30.0)),
            (Action::SpeedUp, PlayerCommand::SpeedTo(1.25)),
            (Action::StopPlayback, PlayerCommand::Stop),
        ] {
            assert_eq!(
                a.handle(action, 20),
                Some(Command::Player(expected)),
                "{action:?} did not reach the player"
            );
        }
    }

    #[test]
    fn skip_only_seeks_when_a_skip_is_actually_offered() {
        // Seeking to an arbitrary place because no interval was known would be worse than
        // saying nothing is there.
        let mut a = app_at_episodes();
        a.handle(Action::Open, 20);
        assert_eq!(a.handle(Action::SkipOpening, 20), None);

        a.apply(Event::SkipAvailable { kind: SkipKind::Opening, to: 93.2 });
        assert_eq!(
            a.handle(Action::SkipOpening, 20),
            Some(Command::Player(PlayerCommand::SeekTo(93.2)))
        );

        a.apply(Event::SkipCleared);
        assert_eq!(a.handle(Action::SkipOpening, 20), None);
    }

    #[test]
    fn detaching_leaves_the_player_alone_and_pops_the_screen() {
        // The point of detach: mpv keeps playing and history keeps recording. Sending it a
        // command would defeat the whole gesture.
        let mut a = app_at_episodes();
        a.handle(Action::Open, 20);
        assert_eq!(a.handle(Action::Detach, 20), None, "detach must not touch the player");
        assert!(!matches!(a.nav.current(), StageView::NowPlaying));
    }

    #[test]
    fn playback_bindings_do_not_hijack_browsing() {
        // With nothing playing, `x` and `n` have to fall through to their browsing meaning
        // rather than being swallowed by a player that is not there.
        let mut a = app();
        a.apply(a.content_here(entries(3)));
        assert_eq!(a.handle(Action::NextEpisode, 20), None);
        assert!(a.playing.is_none());
    }

    #[test]
    fn the_speed_keys_step_to_the_next_quarter_from_what_plays() {
        let mut a = app_at_episodes();
        a.handle(Action::Open, 20);
        a.apply(Event::PlaybackSpeed(1.0));
        let to = |speed| Some(Command::Player(PlayerCommand::SpeedTo(speed)));
        assert_eq!(a.handle(Action::SpeedUp, 20), to(1.25));
        // A second press before the player has said so steps on from the first.
        assert_eq!(a.handle(Action::SpeedUp, 20), to(1.5));
        // From a speed between quarters, to the next quarter that way.
        a.apply(Event::PlaybackSpeed(1.1));
        assert_eq!(a.handle(Action::SpeedDown, 20), to(1.0));
        a.apply(Event::PlaybackSpeed(1.1));
        assert_eq!(a.handle(Action::SpeedUp, 20), to(1.25));
        // Never below a quarter, nor past four.
        a.apply(Event::PlaybackSpeed(0.25));
        assert_eq!(a.handle(Action::SpeedDown, 20), to(0.25));
        a.apply(Event::PlaybackSpeed(4.0));
        assert_eq!(a.handle(Action::SpeedUp, 20), to(4.0));
    }

    #[test]
    fn a_remembered_speed_carries_into_the_next_episode() {
        let mut a = app_at_episodes();
        a.handle(Action::Open, 20);
        // The speed, then what the engine makes of it: remembered, for the next episode.
        a.apply(Event::PlaybackSpeed(1.25));
        let mut owner = Owner::new(Config::default());
        owner.effective.playback.persisted_speed = Some(1.25);
        owner.tell(&mut a);
        assert_eq!(a.settings.speed, Some(1.25));

        // Staged by the engine at it, when an ending rolls on...
        a.apply(finished(Some("2")));
        a.apply(Event::PlaybackStaged {
            id: AnilistId::new(1),
            title: "Title 0".into(),
            episode: "2".into(),
            episode_title: Some("Episode 2".into()),
            speed: 1.25,
        });
        assert_eq!(a.playing.as_ref().map(|p| p.speed), Some(1.25));
        // ...and here, when Enter asks for one.
        a.apply(Event::playback_ended(false, Ending::Stopped));
        a.set_episode_cursor(2);
        a.handle(Action::Open, 20);
        assert_eq!(
            a.playing.as_ref().map(|p| (p.episode.as_str(), p.speed)),
            Some(("3", 1.25))
        );
    }

    #[test]
    fn the_playhead_clock_is_a_fixed_field() {
        // Ragged clocks shuffle sideways once a minute. Padding elapsed to the width of total
        // is what makes the pair sit still.
        let playing =
            NowPlaying { position: 552.0, duration: Some(1435.0), ..NowPlaying::default() };
        assert_eq!(playing.elapsed(), " 9:12");
        assert_eq!(playing.total(), "23:55");
        assert_eq!(playing.elapsed().chars().count(), playing.total().chars().count());
    }

    #[test]
    fn an_unknown_duration_reads_as_unknown_rather_than_zero() {
        // Torrent streams often have no duration until enough has been fetched. Showing 0:00
        // would claim the episode is empty.
        let playing = NowPlaying { position: 30.0, duration: None, ..NowPlaying::default() };
        assert_eq!(playing.total(), "--:--");
        assert_eq!(playing.fraction(), 0.0);
    }

    #[test]
    fn the_clock_grows_to_hours_only_when_it_has_to() {
        assert_eq!(NowPlaying::clock(0.0), "0:00");
        assert_eq!(NowPlaying::clock(59.4), "0:59");
        assert_eq!(NowPlaying::clock(3599.0), "59:59");
        assert_eq!(NowPlaying::clock(3600.0), "1:00:00");
        // A negative position is a bad IPC reply, not a reason to render garbage.
        assert_eq!(NowPlaying::clock(-5.0), "0:00");
    }

    #[test]
    fn on_now_playing_the_title_keys_mean_the_title_playing() {
        // Space from a list plays the row's next episode while an earlier title is still the
        // last one opened; Now Playing is about the one playing.
        let mut a = home_list(5);
        a.handle(Action::Open, 5);
        a.handle(Action::Back, 5);
        a.handle(Action::Down, 5);
        a.handle(Action::Down, 5);
        a.handle(Action::PlayNext, 5);
        assert_eq!(a.nav.current(), &StageView::NowPlaying);
        assert_eq!(a.target().map(|e| e.id), Some(AnilistId::new(3)));
        assert_eq!(
            a.handle(Action::OpenInBrowser, 5),
            Some(Command::OpenExternal { url: "https://anilist.co/anime/3".into() })
        );
    }
}
