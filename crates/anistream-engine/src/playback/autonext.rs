//! What a playback leads to on its own: the next episode an ending rolls on to, with filler
//! stepped over when the settings ask; an episode picked in the player, or stepped to with the
//! terminal's n and N; the episode restarted in the other translation; and the next episode's
//! streams, found while the ending plays. The engine starts each of these itself, as the event
//! that asks for it passes on its way to the front end, and says what it started with
//! [`Event::PlaybackStaged`].
//!
//! Which title these are of is the one Now Playing shows, which the engine keeps track of as
//! [`Staging`]: the playback staged last, by its number, and the one each staged playback was
//! staged over, which Now Playing goes back to when the newer one finds nothing. Kept by number,
//! from the plays the engine starts and what each playback says of itself, so an ending is known
//! to be the one on screen, or one that the screen has already moved on from, without guessing.
//!
//! The terminal used to work all of this out as the events reached it, and ask for the next play
//! through its one slot for work.

use std::collections::HashMap;

use anistream_core::ids::AnilistId;
use anistream_model::{event::Event, notice::Notice, playback::Ending, progress::after_filler};

use crate::{cache::TitleCache, outbox::Played};

/// A playback Now Playing shows: which one, of what, and what it was staged as.
#[derive(Debug, Clone, PartialEq)]
pub struct Staged {
    /// Its number, as the engine numbered it when it started.
    pub serial: u64,
    pub id: AnilistId,
    pub episode: String,
    /// The name it was staged under, which an episode that follows it goes by too.
    pub title: String,
    pub episode_title: Option<String>,
}

/// What Now Playing shows, as the engine keeps track of it. See the module.
#[derive(Debug, Clone, Default)]
pub struct Staging {
    /// The playback Now Playing is for: the one staged last, or the one it went back to.
    front: Option<Staged>,
    /// Each staged playback still finding its stream, by number, and the one it was staged
    /// over: what Now Playing goes back to when it finds nothing and that one keeps the window.
    over: HashMap<u64, Staged>,
}

impl Staging {
    /// A playback staged for Now Playing, over whatever it showed before.
    pub fn staged(&mut self, staged: Staged) {
        if let Some(before) = self.front.take() {
            self.over.insert(staged.serial, before);
        }
        self.front = Some(staged);
    }

    /// The playback Now Playing is for, if any.
    pub fn front(&self) -> Option<&Staged> {
        self.front.as_ref()
    }

    /// Take in what a playback said of itself, from the playback `played`. Returns the
    /// playback Now Playing showed when this is the ending of that one: Now Playing shows
    /// nothing from then, unless the ending rolls on.
    ///
    /// - An ending, of any other playback, is one the screen has moved on from: one staged
    ///   over, or a party's. It leaves Now Playing as it is, and no longer stands behind the
    ///   playback staged over it.
    /// - A window handed over is the same, and Now Playing goes when it is the one handed
    ///   over: that is a watch party taking it, which is not staged.
    /// - A playback that found nothing and left the window to the one it was staged over puts
    ///   that one back, or passes it on to whatever was staged over this one since.
    pub(crate) fn observe(&mut self, event: &Event, played: Option<&Played>) -> Option<Staged> {
        let serial = played?.serial;
        match event {
            Event::PlaybackEnded { .. } => {
                self.over.remove(&serial);
                self.let_go(serial);
                self.front.take_if(|front| front.serial == serial)
            }
            Event::EpisodeLeft { .. } => {
                self.over.remove(&serial);
                self.let_go(serial);
                if self.front.as_ref().is_some_and(|front| front.serial == serial) {
                    self.front = None;
                }
                None
            }
            // Whatever stood behind the playback that found nothing stands behind whatever was
            // staged over it since, or is on screen again when nothing was.
            Event::PlaybackKept => {
                let kept = self.over.remove(&serial);
                if self.front.as_ref().is_some_and(|front| front.serial == serial) {
                    self.front = kept;
                    return None;
                }
                match kept {
                    Some(kept) => self
                        .over
                        .values_mut()
                        .filter(|behind| behind.serial == serial)
                        .for_each(|behind| *behind = kept.clone()),
                    None => self.let_go(serial),
                }
                None
            }
            _ => None,
        }
    }

    /// `serial` is over: no playback staged over it goes back to it any more.
    fn let_go(&mut self, serial: u64) {
        self.over.retain(|_, behind| behind.serial != serial);
    }
}

/// A play the engine starts on its own, and what it is staged as.
#[derive(Debug, Clone, PartialEq)]
pub struct Follow {
    pub id: AnilistId,
    pub episode: String,
    pub title: String,
    pub episode_title: Option<String>,
}

/// What `event` leads the engine to start, from the playback `finished` Now Playing showed
/// until this ended it, and the playback Now Playing shows otherwise; with what to say about it
/// first, in order, before the play is staged.
///
/// - An ending of the playback on screen that the playback kept its window for — auto-next on,
///   Up Next not cancelled, and a next episode on its list — rolls on to that episode, under
///   the name it played under, or past it while that is filler and filler is skipped. Only the
///   rows the engine holds for that title can say what is filler: see [`after_filler`].
/// - An episode picked in the player is of the title on screen, named as the player's list
///   names it, or as the rows here do.
/// - Sub or dub picked in the player restarts the episode on screen, as it was named. Said
///   whether or not anything is on screen to restart.
///
/// `skip_filler` is the setting in effect.
pub fn follow(
    event: &Event,
    finished: Option<Staged>,
    front: Option<&Staged>,
    cache: &TitleCache,
    skip_filler: bool,
) -> (Vec<Event>, Option<Follow>) {
    let mut said = Vec::new();
    let follow = match event {
        Event::PlaybackEnded { ending: Ending::Complete { next: Some(next) }, .. } => finished
            .map(|finished| {
                let rows = cache.rows(finished.id).unwrap_or_default();
                let episode = after_filler(rows, next.clone(), skip_filler);
                if episode != *next {
                    said.push(Event::Toast(Notice::info(format!(
                        "skipped filler — ep {episode} next"
                    ))));
                }
                Follow {
                    id: finished.id,
                    episode_title: cache.episode_title(finished.id, &episode),
                    episode,
                    title: finished.title,
                }
            }),
        Event::PlayerPlayEpisode { episode, title } => front.map(|front| Follow {
            id: front.id,
            episode: episode.clone(),
            title: front.title.clone(),
            episode_title: title.clone().or_else(|| cache.episode_title(front.id, episode)),
        }),
        Event::PlayerTranslation(translation) => {
            said.push(Event::Toast(Notice::info(format!("now using {translation}"))));
            front.map(|front| Follow {
                id: front.id,
                episode: front.episode.clone(),
                title: front.title.clone(),
                episode_title: front
                    .episode_title
                    .clone()
                    .or_else(|| cache.episode_title(front.id, &front.episode)),
            })
        }
        _ => None,
    };
    (said, follow)
}

#[cfg(test)]
mod tests {
    use super::*;
    use anistream_model::episodes::EpisodeRow;

    /// The record holding title `id`'s twelve-episode table, each episode named, with the
    /// episodes in `filler` marked as pure filler.
    fn with_table(id: u32, filler: &[u32]) -> TitleCache {
        let rows = (1..=12)
            .map(|n| EpisodeRow {
                number: n.to_string(),
                title: Some(format!("Episode {n}")),
                thumbnail: None,
                description: None,
                duration_secs: Some(1_435),
                watched: 0.0,
                completed: false,
                kind: None,
                skippable: filler.contains(&n),
            })
            .collect();
        let mut cache = TitleCache::default();
        cache.observe(&Event::Episodes { id: AnilistId::new(id), rows }, None);
        cache
    }

    /// Episode `episode` of title `id` on screen, as playback 1, under `title`.
    fn on_screen(id: u32, title: &str, episode: &str) -> Staged {
        Staged {
            serial: 1,
            id: AnilistId::new(id),
            episode: episode.into(),
            title: title.into(),
            episode_title: None,
        }
    }

    fn complete(next: Option<&str>) -> Event {
        Event::playback_ended(true, Ending::Complete { next: next.map(Into::into) })
    }

    /// What `event` leads to when it ends `finished`, the playback on screen.
    fn ending(event: &Event, finished: Staged, cache: &TitleCache) -> Option<Follow> {
        follow(event, Some(finished), None, cache, false).1
    }

    #[test]
    fn finishing_an_episode_queues_the_next_one() {
        let cache = with_table(1, &[]);
        let next = ending(&complete(Some("2")), on_screen(1, "Title 0", "1"), &cache);
        assert_eq!(
            next.map(|next| (next.id, next.episode)),
            Some((AnilistId::new(1), "2".into())),
            "auto-next did not follow a finished episode"
        );
    }

    #[test]
    fn closing_or_stopping_past_the_threshold_does_not_roll_on() {
        // Past the commit threshold the episode counts as watched, and auto-next used to read
        // that as finished: closing the window during the credits opened a new one on the next
        // episode. Only an episode mpv played to its end rolls on.
        let cache = with_table(1, &[]);
        for end in [Ending::Closed, Ending::Stopped] {
            let event = Event::playback_ended(true, end.clone());
            let next = ending(&event, on_screen(1, "Title 0", "1"), &cache);
            assert_eq!(next, None, "{end:?} rolled on");
        }
    }

    #[test]
    fn an_ending_short_of_the_end_starts_nothing() {
        // Quitting partway, a stream that failed, a party or the browser taking it.
        let cache = with_table(1, &[]);
        for end in [Ending::Closed, Ending::Stopped, Ending::Failed, Ending::Elsewhere] {
            let event = Event::playback_ended(false, end.clone());
            assert_eq!(ending(&event, on_screen(1, "Title 0", "1"), &cache), None, "{end:?}");
        }
    }

    #[test]
    fn nothing_follows_an_ending_the_player_kept_no_window_for() {
        // The finale, auto-next turned off, or Up Next cancelled: the playback names no next
        // episode, and nothing is searched for. Guessing "the number after" here searched for
        // an episode the list said did not exist, after the finale.
        let cache = with_table(1, &[]);
        let (said, next) =
            follow(&complete(None), Some(on_screen(1, "Title 0", "12")), None, &cache, true);
        assert_eq!((said.len(), next), (0, None));
    }

    #[test]
    fn auto_next_continues_the_show_that_played_under_its_own_name() {
        // Whatever is open by the time the episode ends, the next episode is the one of the
        // show that played, named as it was, with its own table's episode title.
        let cache = with_table(1, &[]);
        let next = ending(&complete(Some("2")), on_screen(1, "Title 0", "1"), &cache);
        let expected = Follow {
            id: AnilistId::new(1),
            episode: "2".into(),
            title: "Title 0".into(),
            episode_title: Some("Episode 2".into()),
        };
        assert_eq!(next, Some(expected));
    }

    #[test]
    fn auto_next_steps_over_filler_when_asked() {
        let cache = with_table(1, &[2, 3]);
        let finished = || Some(on_screen(1, "Title 0", "1"));
        let (said, next) = follow(&complete(Some("2")), finished(), None, &cache, true);
        assert_eq!(next.map(|next| next.episode).as_deref(), Some("4"), "eps 2-3 are filler");
        let [Event::Toast(notice)] = &said[..] else { panic!("said why: {said:?}") };
        assert_eq!(notice.text, "skipped filler — ep 4 next");
        // Not asked: filler plays like any other episode, and nothing is said.
        let (said, next) = follow(&complete(Some("2")), finished(), None, &cache, false);
        assert_eq!((said.len(), next.map(|next| next.episode)), (0, Some("2".into())));
    }

    #[test]
    fn a_cancel_does_not_carry_over_to_the_next_episode() {
        // Up Next was cancelled on episode 1, then episode 2 was started from the player. Its
        // own ending rolls on: whether it does is the playback's to say, per episode, and
        // nothing of episode 1's choice is held here to overrule it.
        let cache = with_table(1, &[]);
        let mut staging = Staging::default();
        staging.staged(on_screen(1, "Title 0", "1"));
        staging.staged(Staged { serial: 2, ..on_screen(1, "Title 0", "2") });
        let left = Event::EpisodeLeft {
            id: AnilistId::new(1),
            episode: "1".into(),
            position: 300.0,
            duration: Some(1_435.0),
            watched: false,
        };
        staging.observe(&left, Some(&from_title(1, 1, "1")));
        let ended = complete(Some("3"));
        let finished = staging.observe(&ended, Some(&from_title(2, 1, "2")));
        let (_, next) = follow(&ended, finished, staging.front(), &cache, false);
        assert_eq!(next.map(|next| next.episode).as_deref(), Some("3"));
    }

    #[test]
    fn an_episode_the_player_asks_for_is_of_the_show_playing() {
        // Title 0's table is the one held; Title 1 is the one playing.
        let cache = with_table(1, &[]);
        let playing = on_screen(2, "Title 1", "4");
        let picked =
            Event::PlayerPlayEpisode { episode: "5".into(), title: Some("Fifth".into()) };
        let (_, next) = follow(&picked, None, Some(&playing), &cache, false);
        let expected = Follow {
            id: AnilistId::new(2),
            episode: "5".into(),
            title: "Title 1".into(),
            episode_title: Some("Fifth".into()),
        };
        assert_eq!(next, Some(expected));

        // Named by nobody, it is left unnamed rather than given the held show's episode 6.
        let picked = Event::PlayerPlayEpisode { episode: "6".into(), title: None };
        let (_, next) = follow(&picked, None, Some(&playing), &cache, false);
        assert_eq!(next.map(|next| next.episode_title), Some(None));
    }

    #[test]
    fn sub_or_dub_picked_in_the_player_restarts_the_episode_on_screen() {
        use anistream_core::media::Translation;
        let cache = with_table(1, &[]);
        let pick = Event::PlayerTranslation(Translation::Dub);
        // As it was staged, its episode named as it was; and by the table when it was not.
        let named =
            Staged { episode_title: Some("The fourth".into()), ..on_screen(1, "Title 0", "4") };
        let (said, next) = follow(&pick, None, Some(&named), &cache, false);
        let [Event::Toast(notice)] = &said[..] else { panic!("said: {said:?}") };
        assert_eq!(notice.text, "now using dub");
        let next = next.expect("restarted");
        assert_eq!((next.episode.as_str(), next.title.as_str()), ("4", "Title 0"));
        assert_eq!(next.episode_title.as_deref(), Some("The fourth"));
        let (_, next) = follow(&pick, None, Some(&on_screen(1, "Title 0", "4")), &cache, false);
        assert_eq!(next.and_then(|next| next.episode_title).as_deref(), Some("Episode 4"));
    }

    #[test]
    fn with_nothing_on_screen_nothing_is_picked_or_restarted() {
        use anistream_core::media::Translation;
        let cache = with_table(1, &[]);
        let picked = Event::PlayerPlayEpisode { episode: "5".into(), title: None };
        assert_eq!(follow(&picked, None, None, &cache, false).1, None);
        // The choice is in effect all the same, and said.
        let (said, next) =
            follow(&Event::PlayerTranslation(Translation::Sub), None, None, &cache, false);
        assert_eq!((said.len(), next), (1, None));
    }

    /// Playback `serial`, of episode `episode` of title `id`.
    fn from_title(serial: u64, id: u32, episode: &str) -> Played {
        Played { serial, id: AnilistId::new(id), episode: episode.into() }
    }

    fn staged(serial: u64, episode: &str) -> Staged {
        Staged {
            serial,
            id: AnilistId::new(7),
            episode: episode.into(),
            title: "Hoshikuzu no Tabibito".into(),
            episode_title: None,
        }
    }

    fn from(serial: u64, episode: &str) -> Played {
        Played { serial, id: AnilistId::new(7), episode: episode.into() }
    }

    fn ended() -> Event {
        Event::playback_ended(true, Ending::Complete { next: Some("2".into()) })
    }

    #[test]
    fn only_the_ending_of_the_playback_on_screen_takes_it_down() {
        let mut staging = Staging::default();
        staging.staged(staged(1, "1"));
        staging.staged(staged(2, "2"));
        // Episode 1 ends while 2, staged over it, is found: the screen has moved on.
        assert_eq!(staging.observe(&ended(), Some(&from(1, "1"))), None);
        assert_eq!(staging.front().map(|f| f.serial), Some(2));
        // A stray with no playback named says nothing of the screen.
        assert_eq!(staging.observe(&ended(), None), None);
        assert_eq!(staging.observe(&ended(), Some(&from(2, "2"))), Some(staged(2, "2")));
        assert_eq!(staging.front(), None);
    }

    #[test]
    fn a_playback_that_found_nothing_gives_the_screen_back() {
        let mut staging = Staging::default();
        staging.staged(staged(1, "4"));
        staging.staged(staged(2, "9"));
        staging.observe(&Event::PlaybackKept, Some(&from(2, "9")));
        assert_eq!(staging.front(), Some(&staged(1, "4")));

        // Found this time: 4 hands the window over, and nothing goes back to it after.
        staging.staged(staged(3, "5"));
        let left = Event::EpisodeLeft {
            id: AnilistId::new(7),
            episode: "4".into(),
            position: 300.0,
            duration: None,
            watched: false,
        };
        staging.observe(&left, Some(&from(1, "4")));
        staging.observe(&Event::PlaybackKept, Some(&from(3, "5")));
        assert_eq!(staging.front(), None, "4 left the window: there is nothing to go back to");
    }

    #[test]
    fn a_window_handed_to_a_party_takes_the_screen_down() {
        // A party is not staged: the playback on screen hands its window over to it.
        let mut staging = Staging::default();
        staging.staged(staged(1, "4"));
        let left = Event::EpisodeLeft {
            id: AnilistId::new(7),
            episode: "4".into(),
            position: 300.0,
            duration: None,
            watched: false,
        };
        staging.observe(&left, Some(&from(1, "4")));
        assert_eq!(staging.front(), None);
    }
}
