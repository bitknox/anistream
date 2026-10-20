//! The engine's record of the titles a front end shows: what each is called and how far through
//! it the viewer is, and the episode table loaded last, kept current by what the engine itself
//! does to them.
//!
//! Fed from the one path every event takes to the front end: a detail fetch or a list names a
//! title and says its progress; a loaded table replaces the one before it. The engine's own
//! writes and its playbacks' verdicts then move it on, by the rules in `model::progress`: a
//! mark, the playhead of an episode playing, and how that episode ended. From it the engine
//! answers what a play needs — the name the window being handed over announces, the Up Next
//! card and the player's Episodes menu — and says a title's progress whole, keyed by the title.
//!
//! One table, not one per title. The terminal keeps the table it loaded last and nothing of the
//! tables before it, so a title whose table has since been replaced plays as one with no table:
//! its episodes are loaded behind the playback. Answering from an older table here would be a
//! change the viewer could see — the Episodes menu there at once, Up Next stepping over filler
//! it did not step over, Continue skipping an episode it did not skip — so the engine holds the
//! table as the terminal does.

use std::collections::HashMap;

use anistream_core::ids::AnilistId;
use anistream_model::{
    catalog::{Content, Entry, ResumePoint},
    episodes::EpisodeRow,
    event::Event,
    playback::{EpisodeEntry, NextEpisode},
    progress::{self, Watch},
};

use crate::outbox::Played;

/// How many titles the record keeps, the least recently touched let go first.
///
/// A title is a name and a few numbers, so a thousand of them is a few hundred kilobytes. It is
/// the lists a session browses between seeing a title and playing it from where it was seen:
/// a list is a page of fifty, a calendar a fortnight of broadcasts, a long Library segment a few
/// hundred. A title let go is one the engine says nothing of after a watch, where the terminal
/// would still move it on its rows.
const TITLES: usize = 1024;

/// What a play is: which kind of command started it, for the name it is announced by.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayKind {
    /// `Play` or `PlaySource`: staged on Now Playing, by the name the title goes by here.
    Staged,
    /// `PlayParty`: Syncplay owns the player, and nothing is staged.
    Party,
}

/// What a play takes from the record: the name the window being handed over announces it by,
/// the episode after it for the Up Next card, and the title's episodes for the Episodes menu.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PlayHints {
    pub title: String,
    pub next: Option<NextEpisode>,
    pub episodes: Vec<EpisodeEntry>,
}

/// The engine's record of the titles a front end shows. See the module.
#[derive(Debug, Clone, Default)]
pub struct TitleCache {
    titles: HashMap<AnilistId, Title>,
    /// Counts touches, so the title touched least recently is the one let go.
    clock: u64,
    /// The episode table loaded last, and whose it is.
    table: Option<(AnilistId, Vec<EpisodeRow>)>,
    /// The title the last staged playback was announced as, and whose.
    announced: Option<(AnilistId, String)>,
    /// Each playback started, by its number: where it last said it was.
    playbacks: HashMap<u64, Playhead>,
}

/// One title, as the record holds it.
#[derive(Debug, Clone)]
struct Title {
    /// What it is called, as its page and its list rows name it.
    name: String,
    /// Episodes done, and the one Continue offers.
    progress: Option<(u32, u32)>,
    /// Where Continue resumes that episode.
    resume: Option<ResumePoint>,
    touched: u64,
}

/// What a playback last said of itself.
#[derive(Debug, Clone, Default)]
struct Playhead {
    position: f64,
    duration: Option<f64>,
    /// It has said how it ended, and what it says after is a straggler.
    ended: bool,
}

impl TitleCache {
    /// Take in what an event says of a title, and move the record on by it. Returns the title
    /// whose progress it moved, which the front end is then told whole, ahead of the event.
    ///
    /// `played` is the playback the event came from, when a playback said it.
    pub(crate) fn observe(
        &mut self,
        event: &Event,
        played: Option<&Played>,
    ) -> Option<AnilistId> {
        match event {
            Event::Detail(entry) => {
                self.know(entry);
                // Opened from the page, a relation or a recommendation shows by the name the
                // page lists it under until its own record arrives.
                for related in entry.related.iter().chain(&entry.recommended) {
                    self.name(related.id, &related.title);
                }
                None
            }
            Event::Content { content: Content::Entries(entries), .. }
            | Event::CalendarBackfill { entries, .. } => {
                entries.iter().for_each(|entry| self.know(entry));
                None
            }
            Event::Episodes { id, rows } => {
                self.table = Some((*id, rows.clone()));
                None
            }
            Event::Playback { position, duration, .. } => {
                let played = played?;
                let playhead = self.playing(played)?;
                playhead.position = *position;
                if duration.is_some() {
                    playhead.duration = *duration;
                }
                // Keeps the table honest while detached: `q` leaves mpv running and drops the
                // viewer back on the episode list, which would otherwise sit frozen at the
                // position they started from.
                let watch = Watch {
                    episode: played.episode.clone(),
                    position: *position,
                    duration: playhead.duration,
                    completed: false,
                };
                self.watch(played.id, &watch)
            }
            Event::Resumed { position } => {
                if let Some(playhead) = played.and_then(|played| self.playing(played)) {
                    playhead.position = *position;
                }
                None
            }
            // The window changed hands mid-episode: a watch like an ending, by what the
            // playback says of where it was.
            Event::EpisodeLeft { id, episode, position, duration, watched } => {
                if let Some(playhead) = played.and_then(|played| self.playing(played)) {
                    playhead.ended = true;
                }
                let watch = Watch {
                    episode: episode.clone(),
                    position: *position,
                    duration: *duration,
                    completed: *watched,
                };
                self.watch(*id, &watch)
            }
            Event::PlaybackEnded { watched, .. } => {
                let played = played?;
                let playhead = self.playing(played)?;
                playhead.ended = true;
                let watch = Watch {
                    episode: played.episode.clone(),
                    position: playhead.position,
                    duration: playhead.duration,
                    completed: *watched,
                };
                self.watch(played.id, &watch)
            }
            _ => None,
        }
    }

    /// A playback is starting: what it says is taken in from now until it says it ended.
    pub(crate) fn started(&mut self, played: &Played) {
        self.playbacks.insert(played.serial, Playhead::default());
    }

    /// Episodes marked watched or not, once written: `held` says whether the store holds an
    /// episode watched now. A mark the store holds is reflected as the terminal reflected it, a
    /// finished watch with no position; an unmark it holds, by the unmark's rule. One the store
    /// did not take is not shown as taken. Returns the title, when the record knows it.
    pub fn marked(
        &mut self,
        id: AnilistId,
        episodes: &[String],
        watched: bool,
        held: impl Fn(&str) -> bool,
    ) -> Option<AnilistId> {
        let mut moved = false;
        for episode in episodes.iter().filter(|episode| held(episode) == watched) {
            if watched {
                let watch = Watch {
                    episode: episode.clone(),
                    position: 0.0,
                    duration: None,
                    completed: true,
                };
                moved |= self.watch(id, &watch).is_some();
            } else {
                moved |= self.unmark(id, episode);
            }
        }
        moved.then_some(id)
    }

    /// What a play of `episode` of `id` takes from the record. Not a playback of one's own
    /// table: the rows are the table loaded last, when it is this title's, and otherwise none,
    /// which the playback loads behind itself.
    pub fn hints(
        &self,
        id: AnilistId,
        episode: &str,
        skip_filler: bool,
        kind: PlayKind,
    ) -> PlayHints {
        let rows = self.rows(id).unwrap_or_default();
        let next = progress::next_up(rows, episode, skip_filler).map(|row| NextEpisode {
            number: row.number.clone(),
            title: row.title.clone(),
            thumbnail: row.thumbnail.clone(),
        });
        let episodes = rows
            .iter()
            .map(|row| EpisodeEntry {
                number: row.number.clone(),
                title: row.title.clone(),
                watched: row.completed,
                filler: row.skippable,
            })
            .collect();
        PlayHints { title: self.announce_as(id, kind), next, episodes }
    }

    /// The name a play of `id` is announced by: the one its last playback went by, so an
    /// episode that follows another — rolled on to, picked in the player, restarted in the
    /// other translation — goes by the name the one before did; else what the title is called
    /// here. A playback with no name to go by is staged as just that, playing; a watch party
    /// stages nothing, and has none.
    fn announce_as(&self, id: AnilistId, kind: PlayKind) -> String {
        if let Some((_, title)) =
            self.announced.as_ref().filter(|(announced, _)| *announced == id)
        {
            return title.clone();
        }
        match (self.titles.get(&id), kind) {
            (Some(title), _) => title.name.clone(),
            (None, PlayKind::Staged) => "playing".into(),
            (None, PlayKind::Party) => String::new(),
        }
    }

    /// A staged playback of `id` went by `title`.
    pub fn announce(&mut self, id: AnilistId, title: String) {
        self.announced = Some((id, title));
    }

    /// Where a title stands, as the front end is told it: episodes done and the one Continue
    /// offers, where Continue resumes it, and its episodes' watched state when its table is the
    /// one loaded last.
    pub fn standing(&self, id: AnilistId) -> Option<Standing<'_>> {
        let title = self.titles.get(&id);
        let rows = self.rows(id);
        (title.is_some() || rows.is_some()).then(|| Standing {
            progress: title.and_then(|title| title.progress),
            resume: title.and_then(|title| title.resume),
            rows: rows.unwrap_or_default(),
        })
    }

    /// The table loaded last, and whose it is.
    pub fn table(&self) -> Option<(AnilistId, &[EpisodeRow])> {
        self.table.as_ref().map(|(id, rows)| (*id, rows.as_slice()))
    }

    /// The rows of the table loaded last, when it is this title's.
    fn rows(&self, id: AnilistId) -> Option<&[EpisodeRow]> {
        self.table.as_ref().filter(|(table, _)| *table == id).map(|(_, rows)| rows.as_slice())
    }

    /// A playback under way: started, and not yet ended.
    fn playing(&mut self, played: &Played) -> Option<&mut Playhead> {
        self.playbacks.get_mut(&played.serial).filter(|playhead| !playhead.ended)
    }

    /// A watch on a title: its entry first, read against its table as the table stood, then the
    /// table. Returns the title when the record holds either.
    fn watch(&mut self, id: AnilistId, watch: &Watch) -> Option<AnilistId> {
        let touched = self.touch();
        let rows = self.table.as_mut().filter(|(table, _)| *table == id).map(|(_, rows)| rows);
        let title = self.titles.get_mut(&id);
        if title.is_none() && rows.is_none() {
            return None;
        }
        if let Some(title) = title {
            title.touched = touched;
            title.with_entry(id, |entry| {
                progress::watch_entry(entry, rows.as_deref().map(Vec::as_slice), watch)
            });
        }
        if let Some(rows) = rows {
            progress::watch_row(rows, watch);
        }
        Some(id)
    }

    /// An episode marked not watched: its row emptied, and when it had been watched, the
    /// title's count and Continue put back by the unmark's rule. Whether the record held it.
    fn unmark(&mut self, id: AnilistId, episode: &str) -> bool {
        let touched = self.touch();
        let Some((_, rows)) = self.table.as_mut().filter(|(table, _)| *table == id) else {
            return self.titles.contains_key(&id);
        };
        let was_done = rows.iter().any(|row| row.number == episode && row.completed);
        progress::mark_row(rows, episode, false);
        if was_done && let Some(title) = self.titles.get_mut(&id) {
            title.touched = touched;
            title.with_entry(id, |entry| progress::unmark_entry(entry, rows, episode));
        }
        true
    }

    /// Hold what a detail fetch or a list row says of a title, in place of what was held.
    fn know(&mut self, entry: &Entry) {
        let touched = self.touch();
        self.titles.insert(
            entry.id,
            Title {
                name: entry.title.clone(),
                progress: entry.progress,
                resume: entry.resume,
                touched,
            },
        );
        self.let_go();
    }

    /// Hold a title's name alone, when nothing more is known of it.
    fn name(&mut self, id: AnilistId, name: &str) {
        if self.titles.contains_key(&id) {
            return;
        }
        let touched = self.touch();
        let title = Title { name: name.to_owned(), progress: None, resume: None, touched };
        self.titles.insert(id, title);
        self.let_go();
    }

    fn touch(&mut self) -> u64 {
        self.clock += 1;
        self.clock
    }

    /// Past the bound, let the titles touched least recently go.
    fn let_go(&mut self) {
        while self.titles.len() > TITLES {
            let Some(oldest) =
                self.titles.iter().min_by_key(|(_, t)| t.touched).map(|(id, _)| *id)
            else {
                return;
            };
            self.titles.remove(&oldest);
        }
    }
}

impl Title {
    /// Work on this title's progress as an entry, which is what the rules are written over.
    fn with_entry(&mut self, id: AnilistId, work: impl FnOnce(&mut Entry)) {
        let mut entry =
            Entry { progress: self.progress, resume: self.resume, ..Entry::new(id, "") };
        work(&mut entry);
        (self.progress, self.resume) = (entry.progress, entry.resume);
    }
}

/// Where a title stands, as [`TitleCache::standing`] says it.
#[derive(Debug, Clone, PartialEq)]
pub struct Standing<'a> {
    pub progress: Option<(u32, u32)>,
    pub resume: Option<ResumePoint>,
    /// Its table's rows, when its table is the one loaded last; otherwise none.
    pub rows: &'a [EpisodeRow],
}
