//! A title's episode table: its rows and their filter, the marks made on it, the releases and
//! downloads asked for from it, and the provider's match for the title.

use anistream_core::{ids::AnilistId, traits::SourceCandidate};
use anistream_i18n::t;
use anistream_model::{
    Msg,
    catalog::{ContentList, ResumePoint},
    command::Command,
    episodes::{DubExtent, EpisodeRow},
    event::Event,
    notice::Notice,
    progress::{EpisodeProgress, TitleProgress},
    sources::{MatchCandidate, MatchQuestion},
};

use super::{App, Field, info};
use crate::nav::{Overlay, StageView};

/// `"4"` → one episode, `"1-12"` → inclusive range, `"7-"` → everything from 7 on.
fn parse_episode_range(input: &str) -> Option<(u32, Option<u32>)> {
    let input = input.trim();
    if let Some((from, to)) = input.split_once('-') {
        let from: u32 = from.trim().parse().ok()?;
        let to = to.trim();
        let to: Option<u32> = if to.is_empty() { None } else { Some(to.parse().ok()?) };
        if to.is_some_and(|t| t < from) {
            return None;
        }
        Some((from, to))
    } else {
        let n: u32 = input.parse().ok()?;
        Some((n, Some(n)))
    }
}

/// What the episode table shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EpisodeFilter {
    #[default]
    All,
    /// Only episodes not yet completed.
    Unwatched,
    /// Everything except pure filler. `mixed` stays — it carries story.
    NoFiller,
}

/// A wait in the largest unit that still reads at a glance (`6d 17h`, `1h 5m`, `7m`), in the
/// language in use.
pub(crate) fn countdown_text(seconds: i64) -> String {
    let span = anistream_model::Span::countdown(seconds);
    let int = |v: u64| v as i64;
    if span.days > 0 {
        if span.hours > 0 {
            t::duration_days_hours(int(span.days.into()), int(span.hours.into()))
        } else {
            t::duration_days(int(span.days.into()))
        }
    } else if span.hours > 0 {
        if span.minutes > 0 {
            t::duration_hours_minutes(int(span.hours.into()), int(span.minutes.into()))
        } else {
            t::duration_hours(int(span.hours.into()))
        }
    } else {
        t::duration_minutes(int(span.minutes.into()))
    }
}

impl EpisodeFilter {
    pub fn label(self) -> String {
        match self {
            Self::All => t::episodes_filter_all(),
            Self::Unwatched => t::episodes_filter_unwatched(),
            Self::NoFiller => t::episodes_filter_no_filler(),
        }
    }

    /// What the toast says when the filter changes to this one.
    fn showing(self) -> String {
        match self {
            Self::All => t::episodes_showing_all(),
            Self::Unwatched => t::episodes_showing_unwatched(),
            Self::NoFiller => t::episodes_showing_no_filler(),
        }
    }

    const fn next(self) -> Self {
        match self {
            Self::All => Self::Unwatched,
            Self::Unwatched => Self::NoFiller,
            Self::NoFiller => Self::All,
        }
    }

    fn keeps(self, row: &EpisodeRow) -> bool {
        match self {
            Self::All => true,
            Self::Unwatched => !row.completed,
            Self::NoFiller => !row.skippable,
        }
    }
}

impl App {
    /// Re-derive the visible episode list from the full one, keeping the cursor on the same
    /// episode.
    ///
    /// By episode, not by row: keeping the index put the cursor on whatever episode the new
    /// view had in that slot, so cycling the filter moved you without a key being pressed. An
    /// episode the filter now hides hands the cursor to the next one it keeps — which is also
    /// what makes marking down an unwatched-only list walk forward one episode at a time.
    pub(super) fn apply_episode_filter(&mut self) {
        let filter = self.episode_filter;
        let under_cursor =
            self.episodes.get(self.cursors.episodes.selected).map(|r| r.number.clone());
        self.episodes = self.episodes_all.iter().filter(|r| filter.keeps(r)).cloned().collect();
        let kept = under_cursor.and_then(|number| {
            let from = self.episodes_all.iter().position(|r| r.number == number)?;
            let next = self.episodes_all[from..].iter().find(|r| filter.keeps(r))?;
            self.episodes.iter().position(|r| r.number == next.number)
        });
        self.cursors.episodes.selected = kept
            .unwrap_or(self.cursors.episodes.selected)
            .min(self.episodes.len().saturating_sub(1));
    }

    /// Whether the filter is hiding everything — distinct from a title with no episodes.
    pub fn episodes_all_filtered_out(&self) -> bool {
        self.episodes.is_empty() && !self.episodes_all.is_empty()
    }

    /// Releases to pick from, for the episode the source picker was asked about.
    pub(super) fn apply_sources(
        &mut self,
        id: AnilistId,
        episode: String,
        candidates: Vec<SourceCandidate>,
    ) {
        // A stale answer — the user has navigated away since asking, or asked about
        // another episode — is not a question worth interrupting them with, and offering
        // one episode's releases under another's context would play the wrong one.
        if self.source_context.as_ref() != Some(&(id, episode)) {
            return;
        }
        if candidates.is_empty() {
            self.source_context = None;
            self.push_toast(info(t::episodes_no_releases()));
            return;
        }
        self.sources = candidates;
        self.cursors.overlay.selected = 0;
        self.nav.open_overlay(Overlay::Sources);
    }

    /// What a provider might have the title as, when it could not tell on its own: the question
    /// the Disambiguate overlay asks.
    pub(super) fn apply_match_choices(
        &mut self,
        id: AnilistId,
        provider_id: String,
        candidates: Vec<MatchCandidate>,
        answers: MatchQuestion,
    ) {
        // Nor is one about a title the user has left.
        if !self.detail.as_ref().is_some_and(|d| d.id == id) {
            return;
        }
        // An empty set is not a question worth asking: a search that found nothing says so.
        if candidates.is_empty() {
            if let MatchQuestion::Search { query } = answers {
                self.push_toast(Notice::alert(Msg::NoticeSourceSearchEmpty { query }));
            }
            return;
        }
        self.episodes_loading = false;
        self.match_candidates = candidates;
        self.match_context = Some((id, provider_id));
        self.cursors.overlay.selected = 0;
        self.nav.open_overlay(Overlay::Disambiguate);
    }

    /// A match picked for title `id` that could not be used: said, and the table no longer
    /// waited on, since what the attempt still listed arrives on its own.
    pub(super) fn apply_match_fix_failed(&mut self, id: AnilistId, reason: Msg) {
        if self.detail.as_ref().is_some_and(|d| d.id == id) {
            self.episodes_loading = false;
        }
        self.push_toast(Notice::alert(reason));
    }

    /// A title's episode table.
    pub(super) fn apply_episodes(&mut self, id: AnilistId, rows: Vec<EpisodeRow>) {
        // `detail` is the title the table is a view of; rows for any other belong to a
        // load the user has since walked away from.
        if !self.detail.as_ref().is_some_and(|d| d.id == id) {
            return;
        }
        self.episodes_all = rows;
        self.episodes_for = Some(id);
        self.apply_episode_filter();
        self.episodes_loading = false;
        // The amber bar lands where the viewer is going: the first unwatched
        // row. Row nought means "just opened" — a cursor already mid-list is a
        // position the user chose, and a reload must not move it.
        if self.cursors.episodes.selected == 0
            && let Some(first) = self.episodes.iter().position(|r| !r.completed)
        {
            self.cursors.episodes.selected = first;
        }
    }

    /// How far title `id`'s dub goes, as the engine says: kept for the Episodes screen's line,
    /// and nothing kept when it has none or could not say.
    pub(super) fn apply_dub(&mut self, id: AnilistId, dub: Option<DubExtent>) {
        match dub {
            Some(dub) => self.dub = Some((id, dub)),
            None if self.dub.is_some_and(|(kept, _)| kept == id) => self.dub = None,
            None => {}
        }
    }

    /// The Episodes screen's line on the dub of title `id`: "Dub to episode 6", or `None`.
    pub fn dub_line(&self, id: AnilistId) -> Option<String> {
        let dub = self.dub.filter(|(kept, _)| *kept == id).map(|(_, dub)| dub);
        let dub = dub?;
        Some(if dub.complete {
            t::episodes_dub_complete()
        } else {
            t::episodes_dub_through(i64::from(dub.through))
        })
    }

    /// Where a title stands, and each of its episodes, as the engine says once a watch or a mark
    /// has been written.
    pub(super) fn apply_progress(
        &mut self,
        id: AnilistId,
        progress: Option<TitleProgress>,
        resume: Option<ResumePoint>,
        episodes: Vec<EpisodeProgress>,
    ) {
        // Every entry of the title on screen says it alike: its page and its rows in the
        // list under it. Only this title's: several shows on Home can share a
        // next-episode number.
        for entry in self.detail.iter_mut().chain(match &mut self.content {
            ContentList::Entries(entries) => entries.iter_mut(),
            _ => [].iter_mut(),
        }) {
            if entry.id == id {
                entry.progress = progress;
                entry.resume = resume;
            }
        }
        // The table, row by row, when it is this title's: while detached the viewer can
        // be looking at another show entirely, whose rows must not take this one's.
        // In place, so the cursor and the filter's view stay as they are.
        if self.episodes_for == Some(id) {
            for list in [&mut self.episodes, &mut self.episodes_all] {
                for row in list.iter_mut() {
                    if let Some(said) = episodes.iter().find(|e| e.number == row.number) {
                        row.watched = said.watched;
                        row.completed = said.completed;
                    }
                }
            }
        }
        // The answer to a mark made here. Under the unwatched filter a marked row
        // leaves the view at once — which is the binge-marking flow working.
        if self.marking == Some(id) {
            self.marking = None;
            self.apply_episode_filter();
        }
    }

    /// Start the episode selected in the timing sheet. One listed from AniList alone has
    /// nothing to play it from, so Enter goes to the Providers screen to add a source instead.
    pub(super) fn play_selected_episode(&mut self) -> Option<Command> {
        let id = match self.nav.current() {
            StageView::Episodes(id) => *id,
            _ => return None,
        };
        let row = self.episodes.get(self.cursors.episodes.selected)?;
        if row.metadata_only {
            return self.go_to_section(crate::nav::Section::Providers);
        }
        let episode = row.number.clone();
        Some(self.begin_playback(id, episode))
    }

    /// Whether title `id`'s table is the one loaded and was listed from AniList alone: no
    /// source here has it to play or download.
    pub fn unlisted(&self, id: AnilistId) -> bool {
        self.episodes_for == Some(id)
            && self.episodes_all.first().is_some_and(|r| r.metadata_only)
    }

    /// Whether the highlighted row of the table is one listed from AniList alone.
    pub fn selected_unlisted(&self) -> bool {
        self.in_episodes()
            && self
                .episodes
                .get(self.cursors.episodes.selected)
                .is_some_and(|r| r.metadata_only)
    }

    /// Said for a download asked of a table listed from AniList alone, in place of asking.
    pub(crate) fn say_add_a_source(&mut self) {
        let sources = self.section_key(crate::nav::Section::Providers);
        self.push_toast(info(t::episodes_add_source_to_download(&sources)));
    }

    /// Sources became ready where none were: a table listed from AniList alone while there were
    /// none is asked for again, so it takes the source's numbering.
    pub(super) fn sources_arrived(&mut self) {
        if let Some(id) = self.episodes_for.filter(|id| self.unlisted(*id)) {
            self.commands.push(Command::LoadEpisodes(id));
        }
    }

    /// The episode table of the title here, asked for afresh.
    pub(super) fn show_episodes(&mut self) -> Option<Command> {
        // On a title screen it is that title — the list underneath may be sitting on
        // something else entirely, as it is after stepping through related titles. On a
        // list it is the row under the cursor. See `target`.
        if let Some(entry) = self.target().cloned() {
            let id = entry.id;
            // The episode table is a view *of a title*, and `detail` is how the rest of
            // the app asks which title that is. Reached by `e` from a list it was left
            // unset, so everything downstream answered "none": a finished episode never
            // reached the table it was listed in, and auto-next declined to roll on.
            // Opening the title sets this; entering its episodes has to as well.
            self.detail = Some(entry);
            self.cursors.episodes.selected = 0;
            // Discard the previous title's rows *before* the screen appears. They were
            // being left in place until the new load answered, so opening episodes for one
            // show displayed another show's episode list — complete with its watch
            // progress — and then silently corrected itself. Stale data presented as
            // current is worse than no data: the loading state is honest, and it is the
            // reason the pulse and skeleton rows exist.
            self.episodes.clear();
            self.episodes_loading = true;
            self.nav.push(StageView::Episodes(id));
            // How far its dub goes, for the line beside the title: the engine asks once the
            // table is in, and answers at once from what it kept when it has.
            self.commands.push(Command::LoadDub(id));
            return Some(Command::LoadEpisodes(id));
        }
        None
    }

    /// The highlighted episode marked watched, or unwatched if it was.
    pub(super) fn toggle_watched(&mut self) -> Option<Command> {
        let id = match self.nav.current() {
            StageView::Episodes(id) => *id,
            _ => return None,
        };
        let (number, watched) = {
            let row = self.episodes.get(self.cursors.episodes.selected)?;
            (row.number.clone(), !row.completed)
        };
        // Shown once the engine has written it and says where the title now stands.
        self.marking = Some(id);
        Some(Command::SetWatched { id, episodes: vec![number], watched })
    }

    /// Every episode before the highlighted one that is not finished, marked watched.
    pub(super) fn mark_all_previous(&mut self) -> Option<Command> {
        let id = match self.nav.current() {
            StageView::Episodes(id) => *id,
            _ => return None,
        };
        // Everything strictly before the selected row that is not already done —
        // the "I'm caught up to here" gesture.
        let marked: Vec<String> = self
            .episodes
            .iter()
            .take(self.cursors.episodes.selected)
            .filter(|r| !r.completed)
            .map(|r| r.number.clone())
            .collect();
        if marked.is_empty() {
            // Three different nothings, each said as what it is: "nothing before this
            // episode" over an empty table pointed at an episode that was not there.
            let why = match self.episodes.get(self.cursors.episodes.selected) {
                None => t::episodes_mark_none_listed(),
                Some(_) if self.cursors.episodes.selected == 0 => {
                    t::episodes_mark_nothing_before()
                }
                Some(row) => t::episodes_mark_already_watched(&row.number),
            };
            self.push_toast(info(why));
            return None;
        }
        self.marking = Some(id);
        self.push_toast(info(t::episodes_marked_watched(marked.len() as i64)));
        Some(Command::SetWatched { id, episodes: marked, watched: true })
    }

    /// The prompt for a range of episodes to download, empty.
    pub(super) fn open_download_range(&mut self) {
        if self.title_on_screen().is_some_and(|id| self.unlisted(id)) {
            self.say_add_a_source();
            return;
        }
        self.field_mut(Field::Range).clear();
        self.cursors.overlay.selected = 0;
        self.nav.open_overlay(Overlay::DownloadRange);
    }

    /// Enter on a range of episodes: those of them that exist, queued for download.
    pub(super) fn submit_range(&mut self) -> Option<Command> {
        let id = match self.nav.current() {
            StageView::Episodes(id) => *id,
            _ => return None,
        };
        // Resolved against episodes that actually exist, from the unfiltered
        // list — a filter narrows the view, not what can be fetched.
        let Some((from, to)) = parse_episode_range(self.text(Field::Range)) else {
            // Cleared, with the prompt left open for another try. Kept, the retry's
            // keys were appended to the refused text and refused again.
            self.field_mut(Field::Range).clear();
            self.push_toast(info(t::episodes_range_invalid()));
            return None;
        };
        let episodes: Vec<String> = self
            .episodes_all
            .iter()
            .filter(|row| {
                row.number
                    .trim()
                    .parse::<u32>()
                    .is_ok_and(|n| n >= from && to.is_none_or(|t| n <= t))
            })
            .map(|row| row.number.clone())
            .collect();
        if episodes.is_empty() {
            self.field_mut(Field::Range).clear();
            self.push_toast(info(t::episodes_range_empty()));
            return None;
        }
        self.nav.close_overlay();
        self.push_toast(info(t::episodes_queueing(episodes.len() as i64)));
        Some(Command::DownloadMany { id, episodes })
    }

    /// The episode table's next filter, said as it is applied.
    pub(super) fn cycle_episode_filter(&mut self) {
        self.episode_filter = self.episode_filter.next();
        self.apply_episode_filter();
        self.push_toast(info(self.episode_filter.showing()));
    }

    /// The prompt for a provider's search by hand, for a title it matched wrongly.
    pub(super) fn fix_mapping(&mut self) -> Option<Command> {
        // "This matched the wrong thing" — ask the user what to search for, then
        // re-enter the ordinary disambiguation flow with their words.
        //
        // From a title's page or its episodes only: the answer is a new episode table,
        // which is taken for the title on screen and nowhere else, so from a list the
        // pick would be asked for and then silently dropped — or, with a title opened
        // earlier, applied to that title instead of the highlighted one.
        let Some(id) = self.title_on_screen() else {
            let hint = if self.target().is_some() {
                t::episodes_match_off_page()
            } else {
                t::episodes_match_nothing()
            };
            self.push_toast(info(hint));
            return None;
        };
        self.manual_target = Some(id);
        self.field_mut(Field::ManualQuery).clear();
        self.cursors.overlay.selected = 0;
        self.nav.open_overlay(Overlay::ManualQuery);
        None
    }

    /// Enter on a search by hand: the search, or with nothing typed the pin forgotten.
    pub(super) fn submit_manual_query(&mut self) -> Option<Command> {
        let query = self.text(Field::ManualQuery).trim().to_owned();
        let id = self.manual_target?;
        self.nav.close_overlay();
        // Empty enter is the undo: forget the pin, let the ladder decide again.
        if query.is_empty() {
            self.episodes.clear();
            self.episodes_loading = true;
            return Some(Command::ClearMatch { id });
        }
        Some(Command::ManualSearch { id, query, provider_id: None })
    }

    /// The candidate picked, pinned as the title's match.
    pub(super) fn confirm_match(&mut self) -> Option<Command> {
        let candidate = self.match_candidates.get(self.cursors.overlay.selected)?.clone();
        let (id, provider_id) = self.match_context.clone()?;
        self.nav.close_overlay();
        self.cursors.overlay.selected = 0;
        self.match_candidates.clear();
        self.match_context = None;
        self.episodes_loading = true;
        self.push_toast(info(t::episodes_matched(&candidate.title)));
        Some(Command::FixMatch {
            id,
            provider_id,
            key: candidate.key,
            title: Some(candidate.title),
        })
    }

    /// An episode's releases to pick from: the highlighted one in the table, or the next
    /// unwatched one from a title.
    pub(super) fn show_sources(&mut self) -> Option<Command> {
        // Same shape as Download: the highlighted episode from the table, the next
        // unwatched one from a title.
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
        self.source_context = Some((id, episode.clone()));
        Some(Command::LoadSources { id, episode })
    }

    /// The release picked, played.
    pub(super) fn confirm_source(&mut self) -> Option<Command> {
        let candidate = self.sources.get(self.cursors.overlay.selected)?.clone();
        let (id, episode) = self.source_context.clone()?;
        self.nav.close_overlay();
        self.cursors.overlay.selected = 0;
        self.sources.clear();
        self.source_context = None;
        self.raise_now_playing(id, &episode);
        Some(Command::PlaySource {
            id,
            episode,
            provider_id: candidate.provider_id,
            source_id: candidate.id,
        })
    }

    /// The episode table, asked for again.
    pub(super) fn refresh_episodes(&mut self) -> Option<Command> {
        // The rows stay on screen until the answer lands, and the answer keeps the
        // cursor on its episode; the activity row says the table is being asked again.
        self.images.requests().retry_failed();
        self.title_on_screen().map(Command::LoadEpisodes)
    }

    /// The episode table for a title, when the rows loaded are that title's — what the player's
    /// Episodes menu lists. Empty for any other title rather than another show's rows.
    pub fn episode_rows_for(&self, id: AnilistId) -> &[EpisodeRow] {
        // Every row, not the filtered view: a filter on the title screen is about what to look
        // at there, not about which episodes exist. Asked by the rows' own title rather than
        // `detail`, which moves to a newly opened title before that title's rows arrive.
        if self.episodes_for == Some(id) { &self.episodes_all } else { &[] }
    }

    /// Whether the episode table currently has focus.
    pub fn in_episodes(&self) -> bool {
        matches!(self.nav.current(), StageView::Episodes(_))
    }

    /// [`Self::content_here`] for the open title's episode table.
    pub fn episodes_here(&self, rows: Vec<EpisodeRow>) -> Event {
        // No title open means the reducer drops the rows whatever id they carry.
        let id = self.detail.as_ref().map_or(AnilistId::new(0), |d| d.id);
        Event::Episodes { id, rows, failed: None }
    }

    /// [`Self::content_here`] for the open source picker.
    pub fn sources_here(&self, candidates: Vec<SourceCandidate>) -> Event {
        let (id, episode) =
            self.source_context.clone().unwrap_or((AnilistId::new(0), String::new()));
        Event::Sources { id, episode, candidates }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::fixtures::*;

    /// One episode marked watched or not on the table, in both the visible and the full list,
    /// as a test sets the stage.
    fn set_watched(a: &mut App, number: &str, watched: bool) {
        for list in [&mut a.episodes, &mut a.episodes_all] {
            mark_row(list, number, watched);
        }
    }

    /// The engine's word on a mark, as far as the table goes: each episode the mark asks for
    /// marked or not, with its meter full or empty to match, said back as the engine says a
    /// title's progress. Where the title then stands is the engine's to work out, and its
    /// tests' to check; here it is said as it stood. Returns the command, to be checked.
    fn answer_mark(a: &mut App, task: Option<Command>) -> Option<Command> {
        let Some(Command::SetWatched { id, episodes, watched }) = &task else { return task };
        let mut rows = a.episodes_all.clone();
        for episode in episodes {
            mark_row(&mut rows, episode, *watched);
        }
        let entry = a.detail.as_ref().filter(|d| d.id == *id);
        a.apply(Event::Progress {
            id: *id,
            progress: entry.and_then(|e| e.progress),
            resume: entry.and_then(|e| e.resume),
            episodes: rows
                .iter()
                .map(|r| EpisodeProgress {
                    number: r.number.clone(),
                    watched: r.watched,
                    completed: r.completed,
                })
                .collect(),
        });
        task
    }

    /// What the engine says of a title's episodes, each by number with its meter and whether
    /// it is finished, and of the title as the App's page for it stands.
    fn episodes_said(a: &App, id: AnilistId, rows: &[(&str, f64, bool)]) -> Event {
        let entry = a.detail.as_ref().filter(|d| d.id == id);
        Event::Progress {
            id,
            progress: entry.and_then(|e| e.progress),
            resume: entry.and_then(|e| e.resume),
            episodes: rows
                .iter()
                .map(|(number, watched, completed)| EpisodeProgress {
                    number: (*number).into(),
                    watched: *watched,
                    completed: *completed,
                })
                .collect(),
        }
    }

    #[test]
    fn enter_in_the_episode_table_plays_rather_than_opening() {
        // Enter opens a title everywhere else, so this is the one place it has to mean
        // something different — the table *is* the list of things to play.
        let mut a = app_at_episodes();
        a.set_episode_cursor(4);
        let task = a.handle(Action::Open, 20);
        assert_eq!(
            task,
            Some(Command::play(AnilistId::new(1), "5")),
            "Enter must play the selected episode"
        );
        assert!(matches!(a.nav.current(), StageView::NowPlaying));
        assert_eq!(a.playing.as_ref().map(|p| p.episode.clone()), Some("5".into()));
    }

    fn candidate(title: &str, similarity: f64) -> MatchCandidate {
        MatchCandidate {
            title: title.into(),
            key: anistream_core::ids::ProviderKey::new(title),
            similarity,
            rejected: None,
            year: None,
            episodes: None,
            format: None,
        }
    }

    fn source(title: &str, auto: bool) -> SourceCandidate {
        SourceCandidate {
            id: format!("https://indexer.example/view/{title}"),
            provider_id: "torrent".into(),
            title: title.into(),
            quality: Some(1080),
            label: Some("120 peers".into()),
            size: Some("1.4 GiB".into()),
            dual_audio: false,
            dubbed: false,
            auto_pick: auto,
        }
    }

    #[test]
    fn asking_for_sources_opens_the_slate_and_a_pick_plays_it() {
        let mut a = app_at_episodes();
        let task = a.handle(Action::ShowSources, 20).expect("asking must produce a task");
        assert!(matches!(task, Command::LoadSources { .. }), "got {task:?}");

        a.apply(a.sources_here(vec![
            source("[A] Show - 1 (1080p)", true),
            source("[B] Show - 1 (1080p)", false),
        ]));
        assert_eq!(a.nav.overlay(), Some(&Overlay::Sources));

        // Move to the second release and take it.
        a.handle(Action::Down, 20);
        let task = a.handle(Action::Open, 20).expect("picking must play");
        match task {
            Command::PlaySource { provider_id, source_id, .. } => {
                assert_eq!(provider_id, "torrent");
                assert!(source_id.contains("[B]"), "the pick must be the selected row");
            }
            other => panic!("expected PlaySource, got {other:?}"),
        }
        // The question is put away and playback staging is up, exactly as Enter on an
        // episode row would have it.
        assert_eq!(a.nav.overlay(), None);
        assert!(a.playing.is_some(), "a pick must stage Now Playing");
        assert!(a.sources.is_empty());
    }

    #[test]
    fn an_empty_slate_answers_with_a_toast_rather_than_an_empty_overlay() {
        let mut a = app_at_episodes();
        a.handle(Action::ShowSources, 20);
        a.apply(a.sources_here(Vec::new()));
        assert_ne!(a.nav.overlay(), Some(&Overlay::Sources));
        assert!(a.source_context.is_none(), "an answered question is not left pending");
    }

    #[test]
    fn toggling_watched_flips_the_row_and_persists() {
        let mut a = app_at_episodes();
        let task = a_toggle(&mut a);
        let task = answer_mark(&mut a, task);
        assert!(a.episodes[0].completed, "the table shows the mark once the engine has it");
        assert!(
            matches!(task, Some(Command::SetWatched { watched: true, .. })),
            "got {task:?}"
        );
        assert!(!a.awaiting_mark(), "answered");

        let task = a_toggle(&mut a);
        let task = answer_mark(&mut a, task);
        assert!(!a.episodes[0].completed);
        assert!(
            matches!(task, Some(Command::SetWatched { watched: false, .. })),
            "got {task:?}"
        );
    }

    /// `u` on the table: the mark it asks for, with the App left waiting on the engine's word.
    fn a_toggle(a: &mut App) -> Option<Command> {
        let task = a.handle(Action::ToggleWatched, 20);
        assert!(a.awaiting_mark(), "a mark waits on the engine's word");
        task
    }

    #[test]
    fn a_watched_mark_moves_the_title_and_its_list_row_too() {
        // Where a title stands is the engine's to work out, from the mark; the App shows it
        // wherever the title is on screen: its page and its row in the list under it, and only
        // that title's, though another shares its next-episode number.
        let mut a = app_at_episodes();
        let at = |a: &App, row: usize| {
            (a.detail.as_ref().and_then(|d| d.progress), a.content.entries()[row].progress)
        };
        if let ContentList::Entries(rows) = &mut a.content {
            rows[1].progress = Some(TitleProgress { done: 2, next: 3 });
        }
        a.apply(Event::Progress {
            id: AnilistId::new(1),
            progress: Some(TitleProgress { done: 3, next: 4 }),
            resume: None,
            episodes: Vec::new(),
        });
        let (three, two) = (
            Some(TitleProgress { done: 3, next: 4 }),
            Some(TitleProgress { done: 2, next: 3 }),
        );
        assert_eq!(at(&a, 0), (three, three), "the page and the list row move");
        assert_eq!(at(&a, 1).1, two, "another show is not touched");
    }

    #[test]
    fn marking_previous_marks_only_the_unwatched_before_the_cursor() {
        let mut a = app_at_episodes();
        a.set_episode_cursor(4);
        // Already done — must not be re-marked. Both lists, since the visible one is
        // re-derived from the full one whenever marks change.
        a.episodes[1].completed = true;
        a.episodes_all[1].completed = true;

        let task = a.handle(Action::MarkAllPrevious, 20);
        let task = answer_mark(&mut a, task);
        match task.expect("marking must persist") {
            Command::SetWatched { episodes, watched: true, .. } => {
                assert_eq!(episodes, vec!["1".to_string(), "3".into(), "4".into()]);
            }
            other => panic!("expected SetWatched, got {other:?}"),
        }
        assert!(a.episodes[..4].iter().all(|r| r.completed));
        assert!(!a.episodes[4].completed, "the cursor row itself stays untouched");
    }

    #[test]
    fn a_typed_range_queues_the_episodes_that_exist() {
        let mut a = app_at_episodes();
        a.handle(Action::DownloadRange, 20);
        assert_eq!(a.nav.overlay(), Some(&Overlay::DownloadRange));
        for c in "9-".chars() {
            a.type_char(c);
        }
        let task = a.handle(Action::Open, 20).expect("a range must queue");
        match task {
            Command::DownloadMany { episodes, .. } => {
                assert_eq!(episodes, vec!["9", "10", "11", "12"], "open-ended from 9");
            }
            other => panic!("expected DownloadMany, got {other:?}"),
        }
        assert_eq!(a.nav.overlay(), None);
    }

    #[test]
    fn a_count_of_one_is_said_in_the_singular() {
        let mut a = app_at_episodes();
        a.handle(Action::DownloadRange, 20);
        a.type_char('4');
        let task = a.handle(Action::Open, 20).expect("one episode is a range too");
        assert!(
            a.toasts.iter().any(|t| t.notice.text == t::episodes_queueing(1)),
            "{:?}",
            a.toasts
        );
        assert_eq!(task.activity().as_deref(), Some("queueing 1 episode"));
    }

    #[test]
    fn mark_all_previous_says_which_nothing_it_found() {
        let mut a = app_at_episodes();
        a.handle(Action::MarkAllPrevious, 20);
        assert!(a.toasts.iter().any(|t| t.notice.text == t::episodes_mark_nothing_before()));

        a.set_episode_cursor(3);
        let task = a.handle(Action::MarkAllPrevious, 20);
        answer_mark(&mut a, task);
        assert!(
            a.toasts.iter().any(|t| t.notice.text == t::episodes_marked_watched(3)),
            "{:?}",
            a.toasts
        );
        a.handle(Action::MarkAllPrevious, 20);
        assert!(
            a.toasts.iter().any(|t| t.notice.text == t::episodes_mark_already_watched("4"))
        );

        // An empty table has no episode to be before.
        a.episodes.clear();
        a.set_episode_cursor(0);
        a.handle(Action::MarkAllPrevious, 20);
        assert!(
            a.toasts.iter().any(|t| t.notice.text == t::episodes_mark_none_listed()),
            "{:?}",
            a.toasts
        );
    }

    #[test]
    fn a_nonsense_range_is_refused_with_a_hint() {
        let mut a = app_at_episodes();
        a.handle(Action::DownloadRange, 20);
        for c in "12-4".chars() {
            a.type_char(c);
        }
        assert!(a.handle(Action::Open, 20).is_none(), "a backwards range queues nothing");
        assert!(
            a.toasts.iter().any(|t| t.notice.text.contains("1-12")),
            "the hint names the shape"
        );
    }

    #[test]
    fn the_filter_narrows_the_table_and_cycles_back_to_all() {
        let mut a = app_at_episodes();
        for i in [0, 1, 2] {
            a.episodes[i].completed = true;
            a.episodes_all[i].completed = true;
        }
        a.handle(Action::Filter, 20);
        assert_eq!(a.episode_filter, EpisodeFilter::Unwatched);
        assert_eq!(a.episodes.len(), 9, "three watched episodes leave the view");

        // Marking one under the unwatched filter removes it at once, once it is written.
        let task = a.handle(Action::ToggleWatched, 20);
        answer_mark(&mut a, task);
        assert_eq!(a.episodes.len(), 8);

        a.handle(Action::Filter, 20);
        a.handle(Action::Filter, 20);
        assert_eq!(a.episode_filter, EpisodeFilter::All);
        assert_eq!(a.episodes.len(), 12, "cycling back restores every row");
    }

    #[test]
    fn a_wrong_match_can_be_re_searched_by_hand() {
        let mut a = app_at_episodes();
        a.handle(Action::FixMapping, 20);
        assert_eq!(a.nav.overlay(), Some(&Overlay::ManualQuery));
        assert!(a.is_typing(), "the overlay must take text");

        for c in "hoshikuzu".chars() {
            a.type_char(c);
        }
        let task = a.handle(Action::Open, 20).expect("enter must search");
        match task {
            Command::ManualSearch { query, .. } => assert_eq!(query, "hoshikuzu"),
            other => panic!("expected ManualSearch, got {other:?}"),
        }
        // The overlay closes; the results return through MatchChoices and reuse the
        // Disambiguate flow, which has its own tests.
        assert_eq!(a.nav.overlay(), None);
    }

    #[test]
    fn a_source_slate_nobody_asked_for_stays_quiet() {
        // A stale answer arriving after the user navigated away must not interrupt.
        let mut a = app_at_episodes();
        a.apply(a.sources_here(vec![source("[A] Show - 1 (1080p)", true)]));
        assert_ne!(a.nav.overlay(), Some(&Overlay::Sources));
    }

    #[test]
    fn an_undecidable_match_asks_rather_than_failing() {
        // The candidates were being thrown away and reported as "could not match this title",
        // which is a dead end for a question the user can answer at a glance.
        let mut a = app_at_episodes();
        a.apply(Event::MatchChoices {
            id: AnilistId::new(1),
            provider_id: "torrent".into(),
            candidates: vec![candidate("Hoshikuzu S1", 0.62), candidate("Hoshikuzu S2", 0.61)],
            answers: MatchQuestion::Ladder,
            source: "Torrent".into(),
        });

        assert_eq!(a.nav.overlay(), Some(&Overlay::Disambiguate));
        assert_eq!(a.match_candidates.len(), 2);
        assert!(!a.episodes_loading, "the wait is over; a question is not a wait");
    }

    #[test]
    fn no_candidates_is_a_failure_rather_than_an_empty_question() {
        let mut a = app_at_episodes();
        a.apply(Event::MatchChoices {
            id: AnilistId::new(1),
            provider_id: "torrent".into(),
            candidates: Vec::new(),
            answers: MatchQuestion::Ladder,
            source: "Torrent".into(),
        });
        assert_ne!(a.nav.overlay(), Some(&Overlay::Disambiguate));
    }

    #[test]
    fn a_search_that_found_nothing_says_so_and_a_failed_pick_stops_the_wait() {
        let mut a = app_at_episodes();
        a.apply(Event::MatchChoices {
            id: AnilistId::new(1),
            provider_id: "torrent".into(),
            candidates: Vec::new(),
            answers: MatchQuestion::Search { query: "hoshikuzu".into() },
            source: "Torrent".into(),
        });
        assert_ne!(a.nav.overlay(), Some(&Overlay::Disambiguate));
        let said: Vec<_> = a.toasts.iter().map(|t| t.notice.text.clone()).collect();
        assert!(said.iter().any(|t| t.contains("hoshikuzu")), "{said:?}");

        a.episodes_loading = true;
        a.apply(Event::MatchFixFailed {
            id: AnilistId::new(1),
            reason: Msg::NoticeMatchDidNotStick,
        });
        assert!(!a.episodes_loading, "nothing more is coming for the pick");
    }

    #[test]
    fn picking_a_candidate_pins_it_and_reloads() {
        let mut a = app_at_episodes();
        a.apply(Event::MatchChoices {
            id: AnilistId::new(1),
            provider_id: "torrent".into(),
            candidates: vec![candidate("Hoshikuzu S1", 0.62), candidate("Hoshikuzu S2", 0.61)],
            answers: MatchQuestion::Ladder,
            source: "Torrent".into(),
        });

        // Move to the second candidate and take it.
        a.handle(Action::Down, 20);
        let task = a.handle(Action::Open, 20).expect("picking must do something");
        assert_eq!(
            task,
            Command::FixMatch {
                id: AnilistId::new(1),
                provider_id: "torrent".into(),
                key: anistream_core::ids::ProviderKey::new("Hoshikuzu S2"),
                title: Some("Hoshikuzu S2".into()),
            }
        );

        // And the question is put away rather than left hanging behind the reload.
        assert_ne!(a.nav.overlay(), Some(&Overlay::Disambiguate));
        assert!(a.match_candidates.is_empty());
        assert!(a.episodes_loading, "the answer starts a load");
    }

    #[test]
    fn another_titles_rows_never_speak_for_the_one_opened_after_them() {
        // Title 0's rows are loaded; Title 1 is opened and its own rows have not arrived. The
        // open title is Title 1, and Title 0's rows must not be taken for its.
        let mut a = app_at_episodes();
        a.detail = a.content.entries().get(1).cloned();
        assert!(a.episode_rows_for(AnilistId::new(2)).is_empty());
        assert_eq!(a.episode_rows_for(AnilistId::new(1)).len(), 12, "still Title 0's");
        a.begin_playback(AnilistId::new(2), "1".into());
        assert_eq!(a.playing.as_ref().unwrap().episode_title, None);
    }

    #[test]
    fn enter_on_a_title_page_opens_its_episodes_rather_than_another_title() {
        let mut a = home_list(8);
        a.handle(Action::Open, 5);
        a.handle(Action::Down, 5);
        assert_eq!(a.handle(Action::Open, 5), Some(Command::LoadEpisodes(AnilistId::new(1))));
        assert_eq!(a.nav.current(), &StageView::Episodes(AnilistId::new(1)));
    }

    #[test]
    fn refresh_on_a_title_or_its_episodes_asks_for_that_title() {
        // It used to reload the section's list instead, under a page that never changed.
        let mut a = home_list(3);
        a.handle(Action::Open, 5);
        assert_eq!(a.handle(Action::Refresh, 5), Some(Command::LoadDetail(AnilistId::new(1))));
        a.handle(Action::ShowEpisodes, 5);
        assert_eq!(
            a.handle(Action::Refresh, 5),
            Some(Command::LoadEpisodes(AnilistId::new(1)))
        );
        assert_eq!(a.content.len(), 3, "the list under them is left alone");
    }

    #[test]
    fn back_through_a_related_titles_episodes_finds_the_parents_table() {
        let mut a = app_at_episodes();
        if let Some(detail) = a.detail.as_mut() {
            detail.related = vec![related(90, "Its sequel")];
        }
        a.set_episode_cursor(4);
        a.handle(Action::WatchOrder, 5);
        a.handle(Action::Open, 5);
        a.handle(Action::ShowEpisodes, 5);
        a.apply(Event::Episodes {
            id: AnilistId::new(90),
            rows: vec![EpisodeRow { number: "1".into(), ..a.episodes_all[0].clone() }],
            failed: None,
        });
        assert_eq!(a.episodes.len(), 1);

        a.handle(Action::Back, 5);
        assert_eq!(a.nav.current(), &StageView::Title(AnilistId::new(90)));
        a.handle(Action::Back, 5);
        assert_eq!(a.nav.current(), &StageView::Episodes(AnilistId::new(1)));
        assert_eq!(a.target().map(|e| e.id), Some(AnilistId::new(1)));
        assert_eq!(a.episodes.len(), 12, "the parent's table, not the child's");
        assert_eq!(a.episodes[a.episode_cursor()].number, "5", "and where it was left");
    }

    #[test]
    fn the_filter_keeps_the_same_episode_under_the_cursor() {
        let mut a = app_at_episodes();
        for n in ["1", "2", "3"] {
            set_watched(&mut a, n, true);
        }
        a.apply_episode_filter();
        let under = |a: &App| a.episodes[a.episode_cursor()].number.clone();

        a.set_episode_cursor(4);
        a.handle(Action::Filter, 5);
        assert_eq!(a.episode_filter, EpisodeFilter::Unwatched);
        assert_eq!(under(&a), "5", "the cursor kept its row index, not its episode");
        a.handle(Action::Filter, 5);
        a.handle(Action::Filter, 5);
        assert_eq!(a.episode_filter, EpisodeFilter::All);
        assert_eq!(under(&a), "5");

        // An episode the filter hides hands the cursor to the next one it keeps.
        a.set_episode_cursor(1);
        a.handle(Action::Filter, 5);
        assert_eq!(under(&a), "4");
    }

    #[test]
    fn finishing_an_episode_updates_the_table_without_a_reload() {
        // Reported from real use: the episode table still showed the old state after an episode
        // finished, so it looked as though nothing had been recorded.
        let mut a = app();
        a.apply(a.content_here(entries(3)));
        a.nav.focus_stage();
        let id = a.selected_entry().expect("an entry").id;
        a.detail = Some(a.selected_entry().expect("an entry").clone());
        // Push the view first: it clears any previous title's rows, which is the point.
        a.handle(Action::ShowEpisodes, 10);
        a.apply(a.episodes_here(vec![
            EpisodeRow {
                number: "1".into(),
                title: None,
                duration_secs: Some(1440),
                watched: 0.0,
                completed: false,
                kind: None,
                skippable: false,
                airs_at: None,
                delayed_from: None,
                metadata_only: false,
                source: None,
                unsourced: false,
                past_progress: false,
                thumbnail: None,
                description: None,
            },
            EpisodeRow {
                number: "2".into(),
                title: None,
                duration_secs: Some(1440),
                watched: 0.0,
                completed: false,
                kind: None,
                skippable: false,
                airs_at: None,
                delayed_from: None,
                metadata_only: false,
                source: None,
                unsourced: false,
                past_progress: false,
                thumbnail: None,
                description: None,
            },
        ]));

        a.set_episode_cursor(1);
        a.begin_playback(id, "1".into());
        a.nav.pop();

        // What a watch moves is the engine's to work out — see its record's test of the same
        // name — and it says so ahead of what moved it. The table takes it in place, row by
        // row, with the cursor where it was. Halfway through, then detached:
        a.apply(episodes_said(&a, id, &[("1", 0.5, false), ("2", 0.0, false)]));
        a.apply(Event::Playback { position: 720.0, duration: Some(1440.0), paused: false });
        assert!((a.episodes[0].watched - 0.5).abs() < 1e-6, "got {}", a.episodes[0].watched);
        assert!(!a.episodes[0].completed, "halfway is not finished");

        a.apply(episodes_said(&a, id, &[("1", 1.0, true), ("2", 0.0, false)]));
        a.apply(finished(None));
        assert!(a.episodes[0].completed, "a finished episode must show as finished");
        assert!((a.episodes[0].watched - 1.0).abs() < 1e-6);
        assert_eq!(a.episodes[1].watched, 0.0, "only the episode watched should change");
        assert_eq!(a.episode_cursor(), 1, "and the cursor stays put");

        // Another title's word never lands on this table.
        let other = AnilistId::new(3);
        a.apply(episodes_said(&a, other, &[("2", 1.0, true)]));
        assert!(!a.episodes[1].completed);
    }

    /// Reported from real use: an episode finished, the toast said so, and its row stayed empty
    /// and unmarked until the show was left and re-entered. Reaching the table with `e` rather
    /// than by opening the title left the app with no idea which title was on screen, so the
    /// watch had nowhere to land — and auto-next, asking the same question, stayed put.
    #[test]
    fn reaching_episodes_by_key_still_lands_a_finished_watch_on_the_table() {
        // Auto-next is on by default, and the playback's to decide: it names the next episode
        // in the ending it reports.
        let mut a = app();
        a.apply(a.content_here(entries(3)));
        a.nav.focus_stage();
        let id = a.selected_entry().expect("an entry").id;
        // Straight from the list — no detail screen in between, which is the whole point.
        a.handle(Action::ShowEpisodes, 10);
        a.apply(
            a.episodes_here(
                (1..=3)
                    .map(|n| EpisodeRow {
                        number: n.to_string(),
                        title: None,
                        duration_secs: Some(1440),
                        watched: 0.0,
                        completed: false,
                        kind: None,
                        skippable: false,
                        airs_at: None,
                        delayed_from: None,
                        metadata_only: false,
                        source: None,
                        unsourced: false,
                        past_progress: false,
                        thumbnail: None,
                        description: None,
                    })
                    .collect(),
            ),
        );

        a.begin_playback(id, "1".into());
        // The engine says where the title stands ahead of the ending; reached by `e`, the
        // table is the title's, so that lands on it.
        a.apply(episodes_said(
            &a,
            id,
            &[("1", 1.0, true), ("2", 0.0, false), ("3", 0.0, false)],
        ));
        a.apply(finished(Some("2")));

        assert!(a.episodes[0].completed, "the finished episode must be marked in the table");
        assert!((a.episodes[0].watched - 1.0).abs() < 1e-6, "and its meter filled");
        // Auto-next is the engine's, which stages the next episode of the title that played.
        a.apply(staged(1, "Title 0", "2", None));
        assert_eq!(
            a.playing.as_ref().map(|p| (p.id, p.episode.as_str())),
            Some((Some(id), "2")),
            "auto-next follows the episode that just played"
        );
    }

    /// [`app_at_episodes`], its table listed from AniList alone, as with no source set up.
    fn unlisted_table() -> App {
        let mut a = app_at_episodes();
        let rows =
            a.episodes_all.iter().map(|row| EpisodeRow { metadata_only: true, ..row.clone() });
        a.apply(a.episodes_here(rows.collect()));
        a
    }

    fn ready(id: &str) -> Event {
        let row = anistream_model::sources::ProviderRow {
            id: id.into(),
            usable: true,
            ..Default::default()
        };
        Event::Providers(vec![row])
    }

    #[test]
    fn a_row_from_anilist_alone_is_marked_as_any_and_enter_goes_to_add_a_source() {
        let mut a = unlisted_table();
        assert!(a.selected_unlisted());
        let mark = a.handle(Action::ToggleWatched, 10);
        assert!(matches!(mark, Some(Command::SetWatched { watched: true, .. })), "{mark:?}");
        // The Providers screen, checked as entering it checks it, and nothing played.
        assert_eq!(a.handle(Action::Open, 10), Some(Command::CheckProviders));
        assert_eq!(a.nav.current(), &StageView::Section(Section::Providers));
        assert!(a.playing.is_none());
    }

    #[test]
    fn a_download_from_a_table_from_anilist_alone_is_not_asked_for() {
        let mut a = unlisted_table();
        assert_eq!(a.handle(Action::Download, 10), None);
        a.handle(Action::DownloadRange, 10);
        assert!(a.nav.overlay().is_none(), "no range is asked for either");
        // Said once, as the same toast twice is.
        let said: Vec<_> = a.toasts.iter().map(|t| t.notice.text.as_str()).collect();
        assert_eq!(said, [t::episodes_add_source_to_download("7")]);
    }

    #[test]
    fn a_source_ready_where_none_was_asks_for_a_table_from_anilist_again() {
        let mut a = unlisted_table();
        a.apply(ready("fixture"));
        assert_eq!(a.take_commands(), [Command::LoadEpisodes(AnilistId::new(1))]);
        // Ready already: nothing new to ask for.
        a.apply(ready("fixture"));
        assert!(a.take_commands().is_empty());
        // A table a source listed is the source's numbering already.
        let mut a = app_at_episodes();
        a.apply(ready("fixture"));
        assert!(a.take_commands().is_empty());
    }

    /// Reported from real use: after viewing one show's episodes, every other show opened on
    /// that first one's list. `detail` outlives the screen that sets it, so reading it ahead of
    /// the cursor pinned the table to whatever was seen first — until the section was left and
    /// re-entered, which is the one thing that clears it.
    #[test]
    fn each_show_in_a_list_opens_its_own_episodes() {
        let mut a = app();
        a.apply(a.content_here(entries(3)));
        a.nav.focus_stage();

        assert_eq!(
            a.handle(Action::ShowEpisodes, 10),
            Some(Command::LoadEpisodes(AnilistId::new(1))),
            "the selected row"
        );
        a.handle(Action::Back, 10);
        a.handle(Action::Down, 10);
        assert_eq!(
            a.handle(Action::ShowEpisodes, 10),
            Some(Command::LoadEpisodes(AnilistId::new(2))),
            "the next row, not the one already seen"
        );

        // From a title screen the subject is that title, whatever the list beneath is on.
        a.handle(Action::Back, 10);
        a.detail = Some(Entry::new(AnilistId::new(99), "a related title"));
        a.nav.push(StageView::Title(AnilistId::new(99)));
        assert_eq!(
            a.handle(Action::ShowEpisodes, 10),
            Some(Command::LoadEpisodes(AnilistId::new(99))),
            "a title screen speaks for itself"
        );
    }

    #[test]
    fn opening_episodes_never_shows_the_previous_title_s_rows() {
        // Reported from real use: the table displayed the last title's episodes — with their watch
        // progress — until the new load answered, then corrected itself. Stale content presented as
        // current is worse than an honest wait.
        let mut a = app();
        a.apply(a.content_here(entries(3)));
        a.nav.focus_stage();
        a.detail = Some(a.selected_entry().expect("an entry").clone());
        a.handle(Action::ShowEpisodes, 10);
        a.apply(a.episodes_here(vec![EpisodeRow {
            number: "7".into(),
            title: Some("from the first title".into()),
            duration_secs: Some(1440),
            watched: 0.9,
            completed: true,
            kind: None,
            skippable: false,
            airs_at: None,
            delayed_from: None,
            metadata_only: false,
            source: None,
            unsourced: false,
            past_progress: false,
            thumbnail: None,
            description: None,
        }]));
        assert_eq!(a.episodes.len(), 1);
        assert!(!a.episodes_loading);

        // Back out and into a different title.
        a.handle(Action::Back, 10);
        a.handle(Action::Down, 10);
        a.detail = Some(a.selected_entry().expect("an entry").clone());
        let task = a.handle(Action::ShowEpisodes, 10);

        assert!(a.episodes.is_empty(), "the old rows must be gone before the screen appears");
        assert!(a.episodes_loading, "and the wait must be visible as a wait");
        assert!(matches!(task, Some(Command::LoadEpisodes(_))));
    }

    #[test]
    fn a_source_with_no_episodes_reads_differently_from_one_still_loading() {
        let mut a = app();
        a.apply(a.content_here(entries(1)));
        a.detail = Some(a.selected_entry().expect("an entry").clone());
        a.handle(Action::ShowEpisodes, 10);
        assert!(a.episodes_loading);
        // An empty answer is still an answer, and must clear the wait.
        a.apply(a.episodes_here(Vec::new()));
        assert!(!a.episodes_loading, "an empty result is not a permanent loading state");
    }

    #[test]
    fn late_episodes_for_a_title_already_left_do_not_fill_the_next_ones_table() {
        let mut a = app();
        a.apply(a.content_here(entries(3)));
        a.nav.focus_stage();
        a.handle(Action::ShowEpisodes, 20); // title 1
        a.handle(Action::Back, 20);
        a.handle(Action::Down, 20);
        a.handle(Action::ShowEpisodes, 20); // title 2
        let row = |n: &str| EpisodeRow {
            number: n.into(),
            title: None,
            duration_secs: None,
            watched: 0.0,
            completed: false,
            kind: None,
            skippable: false,
            airs_at: None,
            delayed_from: None,
            metadata_only: false,
            source: None,
            unsourced: false,
            past_progress: false,
            thumbnail: None,
            description: None,
        };
        a.apply(Event::Episodes { id: AnilistId::new(1), rows: vec![row("1")], failed: None });
        assert!(a.episodes.is_empty(), "title two's table showed title one's episodes");
        assert!(a.episodes_loading, "title two's own load is still outstanding");

        a.apply(Event::Episodes {
            id: AnilistId::new(2),
            rows: vec![row("1"), row("2")],
            failed: None,
        });
        assert_eq!(a.episodes.len(), 2);
    }

    #[test]
    fn a_match_question_about_a_title_already_left_is_not_asked() {
        let mut a = app_at_episodes();
        a.apply(Event::MatchChoices {
            id: AnilistId::new(7),
            provider_id: "torrent".into(),
            candidates: vec![candidate("Something else", 0.6)],
            answers: MatchQuestion::Ladder,
            source: "Torrent".into(),
        });
        assert_ne!(a.nav.overlay(), Some(&Overlay::Disambiguate));
    }

    #[test]
    fn releases_for_another_episode_are_not_offered_against_this_one() {
        let mut a = app_at_episodes();
        a.handle(Action::ShowSources, 20);
        let (id, asked) = a.source_context.clone().expect("a pending question");
        a.apply(Event::Sources {
            id,
            episode: format!("{asked}0"),
            candidates: vec![source("[A] Show - 10 (1080p)", true)],
        });
        assert_ne!(
            a.nav.overlay(),
            Some(&Overlay::Sources),
            "offered another episode's releases"
        );

        a.apply(a.sources_here(vec![source("[A] Show - 1 (1080p)", true)]));
        assert_eq!(a.nav.overlay(), Some(&Overlay::Sources));
    }
}
