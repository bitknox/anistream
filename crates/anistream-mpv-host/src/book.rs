//! Which playback the core's file and reports belong to, and the controls' state: the hosted
//! player's bookkeeping, apart from libmpv so it is tested on any machine.

use std::collections::{HashSet, VecDeque};

use anistream_model::playback::PlaybackId;

use crate::ops;

/// What the player's controls draw from: the playhead and the state around it.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PlayerState {
    pub position: Option<f64>,
    pub duration: Option<f64>,
    pub paused: bool,
    /// Held waiting for data: `paused-for-cache`.
    pub buffering: bool,
    pub seeking: bool,
    /// The file on screen has shown its first frame.
    pub has_picture: bool,
    pub frame_drops: i64,
    /// The file's audio and subtitle tracks, from `track-list`.
    pub tracks: Vec<ops::PlayerTrack>,
}

/// Which playback the core's file and reports belong to.
#[derive(Default)]
pub(crate) struct Book {
    /// The playback that opened the player last.
    pub opened: Option<PlaybackId>,
    /// The playback whose file mpv has, from its `start-file`.
    pub current: Option<PlaybackId>,
    /// Loads sent and not yet started, oldest first.
    loads: VecDeque<Load>,
    /// A `start-file` read before its `loadfile` answered with the entry it names.
    started_early: Option<i64>,
    /// Every playback through this one is over for the player.
    pub closed_through: Option<PlaybackId>,
    /// The viewer closed the player: nothing is taken until the engine stages again.
    pub closed_by_viewer: bool,
    /// The playbacks told `Closed` already, so it is said once.
    pub closed_said: HashSet<PlaybackId>,
    pub state: PlayerState,
}

/// A load on its way to mpv, and the playlist entry mpv made for it once it answers.
struct Load {
    playback: PlaybackId,
    entry: Option<i64>,
}

impl Book {
    pub fn accepts(&self, playback: PlaybackId) -> bool {
        !self.closed_by_viewer && self.closed_through.is_none_or(|through| playback > through)
    }

    /// Who mpv's reports are for: the file's playback, or the one that opened the player.
    pub fn target(&self) -> Option<PlaybackId> {
        self.current.or(self.opened)
    }

    /// A load for `playback` is on its way to mpv: what is on screen is not its picture.
    pub fn loading(&mut self, playback: PlaybackId) {
        self.loads.push_back(Load { playback, entry: None });
        self.state.has_picture = false;
    }

    /// mpv took `playback`'s load as playlist `entry`, or refused it (`None`), when it will
    /// never start.
    pub fn loaded_as(&mut self, playback: PlaybackId, entry: Option<i64>) {
        let Some(at) = self
            .loads
            .iter()
            .position(|load| load.playback == playback && load.entry.is_none())
        else {
            // The player closed while it was on its way.
            return;
        };
        match entry {
            Some(entry) => {
                self.loads[at].entry = Some(entry);
                if self.started_early == Some(entry) {
                    self.started(entry);
                }
            }
            None => {
                self.loads.remove(at);
            }
        }
    }

    /// mpv's `start-file` for playlist `entry`. Every load replaces the playlist, and mpv
    /// starts only the newest of the loads it has taken by the time it gets to the next file:
    /// the ones before the file that starts never will.
    pub fn started(&mut self, entry: i64) {
        match self.loads.iter().position(|load| load.entry == Some(entry)) {
            Some(at) => {
                self.current = Some(self.loads[at].playback);
                self.loads.drain(..=at);
                self.started_early = None;
            }
            // Its `loadfile` has not answered yet; it is matched when it does. A file stopped
            // as the player closed is matched by nothing.
            None => self.started_early = Some(entry),
        }
    }

    /// mpv's `playback-restart`: the controls' new state when it is the file's first frame.
    pub fn restarted(&mut self) -> Option<PlayerState> {
        // A frame of the file being replaced, while the next loads, is not the next's.
        let first = !self.state.has_picture && self.loads.is_empty();
        if first {
            self.state.has_picture = true;
        }
        first.then(|| self.state.clone())
    }

    /// The player closed: every playback it has heard of is over for it.
    pub fn close(&mut self) {
        let newest = self.loads.back().map(|load| load.playback);
        let through = [self.current, self.opened, newest, self.closed_through]
            .into_iter()
            .flatten()
            .max();
        self.closed_through = through;
        self.loads.clear();
        self.started_early = None;
        self.current = None;
        self.state = PlayerState::default();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(n: u64) -> PlaybackId {
        PlaybackId::new(n)
    }

    /// N pressed six times fast: each episode's load replaces the one before, and mpv, taking
    /// them back to back, starts only the last. Its first frame is the picture the starting card
    /// waits for, and the file and its reports are the last playback's.
    #[test]
    fn the_last_of_several_quick_loads_shows_its_picture() {
        let mut book = Book::default();
        for n in 1..=6 {
            book.loading(id(n));
            book.loaded_as(id(n), Some(n as i64 + 10));
        }
        book.started(16);
        assert_eq!(book.current, Some(id(6)), "the file is the last load's");
        let state = book.restarted().expect("the first frame is said");
        assert!(state.has_picture);
    }

    /// Loads spaced out enough that mpv starts each in turn, the first ones cut off while they
    /// open: every start moves the file on, and only the last one's frame is the picture.
    #[test]
    fn a_start_for_each_load_moves_the_file_along() {
        let mut book = Book::default();
        book.loading(id(1));
        book.loaded_as(id(1), Some(1));
        book.started(1);
        book.loading(id(2));
        book.loaded_as(id(2), Some(2));
        assert_eq!(book.restarted(), None, "a frame of 1 while 2 loads is not 2's");
        book.started(2);
        assert_eq!(book.current, Some(id(2)));
        assert!(book.restarted().is_some_and(|state| state.has_picture));
    }

    /// The event thread can read the `start-file` before the worker has mpv's answer to the
    /// `loadfile`, which names the entry: the start waits for it.
    #[test]
    fn a_start_read_before_its_load_answered_is_matched_when_it_does() {
        let mut book = Book::default();
        book.loading(id(1));
        book.loading(id(2));
        book.loaded_as(id(1), Some(1));
        book.started(2);
        assert_eq!(book.current, None);
        book.loaded_as(id(2), Some(2));
        assert_eq!(book.current, Some(id(2)));
        assert!(book.restarted().is_some_and(|state| state.has_picture));
    }

    /// A load mpv refuses never starts, so it holds back no one's picture.
    #[test]
    fn a_refused_load_is_forgotten() {
        let mut book = Book::default();
        book.loading(id(1));
        book.loaded_as(id(1), Some(1));
        book.started(1);
        book.loading(id(2));
        book.loaded_as(id(2), None);
        assert!(book.restarted().is_some_and(|state| state.has_picture));
        assert_eq!(book.current, Some(id(1)));
    }

    /// Closing forgets every load, and a file that starts as it stops belongs to nobody.
    #[test]
    fn after_a_close_a_late_start_belongs_to_nobody() {
        let mut book = Book { opened: Some(id(3)), ..Book::default() };
        book.loading(id(3));
        book.close();
        assert_eq!(book.closed_through, Some(id(3)));
        book.loaded_as(id(3), Some(7));
        book.started(7);
        assert_eq!(book.current, None);
        book.loading(id(4));
        book.loaded_as(id(4), Some(8));
        book.started(8);
        assert_eq!(book.current, Some(id(4)));
    }
}
