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
