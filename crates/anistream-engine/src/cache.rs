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
    progress::{self, EpisodeProgress, TitleProgress, Watch},
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
    /// Each playback started, by its number: where it last said it was. Kept once it has
    /// ended, a few bytes each, so that what it says after is known for a straggler.
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
    /// whose progress it moved, when it moved, which the front end is then told whole, ahead of
    /// the event: see [`Self::progress`].
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
    /// did not take is not shown as taken.
    ///
    /// Returns the title whenever the record knows it, moved or not: a mark is answered, so a
    /// front end that waits for the answer before it draws again is never left waiting.
    pub fn marked(
        &mut self,
        id: AnilistId,
        episodes: &[String],
        watched: bool,
        held: impl Fn(&str) -> bool,
    ) -> Option<AnilistId> {
        for episode in episodes.iter().filter(|episode| held(episode) == watched) {
            if watched {
                let watch = Watch {
                    episode: episode.clone(),
                    position: 0.0,
                    duration: None,
                    completed: true,
                };
                self.watch(id, &watch);
            } else {
                self.unmark(id, episode);
            }
        }
        self.titles.contains_key(&id).then_some(id)
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
        PlayHints { title: self.name_for(id, kind), next, episodes }
    }

    /// The name a play of `id` is announced by: the one its last playback went by, so an
    /// episode that follows another — rolled on to, picked in the player, restarted in the
    /// other translation — goes by the name the one before did; else what the title is called
    /// here. A playback with no name to go by is staged as just that, playing; a watch party
    /// stages nothing, and has none.
    pub(crate) fn name_for(&self, id: AnilistId, kind: PlayKind) -> String {
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

    /// Where a title stands, whole, as the front end is told it: episodes done and the one
    /// Continue offers, where Continue resumes it, and its episodes' watched state when its
    /// table is the one loaded last. `None` for a title the record does not know, of which it
    /// has nothing to say.
    pub fn progress(&self, id: AnilistId) -> Option<Event> {
        let title = self.titles.get(&id)?;
        let episodes = self
            .rows(id)
            .unwrap_or_default()
            .iter()
            .map(|row| EpisodeProgress {
                number: row.number.clone(),
                watched: row.watched,
                completed: row.completed,
            })
            .collect();
        Some(Event::Progress {
            id,
            progress: title.progress.map(|(done, next)| TitleProgress { done, next }),
            resume: title.resume,
            episodes,
        })
    }

    /// The table loaded last, and whose it is.
    pub fn table(&self) -> Option<(AnilistId, &[EpisodeRow])> {
        self.table.as_ref().map(|(id, rows)| (*id, rows.as_slice()))
    }

    /// The rows of the table loaded last, when it is this title's.
    pub(crate) fn rows(&self, id: AnilistId) -> Option<&[EpisodeRow]> {
        self.table.as_ref().filter(|(table, _)| *table == id).map(|(_, rows)| rows.as_slice())
    }

    /// An episode's own name, from the table loaded last when it is this title's: what Now
    /// Playing shows under the title. `None` for any other title's, rather than another
    /// show's episode name.
    pub fn episode_title(&self, id: AnilistId, episode: &str) -> Option<String> {
        let row = self.rows(id)?.iter().find(|row| row.number == episode)?;
        row.title.clone()
    }

    /// Whether playback `serial` has started playing and not ended: it has said how long its
    /// episode is, which it does from its first frames, and not yet how it ended.
    pub(crate) fn under_way(&self, serial: u64) -> bool {
        self.playbacks
            .get(&serial)
            .is_some_and(|playhead| playhead.duration.is_some() && !playhead.ended)
    }

    /// A playback under way: started, and not yet ended.
    fn playing(&mut self, played: &Played) -> Option<&mut Playhead> {
        self.playbacks.get_mut(&played.serial).filter(|playhead| !playhead.ended)
    }

    /// A watch on a title: its entry first, read against its table as the table stood, then the
    /// table. Returns the title when that moved it.
    fn watch(&mut self, id: AnilistId, watch: &Watch) -> Option<AnilistId> {
        let touched = self.touch();
        let rows = self.table.as_mut().filter(|(table, _)| *table == id).map(|(_, rows)| rows);
        let row_state = |rows: &Option<&mut Vec<EpisodeRow>>| {
            let row = rows.as_ref()?.iter().find(|row| row.number == watch.episode)?;
            Some((row.watched, row.completed))
        };
        let row_before = row_state(&rows);
        let mut moved = false;
        if let Some(title) = self.titles.get_mut(&id) {
            title.touched = touched;
            let before = (title.progress, title.resume);
            title.with_entry(id, |entry| {
                progress::watch_entry(entry, rows.as_deref().map(Vec::as_slice), watch)
            });
            moved |= before != (title.progress, title.resume);
        }
        let mut rows = rows;
        if let Some(rows) = rows.as_deref_mut() {
            progress::watch_row(rows, watch);
        }
        moved |= row_before != row_state(&rows);
        moved.then_some(id)
    }

    /// An episode marked not watched: its row emptied, and when it had been watched, the
    /// title's count and Continue put back by the unmark's rule.
    fn unmark(&mut self, id: AnilistId, episode: &str) {
        let touched = self.touch();
        let Some((_, rows)) = self.table.as_mut().filter(|(table, _)| *table == id) else {
            return;
        };
        let was_done = rows.iter().any(|row| row.number == episode && row.completed);
        progress::mark_row(rows, episode, false);
        if was_done && let Some(title) = self.titles.get_mut(&id) {
            title.touched = touched;
            title.with_entry(id, |entry| progress::unmark_entry(entry, rows, episode));
        }
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

    /// Past the bound, let the titles touched least recently go: never the one whose table is
    /// held, which is the one being watched from.
    fn let_go(&mut self) {
        let held = self.table.as_ref().map(|(id, _)| *id);
        while self.titles.len() > TITLES {
            let Some(oldest) = self
                .titles
                .iter()
                .filter(|(id, _)| Some(**id) != held)
                .min_by_key(|(_, t)| t.touched)
                .map(|(id, _)| *id)
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

#[cfg(test)]
mod tests {
    use super::*;
    use anistream_model::catalog::ContentFor;

    fn row(number: &str) -> EpisodeRow {
        EpisodeRow {
            number: number.into(),
            title: Some(format!("Part {number}")),
            thumbnail: None,
            description: None,
            duration_secs: Some(1_440),
            watched: 0.0,
            completed: false,
            kind: None,
            skippable: false,
        }
    }

    fn rows(numbers: std::ops::RangeInclusive<u32>) -> Vec<EpisodeRow> {
        numbers.map(|n| row(&n.to_string())).collect()
    }

    fn listed(cache: &mut TitleCache, entries: Vec<Entry>) {
        let content = Content::Entries(entries);
        cache.observe(&Event::Content { view: ContentFor::Continue, content }, None);
    }

    /// Moved from the binary's seams, which worked this out from the screen: a play takes what
    /// the record knows of its title.
    #[test]
    fn a_play_takes_what_the_record_knows_of_its_title() {
        let id = AnilistId::new(7);
        let mut cache = TitleCache::default();
        // Nothing known of it: staged as just playing, a party with no name, and a list the
        // playback loads itself.
        let nothing = cache.hints(id, "2", false, PlayKind::Staged);
        assert_eq!(nothing, PlayHints { title: "playing".into(), ..PlayHints::default() });
        assert_eq!(cache.hints(id, "2", false, PlayKind::Party), PlayHints::default());

        // The title open, as opening it from a list leaves it, and its table loaded.
        cache.observe(&Event::Detail(Box::new(Entry::new(id, "Hoshikuzu no Tabibito"))), None);
        let mut table = rows(1..=3);
        table[0].completed = true;
        cache.observe(&Event::Episodes { id, rows: table }, None);
        for kind in [PlayKind::Staged, PlayKind::Party] {
            let PlayHints { title, next, episodes } = cache.hints(id, "2", false, kind);
            assert_eq!(title, "Hoshikuzu no Tabibito");
            assert_eq!(next.map(|n| n.number).as_deref(), Some("3"));
            let listed: Vec<(&str, bool)> =
                episodes.iter().map(|e| (e.number.as_str(), e.watched)).collect();
            assert_eq!(listed, [("1", true), ("2", false), ("3", false)]);
        }

        // A download goes by the name its row has, and so does the episode after it.
        cache.announce(id, "Hoshikuzu (downloaded)".into());
        assert_eq!(
            cache.hints(id, "3", false, PlayKind::Staged).title,
            "Hoshikuzu (downloaded)"
        );
    }

    /// Moved from the App's `another_titles_rows_never_speak_for_the_one_opened_after_them`:
    /// the table loaded last is one title's, and never answers for another.
    #[test]
    fn another_titles_table_never_answers_for_the_one_played() {
        let mut cache = TitleCache::default();
        listed(
            &mut cache,
            vec![
                Entry::new(AnilistId::new(1), "Title 0"),
                Entry::new(AnilistId::new(2), "Title 1"),
            ],
        );
        cache.observe(&Event::Episodes { id: AnilistId::new(1), rows: rows(1..=12) }, None);
        let other = cache.hints(AnilistId::new(2), "1", false, PlayKind::Staged);
        assert_eq!((other.next, other.episodes.len()), (None, 0));
        assert_eq!(other.title, "Title 1");
        let own = cache.hints(AnilistId::new(1), "1", false, PlayKind::Staged);
        assert_eq!(own.episodes.len(), 12, "still Title 0's");
    }

    #[test]
    fn a_table_loaded_since_replaces_the_one_before() {
        // As the terminal holds one: a title whose table was replaced plays as one with none,
        // its episodes loaded behind the playback.
        let mut cache = TitleCache::default();
        cache.observe(&Event::Episodes { id: AnilistId::new(1), rows: rows(1..=12) }, None);
        cache.observe(&Event::Episodes { id: AnilistId::new(2), rows: rows(1..=3) }, None);
        assert!(
            cache.hints(AnilistId::new(1), "1", false, PlayKind::Staged).episodes.is_empty()
        );
        assert_eq!(
            cache.table().map(|(id, rows)| (id, rows.len())),
            Some((AnilistId::new(2), 3))
        );
    }

    #[test]
    fn the_episode_after_one_that_followed_another_goes_by_its_name() {
        // Rolled on to, picked in the player, or restarted in the other translation: the name
        // the playback before went by, whatever the title is called in a list since.
        let id = AnilistId::new(3);
        let mut cache = TitleCache::default();
        listed(&mut cache, vec![Entry::new(id, "Kaze no Tayori")]);
        cache.announce(id, "Kaze no Tayori".into());
        listed(&mut cache, vec![Entry::new(id, "Kaze no Tayori (renamed)")]);
        assert_eq!(cache.hints(id, "2", false, PlayKind::Staged).title, "Kaze no Tayori");
        // Another title is named by the record, not by what played last.
        let other = AnilistId::new(4);
        listed(&mut cache, vec![Entry::new(other, "Hoshikuzu no Tabibito")]);
        assert_eq!(
            cache.hints(other, "1", false, PlayKind::Staged).title,
            "Hoshikuzu no Tabibito"
        );
    }

    #[test]
    fn up_next_steps_over_filler_only_when_asked() {
        let id = AnilistId::new(5);
        let mut cache = TitleCache::default();
        let mut table = rows(1..=6);
        for filler in [2, 3] {
            table[filler].skippable = true;
        }
        cache.observe(&Event::Episodes { id, rows: table }, None);
        let next = |skip| cache.hints(id, "2", skip, PlayKind::Staged).next.map(|n| n.number);
        assert_eq!(next(false).as_deref(), Some("3"));
        assert_eq!(next(true).as_deref(), Some("5"), "3 and 4 are filler");
    }

    #[test]
    fn a_relation_shows_by_the_name_its_page_lists_it_under() {
        use anistream_model::catalog::RelatedTitle;
        let (parent, sequel) = (AnilistId::new(8), AnilistId::new(9));
        let mut page = Entry::new(parent, "Hoshikuzu no Tabibito");
        page.related = vec![RelatedTitle {
            id: sequel,
            title: "Hoshikuzu no Tabibito II".into(),
            relation: "sequel".into(),
            format: None,
        }];
        let mut cache = TitleCache::default();
        cache.observe(&Event::Detail(Box::new(page)), None);
        assert_eq!(
            cache.hints(sequel, "1", false, PlayKind::Staged).title,
            "Hoshikuzu no Tabibito II"
        );
        let Some(Event::Progress { progress, resume, .. }) = cache.progress(sequel) else {
            panic!("named")
        };
        assert_eq!((progress, resume), (None, None));
    }

    // The App's tests of the progress rules, moved with the rules: each scenario as the engine
    // sees it, three titles listed and the first one's twelve-episode table loaded.

    const SHOW: AnilistId = AnilistId::new(1);

    /// Titles 1 to 3 listed, all saying `progress`, and the first one's table loaded with the
    /// episodes in `done` finished.
    fn at_episodes(progress: Option<(u32, u32)>, done: &[u32]) -> TitleCache {
        let mut cache = TitleCache::default();
        let entries = (1..=3)
            .map(|n| Entry {
                progress,
                ..Entry::new(AnilistId::new(n), format!("Title {}", n - 1))
            })
            .collect();
        listed(&mut cache, entries);
        let mut table = rows(1..=12);
        for row in &mut table {
            row.duration_secs = Some(1_435);
            if done.iter().any(|n| row.number == n.to_string()) {
                row.completed = true;
                row.watched = 1.0;
            }
        }
        cache.observe(&Event::Episodes { id: SHOW, rows: table }, None);
        cache
    }

    /// A playback of `episode` of the first title, started.
    fn play(cache: &mut TitleCache, episode: &str) -> Played {
        let played = Played {
            serial: cache.playbacks.len() as u64 + 1,
            id: SHOW,
            episode: episode.into(),
        };
        cache.started(&played);
        played
    }

    /// What the playback says, taken in: the title said whole when it moved.
    fn say(cache: &mut TitleCache, played: &Played, event: Event) -> Option<Event> {
        cache.observe(&event, Some(played)).and_then(|id| cache.progress(id))
    }

    fn tick(position: f64, duration: f64) -> Event {
        Event::Playback { position, duration: Some(duration), paused: false }
    }

    fn finished(next: Option<&str>) -> Event {
        Event::playback_ended(
            true,
            anistream_model::playback::Ending::Complete { next: next.map(Into::into) },
        )
    }

    fn ended(watched: bool, ending: anistream_model::playback::Ending) -> Event {
        Event::playback_ended(watched, ending)
    }

    /// Episodes done and the one Continue offers, and where it resumes, as the record says.
    fn standing(
        cache: &TitleCache,
        id: AnilistId,
    ) -> (Option<(u32, u32)>, Option<ResumePoint>) {
        let Some(Event::Progress { progress, resume, .. }) = cache.progress(id) else {
            panic!("{id:?} is known")
        };
        (progress.map(|p| (p.done, p.next)), resume)
    }

    /// A row of the first title's table: its meter, and whether it is finished.
    fn row_of(cache: &TitleCache, episode: &str) -> (f64, bool) {
        let (_, rows) = cache.table().expect("a table");
        let row = rows.iter().find(|r| r.number == episode).expect("a row");
        (row.watched, row.completed)
    }

    #[test]
    fn a_first_watch_moves_progress_without_waiting_for_a_reload() {
        // A show with no history has no (done, next) pair to advance, and the old rule —
        // only advance the episode progress was waiting on — silently skipped it, so the
        // detail meter sat empty until the next load from the store.
        let mut cache = at_episodes(None, &[]);
        let played = play(&mut cache, "1");
        assert!(say(&mut cache, &played, finished(None)).is_some(), "said, having moved");
        assert_eq!(standing(&cache, SHOW).0, Some((1, 2)));
    }

    #[test]
    fn a_watch_lands_only_on_the_entry_being_played() {
        // Several shows on Home can share a next-episode number; a finished episode used
        // to advance every one of them.
        let mut cache = at_episodes(Some((0, 1)), &[]);
        let played = play(&mut cache, "1");
        let Some(Event::Progress { id, .. }) = say(&mut cache, &played, finished(None)) else {
            panic!("the title is said")
        };
        assert_eq!(id, SHOW);
        assert_eq!(standing(&cache, SHOW).0, Some((1, 2)));
        assert_eq!(standing(&cache, AnilistId::new(2)).0, Some((0, 1)), "another show took it");
    }

    #[test]
    fn a_watched_mark_moves_the_title_and_its_list_row_too() {
        // Two watched, three next: marking three moves the title on, unmarking it puts it
        // back, and marking everything before episode six catches it up to there.
        let mut cache = at_episodes(Some((2, 3)), &[1, 2]);
        let held = |held: bool| move |_: &str| held;
        assert_eq!(cache.marked(SHOW, &["3".into()], true, held(true)), Some(SHOW));
        assert_eq!(standing(&cache, SHOW).0, Some((3, 4)), "marking ep 3 moves it on");
        assert_eq!(row_of(&cache, "3"), (1.0, true));
        cache.marked(SHOW, &["3".into()], false, held(false));
        assert_eq!(standing(&cache, SHOW).0, Some((2, 3)), "unmarking puts it back");
        assert_eq!(row_of(&cache, "3"), (0.0, false));
        let before_six = ["3", "4", "5"].map(String::from);
        cache.marked(SHOW, &before_six, true, held(true));
        assert_eq!(standing(&cache, SHOW).0, Some((5, 6)), "M catches it up to the cursor");
    }

    #[test]
    fn a_mark_the_store_did_not_take_is_not_shown() {
        // The record is moved on by what the store holds once written, not by what was asked.
        let mut cache = at_episodes(Some((2, 3)), &[1, 2]);
        let answer = cache.marked(SHOW, &["3".into()], true, |_| false);
        assert_eq!(answer, Some(SHOW), "a mark is answered all the same");
        assert_eq!(standing(&cache, SHOW).0, Some((2, 3)));
        assert_eq!(row_of(&cache, "3"), (0.0, false));
        // A title the record does not know has no answer to give.
        assert_eq!(cache.marked(AnilistId::new(99), &["1".into()], true, |_| true), None);
    }

    #[test]
    fn after_the_last_episode_continue_goes_back_to_one_skipped() {
        let done: Vec<u32> = (1..=11).filter(|n| *n != 4).collect();
        let mut cache = at_episodes(Some((10, 12)), &done);
        let played = play(&mut cache, "12");
        say(&mut cache, &played, finished(None));
        assert_eq!(standing(&cache, SHOW).0, Some((11, 4)));
    }

    #[test]
    fn continue_follows_an_unfinished_episode_watched_out_of_order() {
        // Episodes 1-5 finished; episode 8 left halfway. Continue is episode 8, at 10:00, with
        // the count still five.
        let mut cache = at_episodes(Some((5, 6)), &[1, 2, 3, 4, 5]);
        let played = play(&mut cache, "8");
        say(&mut cache, &played, tick(600.0, 1_200.0));
        say(&mut cache, &played, ended(false, anistream_model::playback::Ending::Closed));
        let (progress, resume) = standing(&cache, SHOW);
        assert_eq!(progress, Some((5, 8)));
        assert_eq!(resume.map(|r| r.position), Some(600.0));
    }

    #[test]
    fn continue_never_offers_an_episode_already_watched() {
        // Episodes 1-5 and 7 finished. Finishing 6 counts once, and Continue moves past 7.
        let mut cache = at_episodes(Some((6, 6)), &[1, 2, 3, 4, 5, 7]);
        let six = play(&mut cache, "6");
        say(&mut cache, &six, finished(Some("7")));
        assert_eq!(standing(&cache, SHOW).0, Some((7, 8)));

        // Seven rolled on to and stopped before it played; then a rewatch of episode 3, which
        // moves neither the count nor Continue back into what is done.
        let seven = play(&mut cache, "7");
        say(&mut cache, &seven, ended(false, anistream_model::playback::Ending::Stopped));
        let three = play(&mut cache, "3");
        say(&mut cache, &three, finished(None));
        assert_eq!(standing(&cache, SHOW).0, Some((7, 8)));
    }

    #[test]
    fn a_rewatch_does_not_inflate_the_progress_count() {
        // The store counts *distinct* completed episodes; the record has to hold the same line
        // or the meter jumps on every revisit.
        let mut cache = at_episodes(Some((3, 4)), &[3]);
        let played = play(&mut cache, "3");
        say(&mut cache, &played, finished(None));
        assert_eq!(standing(&cache, SHOW).0, Some((3, 4)));
    }

    #[test]
    fn quitting_partway_moves_the_resume_point_at_once() {
        // The Continue rail reads the resume point; leaving it where the *previous* session
        // stopped made a fresh quit look unrecorded until the next load from the store.
        let mut cache = at_episodes(Some((0, 1)), &[]);
        let played = play(&mut cache, "1");
        say(&mut cache, &played, tick(600.0, 1_200.0));
        say(&mut cache, &played, ended(false, anistream_model::playback::Ending::Closed));
        let resume = standing(&cache, SHOW).1.expect("a resume point");
        assert_eq!(resume.position, 600.0);
        assert_eq!(resume.fraction, Some(0.5));
    }

    #[test]
    fn finishing_an_episode_updates_the_table_without_a_reload() {
        // Reported from real use: the episode table still showed the old state after an
        // episode finished, so it looked as though nothing had been recorded.
        let mut cache = at_episodes(None, &[]);
        // Nothing is playing yet, so a tick is ignored rather than inventing state.
        let stray = Played { serial: 99, id: SHOW, episode: "1".into() };
        assert!(say(&mut cache, &stray, tick(0.0, 1_440.0)).is_none());
        assert_eq!(row_of(&cache, "1"), (0.0, false));

        // Halfway through, then detached: the row shows where the viewer actually is.
        let played = play(&mut cache, "1");
        let Some(Event::Progress { episodes, .. }) =
            say(&mut cache, &played, tick(720.0, 1_440.0))
        else {
            panic!("the tick moved the row")
        };
        let first = episodes.iter().find(|e| e.number == "1").expect("said");
        assert!((first.watched - 0.5).abs() < 1e-6, "got {}", first.watched);
        assert!(!first.completed, "halfway is not finished");

        say(&mut cache, &played, finished(None));
        assert_eq!(row_of(&cache, "1"), (1.0, true), "finished reads as finished");
        assert_eq!(row_of(&cache, "2"), (0.0, false), "only the episode watched changes");
        // And a tick after the end is a straggler, not the playback coming back.
        assert!(say(&mut cache, &played, tick(100.0, 1_440.0)).is_none());
    }

    #[test]
    fn a_re_watch_quit_early_does_not_erase_that_it_was_finished() {
        let mut cache = at_episodes(None, &[1]);
        let played = play(&mut cache, "1");
        say(&mut cache, &played, tick(60.0, 1_440.0));
        assert_eq!(row_of(&cache, "1"), (1.0, true), "still finished, and monotonic");
    }

    #[test]
    fn closing_or_stopping_past_the_threshold_still_counts_as_watched() {
        // Split from the App's test that such an ending does not roll on: it is recorded as
        // watched all the same.
        use anistream_model::playback::Ending;
        for ending in [Ending::Closed, Ending::Stopped] {
            let mut cache = at_episodes(None, &[]);
            let played = play(&mut cache, "1");
            say(&mut cache, &played, ended(true, ending.clone()));
            assert_eq!(
                row_of(&cache, "1"),
                (1.0, true),
                "recorded as watched after {ending:?}"
            );
        }
    }

    #[test]
    fn a_window_handed_over_is_a_watch_by_what_it_said() {
        let mut cache = at_episodes(Some((0, 1)), &[]);
        let played = play(&mut cache, "1");
        let left = Event::EpisodeLeft {
            id: SHOW,
            episode: "1".into(),
            position: 300.0,
            duration: Some(1_200.0),
            watched: false,
        };
        assert!(say(&mut cache, &played, left).is_some());
        assert_eq!(standing(&cache, SHOW).1.map(|r| r.position), Some(300.0));
        assert_eq!(row_of(&cache, "1"), (0.25, false));
        // It has ended: what it says after is a straggler.
        assert!(say(&mut cache, &played, finished(None)).is_none());
    }

    #[test]
    fn the_record_lets_the_titles_touched_least_recently_go() {
        let mut cache = TitleCache::default();
        let first = AnilistId::new(1);
        listed(&mut cache, vec![Entry::new(first, "the first")]);
        let many =
            (2..=TITLES as u32 + 1).map(|n| Entry::new(AnilistId::new(n), format!("t{n}")));
        listed(&mut cache, many.collect());
        assert_eq!(cache.titles.len(), TITLES);
        assert!(cache.progress(first).is_none(), "the oldest went");
        assert!(cache.progress(AnilistId::new(2)).is_some());
    }
}
