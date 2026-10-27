//! The sections' lists: what each one asks for and takes, the calendar's paging back through
//! time, Search with its filters, the Library's segments, and the Continue list.

use anistream_i18n::t;
use anistream_model::{
    catalog::{ContentFor, ContentList, Entry, Season, TitleAired},
    command::Command,
    event::Event,
    search::{MediaFormat, SearchQuery, SearchSort},
};

use super::{App, Cursor, Field, info};
use crate::{
    clock::Clock,
    nav::{Focus, Overlay, Section, StageView},
};

/// The local calendar date an offset-from-now lands on. Shared with the renderer, so
/// where the calendar *opens* and where it *rules* can never disagree.
pub(crate) fn air_date(clock: &Clock, secs_from_now: i64) -> Option<chrono::NaiveDate> {
    clock.date_of(clock.now_epoch() + secs_from_now)
}

/// Idle ticks a rail change waits before fetching. The idle tick is 100 ms, so this is ~300 ms —
/// long enough to coalesce a held arrow key, short enough not to feel like lag.
const RELOAD_IDLE_TICKS: u8 = 3;

/// How close to the top of the calendar the selection gets before more history is fetched.
///
/// A screenful, roughly. The fetch is one request against a 30-per-minute budget and takes a
/// few hundred milliseconds, so it has to start while there are still rows left to scroll
/// through — trigger it at row nought and the user reaches the end of the list before the
/// answer arrives, which is the jump this design exists to avoid.
const CALENDAR_BACKFILL_MARGIN: usize = 12;

impl App {
    /// The highlighted row of the list on screen, when the screen on top is one.
    ///
    /// `content` stays put while a title is pushed over it, and screens with rows of their own
    /// (Settings, Accounts, Downloads) never replace it, so an unconditional read answered with
    /// a row nobody could see: Enter on a title page opened it, and `a` on Accounts asked for
    /// the status of a show from the list before.
    pub fn selected_entry(&self) -> Option<&Entry> {
        self.content_view()?;
        if !matches!(self.nav.current(), StageView::Section(_)) {
            return None;
        }
        self.content.entries().get(self.section_cursor().selected)
    }

    /// Fetch older broadcasts once the selection nears the top of the calendar.
    ///
    /// Deliberately silent, and deliberately early. The question being asked by scrolling up is
    /// "what aired before this", and answering it with a spinner and a viewport that jumps would
    /// be a worse answer than the truncated week this replaces. So the request goes out while
    /// there is still a screenful above the selection left to scroll through, and the rows land
    /// above the viewport without moving it — see [`Self::apply`].
    pub(super) fn backfill_calendar_if_near_top(&mut self) {
        if self.nav.section() != Section::Calendar
            || self.calendar_loading_past
            || !self.calendar_more_past
            || self.cursors.calendar.selected > CALENDAR_BACKFILL_MARGIN
        {
            return;
        }
        let Some(anchor) = self.calendar_anchor else { return };
        self.calendar_loading_past = true;
        self.commands.push(Command::LoadCalendarPast { anchor, page: self.calendar_pages + 1 });
    }

    /// Fetch the next page of search results once the selection nears the end of them, quietly,
    /// as the calendar fetches its past.
    pub(super) fn more_search_if_near_end(&mut self) {
        if self.nav.section() != Section::Search
            || self.search_loading_more
            || !self.search_more
            || self.cursors.search.selected + SEARCH_MORE_MARGIN < self.content.len()
        {
            return;
        }
        self.search_loading_more = true;
        self.commands.push(Command::SearchMore {
            query: self.searched_query.clone(),
            page: self.search_pages + 1,
        });
    }

    /// Whether the search just answered has more pages.
    pub(super) fn apply_search_paging(&mut self, query: SearchQuery, has_next: bool) {
        if query != self.searched_query {
            return;
        }
        self.search_pages = 1;
        self.search_more = has_next;
        self.search_loading_more = false;
    }

    /// A further page of the results on screen, put after them.
    pub(super) fn apply_search_more(
        &mut self,
        query: SearchQuery,
        page: u32,
        entries: Vec<Entry>,
        has_next: bool,
    ) {
        if query != self.searched_query || self.nav.section() != Section::Search {
            return;
        }
        self.search_loading_more = false;
        self.search_more = has_next;
        if entries.is_empty() {
            return;
        }
        self.search_pages = self.search_pages.max(page);
        if let ContentList::Entries(shown) = &mut self.content {
            let known: std::collections::HashSet<_> = shown.iter().map(|e| e.id).collect();
            shown.extend(entries.into_iter().filter(|e| !known.contains(&e.id)));
        }
    }

    /// The search the field and the filters make: the text as typed, without the spaces
    /// around it, and every filter set.
    pub fn search_query(&self) -> SearchQuery {
        SearchQuery {
            text: self.text(Field::Search).trim().to_owned(),
            ..self.search_filter.clone()
        }
    }

    /// Run the search the field and the filters make, from the top of its results.
    fn run_search(&mut self) -> Command {
        let query = self.search_query();
        self.searched_query = query.clone();
        self.search_pages = 0;
        self.search_more = false;
        self.search_loading_more = false;
        self.content = ContentList::Loading;
        // New results start at their top, not at the row the old ones were left on.
        self.cursors.search = Cursor::default();
        Command::Search { query }
    }

    /// The paging of the calendar just loaded: the instant its pages count back from, how many
    /// pages of the past it holds, and whether there are more.
    pub(super) fn apply_calendar_window(&mut self, anchor: i64, pages: u32, more_past: bool) {
        // Paging for a calendar the user has left would let a scroll elsewhere ask for
        // history nobody is looking at.
        if self.nav.section() != Section::Calendar {
            return;
        }
        self.calendar_anchor = Some(anchor);
        self.calendar_pages = pages;
        self.calendar_more_past = more_past;
        self.calendar_loading_past = false;
    }

    /// A page of older broadcasts, as [`Self::backfill_calendar_if_near_top`] asked for, or word
    /// that it could not be had.
    pub(super) fn apply_calendar_backfill(
        &mut self,
        page: Option<u32>,
        entries: Vec<Entry>,
        more_past: bool,
    ) {
        self.calendar_loading_past = false;
        self.calendar_more_past = more_past;
        if let Some(page) = page {
            self.calendar_pages = self.calendar_pages.max(page);
        }
        // Only the screen that asked can take them. A backfill landing after the user
        // has moved on would otherwise splice a fortnight of broadcasts into whatever
        // list is on screen now.
        if !entries.is_empty()
            && self.nav.section() == Section::Calendar
            && let ContentList::Entries(existing) = &mut self.content
        {
            let added = entries.len();
            let mut merged = entries;
            merged.append(existing);
            self.content = ContentList::Entries(merged);
            // The whole point of the exercise: the rows arrive *above* the viewport, so
            // both the selection and the scroll position shift by exactly what was
            // inserted and nothing moves under the user. Without this the screen would
            // lurch a page and a half every time history was extended.
            self.cursors.calendar.selected += added;
            self.cursors.calendar.offset += added;
        }
    }

    /// Whether the Library list has focus, so horizontal keys mean "step segment".
    pub(super) fn in_library_stage(&self) -> bool {
        self.nav.section() == Section::Library
            && self.nav.focus() == Focus::Stage
            && matches!(self.nav.current(), StageView::Section(Section::Library))
    }

    /// Step the Library's segment, and from the first one carry Left on to the rail — the
    /// screen's left edge, where Left goes everywhere else.
    pub(super) fn step_segment(&mut self, delta: isize) -> Option<Command> {
        let next = self.library_segment.step(delta);
        if next == self.library_segment {
            if delta < 0 {
                self.nav.focus_rail();
            }
            return None;
        }
        self.library_segment = next;
        self.cursors.library = Cursor::default();
        self.content = ContentList::Loading;
        Some(Command::LoadLibrary(self.library_segment))
    }

    /// Whether the Search section's own view — the field and its results — is on top.
    pub(super) fn on_search_root(&self) -> bool {
        matches!(self.nav.current(), StageView::Section(Section::Search))
    }

    /// Back into Search's field, with the query and its results as they were.
    pub(super) fn focus_search_field(&mut self) {
        self.search_in_results = false;
        self.nav.focus_stage();
    }

    /// The FILTERS overlay, on its first row.
    pub(super) fn open_search_filters(&mut self) {
        self.cursors.overlay.selected = 0;
        self.nav.open_overlay(Overlay::SearchFilters);
    }

    /// The FILTERS overlay over Search, from wherever it is asked for.
    pub(super) fn filter_search(&mut self) -> Option<Command> {
        // From anywhere: land on Search first, because the overlay's Enter runs a
        // search and the results have to have somewhere to go. The jump closes any
        // overlays, so this one is opened after.
        let task = (self.nav.section() != Section::Search)
            .then(|| self.go_to_section(Section::Search))
            .flatten();
        self.cursors.overlay.selected = 0;
        self.nav.open_overlay(Overlay::SearchFilters);
        task
    }

    /// Enter on FILTERS: the search, with the filters as they are set. With nothing typed it
    /// is a browse, most popular first.
    pub(super) fn confirm_search_filters(&mut self) -> Option<Command> {
        self.nav.close_overlay();
        Some(self.run_search())
    }

    /// Enter in Search's field: the search, or the highlighted result once the results are for
    /// the query as typed. With nothing typed, a browse with the filters set.
    pub(super) fn submit_search(&mut self) -> Option<Command> {
        // Enter submitted the search *every* time, so a result could be highlighted and
        // never opened — the key just re-ran the same query. It submits only while there is
        // something new to ask; once the results on screen are for this query, Enter does
        // what Enter does everywhere else in the app and opens the selection.
        if self.search_opens_on_enter() {
            return self.open_selected();
        }
        Some(self.run_search())
    }

    /// Whether Enter resumes the highlighted row rather than opening it: a part-watched title
    /// on the Home list. One answer for the key and for the hint that names it.
    pub fn resumes_on_enter(&self) -> bool {
        matches!(self.nav.current(), StageView::Section(Section::Home))
            && self.selected_entry().is_some_and(|e| e.resume.is_some() && e.progress.is_some())
    }

    /// The highlighted Continue row's episode, resumed where it was left.
    pub(super) fn resume_selected(&mut self) -> Option<Command> {
        let entry = self.selected_entry().expect("checked");
        let id = entry.id;
        let next = entry.progress.expect("checked").next;
        Some(self.begin_playback(id, next.to_string()))
    }

    /// The highlighted title taken off the Continue list.
    pub(super) fn hide_from_continue(&mut self) -> Option<Command> {
        // The Home list only: the CONTINUE rail is the one list built from your
        // history, so it is the one list a title can be dismissed from. The view
        // check matters as much as the section — a pushed detail or episode table
        // keeps the Home entries as `content`, and removing the show you are
        // *looking into* because a list you cannot see holds it would be absurd.
        // Elsewhere the key says so rather than doing nothing, which would read
        // as broken.
        let at_home_list = self.nav.section() == Section::Home
            && matches!(self.nav.current(), StageView::Section(Section::Home));
        if !at_home_list {
            self.push_toast(info(t::lists_remove_wrong_list()));
            return None;
        }
        let ContentList::Entries(entries) = &mut self.content else {
            return None;
        };
        if entries.is_empty() {
            return None;
        }
        // Dropped from the screen before the write: the list answers immediately, and
        // the store catches up.
        let home = &mut self.cursors.home;
        let entry = entries.remove(home.selected.min(entries.len() - 1));
        home.selected = home.selected.min(entries.len().saturating_sub(1));
        home.offset = home.offset.min(home.selected);
        // Say how to undo, because there is no key for it — the way back is the
        // same thing that put it here.
        self.push_toast(info(t::lists_continue_removed(&entry.title)));
        Some(Command::HideFromContinue { id: entry.id })
    }

    /// Whether Enter in Search's field opens the highlighted result rather than searching:
    /// the results on screen are already for the query as typed.
    pub fn search_opens_on_enter(&self) -> bool {
        self.search_query() == self.searched_query && !self.content.is_empty()
    }

    /// Push the selected title's detail screen — what Enter does on a row with nothing to
    /// resume. Public for `--preview`, which shows a title page from a list whose Enter plays.
    pub fn open_selected(&mut self) -> Option<Command> {
        let Some(entry) = self.selected_entry().cloned() else {
            self.push_toast(info(t::lists_nothing_selected()));
            return None;
        };
        let id = entry.id;
        self.detail = Some(entry);
        self.nav.push(StageView::Title(id));
        Some(Command::LoadDetail(id))
    }

    /// Move the rail selection by `delta`, clamping at the ends.
    ///
    /// Clamps rather than wraps: a wrapping rail means holding ↓ cycles forever with no sense of
    /// where you are, and eight sections is short enough that hitting the end is informative.
    pub(super) fn step_section(&mut self, delta: isize) -> Option<Command> {
        let current = self.nav.section().index() as isize;
        let last = (Section::ALL.len() - 1) as isize;
        let next = (current + delta).clamp(0, last) as usize;
        if next == current as usize {
            return None;
        }
        let section = Section::from_index(next)?;
        // Move now, fetch shortly. Holding the arrow key sweeps the rail at keyboard-repeat speed,
        // and firing a request per step is how a 30-per-minute budget disappears in two seconds.
        self.nav.go_to(section);
        self.nav.focus_rail();
        // Entered at its top, as every section is: see `Cursors`.
        *self.cursors.section_mut(section) = Cursor::default();
        self.detail = None;
        self.title_trail.clear();
        self.search_in_results = false;
        // Loading only where a list is about to be asked for. A section with no list of its
        // own — or Search with nothing typed — would otherwise pulse forever, since the fetch
        // that ends the pulse never goes out.
        self.content =
            if self.fetches_list() { ContentList::Loading } else { ContentList::Empty };
        self.reload_countdown = Some(RELOAD_IDLE_TICKS);
        None
    }

    /// Titles are named another way now: what is on screen names them the old way, so the
    /// list it shows and the title page open over it are asked for again. A screen with no
    /// list, Settings where the change is made among them, has nothing to ask for, and a list
    /// is asked for as it is opened.
    pub(super) fn titles_changed(&mut self) {
        if self.fetches_list()
            && let Some(task) = self.reload()
        {
            self.commands.push(task);
        }
        if let Some(id) = self.title_on_screen() {
            self.commands.push(Command::LoadDetail(id));
        }
    }

    /// Fire a settled section change. Called from the idle tick.
    pub fn tick_pending_reload(&mut self) {
        let Some(remaining) = self.reload_countdown else { return };
        if remaining > 0 {
            self.reload_countdown = Some(remaining - 1);
            return;
        }
        self.reload_countdown = None;
        if let Some(task) = self.reload() {
            // Queued because the idle tick has no return channel, the same route a scroll's
            // backfill takes.
            self.commands.push(task);
        }
    }

    /// Jump to `section`, at the top of its own view, and ask for what it shows.
    ///
    /// Every way into a section is through here or the rail's step, and both put its cursor at
    /// the top: the sections shared one cursor once, which a change of section reset, and
    /// keeping that is what lets each have its own with nothing on screen telling them apart.
    pub fn go_to_section(&mut self, section: Section) -> Option<Command> {
        self.nav.go_to(section);
        *self.cursors.section_mut(section) = Cursor::default();
        self.detail = None;
        self.title_trail.clear();
        self.search_in_results = false;
        self.reload()
    }

    /// The list the current view shows, if it shows one — what an [`Event::Content`] has to
    /// be tagged with to be taken.
    pub fn content_view(&self) -> Option<ContentFor> {
        Some(match self.nav.section() {
            Section::Home => ContentFor::Continue,
            Section::Seasonal => ContentFor::Seasonal,
            Section::Calendar => ContentFor::Calendar,
            Section::Search => ContentFor::Search { query: self.searched_query.clone() },
            Section::Library => ContentFor::Library(self.library_segment),
            _ => return None,
        })
    }

    /// The caption of the list on screen, when the binary sent one for it.
    pub fn list_caption(&self) -> Option<&str> {
        let (view, text) = self.caption.as_ref()?;
        (self.content_view().as_ref() == Some(view)).then_some(text.as_str())
    }

    /// The caption for a list, taken while that list is the one on screen. An empty one clears
    /// it.
    pub(super) fn apply_caption(&mut self, view: ContentFor, text: String) {
        if self.content_view().as_ref() == Some(&view) {
            self.caption = (!text.is_empty()).then_some((view, text));
        }
    }

    /// A list for a view, taken only while that view is on screen: see [`Self::content_view`].
    pub(super) fn apply_content(&mut self, view: ContentFor, content: ContentList) {
        if self.content_view().as_ref() != Some(&view) {
            return;
        }
        // A caption is sent just before the list it describes; one left over from
        // another list describes something no longer on screen.
        if self.caption.as_ref().is_some_and(|(captioned, _)| *captioned != view) {
            self.caption = None;
        }
        self.content = content;
        // A Library pull is the tracker's own answer for the titles it lists, and it
        // outranks one given here earlier.
        if matches!(view, ContentFor::Library(_)) {
            for entry in self.content.entries().iter().filter(|e| e.list_status.is_some()) {
                self.statuses_set.remove(&entry.id);
            }
        }
        // Selection must stay in range or the next render would index past the end.
        let len = self.content.len();
        let cursor = self.cursors.section_mut(self.nav.section());
        cursor.selected = cursor.selected.min(len.saturating_sub(1));
        if len == 0 {
            *cursor = Cursor::default();
        }
        // The calendar is a timeline running from a week ago to a week ahead, so landing
        // on row nought would open it on the *oldest* episode in view. Today is the
        // useful place to be: the window opens at today's first entry — everything that
        // already aired today in view above — with the selection on the next thing to
        // happen. Capped so a very broadcast-heavy day cannot push the selection below
        // a short terminal's fold.
        if self.nav.section() == Section::Calendar
            && let ContentList::Entries(entries) = &self.content
            && let Some(next) =
                entries.iter().position(|e| e.airing_in.is_some_and(|secs| secs > 0))
        {
            let today = self.clock.today();
            let start_of_today = entries
                .iter()
                .position(|e| {
                    e.airing_in.is_some_and(|secs| {
                        air_date(&self.clock, secs).is_some_and(|d| d >= today)
                    })
                })
                .unwrap_or(next);
            self.cursors.calendar = Cursor {
                selected: next,
                offset: next.saturating_sub((next - start_of_today.min(next)).min(12)),
            };
        }
    }

    /// When each title's latest episode aired, and which episode it was.
    pub(super) fn apply_last_aired(&mut self, rows: Vec<TitleAired>) {
        // Applied to both the list and any open detail, since the same title can be
        // on screen twice and a line that appears in one place but not the other
        // reads as a bug.
        if let ContentList::Entries(entries) = &mut self.content {
            for entry in entries.iter_mut() {
                if let Some(row) = rows.iter().find(|row| row.id == entry.id) {
                    entry.last_aired = Some(row.aired);
                }
            }
        }
        if let Some(detail) = &mut self.detail
            && let Some(row) = rows.iter().find(|row| row.id == detail.id)
        {
            detail.last_aired = Some(row.aired);
        }
    }

    /// An answer for what is on screen right now: the list for the current view, the episodes
    /// of the open title, the releases for the open picker.
    ///
    /// For callers that hold the app while they fill it — the preview, tests — and so cannot be
    /// answering a request the user has since left. Background work tags its answer with what
    /// it was asked for instead. An answer with nothing on screen to take it (no list view, no
    /// open title) is tagged so the reducer drops it, exactly as it would a late one.
    pub fn content_here(&self, content: ContentList) -> Event {
        // With no list view on screen the reducer drops every tag, so which one is moot.
        let view = self.content_view().unwrap_or(ContentFor::Continue);
        Event::Content { view, content }
    }

    /// Work needed to populate the current view.
    pub fn reload(&mut self) -> Option<Command> {
        // Whatever is fetched next replaces the content, so the calendar's paging describes a
        // list that is about to stop existing. Leaving it set would let a scroll on the new
        // screen ask for page four of a timeline nobody is looking at any more.
        self.calendar_anchor = None;
        self.calendar_pages = 0;
        self.calendar_more_past = false;
        self.calendar_loading_past = false;

        let task = match self.nav.section() {
            Section::Home => Command::LoadContinue,
            Section::Seasonal => Command::LoadSeasonal,
            Section::Calendar => Command::LoadCalendar,
            Section::Search if !self.search_query().is_blank() => {
                // Recorded as submitted, the same as Enter does, because the answer is tagged
                // with this query and only taken while it is still the one on screen.
                let query = self.search_query();
                self.searched_query = query.clone();
                self.search_pages = 0;
                self.search_more = false;
                self.search_loading_more = false;
                Command::Search { query }
            }
            Section::Providers => Command::CheckProviders,
            Section::Downloads => Command::LoadDownloads,
            Section::Library => Command::LoadLibrary(self.library_segment),
            // Sections with nothing to fetch yet render their own placeholder.
            _ => {
                self.content = ContentList::Empty;
                return None;
            }
        };
        // `content` is the section's list, so a section without one — or a search with nothing
        // typed — empties it rather than leaving the last section's rows to be drawn, scrolled
        // and acted on under its heading.
        self.content =
            if self.fetches_list() { ContentList::Loading } else { ContentList::Empty };
        Some(task)
    }

    /// The section's list, asked for again.
    pub(super) fn refresh_section(&mut self) -> Option<Command> {
        // Asking again means the covers too: one that failed while the network was
        // down should not wait out its backoff once you have said it is back.
        self.images.requests().retry_failed();
        self.reload()
    }

    /// Whether the section on screen fills `content` with a list it is about to ask for.
    fn fetches_list(&self) -> bool {
        match self.nav.section() {
            Section::Home | Section::Seasonal | Section::Calendar | Section::Library => true,
            Section::Search => !self.search_query().is_blank(),
            _ => false,
        }
    }
}

/// The filters overlay's row labels, in order. The cycling and display functions index against
/// this.
///
/// The overlay's rather than the query's own: which rows there are, the words a value is shown in
/// and the ladder each one steps through are how this screen offers a search. The native apps
/// offer the rest of the query's filters.
pub fn search_filter_labels() -> [String; 6] {
    [
        t::lists_filter_genre(),
        t::lists_filter_year(),
        t::lists_filter_season(),
        t::lists_filter_format(),
        t::lists_filter_status(),
        t::lists_filter_sort(),
    ]
}

/// How many rows the filters overlay has.
const SEARCH_FILTER_ROW_COUNT: usize = 6;

const SEARCH_SEASONS: [Season; 4] =
    [Season::Winter, Season::Spring, Season::Summer, Season::Fall];

// Short-form TV, a few minutes an episode, is its own format on AniList, so a filter without it
// had no way to find one.
const SEARCH_FORMATS: [MediaFormat; 7] = [
    MediaFormat::Tv,
    MediaFormat::TvShort,
    MediaFormat::Movie,
    MediaFormat::Ova,
    MediaFormat::Ona,
    MediaFormat::Special,
    MediaFormat::Music,
];

const SEARCH_STATUSES: [anistream_model::catalog::AiringStatus; 3] = {
    use anistream_model::catalog::AiringStatus as S;
    [S::Releasing, S::Finished, S::NotYetReleased]
};

const SEARCH_SORTS: [SearchSort; 4] =
    [SearchSort::Popularity, SearchSort::Score, SearchSort::Trending, SearchSort::Newest];

fn season_word(season: Season) -> String {
    match season {
        Season::Winter => t::lists_season_winter(),
        Season::Spring => t::lists_season_spring(),
        Season::Summer => t::lists_season_summer(),
        Season::Fall => t::lists_season_fall(),
    }
}

fn format_word(format: MediaFormat) -> String {
    match format {
        MediaFormat::Tv => t::lists_format_tv(),
        MediaFormat::TvShort => t::lists_format_tv_short(),
        MediaFormat::Movie => t::lists_format_movie(),
        MediaFormat::Ova => t::lists_format_ova(),
        MediaFormat::Ona => t::lists_format_ona(),
        MediaFormat::Special => t::lists_format_special(),
        MediaFormat::Music => t::lists_format_music(),
    }
}

fn status_word(status: anistream_model::catalog::AiringStatus) -> Option<String> {
    use anistream_model::catalog::AiringStatus as S;
    match status {
        S::Releasing => Some(t::lists_status_airing()),
        S::Finished => Some(t::lists_status_finished()),
        S::NotYetReleased => Some(t::lists_status_upcoming()),
        _ => None,
    }
}

fn sort_word(sort: SearchSort) -> Option<String> {
    match sort {
        SearchSort::Popularity => Some(t::lists_sort_popularity()),
        SearchSort::Score => Some(t::lists_sort_score()),
        SearchSort::Trending => Some(t::lists_sort_trending()),
        SearchSort::Newest => Some(t::lists_sort_newest()),
        _ => None,
    }
}

/// Older than any TV anime that streams anywhere; the year cycle stops here.
const OLDEST_SEARCH_YEAR: u32 = 1970;

/// Rows from the end of the results at which the next page is asked for.
const SEARCH_MORE_MARGIN: usize = 8;

/// What a filter row currently shows: the display form, or "any" when unset.
pub fn search_filter_value(filter: &SearchQuery, row: usize) -> String {
    match row {
        // A genre is a name, and keeps the casing it has everywhere else it is shown.
        0 => filter.genres.first().cloned(),
        1 => filter.years.single().map(|y| y.to_string()),
        2 => filter.season.map(season_word),
        3 => filter.formats.first().copied().map(format_word),
        4 => filter.statuses.first().copied().and_then(status_word),
        5 => {
            return filter.sort.and_then(sort_word).unwrap_or_else(t::lists_sort_relevance);
        }
        _ => None,
    }
    .unwrap_or_else(t::lists_filter_any)
}

/// The set filter values joined for the search prompt line, empty when nothing is.
pub fn search_filter_summary(filter: &SearchQuery) -> String {
    (0..SEARCH_FILTER_ROW_COUNT)
        .filter(|row| {
            // "relevance" is the resting state of sort, not a choice worth announcing.
            match row {
                0 => !filter.genres.is_empty(),
                1 => filter.years.single().is_some(),
                2 => filter.season.is_some(),
                3 => !filter.formats.is_empty(),
                4 => !filter.statuses.is_empty(),
                _ => filter.sort.is_some(),
            }
        })
        .map(|row| search_filter_value(filter, row))
        .collect::<Vec<_>>()
        .join(" · ")
}

/// Step a filter row's value. Forward walks the options and comes back around to "any",
/// so both directions reach everything and nothing is a dead end. The year ladder tops out
/// at next year by `clock`.
pub fn cycle_search_filter(filter: &mut SearchQuery, row: usize, forward: bool, clock: &Clock) {
    fn step<T: PartialEq + Copy>(current: Option<T>, values: &[T], forward: bool) -> Option<T> {
        let at = current.and_then(|c| values.iter().position(|v| *v == c));
        let next = match (at, forward) {
            (None, true) => Some(0),
            (None, false) => Some(values.len() - 1),
            (Some(i), true) => (i + 1 < values.len()).then_some(i + 1),
            (Some(0), false) => None,
            (Some(i), false) => Some(i - 1),
        };
        next.map(|i| values[i])
    }
    match row {
        0 => {
            let genres: Vec<&str> = anistream_model::search::GENRES.to_vec();
            let current = filter.genres.first().map(String::as_str);
            filter.genres =
                step(current, &genres, forward).map(|g| vec![g.to_owned()]).unwrap_or_default();
        }
        1 => {
            let latest = chrono::Datelike::year(&clock.today()).max(0) as u32 + 1;
            let year = match (filter.years.single(), forward) {
                (None, true) => Some(latest),
                (None, false) => Some(OLDEST_SEARCH_YEAR),
                (Some(y), true) => (y > OLDEST_SEARCH_YEAR).then(|| y - 1),
                (Some(y), false) => (y < latest).then(|| y + 1),
            };
            filter.years = anistream_model::search::Bounds { min: year, max: year };
        }
        2 => filter.season = step(filter.season, &SEARCH_SEASONS, forward),
        3 => {
            let next = step(filter.formats.first().copied(), &SEARCH_FORMATS, forward);
            filter.formats = next.into_iter().collect();
        }
        4 => {
            let next = step(filter.statuses.first().copied(), &SEARCH_STATUSES, forward);
            filter.statuses = next.into_iter().collect();
        }
        _ => filter.sort = step(filter.sort, &SEARCH_SORTS, forward),
    }
}

#[cfg(test)]
mod tests {
    use anistream_core::ids::AnilistId;
    use anistream_model::progress::TitleProgress;

    use super::*;
    use crate::app::fixtures::*;

    /// The anchor a test calendar is paged against. Any fixed instant will do — the point is
    /// that it does not move between pages.
    const CALENDAR_TEST_ANCHOR: i64 = 1_700_000_000;

    /// A calendar on screen with three pages of history behind it and more to be had.
    fn calendar_loaded() -> App {
        let mut a = app();
        let _ = a.go_to_section(Section::Calendar);
        a.nav.focus_stage();
        a.apply(Event::CalendarWindow {
            anchor: CALENDAR_TEST_ANCHOR,
            pages: 3,
            more_past: true,
        });
        a.apply(a.content_here(entries(150)));
        a
    }

    #[test]
    fn scrolling_into_the_calendars_past_fetches_more_of_it() {
        let mut a = calendar_loaded();

        // Deep in the list there is nothing to do — every row above is already loaded.
        a.set_cursor(80);
        a.set_offset(61);
        a.handle(Action::Up, 20);
        assert!(
            a.take_commands().is_empty(),
            "a scroll through rows already loaded must not spend a request"
        );

        // Within a screenful of the top, the page behind is asked for — early enough that
        // there are still rows left to scroll through while it arrives.
        a.set_cursor(CALENDAR_BACKFILL_MARGIN + 1);
        a.handle(Action::Up, 20);
        assert_eq!(
            a.take_commands(),
            [Command::LoadCalendarPast { anchor: CALENDAR_TEST_ANCHOR, page: 4 }],
            "nearing the top should reach for the page behind the ones loaded"
        );

        // And exactly once. A held arrow key asking per keystroke would empty a
        // 30-per-minute budget answering one question.
        for _ in 0..5 {
            a.handle(Action::Up, 20);
            assert!(a.take_commands().is_empty(), "a held arrow must not re-request page four");
        }
    }

    #[test]
    fn backfilled_broadcasts_land_above_the_viewport_without_moving_it() {
        let mut a = calendar_loaded();
        a.set_cursor(8);
        a.set_offset(4);

        let older: Vec<Entry> = (0..50)
            .map(|i| Entry::new(AnilistId::new(900 + i), format!("Older {i}")))
            .collect();
        a.apply(Event::CalendarBackfill { page: Some(4), entries: older, more_past: true });

        assert_eq!(a.content.len(), 200, "the older page should have extended the timeline");
        assert_eq!(
            a.content.entries()[0].title,
            "Older 0",
            "older broadcasts belong above — the list reads downward through time"
        );
        // The whole point. Rows arrived above the viewport, so the row under the cursor and
        // its position on screen are both exactly where the user left them.
        assert_eq!(a.cursor(), 58, "the selection must still be on the same broadcast");
        assert_eq!(a.offset(), 54, "and that broadcast must still be in the same place");
        assert_eq!(a.content.entries()[a.cursor()].title, "Title 8");
    }

    #[test]
    fn a_backfill_that_arrives_after_the_user_has_left_is_dropped() {
        let mut a = calendar_loaded();
        let _ = a.go_to_section(Section::Seasonal);
        a.apply(a.content_here(entries(20)));

        a.apply(Event::CalendarBackfill {
            page: Some(4),
            entries: vec![Entry::new(AnilistId::new(999), "Older")],
            more_past: true,
        });

        assert_eq!(
            a.content.len(),
            20,
            "a late backfill must not splice broadcasts into whatever screen is up now"
        );
    }

    #[test]
    fn a_failed_backfill_leaves_its_page_to_be_tried_again() {
        let mut a = calendar_loaded();
        a.set_cursor(CALENDAR_BACKFILL_MARGIN + 1);
        a.handle(Action::Up, 20);
        assert!(!a.take_commands().is_empty(), "the scroll should have asked for page four");

        a.apply(Event::CalendarBackfill { page: None, entries: Vec::new(), more_past: true });
        assert_eq!(a.content.len(), 150, "a failed backfill must not disturb the list");

        a.handle(Action::Up, 20);
        assert_eq!(
            a.take_commands(),
            [Command::LoadCalendarPast { anchor: CALENDAR_TEST_ANCHOR, page: 4 }],
            "the page that failed is the page still missing"
        );
    }

    #[test]
    fn the_calendar_stops_asking_once_its_history_runs_out() {
        let mut a = calendar_loaded();
        a.apply(Event::CalendarBackfill {
            page: Some(4),
            entries: Vec::new(),
            more_past: false,
        });

        a.set_cursor(2);
        a.handle(Action::Up, 20);
        assert!(
            a.take_commands().is_empty(),
            "AniList said there is nothing older; asking again is a wasted request"
        );
    }

    #[test]
    fn leaving_the_calendar_forgets_where_its_pages_were() {
        let mut a = calendar_loaded();
        let _ = a.go_to_section(Section::Seasonal);
        let _ = a.go_to_section(Section::Calendar);
        a.nav.focus_stage();
        a.apply(a.content_here(entries(150)));

        a.set_cursor(2);
        a.handle(Action::Up, 20);
        assert!(
            a.take_commands().is_empty(),
            "page four of the previous timeline is not worth a request against the new one"
        );
    }

    /// What the reducer asks for without a key waits in a queue, in the order it was asked for,
    /// and nothing put there displaces what is already waiting: a backfill, then the reload of
    /// the section the rail was stepped to after it, both go out. In one slot the reload took
    /// the backfill's place, which the loop never let happen only because it took the slot
    /// between the two.
    #[test]
    fn work_asked_for_without_a_key_waits_in_order_and_displaces_nothing() {
        let mut a = calendar_loaded();
        a.set_cursor(CALENDAR_BACKFILL_MARGIN + 1);
        a.handle(Action::Up, 20);
        a.nav.focus_rail();
        a.handle(Action::Down, 20);
        assert_eq!(a.nav.section(), Section::Seasonal);
        // The rail settles, and the section's list is asked for from the idle tick.
        for _ in 0..=RELOAD_IDLE_TICKS {
            a.tick_toasts();
        }
        assert_eq!(
            a.take_commands(),
            [
                Command::LoadCalendarPast { anchor: CALENDAR_TEST_ANCHOR, page: 4 },
                Command::LoadSeasonal
            ]
        );
        assert!(a.take_commands().is_empty(), "taken once, and gone");
    }

    #[test]
    fn enter_on_a_resumable_continue_row_plays_immediately() {
        let mut a = app();
        let mut e = Entry::new(AnilistId::new(5), "Hoshikuzu");
        e.progress = Some(TitleProgress { done: 7, next: 8 });
        e.resume = Some(ResumePoint { position: 600.0, fraction: Some(0.5) });
        a.apply(a.content_here(ContentList::Entries(vec![e])));
        a.nav.focus_stage();

        let task = a.handle(Action::Open, 20).expect("enter must resume");
        assert_eq!(
            task,
            Command::play(AnilistId::new(5), "8"),
            "the section is named Continue; Enter continues"
        );
        assert!(a.playing.is_some());
    }

    #[test]
    fn the_format_filter_can_find_short_tv() {
        let mut filter = SearchQuery::default();
        let clock = Clock::system();
        cycle_search_filter(&mut filter, 3, true, &clock);
        assert_eq!(search_filter_value(&filter, 3), "TV");
        cycle_search_filter(&mut filter, 3, true, &clock);
        assert_eq!(filter.formats, [MediaFormat::TvShort]);
        assert_eq!(search_filter_value(&filter, 3), "TV short");
    }

    #[test]
    fn a_genre_filter_keeps_the_genre_s_own_casing() {
        let filter = SearchQuery { genres: vec!["Slice of Life".into()], ..Default::default() };
        assert_eq!(search_filter_value(&filter, 0), "Slice of Life");
    }

    #[test]
    fn x_on_home_dismisses_the_selected_title_from_continue() {
        let mut a = app();
        a.apply(a.content_here(entries(3)));
        a.set_cursor(1);
        let task = a.handle(Action::HideFromContinue, 20);
        assert_eq!(task, Some(Command::HideFromContinue { id: AnilistId::new(2) }));
        // Gone from the screen before the store is even asked.
        assert_eq!(a.content.len(), 2);
        assert!(a.content.entries().iter().all(|e| e.id != AnilistId::new(2)));
    }

    #[test]
    fn dismissing_from_continue_only_works_on_the_home_list() {
        // A pushed episode table keeps the Home entries as `content`; the key must not
        // remove a show the viewer cannot see, least of all the one they are looking into.
        let mut a = app_at_episodes();
        assert_eq!(a.handle(Action::HideFromContinue, 20), None);
        assert_eq!(a.content.len(), 3, "a row vanished from a list not on screen");
    }

    #[test]
    fn stepping_a_library_segment_reloads_and_resets_the_selection() {
        // A stale selection index into a different list would point at the wrong title.
        let mut a = app();
        a.go_to_section(Section::Library);
        a.apply(a.content_here(entries(10)));
        a.set_cursor(7);
        a.nav.focus_stage();

        let task = a.handle(Action::Right, 20);
        assert_eq!(a.library_segment, LibrarySegment::Planning);
        assert_eq!(task, Some(Command::LoadLibrary(LibrarySegment::Planning)));
        assert_eq!(a.cursor(), 0);
    }

    #[test]
    fn horizontal_keys_still_move_focus_outside_the_library() {
        // The segment behaviour is Library-specific; everywhere else h/l crosses the divider.
        let mut a = app();
        a.apply(a.content_here(entries(3)));
        a.nav.focus_stage();
        a.handle(Action::Left, 20);
        assert_eq!(a.nav.focus(), Focus::Rail);
    }

    #[test]
    fn f_on_a_title_opened_from_search_means_what_it_means_on_any_title() {
        let mut a = app();
        a.handle(Action::JumpSection(4), 5);
        type_text(&mut a, "query");
        assert!(matches!(a.handle(Action::Open, 5), Some(Command::Search { .. })));
        a.apply(a.content_here(entries(3)));
        a.handle(Action::Open, 5);
        assert_eq!(a.nav.current(), &StageView::Title(AnilistId::new(1)));

        a.handle(Action::Filter, 5);
        assert_eq!(a.nav.overlay(), None, "the search filters opened over a title page");
    }

    #[test]
    fn search_with_nothing_typed_shows_its_own_empty_state() {
        // Jumped to, it showed the previous section's list; walked to on the rail, a pulse
        // that never ended, because the fetch that would end it never went out.
        let mut a = home_list(5);
        assert_eq!(a.handle(Action::JumpSection(4), 5), None);
        assert_eq!(a.content, ContentList::Empty);
        assert!(a.target().is_none());

        let mut a = home_list(5);
        a.handle(Action::JumpSection(7), 5);
        a.nav.focus_rail();
        for _ in 0..3 {
            a.handle(Action::Up, 5);
        }
        assert_eq!(a.nav.section(), Section::Search);
        for _ in 0..=RELOAD_IDLE_TICKS {
            a.tick_pending_reload();
        }
        assert_eq!(a.content, ContentList::Empty);
        assert!(a.take_commands().is_empty());
    }

    #[test]
    fn down_from_the_search_field_moves_into_results_that_scroll() {
        let mut a = app();
        a.handle(Action::JumpSection(4), 5);
        type_text(&mut a, "query");
        a.handle(Action::Open, 5);
        a.apply(a.content_here(entries(40)));
        assert!(a.is_typing());

        a.handle(Action::Down, 5);
        assert!(!a.is_typing(), "the results have the keyboard");
        assert_eq!(a.cursor(), 0);
        for _ in 0..12 {
            a.handle(Action::Down, 5);
        }
        assert_eq!(a.cursor(), 12);
        assert!(a.offset() > 0 && a.cursor() < a.offset() + 5, "the window follows");
        a.handle(Action::Bottom, 5);
        a.handle(Action::Down, 5);
        assert_eq!(a.cursor(), 39, "bounded at the last result");
        // A result is a list row like any other.
        assert_eq!(
            a.handle(Action::Download, 5),
            Some(Command::DownloadEpisode { id: AnilistId::new(40), episode: "1".into() })
        );

        // Back to typing: Esc, ↑ from the first result, or `/`, which does not ask again.
        a.handle(Action::Back, 5);
        assert!(a.is_typing());
        a.handle(Action::Down, 5);
        a.handle(Action::Top, 5);
        a.handle(Action::Up, 5);
        assert!(a.is_typing());
        a.handle(Action::Down, 5);
        assert_eq!(a.handle(Action::FocusSearch, 5), None);
        assert!(a.is_typing());
        assert_eq!(a.content.len(), 40);
    }

    #[test]
    fn enter_opens_a_search_result_instead_of_re_running_the_search() {
        // Reported from real use: it was impossible to open a result. Enter submitted the query
        // every time, so a highlighted row could never be entered.
        let mut a = app();
        let task = a.handle(Action::FocusSearch, 10);
        assert_eq!(task, None, "an empty query has nothing to submit");
        for ch in "hoshikuzu".chars() {
            a.type_char(ch);
        }

        // First Enter runs it.
        assert_eq!(
            a.handle(Action::Open, 10),
            Some(Command::Search { query: SearchQuery::text("hoshikuzu") })
        );
        a.apply(a.content_here(entries(3)));

        // Second Enter opens the highlighted result rather than asking again.
        let task = a.handle(Action::Open, 10);
        assert!(matches!(task, Some(Command::LoadDetail(_))), "got {task:?}");
        assert!(matches!(a.nav.current(), StageView::Title(_)));

        // Editing the query makes it submittable again.
        a.handle(Action::Back, 10);
        a.type_char('x');
        assert_eq!(
            a.handle(Action::Open, 10),
            Some(Command::Search { query: SearchQuery::text("hoshikuzux") })
        );
    }

    #[test]
    fn search_filters_cycle_and_apply_without_a_query() {
        let mut a = app();
        a.handle(Action::FilterSearch, 10);
        assert_eq!(a.nav.section(), Section::Search, "filters land on the search screen");
        assert_eq!(a.nav.overlay(), Some(&Overlay::SearchFilters));

        // Right on the genre row picks the first genre; a filter alone is a browse.
        a.handle(Action::Right, 10);
        assert_eq!(a.search_filter.genres, ["Action"]);
        let task = a.handle(Action::Open, 10);
        match task {
            Some(Command::Search { query }) => {
                assert_eq!(query.text, "");
                assert_eq!(query.genres, ["Action"]);
            }
            other => panic!("expected a search, got {other:?}"),
        }
        assert!(!a.nav.has_overlay());
    }

    #[test]
    fn search_filters_step_back_to_any_rather_than_sticking() {
        let mut a = app();
        a.handle(Action::FilterSearch, 10);

        a.handle(Action::Right, 10);
        a.handle(Action::Left, 10);
        assert!(a.search_filter.genres.is_empty(), "left from the first option is back to any");

        // Both empty: Enter closes the overlay and browses, most popular first.
        assert_eq!(
            a.handle(Action::Open, 10),
            Some(Command::Search { query: SearchQuery::default() })
        );
        assert!(!a.nav.has_overlay());
    }

    #[test]
    fn search_filters_open_mid_query_without_stealing_the_letters() {
        // ctrl+f is the whole reason FilterSearch is its own action: `f` belongs to the
        // query text while the search box has focus.
        let mut a = app();
        a.handle(Action::FocusSearch, 10);
        for ch in "mecha".chars() {
            a.type_char(ch);
        }
        assert!(a.is_typing());
        a.handle(Action::FilterSearch, 10);
        assert_eq!(a.nav.overlay(), Some(&Overlay::SearchFilters));
        assert_eq!(a.text(Field::Search), "mecha", "the query survives opening the overlay");
    }

    #[test]
    fn the_filter_key_outside_search_and_episodes_says_where_filters_live() {
        let mut a = app();
        a.handle(Action::Filter, 10);
        assert!(!a.nav.has_overlay());
        assert!(
            a.toasts
                .iter()
                .any(|t| t.notice.text == anistream_i18n::t::toast_filters_elsewhere())
        );
    }

    #[test]
    fn opening_an_entry_pushes_the_title_view_and_asks_for_detail() {
        let mut a = app();
        a.apply(a.content_here(entries(3)));
        a.nav.focus_stage();
        let task = a.handle(Action::Open, 10);
        assert_eq!(task, Some(Command::LoadDetail(AnilistId::new(1))));
        assert_eq!(a.nav.current(), &StageView::Title(AnilistId::new(1)));
        // The already-known entry is shown immediately so the screen is never blank while
        // the detail request is in flight.
        assert!(a.detail.is_some());
    }

    #[test]
    fn switching_section_clears_stale_selection_and_detail() {
        let mut a = app();
        a.apply(a.content_here(entries(10)));
        a.handle(Action::Bottom, 5);
        a.handle(Action::Open, 5);

        a.go_to_section(Section::Seasonal);
        assert_eq!(a.cursor(), 0);
        assert!(a.detail.is_none(), "stale detail from another section");
        assert_eq!(a.nav.depth(), 1);
    }

    // Requests are never cancelled, so every answer below is one the app really receives when
    // the user moves faster than the network: it has to be recognised as late and left alone.

    #[test]
    fn a_late_list_for_the_section_just_left_is_not_shown_under_the_next() {
        let mut a = app();
        let _ = a.go_to_section(Section::Seasonal);
        let _ = a.go_to_section(Section::Calendar);
        a.apply(Event::Content { view: ContentFor::Seasonal, content: entries(5) });
        assert!(
            matches!(a.content, ContentList::Loading),
            "the calendar showed seasonal's list"
        );

        a.apply(Event::Content { view: ContentFor::Calendar, content: entries(3) });
        assert_eq!(a.content.len(), 3, "the calendar's own answer must still land");
    }

    #[test]
    fn a_slow_answer_to_an_earlier_search_does_not_replace_a_later_one() {
        let mut a = app();
        let _ = a.go_to_section(Section::Search);
        a.nav.focus_stage();
        for query in ["hos", "hoshikuzu"] {
            a.set_text(Field::Search, query.into());
            assert!(matches!(a.handle(Action::Open, 20), Some(Command::Search { .. })));
        }
        let answer = |query: &str, n| Event::Content {
            view: ContentFor::Search { query: SearchQuery::text(query) },
            content: entries(n),
        };
        a.apply(answer("hoshikuzu", 2));
        a.apply(answer("fri", 9));
        assert_eq!(
            a.content.len(),
            2,
            "the results for \"hos\" replaced those for \"hoshikuzu\""
        );
    }

    #[test]
    fn a_late_list_for_another_library_segment_is_not_shown() {
        let mut a = app();
        let _ = a.go_to_section(Section::Library);
        a.library_segment = LibrarySegment::Completed;
        a.apply(Event::Content {
            view: ContentFor::Library(LibrarySegment::Watching),
            content: entries(4),
        });
        assert!(a.content.is_empty(), "completed showed the watching list");
    }

    #[test]
    fn each_browsable_section_requests_its_own_data() {
        let mut a = app();
        assert_eq!(a.go_to_section(Section::Home), Some(Command::LoadContinue));
        assert_eq!(a.go_to_section(Section::Seasonal), Some(Command::LoadSeasonal));
        assert_eq!(a.go_to_section(Section::Calendar), Some(Command::LoadCalendar));
        // Search with no query has nothing to fetch yet.
        assert_eq!(a.go_to_section(Section::Search), None);
        // Sections with no remote data render a placeholder rather than spinning.
        assert_eq!(a.go_to_section(Section::Settings), None);
    }

    #[test]
    fn loading_state_is_set_before_the_request_goes_out() {
        // The screen must never look empty while data is in flight — an empty list with no
        // explanation is the failure mode this whole design avoids.
        let mut a = app();
        a.apply(a.content_here(entries(3)));
        a.go_to_section(Section::Home);
        assert_eq!(a.content, ContentList::Loading);
    }

    #[test]
    fn a_failure_is_content_rather_than_an_empty_list() {
        let mut a = app();
        a.apply(a.content_here(ContentList::Failed("anilist unreachable".into())));
        assert!(matches!(a.content, ContentList::Failed(_)));
        assert!(a.content.is_empty());
    }
}
